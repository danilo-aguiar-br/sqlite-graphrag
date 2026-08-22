//! The version a document ANNOUNCES must be the version being shipped.
//!
//! Measured on 2026-08-21, with `Cargo.toml` at `1.2.8`: fourteen documents
//! under `docs/` opened by announcing another release. `docs/HOW_TO_USE.md`
//! titled itself `v1.2.5`, `docs/TESTING.md` said `v1.0.85`, `docs/TEST_PLAN.md`
//! said `v1.0.79`, and `docs/COOKBOOK.md` managed to disagree with ITSELF —
//! `v1.2.5` in the H1 and `Current crate 1.2.2` in the tagline three lines down.
//!
//! Nothing mechanical connected `Cargo.toml` to those headers, so the drift was
//! invisible: every one of those files is otherwise correct, and a reader has no
//! way to tell a stale banner from a deliberate one.
//!
//! WHY THE HEADER AND NOT THE BODY: a version in the body is usually HISTORY,
//! and history is the content. `docs/MIGRATION.md` walks `v1.2.2 → v1.2.5` on
//! purpose, and a gate that rewrote that would destroy the document. Only the
//! opening banner claims "this is what you are holding", so only it is checked.
//!
//! WHY "MENTIONS THE CURRENT ONE" AND NOT "MENTIONS ONLY IT": a migration guide
//! legitimately names a chain in its header. Demanding a single version would
//! turn a correct document into an offence, and a gate that cries wolf is a gate
//! somebody deletes. The rule is the weakest one that still catches the drift:
//! if a header talks about versions at all, the shipped version must be among
//! them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Lines of a document treated as its announcing header.
///
/// The header ends at the first H2, because that is where the document proper
/// starts. The cap exists for files that never emit an H2.
const HEADER_LINE_CAP: usize = 14;

/// Directories and files whose versions are history by definition.
///
/// Each carries the reason inline. An allowlist without a written reason is how
/// the next exemption gets added silently.
const NOT_ANNOUNCING_A_VERSION: &[(&str, &str)] = &[
    (
        "CHANGELOG.md",
        "a changelog IS the version history; every release is named on purpose",
    ),
    ("CHANGELOG.pt-BR.md", "mirror of the changelog, same reason"),
    (
        "docs/decisions/",
        "an ADR records a decision taken AT a version and must never be restamped",
    ),
    (
        "docs/DOCUMENTATION_FRAMEWORK.md",
        "describes the framework's own revisions, not the product release",
    ),
];

/// The version this build ships.
fn shipped_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// True when `rel` was deliberately excused, per [`NOT_ANNOUNCING_A_VERSION`].
fn is_excused(rel: &str) -> bool {
    NOT_ANNOUNCING_A_VERSION
        .iter()
        .any(|(pattern, _)| rel == *pattern || rel.starts_with(pattern))
}

/// Every Markdown document that opens by announcing a release.
///
/// This WALKS the tree instead of listing paths. A fixed list does not grow
/// when a document is born, so a new document would be born outside the gate —
/// the exact shape that let two other gates in this repository under-measure.
fn announcing_documents() -> Vec<(String, String)> {
    let root = repo_root();
    let mut found = Vec::new();

    let mut stack = vec![root.join("docs")];
    // Root-level Markdown is announced too, but only the top level: nested
    // directories there are not documentation.
    collect_markdown(&root, &root, &mut found);
    while let Some(dir) = stack.pop() {
        collect_markdown(&dir, &root, &mut found);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                }
            }
        }
    }

    found.sort();
    found.dedup();
    found
}

fn collect_markdown(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if is_excused(&rel) {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            out.push((rel, text));
        }
    }
}

/// The opening lines that claim which release the reader is holding.
fn header_of(text: &str) -> String {
    let mut header = String::new();
    for (index, line) in text.lines().enumerate() {
        if index >= HEADER_LINE_CAP || (index > 0 && line.starts_with("## ")) {
            break;
        }
        header.push_str(line);
        header.push('\n');
    }
    header
}

/// Every `MAJOR.MINOR.PATCH` literal in `text`.
///
/// Hand-rolled because a schema version like `v16` and a ratio like `20/20` are
/// NOT releases, and a looser scan reports both. Three dot-separated numeric
/// runs is the narrowest shape that still catches `1.2.5` and `v1.0.85`.
fn versions_in(text: &str) -> BTreeSet<String> {
    let bytes: Vec<char> = text.chars().collect();
    let mut found = BTreeSet::new();
    let mut i = 0;

    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        // A digit preceded by a digit or dot is mid-number; skip to the end.
        if i > 0 && (bytes[i - 1].is_ascii_digit() || bytes[i - 1] == '.') {
            i += 1;
            continue;
        }
        let start = i;
        let mut parts = 0;
        let mut cursor = i;
        while parts < 3 {
            let digits_start = cursor;
            while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
                cursor += 1;
            }
            if cursor == digits_start {
                break;
            }
            parts += 1;
            if parts < 3 {
                if cursor < bytes.len() && bytes[cursor] == '.' {
                    cursor += 1;
                } else {
                    break;
                }
            }
        }
        if parts == 3 {
            // A fourth NUMERIC component means it is not a release string.
            //
            // The dot alone is not enough: `Current release: v1.2.8.` ends a
            // sentence, and treating that full stop as a fourth component made
            // this gate report both READMEs as stale while they were correct.
            // Measured on 2026-08-21, before the fix: 16 offences, 2 of them
            // false. A gate that cries wolf is a gate somebody deletes.
            let trails_a_number = cursor + 1 < bytes.len()
                && bytes[cursor] == '.'
                && bytes[cursor + 1].is_ascii_digit();
            if !trails_a_number {
                found.insert(bytes[start..cursor].iter().collect::<String>());
            }
        }
        i = cursor.max(start + 1);
    }

    found
}

