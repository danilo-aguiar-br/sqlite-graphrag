//! NC-3 / B7 class: operational docs must not OFFER product environment
//! variables as a configuration channel. The runtime ignores them
//! (G-T-XDG-04); docs that teach `export SQLITE_GRAPHRAG_...=` or
//! `export OPENROUTER_API_KEY=...` re-open the class the clap help gate
//! already closed.
//!
//! Historical CHANGELOG entries, decisions ADRs, gaps.md and this test file
//! itself are allowlisted. A sentence that names the variable only to DENY
//! it is accepted (v1.2.4: walk includes llms*, SECURITY*, CONTRIBUTING*).

use std::fs;
use std::path::{Path, PathBuf};

const BANNED: &str = "OPENROUTER_API_KEY";

/// GAP-SG-292: the whole product env family, not one literal.
///
/// The gate below bans a single string. That covered the example which
/// motivated it and nothing else: `SQLITE_GRAPHRAG_NAMESPACE`,
/// `SQLITE_GRAPHRAG_DB_PATH` and twenty-six more siblings were offered as
/// working configuration across the operational corpus while this file stayed
/// green. The runtime reads exactly three environment variables —
/// `CLICOLOR_FORCE`, `NO_COLOR` and `XDG_RUNTIME_DIR` — so each of those
/// sentences instructs an action the binary ignores with exit 0.
///
/// A silently ignored instruction is worse than a rejected one: the reader
/// gets no error to react to. `SQLITE_GRAPHRAG_NAMESPACE=x namespace-detect`
/// still answers `global`/`default`, and two documents called that a
/// "golden tip".
const BANNED_FAMILY: &str = "SQLITE_GRAPHRAG_";

const DENIAL_MARKERS: &[&str] = &[
    "not read",
    "never read",
    "is ignored",
    "are ignored",
    "must not be used",
    "não é lida",
    "nao e lida",
    "não são lidas",
    "nunca lê",
    "nunca le",
    "ignored at runtime",
    "product never reads",
    "product env ignored",
    "product env is not",
    "env de produto ignorada",
    "ignora",
    "ignored",
];

/// Denials accepted for [`BANNED_FAMILY`].
///
/// Deliberately NOT [`DENIAL_MARKERS`]: that list contains the bare substrings
/// `ignora` and `ignored`, which any unrelated sentence can carry. A permissive
/// marker is how a gate reports green over a corpus it never really read, so
/// the family gate asks for a phrase that denies THE CHANNEL.
const FAMILY_DENIAL_MARKERS: &[&str] = &[
    "not read",
    "never read",
    "not the config path",
    "no effect",
    "have no effect",
    "has no effect",
    "silently ignored",
    "ignored at runtime",
    "forbidden",
    "deprecated",
    "removed",
    "historical",
    "não é lida",
    "nao e lida",
    "não é lido",
    "não são lidas",
    "nao sao lidas",
    "sem efeito",
    "não tem efeito",
    "nao tem efeito",
    "proibid",
    "obsolet",
    "históric",
    "historic",
    "descontinuad",
    // A prohibition is a denial. The skills corpus writes the rule as
    // "NEVER use `SQLITE_GRAPHRAG_*` as configuration", which offers nothing.
    "never use",
    "nunca use",
    "do not use",
    "don't use",
    "não use",
    "nao use",
    // Denials the corpus actually writes, found by running this gate against
    // it rather than by imagining the phrasing.
    "silenciosamente ignorad",
    "silently ignore",
    "must not advertise",
    "não têm efeito",
    "nao tem efeito",
    "não deve anunciar",
    "nao deve anunciar",
    "no product env",
    "sem product env",
    // Portuguese half of "removed". The list carried the English form only,
    // so a correct Portuguese denial was reported as an offence.
    "removid",
    "retirad",
];

/// Paths that may still name the variable in historical narrative.
const ALLOWLIST_PREFIXES: &[&str] = &[
    "CHANGELOG.md",
    "CHANGELOG.pt-BR.md",
    "tests/",
    "src/",
    "gaps.md",
    "CLAUDE.md",
    // GAP-SG-296 — this entry USED TO BE a false claim about its own subject.
    //
    // The line above says these paths "may still name the variable in
    // historical narrative". For `CHANGELOG.md` and `gaps.md` that was true.
    // For `docs/decisions/` it was NOT: measured on 2026-08-21 across all 129
    // files, 155 lines in 30 ADRs did not NARRATE the dead surface, they
    // INSTRUCTED the reader to use it in the present tense. `adr-0034` carried
    // `SQLITE_GRAPHRAG_IGNORE_SHUTDOWN=1` inside a copyable bash fence;
    // `adr-0045` documented `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1` "for
    // emergencies"; `adr-0051` had a whole `## Env Vars` section.
    //
    // The exemption is now EARNED rather than assumed: every offending ADR
    // opens with a `HISTORICAL:` block that names the dead channel and denies
    // it on the same line. The allowlist stays because line-by-line denial
    // would mean rewriting the body of a decision record, and a rewritten ADR
    // is a destroyed one.
    //
    // An allowlist entry is a claim that has to survive being read out loud.
    // This one did not, for eleven months, because nobody read it out loud.
    "docs/decisions/",
    "docs/TEST_PLAN",
    "docs/TESTING",
    // GAP-SG-292 — reference and specification material, NOT an operator
    // runbook for this CLI, and the distinction is the whole reason the walk
    // can be widened at all.
    //
    // `docs_prd/openrouter_modelos_texto.md` reproduces OpenRouter's own API
    // documentation, where `Authorization: Bearer $OPENROUTER_API_KEY` is the
    // correct instruction for THAT product. `docs_rules/headless_opencode.md`
    // documents the OpenCode CLI the same way. Banning the string there would
    // be a gate demanding that third-party documentation lie.
    //
    // `docs_rules/prd.md` and `docs_rules/projeto_sqlite-graphrag.md` are the
    // original specification: they describe a daemon, an ONNX runtime and
    // opt-in telemetry, none of which the binary has. They are historical by
    // nature and rewriting them would destroy the record of what was designed.
    // That they read as current to an agent is a real defect, filed separately
    // rather than hidden behind this allowlist.
    "docs_rules/",
    "docs_prd/",
    // A migration guide is a historical document by construction: its job is
    // to tell an operator what the OLD version did so they can undo it. Naming
    // a retired env var there is the content, not a defect — the same reason
    // CHANGELOG has been allowlisted since this gate was written.
    "docs/MIGRATION",
];

