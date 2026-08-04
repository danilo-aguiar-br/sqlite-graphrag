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
    // Order matters and matches [`apply_connection_pragmas`]: the resolved
    // busy_timeout must be in force BEFORE any statement that can block. The
    // reverse order left `journal_mode` — and the CREATE/ALTER migrations that
    // `open_queue_db` runs straight after — governed by whatever timeout the
    // driver happened to default to, never by `db.query_timeout_ms`.
    // GAP-SG-87: XDG `db.query_timeout_ms` > factory `BUSY_TIMEOUT_MILLIS`.
    // Without busy_timeout, concurrent claim/write contention surfaces as
    // SQLITE_BUSY immediately (see GAP-SG-76 / rules_rust_sqlite.md).
    conn.pragma_update(None, "busy_timeout", resolved_busy_timeout_ms())?;
    conn.pragma_update(None, "journal_mode", "wal")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn busy_timeout_of(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .expect("busy_timeout must be readable")
    }

    /// The sidecar connection had no test at all, so nothing proved that
    /// `db.query_timeout_ms` ever reached it. Both queue sidecars are opened
    /// through this function under `--rest-concurrency` fan-out, which is
    /// exactly where SQLITE_BUSY shows up.
    ///
    /// Scope note, stated rather than implied: this asserts the RESULT, not the
    /// statement ORDER. The order was corrected so the timeout is in force
    /// before any statement that can block, matching
    /// [`apply_connection_pragmas`]; that ordering is verified by reading the
    /// function, because a post-hoc PRAGMA read cannot distinguish the two
    /// orders once both statements have run.
    #[test]
    fn sidecar_connection_carries_the_resolved_busy_timeout() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        apply_sidecar_queue_pragmas(&conn).expect("pragmas must apply");
        assert_eq!(
            busy_timeout_of(&conn),
            i64::from(resolved_busy_timeout_ms())
        );
    }

    /// The main-DB connection carries the same resolved value. Kept beside the
    /// sidecar test so a future change to one is visibly a change to both.
    #[test]
    fn main_connection_carries_the_resolved_busy_timeout() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        apply_connection_pragmas(&conn).expect("pragmas must apply");
        assert_eq!(
            busy_timeout_of(&conn),
            i64::from(resolved_busy_timeout_ms())
        );
    }
}
