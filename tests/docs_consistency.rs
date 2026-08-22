//! Keeps the diagnosis document and the delivery document from drifting apart.
//!
//! Closing a gap takes two writes in two different files: a line in
//! `CHANGELOG.md` and a status flip in `gaps.md`. Nothing coupled the two, so
//! the changelog advanced while the gap entry stayed frozen — and a root cause
//! that had been refuted during the fix remained published as fact. An audit
//! starting from the stale document then re-investigates what was already
//! solved, or worse, trusts a false mechanism.
//!
//! This is the executor that GAP-SG-161 asked for. It is a test rather than a
//! CI job because this project forbids CI by design; `cargo test` is the only
//! automatic gate available.

use std::collections::{BTreeMap, BTreeSet};

/// Identifier prefix shared by every entry in the gaps document.
const GAP_PREFIX: &str = "GAP-SG-";

/// Heading that opens an entry, e.g. `## GAP-SG-155 — ...`.
const ENTRY_HEADING: &str = "## GAP-SG-";

/// Line that declares the status of the entry currently being read.
const STATUS_MARKER: &str = "- Status:";

/// Heading that opens a released section in the changelog, e.g. `## [1.2.3]`.
const RELEASE_HEADING: &str = "## [";

/// Reads a repository file relative to the crate root.
fn read_repo_file(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Collects every `GAP-SG-NNN` mentioned in `text`, in order of appearance.
///
/// Scanning for the bare prefix keeps this independent of the surrounding
/// prose, which varies between narrative bullets and section headings. Order
/// matters: a heading may cite a second gap in its own title, as GAP-SG-159
/// does with GAP-SG-149, and the entry being declared is the one that comes
/// FIRST in the line — never the one that sorts first.
fn gap_ids_in_order(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    while let Some(hit) = text[cursor..].find(GAP_PREFIX) {
        let start = cursor + hit;
        let digits_at = start + GAP_PREFIX.len();
        let mut end = digits_at;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end > digits_at {
            let id = text[start..end].to_string();
            if !found.contains(&id) {
                found.push(id);
            }
        }
        cursor = end.max(start + GAP_PREFIX.len());
    }
    found
}

/// Every `GAP-SG-NNN` mentioned in `text`, deduplicated and sorted.
fn gap_ids(text: &str) -> BTreeSet<String> {
    gap_ids_in_order(text).into_iter().collect()
}

/// Identifiers the section OPENS rather than credits.
///
/// `gap_ids` scans for the bare prefix in any position, which fuses two things
/// the CHANGELOG keeps apart: it writes `(closed)`, `(reopened and closed)` and
/// `(opened)`. Citing an identifier is not the same as claiming it fixed.
///
/// Without this distinction the two gates below are jointly unsatisfiable for a
/// gap the newest release OPENS. GAP-SG-214 is cited as `(opened)` and its own
/// entry says the residue is why it stays open: omitting it fails
/// `every_gap_credited_in_the_newest_release_exists_in_gaps_md`, and restoring
/// it honestly as `ABERTO` fails
/// `the_newest_release_never_claims_an_outstanding_gap`. The only way to pass
/// both was to write a status contradicting the CHANGELOG — the exact drift
/// this file exists to prevent.
fn opened_in_section(text: &str) -> BTreeSet<String> {
    gap_ids_in_order(text)
        .into_iter()
        .filter(|id| {
            // The marker follows the identifier inside the same bullet, so a
            // plain substring search is both sufficient and precise: `**` and
            // other emphasis sit outside the pair.
            text.contains(&format!("{id} (opened)")) || text.contains(&format!("{id} (aberto)"))
        })
        .collect()
}

#[test]
fn an_opened_gap_is_not_a_credited_gap() {
    let section = "- **GAP-SG-214 (opened)** — four policy documents promised a CI.\n\
                   - **GAP-SG-213** — numeric arguments accepted any value.\n\
                   - **GAP-SG-205 (reopened and closed)** — the target record.";
    let opened = opened_in_section(section);
    assert!(opened.contains("GAP-SG-214"));
    // Credited, not opened: both must stay subject to the outstanding check.
    assert!(!opened.contains("GAP-SG-213"));
    assert!(!opened.contains("GAP-SG-205"));
}

/// The identifier a heading line declares: the first one it names.
fn heading_id(line: &str) -> Option<String> {
    gap_ids_in_order(line).into_iter().next()
}

#[test]
fn a_heading_declares_the_gap_it_names_first() {
    // Sorting would pick GAP-SG-149 here, which is the gap being *referenced*,
    // not the one being declared.
    assert_eq!(
        heading_id("## GAP-SG-159 — a regra derivada do GAP-SG-149 não tem gate").as_deref(),
        Some("GAP-SG-159")
    );
}

/// Returns the body of the most recent release section of the changelog.
///
/// Only the newest section is checked: citing an older gap in an older section
/// is legitimate history, not drift.
fn newest_release_section(changelog: &str) -> &str {
    let start = changelog
        .find(RELEASE_HEADING)
        .expect("CHANGELOG.md carries no release section");
    let rest = &changelog[start..];
    // The next heading of the same level ends the section.
    match rest[RELEASE_HEADING.len()..].find(RELEASE_HEADING) {
        Some(offset) => &rest[..RELEASE_HEADING.len() + offset],
        None => rest,
    }
}

/// Maps each entry of the gaps document to its declared status line.
fn declared_status(gaps: &str) -> BTreeMap<String, String> {
    let mut statuses = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in gaps.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(ENTRY_HEADING) {
            current = heading_id(trimmed);
            continue;
        }
        if let (Some(id), Some(rest)) = (current.as_ref(), trimmed.strip_prefix(STATUS_MARKER)) {
            // Only the first status line of an entry counts; later bullets may
            // quote a previous status for historical record.
            statuses
                .entry(id.clone())
                .or_insert_with(|| rest.trim().to_string());
        }
    }
    statuses
}

