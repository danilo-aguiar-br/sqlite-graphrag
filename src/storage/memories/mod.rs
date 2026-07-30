//! Persistence layer for the `memories` table and its vector companion.
//!
//! Functions here encapsulate every SQL statement touching `memories`,
//! `memory_embeddings` and the FTS5 `fts_memories` shadow table. Callers receive
//! typed [`MemoryRow`] or [`NewMemory`] values and never build SQL strings.

mod soft_delete;
pub use soft_delete::{
    clear_deleted_at, find_by_name_any_state, list_deleted_before, soft_delete,
};

use crate::embedder::f32_to_bytes;
use crate::errors::AppError;
use crate::storage::utils::with_busy_retry;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Input payload for inserting or updating a memory.
///
/// `body_hash` must be the BLAKE3 digest of `body`. The `metadata` field is
/// stored as a TEXT column containing JSON.
#[derive(Debug, Serialize, Deserialize)]
pub struct NewMemory {
    /// Namespace scope.
    pub namespace: String,
    /// Name of this item.
    pub name: String,
    /// Memory type classification.
    pub memory_type: String,
    /// Human-readable description.
    pub description: String,
    /// Full text body.
    pub body: String,
    /// Body hash.
    pub body_hash: String,
    /// Session ID.
    pub session_id: Option<String>,
    /// Source side of the relationship.
    pub source: String,
    /// Arbitrary metadata.
    pub metadata: serde_json::Value,
}

/// Fully materialized row from the `memories` table.
///
/// Returned by [`read_by_name`], [`read_full`], [`list`] and [`fts_search`].
/// The `metadata` field is kept as a JSON string to avoid double parsing.
#[derive(Debug, Serialize)]
pub struct MemoryRow {
    /// Unique identifier.
    pub id: i64,
    /// Namespace scope.
    pub namespace: String,
    /// Name of this item.
    pub name: String,
    /// Memory type classification.
    pub memory_type: String,
    /// Human-readable description.
    pub description: String,
    /// Full text body.
    pub body: String,
    /// Body hash.
    pub body_hash: String,
    /// Session ID.
    pub session_id: Option<String>,
    /// Source side of the relationship.
    pub source: String,
    /// Arbitrary metadata.
    pub metadata: String,
    /// Creation timestamp.
    pub created_at: i64,
    /// Last-update timestamp.
    pub updated_at: i64,
    /// Unix epoch when the memory was soft-deleted, or `None` for active memories.
    /// Surfaced in `list --include-deleted --json` so LLM consumers can distinguish
    /// active from soft-deleted rows without a second SQL query (v1.0.37 H7+M9 fix).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
}

/// Finds a live memory by `(namespace, name)` and returns key metadata.
///
/// # Arguments
///
/// - `conn` — open SQLite connection configured with the project pragmas.
/// - `namespace` — resolved namespace for the lookup.
/// - `name` — kebab-case memory name.
///
/// # Returns
///
/// `Ok(Some((id, updated_at, max_version)))` when the memory exists and is
/// not soft-deleted, `Ok(None)` otherwise.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn find_by_name(
    conn: &Connection,
    namespace: &str,
    name: &str,
) -> Result<Option<(i64, i64, i64)>, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT m.id, m.updated_at, COALESCE(MAX(v.version), 0)
         FROM memories m
         LEFT JOIN memory_versions v ON v.memory_id = m.id
         WHERE m.namespace = ?1 AND m.name = ?2 AND m.deleted_at IS NULL
         GROUP BY m.id",
    )?;
    let result = stmt.query_row(params![namespace, name], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
        ))
    });
    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e)),
    }
}


/// Looks up a live memory by exact `body_hash` within a namespace.
///
/// Used during `remember` to short-circuit semantic duplicates before
/// spending an embedding call.
///
/// # Returns
///
/// `Ok(Some(id))` when a live memory with the same hash exists,
/// `Ok(None)` otherwise.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn find_by_hash(
    conn: &Connection,
    namespace: &str,
    body_hash: &str,
) -> Result<Option<i64>, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id FROM memories WHERE namespace = ?1 AND body_hash = ?2 AND deleted_at IS NULL",
    )?;
    match stmt.query_row(params![namespace, body_hash], |r| r.get(0)) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e)),
    }
}

