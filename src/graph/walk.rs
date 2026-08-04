//! Single bounded-BFS engine behind every graph traversal in the codebase.
//!
//! Four traversals used to exist side by side — directed with a weight floor,
//! bidirectional with a weight floor, bidirectional in memory without one, and
//! a predecessor-tracking variant — each with its own frontier handling and its
//! own drift. They now all declare their parameters through [`GraphWalk`] and
//! share this driver, so hop distance means the same thing everywhere.
//!
//! The driver is a strict FIFO breadth-first search: the depth recorded for an
//! entity is its *minimum* distance from the seed set. A LIFO frontier would
//! silently turn the walk into a depth-first search and report distances that
//! are not distances.

use crate::errors::AppError;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet, VecDeque};

/// Which way edges are followed during the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkDirection {
    /// Follow `source_id -> target_id` only.
    Directed,
    /// Follow edges both ways, as users reason about "related".
    Bidirectional,
}

/// The edge through which an entity was reached.
#[derive(Debug, Clone)]
pub struct EdgeArrival {
    /// Entity the edge was followed *from*.
    pub from_id: i64,
    /// Entity the edge was followed *to* (the discovered neighbour).
    pub neighbor_id: i64,
    /// `entities.name` of the edge's `source_id`, when the source was queried with names.
    pub source_name: Option<String>,
    /// `entities.name` of the edge's `target_id`, when the source was queried with names.
    pub target_name: Option<String>,
    /// Relation label carried by the edge.
    pub relation: String,
    /// Edge weight.
    pub weight: f64,
    /// `true` when the edge was traversed backwards (`target_id -> source_id`).
    pub inbound: bool,
}

/// Traversal parameters. Every caller states its own explicitly.
#[derive(Debug, Clone)]
pub struct GraphWalk {
    /// Directed or bidirectional edge following.
    pub direction: WalkDirection,
    /// Minimum edge weight; `None` follows every edge regardless of weight.
    pub weight_floor: Option<f64>,
    /// Maximum hop distance from the seeds.
    pub max_hops: u32,
    /// Keep only the first `k` unvisited neighbours of each expansion.
    pub max_neighbors_per_hop: Option<usize>,
    /// Follow only edges carrying this relation label.
    pub relation_filter: Option<String>,
}

impl GraphWalk {
    /// Directed walk with a weight floor — the shape used by recall, hybrid-search
    /// and deep-research.
    #[must_use]
    pub fn directed(min_weight: f64, max_hops: u32) -> Self {
        Self {
            direction: WalkDirection::Directed,
            weight_floor: Some(min_weight),
            max_hops,
            max_neighbors_per_hop: None,
            relation_filter: None,
        }
    }

    /// Bidirectional walk with a weight floor — the shape used by `related`.
    #[must_use]
    pub fn bidirectional(min_weight: f64, max_hops: u32) -> Self {
        Self {
            direction: WalkDirection::Bidirectional,
            weight_floor: Some(min_weight),
            max_hops,
            max_neighbors_per_hop: None,
            relation_filter: None,
        }
    }

    /// Sets the per-expansion neighbour cap.
    #[must_use]
    pub fn with_neighbor_cap(mut self, cap: Option<usize>) -> Self {
        self.max_neighbors_per_hop = cap;
        self
    }

    /// Restricts the walk to a single relation label.
    #[must_use]
    pub fn with_relation_filter(mut self, relation: Option<String>) -> Self {
        self.relation_filter = relation;
        self
    }

    /// Runs the walk, discarding per-edge observations.
    ///
    /// # Errors
    ///
    /// Propagates [`AppError::Database`] (exit 10) on SQLite query failures.
    pub fn run<S: NeighborSource>(
        &self,
        source: &S,
        seed_entity_ids: &[i64],
    ) -> Result<WalkOutcome, AppError> {
        self.run_observed(source, seed_entity_ids, |_, _| {})
    }

