//! Regression test for Gap 3 (v1.1.02): guards the re-embed entity
//! dispatch fix landed in v1.1.01 (`strip_prefix("entity:")` dispatch to
//! `call_reembed_entity` in `src/commands/enrich/extraction.rs:587-646`).
//! Asserts that `enrich --operation re-embed --target entities` backfills
//! `entity_embeddings` (0 -> N) and is idempotent (no duplicate rows).
//!
//! Uses the offline OpenRouter stub in `tests/common/openrouter_mock.rs`
//! instead of OpenRouter so the test is hermetic (no API key, no network).

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;

fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

fn cmd(temp: &TempDir) -> Command {
    let mut c = sgr_cmd();
    c.env_clear()
        .arg("--lang")
        .arg("en")
        .current_dir(temp.path());
    for var in &["LOCALAPPDATA", "APPDATA", "USERPROFILE", "SystemRoot"] {
        if let Ok(v) = std::env::var(var) {
            c.env(var, v);
        }
    }
    // GAP-SG-101 / G-T-XDG-04: product env was retired, so `init` without
    // `--db` resolves through XDG and never through `current_dir`. This suite
    // asserts against `temp/graphrag.sqlite`, so `db.path` must be planted or
    // every assertion opens an empty file and reports "no such table".
    common::wire_assert_cmd(temp, &mut c, "graphrag.sqlite");
    c.env("PATH", common::prepend_path(&common::mock_llm_path()));
    c
}

fn init(tmp: &TempDir) {
    cmd(tmp)
        .args(["init", "--embedding-dim", "64", "--json"])
        .assert()
        .success();
}

fn db_path(tmp: &TempDir) -> std::path::PathBuf {
    tmp.path().join("graphrag.sqlite")
}

fn entity_embeddings_count(tmp: &TempDir) -> i64 {
    let conn = rusqlite::Connection::open(db_path(tmp)).expect("db must open");
    conn.query_row("SELECT COUNT(*) FROM entity_embeddings", [], |r| r.get(0))
        .expect("count entity_embeddings")
}

fn entities_without_embedding_count(tmp: &TempDir) -> i64 {
    let conn = rusqlite::Connection::open(db_path(tmp)).expect("db must open");
    conn.query_row(
        "SELECT COUNT(*) FROM entities e \
         LEFT JOIN entity_embeddings ee ON ee.entity_id = e.id \
         WHERE ee.entity_id IS NULL",
        [],
        |r| r.get(0),
    )
    .expect("count entities without embedding")
}

/// Simulates the historical "orphan entity" state: entities exist in the
/// main DB but their `entity_embeddings` rows are absent (the exact state
/// the v1.1.1 re-embed bug left behind, and the state the scanner targets).
fn clear_entity_embeddings(tmp: &TempDir) {
    let conn = rusqlite::Connection::open(db_path(tmp)).expect("db must open");
    conn.execute("DELETE FROM entity_embeddings", [])
        .expect("clear entity_embeddings");
}

const GRAPH_PAYLOAD: &str = r#"{"body":"body about rust tokio runtime","entities":[{"name":"tokio-runtime","entity_type":"tool"},{"name":"rust-language","entity_type":"concept"}],"relationships":[{"source":"tokio-runtime","target":"rust-language","relation":"implements","strength":0.7}]}"#;

fn seed_entities_without_embedding(tmp: &TempDir) {
    cmd(tmp)
        .args([
            "remember",
            "--name",
            "m1",
            "--type",
            "note",
            "--description",
            "d",
            "--graph-stdin",
            "--json",
        ])
        .write_stdin(GRAPH_PAYLOAD)
        .assert()
        .success();
}

fn run_reembed_entities(tmp: &TempDir) {
    cmd(tmp)
        .args([
            "enrich",
            "--operation",
            "re-embed",
            "--target",
            "entities",
            "--mode",
            "openrouter",
            "--openrouter-model",
            "stub/model",
            "--embedding-backend",
            "openrouter",
            "--json",
        ])
        .assert()
        .success();
}

#[test]
#[serial]
fn reembed_entities_backfills_missing_vectors() {
    let tmp = TempDir::new().unwrap();
    init(&tmp);
    seed_entities_without_embedding(&tmp);
    clear_entity_embeddings(&tmp);

    assert_eq!(
        entity_embeddings_count(&tmp),
        0,
        "after clearing entity_embeddings, the orphan state must hold"
    );

    run_reembed_entities(&tmp);

    assert_eq!(
        entity_embeddings_count(&tmp),
        2,
        "entity_embeddings must be backfilled to 2 after re-embed"
    );
    assert_eq!(
        entities_without_embedding_count(&tmp),
        0,
        "no entity should lack embedding after backfill"
    );
}

#[test]
#[serial]
fn reembed_entities_idempotent_does_not_duplicate_rows() {
    let tmp = TempDir::new().unwrap();
    init(&tmp);
    seed_entities_without_embedding(&tmp);

    run_reembed_entities(&tmp);
    assert_eq!(entity_embeddings_count(&tmp), 2, "first run backfills 2");

    run_reembed_entities(&tmp);
    assert_eq!(
        entity_embeddings_count(&tmp),
        2,
        "second run must not duplicate (scan skips fulfilled, upsert is DELETE+INSERT by entity_id)"
    );
}
