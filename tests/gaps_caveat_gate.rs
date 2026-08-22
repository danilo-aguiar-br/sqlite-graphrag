//! A closed entry that admits a leftover must say where the leftover lives.
//!
//! `gaps.md` opens with the rule this gate enforces: `documentar NÃO é resolver`.
//! An earlier round read that rule as "reject any status carrying a caveat" and
//! backed off, correctly: `RESOLVIDO na v1.2.2 com resíduo aberto` on GAP-SG-141
//! is legitimate, because the very next line says the leftover is tracked in
//! GAP-SG-156. Flagging it would have made the gate cry wolf, so
//! `docs_consistency::is_outstanding` deliberately reads only the leading word.
//!
//! That left the real failure uncovered. GAP-SG-162 closed as `RESOLVIDO na
//! v1.2.3 com restrição técnica declarada`, declared three acceptance criteria,
//! and stated in its own body that the first was never executed and the second
//! still fails. Nothing anywhere tracks either. The entry is honest — it says
//! "Limite honesto" — and still leaves two criteria owned by no one.
//!
//! So the checkable rule is not "no caveats". It is: a caveat must name its
//! destination. That distinguishes the two entries by exactly the property that
//! makes one acceptable and the other not, and it needs no judgement about
//! whether the leftover was important.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Heading that opens an entry.
///
/// Since GAP-SG-233 this prefix covers the WHOLE document: the 27 `### Erro N`
/// sections that used to sit outside it were migrated into canonical entries,
/// so a heading this gate cannot read is now a defect rather than the norm.
const ENTRY_HEADING: &str = "## GAP-SG-";

/// Level-two headings that carry structure instead of an entry.
///
/// Matched as PREFIXES, because the `PARTE` headings carry a subtitle that is
/// free to change. Anything at level two that is neither one of these nor a
/// `GAP-SG` entry is a heading nobody declared, and
/// [`the_gate_found_the_entries_it_is_supposed_to_read`] says so instead of
/// letting it disappear.
const STRUCTURAL_HEADINGS: &[&str] = &[
    "## Escopo deste documento",
    "## Correção de rumo antes de tudo",
    "## PARTE ",
];

/// Bullet carrying the verdict.
const STATUS_MARKER: &str = "- Status:";

/// Phrases an author uses to admit that closing left something behind.
///
/// Taken from the vocabulary already present in the document rather than
/// invented here, so the gate reads what authors actually write.
const CAVEAT_MARKERS: &[&str] = &[
    "com resíduo",
    "com restrição",
    "restrição técnica declarada",
    "parcialmente",
    "exceto",
];

/// Reads `gaps.md` from the workspace root.
fn read_gaps() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gaps.md");
    std::fs::read_to_string(&path).expect("gaps.md must be readable from the workspace root")
}

/// Splits the document into `(id, body)` pairs, body being everything from the
/// heading up to the next one.
fn entries(gaps: &str) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut buffer = String::new();
    for line in gaps.lines() {
        if line.trim_start().starts_with(ENTRY_HEADING) {
            if let Some(id) = current.take() {
                out.insert(id, std::mem::take(&mut buffer));
            }
            current = line
                .split_whitespace()
                .nth(1)
                .map(|token| token.trim_end_matches('—').trim().to_string());
        }
        buffer.push_str(line);
        buffer.push('\n');
    }
    if let Some(id) = current {
        out.insert(id, buffer);
    }
    out
}

/// The first `- Status:` bullet of an entry; later ones quote history.
fn status_of(body: &str) -> Option<String> {
    body.lines()
        .map(str::trim_start)
        .find_map(|l| l.strip_prefix(STATUS_MARKER))
        .map(|rest| rest.trim().to_string())
}

/// `true` when the status admits something was left behind.
fn admits_a_leftover(status: &str) -> bool {
    let lowered = status.to_lowercase();
    CAVEAT_MARKERS.iter().any(|m| lowered.contains(m))
}

