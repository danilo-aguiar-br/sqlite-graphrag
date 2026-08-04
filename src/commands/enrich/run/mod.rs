//! Enrich command orchestrator: scan → enqueue → drain (serial/parallel).
//! Extracted from mod.rs (Wave C1).
//!
//! One submodule per STAGE of the drain: [`guards`] settles everything that can
//! end the invocation before a connection is opened, [`provider`] proves the LLM
//! provider is usable, [`budget`] derives the wall-clock deadlines, [`scan_phase`]
//! selects candidates, [`dry_run`] reports them without executing, [`queue_prep`]
//! brings the sidecar queue to a clean state and enqueues, and [`finalize`] closes
//! the run. The `--until-empty` drain loop itself stays here, because it is the
//! orchestration.

use std::time::Instant;

use rusqlite::Connection;

use super::args::{EnrichArgs, EnrichMode, EnrichOperation};
use super::events::ConcurrencyEvent;
use super::queue::{
    count_eligible_pending, enqueue_candidate, item_type_for, item_type_for_key, open_queue_db,
    skipped_item_keys,
};
use super::scheduler;
use super::DEFAULT_RATE_LIMIT_WAIT;
use crate::errors::AppError;
use crate::output::emit_json_line as emit_json;
use crate::paths::AppPaths;
use crate::storage::connection::{ensure_db_ready, open_rw};

mod budget;
mod dry_run;
mod finalize;
mod guards;
mod provider;
mod queue_prep;
mod scan_phase;

