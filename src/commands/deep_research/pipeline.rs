//! Deep-research pipeline pieces: sub-query planning, embedding precompute,
//! hybrid retrieval and evidence-chain reconstruction.
//!
//! Split from the command handler (R-SRP-01) so the orchestration in `run` /
//! `run_async` stays separate from the per-sub-query work.

use super::{DeepResearchArgs, EvidenceChain, EvidenceNode, SubQuery, SubQueryResult};
use crate::errors::AppError;
use crate::graph::{
    bfs_with_predecessors, traverse_from_memories_with_hops_capped, PredecessorMap,
};
use crate::output;
use crate::storage::connection::open_ro;
use crate::storage::fusion::{rrf_fuse, rrf_max_possible};
use crate::storage::{entities, memories};
use std::collections::HashSet;
use std::sync::Arc;

/// Per-sub-query vectors, whether anything degraded, and the FIRST such code.
///
/// The third element arrived in v1.2.5 alongside wiring `--fail-on-degraded`:
/// [`crate::query_embedding::degradation_failure`] derives the error class from
/// the CODE and never from the prose, and until then this path logged the code
/// and dropped it. Without it no caller could honour that contract.
pub(super) type SubEmbeddings = (Vec<Option<Arc<Vec<f32>>>>, bool, Option<&'static str>);

/// GAP-001 (v1.1.04): computes per-sub-query embeddings OUTSIDE the tokio
/// runtime. `try_embed_query_with_embedding_choice` (OpenRouter path) calls
/// `shared_runtime()?.block_on(...)` internally; running it inside the
/// multi-thread runtime built in `run` triggers
/// "Cannot start a runtime from within a runtime" because the nested
/// `block_on` happens on a worker thread already driven by the outer
/// runtime. Resolving embeddings synchronously before the runtime is built
/// removes the nesting entirely.
pub(super) fn compute_sub_embeddings(
    paths: &crate::paths::AppPaths,
    sub_query_texts: &[String],
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    llm_backend: crate::cli::LlmBackendChoice,
) -> SubEmbeddings {
    output::emit_progress_i18n(
        "Computing per-sub-query embeddings...",
        "Calculando embeddings por sub-consulta...",
    );
    let mut sub_embeddings: Vec<Option<Arc<Vec<f32>>>> = Vec::with_capacity(sub_query_texts.len());
    let mut vec_degraded = false;
    // The FIRST degradation's code, kept for `--fail-on-degraded`. Previously
    // `reason_code` was logged and discarded, so no caller could classify the
    // failure by code rather than by prose — which is what
    // `degradation_failure` requires. First and not last: a sub-query that
    // failed on a timeout must not be reclassified by one that failed later.
    let mut first_reason_code: Option<&'static str> = None;
    for sq_text in sub_query_texts {
        match crate::embedder::try_embed_query_with_embedding_choice(
            &paths.models,
            sq_text,
            embedding_backend,
            llm_backend,
        ) {
            Ok((v, _backend)) => sub_embeddings.push(Some(Arc::new(v))),
            Err(reason) => {
                let code = reason.reason_code();
                tracing::warn!(target: "deep_research", fallback_reason = %reason, reason_code = %code, "embedding failed for sub-query; falling back to FTS5");
                sub_embeddings.push(None);
                vec_degraded = true;
                first_reason_code.get_or_insert(code);
            }
        }
    }
    (sub_embeddings, vec_degraded, first_reason_code)
}

/// Build the sub-query plan from CLI strategy (heuristic or manual file).
pub(super) fn resolve_sub_queries(args: &DeepResearchArgs) -> Result<Vec<SubQuery>, AppError> {
    if args.query.trim().is_empty() {
        return Err(AppError::Validation(crate::i18n::validation::empty_query()));
    }
    match args.sub_query_strategy.as_str() {
        "manual" => {
            let path = args.sub_queries_file.as_ref().ok_or_else(|| {
                AppError::Validation(
                    "--sub-query-strategy manual requires --sub-queries-file PATH".to_string(),
                )
            })?;
            let raw = std::fs::read_to_string(path).map_err(AppError::Io)?;
            let mut texts: Vec<String> = raw
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(str::to_string)
                .collect();
            if texts.is_empty() {
                return Err(AppError::Validation(
                    crate::i18n::validation::sub_queries_file_empty(&path.display().to_string()),
                ));
            }
            texts.truncate(args.max_sub_queries);
            Ok(texts
                .into_iter()
                .enumerate()
                .map(|(i, text)| SubQuery {
                    id: i,
                    text,
                    source: "manual",
                })
                .collect())
        }
        _ => {
            let planned = decompose_query_with_sources(&args.query, args.max_sub_queries);
            Ok(planned
                .into_iter()
                .enumerate()
                .map(|(i, (text, source))| SubQuery {
                    id: i,
                    text,
                    source,
                })
                .collect())
        }
    }
}

/// Aspect facets applied when a single-token query cannot be split syntactically.
///
/// Covers the angles operators expect for person/org subjects (patrimony, stack,
/// stakeholders, projects, decisions, relationships, context) in EN and PT so
/// FTS/hybrid retrieval fans out beyond the literal token (v1.1.05 Bug 1).
const SINGLE_TOKEN_ASPECTS: &[&str] = &[
    "patrimonio",
    "stack",
    "tecnologia",
    "stakeholders",
    "pessoas",
    "projeto",
    "decisao",
    "relacionamento",
    "contexto",
    "architecture",
    "history",
];

/// Heuristic query decomposition with per-sub-query source labels.
///
/// Splits by conjunctions, commas, semicolons, relational phrases, word-pairs
/// for multi-word queries, and **single-token aspect expansion** when none of
/// the syntactic branches fire (v1.1.05 Bug 1).
pub(super) fn decompose_query_with_sources(query: &str, max: usize) -> Vec<(String, &'static str)> {
    if query.is_empty() {
        return vec![(query.to_string(), "original")];
    }

    let mut parts: Vec<(String, &'static str)> = Vec::with_capacity(max);

    // Split by relational phrases first (most specific).
    let relational = [
        " that caused ",
        " depending on ",
        " related to ",
        " connected to ",
        " linked to ",
        " caused by ",
        " followed by ",
    ];
    let mut text = query.to_string();
    let mut did_relational_split = false;
    for phrase in &relational {
        if text.to_lowercase().contains(phrase) {
            let lower = text.to_lowercase();
            if let Some(pos) = lower.find(phrase) {
                let left = text[..pos].trim().to_string();
                let right = text[pos + phrase.len()..].trim().to_string();
                if !left.is_empty() {
                    parts.push((left, "decomposed"));
                }
                if !right.is_empty() {
                    text = right;
                }
                did_relational_split = true;
            }
        }
    }
    if did_relational_split && !text.is_empty() {
        parts.push((text.clone(), "decomposed"));
    }

    // If no relational split, try conjunctions and delimiters.
    if parts.is_empty() {
        let semi_parts: Vec<&str> = query.split(';').collect();
        if semi_parts.len() > 1 {
            for p in &semi_parts {
                let trimmed = p.trim();
                if !trimmed.is_empty() {
                    parts.push((trimmed.to_string(), "decomposed"));
                }
            }
        } else {
            let normalized = query
                .replace(" and ", ", ")
                .replace(" AND ", ", ")
                .replace(" e ", ", ")
                .replace(" E ", ", ");
            let comma_parts: Vec<&str> = normalized.split(',').collect();
            if comma_parts.len() > 1 {
                for p in &comma_parts {
                    let trimmed = p.trim();
                    if !trimmed.is_empty() {
                        parts.push((trimmed.to_string(), "decomposed"));
                    }
                }
            }
        }
    }

    // If still no split, try word-pair decomposition for multi-word queries.
    if parts.is_empty() {
        let words: Vec<&str> = query.split_whitespace().filter(|w| w.len() > 2).collect();
        if words.len() >= 3 {
            parts.push((query.to_string(), "original"));
            parts.push((format!("{} {}", words[0], words[1]), "decomposed"));
            parts.push((
                format!("{} {}", words[words.len() - 2], words[words.len() - 1]),
                "decomposed",
            ));
        }
    }

    // v1.1.05 Bug 1: single-token (or unsplittable) queries get aspect fan-out.
    if parts.is_empty() {
        let token_count = query.split_whitespace().filter(|w| !w.is_empty()).count();
        if token_count == 1 {
            let token = query.trim();
            parts.push((token.to_string(), "original"));
            for aspect in SINGLE_TOKEN_ASPECTS {
                if parts.len() >= max {
                    break;
                }
                parts.push((format!("{token} {aspect}"), "aspect"));
            }
        } else {
            return vec![(query.to_string(), "original")];
        }
    }

    parts.truncate(max);
    parts
}

/// Heuristic query decomposition (text-only; unit tests).
#[cfg(test)]
pub(super) fn decompose_query(query: &str, max: usize) -> Vec<String> {
    decompose_query_with_sources(query, max)
        .into_iter()
        .map(|(t, _)| t)
        .collect()
}

/// Reconstruct a directed path from `target_entity_id` back to a seed using the
/// predecessor map built by BFS.  Returns the path nodes from root to target
/// plus the accumulated edge weights.
pub(super) fn reconstruct_path(
    target_id: i64,
    seed_entity_ids: &HashSet<i64>,
    predecessor: &PredecessorMap,
    entity_names: &crate::hash::AHashMap<i64, String>,
) -> Option<(Vec<EvidenceNode>, f64)> {
    let mut path_ids: Vec<(i64, Option<String>, Option<f64>)> = Vec::with_capacity(8);
    let mut total_weight = 1.0_f64;
    let mut current = target_id;

    loop {
        if seed_entity_ids.contains(&current) {
            break;
        }
        let (parent, relation, weight) = predecessor.get(&current)?;
        total_weight *= weight;
        path_ids.push((current, Some(relation.clone()), Some(*weight)));
        current = *parent;
    }
    // Push the seed entity (root).
    path_ids.push((current, None, None));

    // Reverse so path goes from seed → target.
    path_ids.reverse();

    let nodes: Vec<EvidenceNode> = path_ids
        .into_iter()
        .map(|(id, relation, weight)| EvidenceNode {
            entity: entity_names
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("entity-{id}")),
            relation,
            weight,
        })
        .collect();

    Some((nodes, total_weight))
}

/// Execute a single sub-query: hybrid search (KNN + FTS fused via RRF) + graph traversal.
///
/// GAP-07 fix: receives the embedding for THIS sub-query (not the shared original).
/// GAP-08/11 fix: uses rrf_fuse() for proper score fusion instead of hardcoded 0.5.
/// GAP-09/10 fix: builds directed evidence chains filtered to discovered entities.
/// GAP-17: respects max_neighbors_per_hop cap in BFS.
///
/// Runs synchronously on a blocking thread (called from a tokio spawn context).
/// Each call opens its own read-only SQLite connection to leverage WAL concurrency.
#[allow(clippy::too_many_arguments)]
pub(super) fn execute_sub_query(
    sub_query_id: usize,
    query_text: &str,
    embedding: Option<&[f32]>,
    namespace: &str,
    db_path: &std::path::Path,
    k: usize,
    max_hops: usize,
    min_weight: f64,
    rrf_k: f64,
    graph_decay: f64,
    graph_min_score: f64,
    max_neighbors_per_hop: Option<usize>,
) -> Result<SubQueryResult, String> {
    let conn = open_ro(db_path).map_err(|e| format!("failed to open db: {e}"))?;

    let mut hits: Vec<(i64, f64, String, String, String, Option<usize>)> =
        Vec::with_capacity(k * 2);
    let mut seen_ids: crate::hash::AHashSet<i64> =
        crate::hash::AHashSet::with_capacity_and_hasher(k * 2, Default::default());

    // --- GAP-08/11 FIX: Use RRF fusion for KNN + FTS instead of hardcoded 0.5 ---

    // 1. KNN vector search — collect ranked IDs (skipped when embedding unavailable).
    let (knn_ids, knn_distance_map) = if let Some(emb) = embedding {
        let knn_results = memories::knn_search(&conn, emb, &[namespace.to_string()], None, k)
            .map_err(|e| format!("knn_search failed: {e}"))?;
        let ids: Vec<i64> = knn_results.iter().map(|(id, _)| *id).collect();
        tracing::debug!(target: "deep_research", sub_query_id, knn_count = ids.len(), "KNN complete");
        let dist_map: crate::hash::AHashMap<i64, f64> = knn_results
            .iter()
            .map(|(id, dist)| (*id, *dist as f64))
            .collect();
        (ids, dist_map)
    } else {
        tracing::debug!(target: "deep_research", sub_query_id, "KNN skipped (no embedding); FTS5-only");
        (vec![], crate::hash::AHashMap::default())
    };

    // 2. FTS5 search — collect ranked IDs.
    let fts_results = match memories::fts_search(&conn, query_text, namespace, None, k) {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(target: "deep_research",
                sub_query_id,
                "FTS5 search failed, continuing with KNN only: {e}"
            );
            vec![]
        }
    };
    let fts_ids: Vec<i64> = fts_results.iter().map(|r| r.id).collect();
    tracing::debug!(target: "deep_research", sub_query_id, fts_count = fts_ids.len(), "FTS complete");

    // 3. Fuse via RRF.
    let rrf_scores = rrf_fuse(&[(1.0, &knn_ids), (1.0, &fts_ids)], rrf_k);
    let max_possible = rrf_max_possible(&[1.0, 1.0], rrf_k);

    // 4. Sort fused results and build hits.
    let mut fused: Vec<(i64, f64)> = rrf_scores.into_iter().collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    fused.truncate(k * 2);
    tracing::debug!(target: "deep_research",
        sub_query_id,
        fused_count = fused.len(),
        "RRF fusion complete"
    );

    if fused.is_empty() && !knn_ids.is_empty() {
        tracing::warn!(target: "deep_research", sub_query_id, knn_count = knn_ids.len(), fts_count = fts_ids.len(),
            "RRF fusion returned 0 results despite KNN/FTS hits; consider lowering --graph-min-score");
    }

    for (memory_id, combined_score) in &fused {
        if seen_ids.insert(*memory_id) {
            let normalized = if max_possible > 0.0 {
                combined_score / max_possible
            } else {
                0.0
            };
            let score = normalized.clamp(0.0, 1.0);
            let in_knn = knn_distance_map.contains_key(memory_id);
            let in_fts = fts_ids.contains(memory_id);
            let source = match (in_knn, in_fts) {
                (true, true) => "hybrid",
                (true, false) => "knn",
                (false, true) => "fts",
                (false, false) => "graph",
            };
            if let Ok(Some(row)) = memories::read_full(&conn, *memory_id) {
                let snippet: String = row.body.chars().take(300).collect();
                hits.push((
                    *memory_id,
                    score,
                    source.to_string(),
                    snippet,
                    row.body,
                    None,
                ));
            }
        }
    }

    // 5. Graph traversal from discovered memories.
    // GAP-09/10 FIX: entity KNN also uses this sub-query's embedding.
    let memory_ids: Vec<i64> = hits.iter().map(|(id, ..)| *id).collect();
    let mut chains: Vec<EvidenceChain> = Vec::with_capacity(memory_ids.len());

    if !memory_ids.is_empty() && max_hops > 0 {
        // Seed entities from KNN on entity vectors (skipped when embedding unavailable).
        let entity_ids: Vec<i64> = if let Some(emb) = embedding {
            entities::knn_search(&conn, emb, namespace, 5)
                .inspect_err(|e| tracing::warn!(target: "deep_research", error = %e, "entity KNN search failed, skipping graph seed"))
                .unwrap_or_default()
                .iter()
                .map(|(id, _)| *id)
                .collect()
        } else {
            vec![]
        };

        // HIGH-01 FIX: limit seeds to top-5 memories by score to prevent
        // BFS from starting at every node when k >= total memories.
        let top_seed_count = 5.min(memory_ids.len());
        let top_memory_ids = &memory_ids[..top_seed_count];
        let mut seed_entity_ids: Vec<i64> = entity_ids.clone();
        for &mem_id in top_memory_ids {
            let mut stmt = conn
                .prepare_cached("SELECT entity_id FROM memory_entities WHERE memory_id = ?1")
                .map_err(|e| format!("prepare failed: {e}"))?;
            let ids: Vec<i64> = stmt
                .query_map(rusqlite::params![mem_id], |r| r.get(0))
                .map_err(|e| format!("query failed: {e}"))?
                .filter_map(|r| r.ok())
                .collect();
            seed_entity_ids.extend(ids);
        }
        seed_entity_ids.sort_unstable();
        seed_entity_ids.dedup();
        tracing::debug!(target: "deep_research",
            sub_query_id,
            seed_count = seed_entity_ids.len(),
            "seed entities collected"
        );

        let all_seed_ids: Vec<i64> = memory_ids
            .iter()
            .chain(entity_ids.iter())
            .copied()
            .collect();

        // Graph traversal with hop scores.
        if let Ok(graph_results) = traverse_from_memories_with_hops_capped(
            &conn,
            &all_seed_ids,
            namespace,
            min_weight,
            max_hops as u32,
            max_neighbors_per_hop,
        ) {
            // Build seed score map from RRF-fused scores for graph decay computation.
            let seed_score_map: crate::hash::AHashMap<i64, f64> = fused
                .iter()
                .map(|(id, s)| {
                    let normalized = if max_possible > 0.0 {
                        s / max_possible
                    } else {
                        0.0
                    };
                    (*id, normalized.clamp(0.0, 1.0))
                })
                .collect();

            for (graph_mem_id, hop) in graph_results {
                if seen_ids.insert(graph_mem_id) {
                    // GAP-08/11 FIX: graph score = seed_score * decay^hop * edge_weight.
                    // For the seed score, use the best score among the seed memories that
                    // transitively reached this graph memory (approximate with the average
                    // seed score since we don't track the exact path yet).
                    let avg_seed_score: f64 = if seed_score_map.is_empty() {
                        0.5
                    } else {
                        let sum: f64 = seed_score_map.values().sum();
                        sum / seed_score_map.len() as f64
                    };
                    let graph_score =
                        (avg_seed_score * graph_decay.powi(hop as i32)).clamp(0.0, 1.0);

                    if graph_score < graph_min_score {
                        continue;
                    }

                    if let Ok(Some(row)) = memories::read_full(&conn, graph_mem_id) {
                        let snippet: String = row.body.chars().take(300).collect();
                        hits.push((
                            graph_mem_id,
                            graph_score,
                            "graph".to_string(),
                            snippet,
                            row.body,
                            Some(hop as usize),
                        ));
                    }
                }
            }
        }

        // GAP-09/10 FIX: Build directed evidence chains using BFS with predecessor map,
        // filtered to entities discovered in this sub-query.
        if !seed_entity_ids.is_empty() {
            let (entity_depth, predecessor) = bfs_with_predecessors(
                &conn,
                &seed_entity_ids,
                namespace,
                min_weight,
                max_hops as u32,
                max_neighbors_per_hop,
            )
            .unwrap_or_default();

            tracing::debug!(target: "deep_research",
                sub_query_id,
                bfs_nodes = entity_depth.len(),
                predecessors = predecessor.len(),
                "BFS complete"
            );

            let seed_entity_set: HashSet<i64> = seed_entity_ids.iter().copied().collect();

            // Collect entity IDs we need names for.
            let all_entity_ids: Vec<i64> = entity_depth.keys().copied().collect();
            let mut entity_names: crate::hash::AHashMap<i64, String> =
                crate::hash::AHashMap::with_capacity_and_hasher(
                    all_entity_ids.len(),
                    ahash::RandomState::default(),
                );
            for &eid in &all_entity_ids {
                let name_res: rusqlite::Result<String> = conn.query_row(
                    "SELECT name FROM entities WHERE id = ?1",
                    rusqlite::params![eid],
                    |r| r.get(0),
                );
                if let Ok(name) = name_res {
                    entity_names.insert(eid, name);
                }
            }

            // Reconstruct a path for each non-seed entity that has a predecessor.
            for (&target_id, &_hop) in &entity_depth {
                if seed_entity_set.contains(&target_id) {
                    continue;
                }
                if !predecessor.contains_key(&target_id) {
                    continue;
                }
                if let Some((path_nodes, total_weight)) =
                    reconstruct_path(target_id, &seed_entity_set, &predecessor, &entity_names)
                {
                    if path_nodes.len() < 2 {
                        continue;
                    }
                    let from = path_nodes
                        .first()
                        .map(|n| n.entity.clone())
                        .unwrap_or_default();
                    let to = path_nodes
                        .last()
                        .map(|n| n.entity.clone())
                        .unwrap_or_default();
                    let depth = path_nodes.len();
                    chains.push(EvidenceChain {
                        from,
                        to,
                        path: path_nodes,
                        total_weight,
                        depth,
                        sub_query_ids: vec![sub_query_id],
                    });
                }
            }

            // Sort chains by total_weight descending and cap to avoid huge output.
            chains.sort_by(|a, b| {
                b.total_weight
                    .partial_cmp(&a.total_weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            chains.truncate(20);
            tracing::debug!(target: "deep_research",
                sub_query_id,
                chains_count = chains.len(),
                "evidence chains built"
            );
        }
    }

    Ok(SubQueryResult {
        sub_query_id,
        hits,
        chains,
    })
}

// ────────────────────────────────────────────────────────────────────────────
