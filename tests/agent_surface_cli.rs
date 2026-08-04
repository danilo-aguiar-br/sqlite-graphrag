//! GAP-SG-142: end-to-end wiring of the agent-native output surface.
//!
//! The unit tests in `src/agent_surface/tests.rs` prove the reshaping algebra.
//! These tests prove the CLI actually *reaches* it: that the global flags parse,
//! that they are installed before dispatch, and that the documented invariants
//! survive the real binary.
//!
//! Every case here runs against a subcommand that never opens the database, so
//! the suite stays hermetic and fast.

use std::process::{Command, Stdio};

/// Path to the integration binary Cargo builds for this test target.
const BIN: &str = env!("CARGO_BIN_EXE_sqlite-graphrag");

/// Runs the CLI with an isolated config directory so the host's `config.toml`
/// cannot change the resolved surface.
fn run(config_dir: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(BIN)
        .arg("--config-dir")
        .arg(config_dir)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn sqlite-graphrag");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A throwaway config directory for one test.
fn isolated_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sg-agent-surface-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create isolated config dir");
    dir
}

fn parse(stdout: &str) -> serde_json::Value {
    serde_json::from_str(stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {stdout}"))
}

#[test]
fn select_reduces_the_envelope_on_a_real_invocation() {
    let dir = isolated_dir("select");
    let (code, full, stderr) = run(&dir, &["config", "list-keys"]);
    assert_eq!(code, 0, "baseline failed: {stderr}");
    let baseline = parse(&full);
    assert!(baseline.get("keys").is_some(), "baseline shape changed");

    let (code, shaped_out, stderr) = run(&dir, &["config", "list-keys", "--select", "provider"]);
    assert_eq!(code, 0, "projected run failed: {stderr}");
    let shaped = parse(&shaped_out);

    // The surface announces itself, and nothing else leaked into stdout.
    assert!(
        shaped.get("agent_surface").is_some(),
        "projection must record itself: {shaped_out}"
    );
    for item in shaped["keys"].as_array().expect("keys stays an array") {
        let obj = item.as_object().expect("each key entry is an object");
        assert!(!obj.contains_key("masked_value"), "projection leaked a key");
    }
}

#[test]
fn fields_is_accepted_as_a_spelling_of_select() {
    let dir = isolated_dir("fields");
    let (code, stdout, stderr) = run(&dir, &["config", "list-keys", "--fields", "provider"]);
    assert_eq!(code, 0, "--fields must parse: {stderr}");
    assert!(parse(&stdout).get("agent_surface").is_some());
}

#[test]
fn count_only_replaces_the_payload_with_a_count() {
    let dir = isolated_dir("count");
    let (code, stdout, stderr) = run(&dir, &["config", "list-keys", "--count-only"]);
    assert_eq!(code, 0, "--count-only must succeed: {stderr}");
    let value = parse(&stdout);
    assert!(
        value
            .get("count")
            .and_then(serde_json::Value::as_u64)
            .is_some(),
        "--count-only must emit a numeric count: {stdout}"
    );
    assert!(value.get("keys").is_none(), "payload must be replaced");
}

#[test]
fn filter_never_silences_a_failure_envelope() {
    let dir = isolated_dir("filter-error");
    // `config remove-key` on an absent fingerprint fails (exit 4) and the
    // filter deliberately matches nothing.
    let (code, stdout, _stderr) = run(
        &dir,
        &[
            "config",
            "remove-key",
            "0000000000000000",
            "--filter",
            "name=nothing-matches-this",
        ],
    );
    assert_ne!(code, 0, "the underlying command must still fail");
    let value = parse(&stdout);
    assert!(
        value.get("error").is_some() || value.get("ok") == Some(&serde_json::Value::Bool(false)),
        "a failure envelope must survive --filter untouched: {stdout}"
    );
    assert!(
        value.get("agent_surface").is_none(),
        "failure envelopes are never reshaped: {stdout}"
    );
}

#[test]
fn malformed_filter_fails_fast_with_exit_2() {
    let dir = isolated_dir("filter-bad");
    let (code, _stdout, stderr) = run(&dir, &["config", "list-keys", "--filter", "no-operator"]);
    assert_eq!(
        code, 2,
        "a malformed --filter must abort instead of returning an empty set: {stderr}"
    );
}

#[test]
fn max_output_bytes_caps_stdout_and_says_so() {
    let dir = isolated_dir("bytes");
    let (code, stdout, stderr) = run(&dir, &["config", "list-keys", "--max-output-bytes", "40"]);
    assert_eq!(code, 0, "capped run failed: {stderr}");
    let value = parse(&stdout);
    assert_eq!(
        value.get("truncated"),
        Some(&serde_json::Value::Bool(true)),
        "byte truncation must never be silent: {stdout}"
    );
    // Whatever was emitted still parses as JSON — asserted by `parse` above.
}

#[test]
fn print_schema_emits_without_touching_the_database() {
    let dir = isolated_dir("schema");
    let db = dir.join("absent-on-purpose.sqlite");
    let db_arg = db.display().to_string();
    let (code, stdout, stderr) = run(&dir, &["recall", "--print-schema", "--db", &db_arg]);
    assert_eq!(code, 0, "--print-schema must succeed: {stderr}");
    let value = parse(&stdout);
    assert!(
        value.get("$schema").is_some() || value.get("type").is_some(),
        "--print-schema must emit a JSON Schema document: {stdout}"
    );
    assert!(
        !db.exists(),
        "--print-schema must not create or open the database"
    );
}

#[test]
fn print_schema_is_not_reshaped_by_the_surface() {
    let dir = isolated_dir("schema-shape");
    let (code, stdout, stderr) = run(
        &dir,
        &[
            "recall",
            "--print-schema",
            "--select",
            "type",
            "--count-only",
        ],
    );
    assert_eq!(code, 0, "--print-schema must succeed: {stderr}");
    let value = parse(&stdout);
    assert!(
        value.get("count").is_none(),
        "a schema document is a contract, not a result set: {stdout}"
    );
}

#[test]
fn no_surface_flags_leaves_the_envelope_untouched() {
    let dir = isolated_dir("noop");
    let (code, stdout, stderr) = run(&dir, &["config", "list-keys"]);
    assert_eq!(code, 0, "baseline failed: {stderr}");
    assert!(
        parse(&stdout).get("agent_surface").is_none(),
        "the surface must stay invisible when no flag is set: {stdout}"
    );
}
