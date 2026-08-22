#![cfg(feature = "slow-tests")]

//! Suite 11 — tests for the recipes documented in `docs/COOKBOOK.md`: recipes 10 through 15, maintenance and throughput.
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

// Recipe 10 — Purge + vacuum + optimize: full pipeline returns JSON with status ok
#[test]
#[serial]
fn recipe_10_purge_vacuum_optimize() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    // Seed and soft-delete so that purge has data to work on
    cmd(&dir)
        .args([
            "remember",
            "--name",
            "mem-a-purgar",
            "--type",
            "user",
            "--description",
            "será deletada",
            "--body",
            "conteúdo temporário para teste de purge",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    cmd(&dir)
        .args(["forget", "--name", "mem-a-purgar", "--namespace", "global"])
        .assert()
        .success();

    // Purge
    let purge_out = cmd(&dir)
        .args([
            "purge",
            "--retention-days",
            "0",
            "--yes",
            "--namespace",
            "global",
        ])
        .output()
        .unwrap();

    assert!(
        purge_out.status.success(),
        "recipe 10: purge deve ter exit 0"
    );
    let purge_json: serde_json::Value = serde_json::from_slice(&purge_out.stdout)
        .expect("recipe 10: purge deve retornar JSON válido");
    assert!(
        purge_json["elapsed_ms"].is_number(),
        "recipe 10: purge deve ter elapsed_ms"
    );

    // Vacuum
    let vacuum_out = cmd(&dir).arg("vacuum").output().unwrap();
    assert!(
        vacuum_out.status.success(),
        "recipe 10: vacuum deve ter exit 0"
    );
    let vacuum_json: serde_json::Value = serde_json::from_slice(&vacuum_out.stdout)
        .expect("recipe 10: vacuum deve retornar JSON válido");
    assert_eq!(
        vacuum_json["status"], "ok",
        "recipe 10: vacuum.status deve ser ok"
    );

    // Optimize
    let optimize_out = cmd(&dir).arg("optimize").output().unwrap();
    assert!(
        optimize_out.status.success(),
        "recipe 10: optimize deve ter exit 0"
    );
    let optimize_json: serde_json::Value = serde_json::from_slice(&optimize_out.stdout)
        .expect("recipe 10: optimize deve retornar JSON válido");
    assert_eq!(
        optimize_json["status"], "ok",
        "recipe 10: optimize.status deve ser ok"
    );
}

// Recipe 11 — NDJSON export via list: list returns object with items key (not a root array)
// The public COOKBOOK already documents `jaq -c '.items[]'` and this test guards that contract.
#[test]
#[serial]
fn recipe_11_ndjson_list() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    for i in 1..=3u32 {
        cmd(&dir)
            .args([
                "remember",
                "--name",
                &format!("mem-export-{i}"),
                "--type",
                "reference",
                "--description",
                &format!("memória {i} para export"),
                "--body",
                &format!("conteúdo da memória número {i}"),
                "--namespace",
                "global",
            ])
            .assert()
            .success();
    }

    let output = cmd(&dir)
        .args([
            "list",
            "--limit",
            "10000",
            "--format",
            "json",
            "--namespace",
            "global",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "recipe 11: list deve ter exit 0");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("recipe 11: list deve retornar JSON válido");

    // Actual behaviour: an object with an "items" key
    assert!(
        json["items"].is_array(),
        "recipe 11: list retorna objeto com chave 'items' (não array root — drift detectado se mudou)"
    );
    assert!(
        json["elapsed_ms"].is_number(),
        "recipe 11: list deve ter elapsed_ms"
    );

    let items = json["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        3,
        "recipe 11: deve listar 3 memórias inseridas"
    );

    // Every item must carry the expected fields for NDJSON
    let first = &items[0];
    assert!(first["id"].is_number(), "recipe 11: item.id deve existir");
    assert!(
        first["name"].is_string(),
        "recipe 11: item.name deve existir"
    );
    assert!(
        first["namespace"].is_string(),
        "recipe 11: item.namespace deve existir"
    );
}

// Recipe 13 — GNU parallel simulated with threads: parallel recall across 4 namespaces
#[test]
#[serial]
fn recipe_13_parallel_namespaces() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    let namespaces = ["project-a", "project-b", "project-c", "project-d"];

    // Seed one memory in each namespace
    for ns in &namespaces {
        cmd(&dir)
            .args([
                "remember",
                "--name",
                &format!("mem-{ns}"),
                "--type",
                "project",
                "--description",
                &format!("memória do {ns}"),
                "--body",
                &format!("taxa de erro elevada em {ns} detectada"),
                "--namespace",
                ns,
            ])
            .assert()
            .success();
    }

    let db_path = dir.path().join("ng.sqlite").to_owned();
    let root = dir.path().to_owned();
    let bin_path = bin();
    let mock_path = common::prepend_path(&common::mock_llm_path());

    // Simulate `parallel -j 4` with 4 simultaneous threads
    let handles: Vec<_> = namespaces
        .iter()
        .map(|ns| {
            let ns = ns.to_string();
            let db = db_path.clone();
            let root = root.clone();
            let bin = bin_path.clone();
            let path = mock_path.clone();
            std::thread::spawn(move || {
                let mut c = std::process::Command::new(&bin);
                c.env_clear().env("PATH", &path).arg("--skip-memory-guard");
                common::wire_std_cmd(&root, &mut c, &db);
                c.args(["recall", "--db"])
                    .arg(&db)
                    .args(["error rate", "--k", "5", "--namespace", &ns])
                    .output()
                    .expect("recall em thread deve executar sem panic")
            })
        })
        .collect();

    let outputs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    for (i, output) in outputs.iter().enumerate() {
        assert!(
            output.status.success(),
            "recipe 13: recall no namespace {} deve ter exit 0",
            namespaces[i]
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .expect("recipe 13: recall deve retornar JSON válido");
        assert!(
            json["results"].is_array(),
            "recipe 13: recall.results deve ser array no namespace {}",
            namespaces[i]
        );
        let results = json["results"].as_array().unwrap();
        assert!(
            !results.is_empty(),
            "recipe 13: recall deve encontrar memória no namespace {} com query 'error rate'",
            namespaces[i]
        );
    }
}

// Recipe 14 — Debug slow queries: health + stats + --json return the documented fields
#[test]
#[serial]
fn recipe_14_debug_health_stats() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    // Health: fields documented in the COOKBOOK
    let health_out = cmd(&dir).args(["health", "--json"]).output().unwrap();

    assert!(
        health_out.status.success(),
        "recipe 14: health deve ter exit 0"
    );

    let health: serde_json::Value = serde_json::from_slice(&health_out.stdout)
        .expect("recipe 14: health deve retornar JSON válido");

    // Validates the documented fields: `integrity, wal_size_mb, journal_mode`
    assert!(
        health.get("integrity").is_some(),
        "recipe 14: health deve ter campo 'integrity' como documentado"
    );
    assert!(
        health.get("wal_size_mb").is_some(),
        "recipe 14: health deve ter campo 'wal_size_mb' como documentado"
    );
    assert!(
        health.get("journal_mode").is_some(),
        "recipe 14: health deve ter campo 'journal_mode' como documentado"
    );

    // Stats: fields documented in the COOKBOOK
    let stats_out = cmd(&dir).args(["stats", "--json"]).output().unwrap();

    assert!(
        stats_out.status.success(),
        "recipe 14: stats deve ter exit 0"
    );

    let stats: serde_json::Value = serde_json::from_slice(&stats_out.stdout)
        .expect("recipe 14: stats deve retornar JSON válido");

    // Validates the documented fields: `memories, memories_total, entities, entities_total,
    // relationships, relationships_total, edges, chunks_total, avg_body_len,
    // db_size_bytes, db_bytes`
    let expected_fields = [
        "memories",
        "memories_total",
        "entities",
        "entities_total",
        "relationships",
        "relationships_total",
        "edges",
        "chunks_total",
        "avg_body_len",
        "db_size_bytes",
        "db_bytes",
    ];

    for field in &expected_fields {
        assert!(
            stats.get(field).is_some(),
            "recipe 14: stats deve ter campo '{field}' como documentado no COOKBOOK"
        );
    }
}

