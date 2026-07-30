//! Soft-delete lifecycle helpers for the `memories` table.
//!
//! Soft-delete sets `deleted_at = unixepoch()` without removing versions or
//! chunks, so `restore` can undo until `purge` reclaims storage permanently.

use crate::errors::AppError;
use rusqlite::{params, Connection};

/// Looks up a memory by `(namespace, name)` regardless of deletion state.
///
/// Returns `Some((id, is_deleted))` when the row exists.
/// `is_deleted` is `true` when `deleted_at IS NOT NULL`.
///
/// # Errors
///
/// Propagates [`AppError::Database`] on SQLite failures.
pub fn find_by_name_any_state(
    conn: &Connection,
    namespace: &str,
    name: &str,
) -> Result<Option<(i64, bool)>, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, (deleted_at IS NOT NULL) AS is_deleted
         FROM memories WHERE namespace = ?1 AND name = ?2",
    )?;
    let result = stmt.query_row(params![namespace, name], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?))
    });
    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e)),
    }
}

/// Clears `deleted_at` to restore a soft-deleted memory.
///
/// # Errors
///
/// Propagates [`AppError::Database`] on SQLite failures.
pub fn clear_deleted_at(conn: &Connection, memory_id: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE memories SET deleted_at = NULL WHERE id = ?1",
        params![memory_id],
    )?;
    Ok(())
}

/// Soft-deletes a memory by setting `deleted_at = unixepoch()`.
///
/// Versions and chunks are preserved so `restore` can undo the operation
/// until a subsequent `purge` reclaims the storage permanently.
///
/// # Returns
///
/// `Ok(true)` when a live memory was soft-deleted, `Ok(false)` when no
/// matching live row existed.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn soft_delete(conn: &Connection, namespace: &str, name: &str) -> Result<bool, AppError> {
    let affected = conn.execute(
        "UPDATE memories SET deleted_at = unixepoch() WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
        params![namespace, name],
    )?;
    Ok(affected == 1)
}

/// Fetches all memory_ids in a namespace that are soft-deleted and whose
/// `deleted_at` is older than `before_ts` (unix epoch seconds).
///
/// Used by `purge` to collect stale rows for permanent deletion.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn list_deleted_before(
    conn: &Connection,
    namespace: &str,
    before_ts: i64,
) -> Result<Vec<i64>, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id FROM memories WHERE namespace = ?1 AND deleted_at IS NOT NULL AND deleted_at < ?2",
    )?;
    let ids = stmt
        .query_map(params![namespace, before_ts], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}
