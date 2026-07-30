//! Handler for the `deep-research` CLI subcommand.
//!
//! Orchestrates parallel multi-hop GraphRAG search via query decomposition.
//! The workload is I/O-bound (SQLite WAL reads), so tokio is used instead of
//! rayon. Each sub-query opens its own read-only connection.

use crate::errors::AppError;
use crate::output;
use crate::paths::AppPaths;
use crate::storage::connection::open_ro;
use crate::storage::{entities, memories};

use serde::Serialize;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

mod pipeline;
use pipeline::{
    compute_sub_embeddings, execute_sub_query, resolve_sub_queries,
};

#[cfg(test)]
use pipeline::{{decompose_query, decompose_query_with_sources}};

/// Arguments for the `deep-research` subcommand.
#[derive(clap::Args)]
#[command(
    about = "Deep parallel multi-hop GraphRAG research via query decomposition",
    after_long_help = "CONTRACT:\n  \
        stdout = pretty JSON envelope only (machine-readable).\n  \
        stderr = tracing / progress / diagnostics only.\n  \
        Never redirect with `&>` or `2>&1` into the same file as stdout — that\n  \
        contaminates the JSON and breaks jaq/jq. Prefer:\n  \
        sqlite-graphrag deep-research \"q\" > out.json 2>/dev/null\n  \
        or --output out.json (atomic write via atomwrite algorithm).\n\n\
EXAMPLES:\n  \
        # Basic deep research (single-token queries auto-expand into aspects)\n  \
        sqlite-graphrag deep-research \"danilo\"\n\n  \
        # With custom parameters\n  \
        sqlite-graphrag deep-research \"auth\" --k 20 --max-hops 3 --max-sub-queries 7\n\n  \
        # Include full memory bodies in output\n  \
        sqlite-graphrag deep-research \"auth\" --with-bodies\n\n  \
        # Manual sub-queries (one query per line)\n  \
        sqlite-graphrag deep-research \"danilo\" --sub-query-strategy manual \\\n  \
          --sub-queries-file aspects.txt\n\n  \
        # Atomic JSON file (crash-safe; preferred for large --with-bodies runs)\n  \
        sqlite-graphrag deep-research \"auth\" --output /tmp/dr.json\n\n  \
        # Tune RRF and graph scoring\n  \
        sqlite-graphrag deep-research \"auth and deployment\" --rrf-k 60 --graph-decay 0.7"
)]
pub struct DeepResearchArgs {
    /// Research query to decompose and search.
    #[arg(
        value_name = "QUERY",
        allow_hyphen_values = true,
        help = "Research query to decompose and search"
    )]
    pub query: String,
    /// Results per sub-query (Recall@20 captures 95%+ relevant hits).
    #[arg(
        long,
        short,
        aliases = ["limit", "top-k"],
        default_value_t = 20,
        help = "Results per sub-query (Recall@20 captures 95%+ relevant hits)"
    )]
    pub k: usize,
    /// Maximum sub-queries from decomposition (covers complex multi-hop queries).
    #[arg(
        long,
        default_value_t = 7,
        help = "Maximum sub-queries (covers complex multi-hop queries)"
    )]
    pub max_sub_queries: usize,
    /// Multi-hop graph traversal depth (sweet spot: 2-3 hops).
    #[arg(
        long,
        default_value_t = 3,
        help = "Multi-hop graph traversal depth (sweet spot: 2-3 hops)"
    )]
    pub max_hops: usize,
    /// Minimum edge weight for graph traversal.
    #[arg(
        long,
        default_value_t = 0.3,
        help = "Minimum edge weight for graph traversal"
    )]
    pub min_weight: f64,
    /// Maximum concurrent sub-queries (default: min(cpus, 8)).
    #[arg(long, help = "Maximum concurrent sub-queries (default: min(cpus, 8))")]
    pub max_concurrency: Option<usize>,
    /// Timeout per sub-query in seconds.
    #[arg(long, default_value_t = 30, help = "Timeout per sub-query in seconds")]
    pub timeout: u64,
    /// Include full memory bodies in results.
    #[arg(
        long,
        default_value_t = false,
        help = "Include full memory bodies in results"
    )]
    pub with_bodies: bool,
    /// Maximum results after deduplication.
    #[arg(
        long,
        default_value_t = 50,
        help = "Maximum results after deduplication"
    )]
    pub max_results: usize,
    /// RRF k parameter controlling score smoothing (higher = less weight on top ranks).
    #[arg(
        long,
        default_value_t = 60.0,
        help = "RRF k parameter (higher = less weight on top ranks)"
    )]
    pub rrf_k: f64,
    /// Decay factor applied to graph scores per hop (score = seed_score * decay^hop).
    #[arg(
        long,
        default_value_t = 0.7,
        help = "Graph score decay factor per hop (0.0-1.0)"
    )]
    pub graph_decay: f64,
    /// Minimum score threshold for graph-expanded results (filters noise).
    #[arg(
        long,
        default_value_t = 0.05,
        help = "Minimum score threshold for graph-expanded results"
    )]
    pub graph_min_score: f64,
    /// Limit top-k neighbours followed per entity per hop (None = unlimited).
    #[arg(
        long,
        help = "Limit neighbours per entity per hop for graph traversal (default: unlimited)"
    )]
    pub max_neighbors_per_hop: Option<usize>,
    /// Namespace (flag / XDG namespace.default / global).
    #[arg(
        long,
        help = "Namespace (flag / XDG namespace.default / global)"
    )]
    pub namespace: Option<String>,
    /// Research mode: `none` (local heuristic, default), `claude-code`, `codex` (v1.1.0).
    #[arg(long, default_value = "none", value_parser = ["none"], hide = true)]
    pub mode: String,
    /// Maximum LLM cost in USD (effective with --mode claude-code/codex, reserved for v1.1.0).
    #[arg(
        long,
        value_name = "USD",
        help = "Max LLM cost in USD (effective with --mode claude-code/codex)"
    )]
    pub max_cost_usd: Option<f64>,
    /// JSON output (always on, kept for consistency).
    #[arg(long, hide = true)]
    pub json: bool,
    /// Database path.
    #[arg(long)]
    pub db: Option<String>,
    /// Sub-query strategy: `heuristic` (default, syntactic + single-token aspects)
    /// or `manual` (requires `--sub-queries-file`).
    #[arg(
        long,
        default_value = "heuristic",
        value_parser = ["heuristic", "manual"],
        help = "Sub-query strategy: heuristic (default) or manual"
    )]
    pub sub_query_strategy: String,
    /// Path to a UTF-8 text file with one sub-query per line (required when
    /// `--sub-query-strategy manual`). Empty lines and `#` comments are ignored.
    #[arg(
        long,
        value_name = "PATH",
        help = "File with one sub-query per line (manual strategy)"
    )]
    pub sub_queries_file: Option<std::path::PathBuf>,
    /// Write the JSON envelope atomically to this path (tempfile→fsync→rename).
    /// When set, stdout receives a short confirmation JSON
    /// `{ "written": "<path>", "bytes": N, "blake3": "..." }` instead of the full
    /// envelope — preventing shell redirect truncation of multi-MB payloads.
    #[arg(
        short = 'o',
        long,
        value_name = "PATH",
        help = "Atomic JSON output path (atomwrite algorithm; short -o)"
    )]
    pub output: Option<std::path::PathBuf>,
}

