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

/// GAP-SG-201: a bare count over a truncated page reads as the inventory.
///
/// Measured on the live corpus before the fix: `--count-only graph entities`
/// answered `{"count": 50}` with `exit 0` over a universe of 107 111, on a
/// command line that named no limit at all.
#[test]
fn a_count_over_a_truncated_page_is_refused() {
    let mut s = surface();
    s.count_only = true;
    let ceiling = pagination(50, 107_111);

    let err = try_apply_under(&s, envelope(), Some(&ceiling))
        .expect_err("a count over a page must be refused");
    assert_eq!(err.exit_code(), 2, "usage errors exit 2: {err}");
    let message = err.to_string();
    assert!(message.contains("50"), "{message}");
    assert!(message.contains("107111"), "{message}");
}

/// The escape the refusal itself advertises has to work, or the message lies.
#[test]
fn a_count_over_a_page_is_allowed_once_the_narrower_scope_is_declared() {
    let mut s = surface();
    s.count_only = true;
    s.filter_scope = Some(super::super::universe::FilterScope::Page);
    let ceiling = pagination(50, 107_111);

    let shaped =
        try_apply_under(&s, envelope(), Some(&ceiling)).expect("--filter-scope page is the escape");
    assert_eq!(shaped["count"], json!(4));
}

/// A top-k bound IS the answer, not a truncation of one, so it never refuses.
///
/// Without this the fix would break every `--count-only hybrid-search`, which
/// asks a question the ranking answers completely.
#[test]
fn a_count_under_a_top_k_bound_is_never_refused() {
    let mut s = surface();
    s.count_only = true;
    let ceiling = top_k(5);
    assert!(try_apply_under(&s, envelope(), Some(&ceiling)).is_ok());
}

/// A limit wider than the corpus cut nothing, so there is nothing to refuse.
///
/// This is what keeps `--count-only list` working on a corpus smaller than the
/// default page: measured at 7 060 of 7 060, which must stay `exit 0`.
#[test]
fn a_count_under_a_ceiling_that_cut_nothing_is_never_refused() {
    let mut s = surface();
    s.count_only = true;
    let ceiling = pagination(7_060, 7_060);
    assert!(try_apply_under(&s, envelope(), Some(&ceiling)).is_ok());
}

/// With no ceiling declared the surface knows nothing and claims nothing.
#[test]
fn a_count_with_no_declared_ceiling_is_never_refused() {
    let mut s = surface();
    s.count_only = true;
    assert!(try_apply_under(&s, envelope(), None).is_ok());
}

/// The sibling refusal, finally testable.
///
/// It shipped in v1.2.6 and had no test at all, for the same structural reason
/// the count refusal had none: the ceiling was read from a `OnceLock` inside the
/// decision. Asserting it here is what stops the shared escape logic from being
/// changed for one caller and silently broken for the other.
#[test]
fn a_predicate_over_a_truncated_page_is_refused() {
    let mut s = surface();
    s.filters = vec![FilterExpr::parse("type=note").unwrap()];
    let ceiling = pagination(50, 7_060);

    let err = try_apply_under(&s, envelope(), Some(&ceiling))
        .expect_err("a predicate over a page must be refused");
    assert_eq!(err.exit_code(), 2, "{err}");
    assert!(err.to_string().contains("--filter"), "{err}");
}

/// Both refusals honour the SAME escapes, which is the point of sharing them.
#[test]
fn both_refusals_share_every_escape() {
    let ceiling = pagination(50, 7_060);
    for (label, mut s) in [("--filter", surface()), ("--count-only", surface())] {
        match label {
            "--filter" => s.filters = vec![FilterExpr::parse("type=note").unwrap()],
            _ => s.count_only = true,
        }
        assert!(
            try_apply_under(&s, envelope(), Some(&ceiling)).is_err(),
            "{label} must refuse without an escape"
        );

        let mut escaped = s.clone();
        escaped.filter_scope = Some(super::super::universe::FilterScope::Page);
        assert!(
            try_apply_under(&escaped, envelope(), Some(&ceiling)).is_ok(),
            "{label} must accept --filter-scope page"
        );

        assert!(
            try_apply_under(&s, envelope(), Some(&top_k(5))).is_ok(),
            "{label} must accept a top-k bound"
        );
    }
}

/// The label has to name which of THREE sets was counted.
///
/// Measured before the fix, and this is the half that survived the refusal:
/// `--count-only --filter-scope page graph entities` answered `count_scope:
/// "matched"` over 50 of 107 111. The caller had declared the narrower scope, was
/// let through on purpose, and was then handed the strongest of the three labels.
#[test]
fn count_scope_names_the_page_when_the_query_cut_it() {
    let mut s = surface();
    s.count_only = true;
    s.filter_scope = Some(super::super::universe::FilterScope::Page);
    let ceiling = pagination(50, 107_111);

    let shaped = try_apply_under(&s, envelope(), Some(&ceiling)).expect("the escape is allowed");
    assert_eq!(shaped["agent_surface"]["count_scope"], json!("page"));
}

/// The query ceiling OUTRANKS the output ceiling, because it is upstream of it.
///
/// Reporting `emitted` here would name the smaller omission — the rows
/// `--max-items` dropped — and hide the larger one, the rows `LIMIT` never
/// returned.
#[test]
fn count_scope_prefers_the_query_ceiling_over_the_output_ceiling() {
    let mut s = surface();
    s.count_only = true;
    s.max_items = 2;
    s.filter_scope = Some(super::super::universe::FilterScope::Page);
    let ceiling = pagination(50, 107_111);

    let shaped = try_apply_under(&s, envelope(), Some(&ceiling)).expect("the escape is allowed");
    assert_eq!(shaped["agent_surface"]["count_scope"], json!("page"));
}

