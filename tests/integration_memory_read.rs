//! Memory read path: read (including --format raw), list, and the not-found contract.
//!
//! Split out of `integration_memory_crud.rs` by GAP-SG-208: that file reached
//! 1018 lines, past the 800-line ceiling this project sets for itself. It was
//! itself carved out of a 2485-line `integration.rs` in v1.2.5, so this is the
//! second pass of the same decomposition. Helpers stay in `tests/common/`.

#[path = "common/mod.rs"]
mod common;

#[allow(unused_imports)]
use assert_cmd::Command;
#[allow(unused_imports)]
use common::{
    cmd, home_isolated_cmd, init_db, isolated_cmd_in, seed_memory_with_entities, sgr_cmd,
};
#[allow(unused_imports)]
use tempfile::TempDir;
// ---------------------------------------------------------------------------

#[test]
fn test_read_existing_memory() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memory-readable",
            "--type",
            "project",
            "--description",
            "A readable memory",
            "--body",
            "O conteudo do corpo da memoria",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args(["read", "--name", "memory-readable"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["name"], "memory-readable");
    assert_eq!(json["memory_type"], "project");
    assert_eq!(json["description"], "A readable memory");
}

/// GAP-SG-50: `read --format raw` writes the pure body to stdout with no JSON
/// envelope. The unit layer covers the formatter; this asserts the end-to-end
/// CLI stdout contract a caller pipes into `jaq`/files.
#[test]
fn test_read_format_raw_emits_pure_body() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "raw-body-memory",
            "--type",
            "note",
            "--description",
            "raw read contract",
            "--body",
            "CORPO_PURO_SEM_ENVELOPE",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args(["read", "--name", "raw-body-memory", "--format", "raw"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("raw stdout must be valid UTF-8");
    assert!(
        !stdout.trim_start().starts_with('{'),
        "raw output must not be a JSON envelope, got: {stdout:?}"
    );
    assert!(
        stdout.contains("CORPO_PURO_SEM_ENVELOPE"),
        "raw output must contain the verbatim body, got: {stdout:?}"
    );
}

#[test]
fn test_read_nonexistent_returns_exit_4() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args(["read", "--name", "nao-existe"])
        .assert()
        .failure()
        .code(4);
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

#[test]
fn test_list_memories() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "lista-mem-1",
            "--type",
            "user",
            "--description",
            "desc1",
            "--body",
            "corpo1",
        ])
        .assert()
        .success();

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "lista-mem-2",
            "--type",
            "feedback",
            "--description",
            "desc2",
            "--body",
            "corpo2",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .arg("list")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["items"].as_array().unwrap().len() >= 2);
}

// ---------------------------------------------------------------------------
// forget
