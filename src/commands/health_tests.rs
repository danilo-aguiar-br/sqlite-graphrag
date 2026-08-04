use super::embed_stats::{
    chunk_embedding_health, coverage_pct, entity_embedding_health, memory_embedding_health,
};
use super::tables::{first_existing_table, MEMORY_EMBEDDING_TABLES};
use super::{HealthCheck, HealthCounts, HealthResponse};
use rusqlite::Connection;

fn open_health_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    // GAP-SG-119: use DEFAULT_EMBEDDING_DIM (not legacy 384) for new fixtures.
    let dim = crate::constants::DEFAULT_EMBEDDING_DIM;
    conn.execute_batch(&format!(
        "CREATE TABLE memories (
            id INTEGER PRIMARY KEY,
            deleted_at INTEGER
        );
        CREATE TABLE memory_embeddings (
            memory_id INTEGER PRIMARY KEY,
            namespace TEXT NOT NULL,
            embedding BLOB NOT NULL,
            source TEXT NOT NULL,
            model TEXT NOT NULL,
            dim INTEGER NOT NULL DEFAULT {dim},
            created_at TEXT NOT NULL DEFAULT '0'
        );
        CREATE TABLE vec_memories (
            memory_id INTEGER PRIMARY KEY,
            embedding BLOB NOT NULL,
            created_at INTEGER NOT NULL DEFAULT 0
        );",
    ))
    .unwrap();
    conn
}

/// Builds a graph holding `hubs` entities whose degree clears the super-hub
/// threshold, so the count and the sample can be told apart.
fn open_graph_test_db(hubs: usize) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE entities (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
         CREATE TABLE relationships (
             id INTEGER PRIMARY KEY,
             source_id INTEGER NOT NULL,
             target_id INTEGER NOT NULL
         );",
    )
    .unwrap();

    let degree = crate::constants::HEALTH_SUPER_HUB_DEGREE_THRESHOLD + 1;
    let mut rel_id = 0i64;
    for h in 0..hubs {
        let hub_id = h as i64 + 1;
        conn.execute(
            "INSERT INTO entities (id, name) VALUES (?1, ?2)",
            rusqlite::params![hub_id, format!("hub-{h}")],
        )
        .unwrap();
        for _ in 0..degree {
            rel_id += 1;
            conn.execute(
                "INSERT INTO relationships (id, source_id, target_id) VALUES (?1, ?2, ?2)",
                rusqlite::params![rel_id, hub_id],
            )
            .unwrap();
        }
    }
    conn
}

/// The count is a property of the graph; the warning is a sample of it. Reading
/// the count off the sample capped it at the sample size, so a graph with
/// hundreds of hubs reported exactly the sample limit and never moved.
#[test]
fn super_hub_count_spans_the_graph_rather_than_the_sample() {
    let sample = crate::constants::HEALTH_SUPER_HUB_SAMPLE_LIMIT;
    let hubs = sample + 3;
    let conn = open_graph_test_db(hubs);

    let (count, warning) = super::super_hub_stats(&conn).unwrap();

    assert_eq!(
        usize::try_from(count).unwrap(),
        hubs,
        "count must span the graph, not stop at the {sample}-entity sample"
    );
    let warning = warning.expect("hubs are present, so a warning is due");
    assert_eq!(
        warning.matches("degree").count(),
        sample,
        "the warning names only the sample: {warning}"
    );
}

#[test]
fn super_hub_stats_stays_silent_on_a_graph_without_hubs() {
    let conn = open_graph_test_db(0);
    let (count, warning) = super::super_hub_stats(&conn).unwrap();
    assert_eq!(count, 0);
    assert!(warning.is_none());
}