/// Whether a status line means the work is still outstanding.
///
/// The verdict lives in the FIRST word of the status, never anywhere in the
/// line. `RESOLVIDO na v1.2.2 com resíduo aberto` is a closed entry whose
/// leftover is tracked elsewhere; matching the bare word `aberto` would flag it
/// as pending and make this gate cry wolf. `PARCIAL` counts as outstanding,
/// because half-delivered is not delivered.
fn is_outstanding(status: &str) -> bool {
    let upper = status.to_uppercase();
    let verdict = upper.trim_start();
    verdict.starts_with("ABERTO") || verdict.starts_with("PARCIAL")
}

#[test]
fn the_outstanding_verdict_reads_only_the_leading_word() {
    assert!(is_outstanding("ABERTO"));
    assert!(is_outstanding("PARCIAL — resíduo aberto em GAP-SG-157"));
    assert!(!is_outstanding("RESOLVIDO na v1.2.2 com resíduo aberto"));
    assert!(!is_outstanding("RESOLVIDO na v1.2.2 e verificado"));
    assert!(!is_outstanding("FECHADO COMO NÃO APLICÁVEL"));
}

#[test]
fn every_entry_declares_a_status() {
    let gaps = read_repo_file("gaps.md");
    let statuses = declared_status(&gaps);
    let headings: BTreeSet<String> = gaps
        .lines()
        .filter(|l| l.trim_start().starts_with(ENTRY_HEADING))
        .filter_map(heading_id)
        .collect();

    let missing: Vec<&String> = headings
        .iter()
        .filter(|id| !statuses.contains_key(*id))
        .collect();
    assert!(
        missing.is_empty(),
        "these gaps.md entries carry no `- Status:` line: {missing:?}"
    );
}

#[test]
fn the_newest_release_never_claims_an_outstanding_gap() {
    let changelog = read_repo_file("CHANGELOG.md");
    let gaps = read_repo_file("gaps.md");
    let statuses = declared_status(&gaps);
    let section = newest_release_section(&changelog);
    let claimed = gap_ids(section);
    let opened = opened_in_section(section);

    let contradictions: Vec<String> = claimed
        .iter()
        .filter(|id| !opened.contains(*id))
        .filter_map(|id| {
            let status = statuses.get(id)?;
            is_outstanding(status).then(|| format!("{id} is `{status}` in gaps.md"))
        })
        .collect();

    assert!(
        contradictions.is_empty(),
        "the newest CHANGELOG section credits gaps that gaps.md still reports as outstanding.\n\
         Either flip the status in gaps.md or stop crediting the gap here.\n{}",
        contradictions.join("\n")
    );
}

