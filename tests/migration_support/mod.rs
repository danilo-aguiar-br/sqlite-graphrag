//! Shared harness for the schema/migration suites (GAP-SG-210).
//!
//! These helpers lived inside `schema_migration_integration.rs` while that file
//! carried 923 lines and 21 tests, past the 800-line ceiling this project sets
//! for itself. Splitting the suite by subject meant the harness had to reach
//! every part, so it moved here rather than being copied three times.
//!
//! ISOLATION: each test uses a planted `db.path` / `--db` under an exclusive
//! `TempDir`. Introspection runs through rusqlite directly, without depending
//! on any binary output.
//!
//! NOTE: sqlite-vec uses `sqlite3_auto_extension`, which is process-global. To
//! avoid registering the extension more than once across parallel tests, every
//! test that opens a sqlite-vec database does so via `sqlite-graphrag init`
//! (external binary), which loads the extension in its own process. Pure
//! introspection tests (sqlite_master, triggers, FTS) open the database via
//! rusqlite after init for read-only queries — they never load sqlite-vec in
//! the test process.
//!
//! `#[serial]` is mandatory in the suites that use this: although each test
//! owns its database, the compiled artefact is shared and the `TempDir` is only
//! released when the test ends; serialising removes filesystem races and keeps
//! timings predictable.

#![allow(dead_code)]

use assert_cmd::Command;
use rusqlite::Connection;
use tempfile::TempDir;

#[path = "../common/mod.rs"]
pub mod common;

/// Builds a fresh `Command` with the mock LLM PATH prepended.
///
/// The bundled mocks under `tests/mock-llm/` return a fixed zero vector so the
/// binary finishes without reaching a real endpoint. The mock directory is
/// leaked (no `TempDir` cleanup) so the spawned subprocess always finds it.
pub fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

/// GAP-SG-101: isolated command bound to `db_path` via planted `db.path`
/// (product env is not read — G-T-XDG-04).
///
/// GAP-SG-207: carries `--use-active` for the same reason
/// [`common::wire_assert_cmd`] does. Binding through the planted key IS what
/// this helper is for, and the fence refuses a mutating verb that resolved that
/// way unless the dispensation is declared.
pub fn sgr_on(tmp: &TempDir, db_path: &std::path::Path) -> Command {
    let mut c = sgr_cmd();
    common::plant_db_path(&tmp.path().join("config"), db_path);
    c.env("HOME", tmp.path().join("home"))
        .env("XDG_CACHE_HOME", tmp.path().join("xdg_cache"))
        .env("XDG_CONFIG_HOME", tmp.path().join("xdg_config"))
        .env("XDG_DATA_HOME", tmp.path().join("xdg_data"))
        .env("XDG_RUNTIME_DIR", tmp.path().join("xdg_runtime"))
        .arg("--config-dir")
        .arg(tmp.path().join("config"))
        .arg("--embedding-model")
        .arg(common::openrouter_mock::STUB_MODEL)
        .arg("--cache-dir")
        .arg(tmp.path().join("cache"))
        .arg("--use-active")
        .arg("--skip-memory-guard");
    c
}

/// Runs `sqlite-graphrag init` on an isolated temporary database and returns
/// the `TempDir` (to keep the database alive) and the SQLite file path.
pub fn init_isolated_db() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("TempDir must be created");
    let db_path = tmp.path().join("test.sqlite");

    sgr_on(&tmp, &db_path).args(["init"]).assert().success();

    (tmp, db_path)
}

/// Opens the database read-only after init (without sqlite-vec in this process).
pub fn conn_ro(db_path: &std::path::Path) -> Connection {
    Connection::open(db_path).expect("database connection must work")
}

/// Checks whether a table or view exists in `sqlite_master`.
pub fn table_exists(conn: &Connection, name: &str) -> bool {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table','view') AND name = ?1",
            rusqlite::params![name],
            |row| row.get(0),
        )
        .unwrap_or(0);
    count > 0
}

/// Checks whether a trigger exists in `sqlite_master`.
pub fn trigger_exists(conn: &Connection, name: &str) -> bool {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
            rusqlite::params![name],
            |row| row.get(0),
        )
        .unwrap_or(0);
    count > 0
}

/// Checks whether an index exists in `sqlite_master`.
pub fn index_exists(conn: &Connection, name: &str) -> bool {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
            rusqlite::params![name],
            |row| row.get(0),
        )
        .unwrap_or(0);
    count > 0
}
