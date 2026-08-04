//! Keeps every number a document declares about itself equal to the number the
//! repository actually holds.
//!
//! The documents in this project do not only describe behaviour, they COUNT it:
//! "61 keys", "64 schemas", "crate `version = \"1.2.2\"`". Each of those is a
//! claim an operator can act on, and each was true on the day it was typed.
//! Nothing recomputed them afterwards, so by v1.2.5 the registry held 63 keys
//! while both READMEs still announced 61, `docs/schemas/` held 74 files while
//! both READMEs still announced 64, and `llms.txt` — the file an LLM agent
//! reads FIRST — still introduced itself as v1.2.2 next to a crate shipping
//! 1.2.5.
//!
//! A stale count is worse than a missing one. A missing count sends the reader
//! to the source; a wrong count answers the question confidently and sends the
//! reader nowhere.
//!
//! The three sibling guards each cover a different axis and none covers this
//! one: `docs_command_coverage` asserts every command is NAMED, `docs_xdg_coverage`
//! asserts every key APPEARS, `docs_language_parity` asserts the two languages
//! have the same SHAPE. A document can satisfy all three while announcing the
//! wrong total, because "appears" and "counted correctly" are different claims.
//!
//! Like its siblings, this is a test rather than a CI job because this project
//! forbids CI by design; `cargo test` is the only automatic gate.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Documents that may declare a self-describing count or version.
///
/// This is every operator-facing document plus the two `llms*.txt` agent
/// manifests. A document absent from this list is not exempt from being
/// correct — it simply has never carried a declared total, and adding one
/// there should also add it here.
const DECLARING_DOCS: &[&str] = &[
    "README.md",
    "README.pt-BR.md",
    "llms.txt",
    "llms.pt-BR.txt",
    "llms-full.txt",
    "docs/AGENTS.md",
    "docs/AGENTS.pt-BR.md",
    "docs/HOW_TO_USE.md",
    "docs/HOW_TO_USE.pt-BR.md",
    "docs/COOKBOOK.md",
    "docs/COOKBOOK.pt-BR.md",
    "docs/HEADLESS_INVOCATION.md",
    "docs/HEADLESS_INVOCATION.pt-BR.md",
    "INTEGRATIONS.md",
    "INTEGRATIONS.pt-BR.md",
];

/// Marker that opens a registry entry in `src/config/registry.rs`.
const ENTRY_MARKER: &str = "SettingKey {";

/// Field that carries the key name inside a registry entry.
const KEY_FIELD: &str = "key: \"";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Counts the config keys the binary actually accepts.
///
/// Parsed from the registry rather than from `--help`, because the registry is
/// the single place that decides whether `config set` answers 0 or 1.
fn registry_key_count() -> usize {
    let source = read_repo_file("src/config/registry.rs");
    let mut keys = BTreeSet::new();
    for entry in source.split(ENTRY_MARKER).skip(1) {
        let Some(start) = entry.find(KEY_FIELD) else {
            continue;
        };
        let rest = &entry[start + KEY_FIELD.len()..];
        let Some(end) = rest.find('"') else {
            continue;
        };
        keys.insert(rest[..end].to_string());
    }
    keys.len()
}

/// Counts the JSON schema files shipped under `docs/schemas/`.
fn schema_file_count() -> usize {
    let dir = repo_root().join("docs/schemas");
    let entries = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(Result::ok);
    entries
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .count()
}

/// Pulls the integer that immediately precedes `noun` in `line`.
///
/// Returns every occurrence, because one line can carry more than one claim.
/// The scan walks words rather than applying a regex so the two languages share
/// one code path: only the noun differs between "64 schemas cover" and
/// "64 schemas cobrem".
///
/// A number written with a trailing `+` is an approximation, not a total:
/// "regenerates 70+ schemas" stays true as the directory grows, so counting it
/// as a claim would make this guard fail on a sentence that is correct. Those
/// are skipped.
fn counts_before(line: &str, noun: &str) -> Vec<usize> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut found = Vec::new();
    for (i, word) in words.iter().enumerate() {
        let bare = word.trim_matches(|c: char| !c.is_alphanumeric());
        if bare != noun || i == 0 {
            continue;
        }
        let previous = words[i - 1];
        if previous.trim_end_matches(['.', ',']).ends_with('+') {
            continue;
        }
        let digits = previous.trim_matches(|c: char| !c.is_ascii_digit());
        if let Ok(n) = digits.parse::<usize>() {
            found.push(n);
        }
    }
    found
}

