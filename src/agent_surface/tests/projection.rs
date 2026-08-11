//! `--select` projection, and the envelope shapes it has to cope with.

use super::*;

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
