//! Entity-graph expansion of the fused results (`--with-graph`).
//!
//! Seeds a bounded traversal with the top fused memories plus the entities
//! nearest to the query embedding, then materialises every reachable memory
//! that is not already part of the fused results.

use super::{HybridSearchArgs, HybridSearchItem};
use crate::errors::AppError;
use crate::graph::traverse_from_memories_with_hops;
use crate::output::RecallItem;
use crate::storage::entities;
use crate::storage::memories;
use rusqlite::Connection;

/// Traverses the entity graph outward from `results` and returns the extra
/// memories it reached. Returns an empty list when `--with-graph` is off, when
/// the fused results are empty, or when no live embedding is available.
pub(super) fn expand(
    conn: &Connection,
    args: &HybridSearchArgs,
    embedding: Option<&Vec<f32>>,
    namespace: &str,
    results: &[HybridSearchItem],
) -> Result<Vec<RecallItem>, AppError> {
    let mut graph_matches: Vec<RecallItem> = Vec::with_capacity(8);
    let Some(emb) = args
        .with_graph
        .then_some(())
        .filter(|_| !results.is_empty())
        .and(embedding)
    else {
        return Ok(graph_matches);
    };

    let memory_ids: Vec<i64> = results.iter().map(|r| r.memory_id).collect();

    let entity_knn = entities::knn_search(conn, emb, namespace, 5)?;
    let entity_ids: Vec<i64> = entity_knn.iter().map(|(id, _)| *id).collect();

    let all_seed_ids: Vec<i64> = memory_ids
        .iter()
        .chain(entity_ids.iter())
        .copied()
        .collect();

    if !all_seed_ids.is_empty() {
        let graph_memory_ids = traverse_from_memories_with_hops(
            conn,
            &all_seed_ids,
            namespace,
            args.min_weight.unwrap_or(0.3),
            args.max_hops.unwrap_or(2),
        )?;

        let already_in_results: std::collections::HashSet<i64> =
            results.iter().map(|r| r.memory_id).collect();

        // The traversal is seeded by the fused results AND by the entities
        // nearest the query embedding, so what it reaches is sized by the graph
        // rather than by `--k`. Without a ceiling every reached memory ships a
        // 300-character snippet, which measured 1 112 925 bytes from `--k 3`.
        let cap = crate::constants::hybrid_search_max_graph_results(args.max_graph_results);

        for (graph_mem_id, hop) in graph_memory_ids {
            if cap.is_some_and(|n| graph_matches.len() >= n) {
                break;
            }
            if already_in_results.contains(&graph_mem_id) {
                continue;
            }
            if let Some(row) = memories::read_full(conn, graph_mem_id)? {
                let snippet: String = row.body.chars().take(300).collect();
                let graph_distance = 1.0 - 1.0 / (hop as f32 + 1.0);
                graph_matches.push(RecallItem {
                    memory_id: row.id,
                    name: row.name,
                    namespace: row.namespace,
                    memory_type: row.memory_type,
                    description: row.description,
                    snippet,
                    distance: graph_distance,
                    score: RecallItem::score_from_distance(graph_distance),
                    source: "graph".to_string(),
                    graph_depth: Some(hop),
                });
            }
        }
    }

    Ok(graph_matches)
}
