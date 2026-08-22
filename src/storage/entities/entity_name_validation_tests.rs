//! Entity NAME admission and the `type` alias contract (GAP-SG-146).
//!
//! Which names the graph accepts, and how the wire form tolerates `type` as
//! an alias for `entity_type` without accepting both at once.

use super::test_fixtures::*;
use super::*;

#[test]
fn accepts_type_field_as_alias() -> TestResult {
    let json = r#"{"name": "X", "type": "concept"}"#;
    let ent: NewEntity = serde_json::from_str(json)?;
    assert_eq!(ent.entity_type, "concept");
    Ok(())
}

#[test]
fn accepts_canonical_entity_type_field() -> TestResult {
    let json = r#"{"name": "X", "entity_type": "concept"}"#;
    let ent: NewEntity = serde_json::from_str(json)?;
    assert_eq!(ent.entity_type, "concept");
    Ok(())
}

#[test]
fn both_fields_present_yields_duplicate_error() {
    // having both entity_type and type in the same JSON is a duplicate and must fail
    let json = r#"{"name": "X", "entity_type": "concept", "type": "person"}"#;
    let result: Result<NewEntity, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "both fields in the same JSON are a duplicate"
    );
}

#[test]
fn validate_entity_name_accepts_valid() {
    assert!(validate_entity_name("rust-lang").is_ok());
    assert!(validate_entity_name("sqlite-graphrag").is_ok());
    assert!(validate_entity_name("ab").is_ok());
}

#[test]
fn validate_entity_name_rejects_short() {
    assert!(validate_entity_name("a").is_err());
    assert!(validate_entity_name("").is_err());
}

#[test]
fn validate_entity_name_rejects_newlines() {
    assert!(validate_entity_name("foo\nbar").is_err());
    assert!(validate_entity_name("foo\rbar").is_err());
}

#[test]
fn validate_entity_name_rejects_short_allcaps() {
    assert!(validate_entity_name("RAM").is_err());
    assert!(validate_entity_name("NAO").is_err());
    assert!(validate_entity_name("OK").is_err());
}

#[test]
fn validate_entity_name_accepts_long_allcaps() {
    assert!(validate_entity_name("SQLITE").is_ok());
    assert!(validate_entity_name("GRAPHRAG").is_ok());
}

#[test]
fn validate_entity_name_accepts_mixed_case() {
    assert!(validate_entity_name("FTS5").is_ok()); // 4 chars but has digit
    assert!(validate_entity_name("WAL").is_err()); // 3 chars ALL_CAPS
}

// v1.1.05 Bug 5: pure digit names must be rejected (ghost ID entities).
#[test]
fn validate_entity_name_rejects_purely_numeric() {
    assert!(validate_entity_name("89975").is_err());
    assert!(validate_entity_name("35313").is_err());
    assert!(validate_entity_name("12").is_err());
    // Mixed alphanumeric still OK.
    assert!(validate_entity_name("issue-89975").is_ok());
    assert!(validate_entity_name("v2").is_ok());
}

#[test]
fn entity_name_similarity_prefers_prefix_of_kebab() {
    let s = entity_name_similarity("alice", "alice-martins-souza");
    assert!(s >= 0.90, "expected strong prefix score, got {s}");
    let exact = entity_name_similarity("alice", "alice");
    assert!((exact - 1.0).abs() < f64::EPSILON);
}
