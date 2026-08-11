//! A closed entry that admits a leftover must say where the leftover lives.
//!
//! `gaps.md` opens with the rule this gate enforces: *documentar NÃO é resolver*.
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

use std::collections::BTreeMap;

/// Heading that opens an entry.
const ENTRY_HEADING: &str = "## GAP-SG-";

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

/// Headings present in the raw document, counted with the very predicate
/// [`entries`] splits on.
///
/// Kept separate from [`entries`] on purpose: a blindness guard that reuses the
/// splitter's own bookkeeping cannot detect the splitter losing an entry.
fn heading_count(gaps: &str) -> usize {
    gaps.lines()
        .filter(|line| line.trim_start().starts_with(ENTRY_HEADING))
        .count()
}

#[test]
fn the_gate_found_the_entries_it_is_supposed_to_read() {
    let gaps = read_gaps();
    let parsed = entries(&gaps);
    let headings = heading_count(&gaps);

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
