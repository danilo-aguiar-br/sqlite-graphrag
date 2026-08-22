//! Candidate scanners that feed the enrich queue (GAP-SG-146).
//!
//! One test per predicate: which memories and entities a given operation
//! considers unfinished.

use super::test_fixtures::open_test_db;
use super::*;

#[test]
fn dry_run_emits_preview_without_calling_llm() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'dry-mem', 'tiny')",
        [],
    )
    .unwrap();

    let results = scan_short_body_memories(&conn, "global", 1000, None, &[], 512).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "dry-mem");
}

#[test]
fn scan_bound_memories_for_augment_requires_names_and_finds_bound() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (id, namespace, name, body) VALUES (1, 'global', 'bound', 'b')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memories (id, namespace, name, body) VALUES (2, 'global', 'unbound', 'b')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO entities (id, namespace, name) VALUES (10, 'global', 'e')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (1, 10)",
        [],
    )
    .unwrap();

    assert!(scan_bound_memories_for_augment(&conn, "global", None, &[]).is_err());

    let names = scan_bound_memories_for_augment(
        &conn,
        "global",
        None,
        &["bound".to_string(), "unbound".to_string()],
    )
    .unwrap();
    assert_eq!(names, vec!["bound".to_string()]);
}

#[test]
fn scan_entities_without_description_excludes_entities_with_description() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type, description) VALUES ('global', 'described-tool', 'tool', 'Has a description already')",
        [],
    )
    .unwrap();

    let results = scan_entities_without_description(&conn, "global", None, &[], false).unwrap();
    assert!(
        results.is_empty(),
        "entity with description must not appear"
    );
}

#[test]
fn scan_entities_without_description_finds_null_description() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type, description) VALUES ('global', 'my-tool', 'tool', NULL)",
        [],
    )
    .unwrap();

    let results = scan_entities_without_description(&conn, "global", None, &[], false).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "my-tool");
}

/// G-PR-7: a NAMED entity is eligible under `--force-redescribe` even when its
/// description matches no low-quality marker.
///
/// This is the targeted-repair case, and it was broken in the layer BELOW the
/// one that was fixed. `entity_description_scan_predicate` already resolved to
/// `1=1` for a named filter, and then `filter_description_candidates` re-applied
/// `is_low_quality_description` to every row the query returned — two gates in
/// series with only the first one opened.
///
/// The description below is deliberately fluent, confident and WRONG, carrying
/// no marker of any kind. That is exactly the class an operator names by hand,
/// and exactly the class that answered `matched: 0` no matter how the command
/// was phrased.
#[test]
fn a_named_entity_is_eligible_even_without_a_low_quality_marker() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type, description) VALUES \
         ('global', 'nomeada', 'person', \
          'Uma engenheira brasileira reconhecida por arquiteturas resilientes.')",
        [],
    )
    .unwrap();

    // Nobody named: the heuristic decides, and it keeps the fluent description.
    let unnamed = scan_entities_without_description(&conn, "global", None, &[], true).unwrap();
    assert!(
        unnamed.is_empty(),
        "without a name filter the quality heuristic must still gate, or every \
         --force-redescribe run would rewrite the whole namespace"
    );

    // Operator named it: the name IS the eligibility rule.
    let named =
        scan_entities_without_description(&conn, "global", None, &["nomeada".to_string()], true)
            .unwrap();
    assert_eq!(
        named.len(),
        1,
        "a named entity must be eligible under --force-redescribe regardless of \
         markers; got {named:?}"
    );
    assert_eq!(named[0].1, "nomeada");
}

/// Naming an entity WITHOUT `--force-redescribe` must not reopen it.
///
/// The write-once rule still holds: `--entity-names` alone selects whom to
/// visit, it does not by itself authorise overwriting a description.
#[test]
fn naming_an_entity_without_force_redescribe_does_not_reopen_it() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type, description) VALUES \
         ('global', 'nomeada', 'person', 'Descrição existente e adequada.')",
        [],
    )
    .unwrap();

    let results =
        scan_entities_without_description(&conn, "global", None, &["nomeada".to_string()], false)
            .unwrap();
    assert!(
        results.is_empty(),
        "without --force-redescribe a named entity that already has a \
         description must stay untouched"
    );
}

#[test]
fn scan_respects_limit() {
    let conn = open_test_db();
    for i in 0..5 {
        conn.execute(
            &format!("INSERT INTO memories (namespace, name, body) VALUES ('global', 'mem-{i}', 'short')"),
            [],
        )
        .unwrap();
    }

    let results = scan_short_body_memories(&conn, "global", 1000, Some(3), &[], 512).unwrap();
    assert_eq!(results.len(), 3, "limit must be respected");
}

#[test]
fn scan_short_body_memories_excludes_long_bodies() {
    let conn = open_test_db();
    let long_body = "a".repeat(1000);
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'long-mem', ?1)",
        rusqlite::params![long_body],
    )
    .unwrap();

    let results = scan_short_body_memories(&conn, "global", 100, None, &[], 512).unwrap();
    assert!(results.is_empty(), "long memory must not appear in scan");
}

#[test]
fn scan_short_body_memories_finds_short_bodies() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'short-mem', 'hi')",
        [],
    )
    .unwrap();

    let results = scan_short_body_memories(&conn, "global", 100, None, &[], 512).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "short-mem");
}

#[test]
fn scan_unbound_memories_excludes_bound_memories() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'bound-mem', 'body')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='bound-mem'", [], |r| {
            r.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO entities (namespace, name) VALUES ('global', 'some-entity')",
        [],
    )
    .unwrap();
    let ent_id: i64 = conn
        .query_row(
            "SELECT id FROM entities WHERE name='some-entity'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
        rusqlite::params![mem_id, ent_id],
    )
    .unwrap();

    let results = scan_unbound_memories(&conn, "global", None, &[], 512).unwrap();
    assert!(results.is_empty(), "bound memory must not appear in scan");
}

#[test]
fn scan_unbound_memories_finds_memories_without_bindings() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global', 'test-mem', 'some body content')",
        [],
    )
    .unwrap();

    let results = scan_unbound_memories(&conn, "global", None, &[], 512).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "test-mem");
}
