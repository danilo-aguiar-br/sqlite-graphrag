//! Bounded BFS expansion from seed memories over the entity graph.
//!
//! The `recall`, `hybrid-search` and `deep-research` neighbourhood queries: walk
//! `memory_entities` and `relationships` outward from a seed set, honouring a hop
//! limit, a minimum edge weight and an optional per-hop fan-out cap.
//!
//! The walk itself lives in [`super::walk`]; this module only maps memories to
//! their entities and back.

use super::walk::{GraphWalk, SqlNeighbors};
use crate::errors::AppError;
use rusqlite::{params, Connection};
use std::collections::HashSet;

/// Collects the distinct entity ids attached to the given memories.
fn seed_entities_of(conn: &Connection, seed_memory_ids: &[i64]) -> Result<Vec<i64>, AppError> {
    let mut seed_entities: Vec<i64> = Vec::with_capacity(seed_memory_ids.len());
    for &mem_id in seed_memory_ids {
        let mut stmt =
            conn.prepare_cached("SELECT entity_id FROM memory_entities WHERE memory_id = ?1")?;
        let ids: Vec<i64> = stmt
            .query_map(params![mem_id], |r| r.get(0))?
            .filter_map(std::result::Result::ok)
            .collect();
        seed_entities.extend(ids);
    }
    seed_entities.sort_unstable();
    seed_entities.dedup();
    Ok(seed_entities)
}

/// BFS graph traversal returning the hop distance for each reached memory.
///
/// Returns `(memory_id, hop_count)` for every live memory reachable through
/// entity and relationship edges, excluding the seed memories themselves.
/// `hop_count` is the minimum BFS depth at which the memory's entity was
/// discovered, starting at 1 for direct neighbours of the seed entities.
/// The walk follows `source_id -> target_id` only and skips edges whose
/// `weight` is below `min_weight` or whose `namespace` differs.
///
/// # Errors
///
/// Propagates [`AppError::Database`] (exit 10) on SQLite query failures.
///
/// # Examples
///
/// ```
/// use rusqlite::Connection;
/// use sqlite_graphrag::graph::traverse_from_memories_with_hops;
///
/// // Empty seed list returns immediately without querying the database.
/// let conn = Connection::open_in_memory().unwrap();
/// let hops = traverse_from_memories_with_hops(&conn, &[], "global", 0.5, 3).unwrap();
/// assert!(hops.is_empty());
/// ```
///
/// ```
/// use rusqlite::Connection;
/// use sqlite_graphrag::graph::traverse_from_memories_with_hops;
///
/// // max_hops == 0 returns immediately without traversal.
/// let conn = Connection::open_in_memory().unwrap();
/// let hops = traverse_from_memories_with_hops(&conn, &[1, 2], "global", 0.5, 0).unwrap();
/// assert!(hops.is_empty());
/// ```
pub fn traverse_from_memories_with_hops(
    conn: &Connection,
    seed_memory_ids: &[i64],
    namespace: &str,
    min_weight: f64,
    max_hops: u32,
) -> Result<Vec<(i64, u32)>, AppError> {
    traverse_from_memories_with_hops_capped(
        conn,
        seed_memory_ids,
        namespace,
        min_weight,
        max_hops,
        None,
    )
}

/// Extended variant that accepts an optional neighbour cap per hop.
///
/// Pass `max_neighbors_per_hop = Some(k)` to prune each entity's expansion to
/// its top-`k` unvisited neighbours by edge weight, limiting combinatorial
/// blow-up in dense graphs. `None` is equivalent to
/// [`traverse_from_memories_with_hops`].
///
/// # Errors
///
/// Propagates [`AppError::Database`] (exit 10) on SQLite query failures.
pub fn traverse_from_memories_with_hops_capped(
    conn: &Connection,
    seed_memory_ids: &[i64],
    namespace: &str,
    min_weight: f64,
    max_hops: u32,
    max_neighbors_per_hop: Option<usize>,
) -> Result<Vec<(i64, u32)>, AppError> {
    if seed_memory_ids.is_empty() || max_hops == 0 {
        return Ok(vec![]);
    }

    let seed_entities = seed_entities_of(conn, seed_memory_ids)?;
    if seed_entities.is_empty() {
        return Ok(vec![]);
    }

    let walk = GraphWalk::directed(min_weight, max_hops).with_neighbor_cap(max_neighbors_per_hop);
    let outcome = walk.run(&SqlNeighbors::new(conn, namespace), &seed_entities)?;

    // Visit entities nearest-first so each memory keeps its minimum hop count.
    let seed_entity_set: HashSet<i64> = seed_entities.iter().copied().collect();
    let mut discovered: Vec<(i64, u32)> = outcome
        .depth
        .into_iter()
        .filter(|(id, _)| !seed_entity_set.contains(id))
        .collect();
    discovered.sort_unstable_by_key(|&(id, hop)| (hop, id));

    let seed_set: HashSet<i64> = seed_memory_ids.iter().copied().collect();
    let mut result: Vec<(i64, u32)> = Vec::with_capacity(discovered.len());
    let mut seen_memories: HashSet<i64> = HashSet::with_capacity(discovered.len());

    for (entity_id, hop) in discovered {
        let mut stmt = conn.prepare_cached(
            "SELECT DISTINCT me.memory_id
             FROM memory_entities me
             JOIN memories m ON m.id = me.memory_id
             WHERE me.entity_id = ?1 AND m.deleted_at IS NULL",
        )?;
        let mem_ids: Vec<i64> = stmt
            .query_map(params![entity_id], |r| r.get(0))?
            .filter_map(std::result::Result::ok)
            .filter(|id| !seed_set.contains(id) && !seen_memories.contains(id))
            .collect();

        for mem_id in mem_ids {
            seen_memories.insert(mem_id);
            result.push((mem_id, hop));
        }
    }

    result.sort_unstable_by_key(|&(id, _)| id);
    Ok(result)
}
