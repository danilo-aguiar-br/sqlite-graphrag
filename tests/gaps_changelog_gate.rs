//! A gap closed in this release must be announced in BOTH changelogs.
//!
//! GAP-SG-297. The repository already guards the direction `CHANGELOG -> gaps.md`
//! and the internal consistency of each file. Nothing guarded the direction that
//! actually leaks: an entry marked RESOLVED that never reaches the changelog.
//!
//! Measured on 2026-08-21 while auditing v1.2.8: of the 46 entries marked
//! resolved in this release, THREE appeared in neither changelog — GAP-SG-259,
//! GAP-SG-260 and GAP-SG-293. Then, within the same session, the audit itself
//! opened GAP-SG-294, GAP-SG-295 and GAP-SG-296 and left all three in exactly
//! that state. Accuser and defendant by the same mechanism, hours apart.
//!
//! THE MECHANISM IS NOT CARELESSNESS. A gap is written at the moment of
//! DISCOVERY and a changelog entry at the moment of RELEASE. Those are different
//! phases of the work with different context open. Every gap discovered near the
//! end of a release is born with this debt, because that release's changelog
//! section was written before the gap existed. No amount of discipline closes
//! that reliably; only a cross-check does.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// Marks the beginning of an entry in `gaps.md`.
const ENTRY_HEADING: &str = "## GAP-";

/// The bullet carrying the verdict. Only the FIRST one per entry counts:
/// resolved entries preserve their original diagnosis below, and that history
/// carries `- Status:` bullets of its own.
const STATUS_BULLET: &str = "- Status:";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The release this build ships, spelled the way `gaps.md` spells it.
fn shipped_release() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

/// The identifier a `## GAP-...` heading opens, without the title.
fn identifier_of(heading: &str) -> Option<&str> {
    let rest = heading.strip_prefix("## ")?;
    let end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .unwrap_or(rest.len());
    let id = &rest[..end];
    id.starts_with("GAP-").then_some(id)
}

/// Every gap whose FIRST status bullet claims this release closed it.
///
/// Anchored to the ENTRY rather than scanned line by line: a resolved entry
/// keeps its original diagnosis, so `rg -c` over the whole file over-counts.
fn closed_in_this_release() -> BTreeSet<String> {
    let text = read_repo_file("gaps.md");
    let release = shipped_release();
    let mut closed = BTreeSet::new();

    let mut current: Option<String> = None;
    let mut seen_status_for_current = false;

    for line in text.lines() {
        if line.starts_with(ENTRY_HEADING) {
            current = identifier_of(line).map(str::to_string);
            seen_status_for_current = false;
            continue;
        }
        let trimmed = line.trim_start();
        if !trimmed.starts_with(STATUS_BULLET) || seen_status_for_current {
            continue;
        }
        seen_status_for_current = true;
        let Some(id) = current.as_ref() else { continue };
        // "RESOLVIDO na v1.2.8 e verificado" and its variants. An entry that is
        // merely MENTIONED alongside the release ("regression of v1.2.8") is not
        // caught, because the verdict word has to be there too.
        let resolved = trimmed.contains("RESOLVID") || trimmed.contains("RESOLVED");
        if resolved && trimmed.contains(&release) {
            closed.insert(id.clone());
        }
    }

    closed
}

#[test]
fn every_gap_closed_in_this_release_is_announced_in_both_changelogs() {
    let closed = closed_in_this_release();
    assert!(
        closed.len() >= 10,
        "read only {} gaps closed in {}; the entry scanner probably stopped \
         matching the status format in gaps.md",
        closed.len(),
        shipped_release()
    );

    let english = read_repo_file("CHANGELOG.md");
    let portuguese = read_repo_file("CHANGELOG.pt-BR.md");

    let mut missing = Vec::new();
    for id in &closed {
        let in_en = english.contains(id.as_str());
        let in_pt = portuguese.contains(id.as_str());
        match (in_en, in_pt) {
            (true, true) => {}
            (false, false) => missing.push(format!("{id}: absent from BOTH changelogs")),
            (false, true) => missing.push(format!("{id}: absent from CHANGELOG.md")),
            (true, false) => missing.push(format!("{id}: absent from CHANGELOG.pt-BR.md")),
        }
    }

    assert!(
        missing.is_empty(),
        "{} gap(s) marked resolved in {} never reached the changelog:\n{}\n\n\
         Closing a gap takes TWO writes, never one: the entry status and the \
         changelog line are the same transaction. Write the entry in BOTH \
         languages, describing the defect recorded in gaps.md rather than a \
         paraphrase, and spell the identifier literally so this cross-check \
         can see it.",
        missing.len(),
        shipped_release(),
        missing.join("\n")
    );
}

#[test]
fn the_entry_scanner_reads_only_the_first_status_of_an_entry() {
    // A resolved entry preserves its original diagnosis, and that history
    // carries status bullets of its own. Counting them all would report a
    // reopened gap as closed and a closed one twice.
    let closed = closed_in_this_release();
    let distinct: BTreeSet<&String> = closed.iter().collect();
    assert_eq!(
        closed.len(),
        distinct.len(),
        "the same identifier was collected twice"
    );
}

#[test]
fn the_identifier_parser_stops_at_the_title() {
    assert_eq!(
        identifier_of("## GAP-SG-294 — a matriz de cobertura estava atrasada"),
        Some("GAP-SG-294")
    );
    assert_eq!(
        identifier_of("## GAP-CLI-GRAPH-01 — algo"),
        Some("GAP-CLI-GRAPH-01")
    );
    assert_eq!(identifier_of("## Some other heading"), None);
    assert_eq!(identifier_of("### GAP-SG-1 nested"), None);
}

#[test]
fn the_release_string_matches_how_gaps_md_spells_it() {
    let release = shipped_release();
    assert!(release.starts_with('v'), "gaps.md writes the v prefix");
    assert!(
        read_repo_file("gaps.md").contains(&release),
        "no entry mentions {release}; either the release just opened or the \
         spelling in gaps.md drifted away from Cargo.toml"
    );
}
