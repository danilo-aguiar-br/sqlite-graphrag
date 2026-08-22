//! GAP-SG-293: the operational corpus may not teach a flag or a subcommand the
//! parser does not define.
//!
//! `tests/skills_surface_gate.rs` already owns this check, and owns it well:
//! it walks the BUILT clap tree and has unit tests for its own scanner. Its
//! failure message names `--extraction-backend` and `--strict-env-clear` as
//! flags it caught. Both were removed from the two `SKILL.md` files — and both
//! stayed alive in `README.md`, `INTEGRATIONS.md`, `SECURITY.md` and
//! `CONTRIBUTING.md`, because the constant on its line 31 lists exactly two
//! paths.
//!
//! The machinery was right and pointed at 2 documents out of 29. This file is
//! that gate aimed at the rest of the corpus. Same class as GAP-SG-287 (the
//! language gate walked `src/` and never `tests/`) and GAP-SG-292 (the env
//! gate banned one literal).
//!
//! A fabricated flag is not a stale sentence. `sqlite-graphrag --claude-timeout
//! 10 …` dies with exit 2 before doing any work, and an agent that copies the
//! documented invocation never reaches the task.

use clap::CommandFactory;
use sqlite_graphrag::cli::Cli;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Documents an operator or an agent reads as instruction.
///
/// Deliberately a WALK and not a list: a hand-kept list cannot grow when a
/// document is born, so a new document is born outside the gate. That is
/// precisely how `skills_surface_gate` ended up covering two files.
fn operational_documents() -> Vec<(String, String)> {
    let root = repo_root();
    let mut out = Vec::new();
    collect(&root, &root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Paths whose citations are history or third-party material.
const ALLOWLIST_PREFIXES: &[&str] = &[
    // Release history: naming a flag that was removed is the content.
    "CHANGELOG.md",
    "CHANGELOG.pt-BR.md",
    // A migration guide exists to describe what the OLD version accepted.
    "docs/MIGRATION",
    // The gap ledger records defects verbatim, fabricated flags included.
    "gaps.md",
    "CLAUDE.md",
    // Decisions are dated records of what was true when written.
    "docs/decisions/",
    "docs/TEST_PLAN",
    "docs/TESTING",
    // Original specification and third-party documentation. `docs_rules/`
    // documents other CLIs (OpenCode, Codex, Gemini); `docs_prd/` reproduces
    // OpenRouter's own API reference.
    "docs_rules/",
    "docs_prd/",
    // Owned by `skills_surface_gate`, which already runs this check there.
    "skills/",
    "tests/",
    "src/",
];

/// Programs whose flags legitimately appear in this corpus.
///
/// A code span is skipped WHOLE when it starts with one of these, rather than
/// exempting individual flags: `cargo test --no-fail-fast` names a cargo flag,
/// and listing `--no-fail-fast` as an exemption would also excuse it inside a
/// `sqlite-graphrag` invocation, where it would be fabricated.
const FOREIGN_PROGRAMS: &[&str] = &[
    "cargo",
    "rustfmt",
    "rustup",
    "rg",
    "fd",
    "bat",
    "jaq",
    "jq",
    "sd",
    "curl",
    "wget",
    "git",
    "atomwrite",
    "docker",
    "npm",
    "npx",
    "pip",
    "pipx",
    "brew",
    "apt",
    "dnf",
    "tar",
    "unzip",
    "flock",
    "timeout",
    "xargs",
    "sed",
    "awk",
    "grep",
    "find",
    "ls",
    "cat",
    "head",
    "tail",
    "sort",
    "uniq",
    "wc",
    "tokei",
    "difft",
    "grex",
    "choose",
    "dutree",
    "procs",
    "fend",
    "htmlq",
    "ruplacer",
    "ast-grep",
    "sg",
    "python3",
    "python",
    "node",
    "bash",
    "sh",
    "zsh",
    "export",
    "codex",
    "claude",
    "opencode",
    "gemini",
    // Found by running this gate, not by guessing: `--bin` came from
    // `diff <(cargo run --bin dump_schema) …`, whose head is `diff` and not
    // `cargo`, and `--warmup` from a `hyperfine` benchmark.
    "diff",
    "hyperfine",
    "watch",
    "systemd",
    "systemctl",
    "launchctl",
    "cron",
    "crontab",
];

/// Flags that belong to another program and are named in prose rather than
/// inside an attributable invocation.
///
/// Each entry states its owner. An entry without an owner is a defect being
/// waved through, which is how allowlists rot into blanket suppression.
const FLAGS_OF_OTHER_PROGRAMS: &[(&str, &str)] = &[
    ("--all-features", "cargo"),
    ("--no-deps", "cargo"),
    ("--no-fail-fast", "cargo"),
    ("--locked", "cargo"),
    ("--ignored", "cargo test"),
    ("--mcp-config", "claude code CLI"),
    ("--strict-mcp-config", "claude code CLI"),
    ("--dangerously-skip-permissions", "claude code CLI"),
    ("--settings", "claude code CLI"),
    ("--print", "claude code CLI"),
    ("--ask-for-approval", "codex CLI"),
    ("--sandbox", "codex CLI"),
    ("--skip-git-repo-check", "codex CLI"),
    ("--no-install-recommends", "apt-get"),
    ("--bare", "git"),
    (
        "--oauth-only-resolution-use-anthropic-auth-token",
        "prose describing a policy, not a flag",
    ),
    (
        "--oauth-only-resolution-use-codex-auth-json-or-openai-base-url",
        "prose describing a policy, not a flag",
    ),
];

fn is_another_programs_flag(flag: &str) -> bool {
    FLAGS_OF_OTHER_PROGRAMS.iter().any(|(f, _)| *f == flag)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn is_allowlisted(rel: &str) -> bool {
    ALLOWLIST_PREFIXES
        .iter()
        .any(|p| rel.starts_with(p) || rel == *p)
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if rel.starts_with('.') || rel.starts_with("target") {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
            continue;
        }
        let is_doc = rel.ends_with(".md")
            || (rel.starts_with("llms") && rel.ends_with(".txt") && !rel.contains('/'));
        if !is_doc || is_allowlisted(&rel) {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            out.push((rel, text));
        }
    }
}

/// Every long flag the parser accepts, at any depth, including hidden ones.
///
/// The tree is BUILT first. `CommandFactory::command()` returns the declared
/// arguments only: clap synthesises `--help` and `--version` during `build()`,
/// so walking the unbuilt tree reports `--help` as fabricated in a document
/// that names it correctly.
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

/// Every top-level subcommand name and alias.
fn shipped_subcommands() -> BTreeSet<String> {
    let mut root = Cli::command();
    root.build();
    let mut out = BTreeSet::new();
    for sub in root.get_subcommands() {
        out.insert(sub.get_name().to_string());
        for alias in sub.get_all_aliases() {
            out.insert(alias.to_string());
        }
    }
    out.insert("help".to_string());
    out
}

/// Every backticked span of a document.
fn code_spans(markdown: &str) -> Vec<&str> {
    markdown.split('`').skip(1).step_by(2).collect()
}

/// Each command a span contains, one per element.
///
/// A span is NOT one command. A fenced block is a whole script, and a line may
/// chain commands behind `|`, `&&` or `;`. Treating the span as a single
/// invocation is what let `--path` through: it sits inside a ```dockerfile
/// block whose first token is the word `dockerfile`, so the head was never
/// `cargo` and the foreign filter never fired, three lines above the actual
/// `cargo install --path .`.
fn commands_in(span: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for line in span.lines() {
        for segment in line.split(['|', ';']) {
            for part in segment.split("&&") {
                let part = part.trim();
                if !part.is_empty() {
                    out.push(part);
                }
            }
        }
    }
    out
}

/// Tokens that precede the real program name on a command line.
///
/// `RUN`, `CMD` and `ENTRYPOINT` come from Dockerfiles, which this corpus
/// embeds in fenced blocks; `$` and `#` are prompt markers; `VAR=value` is an
/// inline assignment; `sudo`, `env` and `time` wrap the command that follows.
fn strip_command_prefix<'a>(words: &mut impl Iterator<Item = &'a str>) -> &'a str {
    let mut head = words.next().unwrap_or("");
    loop {
        let is_prefix = head == "$"
            || head == "#"
            || head == "RUN"
            || head == "CMD"
            || head == "ENTRYPOINT"
            || head == "sudo"
            || head == "env"
            || head == "time"
            || (head.contains('=') && !head.starts_with('-'));
        if !is_prefix {
            return head;
        }
        head = words.next().unwrap_or("");
        if head.is_empty() {
            return head;
        }
    }
}

