//! GAP-SG-216 / GAP-SG-277: help text that the parser contradicts is worse than
//! no help.
//!
//! Two defects, one gate, because they are the same defect twice.
//!
//! First, until v1.2.8 `remember --help` told callers that `reference`, `skill`,
//! `document`, `note`, `user` and `feedback` were "MEMORY types only — NOT valid
//! for entities". The parser had accepted all six since v1.1.8, folding each one
//! onto a canonical kind. The help promised a refusal that never happened.
//!
//! Then v1.2.8 opened the vocabulary outright (GAP-SG-277, GAP-SG-278): the SQL
//! `CHECK` is gone with V017, `EntityType` is no longer an enum, and no label is
//! rewritten into another one. That inverted this file's central assertion. It
//! used to read the fold arrows the help printed and verify each against
//! `map_to_canonical`. There is no `map_to_canonical` any more, and a printed
//! fold arrow is now itself the defect — the help would be teaching a rewrite
//! that no longer occurs.
//!
//! # What it pins
//!
//! * The help teaches NO fold, because none happens.
//! * The help states the vocabulary is open, and names the escape hatch.
//! * Every recommended kind is listed, so a fourteenth cannot ship unnamed.
//! * The help never claims to reject a label the parser accepts.
//! * The help never denies a form of invocation that clap actually defines.

use clap::CommandFactory;
use sqlite_graphrag::entity_type::CANONICAL_ENTITY_TYPES;

/// The v1.2.8 text, kept verbatim as the detector's own test subject.
///
/// A gate that scans for a defect can pass by not scanning. Feeding it the exact
/// wording it was written to catch is what proves it still looks — see
/// [`the_gate_detects_what_it_claims_to_detect`].
const HELP_AS_IT_WAS_WRONG: &str = "\
ENTITY TYPES (for --graph-stdin entities, NOT memory --type):
  concept, tool, person, file, project, decision, incident,
  organization, location, date, dashboard, issue_tracker, memory
  WARNING: reference, skill, document, note, user, feedback are
  MEMORY types only — NOT valid for entities.
  Mapping: reference→concept, document→file, user→person
NOTE:
  remember does NOT accept positional arguments.";

/// Claims about the parser that the parser does not honour.
///
/// Each is a phrase, not a word, because the offence is the ASSERTION and not
/// the vocabulary: the help may and should still name the thirteen kinds and
/// still warn that `--type` is a different vocabulary. What it may not do is
/// promise a refusal that never happens.
const CLAIMS_THE_PARSER_REFUTES: &[&str] = &[
    "NOT valid for entities",
    "not valid for entities",
    "does NOT accept positional arguments",
    "does not accept positional arguments",
];

/// Rendered long help of one subcommand of the root CLI.
fn long_help(subcommand: &str) -> String {
    let mut root = sqlite_graphrag::cli::Cli::command();
    // `after_long_help` is attached during `build`, so rendering a subcommand
    // straight off `command()` silently drops the whole block this gate reads.
    root.build();
    let child = root
        .find_subcommand_mut(subcommand)
        .unwrap_or_else(|| panic!("`{subcommand}` must exist on the root command"));
    child.render_long_help().to_string()
}

/// Every `a→b` pair printed in a help text, as `(declared, canonical)`.
///
/// Kept from the pre-v1.2.8 gate, where it verified the arrows. It now serves
/// the opposite purpose: finding any arrow at all is the failure, because the
/// rewrite it would describe no longer exists. Both the ASCII `->` and the
/// typographic `→` are accepted, because either could be typed.
fn printed_mappings(help: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for token in help.split_whitespace() {
        let token = token.trim_matches(|c: char| c == ',' || c == '.' || c == ';');
        let Some((left, right)) = token.split_once('→').or_else(|| token.split_once("->")) else {
            continue;
        };
        if left.is_empty() || right.is_empty() {
            continue;
        }
        out.push((left.to_string(), right.to_string()));
    }
    out
}

