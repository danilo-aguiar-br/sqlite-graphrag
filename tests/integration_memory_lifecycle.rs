//! Memory lifecycle: forget, purge, rename, edit, history, restore, and the FTS index integrity check across a forget/purge cycle.
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

#[test]
fn test_forget_soft_delete() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "esquecivel",
            "--type",
            "user",
            "--description",
            "sera deletada",
            "--body",
            "corpo",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args(["forget", "--name", "esquecivel"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["forgotten"], true);

    cmd(&tmp)
        .args(["read", "--name", "esquecivel"])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn test_forget_nonexistent_returns_exit_4() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args(["forget", "--name", "nao-existe"])
        .assert()
        .failure()
        .code(4);
}

// ---------------------------------------------------------------------------
// purge
// ---------------------------------------------------------------------------

#[test]
fn test_purge_removes_soft_deleted_memory() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "purge-target",
            "--type",
            "user",
            "--description",
            "soft delete target",
            "--body",
            "body to purge later",
        ])
        .assert()
        .success();

    cmd(&tmp)
        .args(["forget", "--name", "purge-target"])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args([
            "purge",
            "--name",
            "purge-target",
            "--retention-days",
            "0",
            "--yes",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["purged_count"], 1);
}

#[test]
fn test_purge_yes_flag_is_noop() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "purge-yes-target",
            "--type",
            "user",
            "--description",
            "alvo para teste --yes",
            "--body",
            "corpo yes noop",
        ])
        .assert()
        .success();

    cmd(&tmp)
        .args(["forget", "--name", "purge-yes-target"])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args([
            "purge",
            "--name",
            "purge-yes-target",
            "--retention-days",
            "0",
            "--yes",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["purged_count"], 1);
}

// ---------------------------------------------------------------------------
// rename
// ---------------------------------------------------------------------------

#[test]
fn test_rename_memory_works() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memoria-antiga",
            "--type",
            "user",
            "--description",
            "desc original",
            "--body",
            "corpo original",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args([
            "rename",
            "--name",
            "memoria-antiga",
            "--new-name",
            "memory-renamed",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["name"], "memory-renamed");
    assert!(json["memory_id"].as_i64().unwrap() > 0);
}

#[test]
fn test_rename_nonexistent_returns_exit_4() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args(["rename", "--name", "nao-existe", "--new-name", "novo-nome"])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn test_rename_normalizes_new_name() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memoria-valida",
            "--type",
            "user",
            "--description",
            "desc",
            "--body",
            "corpo",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args([
            "rename",
            "--name",
            "memoria-valida",
            "--new-name",
            "Nome Com Espaco",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["name"], "nome-com-espaco");
    assert_eq!(json["action"], "renamed");
}

// ---------------------------------------------------------------------------
// edit
// ---------------------------------------------------------------------------

#[test]
fn test_edit_memory_works() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memoria-editavel",
            "--type",
            "user",
            "--description",
            "desc original",
            "--body",
            "corpo original",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args([
            "edit",
            "--name",
            "memoria-editavel",
            "--body",
            "corpo atualizado",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["action"], "updated");
}

#[test]
fn test_edit_rejects_body_and_body_stdin_together() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memoria-edit-ambigua",
            "--type",
            "user",
            "--description",
            "desc",
            "--body",
            "corpo original",
        ])
        .assert()
        .success();

    cmd(&tmp)
        .args([
            "edit",
            "--name",
            "memoria-edit-ambigua",
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
fn test_edit_nonexistent_returns_exit_4() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args(["edit", "--name", "nao-existe", "--body", "novo corpo"])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn test_edit_with_conflict_returns_exit_3() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memoria-conflito",
            "--type",
            "user",
            "--description",
            "desc original",
            "--body",
            "corpo original",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let wrong_updated_at = json["version"].as_i64().unwrap() + 999;

    cmd(&tmp)
        .args([
            "edit",
            "--name",
            "memoria-conflito",
            "--body",
            "novo corpo",
            "--expected-updated-at",
            &wrong_updated_at.to_string(),
        ])
        .assert()
        .failure()
        .code(3);
}

// ---------------------------------------------------------------------------
// history
// ---------------------------------------------------------------------------

#[test]
fn test_history_returns_versions() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memoria-historico",
            "--type",
            "user",
            "--description",
            "v1",
            "--body",
            "corpo v1",
        ])
        .assert()
        .success();

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memoria-historico",
            "--type",
            "user",
            "--description",
            "v2",
            "--body",
            "corpo v2",
            "--force-merge",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args(["history", "--name", "memoria-historico"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let versions = json["versions"].as_array().unwrap();
    assert!(versions.len() >= 2);
}

#[test]
fn test_history_nonexistent_returns_exit_4() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args(["history", "--name", "nao-existe"])
        .assert()
        .failure()
        .code(4);
}

// ---------------------------------------------------------------------------
// restore
// ---------------------------------------------------------------------------

#[test]
fn test_restore_memory_works() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memoria-restore",
            "--type",
            "user",
            "--description",
            "v1",
            "--body",
            "corpo v1",
        ])
        .assert()
        .success();

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memoria-restore",
            "--type",
            "user",
            "--description",
            "v2",
            "--body",
            "corpo v2",
            "--force-merge",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args(["restore", "--name", "memoria-restore", "--version", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["restored_from"], 1);
    assert!(json["version"].as_i64().unwrap() >= 3);
}

#[test]
fn test_restore_nonexistent_version_returns_exit_4() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "memoria-sem-versao",
            "--type",
            "user",
            "--description",
            "desc",
            "--body",
            "corpo",
        ])
        .assert()
        .success();

    cmd(&tmp)
        .args(["restore", "--name", "memoria-sem-versao", "--version", "99"])
        .assert()
        .failure()
        .code(4);
}

// ---------------------------------------------------------------------------
// forget+purge regression (FTS5 external-content corruption)
// ---------------------------------------------------------------------------

#[test]
fn test_forget_purge_does_not_corrupt_fts_index() {
    // Regression: forget.rs previously executed `DELETE FROM fts_memories WHERE rowid=?`
    // directly, corrupting the FTS5 external-content index. The corruption only appeared
    // when purge ran a physical DELETE on memories triggering trg_fts_ad.
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    for i in 0..3 {
        let name = format!("fts-reg-{i}");
        cmd(&tmp)
            .args([
                "remember",
                "--name",
                &name,
                "--type",
                "user",
                "--description",
                "regression",
                "--body",
                &format!("corpo fts regression {i}"),
            ])
            .assert()
            .success();

        cmd(&tmp)
            .args(["forget", "--name", &name])
            .assert()
            .success();

        cmd(&tmp)
            .args(["purge", "--name", &name, "--retention-days", "0", "--yes"])
            .assert()
            .success();
    }

    let output = cmd(&tmp)
        .arg("health")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        json["integrity"], "ok",
        "PRAGMA integrity_check DEVE permanecer ok após ciclos forget+purge"
    );
}
