//! GAP-SG-271 / GAP-SG-288: the WRITE half of the schema type-drift gate.
//!
//! Every document here is produced by a command that CHANGES the database, so
//! none of them can join the shared read-only table in
//! `schema_type_drift_gate.rs`: a case that leaves the fixture altered is a
//! case the next one cannot read. Each test below therefore builds its own
//! fixture and runs its verbs in an order where each one has something to act
//! on.
//!
//! The split from the read side is by responsibility, not by size. What both
//! files need — the fixture, the validator wrapper, the published-document
//! reader — lives in `schema_drift_support/mod.rs`, and the census that refuses
//! a contract with no validator stays in `schema_type_drift_gate.rs`, which
//! names the ids exercised HERE explicitly. See the comment on
//! `ids_covered_here` there for why the census had to be told about this file
//! rather than left to discover it.
//!
//! NOT gated behind `slow-tests`, for the reason the contract suites state: a
//! gate the default `cargo test` never compiles is a gate-shaped reassurance.

#[path = "schema_drift_support/mod.rs"]
mod drift;

use drift::{
    check_argv, fixture_env, published_schema_text, validate_schema, Env, FIXTURE_ENTITY,
    FIXTURE_ENTITY_B,
};
use serial_test::serial;

#[test]
#[serial]
fn the_remember_batch_ndjson_lines_match_their_published_contracts() {
    let env = Env::new();
    env.init();
    let output = env
        .cmd()
        .args(["--llm-backend", "none", "remember-batch"])
        // The second line carries entities on purpose: `entities_created` is
        // guarded by `skip_serializing_if`, so a batch that creates none emits
        // a summary that validates against a schema which never declared the
        // member. Only a useful batch exposes that.
        .write_stdin(concat!(
            r#"{"name":"batch-drift-a","type":"project","description":"d","body":"corpo-batch-drift-a"}"#,
            "\n",
            r#"{"name":"batch-drift-b","type":"project","description":"d","body":"corpo-batch-drift-b","#,
            r#""entities":[{"name":"BatchDriftEnt","entity_type":"concept","description":"d"}]}"#,
            "\n",
        ))
        .output()
        .expect("remember-batch failed to spawn");
    assert!(
        output.status.success(),
        "remember-batch exited {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );

    let text = String::from_utf8(output.stdout).expect("remember-batch stdout must be UTF-8");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines.len(),
        3,
        "expected two item lines and one summary, got {} line(s): {text}",
        lines.len()
    );

    let (items, summary) = lines.split_at(lines.len() - 1);
    for line in items {
        let instance: serde_json::Value =
            serde_json::from_str(line).expect("item line must be JSON");
        assert!(
            instance.get("summary").is_none(),
            "an item line carried the `summary` marker, so the split below \
             validated a summary against the item contract: {line}"
        );
        validate_schema(
            "remember-batch",
            &published_schema_text("remember-batch"),
            &instance,
        );
    }
    let instance: serde_json::Value =
        serde_json::from_str(summary[0]).expect("summary line must be JSON");
    validate_schema(
        "remember-batch-summary",
        &published_schema_text("remember-batch-summary"),
        &instance,
    );
}

#[test]
#[serial]
fn the_init_envelope_matches_its_published_contract() {
    // `Env::init` asserts success and throws the bytes away, so the one command
    // every other fixture depends on published a contract nothing had read.
    let env = Env::new();
    check_argv(&env, "init", &["--llm-backend", "none", "init"]);
}

/// `remember` through `purge`: the whole life of one memory, in order.
///
/// Six documents that only a WRITE can produce, so they cannot join
/// [`READ_ONLY_CASES`]. They share one fixture because each step needs the
/// previous one to have happened — `restore` needs a second version, `purge`
/// needs something soft-deleted — and splitting them into six tests would mean
/// six fixtures rebuilding the same state.
#[test]
#[serial]
fn the_memory_lifecycle_envelopes_match_their_published_contracts() {
    let env = fixture_env();
    check_argv(
        &env,
        "remember",
        &[
            "--llm-backend",
            "none",
            "remember",
            "--name",
            "mem-drift-lifecycle",
            "--type",
            "project",
            "--description",
            "d",
            "--body",
            "corpo-do-ciclo-de-vida",
        ],
    );
    check_argv(
        &env,
        "edit",
        &[
            "--llm-backend",
            "none",
            "edit",
            "--name",
            "mem-drift-lifecycle",
            "--body",
            "corpo-do-ciclo-de-vida-editado",
        ],
    );
    check_argv(
        &env,
        "rename",
        &[
            "rename",
            "--from",
            "mem-drift-lifecycle",
            "--to",
            "mem-drift-lifecycle-renamed",
        ],
    );
    check_argv(
        &env,
        "restore",
        &[
            "--llm-backend",
            "none",
            "restore",
            "--name",
            "mem-drift-lifecycle-renamed",
            "--version",
            "1",
        ],
    );
    check_argv(
        &env,
        "forget",
        &["forget", "--name", "mem-drift-lifecycle-renamed"],
    );
    // `--retention-days 0` is load-bearing: at the 90-day default the command
    // succeeds having purged NOTHING, and `purged_count` / `bytes_freed` would
    // be validated on an envelope describing a no-op.
    check_argv(&env, "purge", &["purge", "--yes", "--retention-days", "0"]);
}

