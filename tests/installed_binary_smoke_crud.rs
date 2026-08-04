#![cfg(feature = "slow-tests")]

//! Suite 10 — smoke tests against the INSTALLED binary: lifecycle of a memory (#01–#12)
//!
//! Part of the smoke suite split by GAP-SG-210: the single file held 981 lines
//! and 26 tests, past the 800-line ceiling this project sets for itself. The
//! shared harness lives in `tests/smoke_support/`, which also documents why
//! this suite targets `~/.cargo/bin/sqlite-graphrag` instead of the build
//! output, and how it skips when nothing is installed.

#[path = "smoke_support/mod.rs"]
mod support;

use support::{assert_json_or_not_found, assert_json_stdout, Env};

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #01: init
// ---------------------------------------------------------------------------

#[test]
fn smoke_01_init() {
    let env = Env::new();
    let out = env.cmd().arg("init").output().expect("init failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["status"], "ok", "init deve retornar status=ok: {json}");
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #02: health
// ---------------------------------------------------------------------------

#[test]
fn smoke_02_health() {
    let env = Env::new();
    env.init();
    let out = env.cmd().arg("health").output().expect("health failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["status"], "ok",
        "health deve retornar status=ok: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #03: remember
// ---------------------------------------------------------------------------

#[test]
fn smoke_03_remember() {
    let env = Env::new();
    env.init();
    let out = env
        .cmd()
        .args([
            "remember",
            "--name",
            "smoke-memoria-01",
            "--type",
            "user",
            "--description",
            "Memória de smoke test",
            "--body",
            "Conteúdo da memória de smoke test para validar o subcomando remember.",
        ])
        .output()
        .expect("remember failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    // v2.0.4: remember returns action "created", not a status field
    assert_eq!(
        json["action"], "created",
        "remember deve retornar action=created: {json}"
    );
    assert!(
        json["memory_id"].as_i64().is_some(),
        "memory_id deve ser inteiro: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #04: recall
// ---------------------------------------------------------------------------

#[test]
fn smoke_04_recall() {
    let env = Env::new();
    env.init();
    env.remember("smoke-recall-01", "memória para busca semântica de recall");
    let out = env
        .cmd()
        .args(["recall", "busca semântica", "-k", "5"])
        .output()
        .expect("recall failed");
    assert_json_or_not_found(&out);
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #05: read
// ---------------------------------------------------------------------------

#[test]
fn smoke_05_read() {
    let env = Env::new();
    env.init();
    env.remember("smoke-read-01", "conteúdo para read");
    let out = env
        .cmd()
        .args(["read", "--name", "smoke-read-01"])
        .output()
        .expect("read failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["name"], "smoke-read-01",
        "read deve retornar a memória pelo nome: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #06: list
// ---------------------------------------------------------------------------

#[test]
fn smoke_06_list() {
    let env = Env::new();
    env.init();
    env.remember("smoke-list-01", "memória para listar");
    let out = env
        .cmd()
        .args(["list", "--limit", "10"])
        .output()
        .expect("list failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let arr = json["items"]
        .as_array()
        .expect("list deve retornar objeto com campo 'items'");
    assert!(
        !arr.is_empty(),
        "list deve retornar pelo menos uma memória: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #07: forget
// ---------------------------------------------------------------------------

#[test]
fn smoke_07_forget() {
    let env = Env::new();
    env.init();
    env.remember("smoke-forget-01", "memória para deletar");
    let out = env
        .cmd()
        .args(["forget", "--name", "smoke-forget-01"])
        .output()
        .expect("forget failed");
    assert_json_stdout(&out);
    // v2.0.4: forget retorna {forgotten: true, name, namespace} — sem campo status
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["forgotten"], true,
        "forget deve retornar forgotten=true: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #08: purge
// ---------------------------------------------------------------------------

#[test]
fn smoke_08_purge() {
    let env = Env::new();
    env.init();
    env.remember("smoke-purge-01", "memória para purgar");
    // Soft-delete primeiro
    env.cmd()
        .args(["forget", "--name", "smoke-purge-01"])
        .output()
        .unwrap();
    let out = env
        .cmd()
        .args(["purge", "--yes"])
        .output()
        .expect("purge failed");
    assert_json_stdout(&out);
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #09: rename
// ---------------------------------------------------------------------------

#[test]
fn smoke_09_rename() {
    let env = Env::new();
    env.init();
    env.remember("smoke-rename-src", "memória para renomear");
    // v2.0.4: rename uses --name and --new-name (not --from/--to)
    let out = env
        .cmd()
        .args([
            "rename",
            "--name",
            "smoke-rename-src",
            "--new-name",
            "smoke-rename-dst",
        ])
        .output()
        .expect("rename failed");
    assert_json_stdout(&out);
    // v2.0.4: rename retorna {memory_id, name, version} — sem campo status
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["name"], "smoke-rename-dst",
        "rename deve retornar o novo nome: {json}"
    );
    assert!(
        json["memory_id"].as_i64().is_some(),
        "rename deve retornar memory_id: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #10: edit
// ---------------------------------------------------------------------------

#[test]
fn smoke_10_edit() {
    let env = Env::new();
    env.init();
    env.remember("smoke-edit-01", "conteúdo original");
    let out = env
        .cmd()
        .args([
            "edit",
            "--name",
            "smoke-edit-01",
            "--body",
            "conteúdo editado pelo smoke test",
        ])
        .output()
        .expect("edit failed");
    assert_json_stdout(&out);
    // v2.0.4: edit retorna {memory_id, name, action: "updated", version} — sem campo status
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["action"], "updated",
        "edit deve retornar action=updated: {json}"
    );
    assert!(
        json["memory_id"].as_i64().is_some(),
        "edit deve retornar memory_id: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #11: history
// ---------------------------------------------------------------------------

#[test]
fn smoke_11_history() {
    let env = Env::new();
    env.init();
    env.remember("smoke-history-01", "versão 1 do conteúdo");
    // Generate a second version
    env.cmd()
        .args([
            "edit",
            "--name",
            "smoke-history-01",
            "--body",
            "versão 2 do conteúdo",
        ])
        .output()
        .unwrap();
    let out = env
        .cmd()
        .args(["history", "--name", "smoke-history-01"])
        .output()
        .expect("history failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        json["versions"].is_array(),
        "history deve retornar array versions: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #12: restore
// ---------------------------------------------------------------------------

#[test]
fn smoke_12_restore() {
    let env = Env::new();
    env.init();
    env.remember("smoke-restore-01", "versão 1");
    env.cmd()
        .args(["edit", "--name", "smoke-restore-01", "--body", "versão 2"])
        .output()
        .unwrap();
    // Obtain versions through history
    let hist_out = env
        .cmd()
        .args(["history", "--name", "smoke-restore-01"])
        .output()
        .unwrap();
    let hist_json: serde_json::Value = serde_json::from_slice(&hist_out.stdout).unwrap();
    let versions = hist_json["versions"].as_array().unwrap();
    // Restore to the oldest available version
    // v2.0.4: field is "version" (not "version_id")
    if versions.len() >= 2 {
        let version_id = versions
            .iter()
            .map(|v| v["version"].as_i64().unwrap_or(0))
            .min()
            .unwrap_or(1);
        let out = env
            .cmd()
            .args([
                "restore",
                "--name",
                "smoke-restore-01",
                "--version",
                &version_id.to_string(),
            ])
            .output()
            .expect("restore failed");
        assert_json_stdout(&out);
    }
}
