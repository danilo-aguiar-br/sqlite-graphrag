//! Shared harness for the strict JSON-Schema contract suites (GAP-SG-208).
//!
//! Extracted from `schema_contract_strict.rs`, which carried 1633 lines and 44
//! tests — twice the 800-line ceiling the project sets for itself. The suite
//! was split by command family; this is the part every split file needs.

#![allow(dead_code)]
#![cfg(feature = "slow-tests")]

// Each test runs the binary, captures stdout, parses it as JSON and validates against
// docs/schemas/<cmd>.schema.json using the jsonschema::Validator crate.
//
// Dependency: jsonschema = "0.29" in [dev-dependencies] of Cargo.toml.
use assert_cmd::Command;
use serde_json::Value;

use tempfile::TempDir;

/// Builds a fresh `Command` with the mock LLM PATH prepended.
///
/// v1.0.76 spawns `claude` or `codex` on every `remember` / `ingest` /
/// `edit`. The bundled mocks under `tests/mock-llm/` return a fixed
/// 64-dim zero vector so the binary finishes without a real OAuth
/// login. The mock directory is leaked (no TempDir cleanup) so the
/// spawned subprocess always finds the mocks.
pub fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

#[path = "../common/mod.rs"]
pub mod common;

// ---------------------------------------------------------------------------
// Infraestrutura de teste
// ---------------------------------------------------------------------------

pub struct Env {
    pub tmp: TempDir,
}

impl Env {
    pub fn new() -> Self {
        let tmp = TempDir::new().expect("TempDir::new failed");
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

    pub fn remember_simples(&self, nome: &str) -> Value {
        let saida = self
            .cmd()
            .args([
                "remember",
                "--name",
                nome,
                "--type",
                "project",
                "--description",
                "descricao-contrato",
                "--namespace",
                "global",
                "--body",
                "corpo-de-teste-schema-contract",
            ])
            .output()
            .expect("remember failed ao executar");
        assert!(
            saida.status.success(),
            "remember retornou erro: {:?}\nstdout: {}",
            saida.status.code(),
            String::from_utf8_lossy(&saida.stdout)
        );
        serde_json::from_slice(&saida.stdout).expect("remember stdout não é JSON válido")
    }

    pub fn remember_with_entities(&self, name: &str) -> (String, String) {
        let ent_a = format!("Ent{}Alpha", name.replace('-', ""));
        let ent_b = format!("Ent{}Beta", name.replace('-', ""));
        let caminho_ents = self.tmp.path().join(format!("{name}_ents.json"));
        let json_ents = format!(
            r#"[{{"name":"{ent_a}","entity_type":"concept"}},{{"name":"{ent_b}","entity_type":"concept"}}]"#
        );
        std::fs::write(&caminho_ents, &json_ents).expect("escrita de entidades failed");
        let saida = self
            .cmd()
            .args([
                // The graph fixture only needs the entities persisted; embedding
                // them would require a live OpenRouter key and abort with exit 11.
                "--llm-backend",
                "none",
                "remember",
                "--name",
                name,
                "--type",
                "project",
                "--description",
                "descricao-entidades",
                "--body",
                "corpo-com-entidades-para-schema",
                "--entities-file",
                caminho_ents.to_str().expect("caminho inválido"),
            ])
            .output()
            .expect("remember com entidades failed");
        assert!(
            saida.status.success(),
            "remember com entidades retornou erro: {:?}",
            saida.status.code()
        );
        (ent_a, ent_b)
    }

    pub fn parse_stdout(saida: &std::process::Output, cmd: &str) -> Value {
        serde_json::from_slice(&saida.stdout).unwrap_or_else(|e| {
            panic!(
                "[{cmd}] stdout não é JSON válido: {e}\nstdout bruto: {:?}",
                String::from_utf8_lossy(&saida.stdout)
            )
        })
    }
}

/// GAP-SG-142: shared `agent_surface` definitions, referenced by `$ref` from
/// every response schema whose root sets `additionalProperties: false`.
pub const AGENT_SURFACE_SCHEMA: &str = include_str!("../../docs/schemas/agent-surface.schema.json");

/// Canonical `$id` of [`AGENT_SURFACE_SCHEMA`], used as the retrieval URI.
const AGENT_SURFACE_URI: &str =
    "https://github.com/danilo-aguiar-br/sqlite-graphrag/schemas/agent-surface.schema.json";

/// Validates `instancia` against the schema in `schema_str`.
/// Collects all errors and aborts with a detailed message if any violations exist.
///
/// The shared agent-surface document is registered as an in-memory resource so
/// the `$ref` into it resolves offline; no network retrieval ever happens.
pub fn validar_schema(cmd: &str, schema_str: &str, instancia: &Value) {
    let schema: Value =
        serde_json::from_str(schema_str).unwrap_or_else(|e| panic!("[{cmd}] schema inválido: {e}"));
    let compartilhado: Value = serde_json::from_str(AGENT_SURFACE_SCHEMA)
        .unwrap_or_else(|e| panic!("[{cmd}] agent-surface.schema.json inválido: {e}"));
    let recurso = jsonschema::Resource::from_contents(compartilhado)
        .unwrap_or_else(|e| panic!("[{cmd}] agent-surface não é um recurso válido: {e}"));
    let validador = jsonschema::options()
        .with_resource(AGENT_SURFACE_URI, recurso)
        .build(&schema)
        .unwrap_or_else(|e| panic!("[{cmd}] failure ao compilar schema: {e}"));
    let erros: Vec<String> = validador
        .iter_errors(instancia)
        .map(|e| format!("  - caminho={} tipo={:?}", e.instance_path, e.kind))
        .collect();
    assert!(
        erros.is_empty(),
        "[{cmd}] {n} violação(ões) de schema:\n{lista}\ninstância: {inst}",
        n = erros.len(),
        lista = erros.join("\n"),
        inst = serde_json::to_string_pretty(instancia).unwrap_or_default()
    );
}
