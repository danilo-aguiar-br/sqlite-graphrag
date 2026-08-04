//! Paginated listing and counting over `memories`.

use super::rows::MemoryRow;
use crate::errors::AppError;
use rusqlite::{params, Connection};

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
