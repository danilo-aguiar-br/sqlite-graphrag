//! Scan unit tests (Wave C1).

use super::*;
use rusqlite::Connection;

fn open_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
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
            metadata    TEXT NOT NULL DEFAULT '{}',
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
            dim         INTEGER NOT NULL DEFAULT 384,
            created_at  INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE TABLE entity_embeddings (
            entity_id   INTEGER PRIMARY KEY,
            namespace   TEXT NOT NULL,
            embedding   BLOB NOT NULL,
            source      TEXT NOT NULL,
            model       TEXT NOT NULL DEFAULT '',
            dim         INTEGER NOT NULL DEFAULT 384,
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
            dim         INTEGER NOT NULL DEFAULT 384,
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
    )
    .expect("schema creation must succeed");
    conn
}

#[test]
fn count_operation_backlog_body_enrich_uses_default_threshold() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'short', 'tiny')",
        [],
    )
    .unwrap();
    let long_body = "a".repeat(super::DEFAULT_BODY_ENRICH_MIN_CHARS + 100);
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'long', ?1)",
        rusqlite::params![long_body],
    )
    .unwrap();

    let n = count_operation_backlog(
        &conn,
        &EnrichOperation::BodyEnrich,
        "global",
        ReEmbedTarget::Memories,
    )
    .unwrap();
    assert_eq!(n, 1);
    // Parity against the scanner using the same default threshold.
    let scanned = scan_short_body_memories(
        &conn,
        "global",
        super::DEFAULT_BODY_ENRICH_MIN_CHARS,
        None,
        &[],
    )
    .unwrap();
    assert_eq!(n as usize, scanned.len());
}

#[test]
fn count_operation_backlog_advisory_ops_report_zero() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'm', 'b')",
        [],
    )
    .unwrap();
    for op in [
        EnrichOperation::CrossDomainBridges,
        EnrichOperation::GraphAudit,
        EnrichOperation::BodyExtract,
    ] {
        let n = count_operation_backlog(&conn, &op, "global", ReEmbedTarget::Memories).unwrap();
        assert_eq!(n, 0, "advisory op {op:?} must report zero backlog");
    }
}

#[test]
fn count_operation_backlog_entity_connect_counts_isolated() {
    let conn = open_test_db();
    // entidade degree-0 COM binding NER -> deve contar como backlog
    conn.execute(
        "INSERT INTO entities (namespace, name, type, degree) VALUES ('global','hub','tool',0)",
        [],
    )
    .unwrap();
    let hub_id: i64 = conn
        .query_row("SELECT id FROM entities WHERE name='hub'", [], |r| r.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global','m','b')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='m'", [], |r| r.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (entity_id, memory_id) VALUES (?1, ?2)",
        rusqlite::params![hub_id, mem_id],
    )
    .unwrap();
    let n = count_operation_backlog(
        &conn,
        &EnrichOperation::EntityConnect,
        "global",
        ReEmbedTarget::Memories,
    )
    .unwrap();
    assert!(
        n > 0,
        "entity-connect backlog must count degree-0 entities with NER bindings"
    );
}

#[test]
fn scan_isolated_entity_pairs_excludes_seen() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','a','tool')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','b','tool')",
        [],
    )
    .unwrap();
    let (a_id, b_id): (i64, i64) = conn
        .query_row(
            "SELECT (SELECT id FROM entities WHERE name='a'), \
             (SELECT id FROM entities WHERE name='b')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    // Co-occurrence evidence so the pair would otherwise be a candidate.
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global','m-ab','body')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='m-ab'", [], |r| {
            r.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2), (?1, ?3)",
        rusqlite::params![mem_id, a_id, b_id],
    )
    .unwrap();
    // marca o par como já avaliado (verdict none)
    conn.execute(
        "INSERT INTO entity_connect_seen (source_id, target_id, namespace, verdict) \
         VALUES (?1, ?2, 'global','none')",
        rusqlite::params![a_id, b_id],
    )
    .unwrap();
    let pairs = scan_isolated_entity_pairs(&conn, "global", Some(50)).unwrap();
    assert!(
        pairs
            .iter()
            .all(|(id1, _, id2, _)| !(*id1 == a_id && *id2 == b_id)),
        "seen pair must not be re-scanned"
    );
}

