//! Traversal invariants over a synthetic entity graph.
//!
//! These 16 invariants used to guard `traverse_from_memories`, a directed walk
//! with no production caller. They now guard the live
//! [`traverse_from_memories_with_hops`] used by recall, hybrid-search and
//! deep-research, plus the capped variant and the shared walk engine.

use super::*;
use rusqlite::params;
use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE memories (
            id INTEGER PRIMARY KEY,
            namespace TEXT NOT NULL,
            deleted_at TEXT
        );
        CREATE TABLE memory_entities (
            memory_id INTEGER NOT NULL,
            entity_id INTEGER NOT NULL
        );
        CREATE TABLE relationships (
            source_id INTEGER NOT NULL,
            target_id INTEGER NOT NULL,
            relation TEXT NOT NULL DEFAULT 'related',
            weight REAL NOT NULL,
            namespace TEXT NOT NULL
        );",
    )
    .unwrap();
    conn
}

fn insert_memory(conn: &Connection, id: i64, namespace: &str, deleted: bool) {
    conn.execute(
        "INSERT INTO memories (id, namespace, deleted_at) VALUES (?1, ?2, ?3)",
        params![
            id,
            namespace,
            if deleted { Some("2024-01-01") } else { None }
        ],
    )
    .unwrap();
}

fn link_memory_entity(conn: &Connection, memory_id: i64, entity_id: i64) {
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
        params![memory_id, entity_id],
    )
    .unwrap();
}

fn insert_relationship(conn: &Connection, src: i64, tgt: i64, weight: f64, ns: &str) {
    conn.execute(
        "INSERT INTO relationships (source_id, target_id, relation, weight, namespace)
         VALUES (?1, ?2, 'related', ?3, ?4)",
        params![src, tgt, weight, ns],
    )
    .unwrap();
}

/// Runs the live traversal and drops the hop counts, for invariants about reach.
fn reached(
    conn: &Connection,
    seeds: &[i64],
    namespace: &str,
    min_weight: f64,
    max_hops: u32,
) -> Vec<i64> {
    let mut ids: Vec<i64> =
        traverse_from_memories_with_hops(conn, seeds, namespace, min_weight, max_hops)
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
    ids.sort_unstable();
    ids
}

// --- edge cases retornando vazio ---

#[test]
fn returns_empty_when_seeds_empty() {
    let conn = setup_db();
    assert!(reached(&conn, &[], "ns", 0.5, 3).is_empty());
}

#[test]
fn returns_empty_when_max_hops_zero() {
    let conn = setup_db();
    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);
    assert!(reached(&conn, &[1], "ns", 0.5, 0).is_empty());
}

#[test]
fn returns_empty_when_seed_has_no_entities() {
    let conn = setup_db();
    insert_memory(&conn, 1, "ns", false);
    // memory exists but has no associated entities
    assert!(reached(&conn, &[1], "ns", 0.5, 3).is_empty());
}

#[test]
fn returns_empty_when_no_relationships() {
    let conn = setup_db();
    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);
    // entity 10 has no relationships
    assert!(reached(&conn, &[1], "ns", 0.5, 3).is_empty());
}

// --- basic happy path ---

#[test]
fn traversal_basic_one_hop() {
    let conn = setup_db();

    // seed: memory 1 com entity 10
    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    // vizinha: entity 20 ligada a memory 2
    insert_memory(&conn, 2, "ns", false);
    link_memory_entity(&conn, 2, 20);

    // relacionamento 10 -> 20
    insert_relationship(&conn, 10, 20, 1.0, "ns");

    let result = traverse_from_memories_with_hops(&conn, &[1], "ns", 0.5, 1).unwrap();
    assert_eq!(result, vec![(2, 1)]);
}

#[test]
fn traversal_two_hops() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    insert_memory(&conn, 2, "ns", false);
    link_memory_entity(&conn, 2, 20);

    insert_memory(&conn, 3, "ns", false);
    link_memory_entity(&conn, 3, 30);

    // cadeia 10 -> 20 -> 30
    insert_relationship(&conn, 10, 20, 1.0, "ns");
    insert_relationship(&conn, 20, 30, 1.0, "ns");

    let result = traverse_from_memories_with_hops(&conn, &[1], "ns", 0.5, 2).unwrap();
    assert_eq!(result, vec![(2, 1), (3, 2)]);
}

