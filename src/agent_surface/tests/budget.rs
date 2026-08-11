//! `--max-output-bytes`: the byte ceiling, its record, and its parseable stub.

use super::*;

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
