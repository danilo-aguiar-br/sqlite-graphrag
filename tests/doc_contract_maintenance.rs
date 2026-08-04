#![cfg(feature = "slow-tests")]

//! Contract: maintenance and diagnostics — health, stats, namespace-detect, migrate, optimize, vacuum, sync-safe-copy, cleanup-orphans, debug-schema, fts, backup.
//!
//! Part of the JSON-contract suite split by GAP-SG-208: the single file held
//! 1393 lines and 41 tests, past the 800-line ceiling this project sets for
//! itself. The shared harness lives in `tests/contract_support/`.
//!
//! Ground truth: `docs/schemas/*.schema.json`. Each test checks the expected
//! exit code, valid JSON, and the presence of the required keys.

#[path = "contract_support/mod.rs"]
mod support;

use serial_test::serial;
use support::{assert_has_keys, Env};
// ---------------------------------------------------------------------------
// 03 — health
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_03_health() {
    let env = Env::new();
    env.init();
    let out = env.cmd().arg("health").output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "health",
        &json,
        &[
            "status",
            "db_path",
            "schema_version",
            "counts",
            "checks",
            "elapsed_ms",
        ],
    );
    assert!(json["counts"]["memories"].is_number());
    assert!(json["counts"]["entities"].is_number());
    assert!(json["counts"]["relationships"].is_number());
    assert!(json["checks"].is_array());
}

// ---------------------------------------------------------------------------
// 04 — stats
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_04_stats() {
    let env = Env::new();
    env.init();
    let out = env.cmd().arg("stats").output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "stats",
        &json,
        &[
            "memories",
            "memories_total",
            "entities",
            "entities_total",
            "relationships",
            "relationships_total",
            "edges",
            "chunks_total",
            "avg_body_len",
            "namespaces",
            "db_size_bytes",
            "db_bytes",
            "schema_version",
        ],
    );
}

// ---------------------------------------------------------------------------
// 19 — namespace-detect
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_19_namespace_detect() {
    let env = Env::new();
    env.init();

    let out = env.cmd().arg("namespace-detect").output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "namespace-detect",
        &json,
        &["namespace", "source", "cwd", "elapsed_ms"],
    );
    assert!(json["namespace"].is_string());
}

// ---------------------------------------------------------------------------
// 20 — migrate
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_20_migrate() {
    let env = Env::new();
    env.init();

    let out = env.cmd().arg("migrate").output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys("migrate", &json, &["db_path", "schema_version", "status"]);
    // Since v1.0.35, migrate emits schema_version as JSON number (was string before).
    let sv = &json["schema_version"];
    assert!(
        sv.is_number(),
        "migrate schema_version must be a JSON number since v1.0.35, got: {sv}"
    );
}

// ---------------------------------------------------------------------------
// 21 — optimize
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_21_optimize() {
    let env = Env::new();
    env.init();

    let out = env.cmd().arg("optimize").output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys("optimize", &json, &["db_path", "status"]);
    assert_eq!(json["status"], "ok");
}

// ---------------------------------------------------------------------------
// 22 — vacuum
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_22_vacuum() {
    let env = Env::new();
    env.init();

    let out = env.cmd().arg("vacuum").output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "vacuum",
        &json,
        &["db_path", "size_before_bytes", "size_after_bytes", "status"],
    );
    assert_eq!(json["status"], "ok");
}

// ---------------------------------------------------------------------------
// 23 — sync-safe-copy
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_23_sync_safe_copy() {
    let env = Env::new();
    env.init();
    let dest = env.tmp.path().join("backup.sqlite");

    let out = env
        .cmd()
        .args(["sync-safe-copy", "--dest", dest.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "sync-safe-copy",
        &json,
        &["source_db_path", "dest_path", "bytes_copied", "status"],
    );
    assert_eq!(json["status"], "ok");
    assert!(dest.exists(), "arquivo de destino deve existir");
}

// ---------------------------------------------------------------------------
// 24 — cleanup-orphans
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_24_cleanup_orphans() {
    let env = Env::new();
    env.init();

    let out = env.cmd().arg("cleanup-orphans").output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "cleanup-orphans",
        &json,
        &["orphan_count", "deleted", "dry_run", "namespace"],
    );
    assert!(json["orphan_count"].is_number());
}

// ---------------------------------------------------------------------------
// 25 — __debug_schema (oculto, adicionado na v2.0.5)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_25_debug_schema() {
    let env = Env::new();
    env.init();

    let out = env.cmd().arg("debug-schema").output().unwrap();
    assert!(
        out.status.success(),
        "debug-schema failed: {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "debug-schema",
        &json,
        &[
            "schema_version",
            "user_version",
            "objects",
            "migrations",
            "elapsed_ms",
        ],
    );
    assert!(json["objects"].is_array());
    assert!(json["migrations"].is_array());
}

// ---------------------------------------------------------------------------
// 26 — fts rebuild
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_26_fts_rebuild() {
    let env = Env::new();
    env.init();

    let out = env.cmd().args(["fts", "rebuild"]).output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "fts rebuild",
        &json,
        &["action", "rows_indexed", "elapsed_ms"],
    );
    assert_eq!(json["action"], "rebuilt");
}

// ---------------------------------------------------------------------------
// 27 — fts check
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_27_fts_check() {
    let env = Env::new();
    env.init();

    let out = env.cmd().args(["fts", "check"]).output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "fts check",
        &json,
        &["action", "integrity_ok", "elapsed_ms"],
    );
    assert_eq!(json["action"], "checked");
}

// ---------------------------------------------------------------------------
// 28 — fts stats
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_28_fts_stats() {
    let env = Env::new();
    env.init();

    let out = env.cmd().args(["fts", "stats"]).output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "fts stats",
        &json,
        &["total_rows", "fts_functional", "elapsed_ms"],
    );
}

// ---------------------------------------------------------------------------
// 29 — backup
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_29_backup() {
    let env = Env::new();
    env.init();
    let dest = env.tmp.path().join("contract-backup.sqlite");

    let out = env
        .cmd()
        .args(["backup", "--output", dest.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "backup",
        &json,
        &[
            "action",
            "source",
            "destination",
            "size_bytes",
            "elapsed_ms",
        ],
    );
    assert_eq!(json["action"], "backed_up");
    assert!(dest.exists(), "backup file must exist");
}
