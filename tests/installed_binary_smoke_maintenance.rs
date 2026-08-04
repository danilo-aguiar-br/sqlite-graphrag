#![cfg(feature = "slow-tests")]

//! Suite 10 — smoke tests against the INSTALLED binary: search and database maintenance (#13–#19)
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
// Suite 10 — Smoke #13: hybrid-search
// ---------------------------------------------------------------------------

#[test]
fn smoke_13_hybrid_search() {
    let env = Env::new();
    env.init();
    env.remember(
        "smoke-hybrid-01",
        "conteúdo para busca híbrida com FTS e vetorial",
    );
    let out = env
        .cmd()
        .args(["hybrid-search", "busca híbrida", "-k", "5"])
        .output()
        .expect("hybrid-search failed");
    assert_json_or_not_found(&out);
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #14: stats
// ---------------------------------------------------------------------------

#[test]
fn smoke_14_stats() {
    let env = Env::new();
    env.init();
    let out = env.cmd().arg("stats").output().expect("stats failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        json["memories"].as_i64().is_some(),
        "stats deve ter campo memories como inteiro: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #15: migrate
// ---------------------------------------------------------------------------

#[test]
fn smoke_15_migrate() {
    let env = Env::new();
    env.init();
    let out = env.cmd().arg("migrate").output().expect("migrate failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["status"], "ok",
        "migrate deve retornar status=ok: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #16: namespace-detect
// ---------------------------------------------------------------------------

#[test]
fn smoke_16_namespace_detect() {
    let env = Env::new();
    env.init();
    let out = env
        .cmd()
        .arg("namespace-detect")
        .output()
        .expect("namespace-detect failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        json["namespace"].is_string(),
        "namespace-detect deve retornar campo namespace: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #17: optimize
// ---------------------------------------------------------------------------

#[test]
fn smoke_17_optimize() {
    let env = Env::new();
    env.init();
    let out = env.cmd().arg("optimize").output().expect("optimize failed");
    assert_json_stdout(&out);
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #18: sync-safe-copy
// ---------------------------------------------------------------------------

#[test]
fn smoke_18_sync_safe_copy() {
    let env = Env::new();
    env.init();
    let dest = env.tmp.path().join("snapshot.sqlite");
    let out = env
        .cmd()
        .args(["sync-safe-copy", "--dest", dest.to_str().unwrap()])
        .output()
        .expect("sync-safe-copy failed");
    assert_json_stdout(&out);
    assert!(dest.exists(), "snapshot deve ter sido criado em {dest:?}");
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #19: vacuum
// ---------------------------------------------------------------------------

#[test]
fn smoke_19_vacuum() {
    let env = Env::new();
    env.init();
    let out = env.cmd().arg("vacuum").output().expect("vacuum failed");
    assert_json_stdout(&out);
}