#[test]
fn format_and_parse_pair_key_roundtrip() {
    assert_eq!(format_pair_key(3, 1), "pair:1:3");
    assert_eq!(parse_pair_key("pair:1:3"), Some((1, 3)));
    assert_eq!(parse_pair_key("pair:9:2"), Some((2, 9)));
    assert_eq!(parse_pair_key("legacy-entity-name"), None);
    assert_eq!(parse_pair_key("pair:x:y"), None);
}

#[test]
fn scan_isolated_entity_pairs_uses_cooccurrence_not_cartesian() {
    let conn = open_test_db();
    // Three entities: only a+b co-occur; c is isolated (no shared memory).
    for name in ["a", "b", "c"] {
        conn.execute(
            "INSERT INTO entities (namespace, name, type, degree) VALUES ('global', ?1, 'tool', 0)",
            rusqlite::params![name],
        )
        .unwrap();
    }
    let (a_id, b_id, c_id): (i64, i64, i64) = conn
        .query_row(
            "SELECT \
               (SELECT id FROM entities WHERE name='a'), \
               (SELECT id FROM entities WHERE name='b'), \
               (SELECT id FROM entities WHERE name='c')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global','m','body')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='m'", [], |r| r.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2), (?1, ?3)",
        rusqlite::params![mem_id, a_id, b_id],
    )
    .unwrap();
    // c has a binding alone (island) but does not co-occur with a/b.
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global','m-c','body')",
        [],
    )
    .unwrap();
    let mem_c: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='m-c'", [], |r| r.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
        rusqlite::params![mem_c, c_id],
    )
    .unwrap();

    let pairs = scan_isolated_entity_pairs(&conn, "global", Some(50)).unwrap();
    assert!(
        pairs.iter().any(|(x, _, y, _)| *x == a_id && *y == b_id),
        "co-occurring a-b must be a candidate: {pairs:?}"
    );
    // Without a hub of degree>0, hub×island may not pair c with a/b; the
    // invariant is we never invent a-c/b-c from a pure cartesian product
    // when they never co-occur and no hub fill applies.
    let only_ab = pairs.len() == 1 && pairs[0].0 == a_id && pairs[0].2 == b_id;
    let allowed = |x: i64, y: i64| -> bool {
        if x == a_id && y == b_id {
            return true;
        }
        if x == a_id && y == c_id {
            return true;
        }
        x == b_id && y == c_id
    };
    assert!(
        only_ab || pairs.iter().all(|(x, _, y, _)| allowed(*x, *y)),
        "unexpected pairs: {pairs:?}"
    );
}

#[test]
fn scan_isolated_entity_pairs_respects_limit_on_large_namespace() {
    let conn = open_test_db();
    // 80 entities sharing one memory → many co-pairs; LIMIT must cap.
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global','bulk','x')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='bulk'", [], |r| {
            r.get(0)
        })
        .unwrap();
    for i in 0..80 {
        conn.execute(
            "INSERT INTO entities (namespace, name, type) VALUES ('global', ?1, 'tool')",
            rusqlite::params![format!("e{i}")],
        )
        .unwrap();
        let eid: i64 = conn
            .query_row(
                "SELECT id FROM entities WHERE name = ?1",
                rusqlite::params![format!("e{i}")],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
            rusqlite::params![mem_id, eid],
        )
        .unwrap();
    }
    let started = std::time::Instant::now();
    let pairs = scan_isolated_entity_pairs(&conn, "global", Some(10)).unwrap();
    let elapsed = started.elapsed();
    assert_eq!(pairs.len(), 10, "LIMIT 10 must be honoured");
    assert!(
        elapsed.as_secs() < 5,
        "scan must finish quickly on co-occurrence graph, took {elapsed:?}"
    );
}

// -----------------------------------------------------------------------
// v1.1.1 (P2/P10): re-embed targets — entity/chunk backfill scanners and
// dim-divergence selection.
// -----------------------------------------------------------------------

