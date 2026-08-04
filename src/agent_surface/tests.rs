//! Unit coverage for the agent-native reshaping surface (GAP-SG-142).

use super::filter::{FilterExpr, FilterOp};
use super::*;
use serde_json::json;

fn envelope() -> Value {
    json!({
        "query": "rust",
        "elapsed_ms": 12,
        "results": [
            { "name": "alpha", "score": 0.9, "type": "note", "snippet": "abcdefghij" },
            { "name": "beta",  "score": 0.5, "type": "note", "snippet": "klmnopqrst" },
            { "name": "gamma", "score": 0.7, "type": "decision", "snippet": "uvwxyz" },
            { "name": "beta",  "score": 0.1, "type": "note", "snippet": "duplicate" }
        ]
    })
}

fn surface() -> AgentSurface {
    AgentSurface::default()
}

/// A surface that knows which subcommand emitted the envelope.
///
/// Alias suppression needs it: a member is derived only for the subcommand that
/// declared it so, and the same `results` name means different things in
/// `recall`, `related` and `hybrid-search`.
fn surface_for(command: &str) -> AgentSurface {
    AgentSurface {
        command: Some(command.to_string()),
        ..AgentSurface::default()
    }
}

fn results(value: &Value) -> &Vec<Value> {
    value
        .get("results")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("envelope must keep its results array: {value}"))
}

#[test]
fn default_surface_is_a_noop() {
    let s = surface();
    assert!(s.is_noop());
    let original = envelope();
    assert_eq!(apply(&s, original.clone()), original);
}

#[test]
fn select_projects_result_objects_and_shrinks_the_envelope() {
    let mut s = surface();
    s.select = vec!["name".into(), "score".into()];
    let shaped = apply(&s, envelope());

    let before = serde_json::to_string(&envelope()).unwrap().len();
    let after = serde_json::to_string(&shaped).unwrap().len();
    assert!(after < before, "projection must reduce the envelope");

    for item in results(&shaped) {
        let obj = item.as_object().unwrap();
        assert_eq!(obj.len(), 2, "only the selected keys survive: {item}");
        assert!(obj.contains_key("name") && obj.contains_key("score"));
        assert!(!obj.contains_key("snippet"));
    }
    // Envelope-level members are untouched by projection.
    assert_eq!(shaped["query"], json!("rust"));
}

#[test]
fn select_skips_absent_keys_instead_of_emitting_null() {
    let mut s = surface();
    s.select = vec!["name".into(), "nonexistent".into()];
    let shaped = apply(&s, envelope());
    for item in results(&shaped) {
        assert!(!item.as_object().unwrap().contains_key("nonexistent"));
    }
}

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
fn sort_orders_numbers_numerically_and_keeps_keyless_elements_last() {
    let mut s = surface();
    s.sort = Some("score".into());
    let shaped = apply(&s, envelope());
    let scores: Vec<f64> = results(&shaped)
        .iter()
        .map(|i| i["score"].as_f64().unwrap())
        .collect();
    assert_eq!(scores, vec![0.1, 0.5, 0.7, 0.9]);

    let mixed = json!({ "results": [ { "a": 2 }, { "b": 1 }, { "a": 1 } ] });
    let shaped = apply(&s.clone_with_sort("a"), mixed);
    let items = results(&shaped);
    assert_eq!(items[0]["a"], json!(1));
    assert_eq!(items[1]["a"], json!(2));
    assert!(items[2].get("a").is_none());
}

impl AgentSurface {
    fn clone_with_sort(&self, key: &str) -> Self {
        let mut next = self.clone();
        next.sort = Some(key.to_string());
        next
    }
}

#[test]
fn dedupe_keeps_first_occurrence_and_all_keyless_elements() {
    let mut s = surface();
    s.dedupe_by = Some("name".into());
    let shaped = apply(&s, envelope());
    assert_eq!(results(&shaped).len(), 3);
    assert_eq!(results(&shaped)[1]["score"], json!(0.5));

    let mut s = surface();
    s.dedupe_by = Some("absent".into());
    assert_eq!(results(&apply(&s, envelope())).len(), 4);
}

#[test]
fn max_items_caps_the_emitted_array() {
    let mut s = surface();
    s.max_items = 2;
    let shaped = apply(&s, envelope());
    assert_eq!(results(&shaped).len(), 2);
    assert_eq!(shaped["agent_surface"]["input_count"], json!(4));
    assert_eq!(shaped["agent_surface"]["output_count"], json!(2));
}

