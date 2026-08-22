//! GAP-SG-201: a refusal message that nothing calls is a guard that never runs.
//!
//! `count_only_over_a_page` was written, doc-commented with its gap number,
//! translated into both languages and reviewed — and never called. The defect it
//! described stayed live for two releases while the catalogue read as if it were
//! handled.
//!
//! # Why the compiler could not catch this
//!
//! `dead_code` evaluates reachability WITHIN a crate. A `pub` item in a lib crate
//! is by definition reachable from outside it, so the lint has nothing to say —
//! `cargo clippy --all-targets -- -D warnings` passed clean the whole time, with
//! the guard sitting there disabled. This is not a configuration gap to be closed
//! by adding a lint; it is a consequence of visibility semantics, and no lint
//! setting changes it.
//!
//! Call-site coverage therefore has to be a TEST. "It compiles clean under
//! `-D warnings`" is not evidence that written code is in use.
//!
//! # Scope
//!
//! The agent-surface catalogue only. The defect happened there, the module is
//! small enough that every message is a refusal, and a gate that starts narrow
//! and true beats one that starts wide and needs an allowlist on day one.

use std::path::{Path, PathBuf};

/// The catalogue under contract.
const CATALOGUE: &str = "src/i18n/validation/messages_agent_surface.rs";

/// Every `.rs` file under `root`, recursively.
fn rust_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Names of the `pub fn` items the catalogue declares, in file order.
///
/// Deliberately a line scan rather than a parse: the catalogue is a flat list of
/// free functions with no macros and no nesting, so a parser would be machinery
/// bought to solve a problem this file does not have. The assertion below fails
/// loudly if the shape ever stops being flat, because an empty list means the
/// scan matched nothing and the gate would otherwise pass vacuously.
fn declared_messages(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub fn "))
        .filter_map(|rest| rest.split('(').next())
        .map(str::to_string)
        .collect()
}

/// Whether `path` is somewhere a call site would NOT count as productive.
///
/// A test that calls a message keeps it alive for `dead_code` while leaving the
/// product path exactly as broken as before, which is the precise failure this
/// gate exists to detect. So test files cannot vouch for a message.
fn is_test_only(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.contains("/tests/") || text.ends_with("_tests.rs") || text.ends_with("/tests.rs")
}

/// The gate's own two halves, asserted so it cannot pass by not working.
///
/// A green gate proves nothing unless the detector demonstrably fires. The first
/// half pins the scan — it must find the message whose orphanhood produced
/// GAP-SG-201 — and the second pins the search, on a name no source file carries.
#[test]
fn the_gate_detects_what_it_claims_to_detect() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let catalogue = std::fs::read_to_string(root.join(CATALOGUE)).expect("catalogue");

    let declared = declared_messages(&catalogue);
    assert!(
        declared.iter().any(|n| n == "count_only_over_a_page"),
        "the scan must find the very message that shipped orphaned: {declared:?}"
    );

    let mut sources = Vec::new();
    rust_files(&root.join("src"), &mut sources);
    let bodies: Vec<String> = sources
        .iter()
        .filter(|path| !is_test_only(path))
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect();
    assert!(
        !bodies
            .iter()
            .any(|body| body.contains("uma_mensagem_que_nao_existe_em_lugar_nenhum")),
        "the search must report absence for a name nothing carries, or every \
         message would look called and the gate would be decorative"
    );
}

#[test]
fn every_refusal_message_has_a_productive_call_site() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let catalogue_path = root.join(CATALOGUE);
    let catalogue =
        std::fs::read_to_string(&catalogue_path).expect("the agent-surface catalogue must exist");

    let declared = declared_messages(&catalogue);
    assert!(
        declared.len() >= 5,
        "the scan found {} messages in {CATALOGUE}, which means it stopped matching \
         the file's shape — a gate that matches nothing passes vacuously",
        declared.len()
    );

    let mut sources = Vec::new();
    rust_files(&root.join("src"), &mut sources);
    let bodies: Vec<String> = sources
        .iter()
        .filter(|path| path.as_path() != catalogue_path && !is_test_only(path))
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect();

    let orphans: Vec<&String> = declared
        .iter()
        .filter(|name| !bodies.iter().any(|body| body.contains(name.as_str())))
        .collect();

    assert!(
        orphans.is_empty(),
        "these refusal messages are defined and never called from the product: {orphans:?}\n\
         Each one is a guard that does not run. Wire it to its gate, or delete it — \
         a catalogue entry with no call site reads as a defect already handled.\n\
         `dead_code` cannot see this: they are `pub` in a lib crate, so the lint \
         considers them reachable from outside and stays silent."
    );
}