/// Inserts a raw vector row with the given dim and blob length (bytes).
fn insert_entity_vec_raw(conn: &Connection, entity_id: i64, dim: usize, blob_len: usize) {
    conn.execute(
        "INSERT INTO entity_embeddings (entity_id, namespace, embedding, source, model, dim) \
         VALUES (?1, 'global', ?2, 'test', 'test', ?3)",
        rusqlite::params![entity_id, vec![0u8; blob_len], dim as i64],
    )
    .unwrap();
}

fn insert_entity_named(conn: &Connection, name: &str) -> i64 {
    conn.execute(
        &format!(
            "INSERT INTO entities (namespace, name, type) VALUES ('global', '{name}', 'tool')"
        ),
        [],
    )
    .unwrap();
    conn.last_insert_rowid()
}

#[test]
fn scan_entities_missing_embeddings_selects_missing_stale_and_empty() {
    let conn = open_test_db();
    let dim = crate::constants::embedding_dim();

    let e_missing = insert_entity_named(&conn, "ent-missing");
    let e_live = insert_entity_named(&conn, "ent-live");
    let e_stale = insert_entity_named(&conn, "ent-stale-dim");
    let e_empty = insert_entity_named(&conn, "ent-empty-blob");

    insert_entity_vec_raw(&conn, e_live, dim, dim * 4);
    insert_entity_vec_raw(&conn, e_stale, 64, 64 * 4);
    insert_entity_vec_raw(&conn, e_empty, dim, 0);

    let rows = scan_entities_missing_embeddings(&conn, "global", None, &[]).unwrap();
    let names: Vec<&str> = rows.iter().map(|(_, n)| n.as_str()).collect();
    assert_eq!(
        names,
        vec!["ent-missing", "ent-stale-dim", "ent-empty-blob"],
        "missing, stale-dim and empty-blob entities must be selected; live must not"
    );
    assert!(!names.contains(&"ent-live"));
    let _ = e_missing;
}

#[test]
fn scan_entities_missing_embeddings_respects_name_filter() {
    let conn = open_test_db();
    insert_entity_named(&conn, "ent-a");
    insert_entity_named(&conn, "ent-b");

    let rows = scan_entities_missing_embeddings(&conn, "global", None, &["ent-b".to_string()])
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1, "ent-b");
}

fn insert_chunk_row(conn: &Connection, memory_id: i64, chunk_idx: i32) -> i64 {
    conn.execute(
        "INSERT INTO memory_chunks (memory_id, chunk_idx, chunk_text) \
         VALUES (?1, ?2, 'chunk text')",
        rusqlite::params![memory_id, chunk_idx],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn insert_chunk_vec_raw(conn: &Connection, chunk_id: i64, memory_id: i64, dim: usize) {
    conn.execute(
        "INSERT INTO chunk_embeddings (chunk_id, memory_id, embedding, source, model, dim) \
         VALUES (?1, ?2, ?3, 'test', 'test', ?4)",
        rusqlite::params![chunk_id, memory_id, vec![0u8; dim * 4], dim as i64],
    )
    .unwrap();
}

#[test]
fn scan_chunks_missing_embeddings_selects_missing_and_stale_dim() {
    let conn = open_test_db();
    let dim = crate::constants::embedding_dim();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'chunked', 'b')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='chunked'", [], |r| {
            r.get(0)
        })
        .unwrap();

    let c_live = insert_chunk_row(&conn, mem_id, 0);
    let c_stale = insert_chunk_row(&conn, mem_id, 1);
    let c_missing = insert_chunk_row(&conn, mem_id, 2);
    insert_chunk_vec_raw(&conn, c_live, mem_id, dim);
    insert_chunk_vec_raw(&conn, c_stale, mem_id, 64);

    let ids = scan_chunks_missing_embeddings(&conn, "global", None, &[]).unwrap();
    assert_eq!(
        ids,
        vec![c_stale, c_missing],
        "stale-dim and missing chunks must be selected; live must not"
    );

    // Name filter restricts by PARENT memory name.
    let filtered =
        scan_chunks_missing_embeddings(&conn, "global", None, &["other-mem".to_string()])
            .unwrap();
    assert!(filtered.is_empty());
    let filtered =
        scan_chunks_missing_embeddings(&conn, "global", None, &["chunked".to_string()])
            .unwrap();
    assert_eq!(filtered, vec![c_stale, c_missing]);
}

