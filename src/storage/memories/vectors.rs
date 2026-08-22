//! Vector companion of `memories`: upsert, delete and KNN search.
//!
//! Owns every statement touching `memory_embeddings`, including the
//! `sqlite-vec` KNN query the recall path issues.

use crate::embedder::f32_to_bytes;
use crate::errors::AppError;
use crate::storage::utils::with_busy_retry;
use rusqlite::{params, Connection};

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
    //
    // GAP-SG-268: when `memory_type` is set, the type filter is pushed into
    // this single statement through a `LEFT JOIN` on `memories`. The previous
    // code ran one `SELECT type FROM memories WHERE id = ?1` per surviving
    // candidate, so the cost grew linearly with the candidate set. A `LEFT
    // JOIN` (rather than an inner one) keeps `memory_embeddings` as the
    // driving table of the scan, so the row order the loop below sees is the
    // same one it saw before. Rows whose `memories` parent is missing yield a
    // NULL `type`, which the comparison rejects — matching the old behaviour,
    // where the failed `query_row` produced `None`.
    let placeholders = (0..namespaces.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let ns_clause = if namespaces.is_empty() {
        String::new()
    } else {
        format!(" WHERE e.namespace IN ({placeholders})")
    };
    let sql = if memory_type.is_some() {
        let type_clause = if namespaces.is_empty() {
            " WHERE m.type = ?"
        } else {
            " AND m.type = ?"
        };
        format!(
            "SELECT e.memory_id, e.embedding, e.namespace FROM memory_embeddings e \
             LEFT JOIN memories m ON m.id = e.memory_id{ns_clause}{type_clause}"
        )
    } else {
        format!("SELECT e.memory_id, e.embedding, e.namespace FROM memory_embeddings e{ns_clause}")
    };
    let mut stmt = conn.prepare(&sql)?;
    let mut raw_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    for ns in namespaces {
        raw_params.push(Box::new(ns.clone()));
    }
    if let Some(mt) = memory_type {
        raw_params.push(Box::new(mt.to_string()));
    }
    let param_refs: Vec<&dyn rusqlite::ToSql> = raw_params.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |r| {
        let id: i64 = r.get(0)?;
        let bytes: Vec<u8> = r.get(1)?;
        let ns: String = r.get(2)?;
        Ok((id, bytes, ns))
    })?;

    // The optional `type` restriction is already applied by the statement
    // above, so this loop only scores the rows SQLite handed back.
    let mut candidates: Vec<(i64, f32)> = Vec::new();
    for row in rows {
        let (id, bytes, ns) = row?;
        let stored = crate::embedder::bytes_to_f32(&bytes);
        if stored.len() != embedding.len() {
            continue;
        }
        let sim = crate::similarity::cosine_similarity(embedding, &stored);
        let dist = crate::similarity::similarity_to_distance(sim);
        let _ = ns; // namespace already filtered at SQL level
        candidates.push((id, dist));
    }
    // Sort by distance ascending (best matches first).
    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(k);
    Ok(candidates)
}
