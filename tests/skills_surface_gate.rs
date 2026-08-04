//! The two agent skills teach a surface that nothing checked.
//!
//! `tests/docs_command_coverage.rs` already asserts both `SKILL.md` files NAME
//! every subcommand. Naming is not the same claim as EXISTING: a skill can list
//! all fifty-one verbs correctly and, in the same paragraph, instruct an agent
//! to pass `--extraction-backend`, set the XDG key `spawn.skip_preflight` and
//! branch on exit 16. All three were taught in both languages, and none of the
//! three exists in this binary.
//!
//! That is the failure mode this gate closes. It is worse than a stale sentence
//! in a guide, because a skill is loaded INTO an agent as procedure: a fabricated
//! flag becomes an invocation that dies with exit 2, and a fabricated exit code
//! becomes a retry branch that never fires.
//!
//! # Why clap and not the help text
//!
//! `shipped_commands()` in the sibling gate reads `--help` on purpose, because
//! the question there is what an operator SEES. The question here is what the
//! parser ACCEPTS, which is a superset: `debug-schema` is hidden from help and
//! still works, and a skill documenting it is correct. Walking the clap tree
//! answers the accepted-argument question directly and costs no subprocess.

use clap::CommandFactory;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use sqlite_graphrag::cli::Cli;

/// The skill files this gate reads.
const SKILLS: [&str; 2] = [
    "skills/sqlite-graphrag-en/SKILL.md",
    "skills/sqlite-graphrag-pt/SKILL.md",
];

/// Marker that opens a registry entry in `src/config/registry.rs`.
const ENTRY_MARKER: &str = "SettingKey {";

/// Field that carries the key name inside a registry entry.
const KEY_FIELD: &str = "key: \"";

/// Long flags a skill may name although clap does not define them.
///
/// Empty by construction: every entry here is a hole, so an addition has to be
/// argued rather than typed. Keeping the list present and empty is deliberate —
/// a future exemption gets a reason next to it instead of a silently widened
/// scanner.
const FLAGS_NOT_IN_CLAP: [&str; 0] = [];

/// Dotted tokens that share a registry namespace without being config keys.
///
/// `agent_surface` names both a family of three settings and the diagnostic
/// object the agent-native surface writes into every envelope. A skill teaching
/// an agent to read `agent_surface.secondary_capped` is correct — the field is
/// emitted by `src/agent_surface/mod.rs` — and a namespace-based scanner cannot
/// tell the two apart from the token alone.
///
/// Each entry states where the field is produced, so the exemption stays
/// checkable instead of becoming a place to hide a typo.
const ENVELOPE_FIELDS_SHARING_A_NAMESPACE: [(&str, &str); 1] = [(
    "agent_surface.secondary_capped",
    "envelope diagnostic written by src/agent_surface/mod.rs, not a setting",
)];

fn read_repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
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

/// Every configuration key the registry declares.
fn registry_keys() -> BTreeSet<String> {
    let source = read_repo_file("src/config/registry.rs");
    let mut keys = BTreeSet::new();
    for entry in source.split(ENTRY_MARKER).skip(1) {
        let Some(start) = entry.find(KEY_FIELD) else {
            continue;
        };
        let rest = &entry[start + KEY_FIELD.len()..];
        let Some(end) = rest.find('"') else {
            continue;
        };
        keys.insert(rest[..end].to_string());
    }
    keys
}

/// Every backticked span of a markdown document.
///
/// Splitting on the backtick and taking every other piece is what the XDG guard
/// already does, and it is right for the same reason: these documents put every
/// copyable token in a code span, so unmarked prose is commentary.
fn code_spans(markdown: &str) -> Vec<&str> {
    markdown.split('`').skip(1).step_by(2).collect()
}

