//! Namespace scoping and ID-resolution invariants of `merge-entities`.

use super::*;

// v1.1.1 (P5): ID resolution is namespace-scoped — a homonym in another
// namespace must NOT be reachable through its ID from the wrong namespace.
#[test]
fn find_entity_name_by_id_disambiguates_homonyms_across_namespaces() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE entities (
            id INTEGER PRIMARY KEY,
            namespace TEXT NOT NULL,
            name TEXT NOT NULL,
            UNIQUE(namespace, name)
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO entities (id, namespace, name)
         VALUES (1, 'ns-a', 'auth'), (2, 'ns-b', 'auth')",
        [],
    )
    .unwrap();

    // Same name in two namespaces: each ID resolves only in its own.
    assert_eq!(
        find_entity_name_by_id(&conn, "ns-a", 1, true).unwrap(),
        ("auth".to_string(), "ns-a".to_string())
    );
    assert_eq!(
        find_entity_name_by_id(&conn, "ns-b", 2, true).unwrap(),
        ("auth".to_string(), "ns-b".to_string())
    );
    let err = find_entity_name_by_id(&conn, "ns-a", 2, true).unwrap_err();
    assert_eq!(err.exit_code(), 4, "cross-namespace ID must be NotFound");
    assert!(err.to_string().contains("id=2"), "obtido: {err}");

    // v1.1.03: with enforce_namespace=false, id=2 resolves from any
    // namespace and reports the namespace it actually lives in.
    let (name, ns_actual) = find_entity_name_by_id(&conn, "ns-a", 2, false).unwrap();
    assert_eq!(name, "auth");
    assert_eq!(ns_actual, "ns-b");
}

#[test]
fn find_entity_name_by_id_missing_id_is_not_found() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE entities (
            id INTEGER PRIMARY KEY,
            namespace TEXT NOT NULL,
            name TEXT NOT NULL
        );",
    )
    .unwrap();
    let err = find_entity_name_by_id(&conn, "global", 99, true).unwrap_err();
    assert_eq!(err.exit_code(), 4);
}

// v1.1.1 (P5): clap-level exclusivity between name-based and ID-based
// selectors, and requiredness of at least one selector per side.
#[derive(clap::Parser)]
struct TestCli {
    #[command(flatten)]
    args: MergeEntitiesArgs,
}