/// Inserts a new row into the `memories` table.
///
/// # Arguments
///
/// - `conn` — active SQLite connection, typically inside a transaction.
/// - `m` — validated payload including `body_hash` and serialized metadata.
///
/// # Returns
///
/// The `rowid` assigned to the newly inserted memory.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on insertion failure and
/// `Err(AppError::Json)` if metadata serialization fails.
pub fn insert(conn: &Connection, m: &NewMemory) -> Result<i64, AppError> {
    // G29 Passo 2 (v1.0.69): runtime guard for the CHECK constraint on
    // `source`. Even though `MemorySource` is the typed future, every
    // legacy `NewMemory { source: "..." }` literal still flows through
    // this function; validating here keeps the footgun from regressing
    // for callers that have not yet migrated to the enum.
    let validated_source = crate::memory_source::validate_source(&m.source)?;
    conn.execute(
        "INSERT INTO memories (namespace, name, type, description, body, body_hash, session_id, source, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            m.namespace, m.name, m.memory_type, m.description, m.body,
            m.body_hash, m.session_id, validated_source,
            serde_json::to_string(&m.metadata)?
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Updates an existing memory optionally guarded by optimistic concurrency.
///
/// When `expected_updated_at` is `Some(ts)` the row is only updated if its
/// current `updated_at` equals `ts`. This protects concurrent `edit` calls
/// from silently clobbering each other.
///
/// # Returns
///
/// `Ok(true)` when exactly one row was updated, `Ok(false)` when the
/// optimistic check failed or the memory does not exist.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn update(
    conn: &Connection,
    id: i64,
    m: &NewMemory,
    expected_updated_at: Option<i64>,
) -> Result<bool, AppError> {
    // G29 Passo 2 (v1.0.69): runtime guard for the CHECK constraint on
    // `source`. Mirrors `insert` so `body-enrich` and other mutations
    // cannot reintroduce the historical "enrich" literal that broke
    // `body-enrich` in v1.0.55 - v1.0.68.
    let validated_source = crate::memory_source::validate_source(&m.source)?;
    let affected = if let Some(ts) = expected_updated_at {
        conn.execute(
            "UPDATE memories SET type=?2, description=?3, body=?4, body_hash=?5,
             session_id=?6, source=?7, metadata=?8
             WHERE id=?1 AND updated_at=?9 AND deleted_at IS NULL",
            params![
                id,
                m.memory_type,
                m.description,
                m.body,
                m.body_hash,
                m.session_id,
                validated_source,
                serde_json::to_string(&m.metadata)?,
                ts
            ],
        )?
    } else {
        conn.execute(
            "UPDATE memories SET type=?2, description=?3, body=?4, body_hash=?5,
             session_id=?6, source=?7, metadata=?8
             WHERE id=?1 AND deleted_at IS NULL",
            params![
                id,
                m.memory_type,
                m.description,
                m.body,
                m.body_hash,
                m.session_id,
                validated_source,
                serde_json::to_string(&m.metadata)?
            ],
        )?
    };
    Ok(affected == 1)
}

/// Replaces the vector row for a memory in `memory_embeddings`.
///
/// v1.0.76: sqlite-vec was removed. Embeddings live in a regular BLOB-backed
/// table; cosine similarity is computed in pure Rust on demand. The
/// `memory_type`, `name`, and `snippet` arguments are accepted for API
/// compatibility but are not stored — the FTS5 shadow table is the
/// source of truth for textual metadata.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn upsert_vec(
    conn: &Connection,
    memory_id: i64,
    namespace: &str,
    _memory_type: &str,
    embedding: &[f32],
    _name: &str,
    _snippet: &str,
) -> Result<(), AppError> {
    // v1.1.1 (P1): skip empty vectors so the memory stays visible to the
    // re-embed backfill scanner instead of persisting a vector-less row.
    if embedding.is_empty() {
        tracing::debug!(
            memory_id,
            "empty memory embedding: skipping memory_embeddings row (backfill via enrich re-embed)"
        );
        return Ok(());
    }
    let embedding_bytes = f32_to_bytes(embedding);
    with_busy_retry(|| {
        conn.execute(
            "DELETE FROM memory_embeddings WHERE memory_id = ?1",
            params![memory_id],
        )?;
        conn.execute(
            "INSERT INTO memory_embeddings(memory_id, namespace, embedding, source, model, dim)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                memory_id,
                namespace,
                &embedding_bytes,
                "llm-headless",
                crate::constants::SQLITE_GRAPHRAG_VERSION,
                crate::constants::embedding_dim() as i64,
            ],
        )?;
        Ok(())
    })
}

