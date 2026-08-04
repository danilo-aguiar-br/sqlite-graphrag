//! Predecessor-tracking BFS used to reconstruct evidence chains.
//!
//! Unlike [`super::traverse`], this records HOW each entity was reached, so a
//! caller can walk the path back to its seed. The walk itself is the shared
//! [`super::walk`] engine; this module only reshapes its arrival map.

use super::walk::{GraphWalk, SqlNeighbors};
use crate::errors::AppError;
use rusqlite::Connection;

/// Depth map from BFS: entity_id → hop distance from seeds.
pub type EntityDepthMap = std::collections::HashMap<i64, u32>;

/// Predecessor map from BFS: entity_id → (parent_entity_id, relation_type, edge_weight).
///
/// Enables path reconstruction from any discovered entity back to a seed.
pub type PredecessorMap = std::collections::HashMap<i64, (i64, String, f64)>;

/// BFS that also returns a predecessor map for path reconstruction.
///
/// Used by `deep-research` to reconstruct directed evidence chains from
/// discovered entities back to their seeds.
///
/// Returns `(entity_depth, predecessor)` where:
/// - `entity_depth`: minimum depth of each reached entity (0 = seed).
/// - `predecessor`: the BFS tree edge that first reached each non-seed entity.
///
/// When `max_neighbors_per_hop` is `Some(k)`, only the top-`k` unvisited
/// neighbours by `weight DESC` are followed at each entity expansion.
///
/// # Errors
///
/// Propagates [`AppError::Database`] (exit 10) on SQLite query failures.
pub fn bfs_with_predecessors(
    conn: &Connection,
    seed_entity_ids: &[i64],
    namespace: &str,
    min_weight: f64,
    max_hops: u32,
    max_neighbors_per_hop: Option<usize>,
) -> Result<(EntityDepthMap, PredecessorMap), AppError> {
    let walk = GraphWalk::directed(min_weight, max_hops).with_neighbor_cap(max_neighbors_per_hop);
    let outcome = walk.run(&SqlNeighbors::new(conn, namespace), seed_entity_ids)?;

    let predecessor: PredecessorMap = outcome
        .arrival
        .into_iter()
        .map(|(id, edge)| (id, (edge.from_id, edge.relation, edge.weight)))
        .collect();

    Ok((outcome.depth, predecessor))
}
