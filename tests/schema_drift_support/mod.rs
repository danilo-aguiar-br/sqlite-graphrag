//! Shared fixture and validation harness for the two schema type-drift gates.
//!
//! GAP-SG-288 grew `schema_type_drift_gate.rs` from 654 to 1198 physical lines,
//! past the 800-line ceiling `file_size_ceiling_gate.rs` enforces. The file was
//! split by responsibility rather than by size: the READ side plus the census
//! stayed in `schema_type_drift_gate.rs`, the WRITE side moved to
//! `schema_type_drift_write_gate.rs`, and everything both of them need lives
//! here.
//!
//! Nothing was weakened to make the three fit. Every case that ran before still
//! runs, against the same fixture, through the same validator.
//!
//! This module is deliberately NOT a test target: it declares no `#[test]`, so
//! `cargo test` compiles it once per consumer and runs nothing from it.

#![allow(dead_code)]

// The lower harness — `Env`, `sgr_cmd`, `validate_schema` — is shared with the
// five `schema_contract_*.rs` suites and is reached through the same `#[path]`
// include they use. Re-exported here so a consumer needs ONE import line
// instead of two module declarations that must stay in step.
#[path = "../schema_support/mod.rs"]
pub mod support;

pub use support::{validate_schema, Env};

use std::collections::BTreeSet;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Reading the published documents
// ---------------------------------------------------------------------------

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn schemas_dir() -> PathBuf {
    repo_root().join("docs").join("schemas")
}

/// Reads `docs/schemas/<id>.schema.json` as text.
///
/// The contract suites use `include_str!`, which demands a literal path and so
/// cannot be driven from a table. Reading at runtime costs one `read_to_string`
/// per case and buys a data-driven gate whose coverage is a list, not a
/// sequence of copy-pasted functions.
pub fn published_schema_text(id: &str) -> String {
    let path = schemas_dir().join(format!("{id}.schema.json"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Every contract id under `docs/schemas/`, i.e. every file stem without
/// `.schema.json`.
pub fn published_ids() -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    let entries = std::fs::read_dir(schemas_dir()).expect("docs/schemas must be readable");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(stem) = name.strip_suffix(".schema.json") {
            ids.insert(stem.to_string());
        }
    }
    ids
}

// ---------------------------------------------------------------------------
// The live cases
// ---------------------------------------------------------------------------

/// A command whose real stdout this file validates against its contract.
pub struct LiveCase {
    /// Contract id, i.e. the `docs/schemas/<id>.schema.json` stem.
    pub id: &'static str,
    /// Arguments appended after the harness-supplied globals.
    pub argv: &'static [&'static str],
}

