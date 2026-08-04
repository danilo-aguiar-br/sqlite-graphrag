//! Mechanics of the bounded parallel fan-out.
//!
//! Owns the index arithmetic that partitions a corpus into chunks, the
//! order-restoring reassembly of out-of-order results, and the generic
//! semaphore-bounded execution engine shared by every batch entry point
//! (GAP-OPENROUTER-REST-CONCURRENCY / GAP-SG-147 / G42/S3).

/// DEFAULT number of texts sent per OpenRouter REST call inside the bounded
/// fan-out, when XDG `embedding.batch_size` supplies nothing.
///
/// Read it through [`fan_out_chunk`], never directly: the operator-facing knob
/// is the XDG key, and this constant is only its fallback.
///
/// Also the unit [`reassemble_ordered`] indexes on: chunk `i` covers
/// `[i * chunk, min((i + 1) * chunk, len))`.
pub(crate) const EMBED_FAN_OUT_CHUNK: usize = 32;

/// Effective texts-per-REST-call for the OpenRouter fan-out (GAP-SG-142).
///
/// Deliberately the SAME knob that [`crate::embedding_api::OpenRouterClient`]
/// uses to re-chunk a batch internally. The outer fan-out exists only to feed
/// that inner call, so two independent knobs could be set to values that cancel
/// each other: raising `embedding.batch_size` to 64 while the fan-out still
/// handed over 32 left the inner chunking with nothing to split, and the
/// operator kept 32-text requests with no signal that the setting was inert.
pub(crate) fn fan_out_chunk() -> usize {
    crate::runtime_config::embedding_batch_size(EMBED_FAN_OUT_CHUNK)
}

/// Index ranges that partition `len` items into `chunk` sized pieces.
///
/// GAP-SG-147: mirrors `slice::chunks` exactly, but yields ranges instead of
/// borrowed slices so a spawned `'static` task can hold the range and index
/// into a shared [`Arc`] rather than owning a copy of the data.
pub(crate) fn chunk_ranges(
    len: usize,
    chunk: usize,
) -> impl Iterator<Item = std::ops::Range<usize>> {
    let chunk = chunk.max(1);
    (0..len)
        .step_by(chunk)
        .map(move |start| start..(start + chunk).min(len))
}

/// GAP-OPENROUTER-REST-CONCURRENCY: reassembles the flat vector list in
/// input order from chunk parts produced out-of-order by the bounded
/// `JoinSet` fan-out. Sorts by chunk index, then flattens, so the result
/// matches the original `texts` order exactly.
pub(crate) fn reassemble_ordered(mut parts: Vec<(usize, Vec<Vec<f32>>)>) -> Vec<Vec<f32>> {
    parts.sort_by_key(|(idx, _)| *idx);
    parts.into_iter().flat_map(|(_, v)| v).collect()
}
