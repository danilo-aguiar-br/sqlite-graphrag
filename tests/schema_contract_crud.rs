//! Strict schema contract: memory lifecycle — init, stats, remember, list, read, edit, rename, history, forget, restore.
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
// 01 — init
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_01_init() {
    let env = Env::new();
    let saida = env.cmd().arg("init").output().expect("init failed");
    assert!(
        saida.status.success(),
        "init: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "init");
    validar_schema(
        "init",
        include_str!("../docs/schemas/init.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 02 — stats
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_02_stats() {
    let env = Env::new();
    env.init();
    let saida = env.cmd().arg("stats").output().expect("stats failed");
    assert!(
        saida.status.success(),
        "stats: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "stats");
    validar_schema(
        "stats",
        include_str!("../docs/schemas/stats.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 03 — remember
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_03_remember() {
    let env = Env::new();
    env.init();
    let instancia = env.remember_simples("mem-schema-remember");
    validar_schema(
        "remember",
        include_str!("../docs/schemas/remember.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 04 — list
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_04_list() {
    let env = Env::new();
    env.init();
    env.remember_simples("mem-schema-list");
    let saida = env
        .cmd()
        .args(["list", "--namespace", "global"])
        .output()
        .expect("list failed");
    assert!(
        saida.status.success(),
        "list: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "list");
    validar_schema(
        "list",
        include_str!("../docs/schemas/list.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 05 — read
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_05_read() {
    let env = Env::new();
    env.init();
    env.remember_simples("mem-schema-read");
    let saida = env
        .cmd()
        .args(["read", "--name", "mem-schema-read"])
        .output()
        .expect("read failed");
    assert!(
        saida.status.success(),
        "read: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "read");
    validar_schema(
        "read",
        include_str!("../docs/schemas/read.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 06 — edit
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_06_edit() {
    let env = Env::new();
    env.init();
    env.remember_simples("mem-schema-edit");
    let saida = env
        .cmd()
        .args([
            "edit",
            "--name",
            "mem-schema-edit",
            "--body",
            "corpo-editado-para-schema",
        ])
        .output()
        .expect("edit failed");
    assert!(
        saida.status.success(),
        "edit: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "edit");
    validar_schema(
        "edit",
        include_str!("../docs/schemas/edit.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 07 — rename
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_07_rename() {
    let env = Env::new();
    env.init();
    env.remember_simples("mem-schema-rename-origem");
    let saida = env
        .cmd()
        .args([
            "rename",
            "--name",
            "mem-schema-rename-origem",
            "--new-name",
            "mem-schema-rename-destino",
        ])
        .output()
        .expect("rename failed");
    assert!(
        saida.status.success(),
        "rename: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "rename");
    validar_schema(
        "rename",
        include_str!("../docs/schemas/rename.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 08 — history
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_08_history() {
    let env = Env::new();
    env.init();
    env.remember_simples("mem-schema-history");
    let saida = env
        .cmd()
        .args(["history", "--name", "mem-schema-history"])
        .output()
        .expect("history failed");
    assert!(
        saida.status.success(),
        "history: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "history");
    validar_schema(
        "history",
        include_str!("../docs/schemas/history.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 09 — forget
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_09_forget() {
    let env = Env::new();
    env.init();
    env.remember_simples("mem-schema-forget");
    let saida = env
        .cmd()
        .args(["forget", "--name", "mem-schema-forget"])
        .output()
        .expect("forget failed");
    assert!(
        saida.status.success(),
        "forget: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "forget");
    validar_schema(
        "forget",
        include_str!("../docs/schemas/forget.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 10 — restore
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_10_restore() {
    let env = Env::new();
    env.init();
    env.remember_simples("mem-schema-restore");
    // Create a second version via edit
    env.cmd()
        .args([
            "edit",
            "--name",
            "mem-schema-restore",
            "--body",
            "versao-dois",
        ])
        .assert()
        .success();
    let saida = env
        .cmd()
        .args(["restore", "--name", "mem-schema-restore", "--version", "1"])
        .output()
        .expect("restore failed");
    assert!(
        saida.status.success(),
        "restore: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "restore");
    validar_schema(
        "restore",
        include_str!("../docs/schemas/restore.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 11 — purge
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_11_purge() {
    let env = Env::new();
    env.init();
    let saida = env
        .cmd()
        .args(["purge", "--dry-run", "--namespace", "global"])
        .output()
        .expect("purge failed");
    assert!(
        saida.status.success(),
        "purge: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "purge");
    validar_schema(
        "purge",
        include_str!("../docs/schemas/purge.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 12 — recall
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_12_recall() {
    let env = Env::new();
    env.init();
    env.remember_simples("mem-schema-recall");
    let saida = env
        .cmd()
        .args(["recall", "schema recall teste", "--k", "3"])
        .output()
        .expect("recall failed");
    assert!(
        saida.status.success(),
        "recall: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "recall");
    validar_schema(
        "recall",
        include_str!("../docs/schemas/recall.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 13 — hybrid-search
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_13_hybrid_search() {
    let env = Env::new();
    env.init();
    env.remember_simples("mem-schema-hybrid");
    let saida = env
        .cmd()
        .args(["hybrid-search", "busca hibrida schema", "--k", "3"])
        .output()
        .expect("hybrid-search failed");
    assert!(
        saida.status.success(),
        "hybrid-search: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "hybrid-search");
    validar_schema(
        "hybrid-search",
        include_str!("../docs/schemas/hybrid-search.schema.json"),
        &instancia,
    );

    // The same envelope with the agent-native surface active. Validating only
    // the bare invocation is exactly why alias suppression could delete
    // `graph_matches` — a member this schema lists under `required` — and ship
    // an envelope invalid against the project's own contract.
    //
    // `--select` is deliberately absent: it projects result objects, so it drops
    // item members the schema lists under `required` for EVERY subcommand, not
    // just this one. That is a separate contract question about whether the
    // schemas describe the projected envelope, and it cannot be answered by
    // loosening one schema in isolation. The knobs exercised here reshape the
    // array without touching the shape of its elements.
    for flags in [
        vec!["--max-items", "1"],
        vec!["--filter", "name!=nothing-matches-this"],
        vec!["--dedupe-by", "name"],
    ] {
        let mut cmd = env.cmd();
        cmd.args(&flags);
        let saida = cmd
            .args(["hybrid-search", "busca hibrida schema", "--k", "3"])
            .output()
            .expect("hybrid-search com surface failed");
        assert!(
            saida.status.success(),
            "hybrid-search {flags:?}: exit {:?}",
            saida.status.code()
        );
        let instancia = Env::parse_stdout(&saida, "hybrid-search");
        validar_schema(
            "hybrid-search",
            include_str!("../docs/schemas/hybrid-search.schema.json"),
            &instancia,
        );
        assert!(
            instancia.get("graph_matches").is_some(),
            "graph_matches is required and is disjoint from results in hybrid-search, \
             so no surface knob may drop it (flags {flags:?}): {instancia}"
        );
    }
}
