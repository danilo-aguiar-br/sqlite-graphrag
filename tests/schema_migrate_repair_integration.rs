#![cfg(feature = "slow-tests")]

//! Suite 3 — schema and migrations: `migrate --rehash` and `--to-llm-only` repair flows
//!
//! Part of the schema suite split by GAP-SG-210: the single file held 923 lines
//! and 21 tests, past the 800-line ceiling this project sets for itself. The
//! shared harness lives in `tests/migration_support/`, which documents the
//! sqlite-vec isolation rule and why `#[serial]` is mandatory here.

#[path = "migration_support/mod.rs"]
mod support;

use serial_test::serial;
use support::{conn_ro, init_isolated_db, sgr_on};

// ---------------------------------------------------------------------------
// v1.0.76 — migrate --rehash and --to-llm-only integration tests
// ---------------------------------------------------------------------------
// These tests exercise the CLI subcommands end-to-end through `assert_cmd`.
// They cover three real-world flows:
//   1. --rehash on a healthy fresh DB is a no-op (status = ok_no_changes).
//   2. --rehash rewrites a corrupted V001 checksum and the next `migrate`
//      run no longer fails with "applied migration V1 is different than
//      filesystem one V1".
//   3. --to-llm-only on a fresh v1.0.76 DB reports no vec tables and a
//      successful schema_version 13 (V013 applied).
//   4. --to-llm-only refuses to run without the explicit --drop-vec-tables
//      safety guard (exit code 1, validation error).

#[test]
#[serial]
fn migrate_rehash_is_noop_on_healthy_db() {
    let (tmp, db_path) = init_isolated_db();

    let output = sgr_on(&tmp, &db_path)
        .args(["migrate", "--rehash"])
        .output()
        .expect("migrate --rehash must run");

    assert!(
        output.status.success(),
        "migrate --rehash must succeed on a healthy DB. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(
        json["status"], "ok_no_changes",
        "healthy DB must report ok_no_changes, got: {stdout}"
    );
    assert_eq!(json["rewritten"].as_array().unwrap().len(), 0);
    assert_eq!(json["inspected"], 16);
    assert_eq!(json["schema_version"], 16);
}

