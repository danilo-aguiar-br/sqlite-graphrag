//! Orchestration entry point for the `ingest` command.
//!
//! This module decides the order of the pipeline and owns nothing else. Each
//! stage lives in its own sibling module:
//!
//! 1. [`super::validate`] — mode-conditional flag checks, before any I/O
//! 2. [`super::scan_fs`] — walk the directory and match the glob
//! 3. [`super::plan`] — resolve every memory name, single-threaded
//! 4. [`super::dry_run`] — preview and stop, when `--dry-run` is set
//! 5. [`super::stage_producer`] — Phase A: read, chunk, embed, extract
//! 6. [`super::persist_loop`] — Phase B: write and report, as results arrive
//! 7. [`super::enrich_after`] — optional post-ingest binding pass

use super::args::IngestArgs;
use super::persist::init_storage;
use super::persist_loop::{self, PersistContext};
use super::plan::build_plan;
use super::scan_fs::collect_files;
use super::validate::validate_mode_conditional_flags_ingest;
use super::{dry_run, enrich_after, stage_producer};
use crate::errors::AppError;
use crate::paths::AppPaths;
use std::path::PathBuf;

/// Run the `ingest` command (filesystem scan + stage + persist, or mode adapters).
pub fn run(args: IngestArgs, backends: crate::cli::BackendChoice) -> Result<(), AppError> {
    // G20: mode-conditional flag validation BEFORE any DB access.
    // Surfaces flags that the wrong mode would silently discard.
    validate_mode_conditional_flags_ingest(&args)?;
    // GAP-SG-215: `ingest` emits one record per file and a summary, so it runs
    // under the stream contract — records unannotated, the `agent_surface` block
    // once on the summary. The sample is empty on purpose: `ingest` mutates, so
    // the gate's write fence returns before any question about field names is
    // asked. Refusing here would report failure for files already persisted.
    crate::agent_surface::stream::open(crate::agent_surface::get(), &[], 0)?;
    tracing::debug!(target: "ingest", dir = %args.dir.display(), mode = ?args.mode, "starting ingest");
    let started = std::time::Instant::now();
    let files = scan(&args)?;
    let total = files.len();

    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let memory_type_str = args.r#type.as_str().to_string();

    let paths = AppPaths::resolve(args.db.as_deref())?;
    // Storage failure is carried rather than raised: every file must still be
    // reported as failed, with the same cause, instead of the run dying with
    // no per-file record of what was lost.
    let mut conn_or_err = init_storage(&paths).map_err(|e| format!("{e}"));

    let plan = build_plan(&args, &files)?;

    // --dry-run: preview and exit before loading any model or touching the DB.
    if args.dry_run {
        return dry_run::emit_preview(&args, &plan.slots_meta, total, started);
    }

    let parallelism = stage_producer::resolve_worker_count(&args)?;
    stage_producer::validate_extraction_flags(&args)?;

    let total_to_process = plan.process_items.len();
    tracing::info!(
        target: "ingest",
        phase = "pipeline_start",
        files = total_to_process,
        ingest_parallelism = parallelism,
        "incremental pipeline starting: Phase A (rayon) → channel → Phase B (main thread)",
    );

    let producer = stage_producer::spawn(&args, plan.process_items, &paths, parallelism, backends)?;

    let ctx = PersistContext {
        args: &args,
        namespace: &namespace,
        memory_type: &memory_type_str,
        total,
        started,
    };
    let tally = persist_loop::drain_and_persist(
        &ctx,
        &plan.slots_meta,
        producer.results,
        &mut conn_or_err,
    )?;

    producer
        .handle
        .join()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("ingest producer thread panicked")))?;

    if let Ok(ref conn) = conn_or_err {
        if tally.succeeded > 0 {
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
    }

    persist_loop::emit_summary(&ctx, tally)?;

    if args.enrich_after && tally.succeeded > 0 {
        enrich_after::run(&args, backends)?;
    }

    Ok(())
}

/// Validates the target directory and returns the matched files, sorted.
///
/// # Errors
/// Returns [`AppError::Validation`] when the directory is missing, is not a
/// directory, or the match count exceeds `--max-files`.
fn scan(args: &IngestArgs) -> Result<Vec<PathBuf>, AppError> {
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
    Ok(files)
}
