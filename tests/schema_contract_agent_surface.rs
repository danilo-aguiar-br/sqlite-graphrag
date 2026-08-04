//! Strict schema contract: the agent-native output surface — --select, --count-only, and alias suppression on recall and related (GAP-SG-142).
//!
//! Part of the strict JSON-Schema contract suite split by GAP-SG-208. Each
//! test runs the binary, captures stdout, parses it as JSON and validates it
//! against the published `docs/schemas/*.schema.json`. The shared harness lives
//! in `tests/schema_support/`.

#[path = "schema_support/mod.rs"]
mod support;

use serde_json::Value;
use serial_test::serial;
use support::{validar_schema, Env, AGENT_SURFACE_SCHEMA};

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
    env.remember_simples("mem-schema-surface-select");
    let saida = env
        .cmd()
        .args(["list", "--namespace", "global", "--select", "name"])
        .output()
        .expect("list --select failed");
    assert!(
        saida.status.success(),
        "list --select: exit {:?}\nstderr: {}",
        saida.status.code(),
        String::from_utf8_lossy(&saida.stderr)
    );
    let instancia = Env::parse_stdout(&saida, "list --select");

    validar_schema(
        "list --select",
        include_str!("../docs/schemas/list.schema.json"),
        &instancia,
    );

    assert_eq!(
        instancia["agent_surface"]["select"],
        serde_json::json!(["name"])
    );
    assert!(
        instancia.get("memories").is_none(),
        "the unprojected alias must not survive the projection: {instancia}"
    );
    assert_eq!(
        instancia["agent_surface"]["aliases_removed"],
        serde_json::json!(["memories"])
    );
    for item in instancia["items"].as_array().expect("items é array") {
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
    env.remember_simples("mem-schema-surface-count");
    let saida = env
        .cmd()
        .args(["list", "--namespace", "global", "--count-only"])
        .output()
        .expect("list --count-only failed");
    assert!(
        saida.status.success(),
        "list --count-only: exit {:?}\nstderr: {}",
        saida.status.code(),
        String::from_utf8_lossy(&saida.stderr)
    );
    let instancia = Env::parse_stdout(&saida, "list --count-only");

    // The definition is lifted into a standalone document so its internal
    // `#/$defs/AgentSurfaceMeta` reference still resolves.
    let compartilhado: Value = serde_json::from_str(AGENT_SURFACE_SCHEMA)
        .expect("agent-surface.schema.json deve ser JSON válido");
    let mut envelope_de_contagem = compartilhado["$defs"]["CountOnlyEnvelope"].clone();
    envelope_de_contagem["$schema"] =
        serde_json::json!("https://json-schema.org/draft/2020-12/schema");
    envelope_de_contagem["$defs"] = compartilhado["$defs"].clone();
    let schema_str =
        serde_json::to_string(&envelope_de_contagem).expect("serialização do subschema");

    validar_schema("list --count-only", &schema_str, &instancia);

    assert_eq!(instancia["agent_surface"]["count_only"], true);
    assert!(instancia["count"].as_u64().is_some());
    assert!(
        instancia.get("items").is_none() && instancia.get("memories").is_none(),
        "o payload é substituído pela contagem: {instancia}"
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
    env.remember_simples("mem-schema-surface-recall");
    let saida = env
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
        saida.status.success(),
        "recall --max-items: exit {:?}\nstderr: {}",
        saida.status.code(),
        String::from_utf8_lossy(&saida.stderr)
    );
    let instancia = Env::parse_stdout(&saida, "recall --max-items");

    validar_schema(
        "recall --max-items",
        include_str!("../docs/schemas/recall.schema.json"),
        &instancia,
    );

    assert!(instancia.get("direct_matches").is_none(), "{instancia}");
    assert!(instancia.get("graph_matches").is_none(), "{instancia}");
    assert_eq!(
        instancia["agent_surface"]["aliases_removed"],
        serde_json::json!(["direct_matches", "graph_matches"])
    );
    assert!(
        instancia["results"]
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
    env.remember_simples("mem-schema-surface-related");
    let saida = env
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
        saida.status.success(),
        "related --select: exit {:?}\nstderr: {}",
        saida.status.code(),
        String::from_utf8_lossy(&saida.stderr)
    );
    let instancia = Env::parse_stdout(&saida, "related --select");

    validar_schema(
        "related --select",
        include_str!("../docs/schemas/related.schema.json"),
        &instancia,
    );

    assert!(
        instancia.get("related_memories").is_none(),
        "the unprojected clone must not survive: {instancia}"
    );
    assert_eq!(
        instancia["agent_surface"]["aliases_removed"],
        serde_json::json!(["related_memories"]),
        "members absent from this envelope are never reported: {instancia}"
    );
}
