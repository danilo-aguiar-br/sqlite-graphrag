//! Passage/query embedding entry points.

use super::*;
use crate::errors::AppError;
use std::path::Path;

/// v1.0.89 (BUG-SKIP-EMBED): reads `--skip-embedding-on-failure` / runtime_config
/// (flag > XDG; product env is not read).
/// Returns `true` when the user opted to persist with NULL embedding on failure.
pub fn should_skip_embedding_on_failure() -> bool {
    crate::runtime_config::skip_embedding_on_failure()
}

/// v1.0.89 (BUG-SKIP-EMBED + GAP-EMBED-PROPAGATION): embed a passage
/// honouring both `--llm-backend` and `--skip-embedding-on-failure`.
///
/// On success returns `Ok(Some(vec))`. On failure:
/// - if `--skip-embedding-on-failure` is active, logs a warning and returns `Ok(None)`
/// - otherwise propagates the error (exit 11)
pub fn embed_passage_or_skip(
    models_dir: &Path,
    text: &str,
    choice: Option<crate::cli::LlmBackendChoice>,
) -> Result<Option<Vec<f32>>, AppError> {
    match embed_passage_with_choice(models_dir, text, choice) {
        Ok((v, _backend)) => Ok(Some(v)),
        Err(AppError::Validation(msg)) => Err(AppError::Validation(msg)),
        Err(e) => {
            if should_skip_embedding_on_failure() {
                tracing::warn!(
                    error = %e,
                    "embedding failed but --skip-embedding-on-failure is active; persisting with NULL embedding"
                );
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

// =============================================================================
// v1.0.82 (GAP-003): wrappers that take the CLI choice
// (`crate::cli::LlmBackendChoice`) and translate it into a chain for
// `embed_with_fallback`. They centralize the propagation of the
// `--llm-backend` flag across the 6 commands that produce embeddings
// (`remember`, `edit`, `ingest`, `enrich`, `recall`, `hybrid-search`).
// =============================================================================

/// Embed a single passage using the LLM backend selected by the user via
/// `--llm-backend`. Routes to `embed_with_fallback` so failures fall
/// through to the next backend in the chain before giving up.
///
/// When `choice` is `None` (e.g. a sub-command that does not yet
/// expose the flag), the default `OpenRouter` chain is used.
pub fn embed_passage_with_choice(
    models_dir: &Path,
    text: &str,
    choice: Option<crate::cli::LlmBackendChoice>,
) -> Result<(Vec<f32>, LlmBackendKind), AppError> {
    let _slot_guard = acquire_llm_slot_for_embedding()?;
    let chain = choice
        .unwrap_or(crate::cli::LlmBackendChoice::OpenRouter)
        .to_chain();
    embed_with_fallback(models_dir, text, &chain, false)
}

/// v1.0.93: embedding with `EmbeddingBackendChoice` awareness.
pub fn embed_passage_with_embedding_choice(
    models_dir: &Path,
    text: &str,
    backends: crate::cli::BackendChoice,
) -> Result<(Vec<f32>, LlmBackendKind), AppError> {
    let crate::cli::BackendChoice {
        llm: llm_backend,
        embedding: embedding_backend,
    } = backends;
    let _slot_guard = acquire_llm_slot_for_embedding()?;
    let chain = embedding_backend.to_chain(llm_backend);
    embed_with_fallback(models_dir, text, &chain, false)
}

/// failure, returns a structured `FallbackReason` so the caller can
/// surface `vec_degraded` instead of a hard exit 11.
///
/// `None` matches the legacy `try_embed_query_with_fallback` path
/// (uses the active embedder without an explicit chain).
pub fn try_embed_query_with_choice(
    models_dir: &Path,
    text: &str,
    choice: Option<crate::cli::LlmBackendChoice>,
) -> Result<(Vec<f32>, LlmBackendKind), FallbackReason> {
    match embed_passage_with_choice(models_dir, text, choice) {
        // GAP-004 / v1.0.85.1: when the chain terminates on
        // `LlmBackendKind::None` (i.e. the user passed `--llm-backend none`,
        // or every preceding backend failed), `embed_with_fallback` returns
        // `Ok((vec![], LlmBackendKind::None))` instead of an error. Without
        // this guard the empty vector would propagate to the dimension check,
        // which aborts with exit 11 ("embedding has 0 dims, expected 64").
        // The caller's contract here is to surface a typed `FallbackReason`
        // instead, so `recall` and `hybrid-search` can route to FTS5-puro via
        // the existing `vec_degraded` / `vec_degraded_reason` envelope.
        // Intercept the empty-vector success path and surface it as
        // `FallbackReason::DimZero` (introduced at v1.0.85 / ADR-0043
        // for the symmetric LLM-returned-zero-dim case).
        Ok((v, _backend)) if v.is_empty() => Err(FallbackReason::DimZero),
        Ok((v, backend)) => Ok((v, backend)),
        Err(e) => Err(classify_embedding_error(e)),
    }
}
/// v1.0.93 (GAP-OR-INGEST): query embedding with `EmbeddingBackendChoice`
/// awareness. Mirrors `try_embed_query_with_choice` but routes through
/// `embed_passage_with_embedding_choice` so OpenRouter API is used when
/// configured.
pub fn try_embed_query_with_embedding_choice(
    models_dir: &Path,
    text: &str,
    backends: crate::cli::BackendChoice,
) -> Result<(Vec<f32>, LlmBackendKind), FallbackReason> {
    match embed_passage_with_embedding_choice(models_dir, text, backends) {
        Ok((v, _backend)) if v.is_empty() => Err(FallbackReason::DimZero),
        Ok((v, backend)) => Ok((v, backend)),
        Err(e) => Err(classify_embedding_error(e)),
    }
}

/// call. Reads max-concurrency from `--llm-max-host-concurrency` /
/// XDG `llm.max_host_concurrency` (default derived from `LLM_WORKER_RSS_MB`
/// and available memory), and the wait timeout from XDG
/// `llm.slot_wait_secs` (default 30s).
///
/// Returns `Ok(guard)` for happy path, `AppError::LockBusy` (exit 75)
/// when no slot is available within the wait window, and
/// `AppError::Validation` when the concurrency is 0.
///
/// Tests may force fail-fast via XDG/runtime slot wait of 0.
pub(crate) fn acquire_llm_slot_for_embedding() -> Result<crate::llm_slots::LlmSlotGuard, AppError> {
    use crate::constants::{CLI_LOCK_DEFAULT_WAIT_SECS, LLM_WORKER_RSS_MB};
    let default_max = crate::llm_slots::default_max_concurrency() as usize;
    let max = crate::runtime_config::llm_max_host_concurrency(default_max).max(1) as u32;
    let wait_secs = if crate::runtime_config::llm_slot_no_wait() {
        0
    } else {
        crate::runtime_config::llm_slot_wait_secs(CLI_LOCK_DEFAULT_WAIT_SECS)
    };
    let _ = LLM_WORKER_RSS_MB; // silence the unused import (used in default_max_concurrency)
                               // GAP-003 / ADR-0043: when the slot semaphore is contended beyond the
                               // backoff window (50 + 100 + 200 + 400 = 750ms total), return a
                               // marker message that `classify_embedding_error` maps to
                               // `FallbackReason::SlotExhausted` (discriminator `slot_exhausted`).
                               // The window is shorter than the legacy 30s timeout, so the operator
                               // observes FTS5-puro fallback quickly instead of after 30s of silence.
    match crate::llm_slots::acquire_llm_slot(max, wait_secs) {
        Ok(guard) => Ok(guard),
        Err(e @ AppError::LockBusy { .. }) if wait_secs > 0 => Err(AppError::Embedding(
            crate::i18n::validation::embedding_slot_exhausted(&e),
        )),
        Err(e) => Err(e),
    }
}
