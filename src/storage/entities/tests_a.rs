use super::*;
use crate::constants::embedding_dim;
use crate::entity_type::EntityType;
use crate::storage::connection::register_vec_extension;
use rusqlite::Connection;
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn setup_db() -> Result<(TempDir, Connection), Box<dyn std::error::Error>> {
    register_vec_extension();
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("test.db");
    let mut conn = Connection::open(&db_path)?;
    crate::migrations::runner().run(&mut conn)?;
    Ok((tmp, conn))
}

fn new_entity_helper(name: &str) -> NewEntity {
    NewEntity {
        name: name.to_string(),
        entity_type: EntityType::Project,
        description: None,
    }
}

fn embedding_zero() -> Vec<f32> {
    vec![0.0f32; embedding_dim()]
}

// ------------------------------------------------------------------ //
// upsert_entity
// ------------------------------------------------------------------ //

#[test]
fn test_upsert_entity_creates_new() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("projeto-alpha");
    let id = upsert_entity(&conn, "global", &e)?;
    assert!(id > 0);
    Ok(())
}

#[test]
fn test_upsert_entity_idempotent_returns_same_id() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("projeto-beta");
    let id1 = upsert_entity(&conn, "global", &e)?;
    let id2 = upsert_entity(&conn, "global", &e)?;
    assert_eq!(id1, id2);
    Ok(())
}

#[test]
fn test_upsert_entity_updates_description() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e1 = new_entity_helper("projeto-gamma");
    let id1 = upsert_entity(&conn, "global", &e1)?;

    let e2 = NewEntity {
        name: "projeto-gamma".to_string(),
        entity_type: EntityType::Tool,
        description: Some("nova desc".to_string()),
    };
    let id2 = upsert_entity(&conn, "global", &e2)?;
    assert_eq!(id1, id2);

    let desc: Option<String> = conn.query_row(
        "SELECT description FROM entities WHERE id = ?1",
        params![id1],
        |r| r.get(0),
    )?;
    assert_eq!(desc.as_deref(), Some("nova desc"));
    Ok(())
}

#[test]
fn test_upsert_entity_different_namespaces_create_distinct_records() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("compartilhada");
    let id1 = upsert_entity(&conn, "ns1", &e)?;
    let id2 = upsert_entity(&conn, "ns2", &e)?;
    assert_ne!(id1, id2);
    Ok(())
}

// ------------------------------------------------------------------ //
// upsert_entity_vec — covers DELETE+INSERT (new branch after the OOM fix)
// ------------------------------------------------------------------ //

// v1.1.1 (P1): an empty embedding must NOT create a vector row, so the
// entity stays visible to `enrich re-embed --target entities`.
#[test]
fn test_upsert_entity_vec_empty_embedding_skips_row() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("vec-vazia");
    let entity_id = upsert_entity(&conn, "global", &e)?;

    upsert_entity_vec(
        &conn,
        entity_id,
        "global",
        EntityType::Project,
        &[],
        "vec-vazia",
    )?;

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
        EntityType::Project,
        &emb,
        "vec-preservada",
    )?;

    upsert_entity_vec(
        &conn,
        entity_id,
        "global",
        EntityType::Project,
        &[],
        "vec-preservada",
    )?;

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

    let result = upsert_entity_vec(
        &conn,
        entity_id,
        "global",
        EntityType::Project,
        &emb,
        "vec-nova",
    );
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

    upsert_entity_vec(
        &conn,
        entity_id,
        "global",
        EntityType::Project,
        &emb,
        "vec-existente",
    )?;

    // Second call: DELETE returns 1 removed row, INSERT must succeed.
    let result = upsert_entity_vec(
        &conn,
        entity_id,
        "global",
        EntityType::Tool,
        &emb,
        "vec-existente",
    );
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
        let nome = format!("ent-{i}");
        let e = new_entity_helper(&nome);
        let entity_id = upsert_entity(&conn, "global", &e)?;
        upsert_entity_vec(&conn, entity_id, "global", EntityType::Project, &emb, &nome)?;
    }

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM entity_embeddings", [], |r| r.get(0))?;
    assert_eq!(
        count, 3,
        "must have three distinct rows in entity_embeddings"
    );
    Ok(())
}

// ------------------------------------------------------------------ //
// find_entity_id
// ------------------------------------------------------------------ //

#[test]
fn test_find_entity_id_existing_returns_some() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("entidade-busca");
    let id_inserido = upsert_entity(&conn, "global", &e)?;
    let id_encontrado = find_entity_id(&conn, "global", "entidade-busca")?;
    assert_eq!(id_encontrado, Some(id_inserido));
    Ok(())
}

#[test]
fn test_find_entity_id_missing_returns_none() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id = find_entity_id(&conn, "global", "nao-existe")?;
    assert_eq!(id, None);
    Ok(())
}

// ------------------------------------------------------------------ //
// delete_entities_by_ids
// ------------------------------------------------------------------ //

#[test]
fn test_delete_entities_by_ids_empty_list_returns_zero() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let removed = delete_entities_by_ids(&conn, &[])?;
    assert_eq!(removed, 0);
    Ok(())
}

#[test]
fn test_delete_entities_by_ids_removes_valid_entity() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("to-delete");
    let entity_id = upsert_entity(&conn, "global", &e)?;

    let removed = delete_entities_by_ids(&conn, &[entity_id])?;
    assert_eq!(removed, 1);

    let id = find_entity_id(&conn, "global", "to-delete")?;
    assert_eq!(id, None, "entity must have been removed");
    Ok(())
}

#[test]
fn test_delete_entities_by_ids_missing_id_returns_zero() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let removed = delete_entities_by_ids(&conn, &[9999])?;
    assert_eq!(removed, 0);
    Ok(())
}

#[test]
fn test_delete_entities_by_ids_removes_multiple() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id1 = upsert_entity(&conn, "global", &new_entity_helper("del-a"))?;
    let id2 = upsert_entity(&conn, "global", &new_entity_helper("del-b"))?;
    let id3 = upsert_entity(&conn, "global", &new_entity_helper("del-c"))?;

    let removed = delete_entities_by_ids(&conn, &[id1, id2])?;
    assert_eq!(removed, 2);

    assert!(find_entity_id(&conn, "global", "del-a")?.is_none());
    assert!(find_entity_id(&conn, "global", "del-b")?.is_none());
    assert!(find_entity_id(&conn, "global", "del-c")?.is_some());
    let _ = id3;
    Ok(())
}

#[test]
fn test_delete_entities_by_ids_also_removes_vec() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let e = new_entity_helper("del-com-vec");
    let entity_id = upsert_entity(&conn, "global", &e)?;
    let emb = embedding_zero();
    upsert_entity_vec(
        &conn,
        entity_id,
        "global",
        EntityType::Project,
        &emb,
        "del-com-vec",
    )?;

    let count_antes: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_embeddings WHERE entity_id = ?1",
        params![entity_id],
        |r| r.get(0),
    )?;
    assert_eq!(count_antes, 1);

    delete_entities_by_ids(&conn, &[entity_id])?;

    let count_depois: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_embeddings WHERE entity_id = ?1",
        params![entity_id],
        |r| r.get(0),
    )?;
    assert_eq!(
        count_depois, 0,
        "entity_embeddings deve ser limpo junto com entities"
    );
    Ok(())
}

// ------------------------------------------------------------------ //
// upsert_relationship / find_relationship
// ------------------------------------------------------------------ //
