//! `--sort`, `--dedupe-by`, `--max-items` and `--count-only`: the knobs that
//! decide which elements are emitted and in what order.

use super::*;

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

#[test]
fn dedupe_keeps_first_occurrence_and_all_keyless_elements() {
    let mut s = surface();
    s.dedupe_by = Some("name".into());
    let shaped = apply(&s, envelope());
    assert_eq!(results(&shaped).len(), 3);
    assert_eq!(results(&shaped)[1]["score"], json!(0.5));

    // An element that LACKS the key was never proven duplicate, so it survives.
    // The key itself still has to exist somewhere, or the request is a typo.
    let mixed = json!({ "results": [ { "k": "a" }, { "k": "a" }, { "other": 1 } ] });
    let mut s = surface();
    s.dedupe_by = Some("k".into());
    assert_eq!(results(&apply(&s, mixed)).len(), 2);
}

/// GAP-SG-202: a dedup key nothing carries used to keep every row and exit 0.
///
/// That reads as "no duplicates found" when the truth is "that key does not
/// exist here", and the two demand opposite next moves from the caller.
#[test]
fn dedupe_by_a_key_nothing_carries_is_refused() {
    let mut s = surface();
    s.dedupe_by = Some("absent".into());
    let err = try_apply(&s, envelope()).expect_err("an unresolvable key must be refused");
    assert_eq!(err.exit_code(), 2, "usage errors exit 2: {err}");
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
