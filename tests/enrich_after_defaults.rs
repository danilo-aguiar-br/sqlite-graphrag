// GAP-SG-150 — `ingest --enrich-after` must not invent its own enrich defaults.
//
// `src/commands/ingest/enrich_after.rs` hand-synthesises an `EnrichArgs` with
// ~75 fields. Twice already a literal there drifted away from what clap
// declares: `mode` was pinned to `ClaudeCode` (so `--enrich-after` spawned a
// headless subprocess while every other entry point used the REST path), and
// `openrouter_timeout` was pinned to `Some(300)` (halving the documented 600 s
// budget and shadowing XDG). Both were corrected to `None`, the "operator passed
// nothing" state — and nothing stopped them drifting again.
//
// This suite is that stop. It reads the synthesis site as text and cross-checks
// every field against the clap declaration:
//
//   * fields the phase leaves at the clap default are verified against the
//     value clap actually produces for an argument-free invocation;
//   * fields backed by a shared constant are verified SYMBOLICALLY — both sites
//     must name the SAME constant, so changing the constant's value stays a
//     one-line edit while swapping which constant is used fails loudly;
//   * fields that legitimately diverge are listed with the reason. A declared
//     exception is a contract; a silent one is a bug.
//
// The last test refuses to let a newly added `EnrichArgs` field go unclassified.

use clap::Parser;
use sqlite_graphrag::commands::enrich::{EnrichArgs, ReEmbedTarget};

const ENRICH_AFTER_SRC: &str = include_str!("../src/commands/ingest/enrich_after.rs");
const ENRICH_ARGS_SRC: &str = include_str!("../src/commands/enrich/args.rs");

/// `EnrichArgs` derives `clap::Args`, so it needs a `Parser` host to be built
/// from an argv. This wrapper exists only to reach the declared defaults.
#[derive(Parser)]
struct DefaultsProbe {
    #[command(flatten)]
    args: EnrichArgs,
}

/// The `EnrichArgs` clap produces when the operator passes nothing of substance.
///
/// An entirely empty argv does NOT work here: `operation` and `mode` are
/// declared `required_unless_present_any`, and clap answers a missing required
/// argument by writing usage to stderr and calling `exit(2)` on the spot. Inside
/// a test binary that kills the whole harness before a single assertion runs —
/// the suite reports no failure, it reports `error: test failed` with exit 2.
///
/// `--print-schema` is the cheapest member of the set that dispenses with both:
/// it takes no value and touches nothing else, so every remaining field still
/// falls to its declared default. The one cost is that `print_schema` itself
/// parses as `true` here, so its row is cross-checked against the synthesis site
/// only, which its table entry documents.
fn clap_defaults() -> EnrichArgs {
    DefaultsProbe::parse_from(["enrich", "--print-schema"]).args
}

// ---------------------------------------------------------------------------
// Reading the synthesis site
// ---------------------------------------------------------------------------

