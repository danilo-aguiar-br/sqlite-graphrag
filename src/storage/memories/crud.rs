//! Single-row CRUD over `memories`: lookup, insert, update, read.
//!
//! Every statement that resolves or mutates ONE memory by name, hash or id.

use super::rows::{MemoryRow, NewMemory};
use crate::errors::AppError;
use rusqlite::{params, Connection};

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