/// Long flags a document names inside a code span.
///
/// A span may hold a whole invocation, so every `--token` inside it counts, not
/// only a span that IS a flag. Trailing punctuation and the `=value` form are
/// trimmed, because `--no-input=false` names `--no-input`.
fn cited_flags(markdown: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for span in code_spans(markdown) {
        for word in span.split_whitespace() {
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
    found
}

/// Words whose presence on a line makes every dotted token on it a config key.
const CONFIG_CONTEXT: [&str; 4] = ["config set", "config get", "config unset", "XDG"];

/// Configuration keys a document names inside a code span.
///
/// TWO rules, because one of them alone leaves a hole that this gate was built
/// after walking into.
///
/// Rule A recognises a key by its NAMESPACE, taken from the registry rather
/// than a hardcoded prefix list, so a new family becomes visible the moment it
/// is declared. Rule A catches a typo inside a real family and is blind to an
/// invented family: `spawn.skip_preflight` has no registry namespace, so the
/// namespace test can never see it.
///
/// Rule B closes that: on a line that also says `config set`, `config get`,
/// `config unset` or `XDG`, EVERY dotted lowercase token is a claimed key, no
/// matter how unfamiliar its head. That is the shape the historical defect had
/// — `sqlite-graphrag config set spawn.skip_preflight=1` — and it is the shape
/// any instruction to persist a setting must take.
///
/// What still escapes, stated rather than hidden: a token in an unknown
/// namespace mentioned with no config verb and no `XDG` on the line. Rule B
/// cannot be widened to catch it without flagging `stats.total`, which is a
/// legitimate dotted path for `--select`.
fn cited_keys(markdown: &str, namespaces: &BTreeSet<String>) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for line in markdown.lines() {
        let in_config_context = CONFIG_CONTEXT.iter().any(|marker| line.contains(marker));
        for span in code_spans(line) {
            for word in span.split_whitespace() {
                // A URL is dotted and is never a key.
                if word.contains('/') || word.contains(':') {
                    continue;
                }
                // `key=value` names `key`; the assignment form is how a config
                // instruction is most often written.
                let word = word.split('=').next().unwrap_or(word);
                let candidate = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.');
                let Some((head, tail)) = candidate.split_once('.') else {
                    continue;
                };
                if head.is_empty() || tail.is_empty() {
                    continue;
                }
                // A number is dotted too.
                if candidate.chars().all(|c| c.is_ascii_digit() || c == '.') {
                    continue;
                }
                if namespaces.contains(head) || in_config_context {
                    found.insert(candidate.to_string());
                }
            }
        }
    }
    found
}

/// The namespace of every registry key, so the key scanner needs no hardcoding.
fn registry_namespaces() -> BTreeSet<String> {
    registry_keys()
        .iter()
        .filter_map(|k| k.split_once('.').map(|(head, _)| head.to_string()))
        .collect()
}

/// How many NDJSON contracts `schema` emits.
fn shipped_schema_count() -> usize {
    let output = Command::new(env!("CARGO_BIN_EXE_sqlite-graphrag"))
        .arg("schema")
        .output()
        .expect("cannot run the binary to read its schema catalogue");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// How many subcommands the top-level help lists, `help` included.
///
/// `help` counts here and is excluded by the sibling gate, and the difference is
/// not an inconsistency: that gate asks which commands must be DOCUMENTED, and
/// clap's generated `help` is not a product surface. This one checks a COUNT the
/// skills print, and the skills print the number an operator sees in `--help`.
fn shipped_command_count() -> usize {
    let output = Command::new(env!("CARGO_BIN_EXE_sqlite-graphrag"))
        .arg("--help")
        .output()
        .expect("cannot run the binary to read its command inventory");
    let help = String::from_utf8_lossy(&output.stdout);
    let mut count = 0;
    let mut inside = false;
    for line in help.lines() {
        if line.starts_with("Commands:") {
            inside = true;
            continue;
        }
        if inside {
            if line.trim().is_empty() || line.starts_with("Options:") {
                break;
            }
            if !line.starts_with("  ") {
                continue;
            }
            if let Some(name) = line.split_whitespace().next() {
                if name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                    count += 1;
                }
            }
        }
    }
    count
}

#[test]
fn every_flag_a_skill_teaches_is_a_flag_the_parser_accepts() {
    let shipped = shipped_flags();
    let mut fabricated = Vec::new();

    for skill in SKILLS {
        let text = read_repo_file(skill);
        for flag in cited_flags(&text) {
            if !shipped.contains(&flag) && !FLAGS_NOT_IN_CLAP.contains(&flag.as_str()) {
                fabricated.push(format!("{skill}: {flag}"));
            }
        }
    }

    assert!(
        fabricated.is_empty(),
        "these skills instruct an agent to pass a flag this binary does not \
         define. A skill is loaded INTO the agent as procedure, so a fabricated \
         flag is not a stale sentence — it is an invocation that dies with exit \
         2 on first use. `--extraction-backend`, `--strict-env-clear` and \
         `--graceful-shutdown-secs` were taught in both languages this way.\n\
         Remove the flag, or name the real one.\n{}",
        fabricated.join("\n")
    );
}

#[test]
fn every_config_key_a_skill_teaches_exists_in_the_registry() {
    let shipped = registry_keys();
    let namespaces = registry_namespaces();
    let mut fabricated = Vec::new();

    for skill in SKILLS {
        let text = read_repo_file(skill);
        for key in cited_keys(&text, &namespaces) {
            let exempt = ENVELOPE_FIELDS_SHARING_A_NAMESPACE
                .iter()
                .any(|(token, _)| *token == key);
            if !shipped.contains(&key) && !exempt {
                fabricated.push(format!("{skill}: {key}"));
            }
        }
    }

    assert!(
        fabricated.is_empty(),
        "these skills instruct an agent to `config set` a key the registry does \
         not declare. `config set` on an unknown key exits 1, so the agent is \
         being taught a remediation that cannot run — `spawn.skip_preflight` was \
         taught in both languages as the emergency escape hatch and never \
         existed.\n{}",
        fabricated.join("\n")
    );
}

#[test]
fn every_count_a_skill_declares_matches_the_binary() {
    let commands = shipped_command_count();
    let keys = registry_keys().len();
    let schemas = shipped_schema_count();

    // Each probe is the phrase the skill actually prints, in both languages.
    // Asserting the phrase and not a bare digit is what makes the failure name
    // the claim: `74 NDJSON lines` is a promise about output an agent will
    // count, while a loose `74` anywhere in the file proves nothing.
    let probes: [(&str, [String; 2]); 3] = [
        (
            "top-level command count",
            [format!("{commands} verbs"), format!("{commands} verbos")],
        ),
        (
            "XDG registry size",
            [format!("{keys} keys"), format!("{keys} chaves")],
        ),
        (
            "schema catalogue size",
            [
                format!("{schemas} NDJSON lines"),
                format!("{schemas} linhas NDJSON"),
            ],
        ),
    ];

    let mut wrong = Vec::new();
    for (skill, spelling) in SKILLS.iter().zip([0usize, 1usize]) {
        let text = read_repo_file(skill);
        for (what, phrasing) in &probes {
            if !text.contains(&phrasing[spelling]) {
                wrong.push(format!(
                    "{skill}: no occurrence of `{}` for the {what}",
                    phrasing[spelling]
                ));
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "a skill states a count that the binary contradicts, or states it in a \
         wording this gate cannot see. Both readings are defects: an agent told \
         `schema` emits 75 lines will treat a 74-line catalogue as truncated. \
         The counts measured right now are {commands} commands, {keys} config \
         keys and {schemas} schema contracts.\n{}",
        wrong.join("\n")
    );
}

#[test]
fn the_flag_scanner_finds_the_flags_that_are_really_there() {
    // A scanner that matches nothing passes every assertion above by not
    // looking. The corpus carries well over a hundred distinct flags; a floor
    // this high cannot be met by an extractor that broke.
    for skill in SKILLS {
        let found = cited_flags(&read_repo_file(skill));
        assert!(
            found.len() > 100,
            "{skill}: the flag scanner found only {} flags, so it went blind and \
             every fabricated flag would pass unnoticed",
            found.len()
        );
        assert!(
            found.contains("--rest-concurrency"),
            "{skill}: the flag scanner missed `--rest-concurrency`, which the \
             document names inside a full invocation rather than alone"
        );
    }
}

#[test]
fn the_key_scanner_finds_the_keys_that_are_really_there() {
    let namespaces = registry_namespaces();
    for skill in SKILLS {
        let found = cited_keys(&read_repo_file(skill), &namespaces);
        assert!(
            found.len() >= 60,
            "{skill}: the key scanner found only {} keys against a registry of \
             {}, so it stopped recognising the shape the document uses",
            found.len(),
            registry_keys().len()
        );
    }
}

#[test]
fn the_flag_scanner_ignores_prose_and_trims_the_value_form() {
    let sample = "Do not pass --invented outside a span. \
                  Inside one: `--no-input=false` and `sqlite-graphrag x --db P`.";
    let found = cited_flags(sample);
    assert!(
        !found.contains("--invented"),
        "an unmarked word must not be read as a flag"
    );
    assert!(
        found.contains("--no-input"),
        "`--no-input=false` names `--no-input`"
    );
    assert!(
        found.contains("--db"),
        "a flag inside a full invocation must be seen"
    );
}

#[test]
fn every_envelope_exemption_still_names_a_field_the_code_emits() {
    // An exemption that outlives its subject is worse than none: it keeps a
    // hole open while the sentence explaining it stops describing anything.
    let source = read_repo_file("src/agent_surface/mod.rs");
    for (token, reason) in ENVELOPE_FIELDS_SHARING_A_NAMESPACE {
        let field = token.rsplit('.').next().expect("token has a field part");
        assert!(
            source.contains(field),
            "the exemption for `{token}` claims `{reason}`, but \
             src/agent_surface/mod.rs no longer mentions `{field}`; remove the \
             entry or point it at the file that emits the field now"
        );
        assert!(
            reason.len() > 30,
            "the exemption for `{token}` needs a reason someone can check"
        );
    }
}

#[test]
fn the_key_scanner_requires_a_registry_namespace() {
    let namespaces = registry_namespaces();
    let sample = "`embedding.dim` is real, `example.com` is a hostname.";
    let found = cited_keys(sample, &namespaces);
    assert!(found.contains("embedding.dim"), "a real key must be seen");
    assert!(
        !found.contains("example.com"),
        "a dotted token outside every registry namespace is not a key"
    );
}

#[test]
fn the_key_scanner_catches_an_invented_namespace_in_a_config_instruction() {
    // The defect this gate exists for. `spawn` is in no registry namespace, so
    // rule A is structurally blind to it; only the config-verb context sees it.
    let namespaces = registry_namespaces();
    let sample = "- SET the escape hatch via `sqlite-graphrag config set spawn.skip_preflight=1`";
    let found = cited_keys(sample, &namespaces);
    assert!(
        found.contains("spawn.skip_preflight"),
        "an instruction to `config set` an invented key must be caught even \
         though its namespace is unknown; found {found:?}"
    );
}

#[test]
fn the_key_scanner_leaves_a_selection_path_alone() {
    // `stats.total` is a dotted path for `--select`, not a setting. Flagging it
    // is what stops rule B from being widened to every line.
    let namespaces = registry_namespaces();
    let sample = "- PROJECT a nested value with `--select stats.total`";
    let found = cited_keys(sample, &namespaces);
    assert!(
        !found.contains("stats.total"),
        "a dotted projection path outside a config instruction is not a key"
    );
}
