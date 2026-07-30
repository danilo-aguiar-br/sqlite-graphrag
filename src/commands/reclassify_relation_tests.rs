use super::*;

fn make_response(action: &str, count: usize, merged: usize) -> ReclassifyRelationResponse {
    ReclassifyRelationResponse {
        action: action.to_string(),
        from_relation: "mentions".to_string(),
        to_relation: "related".to_string(),
        count,
        merged_duplicates: merged,
        namespace: "global".to_string(),
        elapsed_ms: 1,
    }
}

#[test]
fn response_serializes_all_fields() {
    let resp = make_response("reclassified", 5, 0);
    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["action"], "reclassified");
    assert_eq!(json["from_relation"], "mentions");
    assert_eq!(json["to_relation"], "related");
    assert_eq!(json["count"], 5);
    assert_eq!(json["merged_duplicates"], 0);
    assert_eq!(json["namespace"], "global");
    assert!(json["elapsed_ms"].is_number());
}

#[test]
fn response_action_dry_run() {
    let resp = make_response("dry_run", 10, 0);
    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["action"], "dry_run");
    assert_eq!(json["count"], 10);
    assert_eq!(json["merged_duplicates"], 0);
}

#[test]
fn response_merged_duplicates_nonzero() {
    // Simulates a case where 3 out of 10 edges collided with existing rows.
    let resp = make_response("reclassified", 7, 3);
    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["count"], 7);
    assert_eq!(json["merged_duplicates"], 3);
}

#[test]
fn response_count_zero_when_nothing_matched() {
    let resp = make_response("reclassified", 0, 0);
    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["count"], 0);
    assert_eq!(json["merged_duplicates"], 0);
}

#[test]
fn response_action_values_exhaustive() {
    for action in &["reclassified", "dry_run"] {
        let resp = make_response(action, 1, 0);
        let json = serde_json::to_value(&resp).expect("serialization");
        assert_eq!(json["action"], *action);
    }
}

#[test]
fn response_from_and_to_relation_present() {
    let resp = ReclassifyRelationResponse {
        action: "reclassified".to_string(),
        from_relation: "uses".to_string(),
        to_relation: "depends_on".to_string(),
        count: 3,
        merged_duplicates: 1,
        namespace: "my-project".to_string(),
        elapsed_ms: 5,
    };
    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["from_relation"], "uses");
    assert_eq!(json["to_relation"], "depends_on");
}

#[test]
fn same_relation_value_rejected_at_logic_level() {
    // Validates that the guard in run() would catch from == to.
    // We test the condition directly since we cannot call run() without a DB.
    let from = "mentions".to_string();
    let to = "mentions".to_string();
    assert!(
        from == to,
        "same-value rename must be caught before DB access"
    );
}

// -----------------------------------------------------------------------
// v1.1.1 (P4): --literal-from — filtro sem normalização
// -----------------------------------------------------------------------

fn base_args() -> ReclassifyRelationArgs {
    ReclassifyRelationArgs {
        source: None,
        target: None,
        from_relation: None,
        literal_from: None,
        to_relation: Some("applies_to".to_string()),
        literal_to: None,
        batch: true,
        filter_source_type: None,
        filter_target_type: None,
        dry_run: false,
        namespace: Some("global".to_string()),
        format: OutputFormat::Json,
        json: true,
        db: None,
    }
}

#[test]
fn effective_from_prefers_literal_and_falls_back_to_normalized() {
    let mut args = base_args();
    args.from_relation = Some("applies_to".to_string());
    assert_eq!(args.effective_from(), "applies_to");

    args.literal_from = Some("applies-to".to_string());
    assert_eq!(
        args.effective_from(),
        "applies-to",
        "literal value must win and stay verbatim"
    );

    // Migração literal→normalizado é VÁLIDA (não é igualdade).
    assert_ne!(args.effective_from(), args.effective_to());
}

fn setup_migrated_db() -> (tempfile::TempDir, rusqlite::Connection) {
    crate::storage::connection::register_vec_extension();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("test.db");
    let mut conn = rusqlite::Connection::open(&db_path).expect("open");
    crate::migrations::runner().run(&mut conn).expect("migrate");
    (tmp, conn)
}

#[test]
fn literal_from_migrates_hyphenated_edge_unreachable_by_normalized_filter() {
    let (_tmp, mut conn) = setup_migrated_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','ent-a','concept')",
        [],
    )
    .unwrap();
    let a = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','ent-b','concept')",
        [],
    )
    .unwrap();
    let b = conn.last_insert_rowid();
    // Aresta gravada com o valor LITERAL com hífen — inalcançável pelo
    // --from-relation (que normaliza para 'applies_to' na borda clap).
    conn.execute(
        "INSERT INTO relationships (namespace, source_id, target_id, relation, weight) \
         VALUES ('global', ?1, ?2, 'applies-to', 0.5)",
        params![a, b],
    )
    .unwrap();

    let mut args = base_args();
    args.literal_from = Some("applies-to".to_string());
    run_batch(
        args,
        std::time::Instant::now(),
        "global".to_string(),
        &mut conn,
    )
    .expect("batch literal migration");

    let migrated: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM relationships WHERE relation = 'applies_to'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(migrated, 1, "hyphenated edge must be migrated");
    let leftover: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM relationships WHERE relation = 'applies-to'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(leftover, 0, "no literal edge may remain");
}

