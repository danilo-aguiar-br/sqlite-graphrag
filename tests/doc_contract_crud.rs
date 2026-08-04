#![cfg(feature = "slow-tests")]

//! Contract: memory lifecycle — init, remember, list, read, forget, purge, rename, edit, history, restore.
//!
//! Part of the JSON-contract suite split by GAP-SG-208: the single file held
//! 1393 lines and 41 tests, past the 800-line ceiling this project sets for
//! itself. The shared harness lives in `tests/contract_support/`.
//!
//! Ground truth: `docs/schemas/*.schema.json`. Each test checks the expected
//! exit code, valid JSON, and the presence of the required keys.

#[path = "contract_support/mod.rs"]
mod support;

use serde_json::Value;
use serial_test::serial;
use support::{assert_array_items_have_keys, assert_has_keys, Env};
// ---------------------------------------------------------------------------
// 01 — init
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_01_init() {
    let env = Env::new();
    let out = env.cmd().arg("init").output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "init",
        &json,
        &[
            "db_path",
            "schema_version",
            "model",
            "dim",
            "namespace",
            "status",
        ],
    );
}

// ---------------------------------------------------------------------------
// 02 — remember
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_02_remember() {
    let env = Env::new();
    env.init();
    let json = env.remember("mem-contrato-remember", "corpo do teste de contrato");
    assert_has_keys(
        "remember",
        &json,
        &[
            "memory_id",
            "name",
            "namespace",
            "action",
            "operation",
            "version",
            "entities_persisted",
            "relationships_persisted",
            "chunks_created",
            "warnings",
            "created_at",
            "created_at_iso",
            "elapsed_ms",
        ],
    );
    assert!(json["memory_id"].is_number(), "memory_id deve ser número");
    assert!(
        json["elapsed_ms"].as_u64().unwrap_or(0) < 60_000,
        "elapsed_ms razoável"
    );
}

// ---------------------------------------------------------------------------
// 05 — list
// O contrato publico atual exige objeto com {elapsed_ms, items:[...]}.
// Aceitar array root aqui enfraquece a deteccao de regressao documental.
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_05_list() {
    let env = Env::new();
    env.init();
    env.remember("mem-list-01", "conteúdo para listar");

    let out = env.cmd().arg("list").output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);

    let items = json
        .get("items")
        .unwrap_or_else(|| panic!("list: expected object with {{items:[...]}}, got: {json}"));

    assert!(items.is_array(), "list: 'items' nao e array: {items}");
    let arr = items.as_array().unwrap();
    if !arr.is_empty() {
        assert_array_items_have_keys(
            "list",
            items,
            &[
                "id",
                "memory_id",
                "name",
                "namespace",
                "type",
                "description",
                "snippet",
                "updated_at",
                "updated_at_iso",
            ],
        );
    }
}

// ---------------------------------------------------------------------------
// 06 — read
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_06_read() {
    let env = Env::new();
    env.init();
    env.remember("mem-read-contrato", "corpo para leitura de contrato");

    let out = env
        .cmd()
        .args(["read", "--name", "mem-read-contrato"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "read",
        &json,
        &[
            "id",
            "memory_id",
            "namespace",
            "name",
            "type",
            "memory_type",
            "description",
            "body",
            "body_hash",
            "source",
            "metadata",
            "version",
            "created_at",
            "created_at_iso",
            "updated_at",
            "updated_at_iso",
        ],
    );
}

// ---------------------------------------------------------------------------
// 07 — forget
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_07_forget() {
    let env = Env::new();
    env.init();
    env.remember("mem-forget-contrato", "corpo para soft-delete");

    let out = env
        .cmd()
        .args(["forget", "--name", "mem-forget-contrato"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys("forget", &json, &["forgotten", "name", "namespace"]);
    assert_eq!(json["forgotten"], true);
}

// ---------------------------------------------------------------------------
// 08 — purge
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_08_purge() {
    let env = Env::new();
    env.init();
    env.remember("mem-purge-contrato", "corpo para purge");
    env.cmd()
        .args(["forget", "--name", "mem-purge-contrato"])
        .assert()
        .success();

    let out = env.cmd().args(["purge", "--yes"]).output().unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "purge",
        &json,
        &["purged_count", "bytes_freed", "dry_run", "namespace"],
    );
    assert!(json["purged_count"].is_number());
}

// ---------------------------------------------------------------------------
// 09 — rename
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_09_rename() {
    let env = Env::new();
    env.init();
    env.remember("mem-rename-src", "corpo rename");

    let out = env
        .cmd()
        .args([
            "rename",
            "--name",
            "mem-rename-src",
            "--new-name",
            "mem-rename-dst",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys("rename", &json, &["memory_id", "name", "version"]);
    assert_eq!(json["name"], "mem-rename-dst");
}

// ---------------------------------------------------------------------------
// 10 — edit
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_10_edit() {
    let env = Env::new();
    env.init();
    env.remember("mem-edit-contrato", "corpo original");

    let out = env
        .cmd()
        .args([
            "edit",
            "--name",
            "mem-edit-contrato",
            "--body",
            "corpo editado contrato",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys("edit", &json, &["memory_id", "name", "action", "version"]);
    assert_eq!(json["action"], "updated");
}

// ---------------------------------------------------------------------------
// 11 — history
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_11_history() {
    let env = Env::new();
    env.init();
    env.remember("mem-history-contrato", "corpo versão 1");
    env.cmd()
        .args([
            "edit",
            "--name",
            "mem-history-contrato",
            "--body",
            "corpo versão 2",
        ])
        .assert()
        .success();

    let out = env
        .cmd()
        .args(["history", "--name", "mem-history-contrato"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys("history", &json, &["name", "namespace", "versions"]);
    assert!(json["versions"].is_array());
    let versions = json["versions"].as_array().unwrap();
    assert!(!versions.is_empty(), "deve ter pelo menos 1 versão");
    // Validate keys of each version
    for v in versions {
        let obj = v.as_object().unwrap();
        for key in &[
            "version",
            "name",
            "type",
            "description",
            "body",
            "metadata",
            "action",
            "change_reason",
            "changed_by",
            "created_at",
            "created_at_iso",
        ] {
            assert!(obj.contains_key(*key), "versão sem key '{key}'");
        }
        // Bug M-A6: action must be a non-null string for the documented contract.
        let action = obj.get("action").unwrap();
        assert!(
            action.is_string(),
            "action must be a string, got {action:?}"
        );
        assert!(
            !action.as_str().unwrap().is_empty(),
            "action must not be empty"
        );
    }
}

// ---------------------------------------------------------------------------
// 12 — restore
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_12_restore() {
    let env = Env::new();
    env.init();
    env.remember("mem-restore-contrato", "corpo versão 1");
    env.cmd()
        .args([
            "edit",
            "--name",
            "mem-restore-contrato",
            "--body",
            "corpo versão 2",
        ])
        .assert()
        .success();

    // Get version 1 via history
    let h_out = env
        .cmd()
        .args(["history", "--name", "mem-restore-contrato"])
        .output()
        .unwrap();
    let h_json: Value = serde_json::from_slice(&h_out.stdout).unwrap();
    let ver = h_json["versions"]
        .as_array()
        .and_then(|v| v.iter().find(|e| e["version"] == 1))
        .and_then(|v| v["version"].as_i64())
        .unwrap_or(1);

    let out = env
        .cmd()
        .args([
            "restore",
            "--name",
            "mem-restore-contrato",
            "--version",
            &ver.to_string(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "restore",
        &json,
        &["memory_id", "name", "version", "restored_from"],
    );
}
