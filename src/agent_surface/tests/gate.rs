//! The refusals: GAP-SG-202, GAP-SG-203 and GAP-SG-204.
//!
//! Each of these used to be answered with an empty set and `exit 0`. The
//! assertions below pin the exit code as much as the shape, because the exit
//! code is what an agent branches on.

use super::*;

/// An envelope with a top-level scalar AND an array, which is the shape that
/// made GAP-SG-203 possible: `health` carries `integrity_ok` beside `checks`.
fn scalar_beside_array() -> Value {
    json!({
        "integrity_ok": true,
        "schema_version": 16,
        "checks": [
            { "id": "fts", "ok": true },
            { "id": "vec", "ok": true }
        ]
    })
}

/// An envelope with no array at all, which is what `stats` and `read` emit.
fn scalar_only() -> Value {
    json!({ "total_memories": 1892, "elapsed_ms": 3 })
}

fn refusal(surface: &AgentSurface, value: Value) -> String {
    let err = try_apply(surface, value).expect_err("this request must be refused");
    assert_eq!(err.exit_code(), 2, "usage errors exit 2: {err}");
    err.to_string()
}

#[test]
fn a_filter_key_no_element_carries_is_refused() {
    let mut s = surface();
    s.filters = vec![FilterExpr::parse("chave_errada=x").unwrap()];
    let message = refusal(&s, envelope());
    assert!(message.contains("chave_errada"), "{message}");
    assert!(message.contains("--filter"), "{message}");
}

/// GAP-SG-203: the predicate names a member of the envelope, so applying it
/// would empty a collection the caller never named while the named scalar
/// survived beside the result, contradicting the predicate.
#[test]
fn a_predicate_aimed_at_an_envelope_member_is_refused_and_names_the_array() {
    let mut s = surface();
    s.filters = vec![FilterExpr::parse("integrity_ok=false").unwrap()];
    let message = refusal(&s, scalar_beside_array());
    assert!(message.contains("integrity_ok"), "{message}");
    assert!(
        message.contains("checks"),
        "the refusal must name the collection the predicate would have hit: {message}"
    );
}

/// The same envelope, filtered on a field the ELEMENTS carry, still works.
/// Without this the previous test could pass by refusing everything.
#[test]
fn a_predicate_on_a_real_element_field_still_shapes_the_same_envelope() {
    let mut s = surface();
    s.filters = vec![FilterExpr::parse("id=fts").unwrap()];
    let shaped = apply(&s, scalar_beside_array());
    assert_eq!(shaped["checks"].as_array().unwrap().len(), 1);
    assert_eq!(shaped["integrity_ok"], json!(true));
}

/// GAP-SG-204: a knob with nothing to act on was accepted and ignored.
#[test]
fn a_knob_with_no_result_array_to_act_on_is_refused() {
    for (label, mut s) in [
        ("--filter", surface()),
        ("--sort", surface()),
        ("--dedupe-by", surface()),
    ] {
        match label {
            "--filter" => s.filters = vec![FilterExpr::parse("total_memories=1").unwrap()],
            "--sort" => s.sort = Some("total_memories".into()),
            _ => s.dedupe_by = Some("total_memories".into()),
        }
        let message = refusal(&s, scalar_only());
        assert!(message.contains(label), "{label} must be named: {message}");
    }
}

/// `--select` is deliberately absent from the inert-knob rule: on an envelope
/// with no result array it projects the envelope itself, which is a real effect.
#[test]
fn select_on_a_scalar_envelope_is_not_an_inert_knob() {
    let mut s = surface();
    s.select = vec!["total_memories".into()];
    let shaped = apply(&s, scalar_only());
    assert_eq!(shaped["total_memories"], json!(1892));
    assert!(shaped.get("elapsed_ms").is_none());
}

#[test]
fn a_projection_that_resolves_nothing_is_refused_and_suggests_a_near_miss() {
    let mut s = surface();
    s.select = vec!["nmae".into()];
    let message = refusal(&s, envelope());
    assert!(
        message.contains("name"),
        "the vocabulary had `name` one edit away: {message}"
    );
}

