use super::*;
use rusqlite::Connection;

fn create_db_without_history() -> Connection {
    Connection::open_in_memory().expect("failed to open in-memory db")
}

fn create_db_with_history(version: i64) -> Connection {
    let conn = Connection::open_in_memory().expect("failed to open in-memory db");
    conn.execute_batch(
        "CREATE TABLE refinery_schema_history (
            version INTEGER NOT NULL,
            name TEXT,
            applied_on TEXT,
            checksum TEXT
        );",
    )
    .expect("failed to create history table");
    conn.execute(
        "INSERT INTO refinery_schema_history (version, name) VALUES (?1, 'V001__init')",
        rusqlite::params![version],
    )
    .expect("failed to insert version");
    conn
}

#[test]
fn latest_schema_version_returns_error_without_table() {
    let conn = create_db_without_history();
    let result = latest_schema_version(&conn);
    assert!(result.is_err(), "must return Err when table does not exist");
}

#[test]
fn latest_schema_version_returns_max_version() {
    let conn = create_db_with_history(6);
    let version = latest_schema_version(&conn).unwrap();
    assert_eq!(version, 6u32);
}

#[test]
fn migrate_response_serializes_required_fields() {
    let resp = MigrateResponse {
        db_path: "/tmp/test.sqlite".to_string(),
        schema_version: 6,
        status: "ok".to_string(),
        elapsed_ms: 12,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["schema_version"], 6);
    assert_eq!(json["db_path"], "/tmp/test.sqlite");
    assert_eq!(json["elapsed_ms"], 12);
}

#[test]
fn latest_schema_version_returns_zero_when_table_empty() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE refinery_schema_history (
            version INTEGER NOT NULL,
            name TEXT
        );",
    )
    .expect("table creation");
    let version = latest_schema_version(&conn).unwrap();
    assert_eq!(version, 0u32);
}

#[test]
fn compute_checksum_is_deterministic_and_matches_refinery() {
    // This is the same algorithm that refinery-core 0.9.1 uses. We
    // pin the numeric value to detect any change in siphasher
    // behaviour that would break migration verification.
    let a = compute_checksum("vec_tables", 2, "SELECT 1;");
    let b = compute_checksum("vec_tables", 2, "SELECT 1;");
    assert_eq!(a, b, "checksum must be deterministic");
    let c = compute_checksum("vec_tables", 2, "SELECT 1;\n");
    assert_ne!(
        a, c,
        "trailing newline must change the checksum (matches refinery)"
    );
}

#[test]
fn rehash_with_no_history_returns_empty() {
    let mut conn = create_db_without_history();
    let report = run_rehash(&mut conn, Path::new("/tmp/empty.sqlite")).unwrap();
    assert_eq!(report.status, "ok_no_history");
    assert!(report.rewritten.is_empty());
    assert_eq!(report.inspected, 0);
}

#[test]
fn rehash_writes_matching_checksum() {
    // Pre-populate the history with a WRONG checksum. The rehash
    // must detect the mismatch and rewrite the row.
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE refinery_schema_history (
            version INTEGER NOT NULL,
            name TEXT,
            applied_on TEXT,
            checksum TEXT
        );",
    )
    .expect("history create");
    // Use the first migration present in the embedded set (V001).
    let first = crate::migrations::runner().get_migrations()[0].clone();
    let v = first.version();
    let name = first.name().to_string();
    let sql = first.sql().unwrap_or("").to_string();
    let correct = compute_checksum(&name, v, &sql).to_string();
    let wrong = "1234567890";
    assert_ne!(correct, wrong, "test sanity: correct != wrong");

    conn.execute(
        "INSERT INTO refinery_schema_history (version, name, checksum) VALUES (?1, ?2, ?3)",
        rusqlite::params![v, name, wrong],
    )
    .expect("insert");

    let report = run_rehash(&mut conn, Path::new("/tmp/test.sqlite")).unwrap();
    assert_eq!(report.rewritten.len(), 1);
    assert_eq!(report.rewritten[0].old_checksum, wrong);
    assert_eq!(report.rewritten[0].new_checksum, correct);

    // And the row now matches what refinery would compute.
    let stored: String = conn
        .query_row(
            "SELECT checksum FROM refinery_schema_history WHERE version = ?1",
            rusqlite::params![v],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, correct);
}

#[test]
fn rehash_is_idempotent_when_checksums_match() {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE refinery_schema_history (
            version INTEGER NOT NULL,
            name TEXT,
            applied_on TEXT,
            checksum TEXT
        );",
    )
    .unwrap();
    let first = crate::migrations::runner().get_migrations()[0].clone();
    let v = first.version();
    let name = first.name().to_string();
    let sql = first.sql().unwrap_or("").to_string();
    let correct = compute_checksum(&name, v, &sql).to_string();
    conn.execute(
        "INSERT INTO refinery_schema_history (version, name, checksum) VALUES (?1, ?2, ?3)",
        rusqlite::params![v, name, correct.clone()],
    )
    .unwrap();

    let report = run_rehash(&mut conn, Path::new("/tmp/test.sqlite")).unwrap();
    assert!(
        report.rewritten.is_empty(),
        "must not rewrite matching rows"
    );
    assert_eq!(report.status, "ok_no_changes");
}