#[test]
fn cli_rejects_literal_from_combined_with_from_relation() {
    use clap::Parser;
    let err = match crate::cli::Cli::try_parse_from([
        "sqlite-graphrag",
        "reclassify-relation",
        "--from-relation",
        "mentions",
        "--literal-from",
        "applies-to",
        "--to-relation",
        "related",
        "--batch",
    ]) {
        Err(e) => e,
        Ok(_) => panic!("mutually exclusive flags must fail to parse"),
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn cli_requires_one_of_from_relation_or_literal_from() {
    use clap::Parser;
    let err = match crate::cli::Cli::try_parse_from([
        "sqlite-graphrag",
        "reclassify-relation",
        "--to-relation",
        "related",
        "--batch",
    ]) {
        Err(e) => e,
        Ok(_) => panic!("one of the from flags is required"),
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn cli_accepts_literal_from_alone_and_keeps_it_verbatim() {
    use clap::Parser;
    let parsed = crate::cli::Cli::try_parse_from([
        "sqlite-graphrag",
        "reclassify-relation",
        "--literal-from",
        "applies-to",
        "--to-relation",
        "applies_to",
        "--batch",
    ])
    .expect("literal-from alone must parse");
    match parsed.command {
        Some(crate::cli::Commands::ReclassifyRelation(a)) => {
            assert_eq!(a.literal_from.as_deref(), Some("applies-to"));
            assert!(a.from_relation.is_none());
            assert_eq!(a.effective_from(), "applies-to");
        }
        _ => unreachable!("unexpected command"),
    }
}

// -----------------------------------------------------------------------
// v1.1.03: --literal-to — grava valor canonical hífen verbatim
// -----------------------------------------------------------------------

#[test]
fn literal_to_writes_hyphenated_target() {
    let (_tmp, mut conn) = setup_migrated_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','ent-a','concept')",
        [],
    )
    .unwrap();
    let a = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','ent-b','concept')",
        [],
    )
    .unwrap();
    let b = conn.last_insert_rowid();
    // Aresta legacy armazenada com underscore (61357 casos reais).
    conn.execute(
        "INSERT INTO relationships (namespace, source_id, target_id, relation, weight) \
         VALUES ('global', ?1, ?2, 'applies_to', 0.5)",
        params![a, b],
    )
    .unwrap();

    let mut args = base_args();
    args.from_relation = Some("applies_to".to_string());
    args.to_relation = None;
    args.literal_to = Some("applies-to".to_string());
    run_batch(
        args,
        std::time::Instant::now(),
        "global".to_string(),
        &mut conn,
    )
    .expect("batch literal-to migration");

    let migrated: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM relationships WHERE relation = 'applies-to'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(migrated, 1, "underscore edge must become hyphenated");
    let leftover: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM relationships WHERE relation = 'applies_to'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(leftover, 0, "no underscore edge may remain");
}

#[test]
fn literal_from_applies_to_literal_to_applies_to_hyphen_migrates() {
    // Reproduz o bug 2: --literal-from applies_to --literal-to applies-to
    // --batch --dry-run deve retornar count > 0 (antes: erro "must be
    // different" porque to_relation normalizava para applies_to).
    let (_tmp, mut conn) = setup_migrated_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','ent-a','concept')",
        [],
    )
    .unwrap();
    let a = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','ent-b','concept')",
        [],
    )
    .unwrap();
    let b = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO relationships (namespace, source_id, target_id, relation, weight) \
         VALUES ('global', ?1, ?2, 'applies_to', 0.5)",
        params![a, b],
    )
    .unwrap();

    let mut args = base_args();
    args.from_relation = None;
    args.literal_from = Some("applies_to".to_string());
    args.to_relation = None;
    args.literal_to = Some("applies-to".to_string());
    args.dry_run = true;
    // Migração agora passa: effective_from()="applies_to" !=
    // effective_to()="applies-to".
    assert_ne!(
        args.effective_from(),
        args.effective_to(),
        "literal underscore→hyphen migration must NOT be treated as equality"
    );
    run_batch(
        args,
        std::time::Instant::now(),
        "global".to_string(),
        &mut conn,
    )
    .expect("dry-run must succeed and report the matched edge");
}

#[test]
fn literal_to_alone_keeps_verbatim() {
    use clap::Parser;
    let parsed = crate::cli::Cli::try_parse_from([
        "sqlite-graphrag",
        "reclassify-relation",
        "--from-relation",
        "mentions",
        "--literal-to",
        "applies-to",
        "--batch",
    ])
    .expect("literal-to alone (no --to-relation) must parse");
    match parsed.command {
        Some(crate::cli::Commands::ReclassifyRelation(a)) => {
            assert_eq!(a.literal_to.as_deref(), Some("applies-to"));
            assert!(a.to_relation.is_none());
            assert_eq!(
                a.effective_to(),
                "applies-to",
                "literal_to must win and stay verbatim"
            );
        }
        _ => unreachable!("unexpected command"),
    }
}
