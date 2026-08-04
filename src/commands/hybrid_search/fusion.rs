//! Reciprocal Rank Fusion of the vector and FTS5 candidate lists.
//!
//! Accumulates the weighted RRF score per memory, ranks, truncates to `k`,
//! batch-fetches the surviving rows (avoiding N+1) and materialises the
//! response items in ranking order.

use super::{HybridSearchArgs, HybridSearchItem};
use crate::errors::AppError;
use crate::storage::memories;
use rusqlite::Connection;
use std::collections::HashMap;

/// Fuses the two candidate lists into the final, ranked result items.
pub(super) fn fuse_candidates(
    conn: &Connection,
    args: &HybridSearchArgs,
    vec_results: &[(i64, f32)],
    fts_results: &[memories::MemoryRow],
) -> Result<Vec<HybridSearchItem>, AppError> {
    // Map vector ranking position by memory_id (1-indexed per schema)
    let vec_rank_map: HashMap<i64, usize> = vec_results
        .iter()
        .enumerate()
        .map(|(pos, (id, _))| (*id, pos + 1))
        .collect();

    // Map raw KNN distance by memory_id for GAP-30: vec_distance field.
    let vec_distance_map: HashMap<i64, f64> = vec_results
        .iter()
        .map(|(id, dist)| (*id, *dist as f64))
        .collect();

    // Map FTS ranking position by memory_id (1-indexed per schema)
    let fts_rank_map: HashMap<i64, usize> = fts_results
        .iter()
        .enumerate()
        .map(|(pos, row)| (row.id, pos + 1))
        .collect();

    let rrf_k = args.rrf_k as f64;

    // Accumulate combined RRF scores
    let mut combined_scores: crate::hash::AHashMap<i64, f64> =
        crate::hash::AHashMap::with_capacity_and_hasher(
            vec_results.len() + fts_results.len(),
            Default::default(),
        );

    for (rank, (memory_id, _)) in vec_results.iter().enumerate() {
        let score = args.weight_vec as f64 * (1.0 / (rrf_k + rank as f64 + 1.0));
        *combined_scores.entry(*memory_id).or_insert(0.0) += score;
    }

    for (rank, row) in fts_results.iter().enumerate() {
        let score = args.weight_fts as f64 * (1.0 / (rrf_k + rank as f64 + 1.0));
        *combined_scores.entry(row.id).or_insert(0.0) += score;
    }

    // Sort by score descending and take the top-k
    let mut ranked: Vec<(i64, f64)> = combined_scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(args.k);

    // Collect all IDs for batch fetch (avoiding N+1)
    let top_ids: Vec<i64> = ranked.iter().map(|(id, _)| *id).collect();

    // Fetch full data for the top memories
    let mut memory_data: crate::hash::AHashMap<i64, memories::MemoryRow> =
        crate::hash::AHashMap::with_capacity_and_hasher(ranked.len(), Default::default());
    for id in &top_ids {
        if let Some(row) = memories::read_full(conn, *id)? {
            memory_data.insert(*id, row);
        }
    }

    let max_possible = args.weight_vec as f64 * (1.0 / (rrf_k + 1.0))
        + args.weight_fts as f64 * (1.0 / (rrf_k + 1.0));

    // Build final results in ranking order
    Ok(ranked
        .into_iter()
        .filter_map(|(memory_id, combined_score)| {
            let normalized_score = if max_possible > 0.0 {
                combined_score / max_possible
            } else {
                0.0
            };
            memory_data.remove(&memory_id).map(|row| {
                let snippet: String = row.body.chars().take(300).collect();
                HybridSearchItem {
                    memory_id: row.id,
                    name: row.name,
                    namespace: row.namespace,
                    memory_type: row.memory_type,
                    description: row.description,
                    body: row.body,
                    snippet,
                    combined_score,
                    score: combined_score,
                    source: "hybrid".to_string(),
                    vec_rank: vec_rank_map.get(&memory_id).copied(),
                    fts_rank: fts_rank_map.get(&memory_id).copied(),
                    rrf_score: Some(combined_score),
                    normalized_score,
                    vec_distance: vec_distance_map.get(&memory_id).copied(),
                    fts_bm25: None,
                }
            })
        })
        .collect())
}