#[derive(Serialize)]
pub(super) struct SubQuery {
    pub(super) id: usize,
    pub(super) text: String,
    pub(super) source: &'static str,
}

#[derive(Serialize)]
struct DeepResult {
    name: String,
    score: f64,
    source: String,
    sub_query_ids: Vec<usize>,
    snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    hop_distance: Option<usize>,
}

/// A node in a reconstructed evidence path.
#[derive(Serialize, Clone)]
pub(super) struct EvidenceNode {
    pub(super) entity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) relation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) weight: Option<f64>,
}

/// A directed evidence chain reconstructed from BFS predecessors.
///
/// Fields:
/// - `from`: name of the seed (source) entity.
/// - `to`: name of the terminal (target) entity.
/// - `path`: ordered list of intermediate nodes from `from` to `to`.
/// - `total_weight`: product of edge weights along the path.
/// - `sub_query_ids`: which sub-queries produced this chain.
#[derive(Serialize)]
pub(super) struct EvidenceChain {
    pub(super) from: String,
    pub(super) to: String,
    pub(super) path: Vec<EvidenceNode>,
    pub(super) total_weight: f64,
    pub(super) depth: usize,
    pub(super) sub_query_ids: Vec<usize>,
}

#[derive(Serialize)]
struct ResearchStats {
    sub_queries_total: usize,
    sub_queries_completed: usize,
    sub_queries_failed: usize,
    sub_queries_timed_out: usize,
    unique_memories_found: usize,
    evidence_chains_found: usize,
    elapsed_ms: u64,
    vec_degraded: bool,
}