/// Marks a line as claiming the CATALOGUE total rather than a subset.
///
/// `docs/AGENTS.pt-BR.md` reports "7 schemas JSON atualizados" while listing
/// the seven files one bug touched. That is a true sentence about a subset, and
/// a guard that read it as a total would be demanding the documentation lie.
/// The catalogue claim is the one that says the schemas COVER the surface.
fn claims_the_schema_catalogue(line: &str) -> bool {
    line.contains("schemas cover") || line.contains("schemas cobrem")
}

/// Words that mark a version as the one shipping RIGHT NOW.
///
/// Without one of these the sentence is a historical label. `INTEGRATIONS.md`
/// writes "Official release name v1.1.06; crate `version = \"1.1.6\"`" as a
/// record of what that release carried, which was accurate then and stays
/// accurate now.
const CURRENCY_MARKERS: &[&str] = &["current", "atual"];

/// Phrases that introduce the release a document presents as the shipped one.
///
/// The version that FOLLOWS one of these on the same line is a currency claim,
/// so it must equal the manifest. A version reached any other way is a
/// historical anchor ("same since v1.2.0", "added in v1.0.90") and is left
/// alone — the record of an old release stays accurate forever.
///
/// The first version of this gate anchored only on the ``crate `version = "…"``
/// spelling of `llms.txt`, which is why it shipped green while six documents
/// introduced v1.2.2 as current three releases after v1.2.5. The lesson is in
/// the shape of the guard, not the documents: a scanner that recognises one
/// phrasing asserts far less than its name promises.
const CURRENCY_PHRASES: &[&str] = &[
    "current release",
    "current version",
    "release atual",
    "versão atual",
    "versao atual",
    // `current (` alone covers `CURRENT (v1.2.2; …)`: the version reader skips
    // the `v`. Listing both spellings would report the same line twice.
    "current (",
];

/// Words that turn the version after a currency phrase into an origin anchor.
const ORIGIN_QUALIFIERS: &[&str] = &["since", "desde", "same since"];

/// Openers that mark a version written BEFORE its currency marker.
///
/// `INTEGRATIONS.md` writes `**v1.2.2 (current):**` and `llms.txt` writes
/// `**v1.2.5** (current — crate …)`: the claim is identical to
/// `Current release: v1.2.5`, only the word order is reversed. Reading only
/// one order is how this gate missed six documents the first time; reading
/// only two is how it would miss the next one.
const TRAILING_CURRENCY: &[&str] = &["(current", "(atual"];

/// Reads the last `MAJOR.MINOR.PATCH` that ENDS at or before `upto`.
fn last_version_before(line: &str, upto: usize) -> Option<String> {
    let head = &line[..upto];
    let bytes = head.as_bytes();
    let mut end = bytes.len();
    while end > 0 {
        if !bytes[end - 1].is_ascii_digit() {
            end -= 1;
            continue;
        }
        let mut start = end;
        let mut dots = 0;
        while start > 0 && (bytes[start - 1].is_ascii_digit() || bytes[start - 1] == b'.') {
            if bytes[start - 1] == b'.' {
                dots += 1;
            }
            start -= 1;
        }
        let candidate = head[start..end].trim_matches('.');
        if dots >= 2 && candidate.split('.').count() == 3 {
            return Some(candidate.to_string());
        }
        end = start;
    }
    None
}

/// Reads the first `MAJOR.MINOR.PATCH` at or after `from`, ignoring a `v`
/// prefix. Returns `None` when the tail holds no version at all.
fn first_version_after(line: &str, from: usize) -> Option<String> {
    let tail = &line[from..];
    let bytes = tail.as_bytes();
    for start in 0..bytes.len() {
        if !bytes[start].is_ascii_digit() {
            continue;
        }
        if start > 0 && (bytes[start - 1].is_ascii_digit() || bytes[start - 1] == b'.') {
            continue;
        }
        let mut end = start;
        let mut dots = 0;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            if bytes[end] == b'.' {
                dots += 1;
            }
            end += 1;
        }
        let candidate = tail[start..end].trim_end_matches('.');
        if dots >= 2 && candidate.split('.').count() == 3 {
            return Some(candidate.to_string());
        }
    }
    None
}

