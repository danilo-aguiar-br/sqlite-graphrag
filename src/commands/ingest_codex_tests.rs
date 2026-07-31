use super::extract::parse_codex_output;
use super::types::EXTRACTION_SCHEMA_CODEX;
use crate::errors::AppError;

fn make_agent_message_event(text: &str) -> String {
    format!(
        r#"{{"type":"item.completed","item":{{"id":"item_0","type":"agent_message","text":{}}}}}"#,
        serde_json::to_string(text).unwrap()
    )
}

fn make_usage_event(input: u64, output: u64) -> String {
    format!(
        r#"{{"type":"turn.completed","usage":{{"input_tokens":{input},"output_tokens":{output}}}}}"#
    )
}

fn valid_extraction_json() -> String {
    r#"{"name":"test-module","description":"A test module for unit testing purposes","entities":[{"name":"test-entity","entity_type":"concept"}],"relationships":[{"source":"test-entity","target":"test-module","relation":"applies-to","strength":0.8}]}"#.to_string()
}

#[test]
fn test_parse_codex_output_valid() {
    let jsonl = format!(
        "{}\n{}\n{}",
        r#"{"type":"thread.started","thread_id":"t1"}"#,
        make_agent_message_event(&valid_extraction_json()),
        make_usage_event(100, 50),
    );

    let (result, usage) = parse_codex_output(&jsonl).expect("parse must succeed");
    assert_eq!(result.name, "test-module");
    assert_eq!(result.entities.len(), 1);
    assert_eq!(result.relationships.len(), 1);
    let u = usage.expect("usage must be present");
    assert_eq!(u.input_tokens, 100);
    assert_eq!(u.output_tokens, 50);
}

#[test]
fn test_parse_codex_output_turn_failed() {
    let jsonl = format!(
        "{}\n{}",
        r#"{"type":"thread.started","thread_id":"t1"}"#,
        r#"{"type":"turn.failed","error":{"message":"model error occurred"}}"#,
    );

    let err = parse_codex_output(&jsonl).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("turn failed") || msg.contains("turno codex falhou"),
        "expected 'turn failed' in: {msg}"
    );
    assert!(msg.contains("model error occurred"));
}

#[test]
fn test_parse_codex_output_rate_limit() {
    let jsonl = r#"{"type":"turn.failed","error":{"message":"rate_limit exceeded, 429 Too Many Requests"}}"#;

    let err = parse_codex_output(jsonl).unwrap_err();
    assert!(
        matches!(err, AppError::RateLimited { .. }),
        "expected AppError::RateLimited, got: {err}"
    );
}

#[test]
fn test_parse_codex_output_schema_error() {
    let jsonl =
        r#"{"type":"error","message":"invalid_json_schema: additional properties not allowed"}"#;

    let err = parse_codex_output(jsonl).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("invalid_json_schema") || msg.contains("schema"),
        "expected schema error in: {msg}"
    );
}

#[test]
fn test_extraction_schema_codex_valid_json() {
    let _: serde_json::Value =
        serde_json::from_str(EXTRACTION_SCHEMA_CODEX).expect("schema must be valid JSON");
}

#[test]
fn test_extraction_schema_codex_has_additional_properties_false() {
    let schema: serde_json::Value =
        serde_json::from_str(EXTRACTION_SCHEMA_CODEX).expect("schema must be valid JSON");

    // Root level
    assert_eq!(
        schema["additionalProperties"].as_bool(),
        Some(false),
        "root must have additionalProperties: false"
    );

    // Entity items level
    assert_eq!(
        schema["properties"]["entities"]["items"]["additionalProperties"].as_bool(),
        Some(false),
        "entity items must have additionalProperties: false"
    );

    // Relationship items level
    assert_eq!(
        schema["properties"]["relationships"]["items"]["additionalProperties"].as_bool(),
        Some(false),
        "relationship items must have additionalProperties: false"
    );
}

#[test]
fn test_parse_codex_output_last_agent_message_wins() {
    // Multiple agent_message items — last one should win
    let first_text = r#"{"name":"first-result","description":"First result should be ignored","entities":[],"relationships":[]}"#;
    let second_text = r#"{"name":"final-result","description":"Final result wins over earlier ones","entities":[{"name":"final-entity","entity_type":"concept"}],"relationships":[]}"#;

    let jsonl = format!(
        "{}\n{}\n{}\n{}",
        r#"{"type":"thread.started","thread_id":"t1"}"#,
        make_agent_message_event(first_text),
        make_agent_message_event(second_text),
        make_usage_event(200, 80),
    );

    let (result, _) = parse_codex_output(&jsonl).expect("parse must succeed");
    assert_eq!(result.name, "final-result", "last agent_message should win");
    assert_eq!(result.entities.len(), 1);
}

#[test]
fn test_parse_codex_output_skips_malformed_lines() {
    let jsonl = format!(
        "not json at all\n{}\n{{broken\n{}",
        make_agent_message_event(&valid_extraction_json()),
        make_usage_event(10, 5),
    );

    // Should succeed despite malformed lines
    let (result, _) = parse_codex_output(&jsonl).expect("malformed lines must be skipped");
    assert_eq!(result.name, "test-module");
}
