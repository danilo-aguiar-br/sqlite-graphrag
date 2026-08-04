//! Entity graph — link, unlink and `related` traversal.
//!
//! Split out of `tests/integration_graph.rs` by GAP-SG-210: that file held 922
//! lines, past the 800-line ceiling this project sets for itself. It was itself
//! carved out of the 2 485-line `tests/integration.rs` in v1.2.5, so this is
//! the second cut of the same tree; the shared helpers stayed in
//! `tests/common/` and were never copied.

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
// link
// ---------------------------------------------------------------------------

#[test]
fn test_link_creates_explicit_relationship() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    seed_memory_with_entities(
        &tmp,
        "link-seed",
        r#"[
            {"name":"projeto-alpha","entity_type":"project","description":null},
            {"name":"tokio","entity_type":"tool","description":null}
        ]"#,
    );

    let output = cmd(&tmp)
        .args([
            "link",
            "--from",
            "projeto-alpha",
            "--to",
            "tokio",
            "--relation",
            "uses",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["action"], "created");
    assert_eq!(json["from"], "projeto-alpha");
    assert_eq!(json["to"], "tokio");
    assert_eq!(json["relation"], "uses");
    assert!((json["weight"].as_f64().unwrap() - 0.5).abs() < 1e-9);
}

#[test]
fn test_link_idempotent_returns_already_exists() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    seed_memory_with_entities(
        &tmp,
        "link-idem",
        r#"[
            {"name":"servico-x","entity_type":"project","description":null},
            {"name":"banco-y","entity_type":"tool","description":null}
        ]"#,
    );

    cmd(&tmp)
        .args([
            "link",
            "--from",
            "servico-x",
            "--to",
            "banco-y",
            "--relation",
            "depends-on",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args([
            "link",
            "--from",
            "servico-x",
            "--to",
            "banco-y",
            "--relation",
            "depends-on",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["action"], "already_exists");
}

#[test]
fn test_link_nonexistent_entity_returns_exit_4() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "link",
            "--from",
            "nao-existe-a",
            "--to",
            "nao-existe-b",
            "--relation",
            "uses",
        ])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn test_link_reflexive_returns_exit_1() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "link",
            "--from",
            "mesmo-nome",
            "--to",
            "mesmo-nome",
            "--relation",
            "uses",
        ])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn test_link_invalid_weight_returns_exit_1() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "link",
            "--from",
            "a",
            "--to",
            "b",
            "--relation",
            "uses",
            "--weight",
            "1.5",
        ])
        .assert()
        .failure()
        .code(1);
}

// ---------------------------------------------------------------------------
// unlink
// ---------------------------------------------------------------------------

#[test]
fn test_unlink_removes_existing_relationship() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    seed_memory_with_entities(
        &tmp,
        "unlink-seed",
        r#"[
            {"name":"ent-u-a","entity_type":"project","description":null},
            {"name":"ent-u-b","entity_type":"tool","description":null}
        ]"#,
    );

    cmd(&tmp)
        .args([
            "link",
            "--from",
            "ent-u-a",
            "--to",
            "ent-u-b",
            "--relation",
            "uses",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args([
            "unlink",
            "--from",
            "ent-u-a",
            "--to",
            "ent-u-b",
            "--relation",
            "uses",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["action"], "deleted");
    assert_eq!(json["from_name"], "ent-u-a");
    assert_eq!(json["to_name"], "ent-u-b");
}

#[test]
fn test_unlink_nonexistent_relation_returns_exit_4() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    seed_memory_with_entities(
        &tmp,
        "unlink-inexistente-seed",
        r#"[
            {"name":"ent-ui-a","entity_type":"project","description":null},
            {"name":"ent-ui-b","entity_type":"tool","description":null}
        ]"#,
    );

    cmd(&tmp)
        .args([
            "unlink",
            "--from",
            "ent-ui-a",
            "--to",
            "ent-ui-b",
            "--relation",
            "uses",
        ])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn test_unlink_missing_entity_returns_exit_4() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "unlink",
            "--from",
            "nenhuma-a",
            "--to",
            "nenhuma-b",
            "--relation",
            "uses",
        ])
        .assert()
        .failure()
        .code(4);
}

