//! Entity graph traversal (BFS over memory_entities + relations).
//!
//! Queries the SQLite entity and relation tables to expand neighbourhood
//! sets used by the `related`, `recall`, `hybrid-search`, `deep-research` and
//! `graph traverse` commands.
//!
//! `walk` holds the single BFS engine; the other modules are thin adapters.
//! `traverse` answers "which memories are within N hops", `bfs` additionally
//! records the predecessor of each entity so evidence chains can be
//! reconstructed.

mod bfs;
mod traverse;
pub mod walk;

pub use bfs::{bfs_with_predecessors, EntityDepthMap, PredecessorMap};
pub use traverse::{traverse_from_memories_with_hops, traverse_from_memories_with_hops_capped};
pub use walk::{
    EdgeArrival, GraphWalk, InMemoryNeighbors, MemoryEdge, NeighborSource, SqlNeighbors,
    WalkDirection, WalkOutcome,
};

#[cfg(test)]
mod tests;
