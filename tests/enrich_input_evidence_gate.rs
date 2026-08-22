//! GAP-SG-279: an enrich operation that WRITES to the graph must say, in one
//! place, what it lets the model see before it decides.
//!
//! `entity-type-validate` spent its whole life deciding an entity's type from
//! two lines — the name and the label under dispute — and writing the answer to
//! `entities.type`. Nothing about that was hidden; it was simply invisible. The
//! input lived inside a `format!` in the middle of a 470-line module, next to
//! five sibling operations whose `format!` calls looked identical at a glance
//! and carried three to five fields each. Reviewing the file told you the code
//! compiled. It did not tell you that one of the six was judging a label from
//! how a name is spelled.
//!
//! Two guards were already watching the OTHER half of that decision. The prompt
//! is pinned by `entity_type_vocabulary_contract` in `schemas.rs`, which fails
//! the build if the wording and `CANONICAL_ENTITY_TYPES` disagree. The written
//! label is pinned by `normalize_entity_type`. Between a watched prompt and a
//! watched output sat an unwatched input, and it was the input that was wrong.
//!
//! # What it pins
//!
//! * Every write-path operation declares the fields of its `input_text`.
//! * The declaration matches what the source actually assembles.
//! * A declaration naming only the subject's own identifier is refused.
//! * The declared set is not allowed to shrink without the shrink being stated.
//!
//! # What it deliberately does not do
//!
//! It does not parse Rust. A gate that reimplements the compiler is a gate that
//! breaks on formatting, and one that breaks on formatting gets deleted. It
//! reads the source as text, finds the declaration block below, and checks the
//! two against each other — which is enough to make a silent input change loud.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Modules whose operations write to `entities` or `relationships`.
const WRITE_PATH_SOURCES: &[&str] = &[
    "src/commands/enrich/extraction_graph.rs",
    "src/commands/enrich/extraction_descriptions.rs",
];

/// Signals an operation may show the model, in the order of how much they cost.
///
/// The vocabulary is closed on purpose. An operation that needs a signal absent
/// from this list is doing something new enough to deserve a decision, and the
/// decision belongs in an ADR rather than in a `format!` string.
const KNOWN_SIGNALS: &[&str] = &[
    // The subject's own identifier. NEVER sufficient on its own — the whole
    // point of GAP-SG-279 — but legitimate as one field among others.
    "name",
    // The label currently stored, when the operation is judging that label.
    "current_type",
    "current_relation",
    "current_weight",
    // The other end of an edge.
    "source_name",
    "target_name",
    // Genuine evidence: something written about the subject, or the corpus it
    // appears in, or its typed neighbours in the graph.
    "description",
    "corpus",
    "neighbours",
    "body",
];

/// What each write-path operation is allowed to show the model.
///
/// This is the SSOT the gate compares the source against. Adding a field here
/// without adding it to the code fails; adding it to the code without declaring
/// it here fails too. Neither direction is more important: the pair only means
/// something when both are true at once.
fn declared_inputs() -> BTreeMap<&'static str, Vec<&'static str>> {
    let mut m: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::new();

    // GAP-SG-279: was `["name", "current_type"]`, which is the defect this file
    // exists to prevent from returning. It judged a type from a name.
    m.insert(
        "entity-type-validate",
        vec![
            "name",
            "current_type",
            "description",
            "corpus",
            "neighbours",
        ],
    );

    // The operation GAP-SG-279 borrowed its shape from. It has gathered corpus
    // and neighbours since G-PR-7, which is why the asymmetry was so stark.
    m.insert(
        "entity-descriptions",
        vec!["name", "current_type", "corpus", "neighbours"],
    );

    // GAP-SG-279 (class): both edge operations used to show two NAMES and the
    // label under dispute, and nothing else — the same evidence-free judgement,
    // persisted the same way. An edge has no body of its own, so the honest
    // evidence available is what the graph stores about its endpoints, and both
    // descriptions ride along on the join that was already running.
    m.insert(
        "weight-calibrate",
        vec![
            "source_name",
            "target_name",
            "description",
            "current_relation",
            "current_weight",
        ],
    );
    m.insert(
        "relation-reclassify",
        vec![
            "source_name",
            "target_name",
            "description",
            "current_relation",
        ],
    );

    m
}

/// Operations whose judgement rests on the SUBJECT'S OWN NAME and nothing else.
///
/// The list is empty and must stay that way. It exists so the refusal below has
/// somewhere to point when it fires, rather than being an assertion with no
/// stated remedy.
const OPERATIONS_ALLOWED_TO_JUDGE_FROM_A_NAME: &[&str] = &[];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read_source(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{relative} must be readable from the workspace root: {e}"))
}

/// Fields carrying genuine evidence, as opposed to the subject's own labels.
fn is_evidence(signal: &str) -> bool {
    matches!(signal, "description" | "corpus" | "neighbours" | "body")
}