/// Partial resolution stays successful: an agent projecting six fields over a
/// heterogeneous result set still gets a useful answer, and learns which field
/// was missing instead of reading the gap as missing data.
#[test]
fn a_partly_resolvable_projection_succeeds_and_reports_what_it_dropped() {
    let mut s = surface();
    s.select = vec!["name".into(), "chave_errada".into()];
    let shaped = apply(&s, envelope());

    assert_eq!(shaped["agent_surface"]["key_resolution"], json!("partial"));
    assert_eq!(
        shaped["agent_surface"]["unresolved_keys"],
        json!(["chave_errada"])
    );
    assert_eq!(shaped["agent_surface"]["resolved_keys"], json!(["name"]));
    for item in results(&shaped) {
        assert!(item.as_object().unwrap().contains_key("name"));
    }
}

/// A projection where everything resolves reports nothing, so the record stays
/// quiet on the common case.
#[test]
fn a_fully_resolved_projection_reports_no_resolution_block() {
    let mut s = surface();
    s.select = vec!["name".into()];
    let shaped = apply(&s, envelope());
    assert!(shaped["agent_surface"].get("key_resolution").is_none());
    assert!(shaped["agent_surface"].get("unresolved_keys").is_none());
}

/// Absence of evidence is not evidence of absence.
///
/// `related --select name` over a seed with no neighbours returns an EMPTY
/// results array. Resolving against zero elements makes every key unresolvable,
/// so the gate refused and its own message suggested the key it had just
/// rejected — inverting this gate into the false negative it exists to remove.
#[test]
fn an_empty_result_array_is_never_grounds_for_refusal() {
    let empty = json!({ "name": "seed", "hops": 1, "results": [] });

    let mut s = surface();
    s.select = vec!["name".into()];
    assert!(
        try_apply(&s, empty.clone()).is_ok(),
        "projection over zero rows"
    );

    let mut s = surface();
    s.filters = vec![FilterExpr::parse("anything=x").unwrap()];
    assert!(
        try_apply(&s, empty.clone()).is_ok(),
        "predicate over zero rows"
    );

    let mut s = surface();
    s.sort = Some("anything".into());
    assert!(try_apply(&s, empty).is_ok(), "sort over zero rows");
}

/// The exclusion is scoped to an empty ARRAY. A scalar envelope has no elements
/// by shape and resolves against the envelope itself, which is real evidence.
#[test]
fn a_scalar_envelope_is_still_resolved_despite_having_no_elements() {
    let mut s = surface();
    s.select = vec!["nao_existe_em_lugar_nenhum".into()];
    let err = try_apply(&s, scalar_only()).expect_err("a scalar envelope still resolves");
    assert_eq!(err.exit_code(), 2, "{err}");
}

#[test]
fn allow_unknown_keys_restores_the_previous_behaviour() {
    let mut s = surface();
    s.allow_unknown_keys = true;
    s.filters = vec![FilterExpr::parse("chave_errada=x").unwrap()];
    let shaped = apply(&s, envelope());
    assert_eq!(
        results(&shaped).len(),
        0,
        "the escape accepts the empty answer instead of refusing it"
    );
}

/// The fence. `apply` runs after the handler already did its work, so refusing
/// for a command that changed durable state would report failure for an
/// operation that succeeded — and a retried `remember` writes twice.
#[test]
fn a_mutating_command_is_never_refused() {
    let mut s = surface();
    s.mutates = true;
    s.filters = vec![FilterExpr::parse("chave_errada=x").unwrap()];
    s.select = vec!["tambem_errada".into()];

    let shaped = try_apply(&s, envelope()).expect("a mutating command is annotated, never refused");
    assert!(shaped.get("agent_surface").is_some());
}

/// Error envelopes reach the caller before the gate can ever look at them, so a
/// failure is never converted into a different failure.
#[test]
fn the_gate_never_sees_a_failure_envelope() {
    let mut s = surface();
    s.filters = vec![FilterExpr::parse("chave_errada=x").unwrap()];
    let failure = json!({ "error": true, "code": 4, "message": "not found" });
    assert_eq!(
        try_apply(&s, failure.clone()).expect("failures pass through"),
        failure
    );
}
