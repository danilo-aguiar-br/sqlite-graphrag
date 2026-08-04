//! CLI-level validation tests for the v1.0.95 `--mode openrouter` JUDGE.
//!
//! These exercise the argument-validation layer of `enrich` end to end
//! through the real binary (via `assert_cmd`), without touching the network
//! or a database: both checks fire *before* any DB or HTTP work in `run()`.

use assert_cmd::Command;
use predicates::str::contains;

/// `--mode openrouter` without `--openrouter-model` must fail fast with exit
/// code 1 (AppError::Validation) and name the missing flag. The model check
/// runs before any API-key or DB access, so the outcome does not depend on
/// `OPENROUTER_API_KEY` being present in the environment.
#[test]
fn openrouter_mode_requires_model_flag() {
    Command::cargo_bin("sqlite-graphrag")
        .expect("binary builds")
        .args([
            "enrich",
            "--operation",
            "memory-bindings",
            "--mode",
            "openrouter",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("openrouter-model"));
}

/// The cross-provider flags were REMOVED with the subprocess backends, so the
/// rejection moved one layer earlier: clap now refuses the unknown argument
/// with exit 2 instead of the G20 conflict check returning exit 1. The input
/// under test is unchanged and it still must not be accepted.
#[test]
fn openrouter_mode_rejects_removed_claude_flag() {
    Command::cargo_bin("sqlite-graphrag")
        .expect("binary builds")
        .args([
            "enrich",
            "--operation",
            "memory-bindings",
            "--mode",
            "openrouter",
            "--openrouter-model",
            "deepseek/deepseek-v4-flash",
            "--claude-binary",
            "/usr/bin/true",
        ])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("claude-binary"));
}

/// A removed codex flag is likewise refused by clap with exit 2.
#[test]
fn openrouter_mode_rejects_removed_codex_flag() {
    Command::cargo_bin("sqlite-graphrag")
        .expect("binary builds")
        .args([
            "enrich",
            "--operation",
            "memory-bindings",
            "--mode",
            "openrouter",
            "--openrouter-model",
            "z-ai/glm-5.2",
            "--codex-model",
            "gpt-5.4-mini",
        ])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("codex-model"));
}