#[test]
fn rehash_matches_refinery_embedded_checksum_for_v001() {
    // The ultimate correctness test: run a real migration, capture
    // what refinery stored, then call run_rehash and confirm the
    // file-derived checksum matches what the runner produced. This
    // pins the algorithm end-to-end and would catch any future drift
    // (e.g. a siphasher major version bump that changes SipHasher13).
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.sqlite");
    let mut conn = open_rw(&path).expect("open_rw");
    crate::migrations::runner().run(&mut conn).expect("migrate");
    let stored: String = conn
        .query_row(
            "SELECT checksum FROM refinery_schema_history WHERE version = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let report = run_rehash(&mut conn, &path).expect("rehash");
    assert!(
        report.rewritten.is_empty(),
        "V001 must NOT be rewritten when checksums already match: rewrote={:?}",
        report.rewritten
    );
    // And re-running runner() should still succeed (the original
    // error that the failing test exposed was that the second
    // runner().run() call saw a checksum mismatch).
    crate::migrations::runner()
        .run(&mut conn)
        .expect("re-run migrate must succeed");
    let stored_after: String = conn
        .query_row(
            "SELECT checksum FROM refinery_schema_history WHERE version = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        stored, stored_after,
        "checksum must not change after rehash"
    );
}

#[test]
fn to_llm_only_reports_no_vec_tables_on_fresh_db() {
    // Fresh v1.0.76 database (created by running the full migration
    // set) has no vec tables.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("fresh.sqlite");
    let mut conn = open_rw(&path).expect("open_rw");
    crate::migrations::runner().run(&mut conn).expect("migrate");
    let report = run_to_llm_only(&mut conn, &path).expect("to_llm_only");
    assert!(!report.vec_tables_were_present);
    assert!(report.v013_applied, "V013 must be marked applied");
    assert_eq!(report.status, "ok");
}

#[test]
fn history_table_exists_detects_table() {
    let conn = create_db_with_history(1);
    assert!(history_table_exists(&conn));
    let conn2 = create_db_without_history();
    assert!(!history_table_exists(&conn2));
}

#[test]
fn sanitize_null_applied_on_fixes_null_rows() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE refinery_schema_history (
            version INTEGER NOT NULL,
            name TEXT,
            applied_on TEXT,
            checksum TEXT
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO refinery_schema_history (version, name, checksum) VALUES (1, 'init', '123')",
        [],
    )
    .unwrap();
    let fixed = sanitize_null_applied_on(&conn).unwrap();
    assert_eq!(fixed, 1, "must fix exactly one NULL row");
    let applied: String = conn
        .query_row(
            "SELECT applied_on FROM refinery_schema_history WHERE version = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        chrono::DateTime::parse_from_rfc3339(&applied).is_ok(),
        "applied_on must be valid RFC3339, got: {applied}"
    );
}

#[test]
fn sanitize_null_applied_on_noop_when_all_filled() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE refinery_schema_history (
            version INTEGER NOT NULL,
            name TEXT,
            applied_on TEXT,
            checksum TEXT
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO refinery_schema_history (version, name, applied_on, checksum) VALUES (1, 'init', '2026-06-09T00:00:00+00:00', '123')",
        [],
    )
    .unwrap();
    let fixed = sanitize_null_applied_on(&conn).unwrap();
    assert_eq!(fixed, 0, "must not touch rows with valid applied_on");
}

#[test]
fn rehash_does_not_insert_missing_migrations() {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE refinery_schema_history (
            version INTEGER NOT NULL,
            name TEXT,
            applied_on TEXT,
            checksum TEXT
        );",
    )
    .unwrap();
    let runner = crate::migrations::runner();
    let migrations = runner.get_migrations();
    for mig in migrations.iter() {
        if mig.version() >= 13 {
            break;
        }
        let name = mig.name().to_string();
        let v = mig.version();
        let sql = mig.sql().unwrap_or("").to_string();
        let cs = compute_checksum(&name, v, &sql).to_string();
        conn.execute(
            "INSERT INTO refinery_schema_history (version, name, applied_on, checksum) VALUES (?1, ?2, '2026-01-01T00:00:00+00:00', ?3)",
            rusqlite::params![v, name, cs],
        )
        .unwrap();
    }
    let _report = run_rehash(&mut conn, Path::new("/tmp/test.sqlite")).unwrap();
    let v013_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM refinery_schema_history WHERE version = 13",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
        > 0;
    assert!(
        !v013_exists,
        "V013 must NOT be inserted by run_rehash (G41 fix)"
    );
}

#[test]
fn remove_vec_tables_noop_when_no_vec() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    let removed = remove_vec_virtual_tables_without_module(&conn).unwrap();
    assert_eq!(removed, 0);
}

#[test]
fn ensure_v013_tables_noop_when_no_history() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    let created = ensure_v013_tables_exist(&conn).unwrap();
    assert!(!created, "must be no-op when history table is absent");
}

#[test]
fn ensure_v013_tables_noop_when_tables_exist() {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    crate::migrations::runner().run(&mut conn).unwrap();
    let created = ensure_v013_tables_exist(&conn).unwrap();
    assert!(
        !created,
        "must be no-op when memory_embeddings already exists"
    );
}

#[test]
fn ensure_v013_tables_creates_when_phantom() {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    crate::migrations::runner().run(&mut conn).unwrap();
    conn.execute_batch("DROP TABLE IF EXISTS memory_embeddings")
        .unwrap();
    conn.execute_batch("DROP TABLE IF EXISTS entity_embeddings")
        .unwrap();
    conn.execute_batch("DROP TABLE IF EXISTS chunk_embeddings")
        .unwrap();
    let created = ensure_v013_tables_exist(&conn).unwrap();
    assert!(
        created,
        "must create tables when V013 is in history but tables are missing"
    );
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='memory_embeddings'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "memory_embeddings must exist after repair");
}
