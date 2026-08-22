//! GAP-SG-271: a published contract nobody ever compares against the binary.
//!
//! `docs/schemas/` holds 76 contracts. Exactly one of them, `health`, is
//! derived from a Rust type by `schemars`; the rest are written by hand, and
//! before this gate a census of `tests/` found TWENTY-FIVE that no test
//! mentioned at all. Those files could say anything — the suites in
//! `tests/schema_contract_*.rs` validate the commands they know about and are
//! silent about every command they do not.
//!
//! The silence has already produced three defects in this same family: a
//! trailer that did not validate against its own schema, an inverted
//! deprecation note in `graph.schema.json`, and a published field the code
//! never filled. Three occurrences make it a CLASS, and a class is closed by a
//! rule, not by three more fixes.
//!
//! This gate closes it from two directions.
//!
//! * It runs the binary for every previously unvalidated command and validates
//!   the REAL envelope against the published document, the same way the
//!   contract suites do, through the same shared harness. The read-only side
//!   runs here; the write side runs in `schema_type_drift_write_gate.rs`.
//! * It then takes a census: every document under `docs/schemas/` must be
//!   exercised by one of those two files, referenced by some other test, or
//!   listed in [`EXEMPT`] with the reason written beside it. A contract added
//!   tomorrow with no validator fails this file on the same commit that adds
//!   it.
//!
//! Two premise checks keep the gate from reporting green while blind. The
//! census refuses an empty scan, because a discovery pattern that matches
//! nothing looks exactly like a clean tree. And the validator itself is fed a
//! knowingly divergent pair in
//! [`the_validator_rejects_a_knowingly_divergent_pair`], so a harness that
//! silently stopped rejecting anything cannot pass this file.
//!
//! Deriving each contract from its type, as `health` does, is not available
//! here: only `HealthResponse` and the `enrich` queue types derive
//! `JsonSchema`, and adding the derive elsewhere means editing `src/`, which is
//! outside this change. Validating the live envelope is the strongest check
//! reachable without that edit, and it catches the same drift from the other
//! end — through the bytes the command actually writes.
//!
//! NOT gated behind `slow-tests`, for the reason the contract suites state: a
//! gate the default `cargo test` never compiles is a gate-shaped reassurance.

#[path = "schema_drift_support/mod.rs"]
mod drift;

use drift::{
    check_case, fixture_env, published_ids, published_schema_text, repo_root, validate_schema, Env,
    LiveCase, READ_ONLY_CASES,
};
use serial_test::serial;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[test]
#[serial]
fn every_read_only_envelope_matches_its_published_contract() {
    let env = fixture_env();
    let mut checked = 0usize;
    for case in READ_ONLY_CASES {
        check_case(&env, case);
        checked += 1;
    }
    // The premise check counts what RAN, not what the table declares.
    // `assert!(!READ_ONLY_CASES.is_empty())` reads like the same guarantee and
    // is not one: the table is a const, so the compiler folds the assertion
    // away and `clippy::const_is_empty` refuses it. A gutted table has to fail
    // here rather than report green over an empty loop.
    assert!(
        checked >= 10,
        "only {checked} case(s) ran; the table was emptied out and this test \
         was about to pass without validating anything — the exact way a \
         contract gate goes blind"
    );
}

// ---------------------------------------------------------------------------
// Cases that change the database, each in its own fixture
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn the_error_envelope_matches_its_published_contract() {
    let env = fixture_env();
    let output = env
        .cmd()
        .args(["read", "--name", "memory-that-was-never-written"])
        .output()
        .expect("read failed to spawn");
    let instance = Env::parse_stdout(&output, "error-envelope");
    assert_eq!(
        instance["error"],
        serde_json::Value::Bool(true),
        "the miss did not produce an error envelope at all, so validating it \
         against `error-envelope.schema.json` would prove nothing"
    );
    validate_schema(
        "error-envelope",
        &published_schema_text("error-envelope"),
        &instance,
    );
}

#[test]
#[serial]
fn the_vec_purge_orphan_envelope_matches_its_published_contract() {
    let env = fixture_env();
    check_case(
        &env,
        &LiveCase {
            id: "vec-purge-orphan",
            argv: &["vec", "purge-orphan", "--yes"],
        },
    );
}

#[test]
#[serial]
fn the_migration_envelopes_match_their_published_contracts() {
    // `--to-llm-only --drop-vec-tables` removes tables `--rehash` inspects, so
    // the order here is load-bearing rather than incidental.
    let env = fixture_env();
    check_case(
        &env,
        &LiveCase {
            id: "migrate-rehash",
            argv: &["migrate", "--rehash"],
        },
    );
    check_case(
        &env,
        &LiveCase {
            id: "migrate-to-llm-only",
            argv: &["migrate", "--to-llm-only", "--drop-vec-tables"],
        },
    );
}