    /// Runs the walk, invoking `on_edge(edge, depth_of_neighbour)` for **every**
    /// edge examined — including edges that lead back to an already-visited
    /// entity. Callers that render an edge list (`graph traverse`) need those;
    /// callers that only need reachable entities ignore them.
    ///
    /// # Errors
    ///
    /// Propagates [`AppError::Database`] (exit 10) on SQLite query failures.
    pub fn run_observed<S, F>(
        &self,
        source: &S,
        seed_entity_ids: &[i64],
        mut on_edge: F,
    ) -> Result<WalkOutcome, AppError>
    where
        S: NeighborSource,
        F: FnMut(&EdgeArrival, u32),
    {
        let mut depth: HashMap<i64, u32> = seed_entity_ids.iter().map(|&id| (id, 0)).collect();
        let mut arrival: HashMap<i64, EdgeArrival> = HashMap::new();
        let mut expanded: HashSet<i64> = HashSet::with_capacity(depth.len());
        let mut queue: VecDeque<i64> = seed_entity_ids.iter().copied().collect();

        // FIFO: the first time an entity is dequeued it carries its minimum depth.
        while let Some(current) = queue.pop_front() {
            let current_depth = depth.get(&current).copied().unwrap_or(0);
            if current_depth >= self.max_hops || !expanded.insert(current) {
                continue;
            }
            let next_depth = current_depth + 1;

            let neighbors = source.neighbors(current, self)?;

            // Cap counts only unvisited candidates, matching the pre-unification
            // behaviour of the capped deep-research walk.
            let mut admitted = 0usize;
            for edge in neighbors {
                on_edge(&edge, next_depth);

                if depth.contains_key(&edge.neighbor_id) {
                    continue;
                }
                if let Some(cap) = self.max_neighbors_per_hop {
                    if admitted >= cap {
                        continue;
                    }
                }
                admitted += 1;
                depth.insert(edge.neighbor_id, next_depth);
                queue.push_back(edge.neighbor_id);
                arrival.insert(edge.neighbor_id, edge);
            }
        }

        Ok(WalkOutcome { depth, arrival })
    }
}

/// Result of a walk.
pub struct WalkOutcome {
    /// entity_id → minimum hop distance from the seed set (seeds map to 0).
    pub depth: HashMap<i64, u32>,
    /// entity_id → the edge that first reached it. Seeds are absent.
    pub arrival: HashMap<i64, EdgeArrival>,
}

/// Supplies the neighbours of an entity to the walk driver.
pub trait NeighborSource {
    /// Returns the neighbours of `entity_id` honouring `walk`'s direction,
    /// weight floor and relation filter.
    ///
    /// # Errors
    ///
    /// Propagates [`AppError::Database`] (exit 10) on SQLite query failures.
    fn neighbors(&self, entity_id: i64, walk: &GraphWalk) -> Result<Vec<EdgeArrival>, AppError>;
}

/// Neighbours read straight from the `relationships` table.
pub struct SqlNeighbors<'a> {
    conn: &'a Connection,
    namespace: &'a str,
    with_names: bool,
}

impl<'a> SqlNeighbors<'a> {
    /// Reads neighbours without joining `entities`; edge names stay `None`.
    #[must_use]
    pub fn new(conn: &'a Connection, namespace: &'a str) -> Self {
        Self {
            conn,
            namespace,
            with_names: false,
        }
    }

    /// Reads neighbours joining `entities` so edge endpoint names are populated.
    #[must_use]
    pub fn with_names(conn: &'a Connection, namespace: &'a str) -> Self {
        Self {
            conn,
            namespace,
            with_names: true,
        }
    }

    fn query(
        &self,
        entity_id: i64,
        walk: &GraphWalk,
        inbound: bool,
    ) -> Result<Vec<EdgeArrival>, AppError> {
        let pivot = if inbound { "target_id" } else { "source_id" };
        let reached = if inbound { "source_id" } else { "target_id" };

        let mut sql = if self.with_names {
            format!(
                "SELECT r.{reached}, se.name, te.name, r.relation, r.weight
                 FROM relationships r
                 JOIN entities se ON se.id = r.source_id
                 JOIN entities te ON te.id = r.target_id
                 WHERE r.{pivot} = ?1 AND r.weight >= ?2 AND r.namespace = ?3"
            )
        } else {
            format!(
                "SELECT r.{reached}, r.relation, r.weight FROM relationships r
                 WHERE r.{pivot} = ?1 AND r.weight >= ?2 AND r.namespace = ?3"
            )
        };
        if walk.relation_filter.is_some() {
            sql.push_str(" AND r.relation = ?4");
        }
        // A directed walk prunes by weight, so the strongest edges must come
        // first for `max_neighbors_per_hop` to keep the strongest ones.
        if walk.direction == WalkDirection::Directed {
            sql.push_str(" ORDER BY r.weight DESC");
        }

        let floor = walk.weight_floor.unwrap_or(f64::NEG_INFINITY);
        let mut stmt = self.conn.prepare_cached(&sql)?;

        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<EdgeArrival> {
            let (neighbor_id, source_name, target_name, relation, weight) = if self.with_names {
                (
                    row.get::<_, i64>(0)?,
                    Some(row.get::<_, String>(1)?),
                    Some(row.get::<_, String>(2)?),
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4)?,
                )
            } else {
                (
                    row.get::<_, i64>(0)?,
                    None,
                    None,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                )
            };
            Ok(EdgeArrival {
                from_id: entity_id,
                neighbor_id,
                source_name,
                target_name,
                relation,
                weight,
                inbound,
            })
        };

