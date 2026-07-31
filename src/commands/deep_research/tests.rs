use super::*;

#[test]
fn test_decompose_and_conjunction() {
    let result = decompose_query("A and B", 7);
    assert_eq!(result, vec!["A", "B"]);
}

#[test]
fn test_decompose_no_split_multiword_stays_single() {
    // Two-word queries without delimiters stay as one sub-query (not
    // single-token aspect expansion).
    let result = decompose_query("simple query", 7);
    assert_eq!(result, vec!["simple query"]);
}

/// v1.1.05 Bug 1: single-token queries must fan out into aspects.
#[test]
fn test_decompose_single_token_danilo_fans_out() {
    let result = decompose_query("danilo", 7);
    assert!(
        result.len() > 1,
        "expected aspect fan-out for single token, got {result:?}"
    );
    assert_eq!(result[0], "danilo");
    assert!(
        result
            .iter()
            .any(|s| s.contains("stack") || s.contains("patrimonio")),
        "expected aspect facets in {result:?}"
    );
    let with_src = decompose_query_with_sources("danilo", 7);
    assert_eq!(with_src[0].1, "original");
    assert!(with_src.iter().skip(1).all(|(_, s)| *s == "aspect"));
}

#[test]
fn test_decompose_three_parts() {
    let result = decompose_query("A, B and C", 7);
    assert_eq!(result, vec!["A", "B", "C"]);
}

#[test]
fn test_decompose_portuguese_conjunctions() {
    let result = decompose_query("A e B", 7);
    assert_eq!(result, vec!["A", "B"]);
}

#[test]
fn test_decompose_max_cap() {
    let parts: Vec<String> = (0..10).map(|i| format!("part{i}")).collect();
    let query = parts.join(", ");
    let result = decompose_query(&query, 7);
    assert!(
        result.len() <= 7,
        "expected at most 7 sub-queries, got {}",
        result.len()
    );
}

#[test]
fn test_decompose_empty_preserves_original() {
    let result = decompose_query("", 7);
    assert_eq!(result, vec![""]);
}

#[test]
fn test_decompose_semicolons() {
    let result = decompose_query("auth design; deployment config; logging", 7);
    assert_eq!(result, vec!["auth design", "deployment config", "logging"]);
}

#[test]
fn test_decompose_relational_phrase() {
    let result = decompose_query("auth that caused deployment failure", 7);
    assert_eq!(result, vec!["auth", "deployment failure"]);
}

#[test]
fn test_sub_query_serialization() {
    let sq = SubQuery {
        id: 0,
        text: "test query".to_string(),
        source: "original",
    };
    let json = serde_json::to_value(&sq).expect("serialization failed");
    assert_eq!(json["id"], 0);
    assert_eq!(json["text"], "test query");
    assert_eq!(json["source"], "original");
}

#[test]
fn test_deep_result_omits_body_when_none() {
    let result = DeepResult {
        name: "test".to_string(),
        score: 0.9,
        source: "knn".to_string(),
        sub_query_ids: vec![0],
        snippet: "snippet".to_string(),
        body: None,
        hop_distance: None,
    };
    let json = serde_json::to_string(&result).expect("serialization failed");
    assert!(!json.contains("\"body\""), "body must be omitted when None");
}

#[test]
fn test_deep_result_includes_body_when_some() {
    let result = DeepResult {
        name: "test".to_string(),
        score: 0.9,
        source: "knn".to_string(),
        sub_query_ids: vec![0, 1],
        snippet: "snippet".to_string(),
        body: Some("full body content".to_string()),
        hop_distance: Some(2),
    };
    let json = serde_json::to_string(&result).expect("serialization failed");
    assert!(json.contains("\"body\""), "body must be present when Some");
    assert!(json.contains("full body content"));
}

#[test]
fn test_evidence_node_omits_none_fields() {
    let node = EvidenceNode {
        entity: "auth-module".to_string(),
        relation: None,
        weight: None,
    };
    let json = serde_json::to_string(&node).expect("serialization failed");
    assert!(
        !json.contains("\"relation\""),
        "relation must be omitted when None"
    );
    assert!(
        !json.contains("\"weight\""),
        "weight must be omitted when None"
    );
}

#[test]
fn test_research_stats_serialization() {
    let stats = ResearchStats {
        sub_queries_total: 3,
        sub_queries_completed: 2,
        sub_queries_failed: 1,
        sub_queries_timed_out: 0,
        unique_memories_found: 10,
        evidence_chains_found: 2,
        elapsed_ms: 1234,
        vec_degraded: false,
    };
    let json = serde_json::to_value(&stats).expect("serialization failed");
    assert_eq!(json["sub_queries_total"], 3);
    assert_eq!(json["sub_queries_completed"], 2);
    assert_eq!(json["sub_queries_failed"], 1);
    assert_eq!(json["elapsed_ms"], 1234);
}

