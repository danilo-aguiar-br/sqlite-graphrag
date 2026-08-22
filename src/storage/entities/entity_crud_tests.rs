//! Entity upsert, lookup and deletion (GAP-SG-146).
//!
//! The `entities` row itself: creation, idempotence, namespace isolation and
//! cascading deletes.

use super::test_fixtures::*;
use super::*;

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
        entity_type: "tool".to_string(),
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
    upsert_entity_vec(&conn, entity_id, "global", "project", &emb, "del-com-vec")?;

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

/// A committed type survives extraction; only the generic one may be refined.
///
/// This is the D-03 regression gate. `upsert_entity` writes
/// `type = excluded.type` unconditionally, and the enrichment worker runs after
/// EVERY successful write, so a `person` declared through `remember
/// --graph-stdin` was replaced by whatever the model guessed minutes later —
/// with no envelope, no warning and no way to notice except by querying the
/// graph and being surprised. Measured on a live corpus: an area of a company
/// (`equipe-suporte`) stored as `person`.
#[test]
fn enrich_path_never_overwrites_a_committed_entity_type() -> Result<(), Box<dyn std::error::Error>>
{
    let (_tmp, conn) = setup_db()?;

    // A human write commits `person`.
    let human = NewEntity {
        name: "equipe-suporte".to_string(),
        entity_type: "person".to_string(),
        description: None,
    };
    let id = upsert_entity(&conn, "global", &human)?;

    // Extraction guesses something else and must NOT win.
    let guess = NewEntity {
        name: "equipe-suporte".to_string(),
        entity_type: "organization".to_string(),
        description: Some("guessed".to_string()),
    };
    let same_id = upsert_entity_preserving_type(&conn, "global", &guess)?;
    assert_eq!(same_id, id, "must address the same row");

    let stored: String = conn.query_row(
        "SELECT type FROM entities WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )?;
    assert_eq!(
        stored, "person",
        "extraction overwrote a committed type; the enrich worker is once again \
         free to re-type whatever the caller declared"
    );

    // The description is still refreshed — withholding the type must not
    // withhold everything, or extraction stops being useful at all.
    let desc: Option<String> = conn.query_row(
        "SELECT description FROM entities WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )?;
    assert_eq!(desc.as_deref(), Some("guessed"));

    // The other half of the asymmetry, asserted here so this gate cannot pass
    // vacuously: `upsert_entity` — the HUMAN path — still overwrites. If both
    // functions behaved the same, the test above would hold for the wrong
    // reason and the defect could return unnoticed.
    let human_correction = NewEntity {
        name: "equipe-suporte".to_string(),
        entity_type: "organization".to_string(),
        description: None,
    };
    upsert_entity(&conn, "global", &human_correction)?;
    let after_human: String = conn.query_row(
        "SELECT type FROM entities WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )?;
    assert_eq!(
        after_human, "organization",
        "the human write path must stay authoritative; if it does not, \
         `remember --graph-file` can no longer correct a wrong type"
    );
    Ok(())
}

/// The generic type carries no commitment, so extraction may refine it.
///
/// Without this, an entity auto-created by `link --create-missing` (which
/// defaults to `concept`) would be frozen as `concept` forever and the
/// enrichment path would lose its whole purpose.
#[test]
fn enrich_path_refines_the_generic_type() -> Result<(), Box<dyn std::error::Error>> {
    let (_tmp, conn) = setup_db()?;
    let generic = NewEntity {
        name: "acme-corp".to_string(),
        entity_type: "concept".to_string(),
        description: None,
    };
    let id = upsert_entity(&conn, "global", &generic)?;

    let refined = NewEntity {
        name: "acme-corp".to_string(),
        entity_type: "organization".to_string(),
        description: None,
    };
    upsert_entity_preserving_type(&conn, "global", &refined)?;

    let stored: String = conn.query_row(
        "SELECT type FROM entities WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )?;
    assert_eq!(stored, "organization", "concept must stay refinable");
    Ok(())
}

/// A brand-new entity takes the type extraction gives it.
#[test]
fn enrich_path_types_a_new_entity() -> Result<(), Box<dyn std::error::Error>> {
    let (_tmp, conn) = setup_db()?;
    let fresh = NewEntity {
        name: "novo-no".to_string(),
        entity_type: "incident".to_string(),
        description: None,
    };
    let id = upsert_entity_preserving_type(&conn, "global", &fresh)?;
    let stored: String = conn.query_row(
        "SELECT type FROM entities WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )?;
    assert_eq!(stored, "incident");
    Ok(())
}
