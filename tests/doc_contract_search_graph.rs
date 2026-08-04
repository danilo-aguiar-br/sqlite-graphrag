#![cfg(feature = "slow-tests")]

//! Contract: retrieval and graph traversal — recall, hybrid-search, link, unlink, related, graph.
//!
//! Part of the JSON-contract suite split by GAP-SG-208: the single file held
//! 1393 lines and 41 tests, past the 800-line ceiling this project sets for
//! itself. The shared harness lives in `tests/contract_support/`.
//!
//! Ground truth: `docs/schemas/*.schema.json`. Each test checks the expected
//! exit code, valid JSON, and the presence of the required keys.

#[path = "contract_support/mod.rs"]
mod support;

use serial_test::serial;
use support::{assert_array_items_have_keys, assert_has_keys, Env};
// ---------------------------------------------------------------------------
// 13 — recall
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_13_recall() {
    let env = Env::new();
    env.init();
    env.remember(
        "mem-recall-contrato",
        "texto de busca semântica de contrato",
    );

    let out = env.cmd().args(["recall", "contrato"]).output().unwrap();
    // exit 0 (found) or 4 (not found) are both valid
    let code = out.status.code().unwrap_or(1);
    assert!(
        code == 0 || code == 4,
        "recall exit code inesperado: {code}"
    );

    if code == 0 {
        let json = Env::parse_stdout(&out);
        assert_has_keys(
            "recall",
            &json,
            &[
                "query",
                "k",
                "direct_matches",
                "graph_matches",
                "results",
                "elapsed_ms",
            ],
        );
        assert!(json["results"].is_array());
    }
}

// ---------------------------------------------------------------------------
// 14 — hybrid-search
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_14_hybrid_search() {
    let env = Env::new();
    env.init();
    env.remember("mem-hybrid-contrato", "texto para hybrid search contrato");

    let out = env
        .cmd()
        .args(["hybrid-search", "contrato"])
        .output()
        .unwrap();
    let code = out.status.code().unwrap_or(1);
    assert!(
        code == 0 || code == 4,
        "hybrid-search exit code inesperado: {code}"
    );

    if code == 0 {
        let json = Env::parse_stdout(&out);
        assert_has_keys(
            "hybrid-search",
            &json,
            &[
                "query",
                "k",
                "rrf_k",
                "weights",
                "results",
                "graph_matches",
                "elapsed_ms",
            ],
        );
        assert!(json["results"].is_array());
    }
}

// ---------------------------------------------------------------------------
// 15 — link
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_15_link() {
    let env = Env::new();
    env.init();
    let (ent_a, ent_b) = env.remember_with_entities("mem-link-contrato", "corpo link entidades");

    let out = env
        .cmd()
        .args([
            "link",
            "--from",
            &ent_a,
            "--to",
            &ent_b,
            "--relation",
            "related",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "link failed: {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "link",
        &json,
        &["action", "from", "to", "relation", "weight", "namespace"],
    );
    assert_eq!(json["action"], "created");
}

// ---------------------------------------------------------------------------
// 16 — unlink
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_16_unlink() {
    let env = Env::new();
    env.init();
    let (ent_a, ent_b) =
        env.remember_with_entities("mem-unlink-contrato", "corpo unlink entidades");
    // Create relation first
    env.cmd()
        .args([
            "link",
            "--from",
            &ent_a,
            "--to",
            &ent_b,
            "--relation",
            "related",
        ])
        .assert()
        .success();

    let out = env
        .cmd()
        .args([
            "unlink",
            "--from",
            &ent_a,
            "--to",
            &ent_b,
            "--relation",
            "related",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "unlink failed: {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "unlink",
        &json,
        &[
            "action",
            "relationships_removed",
            "from_name",
            "to_name",
            "relation",
            "namespace",
            "elapsed_ms",
        ],
    );
    assert_eq!(json["action"], "deleted");
}

// ---------------------------------------------------------------------------
// 17 — related
// O contrato publico atual exige objeto com {elapsed_ms, results:[...]}.
// Aceitar array root aqui enfraquece a deteccao de regressao documental.
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_17_related() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) =
        env.remember_with_entities("mem-related-a", "corpo entidade A para grafo");
    let (ent_c, _ent_d) =
        env.remember_with_entities("mem-related-b", "corpo entidade B para grafo");
    // Liga as entidades para garantir que related retorna algo
    env.cmd()
        .args([
            "link",
            "--from",
            &ent_a,
            "--to",
            &ent_c,
            "--relation",
            "related",
        ])
        .assert()
        .success();

    let out = env
        .cmd()
        .args(["related", "--name", "mem-related-a"])
        .output()
        .unwrap();
    let code = out.status.code().unwrap_or(1);
    assert!(
        code == 0 || code == 4,
        "related exit code inesperado: {code}"
    );

    if code == 0 {
        let json = Env::parse_stdout(&out);
        let results = json.get("results").unwrap_or_else(|| {
            panic!("related: expected object with {{results:[...]}}, got: {json}")
        });
        assert!(
            results.is_array(),
            "related: 'results' nao e array: {results}"
        );
        let arr = results.as_array().unwrap();
        if !arr.is_empty() {
            assert_array_items_have_keys(
                "related",
                results,
                &[
                    "memory_id",
                    "name",
                    "namespace",
                    "type",
                    "description",
                    "hop_distance",
                    "source_entity",
                    "target_entity",
                    "relation",
                    "weight",
                ],
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 18 — graph
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_18_graph() {
    let env = Env::new();
    env.init();

    let out = env
        .cmd()
        .args(["graph", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys("graph", &json, &["nodes", "edges"]);
    assert!(json["nodes"].is_array());
    assert!(json["edges"].is_array());
}