#[test]
fn every_gap_credited_in_the_newest_release_exists_in_gaps_md() {
    let changelog = read_repo_file("CHANGELOG.md");
    let gaps = read_repo_file("gaps.md");
    let known: BTreeSet<String> = declared_status(&gaps).into_keys().collect();
    let claimed = gap_ids(newest_release_section(&changelog));

    let unknown: Vec<&String> = claimed.iter().filter(|id| !known.contains(*id)).collect();
    assert!(
        unknown.is_empty(),
        "the newest CHANGELOG section cites identifiers absent from gaps.md: {unknown:?}"
    );
}

// ---------------------------------------------------------------------------
// Source gates
// ---------------------------------------------------------------------------

/// Walks every `.rs` file under `src/`, in a stable order.
fn source_files() -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&root, &mut out);
    out
}

/// Path relative to the crate root, for stable assertion messages.
fn relative(path: &std::path::Path) -> String {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Blanks out every double-quoted span so its contents cannot trip a gate.
///
/// A string literal is data, not prose: `"  hello  world  "` in a doc-test is a
/// fixture whose spacing is the point.
fn without_string_literals(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in line.chars() {
        match ch {
            _ if escaped => {
                escaped = false;
                out.push(if in_string { ' ' } else { ch });
            }
            '\\' => {
                escaped = true;
                out.push(if in_string { ' ' } else { ch });
            }
            '"' => {
                in_string = !in_string;
                out.push('"');
            }
            _ => out.push(if in_string { ' ' } else { ch }),
        }
    }
    out
}

/// The comment body of `line`, or `None` when the line is not a comment.
fn comment_body(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    for marker in ["///", "//!", "//"] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return Some(rest);
        }
    }
    None
}

/// `true` when a comment shows the scar of a deleted identifier.
///
/// Removing a name from prose leaves the spaces that surrounded it, so
/// `Provides the [`Foo`] trait with` decays into `Provides  trait with`. The
/// signature is therefore a double space BETWEEN TWO WORD CHARACTERS.
///
/// Both halves of that rule carry weight. Requiring word characters on each
/// side spares aligned code samples inside comments, where a double space
/// precedes a parenthetical rather than replacing a name. Blanking string
/// literals first spares fixtures whose spacing is deliberate. A looser
/// heuristic — flagging a doc that ends on a preposition — produced 624 false
/// positives on this repository, because a comment wrapping onto the next line
/// is ordinary.
fn looks_mutilated(line: &str) -> bool {
    let Some(body) = comment_body(line) else {
        return false;
    };
    let cleaned = without_string_literals(body);
    let chars: Vec<char> = cleaned.chars().collect();
    chars
        .windows(4)
        .any(|w| w[0].is_alphanumeric() && w[1] == ' ' && w[2] == ' ' && w[3].is_alphanumeric())
}

#[test]
fn the_mutilation_gate_separates_a_scar_from_deliberate_spacing() {
    // Scars: an identifier was deleted and its surrounding spaces remained.
    assert!(looks_mutilated(
        "//! Provides  trait with concrete implementations for"
    ));
    assert!(looks_mutilated(
        "        // empty vector would propagate to  which"
    ));
    assert!(looks_mutilated("/// so  and  can route to FTS5-puro via"));

    // Deliberate: the double space lives inside a string literal, which is a
    // fixture. This is `src/parsers/mod.rs:192` verbatim.
    assert!(!looks_mutilated(
        r#"/// assert_eq!(normalize_entity_name("  hello  world  "), "hello-world");"#
    ));

    // Deliberate: an aligned code sample inside a comment, where the double
    // space precedes a parenthetical. This is `src/stdin_helper.rs:170`.
    assert!(!looks_mutilated(
        "    //   cargo run --release -- remember --name h1-test  (with the stdin body flag)"
    ));

    // Not a comment at all, and ordinary prose.
    assert!(!looks_mutilated(r#"    let s = "a  b";"#));
    assert!(!looks_mutilated("/// One space between every word here."));
}

#[test]
fn no_comment_in_src_carries_the_scar_of_a_deleted_identifier() {
    let mut scars = Vec::new();
    for path in source_files() {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for (n, line) in text.lines().enumerate() {
            if looks_mutilated(line) {
                scars.push(format!("{}:{}: {}", relative(&path), n + 1, line.trim()));
            }
        }
    }
    assert!(
        scars.is_empty(),
        "a comment carries a double space between two words, which is what an \
         erased identifier leaves behind. Restore the missing name, or move the \
         deliberate spacing inside a string literal.\n{}",
        scars.join("\n")
    );
}

/// Line ranges covered by a `#[cfg(test)]` item, as `(start, end)` inclusive.
///
/// Test code states its own fixtures inline on purpose: a literal there IS the
/// case under test, not a policy the product applies.
fn cfg_test_spans(text: &str) -> Vec<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim_start().starts_with("#[cfg(test)]") {
            let mut depth: i32 = 0;
            let mut opened = false;
            let mut j = i;
            while j < lines.len() {
                depth += lines[j].matches('{').count() as i32;
                depth -= lines[j].matches('}').count() as i32;
                if lines[j].contains('{') {
                    opened = true;
                }
                if opened && depth <= 0 {
                    break;
                }
                j += 1;
            }
            spans.push((i, j.min(lines.len().saturating_sub(1))));
            i = j;
        }
        i += 1;
    }
    spans
}

