//! Derived-array suppression: which members restate the reshaped one, and for
//! which subcommand.

use super::*;

#[test]
fn recall_aliases_are_dropped_once_results_is_reshaped() {
    // The real `recall` envelope: `results` is the concatenation of the two
    // other arrays, so shaping `results` while keeping them would hand the
    // caller the unshaped rows back under a different name.
    let value = json!({
        "query": "x",
        "direct_matches": [{ "n": 1 }, { "n": 2 }],
        "graph_matches": [{ "n": 3 }],
        "results": [{ "n": 1 }, { "n": 2 }, { "n": 3 }]
    });
    let mut s = surface_for("recall");
    s.max_items = 1;
    let shaped = apply(&s, value);

    assert_eq!(shaped["results"].as_array().unwrap().len(), 1);
    assert!(shaped.get("direct_matches").is_none());
    assert!(shaped.get("graph_matches").is_none());
    assert_eq!(
        shaped["agent_surface"]["aliases_removed"],
        json!(["direct_matches", "graph_matches"])
    );
    // The record still describes the canonical array, not the aliases.
    assert_eq!(shaped["agent_surface"]["input_count"], json!(3));
    assert_eq!(shaped["agent_surface"]["output_count"], json!(1));
}

#[test]
fn aliases_removed_is_absent_when_the_envelope_carries_no_alias() {
    let mut s = surface();
    s.max_items = 1;
    let shaped = apply(&s, envelope());
    assert!(shaped["agent_surface"].get("aliases_removed").is_none());
}

#[test]
fn list_alias_leaves_the_envelope_when_the_canonical_array_is_filtered() {
    let mut s = surface_for("list");
    s.filters = vec![FilterExpr::parse("memory_type=skill").unwrap()];
    let shaped = apply(&s, list_envelope());

    assert_eq!(shaped["items"].as_array().unwrap().len(), 2);
    assert!(
        shaped.get("memories").is_none(),
        "the unfiltered clone must not survive the filter: {shaped}"
    );
    assert_eq!(
        shaped["agent_surface"]["aliases_removed"],
        json!(["memories"])
    );
    assert_eq!(shaped["total_count"], json!(3));
}

#[test]
fn list_alias_survives_a_noop_surface_so_the_public_contract_is_intact() {
    let original = list_envelope();
    assert_eq!(apply(&surface(), original.clone()), original);
}

/// `docs/TESTING.md` states the surface is opt-in: with no knob set the
/// envelope is byte-for-byte identical to the pre-v1.2.2 output. Suppression
/// must never weaken that, so the check is on the serialization, not just on
/// structural equality.
#[test]
fn a_noop_surface_emits_every_alias_envelope_byte_for_byte() {
    let related = json!({
        "name": "seed",
        "hops": 1,
        "results": [{ "n": 1 }, { "n": 2 }],
        "related_memories": [{ "n": 1 }, { "n": 2 }]
    });
    let recall = json!({
        "query": "x",
        "direct_matches": [{ "n": 1 }],
        "graph_matches": [{ "n": 2 }],
        "results": [{ "n": 1 }, { "n": 2 }]
    });
    let graph = json!({ "nodes": [{ "n": 1 }], "entities": [{ "n": 1 }], "edges": [] });

    for original in [list_envelope(), related, recall, graph] {
        let before = serde_json::to_string(&original).unwrap();
        let after = serde_json::to_string(&apply(&surface(), original)).unwrap();
        assert_eq!(before, after, "the inert surface must not touch a byte");
    }
}

#[test]
fn related_alias_is_dropped_and_absent_siblings_are_a_silent_noop() {
    // `related` shares the `results` canonical key with `recall` but carries
    // only one of the three declared derived members. The other two must not
    // raise an error and must not appear in the record.
    let value = json!({
        "name": "seed",
        "hops": 1,
        "results": [{ "n": 1 }, { "n": 2 }],
        "related_memories": [{ "n": 1 }, { "n": 2 }]
    });
    let mut s = surface_for("related");
    s.select = vec!["n".into()];
    let shaped = apply(&s, value);

    assert_eq!(shaped["results"].as_array().unwrap().len(), 2);
    assert!(shaped.get("related_memories").is_none());
    assert_eq!(
        shaped["agent_surface"]["aliases_removed"],
        json!(["related_memories"]),
        "only members the envelope actually carried are reported: {shaped}"
    );
    assert_eq!(shaped["name"], json!("seed"));
}

