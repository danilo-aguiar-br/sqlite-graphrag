//! Shared fixtures for the scan test modules (GAP-SG-146).
//!
//! The in-memory schema plus the raw-row inserters the re-embed target
//! tests need to fabricate stale and empty vectors.

use rusqlite::Connection;

pub(super) fn open_test_db() -> Connection {
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

/// Inserts a raw vector row with the given dim and blob length (bytes).
pub(super) fn insert_entity_vec_raw(
    conn: &Connection,
    entity_id: i64,
    dim: usize,
    blob_len: usize,
) {
    conn.execute(
        "INSERT INTO entity_embeddings (entity_id, namespace, embedding, source, model, dim) \
         VALUES (?1, 'global', ?2, 'test', 'test', ?3)",
        rusqlite::params![entity_id, vec![0u8; blob_len], dim as i64],
    )
    .unwrap();
}

pub(super) fn insert_entity_named(conn: &Connection, name: &str) -> i64 {
    conn.execute(
        &format!(
            "INSERT INTO entities (namespace, name, type) VALUES ('global', '{name}', 'tool')"
        ),
        [],
    )
    .unwrap();
    conn.last_insert_rowid()
}

pub(super) fn insert_chunk_row(conn: &Connection, memory_id: i64, chunk_idx: i32) -> i64 {
    conn.execute(
        "INSERT INTO memory_chunks (memory_id, chunk_idx, chunk_text) \
         VALUES (?1, ?2, 'chunk text')",
        rusqlite::params![memory_id, chunk_idx],
    )
    .unwrap();
    conn.last_insert_rowid()
}

pub(super) fn insert_chunk_vec_raw(conn: &Connection, chunk_id: i64, memory_id: i64, dim: usize) {
    conn.execute(
        "INSERT INTO chunk_embeddings (chunk_id, memory_id, embedding, source, model, dim) \
         VALUES (?1, ?2, ?3, 'test', 'test', ?4)",
        rusqlite::params![chunk_id, memory_id, vec![0u8; dim * 4], dim as i64],
    )
    .unwrap();
}
