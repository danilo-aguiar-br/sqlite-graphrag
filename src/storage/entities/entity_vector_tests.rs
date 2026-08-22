//! Entity vector rows (GAP-SG-146).
//!
//! An EMPTY embedding must never replace a live row — that is what keeps a
//! vectorless entity visible to the re-embed backfill instead of silently
//! persisting a hole.

use super::test_fixtures::*;
use super::*;

// v1.1.1 (P1): an empty embedding must NOT create a vector row, so the
// entity stays visible to `enrich re-embed --target entities`.
#[test]
fn test_upsert_entity_vec_empty_embedding_skips_row() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("vec-vazia");
    let entity_id = upsert_entity(&conn, "global", &e)?;

    upsert_entity_vec(&conn, entity_id, "global", "project", &[], "vec-vazia")?;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_embeddings WHERE entity_id = ?1",
        params![entity_id],
        |r| r.get(0),
    )?;
    assert_eq!(count, 0, "empty embedding must not persist a row");
    Ok(())
}

// v1.1.1 (P1): an empty embedding must NOT delete an existing live vector.
#[test]
#[serial_test::serial(env)]
fn test_upsert_entity_vec_empty_embedding_preserves_existing_row() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("vec-preservada");
    let entity_id = upsert_entity(&conn, "global", &e)?;
    let emb = embedding_zero();
    upsert_entity_vec(
        &conn,
        entity_id,
        "global",
        "project",
        &emb,
        "vec-preservada",
    )?;

    upsert_entity_vec(&conn, entity_id, "global", "project", &[], "vec-preservada")?;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_embeddings WHERE entity_id = ?1",
        params![entity_id],
        |r| r.get(0),
    )?;
    assert_eq!(count, 1, "existing vector must survive an empty upsert");
    Ok(())
}

#[test]
#[serial_test::serial(env)]
fn test_upsert_entity_vec_first_time_without_conflict() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("vec-nova");
    let entity_id = upsert_entity(&conn, "global", &e)?;
    let emb = embedding_zero();

    let result = upsert_entity_vec(&conn, entity_id, "global", "project", &emb, "vec-nova");
    assert!(result.is_ok(), "first insertion must succeed");

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_embeddings WHERE entity_id = ?1",
        params![entity_id],
        |r| r.get(0),
    )?;
    assert_eq!(count, 1, "must have exactly one row after insertion");
    Ok(())
}

#[test]
#[serial_test::serial(env)]
fn test_upsert_entity_vec_second_time_replaces_without_error() -> TestResult {
    // Covers the branch where DELETE removes the existing row before INSERT.
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("vec-existente");
    let entity_id = upsert_entity(&conn, "global", &e)?;
    let emb = embedding_zero();

    upsert_entity_vec(&conn, entity_id, "global", "project", &emb, "vec-existente")?;

    // Second call: DELETE returns 1 removed row, INSERT must succeed.
    let result = upsert_entity_vec(&conn, entity_id, "global", "tool", &emb, "vec-existente");
    assert!(
        result.is_ok(),
        "second insertion (replace) must succeed: {result:?}"
    );

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_embeddings WHERE entity_id = ?1",
        params![entity_id],
        |r| r.get(0),
    )?;
    assert_eq!(count, 1, "must have exactly one row after replacement");
    Ok(())
}

#[test]
#[serial_test::serial(env)]
fn test_upsert_entity_vec_multiple_independent_entities() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let emb = embedding_zero();

    for i in 0..3i64 {
        let name = format!("ent-{i}");
        let e = new_entity_helper(&name);
        let entity_id = upsert_entity(&conn, "global", &e)?;
        upsert_entity_vec(&conn, entity_id, "global", "project", &emb, &name)?;
    }

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM entity_embeddings", [], |r| r.get(0))?;
    assert_eq!(
        count, 3,
        "must have three distinct rows in entity_embeddings"
    );
    Ok(())
}