/// Phrases that turn a cross-reference into a DESTINATION rather than an origin.
///
/// Every entry cites other entries — the one it descends from, the one that
/// shares a cause, the one it supersedes. Accepting any reference would let an
/// entry satisfy this gate by naming its own ANCESTOR, which owns nothing going
/// forward. GAP-SG-162 does exactly that: it cites GAP-SG-147 as its origin
/// while its two unmet criteria belong to no one.
const DESTINATION_MARKERS: &[&str] = &[
    "resíduo em",
    "resíduo rastreado em",
    "rastreado em",
    "resíduo extraído para",
    "continua em",
    "transferido para",
];

/// `true` when the entry points its leftover FORWARD at another tracked entry.
///
/// Requires a destination marker and a different id ON THE SAME LINE, so the
/// sentence has to actually say where the leftover went.
fn names_a_destination(id: &str, body: &str) -> bool {
    body.lines().any(|line| {
        let lowered = line.to_lowercase();
        if !DESTINATION_MARKERS.iter().any(|m| lowered.contains(m)) {
            return false;
        }
        line.match_indices("GAP-SG-")
            .filter_map(|(at, _)| line.get(at..at + 10))
            .any(|reference| reference != id)
    })
}

/// Every level-two heading in the raw document.
///
/// Kept independent of [`entries`] on purpose, and since GAP-SG-233 that
/// independence is real rather than nominal. The previous version counted with
/// `ENTRY_HEADING`, the very literal the splitter divides on, so it agreed with
/// the splitter by construction and could not see the document growing 28
/// sections the splitter never read. Scoping on `## ` instead means a heading
/// that matches no known shape has to be accounted for, and the parity
/// assertion below is the only place that decides what "known" means.
///
/// `trim_start` is deliberate: the indented `  # …` that hid a whole half of
/// this document from `^#` searches is exactly the shape a raw prefix match
/// would miss again.
fn level_two_headings(gaps: &str) -> Vec<&str> {
    gaps.lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("## "))
        .collect()
}

#[test]
fn the_gate_found_the_entries_it_is_supposed_to_read() {
    let gaps = read_gaps();
    let parsed = entries(&gaps);

    // Independent census first: every level-two heading is either structure or
    // an entry, and nothing else is tolerated. A heading outside both sets used
    // to be invisible — GAP-SG-233 measured 28 of them.
    let all = level_two_headings(&gaps);
    let undeclared: Vec<&str> = all
        .iter()
        .copied()
        .filter(|line| {
            !line.starts_with(ENTRY_HEADING)
                && !STRUCTURAL_HEADINGS
                    .iter()
                    .any(|known| line.starts_with(known))
        })
        .collect();
    assert!(
        undeclared.is_empty(),
        "these level-two headings are neither a `{ENTRY_HEADING}` entry nor one \
         of the declared structural sections, so every check below skips them \
         in silence. Make each one an entry, or add it to \
         `STRUCTURAL_HEADINGS` with the reason.\n{}",
        undeclared.join("\n")
    );

    let headings = all
        .iter()
        .filter(|line| line.starts_with(ENTRY_HEADING))
        .count();

    // GAP-SG-206: until v1.2.6 this guard demanded more than thirty entries.
    // Thirty was the size of the corpus the day it was written, not a property
    // of the splitter, so the number rotted the moment entries were closed and
    // pruned: the document reached two entries and this gate went red while
    // parsing both of them perfectly. A threshold cannot tell "the splitter
    // broke" from "the backlog shrank", and only the first is a defect.
    //
    // Parity is the invariant that was meant all along. It fails when a heading
    // produces no entry, when two headings collapse into one id, and when the
    // splitter merges bodies — and it holds at two entries, at two hundred, and
    // at none.
    assert_eq!(
        parsed.len(),
        headings,
        "gaps.md carries {headings} `{ENTRY_HEADING}` heading(s) but the \
         splitter produced {} entry(ies), so a heading was dropped or two \
         collapsed onto one id, and every assertion below would pass by not \
         looking",
        parsed.len()
    );

    // Same guard one level down. `a_caveated_verdict_names_where_the_leftover_
    // is_tracked` skips any entry whose status it cannot read, so an entry
    // without the bullet is invisible to it rather than rejected by it.
    let without_status: Vec<&str> = parsed
        .iter()
        .filter(|(_, body)| status_of(body).is_none())
        .map(|(id, _)| id.as_str())
        .collect();
    assert!(
        without_status.is_empty(),
        "every entry must carry a `{STATUS_MARKER}` bullet, or the caveat check \
         below passes it over in silence instead of judging it. Missing on: {}",
        without_status.join(", ")
    );
}

