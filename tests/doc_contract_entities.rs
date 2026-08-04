#![cfg(feature = "slow-tests")]

//! Contract: entity and relation surface — delete-entity, reclassify, merge-entities, memory-entities, prune-ner, rename-entity, deep-research, reclassify-relation, normalize-entities.
//!
//! Part of the JSON-contract suite split by GAP-SG-208: the single file held
//! 1393 lines and 41 tests, past the 800-line ceiling this project sets for
//! itself. The shared harness lives in `tests/contract_support/`.
//!
//! Ground truth: `docs/schemas/*.schema.json`. Each test checks the expected
//! exit code, valid JSON, and the presence of the required keys.

#[path = "contract_support/mod.rs"]
mod support;

use serial_test::serial;
use support::{assert_array_items_have_keys, assert_has_keys, Env};
// ---------------------------------------------------------------------------
// 30 — delete-entity
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_30_delete_entity() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) =
        env.remember_with_entities("del-ent-contract", "body for delete entity contract");

    let out = env
        .cmd()
        .args(["delete-entity", "--name", &ent_a, "--cascade"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "delete-entity",
        &json,
        &[
            "action",
            "entity_name",
            "namespace",
            "relationships_removed",
            "bindings_removed",
            "elapsed_ms",
        ],
    );
    assert_eq!(json["action"], "deleted");
}

// ---------------------------------------------------------------------------
// 31 — reclassify
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_31_reclassify() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) =
        env.remember_with_entities("reclass-contract", "body for reclassify contract");

    let out = env
        .cmd()
        .args(["reclassify", "--name", &ent_a, "--new-type", "tool"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "reclassify",
        &json,
        &["action", "count", "namespace", "elapsed_ms"],
    );
    assert_eq!(json["action"], "reclassified");
}

// ---------------------------------------------------------------------------
// 32 — merge-entities
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_32_merge_entities() {
    let env = Env::new();
    env.init();
    let (ent_a, ent_b) =
        env.remember_with_entities("merge-contract", "body for merge entities contract");

    let out = env
        .cmd()
        .args(["merge-entities", "--names", &ent_a, "--into", &ent_b])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "merge-entities failed: {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "merge-entities",
        &json,
        &[
            "action",
            "sources",
            "target",
            "namespace",
            "relationships_moved",
            "entities_removed",
            "elapsed_ms",
        ],
    );
    assert_eq!(json["action"], "merged");
}

// ---------------------------------------------------------------------------
// 33 — memory-entities
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_33_memory_entities() {
    let env = Env::new();
    env.init();
    env.remember_with_entities("mem-ent-contract", "body for memory entities contract");

    let out = env
        .cmd()
        .args(["memory-entities", "--name", "mem-ent-contract"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "memory-entities",
        &json,
        &["memory_name", "entities", "count", "elapsed_ms"],
    );
    assert!(json["entities"].is_array());
    let ents = json["entities"].as_array().unwrap();
    if !ents.is_empty() {
        assert_array_items_have_keys(
            "memory-entities",
            &json["entities"],
            &["entity_id", "name", "entity_type"],
        );
    }
}

// ---------------------------------------------------------------------------
// 33b — memory-entities reverse lookup (--entity)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_33b_memory_entities_reverse() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) = env.remember_with_entities("mem-ent-reverse", "body for reverse lookup");

    let out = env
        .cmd()
        .args(["memory-entities", "--entity", &ent_a])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "memory-entities --entity (reverse)",
        &json,
        &["entity_name", "memories", "count", "elapsed_ms"],
    );
    assert!(json["memories"].is_array());
    let mems = json["memories"].as_array().unwrap();
    if !mems.is_empty() {
        assert_array_items_have_keys(
            "memory-entities --entity (reverse)",
            &json["memories"],
            &["memory_id", "name", "description", "memory_type"],
        );
    }
}

// ---------------------------------------------------------------------------
// 34 — prune-ner
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_34_prune_ner() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) =
        env.remember_with_entities("prune-ner-contract", "body for prune ner contract");

    let out = env
        .cmd()
        .args(["prune-ner", "--entity", &ent_a, "--dry-run"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "prune-ner",
        &json,
        &["action", "bindings_removed", "namespace", "elapsed_ms"],
    );
    assert_eq!(json["action"], "dry_run");
}

// ---------------------------------------------------------------------------
// 35 — rename-entity
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_35_rename_entity() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) =
        env.remember_with_entities("mem-rename-ent-contrato", "body for rename entity contract");
    let new_name = format!("{ent_a}-renamed");
    let out = env
        .cmd()
        .args(["rename-entity", "--name", &ent_a, "--new-name", &new_name])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "rename-entity failed: {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "rename-entity",
        &json,
        &[
            "action",
            "old_name",
            "new_name",
            "entity_id",
            "namespace",
            "elapsed_ms",
        ],
    );
    assert_eq!(json["action"], "renamed");
}

