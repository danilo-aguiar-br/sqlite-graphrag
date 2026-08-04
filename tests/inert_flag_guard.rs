//! Every global flag must reach a consumer. A flag that parses and is then
//! discarded is worse than a missing one: the operator gets no error, no
//! warning, and a false belief that they configured something.
//!
//! This is the mirror image of `tests/config_channel_reachability_gate.rs`.
//! That file asks whether every XDG key has a READER — the direction where the
//! text promises a channel the code ignores. This file asks whether every CLI
//! field has a CONSUMER — the direction where a flag promises an effect the
//! code no longer produces. Sweeping one direction leaves half the class open,
//! and v1.2.2 found an instance in each:
//!
//! * `--strict-env-clear` flowed through three layers — declaration, copy into
//!   `RuntimeOverrides`, field on `runtime_config` — and no one read the field.
//!   Four `rg` hits across four files, all plumbing, zero consumers.
//! * `--extraction-backend` was worse: ten shipped documents described its four
//!   values, and `src/` held exactly one mention, its own declaration.
//!
//! Counting mentions is what hid both. Plumbing produces a hit at every layer,
//! so the identifier looks alive all the way down.
//!
//! Until v1.2.5 this guard replaced counting with "does the name appear ANYWHERE
//! outside the declaring file" — a cheaper proxy, and still a proxy. GAP-SG-227
//! walked straight through it: `--fail-on-degraded` was parsed, stored, and
//! never read, while the name appeared in `src/query_embedding.rs` as the
//! PARAMETER of a decider that only the test suite ever called. Real text, real
//! file, zero effect. Two other gates agreed it was fine.
//!
//! The predicate now asks who READS the value off the parsed `Cli`, which is the
//! question the sibling test `the_removed_flags_stay_removed` said was the right
//! one all along. A parameter that merely shares the field's name no longer
//! counts, and `a_parameter_named_after_a_field_is_not_a_consumer` holds that
//! line with the exact shape that got through.

use std::collections::BTreeSet;
use std::path::Path;

/// File that declares the global flag surface; mentions here never count.
const DECLARATION_FILE: &str = "src/cli/globals.rs";

/// Fields read from raw `std::env::args` before `Cli::parse` ever runs.
///
/// `verbose` and `quiet` are pre-parsed because tracing has to be live before a
/// parse error can be reported. `config_dir` and `cache_dir` are pre-parsed
/// because the XDG roots have to be resolved before anything reads a path
/// (`src/main.rs`, the `take_value("--config-dir", ..)` loop). All four are
/// consumed by matching the flag STRING, never the struct field, so no
/// field-access guard can see them.
const CLAP_ONLY_FIELDS: &[&str] = &["verbose", "quiet", "config_dir", "cache_dir"];

/// Reads every Rust source file except the declaration itself.
fn sources_excluding_declaration() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut joined = String::new();
    let mut stack = vec![root.join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            if path.ends_with(Path::new(DECLARATION_FILE).file_name().unwrap())
                && path.to_string_lossy().contains("cli")
            {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                joined.push_str(&text);
                joined.push('\n');
            }
        }
    }
    joined
}

/// Extracts the `pub <name>:` field names declared in the `Cli` struct.
fn declared_cli_fields() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DECLARATION_FILE);
    let text = std::fs::read_to_string(&path).expect("globals.rs must be readable");
    let start = text
        .find("pub struct Cli {")
        .expect("the Cli struct must be declared in globals.rs");
    let body = &text[start..];

    let mut fields = BTreeSet::new();
    for line in body.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("pub ") else {
            continue;
        };
        let Some((name, _)) = rest.split_once(':') else {
            continue;
        };
        // `pub command: Option<Commands>` closes the struct in practice; keep it
        // anyway, it is consumed by `main` like any other field.
        if name.chars().all(|c| c.is_ascii_lowercase() || c == '_') && !name.is_empty() {
            fields.insert(name.to_string());
        }
    }
    fields
}

#[test]
fn the_guard_actually_found_the_cli_fields() {
    let fields = declared_cli_fields();
    assert!(
        fields.len() > 20,
        "only {} fields parsed out of the Cli struct, so the extraction broke \
         and every other assertion here would pass by not looking: {fields:?}",
        fields.len()
    );
    assert!(
        fields.contains("fail_on_degraded"),
        "a known global flag is missing from the extraction: {fields:?}"
    );
}

/// Whether `field` is READ off the parsed `Cli`, rather than merely mentioned.
///
/// Two shapes count, and they are the only two the codebase actually uses:
///
/// * `cli.<field>` anywhere outside the declaration — `main` dispatching the
///   value into a command.
/// * `self.<field>` inside the declaration — an accessor on `Cli` that hands the
///   value out, which is how every agent-surface knob reaches `AgentSurface`.
///
/// Anything else is a mention, and a mention is what let GAP-SG-227 ship.
fn reaches_a_consumer(field: &str, sources: &str, declaration: &str) -> bool {
    sources.contains(&format!("cli.{field}")) || declaration.contains(&format!("self.{field}"))
}