/// The `.pt-BR` twin of an English document, when the path names one.
fn mirror_of(rel: &str) -> Option<String> {
    rel.strip_suffix(".md")
        .filter(|stem| !stem.ends_with(".pt-BR"))
        .map(|stem| format!("{stem}.pt-BR.md"))
}

#[test]
fn no_document_announces_a_version_it_is_not() {
    let shipped = shipped_version();
    let mut stale = Vec::new();

    for (rel, text) in announcing_documents() {
        let header = header_of(&text);
        let versions = versions_in(&header);
        if versions.is_empty() {
            // A header that never names a release cannot announce a wrong one.
            continue;
        }
        if !versions.contains(shipped) {
            let listed: Vec<&str> = versions.iter().map(String::as_str).collect();
            stale.push(format!(
                "{rel}: header names {} but not {shipped}",
                listed.join(", ")
            ));
        }
    }

    assert!(
        stale.is_empty(),
        "{} document(s) announce a release this build is not:\n{}\n\n\
         The crate ships {shipped}. Fix the H1 and the tagline, NOT the body: a \
         version named in the body is usually history, and history is the \
         content. A header may list several releases — a migration chain is \
         legitimate — as long as {shipped} is among them.",
        stale.len(),
        stale.join("\n")
    );
}

#[test]
fn a_document_and_its_mirror_announce_the_same_releases() {
    let root = repo_root();
    let mut divergent = Vec::new();

    for (rel, text) in announcing_documents() {
        let Some(mirror) = mirror_of(&rel) else {
            continue;
        };
        let mirror_path = root.join(&mirror);
        let Ok(mirror_text) = std::fs::read_to_string(&mirror_path) else {
            continue;
        };
        let english = versions_in(&header_of(&text));
        let portuguese = versions_in(&header_of(&mirror_text));
        if english != portuguese {
            divergent.push(format!(
                "{rel} names [{}] but {mirror} names [{}]",
                english.iter().cloned().collect::<Vec<_>>().join(", "),
                portuguese.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    }

    assert!(
        divergent.is_empty(),
        "{} mirrored pair(s) announce different releases:\n{}\n\n\
         Mirrored documents carry the SAME technical content and change in the \
         SAME delivery. Measured on 2026-08-21: docs/TESTING.md said v1.0.85 \
         while docs/TESTING.pt-BR.md said v1.0.83, so the two halves of one \
         document disagreed about which product they described.",
        divergent.len(),
        divergent.join("\n")
    );
}

#[test]
fn the_version_scanner_separates_a_release_from_a_schema_number() {
    let found = versions_in("crate 1.2.8, schema v16, offline gate 20/20, dim 1024");
    assert!(found.contains("1.2.8"));
    assert_eq!(found.len(), 1, "only the release is a release: {found:?}");
}

#[test]
fn the_version_scanner_reads_a_v_prefixed_release() {
    let found = versions_in("# HOW TO USE sqlite-graphrag (v1.2.5 — agent-native)");
    assert!(
        found.contains("1.2.5"),
        "the v prefix must not hide the release"
    );
}

#[test]
fn a_full_stop_after_a_release_is_not_a_fourth_component() {
    let found = versions_in("Current release: v1.2.8. Standing contract.");
    assert!(
        found.contains("1.2.8"),
        "a sentence-ending period must not hide the release: {found:?}"
    );
}

#[test]
fn the_version_scanner_rejects_a_four_part_number() {
    let found = versions_in("build 1.2.3.4 is not a release");
    assert!(
        found.is_empty(),
        "a four-part number is not semver: {found:?}"
    );
}

#[test]
fn the_header_stops_at_the_first_section() {
    let doc = "# Title v1.0.0\n\n> tagline\n\n## Body\n\nshipped in v9.9.9\n";
    let found = versions_in(&header_of(doc));
    assert!(found.contains("1.0.0"));
    assert!(
        !found.contains("9.9.9"),
        "a version below the first H2 is body, not announcement: {found:?}"
    );
}

#[test]
fn the_walk_reaches_the_corpus_it_is_supposed_to_read() {
    let docs = announcing_documents();
    let names: BTreeSet<&str> = docs.iter().map(|(rel, _)| rel.as_str()).collect();

    for expected in ["README.md", "docs/HOW_TO_USE.md", "docs/COOKBOOK.md"] {
        assert!(names.contains(expected), "the walk missed {expected}");
    }
    assert!(
        !names.iter().any(|rel| rel.starts_with("docs/decisions/")),
        "ADRs are excused and must not be walked"
    );
    assert!(
        !names.contains("CHANGELOG.md"),
        "a changelog IS version history and must not be restamped"
    );
}
