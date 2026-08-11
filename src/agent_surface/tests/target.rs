//! GAP-SG-205: the resolved target reaches EVERY envelope, knob or no knob.
//!
//! The defect these pin is not "the field is wrong". It is "the field is absent
//! on the only path that matters". v1.2.6 attached the record inside
//! `base_meta`, downstream of two short-circuits, so it appeared for
//! `remember --db T --max-items 50` and vanished for `remember --db T`.
//!
//! Every test here supplies its own target rather than reading the process-wide
//! cell, so none of them depends on whether a `paths` test ran first in the same
//! binary.

use super::*;
use crate::agent_surface::target::{
    DISPENSATION_KEY, DISPENSATION_VALUE, RESOLVED_KEY, SOURCE_KEY,
};
use crate::paths::TargetSource;

/// Builds the record a process with this target would produce.
fn target_record(source: TargetSource, path: &str, use_active: bool) -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert(SOURCE_KEY.into(), json!(source.as_str()));
    meta.insert(RESOLVED_KEY.into(), json!(path));
    if use_active && source != TargetSource::Argv {
        meta.insert(DISPENSATION_KEY.into(), json!(DISPENSATION_VALUE));
    }
    meta
}

/// The `agent_surface` block, or a panic naming what was emitted instead.
fn block(value: &Value) -> &Map<String, Value> {
    value
        .get("agent_surface")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("the envelope must carry an agent_surface block: {value}"))
}

/// The defect itself: no knob is set, and the target must still be reported.
///
/// This is the shape every agent produces. `remember --db T` sets nothing else,
/// so a contract that only survives beside an unrelated flag does not exist.
#[test]
fn an_inert_surface_still_reports_the_resolved_target() {
    let s = surface();
    assert!(s.is_noop(), "the premise is a surface with no knob at all");
    let shaped = super::super::apply_with_target(
        &s,
        json!({ "operation": "created", "name": "alpha" }),
        Some(target_record(TargetSource::Argv, "/tmp/t.sqlite", false)),
    )
    .expect("an inert surface never refuses");

    assert_eq!(block(&shaped)[SOURCE_KEY], json!("argv"));
    assert_eq!(block(&shaped)[RESOLVED_KEY], json!("/tmp/t.sqlite"));
}

/// The inert path annotates and NOTHING else: no reshaping sneaks in with it.
#[test]
fn the_inert_path_adds_the_record_and_changes_nothing_else() {
    let original = json!({ "operation": "created", "name": "alpha", "version": 1 });
    let shaped = super::super::apply_with_target(
        &surface(),
        original.clone(),
        Some(target_record(TargetSource::Xdg, "/x.sqlite", false)),
    )
    .expect("an inert surface never refuses");

    for (key, value) in original.as_object().expect("object fixture") {
        assert_eq!(
            shaped.get(key),
            Some(value),
            "the inert path must not touch '{key}'"
        );
    }
}

/// A shaped envelope reports the same target as an inert one.
///
/// The two paths reach the record through different code; drift between them
/// would mean the same process describing its target two ways depending on which
/// flags the caller happened to pass.
#[test]
fn the_shaped_path_reports_the_same_target_as_the_inert_one() {
    let record = target_record(TargetSource::Argv, "/tmp/t.sqlite", false);
    let mut shaping = surface();
    shaping.max_items = 2;

    let inert = super::super::apply_with_target(&surface(), envelope(), Some(record.clone()))
        .expect("inert");
    let shaped =
        super::super::apply_with_target(&shaping, envelope(), Some(record)).expect("shaped");

    assert_eq!(block(&inert)[SOURCE_KEY], block(&shaped)[SOURCE_KEY]);
    assert_eq!(block(&inert)[RESOLVED_KEY], block(&shaped)[RESOLVED_KEY]);
}

/// Each layer reports itself, because only `argv` is an explicit designation.
#[test]
fn every_layer_names_itself() {
    for (source, wire) in [
        (TargetSource::Argv, "argv"),
        (TargetSource::Xdg, "xdg"),
        (TargetSource::Default, "default"),
    ] {
        let shaped = super::super::apply_with_target(
            &surface(),
            json!({ "ok": true }),
            Some(target_record(source, "/db", false)),
        )
        .expect("inert");
        assert_eq!(block(&shaped)[SOURCE_KEY], json!(wire), "layer {wire}");
    }
}

/// A process that resolved no database reports nothing, and stays byte-identical.
///
/// Absence must mean "touched no database" — `config`, `completions`, `locale` —
/// and never "resolved one but did not say so". Collapsing those two readings is
/// the whole defect.
#[test]
fn a_process_with_no_target_emits_no_block_at_all() {
    let original = json!({ "keys": ["db.path"] });
    let shaped =
        super::super::apply_with_target(&surface(), original.clone(), None).expect("inert");
    assert_eq!(
        shaped, original,
        "with no target and no knob the envelope must be untouched"
    );
}

/// A schema document is a contract, so it is never annotated.
#[test]
fn a_schema_document_is_never_annotated_with_a_target() {
    let doc = json!({ "$schema": "https://json-schema.org/draft/2020-12/schema" });
    let shaped = super::super::apply_with_target(
        &surface(),
        doc.clone(),
        Some(target_record(TargetSource::Argv, "/db", false)),
    )
    .expect("passthrough");
    assert_eq!(shaped, doc);
}

/// The dispensation is recorded only where it changed the outcome.
///
/// On an `argv` target `--use-active` was never consulted, so reporting it would
/// suggest the caller leaned on an escape hatch it did not need.
#[test]
fn the_dispensation_is_recorded_only_when_it_mattered() {
    let inherited = super::super::apply_with_target(
        &surface(),
        json!({ "ok": true }),
        Some(target_record(TargetSource::Xdg, "/db", true)),
    )
    .expect("inert");
    assert_eq!(
        block(&inherited)[DISPENSATION_KEY],
        json!(DISPENSATION_VALUE)
    );

    let designated = super::super::apply_with_target(
        &surface(),
        json!({ "ok": true }),
        Some(target_record(TargetSource::Argv, "/db", true)),
    )
    .expect("inert");
    assert!(
        !block(&designated).contains_key(DISPENSATION_KEY),
        "an argv target never consulted the dispensation"
    );
}

/// The displaced token: XDG points at X while the argv names Y.
///
/// The rule's own worked example. Y must win and the record must say `argv`,
/// because a resolver that preferred the ambient layer would write to the
/// database the command line did not name while reporting success.
#[test]
fn the_argv_outranks_the_ambient_layer_and_says_so() {
    let record = target_record(TargetSource::Argv, "/tmp/Y.sqlite", false);
    let shaped = super::super::apply_with_target(
        &surface(),
        json!({ "operation": "created" }),
        Some(record),
    )
    .expect("inert");

    assert_eq!(block(&shaped)[RESOLVED_KEY], json!("/tmp/Y.sqlite"));
    assert_eq!(
        block(&shaped)[SOURCE_KEY],
        json!("argv"),
        "the layer that won must be the one reported, or the record is decorative"
    );
}

/// A mutating surface is annotated, never refused — the fence still holds.
#[test]
fn a_mutating_surface_is_annotated_rather_than_refused() {
    let mut s = surface();
    s.mutates = true;
    s.select = vec!["a_key_nothing_carries".to_string()];

    let shaped = super::super::apply_with_target(
        &s,
        json!({ "operation": "created", "name": "alpha" }),
        Some(target_record(TargetSource::Argv, "/db", false)),
    )
    .expect("a write is never refused at output time");

    assert_eq!(block(&shaped)[SOURCE_KEY], json!("argv"));
}