#[test]
fn max_hops_limits_depth() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    insert_memory(&conn, 2, "ns", false);
    link_memory_entity(&conn, 2, 20);

    insert_memory(&conn, 3, "ns", false);
    link_memory_entity(&conn, 3, 30);

    insert_relationship(&conn, 10, 20, 1.0, "ns");
    insert_relationship(&conn, 20, 30, 1.0, "ns");

    // with only 1 hop, memory 3 must not appear
    let result = reached(&conn, &[1], "ns", 0.5, 1);
    assert_eq!(result, vec![2]);
    assert!(!result.contains(&3));
}

// --- filtro de peso ---

#[test]
fn relationship_with_weight_below_min_ignored() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    insert_memory(&conn, 2, "ns", false);
    link_memory_entity(&conn, 2, 20);

    // peso 0.3 < min_weight 0.5
    insert_relationship(&conn, 10, 20, 0.3, "ns");

    assert!(reached(&conn, &[1], "ns", 0.5, 3).is_empty());
}

#[test]
fn relationship_with_weight_exactly_at_min_included() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    insert_memory(&conn, 2, "ns", false);
    link_memory_entity(&conn, 2, 20);

    insert_relationship(&conn, 10, 20, 0.5, "ns");

    assert_eq!(reached(&conn, &[1], "ns", 0.5, 1), vec![2]);
}

// --- isolamento de namespace ---

#[test]
fn relationship_from_different_namespace_ignored() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns_a", false);
    link_memory_entity(&conn, 1, 10);

    insert_memory(&conn, 2, "ns_a", false);
    link_memory_entity(&conn, 2, 20);

    // relacionamento no namespace errado
    insert_relationship(&conn, 10, 20, 1.0, "ns_b");

    assert!(reached(&conn, &[1], "ns_a", 0.5, 3).is_empty());
}

// --- exclude seeds from result ---

#[test]
fn seeds_do_not_appear_in_result() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    insert_memory(&conn, 2, "ns", false);
    link_memory_entity(&conn, 2, 20);

    // relacionamento de 20 de volta para 10 (ciclo)
    insert_relationship(&conn, 10, 20, 1.0, "ns");
    insert_relationship(&conn, 20, 10, 1.0, "ns");

    let result = reached(&conn, &[1], "ns", 0.5, 3);
    // memory 1 must not appear even with a cycle
    assert!(!result.contains(&1));
    assert_eq!(result, vec![2]);
}

// --- soft-deleted memories excluded ---

#[test]
fn deleted_memories_not_included() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    // memory 2 foi deletada
    insert_memory(&conn, 2, "ns", true);
    link_memory_entity(&conn, 2, 20);

    insert_relationship(&conn, 10, 20, 1.0, "ns");

    assert!(reached(&conn, &[1], "ns", 0.5, 3).is_empty());
}

// --- multiple seeds ---

#[test]
fn multiple_seeds_merged_in_result() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    insert_memory(&conn, 2, "ns", false);
    link_memory_entity(&conn, 2, 20);

    insert_memory(&conn, 3, "ns", false);
    link_memory_entity(&conn, 3, 30);

    insert_memory(&conn, 4, "ns", false);
    link_memory_entity(&conn, 4, 40);

    insert_relationship(&conn, 10, 30, 1.0, "ns");
    insert_relationship(&conn, 20, 40, 1.0, "ns");

    assert_eq!(reached(&conn, &[1, 2], "ns", 0.5, 1), vec![3, 4]);
}

// --- result deduplication ---