// ---------------------------------------------------------------------------
// 36 — deep-research
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_36_deep_research() {
    let env = Env::new();
    env.init();
    // Seed two memories so the DB is non-empty; deep-research still works on empty DBs
    // (returns zero results) but the JSON contract must be complete either way.
    env.remember(
        "mem-deep-a",
        "auth uses JWT tokens with 15 minute expiry and refresh flow",
    );
    env.remember(
        "mem-deep-b",
        "deploy pipeline stages: build, test, staging, production",
    );

    let out = env
        .cmd()
        .args([
            "deep-research",
            "auth and deploy",
            "--max-sub-queries",
            "2",
            "--k",
            "5",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "deep-research failed: {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let json = Env::parse_stdout(&out);

    // Top-level required keys
    assert_has_keys(
        "deep-research",
        &json,
        &[
            "query",
            "sub_queries",
            "results",
            "evidence_chains",
            "stats",
        ],
    );
    assert_eq!(json["query"], "auth and deploy");

    // sub_queries must be an array
    let sub_queries = json["sub_queries"]
        .as_array()
        .expect("sub_queries must be array");
    assert!(!sub_queries.is_empty(), "at least one sub-query expected");
    for sq in sub_queries {
        assert_has_keys("deep-research.sub_queries[]", sq, &["id", "text", "source"]);
        let source = sq["source"].as_str().expect("source must be string");
        assert!(
            source == "original" || source == "decomposed",
            "unexpected source: {source}"
        );
    }

    // results must be an array
    assert!(json["results"].is_array(), "results must be array");

    // evidence_chains must be an array
    assert!(
        json["evidence_chains"].is_array(),
        "evidence_chains must be array"
    );

    // stats required keys
    assert_has_keys(
        "deep-research.stats",
        &json["stats"],
        &[
            "sub_queries_total",
            "sub_queries_completed",
            "sub_queries_failed",
            "sub_queries_timed_out",
            "unique_memories_found",
            "evidence_chains_found",
            "elapsed_ms",
        ],
    );
    assert!(json["stats"]["elapsed_ms"].as_u64().is_some());
}

// ---------------------------------------------------------------------------
// 37 — reclassify-relation
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_37_reclassify_relation() {
    let env = Env::new();
    env.init();
    // Create two entities with a relationship between them.
    let (ent_a, ent_b) = env.remember_with_entities(
        "mem-reclassify-rel",
        "body for reclassify-relation contract",
    );

    // Link them with a 'mentions' relation so we have something to reclassify.
    let _ = env
        .cmd()
        .args([
            "link",
            "--from",
            &ent_a,
            "--to",
            &ent_b,
            "--relation",
            "mentions",
        ])
        .output()
        .unwrap();

    // Dry-run: should report count without committing.
    let out = env
        .cmd()
        .args([
            "reclassify-relation",
            "--from-relation",
            "mentions",
            "--to-relation",
            "related",
            "--batch",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "reclassify-relation dry-run failed: {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "reclassify-relation",
        &json,
        &[
            "action",
            "from_relation",
            "to_relation",
            "count",
            "merged_duplicates",
            "namespace",
            "elapsed_ms",
        ],
    );
    assert_eq!(json["action"], "dry_run");
    assert_eq!(json["from_relation"], "mentions");
    assert_eq!(json["to_relation"], "related");
    assert!(json["count"].as_u64().is_some());
    assert!(json["merged_duplicates"].as_u64().is_some());
}

// ---------------------------------------------------------------------------
// 38 — normalize-entities
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn contract_38_normalize_entities() {
    let env = Env::new();
    env.init();
    // Seed a memory; normalize-entities works even with no un-normalized names.
    env.remember(
        "mem-normalize-ent",
        "body for normalize-entities contract test",
    );

    // Dry-run: safe to run without --yes.
    let out = env
        .cmd()
        .args(["normalize-entities", "--dry-run"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "normalize-entities dry-run failed: {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let json = Env::parse_stdout(&out);
    assert_has_keys(
        "normalize-entities",
        &json,
        &[
            "action",
            "normalized_count",
            "merged_count",
            "namespace",
            "elapsed_ms",
        ],
    );
    assert_eq!(json["action"], "dry_run");
    assert!(json["normalized_count"].as_u64().is_some());
    assert!(json["merged_count"].as_u64().is_some());
}