#[test]
fn the_gate_detects_what_it_claims_to_detect() {
    // Premise 1: the detector fires on the wording that shipped for two minors.
    let caught: Vec<&str> = CLAIMS_THE_PARSER_REFUTES
        .iter()
        .copied()
        .filter(|claim| HELP_AS_IT_WAS_WRONG.contains(claim))
        .collect();
    assert_eq!(
        caught.len(),
        2,
        "the detector must flag BOTH v1.2.8 claims, caught {caught:?}"
    );

    // Premise 2: the arrow reader really reads arrows, and reads them right.
    let pairs = printed_mappings(HELP_AS_IT_WAS_WRONG);
    assert_eq!(
        pairs.len(),
        3,
        "the v1.2.8 text prints exactly three mappings, read {pairs:?}"
    );
    assert!(pairs.contains(&("reference".to_string(), "concept".to_string())));

    // Premise 3: the help this gate reads is not empty, which would make every
    // `contains` assertion below pass by having nothing to look at.
    let help = long_help("remember");
    assert!(
        help.len() > 500,
        "rendered help is suspiciously short ({} bytes)",
        help.len()
    );
}

/// The inversion of the pre-v1.2.8 assertion.
#[test]
fn the_help_teaches_no_fold_because_none_happens() {
    let help = long_help("remember");
    let pairs = printed_mappings(&help);
    assert!(
        pairs.is_empty(),
        "the help still teaches label rewrites, but v1.2.8 stores every label as \
         written and rewrites none. Printing `a→b` promises a transformation that \
         no longer exists: {pairs:?}"
    );
}

#[test]
fn the_help_states_that_the_vocabulary_is_open() {
    // The single most important fact about this surface, and the one a caller
    // cannot discover from a list of thirteen names that looks exhaustive.
    let help = long_help("remember").to_lowercase();
    assert!(
        help.contains("open"),
        "the help must say the entity vocabulary is open, or the thirteen \
         recommended names read as a closed set"
    );
    assert!(
        help.contains("recommended"),
        "the thirteen must be presented as recommended, not as the only accepted values"
    );
}

#[test]
fn every_recommended_kind_is_named_in_the_help() {
    // A fourteenth kind added to `CANONICAL_ENTITY_TYPES` without a line in the
    // help would be invisible to the only caller who cannot read the source.
    let help = long_help("remember");
    let missing: Vec<&str> = CANONICAL_ENTITY_TYPES
        .iter()
        .copied()
        .filter(|kind| !help.contains(kind))
        .collect();
    assert!(
        missing.is_empty(),
        "recommended entity kinds absent from the help: {missing:?}"
    );
}

#[test]
fn the_help_never_promises_a_refusal_the_parser_does_not_make() {
    let help = long_help("remember");
    let broken: Vec<&str> = CLAIMS_THE_PARSER_REFUTES
        .iter()
        .copied()
        .filter(|claim| help.contains(claim))
        .collect();
    assert!(
        broken.is_empty(),
        "remember --help states a prohibition the code does not enforce: {broken:?}. \
         Non-canonical labels are STORED AS WRITTEN and reported in `warnings`, and \
         the positional name is accepted. Describe what happens, not what stopped \
         happening"
    );
}

#[test]
fn the_help_names_the_way_to_close_the_vocabulary() {
    // Accepting any label is only honest if a caller who wants the old strict
    // behaviour can find it. `--strict-name` set that precedent for the sibling
    // field, and a caller who cannot find the flag does not have the choice.
    let help = long_help("remember");
    assert!(
        help.contains("--strict-entity-types"),
        "an open vocabulary is only honest if the help names the way to close it"
    );
    assert!(
        help.contains("warnings"),
        "the help must say where a non-canonical label is reported"
    );
}

#[test]
fn the_positional_name_the_help_offers_really_exists() {
    // The inverse of the claim test: the help may not deny a form clap defines,
    // and may not offer one clap does not.
    let mut root = sqlite_graphrag::cli::Cli::command();
    let remember = root
        .find_subcommand_mut("remember")
        .expect("`remember` must exist");
    let positionals: Vec<String> = remember
        .get_positionals()
        .map(|a| a.get_id().to_string())
        .collect();
    assert_eq!(
        positionals,
        vec!["name_positional".to_string()],
        "remember must accept exactly the positional its help offers"
    );
}