#[test]
fn memory_embedding_health_prefers_memory_embeddings_and_counts_soft_deleted_as_orphaned() {
    let conn = open_health_test_db();
    conn.execute("INSERT INTO memories (id, deleted_at) VALUES (1, NULL)", [])
        .unwrap();
    conn.execute("INSERT INTO memories (id, deleted_at) VALUES (2, NULL)", [])
        .unwrap();
    conn.execute("INSERT INTO memories (id, deleted_at) VALUES (3, 123)", [])
        .unwrap();
    conn.execute(
        "INSERT INTO memory_embeddings(memory_id, namespace, embedding, source, model, dim, created_at)
         VALUES (1, 'global', X'00', 'llm', 'm', 384, '1')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memory_embeddings(memory_id, namespace, embedding, source, model, dim, created_at)
         VALUES (3, 'global', X'00', 'llm', 'm', 384, '2')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memory_embeddings(memory_id, namespace, embedding, source, model, dim, created_at)
         VALUES (99, 'global', X'00', 'llm', 'm', 384, '3')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO vec_memories(memory_id, embedding, created_at) VALUES (777, X'00', 0)",
        [],
    )
    .unwrap();

    let (ok, total, missing, orphaned) = memory_embedding_health(&conn);
    assert!(ok);
    assert_eq!(total, 3);
    assert_eq!(missing, 1);
    assert_eq!(orphaned, 2);
}

// v1.1.1 (P6a): a DB with entities/chunks lacking vector rows must report
// them as missing — coverage, not just table existence.
#[test]
fn entity_and_chunk_embedding_health_count_missing_rows() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE entities (id INTEGER PRIMARY KEY, name TEXT);
        CREATE TABLE entity_embeddings (
            entity_id INTEGER PRIMARY KEY,
            namespace TEXT NOT NULL,
            embedding BLOB NOT NULL
        );
        CREATE TABLE memory_chunks (id INTEGER PRIMARY KEY, memory_id INTEGER);
        CREATE TABLE chunk_embeddings (
            chunk_id INTEGER PRIMARY KEY,
            memory_id INTEGER NOT NULL,
            embedding BLOB NOT NULL
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO entities (id, name) VALUES (1, 'a'), (2, 'b'), (3, 'c')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO entity_embeddings (entity_id, namespace, embedding)
         VALUES (1, 'global', X'00')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memory_chunks (id, memory_id) VALUES (10, 1), (11, 1)",
        [],
    )
    .unwrap();

    let (e_ok, e_missing) = entity_embedding_health(&conn);
    assert!(e_ok, "entity_embeddings exists");
    assert_eq!(e_missing, 2, "2 of 3 entities lack a vector row");
    let (c_ok, c_missing) = chunk_embedding_health(&conn);
    assert!(c_ok, "chunk_embeddings exists");
    assert_eq!(c_missing, 2, "both chunks lack a vector row");
}

#[test]
fn entity_embedding_health_absent_table_reports_not_ok() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE entities (id INTEGER PRIMARY KEY, name TEXT);")
        .unwrap();
    let (ok, missing) = entity_embedding_health(&conn);
    assert!(!ok, "no entity vector table exists");
    assert_eq!(missing, 0);
}

#[test]
fn coverage_pct_boundaries() {
    assert!((coverage_pct(true, 0, 0) - 100.0).abs() < 1e-9);
    assert!((coverage_pct(false, 5, 0) - 0.0).abs() < 1e-9);
    assert!((coverage_pct(true, 4, 1) - 75.0).abs() < 1e-9);
    assert!((coverage_pct(true, 4, 0) - 100.0).abs() < 1e-9);
}

#[test]
fn first_existing_table_falls_back_to_legacy_vec_name() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE vec_memories (
            memory_id INTEGER PRIMARY KEY,
            embedding BLOB NOT NULL,
            created_at INTEGER NOT NULL DEFAULT 0
        );",
    )
    .unwrap();

    let resolved = first_existing_table(&conn, MEMORY_EMBEDDING_TABLES);
    assert_eq!(resolved, Some("vec_memories"));
}