#[test]
fn every_global_flag_reaches_a_consumer() {
    let fields = declared_cli_fields();
    let sources = sources_excluding_declaration();
    let declaration =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(DECLARATION_FILE))
            .expect("globals.rs must be readable");

    let mut inert = Vec::new();
    for field in &fields {
        if CLAP_ONLY_FIELDS.contains(&field.as_str()) {
            continue;
        }
        if !reaches_a_consumer(field, &sources, &declaration) {
            inert.push(field.clone());
        }
    }

    assert!(
        inert.is_empty(),
        "these global flags parse and are then discarded — nothing READS them \
         off the parsed `Cli`. A flag that promises an effect it cannot have is \
         worse than no flag: the operator gets no error and believes the setting \
         took. Remove them, or wire them to a consumer.\n{inert:?}"
    );
}

/// The guard must reject a mention that is not a read.
///
/// GAP-SG-227: `--fail-on-degraded` shipped inert for exactly this reason. The
/// predicate used to be "does the name appear anywhere outside the declaration",
/// and the name appeared in `src/query_embedding.rs` — as the PARAMETER of
/// `degradation_failure`, and inside its body, in a function no code path called.
/// Three layers of evidence, all of them real text, none of them a consumer:
/// the flag was parsed, stored, and then dropped on the floor while a fully
/// documented and unit-tested decider sat next to it, never asked.
///
/// A parameter named after a field is the cheapest way to fool a name search,
/// and it is not hypothetical — it is what happened. This asserts the fix.
#[test]
fn a_parameter_named_after_a_field_is_not_a_consumer() {
    let mention_only = "pub fn decide(fail_on_degraded: bool) -> bool { !fail_on_degraded }";
    assert!(
        !reaches_a_consumer("fail_on_degraded", mention_only, ""),
        "a parameter named after the field is a mention, not a read; the guard \
         accepted exactly this shape and let an inert flag ship"
    );
    assert!(
        reaches_a_consumer("fail_on_degraded", "run(args, cli.fail_on_degraded)", ""),
        "reading the value off the parsed Cli must count as a consumer"
    );
    assert!(
        reaches_a_consumer("select", "", "let v = self.select.clone();"),
        "an accessor on Cli must count: it is how the agent-surface knobs flow"
    );
}

#[test]
fn the_removed_flags_stay_removed() {
    // Both were found in v1.2.2 by asking who READS the field instead of how
    // many times the identifier appears. Re-adding either without a consumer
    // must fail here rather than ship as a promise the code cannot keep.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DECLARATION_FILE);
    let text = std::fs::read_to_string(&path).expect("globals.rs must be readable");
    for gone in ["pub strict_env_clear", "pub extraction_backend"] {
        assert!(
            !text.contains(gone),
            "{gone} was removed in v1.2.2 because nothing consumed it; \
             re-adding it needs a consumer first"
        );
    }
}

/// A help text must not describe a mode the binary cannot enter.
///
/// GAP-SG-204: `enrich --llm-parallelism` documented itself as inert "with
/// `--mode openrouter`" and added that it "applies only to the subprocess
/// modes". v1.2.0 deleted those backends, so `EnrichMode` has exactly one
/// variant and there is no mode to switch into. The sentence reads as a live
/// capability gated behind a selection the operator merely has to make.
///
/// This is the same failure class the rest of this file guards, one level up:
/// the flag above is inert and SAYS so, while the prose around it promises a
/// way to make it work. A guard that only asks "is it wired?" cannot see that.
#[test]
fn no_help_text_offers_a_mode_the_binary_does_not_have() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let args = std::fs::read_to_string(root.join("src/commands/enrich/args.rs"))
        .expect("enrich args.rs must be readable");

    // Anchor on the enum itself, so this relaxes automatically if a second
    // mode is ever reintroduced with a real implementation behind it.
    let modes = args
        .split("pub enum EnrichMode {")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .map(|body| body.matches("#[value(name =").count())
        .expect("EnrichMode must be declared in enrich/args.rs");

    if modes > 1 {
        return;
    }

    let help = run_enrich_help();
    for forbidden in ["subprocess mode", "subprocess modes"] {
        assert!(
            !help.to_lowercase().contains(forbidden),
            "`enrich --help` names a `{forbidden}` while EnrichMode has a single \
             variant, so no such mode can be selected. Either restore the mode \
             or stop advertising it:\n{help}"
        );
    }
}

/// Renders `enrich --help` from the built binary.
///
/// The RENDER, not the doc comment: clap rewraps and hides text, and what the
/// operator reads is the render.
fn run_enrich_help() -> String {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sqlite-graphrag"))
        .args(["enrich", "--help"])
        .output()
        .expect("failed to run the built binary");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}
