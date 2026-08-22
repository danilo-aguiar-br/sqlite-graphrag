//! Relationships and degree accounting (GAP-SG-146).
//!
//! Edge creation and lookup, memory bindings, degree counters and orphan
//! detection.

use super::test_fixtures::*;
use super::*;

#[test]
fn test_upsert_relationship_creates_new() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id_a = upsert_entity(&conn, "global", &new_entity_helper("rel-a"))?;
    let id_b = upsert_entity(&conn, "global", &new_entity_helper("rel-b"))?;

    let rel = NewRelationship {
        source: "rel-a".to_string(),
        target: "rel-b".to_string(),
        relation: "uses".to_string(),
        strength: 0.8,
        description: None,
    };
    let rel_id = upsert_relationship(&conn, "global", id_a, id_b, &rel)?;
    assert!(rel_id > 0);
    Ok(())
}

#[test]
fn test_upsert_relationship_idempotent() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id_a = upsert_entity(&conn, "global", &new_entity_helper("idem-a"))?;
    let id_b = upsert_entity(&conn, "global", &new_entity_helper("idem-b"))?;

    let rel = NewRelationship {
        source: "idem-a".to_string(),
        target: "idem-b".to_string(),
        relation: "uses".to_string(),
        strength: 0.5,
        description: None,
    };
    let id1 = upsert_relationship(&conn, "global", id_a, id_b, &rel)?;
    let id2 = upsert_relationship(&conn, "global", id_a, id_b, &rel)?;
    assert_eq!(id1, id2);
    Ok(())
}

#[test]
fn test_find_relationship_existing() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id_a = upsert_entity(&conn, "global", &new_entity_helper("fr-a"))?;
    let id_b = upsert_entity(&conn, "global", &new_entity_helper("fr-b"))?;

    let rel = NewRelationship {
        source: "fr-a".to_string(),
        target: "fr-b".to_string(),
        relation: "depends_on".to_string(),
        strength: 0.7,
        description: None,
    };
    upsert_relationship(&conn, "global", id_a, id_b, &rel)?;

    // v1.2.8: the row is stored as `depends-on` even though the caller wrote
    // `depends_on`, because `upsert_relationship` canonicalises at the
    // persistence boundary. That is the fix under test: the caller may use
    // either spelling and the store holds exactly one.
    assert!(
        find_relationship(&conn, id_a, id_b, "depends_on")?.is_none(),
        "the divergent spelling must NOT reach the table; if it does, the \
         boundary stopped canonicalising and the split vocabulary is back"
    );
    let encontrada = find_relationship(&conn, id_a, id_b, "depends-on")?;
    let row = encontrada.ok_or("relationship should exist")?;
    assert_eq!(row.source_id, id_a);
    assert_eq!(row.target_id, id_b);
    assert!((row.weight - 0.7).abs() < 1e-9);
    Ok(())
}

#[test]
fn test_find_relationship_missing_returns_none() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let result = find_relationship(&conn, 9999, 8888, "uses")?;
    assert!(result.is_none());
    Ok(())
}

#[test]
fn test_link_memory_entity_idempotent() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let memory_id = insert_memory(&conn)?;
    let entity_id = upsert_entity(&conn, "global", &new_entity_helper("me-ent"))?;

    link_memory_entity(&conn, memory_id, entity_id)?;
    let result = link_memory_entity(&conn, memory_id, entity_id);
    assert!(
        result.is_ok(),
        "INSERT OR IGNORE must not fail on duplicate"
    );
    Ok(())
}

#[test]
fn test_link_memory_relationship_idempotent() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let memory_id = insert_memory(&conn)?;
    let id_a = upsert_entity(&conn, "global", &new_entity_helper("mr-a"))?;
    let id_b = upsert_entity(&conn, "global", &new_entity_helper("mr-b"))?;

    let rel = NewRelationship {
        source: "mr-a".to_string(),
        target: "mr-b".to_string(),
        relation: "uses".to_string(),
        strength: 0.5,
        description: None,
    };
    let rel_id = upsert_relationship(&conn, "global", id_a, id_b, &rel)?;

    link_memory_relationship(&conn, memory_id, rel_id)?;
    let result = link_memory_relationship(&conn, memory_id, rel_id);
    assert!(
        result.is_ok(),
        "INSERT OR IGNORE must not fail on duplicate"
    );
    Ok(())
}