fn is_allowlisted(rel: &str) -> bool {
    ALLOWLIST_PREFIXES
        .iter()
        .any(|p| rel.starts_with(p) || rel == *p)
}

fn sentence_offers(sentence: &str) -> bool {
    if !sentence.contains(BANNED) {
        return false;
    }
    let lower = sentence.to_lowercase();
    if DENIAL_MARKERS.iter().any(|m| lower.contains(m)) {
        return false;
    }
    // Historical narrative in version notes is not an operator runbook.
    if lower.contains("until v") || lower.contains("before v") || lower.contains("removed") {
        return false;
    }
    true
}

/// Lowercased line with Markdown emphasis stripped.
///
/// Matching raw Markdown compares against the PRESENTATION, not the content.
/// `llms-full.txt` denies the channel as `product `SQLITE_GRAPHRAG_*` **not**
/// read at runtime` — a perfect denial that a search for `"not read"` misses,
/// because the bold asterisks sit between the two words. The gate reported
/// that sentence as an offence until this normalisation existed.
fn normalise(line: &str) -> String {
    line.chars()
        .filter(|c| !matches!(c, '*' | '_' | '`'))
        .collect::<String>()
        .to_lowercase()
}

/// True when the line names a product env var without denying the channel.
fn line_offers_family(line: &str) -> bool {
    if !line.contains(BANNED_FAMILY) {
        return false;
    }
    let normalised = normalise(line);
    !FAMILY_DENIAL_MARKERS.iter().any(|m| normalised.contains(m))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every Markdown and `llms*.txt` file in the corpus, found by walking rather
/// than by a hand-kept list.
///
/// The single-literal gate walks twenty-two named paths. A list cannot grow
/// when a document is added, so a new file joins the corpus already outside
/// the gate — which is how `docs/` accumulated offences nobody was told about.
fn operational_documents() -> Vec<(String, String)> {
    let root = repo_root();
    let mut out = Vec::new();
    collect(&root, &root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
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

#[test]
fn operational_markdown_does_not_offer_openrouter_api_key_env() {
    let mut offences = Vec::new();
    for (rel, text) in operational_documents() {
        for (i, line) in text.lines().enumerate() {
            if sentence_offers(line) {
                offences.push(format!("{rel}:{}: {line}", i + 1));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "operational docs still offer OPENROUTER_API_KEY as a channel (use config add-key):\n{}",
        offences.join("\n")
    );
}

#[test]
fn operational_documents_do_not_offer_the_product_env_family() {
    let mut offences = Vec::new();
    for (rel, text) in operational_documents() {
        for (i, line) in text.lines().enumerate() {
            if line_offers_family(line) {
                offences.push(format!("{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "operational docs offer product env vars the runtime never reads. \
         Configuration is CLI flag > XDG `config set` > default. \
         Rewrite the sentence to name the flag or the XDG key, or mark the \
         mention as historical/denied:\n{}",
        offences.join("\n")
    );
}

/// The walk must actually reach the corpus.
///
/// Without this a typo in the filter silently empties the walk and both gates
/// above report green over nothing — the failure mode this repository recorded
/// as GAP-SG-288 and again as GAP-SG-291.
#[test]
fn the_walk_reaches_the_corpus_it_is_supposed_to_read() {
    let docs = operational_documents();
    let names: Vec<&str> = docs.iter().map(|(rel, _)| rel.as_str()).collect();
    for expected in [
        "README.md",
        "README.pt-BR.md",
        "INTEGRATIONS.md",
        "llms.txt",
        "llms-full.txt",
        "docs/AGENTS.md",
    ] {
        assert!(
            names.contains(&expected),
            "the walk missed `{expected}`; it found {} documents",
            docs.len()
        );
    }
    assert!(
        docs.len() >= 25,
        "the walk found only {} documents, far below the shipped corpus",
        docs.len()
    );
    for (rel, _) in &docs {
        assert!(
            !is_allowlisted(rel),
            "`{rel}` is allowlisted and must never enter the walk"
        );
    }
}

/// The family denial list must be stricter than the single-literal one.
#[test]
fn markdown_emphasis_does_not_hide_a_denial() {
    let bolded = "- product `SQLITE_GRAPHRAG_*` is **not** read at runtime";
    assert!(
        !line_offers_family(bolded),
        "bold markers between the words of a denial must not resurrect the offence"
    );
}

#[test]
fn a_bare_ignora_does_not_disarm_the_family_gate() {
    let line = "- Use `SQLITE_GRAPHRAG_NAMESPACE=proj` to scope memory; o resto se ignora";
    assert!(
        line_offers_family(line),
        "a sentence carrying the loose marker still offers the channel"
    );
    let denied = "- Product env `SQLITE_GRAPHRAG_DB_PATH` is not read at runtime";
    assert!(
        !line_offers_family(denied),
        "a sentence that denies the channel must be accepted"
    );
}