/// Extracts every version a document declares as the crate's version TODAY.
///
/// Two shapes count, because the corpus uses both. The explicit manifest
/// spelling ``crate `version = "…"`` on a line that also claims currency, and
/// any of [`CURRENCY_PHRASES`] followed by a version. A line carrying neither
/// is prose or history and is skipped.
fn declared_current_versions(text: &str) -> Vec<(usize, String)> {
    const ANCHOR: &str = "crate `version = \"";
    let mut found: Vec<(usize, String)> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let lowered = line.to_lowercase();

        if CURRENCY_MARKERS.iter().any(|m| lowered.contains(m)) {
            let mut rest = line;
            while let Some(at) = rest.find(ANCHOR) {
                rest = &rest[at + ANCHOR.len()..];
                if let Some(end) = rest.find('"') {
                    found.push((n + 1, rest[..end].to_string()));
                }
            }
        }

        for phrase in CURRENCY_PHRASES {
            let Some(at) = lowered.find(phrase) else {
                continue;
            };
            let from = at + phrase.len();
            // `CURRENT (since v1.2.0)` names where a behaviour STARTED, not
            // which release ships. A version qualified this way is an origin
            // anchor and stays accurate forever; flagging it would push authors
            // to delete a true statement to silence the gate.
            if ORIGIN_QUALIFIERS
                .iter()
                .any(|q| lowered[from..].trim_start().starts_with(q))
            {
                continue;
            }
            if let Some(v) = first_version_after(line, from) {
                found.push((n + 1, v));
            }
        }

        for opener in TRAILING_CURRENCY {
            let mut at = 0;
            while let Some(rel) = lowered[at..].find(opener) {
                let hit = at + rel;
                // `(current — crate …)` and `(current):` both qualify; `(since`
                // does not reach here because it is not a currency opener.
                if let Some(v) = last_version_before(line, hit) {
                    found.push((n + 1, v));
                }
                at = hit + opener.len();
            }
        }
    }

    // One line can state the same claim in two shapes at once — `v1.2.5
    // (current — crate `version = "1.2.5"`)` matches both the manifest anchor
    // and the trailing marker. Reporting it twice would inflate the failure
    // list without naming a second defect.
    {
        let mut unique = Vec::new();
        for item in found {
            if !unique.contains(&item) {
                unique.push(item);
            }
        }
        found = unique;
    }
    found
}

fn existing_docs() -> Vec<(&'static str, String)> {
    DECLARING_DOCS
        .iter()
        .filter(|rel| repo_root().join(rel).exists())
        .map(|rel| (*rel, read_repo_file(rel)))
        .collect()
}

// ---------------------------------------------------------------------------
// The guards
// ---------------------------------------------------------------------------

#[test]
fn the_gate_found_the_documents_it_is_supposed_to_read() {
    let docs = existing_docs();
    assert!(
        docs.len() >= 10,
        "the scan resolved only {} of {} declaring documents — the path list is \
         broken and every other test in this file is passing on an empty set",
        docs.len(),
        DECLARING_DOCS.len()
    );
}

