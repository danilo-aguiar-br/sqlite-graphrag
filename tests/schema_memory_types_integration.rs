#![cfg(feature = "slow-tests")]

//! Suite 3 — schema and migrations: V009 memory-type lifecycle end to end
//!
//! Part of the schema suite split by GAP-SG-210: the single file held 923 lines
//! and 21 tests, past the 800-line ceiling this project sets for itself. The
//! shared harness lives in `tests/migration_support/`, which documents the
//! sqlite-vec isolation rule and why `#[serial]` is mandatory here.

#[path = "migration_support/mod.rs"]
mod support;

use serial_test::serial;
use support::sgr_on;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Test 13 — V009 e2e: full lifecycle for the new `document` memory type
// ---------------------------------------------------------------------------
// V009 expanded the `memories.type` CHECK constraint to accept `document`
// and `note` in addition to the seven pre-existing types. This test validates
// the full path: remember -> list (filtered by type) -> recall.

#[test]
#[serial]
fn v009_document_type_lifecycle_e2e() {
    let tmp = TempDir::new().expect("TempDir must be created");
    let db_path = tmp.path().join("test.sqlite");

    // Init applies V001..V009 in a fresh DB.
    sgr_on(&tmp, &db_path).args(["init"]).assert().success();

    // Insert a memory with the new type=document accepted by V009.
    let output = sgr_on(&tmp, &db_path)
        .args([
            "remember",
            "--name",
            "doc-test",
            "--type",
            "document",
            "--description",
            "test doc",
            "--body",
            "Sample document body for e2e test",
            "--skip-extraction",
        ])
        .output()
        .expect("remember must run");
    assert!(
        output.status.success(),
        "remember failed: status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    // List filtered by type=document must return the inserted record.
    let output = sgr_on(&tmp, &db_path)
        .args(["list", "--type", "document", "--json"])
        .output()
        .expect("list must run");
    assert!(
        output.status.success(),
        "list failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("list output must be valid JSON");
    let items = json["items"]
        .as_array()
        .expect("list response must contain `items` array");
    assert_eq!(items.len(), 1, "expected exactly 1 document, got {items:?}");
    assert_eq!(items[0]["type"], "document");

    // Recall via FTS5/vector must surface the freshly inserted document.
    let output = sgr_on(&tmp, &db_path)
        .args(["recall", "Sample", "--json"])
        .output()
        .expect("recall must run");
    assert!(
        output.status.success(),
        "recall failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("recall output must be valid JSON");
    let results = json["results"]
        .as_array()
        .expect("recall response must contain `results` array");
    assert!(
        !results.is_empty(),
        "recall must return at least one match for 'Sample', got: {results:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 14 — V009 e2e: full lifecycle for the new `note` memory type
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn v009_note_type_lifecycle_e2e() {
    let tmp = TempDir::new().expect("TempDir must be created");
    let db_path = tmp.path().join("test.sqlite");

    sgr_on(&tmp, &db_path).args(["init"]).assert().success();

    let output = sgr_on(&tmp, &db_path)
        .args([
            "remember",
            "--name",
            "note-test",
            "--type",
            "note",
            "--description",
            "test note",
            "--body",
            "Quick scratch note for e2e validation",
            "--skip-extraction",
        ])
        .output()
        .expect("remember must run");
    assert!(
        output.status.success(),
        "remember failed: status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let output = sgr_on(&tmp, &db_path)
        .args(["list", "--type", "note", "--json"])
        .output()
        .expect("list must run");
    assert!(
        output.status.success(),
        "list failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("list output must be valid JSON");
    let items = json["items"]
        .as_array()
        .expect("list response must contain `items` array");
    assert_eq!(items.len(), 1, "expected exactly 1 note, got {items:?}");
    assert_eq!(items[0]["type"], "note");

    let output = sgr_on(&tmp, &db_path)
        .args(["recall", "scratch", "--json"])
        .output()
        .expect("recall must run");
    assert!(
        output.status.success(),
        "recall failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("recall output must be valid JSON");
    let results = json["results"]
        .as_array()
        .expect("recall response must contain `results` array");
    assert!(
        !results.is_empty(),
        "recall must return at least one match for 'scratch', got: {results:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 15 — V009: invalid memory type must be rejected by clap value_enum
// ---------------------------------------------------------------------------
// `--type` is bound to `MemoryType` via `value_enum`, so clap rejects unknown
// variants before reaching the SQLite CHECK constraint. This guards against
// a future regression where the enum drifts away from the migration's CHECK.

#[test]
#[serial]
fn v009_invalid_type_rejected() {
    let tmp = TempDir::new().expect("TempDir must be created");
    let db_path = tmp.path().join("test.sqlite");

    sgr_on(&tmp, &db_path).args(["init"]).assert().success();

    let output = sgr_on(&tmp, &db_path)
        .args([
            "remember",
            "--name",
            "x",
            "--type",
            "invalid_type_xyz",
            "--description",
            "t",
            "--body",
            "t",
        ])
        .output()
        .expect("remember must run");

    assert!(
        !output.status.success(),
        "remember must reject invalid type 'invalid_type_xyz'"
    );
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        stderr.contains("invalid") || stderr.contains("type") || stderr.contains("possible values"),
        "stderr should mention type rejection, got: {stderr}"
    );
}
