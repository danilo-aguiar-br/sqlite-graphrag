//! Batch-size and permit policy for the embedding fan-out.
//!
//! Owns every knob that decides HOW MUCH work goes into one unit: the
//! dim-adaptive calibration bases (G44), the grouping of texts into batches,
//! and the effective concurrency permit count (G42/S3, BLOCO 2).

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

/// G42/S3 BLOCO 2: effective permit count.
///
/// `permits = clamp(requested, 1, 32) ∧ cpus ∧ ram_livre*0.5/RSS ∧ joint`
///
/// The last term is the joint cap: `--max-concurrency` and `--llm-parallelism`
/// are each validated alone, so before
/// [`crate::constants::joint_parallelism_ceiling`] existed their PRODUCT could
/// authorise `2 × nCPUs × 32` workers on one host. The RSS term uses
/// [`crate::constants::llm_worker_rss_mb`], whose default is an ESTIMATE and not
/// a measurement — see that constant's docs.
pub fn effective_permits(requested: usize) -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let by_ram = ((crate::memory_guard::available_memory_mb() / 2)
        / crate::constants::llm_worker_rss_mb().max(1))
    .max(1) as usize;
    requested
        .clamp(1, 32)
        .min(cpus)
        .min(by_ram)
        .min(crate::constants::joint_parallelism_ceiling())
        .max(1)
}
