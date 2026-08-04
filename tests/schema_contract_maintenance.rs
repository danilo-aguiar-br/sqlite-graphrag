//! Strict schema contract: maintenance and diagnostics — health, migrate, optimize, vacuum, sync-safe-copy, cleanup-orphans, namespace-detect, debug-schema, fts, backup.
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
// 18 — health
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_18_health() {
    let env = Env::new();
    env.init();
    let saida = env.cmd().arg("health").output().expect("health failed");
    assert!(
        saida.status.success(),
        "health: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "health");
    validar_schema(
        "health",
        include_str!("../docs/schemas/health.schema.json"),
        &instancia,
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
    let saida = env.cmd().arg("migrate").output().expect("migrate failed");
    assert!(
        saida.status.success(),
        "migrate: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "migrate");
    validar_schema(
        "migrate",
        include_str!("../docs/schemas/migrate.schema.json"),
        &instancia,
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
    let saida = env.cmd().arg("optimize").output().expect("optimize failed");
    assert!(
        saida.status.success(),
        "optimize: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "optimize");
    validar_schema(
        "optimize",
        include_str!("../docs/schemas/optimize.schema.json"),
        &instancia,
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
    let saida = env.cmd().arg("vacuum").output().expect("vacuum failed");
    assert!(
        saida.status.success(),
        "vacuum: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "vacuum");
    validar_schema(
        "vacuum",
        include_str!("../docs/schemas/vacuum.schema.json"),
        &instancia,
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
    let destino = env.tmp.path().join("backup.sqlite");
    let saida = env
        .cmd()
        .args([
            "sync-safe-copy",
            "--dest",
            destino.to_str().expect("caminho inválido"),
        ])
        .output()
        .expect("sync-safe-copy failed");
    assert!(
        saida.status.success(),
        "sync-safe-copy: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "sync-safe-copy");
    validar_schema(
        "sync-safe-copy",
        include_str!("../docs/schemas/sync-safe-copy.schema.json"),
        &instancia,
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
    let saida = env
        .cmd()
        .args(["cleanup-orphans", "--dry-run"])
        .output()
        .expect("cleanup-orphans failed");
    assert!(
        saida.status.success(),
        "cleanup-orphans: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "cleanup-orphans");
    validar_schema(
        "cleanup-orphans",
        include_str!("../docs/schemas/cleanup-orphans.schema.json"),
        &instancia,
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
    let saida = env
        .cmd()
        .arg("namespace-detect")
        .output()
        .expect("namespace-detect failed");
    assert!(
        saida.status.success(),
        "namespace-detect: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "namespace-detect");
    validar_schema(
        "namespace-detect",
        include_str!("../docs/schemas/namespace-detect.schema.json"),
        &instancia,
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
    let saida = env
        .cmd()
        .arg("debug-schema")
        .output()
        .expect("debug-schema failed");
    assert!(
        saida.status.success(),
        "debug-schema: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "debug-schema");
    validar_schema(
        "debug-schema",
        include_str!("../docs/schemas/debug-schema.schema.json"),
        &instancia,
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
    let saida = env
        .cmd()
        .args(["fts", "rebuild"])
        .output()
        .expect("fts rebuild failed");
    assert!(
        saida.status.success(),
        "fts rebuild: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "fts-rebuild");
    validar_schema(
        "fts-rebuild",
        include_str!("../docs/schemas/fts-rebuild.schema.json"),
        &instancia,
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
    let saida = env
        .cmd()
        .args(["fts", "check"])
        .output()
        .expect("fts check failed");
    assert!(
        saida.status.success(),
        "fts check: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "fts-check");
    validar_schema(
        "fts-check",
        include_str!("../docs/schemas/fts-check.schema.json"),
        &instancia,
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
    let saida = env
        .cmd()
        .args(["fts", "stats"])
        .output()
        .expect("fts stats failed");
    assert!(
        saida.status.success(),
        "fts stats: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "fts-stats");
    validar_schema(
        "fts-stats",
        include_str!("../docs/schemas/fts-stats.schema.json"),
        &instancia,
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
    let saida = env
        .cmd()
        .args(["backup", "--output", dest.to_str().unwrap()])
        .output()
        .expect("backup failed");
    assert!(
        saida.status.success(),
        "backup: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "backup");
    validar_schema(
        "backup",
        include_str!("../docs/schemas/backup.schema.json"),
        &instancia,
    );
}