// ---------------------------------------------------------------------------
// regression: shared entity across memories must not duplicate entity_embeddings rows
// ---------------------------------------------------------------------------
// v1.0.74 hit this bug because vec0 does not support INSERT OR REPLACE. v1.0.76
// replaced vec_entities with a regular BLOB-backed entity_embeddings table whose
// PK is the entity_id. The fix moves the deduplication to the caller (the
// storage layer upserts on entity_id, so two memories sharing one entity
// produce ONE entity_embeddings row). This test pins that invariant.

#[test]
fn test_remember_does_not_duplicate_vec_entities_for_shared_entity() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // First memory with entity "entidade-comum".
    seed_memory_with_entities(
        &tmp,
        "memoria-primeiro",
        r#"[{"name":"entidade-comum","entity_type":"concept","description":null}]"#,
    );

    // Second memory reuses the SAME entity — v1.0.76 storage layer dedups by
    // entity_id, so the upsert must succeed without UNIQUE constraint error.
    seed_memory_with_entities(
        &tmp,
        "memoria-segundo",
        r#"[{"name":"entidade-comum","entity_type":"concept","description":null}]"#,
    );

    // Third memory also reuses it, ensuring robustness with multiple duplicates.
    seed_memory_with_entities(
        &tmp,
        "memoria-terceiro",
        r#"[{"name":"entidade-comum","entity_type":"concept","description":null}]"#,
    );

    // Open the database directly to verify there is exactly ONE entity_embeddings
    // row for the shared entity, not three.
    let conn = rusqlite::Connection::open(tmp.path().join("test.sqlite")).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entity_embeddings e
             JOIN entities en ON en.id = e.entity_id
             WHERE en.name = 'entidade-comum'",
            [],
            |row| row.get(0),
        )
        .expect("entity_embeddings query must succeed");
    assert_eq!(
        count, 1,
        "shared entity across 3 memories must produce exactly 1 entity_embeddings row, found {count}"
    );
}

// ---------------------------------------------------------------------------
// related
// ---------------------------------------------------------------------------

#[test]
fn test_related_finds_memories_via_graph() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // Memory 1 and 2 share the entity "projeto-compartilhado".
    seed_memory_with_entities(
        &tmp,
        "memoria-um",
        r#"[{"name":"projeto-compartilhado","entity_type":"project","description":null}]"#,
    );
    seed_memory_with_entities(
        &tmp,
        "memoria-dois",
        r#"[{"name":"projeto-compartilhado","entity_type":"project","description":null}]"#,
    );

    // Relacionamento artificial para garantir hop>=1.
    seed_memory_with_entities(
        &tmp,
        "memoria-link",
        r#"[
            {"name":"projeto-compartilhado","entity_type":"project","description":null},
            {"name":"ferramenta-x","entity_type":"tool","description":null}
        ]"#,
    );
    cmd(&tmp)
        .args([
            "link",
            "--from",
            "projeto-compartilhado",
            "--to",
            "ferramenta-x",
            "--relation",
            "uses",
            "--weight",
            "0.9",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args(["related", "--name", "memoria-um"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let arr = json["results"]
        .as_array()
        .expect("related must return results array");
    // should contain at least one of the other two memories via hop
    let names: Vec<&str> = arr.iter().filter_map(|v| v["name"].as_str()).collect();
    assert!(
        names.contains(&"memoria-link"),
        "esperava memoria-link em {names:?}"
    );
}

#[test]
fn test_related_nonexistent_memory_returns_exit_4() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args(["related", "--name", "nao-existe-mem"])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn test_related_returns_empty_when_memory_has_no_entities() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "sem-entidades",
            "--type",
            "user",
            "--description",
            "memoria solitaria",
            "--body",
            "corpo",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args(["related", "--name", "sem-entidades"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["results"].as_array().unwrap().len(), 0);
}
