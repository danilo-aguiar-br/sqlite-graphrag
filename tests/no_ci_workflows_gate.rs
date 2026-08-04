//! GAP-SG-206: this project forbids CI, and nothing enforced it.
//!
//! The policy is stated in `docs/DOCUMENTATION_FRAMEWORK.md` and repeated in
//! both `CROSS_PLATFORM` documents: no GitHub Actions, no workflows, none to be
//! recreated. It is the reason `cargo test` is the only automatic gate this
//! repository has, and the reason every other guard in `tests/` exists as a
//! test rather than as a pipeline step.
//!
//! A policy with no check is a preference. Two things could break it silently:
//!
//! 1. Someone adds `.github/workflows/ci.yml`. Nothing in the repository
//!    objects, and the file starts running on a service the project does not
//!    use.
//! 2. A document declares a workflow MANDATORY. That already happened —
//!    `DOCUMENTATION_FRAMEWORK.md` carried a completion checklist claiming
//!    `ci.yml` and `release.yml` had been created, on the same page that
//!    forbids them, and GAP-SG-197 corrected only the prohibition half.
//!
//! Both halves are checked here, because fixing the directory while leaving the
//! document telling contributors to create it just delays the next round.

use std::path::{Path, PathBuf};

/// Repository root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Every file under `dir`, recursively, relative to the repository root.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(root()) {
                out.push(rel.display().to_string());
            }
        }
    }
    out.sort();
    out
}

#[test]
fn no_github_workflow_exists() {
    let workflows = root().join(".github").join("workflows");
    if !workflows.exists() {
        return;
    }
    let found = files_under(&workflows);
    assert!(
        found.is_empty(),
        "this project forbids CI, and `.github/workflows/` now holds {} file(s). \
         `cargo test` is the only automatic gate by design; a pipeline here runs \
         on a service the project does not use and splits the definition of \
         green in two:\n{}",
        found.len(),
        found.join("\n")
    );
}

#[test]
fn the_github_directory_holds_only_templates() {
    // Narrower than the test above and it catches a different mistake: a
    // workflow dropped somewhere else under `.github/`, where the first check
    // would never look.
    let github = root().join(".github");
    if !github.exists() {
        return;
    }
    let offenders: Vec<String> = files_under(&github)
        .into_iter()
        .filter(|path| {
            let lowered = path.to_lowercase();
            lowered.ends_with(".yml") || lowered.ends_with(".yaml")
        })
        // `ISSUE_TEMPLATE/config.yml` is GitHub's issue-chooser config, not a
        // workflow: it has no `on:` trigger and executes nothing.
        .filter(|path| !path.contains("ISSUE_TEMPLATE"))
        .collect();
    assert!(
        offenders.is_empty(),
        "YAML under `.github/` outside ISSUE_TEMPLATE/ is a workflow by another \
         name:\n{}",
        offenders.join("\n")
    );
}

/// Documents that must not instruct a contributor to create a pipeline.
const POLICY_DOCS: [&str; 3] = [
    "docs/DOCUMENTATION_FRAMEWORK.md",
    "docs/CROSS_PLATFORM.md",
    "docs/CROSS_PLATFORM.pt-BR.md",
];

/// Words that turn a mention of a workflow into an INSTRUCTION to create one.
///
/// Every one of these documents has to NAME the workflows — that is how a
/// reader learns they are forbidden and why. Matching the name alone would
/// force the policy to be unwritable. What must not appear is a name inside a
/// line that reads as a requirement or as a completed deliverable.
const INSTRUCTION_MARKERS: [&str; 6] = [
    "[x]",
    "obrigatóri",
    "mandatory",
    "required",
    "crie",
    "criado com",
];

/// Words that make a line a DENIAL, overriding any instruction marker on it.
///
/// A sentence explaining that the requirement was WITHDRAWN has to quote the
/// requirement — "até a v1.2.4 esta seção exigia `ci.yml` como OBRIGATÓRIOS" is
/// the correction, not the offence. Without this override the gate would forbid
/// the project from recording why the rule changed, which is the same
/// documenting-by-deleting that `gaps.md` bans.
///
/// Both languages ship here, so both vocabularies belong. Stems, not whole
/// words: Portuguese inflects, and matching one form is what made the first run
/// of the sibling guard in `docs_xdg_coverage.rs` reject a correct line.
const DENIAL_MARKERS: [&str; 12] = [
    "proibido",
    "forbid",
    "nunca",
    "never",
    "retirado",
    "withdrawn",
    "removed",
    "historical only",
    "não existe",
    "no ci",
    "sem github actions",
    "até a v",
];

#[test]
fn no_policy_document_prescribes_a_workflow() {
    for doc in POLICY_DOCS {
        let path = root().join(doc);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            let lowered = line.to_lowercase();
            let names_a_workflow = lowered.contains("workflows/")
                || lowered.contains("ci.yml")
                || lowered.contains("release.yml");
            if !names_a_workflow {
                continue;
            }
            if DENIAL_MARKERS.iter().any(|m| lowered.contains(m)) {
                continue;
            }
            let prescribes = INSTRUCTION_MARKERS.iter().any(|m| lowered.contains(m));
            assert!(
                !prescribes,
                "{doc}:{} names a CI workflow on a line that reads as a \
                 requirement or as a shipped deliverable, in a project that \
                 forbids CI. Naming it to DENY it is fine; naming it to ask for \
                 it is the contradiction GAP-SG-206 recorded:\n{line}",
                index + 1
            );
        }
    }
}

#[test]
fn the_document_scanner_tells_a_denial_from_a_prescription() {
    // Without this, the assertion above could pass by never matching anything,
    // and nobody would notice until a workflow shipped.
    let denial = "- PROIBIDO `ci.yml`, `release.yml` ou qualquer GitHub Action";
    let prescription = "- [x] .github/workflows/ci.yml criado com pipeline multi-OS";

    let lowered_denial = denial.to_lowercase();
    assert!(
        !INSTRUCTION_MARKERS
            .iter()
            .any(|m| lowered_denial.contains(m)),
        "a line that forbids the workflow must not be read as prescribing it"
    );

    let lowered_prescription = prescription.to_lowercase();
    assert!(
        INSTRUCTION_MARKERS
            .iter()
            .any(|m| lowered_prescription.contains(m)),
        "the exact line this gate exists to catch must be caught"
    );
    assert!(
        !DENIAL_MARKERS
            .iter()
            .any(|m| lowered_prescription.contains(m)),
        "the offending line must not be excused by the denial override"
    );

    // A withdrawal notice quotes the requirement it withdraws. It carries an
    // instruction marker AND a denial marker, and the denial has to win.
    let withdrawal = "- Até a v1.2.4 esta seção exigia `ci.yml` e `release.yml` como OBRIGATÓRIOS"
        .to_lowercase();
    assert!(
        INSTRUCTION_MARKERS.iter().any(|m| withdrawal.contains(m))
            && DENIAL_MARKERS.iter().any(|m| withdrawal.contains(m)),
        "the overlap case must exercise both lists, or the override is untested"
    );
}
