#![cfg(feature = "slow-tests")]

//! End-to-end coverage for the `--low-memory` flag and the
//! the XDG key `ingest.low_memory` on the `ingest` subcommand.
//!
//! Each test owns an isolated `TempDir` and points the binary at it through
//! `SQLITE_GRAPHRAG_DB_PATH` and `XDG_CACHE_HOME`. The
//! `--skip-memory-guard` flag prevents the daemon autostart path during
//! parallel test runs. `#[serial]` is mandatory because the binary artefact
//! is shared and the env var these tests manipulate is process-global.

use assert_cmd::prelude::*;
use serde_json::Value;
use serial_test::serial;
use std::process::Command;
use tempfile::TempDir;

/// Builds a fresh `Command` with the mock LLM PATH prepended.
///
/// v1.0.76 spawns `claude` or `codex` on every `remember` / `ingest` /
/// `edit`. The bundled mocks under `tests/mock-llm/` return a fixed
/// 64-dim zero vector so the binary finishes without a real OAuth
/// login. The mock directory is leaked (no TempDir cleanup) so the
/// spawned subprocess always finds the mocks.
fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

#[path = "common/mod.rs"]
mod common;

/// Base command bound to the sandbox.
///
/// GAP-SG-101: this file used `SQLITE_GRAPHRAG_DB_PATH`, which no production
/// code reads, so every `init` and `ingest` here actually wrote to the
/// developer's real XDG database. `--config-dir` / `--cache-dir` work on every
/// OS, unlike `XDG_*`.
fn base_cmd(temp: &TempDir) -> Command {
    let mut cmd = sgr_cmd();
    common::write_sandbox_config(&temp.path().join("config"), None);
    cmd.arg("--embedding-model")
        .arg(common::openrouter_mock::STUB_MODEL);
    cmd.arg("--config-dir").arg(temp.path().join("config"));
    cmd.arg("--cache-dir").arg(temp.path().join("cache"));
    // GAP-SG-207: `init_db` below selects the database with `config set db.path`
    // instead of threading `--db` after every subcommand, and the fence refuses
    // a mutating verb that resolved through that key. Declaring the
    // dispensation here keeps the selection mechanism this file was written
    // around, which is exactly what `--use-active` exists for.
    cmd.arg("--use-active");
    // Keep tests deterministic regardless of the host shell's env.
    cmd.arg("--skip-memory-guard");
    cmd
}

fn ingest_cmd(temp: &TempDir) -> Command {
    base_cmd(temp)
}

fn init_db(temp: &TempDir) {
    // The database is selected through the XDG key so callers do not have to
    // thread `--db` after every subcommand.
    base_cmd(temp)
        .args(["config", "set", "db.path"])
        .arg(temp.path().join("graphrag.sqlite"))
        .assert()
        .success();
    base_cmd(temp).arg("init").assert().success();
}

fn write_corpus(temp: &TempDir) -> std::path::PathBuf {
    let dir = temp.path().join("corpus");
    std::fs::create_dir_all(&dir).expect("create corpus dir");
    std::fs::write(dir.join("a.md"), "# Alpha\n\nContent of file alpha.").expect("write a.md");
    std::fs::write(dir.join("b.md"), "# Beta\n\nContent of file beta.").expect("write b.md");
    dir
}

/// Locates the final NDJSON summary line emitted by `ingest`.
fn parse_summary(stdout: &[u8]) -> Value {
    let lines: Vec<Value> = String::from_utf8_lossy(stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).expect("ndjson line is valid JSON"))
        .collect();
    lines
        .into_iter()
        .rev()
        .find(|v| v.get("summary").and_then(|x| x.as_bool()).unwrap_or(false))
        .expect("summary line missing")
}

#[test]
#[serial]
fn ingest_low_memory_flag_succeeds() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);
    let dir = write_corpus(&tmp);

    let out = ingest_cmd(&tmp)
        .args([
            "ingest",
            dir.to_str().unwrap(),
            "--type",
            "document",
            "--pattern",
            "*.md",
            "--low-memory",
        ])
        .assert()
        .success();

    let summary = parse_summary(&out.get_output().stdout);
    assert_eq!(
        summary["files_succeeded"].as_u64().unwrap(),
        2,
        "summary: {summary}"
    );
    assert_eq!(summary["files_failed"].as_u64().unwrap(), 0);
}

#[test]
#[serial]
fn ingest_low_memory_rejects_explicit_parallelism() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);
    let dir = write_corpus(&tmp);

    let out = ingest_cmd(&tmp)
        .args([
            "ingest",
            dir.to_str().unwrap(),
            "--type",
            "document",
            "--pattern",
            "*.md",
            "--low-memory",
            "--ingest-parallelism",
            "4",
        ])
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&out.get_output().stderr);
    assert!(
        stderr.contains("conflicts with --low-memory"),
        "stderr must announce the conflict; got:\n{stderr}"
    );
}

#[test]
#[serial]
fn ingest_xdg_setting_low_memory_activates_mode() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);
    let dir = write_corpus(&tmp);

    // The supported channel is the XDG key, not the retired product env: see
    // `src/commands/ingest.rs`, which announces "via XDG ingest.low_memory".
    base_cmd(&tmp)
        .args(["config", "set", "ingest.low_memory", "1"])
        .assert()
        .success();

    let out = ingest_cmd(&tmp)
        .args([
            "-v",
            "ingest",
            dir.to_str().unwrap(),
            "--type",
            "document",
            "--pattern",
            "*.md",
        ])
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&out.get_output().stderr);
    assert!(
        stderr.contains("low-memory mode enabled via XDG ingest.low_memory"),
        "stderr must announce XDG-driven low-memory mode; got:\n{stderr}"
    );
    let summary = parse_summary(&out.get_output().stdout);
    assert_eq!(summary["files_succeeded"].as_u64().unwrap(), 2);
}