        let rows = match walk.relation_filter.as_deref() {
            Some(rel) => stmt
                .query_map(params![entity_id, floor, self.namespace, rel], map_row)?
                .filter_map(std::result::Result::ok)
                .collect(),
            None => stmt
                .query_map(params![entity_id, floor, self.namespace], map_row)?
                .filter_map(std::result::Result::ok)
                .collect(),
        };
        Ok(rows)
    }
}

impl NeighborSource for SqlNeighbors<'_> {
    fn neighbors(&self, entity_id: i64, walk: &GraphWalk) -> Result<Vec<EdgeArrival>, AppError> {
        let mut out = self.query(entity_id, walk, false)?;
        if walk.direction == WalkDirection::Bidirectional {
            out.extend(self.query(entity_id, walk, true)?);
        }
        Ok(out)
    }
}

/// A relationship already loaded in memory.
#[derive(Debug, Clone)]
pub struct MemoryEdge {
    /// Edge source entity id.
    pub source_id: i64,
    /// Edge target entity id.
    pub target_id: i64,
    /// Relation label.
    pub relation: String,
    /// Edge weight.
    pub weight: f64,
}

/// Neighbours resolved against an in-memory edge list.
///
/// `graph export`/`graph traverse` already load the whole namespace to render
/// nodes and edges, so re-querying SQLite per hop would be pure waste.
pub struct InMemoryNeighbors<'a> {
    edges: &'a [MemoryEdge],
    id_to_name: &'a HashMap<i64, String>,
}

impl<'a> InMemoryNeighbors<'a> {
    /// Builds a source over a preloaded edge list and its id→name index.
    #[must_use]
    pub fn new(edges: &'a [MemoryEdge], id_to_name: &'a HashMap<i64, String>) -> Self {
        Self { edges, id_to_name }
    }
}

impl NeighborSource for InMemoryNeighbors<'_> {
    fn neighbors(&self, entity_id: i64, walk: &GraphWalk) -> Result<Vec<EdgeArrival>, AppError> {
        let floor = walk.weight_floor.unwrap_or(f64::NEG_INFINITY);
        let mut out = Vec::with_capacity(8);

        for edge in self.edges {
            if edge.weight < floor {
                continue;
            }
            if let Some(rel) = walk.relation_filter.as_deref() {
                if edge.relation != rel {
                    continue;
                }
            }
            let (neighbor_id, inbound) = if edge.source_id == entity_id {
                (edge.target_id, false)
            } else if edge.target_id == entity_id && walk.direction == WalkDirection::Bidirectional
            {
                (edge.source_id, true)
            } else {
                continue;
            };
            // An edge pointing at an entity absent from the index cannot be rendered.
            let Some(neighbor_name) = self.id_to_name.get(&neighbor_id) else {
                continue;
            };
            let self_name = self.id_to_name.get(&entity_id).cloned();
            let (source_name, target_name) = if inbound {
                (Some(neighbor_name.clone()), self_name)
            } else {
                (self_name, Some(neighbor_name.clone()))
            };
            out.push(EdgeArrival {
                from_id: entity_id,
                neighbor_id,
                source_name,
                target_name,
                relation: edge.relation.clone(),
                weight: edge.weight,
                inbound,
            });
        }
        Ok(out)
    }
}
