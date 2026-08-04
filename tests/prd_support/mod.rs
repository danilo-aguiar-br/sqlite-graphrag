//! Shared harness for the PRD-compliance suites (GAP-SG-208).
//!
//! Extracted from `prd_compliance.rs`, which carried 1367 lines and 32 tests,
//! past the 800-line ceiling the project sets for itself. Each test isolates
//! itself with an exclusive TempDir plus a planted `db.path` / `--config-dir`.

#![allow(dead_code)]

use assert_cmd::Command;

use std::path::PathBuf;
use tempfile::TempDir;

/// Builds a fresh `Command` with the mock LLM PATH prepended.
///
/// v1.0.76 spawns `claude` or `codex` on every `remember` / `ingest` /
/// `edit`. The bundled mocks under `tests/mock-llm/` return a fixed
/// 64-dim zero vector so the binary finishes without a real OAuth
/// login. The mock directory is leaked (no TempDir cleanup) so the
/// spawned subprocess always finds the mocks.
pub fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

#[path = "../common/mod.rs"]
pub mod common;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

pub fn cmd_base(tmp: &TempDir) -> Command {
    // GAP-SG-101: product env is not read (G-T-XDG-04).
    let mut c = sgr_cmd();
    common::wire_assert_cmd(tmp, &mut c, "test.sqlite");
    c.arg("--lang").arg("en");
    c.arg("--skip-memory-guard");
    c
}

pub fn init_db(tmp: &TempDir) {
    cmd_base(tmp).arg("init").assert().success();
}

pub fn remember_ok(tmp: &TempDir, name: &str, body: &str) {
    cmd_base(tmp)
        .args([
            "remember",
            "--name",
            name,
            "--type",
            "user",
            "--description",
            "desc for prd test",
            "--namespace",
            "global",
            "--body",
            body,
            "--skip-extraction",
        ])
        .assert()
        .success();
}

pub fn db_path(tmp: &TempDir) -> PathBuf {
    tmp.path().join("test.sqlite")
}

// ---------------------------------------------------------------------------