#[test]
fn test_increment_degree_increases_counter() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let entity_id = upsert_entity(&conn, "global", &new_entity_helper("grau-ent"))?;

    increment_degree(&conn, entity_id)?;
    increment_degree(&conn, entity_id)?;

    let degree: i64 = conn.query_row(
        "SELECT degree FROM entities WHERE id = ?1",
        params![entity_id],
        |r| r.get(0),
    )?;
    assert_eq!(degree, 2);
    Ok(())
}

#[test]
fn test_recalculate_degree_reflects_actual_relations() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id_a = upsert_entity(&conn, "global", &new_entity_helper("rc-a"))?;
    let id_b = upsert_entity(&conn, "global", &new_entity_helper("rc-b"))?;
    let id_c = upsert_entity(&conn, "global", &new_entity_helper("rc-c"))?;

    let rel1 = NewRelationship {
        source: "rc-a".to_string(),
        target: "rc-b".to_string(),
        relation: "uses".to_string(),
        strength: 0.5,
        description: None,
    };
    let rel2 = NewRelationship {
        source: "rc-c".to_string(),
        target: "rc-a".to_string(),
        relation: "depends_on".to_string(),
        strength: 0.5,
        description: None,
    };
    upsert_relationship(&conn, "global", id_a, id_b, &rel1)?;
    upsert_relationship(&conn, "global", id_c, id_a, &rel2)?;

    recalculate_degree(&conn, id_a)?;

    let degree: i64 = conn.query_row(
        "SELECT degree FROM entities WHERE id = ?1",
        params![id_a],
        |r| r.get(0),
    )?;
    assert_eq!(
        degree, 2,
        "rc-a appears in two relationships (source+target)"
    );
    Ok(())
}

#[test]
fn test_find_orphan_entity_ids_without_orphans() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let memory_id = insert_memory(&conn)?;
    let entity_id = upsert_entity(&conn, "global", &new_entity_helper("nao-orfa"))?;
    link_memory_entity(&conn, memory_id, entity_id)?;

    let orfas = find_orphan_entity_ids(&conn, Some("global"))?;
    assert!(!orfas.contains(&entity_id));
    Ok(())
}

#[test]
fn test_find_orphan_entity_ids_detects_orphans() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let entity_id = upsert_entity(&conn, "global", &new_entity_helper("sim-orfa"))?;

    let orfas = find_orphan_entity_ids(&conn, Some("global"))?;
    assert!(orfas.contains(&entity_id));
    Ok(())
}

#[test]
fn test_find_orphan_entity_ids_without_namespace_returns_all() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id1 = upsert_entity(&conn, "ns-a", &new_entity_helper("orfa-a"))?;
    let id2 = upsert_entity(&conn, "ns-b", &new_entity_helper("orfa-b"))?;

    let orfas = find_orphan_entity_ids(&conn, None)?;
    assert!(orfas.contains(&id1));
    assert!(orfas.contains(&id2));
    Ok(())
}

#[test]
fn test_list_relationships_by_namespace_filters_correctly() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id_a = upsert_entity(&conn, "rel-ns", &new_entity_helper("lr-a"))?;
    let id_b = upsert_entity(&conn, "rel-ns", &new_entity_helper("lr-b"))?;

    let rel = NewRelationship {
        source: "lr-a".to_string(),
        target: "lr-b".to_string(),
        relation: "uses".to_string(),
        strength: 0.5,
        description: None,
    };
    upsert_relationship(&conn, "rel-ns", id_a, id_b, &rel)?;

    let lista = list_relationships_by_namespace(&conn, Some("rel-ns"))?;
    assert!(!lista.is_empty());
    assert!(lista.iter().all(|r| r.namespace == "rel-ns"));
    Ok(())
}

