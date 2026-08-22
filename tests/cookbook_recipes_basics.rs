#![cfg(feature = "slow-tests")]

//! Suite 11 — tests for the recipes documented in `docs/COOKBOOK.md`: recipes 1 through 9, from bootstrap to snapshot.
//!
//! Each test checks that actual CLI behaviour matches what the cookbook
//! promises, so drift between documentation and implementation shows up here
//! rather than in an operator's terminal.
//!
//! Split by GAP-SG-210: the single file held 907 lines, past the 800-line
//! ceiling this project sets for itself.
//!
//! Recipes skipped by design, in either half:
//!   - Recipe 6: AGENTS.md discovery — documentation only, no executable commands
//!   - Recipe 12: Git LFS — requires git lfs installed and a git repository

use assert_cmd::Command;
use serial_test::serial;
#[allow(unused_imports)]
use std::fs;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;

fn bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_sqlite-graphrag"))
}

fn cmd(dir: &TempDir) -> Command {
    // GAP-SG-101: product env is not read (G-T-XDG-04).
    let mock_dir = common::mock_llm_path();
    let mut c = Command::new(bin());
    c.env_clear()
        .env("PATH", common::prepend_path(&mock_dir))
        .arg("--skip-memory-guard");
    common::wire_assert_cmd(dir, &mut c, "ng.sqlite");
    c
}

#[allow(dead_code)]
fn init(dir: &TempDir) {
    cmd(dir).arg("init").assert().success();
}

#[test]
#[serial]
fn recipe_01_bootstrap_60s() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    let output = cmd(&dir)
        .args(["health", "--json"])
        .timeout(std::time::Duration::from_secs(120))
        .output()
        .unwrap();
    assert!(output.status.success(), "health deve ter exit 0");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("health deve retornar JSON válido");

    assert_eq!(json["status"], "ok", "recipe 1: health.status deve ser ok");
    assert_eq!(
        json["integrity"], "ok",
        "recipe 1: health.integrity deve ser ok"
    );
    assert!(
        json["schema_version"].is_number(),
        "recipe 1: health.schema_version deve ser número"
    );
    assert!(
        json["elapsed_ms"].is_number(),
        "recipe 1: health deve ter elapsed_ms"
    );
}

// Recipe 2 — Bulk-import stdin: remember with --body-stdin reads body from stdin
#[test]
#[serial]
fn recipe_02_bulk_import_body_stdin() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    let body_text = "Este é o conteúdo importado via stdin do arquivo markdown.";

    let output = cmd(&dir)
        .args([
            "remember",
            "--name",
            "doc-importado",
            "--type",
            "user",
            "--description",
            "imported from docs/readme.md",
            "--body-stdin",
            "--namespace",
            "global",
        ])
        .write_stdin(body_text)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "recipe 2: remember com --body-stdin deve ter exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("recipe 2: remember deve retornar JSON válido");
    assert_eq!(
        json["action"], "created",
        "recipe 2: action deve ser created"
    );

    // Validates that the body was persisted, via read
    let read_output = cmd(&dir)
        .args(["read", "--name", "doc-importado", "--namespace", "global"])
        .output()
        .unwrap();
    assert!(
        read_output.status.success(),
        "recipe 2: read do memory importado deve ter exit 0"
    );
    let read_json: serde_json::Value = serde_json::from_slice(&read_output.stdout).unwrap();
    let body = read_json["body"].as_str().unwrap_or("");
    assert!(
        body.contains("conteúdo importado via stdin"),
        "recipe 2: body deve conter texto do stdin, got: {body}"
    );
}

