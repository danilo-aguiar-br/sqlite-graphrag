#![cfg(feature = "slow-tests")]

use assert_cmd::Command;
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

fn cmd(tmp: &TempDir) -> Command {
    // GAP-SG-101: product env is not read (G-T-XDG-04).
    let mut c = sgr_cmd();
    common::wire_assert_cmd(tmp, &mut c, "test.sqlite");
    c
}

fn init_db(tmp: &TempDir) {
    cmd(tmp).arg("init").assert().success();
}

#[test]
fn test_vacuum_auto_inits_when_missing() {
    let tmp = TempDir::new().unwrap();
    cmd(&tmp).arg("vacuum").assert().success();
}

#[test]
fn test_vacuum_success_after_init() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp).arg("vacuum").assert().success();
}

#[test]
fn test_vacuum_returns_json_with_status_ok() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd(&tmp)
        .arg("vacuum")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["status"], "ok");
}

#[test]
fn test_vacuum_json_contem_db_path() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd(&tmp)
        .arg("vacuum")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["db_path"].is_string());
    assert!(!json["db_path"].as_str().unwrap().is_empty());
}

#[test]
fn test_vacuum_json_contem_tamanhos() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd(&tmp)
        .arg("vacuum")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["size_before_bytes"].is_number());
    assert!(json["size_after_bytes"].is_number());
}

#[test]
fn test_vacuum_via_db_flag() {
    // GAP-SG-101: product env is not a path channel. --db after the
    // subcommand is the only way to select a non-default database.
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("custom.sqlite");

    let mut init_cmd = sgr_cmd();
    common::wire_assert_cmd(&tmp, &mut init_cmd, "unused.sqlite");
    init_cmd
        .args(["init", "--db"])
        .arg(&db_path)
        .assert()
        .success();

    let mut vac_cmd = sgr_cmd();
    common::wire_assert_cmd(&tmp, &mut vac_cmd, "unused.sqlite");
    vac_cmd
        .args(["vacuum", "--db"])
        .arg(&db_path)
        .assert()
        .success();

    assert!(
        db_path.exists(),
        "--db must create/use the explicit database path"
    );
}
