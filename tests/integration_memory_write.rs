//! Memory write path: remember — creation, duplicates, force-merge, mutually exclusive body sources, graph-stdin validation and body-size limits.
//!
//! Split out of `integration_memory_crud.rs` by GAP-SG-208: that file reached
//! 1018 lines, past the 800-line ceiling this project sets for itself. It was
//! itself carved out of a 2485-line `integration.rs` in v1.2.5, so this is the
//! second pass of the same decomposition. Helpers stay in `tests/common/`.

#[path = "common/mod.rs"]
mod common;

#[allow(unused_imports)]
use assert_cmd::Command;
#[allow(unused_imports)]
use common::{
    cmd, home_isolated_cmd, init_db, isolated_cmd_in, seed_memory_with_entities, sgr_cmd,
};
#[allow(unused_imports)]
use tempfile::TempDir;
// ---------------------------------------------------------------------------
// remember
// ---------------------------------------------------------------------------

#[test]
fn test_remember_creates_memory() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memory-test",
            "--type",
            "user",
            "--description",
            "Descricao de teste",
            "--body",
            "Conteudo do corpo da memoria de teste",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["action"], "created");
    assert_eq!(json["name"], "memory-test");
    assert!(json["memory_id"].as_i64().unwrap() > 0);
}

#[test]
fn test_remember_duplicate_returns_exit_9() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "dup-memoria",
            "--type",
            "user",
            "--description",
            "Primeira versao",
            "--body",
            "Corpo da primeira versao",
        ])
        .assert()
        .success();

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "dup-memoria",
            "--type",
            "user",
            "--description",
            "Segunda versao",
            "--body",
            "Corpo da segunda versao",
        ])
        .assert()
        .failure()
        .code(9);
}

#[test]
fn test_remember_force_merge_updates() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memory-merge",
            "--type",
            "feedback",
            "--description",
            "Descricao original",
            "--body",
            "Corpo original da memoria",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memory-merge",
            "--type",
            "feedback",
            "--description",
            "Descricao atualizada",
            "--body",
            "Corpo atualizado da memoria",
            "--force-merge",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["action"], "updated");
    assert_eq!(json["name"], "memory-merge");
}

#[test]
fn test_remember_rejects_body_and_body_stdin_together() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "entrada-ambigua",
            "--type",
            "project",
            "--description",
            "fontes ambiguas",
            "--body",
            "corpo explicito",
            "--body-stdin",
        ])
        .write_stdin("corpo stdin")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_remember_graph_stdin_invalid_fails_without_saving_memory() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "grafo-invalido",
            "--type",
            "project",
            "--description",
            "json invalido",
            "--graph-stdin",
        ])
        .write_stdin("{not-json")
        .assert()
        .failure()
        .code(1);

    cmd(&tmp)
        .args(["read", "--name", "grafo-invalido"])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn test_remember_graph_stdin_semantic_invalid_fails_without_saving_memory() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // GAP-SG-47 (v1.1.8): `entity_type` and `relation` stopped being rejection
    // classes. `EntityType::map_to_canonical` folds ANY unknown label onto the
    // nearest canonical kind (`agent` -> `person`) and never drops a node, and
    // a non-canonical relation is accepted with a warning — `link
    // --strict-relations` is the only surface that still refuses one. The two
    // payloads that probed those classes were therefore asserting a contract
    // the product no longer has. The question under test is unchanged — "does a
    // semantically invalid graph abort with exit 1 and leave NO memory behind?"
    // — so each retired class was replaced by the class that took its place:
    // the entity-name rule and the relation-FORMAT rule.
    let cases = [
        (
            "nome-de-entidade-invalido",
            r#"{"entities":[{"name":"a","entity_type":"tool"}],"relationships":[]}"#,
        ),
        (
            "formato-de-relacao-invalido",
            r#"{"entities":[{"name":"alpha-node","entity_type":"tool"},{"name":"beta-node","entity_type":"file"}],"relationships":[{"source":"alpha-node","target":"beta-node","relation":"escreve em!","strength":0.5}]}"#,
        ),
        (
            "peso-invalido",
            r#"{"entities":[{"name":"gamma-node","entity_type":"tool"},{"name":"delta-node","entity_type":"file"}],"relationships":[{"source":"gamma-node","target":"delta-node","relation":"uses","strength":2.0}]}"#,
        ),
        (
            "campo-desconhecido",
            r#"{"entities":[{"name":"epsilon-node","entity_type":"tool","extra":"nao"}],"relationships":[]}"#,
        ),
    ];

    for (name, payload) in cases {
        cmd(&tmp)
            .args([
                "remember",
                "--name",
                name,
                "--type",
                "project",
                "--description",
                "grafo invalido",
                "--graph-stdin",
                "--json",
            ])
            .write_stdin(payload)
            .assert()
            .failure()
            .code(1);

        cmd(&tmp)
            .args(["read", "--name", name, "--json"])
            .assert()
            .failure()
            .code(4);
    }
}

// ---------------------------------------------------------------------------
// body-size limits
// ---------------------------------------------------------------------------

#[test]
fn test_remember_accepts_document_above_old_limit_with_chunks() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let body = (0..900)
        .map(|i| format!("termo{i} documento real para chunk seguro"))
        .collect::<Vec<_>>()
        .join(" ");

    let output = cmd(&tmp)
        .args([
            "remember",
            "--name",
            "doc-acima-limite-antigo",
            "--type",
            "reference",
            "--description",
            "documento acima do limite antigo",
            "--body",
            &body,
            "--skip-extraction",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(
        json["chunks_created"].as_u64().unwrap_or_default() > 1,
        "documento deve usar caminho multi-chunk"
    );
}

#[test]
fn test_remember_rejects_body_above_new_operational_limit() {
    let tmp = TempDir::new().unwrap();
    let body_path = tmp.path().join("body-grande.txt");
    std::fs::write(&body_path, "x".repeat(512_001)).unwrap();

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "body-grande",
            "--type",
            "reference",
            "--description",
            "body acima do limite novo",
            "--body-file",
            body_path.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .failure()
        .code(6);
}