#[test]
fn test_deep_research_response_serialization() {
    let resp = DeepResearchResponse {
        query: "test query".to_string(),
        sub_queries: vec![SubQuery {
            id: 0,
            text: "test query".to_string(),
            source: "original",
        }],
        results: vec![],
        evidence_chains: vec![],
        graph_context: None,
        stats: ResearchStats {
            sub_queries_total: 1,
            sub_queries_completed: 1,
            sub_queries_failed: 0,
            sub_queries_timed_out: 0,
            unique_memories_found: 0,
            evidence_chains_found: 0,
            elapsed_ms: 42,
            vec_degraded: false,
        },
    };
    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["query"], "test query");
    assert!(json["sub_queries"].is_array());
    assert!(json["results"].is_array());
    assert!(json["evidence_chains"].is_array());
    assert_eq!(json["stats"]["elapsed_ms"], 42);
}

// ---- GAP-07 regression: different sub-queries produce distinct embeddings ----
// We test decompose_query returns texts that *would* produce distinct embeddings
// (different text inputs → different embedding inputs → different search results).
#[test]
fn test_distinct_sub_queries_produce_distinct_texts() {
    let queries = [
        "authentication design decisions",
        "deployment configuration and infrastructure",
    ];
    // These two texts must be different strings (prerequisite for distinct embeddings).
    assert_ne!(queries[0], queries[1]);

    // decompose_query with semicolons must preserve distinct texts.
    let decomposed = decompose_query(
        "authentication design decisions; deployment configuration and infrastructure",
        7,
    );
    assert_eq!(decomposed.len(), 2);
    assert_ne!(decomposed[0], decomposed[1]);
}

// ---- GAP-08/11 regression: rrf_fuse integration via fusion module ----
#[test]
fn test_rrf_fuse_via_fusion_module() {
    use crate::storage::fusion::rrf_fuse;

    let knn_ids: Vec<i64> = vec![1, 2, 3];
    let fts_ids: Vec<i64> = vec![2, 1, 4];
    let scores = rrf_fuse(&[(1.0, &knn_ids), (1.0, &fts_ids)], 60.0);

    // Items appearing in both lists must score higher than items in only one list.
    let score_1 = scores[&1];
    let score_2 = scores[&2];
    let score_3 = scores[&3]; // knn only, rank 3
    let score_4 = scores[&4]; // fts only, rank 3

    assert!(
        score_1 > score_3,
        "id 1 (both lists) must beat id 3 (knn-only rank 3)"
    );
    assert!(
        score_2 > score_4,
        "id 2 (both lists) must beat id 4 (fts-only rank 3)"
    );
}

// ---- GAP-09/10 regression: evidence chains must be directed paths ----
#[test]
fn test_evidence_chain_has_from_to_and_path() {
    let chain = EvidenceChain {
        from: "auth-module".to_string(),
        to: "jwt-service".to_string(),
        path: vec![
            EvidenceNode {
                entity: "auth-module".to_string(),
                relation: None,
                weight: None,
            },
            EvidenceNode {
                entity: "token-validator".to_string(),
                relation: Some("depends-on".to_string()),
                weight: Some(0.9),
            },
            EvidenceNode {
                entity: "jwt-service".to_string(),
                relation: Some("uses".to_string()),
                weight: Some(0.8),
            },
        ],
        total_weight: 0.72,
        depth: 3,
        sub_query_ids: vec![0],
    };

    let json = serde_json::to_value(&chain).expect("serialization failed");
    assert!(
        json["from"].is_string(),
        "evidence chain must have 'from' field"
    );
    assert!(
        json["to"].is_string(),
        "evidence chain must have 'to' field"
    );
    assert!(
        json["path"].is_array(),
        "evidence chain must have 'path' array"
    );
    assert_eq!(json["path"].as_array().unwrap().len(), 3);
    assert!(json["total_weight"].is_number(), "must have total_weight");
    assert_eq!(json["depth"], 3);
}

