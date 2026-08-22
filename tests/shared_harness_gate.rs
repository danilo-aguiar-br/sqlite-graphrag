//! A shared test harness must not gate ITSELF out of the module graph.
//!
//! `tests/schema_support/mod.rs` carried `#![cfg(feature = "slow-tests")]`.
//! Without the feature the module did not become empty — it disappeared, and
//! the five files declaring `#[path = "schema_support/mod.rs"] mod support;`
//! failed with E0432 `unresolved import support`. That is a COMPILE failure, so
//! it took down the entire default `cargo test`, not merely the slow suites.
//!
//! Two things made it expensive. `include` ships `tests/**/*.rs`, so every
//! crates.io consumer inherited a test tree that does not build. And rustc
//! answers E0432 with "use of unresolved module or unlinked crate `support`"
//! plus a suggestion to `cargo add support`, which points at a nonexistent
//! crates.io package instead of at the attribute one directory over.
//!
//! The rule this gate enforces is the convention the repository already
//! follows everywhere else: the feature gate lives in the CONSUMER file, next
//! to the tests it actually governs, never in the shared harness. A one-time
//! fix would leave the asymmetry that produced it, so the rule is structural.

use std::path::{Path, PathBuf};

/// Inner attributes that remove a whole file from the module graph.
///
/// Only `cfg` is listed. `#![allow(...)]` and `#![deny(...)]` are lint
/// controls: they change diagnostics, never whether the module exists, and
/// `schema_support/mod.rs` legitimately carries `#![allow(dead_code)]` because
/// each consumer uses a different subset of the harness.
const MODULE_ERASING_ATTR: &str = "#![cfg(";

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// Every `mod.rs` under `tests/`, i.e. every shared harness.
fn shared_harnesses() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(tests_dir()) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let candidate = path.join("mod.rs");
            if candidate.is_file() {
                out.push(candidate);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_shared_test_harness_never_gates_itself_out_of_the_module_graph() {
    let harnesses = shared_harnesses();
    assert!(
        !harnesses.is_empty(),
        "the scan found no shared harness at all, which means the discovery \
         pattern is wrong rather than that the tree is clean"
    );

    let mut offences: Vec<String> = Vec::new();
    for path in &harnesses {
        let Ok(body) = std::fs::read_to_string(path) else {
            continue;
        };
        for (idx, line) in body.lines().enumerate() {
            if line.trim_start().starts_with(MODULE_ERASING_ATTR) {
                let rel = path
                    .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")))
                    .unwrap_or(path)
                    .display();
                offences.push(format!("{rel}:{}: {}", idx + 1, line.trim()));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "a shared harness gates itself out of the module graph. Move the \
         attribute to each file that declares `mod support;`, where the other \
         gated suites keep it — leaving it here breaks compilation of every \
         consumer instead of skipping them.\n{}",
        offences.join("\n")
    );
}

/// The five schema-contract suites must stay on the DEFAULT `cargo test` path.
///
/// # Why this assertion is the inverse of the one it replaces
///
/// It used to demand the opposite: that each of the five carry
/// `#![cfg(feature = "slow-tests")]`, so a default invocation would skip them.
/// The product then decided the other way, and wrote the reason at the top of
/// each file — this suite is the only thing that compares the binary's REAL
/// stdout against the published contract, so gating it behind a feature that
/// the default invocation never passes left the schemas free to drift with
/// nothing watching. A gate the default run never executes is not a gate.
///
/// The decision was applied to the five files and never to this test, so the
/// test went red against a change that was correct — the same shape of defect
/// as the migration count that stayed at 16 after V017 shipped. Inverting it
/// keeps the watch rather than deleting it: reintroducing the feature gate now
/// fails here, which forces the argument to be made in a diff instead of being
/// made by accident.
///
/// The companion assertion above is untouched and still matters for a different
/// reason: the attribute must never move into the shared `mod.rs`, because a
/// `mod.rs` that cfg-es itself out VANISHES from the module graph and breaks
/// compilation of every consumer instead of skipping them.
#[test]
fn the_schema_contract_consumers_stay_on_the_default_test_path() {
    let expected = [
        "schema_contract_crud.rs",
        "schema_contract_entities.rs",
        "schema_contract_graph.rs",
        "schema_contract_maintenance.rs",
        "schema_contract_agent_surface.rs",
    ];

    let mut gated: Vec<&str> = Vec::new();
    let mut missing: Vec<&str> = Vec::new();
    for name in expected {
        let path = tests_dir().join(name);
        let Ok(body) = std::fs::read_to_string(&path) else {
            missing.push(name);
            continue;
        };
        if body.contains("#![cfg(feature = \"slow-tests\")]") {
            gated.push(name);
        }
    }

    // Premise check first: a renamed or deleted file would empty the loop and
    // let this test pass over nothing, which is how a gate goes blind quietly.
    assert!(
        missing.is_empty(),
        "these schema-contract suites are named here but not on disk, so this \
         gate would be watching nothing: {missing:?}"
    );
    assert!(
        gated.is_empty(),
        "these schema-contract suites are the only check of the binary's real \
         stdout against the published schemas, and putting them behind \
         `slow-tests` removes them from the default `cargo test`: {gated:?}"
    );
}
