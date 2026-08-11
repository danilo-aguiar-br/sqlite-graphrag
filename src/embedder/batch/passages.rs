//! Multi-passage embedding entry points.
//!
//! The two public ways a caller hands a corpus of passages to the embedder:
//! the borrowed-slice compatibility shim and the shared `Arc` implementation
//! that drives the OpenRouter REST batch API
//! (v1.0.93 GAP-OR-INGEST / GAP-SG-147).

use super::fan_out::{chunk_ranges, fan_out_chunk, reassemble_ordered};
use crate::embedder::{
    is_openrouter_initialized, shared_runtime, LlmBackendKind, OPENROUTER_CLIENT,
};
use crate::errors::AppError;
use std::path::Path;
use std::sync::Arc;
use tokio::task::JoinSet;

/// GAP-OPENROUTER-REST-CONCURRENCY: result of one bounded fan-out chunk —
/// the chunk index paired with the batch embedding result, used to restore
/// input order after out-of-order `JoinSet` completion.
type EmbedChunkResult = (usize, Result<Vec<Vec<f32>>, AppError>);

/// v1.0.93 (GAP-OR-INGEST): embeds multiple passages with
/// `EmbeddingBackendChoice` awareness. When the resolved chain starts
/// with `OpenRouter` and the client is initialised, uses the HTTP batch
/// API (`embed_batch`) — no LLM slot consumed, ~200ms per batch.
///
/// # Deprecated
///
/// This entry point takes a BORROWED slice, which cannot be handed to the
/// `'static` fan-out tasks, so it clones the entire corpus on every call — a
/// 36k-passage backfill copies every string before a single request is sent.
/// `embed_passages_parallel_shared` takes an `Arc<[String]>` instead and is
/// the real implementation this delegates to; migrating costs one
/// `Arc::from(vec)` at the call site and removes the copy.
#[deprecated(
    since = "1.2.3",
    note = "clones the whole corpus; use `embed_passages_parallel_shared`, which takes an \
            `Arc<[String]>` and is the real implementation this delegates to"
)]
pub fn embed_passages_parallel_with_embedding_choice(
    models_dir: &Path,
    texts: &[String],
    parallelism: usize,
    local_batch_size: usize,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    llm_backend: crate::cli::LlmBackendChoice,
) -> Result<Vec<Vec<f32>>, AppError> {
    // GAP-SG-147: COMPATIBILITY SHIM, not the main path. A borrowed slice
    // cannot be handed to `'static` tasks without owning it, so this copies
    // once and delegates. Do not add logic here — put it in
    // `embed_passages_parallel_shared`, which is the real implementation.
    embed_passages_parallel_shared(
        models_dir,
        Arc::from(texts.to_vec()),
        parallelism,
        local_batch_size,
        embedding_backend,
        llm_backend,
    )
}