/// Read-only commands, safe to run in any order against one populated database.
///
/// `graph recompute-degree` writes, but it only reconciles a cached column with
/// the edges already stored, so it leaves the fixture usable by the cases after
/// it. Everything that changes what a later case would read lives in its own
/// test below.
pub const READ_ONLY_CASES: &[LiveCase] = &[
    LiveCase {
        id: "config-list",
        argv: &["config", "list"],
    },
    LiveCase {
        id: "embedding-status",
        argv: &["embedding", "status"],
    },
    LiveCase {
        id: "embedding-list",
        argv: &["embedding", "list"],
    },
    LiveCase {
        id: "slots-status",
        argv: &["slots", "status"],
    },
    LiveCase {
        id: "graph-stats",
        argv: &["graph", "stats"],
    },
    LiveCase {
        id: "graph-entities",
        argv: &["graph", "entities"],
    },
    LiveCase {
        id: "graph-entity-types",
        argv: &["graph", "entity-types"],
    },
    LiveCase {
        id: "graph-traverse",
        argv: &["graph", "traverse", "--from", FIXTURE_ENTITY],
    },
    LiveCase {
        id: "graph-recompute-degree",
        argv: &["graph", "recompute-degree"],
    },
    LiveCase {
        id: "vec-stats",
        argv: &["vec", "stats"],
    },
    LiveCase {
        id: "vec-orphan-list",
        argv: &["vec", "orphan-list"],
    },
    LiveCase {
        id: "prune-relations",
        argv: &["prune-relations", "--relation", "depends-on", "--dry-run"],
    },
    LiveCase {
        id: "split-body",
        argv: &[
            "split-body",
            "--name",
            FIXTURE_MEMORY,
            "--threshold",
            "1",
            "--dry-run",
        ],
    },
    LiveCase {
        id: "health",
        argv: &["health"],
    },
    LiveCase {
        id: "list",
        argv: &["list"],
    },
    // GAP-SG-291: `--limit` below the fixture size is what sets `truncated` and
    // makes `truncation_warning` reach the wire. The unbounded case above never
    // truncates, so the closed root validated while the field it forgot to
    // declare stayed invisible.
    LiveCase {
        id: "list",
        argv: &["list", "--limit", "1"],
    },
    LiveCase {
        id: "read",
        argv: &["read", "--name", FIXTURE_MEMORY],
    },
    // GAP-SG-291: `--with-graph` is the opt-in that makes `entities` and
    // `relationships` reach the wire. Both are skipped without it.
    LiveCase {
        id: "read",
        argv: &["read", "--name", FIXTURE_MEMORY, "--with-graph"],
    },
    LiveCase {
        id: "stats",
        argv: &["stats"],
    },
    LiveCase {
        id: "history",
        argv: &["history", "--name", FIXTURE_MEMORY],
    },
    // GAP-SG-291: the same command through the flag that makes
    // `HistoryVersion::changes` reach the wire. Without `--diff` the field is
    // skipped and the closed `$defs/HistoryVersion` validates; with it the
    // field appears and the document has no declaration for it. Same shape as
    // GAP-SG-290: the common path passes, the path an operator ASKED for does
    // not. Same `id` because `--diff` adds a field to the SAME document rather
    // than publishing another one, unlike `memory-entities-reverse`.
    LiveCase {
        id: "history",
        argv: &["history", "--name", FIXTURE_MEMORY, "--diff"],
    },
    LiveCase {
        id: "related",
        argv: &["related", "--name", FIXTURE_MEMORY],
    },
    LiveCase {
        id: "memory-entities",
        argv: &["memory-entities", "--name", FIXTURE_MEMORY],
    },
    // Same subcommand, the other direction: `--entity` answers "which memories
    // mention this entity?" and publishes a DIFFERENT document. Validating only
    // the `--name` form would leave the reverse contract unchecked while the
    // census reported the command covered.
    LiveCase {
        id: "memory-entities-reverse",
        argv: &["memory-entities", "--entity", FIXTURE_ENTITY],
    },
    LiveCase {
        id: "namespace-detect",
        argv: &["namespace-detect"],
    },
    LiveCase {
        id: "debug-schema",
        argv: &["debug-schema"],
    },
    LiveCase {
        id: "fts-stats",
        argv: &["fts", "stats"],
    },
    LiveCase {
        id: "fts-check",
        argv: &["fts", "check"],
    },
    LiveCase {
        id: "graph",
        argv: &["graph", "--format", "json"],
    },
    // `migrate` with no operation flag reports the schema version of an already
    // migrated database, so it is a read against this fixture. The `--rehash`
    // and `--to-llm-only` forms publish separate documents and live in their own
    // test below, because those two DO change the file.
    LiveCase {
        id: "migrate",
        argv: &["migrate"],
    },
    // Both run against the offline OpenRouter stub the harness already wires
    // in, so the query embedding is produced without leaving the machine.
    LiveCase {
        id: "recall",
        argv: &["recall", "corpo"],
    },
    LiveCase {
        id: "hybrid-search",
        argv: &["hybrid-search", "corpo"],
    },
    // GAP-SG-290: the DEGRADED shape of the same two commands. `--fallback-fts-only`
    // is offline like the pair above, but it takes the branch where
    // `vec_degraded`, `vec_error`, `warning` and `vec_degraded_code` stop being
    // skipped and reach the wire, and where `source` becomes `fts_fallback`.
    // Running the non-degraded path alone is what let five violations sit
    // undetected: every one of them lives on fields that only exist here.
    LiveCase {
        id: "recall",
        argv: &["recall", "corpo", "--fallback-fts-only"],
    },
    LiveCase {
        id: "hybrid-search",
        argv: &["hybrid-search", "corpo", "--fallback-fts-only"],
    },
    LiveCase {
        id: "remember-dry-run",
        argv: &[
            "--llm-backend",
            "none",
            "remember",
            "--name",
            "mem-drift-dry-run",
            "--type",
            "project",
            "--description",
            "d",
            "--body",
            "corpo-que-nao-sera-gravado",
            "--dry-run",
        ],
    },
    LiveCase {
        id: "cleanup-orphans",
        argv: &["cleanup-orphans", "--dry-run"],
    },
    // `--status` is documented as a read-only queue inspector that never calls
    // the LLM and never takes the singleton, so it needs no provider at all.
    // `enrich_status_schema_drift_gate.rs` already reads this document, but it
    // compares it against the Rust STRUCT; this is the first check against the
    // bytes the command writes.
    LiveCase {
        id: "enrich-status",
        argv: &["enrich", "--status"],
    },
    LiveCase {
        id: "normalize-entities",
        argv: &["normalize-entities", "--dry-run"],
    },
    LiveCase {
        id: "prune-ner",
        argv: &["prune-ner", "--all", "--dry-run"],
    },
    // Reads the `depends-on` edge `fixture_env` creates; `--dry-run` keeps it.
    LiveCase {
        id: "reclassify-relation",
        argv: &[
            "reclassify-relation",
            "--source",
            FIXTURE_ENTITY,
            "--target",
            FIXTURE_ENTITY_B,
            "--from-relation",
            "depends-on",
            "--to-relation",
            "relates-to",
            "--dry-run",
        ],
    },
];