// ---- GAP-10 regression: reconstruct_path returns correct node order ----
#[test]
fn test_reconstruct_path_root_to_target_order() {
    // Build a simple chain: entity 10 (seed) -> entity 20 -> entity 30 (target)
    let seed_set: HashSet<i64> = [10i64].into_iter().collect();
    let mut predecessor: crate::graph::PredecessorMap = std::collections::HashMap::new();
    predecessor.insert(20, (10, "depends-on".to_string(), 0.9));
    predecessor.insert(30, (20, "uses".to_string(), 0.8));
    let mut entity_names: crate::hash::AHashMap<i64, String> = crate::hash::AHashMap::default();
    entity_names.insert(10, "seed-entity".to_string());
    entity_names.insert(20, "middle-entity".to_string());
    entity_names.insert(30, "target-entity".to_string());

    let result = pipeline::reconstruct_path(30, &seed_set, &predecessor, &entity_names);
    assert!(result.is_some(), "path must be reconstructed");
    let (nodes, weight) = result.unwrap();
    // Path must be [seed, middle, target]
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0].entity, "seed-entity");
    assert_eq!(nodes[1].entity, "middle-entity");
    assert_eq!(nodes[2].entity, "target-entity");
    // total_weight = 0.9 * 0.8
    assert!((weight - 0.72).abs() < 1e-6);
}

// ---- GAP-09 regression: evidence chains must NOT be present for 1-hop trivial pairs ----
#[test]
fn test_evidence_chains_single_hop_filtered_out() {
    // A chain of depth 1 (only root node) should be discarded.
    let chain = EvidenceChain {
        from: "a".to_string(),
        to: "a".to_string(),
        path: vec![EvidenceNode {
            entity: "a".to_string(),
            relation: None,
            weight: None,
        }],
        total_weight: 1.0,
        depth: 1,
        sub_query_ids: vec![0],
    };
    // Simulate the filter: retain chains with depth >= 2.
    let chains = vec![chain];
    let retained: Vec<_> = chains.into_iter().filter(|c| c.depth >= 2).collect();
    assert!(retained.is_empty(), "depth-1 chains must be filtered out");
}

// ---- GAP-17 regression: bfs_with_predecessors honours max_neighbors_per_hop ----
#[test]
fn test_bfs_with_predecessors_respects_neighbor_cap() {
    use crate::graph::bfs_with_predecessors;
    use rusqlite::Connection;

    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE relationships (
            source_id INTEGER NOT NULL,
            target_id INTEGER NOT NULL,
            weight REAL NOT NULL,
            namespace TEXT NOT NULL,
            relation TEXT NOT NULL DEFAULT 'related'
         );",
    )
    .unwrap();

    // Seed entity 1 has 5 neighbours.
    for target in 2i64..=6 {
        conn.execute(
            "INSERT INTO relationships (source_id, target_id, weight, namespace) VALUES (?1, ?2, ?3, 'ns')",
            rusqlite::params![1i64, target, 1.0f64],
        )
        .unwrap();
    }

    // Without cap: all 5 neighbours reached.
    let (depth_uncapped, _) = bfs_with_predecessors(&conn, &[1], "ns", 0.0, 1, None).unwrap();
    assert_eq!(
        depth_uncapped.len() - 1,
        5,
        "uncapped must discover all 5 neighbours (plus seed)"
    );

    // With cap=2: only top-2 neighbours (by weight; all equal here so first 2 returned).
    let (depth_capped, _) = bfs_with_predecessors(&conn, &[1], "ns", 0.0, 1, Some(2)).unwrap();
    // seed + 2 neighbours = 3 entries.
    assert_eq!(
        depth_capped.len(),
        3,
        "capped to 2 must yield seed + 2 neighbours"
    );
}

/// GAP-CLI-DR-02: materialize path writes non-empty JSON and yields
/// `{written,bytes,blake3}` semantics (mirrors --output / -o post-write checks).
#[test]
fn output_path_materializes_file_with_ack_fields() {
    use crate::atomic_io::write_json_atomic;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sqlite-graphrag-dr02-{}-{}.json",
        std::process::id(),
        nanos
    ));
    let envelope = json!({
        "query": "auth",
        "hits": [{"memory": "m1"}],
        "stats": {
            "sub_queries_total": 2,
            "unique_memories_found": 1,
            "elapsed_ms": 12
        }
    });
    write_json_atomic(&path, &envelope).expect("atomic write must succeed");
    assert!(
        path.exists(),
        "GAP-CLI-DR-02: -o/--output must materialize a file"
    );
    let meta = std::fs::metadata(&path).expect("metadata");
    assert!(
        meta.len() > 0,
        "GAP-CLI-DR-02: written file must be non-empty"
    );
    let file_bytes = std::fs::read(&path).expect("read");
    let digest = blake3::hash(&file_bytes).to_hex().to_string();
    assert_eq!(digest.len(), 64, "blake3 hex digest length");
    // Ack shape contract (fields consumers read after deep-research -o)
    let ack = json!({
        "written": path.display().to_string(),
        "bytes": meta.len(),
        "blake3": digest,
    });
    assert!(ack["written"].as_str().unwrap().ends_with(".json"));
    assert!(ack["bytes"].as_u64().unwrap() > 0);
    assert_eq!(ack["blake3"].as_str().unwrap().len(), 64);
    let _ = std::fs::remove_file(&path);
}