#[test]
fn the_gate_separates_a_tracked_leftover_from_an_orphaned_one() {
    // GAP-SG-141's shape: caveat plus an explicit destination.
    assert!(admits_a_leftover("RESOLVIDO na v1.2.2 com resíduo aberto"));
    assert!(names_a_destination(
        "GAP-SG-141",
        "- Status: RESOLVIDO com resíduo aberto\n- B2 com resíduo em GAP-SG-156\n"
    ));

    // The trap this gate exists to avoid: citing an ANCESTOR looks like a
    // reference and owns nothing going forward. Before the destination markers
    // were required, this exact body passed.
    assert!(
        !names_a_destination(
            "GAP-SG-162",
            "- Status: RESOLVIDO com restrição técnica declarada\n\
             - Limite honesto: a medição não foi executada\n\
             - Relação: resíduo do GAP-SG-147\n"
        ),
        "naming the entry this one DESCENDS from is not naming where its own \
         leftover went"
    );

    // Same entry once the leftover is extracted into a tracked one.
    assert!(names_a_destination(
        "GAP-SG-162",
        "- Status: RESOLVIDO com restrição técnica declarada\n\
         - Relação: resíduo do GAP-SG-147\n\
         - Resíduo rastreado em GAP-SG-185\n"
    ));

    // A self-reference must never satisfy the check.
    assert!(!names_a_destination(
        "GAP-SG-162",
        "- Resíduo rastreado em GAP-SG-162\n"
    ));

    // A plain verdict is never flagged, so the gate stays quiet on the majority.
    assert!(!admits_a_leftover("RESOLVIDO na v1.2.2 e verificado"));
    assert!(!admits_a_leftover("FECHADO COMO NÃO APLICÁVEL"));
}

#[test]
fn a_caveated_verdict_names_where_the_leftover_is_tracked() {
    let entries = entries(&read_gaps());
    let mut orphaned = Vec::new();

    for (id, body) in &entries {
        let Some(status) = status_of(body) else {
            continue;
        };
        if !admits_a_leftover(&status) {
            continue;
        }
        if !names_a_destination(id, body) {
            orphaned.push(format!("{id}: {status}"));
        }
    }

    assert!(
        orphaned.is_empty(),
        "these entries closed while admitting a leftover, and name no other \
         GAP-SG entry that owns it. Either point the leftover at a tracked \
         entry, or change the verdict to PARCIAL — a caveat with no destination \
         is the shape of `documentar NÃO é resolver` that this document's own \
         convention forbids.\n{}",
        orphaned.join("\n")
    );
}

/// Lowest id this document still declares, and the reason the range below it is
/// exempt.
///
/// `gaps.md` opens by saying it was rebuilt on 2026-08-13 after being
/// overwritten, and that entries below GAP-SG-203 were never recovered. The
/// code kept citing them: GAP-SG-233 measured 117 distinct identifiers under
/// this floor, every one of them a legitimate reference to a closed gap whose
/// text is simply gone. Without the exemption the check below would be born red
/// and stay red for a loss nobody can undo, which is the fastest way to teach a
/// reader to ignore a gate.
///
/// Written as a constant with the reason beside it, following `EXEMPT` in
/// `tests/file_size_ceiling_gate.rs`, so raising the floor has to be argued in a
/// diff rather than typed inline at a call site.
const HISTORICAL_LOSS_BELOW: (u32, &str) = (
    203,
    "gaps.md declares in its own header that entries older than GAP-SG-203 were \
     lost when the file was overwritten on 2026-08-13 and were never recovered",
);

/// This file's own literals are fixtures, not citations.
///
/// The self-verification below feeds the detector `GAP-SG-999` on purpose, and
/// the sibling tests quote GAP-SG-141, -156, -162 and -185 as shapes rather than
/// as references to living entries.
const SELF: &str = "tests/gaps_caveat_gate.rs";

