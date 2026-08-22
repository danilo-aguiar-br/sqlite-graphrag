//! Drain loops for enrich (Wave C1 extraction from run.rs).

use std::path::Path;
use std::time::Instant;

use rusqlite::Connection;

use super::args::{EnrichArgs, EnrichOperation};
use super::events::*;
use super::extraction::{
    call_body_enrich, call_body_extract, call_deep_research_synth, call_description_enrich,
    call_domain_classify, call_entity_connect, call_entity_description, call_entity_type_validate,
    call_graph_audit, call_memory_bindings, call_relation_reclassify, call_weight_calibrate,
    take_last_openrouter_failure, BodyEnrichTuning, EnrichItemResult, ProviderCall,
};
use super::prompts;
use super::queue::{
    dequeue_next_pending, heartbeat, item_type_for, mark_done, mark_skipped, record_item_failure,
    record_item_failure_typed, requeue_rate_limited, requeue_wrong_op, skip_wrong_type,
    validate_claim, ClaimCheck, DequeueOutcome,
};
use super::reembed::{run_reembed_cycle, ReembedCycle, ReembedCycleCtx, ReembedTally};
use super::scheduler;
use super::DEFAULT_RATE_LIMIT_WAIT;
use crate::errors::AppError;
use crate::output::emit_json_line as emit_json;
use crate::paths::AppPaths;

use super::drain_parallel::DrainCounters;

/// What the drain reads from and writes to.
///
/// GAP-SG-264: these five travelled together through 23 positional parameters,
/// and `#[allow(clippy::too_many_arguments)]` said so out loud without changing
/// anything. The lint was right: at that arity the compiler stops helping —
/// `provider_model` and `op_label` are both `&str`, `provider_timeout` and
/// `total` are both integers, and transposing either pair type-checks.
pub(crate) struct DrainSession<'a> {
    pub(crate) args: &'a EnrichArgs,
    pub(crate) paths: &'a AppPaths,
    pub(crate) conn: &'a Connection,
    pub(crate) queue_conn: &'a Connection,
    pub(crate) namespace: &'a str,
}

/// Which backends answer, and how patiently.
///
/// The `llm` + `embedding` pair now arrives already named, as
/// [`crate::cli::BackendChoice`]: the two selectors are resolved together in
/// `main`, travel together down every write path, and are consumed together, so
/// carrying them as one field removes a transposition hazard here and keeps this
/// struct agreeing with the rest of the crate.
pub(crate) struct DrainProvider<'a> {
    /// Path to the provider binary; None only for the batched re-embed path.
    pub(crate) binary: Option<&'a Path>,
    /// Model identifier, when the caller pinned one.
    pub(crate) model: Option<&'a str>,
    /// Per-request timeout, in seconds.
    pub(crate) timeout: u64,
    /// Which LLM answers enrichment, and which backend computes embeddings.
    pub(crate) backends: crate::cli::BackendChoice,
}

/// What this drain is called, and how much of it there is.
pub(crate) struct DrainScope<'a> {
    pub(crate) op_label: &'a str,
    pub(crate) backoff_clause: &'a str,
    pub(crate) item_type: &'a str,
    pub(crate) total: usize,
}

/// Every clock the loop answers to.
pub(crate) struct DrainClocks {
    pub(crate) started: Instant,
    pub(crate) until_deadline: Instant,
    pub(crate) rate_limit_deadline: Instant,
    pub(crate) yield_every: usize,
    pub(crate) backoff_secs: u64,
}

/// The mutable tally the caller keeps across `--until-empty` rounds.
pub(crate) struct DrainProgress<'a> {
    pub(crate) counters: &'a mut DrainCounters,
    pub(crate) items_since_yield: &'a mut usize,
    pub(crate) yield_count: &'a mut u64,
    pub(crate) preempted_for_gate: &'a mut bool,
}

