//! SQLite PRAGMA helpers applied at connection open and on each transaction.

use crate::errors::AppError;
use rusqlite::Connection;

/// Applies one-time PRAGMAs on a freshly opened connection (e.g. `auto_vacuum`).
///
/// Calls [`apply_connection_pragmas`] internally and then sets `wal_autocheckpoint`.
/// Must be called once per database file, not once per connection.
///
/// # Errors
/// Returns `Err` when any PRAGMA execution fails.
pub fn apply_init_pragmas(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;")?;
    apply_connection_pragmas(conn)?;
    conn.execute_batch(&format!(
        "PRAGMA wal_autocheckpoint = {};",
        crate::constants::WAL_AUTOCHECKPOINT_PAGES
    ))?;
    Ok(())
}

/// Re-asserts `PRAGMA journal_mode = WAL` after operations that may revert it
/// (notably refinery-driven migrations, which can open internal handles that
/// reset the journal mode in some scenarios). Idempotent and cheap; emits
/// `tracing::warn!` if WAL fails to engage so degraded behaviour is observable.
pub fn ensure_wal_mode(conn: &Connection) -> Result<(), AppError> {
    let mode: String = conn.query_row("PRAGMA journal_mode = WAL;", [], |r| r.get(0))?;
    if mode != "wal" {
        tracing::warn!(target: "pragmas", mode = %mode, "journal_mode did not switch to WAL after re-assertion");
    }
    Ok(())
}

/// Lightweight WAL + busy_timeout for sidecar queue DBs (enrich / ingest).
///
/// # Schema note (GAP-SG-121)
///
/// Enrich (`.enrich-queue.sqlite`) and ingest (`.ingest-queue.sqlite`) queues
/// are **different products**: enrich tracks `(namespace, operation, item_key)`
/// with dead-letter / claim columns; ingest tracks `file_path` progress
/// (claude uses `cost_usd`, codex uses token counters). Do **not** unify their
/// `CREATE TABLE` shapes — only share these connection pragmas.
///
/// # Errors
/// Returns `Err` when any PRAGMA execution fails.
pub fn apply_sidecar_queue_pragmas(conn: &Connection) -> Result<(), AppError> {
    conn.pragma_update(None, "journal_mode", "wal")?;
    // Without busy_timeout, concurrent claim/write contention surfaces as
    // SQLITE_BUSY immediately (see GAP-SG-76 / rules_rust_sqlite.md).
    // GAP-SG-87: XDG `db.query_timeout_ms` > factory `BUSY_TIMEOUT_MILLIS`.
    conn.pragma_update(None, "busy_timeout", resolved_busy_timeout_ms())?;
    Ok(())
}

/// Resolved `PRAGMA busy_timeout` (ms): XDG `db.query_timeout_ms` > factory default.
fn resolved_busy_timeout_ms() -> i32 {
    let ms = crate::runtime_config::db_query_timeout_ms(crate::constants::QUERY_TIMEOUT_MILLIS);
    i32::try_from(ms)
        .unwrap_or(crate::constants::BUSY_TIMEOUT_MILLIS)
        .max(0)
}

/// Applies per-connection PRAGMAs: synchronous, foreign keys, busy timeout, cache, mmap, WAL.
///
/// Safe to call on every new connection; all settings are idempotent.
///
/// # Errors
/// Returns `Err` when any PRAGMA execution fails.
pub fn apply_connection_pragmas(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(&format!(
        "PRAGMA synchronous   = NORMAL;
         PRAGMA foreign_keys  = ON;
         PRAGMA busy_timeout  = {busy};
         PRAGMA cache_size    = {cache};
         PRAGMA temp_store    = MEMORY;
         PRAGMA mmap_size     = {mmap};",
        busy = resolved_busy_timeout_ms(),
        cache = crate::constants::CACHE_SIZE_KB,
        mmap = crate::constants::MMAP_SIZE_BYTES,
    ))?;
    let mode: String = conn.query_row("PRAGMA journal_mode = WAL;", [], |r| r.get(0))?;
    if mode != "wal" {
        tracing::warn!(target: "pragmas", mode = %mode, "journal_mode did not switch to WAL");
    }
    Ok(())
}