#[test]
fn clap_rejects_names_combined_with_ids() {
    use clap::Parser;
    let err =
        match TestCli::try_parse_from(["t", "--names", "a,b", "--ids", "1,2", "--into", "tgt"]) {
            Ok(_) => panic!("expected argument conflict"),
            Err(e) => e,
        };
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn clap_rejects_into_combined_with_into_id() {
    use clap::Parser;
    let err =
        match TestCli::try_parse_from(["t", "--names", "a", "--into", "tgt", "--into-id", "3"]) {
            Ok(_) => panic!("expected argument conflict"),
            Err(e) => e,
        };
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

/// v1.1.05 Bug 4: clap value_delimiter must keep target id out of sources
/// when the operator writes `--ids 35575,35340 --into-id 35340` — the
/// early self-ref guard must fire without needing a live database.
#[test]
fn self_referential_ids_rejected_before_db() {
    let args = MergeEntitiesArgs {
        names: vec![],
        ids: vec![35575, 35340],
        into: None,
        into_id: Some(35340),
        namespace: Some("global".into()),
        format: OutputFormat::Json,
        json: false,
        db: Some("/nonexistent/no-db.sqlite".into()),
        cross_namespace: true,
    };
    let err = run(args).expect_err("self-ref must fail");
    assert_eq!(err.exit_code(), 1, "validation exit");
    let msg = err.to_string();
    assert!(
        msg.contains("self-referential") || msg.contains("35340"),
        "expected self-ref message, got: {msg}"
    );
}

#[test]
fn clap_parses_ids_with_target_in_list() {
    use clap::Parser;
    let ok = TestCli::try_parse_from([
        "t",
        "--ids",
        "35575,35340",
        "--into-id",
        "35340",
        "--cross-namespace",
    ])
    .expect("must parse");
    assert_eq!(ok.args.ids, vec![35575, 35340]);
    assert_eq!(ok.args.into_id, Some(35340));
    assert!(ok.args.cross_namespace);
}

#[test]
fn clap_requires_a_source_and_a_target_selector() {
    use clap::Parser;
    assert!(TestCli::try_parse_from(["t", "--into", "tgt"]).is_err());
    assert!(TestCli::try_parse_from(["t", "--names", "a"]).is_err());
    let ok = match TestCli::try_parse_from(["t", "--ids", "1,2", "--into-id", "3"]) {
        Ok(cli) => cli,
        Err(e) => panic!("expected successful parse: {e}"),
    };
    assert_eq!(ok.args.ids, vec![1, 2]);
    assert_eq!(ok.args.into_id, Some(3));
    assert!(ok.args.names.is_empty());
    assert!(ok.args.into.is_none());
}

#[test]
fn merge_entities_response_serializes_all_fields() {
    let resp = MergeEntitiesResponse {
        action: "merged".to_string(),
        sources: vec!["auth".to_string(), "authentication".to_string()],
        target: "auth-service".to_string(),
        namespace: "global".to_string(),
        target_id: 1,
        relationships_moved: 7,
        entities_removed: 2,
        elapsed_ms: 15,
    };
    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["action"], "merged");
    assert_eq!(json["target"], "auth-service");
    assert_eq!(json["namespace"], "global");
    assert_eq!(json["relationships_moved"], 7);
    assert_eq!(json["entities_removed"], 2);
    let sources = json["sources"].as_array().expect("must be array");
    assert_eq!(sources.len(), 2);
    assert!(json["elapsed_ms"].is_number());
}

#[test]
fn merge_entities_response_action_is_merged() {
    let resp = MergeEntitiesResponse {
        action: "merged".to_string(),
        sources: vec!["src".to_string()],
        target: "tgt".to_string(),
        namespace: "ns".to_string(),
        target_id: 1,
        relationships_moved: 0,
        entities_removed: 1,
        elapsed_ms: 0,
    };
    assert_eq!(resp.action, "merged");
}

#[test]
fn merge_entities_response_empty_sources_serializes() {
    let resp = MergeEntitiesResponse {
        action: "merged".to_string(),
        sources: vec![],
        target: "target".to_string(),
        namespace: "global".to_string(),
        target_id: 1,
        relationships_moved: 0,
        entities_removed: 0,
        elapsed_ms: 1,
    };
    let json = serde_json::to_value(&resp).expect("serialization failed");
    let sources = json["sources"].as_array().expect("must be array");
    assert_eq!(sources.len(), 0);
}

#[test]
fn merge_entities_response_with_zero_relationships_moved() {
    let resp = MergeEntitiesResponse {
        action: "merged".to_string(),
        sources: vec!["src-a".to_string()],
        target: "tgt".to_string(),
        namespace: "global".to_string(),
        target_id: 1,
        relationships_moved: 0,
        entities_removed: 1,
        elapsed_ms: 5,
    };
    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["relationships_moved"], 0);
    assert_eq!(json["entities_removed"], 1);
}

#[test]
fn merge_entities_response_multiple_sources() {
    let resp = MergeEntitiesResponse {
        action: "merged".to_string(),
        sources: vec!["a".into(), "b".into(), "c".into()],
        target: "canonical".to_string(),
        namespace: "proj".to_string(),
        target_id: 1,
        relationships_moved: 12,
        entities_removed: 3,
        elapsed_ms: 42,
    };
    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["entities_removed"], 3);
    let sources = json["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 3);
}

// v1.1.03: integration setup — a fully migrated DB on disk so run() can
// open it via AppPaths + open_rw. Returns the tempdir (kept alive for the
// test lifetime), the seeded connection, and the DB file path.
fn setup_migrated_db_on_disk() -> (tempfile::TempDir, rusqlite::Connection, std::path::PathBuf) {
    crate::storage::connection::register_vec_extension();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("test.db");
    let mut conn = rusqlite::Connection::open(&db_path).expect("open");
    crate::migrations::runner().run(&mut conn).expect("migrate");
    (tmp, conn, db_path)
}