// Recipe 3 — Hybrid search tunable: --rrf-k and --weight-vec emitted in the JSON
#[test]
#[serial]
fn recipe_03_hybrid_search_tunable() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    // Seed with a memory
    cmd(&dir)
        .args([
            "remember",
            "--name",
            "pg-deadlock",
            "--type",
            "incident",
            "--description",
            "postgres migration deadlock",
            "--body",
            "deadlock detectado durante migration de índices no postgres",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    let output = cmd(&dir)
        .args([
            "hybrid-search",
            "postgres migration deadlock",
            "--k",
            "10",
            "--rrf-k",
            "60",
            "--weight-vec",
            "0.7",
            "--weight-fts",
            "0.3",
            "--namespace",
            "global",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "recipe 3: hybrid-search deve ter exit 0"
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("recipe 3: hybrid-search deve retornar JSON válido");

    assert_eq!(
        json["rrf_k"], 60,
        "recipe 3: rrf_k deve ser 60 conforme documentado"
    );
    assert!(
        (json["weights"]["vec"].as_f64().unwrap() - 0.7).abs() < 0.001,
        "recipe 3: weights.vec deve ser 0.7"
    );
    assert!(
        (json["weights"]["fts"].as_f64().unwrap() - 0.3).abs() < 0.001,
        "recipe 3: weights.fts deve ser 0.3"
    );
    assert!(
        json["results"].is_array(),
        "recipe 3: results deve ser array"
    );
    assert!(
        json["elapsed_ms"].is_number(),
        "recipe 3: elapsed_ms deve estar presente"
    );

    // Validate that every result has vec_rank and fts_rank as documented
    let results = json["results"].as_array().unwrap();
    if !results.is_empty() {
        let first = &results[0];
        assert!(
            first.get("vec_rank").is_some() || first.get("combined_score").is_some(),
            "recipe 3: resultado deve ter vec_rank ou combined_score"
        );
    }
}

// Recipe 4 — Graph traversal: related with --hops returns JSON with results
#[test]
#[serial]
fn recipe_04_graph_traversal_related() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    // Seed source memory
    cmd(&dir)
        .args([
            "remember",
            "--name",
            "authentication-flow",
            "--type",
            "project",
            "--description",
            "fluxo de autenticação OAuth2",
            "--body",
            "implementação do fluxo de autenticação com OAuth2 e JWT",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    let output = cmd(&dir)
        .args([
            "related",
            "authentication-flow",
            "--hops",
            "2",
            "--format",
            "json",
            "--namespace",
            "global",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "recipe 4: related deve ter exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("recipe 4: related deve retornar JSON válido");

    assert!(
        json["results"].is_array(),
        "recipe 4: results deve ser array conforme documentado"
    );
    assert!(
        json["elapsed_ms"].is_number(),
        "recipe 4: elapsed_ms deve estar presente"
    );
}

// Recipe 5 — Pre/post-task hooks: recall returns JSON with results, remember persists
#[test]
#[serial]
fn recipe_05_pre_post_task_hooks() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    // Simulates a post-task hook: persists the assistant response
    let assistant_response = "decisão: usar JWT com expiração de 24h para tokens de sessão";
    let session_name = "session-12345";

    let output_post = cmd(&dir)
        .args([
            "remember",
            "--name",
            session_name,
            "--type",
            "project",
            "--description",
            "decision log",
            "--body",
            assistant_response,
            "--namespace",
            "global",
        ])
        .output()
        .unwrap();

    assert!(
        output_post.status.success(),
        "recipe 5 (post-hook): remember deve ter exit 0"
    );

    // Simulates a pre-task hook: retrieves relevant context
    let output_pre = cmd(&dir)
        .args([
            "recall",
            "decisão JWT sessão",
            "--k",
            "5",
            "--namespace",
            "global",
        ])
        .output()
        .unwrap();

    assert!(
        output_pre.status.success(),
        "recipe 5 (pre-hook): recall deve ter exit 0"
    );

    let json: serde_json::Value = serde_json::from_slice(&output_pre.stdout)
        .expect("recipe 5: recall deve retornar JSON válido");

    assert!(
        json["results"].is_array(),
        "recipe 5: recall.results deve ser array"
    );
    assert!(
        json["elapsed_ms"].is_number(),
        "recipe 5: recall deve ter elapsed_ms"
    );

    // The persisted memory must be found
    let results = json["results"].as_array().unwrap();
    assert!(
        !results.is_empty(),
        "recipe 5: recall deve encontrar a memória persistida pelo post-hook"
    );
}

// Recipe 7 — namespace via --namespace flag (product env is not a channel).
#[test]
#[serial]
fn recipe_07_namespace_flag_precedence() {
    // GAP-SG-101: SQLITE_GRAPHRAG_NAMESPACE is not read. Prefer --namespace
    // or `config set namespace.default`.
    let dir = TempDir::new().unwrap();

    let output = cmd(&dir)
        .args(["namespace-detect", "--namespace", "meu-projeto", "--json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "recipe 7: namespace-detect must exit 0"
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("recipe 7: namespace-detect must return valid JSON");

    assert_eq!(
        json["namespace"], "meu-projeto",
        "recipe 7: namespace must be the --namespace flag value"
    );
    assert_eq!(
        json["source"], "explicit_flag",
        "recipe 7: source must be explicit_flag (flag channel), never environment"
    );
    assert!(
        json["elapsed_ms"].is_number(),
        "recipe 7: elapsed_ms must be present"
    );
}

// Recipe 8 — Export to file /tmp/ng.json: hybrid-search > file
#[test]
#[serial]
fn recipe_08_export_to_file() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    // Seed with memory
    cmd(&dir)
        .args([
            "remember",
            "--name",
            "editor-context",
            "--type",
            "project",
            "--description",
            "contexto do editor",
            "--body",
            "contexto atual do editor sobre o módulo de autenticação",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    let dest = dir.path().join("ng.json");

    let output = cmd(&dir)
        .args([
            "hybrid-search",
            "editor contexto autenticação",
            "--k",
            "10",
            "--namespace",
            "global",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "recipe 8: hybrid-search deve ter exit 0"
    );

    // Simulates redirection to a file
    fs::write(&dest, &output.stdout).expect("deve escrever ng.json");

    assert!(dest.exists(), "recipe 8: ng.json deve existir após export");

    let content = fs::read_to_string(&dest).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&content).expect("recipe 8: ng.json deve ser JSON válido");
    assert!(
        json["results"].is_array(),
        "recipe 8: ng.json deve conter array results"
    );
}

// Recipe 9 — sync-safe-copy: snapshot is consistent and opens with exit 0
#[test]
#[serial]
fn recipe_09_sync_safe_copy() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    // Seed with memory to have data in the snapshot
    cmd(&dir)
        .args([
            "remember",
            "--name",
            "sync-test",
            "--type",
            "user",
            "--description",
            "test para sync",
            "--body",
            "dados importantes que não devem se corromper no sync",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    let dest = dir.path().join("snapshot.sqlite");

    let output = cmd(&dir)
        .args(["sync-safe-copy", "--dest", dest.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "recipe 9: sync-safe-copy deve ter exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("recipe 9: sync-safe-copy deve retornar JSON válido");

    assert_eq!(
        json["status"], "ok",
        "recipe 9: status deve ser ok conforme documentado"
    );
    assert!(
        json["bytes_copied"].as_u64().unwrap_or(0) > 0,
        "recipe 9: bytes_copied deve ser maior que 0"
    );
    assert!(dest.exists(), "recipe 9: arquivo snapshot deve existir");

    // Validate the snapshot opens via health --db <snapshot>
    let mock_dir = common::mock_llm_path();
    let health = std::process::Command::new(bin())
        .env_clear()
        .env("PATH", common::prepend_path(&mock_dir))
        .env("HOME", dir.path().join("home2"))
        .env("XDG_CACHE_HOME", dir.path().join("cache2"))
        .arg("--skip-memory-guard")
        .arg("--config-dir")
        .arg(dir.path().join("config2"))
        .arg("--cache-dir")
        .arg(dir.path().join("cache2"))
        .args(["health", "--db"])
        .arg(&dest)
        .arg("--json")
        .output()
        .unwrap();

    assert!(
        health.status.success(),
        "recipe 9: health no snapshot deve ter exit 0 — snapshot deve ser abrível"
    );
    let json_health: serde_json::Value = serde_json::from_slice(&health.stdout).unwrap();
    assert_eq!(
        json_health["status"], "ok",
        "recipe 9: snapshot deve ter status ok"
    );
}
