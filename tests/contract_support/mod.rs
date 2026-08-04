//! Shared harness for the JSON-contract suites (GAP-SG-208).
//!
//! These helpers were declared inside `doc_contract_integration.rs` while that
//! file carried 1393 lines and 41 tests, well past the 800-line ceiling the
//! project sets for itself. Splitting the suite by command family meant the
//! harness had to live somewhere every part could reach, so it moved here
//! rather than being copied four times.

#![allow(dead_code)]

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

#[path = "../common/mod.rs"]
pub mod common;

/// Builds a fresh `Command` with the mock LLM PATH prepended.
///
/// The bundled mocks under `tests/mock-llm/` return a fixed zero vector so the
/// binary finishes without a real OAuth login. The mock directory is leaked
/// (no TempDir cleanup) so the spawned subprocess always finds the mocks.
pub fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}
pub struct Env {
    pub tmp: TempDir,
}

impl Env {
    pub fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        Self { tmp }
    }

    pub fn cmd(&self) -> Command {
        // GAP-SG-101: product env is not read (G-T-XDG-04).
        let mut c = sgr_cmd();
        common::wire_assert_cmd(&self.tmp, &mut c, "test.sqlite");
        c.arg("--skip-memory-guard");
        c
    }

    pub fn init(&self) {
        self.cmd().arg("init").assert().success();
    }

    pub fn remember(&self, name: &str, body: &str) -> Value {
        let out = self
            .cmd()
            .args([
                "remember",
                "--name",
                name,
                "--type",
                "project",
                "--description",
                "desc-contrato",
                "--namespace",
                "global",
                "--body",
                body,
            ])
            .output()
            .unwrap();
        assert!(out.status.success(), "remember failed: {:?}", out.status);
        serde_json::from_slice(&out.stdout).unwrap()
    }

    pub fn remember_with_entities(&self, name: &str, body: &str) -> (String, String) {
        let ent_a = format!("Ent{}A", name.replace('-', ""));
        let ent_b = format!("Ent{}B", name.replace('-', ""));
        let ents_path = self.tmp.path().join(format!("{name}_ents.json"));
        let ents_json = format!(
            r#"[{{"name":"{ent_a}","entity_type":"concept"}},{{"name":"{ent_b}","entity_type":"concept"}}]"#
        );
        std::fs::write(&ents_path, &ents_json).unwrap();
        let out = self
            .cmd()
            .args([
                "remember",
                "--name",
                name,
                "--type",
                "project",
                "--description",
                "desc-entidades",
                "--body",
                body,
                "--entities-file",
                ents_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "remember com entidades failed: {:?}",
            out.status
        );
        (ent_a, ent_b)
    }

    pub fn parse_stdout(out: &std::process::Output) -> Value {
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "JSON inválido: {e}\nstdout: {:?}",
                String::from_utf8_lossy(&out.stdout)
            )
        })
    }
}

/// Checks that all `keys` exist in the JSON object `v`.
pub fn assert_has_keys(cmd: &str, v: &Value, keys: &[&str]) {
    let obj = v
        .as_object()
        .unwrap_or_else(|| panic!("[{cmd}] expected JSON object, got: {v}"));
    for key in keys {
        assert!(
            obj.contains_key(*key),
            "[{cmd}] key ausente: '{key}'. Keys presentes: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
    }
}

/// Checks that all `keys` exist in each item of a JSON array.
pub fn assert_array_items_have_keys(cmd: &str, v: &Value, keys: &[&str]) {
    let arr = v
        .as_array()
        .unwrap_or_else(|| panic!("[{cmd}] expected JSON array, got: {v}"));
    for (i, item) in arr.iter().enumerate() {
        let obj = item
            .as_object()
            .unwrap_or_else(|| panic!("[{cmd}] item[{i}] não é object: {item}"));
        for key in keys {
            assert!(
                obj.contains_key(*key),
                "[{cmd}] item[{i}] key ausente: '{key}'. Keys: {:?}",
                obj.keys().collect::<Vec<_>>()
            );
        }
    }
}
