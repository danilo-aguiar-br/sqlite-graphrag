//! Shared harness for the installed-binary smoke suites (GAP-SG-210).
//!
//! These helpers lived inside `installed_binary_smoke.rs` while that file
//! carried 981 lines and 26 tests, past the 800-line ceiling the project sets
//! for itself. Splitting the suite by command family meant the harness had to
//! reach every part, so it moved here rather than being copied three times.
//!
//! Everything here targets `~/.cargo/bin/sqlite-graphrag` — the INSTALLED
//! binary, not the one `cargo test` just built. That is the whole point of the
//! suite: it catches the case where local code is green and what the operator
//! actually runs is stale.
//!
//! API contracts validated by the suites that use this harness:
//! - `init`     → {status: "ok", db_path, schema_version, ...}
//! - `remember` → {memory_id, name, action: "created", ...}   (no `status`)
//! - `forget`   → {forgotten: true, name, namespace}          (no `status`)
//! - `rename`   → {memory_id, name, version}                  (no `status`)
//! - `edit`     → {memory_id, name, action: "updated", ...}   (no `status`)
//! - `list`     → {items:[...], elapsed_ms}                   (not a root array)
//! - `link`     → {action: "created", from, to, relation, ...}
//! - `unlink`   → {action: "deleted", relationship_id, ...}

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;

#[path = "../common/mod.rs"]
pub mod common;

/// Path of the installed binary, or `None` when it was never installed.
pub fn installed_bin() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    let p = PathBuf::from(home).join(".cargo/bin/sqlite-graphrag");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

/// Resolves the installed binary or exits the whole test binary with 0.
///
/// Skipping rather than failing is deliberate: a machine that never ran
/// `cargo install` has nothing to smoke-test, and turning that into a red
/// suite would train everyone to ignore this file.
pub fn skip_if_not_installed() -> PathBuf {
    // Harness-only opt-out — NOT product config (G-T-XDG-04).
    if std::env::var("SGR_TEST_SKIP_INSTALLED_SMOKE").as_deref() == Ok("1") {
        eprintln!("Suite 10: skipped via SGR_TEST_SKIP_INSTALLED_SMOKE=1");
        std::process::exit(0);
    }
    match installed_bin() {
        Some(p) => p,
        None => {
            eprintln!("Suite 10: sqlite-graphrag não encontrado em ~/.cargo/bin — skipping");
            std::process::exit(0);
        }
    }
}

/// Returns the installed binary version as a string, e.g. "1.2.3".
pub fn installed_version(bin: &PathBuf) -> String {
    let out = Command::new(bin)
        .arg("--version")
        .output()
        .expect("--version failed");
    let s = String::from_utf8_lossy(&out.stdout);
    // formato: "sqlite-graphrag 1.2.3\n"
    s.split_whitespace().nth(1).unwrap_or("0.0.0").to_string()
}

pub fn expected_installed_version() -> String {
    // Harness-only — NOT product config.
    std::env::var("SGR_TEST_EXPECT_INSTALLED_VERSION")
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
}

pub fn allow_installed_version_mismatch() -> bool {
    std::env::var("SGR_TEST_ALLOW_INSTALLED_VERSION_MISMATCH").as_deref() == Ok("1")
}

/// Fails unless the installed binary matches this workspace's version.
///
/// Without this the suite reports green against a binary from three releases
/// ago, which is the exact false positive it exists to prevent.
pub fn assert_expected_installed_version(bin: &PathBuf) {
    let actual = installed_version(bin);
    let expected = expected_installed_version();
    if actual == expected {
        return;
    }

    if allow_installed_version_mismatch() {
        eprintln!(
            "Suite 10: version mismatch allowed explicitly: installed v{actual}, expected v{expected}"
        );
        return;
    }

    panic!(
        "Suite 10: installed binary version mismatch: ~/.cargo/bin/sqlite-graphrag is v{actual}, but this workspace expects v{expected}. Reinstall with `cargo install sqlite-graphrag --version {expected} --locked --force` or set SGR_TEST_ALLOW_INSTALLED_VERSION_MISMATCH=1 for deliberate legacy audits."
    );
}

/// One isolated sandbox plus the resolved installed binary.
pub struct Env {
    pub bin: PathBuf,
    pub tmp: TempDir,
}