/// Files whose entire contents are unit tests included from a sibling module.
fn is_test_only_file(path: &std::path::Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    name == "tests.rs" || name.ends_with("_tests.rs")
}

/// A timing policy written as a bare literal instead of a named constant.
///
/// `Duration::from_secs(300)` and `sleep(500)` bury a decision the operator is
/// supposed to be able to find, read and override. `src/constants.rs` is where
/// such a number states its units, its reason and its XDG key.
fn is_inline_timing_policy(line: &str) -> bool {
    let code = without_string_literals(line);
    if code.trim_start().starts_with("//") {
        return false;
    }
    // `Duration::from_secs` needs its own `(` located; `sleep(` already carries it.
    for (marker, marker_ends_at_paren) in [
        ("Duration::from_", false),
        ("sleep(", true),
        ("timeout(", true),
    ] {
        let mut from = 0;
        while let Some(rel) = code[from..].find(marker) {
            let after_marker = from + rel + marker.len();
            let arg_start = if marker_ends_at_paren {
                Some(after_marker)
            } else {
                code[after_marker..].find('(').map(|o| after_marker + o + 1)
            };
            if let Some(arg_start) = arg_start {
                let arg: String = code[arg_start..]
                    .chars()
                    .take_while(|c| *c != ')')
                    .filter(|c| !c.is_whitespace() && *c != '_')
                    .collect();
                if !arg.is_empty() && arg.chars().all(|c| c.is_ascii_digit()) {
                    return true;
                }
            }
            from = after_marker;
            if from >= code.len() {
                break;
            }
        }
    }
    false
}

#[test]
fn the_timing_policy_gate_reads_the_argument_not_the_call() {
    assert!(is_inline_timing_policy(
        "    std::thread::sleep(Duration::from_secs(300));"
    ));
    assert!(is_inline_timing_policy("    sleep(500);"));
    // A named constant is exactly what the gate asks for.
    assert!(!is_inline_timing_policy(
        "    std::thread::sleep(Duration::from_secs(crate::constants::SHUTDOWN_GRACE_SECS));"
    ));
    assert!(!is_inline_timing_policy("    Duration::from_secs(budget)"));
    // A comment describing the old value is documentation, not policy.
    assert!(!is_inline_timing_policy(
        "    // was Duration::from_secs(300) before v1.2.0"
    ));
}

#[test]
fn no_timing_policy_in_src_is_written_as_a_bare_literal() {
    let mut hits = Vec::new();
    for path in source_files() {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name == "constants.rs" || is_test_only_file(&path) {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let spans = cfg_test_spans(&text);
        for (n, line) in text.lines().enumerate() {
            if spans.iter().any(|(a, b)| n >= *a && n <= *b) {
                continue;
            }
            if is_inline_timing_policy(line) {
                hits.push(format!("{}:{}: {}", relative(&path), n + 1, line.trim()));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "a timeout, sleep or duration is written as a bare literal. Name it in \
         `src/constants.rs` with its units, its reason and its XDG key, and read \
         it through the resolver.\n{}",
        hits.join("\n")
    );
}
