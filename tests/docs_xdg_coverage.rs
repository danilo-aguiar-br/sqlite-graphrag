//! Keeps the published XDG key reference from drifting away from the registry.
//!
//! `src/config/registry.rs` is the only place that decides which keys
//! `config set` accepts. The README pair is where an operator looks them up.
//! Nothing coupled the two, and the gap grew silently in both directions:
//! the v1.2.4 README documented 19 of 61 keys, so 38 knobs were invisible to
//! every reader, while `README.pt-BR.md` advertised `enrich.preserve_threshold`,
//! `enrich.entity_connect.max_runtime_secs` and `llm.concurrency` — three keys
//! that never existed. An operator following that line got exit 1 from a
//! document the project ships as authoritative.
//!
//! Both failure modes are mechanical, so a test can hold the line. Like
//! `docs_consistency.rs`, this is a test rather than a CI job because this
//! project forbids CI by design; `cargo test` is the only automatic gate.

use std::collections::BTreeSet;

/// Marker that opens a registry entry in the source file.
const ENTRY_MARKER: &str = "SettingKey {";

/// Field that carries the key name inside a registry entry.
const KEY_FIELD: &str = "key: \"";

/// Documents that must carry the full reference, one per language.
const REFERENCE_DOCS: [&str; 2] = ["README.md", "README.pt-BR.md"];

/// Prefixes that a documentation token must start with to be judged a config
/// key at all. Anything outside this set is prose, a file name or a flag.
///
/// DERIVED from the registry, never hand-written. GAP-SG-198 shipped this as a
/// fixed array of nineteen and recorded the consequence in its own leftover
/// section: a key in a NEW family was invisible to every scan below, and
/// nothing announced the omission — the guard just quietly stopped covering it.
/// Reading the namespaces off the same list that defines the keys makes a new
/// family self-registering.
fn key_namespaces() -> BTreeSet<String> {
    registry_keys()
        .iter()
        .filter_map(|key| key.split_once('.').map(|(head, _)| format!("{head}.")))
        .collect()
}

/// `true` when `candidate` sits in a namespace the registry actually declares.
fn in_a_known_namespace(candidate: &str) -> bool {
    key_namespaces().iter().any(|ns| candidate.starts_with(ns))
}

/// Keys named in the documentation on purpose while absent from the registry.
///
/// `db.default_path` is the pre-v1.2.0 spelling. The binary rejects it with
/// exit 1 and points at `db.path`, and the migration notes have to keep naming
/// it or a reader upgrading from v1.1.x cannot find the rename.
///
/// The other three were published as usable keys by `README.pt-BR.md` through
/// v1.2.4 and never existed. The v1.2.5 reference names them once more, in a
/// sentence that says they were removed — a reader who copied the old line and
/// got exit 1 needs to find out why. Every entry here is a mention that denies
/// the key, so the guard below can stay strict about every other unknown one.
const DELIBERATE_LEGACY_MENTIONS: [&str; 4] = [
    "db.default_path",
    "enrich.preserve_threshold",
    "enrich.entity_connect.max_runtime_secs",
    "llm.concurrency",
];

