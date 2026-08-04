//! Unit coverage for the output layer.
//!
//! Lives in its own module rather than at the bottom of each emitter so the
//! serialization contract of the response payloads is asserted in one place.

use super::*;
use serde::Serialize;

#[derive(Serialize)]
struct Dummy {
    val: u32,
}

// Non-serializable type to force a JSON serialization error
struct NotSerializable;
impl Serialize for NotSerializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(
            "intentional serialization failure",
        ))
    }
}

#[test]
fn emit_json_returns_ok_for_valid_value() {
    let v = Dummy { val: 42 };
    assert!(emit_json(&v).is_ok());
}

#[test]
fn emit_json_returns_err_for_non_serializable_value() {
    let v = NotSerializable;
    assert!(emit_json(&v).is_err());
}

#[test]
fn emit_json_compact_returns_ok_for_valid_value() {
    let v = Dummy { val: 7 };
    assert!(emit_json_compact(&v).is_ok());
}

#[test]
fn emit_json_compact_returns_err_for_non_serializable_value() {
    let v = NotSerializable;
    assert!(emit_json_compact(&v).is_err());
}

#[test]
fn emit_text_does_not_panic() {
    emit_text("mensagem de teste");
}

#[test]
fn emit_progress_does_not_panic() {
    emit_progress("progresso de teste");
}

#[test]
fn remember_response_serializes_correctly() {
    let r = RememberResponse {
        memory_id: 1,
        name: "teste".to_string(),
        namespace: "ns".to_string(),
        action: "created".to_string(),
        operation: "created".to_string(),
        version: 1,
        entities_persisted: 2,
        relationships_persisted: 3,
        relationships_truncated: false,
        chunks_created: 4,
        chunks_persisted: 4,
        urls_persisted: 2,
        extraction_method: None,
        merged_into_memory_id: None,
        warnings: vec!["aviso".to_string()],
        created_at: 1776569715,
        created_at_iso: "2026-04-19T03:34:15Z".to_string(),
        elapsed_ms: 123,
        name_was_normalized: false,
        original_name: None,
        backend_invoked: None,
        entities_created: vec![],
        enrich_recommended: vec![],
    };
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("memory_id"));
    assert!(json.contains("aviso"));
    assert!(json.contains("\"namespace\""));
    assert!(json.contains("\"merged_into_memory_id\""));
    assert!(json.contains("\"operation\""));
    assert!(json.contains("\"created_at\""));
    assert!(json.contains("\"created_at_iso\""));
    assert!(json.contains("\"elapsed_ms\""));
    assert!(json.contains("\"urls_persisted\""));
    assert!(json.contains("\"relationships_truncated\":false"));
}

#[test]
fn recall_item_serializes_renamed_type_field() {
    let item = RecallItem {
        memory_id: 10,
        name: "entidade".to_string(),
        namespace: "ns".to_string(),
        memory_type: "entity".to_string(),
        description: "desc".to_string(),
        snippet: "trecho".to_string(),
        distance: 0.5,
        score: RecallItem::score_from_distance(0.5),
        source: "db".to_string(),
        graph_depth: None,
    };
    let json = serde_json::to_string(&item).unwrap();
    assert!(json.contains("\"type\""));
    assert!(!json.contains("memory_type"));
    // Field is omitted from JSON when None.
    assert!(!json.contains("graph_depth"));
    assert!(json.contains("\"score\":0.5"));
}

#[test]
fn recall_response_serializes_with_lists() {
    let resp = RecallResponse {
        query: "busca".to_string(),
        k: 10,
        direct_matches: vec![],
        graph_matches: vec![],
        results: vec![],
        elapsed_ms: 42,
        vec_degraded: false,
        vec_error: None,
        warning: None,
        backend_invoked: None,
        vec_degraded_reason: None,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("direct_matches"));
    assert!(json.contains("graph_matches"));
    assert!(json.contains("\"k\":"));
    assert!(json.contains("\"results\""));
    assert!(json.contains("\"elapsed_ms\""));
    // G58: clean response must NOT carry the degradation fields.
    assert!(!json.contains("vec_degraded"));
    assert!(!json.contains("vec_error"));
    assert!(!json.contains("warning"));
}

#[test]
fn recall_response_serializes_vec_degraded_when_fallback_fired() {
    let resp = RecallResponse {
        query: "busca".to_string(),
        k: 10,
        direct_matches: vec![],
        graph_matches: vec![],
        results: vec![],
        elapsed_ms: 42,
        vec_degraded: true,
        vec_error: Some("embedding cancelled by external signal".to_string()),
        warning: Some("live query embedding unavailable; results are FTS5 BM25 only (semantic relevance reduced)".to_string()),
        backend_invoked: None,
        vec_degraded_reason: Some("embedding cancelled by external signal".to_string()),
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"vec_degraded\":true"));
    assert!(json.contains("\"vec_error\":\"embedding cancelled by external signal\""));
    assert!(json.contains("\"warning\":\"live query embedding unavailable"));
}

#[test]
fn error_envelope_serializes_correctly() {
    #[derive(serde::Serialize)]
    struct ErrorEnvelope<'a> {
        error: bool,
        code: i32,
        message: &'a str,
    }
    let envelope = ErrorEnvelope {
        error: true,
        code: 10,
        message: "database disk image is malformed",
    };
    let json = serde_json::to_value(&envelope).unwrap();
    assert_eq!(json["error"], true);
    assert_eq!(json["code"], 10);
    assert_eq!(json["message"], "database disk image is malformed");
}

#[test]
fn output_format_default_is_json() {
    let fmt = OutputFormat::default();
    assert!(matches!(fmt, OutputFormat::Json));
}

#[test]
fn output_format_variants_exist() {
    let _text = OutputFormat::Text;
    let _md = OutputFormat::Markdown;
    let _json = OutputFormat::Json;
}

#[test]
fn recall_item_clone_produces_equal_value() {
    let item = RecallItem {
        memory_id: 99,
        name: "clone".to_string(),
        namespace: "ns".to_string(),
        memory_type: "relation".to_string(),
        description: "d".to_string(),
        snippet: "s".to_string(),
        distance: 0.1,
        score: RecallItem::score_from_distance(0.1),
        source: "src".to_string(),
        graph_depth: Some(2),
    };
    let cloned = item.clone();
    assert_eq!(cloned.memory_id, item.memory_id);
    assert_eq!(cloned.name, item.name);
    assert_eq!(cloned.graph_depth, Some(2));
}