/// Every `.rs` file under `root`, recursively.
///
/// Replicated from `tests/file_size_ceiling_gate.rs` rather than imported:
/// integration tests are separate crates, and a shared harness module would be
/// a third thing to keep in sync for six lines of `read_dir`.
fn rust_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Collects every `GAP-SG-<digits>` identifier in `text` as a number.
///
/// Hand-rolled because this crate carries no regex dependency for tests, and
/// because the shape is fixed enough that a scanner is shorter than the pattern
/// would be. Trailing non-digits end the run, so `GAP-SG-233:` and
/// `GAP-SG-233.` both read as 233.
fn cited_ids(text: &str, out: &mut BTreeSet<u32>) {
    const NEEDLE: &str = "GAP-SG-";
    for (at, _) in text.match_indices(NEEDLE) {
        let rest = &text[at + NEEDLE.len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(id) = digits.parse::<u32>() {
            out.insert(id);
        }
    }
}

/// `true` when an id is either declared in `gaps.md` or covered by the
/// historical loss the document itself admits.
fn accounted_for(id: u32, declared: &BTreeSet<u32>) -> bool {
    id < HISTORICAL_LOSS_BELOW.0 || declared.contains(&id)
}

/// Ids `gaps.md` declares as entries, parsed from the same headings
/// [`entries`] splits on.
fn declared_ids() -> BTreeSet<u32> {
    entries(&read_gaps())
        .keys()
        .filter_map(|id| id.rsplit('-').next()?.parse::<u32>().ok())
        .collect()
}

#[test]
fn every_gap_cited_in_the_tree_exists_in_the_document() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);
    rust_files(&repo.join("tests"), &mut files);

    assert!(
        files.len() > 100,
        "the scan found {} file(s), which is too few to be the real tree — the \
         walk is broken and this gate is passing on an empty set",
        files.len()
    );

    let declared = declared_ids();
    assert!(
        !declared.is_empty(),
        "gaps.md declared no entries at all, so every citation below would look \
         orphaned and the failure would point at the wrong file"
    );

    let mut cited: BTreeSet<u32> = BTreeSet::new();
    for path in &files {
        if path.to_string_lossy().replace('\\', "/").ends_with(SELF) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        cited_ids(&text, &mut cited);
    }

    let orphaned: Vec<String> = cited
        .iter()
        .filter(|id| !accounted_for(**id, &declared))
        .map(|id| format!("GAP-SG-{id}"))
        .collect();

    assert!(
        orphaned.is_empty(),
        "these identifiers are cited in `src/` or `tests/` and exist nowhere in \
         gaps.md as an entry. They sit ABOVE GAP-SG-{}, so the historical loss \
         the document admits does not cover them — {}. A doc-comment that names \
         a gap the document never records sends the next reader looking for a \
         rationale that was never written down.\n{}",
        HISTORICAL_LOSS_BELOW.0,
        HISTORICAL_LOSS_BELOW.1,
        orphaned.join("\n")
    );
}

/// The orphan detector must fire, and the exemption must absolve.
///
/// A scan that returns nothing looks identical to a scan that found nothing
/// wrong, and this is the pair that tells them apart: one id above the floor
/// that no document declares, one below it that the declared loss covers.
#[test]
fn the_orphan_detector_fires_above_the_floor_and_forgives_below_it() {
    let mut found = BTreeSet::new();
    cited_ids(
        "//! GAP-SG-999: an identifier no entry declares.\n\
         //! GAP-SG-042: lost when the file was overwritten.\n",
        &mut found,
    );
    assert_eq!(
        found,
        BTreeSet::from([42, 999]),
        "the scanner must read both identifiers off ordinary doc-comment text"
    );

    let declared = BTreeSet::from([203, 216]);
    assert!(
        !accounted_for(999, &declared),
        "GAP-SG-999 is above the historical floor and undeclared, so it MUST be \
         reported — a detector that stays quiet here would have passed over the \
         four orphans GAP-SG-233 measured"
    );
    assert!(
        accounted_for(42, &declared),
        "GAP-SG-042 is below GAP-SG-{}, the loss gaps.md declares in its own \
         header, so it MUST be absolved",
        HISTORICAL_LOSS_BELOW.0
    );

    // The floor is a floor, not a blanket: a declared id above it passes on its
    // own merit, and an undeclared one next to it still fails.
    assert!(accounted_for(216, &declared));
    assert!(!accounted_for(217, &declared));
}
