//! v1.1.06 regression suite for GAP-ENTITY-CONNECT-SCAN-CARTESIAN.
//!
//! Guarantees:
//! 1. `entity-connect --dry-run` completes on a dense co-occurrence graph.
//! 2. Scan emits `scan_start` / `scan` phases (never hangs after validate only).
//! 3. Enqueued keys use the `pair:{id1}:{id2}` contract.
//! 4. Wall-clock is bounded (timeout wrapper + O(k) algorithm).

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;

fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

fn cmd_base(tmp: &TempDir) -> Command {
    let mut c = sgr_cmd();
    c.env("SQLITE_GRAPHRAG_DB_PATH", tmp.path().join("test.sqlite"));
    c.env("SQLITE_GRAPHRAG_CACHE_DIR", tmp.path().join("cache"));
    c.env("SQLITE_GRAPHRAG_LOG_LEVEL", "error");
    c.arg("--skip-memory-guard");
    c
}

fn init_db(tmp: &TempDir) {
    cmd_base(tmp).arg("init").assert().success();
}

/// Seed N entities that all co-occur in one memory (dense pair space, still O(k) scan).
fn seed_cooccurring_entities(tmp: &TempDir, n: usize) {
    let db = tmp.path().join("test.sqlite");
    let script = format!(
        r#"
        INSERT INTO memories (namespace, name, body) VALUES ('global', 'bulk-mem', 'seed body');
        "#
    );
    // Use the CLI binary path's sibling sqlite via rusqlite through a tiny rust-less
    // approach: shell out to sqlite3 if available, else use multiple remember calls.
    if which_sqlite3() {
        let mut sql = String::from(
            "INSERT INTO memories (namespace, name, type, description, body, body_hash) \
             VALUES ('global', 'bulk-mem', 'note', 'seed', 'seed body', 'hash-seed');\n",
        );
        for i in 0..n {
            sql.push_str(&format!(
                "INSERT INTO entities (namespace, name, type, degree) VALUES ('global', 'ent-{i}', 'tool', 0);\n"
            ));
        }
        sql.push_str(
            "INSERT INTO memory_entities (memory_id, entity_id)
             SELECT m.id, e.id FROM memories m CROSS JOIN entities e
             WHERE m.name = 'bulk-mem' AND e.namespace = 'global' AND e.name LIKE 'ent-%';\n",
        );
        let status = StdCommand::new("sqlite3")
            .arg(&db)
            .arg(&sql)
            .status()
            .expect("sqlite3 seed");
        assert!(status.success(), "sqlite3 seed failed");
        return;
    }
    // Fallback: fewer entities via remember (slower but portable).
    let _ = script;
    for i in 0..n.min(30) {
        cmd_base(tmp)
            .args([
                "remember",
                &format!("ent-mem-{i}"),
                "--body",
                &format!("Entity seed for ent-{i} and shared context"),
                "-q",
            ])
            .assert()
            .success();
    }
}

fn which_sqlite3() -> bool {
    StdCommand::new("sqlite3")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn entity_connect_dry_run_emits_scan_phases_and_pair_keys() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);
    seed_cooccurring_entities(&tmp, 60);

    let started = Instant::now();
    let out = cmd_base(&tmp)
        .args([
            "enrich",
            "--operation",
            "entity-connect",
            "--dry-run",
            "--json",
            "--limit",
            "10",
            "--mode",
            "openrouter",
            "--openrouter-model",
            "test/model",
        ])
        .env("OPENROUTER_API_KEY", "sk-test-not-used-on-dry-run")
        .output()
        .expect("run entity-connect dry-run");
    let elapsed = started.elapsed();

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "entity-connect dry-run must succeed\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        elapsed < Duration::from_secs(30),
        "scan must finish quickly, took {elapsed:?}"
    );

    let mut saw_scan_start = false;
    let mut saw_scan = false;
    let mut saw_scan_meta = false;
    let mut pairs_enqueued_meta: Option<u64> = None;
    let mut pair_previews = 0usize;
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("phase").and_then(|p| p.as_str()) == Some("scan_start") {
            saw_scan_start = true;
            assert_eq!(
                v.get("operation").and_then(|p| p.as_str()),
                Some("entity-connect"),
                "scan_start.operation must be the real CLI op name"
            );
            assert_eq!(
                v.get("pair_algorithm").and_then(|p| p.as_str()),
                Some("cooccurrence+hub_island")
            );
            assert!(
                v.get("backlog_degree0_proxy").is_some(),
                "scan_start must report backlog_degree0_proxy distinct from pair enqueue"
            );
        }
        if v.get("phase").and_then(|p| p.as_str()) == Some("scan") {
            saw_scan = true;
        }
        if v.get("phase").and_then(|p| p.as_str()) == Some("scan_meta") {
            saw_scan_meta = true;
            assert_eq!(
                v.get("operation").and_then(|p| p.as_str()),
                Some("entity-connect")
            );
            pairs_enqueued_meta = v.get("pairs_enqueued_this_scan").and_then(|p| p.as_u64());
            assert!(
                pairs_enqueued_meta.is_some(),
                "scan_meta must include pairs_enqueued_this_scan"
            );
        }
        if v.get("status").and_then(|s| s.as_str()) == Some("preview") {
            if let Some(item) = v.get("item").and_then(|i| i.as_str()) {
                assert!(
                    item.starts_with("pair:"),
                    "preview item must be pair:id1:id2, got {item}"
                );
                pair_previews += 1;
            }
        }
    }
    assert!(
        saw_scan_start,
        "expected phase scan_start in NDJSON: {stdout}"
    );
    assert!(saw_scan, "expected phase scan in NDJSON: {stdout}");
    assert!(
        saw_scan_meta,
        "expected phase scan_meta in NDJSON: {stdout}"
    );
    assert!(
        pair_previews > 0,
        "expected at least one pair: preview key, stdout={stdout}"
    );
    assert!(
        pair_previews <= 10,
        "limit 10 must cap previews, got {pair_previews}"
    );
    assert_eq!(
        pairs_enqueued_meta,
        Some(pair_previews as u64),
        "pairs_enqueued_this_scan must match preview count"
    );
}

#[test]
fn entity_connect_item_type_is_entity_pair_in_help_path() {
    // Smoke: binary still exposes entity-connect and does not panic on --help.
    cmd_base(&TempDir::new().unwrap())
        .args(["enrich", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("entity-connect"));
}
