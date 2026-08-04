//! Re-embed target selection across memories, entities and chunks
//! (GAP-SG-146).
//!
//! Selection is by LIVE blob width, not by the `dim` column, so a stale or
//! empty vector is rescanned.

use super::test_fixtures::{
    insert_chunk_row, insert_chunk_vec_raw, insert_entity_named, insert_entity_vec_raw,
    open_test_db,
};
use super::*;

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

    let ids = scan_chunks_missing_embeddings(&conn, "global", None, &[], 512).unwrap();
    assert_eq!(
        ids,
        vec![c_stale, c_missing],
        "stale-dim and missing chunks must be selected; live must not"
    );

    // Name filter restricts by PARENT memory name.
    let filtered =
        scan_chunks_missing_embeddings(&conn, "global", None, &["other-mem".to_string()], 512)
            .unwrap();
    assert!(filtered.is_empty());
    let filtered =
        scan_chunks_missing_embeddings(&conn, "global", None, &["chunked".to_string()], 512)
            .unwrap();
    assert_eq!(filtered, vec![c_stale, c_missing]);
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

    let ids = scan_chunks_missing_embeddings(&conn, "global", None, &[], 512).unwrap();
    assert!(
        ids.contains(&orphan_chunk),
        "orphan chunk of soft-deleted memory must be selected for re-embed"
    );
}

#[test]
fn scan_entities_missing_embeddings_respects_name_filter() {
    let conn = open_test_db();
    insert_entity_named(&conn, "ent-a");
    insert_entity_named(&conn, "ent-b");

    let rows = scan_entities_missing_embeddings(&conn, "global", None, &["ent-b".to_string()], 512)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1, "ent-b");
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

    let rows = scan_entities_missing_embeddings(&conn, "global", None, &[], 512).unwrap();
    let names: Vec<&str> = rows.iter().map(|(_, n)| n.as_str()).collect();
    assert_eq!(
        names,
        vec!["ent-missing", "ent-stale-dim", "ent-empty-blob"],
        "missing, stale-dim and empty-blob entities must be selected; live must not"
    );
    assert!(!names.contains(&"ent-live"));
    let _ = e_missing;
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

    let rows = scan_memories_without_embeddings(&conn, "global", None, &[], 512).unwrap();
    assert_eq!(rows.len(), 1, "legacy-dim vector must be re-selected");
    assert_eq!(rows[0].1, "stale-dim");
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

    let results = scan_memories_without_embeddings(&conn, "global", None, &[], 512).unwrap();
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
        scan_memories_without_embeddings(&conn, "global", None, &["match-me".to_string()], 512)
            .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "match-me");
}
