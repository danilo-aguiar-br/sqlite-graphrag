#![cfg(feature = "slow-tests")]

//! Suite 3 — schema and migrations: tables, triggers, indexes and metadata after init
//!
//! Part of the schema suite split by GAP-SG-210: the single file held 923 lines
//! and 21 tests, past the 800-line ceiling this project sets for itself. The
//! shared harness lives in `tests/migration_support/`, which documents the
//! sqlite-vec isolation rule and why `#[serial]` is mandatory here.

#[path = "migration_support/mod.rs"]
mod support;

use serial_test::serial;
use support::{conn_ro, index_exists, init_isolated_db, sgr_on, table_exists, trigger_exists};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Test 1 — init applies exactly 16 migrations V001 through V016
// ---------------------------------------------------------------------------
// v1.0.76 added V012 and V013 on top of the historical V001-V011 set.
// v1.1.01 added V014/V015 (pending queues); v1.1.04 added V016 (entity_connect_seen).

#[test]
#[serial]
fn init_creates_16_migrations_v001_to_v016() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    let versions: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT version FROM refinery_schema_history ORDER BY version ASC")
            .expect("prepare must work");
        stmt.query_map([], |row| row.get(0))
            .expect("query must work")
            .map(|r| r.expect("row must be readable"))
            .collect()
    };

    assert_eq!(
        versions.len(),
        16,
        "exactly 16 migrations must be applied, found: {versions:?}"
    );
    assert_eq!(
        versions,
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        "expected versions V001-V016"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — trigger trg_fts_ai exists after V004
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn trigger_trg_fts_ai_exists() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    assert!(
        trigger_exists(&conn, "trg_fts_ai"),
        "trigger trg_fts_ai must exist after V004"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — trigger trg_fts_ad exists after V004
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn trigger_trg_fts_ad_exists() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    assert!(
        trigger_exists(&conn, "trg_fts_ad"),
        "trigger trg_fts_ad must exist after V004"
    );
}

// ---------------------------------------------------------------------------
// Test 4 — trigger trg_fts_au is INTENTIONALLY ABSENT (FTS5 sync handled in Rust)
// ---------------------------------------------------------------------------
// v1.0.76 removed sqlite-vec, but the design choice of handling FTS5 sync
// in Rust (edit.rs, rename.rs, restore.rs) instead of a trigger is kept.
// trg_fts_ai and trg_fts_ad are created by V004; trg_fts_au is NOT,
// because the Rust handlers cover UPDATE-equivalent operations explicitly
// and we avoid the historical sqlite-vec / FTS5 conflict inside the
// trigger body for symmetry with the v1.0.74 design.

#[test]
#[serial]
fn trigger_trg_fts_au_absent_handled_in_rust() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    assert!(
        !trigger_exists(&conn, "trg_fts_au"),
        "trigger trg_fts_au must NOT exist — FTS5 sync is handled in Rust (edit.rs, rename.rs, restore.rs)"
    );
}

// ---------------------------------------------------------------------------
// Test 5 — memory_embeddings uses BLOB and dim=384 (v1.0.76 replacement for vec_memories)
// ---------------------------------------------------------------------------
// v1.0.76 dropped vec_memories (sqlite-vec virtual table) and replaced it with
// a regular BLOB-backed memory_embeddings table. The embedding dimensionality
// is recorded in the dim column rather than in the DDL. Cosine similarity is
// computed in pure Rust at query time (src/similarity.rs).

#[test]
#[serial]
fn memory_embeddings_blob_dim_384() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    let ddl: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'memory_embeddings'",
            [],
            |row| row.get(0),
        )
        .expect("memory_embeddings must exist in sqlite_master");

    assert!(
        ddl.contains("BLOB"),
        "memory_embeddings must declare embedding as BLOB, DDL was: {ddl}"
    );
    assert!(
        ddl.contains("dim"),
        "memory_embeddings must declare a dim column, DDL was: {ddl}"
    );
    assert!(
        ddl.contains("384"),
        "memory_embeddings must default dim to 384, DDL was: {ddl}"
    );

    // Confirm sqlite-vec tables are GONE.
    let vec_present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'vec_memories'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(1);
    assert_eq!(
        vec_present, 0,
        "vec_memories must NOT exist after V013, but it is still present"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — memory_embeddings has 2 partition-like indexes (namespace, source)
// ---------------------------------------------------------------------------
// vec_memories used sqlite-vec partition keys. memory_embeddings uses regular
// SQLite indexes. The functional requirement is "find embeddings by namespace"
// and "audit embeddings by source".

#[test]
#[serial]
fn memory_embeddings_partition_indexes() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    let has_ns_index: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'idx_memory_embeddings_ns'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    assert_eq!(
        has_ns_index, 1,
        "idx_memory_embeddings_ns must exist (namespace partition)"
    );

    let has_source_index: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'idx_memory_embeddings_source'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    assert_eq!(
        has_source_index, 1,
        "idx_memory_embeddings_source must exist (source partition)"
    );
}

