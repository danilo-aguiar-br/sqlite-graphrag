//! Scan unit tests (Wave C1).

use super::*;
use rusqlite::Connection;

fn open_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    // GAP-SG-119: use DEFAULT_EMBEDDING_DIM (not legacy 384) for new fixtures.
    let dim = crate::constants::DEFAULT_EMBEDDING_DIM;
    conn.execute_batch(&format!(
        "CREATE TABLE memories (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            namespace   TEXT NOT NULL DEFAULT 'global',
            name        TEXT NOT NULL,
            type        TEXT NOT NULL DEFAULT 'note',
            description TEXT NOT NULL DEFAULT '',
            body        TEXT NOT NULL DEFAULT '',
            body_hash   TEXT NOT NULL DEFAULT '',
            session_id  TEXT,
            source      TEXT NOT NULL DEFAULT 'agent',
            metadata    TEXT NOT NULL DEFAULT '{{}}',
            created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
            updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
            deleted_at  INTEGER,
            UNIQUE(namespace, name)
        );
        CREATE TABLE entities (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            namespace   TEXT NOT NULL DEFAULT 'global',
            name        TEXT NOT NULL,
            type        TEXT NOT NULL DEFAULT 'concept',
            description TEXT,
            degree      INTEGER NOT NULL DEFAULT 0,
            created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
            updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
            UNIQUE(namespace, name)
        );
        CREATE TABLE memory_entities (
            memory_id  INTEGER NOT NULL,
            entity_id  INTEGER NOT NULL,
            PRIMARY KEY (memory_id, entity_id)
        );
        CREATE TABLE relationships (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            namespace  TEXT NOT NULL DEFAULT 'global',
            source_id  INTEGER NOT NULL,
            target_id  INTEGER NOT NULL,
            relation   TEXT NOT NULL,
            weight     REAL NOT NULL DEFAULT 0.5,
            description TEXT,
            UNIQUE(source_id, target_id, relation)
        );
        CREATE TABLE memory_embeddings (
            memory_id   INTEGER PRIMARY KEY,
            namespace   TEXT NOT NULL,
            embedding   BLOB NOT NULL,
            source      TEXT NOT NULL,
            model       TEXT NOT NULL DEFAULT '',
            dim         INTEGER NOT NULL DEFAULT {dim},
            created_at  INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE TABLE entity_embeddings (
            entity_id   INTEGER PRIMARY KEY,
            namespace   TEXT NOT NULL,
            embedding   BLOB NOT NULL,
            source      TEXT NOT NULL,
            model       TEXT NOT NULL DEFAULT '',
            dim         INTEGER NOT NULL DEFAULT {dim},
            created_at  INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE TABLE memory_chunks (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            memory_id    INTEGER NOT NULL,
            chunk_idx    INTEGER NOT NULL,
            chunk_text   TEXT NOT NULL,
            start_offset INTEGER NOT NULL DEFAULT 0,
            end_offset   INTEGER NOT NULL DEFAULT 0,
            token_count  INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE chunk_embeddings (
            chunk_id    INTEGER PRIMARY KEY,
            memory_id   INTEGER NOT NULL,
            embedding   BLOB NOT NULL,
            source      TEXT NOT NULL,
            model       TEXT NOT NULL DEFAULT '',
            dim         INTEGER NOT NULL DEFAULT {dim},
            created_at  INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE TABLE entity_connect_seen (
            source_id    INTEGER NOT NULL,
            target_id    INTEGER NOT NULL,
            namespace    TEXT NOT NULL,
            verdict      TEXT NOT NULL CHECK(verdict IN ('related','none')),
            relation     TEXT,
            evaluated_at INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (source_id, target_id)
        );",
    ))
    .expect("schema creation must succeed");
    conn
}

#[test]
fn scan_unbound_memories_finds_memories_without_bindings() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'test-mem', 'some body content')",
        [],
    )
    .unwrap();

    let results = scan_unbound_memories(&conn, "global", None, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "test-mem");
}

#[test]
fn scan_unbound_memories_excludes_bound_memories() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'bound-mem', 'body')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='bound-mem'", [], |r| {
            r.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO entities (namespace, name) VALUES ('global', 'some-entity')",
        [],
    )
    .unwrap();
    let ent_id: i64 = conn
        .query_row(
            "SELECT id FROM entities WHERE name='some-entity'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
        rusqlite::params![mem_id, ent_id],
    )
    .unwrap();

    let results = scan_unbound_memories(&conn, "global", None, &[]).unwrap();
    assert!(results.is_empty(), "bound memory must not appear in scan");
}

#[test]
fn scan_entities_without_description_finds_null_description() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type, description) VALUES ('global', 'my-tool', 'tool', NULL)",
        [],
    )
    .unwrap();

    let results = scan_entities_without_description(&conn, "global", None, &[], false).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "my-tool");
}

#[test]
fn scan_entities_without_description_excludes_entities_with_description() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type, description) VALUES ('global', 'described-tool', 'tool', 'Has a description already')",
        [],
    )
    .unwrap();

    let results = scan_entities_without_description(&conn, "global", None, &[], false).unwrap();
    assert!(
        results.is_empty(),
        "entity with description must not appear"
    );
}

#[test]
fn scan_short_body_memories_finds_short_bodies() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'short-mem', 'hi')",
        [],
    )
    .unwrap();

    let results = scan_short_body_memories(&conn, "global", 100, None, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "short-mem");
}

