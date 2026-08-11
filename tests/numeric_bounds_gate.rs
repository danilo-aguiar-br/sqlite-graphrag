//! GAP-SG-213: numeric read-path arguments accepted any value the shell allowed.
//!
//! `crate::parsers::parse_k_range` has existed for a long time and bounds `-k`
//! to `1..=4096`. It was wired to exactly two arguments. Meanwhile every
//! concurrency knob in the CLI carries a `clap::value_parser!(T).range(..)`,
//! so the parallelism surface was audited and the memory surface was not.
//!
//! The sharp end was `related --limit`. `src/commands/related.rs` allocates
//! `Vec::with_capacity(limit)` and an `AHashSet` of the same capacity before
//! any row can bound them, so an absurd value aborted the process on allocation
//! instead of returning the exit code this crate reserves for memory pressure.
//!
//! A one-time fix would leave the asymmetry that produced it, so this gate
//! makes the rule structural: a numeric clap argument whose name says it bounds
//! a result set must declare a `value_parser`. Reading the source rather than
//! the parsed `Command` is deliberate — clap erases the range into an opaque
//! `ValueParser`, so the built tree cannot answer "is this bounded?" while the
//! text can.

use std::path::{Path, PathBuf};

/// Field names that denote a bound on a result set, a page, or a walk.
///
/// Deliberately exact rather than a substring match: `max_concurrency` also
/// ends in a number-shaped word but is clamped against the host CPU count
/// inside the command, and `timeout` is a duration, not a size.
const BOUNDED_FIELDS: &[&str] = &[
    "limit",
    "k",
    "top_k",
    "depth",
    "max_hops",
    "max_results",
    "max_sub_queries",
];

/// Integer types that reach an allocation or a `LIMIT` clause unchanged.
const NUMERIC_TYPES: &[&str] = &["usize", "u32", "u64", "u16"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Every `.rs` file under `src/commands`, recursively.
fn command_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root().join("src").join("commands")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Parses `    pub limit: usize,` into `Some("limit")` when the type is numeric.
fn bounded_field_name(line: &str) -> Option<&'static str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("pub ")?;
    let (name, ty) = rest.split_once(": ")?;
    let ty = ty.trim_end_matches(',').trim();
    if !NUMERIC_TYPES.contains(&ty) {
        return None;
    }
    BOUNDED_FIELDS.iter().copied().find(|f| *f == name)
}

/// Walks up from a field declaration and returns the `#[arg(...)]` above it.
///
/// Returns `None` when the field is not a clap argument at all, which is how
/// response and envelope structs under `src/commands` stay out of scope.
fn attribute_above(lines: &[&str], field_idx: usize) -> Option<String> {
    let mut idx = field_idx;
    let mut collected: Vec<&str> = Vec::new();
    while idx > 0 {
        idx -= 1;
        let line = lines[idx].trim();
        if line.starts_with("///") || line.is_empty() {
            // Doc comments sit between the attribute and the field; keep going
            // only while nothing else has been collected, so an unrelated field
            // above is never absorbed into this attribute.
            if collected.is_empty() {
                continue;
            }
            break;
        }
        collected.push(lines[idx]);
        if line.starts_with("#[arg(") || line.starts_with("#[arg]") {
            collected.reverse();
            return Some(collected.join("\n"));
        }
        if collected.len() > 24 {
            break;
        }
    }
    None
}

#[test]
fn every_bounded_numeric_argument_declares_a_value_parser() {
    let mut offences: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for path in command_sources() {
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        let lines: Vec<&str> = body.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let Some(field) = bounded_field_name(line) else {
                continue;
            };
            let Some(attr) = attribute_above(&lines, idx) else {
                continue; // not a clap argument
            };
            checked += 1;
            if attr.contains("value_parser") {
                continue;
            }
            let rel = path
                .strip_prefix(root())
                .unwrap_or(&path)
                .display()
                .to_string();
            offences.push(format!("{rel}:{} `{field}`", idx + 1));
        }
    }

    assert!(
        checked > 0,
        "the scan found no bounded numeric arguments at all, which means the field pattern \
         stopped matching rather than that the surface is clean. Fix the parser in this file \
         before trusting a green result."
    );

    assert!(
        offences.is_empty(),
        "{} bounded numeric argument(s) accept any value the shell allows. Give each one a \
         `value_parser` from `crate::parsers` — `parse_k_range` for retrieval, \
         `parse_list_limit_range` for paging over stored rows, `parse_hops_range_u32` or \
         `parse_hops_range_usize` for traversal depth, `parse_sub_queries_range` for fan-out. \
         Offenders: {}",
        offences.len(),
        offences.join(", ")
    );
}
