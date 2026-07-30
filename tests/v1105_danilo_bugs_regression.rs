//! v1.1.05 regression suite for the "danilo" deep-research incident report.
//!
//! Covers Bugs 1–5 at the CLI boundary (assert_cmd):
//! 1. single-token deep-research fan-out
//! 2. --output atomic JSON write + quiet stderr contract
//! 3. graph traverse fuzzy / suggestions for short names
//! 4. merge-entities self-referential rejection
//! 5. link --from-id/--to-id and rejection of pure-numeric names

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;

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
    c
}

fn init_db(tmp: &TempDir) {
    cmd_base(tmp).arg("init").assert().success();
}

/// Bug 1: `deep-research "danilo"` must emit more than one sub-query (aspect fan-out).
#[test]
fn bug1_single_token_deep_research_fans_out_sub_queries() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let out = cmd_base(&tmp)
        .args(["deep-research", "danilo", "--max-sub-queries", "7", "-q"])
        .output()
        .expect("run deep-research");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: Value = serde_json::from_slice(&out.stdout).expect("valid JSON stdout");
    let subs = json["sub_queries"]
        .as_array()
        .expect("sub_queries array present");
    assert!(
        subs.len() > 1,
        "expected aspect fan-out for single token, got {subs:?}"
    );
    assert_eq!(subs[0]["text"], "danilo");
    assert_eq!(subs[0]["source"], "original");
    assert!(
        subs.iter().skip(1).any(|s| s["source"] == "aspect"),
        "expected source=aspect entries: {subs:?}"
    );
}

/// Bug 2: `--output` writes a full envelope atomically; stdout is a small ack.
#[test]
fn bug2_output_writes_atomic_json_with_blake3_ack() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);
    let out_path = tmp.path().join("dr.json");

    let out = cmd_base(&tmp)
        .args([
            "deep-research",
            "danilo",
            "--max-sub-queries",
            "3",
            "--output",
            out_path.to_str().unwrap(),
            "-q",
        ])
        .output()
        .expect("run deep-research --output");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let ack: Value = serde_json::from_slice(&out.stdout).expect("ack JSON");
    assert!(ack["written"].as_str().unwrap().contains("dr.json"));
    assert!(ack["bytes"].as_u64().unwrap() > 0);
    assert!(ack["blake3"].as_str().unwrap().len() == 64);

    let file_raw = fs::read_to_string(&out_path).expect("read atomic output");
    let envelope: Value = serde_json::from_str(&file_raw).expect("file must be valid JSON");
    assert!(
        envelope["sub_queries"].as_array().unwrap().len() > 1,
        "envelope must contain fan-out sub_queries"
    );
    // Integrity: recompute blake3 of on-disk bytes.
    let file_bytes = fs::read(&out_path).unwrap();
    let expected = blake3::hash(&file_bytes).to_hex().to_string();
    assert_eq!(ack["blake3"].as_str().unwrap(), expected);
}

/// Bug 3: short name without --fuzzy → exit 4 with suggestions; with --fuzzy resolves.
#[test]
fn bug3_traverse_short_name_suggests_and_fuzzy_resolves() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // Seed a memory that will create a long canonical entity via graph-stdin.
    let graph = r#"{
      "body":"Danilo Aguiar Teixeira works on GraphRAG.",
      "entities":[{"name":"danilo-aguiar-teixeira","type":"person"}],
      "relationships":[]
    }"#;
    cmd_base(&tmp)
        .args([
            "remember",
            "--name",
            "seed-danilo",
            "--type",
            "user",
            "--description",
            "seed",
            "--namespace",
            "audit",
            "--graph-stdin",
        ])
        .write_stdin(graph)
        .assert()
        .success();

    // Without --fuzzy: exit 4 and suggestion text.
    let fail = cmd_base(&tmp)
        .args([
            "graph",
            "traverse",
            "--from",
            "danilo",
            "--depth",
            "1",
            "--namespace",
            "audit",
            "-q",
        ])
        .output()
        .unwrap();
    assert_eq!(fail.status.code(), Some(4));
    let err_text = format!(
        "{}{}",
        String::from_utf8_lossy(&fail.stdout),
        String::from_utf8_lossy(&fail.stderr)
    );
    assert!(
        err_text.contains("danilo-aguiar-teixeira") || err_text.contains("Did you mean"),
        "expected fuzzy suggestions in error, got: {err_text}"
    );

    // With --fuzzy: exit 0 and from resolved to canonical name.
    let ok = cmd_base(&tmp)
        .args([
            "graph",
            "traverse",
            "--from",
            "danilo",
            "--fuzzy",
            "--depth",
            "1",
            "--namespace",
            "audit",
            "-q",
        ])
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let json: Value = serde_json::from_slice(&ok.stdout).unwrap();
    assert_eq!(json["from"], "danilo-aguiar-teixeira");
}

/// Bug 4: merge-entities with target in --ids must exit 1 (self-ref).
#[test]
fn bug4_merge_self_referential_ids_rejected() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // Even without real entities, early guard fires before DB lookups when
    // into_id appears in ids — but we still need a DB for ensure_db_ready
    // only if early guard is after open... Early guard is first, so no seed.
    cmd_base(&tmp)
        .args([
            "merge-entities",
            "--ids",
            "35575,35340",
            "--into-id",
            "35340",
            "--cross-namespace",
            "-q",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("self-referential").or(predicate::str::contains("35340")));
}

/// Bug 5: pure-numeric --from rejected; --from-id/--to-id work for real IDs.
#[test]
fn bug5_link_rejects_numeric_name_and_accepts_from_id() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let graph = r#"{
      "body":"alpha and beta concepts",
      "entities":[
        {"name":"concept-alpha","type":"concept"},
        {"name":"concept-beta","type":"concept"}
      ],
      "relationships":[]
    }"#;
    cmd_base(&tmp)
        .args([
            "remember",
            "--name",
            "seed-link",
            "--type",
            "user",
            "--description",
            "seed",
            "--namespace",
            "global",
            "--graph-stdin",
        ])
        .write_stdin(graph)
        .assert()
        .success();

    // Pure numeric name + create-missing must fail validation (exit 1), not create ghosts.
    cmd_base(&tmp)
        .args([
            "link",
            "--from",
            "89975",
            "--to",
            "35313",
            "--relation",
            "supports",
            "--create-missing",
            "-q",
        ])
        .assert()
        .failure()
        .code(1);

    // Resolve real IDs via graph entities JSON.
    let list = cmd_base(&tmp)
        .args(["graph", "entities", "--namespace", "global", "-q"])
        .output()
        .unwrap();
    assert!(list.status.success());
    let entities: Value = serde_json::from_slice(&list.stdout).unwrap();
    let arr = entities["entities"].as_array().expect("entities array");
    let mut alpha_id = None;
    let mut beta_id = None;
    for e in arr {
        match e["name"].as_str() {
            Some("concept-alpha") => alpha_id = e["id"].as_i64(),
            Some("concept-beta") => beta_id = e["id"].as_i64(),
            _ => {}
        }
    }
    let alpha_id = alpha_id.expect("concept-alpha id");
    let beta_id = beta_id.expect("concept-beta id");

    cmd_base(&tmp)
        .args([
            "link",
            "--from-id",
            &alpha_id.to_string(),
            "--to-id",
            &beta_id.to_string(),
            "--relation",
            "supports",
            "-q",
        ])
        .assert()
        .success();
}
