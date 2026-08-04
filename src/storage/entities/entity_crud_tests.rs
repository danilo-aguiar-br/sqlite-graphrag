//! Entity upsert, lookup and deletion (GAP-SG-146).
//!
//! The `entities` row itself: creation, idempotence, namespace isolation and
//! cascading deletes.

use super::test_fixtures::*;
use super::*;
use crate::entity_type::EntityType;

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

#[test]
fn test_list_entities_with_namespace() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    upsert_entity(&conn, "le-ns", &new_entity_helper("le-ent-1"))?;
    upsert_entity(&conn, "le-ns", &new_entity_helper("le-ent-2"))?;
    upsert_entity(&conn, "outro-ns", &new_entity_helper("le-ent-3"))?;

    let lista = list_entities(&conn, Some("le-ns"))?;
    assert_eq!(lista.len(), 2);
    assert!(lista.iter().all(|e| e.namespace == "le-ns"));
    Ok(())
}

#[test]
fn test_list_entities_without_namespace_returns_all() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    upsert_entity(&conn, "ns1", &new_entity_helper("all-ent-1"))?;
    upsert_entity(&conn, "ns2", &new_entity_helper("all-ent-2"))?;

    let lista = list_entities(&conn, None)?;
    assert!(lista.len() >= 2);
    Ok(())
}