#[test]
fn test_delete_relationship_by_id_removes_relation() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id_a = upsert_entity(&conn, "global", &new_entity_helper("dr-a"))?;
    let id_b = upsert_entity(&conn, "global", &new_entity_helper("dr-b"))?;

    let rel = NewRelationship {
        source: "dr-a".to_string(),
        target: "dr-b".to_string(),
        relation: "uses".to_string(),
        strength: 0.5,
        description: None,
    };
    let rel_id = upsert_relationship(&conn, "global", id_a, id_b, &rel)?;

    delete_relationship_by_id(&conn, rel_id)?;

    let encontrada = find_relationship(&conn, id_a, id_b, "uses")?;
    assert!(encontrada.is_none(), "relationship must have been removed");
    Ok(())
}

#[test]
fn test_create_or_fetch_relationship_creates_new() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id_a = upsert_entity(&conn, "global", &new_entity_helper("cf-a"))?;
    let id_b = upsert_entity(&conn, "global", &new_entity_helper("cf-b"))?;

    let (rel_id, created) =
        create_or_fetch_relationship(&conn, "global", id_a, id_b, "uses", 0.5, None)?;
    assert!(rel_id > 0);
    assert!(created);
    Ok(())
}

#[test]
fn test_create_or_fetch_relationship_returns_existing() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let id_a = upsert_entity(&conn, "global", &new_entity_helper("cf2-a"))?;
    let id_b = upsert_entity(&conn, "global", &new_entity_helper("cf2-b"))?;

    create_or_fetch_relationship(&conn, "global", id_a, id_b, "uses", 0.5, None)?;
    let (_, created) =
        create_or_fetch_relationship(&conn, "global", id_a, id_b, "uses", 0.5, None)?;
    assert!(
        !created,
        "second call must return the existing relationship"
    );
    Ok(())
}

// GAP-SG-52: unlink_memory_entity removes exactly the targeted binding.
#[test]
fn test_unlink_memory_entity_removes_single_binding() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let memory_id = insert_memory(&conn)?;
    let e1 = upsert_entity(&conn, "global", &new_entity_helper("entidade-um"))?;
    let e2 = upsert_entity(&conn, "global", &new_entity_helper("entidade-dois"))?;
    link_memory_entity(&conn, memory_id, e1)?;
    link_memory_entity(&conn, memory_id, e2)?;

    let removed = unlink_memory_entity(&conn, memory_id, e1)?;
    assert_eq!(removed, 1);

    // e1 binding gone, e2 binding kept.
    let remaining: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory_entities WHERE memory_id = ?1",
        params![memory_id],
        |r| r.get(0),
    )?;
    assert_eq!(remaining, 1);

    // Idempotent: a second unlink of the same pair removes nothing.
    assert_eq!(unlink_memory_entity(&conn, memory_id, e1)?, 0);
    Ok(())
}

// GAP-SG-51: clear_memory_graph_bindings zeroes every binding for a memory.
#[test]
fn test_clear_memory_graph_bindings_clears_all() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let memory_id = insert_memory(&conn)?;
    let e1 = upsert_entity(&conn, "global", &new_entity_helper("alpha-node"))?;
    let e2 = upsert_entity(&conn, "global", &new_entity_helper("beta-node"))?;
    link_memory_entity(&conn, memory_id, e1)?;
    link_memory_entity(&conn, memory_id, e2)?;
    let rel = NewRelationship {
        source: "alpha-node".to_string(),
        target: "beta-node".to_string(),
        relation: "related".to_string(),
        strength: 0.5,
        description: None,
    };
    let rel_id = upsert_relationship(&conn, "global", e1, e2, &rel)?;
    link_memory_relationship(&conn, memory_id, rel_id)?;

    let (e_removed, r_removed) = clear_memory_graph_bindings(&conn, memory_id)?;
    assert_eq!(e_removed, 2);
    assert_eq!(r_removed, 1);

    let ent_left: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory_entities WHERE memory_id = ?1",
        params![memory_id],
        |r| r.get(0),
    )?;
    let rel_left: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory_relationships WHERE memory_id = ?1",
        params![memory_id],
        |r| r.get(0),
    )?;
    assert_eq!(ent_left, 0);
    assert_eq!(rel_left, 0);
    Ok(())
}
