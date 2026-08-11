//! `--filter` grammar and evaluation, plus the envelopes it must never touch.

use super::*;

#[test]
fn filter_equals_keeps_only_matching_elements() {
    let mut s = surface();
    s.filters = vec![FilterExpr::parse("type=note").unwrap()];
    let shaped = apply(&s, envelope());
    assert_eq!(results(&shaped).len(), 3);
}

#[test]
fn filter_not_equals_and_contains() {
    let mut s = surface();
    s.filters = vec![FilterExpr::parse("type!=note").unwrap()];
    assert_eq!(results(&apply(&s, envelope())).len(), 1);

    let mut s = surface();
    s.filters = vec![FilterExpr::parse("name~ET").unwrap()];
    assert_eq!(results(&apply(&s, envelope())).len(), 2);
}

#[test]
fn filters_are_conjoined_with_and() {
    let mut s = surface();
    s.filters = vec![
        FilterExpr::parse("type=note").unwrap(),
        FilterExpr::parse("name=beta").unwrap(),
    ];
    assert_eq!(results(&apply(&s, envelope())).len(), 2);
}

#[test]
fn filter_matches_numbers_rendered_as_text() {
    let mut s = surface();
    s.filters = vec![FilterExpr::parse("score=0.5").unwrap()];
    assert_eq!(results(&apply(&s, envelope())).len(), 1);
}

#[test]
fn filter_reaches_nested_scalars_through_dotted_paths() {
    let value = json!({ "results": [ { "meta": { "kind": "x" } }, { "meta": { "kind": "y" } } ] });
    let mut s = surface();
    s.filters = vec![FilterExpr::parse("meta.kind=x").unwrap()];
    assert_eq!(results(&apply(&s, value)).len(), 1);
}

#[test]
fn filter_parses_every_operator_form() {
    assert_eq!(
        FilterExpr::parse("a=b").unwrap(),
        FilterExpr::parse("a==b").unwrap()
    );
    assert!(FilterExpr::parse("a!=b")
        .unwrap()
        .matches(&json!({"a": "c"})));
    assert!(FilterExpr::parse("a~B")
        .unwrap()
        .matches(&json!({"a": "abc"})));
    // `!=` wins over the `=` it contains.
    assert!(!FilterExpr::parse("a!=b")
        .unwrap()
        .matches(&json!({"a": "b"})));
}

#[test]
fn filter_rejects_malformed_expressions() {
    assert!(FilterExpr::parse("no-operator-here").is_err());
    assert!(FilterExpr::parse("=orphan").is_err());
    assert!(FilterExpr::parse("~orphan").is_err());
}

#[test]
fn filter_treats_missing_key_as_not_equal_but_never_as_equal() {
    let element = json!({ "other": 1 });
    assert!(!FilterExpr::parse("name=x").unwrap().matches(&element));
    assert!(FilterExpr::parse("name!=x").unwrap().matches(&element));
    assert!(!FilterExpr::parse("name~x").unwrap().matches(&element));
}

#[test]
fn filter_never_silences_an_error_envelope() {
    let mut s = surface();
    s.filters = vec![FilterExpr::parse("name=nothing-matches-this").unwrap()];
    s.select = vec!["name".into()];
    s.count_only = true;

    let failure = json!({ "error": true, "code": 4, "message": "not found" });
    assert_eq!(apply(&s, failure.clone()), failure);

    let not_ok = json!({ "ok": false, "code": 11, "message": "embedding failed" });
    assert_eq!(apply(&s, not_ok.clone()), not_ok);
}

#[test]
fn json_schema_documents_pass_through_untouched() {
    let mut s = surface();
    s.select = vec!["name".into()];
    let schema =
        json!({ "$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object" });
    assert_eq!(apply(&s, schema.clone()), schema);
}

#[test]
fn filter_op_variants_are_distinct() {
    assert_ne!(FilterOp::Equals, FilterOp::NotEquals);
    assert_ne!(FilterOp::Equals, FilterOp::Contains);
}
