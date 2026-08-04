//! GAP-SG-210: the 800-line ceiling, pinned to ONE metric and enforced.
//!
//! The ceiling was declared by GAP-SG-89 and cured twice — GAP-SG-195 for
//! `src/`, GAP-SG-208 for `tests/` — without either closure pinning WHAT is
//! counted. That gap in the definition is what let the two closures use
//! different metrics and both report green:
//!
//! * GAP-SG-195 measured PHYSICAL lines: `src/constants.rs` "went from 960 to
//!   799", and 799 is the physical size of the largest file in `src/` today.
//! * GAP-SG-208 measured `tokei` CODE lines, which exclude comments and blanks.
//!   Under that metric nothing in `tests/` exceeded the ceiling. Under the
//!   metric GAP-SG-195 used, four files still did — `installed_binary_smoke.rs`
//!   at 981, `schema_migration_integration.rs` at 923, `integration_graph.rs`
//!   at 922 and `cookbook_recipes.rs` at 907.
//!
//! Neither closure was dishonest; the ceiling simply did not say. A rule that
//! does not name its unit cannot be enforced, and an unenforceable rule reads
//! as satisfied the moment someone picks the forgiving reading.
//!
//! This gate settles it on PHYSICAL lines, for two reasons. It is the number an
//! editor shows and a reviewer scrolls, which is the cost the ceiling exists to
//! bound. And it is the stricter of the two, so a file that passes here passes
//! under any other reading — the reverse is not true.

use std::path::{Path, PathBuf};

/// Physical lines. See the module docs for why this metric and not `tokei`'s.
const CEILING: usize = 800;

/// Files exempt from the ceiling, each with the reason written next to it.
///
/// Empty on purpose. An allowlist that starts populated teaches that the
/// ceiling is negotiable; one that starts empty forces the next exemption to be
/// argued in a diff.
const EXEMPT: &[(&str, &str)] = &[];

/// Every `.rs` file under `root`, recursively.
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

/// Repo-relative path with forward slashes, so the allowlist and the failure
/// message read the same on every platform.
fn relative(path: &Path, repo: &Path) -> String {
    path.strip_prefix(repo)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn no_source_or_test_file_exceeds_the_declared_ceiling() {
    let repo = repo_root();
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);
    rust_files(&repo.join("tests"), &mut files);

    assert!(
        files.len() > 100,
        "the scan found {} files, which is too few to be the real tree — the \
         walk is broken and this gate is passing on an empty set",
        files.len()
    );

    let mut over = Vec::new();
    for path in &files {
        let rel = relative(path, &repo);
        if EXEMPT.iter().any(|(name, _)| *name == rel) {
            continue;
        }
        let text = std::fs::read_to_string(path).expect("source file must be readable");
        let lines = text.lines().count();
        if lines > CEILING {
            over.push((rel, lines));
        }
    }

    over.sort_by(|a, b| b.1.cmp(&a.1));
    assert!(
        over.is_empty(),
        "{} file(s) above the {CEILING}-line ceiling:\n{}\n\nSplit by \
         responsibility, or add an entry to EXEMPT with the reason written \
         next to it. Do not switch to a metric that counts fewer lines: that \
         is how the ceiling was reported closed twice while four test files \
         sat above it.",
        over.len(),
        over.iter()
            .map(|(name, lines)| format!("  {lines:>5}  {name}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_exemption_list_names_only_files_that_exist() {
    // An allowlist entry for a deleted file is worse than no entry: it silently
    // covers whatever file later takes that path.
    let repo = repo_root();
    for (name, reason) in EXEMPT {
        assert!(
            repo.join(name).is_file(),
            "EXEMPT names `{name}` ({reason}), which no longer exists"
        );
    }
}

#[test]
fn every_exemption_carries_a_reason() {
    for (name, reason) in EXEMPT {
        assert!(
            reason.len() > 20,
            "the exemption for `{name}` has no real justification: {reason:?}"
        );
    }
}
