//! Handler for the `hybrid-search` CLI subcommand.
//!
//! The pipeline runs in stages, one per submodule: [`args`] parses and
//! validates the flags, [`retrieval`] produces the vector and FTS5 candidate
//! lists, [`fusion`] combines them via RRF, [`graph_expansion`] widens the
//! answer through the entity graph, and [`envelope`] describes the JSON shape
//! that goes back to the caller.

use crate::errors::AppError;
use crate::output::{self, RecallItem};
use crate::paths::AppPaths;
use crate::storage::connection::open_ro;

mod args;
mod envelope;
mod fusion;
mod graph_expansion;
mod retrieval;

pub use args::HybridSearchArgs;
pub use envelope::{HybridSearchItem, HybridSearchResponse, Weights};

/// Run.
#[tracing::instrument(skip_all, level = "debug", name = "hybrid_search")]
pub fn run(
    args: HybridSearchArgs,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    fail_on_degraded: bool,
) -> Result<(), AppError> {
    let start = std::time::Instant::now();
    let _ = args.format;
    tracing::debug!(target: "hybrid_search", query = %args.query, k = args.k, "fusing results");

    args.validate_graph_flags()?;

    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let paths = AppPaths::resolve(args.db.as_deref())?;
    crate::storage::connection::ensure_db_ready(&paths)?;

    output::emit_progress_i18n(
        "Computing query embedding...",
        "Calculando embedding da consulta...",
    );
    let conn = open_ro(&paths.db)?;
    let resolved =
        retrieval::resolve_query_embedding(&args, &paths.models, embedding_backend, llm_backend);
    // `--fail-on-degraded` decide ANTES de qualquer consulta: sem isto a leitura
    // devolvia só-FTS com exit 0 e a flag era placebo. `degradation_failure` isenta
    // `--fallback-fts-only`, que é degradação PEDIDA pelo operador.
    if let Some(err) = crate::query_embedding::degradation_failure(
        fail_on_degraded,
        resolved.degraded,
        resolved.reason_code,
    ) {
        return Err(err);
    }
    let crate::query_embedding::QueryEmbedding {
        embedding,
        degraded: vec_degraded,
        error: vec_error,
        backend_invoked,
        ..
    } = resolved;

    let memory_type_str = args.r#type.map(|t| t.as_str());

    let vec_results = retrieval::vector_candidates(
        &conn,
        embedding.as_ref(),
        std::slice::from_ref(&namespace),
        memory_type_str,
        args.k,
    )?;

    let (fts_results, fts_degraded, fts_error, fts_auto_rebuilt) =
        retrieval::fts_candidates(&conn, &args, &namespace, memory_type_str);

    let results = fusion::fuse_candidates(&conn, &args, &vec_results, &fts_results)?;

    let graph_matches: Vec<RecallItem> =
        graph_expansion::expand(&conn, &args, embedding.as_ref(), &namespace, &results)?;

    output::emit_json(&HybridSearchResponse {
        query: args.query,
        k: args.k,
        rrf_k: args.rrf_k,
        weights: Weights {
            vec: args.weight_vec,
            fts: args.weight_fts,
        },
        results,
        graph_matches,
        max_graph_results: crate::constants::hybrid_search_max_graph_results(
            args.max_graph_results,
        ),
        fts_degraded,
        fts_error,
        fts_auto_rebuilt,
        vec_degraded,
        vec_error: vec_error.clone(),
        warning: if vec_degraded {
            Some(
                "live query embedding unavailable; results are FTS5 BM25 only (semantic relevance reduced)"
                    .to_string(),
            )
        } else {
            None
        },
        backend_invoked,
        vec_degraded_reason: if vec_degraded { vec_error } else { None },
        elapsed_ms: start.elapsed().as_millis() as u64,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests;