/// Reads a repository file relative to the crate root.
fn read_repo_file(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Collects every key the registry declares.
///
/// The registry is a flat array of struct literals, so scanning for the field
/// prefix is both sufficient and immune to reordering. Parsing Rust here would
/// buy nothing: a key that is not spelled as a literal cannot be typed by an
/// operator either.
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

/// Collects every config key a document names inside backticks.
///
/// Only backticked tokens count. Prose mentions a key without markup often
/// enough that a looser scan would flag ordinary sentences, and the reference
/// tables this guard protects are backticked by construction.
fn documented_keys(markdown: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for token in markdown.split('`').skip(1).step_by(2) {
        let candidate = token.trim();
        if !in_a_known_namespace(candidate) {
            continue;
        }
        // A Rust module path shares the dotted shape but ends in `.rs`, and a
        // sentence can trap a trailing comma inside the backticks. A `*` marks
        // prose naming a family (`network.openrouter.*`) rather than a key an
        // operator can type, so it is not a candidate for either direction of
        // this guard.
        if candidate.ends_with(".rs") || candidate.contains(' ') || candidate.contains('*') {
            continue;
        }
        found.insert(candidate.trim_end_matches(['.', ',']).to_string());
    }
    found
}

#[test]
fn every_registry_key_appears_in_both_reference_documents() {
    let registry = registry_keys();
    assert!(
        registry.len() >= 61,
        "registry shrank to {} keys; update this floor deliberately, never to make the guard pass",
        registry.len()
    );

    for doc in REFERENCE_DOCS {
        let documented = documented_keys(&read_repo_file(doc));
        let missing: Vec<&String> = registry.difference(&documented).collect();
        assert!(
            missing.is_empty(),
            "{doc} documents {}/{} config keys; {} are invisible to the reader: {:?}",
            registry.len() - missing.len(),
            registry.len(),
            missing.len(),
            missing
        );
    }
}

#[test]
fn no_reference_document_advertises_a_key_the_binary_rejects() {
    let registry = registry_keys();
    let allowed: BTreeSet<String> = DELIBERATE_LEGACY_MENTIONS
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    for doc in REFERENCE_DOCS {
        let documented = documented_keys(&read_repo_file(doc));
        let ghosts: Vec<&String> = documented
            .difference(&registry)
            .filter(|key| !allowed.contains(*key))
            .collect();
        assert!(
            ghosts.is_empty(),
            "{doc} names {} config key(s) absent from src/config/registry.rs; \
             `config set` answers exit 1 for each: {:?}",
            ghosts.len(),
            ghosts
        );
    }
}

/// Words that mark a line as denying a key rather than teaching it.
///
/// Both languages ship in this repository, so both vocabularies belong here.
/// Stems, not whole words: Portuguese inflects `legado`/`legada`/`legados`,
/// and matching only the feminine form is what made the first run of this
/// guard reject a correct line in `README.pt-BR.md`.
const DENIAL_MARKERS: [&str; 10] = [
    "never existed",
    "nunca existiram",
    "nunca foi",
    "reject",
    "rejeit",
    "exit 1",
    "legac",
    "legad",
    "removed",
    "não é alias",
];

#[test]
fn every_legacy_mention_sits_on_a_line_that_denies_the_key() {
    // Without this, the allowlist above would be a hole: someone could
    // reintroduce `llm.concurrency` as a usable knob and the ghost guard would
    // wave it through. Allowing the word is not the same as allowing the claim.
    for doc in REFERENCE_DOCS {
        let text = read_repo_file(doc);
        for legacy in DELIBERATE_LEGACY_MENTIONS {
            for (index, line) in text.lines().enumerate() {
                if !line.contains(&format!("`{legacy}`")) {
                    continue;
                }
                let lowered = line.to_lowercase();
                assert!(
                    DENIAL_MARKERS.iter().any(|m| lowered.contains(m)),
                    "{doc}:{} names `{legacy}` without denying it; \
                     the binary answers exit 1 for that key, so the line reads as a lie:\n{line}",
                    index + 1
                );
            }
        }
    }
}

#[test]
fn the_key_scanner_separates_a_config_key_from_a_module_path() {
    // `enrich.rs` and `retry.rs` sit in the same namespaces as real keys and
    // appear in prose about the source tree. Confusing the two is what made an
    // earlier hand-rolled sweep report seven ghosts when only four were real.
    let sample = "See `src/commands/enrich.rs` and `retry.rs` for `enrich.scan_page_size`.";
    let found = documented_keys(sample);
    assert!(found.contains("enrich.scan_page_size"));
    assert!(!found.contains("retry.rs"));
    assert_eq!(found.len(), 1, "unexpected extraction: {found:?}");
}

#[test]
fn the_key_scanner_ignores_a_family_glob() {
    // `network.openrouter.*` names a family in prose. Reading it as a key made
    // the first run of this guard fail against a correct README line.
    let sample = "URLs come from XDG `network.openrouter.*`, not `network.chat_url`.";
    let found = documented_keys(sample);
    assert!(!found.contains("network.openrouter.*"));
    assert!(found.contains("network.chat_url"));
}

#[test]
fn the_key_scanner_survives_a_trailing_separator_inside_the_backticks() {
    let sample = "Set `log.level`, `log.format`, and `display.tz`.";
    let found = documented_keys(sample);
    assert!(found.contains("log.level"));
    assert!(found.contains("log.format"));
    assert!(found.contains("display.tz"));
}

/// Every XDG key the binary's own `--help` promises must exist in the registry.
///
/// This is the direction the first version of this guard did not cover, and it
/// is the one three keys walked through. `registry -> README` was checked;
/// `--help -> registry` was not, so `--embedding-backend` could advertise
/// "optional XDG `config set embedding.backend`" while the key was absent, and
/// the documented command answered exit 1 with a did-you-mean. `--llm-backend`
/// promised `llm.backend` the same way, and `enrich --help` named
/// `enrich.max_load_per_ncpu` when the real key is `system.max_load_per_ncpu`.
///
/// The help text is the most authoritative document the project ships: it
/// travels inside the binary and needs no network, so a promise made there and
/// broken by the registry is the worst kind of documentation drift.
/// Dotted names in a registry namespace that are ENVELOPE FIELDS, not keys.
///
/// `agent_surface.` is the one namespace the product uses for both: XDG knobs
/// (`agent_surface.max_items`) and members of the JSON envelope the reshape
/// layer emits. Help text has to name the envelope members — that is how an
/// operator learns where truncation is reported — so a prefix scan alone
/// cannot tell the two apart.
///
/// Listing them beats loosening the scan: every other unknown dotted name in a
/// registry namespace stays a hard failure, which is the property that caught
/// three ghost keys in the first place.
const ENVELOPE_FIELDS_NOT_KEYS: [&str; 2] = [
    "agent_surface.content_truncated",
    "agent_surface.output_truncated",
];

#[test]
fn every_xdg_key_promised_by_help_exists_in_the_registry() {
    let registry = registry_keys();
    let mut promised: BTreeSet<String> = BTreeSet::new();

    for scope in help_texts() {
        for key in xdg_keys_in_help(&scope) {
            if ENVELOPE_FIELDS_NOT_KEYS.contains(&key.as_str()) {
                continue;
            }
            promised.insert(key);
        }
    }

    assert!(
        !promised.is_empty(),
        "extracted zero XDG keys from --help; the scanner is broken, not the docs"
    );

    let ghosts: Vec<&String> = promised.difference(&registry).collect();
    assert!(
        ghosts.is_empty(),
        "`--help` promises {} XDG key(s) that `config set` rejects with exit 1: {:?}\n\
         Either register the key in src/config/registry.rs and resolve it in \
         src/runtime_config.rs, or correct the help text to name the key that exists.",
        ghosts.len(),
        ghosts
    );
}

/// Runs `--help` for the root, for every subcommand it lists, AND for every
/// sub-subcommand those list in turn.
///
/// Reading the rendered help rather than the doc comments is deliberate: clap
/// rewraps, renames and hides text, and what an operator copies is the render.
///
/// The second level is what GAP-SG-203 added. Stopping at depth one meant
/// `config set --help` — the single most likely place to name a config key —
/// was never read, so the three ghost keys this file exists to catch sat in
/// plain sight inside the binary while the guard reported success.
fn help_texts() -> Vec<String> {
    let bin = env!("CARGO_BIN_EXE_sqlite-graphrag");
    let root = run_help(bin, &[]);
    let mut out = vec![root.clone()];
    for name in subcommand_names(&root) {
        let level_one = run_help(bin, &[&name]);
        for leaf in subcommand_names(&level_one) {
            out.push(run_help(bin, &[&name, &leaf]));
        }
        out.push(level_one);
    }
    out
}

/// Extracts the subcommand names a help text lists.
///
/// clap indents command rows by exactly two spaces under `Commands:`, which is
/// what distinguishes them from option rows and from wrapped prose.
fn subcommand_names(help: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in help.lines() {
        let trimmed = line.trim_start();
        if line.len() - trimmed.len() != 2 {
            continue;
        }
        let Some(name) = trimmed.split_whitespace().next() else {
            continue;
        };
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            continue;
        }
        // `help` recurses into itself forever and carries no key text.
        if name == "help" {
            continue;
        }
        out.push(name.to_string());
    }
    out
}

