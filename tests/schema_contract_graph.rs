//! Strict schema contract: retrieval and graph — related, link, unlink, graph.
//!
//! Part of the strict JSON-Schema contract suite split by GAP-SG-208. Each
//! test runs the binary, captures stdout, parses it as JSON and validates it
//! against the published `docs/schemas/*.schema.json`. The shared harness lives
//! in `tests/schema_support/`.

#[path = "schema_support/mod.rs"]
mod support;

use serial_test::serial;
use support::{validar_schema, Env};
// ---------------------------------------------------------------------------
// 14 — related
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_14_related() {
    let env = Env::new();
    env.init();
    env.remember_simples("mem-schema-related");
    let saida = env
        .cmd()
        .args(["related", "--name", "mem-schema-related", "--hops", "1"])
        .output()
        .expect("related failed");
    assert!(
        saida.status.success(),
        "related: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "related");
    validar_schema(
        "related",
        include_str!("../docs/schemas/related.schema.json"),
        &instancia,
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
    let saida = env
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
        saida.status.success(),
        "link: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "link");
    validar_schema(
        "link",
        include_str!("../docs/schemas/link.schema.json"),
        &instancia,
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
    // Cria o link primeiro
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
    let saida = env
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
        saida.status.success(),
        "unlink: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "unlink");
    validar_schema(
        "unlink",
        include_str!("../docs/schemas/unlink.schema.json"),
        &instancia,
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
    env.remember_simples("mem-schema-graph");
    let saida = env
        .cmd()
        .args(["graph", "--format", "json", "--namespace", "global"])
        .output()
        .expect("graph failed");
    assert!(
        saida.status.success(),
        "graph: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "graph");
    validar_schema(
        "graph",
        include_str!("../docs/schemas/graph.schema.json"),
        &instancia,
    );
}