#[test]
fn result_without_duplicates() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);
    link_memory_entity(&conn, 1, 11); // dois seeds na mesma memory

    insert_memory(&conn, 2, "ns", false);
    link_memory_entity(&conn, 2, 20);

    // ambos os seeds apontam para a mesma entity 20
    insert_relationship(&conn, 10, 20, 1.0, "ns");
    insert_relationship(&conn, 11, 20, 1.0, "ns");

    let result = reached(&conn, &[1], "ns", 0.5, 1);
    // memory 2 deve aparecer apenas uma vez
    assert_eq!(result.len(), 1);
    assert_eq!(result, vec![2]);
}

// --- single node ---

#[test]
fn single_node_without_neighbors_returns_empty() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);
    // entity 10 has no outgoing relationships

    assert!(reached(&conn, &[1], "ns", 0.5, 5).is_empty());
}

// --- ciclos no grafo ---

#[test]
fn cycle_does_not_cause_infinite_loop() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    insert_memory(&conn, 2, "ns", false);
    link_memory_entity(&conn, 2, 20);

    insert_memory(&conn, 3, "ns", false);
    link_memory_entity(&conn, 3, 30);

    // triangle 10 -> 20 -> 30 -> 10
    insert_relationship(&conn, 10, 20, 1.0, "ns");
    insert_relationship(&conn, 20, 30, 1.0, "ns");
    insert_relationship(&conn, 30, 10, 1.0, "ns");

    // deve retornar 2 e 3 sem loop infinito
    assert_eq!(reached(&conn, &[1], "ns", 0.5, 10), vec![2, 3]);
}

// --- variante com cap de vizinhos ---

#[test]
fn neighbor_cap_keeps_strongest_edges() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    for (mem, ent, weight) in [(2i64, 20i64, 0.9f64), (3, 30, 0.8), (4, 40, 0.7)] {
        insert_memory(&conn, mem, "ns", false);
        link_memory_entity(&conn, mem, ent);
        insert_relationship(&conn, 10, ent, weight, "ns");
    }

    let capped =
        traverse_from_memories_with_hops_capped(&conn, &[1], "ns", 0.0, 1, Some(2)).unwrap();
    // ORDER BY weight DESC: the two strongest edges survive the cap.
    assert_eq!(capped, vec![(2, 1), (3, 1)]);

    let uncapped =
        traverse_from_memories_with_hops_capped(&conn, &[1], "ns", 0.0, 1, None).unwrap();
    assert_eq!(uncapped.len(), 3);
}

// --- distância mínima, não a primeira encontrada ---

#[test]
fn hop_count_is_minimum_distance() {
    let conn = setup_db();

    insert_memory(&conn, 1, "ns", false);
    link_memory_entity(&conn, 1, 10);

    // alvo alcançável por 1 salto (10 -> 20) e por 3 saltos (10 -> 30 -> 40 -> 20)
    for (mem, ent) in [(2i64, 20i64), (3, 30), (4, 40)] {
        insert_memory(&conn, mem, "ns", false);
        link_memory_entity(&conn, mem, ent);
    }
    insert_relationship(&conn, 10, 30, 1.0, "ns");
    insert_relationship(&conn, 30, 40, 1.0, "ns");
    insert_relationship(&conn, 40, 20, 1.0, "ns");
    insert_relationship(&conn, 10, 20, 1.0, "ns");

    let result = traverse_from_memories_with_hops(&conn, &[1], "ns", 0.5, 3).unwrap();
    let hop_of_2 = result.iter().find(|(id, _)| *id == 2).map(|&(_, h)| h);
    assert_eq!(hop_of_2, Some(1), "BFS deve reportar a distância mínima");
}

// --- motor compartilhado: direção declarada ---

#[test]
fn walk_direction_controls_edge_following() {
    let conn = setup_db();
    insert_relationship(&conn, 10, 20, 1.0, "ns");

    let directed = GraphWalk::directed(0.0, 3)
        .run(&SqlNeighbors::new(&conn, "ns"), &[20])
        .unwrap();
    // 20 has no outgoing edge, so a directed walk reaches nothing.
    assert_eq!(directed.depth.len(), 1);

    let bidirectional = GraphWalk::bidirectional(0.0, 3)
        .run(&SqlNeighbors::new(&conn, "ns"), &[20])
        .unwrap();
    assert_eq!(bidirectional.depth.get(&10), Some(&1));
}