/// With no query ceiling, `--max-items` is still reported — the old behaviour.
#[test]
fn count_scope_still_names_the_output_ceiling_on_its_own() {
    let mut s = surface();
    s.count_only = true;
    s.max_items = 2;
    let shaped = try_apply_under(&s, envelope(), None).expect("no ceiling, no refusal");
    assert_eq!(shaped["agent_surface"]["count_scope"], json!("emitted"));
    assert_eq!(shaped["count"], json!(2));
}

/// And with neither ceiling the count really does describe what matched.
#[test]
fn count_scope_names_the_match_when_nothing_was_hidden() {
    let mut s = surface();
    s.count_only = true;
    let shaped = try_apply_under(&s, envelope(), None).expect("no ceiling, no refusal");
    assert_eq!(shaped["agent_surface"]["count_scope"], json!("matched"));
}

/// GAP-SG-209: knobs that need the whole set, aimed at one record per line.
///
/// Measured before the fix: `--count-only export --limit 10` emitted ELEVEN
/// `{"count":1}` lines instead of one count.
#[test]
fn whole_set_knobs_are_refused_on_a_stream() {
    for (label, mut s) in [
        ("--count-only", surface()),
        ("--sort", surface()),
        ("--dedupe-by", surface()),
        ("--max-output-bytes", surface()),
    ] {
        s.streamed = true;
        match label {
            "--count-only" => s.count_only = true,
            "--sort" => s.sort = Some("name".into()),
            "--dedupe-by" => s.dedupe_by = Some("name".into()),
            _ => s.max_output_bytes = 4096,
        }
        let message = refusal(&s, envelope());
        assert!(message.contains(label), "{label} must be named: {message}");
    }
}

/// The knobs that mean the same thing per record keep working on a stream.
///
/// Without this the previous test could pass by refusing streams wholesale,
/// which would remove a working feature to cure nothing.
///
/// NARROWED in v1.2.8 by the GAP-SG-215 contract decision, and the narrowing is
/// deliberate rather than a regression this test was bent around. `--max-items`
/// was in this list until it was MEASURED accepted and inert:
/// `--max-items 2 export --limit 5` answered with all five records and `exit 0`,
/// because it caps elements inside an envelope and a record line carries no
/// array to cap. `--filter` was in it too, and it turns the trailer's `exported`
/// count — computed by the command, before the surface sees a line — into a
/// claim about rows the caller never received. Both now refuse, which is what
/// the sibling test above pins.
#[test]
fn per_record_knobs_still_work_on_a_stream() {
    let mut s = surface();
    s.streamed = true;
    s.select = vec!["name".into()];
    s.truncate_content = 4;
    let shaped = apply(&s, envelope());
    assert_eq!(results(&shaped).len(), 4);
    assert!(results(&shaped)[0].get("name").is_some());
}

/// GAP-SG-206: a write already happened, so the count must not eat the receipt.
///
/// The gate deliberately refuses nothing after a write — retrying a succeeded
/// `remember` writes twice — but nothing stopped the SHAPING from replacing
/// `memory_id` and `entities_created` with a count.
///
/// This envelope carries ARRAYS on purpose, because that is the shape `remember`
/// actually emits and it is the one the first attempt at this fix missed. No
/// member here is a declared result key, so the surface elects `entities_created`
/// by FALLBACK and counts it: a guard scoped to the scalar branch would have
/// protected nothing at all.
#[test]
fn count_only_never_replaces_a_write_receipt() {
    let receipt = json!({
        "memory_id": 8458,
        "entities_created": ["alice-martins-souza"],
        "enrich_recommended": ["memory-bindings"]
    });
    let mut s = surface();
    s.mutates = true;
    s.writes_receipt = true;
    s.count_only = true;

    let shaped = apply(&s, receipt);
    assert_eq!(shaped["memory_id"], json!(8458), "the receipt must survive");
    assert_eq!(shaped["entities_created"], json!(["alice-martins-souza"]));
    assert_eq!(
        shaped["agent_surface"]["count_only_suppressed"],
        json!(true),
        "the suppression must be visible, never silent"
    );
    assert!(shaped.get("count").is_none());
}

/// The suppression turns on "did it WRITE", never on `mutates` alone.
///
/// `mutates` lists the read-only variants explicitly and defaults everything else
/// to `true`, so `config list-keys` — which writes nothing — reports `true`.
/// Keying the suppression on that alone took `--count-only config list-keys`
/// away, and the integration suite caught it.
#[test]
fn count_only_still_answers_for_a_command_that_only_looks_mutating() {
    let mut s = surface();
    s.mutates = true;
    s.writes_receipt = false;
    s.count_only = true;
    let shaped = apply(&s, envelope());
    assert_eq!(shaped["count"], json!(4));
    assert!(shaped["agent_surface"]
        .get("count_only_suppressed")
        .is_none());
}

/// The suppression is scoped to writes: a read still gets its count.
#[test]
fn count_only_still_answers_for_a_read() {
    let mut s = surface();
    s.count_only = true;
    let shaped = apply(&s, envelope());
    assert_eq!(shaped["count"], json!(4));
    assert!(shaped["agent_surface"]
        .get("count_only_suppressed")
        .is_none());
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