impl Env {
    pub fn new() -> Self {
        let bin = skip_if_not_installed();
        assert_expected_installed_version(&bin);
        let tmp = TempDir::new().expect("TempDir failed");
        Self { bin, tmp }
    }

    pub fn cmd(&self) -> Command {
        // GAP-SG-101: product env is not read (G-T-XDG-04).
        let mock_dir = common::mock_llm_path();
        let mut c = Command::new(&self.bin);
        let db = self.tmp.path().join("smoke.sqlite");
        c.env("PATH", common::prepend_path(&mock_dir));
        common::wire_std_cmd(self.tmp.path(), &mut c, &db);
        c.arg("--skip-memory-guard");
        c
    }

    pub fn cmd_default_db_in_tmp_dir(&self) -> Command {
        let mock_dir = common::mock_llm_path();
        let mut c = Command::new(&self.bin);
        c.current_dir(self.tmp.path());
        c.env("HOME", self.tmp.path().join("home"));
        c.env("XDG_CACHE_HOME", self.tmp.path().join("cache"));
        c.env("XDG_CONFIG_HOME", self.tmp.path().join("config_home"));
        c.env("XDG_DATA_HOME", self.tmp.path().join("data"));
        c.env("PATH", common::prepend_path(&mock_dir));
        c.arg("--config-dir")
            .arg(self.tmp.path().join("config_default"));
        c.arg("--cache-dir").arg(self.tmp.path().join("cache"));
        c.arg("--skip-memory-guard");
        // This constructor bypasses `wire_std_cmd`, so the offline stub has to
        // be wired here too or the child reaches the real api.openrouter.ai.
        common::write_sandbox_config(&self.tmp.path().join("config_default"), None);
        c.arg("--embedding-model")
            .arg(common::openrouter_mock::STUB_MODEL);
        c
    }

    pub fn init(&self) {
        let db = self.tmp.path().join("smoke.sqlite");
        let out = self
            .cmd()
            .args(["init", "--db"])
            .arg(&db)
            .output()
            .expect("init failed");
        assert!(out.status.success(), "init failed: {}", stderr(&out));
    }

    pub fn remember(&self, name: &str, body: &str) {
        let out = self
            .cmd()
            .args([
                "remember",
                "--name",
                name,
                "--type",
                "project",
                "--description",
                "smoke test",
                "--body",
                body,
            ])
            .output()
            .expect("remember failed");
        assert!(
            out.status.success(),
            "remember {name} failed: {}",
            stderr(&out)
        );
    }

    /// Creates a memory with two entities in the graph and returns the entity names.
    /// entities-file requires `entity_type` field (not `kind`).
    pub fn remember_with_entities(&self, name: &str, body: &str) -> (String, String) {
        let ent_a = format!("Ent{name}A");
        let ent_b = format!("Ent{name}B");
        let ents_path = self.tmp.path().join(format!("{name}_ents.json"));
        let ents_json = format!(
            r#"[{{"name":"{ent_a}","entity_type":"concept"}},{{"name":"{ent_b}","entity_type":"concept"}}]"#
        );
        std::fs::write(&ents_path, ents_json).expect("escrita entities-file failed");

        let out = self
            .cmd()
            .args([
                "remember",
                "--name",
                name,
                "--type",
                "project",
                "--description",
                "smoke test com entidades",
                "--body",
                body,
                "--entities-file",
                ents_path.to_str().unwrap(),
            ])
            .output()
            .expect("remember com entities failed");
        assert!(
            out.status.success(),
            "remember {name} com entities failed: {}",
            stderr(&out)
        );
        (ent_a, ent_b)
    }
}

pub fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

pub fn assert_json_stdout(out: &Output) {
    assert!(
        out.status.success(),
        "exit code {:?}: {}",
        out.status.code(),
        stderr(out)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
    assert!(parsed.is_ok(), "stdout não é JSON válido: {stdout}");
}

/// Acceptable for commands that may return 0 or 4 (not found).
pub fn assert_json_or_not_found(out: &Output) {
    let code = out.status.code().unwrap_or(1);
    assert!(
        code == 0 || code == 4,
        "exit code inesperado {code}: {}",
        stderr(out)
    );
    if code == 0 {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
        assert!(parsed.is_ok(), "stdout não é JSON válido: {stdout}");
    }
}
