//! GAP-SG-199: the busy-retry policy asserted under an isolated XDG root.
//!
//! `storage::utils::tests::resolved_busy_policy_defaults_match_constants` used
//! to compare a RESOLVED value against a compiled constant. The resolver reads
//! the real XDG config, so an operator who ran `config set db.busy_retries 12`
//! — a documented, legitimate key — turned the suite red without touching a
//! line of code. GAP-SG-198 recorded that as a leftover and closed anyway.
//!
//! The claim itself is worth keeping: an absent override MUST resolve to the
//! factory default, or the constants are decoration. It just cannot be checked
//! against whatever config the developer happens to have. So it moved here,
//! where `common::isolated_env` redirects every directory into a `TempDir` that
//! `xdg_isolation_guard` proves is really isolated.
//!
//! Asserting through the binary's own surface rather than the private resolver
//! is deliberate: `config list --effective` is what an operator reads to learn
//! the active value, so a drift invisible there is a drift that does not matter,
//! and one visible there is exactly the one this gate must catch.

#[path = "common/mod.rs"]
mod common;

use sqlite_graphrag::constants::{MAX_SQLITE_BUSY_RETRIES, SQLITE_BUSY_BASE_DELAY_MS};

/// Parses the `settings` map out of a `config list` envelope.
fn settings_of(stdout: &[u8]) -> serde_json::Value {
    let text = String::from_utf8_lossy(stdout);
    let value: serde_json::Value = serde_json::from_str(text.trim())
        .unwrap_or_else(|e| panic!("config list must emit one JSON object: {e}\n{text}"));
    value
        .get("settings")
        .cloned()
        .unwrap_or_else(|| panic!("config list envelope has no `settings` member:\n{text}"))
}

#[test]
fn an_absent_override_resolves_to_the_compiled_default() {
    let env = common::isolated_env();

    let output = env
        .cmd()
        .args(["config", "list", "--effective", "--json"])
        .output()
        .expect("config list must run");
    assert!(
        output.status.success(),
        "config list --effective failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let settings = settings_of(&output.stdout);
    assert_eq!(
        settings.get("db.busy_retries").and_then(|v| v.as_str()),
        Some(MAX_SQLITE_BUSY_RETRIES.to_string().as_str()),
        "with no XDG override, the effective retry budget must be the compiled \
         constant; anything else means the default is decoration"
    );
    assert_eq!(
        settings
            .get("db.busy_base_delay_ms")
            .and_then(|v| v.as_str()),
        Some(SQLITE_BUSY_BASE_DELAY_MS.to_string().as_str()),
        "same claim for the base delay"
    );
}

#[test]
fn a_stored_override_wins_over_the_compiled_default() {
    let env = common::isolated_env();

    env.cmd()
        .args(["config", "set", "db.busy_retries", "12", "--json"])
        .assert()
        .success();

    let output = env
        .cmd()
        .args(["config", "list", "--effective", "--json"])
        .output()
        .expect("config list must run");
    assert!(output.status.success());

    let settings = settings_of(&output.stdout);
    assert_eq!(
        settings.get("db.busy_retries").and_then(|v| v.as_str()),
        Some("12"),
        "a stored value must shadow the constant; without this half the test \
         above would also pass on a resolver that ignores XDG entirely"
    );
}

#[test]
fn setting_the_key_on_the_host_cannot_reach_this_test() {
    // The regression in one line: the sandbox must not inherit whatever the
    // developer stored. If `isolated_env` ever stops redirecting the config
    // root, the assertion above starts reading the real machine again and this
    // file becomes as fragile as the unit test it replaced.
    let env = common::isolated_env();
    let output = env
        .cmd()
        .args(["config", "path"])
        .output()
        .expect("config path must run");
    let text = String::from_utf8_lossy(&output.stdout);
    let sandbox = env.root().display().to_string();
    assert!(
        text.contains(&sandbox),
        "config path resolved outside the sandbox, so this suite reads the real \
         host config:\nexpected a path under {sandbox}\ngot {text}"
    );
}
