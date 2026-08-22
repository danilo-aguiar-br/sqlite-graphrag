//! Strict schema contract: memory lifecycle — init, stats, remember, list, read, edit, rename, history, forget, restore.
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

use serial_test::serial;
use support::{validate_schema, Env};
// ---------------------------------------------------------------------------
// 01 — init
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_01_init() {
    let env = Env::new();
    let output = env.cmd().arg("init").output().expect("init failed");
    assert!(
        output.status.success(),
        "init: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "init");
    validate_schema(
        "init",
        include_str!("../docs/schemas/init.schema.json"),
        &instance,
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
    let output = env.cmd().arg("stats").output().expect("stats failed");
    assert!(
        output.status.success(),
        "stats: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "stats");
    validate_schema(
        "stats",
        include_str!("../docs/schemas/stats.schema.json"),
        &instance,
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
    let instance = env.remember_simple("mem-schema-remember");
    validate_schema(
        "remember",
        include_str!("../docs/schemas/remember.schema.json"),
        &instance,
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
    env.remember_simple("mem-schema-list");
    let output = env
        .cmd()
        .args(["list", "--namespace", "global"])
        .output()
        .expect("list failed");
    assert!(
        output.status.success(),
        "list: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "list");
    validate_schema(
        "list",
        include_str!("../docs/schemas/list.schema.json"),
        &instance,
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
    env.remember_simple("mem-schema-read");
    let output = env
        .cmd()
        .args(["read", "--name", "mem-schema-read"])
        .output()
        .expect("read failed");
    assert!(
        output.status.success(),
        "read: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "read");
    validate_schema(
        "read",
        include_str!("../docs/schemas/read.schema.json"),
        &instance,
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
    env.remember_simple("mem-schema-edit");
    let output = env
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
        output.status.success(),
        "edit: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "edit");
    validate_schema(
        "edit",
        include_str!("../docs/schemas/edit.schema.json"),
        &instance,
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
    env.remember_simple("mem-schema-rename-origem");
    let output = env
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
        output.status.success(),
        "rename: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "rename");
    validate_schema(
        "rename",
        include_str!("../docs/schemas/rename.schema.json"),
        &instance,
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
    env.remember_simple("mem-schema-history");
    let output = env
        .cmd()
        .args(["history", "--name", "mem-schema-history"])
        .output()
        .expect("history failed");
    assert!(
        output.status.success(),
        "history: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "history");
    validate_schema(
        "history",
        include_str!("../docs/schemas/history.schema.json"),
        &instance,
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
    env.remember_simple("mem-schema-forget");
    let output = env
        .cmd()
        .args(["forget", "--name", "mem-schema-forget"])
        .output()
        .expect("forget failed");
    assert!(
        output.status.success(),
        "forget: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "forget");
    validate_schema(
        "forget",
        include_str!("../docs/schemas/forget.schema.json"),
        &instance,
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
    env.remember_simple("mem-schema-restore");
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
    let output = env
        .cmd()
        .args(["restore", "--name", "mem-schema-restore", "--version", "1"])
        .output()
        .expect("restore failed");
    assert!(
        output.status.success(),
        "restore: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "restore");
    validate_schema(
        "restore",
        include_str!("../docs/schemas/restore.schema.json"),
        &instance,
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
    let output = env
        .cmd()
        .args(["purge", "--dry-run", "--namespace", "global"])
        .output()
        .expect("purge failed");
    assert!(
        output.status.success(),
        "purge: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "purge");
    validate_schema(
        "purge",
        include_str!("../docs/schemas/purge.schema.json"),
        &instance,
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
    env.remember_simple("mem-schema-recall");
    let output = env
        .cmd()
        .args(["recall", "schema recall teste", "--k", "3"])
        .output()
        .expect("recall failed");
    assert!(
        output.status.success(),
        "recall: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "recall");
    validate_schema(
        "recall",
        include_str!("../docs/schemas/recall.schema.json"),
        &instance,
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
    env.remember_simple("mem-schema-hybrid");
    let output = env
        .cmd()
        .args(["hybrid-search", "busca hibrida schema", "--k", "3"])
        .output()
        .expect("hybrid-search failed");
    assert!(
        output.status.success(),
        "hybrid-search: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "hybrid-search");
    validate_schema(
        "hybrid-search",
        include_str!("../docs/schemas/hybrid-search.schema.json"),
        &instance,
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
        let output = cmd
            .args(["hybrid-search", "busca hibrida schema", "--k", "3"])
            .output()
            .expect("hybrid-search com surface failed");
        assert!(
            output.status.success(),
            "hybrid-search {flags:?}: exit {:?}",
            output.status.code()
        );
        let instance = Env::parse_stdout(&output, "hybrid-search");
        validate_schema(
            "hybrid-search",
            include_str!("../docs/schemas/hybrid-search.schema.json"),
            &instance,
        );
        assert!(
            instance.get("graph_matches").is_some(),
            "graph_matches is required and is disjoint from results in hybrid-search, \
             so no surface knob may drop it (flags {flags:?}): {instance}"
        );
    }
}
