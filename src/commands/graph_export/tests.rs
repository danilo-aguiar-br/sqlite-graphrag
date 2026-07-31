use super::args::*;
use super::formats::*;
use super::handlers::{build_order_by, recompute_degrees, RecomputeDegreeSummary};
use crate::cli::{Cli, Commands};
use clap::Parser;

fn make_node(kind: &str) -> NodeOut {
    NodeOut {
        id: 1,
        name: "test-entity".to_string(),
        namespace: "default".to_string(),
        kind: kind.to_string(),
        r#type: kind.to_string(),
    }
}

#[test]
fn node_out_type_duplicates_kind() {
    let node = make_node("agent");
    let json = serde_json::to_value(&node).expect("serialization must work");
    assert_eq!(json["kind"], json["type"]);
    assert_eq!(json["kind"], "agent");
    assert_eq!(json["type"], "agent");
}

#[test]
fn node_out_serializes_all_fields() {
    let node = make_node("document");
    let json = serde_json::to_value(&node).expect("serialization must work");
    assert!(json.get("id").is_some());
    assert!(json.get("name").is_some());
    assert!(json.get("namespace").is_some());
    assert!(json.get("kind").is_some());
    assert!(json.get("type").is_some());
}

#[test]
fn graph_snapshot_serializes_nodes_with_type() {
    let node = make_node("concept");
    let entities = vec![make_node("concept")];
    let snapshot = GraphSnapshot {
        nodes: vec![node],
        entities,
        edges: vec![],
        elapsed_ms: 0,
    };
    let json_str = render_json(&snapshot).expect("rendering must work");
    let json: serde_json::Value = serde_json::from_str(&json_str).expect("valid json");
    let first_node = &json["nodes"][0];
    assert_eq!(first_node["kind"], first_node["type"]);
    assert_eq!(first_node["type"], "concept");
}

#[test]
fn graph_traverse_response_serializes_correctly() {
    let resp = GraphTraverseResponse {
        from: "entity-a".to_string(),
        namespace: "global".to_string(),
        depth: 2,
        hops: vec![TraverseHop {
            entity: "entity-b".to_string(),
            relation: "uses".to_string(),
            direction: "outbound".to_string(),
            weight: 1.0,
            depth: 1,
        }],
        elapsed_ms: 5,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["from"], "entity-a");
    assert_eq!(json["depth"], 2);
    assert!(json["hops"].is_array());
    assert_eq!(json["hops"][0]["direction"], "outbound");
}

#[test]
fn graph_stats_response_serializes_correctly() {
    let resp = GraphStatsResponse {
        namespace: Some("global".to_string()),
        node_count: 10,
        edge_count: 15,
        avg_degree: 3.0,
        max_degree: 7,
        elapsed_ms: 2,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["node_count"], 10);
    assert_eq!(json["edge_count"], 15);
    assert_eq!(json["avg_degree"], 3.0);
    assert_eq!(json["max_degree"], 7);
}

fn compute_avg_degree(node_count: i64, edge_count: i64) -> f64 {
    if node_count > 0 {
        2.0 * (edge_count as f64) / (node_count as f64)
    } else {
        0.0
    }
}

#[test]
fn avg_degree_is_zero_when_no_nodes() {
    assert_eq!(compute_avg_degree(0, 0), 0.0);
}

#[test]
fn avg_degree_is_zero_when_nodes_but_no_edges() {
    // Reproduces L1 bug: previously returned 1.0 instead of 0.0.
    assert_eq!(compute_avg_degree(2, 0), 0.0);
}

#[test]
fn avg_degree_is_two_when_triangle() {
    // 3 nodes, 3 edges: 2 * 3 / 3 = 2.0
    assert_eq!(compute_avg_degree(3, 3), 2.0);
}