/// True when the command belongs to another program.
fn is_foreign(command: &str) -> bool {
    let mut words = command.split_whitespace();
    let head = strip_command_prefix(&mut words);
    let head = head.rsplit('/').next().unwrap_or(head);
    FOREIGN_PROGRAMS.contains(&head)
}

/// Long flags a document names inside a code span it does not attribute to
/// another program.
fn cited_flags(markdown: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for span in code_spans(markdown) {
        for command in commands_in(span) {
            if is_foreign(command) {
                continue;
            }
            for word in command.split_whitespace() {
                let Some(name) = word.strip_prefix("--") else {
                    continue;
                };
                let name: String = name
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                    .collect();
                if name.len() >= 2 {
                    found.insert(format!("--{name}"));
                }
            }
        }
    }
    found
}

/// Subcommands a document invokes explicitly as `sqlite-graphrag <verb>`.
///
/// Only the explicit form counts. A bare backticked word is ambiguous — it may
/// be a field, a type or an English noun — and a gate that guesses produces the
/// false positives that get gates deleted.
fn cited_subcommands(markdown: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for span in code_spans(markdown) {
        for command in commands_in(span) {
            let mut words = command.split_whitespace();
            let head = strip_command_prefix(&mut words);
            // The binary must OPEN the command. Requiring that is what stops
            // prose inside a fenced block from being parsed as an invocation:
            // `# Wrap a long-running sqlite-graphrag invocation ...` and
            // `git commit -m "track sqlite-graphrag db via LFS"` were both
            // reported as fabricated subcommands before this rule existed.
            if !head.ends_with("sqlite-graphrag") {
                continue;
            }
            let Some(next) = words.next() else { continue };
            // A flag follows: give up on this command rather than guess.
            // Skipping the flag would then have to decide whether the token
            // after it is a VALUE or the verb, and guessing wrong is what made
            // this scanner report `sqlite-graphrag openrouter` — the value of
            // `--embedding-backend` — as a fabricated subcommand. A declared
            // false negative beats a false positive: a gate that cries wolf is
            // a gate somebody deletes.
            if next.starts_with('-') {
                continue;
            }
            if next
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && next.len() >= 2
            {
                found.insert(next.to_string());
            }
        }
    }
    found
}