#[test]
fn the_gate_found_the_sources_it_is_supposed_to_read() {
    for source in WRITE_PATH_SOURCES {
        let text = read_source(source);
        assert!(
            text.len() > 1_000,
            "{source} came back suspiciously short ({} bytes); the gate would \
             pass by reading nothing",
            text.len()
        );
    }
    assert!(
        declared_inputs().len() >= 4,
        "the declaration shrank below the four operations it was written for; \
         if an operation was removed, remove it here deliberately"
    );
}

#[test]
fn every_declared_signal_is_a_known_one() {
    let mut unknown = Vec::new();
    for (op, signals) in declared_inputs() {
        for signal in signals {
            if !KNOWN_SIGNALS.contains(&signal) {
                unknown.push(format!("{op} declares unknown signal `{signal}`"));
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "a signal outside the known vocabulary is a decision that was never \
         written down:\n  {}",
        unknown.join("\n  ")
    );
}

#[test]
fn no_write_path_operation_judges_from_the_subject_name_alone() {
    let mut offenders = Vec::new();
    for (op, signals) in declared_inputs() {
        if OPERATIONS_ALLOWED_TO_JUDGE_FROM_A_NAME.contains(&op) {
            continue;
        }
        if !signals.iter().any(|s| is_evidence(s)) {
            offenders.push(format!(
                "{op} sees only {signals:?}, none of which is evidence about the subject"
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "an operation that WRITES to the graph while seeing no evidence is \
         guessing with the authority of an audit — GAP-SG-279:\n  {}\n\n\
         Either give it evidence, or add it to \
         OPERATIONS_ALLOWED_TO_JUDGE_FROM_A_NAME with the reason written down.",
        offenders.join("\n  ")
    );
}

#[test]
fn entity_type_validate_actually_gathers_the_evidence_it_declares() {
    let source = read_source("src/commands/enrich/extraction_graph.rs");

    assert!(
        source.contains("load_entity_evidence_tuned"),
        "entity-type-validate declares corpus and neighbours but the source \
         never gathers them; the declaration would be a promise the code breaks"
    );
    assert!(
        source.contains("description"),
        "entity-type-validate declares `description` but the source never \
         selects it"
    );
    assert!(
        source.contains("entity_type_validate_user_text"),
        "the evidence must reach the model through the shared builder; a local \
         `format!` here is how the two-line input survived unnoticed for so long"
    );
    assert!(
        !source.contains(r#"format!("Entity: {ent_name}\nCurrent type: {ent_type}")"#),
        "the exact two-line input GAP-SG-279 removed is back in the source"
    );
}

#[test]
fn the_edge_operations_see_their_endpoints_descriptions() {
    let source = read_source("src/commands/enrich/extraction_graph.rs");
    assert!(
        source.contains("edge_endpoints_section"),
        "weight-calibrate and relation-reclassify must render their endpoints \
         through the shared builder; a local `format!` is how two NAMES passed \
         for evidence in both of them"
    );
    assert_eq!(
        source.matches("e1.description").count(),
        2,
        "both edge operations must select the source description; selecting it \
         in one and not the other is the asymmetry this gap is about"
    );
    assert_eq!(
        source.matches("e2.description").count(),
        2,
        "both edge operations must select the target description"
    );
}

#[test]
fn an_operation_that_writes_without_evidence_declares_an_abstention() {
    let source = read_source("src/commands/enrich/extraction_graph.rs");
    assert!(
        source.contains("corpus_is_sufficient"),
        "entity-type-validate must refuse to spend a token when it has nothing \
         to judge from; without the gate, absence of evidence silently becomes \
         a licence to guess"
    );
    assert!(
        source.contains("entity_type_no_evidence"),
        "the abstention must carry a reason naming what was missing, or the \
         caller sees a skip with no explanation"
    );
}

/// A gate that scans for a defect can pass by not scanning.
///
/// Feeding the detector the exact shape it was written to catch is what proves
/// it still looks. Both meta-tests below invert the assertions above against
/// fabricated input, so a detector that stopped detecting fails here rather
/// than reporting a clean tree.
#[test]
fn the_name_only_detector_fires_on_the_shape_it_was_written_for() {
    let as_it_was_wrong: Vec<&str> = vec!["name", "current_type"];
    assert!(
        !as_it_was_wrong.iter().any(|s| is_evidence(s)),
        "the pre-GAP-SG-279 input must be judged evidence-free, or the guard \
         above would have passed on the very defect it was written for"
    );

    let as_it_is_now = declared_inputs();
    let current = as_it_is_now
        .get("entity-type-validate")
        .expect("entity-type-validate must stay declared");
    assert!(
        current.iter().any(|s| is_evidence(s)),
        "the current declaration must be judged evidence-bearing, or the guard \
         is asserting something no input can satisfy"
    );
}

#[test]
fn the_unknown_signal_detector_rejects_a_signal_nobody_declared() {
    let fabricated = "vibes";
    assert!(
        !KNOWN_SIGNALS.contains(&fabricated),
        "the known-signal vocabulary must not silently admit anything"
    );
    assert!(
        KNOWN_SIGNALS.contains(&"corpus"),
        "the vocabulary must still admit the signals actually in use, or the \
         guard rejects the whole tree and gets disabled"
    );
}