/// Name of the memory every live case reads.
pub const FIXTURE_MEMORY: &str = "mem-type-drift-fixture";

/// Second memory, sharing both of [`FIXTURE_MEMORY`]'s entities.
///
/// Exists so `list --limit 1` actually truncates and `related` actually walks
/// to something, instead of validating empty answers (GAP-SG-291).
pub const FIXTURE_MEMORY_SIBLING: &str = "mem-type-drift-fixture-sibling";

/// Third entity, owned ONLY by [`FIXTURE_MEMORY_SIBLING`] and reached from the
/// fixture memory across the `relates-to` edge `fixture_env` creates.
pub const FIXTURE_ENTITY_C: &str = "EntmemtypedriftfixtureGamma";

/// First entity attached to [`FIXTURE_MEMORY`] by `remember_with_entities`,
/// which derives both names from the memory name with the dashes removed.
pub const FIXTURE_ENTITY: &str = "EntmemtypedriftfixtureAlpha";

/// Second entity attached to [`FIXTURE_MEMORY`], i.e. the other end of the
/// `depends-on` edge `fixture_env` creates.
pub const FIXTURE_ENTITY_B: &str = "EntmemtypedriftfixtureBeta";

/// Builds the shared fixture: an initialised database holding one memory with
/// two entities linked by one relationship.
pub fn fixture_env() -> Env {
    let env = Env::new();
    env.init();
    let (from, to) = env.remember_with_entities(FIXTURE_MEMORY);
    assert_eq!(
        from, FIXTURE_ENTITY,
        "the harness changed how it derives entity names, so `graph traverse` \
         is now starting from an entity that does not exist and would validate \
         an empty answer instead of a populated one"
    );
    assert_eq!(
        to, FIXTURE_ENTITY_B,
        "the harness changed how it derives entity names, so every case naming \
         the second entity is now acting on something that does not exist"
    );
    env.cmd()
        .args([
            "link",
            "--from",
            &from,
            "--to",
            &to,
            "--relation",
            "depends-on",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    // GAP-SG-291: a SECOND memory, holding a THIRD entity one hop away.
    //
    // With one memory the fixture could not exercise two contracts it claimed
    // to cover. `list --limit 1` never set `truncated`, so `truncation_warning`
    // stayed off the wire; and `related` returned an empty `results`, so
    // `$defs/RelatedMemory` — and the `from`/`to` aliases it does not declare —
    // was never validated at all. Both read as covered in the census while
    // checking nothing: the same failure mode GAP-SG-288 measured one layer up.
    //
    // The third entity is not decoration. `traverse_related` filters the SEED
    // entities out of the walk, so a sibling carrying the same two entities
    // still yields nothing; the sibling must own an entity the seed reaches
    // ACROSS an edge. Hence Gamma, and the Beta->Gamma link below.
    let sibling_ents = env.tmp.path().join("sibling_entities.json");
    std::fs::write(
        &sibling_ents,
        format!(r#"[{{"name":"{FIXTURE_ENTITY_C}","entity_type":"concept"}}]"#),
    )
    .expect("writing the sibling entities file failed");
    env.cmd()
        .args([
            "--llm-backend",
            "none",
            "remember",
            "--name",
            FIXTURE_MEMORY_SIBLING,
            "--type",
            "project",
            "--description",
            "sibling of the fixture memory, one hop away through Gamma",
            "--body",
            "corpo-irmao-para-truncation-e-related",
            "--entities-file",
            sibling_ents.to_str().expect("path is not valid UTF-8"),
        ])
        .assert()
        .success();
    env.cmd()
        .args([
            "link",
            "--from",
            FIXTURE_ENTITY_B,
            "--to",
            FIXTURE_ENTITY_C,
            "--relation",
            "relates-to",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    env
}

/// Runs one invocation and validates its stdout against the published contract.
///
/// Takes the argv as a slice rather than a whole [`LiveCase`] so an invocation
/// whose arguments are only known at runtime — `backup --output <tempdir>` —
/// reaches the same validation as the table-driven ones instead of growing a
/// second, weaker copy of it.
pub fn check_argv(env: &Env, id: &str, argv: &[&str]) {
    let output = env
        .cmd()
        .args(argv)
        .output()
        .unwrap_or_else(|e| panic!("[{id}] failed to spawn: {e}"));
    assert!(
        output.status.success(),
        "[{id}] `{}` exited {:?}\nstdout: {}\nstderr: {}",
        argv.join(" "),
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let instance = Env::parse_stdout(&output, id);
    validate_schema(id, &published_schema_text(id), &instance);
}

/// Runs one table case and validates its stdout against the published contract.
pub fn check_case(env: &Env, case: &LiveCase) {
    check_argv(env, case.id, case.argv);
}
