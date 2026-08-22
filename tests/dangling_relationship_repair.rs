//! A relationship whose entity is gone must be findable and removable.
//!
//! Until this repair existed, such a row was a dead end. `PRAGMA
//! foreign_key_check` reported it after every migration, `ensure_db_ready`
//! migrates on open, and nearly every subcommand calls `ensure_db_ready` — so
//! the file answered every command with the same failure, including
//! `cleanup-orphans`, which is the command named for the repair. That command
//! only ever looked for the mirror case: entities carrying no edges.
//!
//! The row cannot be written while enforcement is on. It exists in files that
//! predate enforcement, or that were written while `PRAGMA foreign_keys = OFF`
//! was in effect for a schema rebuild — which is exactly what the migration
//! runner turns off on purpose.

use sqlite_graphrag::paths::AppPaths;
use sqlite_graphrag::storage::entities;

/// Builds a migrated database holding one edge whose target entity is gone.
///
/// The entity is deleted with enforcement OFF on purpose: with it ON, SQLite
/// would cascade the edge away and there would be nothing to test.
fn database_with_one_dangling_edge(dir: &std::path::Path) -> rusqlite::Connection {
    let db_path = dir.join("graphrag.sqlite");
    let paths =
        AppPaths::resolve(Some(db_path.to_str().expect("utf-8 path"))).expect("resolve paths");
    sqlite_graphrag::storage::connection::ensure_db_ready(&paths).expect("migrate");

    let conn = rusqlite::Connection::open(&db_path).expect("open");
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         INSERT INTO entities (namespace, name, type) VALUES ('global', 'kept', 'concept');
         INSERT INTO entities (namespace, name, type) VALUES ('global', 'doomed', 'concept');
         INSERT INTO relationships (namespace, source_id, target_id, relation)
           SELECT 'global',
                  (SELECT id FROM entities WHERE name = 'kept'),
                  (SELECT id FROM entities WHERE name = 'doomed'),
                  'related';
         DELETE FROM entities WHERE name = 'doomed';",
    )
    .expect("seed dangling edge");
    conn
}

#[test]
fn a_dangling_edge_is_found_and_removed() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let conn = database_with_one_dangling_edge(tmp.path());

    let before: i64 = conn
        .query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))
        .expect("count edges");
    assert_eq!(before, 1, "fixture must leave exactly one edge behind");

    let ids = entities::find_dangling_relationship_ids(&conn, None).expect("find dangling");
    assert_eq!(ids.len(), 1, "the edge with a missing target must be found");

    let removed = entities::delete_relationships_by_ids(&conn, &ids).expect("delete dangling");
    assert_eq!(removed, 1, "the dangling edge must be deleted");

    let left = entities::find_dangling_relationship_ids(&conn, None).expect("re-check");
    assert!(left.is_empty(), "repair must leave nothing dangling");
}

/// The whole point of the repair: afterwards the pragma has nothing to report.
#[test]
fn after_the_repair_the_foreign_key_check_is_clean() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let conn = database_with_one_dangling_edge(tmp.path());

    let violations_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .expect("check before");
    assert!(
        violations_before > 0,
        "fixture must reproduce the state that blocked every command"
    );

    let ids = entities::find_dangling_relationship_ids(&conn, None).expect("find dangling");
    entities::delete_relationships_by_ids(&conn, &ids).expect("delete dangling");

    let violations_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .expect("check after");
    assert_eq!(
        violations_after, 0,
        "after the repair the database must satisfy its own foreign keys"
    );
}

/// Repairing one project must never reach another project's edges.
#[test]
fn the_repair_is_scoped_by_namespace() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let conn = database_with_one_dangling_edge(tmp.path());

    let other = entities::find_dangling_relationship_ids(&conn, Some("some-other-namespace"))
        .expect("find in other namespace");
    assert!(
        other.is_empty(),
        "a different namespace must not see this edge"
    );

    let mine =
        entities::find_dangling_relationship_ids(&conn, Some("global")).expect("find in global");
    assert_eq!(mine.len(), 1, "the owning namespace must see its own edge");
}

/// A healthy database must report nothing, or the repair would delete live data.
#[test]
fn a_healthy_database_has_nothing_to_repair() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("graphrag.sqlite");
    let paths =
        AppPaths::resolve(Some(db_path.to_str().expect("utf-8 path"))).expect("resolve paths");
    sqlite_graphrag::storage::connection::ensure_db_ready(&paths).expect("migrate");

    let conn = rusqlite::Connection::open(&db_path).expect("open");
    conn.execute_batch(
        "INSERT INTO entities (namespace, name, type) VALUES ('global', 'a', 'concept');
         INSERT INTO entities (namespace, name, type) VALUES ('global', 'b', 'concept');
         INSERT INTO relationships (namespace, source_id, target_id, relation)
           SELECT 'global',
                  (SELECT id FROM entities WHERE name = 'a'),
                  (SELECT id FROM entities WHERE name = 'b'),
                  'related';",
    )
    .expect("seed healthy graph");

    let ids = entities::find_dangling_relationship_ids(&conn, None).expect("find dangling");
    assert!(
        ids.is_empty(),
        "a graph with both endpoints present must report no dangling edge"
    );
}
