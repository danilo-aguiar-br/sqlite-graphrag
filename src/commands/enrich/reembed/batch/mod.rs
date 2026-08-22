//! Batched re-embed: one embedding request per claim (GAP-SG-141 B1).
//!
//! # Why
//!
//! The historical one-row-per-call handlers issued one REST request
//! per queue row. On a backfill of 36343 eligible items that is 36343 requests
//! where 1137 would carry the same payload — a measured ~32x overhead that is
//! pure latency, because
//! [`crate::embedder::embed_passages_parallel_shared`] already
//! accepts N texts and, for N of 32 or fewer on the OpenRouter path, issues
//! exactly ONE serial call.
//!
//! # Three phases
//!
//! 1. **Resolve** ([`resolve`]) — no network. Each claimed key becomes a
//!    [`target::ReembedTarget`] carrying its row id and the exact text the write
//!    path would embed. A key that no longer resolves, or whose text is empty, is
//!    recorded as skipped and does NOT abort the batch. A target that already
//!    holds a live vector at the active dim is recorded as done and consumes no
//!    request.
//! 2. **Embed** ([`embed`]) — ONE call with the surviving texts.
//! 3. **Write** ([`write`]) — one transaction upserting every vector.
//!
//! [`cycle`] wraps the three in the queue lifecycle the drain loops share.

mod cycle;
mod embed;
mod resolve;
mod target;
mod write;

pub(in crate::commands::enrich) use cycle::{
    run_reembed_cycle, ReembedCycle, ReembedCycleCtx, ReembedTally,
};
// `call_reembed_batch` and `BatchItemOutcome` are consumed only by `cycle`,
// which reaches them through `super::embed::` — no re-export needed.
