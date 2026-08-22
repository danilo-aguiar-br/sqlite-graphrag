//! The repair must cover every child table the guard can report, not one.
//!
//! `PRAGMA foreign_key_check` inspects eleven child tables in this schema —
//! `memory_versions`, `memory_chunks`, `memory_urls`, `memory_entities`,
//! `memory_relationships`, `relationships`, `memory_embeddings`,
//! `entity_embeddings`, `chunk_embeddings`, `pending_embeddings` and
//! `entity_connect_seen`. A rebuild-and-rename of `entities`, which is how this
//! state is produced, orphans four of them at once.
//!
//! The first repair written for this covered `relationships` alone, while the
//! migration warning named `cleanup-orphans` for every pair the pragma
//! reported. An operator with a dangling `memory_entities` row would run the
//! suggested command, read `dangling_relationship_count: 0`, and conclude the
//! file was healthy while the pragma kept reporting it. The first test below
//! is the witness for that gap: it asserts the narrow repair finds nothing and
//! the general one finds the row.

use sqlite_graphrag::paths::AppPaths;
use sqlite_graphrag::storage::entities;
use sqlite_graphrag::storage::foreign_keys::{
    delete_foreign_key_violations, find_foreign_key_violations,
};

/// Builds a migrated database holding one `memory_entities` row whose entity is
/// gone — a violation in a table the relationship-specific repair cannot see.
fn database_with_one_dangling_membership(dir: &std::path::Path) -> rusqlite::Connection {
    let db_path = dir.join("graphrag.sqlite");
    let paths =
        AppPaths::resolve(Some(db_path.to_str().expect("utf-8 path"))).expect("resolve paths");
    sqlite_graphrag::storage::connection::ensure_db_ready(&paths).expect("migrate");

    let conn = rusqlite::Connection::open(&db_path).expect("open");
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         INSERT INTO memories (namespace, name, type, description, body, body_hash)
           VALUES ('global', 'mem-a', 'reference', 'd', 'b', 'h');
         INSERT INTO entities (namespace, name, type) VALUES ('global', 'doomed', 'concept');
         INSERT INTO memory_entities (memory_id, entity_id)
           SELECT (SELECT id FROM memories WHERE name = 'mem-a'),
                  (SELECT id FROM entities WHERE name = 'doomed');
         DELETE FROM entities WHERE name = 'doomed';",
    )
    .expect("seed dangling membership");
    conn
}

#[test]
fn the_narrow_repair_misses_what_the_general_one_finds() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let conn = database_with_one_dangling_membership(tmp.path());

    let narrow = entities::find_dangling_relationship_ids(&conn, None).expect("find dangling");
    assert!(
        narrow.is_empty(),
        "the relationship-specific repair cannot see a memory_entities row — \
         this is the gap the general repair exists to close"
    );

    let general = find_foreign_key_violations(&conn).expect("find violations");
    assert_eq!(
        general.len(),
        1,
        "the general repair must see the row the pragma reports"
    );
    assert_eq!(
        general[0].0, "memory_entities",
        "and must name the child table it lives in"
    );
}

#[test]
fn the_general_repair_leaves_the_pragma_clean() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let conn = database_with_one_dangling_membership(tmp.path());

    let before: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .expect("check before");
    assert!(before > 0, "fixture must reproduce a reported violation");

    let violations = find_foreign_key_violations(&conn).expect("find violations");
    let removed = delete_foreign_key_violations(&conn, &violations).expect("delete violations");
    assert_eq!(removed, 1, "the dangling row must be deleted");

    let after: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .expect("check after");
    assert_eq!(
        after, 0,
        "after the repair the file must satisfy its own foreign keys"
    );
}

/// The command runs its repair inside an IMMEDIATE transaction, and the only
/// published test of the command passes `--dry-run`, which never opens one.
///
/// `PRAGMA foreign_key_check` is a read-only pragma, so it is expected to work
/// there — but "expected to" is not a measurement, and the executed path is the
/// one that deletes rows.
#[test]
fn the_repair_works_inside_an_immediate_transaction() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let mut conn = database_with_one_dangling_membership(tmp.path());

    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .expect("open immediate transaction");
    let violations = find_foreign_key_violations(&tx).expect("pragma inside a transaction");
    assert_eq!(
        violations.len(),
        1,
        "the pragma must still see the row here"
    );
    let removed = delete_foreign_key_violations(&tx, &violations).expect("delete inside tx");
    assert_eq!(removed, 1);
    tx.commit().expect("commit");

    let after: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .expect("check after commit");
    assert_eq!(after, 0, "the repair must survive the commit");
}

/// Deleting the same rowid twice must not be counted as a second repair.
#[test]
fn repairing_twice_reports_no_second_repair() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let conn = database_with_one_dangling_membership(tmp.path());

    let violations = find_foreign_key_violations(&conn).expect("find violations");
    delete_foreign_key_violations(&conn, &violations).expect("first repair");
    let again = delete_foreign_key_violations(&conn, &violations).expect("second repair");
    assert_eq!(
        again, 0,
        "a stale rowid must delete nothing, or a re-run would inflate the tally"
    );
}

#[test]
fn a_healthy_database_reports_no_violation() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("graphrag.sqlite");
    let paths =
        AppPaths::resolve(Some(db_path.to_str().expect("utf-8 path"))).expect("resolve paths");
    sqlite_graphrag::storage::connection::ensure_db_ready(&paths).expect("migrate");

    let conn = rusqlite::Connection::open(&db_path).expect("open");
    conn.execute_batch(
        "INSERT INTO memories (namespace, name, type, description, body, body_hash)
           VALUES ('global', 'mem-a', 'reference', 'd', 'b', 'h');
         INSERT INTO entities (namespace, name, type) VALUES ('global', 'alive', 'concept');
         INSERT INTO memory_entities (memory_id, entity_id)
           SELECT (SELECT id FROM memories WHERE name = 'mem-a'),
                  (SELECT id FROM entities WHERE name = 'alive');",
    )
    .expect("seed healthy graph");

    let violations = find_foreign_key_violations(&conn).expect("find violations");
    assert!(
        violations.is_empty(),
        "a healthy file must report nothing, or the repair would delete live data"
    );
}
