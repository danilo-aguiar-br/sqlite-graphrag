//! NC-3 / B7 class: operational docs must not OFFER `OPENROUTER_API_KEY`
//! as a product configuration channel. The runtime ignores that env var
//! (G-T-XDG-04); docs that teach `export OPENROUTER_API_KEY=...` re-open the
//! class the clap help gate already closed.
//!
//! Historical CHANGELOG entries, decisions ADRs, gaps.md and this test file
//! itself are allowlisted. A sentence that names the variable only to DENY
//! it is accepted (v1.2.4: walk includes llms*, SECURITY*, CONTRIBUTING*).

use std::fs;
use std::path::PathBuf;

const BANNED: &str = "OPENROUTER_API_KEY";

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

/// Paths that may still name the variable in historical narrative.
const ALLOWLIST_PREFIXES: &[&str] = &[
    "CHANGELOG.md",
    "CHANGELOG.pt-BR.md",
    "tests/",
    "src/",
    "gaps.md",
    "docs/decisions/",
    "docs/TEST_PLAN",
    "docs/TESTING",
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

#[test]
fn operational_markdown_does_not_offer_openrouter_api_key_env() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut offences = Vec::new();
    // v1.2.4: widen the walk so class B7 cannot hide in llms* / SECURITY / CONTRIBUTING.
    let walk = [
        "README.md",
        "README.pt-BR.md",
        "INTEGRATIONS.md",
        "INTEGRATIONS.pt-BR.md",
        "SECURITY.md",
        "SECURITY.pt-BR.md",
        "CONTRIBUTING.md",
        "CONTRIBUTING.pt-BR.md",
        "llms.txt",
        "llms-full.txt",
        "llms.pt-BR.txt",
        "docs/AGENTS.md",
        "docs/AGENTS.pt-BR.md",
        "docs/HOW_TO_USE.md",
        "docs/HOW_TO_USE.pt-BR.md",
        "docs/HEADLESS_INVOCATION.md",
        "docs/HEADLESS_INVOCATION.pt-BR.md",
        "docs/MIGRATION.md",
        "docs/MIGRATION.pt-BR.md",
        "docs/COOKBOOK.md",
        "docs/COOKBOOK.pt-BR.md",
    ];
    for rel in walk {
        if is_allowlisted(rel) {
            continue;
        }
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap_or_default();
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
