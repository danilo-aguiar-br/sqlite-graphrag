//! GAP-SG-205: every document ships in two languages, and nothing held them
//! level.
//!
//! `tests/i18n_bilingual_integration.rs` proves the ERROR MESSAGES are paired.
//! `tests/docs_consistency.rs` guards `gaps.md`. Neither looks at the
//! `X.md` / `X.pt-BR.md` pairs, so a section could be added, corrected or
//! retired on one side and quietly skipped on the other. Nine pairs had
//! drifted when this was written, and one of them was not cosmetic:
//! `CROSS_PLATFORM.pt-BR.md` still described an active CI job in a project
//! that forbids CI, because the English side had been corrected and the
//! Portuguese side had not.
//!
//! # Why the count and not the text
//!
//! The headings are TRANSLATED — "Platform contracts" against "Contratos de
//! plataforma" — so a textual diff is noise by construction. What must match is
//! the STRUCTURE: same number of `##` sections, in the same order, so a section
//! present in one language has a counterpart in the other. That is mechanical,
//! it has no opinion about translation quality, and it catches exactly the
//! failure mode above.

use std::path::{Path, PathBuf};

/// Repository root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Counts the `##` headings of a markdown file, ignoring fenced code blocks.
///
/// A `## ` inside a fence is shell output or sample markdown, not a section of
/// this document. Counting it would make the guard fail on a correct pair.
fn section_count(markdown: &str) -> usize {
    let mut inside_fence = false;
    let mut count = 0;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            inside_fence = !inside_fence;
            continue;
        }
        if inside_fence {
            continue;
        }
        // `##` exactly: `###` is a subsection and pairs are allowed to differ
        // in how deeply they subdivide a section.
        if line.starts_with("## ") {
            count += 1;
        }
    }
    count
}

/// Pairs whose section counts are allowed to differ, each with its reason.
///
/// An allowlist entry is a claim that has to survive being read out loud, and
/// it has to survive the repository changing under it. The earlier reason here
/// said the English COOKBOOK duplicated 68 sections verbatim, which was true
/// when written and stopped being true the moment those duplicates were
/// removed. A stale justification is worse than no allowlist: the hole stays
/// open while the sentence explaining it no longer describes anything.
///
/// Re-measured after the deduplication: English carries 96 unique sections in
/// 146 184 bytes, Portuguese 82 sections in 152 983 bytes. The Portuguese file
/// is the LARGER of the two while holding fourteen fewer headings, so the gap
/// is how each side partitions the same material, not material the Portuguese
/// side lacks.
const KNOWN_ASYMMETRIES: &[(&str, &str)] = &[(
    "docs/COOKBOOK",
    "Portuguese holds 14 fewer headings over MORE bytes (152 983 against \
     146 184), so the two sides partition the same material differently; the \
     English duplication that once explained this entry was removed and the \
     asymmetry survived it",
)];

/// Document stems that ship in both languages.
fn bilingual_stems() -> Vec<String> {
    let mut out = Vec::new();
    let candidates = [
        "README",
        "CHANGELOG",
        "CONTRIBUTING",
        "SECURITY",
        "INTEGRATIONS",
        "docs/AGENTS",
        "docs/COOKBOOK",
        "docs/CROSS_PLATFORM",
        "docs/HEADLESS_INVOCATION",
        "docs/HOW_TO_USE",
        "docs/MIGRATION",
        "docs/TESTING",
    ];
    for stem in candidates {
        let en = root().join(format!("{stem}.md"));
        let pt = root().join(format!("{stem}.pt-BR.md"));
        if en.exists() && pt.exists() {
            out.push(stem.to_string());
        }
    }
    out
}

#[test]
fn the_gate_found_the_pairs_it_is_supposed_to_read() {
    let stems = bilingual_stems();
    assert!(
        stems.len() >= 10,
        "only {} bilingual pairs found, so the list below went stale and every \
         assertion would pass by not looking: {stems:?}",
        stems.len()
    );
}

#[test]
fn every_bilingual_pair_has_the_same_section_count() {
    let mut drifted = Vec::new();

    for stem in bilingual_stems() {
        if KNOWN_ASYMMETRIES.iter().any(|(known, _)| *known == stem) {
            continue;
        }
        let en = std::fs::read_to_string(root().join(format!("{stem}.md")))
            .unwrap_or_else(|e| panic!("{stem}.md unreadable: {e}"));
        let pt = std::fs::read_to_string(root().join(format!("{stem}.pt-BR.md")))
            .unwrap_or_else(|e| panic!("{stem}.pt-BR.md unreadable: {e}"));

        let (en_count, pt_count) = (section_count(&en), section_count(&pt));
        if en_count != pt_count {
            drifted.push(format!(
                "{stem}: EN has {en_count} sections, pt-BR has {pt_count}"
            ));
        }
    }

    assert!(
        drifted.is_empty(),
        "these documents ship in two languages and no longer describe the same \
         product. A section added, corrected or retired on one side and skipped \
         on the other is how `CROSS_PLATFORM.pt-BR.md` kept advertising a CI job \
         this project forbids, months after the English side was fixed.\n\
         Translate the missing section, or add the pair to KNOWN_ASYMMETRIES \
         with a reason that survives being read out loud.\n{}",
        drifted.join("\n")
    );
}

#[test]
fn every_allowlisted_asymmetry_still_names_a_real_pair() {
    // An allowlist that outlives its subject is worse than no allowlist: it
    // silently exempts a file that may have been renamed onto a real gap.
    for (stem, reason) in KNOWN_ASYMMETRIES {
        assert!(
            root().join(format!("{stem}.md")).exists()
                && root().join(format!("{stem}.pt-BR.md")).exists(),
            "KNOWN_ASYMMETRIES names `{stem}`, which is not a bilingual pair \
             anymore; remove the entry"
        );
        assert!(
            reason.len() > 40,
            "the exemption for `{stem}` needs a reason someone can check, not a \
             label"
        );
    }
}

#[test]
fn the_section_counter_ignores_headings_inside_a_fence() {
    let sample = "## Real\ntext\n```\n## Not a section\n```\n## Also real\n";
    assert_eq!(
        section_count(sample),
        2,
        "a `##` inside a code fence is sample output, not a section of the \
         document; counting it would fail a correct pair"
    );
}

#[test]
fn the_section_counter_ignores_deeper_headings() {
    let sample = "## Section\n### Subsection\n#### Deeper\n## Second\n";
    assert_eq!(section_count(sample), 2);
}
