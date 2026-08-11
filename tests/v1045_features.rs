//! Integration tests for v1.0.45 features:
//! - A1: FTS5 query preprocessing handles compound terms with separators
//!
//! The S5 section covered `--enable-ner` and the `SQLITE_GRAPHRAG_ENABLE_NER`
//! product env. Both tests asserted that `--enable-ner --skip-extraction`
//! SUCCEEDS, which the CLI rejects as mutually exclusive, so they could only
//! ever have passed against a v1.0.45 binary; they were `#[ignore]`d through
//! five minor versions after v1.0.76 deleted NER (ADR-0025). Removed rather
//! than repaired: the extraction pipeline they drove no longer exists, and the
//! product env they were named for is forbidden.

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

/// Builds a fresh `Command` with the mock LLM PATH prepended.
///
/// v1.0.76 spawns `claude` or `codex` on every `remember` / `ingest` /
/// `edit`. The bundled mocks under `tests/mock-llm/` return a fixed
/// 64-dim zero vector so the binary finishes without a real OAuth
/// login. The mock directory is leaked (no TempDir cleanup) so the
/// spawned subprocess always finds the mocks.
fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

#[path = "common/mod.rs"]
mod common;

fn cmd(temp: &TempDir) -> Command {
    let cache = temp.path().join("cache");
    let mut c = sgr_cmd();
    let mock_dir = common::mock_llm_path();
    c.env_clear()
        .env("HOME", temp.path())
        .env("HOME", temp.path())
        .env("XDG_CACHE_HOME", &cache)
        .arg("--lang")
        .arg("en")
        // GAP-SG-207: this sandbox isolates HOME precisely so the XDG default
        // IS the intended target, which is a deliberate choice rather than an
        // inherited one. Declaring it keeps the mutating verbs below running
        // without pinning `--db` on every single invocation.
        .arg("--use-active")
        .current_dir(temp.path());
    for var in &["LOCALAPPDATA", "APPDATA", "USERPROFILE", "SystemRoot"] {
        if let Ok(v) = std::env::var(var) {
            c.env(var, v);
        }
    }
    c.env("PATH", common::prepend_path(&mock_dir));
    // Offline OpenRouter stub: `env_clear` leaves HOME as the only config
    // channel, so the sandbox config lands under $HOME/.config.
    common::write_sandbox_config(&temp.path().join(".config").join("sqlite-graphrag"), None);
    c.arg("--embedding-model")
        .arg(common::openrouter_mock::STUB_MODEL);
    c
}

fn init_db(tmp: &TempDir) {
    cmd(tmp).arg("init").assert().success();
}

fn remember_with_body(tmp: &TempDir, name: &str, body: &str) {
    cmd(tmp)
        .args([
            "remember",
            "--name",
            name,
            "--type",
            "note",
            "--description",
            "test memory",
            "--body",
            body,
            "--skip-extraction",
        ])
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// A1: FTS5 compound term search via hybrid-search
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn hybrid_search_finds_hyphenated_compound_term() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);
    remember_with_body(
        &tmp,
        "fts-hyphen-test",
        "the graphrag-precompact script runs daily",
    );

    let output = cmd(&tmp)
        .args(["hybrid-search", "graphrag-precompact", "--k", "5"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let names: Vec<&str> = json["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"fts-hyphen-test"),
        "should find memory by hyphenated term; got {names:?}"
    );
}

#[test]
#[serial]
fn hybrid_search_finds_dotted_version() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);
    remember_with_body(
        &tmp,
        "fts-dot-test",
        "release notes for v1.0.44 are published",
    );

    let output = cmd(&tmp)
        .args(["hybrid-search", "v1.0.44", "--k", "5"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let names: Vec<&str> = json["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"fts-dot-test"),
        "should find memory by dotted version; got {names:?}"
    );
}
