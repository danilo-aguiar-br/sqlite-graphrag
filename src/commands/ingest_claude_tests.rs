use super::binary::find_claude_binary;
use super::extract::parse_claude_output;
use super::types::EXTRACTION_SCHEMA;
use crate::entity_type::EntityType;
use crate::errors::AppError;

#[test]
fn test_extraction_schema_valid_json() {
    let _: serde_json::Value =
        serde_json::from_str(EXTRACTION_SCHEMA).expect("schema must be valid JSON");
}

#[test]
fn test_parse_claude_output_valid() {
    let output = r#"[
        {"type":"system","subtype":"init"},
        {"type":"assistant"},
        {"type":"result","is_error":false,"total_cost_usd":0.02,"structured_output":{"name":"test-doc","description":"A test document","entities":[{"name":"test-entity","entity_type":"concept"}],"relationships":[{"source":"test-entity","target":"test-doc","relation":"applies-to","strength":0.8}]}}
    ]"#;
    let (result, cost, _is_oauth) = parse_claude_output(output).expect("parse must succeed");
    assert_eq!(result.name, "test-doc");
    assert_eq!(result.entities.len(), 1);
    assert_eq!(result.relationships.len(), 1);
    assert!((cost - 0.02).abs() < f64::EPSILON);
}

#[test]
fn test_parse_claude_output_error() {
    let output = r#"[
        {"type":"system","subtype":"init"},
        {"type":"result","is_error":true,"error":"authentication failed"}
    ]"#;
    let err = parse_claude_output(output).unwrap_err();
    assert!(format!("{err}").contains("authentication failed"));
}

#[test]
fn test_parse_claude_output_rate_limit() {
    let output = r#"[
        {"type":"system","subtype":"init"},
        {"type":"result","is_error":true,"error":"rate_limit exceeded"}
    ]"#;
    let err = parse_claude_output(output).unwrap_err();
    assert!(matches!(err, AppError::RateLimited { .. }));
}

#[test]
fn test_parse_claude_output_malformed() {
    let output = "not json at all";
    assert!(parse_claude_output(output).is_err());
}

#[test]
fn test_find_claude_binary_not_found() {
    let original_path = std::env::var_os("PATH");
    std::env::set_var("PATH", "/nonexistent");
    std::env::remove_var("SQLITE_GRAPHRAG_CLAUDE_BINARY");
    let result = find_claude_binary(None);
    if let Some(p) = original_path {
        std::env::set_var("PATH", p);
    }
    assert!(result.is_err());
}

#[test]
fn test_parse_claude_output_result_fallback() {
    let output = r#"[
        {"type":"system","subtype":"init"},
        {"type":"result","is_error":false,"total_cost_usd":0.01,"structured_output":null,"result":"{\"name\":\"test-fallback\",\"description\":\"A fallback test\",\"entities\":[{\"name\":\"fb-entity\",\"entity_type\":\"concept\"}],\"relationships\":[]}"}
    ]"#;
    let (result, cost, _is_oauth) =
        parse_claude_output(output).expect("result fallback must work");
    assert_eq!(result.name, "test-fallback");
    assert_eq!(result.entities.len(), 1);
    assert!(result.relationships.is_empty());
    assert!((cost - 0.01).abs() < f64::EPSILON);
}

#[test]
fn test_parse_claude_output_error_with_result_field() {
    let output = r#"[
        {"type":"system","subtype":"init"},
        {"type":"result","is_error":true,"result":"Not logged in · Please run /login"}
    ]"#;
    let err = parse_claude_output(output).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Not logged in"),
        "expected 'Not logged in' in: {msg}"
    );
}

#[test]
fn test_terminal_reason_max_turns_detected() {
    let output = r#"[
        {"type":"system","subtype":"init"},
        {"type":"result","is_error":false,"terminal_reason":"max_turns","structured_output":{"name":"t","description":"d","entities":[],"relationships":[]}}
    ]"#;
    let err_or_ok = parse_claude_output(output);
    assert!(
        err_or_ok.is_ok(),
        "max_turns in result without is_error should still parse"
    );
}

#[test]
fn test_detect_oauth_from_init_json() {
    let output = r#"[
        {"type":"system","subtype":"init","apiKeySource":"none"},
        {"type":"result","is_error":false,"total_cost_usd":0.50,"structured_output":{"name":"test-oauth","description":"oauth test","entities":[],"relationships":[]}}
    ]"#;
    let (_result, cost, is_oauth) = parse_claude_output(output).expect("parse must succeed");
    assert!(is_oauth, "apiKeySource=none must be detected as OAuth");
    assert!((cost - 0.50).abs() < f64::EPSILON);
}

#[test]
fn test_api_key_source_not_oauth() {
    let output = r#"[
        {"type":"system","subtype":"init","apiKeySource":"env"},
        {"type":"result","is_error":false,"total_cost_usd":0.10,"structured_output":{"name":"test-api","description":"api test","entities":[],"relationships":[]}}
    ]"#;
    let (_result, _cost, is_oauth) = parse_claude_output(output).expect("parse must succeed");
    assert!(!is_oauth, "apiKeySource=env must NOT be detected as OAuth");
}

#[test]
fn test_missing_api_key_source_defaults_not_oauth() {
    let output = r#"[
        {"type":"system","subtype":"init"},
        {"type":"result","is_error":false,"total_cost_usd":0.05,"structured_output":{"name":"test-missing","description":"missing test","entities":[],"relationships":[]}}
    ]"#;
    let (_result, _cost, is_oauth) = parse_claude_output(output).expect("parse must succeed");
    assert!(!is_oauth, "missing apiKeySource must default to not OAuth");
}

#[test]
fn test_extraction_schema_entity_types_match_enum() {
    let schema: serde_json::Value = serde_json::from_str(EXTRACTION_SCHEMA).unwrap();
    let types = schema["properties"]["entities"]["items"]["properties"]["entity_type"]["enum"]
        .as_array()
        .expect("schema must have entity_type enum");
    for t in types {
        let s = t.as_str().unwrap();
        assert!(
            s.parse::<EntityType>().is_ok(),
            "schema entity_type '{s}' not in EntityType enum"
        );
    }
}
