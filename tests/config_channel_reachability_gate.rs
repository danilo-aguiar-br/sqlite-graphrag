//! ACHADO H — the gate against DEAD CONFIGURATION CHANNELS, as a class.
//!
//! Three documented knobs turned out to be unreachable, each for a different
//! reason, and none of them failed anything:
//!
//! - `llm.fallback` — a clap `default_value` always won, so the XDG key was
//!   read and then discarded;
//! - `llm.query_embed_timeout_secs` — its only reader passed a literal `None`
//!   as the flag argument, closing the CLI half of the precedence;
//! - `--openrouter-timeout` — declared on one subcommand, so every other
//!   embedding path was pinned to the compiled default.
//!
//! What they share is the failure MODE, not the cause: `config set` accepted
//! the key, the documentation promised an effect, and the operator got silence.
//! An individual fix for each is worth nothing against the fourth instance, so
//! this file guards the invariant directly.
//!
//! # Why this is a source gate and not a behavioural one
//!
//! Exercising all 62 keys end to end would need one live subcommand invocation
//! per key, each with its own database, provider and side effects — minutes of
//! runtime and a mountain of fixtures for a property that is decidable from the
//! source. The three historical defects were all visible statically: an orphan
//! key, a literal `None`, a missing `global = true`. So is the fourth.

use std::collections::BTreeSet;

/// The registry: single source of truth for what `config set` accepts.
const REGISTRY_SRC: &str = include_str!("../src/config/registry.rs");

/// Every registered key name, in registration order.
fn registered_keys() -> Vec<String> {
    let mut keys = Vec::new();
    for chunk in REGISTRY_SRC.split("key: \"").skip(1) {
        if let Some(end) = chunk.find('"') {
            keys.push(chunk[..end].to_string());
        }
    }
    keys
}