pub(crate) fn drain_serial(
    session: DrainSession<'_>,
    provider: DrainProvider<'_>,
    scope: DrainScope<'_>,
    clocks: DrainClocks,
    progress: DrainProgress<'_>,
) -> Result<(), AppError> {
    let DrainSession {
        args,
        paths,
        conn,
        queue_conn,
        namespace,
    } = session;
    let DrainProvider {
        binary: provider_binary,
        model: provider_model,
        timeout: provider_timeout,
        backends,
    } = provider;
    let DrainScope {
        op_label,
        backoff_clause,
        item_type,
        total,
    } = scope;
    let DrainClocks {
        started: enrich_started,
        until_deadline,
        rate_limit_deadline,
        yield_every,
        mut backoff_secs,
    } = clocks;
    let DrainProgress {
        counters,
        items_since_yield,
        yield_count,
        preempted_for_gate,
    } = progress;
    let mut completed = counters.completed;
    let mut failed = counters.failed;
    let mut skipped = counters.skipped;
    let mut retyped = counters.retyped;
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

        // GAP-SG-141 (B1): `re-embed` is the only operation whose handler can
        // serve N items with ONE remote call, so it claims a whole batch and
        // bypasses the one-row-per-call body below. The other 13 operations are
        // chat-backed and have no batch handler — they fall through unchanged.
        if matches!(args.operation(), EnrichOperation::ReEmbed) {
            let ctx = ReembedCycleCtx {
                main_conn: conn,
                queue_conn,
                namespace,
                op_label,
                backoff_clause,
                paths,
                backends,
                max_attempts: args.max_attempts,
                total,
                stdout_mu: None,
            };
            let mut tally = ReembedTally {
                completed,
                failed,
                skipped,
            };
            let cycle = run_reembed_cycle(&ctx, &mut tally, None);
            completed = tally.completed;
            failed = tally.failed;
            skipped = tally.skipped;
            match cycle {
                ReembedCycle::Progressed => continue,
                ReembedCycle::Empty | ReembedCycle::BreakerOpen => break,
                ReembedCycle::DbBusy => {
                    return Err(AppError::DbBusy(
                        crate::i18n::validation::sqlite_busy_exhausted_on_reembed_claim(),
                    ))
                }
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
            dequeue_next_pending(queue_conn, op_label, namespace, backoff_clause)
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
        let provider_bin = provider_binary.unwrap_or_else(|| std::path::Path::new(""));
        let mode = args.mode();
        let provider_call = ProviderCall {
            model: provider_model,
            timeout: provider_timeout,
            mode: &mode,
        };
        let call_result = match args.operation() {
            EnrichOperation::MemoryBindings | EnrichOperation::AugmentBindings => {
                call_memory_bindings(
                    conn,
                    namespace,
                    &item_key,
                    provider_bin,
                    provider_model,
                    provider_timeout,
                    &mode,
                )
            }
            EnrichOperation::EntityDescriptions => call_entity_description(
                conn,
                namespace,
                &item_key,
                provider_call,
                args.entity_description_grounding_threshold(),
                &prompts::resolve_entity_description_domain(&args.entity_description_domain),
            ),
            EnrichOperation::BodyEnrich => call_body_enrich(
                conn,
                namespace,
                &item_key,
                provider_call,
                BodyEnrichTuning {
                    min_output_chars: args.min_output_chars,
                    max_output_chars: args.max_output_chars,
                    prompt_template: args.prompt_template.as_deref(),
                    preserve_threshold: args.preserve_threshold,
                },
                paths,
                backends,
            ),
            // GAP-SG-141 (B1): handled by the batched cycle above; the arm
            // stays so the match remains exhaustive.
            EnrichOperation::ReEmbed => Ok(EnrichItemResult::Skipped {
                cost: 0.0,
                reason: crate::i18n::validation::reembed_served_by_batch_path(),
            }),
            EnrichOperation::WeightCalibrate => call_weight_calibrate(
                conn,
                namespace,
                &item_key,
                provider_bin,
                provider_model,
                provider_timeout,
                &mode,
            ),
            EnrichOperation::RelationReclassify => call_relation_reclassify(
                conn,
                namespace,
                &item_key,
                provider_bin,
                provider_model,
                provider_timeout,
                &mode,
            ),
            EnrichOperation::EntityConnect | EnrichOperation::CrossDomainBridges => {
                call_entity_connect(
                    conn,
                    namespace,
                    &item_key,
                    provider_bin,
                    provider_model,
                    provider_timeout,
                    &mode,
                )
            }
            EnrichOperation::EntityTypeValidate => call_entity_type_validate(
                conn,
                namespace,
                &item_key,
                provider_bin,
                provider_model,
                provider_timeout,
                &mode,
            ),
            EnrichOperation::DescriptionEnrich => call_description_enrich(
                conn,
                namespace,
                &item_key,
                provider_bin,
                provider_model,
                provider_timeout,
                &mode,
            ),
            EnrichOperation::DomainClassify => call_domain_classify(
                conn,
                namespace,
                &item_key,
                provider_bin,
                provider_model,
                provider_timeout,
                &mode,
            ),
            EnrichOperation::GraphAudit => call_graph_audit(
                conn,
                namespace,
                &item_key,
                provider_bin,
                provider_model,
                provider_timeout,
                &mode,
            ),
            EnrichOperation::DeepResearchSynth => call_deep_research_synth(
                conn,
                namespace,
                &item_key,
                provider_bin,
                provider_model,
                provider_timeout,
                &mode,
            ),
            EnrichOperation::BodyExtract => call_body_extract(
                conn,
                namespace,
                &item_key,
                provider_call,
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

                if let Err(e) = mark_done(
                    queue_conn,
                    queue_id,
                    memory_id,
                    entity_id,
                    entities,
                    rels,
                    cost,
                    item_started.elapsed().as_millis() as i64,
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
                            super::queue::count_priority_pending(
                                queue_conn,
                                "EntityDescriptions",
                                namespace,
                                super::queue::PRIORITY_HOT,
                            )
                            .unwrap_or(0)
                                > 0,
                            matches!(
                                args.operation(),
                                EnrichOperation::EntityConnect
                                    | EnrichOperation::CrossDomainBridges
                            ),
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
                        index: current_index,
                        total,
                        ..Default::default()
                    });
                } else {
                    failed += 1;
                    emit_json(&ItemEvent {
                        item: &item_key,
                        status: "failed",
                        elapsed_ms: Some(item_started.elapsed().as_millis() as u64),
                        error: persist_err,
                        index: current_index,
                        total,
                        ..Default::default()
                    });
                }
            }
            Ok(EnrichItemResult::Skipped { reason, cost }) => {
                skipped += 1;
                // G-PR-7: abstention is BILLED. A skip taken before the request
                // carries zero, but one taken because the model read the corpus
                // and declined costs full price — folding it in here is what
                // keeps the budget guard and the summary honest.
                cost_total += cost;
                if let Err(e) = mark_skipped(queue_conn, queue_id, &reason) {
                    tracing::warn!(target: "enrich", error = %e, "queue skipped update failed");
                }
                // GAP-SG-279: the reason travels to the CALLER, not only into the
                // sidecar. `mark_skipped` above has always recorded it; the event
                // carried `error: None` and nothing else, so a stream watcher saw
                // `skipped` with no way to tell an abstention from a duplicate
                // without opening the queue database by hand.
                emit_json(&ItemEvent {
                    item: &item_key,
                    status: "skipped",
                    cost_usd: (cost > 0.0).then_some(cost),
                    elapsed_ms: Some(item_started.elapsed().as_millis() as u64),
                    reason: Some(reason),
                    index: current_index,
                    total,
                    ..Default::default()
                });
            }
            Ok(EnrichItemResult::Retyped {
                entity_id,
                previous_type,
                validated_type,
                evidence_chars,
                cost,
                is_oauth,
            }) => {
                // GAP-SG-279: a reclassification is a completion that says WHAT
                // it changed. It feeds `completed` like any other success, plus a
                // dedicated counter, because the question an operator has after a
                // paid drain is how many labels moved — and `completed` alone
                // answered it identically whether ten thousand types changed or
                // none did.
                let elapsed_ms = item_started.elapsed().as_millis() as i64;
                if let Err(e) = mark_done(
                    queue_conn,
                    queue_id,
                    None,
                    Some(entity_id),
                    1,
                    0,
                    cost,
                    elapsed_ms,
                ) {
                    tracing::warn!(target: "enrich", error = %e, "queue done update failed");
                }
                completed += 1;
                retyped += 1;
                if !is_oauth {
                    cost_total += cost;
                }
                emit_json(&ItemEvent {
                    item: &item_key,
                    status: "retyped",
                    entity_id: Some(entity_id),
                    entities: Some(1),
                    rels: Some(0),
                    cost_usd: if is_oauth { None } else { Some(cost) },
                    elapsed_ms: Some(elapsed_ms as u64),
                    previous_type: Some(previous_type),
                    validated_type: Some(validated_type),
                    evidence_chars: Some(evidence_chars),
                    index: current_index,
                    total,
                    ..Default::default()
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
                if let Err(qe) = mark_skipped(queue_conn, queue_id, &reason) {
                    tracing::warn!(target: "enrich", error = %qe, "queue preservation_failed update failed");
                }
                emit_json(&ItemEvent {
                    item: &item_key,
                    status: "preservation_failed",
                    chars_before: Some(chars_before),
                    chars_after: Some(chars_after),
                    elapsed_ms: Some(item_started.elapsed().as_millis() as u64),
                    error: Some(reason),
                    index: current_index,
                    total,
                    ..Default::default()
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
                        if let Err(qe) = requeue_rate_limited(queue_conn, queue_id) {
                            tracing::warn!(target: "enrich", error = %qe, "queue pending update failed");
                        }
                        std::thread::sleep(std::time::Duration::from_secs(actual_wait));
                        backoff_secs =
                            (backoff_secs * 2).min(crate::constants::ENRICH_BACKOFF_CEILING_SECS);
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
                    elapsed_ms: Some(item_started.elapsed().as_millis() as u64),
                    error: Some(err_str),
                    index: current_index,
                    total,
                    ..Default::default()
                });
            }
        }

        let _ = item_type; // used via queue schema only
    }

    counters.completed = completed;
    counters.failed = failed;
    counters.skipped = skipped;
    counters.retyped = retyped;
    counters.cost_total = cost_total;
    counters.oauth_detected = oauth_detected;
    let _ = (
        paths,
        conn,
        item_type,
        yield_every,
        items_since_yield,
        yield_count,
        preempted_for_gate,
        enrich_started,
        until_deadline,
    );
    Ok(())
}
