//! Drain loops for enrich (Wave C1 extraction from run.rs).

use std::path::Path;
use std::time::Instant;

use super::args::{EnrichArgs, EnrichOperation};
use super::events::*;
use super::extraction::{
    call_body_enrich, call_body_extract, call_deep_research_synth, call_description_enrich,
    call_domain_classify, call_entity_connect, call_entity_description, call_entity_type_validate,
    call_graph_audit, call_memory_bindings, call_relation_reclassify, call_weight_calibrate,
    take_last_openrouter_failure, EnrichItemResult,
};
use super::prompts;
use super::queue::{
    dequeue_next_pending, heartbeat, item_type_for, mark_done, mark_skipped, open_queue_db,
    record_item_failure, record_item_failure_typed, requeue_rate_limited, requeue_wrong_op,
    skip_wrong_type, validate_claim, writeback, ClaimCheck, DequeueOutcome,
};
use super::reembed::{run_reembed_cycle, ReembedCycle, ReembedCycleCtx, ReembedTally};
use super::DEFAULT_RATE_LIMIT_WAIT;
use crate::errors::AppError;
use crate::output::emit_json_line as emit_json;
use crate::paths::AppPaths;
use crate::storage::connection::open_rw;

/// Mutable counters shared between drain loops and the orchestrator.
#[derive(Default)]
pub(crate) struct DrainCounters {
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cost_total: f64,
    pub oauth_detected: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn drain_parallel(
    args: &EnrichArgs,
    paths: &AppPaths,
    queue_path: &Path,
    namespace: &str,
    provider_binary: Option<&Path>,
    provider_model: Option<&str>,
    provider_timeout: u64,
    op_label: &str,
    backoff_clause: &str,
    parallelism: usize,
    total: usize,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    counters: &mut DrainCounters,
) -> Result<(), AppError> {
    let stdout_mu = parking_lot::Mutex::new(());
    let budget = args.max_cost_usd;
    let operation = args.operation().clone();
    let mode = args.mode().clone();
    let min_oc = args.min_output_chars;
    let max_oc = args.max_output_chars;
    let prompt_tpl = args.prompt_template.as_deref().map(|p| p.to_path_buf());

    struct WorkerResult {
        completed: usize,
        failed: usize,
        skipped: usize,
        cost: f64,
        oauth: bool,
        // GAP-SG-76 fix: distinct signal for "worker aborted because
        // SQLITE_BUSY exhausted all bounded retries" so the caller
        // fails loud (exit 15) instead of silently treating it like
        // an exhausted/empty backlog.
        db_busy: bool,
        // A `mark_done` / `mark_skipped` that never landed. Distinct from
        // `db_busy` so the operator is told the item was PROCESSED and its
        // completion was lost, not that the claim itself failed.
        writeback_lost: bool,
    }

    let results: Vec<WorkerResult> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..parallelism)
    .map(|worker_id| {
        let stdout_mu = &stdout_mu;
        let paths = &paths;
        let queue_path = &queue_path;
        let namespace = &namespace;
        let operation = &operation;
        let mode = &mode;
        let op_label = &op_label;
        let expected_item_type = item_type_for(operation);
        let prompt_tpl = prompt_tpl.as_deref();
        s.spawn(move || {
            let w_conn = match open_rw(&paths.db) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(target: "enrich", worker = worker_id, error = %e, "worker failed to open DB");
                    return WorkerResult { completed: 0, failed: 0, skipped: 0, cost: 0.0, oauth: false, db_busy: false, writeback_lost: false };
                }
            };
            let w_queue = match open_queue_db(queue_path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(target: "enrich", worker = worker_id, error = %e, "worker failed to open queue DB");
                    return WorkerResult { completed: 0, failed: 0, skipped: 0, cost: 0.0, oauth: false, db_busy: false, writeback_lost: false };
                }
            };
            let mut w_completed = 0usize;
            let mut w_failed = 0usize;
            let mut w_skipped = 0usize;
            let mut w_cost = 0.0f64;
            let mut w_oauth = false;
            let mut w_db_busy = false;
            let mut w_writeback_lost = false;
            let mut w_backoff = DEFAULT_RATE_LIMIT_WAIT;
            let w_deadline = std::time::Instant::now() + crate::runtime_config::rate_limit_deadline_secs();
            // G28-D: per-worker circuit breaker that aborts the
            // loop after `circuit_breaker_threshold` consecutive
            // HardFailure outcomes (transient/rate-limited errors
            // do NOT count, so a recovering provider is not
            // penalised).
            let mut w_breaker = crate::retry::CircuitBreaker::new(
                args.circuit_breaker_threshold.max(1),
                crate::runtime_config::enrich_circuit_breaker_reset_secs(),
            );

            loop {
                if crate::shutdown_requested() {
                    tracing::info!(target: "enrich", "shutdown requested, worker stopping");
                    break;
                }
                if let Some(b) = budget {
                    if !w_oauth && w_cost >= b {
                        break;
                    }
                }
                // GAP-SG-141 (B1): `re-embed` claims a whole batch and serves
                // it with ONE remote call. The other 13 operations are
                // chat-backed, have no batch handler, and fall through to the
                // unchanged one-row-per-call body below.
                if matches!(operation, EnrichOperation::ReEmbed) {
                    let ctx = ReembedCycleCtx {
                        main_conn: &w_conn,
                        queue_conn: &w_queue,
                        namespace,
                        op_label,
                        backoff_clause,
                        paths,
                        llm_backend,
                        embedding_backend,
                        max_attempts: args.max_attempts,
                        total,
                        stdout_mu: Some(stdout_mu),
                    };
                    let mut tally = ReembedTally { completed: w_completed, failed: w_failed, skipped: w_skipped };
                    let cycle = run_reembed_cycle(&ctx, &mut tally, Some(&mut w_breaker));
                    w_completed = tally.completed;
                    w_failed = tally.failed;
                    w_skipped = tally.skipped;
                    match cycle {
                        ReembedCycle::Progressed => continue,
                        ReembedCycle::Empty | ReembedCycle::BreakerOpen => break,
                        ReembedCycle::DbBusy => {
                            w_db_busy = true;
                            break;
                        }
                    }
                }
                // GAP-SG-16: --ignore-backoff drops the next_retry_at
                // cooldown filter so items waiting on backoff are
                // claimed immediately.
                // GAP-SG-76: distinguish a genuinely empty backlog
                // (QueryReturnedNoRows) from SQLITE_BUSY lock
                // contention with the main writer or another
                // worker — a busy claim retries briefly instead of
                // breaking the drain loop early.
                // GAP-SG-76/v1.1.00 fix: bounded busy-retry via the
                // shared with_busy_retry helper (5 attempts, exponential
                // half-jitter backoff, kill-switch aware) instead of an
                // unbounded `loop { ... continue; }` on SQLITE_BUSY. When
                // retries are exhausted, with_busy_retry converts to
                // AppError::DbBusy — that is NOT treated as an empty
                // backlog. It sets w_db_busy and stops this worker; the
                // caller (after collecting all workers) fails loud with
                // exit code 15 instead of silently under-reporting a
                // convergent drain.
                let pending = match crate::storage::utils::with_busy_retry(|| {
                    // GAP-CLI-QISO-01: claim only rows for this operation.
                    dequeue_next_pending(&w_queue, op_label, namespace, backoff_clause)
                }) {
                    Ok(DequeueOutcome::Claimed(p)) => Some(p),
                    Ok(DequeueOutcome::Empty) => None,
                    Err(AppError::DbBusy(msg)) => {
                        tracing::error!(target: "enrich", worker = worker_id, error = %msg, "SQLITE_BUSY exhausted bounded retries, worker aborting");
                        w_db_busy = true;
                        None
                    }
                    Err(e) => {
                        tracing::error!(target: "enrich", worker = worker_id, error = %e, "dequeue failed");
                        None
                    }
                };
                let claimed = match pending {
                    Some(p) => p,
                    None => break,
                };
                // GAP-CLI-QISO-03: defense-in-depth type/op validation.
                match validate_claim(&claimed, op_label, expected_item_type) {
                    ClaimCheck::Ok => {}
                    ClaimCheck::RequeueWrongOp => {
                        let _ = requeue_wrong_op(&w_queue, claimed.id);
                        continue;
                    }
                    ClaimCheck::SkipWrongType { reason } => {
                        let _ = skip_wrong_type(&w_queue, claimed.id, &reason);
                        w_skipped += 1;
                        continue;
                    }
                }
                let queue_id = claimed.id;
                let item_key = claimed.item_key;
                let attempt_current = claimed.attempt;
                // v1.1.2 (Bug 4): refresh claimed_at so a slow LLM
                // call is not mistaken for a stale claim by a
                // concurrent startup sweep. The dequeue already set
                // it; this bumps it right before the long call.
                // Best-effort by design: a missed heartbeat only risks a stale-claim
                // sweep, never a lost completion, so it is retried but not fatal.
                let _ = writeback("heartbeat", worker_id, &item_key, || {
                    heartbeat(&w_queue, queue_id)
                });
                let item_started = Instant::now();
                let current_index = w_completed + w_failed + w_skipped;

                // provider_binary validated upfront (Some for every
                // LLM-backed op; None only for ReEmbed, which ignores
                // it). unwrap_or_default yields "" solely in that
                // unread case, so a broken invariant surfaces as a
                // recoverable per-item error instead of a panic.
                let provider_bin = provider_binary.unwrap_or_else(|| std::path::Path::new(""));
                let call_result = match operation {
                    EnrichOperation::MemoryBindings | EnrichOperation::AugmentBindings => call_memory_bindings(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode),
                    EnrichOperation::EntityDescriptions => call_entity_description(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode, args.entity_description_grounding_threshold, &prompts::resolve_entity_description_domain(&args.entity_description_domain)),
                    EnrichOperation::BodyEnrich => call_body_enrich(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode, min_oc, max_oc, prompt_tpl, args.preserve_threshold, paths, llm_backend, embedding_backend),
                    // GAP-SG-141 (B1): handled by the batched cycle above; the
                    // arm stays so the match remains exhaustive.
                    EnrichOperation::ReEmbed => Ok(EnrichItemResult::Skipped { reason: "re-embed is served by the batched claim path".to_string() }),
                    EnrichOperation::WeightCalibrate => call_weight_calibrate(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode),
                    EnrichOperation::RelationReclassify => call_relation_reclassify(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode),
                    EnrichOperation::EntityConnect | EnrichOperation::CrossDomainBridges => call_entity_connect(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode),
                    EnrichOperation::EntityTypeValidate => call_entity_type_validate(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode),
                    EnrichOperation::DescriptionEnrich => call_description_enrich(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode),
                    EnrichOperation::DomainClassify => call_domain_classify(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode),
                    EnrichOperation::GraphAudit => call_graph_audit(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode),
                    EnrichOperation::DeepResearchSynth => call_deep_research_synth(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode),
                    EnrichOperation::BodyExtract => call_body_extract(&w_conn, namespace, &item_key, provider_bin, provider_model, provider_timeout, mode, args.body_extract_graph_only),
                };
                // GAP-SG-72/73: drain UNCONDITIONALLY right after
                // every call_result (success or failure) so a
                // diagnostic never survives past the item that
                // produced it — see the doc comment on
                // OpenRouterFailureDiagnostics.
                let openrouter_diag = take_last_openrouter_failure();

                match call_result {
                    Ok(EnrichItemResult::Done { cost, is_oauth, memory_id, entity_id, entities, rels, chars_before, chars_after }) => {
                        if is_oauth { w_oauth = true; }
                        w_backoff = DEFAULT_RATE_LIMIT_WAIT;
                        let elapsed_ms = item_started.elapsed().as_millis() as i64;
                        if !writeback("mark_done", worker_id, &item_key, || {
                            mark_done(
                                &w_queue,
                                queue_id,
                                memory_id,
                                entity_id,
                                entities,
                                rels,
                                cost,
                                elapsed_ms,
                            )
                        }) {
                            w_writeback_lost = true;
                        }
                        w_completed += 1;
                        if !is_oauth { w_cost += cost; }
                        // G28-D: count success; resets breaker.
                        let _ = w_breaker
                            .record(crate::retry::AttemptOutcome::Success);
                        let _guard = stdout_mu.lock();
                        emit_json(&ItemEvent { item: &item_key, status: "done", memory_id, entity_id, entities: Some(entities), rels: Some(rels), chars_before, chars_after, cost_usd: if is_oauth { None } else { Some(cost) }, elapsed_ms: Some(item_started.elapsed().as_millis() as u64), error: None, index: current_index, total });
                    }
                    Ok(EnrichItemResult::Skipped { reason }) => {
                        w_skipped += 1;
                        if !writeback("mark_skipped", worker_id, &item_key, || {
                            mark_skipped(&w_queue, queue_id, &reason)
                        }) {
                            w_writeback_lost = true;
                        }
                        let _guard = stdout_mu.lock();
                        emit_json(&ItemEvent { item: &item_key, status: "skipped", memory_id: None, entity_id: None, entities: None, rels: None, chars_before: None, chars_after: None, cost_usd: None, elapsed_ms: Some(item_started.elapsed().as_millis() as u64), error: None, index: current_index, total });
                    }
                    Ok(EnrichItemResult::PreservationFailed { score, threshold, chars_before, chars_after }) => {
                        // G29 Passo 4: worker mirror of the
                        // serial path. Counted as a soft
                        // skip so the queue surface shows
                        // a quality issue rather than a
                        // transport failure.
                        w_skipped += 1;
                        let reason = format!(
                            "preservation_failed: jaccard={score:.3} threshold={threshold:.3} (orig={chars_before} chars, new={chars_after} chars)"
                        );
                        if !writeback("mark_skipped", worker_id, &item_key, || {
                            mark_skipped(&w_queue, queue_id, &reason)
                        }) {
                            w_writeback_lost = true;
                        }
                        let _guard = stdout_mu.lock();
                        emit_json(&ItemEvent {
                            item: &item_key,
                            status: "preservation_failed",
                            memory_id: None,
                            entity_id: None,
                            entities: None,
                            rels: None,
                            chars_before: Some(chars_before),
                            chars_after: Some(chars_after),
                            cost_usd: None,
                            elapsed_ms: Some(item_started.elapsed().as_millis() as u64),
                            error: Some(reason),
                            index: current_index,
                            total,
                        });
                    }
                    Err(e) => {
                        let err_str = format!("{e}");
                        if matches!(e, AppError::RateLimited { .. }) {
                            if crate::retry::is_kill_switch_active() {
                                tracing::warn!(target: "enrich", "retry.disable is set, skipping rate-limit retry");
                            } else if std::time::Instant::now() >= w_deadline {
                                tracing::error!(target: "enrich", "rate-limit retry deadline (1h) exhausted in worker");
                            } else {
                                let half = w_backoff / 2;
                                let jitter = if half == 0 { 0 } else { fastrand::u64(0..half) };
                                let actual_wait = half + jitter;
                                tracing::warn!(target: "enrich", delay_secs = actual_wait, error_kind = "rate_limited", "rate limited in worker, backing off");
                                let _ = requeue_rate_limited(&w_queue, queue_id);
                                std::thread::sleep(std::time::Duration::from_secs(actual_wait));
                                w_backoff = (w_backoff * 2).min(900);
                                continue;
                            }
                        }
                        w_failed += 1;
                        // GAP-SG-73: prefer the origin-typed verdict
                        // (ChatError::retry_class, computed at the
                        // exact HTTP status / provider code in
                        // chat_api.rs) over the untyped fallback
                        // classifier whenever this item's failure
                        // came from an OpenRouter chat call.
                        let outcome = match openrouter_diag {
                            Some(diag) => record_item_failure_typed(
                                &w_queue,
                                queue_id,
                                attempt_current,
                                args.max_attempts,
                                diag.retry_class,
                                &err_str,
                                diag.finish_reason.as_deref(),
                                diag.prompt_tokens,
                                diag.completion_tokens,
                            ),
                            None => record_item_failure(&w_queue, queue_id, attempt_current, args.max_attempts, &e),
                        };
                        let _guard = stdout_mu.lock();
                        emit_json(&ItemEvent { item: &item_key, status: "failed", memory_id: None, entity_id: None, entities: None, rels: None, chars_before: None, chars_after: None, cost_usd: None, elapsed_ms: Some(item_started.elapsed().as_millis() as u64), error: Some(err_str), index: current_index, total });
                        // G28-D: feed the classified outcome to the breaker (transient
                        // failures do not count toward opening it).
                        let breaker_opened = w_breaker.record(outcome);
                        if breaker_opened {
                            tracing::error!(target: "enrich",
                                consecutive_failures = w_breaker.consecutive_failures(),
                                "circuit breaker opened — aborting worker"
                            );
                            break;
                        }
                    }
                }
            }
            WorkerResult { completed: w_completed, failed: w_failed, skipped: w_skipped, cost: w_cost, oauth: w_oauth, db_busy: w_db_busy, writeback_lost: w_writeback_lost }
        })
    })
    .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or(WorkerResult {
                    completed: 0,
                    failed: 0,
                    skipped: 0,
                    cost: 0.0,
                    oauth: false,
                    db_busy: false,
                    writeback_lost: false,
                })
            })
            .collect()
    });

    // GAP-SG-76 fix: a worker that aborted due to exhausted
    // SQLITE_BUSY retries must fail the whole `enrich` invocation
    // loudly (exit code 15 via AppError::DbBusy) rather than being
    // folded silently into the completed/failed/skipped counters,
    // which would understate a genuinely unfinished drain.
    if results.iter().any(|r| r.db_busy) {
        return Err(AppError::DbBusy(
            "SQLITE_BUSY exhausted bounded retries while dequeuing (parallel worker)".into(),
        ));
    }

    // A lost write-back is as unfinished as a lost claim, and worse for the
    // operator: the item was processed and paid for, the queue does not know it,
    // and the next stale-claim sweep will hand it back to be billed again.
    // Reporting a convergent drain here would be a false green.
    if results.iter().any(|r| r.writeback_lost) {
        return Err(AppError::DbBusy(
            "SQLITE_BUSY exhausted bounded retries while writing back a completed item \
             (parallel worker); the item stays claimed and will be reprocessed"
                .into(),
        ));
    }

    for r in &results {
        counters.completed += r.completed;
        counters.failed += r.failed;
        counters.skipped += r.skipped;
        counters.cost_total += r.cost;
        if r.oauth && !counters.oauth_detected {
            counters.oauth_detected = true;
        }
    }
    Ok(())
}
