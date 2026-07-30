//! Body and entity embedding for `remember`.

use crate::chunking;
use crate::cli::{EmbeddingBackendChoice, LlmBackendChoice};
use crate::errors::AppError;
use crate::output;
use crate::paths::AppPaths;

/// Outcome of the passage + entity embedding stage.
pub(crate) struct EmbedPhaseOutcome {
    pub embedding: Option<Vec<f32>>,
    pub backend_invoked_passage: Option<&'static str>,
    pub chunk_embeddings_cache: Option<Vec<Vec<f32>>>,
    pub graph_entity_embeddings: Vec<Vec<f32>>,
    pub skip_embed: bool,
}

/// Embed the memory body (and entity names) for a remember write.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_embed_phase(
    paths: &AppPaths,
    raw_body: &str,
    chunks_info: &[crate::chunking::Chunk],
    graph: &super::graph_input::GraphInput,
    args: &super::args::RememberArgs,
    embedding_backend: EmbeddingBackendChoice,
    llm_backend: LlmBackendChoice,
) -> Result<EmbedPhaseOutcome, AppError> {
    output::emit_progress_i18n("Computing embedding...", "Calculando embedding...");
    let mut chunk_embeddings_cache: Option<Vec<Vec<f32>>> = None;

    // v1.0.84 (ADR-0042): extrai o backend que efetivamente executou o
    // passage embedding (or chunk batch) to populate
    // `backend_invoked` no envelope de resposta.
    let skip_embed = crate::embedder::should_skip_embedding_on_failure();
    let (embedding, backend_invoked_passage): (Option<Vec<f32>>, Option<&str>) = if chunks_info
    .len()
    == 1
{
    match crate::embedder::embed_passage_with_embedding_choice(
        &paths.models,
        raw_body,
        embedding_backend,
        llm_backend,
    ) {
        // GAP-CLI-EMBED-NONE (v1.1.8): intentional `--llm-backend none`
        // returns an empty vector — treat as no embedding (do not
        // persist a zero-dim vector).
        Ok((v, k)) if v.is_empty() => (None, Some(k.as_str())),
        Ok((v, k)) => (Some(v), Some(k.as_str())),
        // v1.1.2 (Gap 2): permanent payload rejections from the embedder
        // (typed ceilings) must not be swallowed by --skip-embedding-on-failure.
        Err(
            e @ (AppError::Validation(_)
            | AppError::BodyTooLarge { .. }
            | AppError::TooManyTokens { .. }),
        ) => return Err(e),
        Err(e) if skip_embed => {
            tracing::warn!(error = %e, "embedding failed; --skip-embedding-on-failure active, persisting without embedding");
            (None, None)
        }
        Err(e) => return Err(e),
    }
} else {
    let chunk_texts: Vec<String> = chunks_info
        .iter()
        .map(|c| chunking::chunk_text(raw_body, c).to_string())
        .collect();
    // G42/S2+S3 (v1.0.79): chunks are embedded in dim-adaptive
    // batches per LLM call (G44: clamp(base*64/dim, 1, base)), with up to
    // --llm-parallelism bounded subprocesses in flight. The old
    // serial loop spent SUM(items) wall time; the fan-out spends
    // roughly MAX(batch).
    output::emit_progress_i18n(
        &format!(
            "Embedding {} chunks in parallel batches (parallelism {})...",
            chunks_info.len(),
            args.llm_parallelism
        ),
        &format!(
            "Embedding {} chunks em lotes paralelos (paralelismo {})...",
            chunks_info.len(),
            args.llm_parallelism
        ),
    );
    if let Some(rss) = crate::memory_guard::current_process_memory_mb() {
        if rss > args.max_rss_mb {
            tracing::error!(target: "remember",
                rss_mb = rss,
                max_rss_mb = args.max_rss_mb,
                "RSS exceeded --max-rss-mb threshold; aborting to prevent system instability"
            );
            return Err(AppError::LowMemory {
                available_mb: crate::memory_guard::available_memory_mb(),
                required_mb: args.max_rss_mb,
            });
        }
    }
    match crate::embedder::embed_passages_parallel_with_embedding_choice(
        &paths.models,
        &chunk_texts,
        args.llm_parallelism as usize,
        crate::embedder::chunk_embed_batch_size(),
        embedding_backend,
        llm_backend,
    ) {
        Ok(chunk_embeddings) => {
            output::emit_progress_i18n(
                &format!(
                    "Remember stage: chunk embeddings complete; process RSS {} MB",
                    crate::memory_guard::current_process_memory_mb().unwrap_or(0)
                ),
                &format!(
                    "Stage remember: chunk embeddings completed; process RSS {} MB",
                    crate::memory_guard::current_process_memory_mb().unwrap_or(0)
                ),
            );
            let aggregated = chunking::aggregate_embeddings(&chunk_embeddings);
            chunk_embeddings_cache = Some(chunk_embeddings);
            (Some(aggregated), None)
        }
        Err(e) if skip_embed => {
            tracing::warn!(error = %e, "chunk embedding failed; --skip-embedding-on-failure active, persisting without embedding");
            (None, None)
        }
        Err(e) => return Err(e),
    }
};
    // G42/S2+A4 (v1.0.79): entity names are SHORT texts — they get their
    // own batch profile (25 per LLM call) instead of one subprocess per
    // 3-15 byte name (21 names used to cost ~12 minutes, 46% of the
    // measured remember total).
    let entity_texts: Vec<String> = graph
    .entities
    .iter()
    .map(|entity| match &entity.description {
        Some(desc) => format!("{} {}", entity.name, desc),
        None => entity.name.clone(),
    })
    .collect();
    // G56 (v1.0.80): route entity-name embedding through the in-process
    // cache. Repeated `remember` invocations within one CLI process — and
    // re-embedded entities inside a single batch — skip the LLM call
    // entirely when the (model, text) pair was already produced. The
    // chunk body embedding below still uses `embed_passages_parallel_local`
    // because chunks are unique per memory and the cache hit rate is
    // effectively zero.
    let (graph_entity_embeddings, embed_cache_stats) =
    match crate::embedder::embed_entity_texts_cached(
        &paths.models,
        &entity_texts,
        args.llm_parallelism as usize,
        embedding_backend,
        llm_backend,
    ) {
        Ok(r) => r,
        Err(e) if skip_embed => {
            tracing::warn!(error = %e, "entity embedding failed; --skip-embedding-on-failure active");
            let empty: Vec<Vec<f32>> = entity_texts.iter().map(|_| vec![]).collect();
            (empty, crate::embedder::EmbedCacheStats::default())
        }
        Err(e) => return Err(e),
    };
    if embed_cache_stats.hits > 0 {
    tracing::debug!(
        hits = embed_cache_stats.hits,
        misses = embed_cache_stats.misses,
        requested = embed_cache_stats.requested,
        "G56: entity embed cache hit (remember)"
    );
}

    Ok(EmbedPhaseOutcome {
        embedding,
        backend_invoked_passage,
        chunk_embeddings_cache,
        graph_entity_embeddings,
        skip_embed,
    })
}
