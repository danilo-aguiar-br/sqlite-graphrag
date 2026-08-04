//! v1.0.82 (GAP-003): integration tests for the `--llm-backend` flag
//! propagation across the 6 write/read paths (`remember`, `edit`,
//! `ingest`, `enrich`, `recall`, `hybrid-search`).
//!
//! The flag is a global `Cli` flag added in v1.0.82 (GAP-003). Each
//! command accepts `LlmBackendChoice::{Auto,Claude,Codex,None}` and
//! routes the embedding call through `embedder::embed_with_fallback`
//! or `embedder::try_embed_query_with_choice`.
//!
//! These tests verify the `None` path (which short-circuits the LLM
//! and returns an empty vector) because the mock LLM cannot reliably
//! emit deterministic vectors across releases — the `None` path is
//! the only one that produces a deterministic, reproducible outcome
//! without OAuth.

#![cfg(feature = "slow-tests")]

use assert_cmd::Command;
use serde_json::Value;
use serial_test::serial;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;

/// Builds a fresh `Command` with the mock LLM PATH prepended so any
/// accidental fallback to `codex`/`claude` (rather than `none`) does
/// not crash the test — the mock returns a fixed 64-dim zero vector.
fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

fn cmd_base(tmp: &TempDir) -> Command {
    let mut c = sgr_cmd();
    common::wire_assert_cmd(tmp, &mut c, "test.sqlite");
    c.env("XDG_CACHE_HOME", tmp.path().join("cache"));
    c.arg("--skip-memory-guard");
    // `--embedding-backend auto` prepends OpenRouter whenever a client is
    // live, so the `none` chain can only terminate on `none` in a KEYLESS
    // sandbox. That is the precondition this suite has always assumed.
    common::write_sandbox_config_without_key(&tmp.path().join("config"), None);
    c
}

/// GAP-CLI-EMBED-NONE (v1.1.8) contract, superseding BUG-11 (v1.0.88,
/// ADR-0046) for an INTENTIONAL `none`-only chain.
///
/// BUG-11 made a fallback chain of only `[None]` abort with exit 11 and
/// "no LLM backends available; fallback chain exhausted", so that a memory
/// could never be persisted carrying an invisible zero-dimensional embedding.
/// `src/embedder/backend.rs::embed_via_backend_strict` now splits the two
/// cases: reaching `None` AFTER a real backend failed still propagates the
/// prior error (BUG-11 intact), but an explicit `--llm-backend none` — with
/// `last_err.is_none()` — returns an empty vector and the write succeeds while
/// `run_embed_phase` maps that empty vector to "no embedding at all".
///
/// The question under test is unchanged: does `--llm-backend none` avoid
/// persisting a bogus embedding? Only the mechanism changed, from "abort" to
/// "persist the memory with NO embedding row", so the assertion moved from
/// exit 11 to exit 0 plus `backend_invoked = none` plus a `health` report
/// showing the memory has no vector.
#[test]
#[serial]
fn llm_backend_none_persists_without_embedding() {
    let tmp = TempDir::new().expect("tempdir");
    let out = cmd_base(&tmp)
        .arg("remember")
        .arg("--name")
        .arg("smoke-none")
        .arg("--type")
        .arg("note")
        .arg("--description")
        .arg("GAP-CLI-EMBED-NONE none backend")
        .arg("--body")
        .arg("body without LLM call")
        .arg("--llm-backend")
        .arg("none")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).expect("remember stdout must be valid JSON");
    assert_eq!(
        json["backend_invoked"], "none",
        "the envelope must report the none backend, got: {json}"
    );

    let health = cmd_base(&tmp)
        .arg("health")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let health: Value = serde_json::from_slice(&health).expect("health stdout must be valid JSON");
    assert_eq!(
        health["vec_memories_missing"], 1,
        "the memory must be persisted WITHOUT an embedding row, got: {health}"
    );
    assert_eq!(
        health["vec_memories_coverage_pct"], 0.0,
        "no zero-dimensional vector may be persisted, got: {health}"
    );
}

/// Same GAP-CLI-EMBED-NONE (v1.1.8) contract reached through the GLOBAL
/// position of the flag, `sqlite-graphrag --llm-backend none remember ...`,
/// rather than the per-subcommand position used by the sibling test.
///
/// The test was named `..._via_env_var_aborts` and its docblock claimed
/// `SQLITE_GRAPHRAG_LLM_BACKEND=none` as the channel, but the body never set
/// that variable — it always passed the global flag. GAP-SG-101 / G-T-XDG-04
/// (v1.2.0) then retired every product `SQLITE_GRAPHRAG_*` binding, so the
/// documented channel no longer exists at all. Name and docblock now describe
/// what the body actually exercises. The question under test is unchanged:
/// does the choice reach the embedding phase from the global flag position,
/// with the same outcome as the subcommand position?
#[test]
#[serial]
fn llm_backend_none_via_global_flag_persists_without_embedding() {
    let tmp = TempDir::new().expect("tempdir");
    let out = cmd_base(&tmp)
        .arg("--llm-backend")
        .arg("none")
        .arg("remember")
        .arg("--name")
        .arg("smoke-global-none")
        .arg("--type")
        .arg("note")
        .arg("--description")
        .arg("GAP-CLI-EMBED-NONE global flag")
        .arg("--body")
        .arg("body via global flag")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).expect("remember stdout must be valid JSON");
    assert_eq!(
        json["backend_invoked"], "none",
        "the global flag must reach the embedding phase, got: {json}"
    );

    let health = cmd_base(&tmp)
        .arg("health")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let health: Value = serde_json::from_slice(&health).expect("health stdout must be valid JSON");
    assert_eq!(
        health["vec_memories_missing"], 1,
        "the memory must be persisted WITHOUT an embedding row, got: {health}"
    );
}

/// GAP-003 acceptance: invalid values are rejected at clap parse time
/// with exit code 2 (clap arg-parsing error). The error envelope
/// surfaces the accepted values via the `--help` text of the flag.
#[test]
#[serial]
fn llm_backend_rejects_unknown_value() {
    let tmp = TempDir::new().expect("tempdir");
    cmd_base(&tmp)
        .arg("remember")
        .arg("--name")
        .arg("smoke-invalid")
        .arg("--type")
        .arg("note")
        .arg("--description")
        .arg("GAP-003 invalid value")
        .arg("--body")
        .arg("x")
        .arg("--llm-backend")
        .arg("totally-bogus")
        .arg("--json")
        .assert()
        .failure()
        .code(2);
}