/// The six entity verbs, in an order where each one has something to act on.
///
/// `unlink` runs on the edge `link` just created rather than on the fixture's
/// `depends-on`, which `reclassify-relation` still reads from
/// [`READ_ONLY_CASES`]; `delete-entity --cascade` is last because it takes the
/// rest of the graph with it.
#[test]
#[serial]
fn the_entity_verb_envelopes_match_their_published_contracts() {
    let env = fixture_env();
    check_argv(
        &env,
        "link",
        &[
            "link",
            "--from",
            FIXTURE_ENTITY,
            "--to",
            FIXTURE_ENTITY_B,
            "--relation",
            "related",
            "--namespace",
            "global",
        ],
    );
    // `related`, not a synonym: `link` maps the relation through
    // `map_to_canonical_relation` before storing it and `unlink` does not, so
    // `link --relation relates-to` writes `related` and the matching `unlink
    // --relation relates-to` then exits 4 against an edge that is right there.
    check_argv(
        &env,
        "unlink",
        &[
            "unlink",
            "--from",
            FIXTURE_ENTITY,
            "--to",
            FIXTURE_ENTITY_B,
            "--relation",
            "related",
            "--namespace",
            "global",
        ],
    );
    check_argv(
        &env,
        "reclassify",
        &[
            "reclassify",
            "--name",
            FIXTURE_ENTITY_B,
            "--new-type",
            "tool",
        ],
    );
    // GAP-SG-291: the BATCH form of the same command. `--batch` is what makes
    // `matched_targets` reach the wire — the single-entity form above skips it,
    // so the closed root validated while the field it forgot to declare stayed
    // invisible. Runs AFTER the single form, on the type that one just set.
    check_argv(
        &env,
        "reclassify",
        &[
            "reclassify",
            "--from-type",
            "concept",
            "--to-type",
            "tool",
            "--batch",
        ],
    );
    check_argv(
        &env,
        "rename-entity",
        &[
            "--llm-backend",
            "none",
            "rename-entity",
            "--name",
            FIXTURE_ENTITY_B,
            "--new-name",
            "entdriftbetarenamed",
        ],
    );
    check_argv(
        &env,
        "merge-entities",
        &[
            "--llm-backend",
            "none",
            "merge-entities",
            "--names",
            "entdriftbetarenamed",
            "--into",
            FIXTURE_ENTITY,
        ],
    );
    check_argv(
        &env,
        "delete-entity",
        &["delete-entity", "--name", FIXTURE_ENTITY, "--cascade"],
    );
}

/// Maintenance verbs that rewrite the file without changing what it says.
///
/// They are kept out of [`READ_ONLY_CASES`] anyway: `vacuum` rebuilds the
/// database and `fts rebuild` drops and refills the index, and a case that
/// leaves the shared fixture mid-rebuild for the next case to read is a flake
/// waiting for a slower machine.
#[test]
#[serial]
fn the_maintenance_envelopes_match_their_published_contracts() {
    let env = fixture_env();
    check_argv(&env, "fts-rebuild", &["fts", "rebuild"]);
    check_argv(&env, "optimize", &["optimize"]);
    check_argv(&env, "vacuum", &["vacuum"]);

    let backup_dest = env.tmp.path().join("drift-backup.sqlite");
    check_argv(
        &env,
        "backup",
        &[
            "backup",
            "--output",
            backup_dest.to_str().expect("path is UTF-8"),
        ],
    );

    let copy_dest = env.tmp.path().join("drift-safe-copy.sqlite");
    check_argv(
        &env,
        "sync-safe-copy",
        &[
            "sync-safe-copy",
            "--output",
            copy_dest.to_str().expect("path is UTF-8"),
        ],
    );
}

/// `export` writes NDJSON: one line per memory, then a summary line.
///
/// Two documents, and the split between them is the whole point — a summary
/// validated against the per-memory contract, or the reverse, would report
/// green while checking the wrong shape.
#[test]
#[serial]
fn the_export_ndjson_lines_match_their_published_contracts() {
    let env = fixture_env();
    let output = env
        .cmd()
        .arg("export")
        .output()
        .expect("export failed to spawn");
    assert!(
        output.status.success(),
        "export exited {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );

    let text = String::from_utf8(output.stdout).expect("export stdout must be UTF-8");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines.len(),
        3,
        "expected two memory lines and one summary, got {} line(s): {text}",
        lines.len()
    );

    let (memories, summary) = lines.split_at(lines.len() - 1);
    for line in memories {
        let instance: serde_json::Value =
            serde_json::from_str(line).expect("memory line must be JSON");
        assert!(
            instance.get("summary").is_none(),
            "a memory line carried the `summary` marker, so the split below \
             validated a summary against the per-memory contract: {line}"
        );
        validate_schema(
            "export-memory-line",
            &published_schema_text("export-memory-line"),
            &instance,
        );
    }
    let instance: serde_json::Value =
        serde_json::from_str(summary[0]).expect("summary line must be JSON");
    validate_schema(
        "export-summary",
        &published_schema_text("export-summary"),
        &instance,
    );
}

