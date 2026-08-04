//! Response-shape and flag-parsing invariants of `hybrid-search`.

use super::*;

#[derive(clap::Parser)]
struct TestCli {
    #[command(flatten)]
    args: HybridSearchArgs,
}

#[test]
fn graph_flags_parse_as_none_when_absent() {
    // G48: with plain u32/f64 defaults, an explicit `--max-hops 2` was
    // indistinguishable from the default and silently bypassed the G20
    // validation. Option<T> restores real flag-presence detection.
    use clap::Parser;
    let cli = TestCli::try_parse_from(["hybrid-search", "q"]).expect("bare query parses");
    assert!(cli.args.max_hops.is_none());
    assert!(cli.args.min_weight.is_none());
    let cli = TestCli::try_parse_from(["hybrid-search", "q", "--max-hops", "2"])
        .expect("explicit flag parses");
    assert_eq!(cli.args.max_hops, Some(2));
}

fn empty_response(k: usize, rrf_k: u32, weight_vec: f32, weight_fts: f32) -> HybridSearchResponse {
    HybridSearchResponse {
        query: "test query".to_string(),
        k,
        rrf_k,
        weights: Weights {
            vec: weight_vec,
            fts: weight_fts,
        },
        results: vec![],
        graph_matches: vec![],
        max_graph_results: Some(crate::constants::DEFAULT_HYBRID_MAX_GRAPH_RESULTS),
        fts_degraded: false,
        fts_error: None,
        fts_auto_rebuilt: false,
        vec_degraded: false,
        vec_error: None,
        warning: None,
        backend_invoked: None,
        vec_degraded_reason: None,
        elapsed_ms: 0,
    }
}

#[test]
fn hybrid_search_response_empty_serializes_correct_fields() {
    let resp = empty_response(10, 60, 1.0, 1.0);
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"results\""), "must contain results field");
    assert!(json.contains("\"query\""), "must contain query field");
    assert!(json.contains("\"k\""), "must contain k field");
    assert!(
        json.contains("\"graph_matches\""),
        "must contain graph_matches field"
    );
    assert!(
        !json.contains("\"combined_rank\""),
        "must not contain combined_rank"
    );
    assert!(
        !json.contains("\"vec_rank_list\""),
        "must not contain vec_rank_list"
    );
    assert!(
        !json.contains("\"fts_rank_list\""),
        "must not contain fts_rank_list"
    );
}

#[test]
fn hybrid_search_response_serializes_rrf_k_and_weights() {
    let resp = empty_response(5, 60, 0.7, 0.3);
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"rrf_k\""), "must contain rrf_k field");
    assert!(json.contains("\"weights\""), "must contain weights field");
    assert!(json.contains("\"vec\""), "must contain weights.vec field");
    assert!(json.contains("\"fts\""), "must contain weights.fts field");
}

#[test]
fn hybrid_search_response_serializes_elapsed_ms() {
    let mut resp = empty_response(5, 60, 1.0, 1.0);
    resp.elapsed_ms = 123;
    let json = serde_json::to_string(&resp).unwrap();
    assert!(
        json.contains("\"elapsed_ms\""),
        "must contain elapsed_ms field"
    );
    assert!(json.contains("123"), "deve serializar valor de elapsed_ms");
}

#[test]
fn weights_struct_serializes_correctly() {
    let w = Weights { vec: 0.6, fts: 0.4 };
    let json = serde_json::to_string(&w).unwrap();
    assert!(json.contains("\"vec\""));
    assert!(json.contains("\"fts\""));
}

#[test]
fn hybrid_search_item_omits_fts_rank_when_none() {
    let item = HybridSearchItem {
        memory_id: 1,
        name: "mem".to_string(),
        namespace: "default".to_string(),
        memory_type: "user".to_string(),
        description: "desc".to_string(),
        body: "content".to_string(),
        snippet: "content".to_string(),
        combined_score: 0.0328,
        score: 0.0328,
        source: "hybrid".to_string(),
        vec_rank: Some(1),
        fts_rank: None,
        rrf_score: Some(0.0328),
        normalized_score: 1.0,
        vec_distance: Some(0.12),
        fts_bm25: None,
    };
    let json = serde_json::to_string(&item).unwrap();
    assert!(
        json.contains("\"vec_rank\""),
        "must contain vec_rank when Some"
    );
    assert!(
        !json.contains("\"fts_rank\""),
        "must not contain fts_rank when None"
    );
}