#[test]
fn every_flag_the_documentation_teaches_is_a_flag_the_parser_accepts() {
    let shipped = shipped_flags();
    let mut fabricated = Vec::new();
    for (rel, text) in operational_documents() {
        for flag in cited_flags(&text) {
            if !shipped.contains(&flag) && !is_another_programs_flag(&flag) {
                fabricated.push(format!("{rel}: {flag}"));
            }
        }
    }
    assert!(
        fabricated.is_empty(),
        "the documentation teaches flags this binary does not define. An agent \
         that copies the documented invocation dies with exit 2 before doing \
         any work.\n\
         \n\
         TWO fixes work, and a third does NOT. Name the real flag; or take the \
         dead flag OUT of the backticks, keeping the historical sentence whole \
         in prose — no copyable invocation remains, which is the whole point. \
         What does NOT work is labelling the paragraph `historical`: this gate \
         reads code spans and the FLAGS_OF_OTHER_PROGRAMS list, and no textual \
         marker at all. An earlier version of this message promised otherwise \
         and was wrong.\n\
         \n\
         When you unwrap a flag, delete the backtick PAIR. A lone backtick \
         flips span parity for the rest of the file, turning prose into code \
         and code into prose for every scanner using this technique — this \
         one included. `documents_keep_their_backticks_balanced` guards it.\n{}",
        fabricated.join("\n")
    );
}

#[test]
fn every_subcommand_the_documentation_invokes_exists() {
    let shipped = shipped_subcommands();
    let mut fabricated = Vec::new();
    for (rel, text) in operational_documents() {
        for verb in cited_subcommands(&text) {
            if !shipped.contains(&verb) {
                fabricated.push(format!("{rel}: sqlite-graphrag {verb}"));
            }
        }
    }
    assert!(
        fabricated.is_empty(),
        "the documentation invokes subcommands this binary does not ship.\n{}",
        fabricated.join("\n")
    );
}

/// Backticks must pair off in every document.
///
/// `code_spans` takes every other piece of a split on the backtick, so parity
/// IS the parse. One orphan backtick silently swaps prose and code from that
/// point to the end of the file, and both gates in this repository then read a
/// different document than the one on screen — reporting green over spans that
/// are really prose, and offences over prose that is really nothing.
///
/// This is not hypothetical. Unwrapping a dead flag from its backticks is the
/// prescribed fix for the gate above, and deleting one of the two is the
/// obvious way to get it wrong.
#[test]
fn documents_keep_their_backticks_balanced() {
    let mut odd = Vec::new();
    for (rel, text) in operational_documents() {
        let count = text.chars().filter(|c| *c == '`').count();
        if !count.is_multiple_of(2) {
            odd.push(format!("{rel}: {count} backticks"));
        }
    }
    assert!(
        odd.is_empty(),
        "these documents carry an odd number of backticks, so every code span \
         after the orphan is parsed inverted:\n{}",
        odd.join("\n")
    );
}

