//! Batch sizing, bounded parallel fan-out and entity-embed cache.
//!
//! G42/S2–S3 / G44 / G56: adaptive batch sizes, `Arc<Semaphore>` permit
//! accounting, OpenRouter REST batch fan-out, and the process-wide entity
//! embedding cache. Split from the root embedder module (R-SRP-01) so the
//! single-vector paths stay independent of the multi-text orchestration.

use super::{
    clone_client, embed_passage, embed_passages_controlled, get_embedder,
    is_openrouter_initialized, shared_runtime, LlmBackendKind, OPENROUTER_CLIENT,
};
use crate::errors::AppError;
use crate::extract::llm_embedding::LlmEmbedding;
use parking_lot::Mutex;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

/// Calibration base: chunk (long-text) batch size per LLM call at the
/// calibration dimensionality (G42/S2). Use [`chunk_embed_batch_size`]
/// for the dim-adaptive value (G44).
pub const CHUNK_EMBED_BATCH_SIZE: usize = 8;

/// Calibration base: entity-name (short-text) batch size per LLM call at
/// the calibration dimensionality (G42/S2). Use [`entity_embed_batch_size`]
/// for the dim-adaptive value (G44).
pub const ENTITY_EMBED_BATCH_SIZE: usize = 25;

/// Dimensionality the batch bases above were calibrated against (G44).
pub const EMBED_BATCH_CALIBRATION_DIM: usize = 64;

/// G44: scales a calibration-base batch size to the active dimensionality,
/// keeping the float budget per LLM call constant (~512 floats for chunks,
/// ~1600 for entity names — the budgets empirically validated at dim 64).
/// Fixed batches of 8 at 384 dims asked for ~3072 floats per response:
/// claude returned partial coverage (3 of 8 items, caught by the G42/C5
/// check) and codex timed out at 300s. `base.max(1)` keeps the function
/// total — `clamp` panics when the upper bound is below the lower one.
pub(crate) fn adaptive_batch_for_dim(base: usize, dim: usize) -> usize {
    let base = base.max(1);
    (base * EMBED_BATCH_CALIBRATION_DIM / dim.max(1)).clamp(1, base)
}

/// Dim-adaptive batch size for chunk (long-text) embedding calls (G44).
pub fn chunk_embed_batch_size() -> usize {
    let dim = crate::constants::embedding_dim();
    let batch = adaptive_batch_for_dim(CHUNK_EMBED_BATCH_SIZE, dim);
    tracing::debug!(
        dim,
        base = CHUNK_EMBED_BATCH_SIZE,
        batch,
        "adaptive chunk batch size (G44)"
    );
    batch
}

/// Dim-adaptive batch size for entity-name (short-text) embedding calls (G44).
pub fn entity_embed_batch_size() -> usize {
    let dim = crate::constants::embedding_dim();
    let batch = adaptive_batch_for_dim(ENTITY_EMBED_BATCH_SIZE, dim);
    tracing::debug!(
        dim,
        base = ENTITY_EMBED_BATCH_SIZE,
        batch,
        "adaptive entity batch size (G44)"
    );
    batch
}

/// Embed passages controlled local.
pub fn embed_passages_controlled_local(
    models_dir: &Path,
    texts: &[&str],
    token_counts: &[usize],
) -> Result<Vec<Vec<f32>>, AppError> {
    let embedder = get_embedder(models_dir)?;
    embed_passages_controlled(embedder, texts, token_counts)
}

/// G42/S3: embeds `texts` through the bounded parallel fan-out and
/// returns vectors in input order.
pub fn embed_passages_parallel_local(
    models_dir: &Path,
    texts: &[String],
    parallelism: usize,
    batch_size: usize,
) -> Result<Vec<Vec<f32>>, AppError> {
    let embedder = get_embedder(models_dir)?;
    embed_texts_parallel(embedder, texts, parallelism, batch_size)
}

/// GAP-OPENROUTER-REST-CONCURRENCY: result of one bounded fan-out chunk —
/// the chunk index paired with the batch embedding result, used to restore
/// input order after out-of-order `JoinSet` completion.
type EmbedChunkResult = (usize, Result<Vec<Vec<f32>>, AppError>);

