//! Strict schema contract: the agent-native output surface — --select, --count-only, and alias suppression on recall and related (GAP-SG-142).
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

use serde_json::Value;
use serial_test::serial;
use support::{validate_schema, Env, AGENT_SURFACE_SCHEMA};

// ---------------------------------------------------------------------------
// 40 — list under --select (GAP-SG-142 agent-native surface)
// ---------------------------------------------------------------------------

/// A projected envelope must still satisfy the published contract.
///
/// Before GAP-SG-142 the surface emitted an `agent_surface` record into an
/// envelope whose root declared `additionalProperties: false`, so every shaped
/// response failed validation. It also left the `memories` clone behind,
/// unprojected, next to the projected `items`.
#[test]
#[serial]
fn schema_40_list_agent_surface_select() {
    let env = Env::new();
    env.init();
    env.remember_simple("mem-schema-surface-select");
    let output = env
        .cmd()
        .args(["list", "--namespace", "global", "--select", "name"])
        .output()
        .expect("list --select failed");
    assert!(
        output.status.success(),
        "list --select: exit {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let instance = Env::parse_stdout(&output, "list --select");

    validate_schema(
        "list --select",
        include_str!("../docs/schemas/list.schema.json"),
        &instance,
    );

    assert_eq!(
        instance["agent_surface"]["select"],
        serde_json::json!(["name"])
    );
    assert!(
        instance.get("memories").is_none(),
        "the unprojected alias must not survive the projection: {instance}"
    );
    assert_eq!(
        instance["agent_surface"]["aliases_removed"],
        serde_json::json!(["memories"])
    );
    for item in instance["items"].as_array().expect("items é array") {
        let obj = item.as_object().expect("item é objeto");
        assert_eq!(obj.len(), 1, "somente a chave projetada sobrevive: {item}");
        assert!(obj.contains_key("name"));
    }
}

// ---------------------------------------------------------------------------
// 41 — list under --count-only (GAP-SG-142 agent-native surface)
// ---------------------------------------------------------------------------

/// `--count-only` replaces the command envelope, so it answers to the shared
/// count contract rather than to `list.schema.json`.
#[test]
#[serial]
fn schema_41_list_agent_surface_count_only() {
    let env = Env::new();
    env.init();
    env.remember_simple("mem-schema-surface-count");
    let output = env
        .cmd()
        .args(["list", "--namespace", "global", "--count-only"])
        .output()
        .expect("list --count-only failed");
    assert!(
        output.status.success(),
        "list --count-only: exit {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let instance = Env::parse_stdout(&output, "list --count-only");

    // The definition is lifted into a standalone document so its internal
    // `#/$defs/AgentSurfaceMeta` reference still resolves.
    let shared_schema: Value = serde_json::from_str(AGENT_SURFACE_SCHEMA)
        .expect("agent-surface.schema.json deve ser JSON válido");
    let mut count_envelope = shared_schema["$defs"]["CountOnlyEnvelope"].clone();
    count_envelope["$schema"] = serde_json::json!("https://json-schema.org/draft/2020-12/schema");
    count_envelope["$defs"] = shared_schema["$defs"].clone();
    let schema_str = serde_json::to_string(&count_envelope).expect("serialização do subschema");

    validate_schema("list --count-only", &schema_str, &instance);

    assert_eq!(instance["agent_surface"]["count_only"], true);
    assert!(instance["count"].as_u64().is_some());
    assert!(
        instance.get("items").is_none() && instance.get("memories").is_none(),
        "o payload é substituído pela contagem: {instance}"
    );
}

// ---------------------------------------------------------------------------
// 42 — recall with its aliases suppressed (GAP-SG-142 agent-native surface)
// ---------------------------------------------------------------------------

/// `results` is the concatenation of `direct_matches` and `graph_matches`.
/// Once the surface reshapes `results`, keeping the two halves would hand the
/// caller the unshaped rows back under another name — and blow the byte ceiling
/// for a payload that is redundant by construction.
#[test]
#[serial]
fn schema_42_recall_agent_surface_alias_suppression() {
    let env = Env::new();
    env.init();
    env.remember_simple("mem-schema-surface-recall");
    let output = env
        .cmd()
        .args([
            "recall",
            "schema surface recall",
            "--k",
            "3",
            "--max-items",
            "1",
        ])
        .output()
        .expect("recall --max-items failed");
    assert!(
        output.status.success(),
        "recall --max-items: exit {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let instance = Env::parse_stdout(&output, "recall --max-items");

    validate_schema(
        "recall --max-items",
        include_str!("../docs/schemas/recall.schema.json"),
        &instance,
    );

    assert!(instance.get("direct_matches").is_none(), "{instance}");
    assert!(instance.get("graph_matches").is_none(), "{instance}");
    assert_eq!(
        instance["agent_surface"]["aliases_removed"],
        serde_json::json!(["direct_matches", "graph_matches"])
    );
    assert!(
        instance["results"]
            .as_array()
            .expect("results é array")
            .len()
            <= 1
    );
}

// ---------------------------------------------------------------------------
// 43 — related with its alias suppressed (GAP-SG-142 agent-native surface)
// ---------------------------------------------------------------------------

/// `related_memories` is a clone of `results`. It shares the `results`
/// canonical key with `recall`, which contributes two other derived members;
/// the ones this envelope never carried must be a silent no-op.
#[test]
#[serial]
fn schema_43_related_agent_surface_alias_suppression() {
    let env = Env::new();
    env.init();
    env.remember_simple("mem-schema-surface-related");
    let output = env
        .cmd()
        .args([
            "related",
            "--name",
            "mem-schema-surface-related",
            "--hops",
            "1",
            "--select",
            "name",
        ])
        .output()
        .expect("related --select failed");
    assert!(
        output.status.success(),
        "related --select: exit {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let instance = Env::parse_stdout(&output, "related --select");

    validate_schema(
        "related --select",
        include_str!("../docs/schemas/related.schema.json"),
        &instance,
    );

    assert!(
        instance.get("related_memories").is_none(),
        "the unprojected clone must not survive: {instance}"
    );
    assert_eq!(
        instance["agent_surface"]["aliases_removed"],
        serde_json::json!(["related_memories"]),
        "members absent from this envelope are never reported: {instance}"
    );
}
