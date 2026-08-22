//! Strict schema contract: retrieval and graph — related, link, unlink, graph.
//!
//! Part of the strict JSON-Schema contract suite split by GAP-SG-208. Each
//! test runs the binary, captures stdout, parses it as JSON and validates it
//! against the published `docs/schemas/*.schema.json`. The shared harness lives
//! in `tests/schema_support/`.
//!
//! NOT gated behind `slow-tests`, unlike the 29 other heavy test files, because
//! this suite is the only thing that compares the binary's REAL stdout against
//! the published contract. GAP-SG-271 measured what the gate cost while it was
//! on: five files sat behind the feature, `cargo test` never compiled them, and
//! the published schemas drifted with nothing to notice. A gate the default
//! invocation never runs is not a gate — it is a gate-shaped reassurance.
//!
//! The attribute must never move back into `tests/schema_support/mod.rs`: a
//! shared `mod.rs` that cfg-es itself out does not become empty, it VANISHES
//! from the module graph, so every `use support::…` fails to resolve and the
//! whole test build breaks.

#[path = "schema_support/mod.rs"]
mod support;

use serial_test::serial;
use support::{validate_schema, Env};
// ---------------------------------------------------------------------------
// 14 — related
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_14_related() {
    let env = Env::new();
    env.init();
    env.remember_simple("mem-schema-related");
    let output = env
        .cmd()
        .args(["related", "--name", "mem-schema-related", "--hops", "1"])
        .output()
        .expect("related failed");
    assert!(
        output.status.success(),
        "related: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "related");
    validate_schema(
        "related",
        include_str!("../docs/schemas/related.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 15 — link
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_15_link() {
    let env = Env::new();
    env.init();
    let (ent_a, ent_b) = env.remember_with_entities("mem-schema-link");
    let output = env
        .cmd()
        .args([
            "link",
            "--from",
            &ent_a,
            "--to",
            &ent_b,
            "--relation",
            "depends-on",
            "--namespace",
            "global",
        ])
        .output()
        .expect("link failed");
    assert!(
        output.status.success(),
        "link: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "link");
    validate_schema(
        "link",
        include_str!("../docs/schemas/link.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 16 — unlink
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_16_unlink() {
    let env = Env::new();
    env.init();
    let (ent_a, ent_b) = env.remember_with_entities("mem-schema-unlink");
    // Create the link first
    env.cmd()
        .args([
            "link",
            "--from",
            &ent_a,
            "--to",
            &ent_b,
            "--relation",
            "uses",
            "--namespace",
            "global",
        ])
        .assert()
        .success();
    let output = env
        .cmd()
        .args([
            "unlink",
            "--from",
            &ent_a,
            "--to",
            &ent_b,
            "--relation",
            "uses",
            "--namespace",
            "global",
        ])
        .output()
        .expect("unlink failed");
    assert!(
        output.status.success(),
        "unlink: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "unlink");
    validate_schema(
        "unlink",
        include_str!("../docs/schemas/unlink.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 17 — graph
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_17_graph() {
    let env = Env::new();
    env.init();
    env.remember_simple("mem-schema-graph");
    let output = env
        .cmd()
        .args(["graph", "--format", "json", "--namespace", "global"])
        .output()
        .expect("graph failed");
    assert!(
        output.status.success(),
        "graph: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "graph");
    validate_schema(
        "graph",
        include_str!("../docs/schemas/graph.schema.json"),
        &instance,
    );
}