// Recipe 15 — Simulated benchmark: recall and hybrid-search execute in reasonable time
// Simulates `hyperfine` by checking that both commands complete without a timeout
#[test]
#[serial]
fn recipe_15_hyperfine_timing() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    // Seed with memory for non-trivial search
    cmd(&dir)
        .args([
            "remember",
            "--name",
            "pg-migration",
            "--type",
            "incident",
            "--description",
            "postgres migration benchmark",
            "--body",
            "migração postgres com deadlock em ambiente de produção durante janela de manutenção",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    // Measure recall
    let t0 = std::time::Instant::now();
    let recall_out = cmd(&dir)
        .args([
            "recall",
            "postgres migration",
            "--k",
            "10",
            "--namespace",
            "global",
        ])
        .output()
        .unwrap();
    let recall_elapsed = t0.elapsed();

    assert!(
        recall_out.status.success(),
        "recipe 15: recall deve ter exit 0"
    );
    assert!(
        recall_elapsed.as_secs() < 30,
        "recipe 15: recall deve completar em menos de 30s, levou {recall_elapsed:?}"
    );

    // Measure hybrid-search
    let t1 = std::time::Instant::now();
    let hybrid_out = cmd(&dir)
        .args([
            "hybrid-search",
            "postgres migration",
            "--k",
            "10",
            "--namespace",
            "global",
        ])
        .output()
        .unwrap();
    let hybrid_elapsed = t1.elapsed();

    assert!(
        hybrid_out.status.success(),
        "recipe 15: hybrid-search deve ter exit 0"
    );
    assert!(
        hybrid_elapsed.as_secs() < 30,
        "recipe 15: hybrid-search deve completar em menos de 30s, levou {hybrid_elapsed:?}"
    );

    // Both return valid JSON results with elapsed_ms
    let recall_json: serde_json::Value = serde_json::from_slice(&recall_out.stdout).unwrap();
    let hybrid_json: serde_json::Value = serde_json::from_slice(&hybrid_out.stdout).unwrap();

    assert!(
        recall_json["elapsed_ms"].is_number(),
        "recipe 15: recall deve reportar elapsed_ms no JSON"
    );
    assert!(
        hybrid_json["elapsed_ms"].is_number(),
        "recipe 15: hybrid-search deve reportar elapsed_ms no JSON"
    );
}