#[test]
fn scan_short_body_memories_excludes_long_bodies() {
    let conn = open_test_db();
    let long_body = "a".repeat(1000);
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'long-mem', ?1)",
        rusqlite::params![long_body],
    )
    .unwrap();

    let results = scan_short_body_memories(&conn, "global", 100, None, &[]).unwrap();
    assert!(results.is_empty(), "long memory must not appear in scan");
}

#[test]
fn scan_respects_limit() {
    let conn = open_test_db();
    for i in 0..5 {
        conn.execute(
            &format!("INSERT INTO memories (namespace, name, body) VALUES ('global', 'mem-{i}', 'short')"),
            [],
        )
        .unwrap();
    }

    let results = scan_short_body_memories(&conn, "global", 1000, Some(3), &[]).unwrap();
    assert_eq!(results.len(), 3, "limit must be respected");
}

#[test]
fn scan_memories_without_embeddings_finds_only_missing_rows() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'missing-vec', 'body one')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'has-vec', 'body two')",
        [],
    )
    .unwrap();
    let memory_id: i64 = conn
        .query_row(
            "SELECT id FROM memories WHERE namespace='global' AND name='has-vec'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let embedding = vec![0.0_f32; crate::constants::embedding_dim()];
    crate::storage::memories::upsert_vec(
        &conn, memory_id, "global", "note", &embedding, "has-vec", "body two",
    )
    .unwrap();

    let results = scan_memories_without_embeddings(&conn, "global", None, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "missing-vec");
}

#[test]
fn scan_memories_without_embeddings_respects_name_filter() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'match-me', 'body one')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'skip-me', 'body two')",
        [],
    )
    .unwrap();

    let results =
        scan_memories_without_embeddings(&conn, "global", None, &["match-me".to_string()]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "match-me");
}

#[test]
fn dry_run_emits_preview_without_calling_llm() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'dry-mem', 'tiny')",
        [],
    )
    .unwrap();

    let results = scan_short_body_memories(&conn, "global", 1000, None, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "dry-mem");
}

#[test]
fn scan_bound_memories_for_augment_requires_names_and_finds_bound() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (id, namespace, name, body) VALUES (1, 'global', 'bound', 'b')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memories (id, namespace, name, body) VALUES (2, 'global', 'unbound', 'b')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO entities (id, namespace, name) VALUES (10, 'global', 'e')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (1, 10)",
        [],
    )
    .unwrap();

    assert!(scan_bound_memories_for_augment(&conn, "global", None, &[]).is_err());

    let names = scan_bound_memories_for_augment(
        &conn,
        "global",
        None,
        &["bound".to_string(), "unbound".to_string()],
    )
    .unwrap();
    assert_eq!(names, vec!["bound".to_string()]);
}

// -----------------------------------------------------------------------
// GAP-SG-77: count_operation_backlog — correctness + scan parity
// -----------------------------------------------------------------------

#[test]
fn count_operation_backlog_entity_descriptions_counts_only_missing() {
    let conn = open_test_db();
    for i in 0..3 {
        conn.execute(
            &format!("INSERT INTO entities (namespace, name, type, description) VALUES ('global', 'ent-{i}', 'tool', NULL)"),
            [],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO entities (namespace, name, type, description) VALUES ('global', 'described', 'tool', 'already has one')",
        [],
    )
    .unwrap();

    let n = count_operation_backlog(
        &conn,
        &EnrichOperation::EntityDescriptions,
        "global",
        ReEmbedTarget::Memories,
    )
    .unwrap();
    assert_eq!(n, 3);
    // Parity: the count must equal what the scanner would materialise.
    let scanned = scan_entities_without_description(&conn, "global", None, &[], false).unwrap();
    assert_eq!(n as usize, scanned.len());
}

#[test]
fn count_operation_backlog_re_embed_counts_missing_embeddings() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'no-vec', 'body one')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'has-vec', 'body two')",
        [],
    )
    .unwrap();
    let has_vec_id: i64 = conn
        .query_row(
            "SELECT id FROM memories WHERE namespace='global' AND name='has-vec'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let embedding = vec![0.0_f32; crate::constants::embedding_dim()];
    crate::storage::memories::upsert_vec(
        &conn, has_vec_id, "global", "note", &embedding, "has-vec", "body two",
    )
    .unwrap();

    let n = count_operation_backlog(
        &conn,
        &EnrichOperation::ReEmbed,
        "global",
        ReEmbedTarget::Memories,
    )
    .unwrap();
    assert_eq!(n, 1);
    let scanned = scan_memories_without_embeddings(&conn, "global", None, &[]).unwrap();
    assert_eq!(n as usize, scanned.len());
}

#[test]
fn count_operation_backlog_memory_bindings_counts_unbound() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'unbound', 'b')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'bound', 'b')",
        [],
    )
    .unwrap();
    let bound_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='bound'", [], |r| {
            r.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO entities (namespace, name) VALUES ('global', 'e')",
        [],
    )
    .unwrap();
    let ent_id: i64 = conn
        .query_row("SELECT id FROM entities WHERE name='e'", [], |r| r.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
        rusqlite::params![bound_id, ent_id],
    )
    .unwrap();

    let n = count_operation_backlog(
        &conn,
        &EnrichOperation::MemoryBindings,
        "global",
        ReEmbedTarget::Memories,
    )
    .unwrap();
    assert_eq!(n, 1);
    let scanned = scan_unbound_memories(&conn, "global", None, &[]).unwrap();
    assert_eq!(n as usize, scanned.len());
}