fn run_help(bin: &str, path: &[&str]) -> String {
    let mut cmd = std::process::Command::new(bin);
    cmd.args(path);
    let output = cmd
        .arg("--help")
        .output()
        .expect("failed to run the built binary with --help");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

/// Extracts every dotted key named as an XDG setting in one help text.
///
/// Scans EVERY token, not just the ones trailing an anchor phrase. GAP-SG-198
/// anchored on "XDG " and "config set " only, and GAP-SG-203 found what walked
/// through the gap: the `config set` doc comment introduced its list with
/// "Known keys (non-exhaustive):" and then simply listed backticked keys, so
/// not one of them followed either anchor and all three ghosts went unseen.
///
/// A token in a registry namespace is a key claim wherever it appears in help
/// text — there is no other reason for `enrich.preserve_threshold` to be
/// printed by a binary — so the namespace check is the filter that matters and
/// the anchors were never load-bearing.
fn xdg_keys_in_help(help: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    // Split on backticks too, so `key` inside markup is seen as its own token
    // rather than glued to the punctuation around it.
    for token in help.split([' ', '\n', '\t', '`', '\r']) {
        let candidate = token.trim_matches(['`', '"', '(', ')', ',', '.', ';', ':', '\'']);
        if !in_a_known_namespace(candidate) {
            continue;
        }
        // A `*` marks a family in prose (`network.openrouter.*`), and `.rs` a
        // module path. Neither is a key an operator can type.
        if candidate.contains('*') || candidate.ends_with(".rs") {
            continue;
        }
        // No `break`: GAP-SG-203's third hole. A line naming two keys had its
        // second one silently dropped, which is how a guard reports success
        // while looking at half the evidence.
        found.insert(candidate.to_string());
    }
    found
}

#[test]
fn the_help_scanner_finds_a_key_behind_either_spelling() {
    let a = xdg_keys_in_help("Prefer the flag; optional XDG `embedding.model` here.");
    assert!(
        a.contains("embedding.model"),
        "backticked XDG spelling missed"
    );

    let b = xdg_keys_in_help("Prefer the flag; optional XDG `config set embedding.backend`.");
    assert!(
        b.contains("embedding.backend"),
        "`config set` spelling missed"
    );

    let c = xdg_keys_in_help("falls back to XDG `network.openrouter.*` for the family");
    assert!(c.is_empty(), "a family glob is not a settable key");
}