/// Embeds many passages with `EmbeddingBackendChoice` awareness (GAP-SG-147).
///
/// THIS IS THE REAL IMPLEMENTATION.
/// [`embed_passages_parallel_with_embedding_choice`] is a thin compatibility
/// shim that copies a borrowed slice and calls straight into this function; it
/// only exists because that borrowed-slice signature is published API. Every
/// in-crate caller should land here, and any new behaviour belongs here.
///
/// Takes ownership of the corpus through an `Arc<[String]>` so the OpenRouter
/// fan-out can hand each task a refcount bump plus an index range instead of a
/// cloned `Vec<String>` per chunk. `Arc::from(vec)` MOVES the string buffers
/// into the `Arc` allocation — only the 24-byte headers are memcpy'd, never the
/// heap data — so a 36k-text backfill no longer copies the whole corpus.
///
/// Chunk boundaries and ordering are unchanged: chunk `i` still covers
/// `[i * chunk, min((i + 1) * chunk, len))` and [`reassemble_ordered`] still
/// sorts on that same index.
///
/// # Why `local_batch_size` reaches only ONE branch
///
/// The name is deliberate: this value governs the LOCAL (subprocess) branch and
/// is IGNORED under OpenRouter, which sizes its requests from XDG
/// `embedding.batch_size` through [`fan_out_chunk`]. That is not an oversight,
/// and "fixing" it would be a regression.
///
/// [`super::adaptive_batch_for_dim`], which produces the value callers pass
/// here, was calibrated against SUBPROCESS backends. Its failure mode is an LLM
/// completing a prompt and truncating the JSON reply: at dim 384 with a fixed
/// batch of 8, claude returned 3 of 8 items and codex timed out at 300s.
/// Shrinking the batch as dimensionality grows is what keeps that from
/// happening.
///
/// The REST path cannot fail that way. OpenRouter exposes a native batch
/// embedding API whose response is structured API JSON, not a model completion,
/// so there is no token budget to truncate.
///
/// The cost of unifying them is concrete: `adaptive_batch_for_dim(8, 1024)`
/// resolves to `1` at this project's active dimensionality. Letting the
/// dim-adaptive value win on the REST path would collapse every request to a
/// single text and destroy the 32x batching win of GAP-SG-141.
///
/// `openrouter_branch_ignores_local_batch_size` in this module's tests fails if
/// the OpenRouter branch ever starts reading this parameter.
pub(crate) fn embed_passages_parallel_shared(
    _models_dir: &Path,
    texts: Arc<[String]>,
    parallelism: usize,
    _local_batch_size: usize,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    llm_backend: crate::cli::LlmBackendChoice,
) -> Result<Vec<Vec<f32>>, AppError> {
    let texts: &Arc<[String]> = &texts;
    let chain = embedding_backend.to_chain(llm_backend);
    if chain.first() == Some(&LlmBackendKind::OpenRouter) && is_openrouter_initialized() {
        let client = OPENROUTER_CLIENT.get().ok_or_else(|| {
            AppError::Embedding(
                crate::i18n::validation::embedding_openrouter_client_not_initialised(),
            )
        })?;

        // GAP-OPENROUTER-REST-CONCURRENCY: reuse the caller's `parallelism`
        // as a bounded fan-out width, clamped to a Cloudflare-safe range.
        // Small inputs stay serial — a single batch is one REST call, so the
        // JoinSet overhead would only add latency.
        // The joint cap also applies here: `--max-concurrency` bounds how many
        // CLI processes run, this bounds how wide each one fans out, and only
        // their PRODUCT describes the load on the host.
        let k = parallelism
            .clamp(1, 16)
            .min(crate::constants::joint_parallelism_ceiling())
            .max(1);
        // Same knob as the fan-out slice: a corpus that fits in ONE request has
        // nothing to fan out, so the JoinSet would only add latency. Using a
        // literal here meant a lowered `embedding.batch_size` still sent short
        // corpora down the serial path, where the inner chunking then issued
        // several SEQUENTIAL requests instead of parallel ones.
        let chunk = fan_out_chunk();
        if texts.len() <= chunk || k == 1 {
            let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
            // GAP-001 (v1.1.04): canonical nested-runtime guard.
            let vecs = match tokio::runtime::Handle::try_current() {
                Ok(handle) => tokio::task::block_in_place(|| {
                    handle.block_on(client.embed_batch(&refs, client.default_input_type()))
                })?,
                Err(_) => shared_runtime()?
                    .block_on(client.embed_batch(&refs, client.default_input_type()))?,
            };
            return Ok(vecs);
        }

        // `client` is a `&'static OpenRouterClient` (OPENROUTER_CLIENT is a
        // static OnceLock), so it is Copy + Send + 'static and moves freely
        // into each spawned task.
        //
        // GAP-SG-147: each task used to receive `chunk.to_vec()`, an owned
        // copy of its slice, purely to satisfy the `'static` bound on
        // `JoinSet::spawn`. Summed over the disjoint chunks that copied the
        // entire corpus once per call. Now the task captures an `Arc` clone
        // (a refcount bump) plus the chunk's index range and slices the shared
        // allocation itself, so nothing is copied.
        //
        // GAP-001 (v1.1.04): canonical nested-runtime guard. The async block
        // borrows `client`, `texts` and `k`, all of which remain valid for
        // both branches.
        let fan_out = async move {
            let mut set: JoinSet<EmbedChunkResult> = JoinSet::new();
            let mut parts: Vec<(usize, Vec<Vec<f32>>)> = Vec::new();

            for (idx, range) in chunk_ranges(texts.len(), chunk).enumerate() {
                if set.len() >= k {
                    if let Some(joined) = set.join_next().await {
                        let (cidx, res) = joined.map_err(|e| {
                            AppError::Embedding(crate::i18n::validation::embedding_task_join_error(
                                e,
                            ))
                        })?;
                        parts.push((cidx, res?));
                    }
                }
                let shared = Arc::clone(texts);
                set.spawn(async move {
                    let refs: Vec<&str> =
                        shared[range.clone()].iter().map(|s| s.as_str()).collect();
                    // `EmbedChunkResult` carries `AppError` (retry_class is
                    // only consumed by callers that match `EmbedError`
                    // directly, e.g. the enrich re-embed path).
                    let r = client
                        .embed_batch(&refs, client.default_input_type())
                        .await
                        .map_err(AppError::from);
                    (idx, r)
                });
            }

            while let Some(joined) = set.join_next().await {
                let (cidx, res) = joined.map_err(|e| {
                    AppError::Embedding(crate::i18n::validation::embedding_task_join_error(e))
                })?;
                parts.push((cidx, res?));
            }

            Ok::<Vec<Vec<f32>>, AppError>(reassemble_ordered(parts))
        };
        let vecs = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fan_out))?,
            Err(_) => shared_runtime()?.block_on(fan_out)?,
        };
        Ok(vecs)
    } else {
        Err(AppError::Embedding(
            crate::i18n::validation::embedding_openrouter_client_not_initialised(),
        ))
    }
}
