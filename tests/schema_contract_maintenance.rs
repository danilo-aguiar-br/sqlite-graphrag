//! Strict schema contract: maintenance and diagnostics — health, migrate, optimize, vacuum, sync-safe-copy, cleanup-orphans, namespace-detect, debug-schema, fts, backup.
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
// 18 — health
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_18_health() {
    let env = Env::new();
    env.init();
    let output = env.cmd().arg("health").output().expect("health failed");
    assert!(
        output.status.success(),
        "health: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "health");
    validate_schema(
        "health",
        include_str!("../docs/schemas/health.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 19 — migrate
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_19_migrate() {
    let env = Env::new();
    env.init();
    let output = env.cmd().arg("migrate").output().expect("migrate failed");
    assert!(
        output.status.success(),
        "migrate: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "migrate");
    validate_schema(
        "migrate",
        include_str!("../docs/schemas/migrate.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 20 — optimize
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_20_optimize() {
    let env = Env::new();
    env.init();
    let output = env.cmd().arg("optimize").output().expect("optimize failed");
    assert!(
        output.status.success(),
        "optimize: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "optimize");
    validate_schema(
        "optimize",
        include_str!("../docs/schemas/optimize.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 21 — vacuum
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_21_vacuum() {
    let env = Env::new();
    env.init();
    let output = env.cmd().arg("vacuum").output().expect("vacuum failed");
    assert!(
        output.status.success(),
        "vacuum: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "vacuum");
    validate_schema(
        "vacuum",
        include_str!("../docs/schemas/vacuum.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 22 — sync-safe-copy
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_22_sync_safe_copy() {
    let env = Env::new();
    env.init();
    let destination = env.tmp.path().join("backup.sqlite");
    let output = env
        .cmd()
        .args([
            "sync-safe-copy",
            "--dest",
            destination.to_str().expect("caminho inválido"),
        ])
        .output()
        .expect("sync-safe-copy failed");
    assert!(
        output.status.success(),
        "sync-safe-copy: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "sync-safe-copy");
    validate_schema(
        "sync-safe-copy",
        include_str!("../docs/schemas/sync-safe-copy.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 23 — cleanup-orphans
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_23_cleanup_orphans() {
    let env = Env::new();
    env.init();
    let output = env
        .cmd()
        .args(["cleanup-orphans", "--dry-run"])
        .output()
        .expect("cleanup-orphans failed");
    assert!(
        output.status.success(),
        "cleanup-orphans: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "cleanup-orphans");
    validate_schema(
        "cleanup-orphans",
        include_str!("../docs/schemas/cleanup-orphans.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 24 — namespace-detect
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_24_namespace_detect() {
    let env = Env::new();
    env.init();
    let output = env
        .cmd()
        .arg("namespace-detect")
        .output()
        .expect("namespace-detect failed");
    assert!(
        output.status.success(),
        "namespace-detect: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "namespace-detect");
    validate_schema(
        "namespace-detect",
        include_str!("../docs/schemas/namespace-detect.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 25 — __debug_schema
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_25_debug_schema() {
    let env = Env::new();
    env.init();
    let output = env
        .cmd()
        .arg("debug-schema")
        .output()
        .expect("debug-schema failed");
    assert!(
        output.status.success(),
        "debug-schema: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "debug-schema");
    validate_schema(
        "debug-schema",
        include_str!("../docs/schemas/debug-schema.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 26 — fts rebuild
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_26_fts_rebuild() {
    let env = Env::new();
    env.init();
    let output = env
        .cmd()
        .args(["fts", "rebuild"])
        .output()
        .expect("fts rebuild failed");
    assert!(
        output.status.success(),
        "fts rebuild: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "fts-rebuild");
    validate_schema(
        "fts-rebuild",
        include_str!("../docs/schemas/fts-rebuild.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 27 — fts check
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_27_fts_check() {
    let env = Env::new();
    env.init();
    let output = env
        .cmd()
        .args(["fts", "check"])
        .output()
        .expect("fts check failed");
    assert!(
        output.status.success(),
        "fts check: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "fts-check");
    validate_schema(
        "fts-check",
        include_str!("../docs/schemas/fts-check.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 28 — fts stats
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_28_fts_stats() {
    let env = Env::new();
    env.init();
    let output = env
        .cmd()
        .args(["fts", "stats"])
        .output()
        .expect("fts stats failed");
    assert!(
        output.status.success(),
        "fts stats: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "fts-stats");
    validate_schema(
        "fts-stats",
        include_str!("../docs/schemas/fts-stats.schema.json"),
        &instance,
    );
}

// ---------------------------------------------------------------------------
// 29 — backup
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_29_backup() {
    let env = Env::new();
    env.init();
    let dest = env.tmp.path().join("schema-backup.sqlite");
    let output = env
        .cmd()
        .args(["backup", "--output", dest.to_str().unwrap()])
        .output()
        .expect("backup failed");
    assert!(
        output.status.success(),
        "backup: exit {:?}",
        output.status.code()
    );
    let instance = Env::parse_stdout(&output, "backup");
    validate_schema(
        "backup",
        include_str!("../docs/schemas/backup.schema.json"),
        &instance,
    );
}
