//! Shared fixtures for the queue test modules (GAP-SG-146).
//!
//! `queue_tests_a.rs` and `queue_tests_b.rs` each carried a byte-identical
//! copy of these four helpers. One copy now serves every queue test module.

use super::*;
use rusqlite::Connection;

pub(super) fn open_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE memories (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace   TEXT NOT NULL DEFAULT 'global',
                name        TEXT NOT NULL,
                type        TEXT NOT NULL DEFAULT 'note',
                description TEXT NOT NULL DEFAULT '',
                body        TEXT NOT NULL DEFAULT '',
                body_hash   TEXT NOT NULL DEFAULT '',
                session_id  TEXT,
                source      TEXT NOT NULL DEFAULT 'agent',
                metadata    TEXT NOT NULL DEFAULT '{}',
                created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                deleted_at  INTEGER,
                UNIQUE(namespace, name)
            );",
    )
    .expect("schema creation must succeed");
    conn
}

/// Opens a queue database on a fresh file in the OS temp directory, returning
/// the connection and the path so the caller can remove it.
///
/// The path used to be a hand-built `/tmp/...` literal, which made every test
/// reaching this fixture Unix-only — coverage missing precisely on the platform
/// (Windows) where the product claims support and gets the least exercise.
/// `tempfile` resolves the platform's temp directory and reserves the name
/// atomically, so the pid+random suffix that stood in for collision avoidance
/// is no longer hand-rolled either.
///
/// The file is kept rather than guarded by a `TempPath`: the returned `String`
/// is the fixture's contract with four sibling test modules that already delete
/// it themselves, and a guard would have to be threaded through all of them.
pub(super) fn open_temp_queue() -> (Connection, String) {
    let path = tempfile::Builder::new()
        .prefix("test-enrich-dl-")
        .suffix(".sqlite")
        .tempfile()
        .expect("temp file must be creatable")
        .into_temp_path()
        .keep()
        .expect("temp file must persist for the queue db to open it");
    let path = path.to_string_lossy().into_owned();
    let conn = open_queue_db(&path).expect("queue db must open");
    (conn, path)
}

pub(super) fn insert_pending(conn: &Connection, key: &str) -> i64 {
    insert_pending_op(conn, key, "memory", "MemoryBindings")
}

pub(super) fn insert_pending_op(
    conn: &Connection,
    key: &str,
    item_type: &str,
    operation: &str,
) -> i64 {
    conn.execute(
        "INSERT INTO queue (item_key, item_type, status, operation) VALUES (?1, ?2, 'pending', ?3)",
        rusqlite::params![key, item_type, operation],
    )
    .unwrap();
    conn.last_insert_rowid()
}
