//! Multi-passage embedding entry points.
//!
//! The single public way a caller hands a corpus of passages to the embedder:
//! the shared `Arc` implementation that drives the OpenRouter REST batch API
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

/// Embeds many passages with `EmbeddingBackendChoice` awareness (GAP-SG-147).
///
/// THIS IS THE ONLY MULTI-PASSAGE ENTRY POINT. v1.0.93 (GAP-OR-INGEST): when
/// the resolved chain starts with `OpenRouter` and the client is initialised,
/// it uses the HTTP batch API (`embed_batch`) — no LLM slot consumed, ~200ms
/// per batch.
///
/// # Why the corpus arrives as an `Arc<[String]>`
///
/// A BORROWED slice cannot be handed to the `'static` fan-out tasks, so an
/// entry point taking `&[String]` has to clone the entire corpus on every
/// call: a 36k-passage backfill copies every string before a single request
/// leaves the process. That borrowed-slice shim existed until v1.2.8 and was
/// removed; this signature is the reason it was never needed.
///
/// Taking ownership through an `Arc<[String]>` lets the OpenRouter fan-out
/// hand each task a refcount bump plus an index range instead of a cloned
/// `Vec<String>` per chunk. `Arc::from(vec)` MOVES the string buffers into the
/// `Arc` allocation — only the 24-byte headers are memcpy'd, never the heap
/// data — so the same 36k-text backfill copies nothing. Callers that hold a
/// `Vec<String>` pay one `Arc::from(vec)` and are done.
///
/// Chunk boundaries and ordering are unchanged: chunk `i` still covers
/// `[i * chunk, min((i + 1) * chunk, len))` and `reassemble_ordered` still
/// sorts on that same index.
///
/// # Why `local_batch_size` reaches only ONE branch
///
/// The name is deliberate: this value governs the LOCAL (subprocess) branch and
/// is IGNORED under OpenRouter, which sizes its requests from XDG
/// `embedding.batch_size` through `fan_out_chunk`. That is not an oversight,
/// and "fixing" it would be a regression.
///
/// `adaptive_batch_for_dim`, which produces the value callers pass
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
pub fn embed_passages_parallel_shared(
    _models_dir: &Path,
    texts: Arc<[String]>,
    parallelism: usize,
    _local_batch_size: usize,
    backends: crate::cli::BackendChoice,
) -> Result<Vec<Vec<f32>>, AppError> {
    let crate::cli::BackendChoice {
        llm: llm_backend,
        embedding: embedding_backend,
    } = backends;
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
            .clamp(
                crate::constants::MIN_EMBED_PASSAGE_FAN_OUT,
                crate::constants::MAX_EMBED_PASSAGE_FAN_OUT,
            )
            .min(crate::constants::joint_parallelism_ceiling())
            .max(crate::constants::MIN_EMBED_PASSAGE_FAN_OUT);
        // Same knob as the fan-out slice: a corpus that fits in ONE request has
        // nothing to fan out, so the JoinSet would only add latency. Using a
        // literal here meant a lowered `embedding.batch_size` still sent short
        // corpora down the serial path, where the inner chunking then issued
        // several SEQUENTIAL requests instead of parallel ones.
        let chunk = fan_out_chunk();
        if texts.len() <= chunk || k == 1 {
            let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
            // GAP-001 (v1.1.04): canonical nested-runtime guard.
            // GAP-SG-270: preserve the origin-computed retry verdict instead of
            // letting `?` unwrap it away through `From<EmbedError>`.
            let vecs = match tokio::runtime::Handle::try_current() {
                Ok(handle) => tokio::task::block_in_place(|| {
                    handle.block_on(client.embed_batch(&refs, client.default_input_type()))
                })
                .map_err(crate::embedder::app_error_preserving_retry_class)?,
                Err(_) => shared_runtime()?
                    .block_on(client.embed_batch(&refs, client.default_input_type()))
                    .map_err(crate::embedder::app_error_preserving_retry_class)?,
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
                    // GAP-SG-270: `EmbedChunkResult` carries `AppError`, and the
                    // fan-out keeps the origin-computed `retry_class` inside it
                    // so the enrich re-embed queue still reads the verdict.
                    let r = client
                        .embed_batch(&refs, client.default_input_type())
                        .await
                        .map_err(crate::embedder::app_error_preserving_retry_class);
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