// ---------------------------------------------------------------------------
// Test 7 — fts_memories uses tokenizer unicode61 with remove_diacritics 1
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn fts_memories_tokenizer_unicode61_remove_diacritics() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    let ddl: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'fts_memories'",
            [],
            |row| row.get(0),
        )
        .expect("fts_memories must exist in sqlite_master");

    assert!(
        ddl.contains("unicode61"),
        "fts_memories must use the unicode61 tokenizer, DDL: {ddl}"
    );
    assert!(
        ddl.contains("remove_diacritics"),
        "fts_memories must declare remove_diacritics, DDL: {ddl}"
    );
}

// ---------------------------------------------------------------------------
// Test 8 — FTS5 search 'cafe' matches text containing 'café' (remove_diacritics)
// ---------------------------------------------------------------------------
// Inserts a memory with an accented body via the CLI and verifies that an
// unaccented search succeeds, confirming that remove_diacritics is active.

#[test]
#[serial]
fn fts5_matching_with_accents_cafe_cafe() {
    let tmp = TempDir::new().expect("TempDir must be created");
    let db_path = tmp.path().join("test.sqlite");

    // DB init
    sgr_on(&tmp, &db_path).args(["init"]).assert().success();

    // Insert memory with accented text
    sgr_on(&tmp, &db_path)
        .args([
            "remember",
            "--name",
            "nota-cafe",
            "--type",
            "user",
            "--description",
            "note about café",
            "--body",
            "Brazilian café is famous worldwide for its quality",
        ])
        .assert()
        .success();

    // Unaccented search must find the accented memory (remove_diacritics=1)
    let conn = conn_ro(&db_path);
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM fts_memories WHERE fts_memories MATCH 'cafe'",
            [],
            |row| row.get(0),
        )
        .expect("FTS5 query must work");

    assert!(
        count >= 1,
        "FTS5 with remove_diacritics must match 'café' when searching 'cafe', count={count}"
    );
}

// ---------------------------------------------------------------------------
// Test 9 — main tables exist after init
// ---------------------------------------------------------------------------
// Verifies all 7 regular tables plus virtual vec/fts tables created by migrations.

#[test]
#[serial]
fn all_main_tables_exist_after_init() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    let tables = [
        "schema_meta",
        "memories",
        "memory_versions",
        "memory_chunks",
        "entities",
        "relationships",
        "memory_entities",
        "memory_relationships",
        "fts_memories",
    ];

    for name in tables {
        assert!(
            table_exists(&conn, name),
            "table '{name}' must exist after init"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 10 — main indexes from V001 and V005 exist
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn main_indexes_exist_after_init() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    let indexes = [
        "idx_memories_ns_type",
        "idx_memories_ns_live",
        "idx_memories_body_hash",
        "idx_entities_ns",
        "idx_me_entity",
        "idx_relationships_source",
        "idx_relationships_target",
        "idx_relationships_ns",
        "idx_relationships_ns_relation",
        "idx_entities_namespace_degree",
        "idx_memory_chunks_memory_id",
        "idx_memory_relationships_relationship_id",
    ];

    for name in indexes {
        assert!(
            index_exists(&conn, name),
            "index '{name}' must exist after init"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 11 — schema_meta contains required keys after init
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_meta_required_keys_exist() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    let expected_keys = [
        "schema_version",
        "model",
        "dim",
        "created_at",
        "namespace_initial",
    ];

    for key in expected_keys {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_meta WHERE key = ?1",
                rusqlite::params![key],
                |row| row.get(0),
            )
            .expect("schema_meta query must work");

        assert!(count > 0, "schema_meta must contain key '{key}' after init");
    }
}

// ---------------------------------------------------------------------------
// Test 12 — schema_version in schema_meta matches CURRENT_SCHEMA_VERSION (16)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn schema_version_meta_equals_16() {
    let (_tmp, db_path) = init_isolated_db();
    let conn = conn_ro(&db_path);

    let version: String = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .expect("schema_version must exist in schema_meta");

    assert_eq!(
        version, "16",
        "schema_version in schema_meta must be '16' after V016"
    );
}
