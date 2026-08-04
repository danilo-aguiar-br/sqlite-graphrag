//! Strict schema contract: entity and relation surface — delete-entity, reclassify, merge-entities, memory-entities, prune-ner, rename-entity, deep-research, reclassify-relation, normalize-entities, enrich.
//!
//! Part of the strict JSON-Schema contract suite split by GAP-SG-208. Each
//! test runs the binary, captures stdout, parses it as JSON and validates it
//! against the published `docs/schemas/*.schema.json`. The shared harness lives
//! in `tests/schema_support/`.

#[path = "schema_support/mod.rs"]
mod support;

use serde_json::Value;
use serial_test::serial;
use support::{validar_schema, Env};
// ---------------------------------------------------------------------------
// 30 — delete-entity
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_30_delete_entity() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) = env.remember_with_entities("del-ent-schema");
    let saida = env
        .cmd()
        .args(["delete-entity", "--name", &ent_a, "--cascade"])
        .output()
        .expect("delete-entity failed");
    assert!(
        saida.status.success(),
        "delete-entity: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "delete-entity");
    validar_schema(
        "delete-entity",
        include_str!("../docs/schemas/delete-entity.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 31 — reclassify
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_31_reclassify() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) = env.remember_with_entities("reclass-schema");
    let saida = env
        .cmd()
        .args(["reclassify", "--name", &ent_a, "--new-type", "tool"])
        .output()
        .expect("reclassify failed");
    assert!(
        saida.status.success(),
        "reclassify: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "reclassify");
    validar_schema(
        "reclassify",
        include_str!("../docs/schemas/reclassify.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 32 — merge-entities
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_32_merge_entities() {
    let env = Env::new();
    env.init();
    let (ent_a, ent_b) = env.remember_with_entities("merge-schema");
    let saida = env
        .cmd()
        .args(["merge-entities", "--names", &ent_a, "--into", &ent_b])
        .output()
        .expect("merge-entities failed");
    assert!(
        saida.status.success(),
        "merge-entities: exit {:?}\nstdout: {}\nstderr: {}",
        saida.status.code(),
        String::from_utf8_lossy(&saida.stdout),
        String::from_utf8_lossy(&saida.stderr)
    );
    let instancia = Env::parse_stdout(&saida, "merge-entities");
    validar_schema(
        "merge-entities",
        include_str!("../docs/schemas/merge-entities.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 33 — memory-entities
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_33_memory_entities() {
    let env = Env::new();
    env.init();
    env.remember_with_entities("mem-ent-schema");
    let saida = env
        .cmd()
        .args(["memory-entities", "--name", "mem-ent-schema"])
        .output()
        .expect("memory-entities failed");
    assert!(
        saida.status.success(),
        "memory-entities: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "memory-entities");
    validar_schema(
        "memory-entities",
        include_str!("../docs/schemas/memory-entities.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 33b — memory-entities reverse lookup (--entity)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_33b_memory_entities_reverse() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) = env.remember_with_entities("mem-ent-rev-schema");
    let saida = env
        .cmd()
        .args(["memory-entities", "--entity", &ent_a])
        .output()
        .expect("memory-entities --entity failed");
    assert!(
        saida.status.success(),
        "memory-entities --entity: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "memory-entities --entity");
    validar_schema(
        "memory-entities-reverse",
        include_str!("../docs/schemas/memory-entities-reverse.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 34 — prune-ner
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_34_prune_ner() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) = env.remember_with_entities("prune-schema");
    let saida = env
        .cmd()
        .args(["prune-ner", "--entity", &ent_a, "--dry-run"])
        .output()
        .expect("prune-ner failed");
    assert!(
        saida.status.success(),
        "prune-ner: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "prune-ner");
    validar_schema(
        "prune-ner",
        include_str!("../docs/schemas/prune-ner.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 35 — rename-entity
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_35_rename_entity() {
    let env = Env::new();
    env.init();
    let (ent_a, _ent_b) = env.remember_with_entities("rename-ent-schema");
    let new_name = format!("{ent_a}-renamed");
    let saida = env
        .cmd()
        // rename-entity re-embeds the renamed entity; without a live key that
        // would abort with exit 11 before any schema could be validated.
        .args([
            "--llm-backend",
            "none",
            "rename-entity",
            "--name",
            &ent_a,
            "--new-name",
            &new_name,
        ])
        .output()
        .expect("rename-entity failed");
    assert!(
        saida.status.success(),
        "rename-entity: exit {:?}",
        saida.status.code()
    );
    let instancia = Env::parse_stdout(&saida, "rename-entity");
    validar_schema(
        "rename-entity",
        include_str!("../docs/schemas/rename-entity.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 36 — deep-research
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_36_deep_research() {
    let env = Env::new();
    env.init();
    env.remember_simples("schema36-mem-a");
    env.remember_simples("schema36-mem-b");

    let saida = env
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
        .expect("deep-research failed");
    assert!(
        saida.status.success(),
        "deep-research: exit {:?}\nstderr: {}",
        saida.status.code(),
        String::from_utf8_lossy(&saida.stderr)
    );
    let instancia = Env::parse_stdout(&saida, "deep-research");
    validar_schema(
        "deep-research",
        include_str!("../docs/schemas/deep-research.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 37 — reclassify-relation
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_37_reclassify_relation() {
    let env = Env::new();
    env.init();
    let (ent_a, ent_b) = env.remember_with_entities("schema37-reclassify-rel");

    // Link entities with a 'mentions' relation to give the command something to work with.
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
        .expect("link failed");

    // Dry-run: safe, validates JSON contract without committing.
    let saida = env
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
        .expect("reclassify-relation failed");
    assert!(
        saida.status.success(),
        "reclassify-relation: exit {:?}\nstderr: {}",
        saida.status.code(),
        String::from_utf8_lossy(&saida.stderr)
    );
    let instancia = Env::parse_stdout(&saida, "reclassify-relation");
    validar_schema(
        "reclassify-relation",
        include_str!("../docs/schemas/reclassify-relation.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 38 — normalize-entities
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_38_normalize_entities() {
    let env = Env::new();
    env.init();
    env.remember_simples("schema38-normalize-ent");

    // Dry-run: validates JSON contract without modifying data.
    let saida = env
        .cmd()
        .args(["normalize-entities", "--dry-run"])
        .output()
        .expect("normalize-entities failed");
    assert!(
        saida.status.success(),
        "normalize-entities: exit {:?}\nstderr: {}",
        saida.status.code(),
        String::from_utf8_lossy(&saida.stderr)
    );
    let instancia = Env::parse_stdout(&saida, "normalize-entities");
    validar_schema(
        "normalize-entities",
        include_str!("../docs/schemas/normalize-entities.schema.json"),
        &instancia,
    );
}

// ---------------------------------------------------------------------------
// 39 — enrich (dry-run, NDJSON: validate each line type against its schema)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_39_enrich() {
    let env = Env::new();
    env.init();
    env.remember_simples("schema39-enrich-mem");

    let saida = env
        .cmd()
        .args([
            "enrich",
            "--operation",
            "memory-bindings",
            // `--mode codex` stopped parsing when the codex backend was removed;
            // `--dry-run` plans without an LLM, so it needs no mode at all and
            // no API key.
            "--dry-run",
        ])
        .output()
        .expect("enrich failed");
    assert!(
        saida.status.success(),
        "enrich: exit {:?}\nstderr: {}",
        saida.status.code(),
        String::from_utf8_lossy(&saida.stderr)
    );

    let stdout_str = String::from_utf8_lossy(&saida.stdout);
    let lines: Vec<&str> = stdout_str
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    assert!(
        !lines.is_empty(),
        "enrich must emit at least one NDJSON line"
    );

    let phase_schema_str = include_str!("../docs/schemas/enrich-phase.schema.json");
    let item_schema_str = include_str!("../docs/schemas/enrich-item-event.schema.json");
    let summary_schema_str = include_str!("../docs/schemas/enrich-summary.schema.json");

    let mut summary_found = false;

    for line in &lines {
        let val: Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("enrich NDJSON line not valid JSON: {e}\n{line}"));

        if val["summary"] == true {
            validar_schema("enrich-summary", summary_schema_str, &val);
            summary_found = true;
        } else if val.get("phase").is_some() {
            validar_schema("enrich-phase", phase_schema_str, &val);
        } else if val.get("item").is_some() {
            validar_schema("enrich-item", item_schema_str, &val);
        }
        // Lines from non-implemented operations include "operation" key — skip gracefully.
    }

    assert!(summary_found, "enrich must emit a summary line");
}