#[test]
fn health_check_serializes_all_new_fields() {
    let response = HealthResponse {
        status: "ok".to_string(),
        namespace: None,
        integrity: "ok".to_string(),
        integrity_ok: true,
        schema_ok: true,
        vec_memories_ok: true,
        vec_memories_missing: 0,
        vec_memories_orphaned: 0,
        vec_entities_ok: true,
        vec_entities_missing: 0,
        vec_chunks_ok: true,
        vec_chunks_missing: 0,
        vec_memories_coverage_pct: 100.0,
        vec_entities_coverage_pct: 100.0,
        vec_chunks_coverage_pct: 100.0,
        fts_ok: true,
        fts_query_ok: true,
        model_ok: false,
        counts: HealthCounts {
            memories: 5,
            memories_total: 5,
            entities: 3,
            relationships: 2,
            vec_memories: 5,
        },
        db_path: "/tmp/test.sqlite".to_string(),
        db_size_bytes: 4096,
        schema_version: 6,
        sqlite_version: "3.46.0".to_string(),
        elapsed_ms: 0,
        missing_entities: vec![],
        wal_size_mb: 0.0,
        journal_mode: "wal".to_string(),
        mentions_ratio: None,
        mentions_warning: None,
        top_relation: None,
        top_relation_ratio: None,
        applies_to_ratio: None,
        relation_concentration_warning: None,
        non_normalized_count: None,
        normalization_warning: None,
        super_hub_count: None,
        super_hub_warning: None,
        top_hub_entity: None,
        top_hub_degree: None,
        hub_warning: None,
        llm_slots_total: None,
        llm_slots_occupied: None,
        llm_slots_stale: None,
        checks: vec![
            HealthCheck {
                name: "integrity".to_string(),
                ok: true,
                detail: None,
            },
            HealthCheck {
                name: "model_onnx".to_string(),
                ok: false,
                detail: Some("model missing".to_string()),
            },
        ],
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["integrity_ok"], true);
    assert_eq!(json["schema_ok"], true);
    assert_eq!(json["vec_memories_ok"], true);
    assert_eq!(json["vec_entities_ok"], true);
    assert_eq!(json["vec_chunks_ok"], true);
    // v1.1.1 (P6a): coverage fields always serialize (additive contract).
    assert_eq!(json["vec_entities_missing"], 0);
    assert_eq!(json["vec_chunks_missing"], 0);
    assert!((json["vec_memories_coverage_pct"].as_f64().unwrap() - 100.0).abs() < 1e-9);
    assert!((json["vec_entities_coverage_pct"].as_f64().unwrap() - 100.0).abs() < 1e-9);
    assert!((json["vec_chunks_coverage_pct"].as_f64().unwrap() - 100.0).abs() < 1e-9);
    assert_eq!(json["fts_ok"], true);
    assert_eq!(json["model_ok"], false);
    assert_eq!(json["db_size_bytes"], 4096u64);
    assert!(json["checks"].is_array());
    assert_eq!(json["checks"].as_array().unwrap().len(), 2);

    // Verifies that detail is absent when ok=true (skip_serializing_if)
    let integrity_check = &json["checks"][0];
    assert_eq!(integrity_check["name"], "integrity");
    assert_eq!(integrity_check["ok"], true);
    assert!(integrity_check.get("detail").is_none());

    // Verifies that detail is present when ok=false
    let model_check = &json["checks"][1];
    assert_eq!(model_check["name"], "model_onnx");
    assert_eq!(model_check["ok"], false);
    assert_eq!(model_check["detail"], "model missing");
}

#[test]
fn health_check_without_detail_omits_field() {
    let check = HealthCheck {
        name: "vec_memories".to_string(),
        ok: true,
        detail: None,
    };
    let json = serde_json::to_value(&check).unwrap();
    assert!(
        json.get("detail").is_none(),
        "detail field must be omitted when None"
    );
}