/// GAP-OPENROUTER-REST-CONCURRENCY: reassembles the flat vector list in
/// input order from chunk parts produced out-of-order by the bounded
/// `JoinSet` fan-out. Sorts by chunk index, then flattens, so the result
/// matches the original `texts` order exactly.
pub(crate) fn reassemble_ordered(mut parts: Vec<(usize, Vec<Vec<f32>>)>) -> Vec<Vec<f32>> {
    parts.sort_by_key(|(idx, _)| *idx);
    parts.into_iter().flat_map(|(_, v)| v).collect()
}

/// v1.0.93 (GAP-OR-INGEST): embeds multiple passages with
/// `EmbeddingBackendChoice` awareness. When the resolved chain starts
/// with `OpenRouter` and the client is initialised, uses the HTTP batch
/// API (`embed_batch`) instead of subprocess fan-out — no LLM slot
/// consumed, ~200ms per batch vs ~15s per subprocess cold-start.
/// Falls back to `embed_passages_parallel_local` for LLM backends.
pub fn embed_passages_parallel_with_embedding_choice(
    models_dir: &Path,
    texts: &[String],
    parallelism: usize,
    batch_size: usize,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    llm_backend: crate::cli::LlmBackendChoice,
) -> Result<Vec<Vec<f32>>, AppError> {
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
        let k = parallelism.clamp(1, 16);
        if texts.len() <= 32 || k == 1 {
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
        // into each spawned task. Chunk contents are cloned into owned
        // `Vec<String>` because `texts` is only borrowed.
        //
        // GAP-001 (v1.1.04): canonical nested-runtime guard. The async block
        // borrows `client`, `texts` and `k`, all of which remain valid for
        // both branches.
        let fan_out = async move {
            let mut set: JoinSet<EmbedChunkResult> = JoinSet::new();
            let mut parts: Vec<(usize, Vec<Vec<f32>>)> = Vec::new();

            for (idx, chunk) in texts.chunks(32).enumerate() {
                if set.len() >= k {
                    if let Some(joined) = set.join_next().await {
                        let (cidx, res) = joined.map_err(|e| {
                            AppError::Embedding(
                                crate::i18n::validation::embedding_task_join_error(e),
                            )
                        })?;
                        parts.push((cidx, res?));
                    }
                }
                let owned: Vec<String> = chunk.to_vec();
                set.spawn(async move {
                    let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
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
        embed_passages_parallel_local(models_dir, texts, parallelism, batch_size)
    }
}

/// G56: in-process cache for entity embeddings keyed by `(model, text)`.
///
/// Schema v13 is immutable: `entity_embeddings` does not have a `text`
/// column, so a pure DB-side cache would require a schema bump. Instead
/// we keep a process-wide LRU-style map that survives within one CLI
/// invocation. The hit rate is high in `ingest` (re-embedding the same
/// canonical entity across thousands of memories) and modest in `remember`
/// (typical single-memory invocations).
///
/// Key: `blake3(model || "\0" || text)`. Value: `Arc<Vec<f32>>` so the
/// collector can drop the map entry while a `Vec` is still in flight.
type EntityEmbedCacheMap = std::collections::HashMap<u64, Arc<Vec<f32>>>;

static ENTITY_EMBED_CACHE: OnceLock<parking_lot::Mutex<EntityEmbedCacheMap>> = OnceLock::new();

pub(crate) fn entity_embed_cache() -> &'static parking_lot::Mutex<EntityEmbedCacheMap> {
    ENTITY_EMBED_CACHE.get_or_init(|| parking_lot::Mutex::new(std::collections::HashMap::new()))
}

pub(crate) fn entity_cache_key(model: &str, text: &str) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(model.as_bytes());
    hasher.update(b"\0");
    hasher.update(text.as_bytes());
    let h = hasher.finalize();
    let bytes = h.as_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// G56: embeds entity-name texts through a process-wide cache.
///
/// Skips any `(model, text)` pair already produced in this CLI invocation
/// and only spawns subprocesses for the cache misses. Returns vectors in
/// the same order as `texts`.
///
/// Designed for entity-name batches (short texts). For chunk embeds use
/// [`embed_passages_parallel_local`] directly — chunks are unique per
/// memory and cache hit rate is negligible.
pub fn embed_entity_texts_cached(
    models_dir: &Path,
    texts: &[String],
    parallelism: usize,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    llm_backend: crate::cli::LlmBackendChoice,
) -> Result<(Vec<Vec<f32>>, EmbedCacheStats), AppError> {
    if texts.is_empty() {
        return Ok((Vec::new(), EmbedCacheStats::default()));
    }
    // GAP-OR-ENTITY-EMBED: resolve the SAME chain the chunk path uses so the
    // entity embedding honours `--embedding-backend`/`--llm-backend` instead
    // of always forcing the codex subprocess (the old G56 code path).
    let chain = embedding_backend.to_chain(llm_backend);

    // `none` short-circuit: when the resolved chain is exactly `[None]`
    // (`--embedding-backend llm --llm-backend none`) skip every backend and
    // return empty vectors WITHOUT spawning a subprocess. Empties are never
    // cached so a later call with a real backend in the same process is not
    // poisoned; they count as misses for stats parity with the chunk path.
    if chain.as_slice() == [LlmBackendKind::None] {
        let out: Vec<Vec<f32>> = texts.iter().map(|_| Vec::new()).collect();
        return Ok((
            out,
            EmbedCacheStats {
                requested: texts.len(),
                hits: 0,
                misses: texts.len(),
            },
        ));
    }

    // Cache model label reflects the EFFECTIVE embedding backend. When the
    // chain actually routes through OpenRouter, vectors carry that model's
    // dim/MRL profile and must never collide with codex-produced vectors;
    // for the local path we keep the prior `model_label()` so the in-process
    // cache key is unchanged (no regression — this cache is process-local).
    let routed_openrouter =
        chain.first() == Some(&LlmBackendKind::OpenRouter) && is_openrouter_initialized();
    let model = if routed_openrouter {
        format!("openrouter:{}", crate::constants::embedding_dim())
    } else {
        get_embedder(models_dir)?.lock().model_label()
    };
    let cache = entity_embed_cache();
    let mut hits: Vec<Option<Arc<Vec<f32>>>> = vec![None; texts.len()];
    let mut miss_indices: Vec<usize> = Vec::with_capacity(texts.len());
    {
        let guard = cache.lock();
        for (i, text) in texts.iter().enumerate() {
            let key = entity_cache_key(&model, text);
            if let Some(v) = guard.get(&key) {
                hits[i] = Some(Arc::clone(v));
            } else {
                miss_indices.push(i);
            }
        }
    }
    let miss_count = miss_indices.len();
    if miss_count > 0 {
        let miss_texts: Vec<String> = miss_indices.iter().map(|&i| texts[i].clone()).collect();
        // GAP-OR-ENTITY-EMBED: route misses through the backend-aware batch
        // helper (same one the chunk path uses). With OpenRouter this hits the
        // REST `embed_batch` (~200ms) instead of the codex subprocess (~120s).
        let miss_vecs = embed_passages_parallel_with_embedding_choice(
            models_dir,
            &miss_texts,
            parallelism,
            entity_embed_batch_size(),
            embedding_backend,
            llm_backend,
        )?;
        let mut guard = cache.lock();
        for (slot, &orig_idx) in miss_indices.iter().enumerate() {
            let vec = Arc::new(miss_vecs[slot].clone());
            let key = entity_cache_key(&model, &texts[orig_idx]);
            guard.insert(key, Arc::clone(&vec));
            hits[orig_idx] = Some(vec);
        }
    }
    let mut out = Vec::with_capacity(texts.len());
    for hit in hits.into_iter() {
        let v = hit.ok_or_else(|| {
            AppError::Embedding(crate::i18n::validation::embedding_entity_cache_null())
        })?;
        out.push((*v).clone());
    }
    Ok((
        out,
        EmbedCacheStats {
            requested: texts.len(),
            hits: texts.len() - miss_count,
            misses: miss_count,
        },
    ))
}

/// G56: stats snapshot returned by [`embed_entity_texts_cached`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct EmbedCacheStats {
    /// Requested.
    pub requested: usize,
    /// Hits.
    pub hits: usize,
    /// Misses.
    pub misses: usize,
}

impl EmbedCacheStats {
    /// Hit rate as a fraction in `[0.0, 1.0]`. Returns 0.0 when nothing was requested.
    pub fn hit_rate(&self) -> f64 {
        if self.requested == 0 {
            0.0
        } else {
            self.hits as f64 / self.requested as f64
        }
    }
}

/// G42/S3 core: bounded parallel batch embedding.
///
/// - texts are grouped into batches of `batch_size` (one LLM call per
///   batch, G42/S2);
/// - at most `effective_permits(parallelism)` LLM subprocesses run
///   simultaneously (`Arc<Semaphore>` + `acquire_owned`, BLOCO 2);
/// - results stream through a BOUNDED mpsc channel so the caller-side
///   collector applies backpressure and can persist incrementally
///   (BLOCO 5);
/// - the global `CancellationToken` aborts in-flight work on the first
///   signal; subprocesses die with their futures via `kill_on_drop`
///   (BLOCO 6).
pub fn embed_texts_parallel(
    embedder: &Mutex<LlmEmbedding>,
    texts: &[String],
    parallelism: usize,
    batch_size: usize,
) -> Result<Vec<Vec<f32>>, AppError> {
    let mut slots: Vec<Option<Vec<f32>>> = vec![None; texts.len()];
    embed_texts_parallel_with(embedder, texts, parallelism, batch_size, |idx, v| {
        slots[idx] = Some(v.to_vec());
        Ok(())
    })?;
    let mut out = Vec::with_capacity(slots.len());
    for (idx, slot) in slots.into_iter().enumerate() {
        out.push(slot.ok_or_else(|| {
            AppError::Embedding(crate::i18n::validation::embedding_fanout_lost_index(idx))
        })?);
    }
    Ok(out)
}

/// Like [`embed_texts_parallel`] but invokes `on_result` as soon as each
/// embedding arrives (BLOCO 5: incremental persistence — a kill loses at
/// most the in-flight batches, never the already-delivered items).
pub fn embed_texts_parallel_with(
    embedder: &Mutex<LlmEmbedding>,
    texts: &[String],
    parallelism: usize,
    batch_size: usize,
    mut on_result: impl FnMut(usize, &[f32]) -> Result<(), AppError>,
) -> Result<(), AppError> {
    if texts.is_empty() {
        return Ok(());
    }
    let dim = crate::constants::embedding_dim();
    if texts.len() == 1 {
        let v = embed_passage(embedder, &texts[0])?;
        return on_result(0, &v);
    }

    let client = clone_client(embedder);
    let permits = effective_permits(parallelism);
    let batches = build_batches(texts, batch_size.max(1));
    let token = crate::cancel_token().clone();

    let work = move |batch: Vec<(usize, String)>| {
        let client = client.clone();
        async move {
            client
                .embed_batch_async(crate::constants::PASSAGE_PREFIX, &batch)
                .await
        }
    };

    let fan_out = run_bounded(batches, permits, dim, token, work, &mut on_result);
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fan_out)),
        Err(_) => shared_runtime()?.block_on(fan_out),
    }
}