#[test]
#[serial]
fn migrate_rehash_fixes_corrupted_checksum() {
    let (tmp, db_path) = init_isolated_db();

    // Corrupt the V001 checksum so the next `migrate` would fail.
    let conn = conn_ro(&db_path);
    conn.execute_batch(
        "UPDATE refinery_schema_history SET checksum = '999999999999' WHERE version = 1",
    )
    .expect("corrupt V001 checksum");
    drop(conn);

    // GAP-SG-140 (v1.2.0): plain `migrate` now runs the refinery runner with
    // `set_abort_divergent(false)`, mirroring the auto-migration tolerance in
    // `storage::connection::ensure_db_ready`, because a legacy database carries
    // a divergent checksum for a migration V013 already superseded. Aborting
    // would make `migrate` fail on exactly the databases it must upgrade. So
    // the pre-condition flipped from "plain migrate FAILS" to "plain migrate
    // tolerates the divergence and leaves the corrupted row untouched" — which
    // is what keeps `--rehash` the only repair path. The question under test is
    // unchanged: does `--rehash` detect and rewrite a corrupted V001 checksum?
    let tolerated = sgr_on(&tmp, &db_path)
        .args(["migrate"])
        .output()
        .expect("migrate must run");
    assert!(
        tolerated.status.success(),
        "migrate must tolerate a divergent checksum (GAP-SG-140), got: {:?} stderr={}",
        tolerated.status,
        String::from_utf8_lossy(&tolerated.stderr)
    );
    let still_corrupt: String = conn_ro(&db_path)
        .query_row(
            "SELECT checksum FROM refinery_schema_history WHERE version = 1",
            [],
            |r| r.get(0),
        )
        .expect("read V001 checksum");
    assert_eq!(
        still_corrupt, "999999999999",
        "plain migrate must NOT repair the checksum; only --rehash does"
    );

    // `migrate --rehash` should detect the mismatch, rewrite the row,
    // and exit 0 with status=ok_rewritten.
    let good = sgr_on(&tmp, &db_path)
        .args(["migrate", "--rehash"])
        .output()
        .expect("migrate --rehash must run");
    assert!(
        good.status.success(),
        "migrate --rehash must succeed. stderr={}",
        String::from_utf8_lossy(&good.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&good.stdout).expect("JSON");
    assert_eq!(json["status"], "ok_rewritten");
    assert_eq!(json["rewritten"].as_array().unwrap().len(), 1);
    assert_eq!(json["rewritten"][0]["version"], 1);
    assert_eq!(json["rewritten"][0]["name"], "init");
    assert_eq!(json["rewritten"][0]["old_checksum"], "999999999999");

    // And a subsequent plain `migrate` should now succeed.
    let after = sgr_on(&tmp, &db_path)
        .args(["migrate"])
        .output()
        .expect("migrate must run");
    assert!(
        after.status.success(),
        "migrate must succeed after rehash. stderr={}",
        String::from_utf8_lossy(&after.stderr)
    );
}

#[test]
#[serial]
fn migrate_to_llm_only_reports_no_vec_tables_on_fresh_db() {
    let (tmp, db_path) = init_isolated_db();

    let output = sgr_on(&tmp, &db_path)
        .args(["migrate", "--to-llm-only", "--drop-vec-tables"])
        .output()
        .expect("migrate --to-llm-only must run");

    assert!(
        output.status.success(),
        "migrate --to-llm-only must succeed on a fresh v1.0.76 DB. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["schema_version"], 16);
    assert_eq!(json["v013_applied"], true);
    assert_eq!(
        json["vec_tables_were_present"], false,
        "fresh v1.0.76 DBs must not have vec0 virtual tables"
    );
    assert_eq!(json["rehashed"].as_array().unwrap().len(), 0);
}

#[test]
#[serial]
fn migrate_to_llm_only_requires_drop_vec_tables_safety_guard() {
    let (tmp, db_path) = init_isolated_db();

    let output = sgr_on(&tmp, &db_path)
        .args(["migrate", "--to-llm-only"])
        .output()
        .expect("migrate --to-llm-only must run");

    assert!(
        !output.status.success(),
        "migrate --to-llm-only without --drop-vec-tables must refuse to run"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("JSON");
    assert_eq!(json["code"], 1, "validation error code 1 expected");
    let msg = json["message"].as_str().unwrap_or("").to_string();
    assert!(
        msg.contains("--drop-vec-tables"),
        "error message must mention --drop-vec-tables, got: {msg}"
    );
}

#[test]
#[serial]
fn migrate_rehash_fixes_null_applied_on() {
    let (tmp, db_path) = init_isolated_db();

    // NULL out applied_on for all rows to simulate the G40 bug.
    let conn = conn_ro(&db_path);
    conn.execute_batch("UPDATE refinery_schema_history SET applied_on = NULL")
        .expect("nullify applied_on");
    drop(conn);

    // migrate --rehash must succeed and fix the NULL rows.
    let output = sgr_on(&tmp, &db_path)
        .args(["migrate", "--rehash"])
        .output()
        .expect("migrate --rehash must run");

    assert!(
        output.status.success(),
        "migrate --rehash must succeed on DB with NULL applied_on. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert!(
        json["null_rows_fixed"].as_u64().unwrap_or(0) > 0,
        "must report null_rows_fixed > 0, got: {}",
        json["null_rows_fixed"]
    );

    // A subsequent plain migrate must also succeed (runner reads applied_on).
    let after = sgr_on(&tmp, &db_path)
        .args(["migrate"])
        .output()
        .expect("migrate must run");
    assert!(
        after.status.success(),
        "migrate must succeed after rehash fixed NULLs. stderr={}",
        String::from_utf8_lossy(&after.stderr)
    );

    // Verify zero NULL rows remain via rusqlite.
    let conn = conn_ro(&db_path);
    let null_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM refinery_schema_history WHERE applied_on IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(null_count, 0, "no NULL applied_on rows must remain");
}

#[test]
#[serial]
fn migrate_to_llm_only_fixes_null_applied_on() {
    let (tmp, db_path) = init_isolated_db();

    let conn = conn_ro(&db_path);
    conn.execute_batch("UPDATE refinery_schema_history SET applied_on = NULL")
        .expect("nullify applied_on");
    drop(conn);

    let output = sgr_on(&tmp, &db_path)
        .args(["migrate", "--to-llm-only", "--drop-vec-tables"])
        .output()
        .expect("migrate --to-llm-only must run");

    assert!(
        output.status.success(),
        "migrate --to-llm-only must succeed with NULL applied_on. stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert!(
        json["null_rows_fixed"].as_u64().unwrap_or(0) > 0,
        "must report null_rows_fixed > 0, got: {}",
        json["null_rows_fixed"]
    );
    assert_eq!(json["status"], "ok");
}