#[test]
fn health_check_with_detail_serializes_field() {
    let check = HealthCheck {
        name: "fts_memories".to_string(),
        ok: false,
        detail: Some("fts_memories table missing from sqlite_master".to_string()),
    };
    let json = serde_json::to_value(&check).unwrap();
    assert_eq!(
        json["detail"],
        "fts_memories table missing from sqlite_master"
    );
}

#[test]
fn health_response_fts_query_ok_and_sqlite_version_serialize() {
    // Verifies that fts_query_ok and sqlite_version appear in the serialized JSON
    // with the expected keys and values.
    let response = HealthResponse {
        status: "ok".to_string(),
        namespace: Some("test-ns".to_string()),
        integrity: "ok".to_string(),
        integrity_ok: true,
        schema_ok: true,
        vec_memories_ok: true,
        vec_memories_missing: 0,
        vec_memories_orphaned: 0,
        vec_entities_ok: true,
        vec_entities_missing: 0,
        vec_chunks_ok: true,
        vec_chunks_missing: 0,
        vec_memories_coverage_pct: 100.0,
        vec_entities_coverage_pct: 100.0,
        vec_chunks_coverage_pct: 100.0,
        fts_ok: true,
        fts_query_ok: true,
        model_ok: true,
        counts: HealthCounts {
            memories: 0,
            memories_total: 0,
            entities: 0,
            relationships: 0,
            vec_memories: 0,
        },
        db_path: "/tmp/test.sqlite".to_string(),
        db_size_bytes: 0,
        schema_version: 1,
        sqlite_version: "3.45.1".to_string(),
        elapsed_ms: 0,
        missing_entities: vec![],
        wal_size_mb: 0.0,
        journal_mode: "wal".to_string(),
        mentions_ratio: None,
        mentions_warning: None,
        top_relation: None,
        top_relation_ratio: None,
        applies_to_ratio: None,
        relation_concentration_warning: None,
        non_normalized_count: None,
        normalization_warning: None,
        super_hub_count: None,
        super_hub_warning: None,
        top_hub_entity: None,
        top_hub_degree: None,
        hub_warning: None,
        llm_slots_total: None,
        llm_slots_occupied: None,
        llm_slots_stale: None,
        checks: vec![],
    };

    let json = serde_json::to_value(&response).unwrap();

    // fts_query_ok must appear at the top level
    assert_eq!(
        json["fts_query_ok"], true,
        "fts_query_ok must be present and true in serialized JSON"
    );

    // sqlite_version must appear at the top level with the exact string
    assert_eq!(
        json["sqlite_version"], "3.45.1",
        "sqlite_version must be present and match the provided string"
    );

    // Verify fts_query_ok=false path includes the expected detail message
    let check_fail = HealthCheck {
        name: "fts_query".to_string(),
        ok: false,
        detail: Some("FTS5 MATCH query failed — run 'sqlite-graphrag fts rebuild'".to_string()),
    };
    let check_json = serde_json::to_value(&check_fail).unwrap();
    assert_eq!(check_json["name"], "fts_query");
    assert_eq!(check_json["ok"], false);
    assert_eq!(
        check_json["detail"],
        "FTS5 MATCH query failed — run 'sqlite-graphrag fts rebuild'"
    );
}

