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

/// GAP-SG-230 fixtures: the two spellings the same column ships under.
///
/// `graph --format json` emits `type` (`NodeOut` renames `r#type`), while
/// `graph entities`, `memory-entities` and `read --with-graph` emit
/// `entity_type`. Both shapes are reproduced literally so a reader can see that
/// the divergence is in the payload and not in the test.
fn nodes_spelled_type() -> Vec<Value> {
    vec![
        json!({ "id": 1, "name": "jwt", "kind": "concept", "type": "concept" }),
        json!({ "id": 2, "name": "auth-svc", "kind": "tool", "type": "tool" }),
    ]
}

fn entities_spelled_entity_type() -> Vec<Value> {
    vec![
        json!({ "name": "jwt", "entity_type": "concept" }),
        json!({ "name": "auth-svc", "entity_type": "tool" }),
    ]
}

/// GAP-SG-230: the caller learned `entity_type` on `graph entities` and asks
/// with it against `graph --format json`, which spells the same column `type`.
#[test]
fn entity_type_names_the_type_column_a_payload_actually_spells_type() {
    let items = nodes_spelled_type();
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    assert_eq!(
        scope.effective_key("entity_type").as_deref(),
        Some("type"),
        "the synonym has to name the spelling the payload really carries"
    );
}

/// The mirror: a caller that learned `type` on the json snapshot asks with it
/// against a surface that spells the column `entity_type`.
#[test]
fn type_names_the_entity_type_column_a_payload_actually_spells_entity_type() {
    let items = entities_spelled_entity_type();
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    assert_eq!(scope.effective_key("type").as_deref(), Some("entity_type"));
}

/// The caller's own spelling wins whenever the payload carries it, so a future
/// struct emitting BOTH names resolves to what was asked for rather than to
/// whichever the synonym table happens to list first.
#[test]
fn a_spelling_the_payload_carries_is_never_rewritten_to_its_synonym() {
    let items = vec![json!({ "entity_type": "concept", "type": "concept" })];
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    assert_eq!(
        scope.effective_key("entity_type").as_deref(),
        Some("entity_type")
    );
    assert_eq!(scope.effective_key("type").as_deref(), Some("type"));
}

/// The trap the scope column is written to avoid.
///
/// `kind` carries TWO incompatible meanings inside the single `graph` command.
/// In `NodeOut` it is the deprecated alias of the entity type; in `NdjsonNode` it
/// is the LINE DISCRIMINATOR, valued `"node"`, `"edge"` or `"summary"`. Under the
/// `graph-ndjson` slug it must therefore stay itself: were it a synonym of `type`
/// there, `--filter kind=concept` would reach edge and summary lines and
/// `--select type` would answer `"edge"` for an edge. GAP-SG-229 sharpened this:
/// `graph --format ndjson` now passes through the surface, so the discriminator
/// became a field a caller can project.
#[test]
fn kind_is_not_a_synonym_of_type_because_it_discriminates_ndjson_lines() {
    let lines = vec![
        json!({ "kind": "node", "id": 1, "name": "jwt", "type": "concept" }),
        json!({ "kind": "edge", "from": "jwt", "to": "auth-svc", "relation": "uses" }),
        json!({ "kind": "summary", "nodes": 1, "edges": 1 }),
    ];
    let envelope = json!({});
    let scope = Scope::new(&lines, &envelope).with_command(Some("graph-ndjson"));
    // Each name resolves to ITSELF and never to the other one.
    assert_eq!(scope.effective_key("kind").as_deref(), Some("kind"));
    assert_eq!(scope.effective_key("type").as_deref(), Some("type"));

    // And on a payload that carries only the discriminator, `type` must find
    // nothing rather than silently landing on `"edge"`.
    let edges = vec![json!({ "kind": "edge", "from": "a", "to": "b" })];
    let edge_scope = Scope::new(&edges, &envelope).with_command(Some("graph-ndjson"));
    assert_eq!(edge_scope.effective_key("type"), None);
    assert_eq!(edge_scope.effective_key("entity_type"), None);
}