#[test]
fn no_document_declares_a_crate_version_other_than_the_shipped_one() {
    let shipped = env!("CARGO_PKG_VERSION");
    let mut wrong = Vec::new();

    for (doc, text) in existing_docs() {
        for (line, declared) in declared_current_versions(&text) {
            if declared != shipped {
                wrong.push(format!("{doc}:{line} introduces {declared} as current"));
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "the crate ships {shipped}, but {} document(s) introduce it as another \
         version; an agent reading the manifest first believes the wrong \
         surface:\n  {}",
        wrong.len(),
        wrong.join("\n  ")
    );
}

#[test]
fn every_declared_key_count_matches_the_registry() {
    let real = registry_key_count();
    let mut wrong = Vec::new();

    for (doc, text) in existing_docs() {
        for (n, line) in text.lines().enumerate() {
            for noun in ["keys", "chaves"] {
                for claimed in counts_before(line, noun) {
                    // Only a claim about the `config set` surface is this
                    // guard's business; "10 keys" inside an unrelated example
                    // is not a total.
                    if !line.contains("config set") {
                        continue;
                    }
                    if claimed != real {
                        wrong.push(format!("{doc}:{} claims {claimed} {noun}", n + 1));
                    }
                }
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "src/config/registry.rs holds {real} keys, but {} declaration(s) \
         disagree:\n  {}",
        wrong.len(),
        wrong.join("\n  ")
    );
}

#[test]
fn every_declared_schema_count_matches_the_directory() {
    let real = schema_file_count();
    let mut wrong = Vec::new();

    for (doc, text) in existing_docs() {
        for (n, line) in text.lines().enumerate() {
            if !claims_the_schema_catalogue(line) {
                continue;
            }
            for claimed in counts_before(line, "schemas") {
                if claimed != real {
                    wrong.push(format!("{doc}:{} claims {claimed} schemas", n + 1));
                }
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "docs/schemas/ holds {real} JSON schema file(s), but {} declaration(s) \
         disagree:\n  {}",
        wrong.len(),
        wrong.join("\n  ")
    );
}

/// Reads the release headings of a changelog in the order they appear.
///
/// Only `## [x.y.z] - date` headings count. `## [Legacy NeuroGraphRAG]` is a
/// named era rather than a release and carries no version to compare.
fn release_order(changelog: &str) -> Vec<String> {
    let mut order = Vec::new();
    for line in changelog.lines() {
        let Some(rest) = line.strip_prefix("## [") else {
            continue;
        };
        let Some(end) = rest.find(']') else {
            continue;
        };
        let version = &rest[..end];
        if version.starts_with(|c: char| c.is_ascii_digit()) {
            order.push(version.to_string());
        }
    }
    order
}

#[test]
fn both_changelogs_list_their_releases_in_the_same_order() {
    let en = release_order(&read_repo_file("CHANGELOG.md"));
    let pt = release_order(&read_repo_file("CHANGELOG.pt-BR.md"));

    assert!(
        en.len() > 50,
        "the changelog scan found {} releases, too few to be the real file",
        en.len()
    );

    assert_eq!(
        en.len(),
        pt.len(),
        "CHANGELOG.md lists {} releases and CHANGELOG.pt-BR.md lists {}",
        en.len(),
        pt.len()
    );

    // Compared position by position rather than as sets. The two files held the
    // same 128 releases while `[1.2.4]` sat between the 1.0.7x entries in the
    // Portuguese file only, so a set comparison — and the section-count guard
    // in `docs_language_parity` — both passed on a reader-visible defect.
    let first_divergence = en.iter().zip(pt.iter()).position(|(a, b)| a != b);
    assert!(
        first_divergence.is_none(),
        "the two changelogs diverge at position {}: EN has {} where pt-BR has {}",
        first_divergence.unwrap(),
        en[first_divergence.unwrap()],
        pt[first_divergence.unwrap()]
    );
}

// ---------------------------------------------------------------------------
// The scanners are themselves tested, so a silent scanner cannot fake a pass
// ---------------------------------------------------------------------------

#[test]
fn the_count_scanner_reads_the_number_that_precedes_the_noun() {
    assert_eq!(
        counts_before("- 64 schemas cover `init`", "schemas"),
        vec![64]
    );
    assert_eq!(
        counts_before("### key reference (61 keys, v1.2.5)", "keys"),
        vec![61]
    );
}

#[test]
fn the_count_scanner_ignores_a_noun_with_no_leading_number() {
    assert!(counts_before("the schemas live under docs/", "schemas").is_empty());
    assert!(counts_before("schemas cover everything", "schemas").is_empty());
}

#[test]
fn the_count_scanner_treats_a_trailing_plus_as_an_approximation() {
    // `dump-schema regenerates 70+ schemas` stays true as the directory grows.
    assert!(counts_before("regenerates 70+ schemas", "schemas").is_empty());
    assert!(counts_before("regenera 70+ schemas", "schemas").is_empty());
}

#[test]
fn the_catalogue_filter_separates_a_total_from_a_subset() {
    assert!(claims_the_schema_catalogue(
        "- 64 schemas cover `init`, `remember`"
    ));
    assert!(claims_the_schema_catalogue(
        "- 64 schemas cobrem `init`, `remember`"
    ));
    // A bug that touched seven files is not a claim about the catalogue.
    assert!(!claims_the_schema_catalogue(
        "BUG-15: 7 schemas JSON atualizados: enum expandida"
    ));
}

/// Collects only the versions, discarding line numbers, for the scanner tests.
fn versions_only(text: &str) -> Vec<String> {
    declared_current_versions(text)
        .into_iter()
        .map(|(_, v)| v)
        .collect()
}

#[test]
fn the_version_scanner_requires_a_currency_marker() {
    let now = "v1.2.2 (current — crate `version = \"1.2.2\"`; and v1.0.90 added x)";
    assert_eq!(versions_only(now), vec!["1.2.2".to_string()]);

    let now_pt = "v1.2.2 (atual — crate `version = \"1.2.2\"`)";
    assert_eq!(versions_only(now_pt), vec!["1.2.2".to_string()]);

    // A historical record of what a past release carried is not a claim about
    // the version shipping today.
    let history = "Official release name v1.1.06; crate `version = \"1.1.6\"` — pin `=1.1.6`.";
    assert!(versions_only(history).is_empty());

    assert!(versions_only("v1.0.90 added the opencode backend").is_empty());
}

#[test]
fn the_version_scanner_reads_every_currency_phrasing_in_the_corpus() {
    // The three spellings that shipped past the first version of this gate.
    assert_eq!(
        versions_only("> **Current release: v1.2.2 — agent-native output surface**"),
        vec!["1.2.2".to_string()]
    );
    assert_eq!(
        versions_only("> **Release atual: v1.2.2 — superfície agent-native**"),
        vec!["1.2.2".to_string()]
    );
    assert_eq!(
        versions_only("- Current version **1.2.2** (crate `1.2.2`; inherits schema v16)"),
        vec!["1.2.2".to_string()]
    );
    assert_eq!(
        versions_only("- Versão atual **1.2.2** (crate `1.2.2`; herda schema v16)"),
        vec!["1.2.2".to_string()]
    );

    // A currency phrase followed by a version, then an older anchor: only the
    // first counts, because the tail is a since-when statement.
    assert_eq!(
        versions_only("> **CURRENT (v1.2.2; same since v1.2.0):** DEFAULT_EMBEDDING_DIM=1024"),
        vec!["1.2.2".to_string()]
    );

    // The version can precede its marker; the claim is the same either way.
    assert_eq!(
        versions_only("> **v1.2.2 (current):** agent-native output surface"),
        vec!["1.2.2".to_string()]
    );
    assert_eq!(
        versions_only("- **v1.2.2 (atual):** superfície de saída agent-native"),
        vec!["1.2.2".to_string()]
    );
    assert_eq!(
        versions_only("sqlite-graphrag **v1.2.5** (current — crate `1.2.5`)"),
        vec!["1.2.5".to_string()]
    );

    // An origin anchor survives every release and must not be flagged.
    assert!(versions_only("> **CURRENT (since v1.2.0):** DEFAULT_EMBEDDING_DIM=1024").is_empty());
    assert!(versions_only("> **ATUAL (desde v1.2.0):** dim default 1024").is_empty());

    // A schema number is not a crate version: two dots are required.
    assert!(versions_only("A versão atual de schema é 13").is_empty());
    // A currency phrase with no version at all yields nothing.
    assert!(versions_only("The current release ships no local model").is_empty());
}

#[test]
fn the_version_reader_stops_at_the_first_complete_triple() {
    assert_eq!(
        first_version_after("x v1.2.5 y", 0).as_deref(),
        Some("1.2.5")
    );
    // A trailing dot belongs to the sentence, not to the version.
    assert_eq!(
        first_version_after("ships 1.2.5.", 0).as_deref(),
        Some("1.2.5")
    );
    // Two segments are a schema or a minor pair, never a crate version.
    assert_eq!(first_version_after("v1.2 only", 0), None);
    assert_eq!(first_version_after("no digits here", 0), None);
}

#[test]
fn the_registry_and_schema_counters_return_a_plausible_repository() {
    assert!(
        registry_key_count() > 40,
        "the registry scan found {} keys, too few to be the real registry",
        registry_key_count()
    );
    assert!(
        schema_file_count() > 40,
        "the schema scan found {} files, too few to be the real directory",
        schema_file_count()
    );
    assert!(Path::new(&repo_root().join("Cargo.toml")).exists());
}