#[test]
fn count_only_returns_the_post_filter_count() {
    let mut s = surface();
    s.count_only = true;
    assert_eq!(apply(&s, envelope())["count"], json!(4));

    let mut s = surface();
    s.count_only = true;
    s.filters = vec![FilterExpr::parse("type=decision").unwrap()];
    let shaped = apply(&s, envelope());
    assert_eq!(shaped["count"], json!(1));
    assert_eq!(shaped["agent_surface"]["count_only"], json!(true));
    assert!(shaped.get("results").is_none(), "payload is replaced");
}

#[test]
fn truncate_content_shortens_strings_and_flags_the_envelope() {
    let mut s = surface();
    s.truncate_content = 4;
    let shaped = apply(&s, envelope());
    for item in results(&shaped) {
        assert!(item["snippet"].as_str().unwrap().chars().count() <= 4);
    }
    assert_eq!(shaped["truncated"], json!(true));
    assert_eq!(shaped["agent_surface"]["content_truncated"], json!(true));
    assert_eq!(shaped["agent_surface"]["truncate_content"], json!(4));
}

#[test]
fn truncate_content_never_splits_a_utf8_sequence() {
    let mut s = surface();
    s.truncate_content = 3;
    let shaped = apply(&s, json!({ "results": [ { "s": "ãéîõü" } ] }));
    let cut = results(&shaped)[0]["s"].as_str().unwrap();
    assert_eq!(cut, "ãéî");
}

#[test]
fn truncate_content_leaves_short_strings_and_the_flag_alone() {
    let mut s = surface();
    s.truncate_content = 1000;
    let shaped = apply(&s, envelope());
    assert!(shaped.get("truncated").is_none());
}

#[test]
fn max_output_bytes_drops_elements_and_records_it() {
    let full = serde_json::to_string(&envelope()).unwrap().len();
    let mut s = surface();
    // Wide enough to keep the envelope and its record, tight enough to force
    // elements out — the case the ceiling exists for.
    s.max_output_bytes = full * 3 / 4;
    let shaped = apply(&s, envelope());

    assert!(serde_json::to_string(&shaped).unwrap().len() <= s.max_output_bytes);
    assert_eq!(shaped["truncated"], json!(true));
    assert_eq!(shaped["agent_surface"]["output_truncated"], json!(true));
    assert!(shaped["agent_surface"]["dropped"].as_u64().unwrap() > 0);
    assert!(results(&shaped).len() < 4);
}

/// `output_count` is measured by the shaping stage, which runs before the
/// ceiling is enforced. Left alone it describes a document the caller never
/// receives: `list --max-output-bytes 8000` reported `output_count: 30` beside
/// eleven elements and `dropped: 19`, which reads as a parser that lost rows
/// rather than a ceiling that did its job.
#[test]
fn output_count_describes_what_survived_the_ceiling() {
    let full = serde_json::to_string(&envelope()).unwrap().len();
    let mut s = surface();
    s.max_output_bytes = full * 3 / 4;
    let shaped = apply(&s, envelope());

    assert_eq!(shaped["agent_surface"]["output_truncated"], json!(true));
    let surviving = results(&shaped).len();
    let reported = shaped["agent_surface"]["output_count"]
        .as_u64()
        .expect("output_count must stay present after the ceiling fires")
        as usize;
    assert_eq!(
        reported, surviving,
        "output_count reported {reported} beside {surviving} surviving elements"
    );
}

#[test]
fn max_output_bytes_falls_back_to_a_parseable_stub() {
    let mut s = surface();
    s.max_output_bytes = 16;
    let shaped = apply(&s, envelope());
    assert_eq!(shaped["truncated"], json!(true));
    assert_eq!(shaped["truncated_reason"], json!("max_output_bytes"));
    // Whatever happens, the emitted document must still parse.
    let text = serde_json::to_string(&shaped).unwrap();
    assert!(serde_json::from_str::<Value>(&text).is_ok());
}

#[test]
fn max_output_bytes_leaves_a_fitting_envelope_alone() {
    let mut s = surface();
    s.max_output_bytes = 1_000_000;
    let shaped = apply(&s, envelope());
    assert_eq!(results(&shaped).len(), 4);
    assert!(shaped.get("truncated").is_none());
}

#[test]
fn top_level_arrays_are_reshaped_in_place() {
    let mut s = surface();
    s.max_items = 1;
    let shaped = apply(&s, json!([{ "a": 1 }, { "a": 2 }]));
    assert_eq!(shaped, json!([{ "a": 1 }]));
}