#[test]
fn graph_entities_response_serializes_required_fields() {
    let resp = GraphEntitiesResponse {
        entities: vec![EntityItem {
            id: 1,
            name: "claude-code".to_string(),
            entity_type: "agent".to_string(),
            namespace: "global".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            degree: 0,
            description: None,
        }],
        total_count: 1,
        limit: 50,
        offset: 0,
        namespace: Some("global".to_string()),
        elapsed_ms: 3,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert!(json["entities"].is_array());
    assert_eq!(json["entities"][0]["name"], "claude-code");
    assert_eq!(json["entities"][0]["entity_type"], "agent");
    assert_eq!(json["total_count"], 1);
    assert_eq!(json["limit"], 50);
    assert_eq!(json["offset"], 0);
    assert_eq!(json["namespace"], "global");
}

#[test]
fn entity_item_serializes_all_fields() {
    let item = EntityItem {
        id: 42,
        name: "test-entity".to_string(),
        entity_type: "concept".to_string(),
        namespace: "project-a".to_string(),
        created_at: "2026-04-19T12:00:00Z".to_string(),
        degree: 3,
        description: Some("test description".to_string()),
    };
    let json = serde_json::to_value(&item).unwrap();
    assert_eq!(json["id"], 42);
    assert_eq!(json["name"], "test-entity");
    assert_eq!(json["entity_type"], "concept");
    assert_eq!(json["namespace"], "project-a");
    assert_eq!(json["created_at"], "2026-04-19T12:00:00Z");
}

#[test]
fn entity_item_entity_type_is_never_null() {
    // P2-C: entity_type must never be null, even when DB column is empty.
    let item = EntityItem {
        id: 1,
        name: "sem-tipo".to_string(),
        entity_type: String::new(),
        namespace: "ns".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        degree: 0,
        description: None,
    };
    let json = serde_json::to_value(&item).unwrap();
    assert!(
        !json["entity_type"].is_null(),
        "entity_type must not be null"
    );
    assert!(json["entity_type"].is_string());
}

#[test]
fn graph_traverse_cli_rejects_format_dot() {
    let parsed = Cli::try_parse_from([
        "sqlite-graphrag",
        "graph",
        "traverse",
        "--from",
        "AuthDecision",
        "--format",
        "dot",
    ]);
    assert!(parsed.is_err(), "graph traverse must reject format=dot");
}

#[test]
fn graph_stats_cli_accepts_format_text() {
    let parsed = Cli::try_parse_from(["sqlite-graphrag", "graph", "stats", "--format", "text"])
        .expect("graph stats --format text must be accepted");

    match parsed.command {
        Some(Commands::Graph(args)) => match args.subcommand {
            Some(GraphSubcommand::Stats(stats)) => {
                assert_eq!(stats.format, GraphStatsFormat::Text);
            }
            _ => unreachable!("unexpected subcommand"),
        },
        _ => unreachable!("unexpected command"),
    }
}

#[test]
fn graph_stats_cli_rejects_format_mermaid() {
    let parsed = Cli::try_parse_from(["sqlite-graphrag", "graph", "stats", "--format", "mermaid"]);
    assert!(parsed.is_err(), "graph stats must reject format=mermaid");
}

#[test]
fn graph_entities_response_has_no_items_key() {
    let resp = GraphEntitiesResponse {
        entities: vec![],
        total_count: 0,
        limit: 50,
        offset: 0,
        namespace: None,
        elapsed_ms: 0,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert!(
        json.get("items").is_none(),
        "legacy 'items' key must not appear"
    );
    assert!(
        json.get("entities").is_some(),
        "'entities' key must be present"
    );
}

#[test]
fn build_order_by_defaults_to_name_asc() {
    let clause = build_order_by(None, SortOrder::Asc);
    assert_eq!(clause, "ORDER BY e.name ASC");
}

#[test]
fn build_order_by_name_desc() {
    let clause = build_order_by(Some(EntitySortField::Name), SortOrder::Desc);
    assert_eq!(clause, "ORDER BY e.name DESC");
}

#[test]
fn build_order_by_degree_desc() {
    let clause = build_order_by(Some(EntitySortField::Degree), SortOrder::Desc);
    assert_eq!(clause, "ORDER BY degree DESC");
}

#[test]
fn build_order_by_degree_asc() {
    let clause = build_order_by(Some(EntitySortField::Degree), SortOrder::Asc);
    assert_eq!(clause, "ORDER BY degree ASC");
}

#[test]
fn build_order_by_created_at_asc() {
    let clause = build_order_by(Some(EntitySortField::CreatedAt), SortOrder::Asc);
    assert_eq!(clause, "ORDER BY e.created_at ASC");
}

#[test]
fn build_order_by_created_at_desc() {
    let clause = build_order_by(Some(EntitySortField::CreatedAt), SortOrder::Desc);
    assert_eq!(clause, "ORDER BY e.created_at DESC");
}

#[test]
fn graph_entities_cli_accepts_sort_by_degree_desc() {
    let parsed = Cli::try_parse_from([
        "sqlite-graphrag",
        "graph",
        "entities",
        "--sort-by",
        "degree",
        "--order",
        "desc",
    ])
    .expect("graph entities --sort-by degree --order desc must parse");
    match parsed.command {
        Some(Commands::Graph(args)) => match args.subcommand {
            Some(GraphSubcommand::Entities(e)) => {
                assert!(matches!(e.sort_by, Some(EntitySortField::Degree)));
                assert!(matches!(e.order, SortOrder::Desc));
            }
            _ => unreachable!("unexpected subcommand"),
        },
        _ => unreachable!("unexpected command"),
    }
}

#[test]
fn graph_entities_cli_accepts_sort_by_created_at_asc() {
    let parsed = Cli::try_parse_from([
        "sqlite-graphrag",
        "graph",
        "entities",
        "--sort-by",
        "created-at",
    ])
    .expect("graph entities --sort-by created-at must parse");
    match parsed.command {
        Some(Commands::Graph(args)) => match args.subcommand {
            Some(GraphSubcommand::Entities(e)) => {
                assert!(matches!(e.sort_by, Some(EntitySortField::CreatedAt)));
                assert!(matches!(e.order, SortOrder::Asc));
            }
            _ => unreachable!("unexpected subcommand"),
        },
        _ => unreachable!("unexpected command"),
    }
}

#[test]
fn graph_entities_cli_defaults_to_no_sort_by() {
    let parsed = Cli::try_parse_from(["sqlite-graphrag", "graph", "entities"])
        .expect("graph entities must parse without sort flags");
    match parsed.command {
        Some(Commands::Graph(args)) => match args.subcommand {
            Some(GraphSubcommand::Entities(e)) => {
                assert!(e.sort_by.is_none(), "sort_by must default to None");
                assert!(
                    matches!(e.order, SortOrder::Asc),
                    "order must default to Asc"
                );
            }
            _ => unreachable!("unexpected subcommand"),
        },
        _ => unreachable!("unexpected command"),
    }
}

// -----------------------------------------------------------------------
// v1.1.1 (P3): graph recompute-degree — reconciliação do cache `degree`
// -----------------------------------------------------------------------

fn setup_migrated_db() -> (tempfile::TempDir, rusqlite::Connection) {
    crate::storage::connection::register_vec_extension();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("test.db");
    let mut conn = rusqlite::Connection::open(&db_path).expect("open");
    crate::migrations::runner().run(&mut conn).expect("migrate");
    (tmp, conn)
}

fn insert_entity_with_degree(
    conn: &rusqlite::Connection,
    ns: &str,
    name: &str,
    degree: i64,
) -> i64 {
    conn.execute(
        "INSERT INTO entities (namespace, name, type, degree) VALUES (?1, ?2, 'concept', ?3)",
        rusqlite::params![ns, name, degree],
    )
    .expect("insert entity");
    conn.last_insert_rowid()
}

fn insert_edge(conn: &rusqlite::Connection, ns: &str, source: i64, target: i64) {
    conn.execute(
        "INSERT INTO relationships (namespace, source_id, target_id, relation, weight) \
         VALUES (?1, ?2, ?3, 'uses', 0.5)",
        rusqlite::params![ns, source, target],
    )
    .expect("insert edge");
}

#[test]
fn recompute_degrees_reconciles_updated_zeroed_and_unchanged() {
    let (_tmp, mut conn) = setup_migrated_db();
    // a—b conectadas mas com degree armazenado errado (0 e 5); c órfã com
    // degree fantasma 7; d já correta com degree 0.
    let a = insert_entity_with_degree(&conn, "global", "ent-a", 0);
    let b = insert_entity_with_degree(&conn, "global", "ent-b", 5);
    let c = insert_entity_with_degree(&conn, "global", "ent-c", 7);
    let d = insert_entity_with_degree(&conn, "global", "ent-d", 0);
    insert_edge(&conn, "global", a, b);

    let summary = recompute_degrees(&mut conn, Some("global"), false).expect("recompute");
    assert_eq!(
        summary,
        RecomputeDegreeSummary {
            total: 4,
            updated: 2,
            zeroed: 1,
            unchanged: 1,
        }
    );

    let degree_of = |id: i64| -> i64 {
        conn.query_row(
            "SELECT degree FROM entities WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(degree_of(a), 1);
    assert_eq!(degree_of(b), 1);
    assert_eq!(degree_of(c), 0, "entidade sem arestas deve ser zerada");
    assert_eq!(degree_of(d), 0);

    // Segunda passada converge: tudo unchanged.
    let second = recompute_degrees(&mut conn, Some("global"), false).expect("recompute 2");
    assert_eq!(second.updated + second.zeroed, 0);
    assert_eq!(second.unchanged, 4);
}

#[test]
fn recompute_degrees_dry_run_reports_without_writing() {
    let (_tmp, mut conn) = setup_migrated_db();
    let a = insert_entity_with_degree(&conn, "global", "ent-a", 9);

    let summary = recompute_degrees(&mut conn, Some("global"), true).expect("dry-run");
    assert_eq!(summary.zeroed, 1, "divergência reportada no dry-run");

    let stored: i64 = conn
        .query_row(
            "SELECT degree FROM entities WHERE id = ?1",
            rusqlite::params![a],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, 9, "dry-run não pode escrever");
}

#[test]
fn recompute_degrees_scopes_by_namespace_and_none_covers_all() {
    let (_tmp, mut conn) = setup_migrated_db();
    insert_entity_with_degree(&conn, "ns1", "ent-ns1", 3);
    insert_entity_with_degree(&conn, "ns2", "ent-ns2", 4);

    let only_ns1 = recompute_degrees(&mut conn, Some("ns1"), false).expect("ns1");
    assert_eq!(only_ns1.total, 1);

    // ns2 permanece divergente até uma passada sem namespace (todas).
    let all = recompute_degrees(&mut conn, None, false).expect("all");
    assert_eq!(all.total, 2);
    assert_eq!(all.zeroed, 1, "só ns2 ainda divergia");
    assert_eq!(all.unchanged, 1);
}

#[test]
fn graph_recompute_degree_cli_parses_flags() {
    let parsed = Cli::try_parse_from([
        "sqlite-graphrag",
        "graph",
        "recompute-degree",
        "--dry-run",
        "--namespace",
        "project-x",
    ])
    .expect("recompute-degree must parse");
    match parsed.command {
        Some(Commands::Graph(args)) => match args.subcommand {
            Some(GraphSubcommand::RecomputeDegree(a)) => {
                assert!(a.dry_run);
                assert_eq!(a.namespace.as_deref(), Some("project-x"));
            }
            _ => unreachable!("unexpected subcommand"),
        },
        _ => unreachable!("unexpected command"),
    }
}