#[derive(Serialize)]
struct GraphContextEntity {
    name: String,
    entity_type: String,
    degree: u32,
}

#[derive(Serialize)]
struct GraphContextRel {
    from: String,
    to: String,
    relation: String,
    weight: f64,
}

#[derive(Serialize)]
struct GraphContext {
    entities: Vec<GraphContextEntity>,
    relationships: Vec<GraphContextRel>,
}

#[derive(Serialize)]
struct DeepResearchResponse {
    query: String,
    sub_queries: Vec<SubQuery>,
    results: Vec<DeepResult>,
    evidence_chains: Vec<EvidenceChain>,
    #[serde(skip_serializing_if = "Option::is_none")]
    graph_context: Option<GraphContext>,
    stats: ResearchStats,
}

/// Aggregated hit data: (score, source_label, snippet, body, hop_distance, sub_query_ids).
type MergedHit = (f64, String, String, String, Option<usize>, Vec<usize>);

/// Intermediate result from a single sub-query execution.
pub(super) struct SubQueryResult {
    pub(super) sub_query_id: usize,
    /// (memory_id, score, source_label, snippet, body, hop_distance)
    pub(super) hits: Vec<(i64, f64, String, String, String, Option<usize>)>,
    /// Evidence chains reconstructed from BFS.
    pub(super) chains: Vec<EvidenceChain>,
}

