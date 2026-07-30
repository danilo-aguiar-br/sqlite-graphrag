//! Ingest file-progress queue and file collection helpers.

use crate::errors::AppError;
use rusqlite::Connection;
use std::path::{Path, PathBuf};


/// Collects files matching the pattern (reuses ingest logic).
pub(crate) fn collect_matching_files(
    dir: &Path,
    pattern: &str,
    recursive: bool,
    max_files: usize,
) -> Result<Vec<PathBuf>, AppError> {
    let mut files = Vec::new();
    crate::commands::ingest::collect_files(dir, pattern, recursive, &mut files)?;
    files.sort_unstable();

    if files.len() > max_files {
        return Err(AppError::Validation(
            crate::i18n::validation::max_files_exceeded(files.len(), max_files),
        ));
    }

    Ok(files)
}

/// Opens or creates the ingest file-progress queue (`.ingest-queue.sqlite`).
///
/// # Schema note (GAP-SG-121)
///
/// This is **not** the enrich queue (`commands::enrich::queue::open_queue_db`).
/// Ingest keys rows by `file_path` and tracks cost; enrich keys by
/// `(namespace, operation, item_key)`. Only connection pragmas are shared.
pub(crate) fn open_queue_db<P: AsRef<std::path::Path>>(path: P) -> Result<Connection, AppError> {
    let conn = Connection::open(path)?;
    crate::pragmas::apply_sidecar_queue_pragmas(&conn)?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS queue (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path   TEXT NOT NULL UNIQUE,
            name        TEXT,
            status      TEXT NOT NULL DEFAULT 'pending',
            memory_id   INTEGER,
            entities    INTEGER DEFAULT 0,
            rels        INTEGER DEFAULT 0,
            error       TEXT,
            cost_usd    REAL DEFAULT 0.0,
            attempt     INTEGER DEFAULT 0,
            elapsed_ms  INTEGER,
            created_at  TEXT DEFAULT (datetime('now')),
            done_at     TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_queue_status ON queue(status);",
    )?;

    Ok(conn)
}
