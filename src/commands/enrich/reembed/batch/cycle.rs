//! The drain cycle that feeds batches: claim, dispatch, report.
//!
//! Wraps [`super::embed::call_reembed_batch`] in the queue lifecycle the serial
//! loop and a parallel worker both use — claiming a batch, heartbeating it,
//! recording each outcome and emitting one NDJSON item event per key.

use super::embed::call_reembed_batch;
use crate::commands::enrich::events::ItemEvent;
use crate::commands::enrich::extraction::EnrichItemResult;
use crate::commands::enrich::queue::{
    dequeue_batch_pending, heartbeat, mark_done, mark_skipped, record_item_failure, writeback,
};
use crate::errors::AppError;
use crate::output::emit_json_line as emit_json;
use rusqlite::Connection;
use std::time::Instant;

/// Everything one batched re-embed cycle needs, gathered so the serial loop and
/// a parallel worker can share the same body.
pub(in crate::commands::enrich) struct ReembedCycleCtx<'a> {
    /// Main graph database (reads targets, writes vectors).
    pub main_conn: &'a Connection,
    /// Sidecar queue database.
    pub queue_conn: &'a Connection,
    /// Namespace both the claim and the target lookups are scoped to.
    pub namespace: &'a str,
    /// Debug label of the current operation, as stored in `queue.operation`.
    pub op_label: &'a str,
    /// Backoff fragment interpolated into the claim, possibly empty.
    pub backoff_clause: &'a str,
    /// Model and cache directories.
    pub paths: &'a crate::paths::AppPaths,
    /// Resolved LLM backend choice.
    pub llm_backend: crate::cli::LlmBackendChoice,
    /// Resolved embedding backend choice.
    pub embedding_backend: crate::cli::EmbeddingBackendChoice,
    /// `--max-attempts` floor used when a failure is recorded.
    pub max_attempts: u32,
    /// Total item count reported in each NDJSON event.
    pub total: usize,
    /// Stdout serialisation lock; `None` for the single-threaded serial loop.
    pub stdout_mu: Option<&'a parking_lot::Mutex<()>>,
}

/// Running per-drain counters a cycle advances.
#[derive(Default)]
pub(in crate::commands::enrich) struct ReembedTally {
    /// Items that ended `done`.
    pub completed: usize,
    /// Items that ended `failed` (or dead-lettered).
    pub failed: usize,
    /// Items that ended `skipped`.
    pub skipped: usize,
}

/// What the caller should do after one cycle.
pub(in crate::commands::enrich) enum ReembedCycle {
    /// Backlog for this operation + namespace is empty; stop draining.
    Empty,
    /// A claim was processed; keep draining.
    Progressed,
    /// `SQLITE_BUSY` survived the bounded retry budget; fail loud.
    DbBusy,
    /// The circuit breaker opened; abort this drain.
    BreakerOpen,
}