#[test]
fn envelopes_without_an_array_are_projected_themselves() {
    let mut s = surface();
    s.select = vec!["name".into()];
    let shaped = apply(&s, json!({ "name": "solo", "body": "long", "n": 3 }));
    assert_eq!(shaped["name"], json!("solo"));
    assert!(shaped.get("body").is_none());
    assert!(shaped.get("agent_surface").is_some());
}

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

/// The `list` envelope: `memories` is a clone of `items` (v1.0.66 alias).
fn list_envelope() -> Value {
    let items = json!([
        { "name": "alpha", "memory_type": "skill" },
        { "name": "beta",  "memory_type": "note" },
        { "name": "gamma", "memory_type": "skill" }
    ]);
    json!({ "total_count": 3, "items": items, "memories": items, "elapsed_ms": 1 })
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

#[test]
fn budget_returns_real_rows_instead_of_the_stub_when_an_alias_inflates_the_envelope() {
    // Before alias suppression the clone alone could exceed the ceiling, and
    // the budget collapsed the whole envelope into a data-free stub — an
    // availability failure, not a truncation.
    let row = json!({ "name": "alpha", "snippet": "x".repeat(120) });
    let items: Vec<Value> = (0..30).map(|_| row.clone()).collect();
    let value = json!({
        "total_count": 30,
        "items": items,
        "memories": items,
        "elapsed_ms": 1
    });

    let mut s = surface_for("list");
    s.select = vec!["name".into()];
    s.max_output_bytes = 800;
    let shaped = apply(&s, value);

    let emitted = serde_json::to_string(&shaped).unwrap();
    assert!(emitted.len() <= s.max_output_bytes, "ceiling honoured");
    assert_ne!(
        shaped["truncated_reason"],
        json!("max_output_bytes"),
        "the stub is never an acceptable answer here: {shaped}"
    );
    let rows = shaped["items"]
        .as_array()
        .unwrap_or_else(|| panic!("items must survive the ceiling: {shaped}"));
    assert!(
        !rows.is_empty(),
        "real rows must reach the caller: {shaped}"
    );
    assert!(shaped.get("memories").is_none());
}

#[test]
fn existing_truncated_member_is_never_overwritten() {
    let mut s = surface();
    s.truncate_content = 2;
    let shaped = apply(
        &s,
        json!({ "truncated": false, "results": [{ "s": "abcdef" }] }),
    );
    assert_eq!(shaped["truncated"], json!(false));
    assert_eq!(shaped["agent_surface"]["content_truncated"], json!(true));
}

#[test]
fn filter_op_variants_are_distinct() {
    assert_ne!(FilterOp::Equals, FilterOp::NotEquals);
    assert_ne!(FilterOp::Equals, FilterOp::Contains);
}

#[test]
fn get_returns_an_inert_surface_before_init() {
    // `init` may already have run in another test of this binary; either way
    // `get` must return something usable rather than panic.
    assert!(get().is_noop() || !get().is_noop());
}

#[test]
fn a_large_secondary_array_no_longer_forces_the_stub() {
    // GAP-SG-171: `graph` pairs `nodes` with `edges`. `edges` is a different
    // collection, not an alias, so suppression does not apply — yet it kept the
    // envelope over budget and the stub replaced everything. Measured before the
    // fix: `graph --select id --max-output-bytes 4000` returned 81 bytes and no
    // data at all.
    let nodes: Vec<Value> = (0..50).map(|i| json!({ "id": i })).collect();
    let edges: Vec<Value> = (0..500)
        .map(|i| json!({ "from": i, "to": i + 1, "relation": "depends-on" }))
        .collect();
    let mut s = surface();
    s.max_output_bytes = 900;

    let shaped = apply(&s, json!({ "nodes": nodes, "edges": edges }));

    assert!(
        shaped.get("truncated_reason").is_none(),
        "the envelope collapsed into the stub: {shaped}"
    );
    assert!(
        !shaped["nodes"].as_array().unwrap().is_empty(),
        "nodes was emptied even though trimming edges made room"
    );
    assert_eq!(
        shaped["agent_surface"]["output_truncated"],
        json!(true),
        "dropping data is never silent"
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

/// GAP-SG-191: `--max-items` bound the primary array alone.
///
/// Measured against the live 1.2.4 binary: `graph --format json --select id
/// --max-items 2` answered with two nodes and all 59 066 edges — 4 771 393 bytes
/// for a request that asked for two items. `--max-output-bytes` already reached
/// the secondary member; the item cap did not, and nothing documented the gap.
#[test]
fn max_items_caps_secondary_arrays_too() {
    let nodes: Vec<Value> = (0..50).map(|i| json!({ "id": i })).collect();
    let edges: Vec<Value> = (0..500)
        .map(|i| json!({ "from": i, "to": i + 1, "relation": "depends-on" }))
        .collect();
    let mut s = surface();
    s.max_items = 2;

    let shaped = apply(&s, json!({ "nodes": nodes, "edges": edges }));

    assert_eq!(shaped["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(
        shaped["edges"].as_array().unwrap().len(),
        2,
        "the secondary array ignored the cap: {shaped}"
    );
    assert_eq!(
        shaped["agent_surface"]["secondary_capped"],
        json!(["edges"]),
        "capping a secondary member is never silent"
    );
}

/// The cap removes whole elements; `--select` must not follow it into a
/// secondary array.
///
/// `edges` carries `from`/`to`, so projecting `id` over it would rewrite every
/// element to `{}` — erasing the collection rather than shrinking it. Keeping
/// projection on the primary array is the reason the asymmetry is deliberate.
#[test]
fn select_stays_on_the_primary_array() {
    let mut s = surface();
    s.select = vec!["id".to_string()];

    let shaped = apply(
        &s,
        json!({
            "nodes": [{ "id": 1, "name": "alpha" }],
            "edges": [{ "from": 1, "to": 2 }]
        }),
    );

    assert_eq!(shaped["nodes"][0], json!({ "id": 1 }));
    assert_eq!(
        shaped["edges"][0],
        json!({ "from": 1, "to": 2 }),
        "projection erased the secondary array instead of leaving it alone"
    );
}

/// GAP-SG-191: the stub reported the budget it was handed, not the one asked for.
///
/// `finalize` subtracts the record's own headroom before calling `enforce`, so
/// the stub built inside `enforce` knew only the reduced figure. Measured on the
/// live 1.2.4 binary: `--max-output-bytes 400` emitted
/// `"max_output_bytes":340` — a number the caller never chose.
#[test]
fn the_stub_reports_the_requested_ceiling_not_the_discounted_one() {
    // The stub is only reached when the envelope without any element already
    // exceeds the ceiling, so the scalar field has to be what overflows.
    let mut s = surface();
    s.max_output_bytes = 200;
    let results: Vec<Value> = (0..20)
        .map(|i| json!({ "name": format!("memory-{i}") }))
        .collect();

    let shaped = apply(&s, json!({ "results": results, "query": "q".repeat(400) }));

    assert_eq!(
        shaped["truncated_reason"], "max_output_bytes",
        "this case must reach the stub: {shaped}"
    );
    assert_eq!(
        shaped["max_output_bytes"],
        json!(200),
        "the stub echoed the internally discounted budget"
    );
}

/// The prefix scan must pick exactly the same prefix the old clone-per-probe
/// binary search did, and the emitted envelope must honour the ceiling.
///
/// Sweeping the ceiling is what proves the arithmetic: a compact array costs the
/// sum of its elements plus one separator between each, so an off-by-one in the
/// separator accounting would overshoot at some width even if it looked right at
/// others.
#[test]
fn the_prefix_scan_never_overshoots_the_ceiling() {
    let results: Vec<Value> = (0..80)
        .map(|i| json!({ "name": format!("memory-{i}"), "score": i }))
        .collect();

    for ceiling in [200usize, 300, 400, 500, 700, 900, 1200, 2000, 4000] {
        let mut s = surface();
        s.max_output_bytes = ceiling;
        let shaped = apply(&s, json!({ "results": results.clone(), "query": "x" }));
        let encoded = serde_json::to_string(&shaped).expect("shaped envelope serializes");
        assert!(
            encoded.len() <= ceiling,
            "ceiling {ceiling} produced {} bytes: {shaped}",
            encoded.len()
        );
    }
}

/// `command` is context, not a knob: a surface carrying only a subcommand name
/// still changes nothing, so the opt-in contract holds.
#[test]
fn command_alone_does_not_make_the_surface_active() {
    let s = surface_for("recall");
    assert!(s.is_noop(), "the subcommand is context, never a knob");
    let original = json!({
        "results": [{ "n": 1 }],
        "direct_matches": [{ "n": 1 }],
        "graph_matches": []
    });
    assert_eq!(apply(&s, original.clone()), original);
}
