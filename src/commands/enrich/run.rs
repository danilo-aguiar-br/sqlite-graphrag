//! Enrich command orchestrator: scan → enqueue → drain (serial/parallel).
//! Extracted from mod.rs (Wave C1).

#![allow(unused_assignments)] // counters reassigned across drain modes

use std::path::PathBuf;
use std::time::Instant;

use rusqlite::Connection;

use super::args::{EnrichArgs, EnrichMode, EnrichOperation, ReEmbedTarget};
use super::events::*;
use super::extraction::find_codex_binary;
use super::postprocess::take_enrich_backend;
use super::queue::{
    enqueue_candidate, item_type_for, item_type_for_key,
    open_queue_db, reset_stale_processing_claims,
    skipped_item_keys,
};
use super::scan::{
    self as scan, count_operation_backlog,
};
use super::scheduler;
use super::DEFAULT_RATE_LIMIT_WAIT;
use crate::commands::ingest_claude::find_claude_binary;
use crate::errors::AppError;
use crate::output::emit_json_line as emit_json;
use crate::paths::AppPaths;
use crate::storage::connection::{ensure_db_ready, open_rw};

pub fn run(
    args: &EnrichArgs,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<(), AppError> {
    // GAP-CLI-PRIO-04 / OBS-02: --ops-gate runs quality gate ops first, in order.
    if args.ops_gate {
        for op in scheduler::gate_ops_order() {
            let mut gate_args = args.clone();
            gate_args.operation = Some(op);
            gate_args.ops_gate = false; // prevent recursion
            run(&gate_args, llm_backend, embedding_backend)?;
        }
        return Ok(());
    }

    // G20: mode-conditional flag validation BEFORE any DB access.
    // Surfaces flags that the wrong mode would silently discard.
    validate_mode_conditional_flags_enrich(args)?;

    // v1.1.1 (P2): --target only means something for re-embed. Fail loud
    // instead of silently ignoring it under another operation.
    if args.target != ReEmbedTarget::Memories
        && !matches!(args.operation(), EnrichOperation::ReEmbed)
    {
        let target_label = match args.target {
            ReEmbedTarget::Memories => "memories",
            ReEmbedTarget::Entities => "entities",
            ReEmbedTarget::Chunks => "chunks",
            ReEmbedTarget::All => "all",
        };
        return Err(AppError::Validation(format!(
            "--target {target_label} only applies to --operation re-embed"
        )));
    }

    if super::status::try_handle_maintenance(args)? {
        return Ok(());
    }

    // v1.0.95 (ADR-0054): when the JUDGE is OpenRouter the model is mandatory
    // (no default) and the API key must resolve BEFORE any network or DB work.
    // The chat client singleton is initialised here so every per-item dispatch
    // fetches it without re-threading the key.
    //
    // GAP-CLI-DRY-01 (v1.1.8): dry-run never calls the LLM — skip provider
    // key/model resolution so offline agents can preview candidates without
    // credentials. `--status` already returns earlier above.
    if args.mode() == EnrichMode::OpenRouter && !args.dry_run {
        let model = args.openrouter_model.as_deref().ok_or_else(|| {
            AppError::Validation(
                "--mode openrouter requires --openrouter-model (no default model is allowed)"
                    .into(),
            )
        })?;
        let resolved =
            crate::config::resolve_api_key("openrouter", args.openrouter_api_key.as_deref())
                .ok_or_else(|| {
                    AppError::Validation(
                        "OpenRouter API key not found; store it via \
                         `config add-key --provider openrouter`, or pass --openrouter-api-key \
                         (product env is deprecated)"
                            .into(),
                    )
                })?;
        crate::embedder::get_openrouter_chat_client(
            resolved.value,
            model,
            args.openrouter_timeout,
        )?;
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
    let wait_secs = args.wait_job_singleton;
    let force_flag = args.force_job_singleton;
    let _singleton = crate::lock::acquire_job_singleton(
        crate::lock::JobType::Enrich,
        &namespace,
        &paths.db,
        wait_secs,
        force_flag,
    )?;

    // Validate provider binary upfront only for LLM-backed write operations.
    // GAP-CLI-DRY-01: dry-run never spawns a provider — skip binary resolution.
    let provider_binary = if args.dry_run || matches!(args.operation(), EnrichOperation::ReEmbed) {
        None
    } else {
        Some(match args.mode() {
            EnrichMode::ClaudeCode => {
                let bin = find_claude_binary(args.claude_binary.as_deref())?;
                let version = crate::commands::claude_runner::validate_claude_version(&bin)?;
                tracing::info!(target: "enrich", binary = %bin.display(), version = %version, "Claude Code binary validated");
                emit_json(&PhaseEvent {
                    phase: "validate",
                    binary_path: bin.to_str(),
                    version: Some(&version),
                    items_total: None,
                    items_pending: None,
                    llm_parallelism: None,
                });
                bin
            }
            EnrichMode::Codex => {
                let bin = find_codex_binary(args.codex_binary.as_deref())?;
                emit_json(&PhaseEvent {
                    phase: "validate",
                    binary_path: bin.to_str(),
                    version: None,
                    items_total: None,
                    items_pending: None,
                    llm_parallelism: None,
                });
                bin
            }
            EnrichMode::Opencode => {
                let bin = crate::commands::opencode_runner::find_opencode_binary_with_override(
                    args.opencode_binary.as_deref(),
                )?;
                emit_json(&PhaseEvent {
                    phase: "validate",
                    binary_path: bin.to_str(),
                    version: None,
                    items_total: None,
                    items_pending: None,
                    llm_parallelism: None,
                });
                bin
            }
            EnrichMode::OpenRouter => {
                // v1.0.95: the OpenRouter JUDGE is a REST call, not a spawned
                // binary. The chat client singleton was initialised at the top
                // of run(); this placeholder path threads through the dispatch
                // but is never dereferenced by the OpenRouter arm.
                emit_json(&PhaseEvent {
                    phase: "validate",
                    binary_path: None,
                    version: None,
                    items_total: None,
                    items_pending: None,
                    llm_parallelism: None,
                });
                PathBuf::new()
            }
        })
    };

    // G28-D: refuse to start when the system is saturated. This check
    // is BEFORE preflight so we never spend an OAuth turn on a host
    // that is already at the limit.
    if args.max_load_check && !args.dry_run && crate::system_load::is_system_saturated() {
        let load = crate::system_load::load_average_one();
        let n = crate::system_load::ncpus();
        return Err(AppError::Validation(format!(
            "system load average {load:.2} exceeds 2x ncpus ({n}); \
             pass --no-max-load-check to override (not recommended)"
        )));
    }

    // G35: preflight probe — issue a single ping turn to verify the
    // provider is healthy before scanning N candidates. If the probe
    // fails with a rate-limit error, optionally fall back to a
    // different mode (typically codex) instead of failing the entire
    // batch. The probe itself consumes 1 OAuth turn, so it stays
    // opt-in (default off) to keep --dry-run and CI flows zero-cost.
    if args.preflight_check
        && !args.dry_run
        && !matches!(args.operation(), EnrichOperation::ReEmbed)
    {
        let preflight_result = run_preflight_probe(args);
        match preflight_result {
            PreflightOutcome::Healthy => {
                tracing::info!(target: "enrich", mode = ?args.mode(), "preflight probe healthy");
            }
            PreflightOutcome::RateLimited { reason, suggestion } => {
                if let Some(fallback) = args.fallback_mode.clone() {
                    if fallback != args.mode() {
                        // G35 (v1.0.69): the mid-batch mode switch is
                        // intentionally NOT applied because it would
                        // desynchronise the per-item rate-limit wait
                        // state (rate-limited items in the worker are
                        // timed against the original provider). Instead
                        // we abort cleanly so the operator can re-invoke
                        // with `--mode {fallback:?}`. This guarantees no
                        // OAuth window is wasted and no partial state
                        // is left in the queue.
                        return Err(AppError::Validation(format!(
                            "preflight detected rate limit on {mode:?}: {reason}; \
                             re-invoke with `--mode {fallback:?}` to use the fallback provider",
                            mode = args.mode()
                        )));
                    }
                    return Err(AppError::Validation(format!(
                        "preflight detected rate limit on {mode:?}: {reason}; \
                         --fallback-mode matches --mode, no recovery possible",
                        mode = args.mode()
                    )));
                }
                return Err(AppError::Validation(format!(
                    "preflight detected rate limit on {mode:?}: {reason}; \
                     {suggestion}; pass --fallback-mode codex to recover",
                    mode = args.mode()
                )));
            }
            PreflightOutcome::Error(e) => {
                return Err(e);
            }
        }
    }

    // v1.1.06 (GAP-ENTITY-CONNECT-SCAN-CARTESIAN): wall-clock deadline covers
    // the **first** scan (and later rescans), not only the drain loop tail.
    // Default 3600s matches --max-runtime; entity-connect also gets a soft
    // ceiling so a hung SQL cannot pin the singleton forever when the operator
    // omits --max-runtime without --until-empty.
    let max_runtime_secs = args.max_runtime.unwrap_or(3600);
    let until_deadline = Instant::now() + std::time::Duration::from_secs(max_runtime_secs);
    let pair_scan_ops = matches!(
        args.operation(),
        EnrichOperation::EntityConnect | EnrichOperation::CrossDomainBridges
    );
    // Soft ceiling for pair scans when no explicit short budget is set.
    const ENTITY_CONNECT_SCAN_SOFT_CEILING_SECS: u64 = 120;
    let scan_deadline = if pair_scan_ops {
        let soft =
            Instant::now() + std::time::Duration::from_secs(ENTITY_CONNECT_SCAN_SOFT_CEILING_SECS);
        Some(soft.min(until_deadline))
    } else if args.until_empty || args.max_runtime.is_some() {
        Some(until_deadline)
    } else {
        None
    };

    let pair_op_cli = enrich_operation_cli_name(&args.operation());
    let mut backlog_degree0_proxy: Option<i64> = None;
    if pair_scan_ops {
        let entities_in_namespace: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entities WHERE namespace = ?1",
                rusqlite::params![namespace],
                |r| r.get(0),
            )
            .unwrap_or(0);
        // Distinct from pairs_enqueued_this_scan: status proxy of islands with NER.
        backlog_degree0_proxy =
            count_operation_backlog(&conn, &args.operation(), &namespace, args.target).ok();
        emit_json(&ScanStartEvent {
            phase: "scan_start",
            operation: pair_op_cli,
            entities_in_namespace,
            backlog_degree0_proxy,
            pair_algorithm: Some("cooccurrence+hub_island"),
            limit: args.limit,
            scan_deadline_secs: scan_deadline
                .map(|d| d.saturating_duration_since(Instant::now()).as_secs()),
        });
    }

    // SCAN phase
    let scan_started = Instant::now();
    let mut scan_result = scan_operation_with_deadline(&conn, &namespace, args, scan_deadline)?;
    // GAP-SG-69: body-enrich candidates are scanned purely by `LENGTH(body) <
    // min_output_chars`, so a short body whose rewrite the preservation guard
    // keeps rejecting is re-scanned every pass — items_total never reaches 0 and
    // `--until-empty` never converges (the detached worker reported a stuck
    // backlog for 30+ min). Exclude memories already vetoed `status='skipped'`
    // for this operation in the sidecar queue; `cleanup_queue_entry`
    // (remember/edit/forget/purge) clears the veto when the body actually
    // changes, so a genuinely updated memory is reconsidered automatically.
    if matches!(args.operation(), EnrichOperation::BodyEnrich) {
        let q_path = crate::paths::sidecar_path(&paths.db, ".enrich-queue.sqlite");
        if let Ok(q) = open_queue_db(&q_path) {
            if let Ok(vetoed) = skipped_item_keys(&q, &format!("{:?}", args.operation())) {
                scan_result.retain(|k| !vetoed.contains(k));
            }
        }
    }
    let total = scan_result.len();
    let scan_elapsed_ms = scan_started.elapsed().as_millis() as u64;

    emit_json(&PhaseEvent {
        phase: "scan",
        binary_path: None,
        version: None,
        items_total: Some(total),
        items_pending: Some(total),
        llm_parallelism: Some(args.llm_parallelism),
    });
    if pair_scan_ops {
        emit_json(&serde_json::json!({
            "phase": "scan_meta",
            "operation": pair_op_cli,
            "pair_algorithm": "cooccurrence+hub_island",
            "items_total": total,
            "pairs_enqueued_this_scan": total,
            "backlog_degree0_proxy": backlog_degree0_proxy,
            "scan_elapsed_ms": scan_elapsed_ms,
            "scan_aborted_reason": serde_json::Value::Null,
        }));
    }

    // Dry-run: emit preview events and summary without calling LLM
    if args.dry_run {
        // GAP-CLI-NAMES-03 / G-T-ONESHOT-01: explicit empty-match when a name
        // filter was provided but no candidates matched.
        if total == 0 {
            let name_filter = scan::resolve_name_filter(args).unwrap_or_default();
            if !name_filter.is_empty() {
                emit_json(&serde_json::json!({
                    "matched": 0,
                    "hint": "no candidates matched --names/--entity-names/--memory-names for this operation; verify name space (entity vs memory) and predicates (e.g. empty description unless --force-redescribe)",
                    "operation": format!("{:?}", args.operation()),
                    "names_requested": name_filter,
                }));
            }
        }
        for (idx, key) in scan_result.iter().enumerate() {
            emit_json(&ItemEvent {
                item: key,
                status: "preview",
                memory_id: None,
                entity_id: None,
                entities: None,
                rels: None,
                chars_before: None,
                chars_after: None,
                cost_usd: None,
                elapsed_ms: None,
                error: None,
                index: idx,
                total,
            });
        }
        emit_json(&EnrichSummary {
            summary: true,
            operation: format!("{:?}", args.operation()),
            items_total: total,
            completed: 0,
            failed: 0,
            skipped: 0,
            cost_usd: 0.0,
            elapsed_ms: started.elapsed().as_millis() as u64,
            backend_invoked: take_enrich_backend(),
            waiting: 0,
            dead: 0,
            budget_exhausted: None,
            pairs_remaining_estimate: None,
            yields: None,
            preempted_for_gate: None,
        });
        return Ok(());
    }

    // All operations in this enum have an execution path.

    // Queue setup for resume/retry (GAP-SG-64: sidecar alongside --db).
    // v1.1.2 (Bug 4): `mut` is required because the enqueue batch (D5) opens a
    // transaction on this connection.
    let queue_path = crate::paths::sidecar_path(&paths.db, ".enrich-queue.sqlite");
    let mut queue_conn = open_queue_db(&queue_path)?;

    // v1.1.2 (Bug 4): sweep stale `processing` claims left by a previous kill -9
    // BEFORE the singleton/drain starts, on EVERY run (not only --resume). A row
    // orphaned mid-LLM-call never clears its claimed_at, so without this sweep the
    // next run would never re-select it and the backlog would silently shrink.
    {
        let stale_reset = reset_stale_processing_claims(&queue_conn, args.stale_claim_secs)?;
        if stale_reset > 0 {
            tracing::info!(
                target: "enrich",
                count = stale_reset,
                max_age_secs = args.stale_claim_secs,
                "reset stale processing claims (older than threshold)"
            );
        }
    }

    if args.resume {
        let reset = queue_conn
            .execute(
                "UPDATE queue SET status='pending' WHERE status='processing'",
                [],
            )
            .map_err(|e| AppError::Validation(format!("queue resume failed: {e}")))?;
        if reset > 0 {
            tracing::info!(target: "enrich", count = reset, "reset stuck processing items to pending");
        }
    }

    if args.retry_failed {
        let count = queue_conn
            .execute(
                "UPDATE queue SET status='pending', attempt=0 WHERE status='failed'",
                [],
            )
            .map_err(|e| AppError::Validation(format!("queue retry-failed reset failed: {e}")))?;
        tracing::info!(target: "enrich", count, "retrying failed items");
    }

    if !args.resume && !args.retry_failed && !args.until_empty {
        queue_conn
            .execute("DELETE FROM queue", [])
            .map_err(|e| AppError::Validation(format!("queue clear failed: {e}")))?;
    }

    // Populate queue (GAP-SG-12: tag rows with the operation + link memory_id).
    // v1.1.2 (Bug 4, D5): batch every INSERT in a single transaction so hundreds
    // of candidates commit with one fsync instead of one-per-statement. The
    // memory_id resolution SELECT runs against the main DB (read-only here) and
    // stays outside the queue transaction.
    let op_label = format!("{:?}", args.operation());
    let item_type = item_type_for(&args.operation());
    {
        let tx = queue_conn.transaction()?;
        // v1.1.2 (Bug 4): `Transaction` derefs to `Connection`, so `&*tx` yields
        // the `&Connection` the existing enqueue_candidate signature expects.
        let tx_conn: &Connection = &tx;
        for key in scan_result.iter() {
            // v1.1.1 (P2): re-embed keys may be prefixed (`entity:` / `chunk:`);
            // derive the row item_type from the key so prune-dead-orphans never
            // mistakes an entity/chunk row for an orphaned memory.
            let it = item_type_for_key(key, item_type);
            enqueue_candidate(tx_conn, &conn, &namespace, key, it, &op_label);
        }
        tx.commit()?;
    }

    let parallelism = super::events::resolve_drain_parallelism(args);

    let mut completed = 0usize;
    #[allow(unused_assignments)]
    let mut failed = 0usize;
    #[allow(unused_assignments)]
    let mut skipped = 0usize;
    #[allow(unused_assignments)]
    let mut cost_total = 0.0f64;
    #[allow(unused_assignments, unused_variables)]
    let mut oauth_detected = false;
    let mut counters = super::drain_parallel::DrainCounters::default();
    let backoff_secs = DEFAULT_RATE_LIMIT_WAIT;
    let rate_limit_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3600);
    let enrich_started = std::time::Instant::now();

    let provider_timeout = match args.mode() {
        EnrichMode::ClaudeCode => args.claude_timeout,
        EnrichMode::Codex => args.codex_timeout,
        EnrichMode::Opencode => args.opencode_timeout,
        EnrichMode::OpenRouter => args.openrouter_timeout,
    };

    let provider_model: Option<&str> = match args.mode() {
        EnrichMode::ClaudeCode => args.claude_model.as_deref(),
        EnrichMode::Codex => args.codex_model.as_deref(),
        EnrichMode::Opencode => args.opencode_model.as_deref(),
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
                let mut rescan =
                    scan_operation_with_deadline(&conn, &namespace, args, Some(until_deadline))?;
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
        let completed_before = completed;

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
            completed = counters.completed;
            failed = counters.failed;
            skipped = counters.skipped;
            cost_total = counters.cost_total;
            oauth_detected = counters.oauth_detected;
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
                until_deadline,
                rate_limit_deadline,
                backoff_secs,
            )?;
            completed = counters.completed;
            failed = counters.failed;
            skipped = counters.skipped;
            cost_total = counters.cost_total;
            oauth_detected = counters.oauth_detected;
        }

        if !args.until_empty {
            break;
        }
        let eligible_remaining: i64 = queue_conn
            .query_row(
                &format!("SELECT COUNT(*) FROM queue WHERE status='pending' {backoff_clause}"),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let progressed = completed > completed_before;
        if std::time::Instant::now() >= until_deadline {
            tracing::info!(target: "enrich", "until-empty: max-runtime reached, stopping");
            break;
        }
        if !progressed && eligible_remaining == 0 {
            tracing::info!(target: "enrich", "until-empty: converged (no eligible items remain)");
            break;
        }
        if eligible_remaining == 0 {
            // Remaining pending items are waiting on backoff; nap and re-check.
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    } // end until-empty loop

    // v1.1.2 (Bug 4 / Omissão 3): SIGTERM graceful cleanup. When a shutdown was
    // requested mid-drain (the worker/serial loops already broke out), reset the
    // in-flight `processing` claims back to `pending` so the NEXT run re-selects
    // them — without this a kill recycles the rows via the startup stale-claim
    // sweep (which waits `stale_claim_secs`), delaying recovery. Scoped to the
    // enrich run (not the global signal handler) so unrelated code paths keep
    // their existing exit semantics. Best-effort: errors are logged, not fatal.
    if crate::shutdown_requested() {
        let reset = queue_conn
            .execute(
                "UPDATE queue SET status='pending', claimed_at=NULL WHERE status='processing'",
                [],
            )
            .unwrap_or(0);
        let _ = queue_conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        tracing::info!(
            target: "enrich",
            reset,
            "graceful shutdown: WAL checkpointed, processing claims reset"
        );
    }

    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    let _ = queue_conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");

    // GAP-SG-15: report items still in cooldown (waiting) and dead-lettered
    // alongside completed, so `--until-empty` makes the convergence state
    // explicit (cooldown vs. dead vs. truly empty) instead of just "done".
    let waiting_final: i64 = queue_conn
        .query_row(
            "SELECT COUNT(*) FROM queue WHERE status='pending' \
             AND (operation = ?1 OR operation IS NULL) \
             AND next_retry_at IS NOT NULL AND next_retry_at > datetime('now')",
            rusqlite::params![op_label],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let dead_final: i64 = queue_conn
        .query_row(
            "SELECT COUNT(*) FROM queue WHERE status='dead' \
             AND (operation = ?1 OR operation IS NULL)",
            rusqlite::params![op_label],
            |r| r.get(0),
        )
        .unwrap_or(0);

    emit_json(&EnrichSummary {
        summary: true,
        operation: format!("{:?}", args.operation()),
        items_total: total,
        completed,
        failed,
        skipped,
        cost_usd: cost_total,
        elapsed_ms: started.elapsed().as_millis() as u64,
        backend_invoked: take_enrich_backend(),
        waiting: waiting_final,
        dead: dead_final,
        budget_exhausted: if pair_scan_ops && std::time::Instant::now() >= until_deadline {
            Some(true)
        } else {
            None
        },
        pairs_remaining_estimate: backlog_degree0_proxy,
        yields: if yield_count > 0 { Some(yield_count) } else { None },
        preempted_for_gate: if preempted_for_gate {
            Some(true)
        } else {
            None
        },
    });

    if failed == 0 {
        // GAP-ENRICH-BACKLOG-CONVERGE: keep the queue file when dead-letter rows
        // exist so `enrich --status` can still report them on the next run.
        let dead: i64 = queue_conn
            .query_row("SELECT COUNT(*) FROM queue WHERE status='dead'", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        // GAP-SG-69: keep the sidecar queue while it still holds `skipped`
        // verdicts. Those rows tell the next scan which short bodies are
        // non-expandable; removing the file would lose the veto and the
        // body-enrich backlog would never converge. cleanup_queue_entry clears
        // a row when its memory is edited/forgotten, so the veto is not permanent.
        let skipped_remaining: i64 = queue_conn
            .query_row(
                "SELECT COUNT(*) FROM queue WHERE status='skipped'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if dead == 0 && skipped_remaining == 0 {
            let _ = std::fs::remove_file(&queue_path);
        }
    }

    Ok(())
}

// EnrichItemResult + call_* functions moved to extraction.rs
