//! Drain loops for enrich (Wave C1 extraction from run.rs).

use std::path::Path;
use std::time::Instant;

use rusqlite::Connection;

use super::args::{EnrichArgs, EnrichOperation};
use super::events::*;
use super::extraction::{
    call_body_enrich, call_body_extract, call_deep_research_synth, call_description_enrich,
    call_domain_classify, call_entity_connect, call_entity_description, call_entity_type_validate,
    call_graph_audit, call_memory_bindings, call_reembed, call_relation_reclassify,
    call_weight_calibrate, take_last_openrouter_failure, EnrichItemResult,
};
use super::queue::{
    dequeue_next_pending, heartbeat, item_type_for, record_item_failure, record_item_failure_typed,
    requeue_wrong_op, skip_wrong_type, validate_claim, ClaimCheck, DequeueOutcome,
};
use super::prompts;
use super::scheduler;
use super::DEFAULT_RATE_LIMIT_WAIT;
use crate::errors::AppError;
use crate::output::emit_json_line as emit_json;
use crate::paths::AppPaths;

use super::drain_parallel::DrainCounters;

#[allow(clippy::too_many_arguments)]
pub(crate) fn drain_serial(
    args: &EnrichArgs,
    paths: &AppPaths,
    conn: &Connection,
    queue_conn: &Connection,
    namespace: &str,
    provider_binary: Option<&Path>,
    provider_model: Option<&str>,
    provider_timeout: u64,
    op_label: &str,
    backoff_clause: &str,
    item_type: &str,
    total: usize,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    yield_every: usize,
    counters: &mut DrainCounters,
    items_since_yield: &mut usize,
    yield_count: &mut u64,
    preempted_for_gate: &mut bool,
    enrich_started: Instant,
    until_deadline: Instant,
    rate_limit_deadline: Instant,
    mut backoff_secs: u64,
) -> Result<(), AppError> {
    let mut completed = counters.completed;
    let mut failed = counters.failed;
    let mut skipped = counters.skipped;
    let mut cost_total = counters.cost_total;
    let mut oauth_detected = counters.oauth_detected;
loop {
    if crate::shutdown_requested() {
        tracing::info!(target: "enrich", "shutdown requested, stopping enrichment");
        break;
    }

    // Budget check
    if let Some(budget) = args.max_cost_usd {
        if !oauth_detected && cost_total >= budget {
            tracing::warn!(target: "enrich", spent = cost_total, budget, "budget exceeded, stopping");
            break;
        }
    }

    // Dequeue next pending item (GAP-SG-16: --ignore-backoff drops
    // the next_retry_at cooldown filter).
    // GAP-SG-76: distinguish a genuinely empty backlog
    // (QueryReturnedNoRows) from SQLITE_BUSY lock contention with
    // a concurrent writer — a busy claim retries briefly instead
    // of breaking the drain loop early.
    // GAP-SG-76/v1.1.00 fix: bounded busy-retry via the shared
    // with_busy_retry helper (5 attempts, exponential half-jitter
    // backoff, kill-switch aware) instead of an unbounded
    // `loop { ... continue; }` on SQLITE_BUSY. When retries are
    // exhausted, with_busy_retry converts to AppError::DbBusy,
    // which we propagate immediately (fail loud, exit code 15)
    // instead of silently treating sustained contention as an
    // empty backlog — that would end the drain early and under-
    // report queue_pending as converged.
    let pending = match crate::storage::utils::with_busy_retry(|| {
        // GAP-CLI-QISO-01: claim only rows for this operation.
        dequeue_next_pending(queue_conn, op_label, backoff_clause)
    }) {
        Ok(DequeueOutcome::Claimed(p)) => Some(p),
        Ok(DequeueOutcome::Empty) => None,
        Err(e @ AppError::DbBusy(_)) => {
            tracing::error!(target: "enrich", error = %e, "SQLITE_BUSY exhausted bounded retries, aborting drain loop");
            return Err(e);
        }
        Err(e) => {
            tracing::error!(target: "enrich", error = %e, "dequeue failed");
            None
        }
    };

    let claimed = match pending {
        Some(p) => p,
        None => break,
    };
    // GAP-CLI-QISO-03: defense-in-depth type/op validation.
    let expected_item_type = item_type_for(&args.operation());
    match validate_claim(&claimed, op_label, expected_item_type) {
        ClaimCheck::Ok => {}
        ClaimCheck::RequeueWrongOp => {
            let _ = requeue_wrong_op(queue_conn, claimed.id);
            continue;
        }
        ClaimCheck::SkipWrongType { reason } => {
            let _ = skip_wrong_type(queue_conn, claimed.id, &reason);
            skipped += 1;
            continue;
        }
    }
    let queue_id = claimed.id;
    let item_key = claimed.item_key;
    let item_type = claimed.item_type;
    let attempt_current = claimed.attempt;
    let _ = item_type; // used by some handlers via key shape; keep for diagnostics

    // v1.1.2 (Bug 4): refresh claimed_at before the long LLM call so
    // a startup sweep does not reclaim this row mid-processing.
    let _ = heartbeat(queue_conn, queue_id);

    let item_started = Instant::now();
    let current_index = completed + failed + skipped;

    // See worker note: provider_binary is Some for every LLM-backed
    // op; "" here only for ReEmbed, which never reads it.
    let provider_bin = provider_binary
        .unwrap_or_else(|| std::path::Path::new(""));
    let call_result = match args.operation() {
        EnrichOperation::MemoryBindings | EnrichOperation::AugmentBindings => {
            call_memory_bindings(
                conn,
                namespace,
                &item_key,
                provider_bin,
                provider_model,
                provider_timeout,
                &args.mode(),
            )
        }
        EnrichOperation::EntityDescriptions => call_entity_description(
            conn,
            namespace,
            &item_key,
            provider_bin,
            provider_model,
            provider_timeout,
            &args.mode(),
            args.entity_description_grounding_threshold,
            &prompts::resolve_entity_description_domain(&args.entity_description_domain),
        ),
        EnrichOperation::BodyEnrich => call_body_enrich(
            conn,
            namespace,
            &item_key,
            provider_bin,
            provider_model,
            provider_timeout,
            &args.mode(),
            args.min_output_chars,
            args.max_output_chars,
            args.prompt_template.as_deref(),
            args.preserve_threshold,
            paths,
            llm_backend,
            embedding_backend,
        ),
        EnrichOperation::ReEmbed => call_reembed(
            conn,
            namespace,
            &item_key,
            paths,
            llm_backend,
            embedding_backend,
        ),
        EnrichOperation::WeightCalibrate => call_weight_calibrate(
            conn,
            namespace,
            &item_key,
            provider_bin,
            provider_model,
            provider_timeout,
            &args.mode(),
        ),
        EnrichOperation::RelationReclassify => call_relation_reclassify(
            conn,
            namespace,
            &item_key,
            provider_bin,
            provider_model,
            provider_timeout,
            &args.mode(),
        ),
        EnrichOperation::EntityConnect | EnrichOperation::CrossDomainBridges => {
            call_entity_connect(
                conn,
                namespace,
                &item_key,
                provider_bin,
                provider_model,
                provider_timeout,
                &args.mode(),
            )
        }
        EnrichOperation::EntityTypeValidate => call_entity_type_validate(
            conn,
            namespace,
            &item_key,
            provider_bin,
            provider_model,
            provider_timeout,
            &args.mode(),
        ),
        EnrichOperation::DescriptionEnrich => call_description_enrich(
            conn,
            namespace,
            &item_key,
            provider_bin,
            provider_model,
            provider_timeout,
            &args.mode(),
        ),
        EnrichOperation::DomainClassify => call_domain_classify(
            conn,
            namespace,
            &item_key,
            provider_bin,
            provider_model,
            provider_timeout,
            &args.mode(),
        ),
        EnrichOperation::GraphAudit => call_graph_audit(
            conn,
            namespace,
            &item_key,
            provider_bin,
            provider_model,
            provider_timeout,
            &args.mode(),
        ),
        EnrichOperation::DeepResearchSynth => call_deep_research_synth(
            conn,
            namespace,
            &item_key,
            provider_bin,
            provider_model,
            provider_timeout,
            &args.mode(),
        ),
        EnrichOperation::BodyExtract => call_body_extract(
            conn,
            namespace,
            &item_key,
            provider_bin,
            provider_model,
            provider_timeout,
            &args.mode(),
            args.body_extract_graph_only,
        ),
    };
    // GAP-SG-72/73: drain UNCONDITIONALLY right after every
    // call_result (mirrors the worker loop above).
    let openrouter_diag = take_last_openrouter_failure();

    match call_result {
        Ok(EnrichItemResult::Done {
            memory_id,
            entity_id,
            entities,
            rels,
            chars_before,
            chars_after,
            cost,
            is_oauth,
        }) => {
            if is_oauth && !oauth_detected {
                oauth_detected = true;
                tracing::info!(target: "enrich", "OAuth subscription detected — cost_usd omitted from output");
            }
            backoff_secs = DEFAULT_RATE_LIMIT_WAIT;

            // Persist depends on the operation
            let persist_err: Option<String> = match args.operation() {
                EnrichOperation::MemoryBindings => {
                    // Bindings already persisted inside call_memory_bindings
                    None
                }
                EnrichOperation::EntityDescriptions => {
                    // Description already persisted inside call_entity_description
                    None
                }
                EnrichOperation::BodyEnrich => {
                    // Body already persisted inside call_body_enrich
                    None
                }
                _ => {
                    // All G27 operations persist inside their call_* function
                    None
                }
            };

            if let Err(e) = queue_conn.execute(
        "UPDATE queue SET status='done', memory_id=?1, entity_id=?2, entities=?3, rels=?4, cost_usd=?5, elapsed_ms=?6, done_at=datetime('now') WHERE id=?7",
        rusqlite::params![
            memory_id,
            entity_id,
            entities as i64,
            rels as i64,
            cost,
            item_started.elapsed().as_millis() as i64,
            queue_id
        ],
    ) {
            tracing::warn!(target: "enrich", error = %e, "queue done update failed");
        }

            if persist_err.is_none() {
                completed += 1;
                *items_since_yield = items_since_yield.saturating_add(1);
                if yield_every > 0 && *items_since_yield >= yield_every {
                    scheduler::cooperative_yield();
                    *yield_count = yield_count.saturating_add(1);
                    *items_since_yield = 0;
                    // EC-09: if HOT ED is pending while we are on entity-connect, stop batch.
                    if scheduler::should_preempt_for_hot_ed(
                        super::queue::count_priority_pending(queue_conn, "EntityDescriptions", super::queue::PRIORITY_HOT).unwrap_or(0) > 0,
                        matches!(args.operation(), EnrichOperation::EntityConnect | EnrichOperation::CrossDomainBridges),
                    ) {
                        tracing::info!(target: "enrich", "preempting EC for hot entity-descriptions");
                        *preempted_for_gate = true;
                        break;
                    }
                }
                if !is_oauth {
                    cost_total += cost;
                }
                emit_json(&ItemEvent {
                    item: &item_key,
                    status: "done",
                    memory_id,
                    entity_id,
                    entities: Some(entities),
                    rels: Some(rels),
                    chars_before,
                    chars_after,
                    cost_usd: if is_oauth { None } else { Some(cost) },
                    elapsed_ms: Some(item_started.elapsed().as_millis() as u64),
                    error: None,
                    index: current_index,
                    total,
                });
            } else {
                failed += 1;
                emit_json(&ItemEvent {
                    item: &item_key,
                    status: "failed",
                    memory_id: None,
                    entity_id: None,
                    entities: None,
                    rels: None,
                    chars_before: None,
                    chars_after: None,
                    cost_usd: None,
                    elapsed_ms: Some(item_started.elapsed().as_millis() as u64),
                    error: persist_err,
                    index: current_index,
                    total,
                });
            }
        }
        Ok(EnrichItemResult::Skipped { reason }) => {
            skipped += 1;
            if let Err(e) = queue_conn.execute(
        "UPDATE queue SET status='skipped', error=?1, done_at=datetime('now') WHERE id=?2",
        rusqlite::params![reason, queue_id],
    ) {
            tracing::warn!(target: "enrich", error = %e, "queue skipped update failed");
        }
            emit_json(&ItemEvent {
                item: &item_key,
                status: "skipped",
                memory_id: None,
                entity_id: None,
                entities: None,
                rels: None,
                chars_before: None,
                chars_after: None,
                cost_usd: None,
                elapsed_ms: Some(item_started.elapsed().as_millis() as u64),
                error: None,
                index: current_index,
                total,
            });
        }
        Ok(EnrichItemResult::PreservationFailed {
            score,
            threshold,
            chars_before,
            chars_after,
        }) => {
            // G29 Passo 4: the LLM rewrite diverged too far from
            // the original body. Count as a soft failure (not
            // `failed`) so the queue surfaces it as a quality
            // issue, not a transport error. The reason is
            // structured so the operator can audit why a body
            // was rejected.
            skipped += 1;
            let reason = format!(
            "preservation_failed: jaccard={score:.3} threshold={threshold:.3} (orig={chars_before} chars, new={chars_after} chars)"
        );
            if let Err(qe) = queue_conn.execute(
            "UPDATE queue SET status='skipped', error=?1, done_at=datetime('now') WHERE id=?2",
            rusqlite::params![reason, queue_id],
        ) {
            tracing::warn!(target: "enrich", error = %qe, "queue preservation_failed update failed");
        }
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
                } else if std::time::Instant::now() >= rate_limit_deadline {
                    tracing::error!(target: "enrich", total_elapsed_secs = enrich_started.elapsed().as_secs(), "rate-limit retry deadline (1h) exhausted");
                } else {
                    let half = backoff_secs / 2;
                    let jitter = if half == 0 { 0 } else { fastrand::u64(0..half) };
                    let actual_wait = half + jitter;
                    tracing::warn!(target: "enrich", delay_secs = actual_wait, error_kind = "rate_limited", "rate limited, backing off");
                    if let Err(qe) = queue_conn.execute(
                        "UPDATE queue SET status='pending' WHERE id=?1",
                        rusqlite::params![queue_id],
                    ) {
                        tracing::warn!(target: "enrich", error = %qe, "queue pending update failed");
                    }
                    std::thread::sleep(std::time::Duration::from_secs(actual_wait));
                    backoff_secs = (backoff_secs * 2).min(900);
                    continue;
                }
            }

            failed += 1;
            // GAP-SG-73: prefer the origin-typed verdict
            // (ChatError::retry_class) over the untyped fallback
            // classifier whenever this item's failure came from
            // an OpenRouter chat call.
            let _outcome = match openrouter_diag {
                Some(diag) => record_item_failure_typed(
                    queue_conn,
                    queue_id,
                    attempt_current,
                    args.max_attempts,
                    diag.retry_class,
                    &err_str,
                    diag.finish_reason.as_deref(),
                    diag.prompt_tokens,
                    diag.completion_tokens,
                ),
                None => record_item_failure(
                    queue_conn,
                    queue_id,
                    attempt_current,
                    args.max_attempts,
                    &e,
                ),
            };
            emit_json(&ItemEvent {
                item: &item_key,
                status: "failed",
                memory_id: None,
                entity_id: None,
                entities: None,
                rels: None,
                chars_before: None,
                chars_after: None,
                cost_usd: None,
                elapsed_ms: Some(item_started.elapsed().as_millis() as u64),
                error: Some(err_str),
                index: current_index,
                total,
            });
        }
    }

    let _ = item_type; // used via queue schema only
}

    counters.completed = completed;
    counters.failed = failed;
    counters.skipped = skipped;
    counters.cost_total = cost_total;
    counters.oauth_detected = oauth_detected;
    let _ = (paths, conn, item_type, yield_every, items_since_yield, yield_count, preempted_for_gate, enrich_started, until_deadline);
    Ok(())
}