// v1.1.03 (Bug 3): --cross-namespace allows merging a source that lives in
// a DIFFERENT namespace into the target. Relationships are retargeted and
// the source row is deleted from its origin namespace.
#[test]
fn cross_namespace_merges_source_from_other_namespace() {
    let (_tmp, conn, db_path) = setup_migrated_db_on_disk();
    // Target lives in "global".
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','tgt','concept')",
        [],
    )
    .unwrap();
    let tgt_id = conn.last_insert_rowid();
    // Source lives in "ai-sdd" — a homonym in a different namespace.
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('ai-sdd','dup-src','concept')",
        [],
    )
    .unwrap();
    let src_id = conn.last_insert_rowid();
    // A third entity in "global" that the source points to, so the
    // retarget produces an observable relationship_moved > 0.
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','third','concept')",
        [],
    )
    .unwrap();
    let third_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO relationships (namespace, source_id, target_id, relation, weight) \
         VALUES ('ai-sdd', ?1, ?2, 'related', 0.5)",
        params![src_id, third_id],
    )
    .unwrap();
    // Drop the seeding connection so run() can open the DB exclusively.
    drop(conn);

    let args = MergeEntitiesArgs {
        names: vec![],
        ids: vec![src_id],
        into: None,
        into_id: Some(tgt_id),
        namespace: Some("global".to_string()),
        format: OutputFormat::Json,
        json: false,
        db: Some(db_path.to_string_lossy().into_owned()),
        cross_namespace: true,
    };
    run(args).expect("cross-namespace merge must succeed");

    // Reopen and verify: source deleted, relationship retargeted to target.
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let src_remaining: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE id = ?1",
            params![src_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(src_remaining, 0, "cross-namespace source must be deleted");
    let moved: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM relationships WHERE source_id = ?1 AND target_id = ?2",
            params![tgt_id, third_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        moved > 0,
        "relationship must be retargeted to the target entity"
    );
}

// v1.1.03 (Bug 3): without --cross-namespace, a source ID from another
// namespace is rejected with NotFound — preserves the same-namespace safety
// (non-regression of the v1.1.1 P5 behaviour).
#[test]
fn cross_namespace_default_false_rejects_cross_id() {
    let (_tmp, conn, db_path) = setup_migrated_db_on_disk();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','tgt','concept')",
        [],
    )
    .unwrap();
    let tgt_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('ai-sdd','dup-src','concept')",
        [],
    )
    .unwrap();
    let src_id = conn.last_insert_rowid();
    drop(conn);

    let args = MergeEntitiesArgs {
        names: vec![],
        ids: vec![src_id],
        into: None,
        into_id: Some(tgt_id),
        namespace: Some("global".to_string()),
        format: OutputFormat::Json,
        json: false,
        db: Some(db_path.to_string_lossy().into_owned()),
        cross_namespace: false,
    };
    let err = run(args).expect_err("default must reject cross-namespace ID");
    assert_eq!(err.exit_code(), 4, "cross-namespace ID must be NotFound");
}

// v1.1.03 (Bug 3): even with --cross-namespace, the TARGET must still exist
// in the resolved namespace — cross-namespace only relaxes SOURCES.
#[test]
fn cross_namespace_target_must_still_be_in_resolved_namespace() {
    let (_tmp, conn, db_path) = setup_migrated_db_on_disk();
    // Target lives in "ai-sdd", but we will resolve namespace to "global".
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('ai-sdd','tgt','concept')",
        [],
    )
    .unwrap();
    let tgt_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','src','concept')",
        [],
    )
    .unwrap();
    let src_id = conn.last_insert_rowid();
    drop(conn);

    let args = MergeEntitiesArgs {
        names: vec![],
        ids: vec![src_id],
        into: None,
        into_id: Some(tgt_id),
        namespace: Some("global".to_string()),
        format: OutputFormat::Json,
        json: false,
        db: Some(db_path.to_string_lossy().into_owned()),
        cross_namespace: true,
    };
    let err = run(args).expect_err("target in wrong namespace must fail");
    assert_eq!(
        err.exit_code(),
        4,
        "target must still be NotFound in the resolved namespace"
    );
}