/// Run.
pub fn run(
    args: &EnrichArgs,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<(), AppError> {
    if guards::handle_pre_db_guards(args, llm_backend, embedding_backend)? {
        return Ok(());
    }

    let started = Instant::now();

    let paths = AppPaths::resolve(args.db.as_deref())?;
    ensure_db_ready(&paths)?;
    let conn = open_rw(&paths.db)?;
    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;

    // G28-B (v1.0.68) + G30 (v1.0.69): enforce singleton per
    // (job_type, namespace, db_hash) so two parallel `enrich` invocations
    // on the same DB cannot co-exist, but concurrent enrich on different
    // databases works as expected. The force flag (--force) breaks a
    // stale lock from a previously crashed invocation.
    let _singleton = crate::lock::acquire_job_singleton(
        crate::lock::JobType::Enrich,
        &namespace,
        &paths.db,
        args.wait_job_singleton,
        args.force_job_singleton,
    )?;

    let provider_binary = provider::resolve_provider_binary(args)?;
    provider::check_system_load(args)?;
    provider::run_preflight(args)?;

    let budget = budget::resolve(args);

    // Dry-run still materialises the full key list (offline preview). Production
    // uses page→enqueue (GAP-SG-185 v1.2.4) so peak key-buffer RSS tracks page_size.
    if args.dry_run {
        let scan = scan_phase::run_scan(&conn, &paths.db, &namespace, args, &budget)?;
        dry_run::emit_preview(args, &scan.keys, started);
        return Ok(());
    }

    // All operations in this enum have an execution path.

    // Queue setup for resume/retry (GAP-SG-64: sidecar alongside --db).
    // v1.1.2 (Bug 4): `mut` is required because the enqueue batch (D5) opens a
    // transaction on this connection.
    let queue_path = crate::paths::sidecar_path(&paths.db, ".enrich-queue.sqlite");
    let mut queue_conn = open_queue_db(&queue_path)?;

    // GAP-SG-97: never wipe the whole sidecar — scope clear to this operation
    // (and prefer namespace when the column is present).
    let op_label = format!("{:?}", args.operation());

    // prepare_queue force-redescribe reopen is applied per page below.
    queue_prep::prepare_queue(&queue_conn, &conn, args, &namespace, &op_label, &[])?;

    let item_type = item_type_for(&args.operation());
    let force_redescribe =
        args.force_redescribe && matches!(args.operation(), EnrichOperation::EntityDescriptions);
    // Emit scan_start for pair ops before streaming (same as scan_phase).
    let backlog_degree0_proxy = if budget.pair_scan_ops {
        scan_phase::emit_scan_start(
            &conn,
            &namespace,
            args,
            &budget,
            super::events::enrich_operation_cli_name(&args.operation()),
        )
    } else {
        None
    };
    let scan_started = Instant::now();
    let total = super::scan::scan_operation_for_each(&conn, &namespace, args, |page| {
        if force_redescribe {
            let _ = queue_prep::reopen_force_redescribe_page(&queue_conn, &namespace, &page);
        }
        queue_prep::enqueue_page(
            &mut queue_conn,
            &conn,
            &namespace,
            &page,
            item_type,
            &op_label,
        )?;
        Ok(())
    })?;
    // Body-enrich: candidates already vetoed as skipped are not re-enqueued
    // because INSERT OR IGNORE keeps the skipped row; scan still counted them.
    // Match legacy scan_phase filter by not special-casing here: skipped rows
    // remain skipped in the queue (GAP-SG-69).
    let scan_elapsed_ms = scan_started.elapsed().as_millis() as u64;
    emit_json(&super::events::PhaseEvent {
        phase: "scan",
        binary_path: None,
        version: None,
        items_total: Some(total),
        items_pending: Some(total),
        llm_parallelism: args.llm_parallelism,
    });
    if budget.pair_scan_ops {
        let op_cli = super::events::enrich_operation_cli_name(&args.operation());
        emit_json(&serde_json::json!({
            "phase": "scan_meta",
            "operation": op_cli,
            "pair_algorithm": "cooccurrence+hub_island",
            "items_total": total,
            "pairs_enqueued_this_scan": total,
            "backlog_degree0_proxy": backlog_degree0_proxy,
            "scan_elapsed_ms": scan_elapsed_ms,
            "scan_aborted_reason": serde_json::Value::Null,
        }));
    }
    queue_prep::log_enqueue_result(&queue_conn, &op_label, &namespace, total);

    let parallelism = super::events::resolve_drain_parallelism(args);

    let mut counters = super::drain_parallel::DrainCounters::default();
    let backoff_secs = DEFAULT_RATE_LIMIT_WAIT;
    let rate_limit_deadline = Instant::now() + crate::runtime_config::rate_limit_deadline_secs();
    let enrich_started = Instant::now();

    let provider_timeout = match args.mode() {
        EnrichMode::OpenRouter => args.openrouter_chat_timeout_secs(),
    };

    let provider_model: Option<&str> = match args.mode() {
        EnrichMode::OpenRouter => args.openrouter_model.as_deref(),
    };

    // GAP-SG-16: when --ignore-backoff is set, drop the per-item cooldown filter
    // from candidate selection so items parked on `next_retry_at` are eligible
    // immediately. Shared by the parallel workers and the serial loop.
    let backoff_clause: &str = if args.ignore_backoff {
        ""
    } else {
        "AND (next_retry_at IS NULL OR next_retry_at <= datetime('now'))"
    };

    // GAP-SG-45: announce the scan-vs-drain concurrency split (scan is always
    // serial; drain uses `parallelism` workers).
    emit_json(&ConcurrencyEvent {
        phase: "concurrency",
        scan_parallelism: 1,
        drain_parallelism: parallelism as u32,
    });

    // GAP-ENRICH-BACKLOG-CONVERGE: --until-empty wraps the scan→populate→drain
    // cycle in an internal loop so the external bash retry loop is unnecessary.
    // Without --until-empty the loop body runs exactly once (legacy behaviour).
    //
    // v1.1.06: `until_deadline` was already computed before the first scan so
    // --max-runtime covers scan+drain. Skip the identical re-scan on the first
    // until-empty iteration (candidates were just enqueued above).
    let mut until_empty_iter: u32 = 0;
    let yield_every = scheduler::resolve_yield_every_n(args.yield_every_n_items);
    let mut yield_count: u64 = 0;
    let mut items_since_yield: usize = 0;
    // Wave 3: set when EC breaks to let HOT entity-descriptions run.
    let mut preempted_for_gate = false;
    // Workload: mixed — SQLite queue I/O is serial; LLM fan-out is bounded
    // by host semaphore elsewhere. Yield/preempt keep gate ops responsive.
    loop {
        if args.until_empty {
            until_empty_iter = until_empty_iter.saturating_add(1);
            if until_empty_iter > 1 {
                // Re-scan and re-enqueue eligible candidates each iteration.
                // INSERT OR IGNORE never resurrects a dead-letter row (item_key is
                // UNIQUE), so the backlog converges instead of looping forever.
                let mut rescan = super::events::scan_operation_with_deadline(
                    &conn,
                    &namespace,
                    args,
                    Some(budget.until_deadline),
                )?;
                // GAP-SG-69: drop memories already vetoed `status='skipped'` so the
                // re-scan converges instead of re-enqueuing a non-expandable short
                // body every iteration (body-enrich only; the verdict persists in
                // the sidecar queue and is cleared by cleanup_queue_entry on edit).
                if matches!(args.operation(), EnrichOperation::BodyEnrich) {
                    if let Ok(vetoed) = skipped_item_keys(&queue_conn, &op_label) {
                        rescan.retain(|k| !vetoed.contains(k));
                    }
                }
                // v1.1.2 (Bug 4, D5): batch the re-scan INSERTs in one transaction.
                {
                    let tx = queue_conn.transaction()?;
                    let tx_conn: &Connection = &tx;
                    for key in &rescan {
                        let it = item_type_for_key(key, item_type);
                        enqueue_candidate(tx_conn, &conn, &namespace, key, it, &op_label);
                    }
                    tx.commit()?;
                }
            }
        }
        let completed_before = counters.completed;

        // G19: when parallelism > 1, spawn bounded worker threads.
        // Each worker opens its own DB connections (WAL supports concurrent readers + serialized writers).
        // The queue DB claim is atomic via UPDATE...RETURNING — no external lock needed.
        if parallelism > 1 {
            super::drain_parallel::drain_parallel(
                args,
                &paths,
                &queue_path,
                &namespace,
                provider_binary.as_deref(),
                provider_model,
                provider_timeout,
                &op_label,
                backoff_clause,
                parallelism,
                total,
                llm_backend,
                embedding_backend,
                &mut counters,
            )?;
        } else {
            super::drain_serial::drain_serial(
                args,
                &paths,
                &conn,
                &queue_conn,
                &namespace,
                provider_binary.as_deref(),
                provider_model,
                provider_timeout,
                &op_label,
                backoff_clause,
                item_type,
                total,
                llm_backend,
                embedding_backend,
                yield_every,
                &mut counters,
                &mut items_since_yield,
                &mut yield_count,
                &mut preempted_for_gate,
                enrich_started,
                budget.until_deadline,
                rate_limit_deadline,
                backoff_secs,
            )?;
        }

        if !args.until_empty {
            break;
        }
        // CAPA-A: isolate until-empty convergence to this op+ns (dequeue parity).
        let eligible_remaining =
            count_eligible_pending(&queue_conn, &op_label, &namespace, backoff_clause);
        let progressed = counters.completed > completed_before;
        if Instant::now() >= budget.until_deadline {
            tracing::info!(target: "enrich", "until-empty: max-runtime reached, stopping");
            break;
        }
        if !progressed && eligible_remaining == 0 {
            tracing::info!(target: "enrich", "until-empty: converged (no eligible items remain)");
            break;
        }
        if eligible_remaining == 0 {
            // Remaining pending items are waiting on backoff; nap and re-check.
            std::thread::sleep(std::time::Duration::from_secs(
                crate::constants::ENRICH_UNTIL_EMPTY_IDLE_NAP_SECS,
            ));
        }
    } // end until-empty loop

    finalize::finish(
        &conn,
        &queue_conn,
        &queue_path,
        args,
        &op_label,
        &namespace,
        finalize::FinalTally {
            counters: &counters,
            items_total: total,
            started,
            until_deadline: budget.until_deadline,
            pair_scan_ops: budget.pair_scan_ops,
            backlog_degree0_proxy,
            yield_count,
            preempted_for_gate,
        },
    );

    Ok(())
}
