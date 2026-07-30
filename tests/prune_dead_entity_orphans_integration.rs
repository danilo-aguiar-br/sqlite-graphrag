//! Integration test for the v1.1.02 `--prune-dead-entity-orphans` flag.
//! Guards the clap wiring (flag accepted, conflicts_with the memory variant)
//! and the envelope contract (action label). The row-level DELETE logic is
//! covered by the unit test `prune_dead_entity_orphans_removes_only_entity_dead_rows`
//! in `src/commands/enrich/queue.rs`.

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
        .arg("--lang").arg("en")
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
fn prune_dead_entity_orphans_emits_envelope() {
    let tmp = TempDir::new().unwrap();
    cmd(&tmp)
        .args(["init", "--embedding-dim", "64", "--json"])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args([
            "enrich",
            "--operation",
            "re-embed",
            "--prune-dead-entity-orphans",
            "--json",
        ])
        .output()
        .expect("run enrich --prune-dead-entity-orphans");
    assert!(
        output.status.success(),
        "enrich --prune-dead-entity-orphans failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"action\":\"prune-dead-entity-orphans\""),
        "action label missing in envelope: {stdout}"
    );
}

#[test]
#[serial]
fn prune_dead_entity_orphans_conflicts_with_memory_variant() {
    let tmp = TempDir::new().unwrap();
    cmd(&tmp)
        .args(["init", "--embedding-dim", "64", "--json"])
        .assert()
        .success();

    // Passing both prune flags must be rejected by clap (conflicts_with).
    cmd(&tmp)
        .args([
            "enrich",
            "--operation",
            "re-embed",
            "--prune-dead-orphans",
            "--prune-dead-entity-orphans",
            "--json",
        ])
        .assert()
        .failure()
        .code(2);
}