// ---------------------------------------------------------------------------
// The census: no contract without a validator
// ---------------------------------------------------------------------------

// GAP-SG-290, measured 2026-08-21 and CLOSED in v1.2.8.
//
// The degraded path of `recall` and `hybrid-search` used to be the one shape
// nothing here checked, and it was the shape most worth checking: both run
// entirely offline under `--fallback-fts-only`, and both failed their own
// document. Five violations, all of them the document trailing the binary:
// `source` emitted `fts_fallback` against an enum listing only `direct` and
// `graph`, and each root closed with `additionalProperties: false` while the
// command emitted `vec_degraded`, `vec_error` and `warning`, none declared.
//
// The silence had a shape worth naming. Those three fields are guarded by
// `skip_serializing_if`, so they VANISH on the happy path and the envelope
// validated perfectly there. The contract broke only when the search had
// fallen back to lexical ranking — precisely when a consumer most needs to
// trust it. Same form as `remember-batch-summary` under GAP-SG-271, where the
// empty batch validated and only the useful one was rejected.
//
// `vec_degraded_reason` was the deepest of the five. The document published a
// seven-variant enum, and `src/embedder/fallback.rs` really does define those
// seven — but the wire never carried them: `recall.rs` assigns `vec_error`,
// which is `FallbackReason`'s `Display`, prose embedding the provider's own
// message. The document described the INTENT the code documents, and no enum
// could hold a field that carries free text. v1.2.8 publishes the stable half
// beside it as `vec_degraded_code` rather than replacing the prose, so nothing
// reading the old field breaks.
//
// The degraded cases below exist so that shape can never go unchecked again.

/// Contracts no test can produce, each with the reason written beside it.
///
/// The list is short on purpose. Every entry is a document whose emitter needs
/// something a hermetic test cannot supply, and each one is a standing debt
/// rather than a decision: closing it means making the input reachable, not
/// deleting the line.
const EXEMPT: &[(&str, &str)] = &[
    (
        "deep-research-output-ack",
        "emitted only by `deep-research --output PATH`, which needs a live \
         OpenRouter key; the mock LLM under tests/mock-llm returns a fixed \
         embedding vector and cannot drive a research run",
    ),
    (
        "ingest-claude-phase",
        "emitted only by `ingest --mode claude-code`, which reads a real \
         Claude Code installation tree; the sibling file and summary events \
         are covered by tests/ingest_* against a synthesised tree, but the \
         validate phase reports the installed `claude` version",
    ),
];

/// Every `.rs` file under `tests/`, recursively, except this gate's own family.
///
/// THE DECISION, taken when GAP-SG-288 split this file: the two siblings are
/// excluded from the scan, not merely from the walk's convenience. Leaving them
/// in would let a contract count as validated because its id appears as a
/// STRING in `READ_ONLY_CASES` or in a moved test — citation, which is the weak
/// coverage this gate exists to replace. Worse, deleting a live case would then
/// keep passing the census as long as the id survived somewhere in the module.
/// The ids those two files exercise are declared explicitly in
/// [`ids_covered_here`] instead, so the census counts them as covered HERE and
/// its strength is unchanged by the split.
fn other_test_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo_root().join("tests"), &mut out);
    out.retain(|p| {
        p.file_name().is_some_and(|n| {
            n != "schema_type_drift_gate.rs" && n != "schema_type_drift_write_gate.rs"
        }) && !p
            .components()
            .any(|c| c.as_os_str() == "schema_drift_support")
    });
    out.sort();
    out
}

/// Contract ids some other test file names.
///
/// A file counts as a validator when it names the document (`<id>.schema.json`)
/// or the bare id in a string literal — `enrich_status_schema_drift_gate.rs`
/// builds the filename with `format!`, so matching only the full name would
/// declare a covered contract uncovered and push it into [`EXEMPT`], which is
/// how an exemption list fills up with entries that never needed to be there.
fn ids_referenced_by_other_tests(ids: &BTreeSet<String>) -> BTreeSet<String> {
    let sources: Vec<String> = other_test_sources()
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .collect();
    assert!(
        !sources.is_empty(),
        "the walk read no test source at all, which means it is looking in the \
         wrong place rather than that the tree is empty"
    );
    ids.iter()
        .filter(|id| {
            let document = format!("{id}.schema.json");
            let quoted = format!("\"{id}\"");
            sources
                .iter()
                .any(|s| s.contains(&document) || s.contains(&quoted))
        })
        .cloned()
        .collect()
}

