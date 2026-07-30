//! Orchestration entry point for the `ingest` command.

use super::args::{resolve_parallelism, IngestArgs, IngestMode};
use super::persist::{init_storage, persist_staged};
use super::report::{
    FileSuccess, IngestDryRunBudget, IngestFileEvent, IngestSummary, StageProgressEvent,
};
use super::scan_fs::{collect_files, derive_kebab_name, unique_name, validate_name_prefix};
use super::stage::{stage_file, StagedFile};
use super::validate::validate_mode_conditional_flags_ingest;
use crate::chunking;
use crate::constants::DERIVED_NAME_MAX_LEN;
use crate::errors::AppError;
use crate::output;
use crate::paths::AppPaths;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc;

/// Run the `ingest` command (filesystem scan + stage + persist, or mode adapters).
pub fn run(
    args: IngestArgs,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<(), AppError> {
    // G20: mode-conditional flag validation BEFORE any DB access.
    // Surfaces flags that the wrong mode would silently discard.
    validate_mode_conditional_flags_ingest(&args)?;
    tracing::debug!(target: "ingest", dir = %args.dir.display(), mode = ?args.mode, "starting ingest");
    if args.mode == IngestMode::ClaudeCode {
        return crate::commands::ingest_claude::run_claude_ingest(&args, embedding_backend, llm_backend);
    }
    if args.mode == IngestMode::Codex {
        return crate::commands::ingest_codex::run_codex_ingest(&args);
    }
    if args.mode == IngestMode::Opencode {
        return crate::commands::ingest_opencode::run_opencode_ingest(&args);
    }

    let started = std::time::Instant::now();

    if !args.dir.exists() {
        return Err(AppError::Validation(
            crate::i18n::validation::directory_not_found(&args.dir.display().to_string()),
        ));
    }
    if !args.dir.is_dir() {
        return Err(AppError::Validation(
            crate::i18n::validation::not_a_directory(&args.dir.display().to_string()),
        ));
    }

    let mut files: Vec<PathBuf> = Vec::with_capacity(128);
    collect_files(&args.dir, &args.pattern, args.recursive, &mut files)?;
    files.sort_unstable();

    if files.len() > args.max_files {
        return Err(AppError::Validation(
            crate::i18n::validation::max_files_exceeded_matching(files.len(), args.max_files),
        ));
    }

    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let memory_type_str = args.r#type.as_str().to_string();

    let paths = AppPaths::resolve(args.db.as_deref())?;
    let mut conn_or_err = match init_storage(&paths) {
        Ok(c) => Ok(c),
        Err(e) => Err(format!("{e}")),
    };

    let mut succeeded: usize = 0;
    let mut failed: usize = 0;
    let mut skipped: usize = 0;
    let total = files.len();

    // Pre-resolve all names before parallelisation so Phase A workers see a
    // consistent, immutable name assignment (v1.0.31 A10 contract preserved).
    let mut taken_names: BTreeSet<String> = BTreeSet::new();

    // SlotMeta: per-slot output metadata retained on the main thread for NDJSON.
    // ProcessItem: the data moved into the producer thread for Phase A computation.
    // We split these so `slots_meta` (non-Send BTreeSet-dependent) stays on main
    // thread while `process_items` (Send: only PathBuf + String) crosses the thread
    // boundary into the rayon producer.
    enum SlotMeta {
        Skip {
            file_str: String,
            derived_base: String,
            name_truncated: bool,
            original_name: Option<String>,
            original_filename: Option<String>,
            reason: String,
        },
        Process {
            file_str: String,
            derived_name: String,
            name_truncated: bool,
            original_name: Option<String>,
            original_filename: Option<String>,
        },
    }

    struct ProcessItem {
        idx: usize,
        path: PathBuf,
        file_str: String,
        derived_name: String,
    }

    let files_cap = files.len();
    let mut slots_meta: Vec<SlotMeta> = Vec::new();
    slots_meta.try_reserve(files_cap).map_err(|_| {
        AppError::LimitExceeded(format!(
            "allocation of {files_cap} slot metadata entries would exceed available memory"
        ))
    })?;
    let mut process_items: Vec<ProcessItem> = Vec::new();
    process_items.try_reserve(files_cap).map_err(|_| {
        AppError::LimitExceeded(format!(
            "allocation of {files_cap} process items would exceed available memory"
        ))
    })?;
    let mut truncations: Vec<(String, String)> = Vec::new();
    truncations.try_reserve(files_cap).map_err(|_| {
        AppError::LimitExceeded(format!(
            "allocation of {files_cap} truncation entries would exceed available memory"
        ))
    })?;

    // v1.1.1 (P12): validate the prefix once and shrink the derived-name
    // budget so `prefix + derived` always fits MAX_MEMORY_NAME_LEN.
    let max_name_length = match args.name_prefix.as_deref() {
        Some(prefix) => validate_name_prefix(prefix, args.max_name_length)?,
        None => args.max_name_length,
    };
    for path in &files {
        let file_str = path.to_string_lossy().into_owned();
        let (derived_base, name_truncated, original_name) =
            derive_kebab_name(path, max_name_length);
        let original_basename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        if name_truncated {
            if let Some(ref orig) = original_name {
                truncations.push((orig.clone(), derived_base.clone()));
            }
        }

        if derived_base.is_empty() {
            // original_filename: always include when it differs from the empty derived name
            let orig_filename = if !original_basename.is_empty() {
                Some(original_basename.to_string())
            } else {
                None
            };
            slots_meta.push(SlotMeta::Skip {
                file_str,
                derived_base: String::new(),
                name_truncated: false,
                original_name: None,
                original_filename: orig_filename,
                reason: "could not derive a non-empty kebab-case name from filename".to_string(),
            });
            continue;
        }

        // v1.1.1 (P12): prefix applied AFTER kebab normalization of the
        // basename; the shrunken budget above guarantees the final length
        // fits MAX_MEMORY_NAME_LEN.
        let derived_base = match args.name_prefix.as_deref() {
            Some(prefix) => format!("{prefix}{derived_base}"),
            None => derived_base,
        };

        match unique_name(&derived_base, &taken_names) {
            Ok(derived_name) => {
                taken_names.insert(derived_name.clone());
                let idx = slots_meta.len();
                // original_filename: present only when the raw basename differs from the derived name
                let orig_filename = if original_basename != derived_name {
                    Some(original_basename.to_string())
                } else {
                    None
                };
                process_items.push(ProcessItem {
                    idx,
                    path: path.clone(),
                    file_str: file_str.clone(),
                    derived_name: derived_name.clone(),
                });
                slots_meta.push(SlotMeta::Process {
                    file_str,
                    derived_name,
                    name_truncated,
                    original_name,
                    original_filename: orig_filename,
                });
            }
            Err(e) => {
                let orig_filename = if original_basename != derived_base {
                    Some(original_basename.to_string())
                } else {
                    None
                };
                slots_meta.push(SlotMeta::Skip {
                    file_str,
                    derived_base,
                    name_truncated,
                    original_name,
                    original_filename: orig_filename,
                    reason: e.to_string(),
                });
            }
        }
    }

    if !truncations.is_empty() {
        tracing::info!(
            target: "ingest",
            count = truncations.len(),
            max_name_length = max_name_length,
            max_len = DERIVED_NAME_MAX_LEN,
            "derived names truncated; pass -vv (debug) for per-file detail"
        );
    }

    // --dry-run: emit preview events and exit before loading ONNX or touching DB.
    if args.dry_run {
        for meta in &slots_meta {
            match meta {
                SlotMeta::Skip {
                    file_str,
                    derived_base,
                    name_truncated,
                    original_name,
                    original_filename,
                    reason,
                } => {
                    output::emit_json_compact(&IngestFileEvent {
                        file: file_str,
                        name: derived_base,
                        status: "skip",
                        truncated: *name_truncated,
                        original_name: original_name.clone(),
                        original_filename: original_filename.as_deref(),
                        error: Some(reason.clone()),
                        memory_id: None,
                        action: None,
                        body_length: 0,
                        backend_invoked: None,
                    })?;
                }
                SlotMeta::Process {
                    file_str,
                    derived_name,
                    name_truncated,
                    original_name,
                    original_filename,
                } => {
                    output::emit_json_compact(&IngestFileEvent {
                        file: file_str,
                        name: derived_name,
                        status: "preview",
                        truncated: *name_truncated,
                        original_name: original_name.clone(),
                        original_filename: original_filename.as_deref(),
                        error: None,
                        memory_id: None,
                        action: None,
                        body_length: 0,
                        backend_invoked: None,
                    })?;

                    // GAP-SG-06: report chunk + token counts and how many
                    // sub-memories an auto-split would create, so the operator
                    // detects chunk/token overflow before a real ingest.
                    match std::fs::read_to_string(file_str) {
                        Ok(body) => {
                            let budget = chunking::assess_body_budget(&body);
                            output::emit_json_compact(&IngestDryRunBudget {
                                budget: true,
                                file: file_str,
                                name: derived_name,
                                bytes: budget.bytes,
                                chunk_count: budget.chunk_count,
                                token_count: budget.approx_tokens,
                                partition_count: budget.partition_count,
                                exceeds_limits: budget.exceeds_limits,
                            })?;
                        }
                        Err(e) => {
                            tracing::warn!(
                                target: "ingest",
                                file = %file_str,
                                "dry-run: could not read file for budget assessment: {e}"
                            );
                        }
                    }
                }
            }
        }
        output::emit_json_compact(&IngestSummary {
            summary: true,
            dir: args.dir.to_string_lossy().into_owned(),
            pattern: args.pattern.clone(),
            recursive: args.recursive,
            files_total: total,
            files_succeeded: 0,
            files_failed: 0,
            files_skipped: 0,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })?;
        return Ok(());
    }

    // Reject contradictory flag combination: explicit parallelism > 1 with --low-memory.
    if args.low_memory {
        if let Some(n) = args.ingest_parallelism {
            if n > 1 {
                return Err(AppError::Validation(
                    "--ingest-parallelism N>1 conflicts with --low-memory; use one or the other"
                        .to_string(),
                ));
            }
        }
    }

    // Determine rayon thread pool size, honoring --low-memory and the XDG
    // setting `ingest.low_memory` (both force parallelism = 1).
    let parallelism = resolve_parallelism(args.low_memory, args.ingest_parallelism);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(parallelism)
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rayon pool: {e}")))?;

    if args.enable_ner && args.skip_extraction {
        return Err(AppError::Validation(
            crate::i18n::validation::enable_ner_skip_extraction_exclusive(),
        ));
    }
    if args.skip_extraction && !args.enable_ner {
        // v1.0.74: revert to v1.0.45 hidden no-op behavior. The v1.0.67
        // commit (9ddb17b) promoted this to a hard validation error, which
        // broke the "kept as a hidden no-op for backwards compatibility"
        // promise documented in CHANGELOG v1.0.45 and started failing
        // 5+ CI jobs whose E2E tests use this flag to skip the
        // (since-removed) GLiNER-ONNX model download in CI environments.
        tracing::warn!(
            "--skip-extraction is deprecated since v1.0.45 and has no effect (NER is disabled by default); remove this flag to silence the warning"
        );
    }
    let enable_ner = args.enable_ner;
    let auto_describe = args.auto_describe && !args.no_auto_describe;
    let max_rss_mb = args.max_rss_mb;
    let llm_parallelism = args.llm_parallelism as usize;

    let total_to_process = process_items.len();
    tracing::info!(
        target: "ingest",
        phase = "pipeline_start",
        files = total_to_process,
        ingest_parallelism = parallelism,
        "incremental pipeline starting: Phase A (rayon) → channel → Phase B (main thread)",
    );

    // Bounded channel: producer never gets more than parallelism*2 items ahead of
    // the consumer, preventing memory blowup when Phase A is faster than Phase B.
    // Each message carries the slot index so Phase B can look up SlotMeta in order.
    let channel_bound = (parallelism * 2).max(1);
    let (tx, rx) = mpsc::sync_channel::<(usize, Result<Vec<StagedFile>, AppError>)>(channel_bound);

    // Phase A: launched in a dedicated OS thread so the main thread can consume
    // the channel concurrently. pool.install() blocks the calling thread until
    // all rayon workers finish — if called on the main thread it would
    // reintroduce the 2-phase blocking behaviour we are eliminating.
    let paths_owned = paths.clone();
    let llm_backend_owned = llm_backend;
    let embedding_backend_owned = embedding_backend;
    let producer_handle = std::thread::spawn(move || {
        pool.install(|| {
            process_items.into_par_iter().for_each(|item| {
                if crate::shutdown_requested() {
                    return;
                }
                let t0 = std::time::Instant::now();
                let result = stage_file(
                    item.idx,
                    &item.path,
                    &item.derived_name,
                    &paths_owned,
                    enable_ner,
                    max_rss_mb,
                    llm_parallelism,
                    llm_backend_owned,
                    embedding_backend_owned,
                    auto_describe,
                );
                let elapsed_ms = t0.elapsed().as_millis() as u64;

                // Emit NDJSON progress event to stderr so the user sees work
                // happening during long NER runs (e.g. 50 files × 27s each).
                let (n_entities, n_relationships) = match &result {
                    Ok(parts) => (
                        parts.iter().map(|sf| sf.entities.len()).sum::<usize>(),
                        parts.iter().map(|sf| sf.relationships.len()).sum::<usize>(),
                    ),
                    Err(_) => (0, 0),
                };
                let progress = StageProgressEvent {
                    schema_version: 1,
                    event: "file_extracted",
                    path: &item.file_str,
                    ms: elapsed_ms,
                    entities: n_entities,
                    relationships: n_relationships,
                };
                if let Ok(line) = serde_json::to_string(&progress) {
                    tracing::info!(target: "ingest_progress", "{}", line);
                }

                // Blocking send applies backpressure: if Phase B is slower,
                // Phase A workers wait here instead of accumulating staged files
                // in memory. If the receiver is dropped (fail_fast abort), ignore.
                let _ = tx.send((item.idx, result));
            });
            // Explicit drop of tx signals Phase B (rx iteration) to stop.
            drop(tx);
        });
    });

    // Phase B: main thread persists files as results arrive from the channel.
    // Results arrive in completion order (par_iter is unordered). We persist
    // each file immediately on arrival — this is the key fix for B1: with the
    // old 2-phase design the first DB write happened only after ALL files had
    // finished Phase A. Now the first commit happens as soon as the first file
    // completes Phase A, regardless of how many files remain.
    //
    // NDJSON output order follows completion order (not file-system sort order).
    // Skip slots are emitted at the end, after all Process results are consumed.
    // This trade-off is intentional: deterministic NDJSON ordering is a lesser
    // requirement than ensuring data is persisted before the user's timeout fires.
    let fail_fast = args.fail_fast;

    // Emit pending Skip events first so agents see them early.
    for meta in &slots_meta {
        if let SlotMeta::Skip {
            file_str,
            derived_base,
            name_truncated,
            original_name,
            original_filename,
            reason,
        } = meta
        {
            output::emit_json_compact(&IngestFileEvent {
                file: file_str,
                name: derived_base,
                status: "skipped",
                truncated: *name_truncated,
                original_name: original_name.clone(),
                original_filename: original_filename.as_deref(),
                error: Some(reason.clone()),
                memory_id: None,
                action: None,
                body_length: 0,
                backend_invoked: None,
            })?;
            skipped += 1;
        }
    }

    // Build a quick index from slot index → SlotMeta reference for O(1) lookups
    // as channel messages arrive in completion order.
    let meta_index: std::collections::HashMap<usize, &SlotMeta> = slots_meta
        .iter()
        .enumerate()
        .filter(|(_, m)| matches!(m, SlotMeta::Process { .. }))
        .collect();

    tracing::info!(
        target: "ingest",
        phase = "persist_start",
        files = total_to_process,
        "phase B starting: persisting files incrementally as Phase A completes each one",
    );

    // Drain channel and persist each file immediately — no accumulation into a
    // HashMap. The bounded channel ensures Phase A cannot run too far ahead of
    // Phase B without applying backpressure.
    for (idx, stage_result) in rx {
        if crate::shutdown_requested() {
            tracing::info!(target: "ingest", "shutdown requested, stopping persistence loop");
            break;
        }
        let meta = meta_index.get(&idx).ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "channel idx {idx} has no corresponding Process slot"
            ))
        })?;
        let (file_str, derived_name, name_truncated, original_name, original_filename) = match meta
        {
            SlotMeta::Process {
                file_str,
                derived_name,
                name_truncated,
                original_name,
                original_filename,
            } => (
                file_str,
                derived_name,
                name_truncated,
                original_name,
                original_filename,
            ),
            SlotMeta::Skip { .. } => unreachable!("channel only carries Process results"),
        };

        // If storage init failed, every file fails with the same error.
        let conn = match conn_or_err.as_mut() {
            Ok(c) => c,
            Err(err_msg) => {
                let err_clone = err_msg.clone();
                output::emit_json_compact(&IngestFileEvent {
                    file: file_str,
                    name: derived_name,
                    status: "failed",
                    truncated: *name_truncated,
                    original_name: original_name.clone(),
                    original_filename: original_filename.as_deref(),
                    error: Some(err_clone.clone()),
                    memory_id: None,
                    action: None,
                    body_length: 0,
                    backend_invoked: None,
                })?;
                failed += 1;
                if fail_fast {
                    output::emit_json_compact(&IngestSummary {
                        summary: true,
                        dir: args.dir.display().to_string(),
                        pattern: args.pattern.clone(),
                        recursive: args.recursive,
                        files_total: total,
                        files_succeeded: succeeded,
                        files_failed: failed,
                        files_skipped: skipped,
                        elapsed_ms: started.elapsed().as_millis() as u64,
                    })?;
                    return Err(AppError::Validation(
                        crate::i18n::validation::ingest_aborted_on_first_failure(&err_clone),
                    ));
                }
                continue;
            }
        };

        match stage_result {
            Ok(parts) => {
                // GAP-SG-04/07: one source file can stage as multiple
                // sub-memories (auto-split partitions); persist and report each.
                for staged in parts {
                    let part_name = staged.name.clone();
                    match persist_staged(
                        conn,
                        &namespace,
                        &memory_type_str,
                        staged,
                        args.force_merge,
                    ) {
                        Ok(FileSuccess {
                            memory_id,
                            action,
                            body_length,
                            backend_invoked: file_backend_invoked,
                        }) => {
                            output::emit_json_compact(&IngestFileEvent {
                                file: file_str,
                                name: &part_name,
                                status: "indexed",
                                truncated: *name_truncated,
                                original_name: original_name.clone(),
                                original_filename: original_filename.as_deref(),
                                error: None,
                                memory_id: Some(memory_id),
                                action: Some(action),
                                body_length,
                                backend_invoked: file_backend_invoked,
                            })?;
                            succeeded += 1;
                        }
                        Err(ref e) if matches!(e, AppError::Duplicate(_)) => {
                            output::emit_json_compact(&IngestFileEvent {
                                file: file_str,
                                name: &part_name,
                                status: "skipped",
                                truncated: *name_truncated,
                                original_name: original_name.clone(),
                                original_filename: original_filename.as_deref(),
                                error: Some(format!("{e}")),
                                memory_id: None,
                                action: Some("duplicate".to_string()),
                                body_length: 0,
                                backend_invoked: None,
                            })?;
                            skipped += 1;
                        }
                        Err(e) => {
                            let err_msg = format!("{e}");
                            output::emit_json_compact(&IngestFileEvent {
                                file: file_str,
                                name: &part_name,
                                status: "failed",
                                truncated: *name_truncated,
                                original_name: original_name.clone(),
                                original_filename: original_filename.as_deref(),
                                error: Some(err_msg.clone()),
                                memory_id: None,
                                action: None,
                                body_length: 0,
                                backend_invoked: None,
                            })?;
                            failed += 1;
                            if fail_fast {
                                output::emit_json_compact(&IngestSummary {
                                    summary: true,
                                    dir: args.dir.display().to_string(),
                                    pattern: args.pattern.clone(),
                                    recursive: args.recursive,
                                    files_total: total,
                                    files_succeeded: succeeded,
                                    files_failed: failed,
                                    files_skipped: skipped,
                                    elapsed_ms: started.elapsed().as_millis() as u64,
                                })?;
                                return Err(AppError::Validation(
                                    crate::i18n::validation::ingest_aborted_on_first_failure(
                                        &err_msg,
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let err_msg = format!("{e}");
                output::emit_json_compact(&IngestFileEvent {
                    file: file_str,
                    name: derived_name,
                    status: "failed",
                    truncated: *name_truncated,
                    original_name: original_name.clone(),
                    original_filename: original_filename.as_deref(),
                    error: Some(err_msg.clone()),
                    memory_id: None,
                    action: None,
                    body_length: 0,
                    backend_invoked: None,
                })?;
                failed += 1;
                if fail_fast {
                    output::emit_json_compact(&IngestSummary {
                        summary: true,
                        dir: args.dir.display().to_string(),
                        pattern: args.pattern.clone(),
                        recursive: args.recursive,
                        files_total: total,
                        files_succeeded: succeeded,
                        files_failed: failed,
                        files_skipped: skipped,
                        elapsed_ms: started.elapsed().as_millis() as u64,
                    })?;
                    return Err(AppError::Validation(
                        crate::i18n::validation::ingest_aborted_on_first_failure(&err_msg),
                    ));
                }
            }
        }
    }

    // Wait for the producer thread to finish cleanly.
    producer_handle
        .join()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("ingest producer thread panicked")))?;

    if let Ok(ref conn) = conn_or_err {
        if succeeded > 0 {
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
    }

    output::emit_json_compact(&IngestSummary {
        summary: true,
        dir: args.dir.display().to_string(),
        pattern: args.pattern.clone(),
        recursive: args.recursive,
        files_total: total,
        files_succeeded: succeeded,
        files_failed: failed,
        files_skipped: skipped,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })?;

    if args.enrich_after && succeeded > 0 {
        output::emit_json_compact(&serde_json::json!({
            "event": "enrich_phase_started",
            "operation": "memory-bindings"
        }))?;
        let enrich_args = crate::commands::enrich::EnrichArgs {
            operation: Some(crate::commands::enrich::EnrichOperation::MemoryBindings),
            mode: Some(crate::commands::enrich::EnrichMode::ClaudeCode),
            limit: None,
            target: crate::commands::enrich::ReEmbedTarget::Memories,
            dry_run: false,
            namespace: args.namespace.clone(),
            claude_binary: args.claude_binary.clone(),
            claude_model: args.claude_model.clone(),
            claude_timeout: args.claude_timeout,
            codex_binary: args.codex_binary.clone(),
            codex_model: args.codex_model.clone(),
            codex_timeout: args.codex_timeout,
            opencode_binary: args.opencode_binary.clone(),
            opencode_model: args.opencode_model.clone(),
            opencode_timeout: args.opencode_timeout,
            openrouter_model: None,
            openrouter_api_key: None,
            openrouter_timeout: 300,
            openrouter_base_url: None,
            db: args.db.clone(),
            json: false,
            resume: false,
            retry_failed: false,
            reset_stale_claims: false,
            stale_claim_secs: 1800,
            max_cost_usd: args.max_cost_usd,
            llm_parallelism: args.llm_parallelism as u32,
            wait_job_singleton: args.wait_job_singleton,
            force_job_singleton: args.force_job_singleton,
            names: Vec::new(),
            names_file: None,
            preflight_check: false,
            fallback_mode: None,
            rate_limit_buffer: 300,
            max_load_check: true,
            circuit_breaker_threshold: 5,
            preserve_threshold: 0.7,
            entity_description_grounding_threshold: 0.12,
            force_redescribe: false,
            quality_sample: None,
            entity_names: Vec::new(),
            memory_names: Vec::new(),
            anchor_memory: None,
            entity_description_domain: "auto".to_string(),
            yield_every_n_items: None,
            ops_gate: false,
            codex_model_validate: true,
            codex_model_fallback: None,
            min_output_chars: 500,
            max_output_chars: 2000,
            preserve_check: true,
            prompt_template: None,
            until_empty: false,
            max_runtime: None,
            max_attempts: 5,
            status: false,
            rest_concurrency: None,
            // enrich-after runs a plain memory-bindings pass; dead-letter,
            // backoff-ignore and graph-only flags stay at their defaults.
            list_dead: false,
            requeue_dead: false,
            list_skipped: false,
            requeue_skipped: false,
            prune_dead_orphans: false,
            prune_dead_entity_orphans: false,
            ignore_backoff: false,
            body_extract_graph_only: false,
            print_schema: false,
        };
        match crate::commands::enrich::run(&enrich_args, llm_backend, embedding_backend) {
            Ok(()) => {
                output::emit_json_compact(&serde_json::json!({
                    "event": "enrich_phase_completed"
                }))?;
            }
            Err(e) => {
                tracing::warn!(error = %e, "enrich --operation memory-bindings failed after ingest");
                output::emit_json_compact(&serde_json::json!({
                    "event": "enrich_phase_failed",
                    "error": e.to_string()
                }))?;
            }
        }
    }

    Ok(())
}