/// Groups `(global_index, text)` pairs into batches of `batch_size`.
pub(crate) fn build_batches(texts: &[String], batch_size: usize) -> Vec<Vec<(usize, String)>> {
    texts
        .iter()
        .cloned()
        .enumerate()
        .collect::<Vec<_>>()
        .chunks(batch_size)
        .map(|c| c.to_vec())
        .collect()
}

/// G42/S3 BLOCO 2: effective permit count.
///
/// `permits = clamp(requested, 1, 32) ∧ cpus ∧ ram_livre*0.5/RSS` — see
/// the module docs for the measured RSS rationale.
pub fn effective_permits(requested: usize) -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let by_ram = ((crate::memory_guard::available_memory_mb() / 2)
        / crate::constants::LLM_WORKER_RSS_MB)
        .max(1) as usize;
    requested.clamp(1, 32).min(cpus).min(by_ram).max(1)
}

/// Bounded fan-out engine. Generic over the per-batch work so the
/// concurrency contract is testable without spawning real LLMs.
///
/// Cancel safety (BLOCO 6/10): every task races its work against
/// `token.cancelled()` inside `tokio::select!`; both branches are
/// cancel-safe (the work future owns its subprocess via `kill_on_drop`,
/// and `cancelled()` is pure). On collector-side errors the `JoinSet`
/// is shut down, which drops in-flight futures and kills their
/// subprocesses.
pub(crate) async fn run_bounded<F, Fut>(
    batches: Vec<Vec<(usize, String)>>,
    permits: usize,
    dim: usize,
    token: CancellationToken,
    work: F,
    on_result: &mut impl FnMut(usize, &[f32]) -> Result<(), AppError>,
) -> Result<(), AppError>
where
    F: Fn(Vec<(usize, String)>) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = Result<Vec<(usize, Vec<f32>)>, AppError>> + Send,
{
    let total_batches = batches.len();
    let semaphore = Arc::new(Semaphore::new(permits));
    // BLOCO 5: bounded channel — producers block when the collector is
    // behind (backpressure); PROIBIDO unbounded_channel between stages.
    let (tx, mut rx) = mpsc::channel::<Result<Vec<(usize, Vec<f32>)>, AppError>>(permits * 2);
    let mut set: JoinSet<()> = JoinSet::new();

    for (batch_idx, batch) in batches.into_iter().enumerate() {
        let sem = Arc::clone(&semaphore);
        let token = token.clone();
        let tx = tx.clone();
        let work = work.clone();
        set.spawn(async move {
            let wait_start = std::time::Instant::now();
            // acquire_owned: RAII permit moved into the task; returned
            // on every exit path INCLUDING panic (BLOCO 2).
            let Ok(_permit) = sem.acquire_owned().await else {
                let _ = tx
                    .send(Err(AppError::Embedding(
                        crate::i18n::validation::embedding_semaphore_closed(),
                    )))
                    .await;
                return;
            };
            let permit_wait_ms = wait_start.elapsed().as_millis() as u64;
            let work_start = std::time::Instant::now();
            // ADR-0034: when `SQLITE_GRAPHRAG_IGNORE_SHUTDOWN=1` is set the
            // cancellation arm is dropped and the batch runs to completion.
            // This unblocks audit/test invocations whose `SHUTDOWN` flag was
            // contaminated by an earlier signal handler in the same process
            // tree. Production code never sees this branch.
            let outcome = if crate::should_obey_shutdown() {
                tokio::select! {
                    res = work(batch) => res,
                    _ = token.cancelled() => Err(AppError::Embedding(
                        crate::i18n::validation::embedding_cancelled_by_shutdown(),
                    )),
                }
            } else {
                work(batch).await
            };
            // BLOCO 8: permit wait time logged SEPARATELY from work time.
            tracing::debug!(
                target: "embedding",
                batch_idx,
                permit_wait_ms,
                work_ms = work_start.elapsed().as_millis() as u64,
                ok = outcome.is_ok(),
                "embedding batch finished"
            );
            let _ = tx.send(outcome).await;
        });
    }
    drop(tx);

    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut cancelled = 0usize;
    let mut first_error: Option<AppError> = None;

    while let Some(message) = rx.recv().await {
        match message {
            Ok(items) => {
                completed += 1;
                if first_error.is_none() {
                    for (idx, v) in items {
                        if v.len() != dim {
                            first_error = Some(AppError::Embedding(
                                crate::i18n::validation::embedding_llm_item_dims(
                                    v.len(),
                                    idx,
                                    dim,
                                ),
                            ));
                            break;
                        }
                        if let Err(e) = on_result(idx, &v) {
                            first_error = Some(e);
                            break;
                        }
                    }
                    if first_error.is_some() {
                        // Abort remaining work: dropped futures kill
                        // their subprocesses via kill_on_drop (BLOCO 6).
                        set.shutdown().await;
                    }
                }
            }
            Err(e) => {
                if matches!(&e, AppError::Embedding(msg) if msg.contains("cancelled")) {
                    cancelled += 1;
                } else {
                    failed += 1;
                }
                if first_error.is_none() {
                    first_error = Some(e);
                    set.shutdown().await;
                }
            }
        }
    }

    // Drain the JoinSet: surface panics distinctly (panic handling —
    // JoinError::is_panic tratado em todo join_next, BLOCO 9).
    while let Some(join_result) = set.join_next().await {
        if let Err(join_err) = join_result {
            if join_err.is_panic() {
                failed += 1;
                if first_error.is_none() {
                    first_error = Some(AppError::Embedding(
                        crate::i18n::validation::embedding_task_panicked(join_err),
                    ));
                }
            } else {
                cancelled += 1;
            }
        }
    }

    // v1.0.85 (ADR-0043 hygiene): the fan-out summary event moved
    // from `tracing::info!` to `tracing::debug!` and the
    // `available_permits` field was removed — the user prohibited
    // pool-state telemetry (slot_pool_stats / slot_wait_ms) and
    // decorative `tracing::info!` events. The remaining counters
    // (total_batches / completed / failed / cancelled) describe the
    // progress of the operation itself, not the slot pool, and
    // remain visible to operators running with `RUST_LOG=debug` or
    // `-vvv`.
    tracing::debug!(
        target: "embedding",
        total_batches,
        completed,
        failed,
        cancelled,
        "embedding fan-out finished"
    );

    match first_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
