//! Key resolution itself: where a key lives, and what a near miss suggests.

use super::*;
use crate::agent_surface::vocabulary::{KeyOrigin, Scope};

fn elements() -> Vec<Value> {
    vec![
        json!({ "name": "alpha", "meta": { "kind": "note" } }),
        json!({ "name": "beta" }),
    ]
}

#[test]
fn a_key_carried_by_any_element_resolves_to_the_elements() {
    let items = elements();
    let envelope = json!({ "total_count": 2 });
    let scope = Scope::new(&items, &envelope);
    assert_eq!(scope.classify("name"), KeyOrigin::Element);
}

/// Resolution scans EVERY element, never a sample: a key present only in the
/// last element still resolves, or the gate would refuse a legitimate request.
#[test]
fn a_key_present_in_a_single_late_element_still_resolves() {
    let mut items: Vec<Value> = (0..500).map(|i| json!({ "id": i })).collect();
    items.push(json!({ "rare_field": true }));
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    assert_eq!(scope.classify("rare_field"), KeyOrigin::Element);
}

#[test]
fn a_dotted_path_resolves_through_nested_objects() {
    let items = elements();
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    assert_eq!(scope.classify("meta.kind"), KeyOrigin::Element);
    assert_eq!(scope.classify("meta.missing"), KeyOrigin::Absent);
}

/// The distinction GAP-SG-203 turns on: the key exists, but not where the
/// predicate would look for it.
#[test]
fn a_key_only_on_the_envelope_is_reported_as_envelope_only() {
    let items = elements();
    let envelope = json!({ "total_count": 2, "integrity_ok": true });
    let scope = Scope::new(&items, &envelope);
    assert_eq!(scope.classify("integrity_ok"), KeyOrigin::EnvelopeOnly);
    assert_eq!(scope.classify("total_count"), KeyOrigin::EnvelopeOnly);
}

#[test]
fn a_key_nowhere_in_scope_is_absent() {
    let items = elements();
    let envelope = json!({ "total_count": 2 });
    let scope = Scope::new(&items, &envelope);
    assert_eq!(scope.classify("chave_errada"), KeyOrigin::Absent);
}

#[test]
fn suggestions_rank_the_nearest_element_key_first() {
    let items = elements();
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    let hits = scope.suggestions("nmae");
    assert_eq!(hits.first().map(String::as_str), Some("name"), "{hits:?}");
}

/// A `read` envelope carries no array, so the vocabulary has to fall back to the
/// envelope's own keys — otherwise the refusal GAP-SG-202 was written from
/// (`--select body_length read`) names no alternative at all.
#[test]
fn suggestions_fall_back_to_envelope_keys_when_there_are_no_elements() {
    let items: Vec<Value> = Vec::new();
    let envelope = json!({ "body": "text", "body_hash": "abc", "name": "x" });
    let scope = Scope::new(&items, &envelope);
    let hits = scope.suggestions("body_lenght");
    assert!(
        hits.iter().any(|h| h == "body"),
        "expected `body` among the alternatives: {hits:?}"
    );
}

/// Nothing close means nothing offered. An empty list is informative on its own:
/// the caller is looking at the wrong command, not at a typo.
#[test]
fn a_key_resembling_nothing_produces_no_suggestion() {
    let items = elements();
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    assert!(scope.suggestions("zzzzzzzzzzzz").is_empty());
}

/// Ties break by name, so two equally close candidates come out in the same
/// order on Linux, macOS and Windows.
#[test]
fn suggestions_are_deterministic_across_platforms() {
    let items = vec![json!({ "aaa": 1, "aab": 2, "aac": 3 })];
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    assert_eq!(scope.suggestions("aad"), scope.suggestions("aad"));
}