#[test]
fn hybrid_search_item_omits_vec_rank_when_none() {
    let item = HybridSearchItem {
        memory_id: 2,
        name: "mem2".to_string(),
        namespace: "default".to_string(),
        memory_type: "fact".to_string(),
        description: "desc2".to_string(),
        body: "corpo2".to_string(),
        snippet: "corpo2".to_string(),
        combined_score: 0.016,
        score: 0.016,
        source: "hybrid".to_string(),
        vec_rank: None,
        fts_rank: Some(2),
        rrf_score: Some(0.016),
        normalized_score: 0.5,
        vec_distance: None,
        fts_bm25: None,
    };
    let json = serde_json::to_string(&item).unwrap();
    assert!(
        !json.contains("\"vec_rank\""),
        "must not contain vec_rank when None"
    );
    assert!(
        json.contains("\"fts_rank\""),
        "must contain fts_rank when Some"
    );
}

#[test]
fn hybrid_search_item_serializes_both_ranks_when_some() {
    let item = HybridSearchItem {
        memory_id: 3,
        name: "mem3".to_string(),
        namespace: "ns".to_string(),
        memory_type: "entity".to_string(),
        description: "desc3".to_string(),
        body: "corpo3".to_string(),
        snippet: "corpo3".to_string(),
        combined_score: 0.05,
        score: 0.05,
        source: "hybrid".to_string(),
        vec_rank: Some(3),
        fts_rank: Some(1),
        rrf_score: Some(0.05),
        normalized_score: 0.8,
        vec_distance: Some(0.25),
        fts_bm25: None,
    };
    let json = serde_json::to_string(&item).unwrap();
    assert!(json.contains("\"vec_rank\""), "must contain vec_rank");
    assert!(json.contains("\"fts_rank\""), "must contain fts_rank");
    assert!(json.contains("\"type\""), "deve serializar type renomeado");
    assert!(!json.contains("memory_type"), "must not expose memory_type");
}

#[test]
fn hybrid_search_response_serializes_k_correctly() {
    let resp = empty_response(5, 60, 1.0, 1.0);
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"k\":5"), "deve serializar k=5");
}

#[test]
fn hybrid_search_response_with_graph_matches() {
    use crate::output::RecallItem;
    let resp = HybridSearchResponse {
        query: "test".to_string(),
        k: 5,
        rrf_k: 60,
        weights: Weights { vec: 1.0, fts: 1.0 },
        results: vec![],
        graph_matches: vec![RecallItem {
            memory_id: 1,
            name: "graph-hit".to_string(),
            namespace: "global".to_string(),
            memory_type: "document".to_string(),
            description: "found via graph".to_string(),
            snippet: "graph content".to_string(),
            distance: 0.1,
            score: 0.9,
            source: "graph".to_string(),
            graph_depth: Some(1),
        }],
        max_graph_results: Some(crate::constants::DEFAULT_HYBRID_MAX_GRAPH_RESULTS),
        fts_degraded: false,
        fts_error: None,
        fts_auto_rebuilt: false,
        vec_degraded: false,
        vec_error: None,
        warning: None,
        backend_invoked: None,
        vec_degraded_reason: None,
        elapsed_ms: 42,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["graph_matches"].as_array().unwrap().len(), 1);
    assert_eq!(json["graph_matches"][0]["source"], "graph");
    assert_eq!(json["graph_matches"][0]["graph_depth"], 1);
}

#[test]
fn fts_degraded_omitted_on_success_present_on_failure() {
    // Happy path: fts_degraded=false must be absent from JSON (skip_serializing_if).
    let ok_resp = empty_response(5, 60, 1.0, 1.0);
    let ok_json = serde_json::to_string(&ok_resp).unwrap();
    assert!(
        !ok_json.contains("\"fts_degraded\""),
        "fts_degraded must be absent when false"
    );
    assert!(
        !ok_json.contains("\"fts_error\""),
        "fts_error must be absent when None"
    );

    // Degraded path: fts_degraded=true and fts_error=Some must appear in JSON.
    let mut degraded_resp = empty_response(5, 60, 1.0, 1.0);
    degraded_resp.fts_degraded = true;
    degraded_resp.fts_error = Some("FTS5 table corrupted".to_string());
    let degraded_json = serde_json::to_string(&degraded_resp).unwrap();
    assert!(
        degraded_json.contains("\"fts_degraded\":true"),
        "fts_degraded must be present and true when degraded"
    );
    assert!(
        degraded_json.contains("\"fts_error\""),
        "fts_error must be present when Some"
    );
    assert!(
        degraded_json.contains("FTS5 table corrupted"),
        "fts_error must contain the error message"
    );
}