/// Deletes the vector row for `memory_id` from `memory_embeddings`.
///
/// Called during `forget` and `purge` to keep the embeddings table
/// consistent with the logical state of `memories`. FK CASCADE on
/// `memory_embeddings.memory_id` handles the common case, but this
/// function exists so callers can delete the embedding first
/// (preserving the row in `memories` for audit).
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn delete_vec(conn: &Connection, memory_id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM memory_embeddings WHERE memory_id = ?1",
        params![memory_id],
    )?;
    Ok(())
}

/// Fetches a live memory by `(namespace, name)` and returns all columns.
///
/// # Returns
///
/// `Ok(Some(row))` when found, `Ok(None)` when missing or soft-deleted.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn read_by_name(
    conn: &Connection,
    namespace: &str,
    name: &str,
) -> Result<Option<MemoryRow>, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, namespace, name, type, description, body, body_hash,
                session_id, source, metadata, created_at, updated_at, deleted_at
         FROM memories WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
    )?;
    match stmt.query_row(params![namespace, name], |r| {
        Ok(MemoryRow {
            id: r.get(0)?,
            namespace: r.get(1)?,
            name: r.get(2)?,
            memory_type: r.get(3)?,
            description: r.get(4)?,
            body: r.get(5)?,
            body_hash: r.get(6)?,
            session_id: r.get(7)?,
            source: r.get(8)?,
            metadata: r.get(9)?,
            created_at: r.get(10)?,
            updated_at: r.get(11)?,
            deleted_at: r.get(12)?,
        })
    }) {
        Ok(m) => Ok(Some(m)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e)),
    }
}


