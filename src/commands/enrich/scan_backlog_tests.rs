//! Backlog counters and their parity with the scanners (GAP-SG-146).
//!
//! `count_operation_backlog` must agree with the scanner for the same
//! operation; a divergence is what makes `--until-empty` spin forever.

use super::test_fixtures::{
    insert_chunk_row, insert_entity_named, insert_entity_vec_raw, open_test_db,
};
use super::*;

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
fn count_operation_backlog_body_enrich_uses_default_threshold() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'short', 'tiny')",
        [],
    )
    .unwrap();
    let long_body = "a".repeat(crate::commands::enrich::DEFAULT_BODY_ENRICH_MIN_CHARS + 100);
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
        crate::commands::enrich::DEFAULT_BODY_ENRICH_MIN_CHARS,
        None,
        &[],
        512,
    )
    .unwrap();
    assert_eq!(n as usize, scanned.len());
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
    let scanned = scan_unbound_memories(&conn, "global", None, &[], 512).unwrap();
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
    let scanned = scan_memories_without_embeddings(&conn, "global", None, &[], 512).unwrap();
    assert_eq!(n as usize, scanned.len());
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
        scan_memories_without_embeddings(&conn, "global", None, &[], 512)
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
        scan_entities_missing_embeddings(&conn, "global", None, &[], 512)
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
        scan_chunks_missing_embeddings(&conn, "global", None, &[], 512)
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