/// Returns the initialiser text `enrich_after.rs` writes for `field`.
///
/// Matches `        <field>:` at the struct-literal indentation and returns
/// everything up to the terminating comma with whitespace collapsed, so a
/// multi-line initialiser (rustfmt wraps the long constant paths) is captured
/// whole and compares equal to its single-line spelling.
fn synthesised(field: &str) -> String {
    let needle = format!("\n        {field}:");
    let start = ENRICH_AFTER_SRC.find(&needle).unwrap_or_else(|| {
        panic!(
            "enrich_after.rs no longer initialises `{field}`; either the field \
             was removed from EnrichArgs or the synthesis site was restructured"
        )
    }) + needle.len();
    let rest = &ENRICH_AFTER_SRC[start..];
    let end = rest.find(",\n").unwrap_or_else(|| {
        panic!("initialiser for `{field}` in enrich_after.rs is not comma-terminated")
    });
    rest[..end].split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every `pub <name>:` field declared on `EnrichArgs`, in declaration order.
fn declared_fields() -> Vec<String> {
    let body_start = ENRICH_ARGS_SRC
        .find("pub struct EnrichArgs {")
        .expect("EnrichArgs struct not found in args.rs");
    ENRICH_ARGS_SRC[body_start..]
        .lines()
        .take_while(|line| *line != "}")
        .filter_map(|line| {
            let trimmed = line.trim();
            let name = trimmed.strip_prefix("pub ")?.split(':').next()?;
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                .then(|| name.to_string())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Group 1 — left at the clap default
// ---------------------------------------------------------------------------

/// A field the phase leaves at its clap default.
struct DefaultedField {
    /// Field name on `EnrichArgs`.
    name: &'static str,
    /// Exact initialiser text expected in `enrich_after.rs`.
    literal: &'static str,
    /// True when the clap default agrees with that initialiser.
    agrees: fn(&EnrichArgs) -> bool,
}

/// Fields whose synthesised value must equal the clap default.
fn defaulted_fields() -> Vec<DefaultedField> {
    macro_rules! defaulted {
        ($name:ident, $literal:expr, $agrees:expr) => {
            DefaultedField {
                name: stringify!($name),
                literal: $literal,
                agrees: $agrees,
            }
        };
    }
    vec![
        // The two fields that actually drifted. Kept here as well as in their
        // own dedicated tests, so the table stays the complete picture.
        defaulted!(mode, "None", |a: &EnrichArgs| a.mode.is_none()),
        defaulted!(limit, "None", |a: &EnrichArgs| a.limit.is_none()),
        defaulted!(
            target,
            "crate::commands::enrich::ReEmbedTarget::Memories",
            |a: &EnrichArgs| a.target == ReEmbedTarget::Memories
        ),
        defaulted!(dry_run, "false", |a: &EnrichArgs| !a.dry_run),
        defaulted!(openrouter_model, "None", |a: &EnrichArgs| a
            .openrouter_model
            .is_none()),
        defaulted!(openrouter_api_key, "None", |a: &EnrichArgs| a
            .openrouter_api_key
            .is_none()),
        defaulted!(openrouter_base_url, "None", |a: &EnrichArgs| a
            .openrouter_base_url
            .is_none()),
        defaulted!(json, "false", |a: &EnrichArgs| !a.json),
        defaulted!(resume, "false", |a: &EnrichArgs| !a.resume),
        defaulted!(retry_failed, "false", |a: &EnrichArgs| !a.retry_failed),
        defaulted!(reset_stale_claims, "false", |a: &EnrichArgs| !a
            .reset_stale_claims),
        defaulted!(names, "Vec::new()", |a: &EnrichArgs| a.names.is_empty()),
        defaulted!(names_file, "None", |a: &EnrichArgs| a.names_file.is_none()),
        defaulted!(preflight_check, "false", |a: &EnrichArgs| !a
            .preflight_check),
        defaulted!(max_load_check, "true", |a: &EnrichArgs| a.max_load_check),
        defaulted!(force_redescribe, "false", |a: &EnrichArgs| !a
            .force_redescribe),
        defaulted!(quality_sample, "None", |a: &EnrichArgs| a
            .quality_sample
            .is_none()),
        defaulted!(entity_names, "Vec::new()", |a: &EnrichArgs| a
            .entity_names
            .is_empty()),
        // v1.2.8: narrows an entity-keyed operation to one stored type label.
        // `ingest --enrich-after` runs memory-bindings over what the ingest just
        // wrote, which is not scoped to a type, so it leaves the clap default.
        defaulted!(entity_type, "None", |a: &EnrichArgs| a
            .entity_type
            .is_none()),
        defaulted!(memory_names, "Vec::new()", |a: &EnrichArgs| a
            .memory_names
            .is_empty()),
        defaulted!(anchor_memory, "None", |a: &EnrichArgs| a
            .anchor_memory
            .is_none()),
        defaulted!(
            entity_description_domain,
            "\"auto\".to_string()",
            |a: &EnrichArgs| a.entity_description_domain == "auto"
        ),
        defaulted!(yield_every_n_items, "None", |a: &EnrichArgs| a
            .yield_every_n_items
            .is_none()),
        defaulted!(ops_gate, "false", |a: &EnrichArgs| !a.ops_gate),
        defaulted!(preserve_check, "true", |a: &EnrichArgs| a.preserve_check),
        defaulted!(prompt_template, "None", |a: &EnrichArgs| a
            .prompt_template
            .is_none()),
        defaulted!(until_empty, "false", |a: &EnrichArgs| !a.until_empty),
        defaulted!(max_runtime, "None", |a: &EnrichArgs| a
            .max_runtime
            .is_none()),
        defaulted!(status, "false", |a: &EnrichArgs| !a.status),
        defaulted!(list_dead, "false", |a: &EnrichArgs| !a.list_dead),
        defaulted!(requeue_dead, "false", |a: &EnrichArgs| !a.requeue_dead),
        defaulted!(list_skipped, "false", |a: &EnrichArgs| !a.list_skipped),
        defaulted!(requeue_skipped, "false", |a: &EnrichArgs| !a
            .requeue_skipped),
        defaulted!(prune_dead_orphans, "false", |a: &EnrichArgs| !a
            .prune_dead_orphans),
        defaulted!(prune_dead_entity_orphans, "false", |a: &EnrichArgs| !a
            .prune_dead_entity_orphans),
        defaulted!(ignore_backoff, "false", |a: &EnrichArgs| !a.ignore_backoff),
        defaulted!(body_extract_graph_only, "false", |a: &EnrichArgs| !a
            .body_extract_graph_only),
        // GAP-SG-185 added the keyset page width in v1.2.4 and left it out of
        // this table, which is what turned the guard red. `None` is the omission
        // state that lets `EnrichArgs::scan_page_size()` resolve the documented
        // precedence (flag > XDG `enrich.scan_page_size` > 512); pinning a
        // literal here would shadow XDG exactly like `openrouter_timeout` did.
        defaulted!(scan_page_size, "None", |a: &EnrichArgs| a
            .scan_page_size
            .is_none()),
        // `--print-schema` is what lets `clap_defaults()` parse at all, so the
        // parsed value is `true` by construction and the clap side of this row
        // is vacuous. The synthesis-site literal is still enforced, which is the
        // half that catches drift in `enrich_after.rs`.
        defaulted!(print_schema, "false", |_: &EnrichArgs| true),
        // GAP-SG-283: the entity type vocabulary policy. `ingest --enrich-after`
        // never declared one, so both stay at the clap default and fall through
        // to the compiled `keep` over the canonical set — the v1.2.8 behaviour,
        // byte for byte. A value here would make `ingest` impose a taxonomy
        // policy nobody asked it for.
        defaulted!(allowed_types, "Vec::new()", |a: &EnrichArgs| a
            .allowed_types
            .is_empty()),
        defaulted!(on_unknown_type, "None", |a: &EnrichArgs| a
            .on_unknown_type
            .is_none()),
    ]
}

#[test]
fn defaulted_fields_agree_with_clap() {
    let parsed = clap_defaults();
    for field in defaulted_fields() {
        assert_eq!(
            synthesised(field.name),
            field.literal,
            "enrich_after.rs pins `{}` to something other than the expected \
             initialiser; if the change is deliberate, update this table and say why",
            field.name
        );
        assert!(
            (field.agrees)(&parsed),
            "clap's default for `{}` no longer agrees with the value \
             enrich_after.rs synthesises; the two have drifted apart",
            field.name
        );
    }
}

// ---------------------------------------------------------------------------
// Group 2 — backed by a shared constant, checked symbolically
// ---------------------------------------------------------------------------

/// Fields where both sites must name the SAME constant.
///
/// Checking the constant NAME rather than its value keeps a legitimate tuning
/// change a one-line edit, while still failing when one site starts using a
/// different constant or an inline literal.
const CONST_BACKED: &[(&str, &str)] = &[
    ("stale_claim_secs", "DEFAULT_ENRICH_STALE_CLAIM_SECS"),
    ("rate_limit_buffer", "DEFAULT_ENRICH_RATE_LIMIT_BUFFER_SECS"),
    (
        "circuit_breaker_threshold",
        "DEFAULT_ENRICH_CIRCUIT_BREAKER_THRESHOLD",
    ),
    ("preserve_threshold", "DEFAULT_ENRICH_PRESERVE_THRESHOLD"),
    ("max_attempts", "DEFAULT_ENRICH_MAX_ATTEMPTS"),
    ("min_output_chars", "DEFAULT_BODY_ENRICH_MIN_CHARS"),
    ("max_output_chars", "DEFAULT_BODY_ENRICH_MAX_CHARS"),
    ("rest_concurrency", "DEFAULT_ENRICH_REST_CONCURRENCY"),
];

/// Fields resolved through an ACCESSOR rather than by clap's default.
///
/// These are `Option<T>` with NO `default_value_t`, so `None` is the correct
/// value at every synthesising site: the accessor applies flag > XDG >
/// constant, and naming the constant at a call site would pin that site to the
/// compiled default and silently ignore an operator's `config set`.
///
/// This class exists because putting such a field in [`CONST_BACKED`] hid a
/// real defect. `entity_description_grounding_threshold` carried
/// `default_value_t`, so clap ALWAYS supplied a value and the branch reading
/// the compiled constant was reachable only by typing `0` explicitly — raising
/// the constant changed nothing in production, and the XDG key had no reader at
/// all. The assertions below are the ones that would have caught it: no
/// `default_value_t`, and the constant named inside the accessor.
const ACCESSOR_RESOLVED: &[(&str, &str, &str)] = &[(
    "entity_description_grounding_threshold",
    "DEFAULT_ENRICH_GROUNDING_THRESHOLD",
    "entity_description_grounding_threshold",
)];

#[test]
fn accessor_resolved_fields_carry_no_clap_default() {
    for (field, konst, accessor) in ACCESSOR_RESOLVED {
        let synth = synthesised(field);
        assert!(
            synth.contains("None"),
            "enrich_after.rs sets `{field}` to `{synth}`; it must be `None` so \
             the accessor can consult the XDG key before the compiled default"
        );

        let attr = clap_attr(field);
        assert!(
            !attr.contains("default_value_t"),
            "clap declares `{field}` with `{attr}`. A `default_value_t` here \
             means clap always supplies a value, so the accessor's XDG lookup \
             and compiled default become unreachable — the exact defect this \
             class was created to prevent"
        );

        let accessor_at = ENRICH_ARGS_SRC
            .find(&format!("fn {accessor}(&self)"))
            .unwrap_or_else(|| panic!("`{field}` declares no accessor `{accessor}`"));
        let body = &ENRICH_ARGS_SRC[accessor_at..];
        let body = &body[..body.find("\n    }").unwrap_or(body.len())];
        assert!(
            body.contains(konst),
            "the accessor `{accessor}` does not name `{konst}`, so the compiled \
             default it falls back to is no longer the shared constant"
        );
    }
}

/// Returns the `#[arg(...)]` attribute text clap declares for `field`.
fn clap_attr(field: &str) -> String {
    let needle = format!("\n    pub {field}:");
    let field_at = ENRICH_ARGS_SRC
        .find(&needle)
        .unwrap_or_else(|| panic!("`{field}` is not declared on EnrichArgs"));
    let before = &ENRICH_ARGS_SRC[..field_at];
    let attr_at = before
        .rfind("#[arg(")
        .unwrap_or_else(|| panic!("`{field}` carries no #[arg(...)] attribute"));
    before[attr_at..]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn const_backed_fields_name_the_same_constant_on_both_sides() {
    for (field, konst) in CONST_BACKED {
        let synth = synthesised(field);
        assert!(
            synth.contains(konst),
            "enrich_after.rs sets `{field}` to `{synth}` instead of naming \
             `{konst}`; an inline literal here is exactly how the default drifted before"
        );
        let attr = clap_attr(field);
        assert!(
            attr.contains(&format!("default_value_t = {konst}")),
            "clap declares `{field}` with `{attr}`, which no longer defaults to \
             `{konst}`; enrich_after.rs and the CLI now disagree"
        );
    }
}

// ---------------------------------------------------------------------------
// The two fields that actually drifted, pinned individually
// ---------------------------------------------------------------------------

/// GAP-SG-149: `mode` was pinned to `ClaudeCode`, so `ingest --enrich-after`
/// spawned a headless subprocess while every other entry point resolved through
/// `EnrichArgs::mode()` to the REST path. `None` is the omission state that lets
/// that resolution happen.
#[test]
fn mode_stays_none() {
    assert_eq!(
        synthesised("mode"),
        "None",
        "enrich_after.rs pinned `mode` again; it MUST stay `None` so \
         EnrichArgs::mode() resolves the documented default"
    );
    assert!(
        clap_defaults().mode.is_none(),
        "clap no longer defaults `mode` to None, so the omission state \
         enrich_after.rs relies on no longer exists"
    );
}

/// GAP-SG-149 closed for good: `openrouter_timeout` is no longer a FIELD.
///
/// The original defect was `enrich_after.rs` pinning `Some(300)`, which halved
/// the documented 600 s chat budget and shadowed XDG. v1.2.3 removes the whole
/// category: the flag is GLOBAL on `Cli`, so there is no per-variant field left
/// to pin, and the budget resolves through `runtime_config` in the documented
/// precedence. This test now guards the REMOVAL — if the field ever comes back
/// on `EnrichArgs`, clap sees a duplicate argument id and the pinning bug has a
/// place to live again.
#[test]
fn openrouter_timeout_is_not_an_enrich_field() {
    let args_src = include_str!("../src/commands/enrich/args.rs");
    assert!(
        !args_src.contains("pub openrouter_timeout:"),
        "`openrouter_timeout` came back as an EnrichArgs field; it is a global \
         flag since v1.2.3 and redeclaring it gives clap a duplicate argument id"
    );
    assert!(
        args_src.contains("runtime_config::openrouter_chat_timeout_secs"),
        "the chat budget must resolve through runtime_config so XDG \
         `llm.openrouter_timeout_secs` can win over the compiled default"
    );

    let after_src = include_str!("../src/commands/ingest/enrich_after.rs");
    assert!(
        !after_src.contains("openrouter_timeout:"),
        "enrich_after.rs is synthesising `openrouter_timeout` again"
    );
}

// ---------------------------------------------------------------------------
// Declared exceptions, and the guard that keeps the classification complete
// ---------------------------------------------------------------------------

/// Fields that legitimately diverge from the clap default, with the reason.
///
/// Everything here is either the point of the phase or a value carried over
/// from the `ingest` invocation that triggered it. None of them may be silently
/// added to: an unclassified field means `enrich_after.rs` is deciding
/// something the operator never asked it to decide.
const DECLARED_EXCEPTIONS: &[(&str, &str)] = &[
    (
        "operation",
        "the phase exists to run memory-bindings; clap's default is None",
    ),
    ("namespace", "carried from the ingest invocation"),
    ("db", "carried from the ingest invocation"),
    ("max_cost_usd", "carried from the ingest invocation"),
    ("llm_parallelism", "carried from the ingest invocation"),
    ("wait_job_singleton", "carried from the ingest invocation"),
    ("force_job_singleton", "carried from the ingest invocation"),
];

#[test]
fn every_enrich_args_field_is_classified() {
    let defaulted: Vec<&str> = defaulted_fields().iter().map(|f| f.name).collect();
    let unclassified: Vec<String> = declared_fields()
        .into_iter()
        .filter(|field| {
            !defaulted.contains(&field.as_str())
                && !CONST_BACKED.iter().any(|(name, _)| *name == field.as_str())
                && !ACCESSOR_RESOLVED
                    .iter()
                    .any(|(name, _, _)| *name == field.as_str())
                && !DECLARED_EXCEPTIONS
                    .iter()
                    .any(|(name, _)| *name == field.as_str())
        })
        .collect();
    assert!(
        unclassified.is_empty(),
        "EnrichArgs gained {n} field(s) that `ingest --enrich-after` synthesises \
         without this suite knowing what value they should hold: {list}.\n\
         Classify each one: add it to `defaulted_fields()` if the phase leaves it \
         at the clap default, to `CONST_BACKED` if a shared constant backs it, to \
         `ACCESSOR_RESOLVED` if an accessor applies flag > XDG > constant, or \
         to `DECLARED_EXCEPTIONS` with the reason it diverges.",
        n = unclassified.len(),
        list = unclassified.join(", ")
    );
}

#[test]
fn declared_exceptions_are_really_synthesised() {
    for (field, reason) in DECLARED_EXCEPTIONS {
        let synth = synthesised(field);
        assert!(
            !synth.is_empty(),
            "`{field}` is listed as a declared exception ({reason}) but \
             enrich_after.rs no longer initialises it"
        );
    }
}