/// Lists live memories in a namespace ordered by `updated_at` descending.
///
/// # Arguments
///
/// - `memory_type` — optional filter on the `type` column.
/// - `limit` / `offset` — standard pagination controls in rows.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn list(
    conn: &Connection,
    namespace: &str,
    memory_type: Option<&str>,
    limit: usize,
    offset: usize,
    include_deleted: bool,
) -> Result<Vec<MemoryRow>, AppError> {
    if let Some(mt) = memory_type {
        let sql = if include_deleted {
            "SELECT id, namespace, name, type, description, body, body_hash,
                    session_id, source, metadata, created_at, updated_at, deleted_at
             FROM memories WHERE namespace=?1 AND type=?2
             ORDER BY updated_at DESC LIMIT ?3 OFFSET ?4"
        } else {
            "SELECT id, namespace, name, type, description, body, body_hash,
                    session_id, source, metadata, created_at, updated_at, deleted_at
             FROM memories WHERE namespace=?1 AND type=?2 AND deleted_at IS NULL
             ORDER BY updated_at DESC LIMIT ?3 OFFSET ?4"
        };
        let mut stmt = conn.prepare_cached(sql)?;
        let rows = stmt
            .query_map(params![namespace, mt, limit as i64, offset as i64], |r| {
                Ok(MemoryRow {
                    id: r.get(0)?,
                    namespace: r.get(1)?,
                    name: r.get(2)?,
                    memory_type: r.get(3)?,
                    description: r.get(4)?,
                    body: r.get(5)?,
                    body_hash: r.get(6)?,
                    session_id: r.get(7)?,
                    source: r.get(8)?,
                    metadata: r.get(9)?,
                    created_at: r.get(10)?,
                    updated_at: r.get(11)?,
                    deleted_at: r.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let sql = if include_deleted {
            "SELECT id, namespace, name, type, description, body, body_hash,
                    session_id, source, metadata, created_at, updated_at, deleted_at
             FROM memories WHERE namespace=?1
             ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3"
        } else {
            "SELECT id, namespace, name, type, description, body, body_hash,
                    session_id, source, metadata, created_at, updated_at, deleted_at
             FROM memories WHERE namespace=?1 AND deleted_at IS NULL
             ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3"
        };
        let mut stmt = conn.prepare_cached(sql)?;
        let rows = stmt
            .query_map(params![namespace, limit as i64, offset as i64], |r| {
                Ok(MemoryRow {
                    id: r.get(0)?,
                    namespace: r.get(1)?,
                    name: r.get(2)?,
                    memory_type: r.get(3)?,
                    description: r.get(4)?,
                    body: r.get(5)?,
                    body_hash: r.get(6)?,
                    session_id: r.get(7)?,
                    source: r.get(8)?,
                    metadata: r.get(9)?,
                    created_at: r.get(10)?,
                    updated_at: r.get(11)?,
                    deleted_at: r.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// Count.
pub fn count(
    conn: &Connection,
    namespace: &str,
    memory_type: Option<&str>,
    include_deleted: bool,
) -> Result<usize, AppError> {
    let (sql, params_vec): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match (
        memory_type,
        include_deleted,
    ) {
        (Some(mt), true) => (
            "SELECT COUNT(*) FROM memories WHERE namespace=?1 AND type=?2",
            vec![
                Box::new(namespace.to_string()) as Box<dyn rusqlite::types::ToSql>,
                Box::new(mt.to_string()),
            ],
        ),
        (Some(mt), false) => (
            "SELECT COUNT(*) FROM memories WHERE namespace=?1 AND type=?2 AND deleted_at IS NULL",
            vec![
                Box::new(namespace.to_string()) as Box<dyn rusqlite::types::ToSql>,
                Box::new(mt.to_string()),
            ],
        ),
        (None, true) => (
            "SELECT COUNT(*) FROM memories WHERE namespace=?1",
            vec![Box::new(namespace.to_string()) as Box<dyn rusqlite::types::ToSql>],
        ),
        (None, false) => (
            "SELECT COUNT(*) FROM memories WHERE namespace=?1 AND deleted_at IS NULL",
            vec![Box::new(namespace.to_string()) as Box<dyn rusqlite::types::ToSql>],
        ),
    };
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|b| b.as_ref()).collect();
    let n: i64 = conn.query_row(sql, params_refs.as_slice(), |r| r.get(0))?;
    Ok(n as usize)
}

/// Runs a KNN search over `memory_embeddings`, optionally restricted to namespaces.
///
/// # Arguments
///
/// - `embedding` — query vector of length [`crate::constants::embedding_dim()`].
/// - `namespaces` — namespaces to search. Empty slice means "all namespaces".
/// - `memory_type` — optional filter on the `type` column.
/// - `k` — maximum number of hits to return.
///
/// # Returns
///
/// A vector of `(memory_id, distance)` pairs sorted by ascending distance.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn knn_search(
    conn: &Connection,
    embedding: &[f32],
    namespaces: &[String],
    memory_type: Option<&str>,
    k: usize,
) -> Result<Vec<(i64, f32)>, AppError> {
    if embedding.len() != crate::constants::embedding_dim() {
        return Err(AppError::Embedding(
            crate::i18n::validation::embedding_knn_search_dim_mismatch(
                embedding.len(),
                crate::constants::embedding_dim(),
            ),
        ));
    }
    // v1.0.76: full table scan + in-process cosine similarity. The
    // `memory_embeddings` table no longer has a `distance` column or a
    // `type` column (the namespace/type filters were dropped for the
    // BLOB-backed table — they live on the `memories` table). The
    // cosine result is converted to a "distance" so callers that read
    // `distance` keep working unchanged.

    // Build the SQL once with the namespace IN clause shape.
    let placeholders = (0..namespaces.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = if namespaces.is_empty() {
        "SELECT memory_id, embedding, namespace FROM memory_embeddings".to_string()
    } else {
        format!(
            "SELECT memory_id, embedding, namespace FROM memory_embeddings \
             WHERE namespace IN ({placeholders})"
        )
    };
    let mut stmt = conn.prepare(&sql)?;
    let mut raw_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    for ns in namespaces {
        raw_params.push(Box::new(ns.clone()));
    }
    let param_refs: Vec<&dyn rusqlite::ToSql> = raw_params.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |r| {
        let id: i64 = r.get(0)?;
        let bytes: Vec<u8> = r.get(1)?;
        let ns: String = r.get(2)?;
        Ok((id, bytes, ns))
    })?;

    // Optionally restrict to a memory type by joining against the
    // `memories` table on the fly.
    let type_filter = memory_type.map(|t| t.to_string());
    let mut candidates: Vec<(i64, f32)> = Vec::new();
    for row in rows {
        let (id, bytes, ns) = row?;
        let stored = crate::embedder::bytes_to_f32(&bytes);
        if stored.len() != embedding.len() {
            continue;
        }
        let sim = crate::similarity::cosine_similarity(embedding, &stored);
        let dist = crate::similarity::similarity_to_distance(sim);
        if let Some(mt) = &type_filter {
            // Look up the memory's type via a per-row check. For very
            // large candidate sets this should be batched; for the
            // v1.0.76 default namespace size (<10k memories) the
            // per-row lookup is acceptable.
            let actual: Option<String> = conn
                .query_row(
                    "SELECT type FROM memories WHERE id = ?1",
                    params![id],
                    |r| r.get(0),
                )
                .ok();
            if actual.as_deref() != Some(mt.as_str()) {
                continue;
            }
        }
        let _ = ns; // namespace already filtered at SQL level
        candidates.push((id, dist));
    }
    // Sort by distance ascending (best matches first).
    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(k);
    Ok(candidates)
}

/// Fetches a live memory by `(namespace, name)` and returns all columns.
/// Fetches a live memory by primary key and returns all columns.
///
/// Mirrors [`read_by_name`] but keyed on `rowid` for use after a KNN search.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn read_full(conn: &Connection, memory_id: i64) -> Result<Option<MemoryRow>, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, namespace, name, type, description, body, body_hash,
                session_id, source, metadata, created_at, updated_at, deleted_at
         FROM memories WHERE id=?1 AND deleted_at IS NULL",
    )?;
    match stmt.query_row(params![memory_id], |r| {
        Ok(MemoryRow {
            id: r.get(0)?,
            namespace: r.get(1)?,
            name: r.get(2)?,
            memory_type: r.get(3)?,
            description: r.get(4)?,
            body: r.get(5)?,
            body_hash: r.get(6)?,
            session_id: r.get(7)?,
            source: r.get(8)?,
            metadata: r.get(9)?,
            created_at: r.get(10)?,
            updated_at: r.get(11)?,
            deleted_at: r.get(12)?,
        })
    }) {
        Ok(m) => Ok(Some(m)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e)),
    }
}


/// Preprocesses a raw user query for FTS5 `MATCH`.
///
/// Technical separators (`-`, `.`, `_`, `/`) are treated as word boundaries by
/// the `unicode61` tokenizer.  When the query contains any of these characters
/// the function builds a compound FTS5 expression:
///   1. A phrase query with the separated tokens (exact compound matching).
///   2. Individual prefix terms joined with OR (broader recall).
///
/// Queries without separators keep the original `term*` prefix behaviour.
fn preprocess_fts_query(raw: &str) -> String {
    const SEPARATORS: &[char] = &['-', '.', '_', '/'];
    const FTS5_SYNTAX: &[char] = &['"', '*', '(', ')', '^', ':'];
    const FTS5_KEYWORDS: &[&str] = &["OR", "AND", "NOT", "NEAR"];

    let sanitized: String = raw.chars().filter(|c| !FTS5_SYNTAX.contains(c)).collect();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let is_fts_keyword = |t: &str| FTS5_KEYWORDS.iter().any(|kw| kw.eq_ignore_ascii_case(t));

    if !trimmed.chars().any(|c| SEPARATORS.contains(&c)) {
        return trimmed
            .split_whitespace()
            .filter(|t| !is_fts_keyword(t))
            .map(|t| format!("{t}*"))
            .collect::<Vec<_>>()
            .join(" ");
    }
    let tokens: Vec<&str> = trimmed
        .split(|c: char| SEPARATORS.contains(&c) || c.is_whitespace())
        .filter(|t| !t.is_empty() && !is_fts_keyword(t))
        .collect();
    if tokens.is_empty() {
        return String::new();
    }
    let phrase = format!("\"{}\"", tokens.join(" "));
    let prefix_terms: Vec<String> = tokens.iter().map(|t| format!("{t}*")).collect();
    format!("{phrase} OR {}", prefix_terms.join(" OR "))
}

/// Executes an FTS5 search against `fts_memories` with query preprocessing.
///
/// Technical separators in the query are converted to phrase + prefix OR
/// expressions so compound terms like `graphrag-precompact.sh` match correctly.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn fts_search(
    conn: &Connection,
    query: &str,
    namespace: &str,
    memory_type: Option<&str>,
    limit: usize,
) -> Result<Vec<MemoryRow>, AppError> {
    let fts_query = preprocess_fts_query(query);
    if let Some(mt) = memory_type {
        let mut stmt = conn.prepare_cached(
            "SELECT m.id, m.namespace, m.name, m.type, m.description, m.body, m.body_hash,
                    m.session_id, m.source, m.metadata, m.created_at, m.updated_at, m.deleted_at
             FROM fts_memories fts
             JOIN memories m ON m.id = fts.rowid
             WHERE fts_memories MATCH ?1 AND m.namespace = ?2 AND m.type = ?3 AND m.deleted_at IS NULL
             ORDER BY rank LIMIT ?4",
        )?;
        let rows = stmt
            .query_map(params![fts_query, namespace, mt, limit as i64], |r| {
                Ok(MemoryRow {
                    id: r.get(0)?,
                    namespace: r.get(1)?,
                    name: r.get(2)?,
                    memory_type: r.get(3)?,
                    description: r.get(4)?,
                    body: r.get(5)?,
                    body_hash: r.get(6)?,
                    session_id: r.get(7)?,
                    source: r.get(8)?,
                    metadata: r.get(9)?,
                    created_at: r.get(10)?,
                    updated_at: r.get(11)?,
                    deleted_at: r.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let mut stmt = conn.prepare_cached(
            "SELECT m.id, m.namespace, m.name, m.type, m.description, m.body, m.body_hash,
                    m.session_id, m.source, m.metadata, m.created_at, m.updated_at, m.deleted_at
             FROM fts_memories fts
             JOIN memories m ON m.id = fts.rowid
             WHERE fts_memories MATCH ?1 AND m.namespace = ?2 AND m.deleted_at IS NULL
             ORDER BY rank LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![fts_query, namespace, limit as i64], |r| {
                Ok(MemoryRow {
                    id: r.get(0)?,
                    namespace: r.get(1)?,
                    name: r.get(2)?,
                    memory_type: r.get(3)?,
                    description: r.get(4)?,
                    body: r.get(5)?,
                    body_hash: r.get(6)?,
                    session_id: r.get(7)?,
                    source: r.get(8)?,
                    metadata: r.get(9)?,
                    created_at: r.get(10)?,
                    updated_at: r.get(11)?,
                    deleted_at: r.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// Syncs FTS5 external-content index after an UPDATE on the memories table.
///
/// The AFTER UPDATE trigger (`trg_fts_au`) is intentionally absent because
/// sqlite-vec loaded via `sqlite3_auto_extension` conflicts with FTS5 inside
/// UPDATE triggers. This function performs the equivalent sync in Rust:
/// DELETE the old entry, then INSERT the new one (external-content FTS5
/// tables do not support in-place UPDATE).
#[allow(clippy::too_many_arguments)]
pub fn sync_fts_after_update(
    conn: &Connection,
    memory_id: i64,
    old_name: &str,
    old_desc: &str,
    old_body: &str,
    new_name: &str,
    new_desc: &str,
    new_body: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO fts_memories(fts_memories, rowid, name, description, body)
         VALUES('delete', ?1, ?2, ?3, ?4)",
        params![memory_id, old_name, old_desc, old_body],
    )?;
    conn.execute(
        "INSERT INTO fts_memories(rowid, name, description, body)
         VALUES(?1, ?2, ?3, ?4)",
        params![memory_id, new_name, new_desc, new_body],
    )?;
    Ok(())
}
#[cfg(test)]
mod tests;