/// GAP-SG-274: the same key, the same payload, two slugs, two verdicts.
///
/// This is what the scope column buys. `kind` is the entity type under `graph`,
/// so a caller that learned the spelling on the json snapshot resolves against
/// `graph entities`, which spells the column `entity_type`. Under
/// `graph-ndjson` the very same key must stay unresolved, because there it names
/// the line discriminator and a payload without one carries no such field.
#[test]
fn kind_resolves_as_the_type_column_only_under_the_slug_where_it_is_an_alias() {
    let items = entities_spelled_entity_type();
    let envelope = json!({});

    let json_scope = Scope::new(&items, &envelope).with_command(Some("graph"));
    assert_eq!(json_scope.classify("kind"), KeyOrigin::Element);
    assert_eq!(
        json_scope.effective_key("kind").as_deref(),
        Some("entity_type"),
        "under `graph` the deprecated alias names the entity type"
    );

    let ndjson_scope = Scope::new(&items, &envelope).with_command(Some("graph-ndjson"));
    assert_eq!(ndjson_scope.classify("kind"), KeyOrigin::Absent);
    assert_eq!(
        ndjson_scope.effective_key("kind"),
        None,
        "under `graph-ndjson` the discriminator is a different field entirely"
    );

    // An unstated slug takes the unscoped groups alone, which is the fail-safe
    // reading: no mode-specific synonym is invented for a caller that never said
    // which mode it is in.
    let unscoped = Scope::new(&items, &envelope);
    assert_eq!(unscoped.classify("kind"), KeyOrigin::Absent);
    assert_eq!(unscoped.effective_key("kind"), None);
}

/// The projection has to READ under the same scope the gate ADMITTED under.
///
/// GAP-SG-274. `--select kind` against `graph entities` passes the gate through
/// the scoped synonym; if the shaping pipeline did not carry the slug too, the
/// walk would miss `entity_type` and every element would project to `{}` — a key
/// accepted and then silently ignored, which is the failure class this surface
/// exists to remove.
#[test]
fn a_key_admitted_through_a_scoped_synonym_is_also_projected_through_it() {
    let mut surface = surface();
    surface.command = Some("graph".to_string());
    surface.select = vec!["kind".to_string()];
    let shaped = apply(
        &surface,
        json!({ "nodes": [ { "name": "jwt", "entity_type": "concept" } ] }),
    );
    assert_eq!(
        shaped["nodes"][0],
        json!({ "kind": "concept" }),
        "the projection answers under the name the caller asked with"
    );
}

/// The synonym is offered as a correction even though Jaro-Winkler scores it
/// below the floor: `entity_type` and `type` share no prefix, so the metric —
/// which is a proxy for "you mistyped this" — cannot see a relationship the
/// synonym table states as a fact.
#[test]
fn the_sibling_spelling_is_suggested_despite_the_similarity_floor() {
    let items = nodes_spelled_type();
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    let hits = scope.suggestions("entity_type");
    assert_eq!(
        hits.first().map(String::as_str),
        Some("type"),
        "the declared synonym takes the first slot: {hits:?}"
    );
}

/// A synonym never advertises a column this envelope has no data for.
#[test]
fn a_synonym_absent_from_the_payload_is_not_offered() {
    let items = vec![json!({ "name": "alpha", "score": 1 })];
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    assert!(
        !scope.suggestions("entity_type").iter().any(|h| h == "type"),
        "nothing here carries either spelling"
    );
}

/// Without this the synonym would degenerate into "everything resolves", which
/// would undo GAP-SG-202 wholesale.
#[test]
fn a_key_in_no_synonym_group_and_in_no_payload_still_resolves_to_nothing() {
    let items = nodes_spelled_type();
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    assert_eq!(scope.effective_key("chave_errada"), None);
    assert_eq!(scope.classify("chave_errada"), KeyOrigin::Absent);
}

/// The synonym rewrites the LAST segment and carries the prefix over, because
/// where a field sits is the caller's statement about the payload and is not
/// ours to rewrite. `deep-research` nests the column under `graph_context`.
#[test]
fn a_dotted_path_keeps_its_prefix_when_the_leaf_is_a_synonym() {
    let items = vec![json!({ "graph_context": { "type": "concept" } })];
    let envelope = json!({});
    let scope = Scope::new(&items, &envelope);
    assert_eq!(
        scope.effective_key("graph_context.entity_type").as_deref(),
        Some("graph_context.type")
    );
    // A bare leaf must NOT be produced: the column does not live at the root.
    assert_eq!(scope.effective_key("entity_type"), None);
}
