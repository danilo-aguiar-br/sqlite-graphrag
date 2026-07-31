//! Regression test for Gap 1 (v1.1.02): the deprecated `--gliner-variant`
//! no-op argument is fully removed from `ingest` and `remember`. clap now
//! rejects it with exit 2 (unknown argument), matching the
//! `--max-entity-degree` precedent from v1.0.99. Also guards that
//! `ingest --help` is gliner-free so users and LLMs reading the help are
//! not taught a dead flag.

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;

fn cmd(temp: &TempDir) -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env_clear()
        .env("HOME", temp.path())
        .env("HOME", temp.path())
        .arg("--lang")
        .arg("en")
        .current_dir(temp.path());
    for var in &["LOCALAPPDATA", "APPDATA", "USERPROFILE", "SystemRoot"] {
        if let Ok(v) = std::env::var(var) {
            c.env(var, v);
        }
    }
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

#[test]
#[serial]
fn ingest_rejects_gliner_variant_with_exit_2() {
    let tmp = TempDir::new().unwrap();
    cmd(&tmp)
        .args([
            "ingest",
            "/tmp",
            "--gliner-variant",
            "small",
            "--dry-run",
            "--json",
        ])
        .assert()
        .failure()
        .code(2);
}

#[test]
#[serial]
fn remember_rejects_gliner_variant_with_exit_2() {
    let tmp = TempDir::new().unwrap();
    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "x",
            "--type",
            "note",
            "--description",
            "d",
            "--body",
            "b",
            "--gliner-variant",
            "small",
            "--json",
        ])
        .assert()
        .failure()
        .code(2);
}

#[test]
#[serial]
fn ingest_help_is_gliner_free() {
    let tmp = TempDir::new().unwrap();
    let output = cmd(&tmp)
        .args(["ingest", "--help"])
        .output()
        .expect("failed to run ingest --help");
    assert!(output.status.success(), "ingest --help failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.to_lowercase().contains("gliner"),
        "ingest --help still mentions gliner: {stdout}"
    );
}