/// Claims up to `enrich.reembed_claim_batch` rows and re-embeds them with ONE
/// shared request.
///
/// Emits exactly one NDJSON event per claimed item, preserving the cardinality
/// the one-row-per-call path produced. `elapsed_ms` on each event is the whole
/// batch's elapsed time and `cost_usd` is the batch cost split evenly across
/// the successful items — a shared call has no per-item timing or price, so
/// both fields mean something different here and are documented as such.
pub(in crate::commands::enrich) fn run_reembed_cycle(
    ctx: &ReembedCycleCtx<'_>,
    tally: &mut ReembedTally,
    mut breaker: Option<&mut crate::retry::CircuitBreaker>,
) -> ReembedCycle {
    let limit = crate::runtime_config::reembed_claim_batch();
    let claimed = match crate::storage::utils::with_busy_retry(|| {
        dequeue_batch_pending(
            ctx.queue_conn,
            ctx.op_label,
            ctx.namespace,
            ctx.backoff_clause,
            limit,
        )
    }) {
        Ok(rows) => rows,
        Err(AppError::DbBusy(msg)) => {
            tracing::error!(target: "enrich", error = %msg, "SQLITE_BUSY exhausted bounded retries, aborting re-embed batch claim");
            return ReembedCycle::DbBusy;
        }
        Err(e) => {
            tracing::error!(target: "enrich", error = %e, "re-embed batch claim failed");
            return ReembedCycle::Empty;
        }
    };
    if claimed.is_empty() {
        return ReembedCycle::Empty;
    }

    // Refresh every claim before the shared call so a startup sweep cannot
    // reclaim rows mid-batch.
    // Best-effort by design: a missed heartbeat only risks a stale-claim sweep,
    // never a lost completion, so it is retried but not fatal.
    for row in &claimed {
        let _ = writeback("heartbeat", 0, &row.item_key, || {
            heartbeat(ctx.queue_conn, row.id)
        });
    }

    let keys: Vec<String> = claimed.iter().map(|r| r.item_key.clone()).collect();
    let started = Instant::now();
    let batch = call_reembed_batch(
        ctx.main_conn,
        ctx.namespace,
        &keys,
        ctx.paths,
        ctx.llm_backend,
        ctx.embedding_backend,
    );
    let elapsed_ms = started.elapsed().as_millis() as i64;

    let outcomes = match batch {
        Ok(v) => v,
        Err(e) => {
            // ONE remote call failed, so ONE outcome reaches the breaker even
            // though N rows are affected. Each row is recorded individually so
            // the shared `--max-attempts` floor and backoff still apply; the
            // attempt consumed by the claim is NOT refunded, because every item
            // really was tried at the same per-item rate the one-row-per-call
            // path charges.
            let err_str = format!("{e}");
            let mut outcome = crate::retry::AttemptOutcome::HardFailure;
            for row in &claimed {
                outcome =
                    record_item_failure(ctx.queue_conn, row.id, row.attempt, ctx.max_attempts, &e);
                let index = tally.completed + tally.failed + tally.skipped;
                tally.failed += 1;
                emit_item_event(
                    ctx,
                    &row.item_key,
                    "failed",
                    None,
                    Some(err_str.clone()),
                    elapsed_ms,
                    index,
                );
            }
            if let Some(b) = breaker.as_deref_mut() {
                if b.record(outcome) {
                    tracing::error!(target: "enrich",
                        consecutive_failures = b.consecutive_failures(),
                        "circuit breaker opened — aborting worker"
                    );
                    return ReembedCycle::BreakerOpen;
                }
            }
            return ReembedCycle::Progressed;
        }
    };

    // Re-embed reports no cost today, so this split is exact; it stays correct
    // if a priced embedding backend is added later.
    let done_count = outcomes
        .iter()
        .filter(|o| matches!(o.result, EnrichItemResult::Done { .. }))
        .count();
    let batch_cost: f64 = outcomes
        .iter()
        .map(|o| match o.result {
            EnrichItemResult::Done { cost, .. } => cost,
            _ => 0.0,
        })
        .sum();
    let cost_per_item = if done_count == 0 {
        0.0
    } else {
        batch_cost / done_count as f64
    };

    // A write-back that never lands leaves the row claimed, the stale-claim
    // sweep hands it back, and the embedding is recomputed and re-billed.
    // Reporting Progressed on that would be a false green, so the cycle ends
    // in DbBusy — the same loud path the claim already uses.
    let mut writeback_lost = false;
    for (row, outcome) in claimed.iter().zip(outcomes) {
        let index = tally.completed + tally.failed + tally.skipped;
        match outcome.result {
            EnrichItemResult::Done {
                memory_id,
                entity_id,
                entities,
                rels,
                ..
            } => {
                if !writeback("mark_done", 0, &outcome.item_key, || {
                    mark_done(
                        ctx.queue_conn,
                        row.id,
                        memory_id,
                        entity_id,
                        entities,
                        rels,
                        cost_per_item,
                        elapsed_ms,
                    )
                }) {
                    writeback_lost = true;
                }
                tally.completed += 1;
                emit_item_event(
                    ctx,
                    &outcome.item_key,
                    "done",
                    Some((memory_id, entity_id, entities, rels)),
                    None,
                    elapsed_ms,
                    index,
                );
            }
            EnrichItemResult::Skipped { reason } => {
                if !writeback("mark_skipped", 0, &outcome.item_key, || {
                    mark_skipped(ctx.queue_conn, row.id, &reason)
                }) {
                    writeback_lost = true;
                }
                tally.skipped += 1;
                emit_item_event(
                    ctx,
                    &outcome.item_key,
                    "skipped",
                    None,
                    None,
                    elapsed_ms,
                    index,
                );
            }
            // `re-embed` never runs the body-preservation gate; treat the
            // variant defensively as a soft skip rather than panicking.
            EnrichItemResult::PreservationFailed { .. } => {
                if !writeback("mark_skipped", 0, &outcome.item_key, || {
                    mark_skipped(ctx.queue_conn, row.id, "preservation_failed")
                }) {
                    writeback_lost = true;
                }
                tally.skipped += 1;
                emit_item_event(
                    ctx,
                    &outcome.item_key,
                    "skipped",
                    None,
                    None,
                    elapsed_ms,
                    index,
                );
            }
        }
    }

    if let Some(b) = breaker {
        let _ = b.record(crate::retry::AttemptOutcome::Success);
    }
    if writeback_lost {
        return ReembedCycle::DbBusy;
    }
    ReembedCycle::Progressed
}

/// Emits one NDJSON item event, taking the stdout lock when the caller is a
/// parallel worker.
fn emit_item_event(
    ctx: &ReembedCycleCtx<'_>,
    item: &str,
    status: &str,
    done: Option<(Option<i64>, Option<i64>, usize, usize)>,
    error: Option<String>,
    elapsed_ms: i64,
    index: usize,
) {
    let (memory_id, entity_id, entities, rels) = match done {
        Some((m, e, ent, rel)) => (m, e, Some(ent), Some(rel)),
        None => (None, None, None, None),
    };
    let event = ItemEvent {
        item,
        status,
        memory_id,
        entity_id,
        entities,
        rels,
        chars_before: None,
        chars_after: None,
        cost_usd: None,
        elapsed_ms: Some(elapsed_ms.max(0) as u64),
        error,
        index,
        total: ctx.total,
    };
    match ctx.stdout_mu {
        Some(mu) => {
            let _guard = mu.lock();
            emit_json(&event);
        }
        None => emit_json(&event),
    }
}