#[test]
fn a_declared_alias_name_holding_a_scalar_is_left_alone() {
    let value = json!({ "results": [{ "n": 1 }], "related_memories": 7 });
    let mut s = surface_for("related");
    s.max_items = 1;
    let shaped = apply(&s, value);
    assert_eq!(shaped["related_memories"], json!(7));
    assert!(shaped["agent_surface"].get("aliases_removed").is_none());
}

#[test]
fn graph_alias_is_dropped_and_nodes_is_the_reshaped_array() {
    let nodes = json!([{ "name": "a" }, { "name": "b" }]);
    let value = json!({ "nodes": nodes, "entities": nodes, "edges": [], "elapsed_ms": 1 });
    let mut s = surface_for("graph");
    s.max_items = 1;
    let shaped = apply(&s, value);

    assert_eq!(shaped["nodes"].as_array().unwrap().len(), 1);
    assert!(shaped.get("entities").is_none());
    assert_eq!(
        shaped["agent_surface"]["aliases_removed"],
        json!(["entities"])
    );
}

/// GAP-SG-142 regression: `graph_matches` is derived in `recall` and disjoint in
/// `hybrid-search`, so suppression may only fire for the subcommand that
/// declared it.
///
/// `hybrid-search` builds `graph_matches` in `graph_expansion.rs`, which skips
/// every id already present in `results`; the two sets share no element and do
/// not even share a type (`HybridSearchItem` against `RecallItem`). Deleting it
/// therefore destroys data no other member restates, and
/// `docs/schemas/hybrid-search.schema.json` lists `graph_matches` under
/// `required`, so the deletion produced an envelope invalid against this
/// project's own schema.
#[test]
fn hybrid_search_graph_matches_survive_because_they_are_not_a_derived_alias() {
    // Disjoint by construction, exactly as the command emits it.
    let value = json!({
        "query": "auth",
        "k": 2,
        "results": [
            { "name": "a", "combined_score": 0.9, "vec_rank": 1, "fts_rank": 1 },
            { "name": "b", "combined_score": 0.5, "vec_rank": 2, "fts_rank": 3 }
        ],
        "graph_matches": [{ "name": "c", "distance": 0.4, "source": "graph" }],
        "elapsed_ms": 7
    });
    let mut s = surface_for("hybrid-search");
    s.max_items = 1;
    let shaped = apply(&s, value);

    assert_eq!(shaped["results"].as_array().unwrap().len(), 1);
    let graph = shaped
        .get("graph_matches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("graph_matches is required by the schema: {shaped}"));
    assert_eq!(graph.len(), 1, "the disjoint set must survive untouched");
    assert_eq!(graph[0]["name"], json!("c"));
    assert!(
        shaped["agent_surface"].get("aliases_removed").is_none(),
        "nothing was derived here, so nothing may be reported as removed: {shaped}"
    );
}

/// The paired half of the regression: the same member name, the same knob, and
/// the opposite outcome, because in `recall` `results` really is the
/// concatenation of `direct_matches` and `graph_matches`.
#[test]
fn recall_still_suppresses_the_same_member_name_that_hybrid_search_keeps() {
    let value = json!({
        "query": "auth",
        "direct_matches": [{ "n": 1 }, { "n": 2 }],
        "graph_matches": [{ "n": 3 }],
        "results": [{ "n": 1 }, { "n": 2 }, { "n": 3 }]
    });
    let mut s = surface_for("recall");
    s.max_items = 1;
    let shaped = apply(&s, value);

    assert!(shaped.get("graph_matches").is_none());
    assert!(shaped.get("direct_matches").is_none());
    assert_eq!(
        shaped["agent_surface"]["aliases_removed"],
        json!(["direct_matches", "graph_matches"])
    );
}

/// An envelope whose subcommand the surface could not resolve keeps every
/// member: suppression is opt-in per subcommand, never a guess from the shape.
#[test]
fn an_unknown_subcommand_suppresses_nothing() {
    let value = json!({
        "results": [{ "n": 1 }, { "n": 2 }],
        "related_memories": [{ "n": 1 }, { "n": 2 }]
    });
    let mut s = surface();
    s.max_items = 1;
    let shaped = apply(&s, value);

    assert!(s.command.is_none());
    assert!(shaped.get("related_memories").is_some());
    assert!(shaped["agent_surface"].get("aliases_removed").is_none());
}
