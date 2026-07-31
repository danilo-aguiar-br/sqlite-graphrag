use super::*;
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

fn insert_memory(conn: &Connection) -> Result<i64, Box<dyn std::error::Error>> {
    conn.execute(
        "INSERT INTO memories (namespace, name, type, description, body, body_hash)
         VALUES ('global', 'test-mem', 'user', 'desc', 'body', 'hash1')",
        [],
    )?;
    Ok(conn.last_insert_rowid())
}

fn new_entity_helper(name: &str) -> NewEntity {
    NewEntity {
        name: name.to_string(),
        entity_type: EntityType::Project,
        description: None,
    }
}

// ------------------------------------------------------------------ //
// upsert_entity
// ------------------------------------------------------------------ //

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

    let encontrada = find_relationship(&conn, id_a, id_b, "depends_on")?;
    let row = encontrada.ok_or("relationship should exist")?;
    assert_eq!(row.source_id, id_a);
    assert_eq!(row.target_id, id_b);
    assert!((row.weight - 0.7).abs() < 1e-9);
    Ok(())
}

#[test]
fn test_find_relationship_missing_returns_none() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let resultado = find_relationship(&conn, 9999, 8888, "uses")?;
    assert!(resultado.is_none());
    Ok(())
}

// ------------------------------------------------------------------ //
// link_memory_entity / link_memory_relationship
// ------------------------------------------------------------------ //

#[test]
fn test_link_memory_entity_idempotent() -> TestResult {
    let (_tmp, conn) = setup_db()?;
    let memory_id = insert_memory(&conn)?;
    let entity_id = upsert_entity(&conn, "global", &new_entity_helper("me-ent"))?;

    link_memory_entity(&conn, memory_id, entity_id)?;
    let resultado = link_memory_entity(&conn, memory_id, entity_id);
    assert!(
        resultado.is_ok(),
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
    let resultado = link_memory_relationship(&conn, memory_id, rel_id);
    assert!(
        resultado.is_ok(),
        "INSERT OR IGNORE must not fail on duplicate"
    );
    Ok(())
}

// ------------------------------------------------------------------ //
// increment_degree / recalculate_degree
// ------------------------------------------------------------------ //

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

// ------------------------------------------------------------------ //
// find_orphan_entity_ids
// ------------------------------------------------------------------ //

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

// ------------------------------------------------------------------ //
// list_entities / list_relationships_by_namespace
// ------------------------------------------------------------------ //

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

// ------------------------------------------------------------------ //
// delete_relationship_by_id / create_or_fetch_relationship
// ------------------------------------------------------------------ //

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

// ------------------------------------------------------------------ //
// serde alias: field "type" accepted as a synonym for "entity_type"
// ------------------------------------------------------------------ //

#[test]
fn accepts_type_field_as_alias() -> TestResult {
    let json = r#"{"name": "X", "type": "concept"}"#;
    let ent: NewEntity = serde_json::from_str(json)?;
    assert_eq!(ent.entity_type, EntityType::Concept);
    Ok(())
}

#[test]
fn accepts_canonical_entity_type_field() -> TestResult {
    let json = r#"{"name": "X", "entity_type": "concept"}"#;
    let ent: NewEntity = serde_json::from_str(json)?;
    assert_eq!(ent.entity_type, EntityType::Concept);
    Ok(())
}

#[test]
fn both_fields_present_yields_duplicate_error() {
    // having both entity_type and type in the same JSON is a duplicate and must fail
    let json = r#"{"name": "X", "entity_type": "concept", "type": "person"}"#;
    let resultado: Result<NewEntity, _> = serde_json::from_str(json);
    assert!(
        resultado.is_err(),
        "both fields in the same JSON are a duplicate"
    );
}

#[test]
fn validate_entity_name_accepts_valid() {
    assert!(validate_entity_name("rust-lang").is_ok());
    assert!(validate_entity_name("sqlite-graphrag").is_ok());
    assert!(validate_entity_name("ab").is_ok());
}

#[test]
fn validate_entity_name_rejects_short() {
    assert!(validate_entity_name("a").is_err());
    assert!(validate_entity_name("").is_err());
}

#[test]
fn validate_entity_name_rejects_newlines() {
    assert!(validate_entity_name("foo\nbar").is_err());
    assert!(validate_entity_name("foo\rbar").is_err());
}

#[test]
fn validate_entity_name_rejects_short_allcaps() {
    assert!(validate_entity_name("RAM").is_err());
    assert!(validate_entity_name("NAO").is_err());
    assert!(validate_entity_name("OK").is_err());
}

#[test]
fn validate_entity_name_accepts_long_allcaps() {
    assert!(validate_entity_name("SQLITE").is_ok());
    assert!(validate_entity_name("GRAPHRAG").is_ok());
}

#[test]
fn validate_entity_name_accepts_mixed_case() {
    assert!(validate_entity_name("FTS5").is_ok()); // 4 chars but has digit
    assert!(validate_entity_name("WAL").is_err()); // 3 chars ALL_CAPS
}

// v1.1.05 Bug 5: pure digit names must be rejected (ghost ID entities).
#[test]
fn validate_entity_name_rejects_purely_numeric() {
    assert!(validate_entity_name("89975").is_err());
    assert!(validate_entity_name("35313").is_err());
    assert!(validate_entity_name("12").is_err());
    // Mixed alphanumeric still OK.
    assert!(validate_entity_name("issue-89975").is_ok());
    assert!(validate_entity_name("v2").is_ok());
}

#[test]
fn entity_name_similarity_prefers_prefix_of_kebab() {
    let s = entity_name_similarity("danilo", "danilo-aguiar-teixeira");
    assert!(s >= 0.90, "expected strong prefix score, got {s}");
    let exact = entity_name_similarity("danilo", "danilo");
    assert!((exact - 1.0).abs() < f64::EPSILON);
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