/// Contract ids this gate's family validates live.
///
/// "Here" means this file OR `schema_type_drift_write_gate.rs`, which was
/// carved out of it by GAP-SG-288 and whose cases are named in the list below
/// exactly as they were when they lived in this file. The census must not be
/// left to discover the sibling by scanning it — see [`other_test_sources`] for
/// why that would trade live validation for citation.
fn ids_covered_here() -> BTreeSet<String> {
    let mut ids: BTreeSet<String> = READ_ONLY_CASES.iter().map(|c| c.id.to_string()).collect();
    for extra in [
        "error-envelope",
        "vec-purge-orphan",
        "migrate-rehash",
        "migrate-to-llm-only",
        "remember-batch",
        "remember-batch-summary",
        "entities-input",
        "relationships-input",
        "graph-input",
        "init",
        "remember",
        "edit",
        "rename",
        "restore",
        "forget",
        "purge",
        "link",
        "unlink",
        "reclassify",
        "rename-entity",
        "merge-entities",
        "delete-entity",
        "fts-rebuild",
        "optimize",
        "vacuum",
        "backup",
        "sync-safe-copy",
        "export-memory-line",
        "export-summary",
        "enrich-phase",
        "enrich-item-event",
        "enrich-summary",
    ] {
        ids.insert(extra.to_string());
    }
    ids
}

#[test]
fn every_published_contract_has_something_that_checks_it() {
    let ids = published_ids();
    assert!(
        ids.len() >= 70,
        "the scan found only {} contract(s) under docs/schemas, far below the \
         75 the tree carries — the discovery pattern is broken, and an empty \
         or truncated set is how this gate would report green while checking \
         nothing",
        ids.len()
    );

    let here = ids_covered_here();
    let unknown: Vec<&String> = here.difference(&ids).collect();
    assert!(
        unknown.is_empty(),
        "this file claims to cover contract(s) that do not exist under \
         docs/schemas: {unknown:?}. A renamed document leaves the case behind \
         reading a file that is gone."
    );

    let exempt: BTreeSet<String> = EXEMPT.iter().map(|(id, _)| id.to_string()).collect();
    let stale_exemptions: Vec<&String> = exempt.difference(&ids).collect();
    assert!(
        stale_exemptions.is_empty(),
        "EXEMPT names contract(s) that no longer exist: {stale_exemptions:?}. \
         An exemption outliving its document is a permission nobody is using \
         and nobody will notice."
    );

    let elsewhere = ids_referenced_by_other_tests(&ids);
    let mut unchecked: Vec<&String> = ids
        .iter()
        .filter(|id| !here.contains(*id) && !elsewhere.contains(*id) && !exempt.contains(*id))
        .collect();
    unchecked.sort();

    assert!(
        unchecked.is_empty(),
        "{} published contract(s) have no validator anywhere: {unchecked:?}.\n\
         Add a live case to READ_ONLY_CASES in this file, validate the envelope \
         in a tests/schema_contract_*.rs suite, or add an EXEMPT entry stating \
         what a hermetic test cannot supply. A document nothing compares \
         against is free to say anything, which is how GAP-SG-271's three \
         earlier defects survived review.",
        unchecked.len()
    );
}

// ---------------------------------------------------------------------------
// The gate proves it has teeth
// ---------------------------------------------------------------------------

/// Runs `validate_schema` and reports whether it rejected.
///
/// The panic hook is silenced for the duration: a deliberate rejection prints
/// its whole violation list to stderr, and a test whose PASSING output looks
/// like a failure teaches the reader to ignore the failures that matter.
fn rejects(id: &str, schema: &str, instance: &serde_json::Value) -> bool {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = std::panic::catch_unwind(|| validate_schema(id, schema, instance));
    std::panic::set_hook(previous);
    outcome.is_err()
}

#[test]
fn the_validator_rejects_a_knowingly_divergent_pair() {
    let schema = published_schema_text("slots-status");

    let honest = serde_json::json!({
        "action": "slots_status",
        "max_concurrency": 4,
        "active": 0,
        "free": 4,
        "slots": [],
        "elapsed_ms": 0
    });
    assert!(
        !rejects("slots-status", &schema, &honest),
        "the validator rejected an envelope shaped like the real one, so every \
         live case above is failing for the wrong reason"
    );

    let mut wrong_type = honest.clone();
    wrong_type["max_concurrency"] = serde_json::json!("four");
    assert!(
        rejects("slots-status", &schema, &wrong_type),
        "the validator accepted a string where the contract declares an \
         integer, so it is not validating and every live case above is a \
         no-op that reports green"
    );

    let mut missing_required = honest.clone();
    missing_required
        .as_object_mut()
        .expect("object")
        .remove("action");
    assert!(
        rejects("slots-status", &schema, &missing_required),
        "the validator accepted an envelope missing a required member, so a \
         command that stopped emitting a documented field would pass unnoticed"
    );

    let mut undeclared_member = honest;
    undeclared_member["a_member_no_command_emits"] = serde_json::json!(1);
    assert!(
        rejects("slots-status", &schema, &undeclared_member),
        "the validator accepted a member `slots-status.schema.json` does not \
         declare. That document sets `additionalProperties: false`; if this \
         assertion is what broke, the flag was flipped and the contract can no \
         longer detect an added field"
    );
}