/// The walk must actually reach the corpus.
///
/// Without this a typo in the filter empties the walk and both gates above
/// report green over nothing — the failure recorded as GAP-SG-288 and again as
/// GAP-SG-291.
#[test]
fn the_walk_reaches_the_corpus_it_is_supposed_to_read() {
    let docs = operational_documents();
    let names: Vec<&str> = docs.iter().map(|(rel, _)| rel.as_str()).collect();
    for expected in [
        "README.md",
        "README.pt-BR.md",
        "INTEGRATIONS.md",
        "SECURITY.md",
        "CONTRIBUTING.md",
        "llms.txt",
        "docs/AGENTS.md",
        "docs/COOKBOOK.md",
    ] {
        assert!(
            names.contains(&expected),
            "the walk missed `{expected}`; it found {} documents",
            docs.len()
        );
    }
    assert!(
        docs.len() >= 20,
        "the walk found only {} documents, far below the shipped corpus",
        docs.len()
    );
}

/// The parser inventory must be plausible before anything is judged against it.
#[test]
fn the_parser_inventory_is_not_empty() {
    let flags = shipped_flags();
    let subs = shipped_subcommands();
    assert!(
        flags.len() > 100,
        "only {} flags walked out of the clap tree; the walk is broken",
        flags.len()
    );
    assert!(
        subs.len() >= 40,
        "only {} subcommands walked out of the clap tree",
        subs.len()
    );
    assert!(
        flags.contains("--json"),
        "`--json` must be in the inventory"
    );
    assert!(flags.contains("--db"), "`--db` must be in the inventory");
    assert!(
        subs.contains("remember"),
        "`remember` must be in the inventory"
    );
    assert!(
        !subs.contains("pending"),
        "`pending` was removed in v1.2.8; if it is back, this gate's premise changed"
    );
}

#[test]
fn the_flag_scanner_reaches_a_command_inside_a_fenced_block() {
    let block = "`dockerfile\nFROM rust:1.88\nRUN cargo install --path . --locked`";
    assert!(
        !cited_flags(block).contains("--path"),
        "a cargo flag three lines into a fenced block is still cargo's"
    );
}

#[test]
fn the_flag_scanner_skips_a_foreign_invocation() {
    let doc = "run `cargo test --no-fail-fast` then `sqlite-graphrag list --json`";
    let found = cited_flags(doc);
    assert!(
        !found.contains("--no-fail-fast"),
        "a cargo flag inside a cargo invocation is not this binary's surface"
    );
    assert!(
        found.contains("--json"),
        "the local flag must still be seen"
    );
}

#[test]
fn the_subcommand_scanner_ignores_the_binary_named_mid_sentence() {
    let prose = "`# Wrap a long-running sqlite-graphrag invocation in a handler`";
    assert!(
        cited_subcommands(prose).is_empty(),
        "the binary named inside a comment is not an invocation"
    );
    let commit = "`git commit -m \"chore: track sqlite-graphrag db via LFS\"`";
    assert!(
        cited_subcommands(commit).is_empty(),
        "the binary named inside a commit message is not an invocation"
    );
    let piped = "`echo x | sqlite-graphrag remember --name y`";
    assert!(
        cited_subcommands(piped).contains("remember"),
        "an invocation behind a pipe must still be read"
    );
}

#[test]
fn the_subcommand_scanner_never_reads_a_flag_value_as_a_verb() {
    let doc = "`sqlite-graphrag --embedding-backend openrouter remember --name x`";
    let found = cited_subcommands(doc);
    assert!(
        found.is_empty(),
        "a span whose first token after the binary is a flag must be skipped \
         entirely, got {found:?}"
    );
    let plain = "`sqlite-graphrag recall \"x\" --db /tmp/a.sqlite`";
    assert!(
        cited_subcommands(plain).contains("recall"),
        "the direct form must still be read"
    );
}

#[test]
fn every_foreign_flag_entry_names_its_owner() {
    for (flag, owner) in FLAGS_OF_OTHER_PROGRAMS {
        assert!(
            !owner.trim().is_empty(),
            "`{flag}` is exempted without naming who owns it"
        );
    }
}