/// Sync entry point — builds a tokio runtime for the async fan-out.
#[tracing::instrument(skip_all, level = "debug", name = "deep_research")]
pub fn run(
    args: DeepResearchArgs,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<(), AppError> {
    tracing::debug!(target: "deep_research", query = %args.query, k = args.k, "starting deep research");

    // GAP-001 (v1.1.04): resolve embeddings for every sub-query BEFORE the
    // multi-thread runtime is built. `compute_sub_embeddings` calls the
    // OpenRouter REST path, which internally does
    // `shared_runtime()?.block_on(...)`; running that inside the worker
    // threads of the runtime created below panics with
    // "Cannot start a runtime from within a runtime". Doing the work
    // synchronously here removes the nesting entirely.
    let paths = AppPaths::resolve(args.db.as_deref())?;
    crate::storage::connection::ensure_db_ready(&paths)?;
    // Resolve sub-queries once (shared by embedding precompute + fan-out).
    let sub_query_plan = resolve_sub_queries(&args)?;
    let sub_query_texts: Vec<String> = sub_query_plan.iter().map(|s| s.text.clone()).collect();
    let (sub_embeddings, vec_degraded) =
        compute_sub_embeddings(&paths, &sub_query_texts, embedding_backend, llm_backend);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to build tokio runtime: {e}")))?;
    rt.block_on(run_async(
        args,
        llm_backend,
        embedding_backend,
        sub_query_plan,
        sub_embeddings,
        vec_degraded,
    ))
}

/// Main async logic: decompose, fan-out, assemble, emit JSON.
///
/// `sub_embeddings` and `vec_degraded` are computed synchronously in
/// [`run`] before the tokio runtime is built (GAP-001, v1.1.04) to avoid
/// a nested-runtime panic on the OpenRouter embedding path.
/// `sub_queries` is also resolved in [`run`] so embedding precompute and
/// fan-out share one plan (v1.1.05).
async fn run_async(
    args: DeepResearchArgs,
    _llm_backend: crate::cli::LlmBackendChoice,
    _embedding_backend: crate::cli::EmbeddingBackendChoice,
    sub_queries: Vec<SubQuery>,
    sub_embeddings: Vec<Option<Arc<Vec<f32>>>>,
    vec_degraded: bool,
) -> Result<(), AppError> {
    let start = std::time::Instant::now();

    if args.query.trim().is_empty() {
        return Err(AppError::Validation(crate::i18n::validation::empty_query()));
    }

    if args.max_cost_usd.is_some() && args.mode == "none" {
        tracing::warn!(target: "deep_research", "--max-cost-usd has no effect without --mode claude-code/codex");
    }

    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let paths = AppPaths::resolve(args.db.as_deref())?;
    crate::storage::connection::ensure_db_ready(&paths)?;

    // Phase 1: sub-queries already resolved in `run` (heuristic / manual / aspects).
    let sub_query_texts: Vec<String> = sub_queries.iter().map(|s| s.text.clone()).collect();

    // GAP-001 (v1.1.04): sub-query embeddings were already resolved in
    // `run` before the tokio runtime was built. Using them here keeps the
    // OpenRouter REST path out of the worker threads (nested-runtime panic).
    // `vec_degraded` reflects per-sub-query FTS5 fallback (GAP-DEEPRESEARCH-001).
    if vec_degraded {
        tracing::debug!(target: "deep_research", "vector degraded: at least one sub-query fell back to FTS5");
    }

    // Phase 2: Fan-out — parallel sub-query execution.
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let permits = args
        .max_concurrency
        .unwrap_or_else(|| cpu_count.min(8))
        .min(sub_queries.len())
        .max(1);
    let semaphore = Arc::new(Semaphore::new(permits));
    let timeout_dur = std::time::Duration::from_secs(args.timeout);

    let mut join_set: JoinSet<Result<SubQueryResult, (usize, String)>> = JoinSet::new();

    for (idx, sq_text) in sub_query_texts.iter().enumerate() {
        let sem = Arc::clone(&semaphore);
        // GAP-DEEPRESEARCH-001 FIX: pass Optional embedding (None = FTS5-only).
        let emb = sub_embeddings[idx].clone();
        let ns = namespace.clone();
        let db_path = paths.db.clone();
        let query_text = sq_text.clone();
        let k = args.k;
        let max_hops = args.max_hops;
        let min_weight = args.min_weight;
        let rrf_k = args.rrf_k;
        let graph_decay = args.graph_decay;
        let graph_min_score = args.graph_min_score;
        let max_neighbors_per_hop = args.max_neighbors_per_hop;

        join_set.spawn(async move {
            let _permit = sem
                .acquire_owned()
                .await
                .map_err(|e| (idx, format!("semaphore closed: {e}")))?;

            // Dereference the Arc to obtain a &[f32] slice for the sync function.
            let result = tokio::time::timeout(timeout_dur, async move {
                execute_sub_query(
                    idx,
                    &query_text,
                    emb.as_ref().map(|v| v.as_slice()),
                    &ns,
                    &db_path,
                    k,
                    max_hops,
                    min_weight,
                    rrf_k,
                    graph_decay,
                    graph_min_score,
                    max_neighbors_per_hop,
                )
            })
            .await;

            match result {
                Ok(inner) => inner.map_err(|e| (idx, e)),
                Err(_) => Err((idx, "timeout".to_string())),
            }
        });
    }

    // Collect results incrementally.
    let mut sub_query_results: Vec<SubQueryResult> = Vec::with_capacity(sub_queries.len());
    let mut failed_count = 0usize;
    let mut timed_out_count = 0usize;

    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok(Ok(sqr)) => sub_query_results.push(sqr),
            Ok(Err((_idx, reason))) => {
                if reason == "timeout" {
                    timed_out_count += 1;
                } else {
                    failed_count += 1;
                }
                tracing::warn!(target: "deep_research", sub_query_id = _idx, reason = %reason, "sub-query failed");
            }
            Err(join_err) => {
                failed_count += 1;
                if join_err.is_panic() {
                    tracing::error!(target: "deep_research", error = %join_err, "sub-query task panicked");
                } else {
                    tracing::warn!(target: "deep_research", error = %join_err, "sub-query task cancelled");
                }
            }
        }
    }

    // Phase 3: Evidence assembly — merge, dedup, rank.
    // Aggregate hits: memory_id -> (best_score, source, snippet, body, hop_distance, sub_query_ids)
    let mut merged: crate::hash::AHashMap<i64, MergedHit> =
        crate::hash::AHashMap::with_capacity_and_hasher(
            sub_query_results.len() * args.k,
            Default::default(),
        );

    for sqr in &sub_query_results {
        for (mem_id, score, source, snippet, body, hop) in &sqr.hits {
            let entry = merged.entry(*mem_id).or_insert_with(|| {
                (
                    *score,
                    source.clone(),
                    snippet.clone(),
                    body.clone(),
                    *hop,
                    Vec::new(),
                )
            });
            // Keep best score.
            if *score > entry.0 {
                entry.0 = *score;
                entry.1 = source.clone();
                entry.2 = snippet.clone();
                entry.3 = body.clone();
                entry.4 = *hop;
            }
            if !entry.5.contains(&sqr.sub_query_id) {
                entry.5.push(sqr.sub_query_id);
            }
        }
    }

    // Resolve memory names for merged results.
    let conn = open_ro(&paths.db)?;
    let mut results: Vec<DeepResult> = Vec::with_capacity(merged.len().min(args.max_results));

    // Sort by score descending.
    let mut ranked: Vec<(i64, MergedHit)> = merged.into_iter().collect();
    ranked.sort_by(|a, b| {
        b.1 .0
            .partial_cmp(&a.1 .0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.truncate(args.max_results);

    for (mem_id, (score, source, snippet, body, hop, sq_ids)) in ranked {
        let name = match memories::read_full(&conn, mem_id)? {
            Some(row) => row.name,
            None => continue,
        };
        results.push(DeepResult {
            name,
            score,
            source,
            sub_query_ids: sq_ids,
            snippet,
            body: if args.with_bodies { Some(body) } else { None },
            hop_distance: hop,
        });
    }

    // GAP-09/10 FIX: Collect evidence chains from reconstructed BFS paths.
    // The old code appended flat node pairs from a global SELECT; now each
    // sub-query returns directed EvidenceChain structs (from, to, path).
    let completed_count = sub_query_results.len();
    let mut evidence_chains: Vec<EvidenceChain> = Vec::with_capacity(completed_count * 2);
    let mut seen_chain_keys: HashSet<String> = HashSet::with_capacity(completed_count * 2);

    for sqr in sub_query_results {
        for chain in sqr.chains {
            // Deduplicate chains by (from, to) pair.
            let key = format!("{}->{}", chain.from, chain.to);
            if seen_chain_keys.insert(key) {
                evidence_chains.push(chain);
            }
        }
    }

    // Sort evidence chains by total_weight descending, discard single-hop trivial chains.
    evidence_chains.retain(|c| c.depth >= 2);
    evidence_chains.sort_by(|a, b| {
        b.total_weight
            .partial_cmp(&a.total_weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let unique_memories = results.len();
    let evidence_count = evidence_chains.len();

    // MEDIUM-01b: Build graph_context with entities and relationships from result memories.
    let graph_context = if !results.is_empty() {
        let result_names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
        let mut ctx_entities: Vec<GraphContextEntity> = Vec::with_capacity(results.len());
        let mut ctx_rels: Vec<GraphContextRel> = Vec::with_capacity(results.len() * 2);
        let mut seen_entity_ids: crate::hash::AHashSet<i64> =
            crate::hash::AHashSet::with_capacity_and_hasher(results.len(), Default::default());

        for name in &result_names {
            if let Ok(Some(eid)) = entities::find_entity_id(&conn, &namespace, name) {
                if seen_entity_ids.insert(eid) {
                    let etype: String = conn
                        .query_row(
                            "SELECT COALESCE(type,'concept') FROM entities WHERE id = ?1",
                            rusqlite::params![eid],
                            |r| r.get(0),
                        )
                        .unwrap_or_else(|_| "concept".to_string());
                    let degree: u32 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM relationships WHERE source_id = ?1 OR target_id = ?1",
                            rusqlite::params![eid],
                            |r| r.get(0),
                        )
                        .unwrap_or(0);
                    ctx_entities.push(GraphContextEntity {
                        name: name.to_string(),
                        entity_type: etype,
                        degree,
                    });
                }
            }
        }

        let entity_ids: Vec<i64> = seen_entity_ids.iter().copied().collect();
        if entity_ids.len() >= 2 {
            let placeholders: String = entity_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT s.name, t.name, r.relation, r.weight \
                 FROM relationships r \
                 JOIN entities s ON s.id = r.source_id \
                 JOIN entities t ON t.id = r.target_id \
                 WHERE r.source_id IN ({placeholders}) AND r.target_id IN ({placeholders}) \
                 LIMIT 50"
            );
            if let Ok(mut stmt) = conn.prepare(&sql) {
                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
                    Vec::with_capacity(entity_ids.len() * 2);
                for id in &entity_ids {
                    params.push(Box::new(*id));
                }
                for id in &entity_ids {
                    params.push(Box::new(*id));
                }
                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                if let Ok(rows) = stmt.query_map(param_refs.as_slice(), |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, f64>(3)?,
                    ))
                }) {
                    for row in rows.flatten() {
                        ctx_rels.push(GraphContextRel {
                            from: row.0,
                            to: row.1,
                            relation: row.2,
                            weight: row.3,
                        });
                    }
                }
            }
        }

        if ctx_entities.is_empty() {
            None
        } else {
            Some(GraphContext {
                entities: ctx_entities,
                relationships: ctx_rels,
            })
        }
    } else {
        None
    };

    tracing::debug!(target: "deep_research",
        total_results = results.len(),
        total_chains = evidence_chains.len(),
        "assembly complete"
    );

    // Phase 4: JSON output (stdout and/or atomic --output).
    let response = DeepResearchResponse {
        query: args.query,
        sub_queries,
        results,
        evidence_chains,
        graph_context,
        stats: ResearchStats {
            sub_queries_total: sub_query_texts.len(),
            sub_queries_completed: completed_count,
            sub_queries_failed: failed_count,
            sub_queries_timed_out: timed_out_count,
            unique_memories_found: unique_memories,
            evidence_chains_found: evidence_count,
            elapsed_ms: start.elapsed().as_millis() as u64,
            vec_degraded,
        },
    };

    if let Some(path) = args.output.as_ref() {
        // v1.1.05 Bug 2: atomic write avoids truncated envelopes under SIGTERM /
        // shell redirect races. Full envelope goes to the file; stdout gets a
        // small confirmation so pipelines can still check exit 0 + path.
        // v1.1.8 GAP-CLI-DR-01..03: short `-o` is registered; fail-fast if the
        // path was requested and the final file is missing or empty.
        crate::atomic_io::write_json_atomic(path, &response)?;
        if !path.exists() {
            return Err(AppError::Validation(crate::i18n::validation::deep_research_output_missing(
                    &path.display().to_string(),
                )));
        }
        let meta = std::fs::metadata(path).map_err(AppError::Io)?;
        if meta.len() == 0 {
            return Err(AppError::Validation(crate::i18n::validation::deep_research_output_empty(&path.display().to_string())));
        }
        let file_bytes = std::fs::read(path).map_err(AppError::Io)?;
        let digest = blake3::hash(&file_bytes).to_hex().to_string();
        #[derive(Serialize)]
        struct WrittenAck {
            written: String,
            bytes: u64,
            blake3: String,
            sub_queries_total: usize,
            unique_memories_found: usize,
            elapsed_ms: u64,
        }
        output::emit_json(&WrittenAck {
            written: path.display().to_string(),
            bytes: meta.len(),
            blake3: digest,
            sub_queries_total: response.stats.sub_queries_total,
            unique_memories_found: response.stats.unique_memories_found,
            elapsed_ms: response.stats.elapsed_ms,
        })?;
    } else {
        output::emit_json(&response)?;
    }

    Ok(())
}

// Re-export sub_query_results field initialisation for the stats counter.
// The field is moved out of run_async after the join loop; we need to shadow it.
// ────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests;
