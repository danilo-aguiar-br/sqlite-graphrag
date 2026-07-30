//! Passage/query embedding entry points.

use super::*;
use crate::errors::AppError;
use crate::extract::llm_embedding::LlmEmbedding;
use parking_lot::Mutex;
use std::path::Path;


/// Embeds a single passage for storage. Delegates to the configured LLM
/// headless (claude code / codex). Returns a vector of the active
/// dimensionality.
pub fn embed_passage(embedder: &Mutex<LlmEmbedding>, text: &str) -> Result<Vec<f32>, AppError> {
    let client = apply_query_timeout_if_needed(clone_client(embedder));
    let result = client.embed_passage(text)?;
    validate_dim(result)
}

/// Embeds a single query for similarity search. Same model and dim as
/// `embed_passage`; the only difference is the LLM-side prompt prefix
/// that the headless invocation uses to disambiguate.
pub fn embed_query(embedder: &Mutex<LlmEmbedding>, text: &str) -> Result<Vec<f32>, AppError> {
    let client = apply_query_timeout_if_needed(clone_client(embedder));
    let result = client.embed_query(text)?;
    validate_dim(result)
}

/// Embeds a batch of passages with token-count-aware batching.
///
/// Kept for API compatibility; since v1.0.79 it routes through the
/// bounded parallel fan-out with conservative defaults.
pub fn embed_passages_controlled(
    embedder: &Mutex<LlmEmbedding>,
    texts: &[&str],
    _token_counts: &[usize],
) -> Result<Vec<Vec<f32>>, AppError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let owned: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
    embed_texts_parallel(embedder, &owned, 1, chunk_embed_batch_size())
}

/// Embed passage local.
pub fn embed_passage_local(models_dir: &Path, text: &str) -> Result<Vec<f32>, AppError> {
    let _slot_guard = acquire_llm_slot_for_embedding()?;
    let embedder = get_embedder(models_dir)?;
    embed_passage(embedder, text)
}

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

/// BUG-003 / v1.0.85: split of `embed_passage_local` that reports the
/// resolved [`LlmBackendKind`] based on the ACTUAL
/// [`LlmEmbedding::flavour`] of the embedder constructed. When
/// `LlmEmbedding::detect_available` substitutes claude for a missing
/// codex, the operator sees the truth in `envelope.backend_invoked`.
pub fn embed_passage_local_resolved(
    models_dir: &Path,
    text: &str,
) -> Result<(Vec<f32>, LlmBackendKind), AppError> {
    let _slot_guard = acquire_llm_slot_for_embedding()?;
    let embedder = get_embedder(models_dir)?;
    let v = embed_passage(embedder, text)?;
    let kind = match embedder.lock().flavour() {
        crate::extract::llm_embedding::EmbeddingFlavour::Codex => LlmBackendKind::Codex,
        crate::extract::llm_embedding::EmbeddingFlavour::Claude => LlmBackendKind::Claude,
        crate::extract::llm_embedding::EmbeddingFlavour::Opencode => LlmBackendKind::Opencode,
    };
    Ok((v, kind))
}

/// Embed query local.
pub fn embed_query_local(models_dir: &Path, text: &str) -> Result<Vec<f32>, AppError> {
    let _slot_guard = acquire_llm_slot_for_embedding()?;
    let embedder = get_embedder(models_dir)?;
    embed_query(embedder, text)
}

// =============================================================================
// v1.0.82 (GAP-003): wrappers que aceitam a escolha do CLI
// (`crate::cli::LlmBackendChoice`) e a traduzem em uma chain para
// `embed_with_fallback`. Centralizam a propagação do flag `--llm-backend`
// nos 6 comandos que produzem embedding (`remember`, `edit`, `ingest`,
// `enrich`, `recall`, `hybrid-search`).
// =============================================================================

/// Embed a single passage using the LLM backend selected by the user via
/// `--llm-backend`. Routes to `embed_with_fallback` so failures fall
/// through to the next backend in the chain before giving up.
///
/// When `choice` is `None` (e.g. a sub-command that does not yet
/// expose the flag), behaviour matches `embed_passage_local` — the
/// active embedder from `LlmEmbedding::detect_available` decides the
/// backend.
pub fn embed_passage_with_choice(
    models_dir: &Path,
    text: &str,
    choice: Option<crate::cli::LlmBackendChoice>,
) -> Result<(Vec<f32>, LlmBackendKind), AppError> {
    let _slot_guard = acquire_llm_slot_for_embedding()?;
    match choice {
        None => {
            let embedder = get_embedder(models_dir)?;
            embed_passage(embedder, text).map(|v| (v, LlmBackendKind::None))
        }
        Some(choice) => embed_with_fallback(models_dir, text, &choice.to_chain(), false),
    }
}

/// v1.0.93: embedding with `EmbeddingBackendChoice` awareness. When the
/// embedding backend is `Openrouter` or `Auto` with a live client, the
/// chain prepends `OpenRouter` before the LLM subprocess backends.
pub fn embed_passage_with_embedding_choice(
    models_dir: &Path,
    text: &str,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    llm_backend: crate::cli::LlmBackendChoice,
) -> Result<(Vec<f32>, LlmBackendKind), AppError> {
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
    match with_query_embed_fast(|| embed_passage_with_choice(models_dir, text, choice)) {
        // GAP-004 / v1.0.85.1: when the chain terminates on
        //  (i.e. user passed
        // or every preceding backend failed),  returns
        //  instead of an error. Without this guard the
        // empty vector would propagate to  which
        // aborts with exit 11 ("embedding has 0 dims, expected 64").
        // The caller's contract is to surface a typed
        // so  and  can route to FTS5-puro via
        // the existing  /  envelope.
        // Intercept the empty-vector success path and surface it as
        //  (introduced at v1.0.85 / ADR-0043
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
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    llm_backend: crate::cli::LlmBackendChoice,
) -> Result<(Vec<f32>, LlmBackendKind), FallbackReason> {
    match with_query_embed_fast(|| {
        embed_passage_with_embedding_choice(models_dir, text, embedding_backend, llm_backend)
    }) {
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
