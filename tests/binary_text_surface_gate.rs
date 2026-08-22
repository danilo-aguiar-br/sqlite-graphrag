//! The text this binary PRINTS was never checked against the parser it ships.
//!
//! Two gates already cross a document against the clap tree:
//! `docs_surface_gate` for `docs/`, `skills_surface_gate` for `skills/`. Neither
//! covers the third document this project publishes, which is the one with the
//! highest authority: the strings inside `src/` that the binary prints back at
//! the operator as help examples and runtime advice.
//!
//! That gap is not theoretical. `--no-fts-skip-when-functional` lived in the
//! `after_long_help` example of `optimize` and in a runtime message from v1.0.69
//! to v1.2.8 while the parser rejected it with exit 2. ADR-0016 had decided the
//! flag should exist; the decision was never implemented, and the examples were
//! written against the decision rather than against the parser. An operator who
//! obeyed the binary got exit 2 before any work happened, and both skills cited
//! the flag because they trusted the binary — correctly, which is the point.
//!
//! # Scope, stated so the failures stay readable
//!
//! Only tokens inside a STRING LITERAL or a doc comment count, because those are
//! the two forms that reach the operator's terminal. A `//` comment addressed to
//! the next maintainer is not published text and is skipped: flagging it would
//! reproduce the signal-to-noise failure GAP-SG-299 measured, where twelve false
//! findings per real one taught the reader to close the panic unread.

use clap::CommandFactory;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use sqlite_graphrag::cli::Cli;

/// Long flags this binary may print although clap does not define them.
///
/// Each entry names the OWNER of the flag, because the only legitimate reason to
/// print a flag we do not define is to talk about another program. An entry
/// whose owner is this binary is a bug being whitelisted, not an exemption.
const FLAGS_OWNED_BY_ANOTHER_PROGRAM: [(&str, &str); 1] = [(
    "--bin",
    "cargo, in the `cargo run --bin dump_schema` regeneration recipe",
)];

/// Every long flag the parser accepts, at any depth, including hidden ones.
///
/// The tree is BUILT first: clap synthesises `--help` and `--version` during
/// `build()`, so walking the unbuilt tree reports them as fabricated in text
/// that names them correctly.
fn shipped_flags() -> BTreeSet<String> {
    fn walk(cmd: &clap::Command, out: &mut BTreeSet<String>) {
        for arg in cmd.get_arguments() {
            if let Some(long) = arg.get_long() {
                out.insert(format!("--{long}"));
            }
            if let Some(aliases) = arg.get_all_aliases() {
                for alias in aliases {
                    out.insert(format!("--{alias}"));
                }
            }
        }
        for sub in cmd.get_subcommands() {
            walk(sub, out);
        }
    }

    let mut flags = BTreeSet::new();
    let mut root = Cli::command();
    root.build();
    walk(&root, &mut flags);
    flags
}

/// Every `.rs` file under `src/`.
fn source_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    let mut out = Vec::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut out);
    out.sort();
    out
}

/// Whether a line carries text this binary can print at the operator.
///
/// A doc comment on a clap type becomes help output, and a string literal can
/// become any message. A plain `//` comment reaches no operator, so it is out of
/// scope by the same reasoning that keeps unmarked prose out of the skill gate.
fn line_is_published_text(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("///") || trimmed.starts_with("//!") || line.contains('"')
}

/// Long flags a line names, when that line carries published text.
///
/// Read from the whole line rather than from a parsed literal: a `help = "..."`
/// value, a `format!` template and an `after_long_help` block all differ in
/// syntax and agree in effect, and re-implementing Rust's lexer to tell them
/// apart would buy nothing this assertion needs.
fn printed_flags(source: &str) -> BTreeMap<String, usize> {
    let mut found = BTreeMap::new();
    for (index, line) in source.lines().enumerate() {
        if !line_is_published_text(line) {
            continue;
        }
        for (offset, _) in line.match_indices("--") {
            // A flag opens a token. `"foo--bar"` embeds `--bar` and names no
            // flag, and neither does an em-dash written as `word--word`.
            let opens_a_token = line[..offset]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-');
            if !opens_a_token {
                continue;
            }
            let rest = &line[offset + 2..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                .collect();
            // `-` alone closes an em-dash or a `--` separator, not a flag.
            if name.len() >= 3 && !name.ends_with('-') {
                found.entry(format!("--{name}")).or_insert(index + 1);
            }
        }
    }
    found
}

#[test]
fn every_flag_this_binary_prints_is_a_flag_it_accepts() {
    let shipped = shipped_flags();
    let mut fabricated = Vec::new();

    for path in source_files() {
        let source = std::fs::read_to_string(&path).expect("source file is readable");
        let relative = path
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(&path)
            .display()
            .to_string();
        for (flag, line) in printed_flags(&source) {
            let exempt = FLAGS_OWNED_BY_ANOTHER_PROGRAM
                .iter()
                .any(|(token, _)| *token == flag);
            if !shipped.contains(&flag) && !exempt {
                fabricated.push(format!("{relative}:{line}: {flag}"));
            }
        }
    }

    assert!(
        fabricated.is_empty(),
        "this binary PRINTS a flag its own parser does not define. The operator \
         who obeys the message gets exit 2 before any work happens, and every \
         document downstream copies the mistake in good faith: both skills cited \
         `--no-fts-skip-when-functional` because the `optimize` help example \
         taught it for six releases.\n\
         Declare the flag, or correct the text — and if an ADR decided the flag \
         should exist, declaring it is the fix.\n{}",
        fabricated.join("\n")
    );
}

#[test]
fn every_exemption_names_an_owner_that_is_not_this_binary() {
    for (flag, owner) in FLAGS_OWNED_BY_ANOTHER_PROGRAM {
        assert!(
            !owner.is_empty() && owner != "sqlite-graphrag",
            "the exemption for `{flag}` names `{owner}` as its owner. An \
             exemption owned by this binary is a defect being whitelisted: \
             declare the flag instead"
        );
    }
}

#[test]
fn the_scanner_reads_published_text_and_skips_maintainer_comments() {
    let source = "\
/// Pass --doc-comment-flag to do it.
// Internal note about --maintainer-only-flag.
let msg = \"use --string-literal-flag to override\";
let embedded = \"foo--bar\"; // the -- inside a word opens no token
";
    let found = printed_flags(source);
    assert!(
        found.contains_key("--doc-comment-flag"),
        "a doc comment becomes help output and must be read"
    );
    assert!(
        found.contains_key("--string-literal-flag"),
        "a string literal can become any printed message"
    );
    assert!(
        !found.contains_key("--maintainer-only-flag"),
        "a plain comment reaches no operator and must stay out of scope"
    );
    assert!(
        !found.contains_key("--bar"),
        "`foo--bar` inside a literal embeds `--bar` and names no flag"
    );
}

#[test]
fn the_scanner_reports_the_line_it_found_the_flag_on() {
    let source = "let a = 1;\nlet msg = \"pass --some-real-flag now\";\n";
    let found = printed_flags(source);
    assert_eq!(
        found.get("--some-real-flag"),
        Some(&2),
        "the report has to point at the line, or the fix means grepping again"
    );
}
