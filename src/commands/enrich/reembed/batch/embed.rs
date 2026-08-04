//! Phase 2 — EMBED: one embedding request per claim, then one write pass.
//!
//! Holds the batch entry point [`call_reembed_batch`], which resolves every
//! claimed key, issues a SINGLE embedding call with the surviving texts and
//! upserts the results in one transaction (GAP-SG-141 B1).

use super::resolve::{resolve_key, Resolved};
use super::target::{done_result, PendingEmbed};
use super::write::write_vector;
use crate::commands::enrich::extraction::EnrichItemResult;
use crate::commands::enrich::postprocess::record_enrich_backend;
use crate::errors::AppError;
use rusqlite::Connection;
use std::time::Instant;

/// Per-item outcome of a batch, in the caller's key order.
///
/// Every claimed key produces exactly one of these, so the NDJSON stream keeps
/// its current one-event-per-item cardinality.
pub(in crate::commands::enrich) struct BatchItemOutcome {
    /// The queue `item_key` this outcome belongs to.
    pub item_key: String,
    /// Terminal result for the item, mirroring the single-item handlers.
    pub result: EnrichItemResult,
}

/// Re-embeds a whole claim in three phases (resolve, embed, write).
///
/// # Failure policy
///
/// A failure of the shared embedding call fails the WHOLE batch with a single
/// `Err`. The caller returns every claimed row to the queue and records exactly
/// ONE outcome against the circuit breaker, because exactly one remote call was
/// made. The `attempt` already consumed by the claim is deliberately NOT
/// refunded: each item really was tried, at the same per-item rate the
/// one-row-per-call path charges, and refunding would let a permanently failing
/// item outlive `--max-attempts` forever.
///
/// A dimension mismatch inside the response fails the entire HTTP chunk (see
/// `truncate_embedding` in `embedding_api.rs`) and therefore lands in this same
/// whole-batch policy — a known residual risk of sharing one call.
///
/// Returns one [`BatchItemOutcome`] per input key, in input order.
pub(in crate::commands::enrich) fn call_reembed_batch(
    conn: &Connection,
    namespace: &str,
    item_keys: &[String],
    paths: &crate::paths::AppPaths,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<Vec<BatchItemOutcome>, AppError> {
    let started = Instant::now();
    let dim = crate::constants::embedding_dim();

    // Phase 1: resolve. `None` in `outcomes` means "still needs an embedding".
    let mut outcomes: Vec<Option<EnrichItemResult>> = Vec::with_capacity(item_keys.len());
    let mut pending: Vec<PendingEmbed> = Vec::with_capacity(item_keys.len());
    for (slot, key) in item_keys.iter().enumerate() {
        match resolve_key(conn, namespace, key, dim) {
            Resolved::Settled(result) => outcomes.push(Some(result)),
            Resolved::NeedsEmbedding { target, text } => {
                outcomes.push(None);
                pending.push(PendingEmbed { slot, target, text });
            }
        }
    }

    // Phase 2: embed. One call for every survivor.
    if !pending.is_empty() {
        let texts: Vec<String> = pending.iter().map(|p| p.text.clone()).collect();
        // GAP-SG-147: hand the batch over by shared ownership instead of by
        // reference. `Arc::from(Vec<String>)` MOVES the elements — only the
        // 24-byte `String` headers are relocated, the heap buffers stay put.
        // `Arc::from(&[String])` would clone every body, which is the trap this
        // variant exists to close: a claim of 32 memory bodies would copy every
        // byte of them for nothing.
        let vectors = crate::embedder::embed_passages_parallel_shared(
            &paths.models,
            std::sync::Arc::from(texts),
            crate::constants::DEFAULT_REEMBED_CLAIM_BATCH,
            crate::constants::DEFAULT_REEMBED_CLAIM_BATCH,
            embedding_backend,
            llm_backend,
        )?;
        if vectors.len() != pending.len() {
            return Err(AppError::Embedding(
                crate::i18n::errors_ops::batch_embedding_count_mismatch(
                    vectors.len(),
                    pending.len(),
                ),
            ));
        }
        record_enrich_backend(effective_backend_label(embedding_backend, llm_backend));

        // Phase 3: write. One transaction for the whole claim, so a mid-batch
        // failure cannot leave half the vectors persisted.
        let tx = conn.unchecked_transaction()?;
        for (item, embedding) in pending.iter().zip(vectors.iter()) {
            if embedding.is_empty() {
                outcomes[item.slot] = Some(EnrichItemResult::Skipped {
                    reason: "embedding backend returned an empty vector (chain resolved to none)"
                        .to_string(),
                });
                continue;
            }
            write_vector(&tx, namespace, &item.target, embedding)?;
            outcomes[item.slot] = Some(done_result(&item.target, item.text.chars().count()));
        }
        tx.commit()?;
    }

    // `elapsed_ms` is the WHOLE batch attributed to each item, and `cost_usd`
    // is the batch cost split evenly across the items: a shared call has no
    // per-item timing or price to report. Both fields therefore mean something
    // different here than on the one-row-per-call path. Re-embed currently
    // reports cost 0.0, so the split is exact today and stays correct if a
    // priced embedding backend is added later.
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let _ = elapsed_ms;

    Ok(item_keys
        .iter()
        .zip(outcomes)
        .map(|(key, result)| BatchItemOutcome {
            item_key: key.clone(),
            result: result.unwrap_or_else(|| EnrichItemResult::Skipped {
                reason: "re-embed batch produced no outcome for this key".to_string(),
            }),
        })
        .collect())
}

/// Label of the backend the shared batch call actually used.
///
/// The batch embedding entry point returns vectors only, not the resolved
/// backend, so this mirrors its own branch condition: the OpenRouter HTTP path
/// when the chain starts there and the client is live, otherwise whatever the
/// chain head is. Keeps `backend_invoked` populated for batched drains, which
/// would otherwise report nothing.
fn effective_backend_label(
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    llm_backend: crate::cli::LlmBackendChoice,
) -> &'static str {
    let chain = embedding_backend.to_chain(llm_backend);
    match chain.first() {
        Some(&crate::embedder::LlmBackendKind::OpenRouter)
            if crate::embedder::is_openrouter_initialized() =>
        {
            crate::embedder::LlmBackendKind::OpenRouter.as_str()
        }
        Some(kind) => kind.as_str(),
        None => crate::embedder::LlmBackendKind::None.as_str(),
    }
}