/// Every `.rs` file under `src/`, minus the registry itself.
///
/// The registry is excluded because it DECLARES every key by definition; if it
/// counted as a reader, every key would look reachable and the gate would be
/// decorative.
///
/// `exclude_resolver_module` selects between the two scopes this file needs, and
/// they are genuinely different. Asking "is this key READ anywhere?" must
/// INCLUDE `runtime_config.rs`, because that is where most readers live —
/// excluding it reports 22 false orphans. Asking "is this key resolved with a
/// literal `None`?" must EXCLUDE it, because its own unit tests resolve a
/// deliberately unregistered key and that fixture would read as production.
fn crate_sources_excluding_registry(exclude_resolver_module: bool) -> String {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(
        !files.is_empty(),
        "the source walk found nothing — the gate itself is broken, which is \
         exactly how it would go silently blind"
    );
    files
        .iter()
        .filter(|p| !p.ends_with("config/registry.rs"))
        .filter(|p| !(exclude_resolver_module && p.ends_with("src/runtime_config.rs")))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Keys with NO reader anywhere in the crate: `config set` accepts them and
/// nothing consults them.
///
/// This list is DEBT, not an exemption. It exists so the gate can fail on a
/// NEW orphan — the ninth — instead of drowning in the eight that predate it.
/// Every entry here is a documented channel that does nothing today. Wiring or
/// removing one of them means deleting its line; the gate then holds it there.
///
/// The list may only SHRINK. `no_new_dead_configuration_channel` fails on any
/// orphan absent from it, and `dead_channel_debt_list_is_accurate` fails on any
/// entry here that has since gained a reader — so a stale exemption cannot
/// quietly survive its own fix.
const KNOWN_DEAD_CHANNELS: &[&str] = &[
    "enrich.entity_connect.default_limit",
    "enrich.entity_connect.large_ns_limit",
    "enrich.entity_description.grounding_threshold",
    "log.retention_days",
];

/// Every registered key must be READ somewhere, or be declared as known debt.
#[test]
fn no_new_dead_configuration_channel() {
    let sources = crate_sources_excluding_registry(false);
    let known: BTreeSet<&str> = KNOWN_DEAD_CHANNELS.iter().copied().collect();

    let mut orphans = Vec::new();
    for key in registered_keys() {
        if sources.contains(&format!("\"{key}\"")) {
            continue;
        }
        if known.contains(key.as_str()) {
            continue;
        }
        orphans.push(key);
    }
    orphans.sort();
    assert!(
        orphans.is_empty(),
        "these keys are advertised by `config set` and read by NOTHING: {orphans:?}\n\
         An operator who sets one gets silence, which is the exact defect class \
         ACHADO H closed. Wire a reader, or delete the key from SETTING_KEYS. \
         Adding it to KNOWN_DEAD_CHANNELS is not a fix and will be rejected in \
         review — that list may only shrink."
    );
}

/// The debt list must describe reality: an entry that gained a reader is a fix
/// that nobody finished recording, and it hides the next orphan.
#[test]
fn dead_channel_debt_list_is_accurate() {
    let sources = crate_sources_excluding_registry(false);
    let registered: BTreeSet<String> = registered_keys().into_iter().collect();

    let mut resurrected = Vec::new();
    let mut unregistered = Vec::new();
    for key in KNOWN_DEAD_CHANNELS {
        if sources.contains(&format!("\"{key}\"")) {
            resurrected.push(*key);
        }
        if !registered.contains(*key) {
            unregistered.push(*key);
        }
    }
    assert!(
        resurrected.is_empty(),
        "these keys now HAVE a reader and must be removed from \
         KNOWN_DEAD_CHANNELS: {resurrected:?}"
    );
    assert!(
        unregistered.is_empty(),
        "these keys are listed as dead but are no longer registered at all; \
         drop them from KNOWN_DEAD_CHANNELS: {unregistered:?}"
    );
}

/// A `resolve_*` call whose flag argument is a literal `None` closes the CLI
/// half of the precedence for that key.
///
/// This is the `llm.query_embed_timeout_secs` defect verbatim: the key was
/// registered, it HAD a reader, the reader honoured XDG — and no flag on earth
/// could reach it, because the override slot was hard-coded to `None`.
///
/// A literal `None` is legitimate for a key with no CLI flag by design. What is
/// never legitimate is doing it by accident, so each one must be declared here.
const DECLARED_XDG_ONLY_RESOLVERS: &[&str] = &[
    "enrich.entity_description.corpus_top_k",
    "enrich.entity_description.min_corpus_chars",
    "enrich.entity_description.snippet_chars",
    "enrich.yield_every_n_items",
];

/// Every key resolved with a literal `None` override must be declared XDG-only.
#[test]
fn literal_none_override_is_always_a_declared_choice() {
    let sources = crate_sources_excluding_registry(true);
    // Whitespace-insensitive so a rustfmt line wrap cannot hide a call.
    let packed: String = sources.chars().filter(|c| !c.is_whitespace()).collect();
    let declared: BTreeSet<&str> = DECLARED_XDG_ONLY_RESOLVERS.iter().copied().collect();
    let known_dead: BTreeSet<&str> = KNOWN_DEAD_CHANNELS.iter().copied().collect();

    let mut undeclared = Vec::new();
    for resolver in [
        "resolve_u64(",
        "resolve_usize(",
        "resolve_string(",
        "resolve_bool(",
    ] {
        for chunk in packed.split(resolver).skip(1) {
            let Some(rest) = chunk.strip_prefix("None,\"") else {
                continue;
            };
            let Some(end) = rest.find('"') else { continue };
            let key = &rest[..end];
            if declared.contains(key) || known_dead.contains(key) {
                continue;
            }
            undeclared.push(key.to_string());
        }
    }
    undeclared.sort();
    undeclared.dedup();
    assert!(
        undeclared.is_empty(),
        "these keys are resolved with a hard-coded `None` override, so NO CLI \
         flag can ever reach them: {undeclared:?}\n\
         If that is deliberate, add the key to DECLARED_XDG_ONLY_RESOLVERS and \
         say so in its doc. If it is not, thread the flag through — this is the \
         `llm.query_embed_timeout_secs` defect."
    );
}

/// The XDG-only declarations must stay true: a key that gained a real flag
/// should stop being listed, or the list becomes folklore.
#[test]
fn xdg_only_declarations_still_describe_a_literal_none_call() {
    let sources = crate_sources_excluding_registry(true);
    let packed: String = sources.chars().filter(|c| !c.is_whitespace()).collect();

    let mut stale = Vec::new();
    for key in DECLARED_XDG_ONLY_RESOLVERS {
        if !packed.contains(&format!("None,\"{key}\"")) {
            stale.push(*key);
        }
    }
    assert!(
        stale.is_empty(),
        "these keys are declared XDG-only but no longer have a literal `None` \
         resolver; drop them from DECLARED_XDG_ONLY_RESOLVERS: {stale:?}"
    );
}

/// Every key in the registry must also carry a documented default or an
/// explicit `None`, so `config doctor` can always state what is in force.
#[test]
fn every_registered_key_declares_its_default() {
    let entries = REGISTRY_SRC.matches("SettingKey {").count();
    let defaults = REGISTRY_SRC.matches("default:").count();
    assert_eq!(
        entries, defaults,
        "every SettingKey must state a default (`Some(..)`) or its absence \
         (`None`); {entries} entries carry {defaults} defaults"
    );
}

/// `--openrouter-timeout` must stay GLOBAL.
///
/// The third historical instance: declared on `enrich` alone, it left every
/// other embedding path pinned to the compiled default with no recourse, and a
/// slow provider became exit 11.
#[test]
fn openrouter_timeout_is_a_global_flag_with_no_clap_default() {
    let globals = include_str!("../src/cli/globals.rs");
    let decl = globals
        .split("pub openrouter_timeout:")
        .next()
        .and_then(|before| before.rfind("#[arg(").map(|i| before[i..].to_string()))
        .expect("`openrouter_timeout` must be declared on the global Cli struct");
    assert!(
        decl.contains("global = true"),
        "`--openrouter-timeout` must be global, or it reaches only one subcommand"
    );
    assert!(
        !decl.contains("default_value"),
        "a clap default would always beat the XDG key, which is the \
         `llm.fallback` defect"
    );
}

/// `llm.fallback` must not regain a clap default.
///
/// The first historical instance, and the cheapest to reintroduce: adding
/// `default_value` back makes the flag always `Some`, so `resolve_string`
/// returns before it ever consults XDG.
#[test]
fn llm_fallback_has_no_clap_default() {
    let globals = include_str!("../src/cli/globals.rs");
    let decl = globals
        .split("pub llm_fallback:")
        .next()
        .and_then(|before| before.rfind("#[arg(").map(|i| before[i..].to_string()))
        .expect("`llm_fallback` must be declared on the global Cli struct");
    assert!(
        !decl.contains("default_value"),
        "`--llm-fallback` regained a clap default; the XDG key `llm.fallback` \
         is now unreachable again"
    );
    assert!(
        globals.contains("pub llm_fallback: Option<String>"),
        "`llm_fallback` must stay optional so an omitted flag is distinguishable"
    );
}