// P10: a memory whose stored vector has a divergent dim is re-scanned.
#[test]
fn scan_memories_with_stale_dim_are_rescanned() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'stale-dim', 'body')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='stale-dim'", [], |r| {
            r.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO memory_embeddings (memory_id, namespace, embedding, source, model, dim) \
         VALUES (?1, 'global', ?2, 'test', 'test', 64)",
        rusqlite::params![mem_id, vec![0u8; 64 * 4]],
    )
    .unwrap();

    let rows = scan_memories_without_embeddings(&conn, "global", None, &[]).unwrap();
    assert_eq!(rows.len(), 1, "legacy-dim vector must be re-selected");
    assert_eq!(rows[0].1, "stale-dim");
}

#[test]
fn count_operation_backlog_re_embed_targets_match_scanners() {
    let conn = open_test_db();
    let dim = crate::constants::embedding_dim();

    // One memory without vector, one entity stale, one chunk missing.
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'no-vec', 'b')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='no-vec'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let eid = insert_entity_named(&conn, "ent-stale");
    insert_entity_vec_raw(&conn, eid, 64, 64 * 4);
    insert_chunk_row(&conn, mem_id, 0);
    let _ = dim;

    let n_mem = count_operation_backlog(
        &conn,
        &EnrichOperation::ReEmbed,
        "global",
        ReEmbedTarget::Memories,
    )
    .unwrap();
    assert_eq!(
        n_mem as usize,
        scan_memories_without_embeddings(&conn, "global", None, &[])
            .unwrap()
            .len()
    );

    let n_ent = count_operation_backlog(
        &conn,
        &EnrichOperation::ReEmbed,
        "global",
        ReEmbedTarget::Entities,
    )
    .unwrap();
    assert_eq!(
        n_ent as usize,
        scan_entities_missing_embeddings(&conn, "global", None, &[])
            .unwrap()
            .len()
    );

    let n_chunk = count_operation_backlog(
        &conn,
        &EnrichOperation::ReEmbed,
        "global",
        ReEmbedTarget::Chunks,
    )
    .unwrap();
    assert_eq!(
        n_chunk as usize,
        scan_chunks_missing_embeddings(&conn, "global", None, &[])
            .unwrap()
            .len()
    );

    let n_all = count_operation_backlog(
        &conn,
        &EnrichOperation::ReEmbed,
        "global",
        ReEmbedTarget::All,
    )
    .unwrap();
    assert_eq!(n_all, n_mem + n_ent + n_chunk, "all = soma dos três alvos");
}

// Bug 6: chunks whose parent memory was soft-deleted stay invisible to
// re-embed under the old INNER JOIN + `m.deleted_at IS NULL` filter.
// LEFT JOIN + `(m.namespace = ?1 OR m.id IS NULL)` must surface them.
#[test]
fn scan_chunks_of_soft_deleted_memory_are_selected() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body, deleted_at) \
         VALUES ('global', 'gone-mem', 'b', 1700000000)",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='gone-mem'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let orphan_chunk = insert_chunk_row(&conn, mem_id, 0);

    let ids = scan_chunks_missing_embeddings(&conn, "global", None, &[]).unwrap();
    assert!(
        ids.contains(&orphan_chunk),
        "orphan chunk of soft-deleted memory must be selected for re-embed"
    );
}

#[test]
fn count_backlog_includes_orphan_chunks() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body, deleted_at) \
         VALUES ('global', 'gone-mem', 'b', 1700000000)",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='gone-mem'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let orphan_chunk = insert_chunk_row(&conn, mem_id, 0);

    let n = count_operation_backlog(
        &conn,
        &EnrichOperation::ReEmbed,
        "global",
        ReEmbedTarget::Chunks,
    )
    .unwrap();
    assert!(
        n >= 1,
        "orphan chunk of soft-deleted memory must be counted in backlog"
    );
    let _ = orphan_chunk;
}