fn make_full_response(
    top_relation: Option<String>,
    top_relation_ratio: Option<f64>,
    applies_to_ratio: Option<f64>,
    relation_concentration_warning: Option<String>,
) -> HealthResponse {
    HealthResponse {
        status: "ok".to_string(),
        namespace: None,
        integrity: "ok".to_string(),
        integrity_ok: true,
        schema_ok: true,
        vec_memories_ok: true,
        vec_memories_missing: 0,
        vec_memories_orphaned: 0,
        vec_entities_ok: true,
        vec_entities_missing: 0,
        vec_chunks_ok: true,
        vec_chunks_missing: 0,
        vec_memories_coverage_pct: 100.0,
        vec_entities_coverage_pct: 100.0,
        vec_chunks_coverage_pct: 100.0,
        fts_ok: true,
        fts_query_ok: true,
        model_ok: true,
        counts: HealthCounts {
            memories: 10,
            memories_total: 10,
            entities: 5,
            relationships: 20,
            vec_memories: 10,
        },
        db_path: "/tmp/test.sqlite".to_string(),
        db_size_bytes: 8192,
        schema_version: 3,
        sqlite_version: "3.46.0".to_string(),
        elapsed_ms: 1,
        missing_entities: vec![],
        wal_size_mb: 0.0,
        journal_mode: "wal".to_string(),
        mentions_ratio: None,
        mentions_warning: None,
        top_relation,
        top_relation_ratio,
        applies_to_ratio,
        relation_concentration_warning,
        non_normalized_count: None,
        normalization_warning: None,
        super_hub_count: None,
        super_hub_warning: None,
        top_hub_entity: None,
        top_hub_degree: None,
        hub_warning: None,
        llm_slots_total: None,
        llm_slots_occupied: None,
        llm_slots_stale: None,
        checks: vec![],
    }
}

#[test]
fn health_concentration_fields_omitted_when_no_relationships() {
    // Represents a DB with zero relationships.
    let resp = make_full_response(None, None, None, None);
    let json = serde_json::to_value(&resp).unwrap();
    assert!(
        json.get("top_relation").is_none(),
        "top_relation must be omitted when None"
    );
    assert!(
        json.get("top_relation_ratio").is_none(),
        "top_relation_ratio must be omitted when None"
    );
    assert!(
        json.get("applies_to_ratio").is_none(),
        "applies_to_ratio must be omitted when None"
    );
    assert!(
        json.get("relation_concentration_warning").is_none(),
        "relation_concentration_warning must be omitted when None"
    );
}

#[test]
fn health_concentration_fields_present_with_data() {
    let resp = make_full_response(
        Some("mentions".to_string()),
        Some(0.60),
        Some(0.10),
        Some("relation 'mentions' dominates graph at 60.0%".to_string()),
    );
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["top_relation"], "mentions");
    assert!((json["top_relation_ratio"].as_f64().unwrap() - 0.60).abs() < 1e-9);
    assert!((json["applies_to_ratio"].as_f64().unwrap() - 0.10).abs() < 1e-9);
    assert!(json["relation_concentration_warning"]
        .as_str()
        .unwrap()
        .contains("60.0%"));
}

#[test]
fn health_concentration_warning_absent_when_ratio_below_threshold() {
    // top_relation_ratio of 0.39 is below the 0.40 threshold — no warning.
    let resp = make_full_response(Some("uses".to_string()), Some(0.39), None, None);
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["top_relation"], "uses");
    assert!(
        json.get("relation_concentration_warning").is_none(),
        "warning must be absent when ratio <= 0.40"
    );
}

#[test]
fn health_concentration_warning_present_at_threshold() {
    // Exactly at 0.41 (above 0.40) — warning must appear.
    let resp = make_full_response(
        Some("depends_on".to_string()),
        Some(0.41),
        None,
        Some("relation 'depends_on' dominates graph at 41.0%".to_string()),
    );
    let json = serde_json::to_value(&resp).unwrap();
    assert!(
        json["relation_concentration_warning"].is_string(),
        "warning must be present when top_relation_ratio > 0.40"
    );
}

#[test]
fn health_applies_to_ratio_omitted_when_none() {
    // applies_to_ratio is None when there are no applies_to edges.
    let resp = make_full_response(Some("related".to_string()), Some(0.30), None, None);
    let json = serde_json::to_value(&resp).unwrap();
    assert!(
        json.get("applies_to_ratio").is_none(),
        "applies_to_ratio must be omitted when None"
    );
}