/// `enrich --dry-run` streams the three enrichment documents with no provider.
///
/// The operation previews the items it WOULD send and consumes zero tokens, so
/// the phase, item and summary lines are produced by the same code paths a real
/// run uses while the LLM is never contacted. That is what makes these three
/// reachable here: they were assumed to need a live provider, and only the
/// write path does.
#[test]
#[serial]
fn the_enrich_dry_run_ndjson_lines_match_their_published_contracts() {
    let env = fixture_env();
    let output = env
        .cmd()
        .args(["enrich", "--operation", "entity-descriptions", "--dry-run"])
        .output()
        .expect("enrich --dry-run failed to spawn");
    assert!(
        output.status.success(),
        "enrich --dry-run exited {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );

    let text = String::from_utf8(output.stdout).expect("enrich stdout must be UTF-8");
    let mut phases = 0usize;
    let mut items = 0usize;
    let mut summaries = 0usize;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let instance: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("line is not JSON: {e}: {line}"));
        // The three shapes share one stream, and each one carries the marker
        // its own document declares: routing by marker is what keeps a summary
        // from being validated against the item contract and reported green.
        let id = if instance.get("summary").is_some() {
            summaries += 1;
            "enrich-summary"
        } else if instance.get("phase").is_some() {
            phases += 1;
            "enrich-phase"
        } else {
            items += 1;
            "enrich-item-event"
        };
        validate_schema(id, &published_schema_text(id), &instance);
    }
    assert!(
        phases >= 1 && items >= 1 && summaries == 1,
        "expected at least one phase line, at least one item line and exactly \
         one summary, got {phases}/{items}/{summaries}; a stream missing a \
         shape validates the two that remain and reports the third covered"
    );
}

// ---------------------------------------------------------------------------
// Input contracts: what the schema accepts, the binary must accept
// ---------------------------------------------------------------------------

/// The three input documents describe what a caller may HAND to `remember`, so
/// there is no stdout to validate. The check runs the other way: a document
/// that satisfies the published schema is fed to the binary, and the command
/// has to accept it. A schema promising a shape `remember` rejects is exactly
/// as misleading as a response schema promising a field nobody emits.
#[test]
#[serial]
fn the_input_contracts_describe_documents_remember_accepts() {
    let env = Env::new();
    env.init();

    let entities = serde_json::json!([
        {"name": "InputEntAlpha", "entity_type": "concept", "description": "d"},
        {"name": "InputEntBeta", "entity_type": "concept", "description": "d"}
    ]);
    let relationships = serde_json::json!([
        {"source": "InputEntAlpha", "target": "InputEntBeta",
         "relation": "depends-on", "strength": 0.7, "description": "d"}
    ]);
    let graph = serde_json::json!({
        "body": "corpo-do-contrato-de-entrada",
        "entities": entities,
        "relationships": relationships
    });

    validate_schema(
        "entities-input",
        &published_schema_text("entities-input"),
        &entities,
    );
    validate_schema(
        "relationships-input",
        &published_schema_text("relationships-input"),
        &relationships,
    );
    validate_schema("graph-input", &published_schema_text("graph-input"), &graph);

    let ents_path = env.tmp.path().join("entities-input.json");
    let rels_path = env.tmp.path().join("relationships-input.json");
    let graph_path = env.tmp.path().join("graph-input.json");
    std::fs::write(&ents_path, entities.to_string()).expect("entities file");
    std::fs::write(&rels_path, relationships.to_string()).expect("relationships file");
    std::fs::write(&graph_path, graph.to_string()).expect("graph file");

    let output = env
        .cmd()
        .args([
            "--llm-backend",
            "none",
            "remember",
            "--name",
            "mem-input-contract",
            "--type",
            "project",
            "--description",
            "d",
            "--body",
            "corpo-do-contrato-de-entrada",
            "--entities-file",
            ents_path.to_str().expect("path is UTF-8"),
            "--relationships-file",
            rels_path.to_str().expect("path is UTF-8"),
        ])
        .output()
        .expect("remember failed to spawn");
    assert!(
        output.status.success(),
        "`remember` refused a document `entities-input.schema.json` and \
         `relationships-input.schema.json` both accept; exit {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );

    let output = env
        .cmd()
        .args([
            "--llm-backend",
            "none",
            "remember",
            "--name",
            "mem-graph-contract",
            "--type",
            "project",
            "--description",
            "d",
            "--graph-file",
            graph_path.to_str().expect("path is UTF-8"),
        ])
        .output()
        .expect("remember --graph-file failed to spawn");
    assert!(
        output.status.success(),
        "`remember --graph-file` refused a document `graph-input.schema.json` \
         accepts; exit {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
}
