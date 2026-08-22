//! Phase B of the ingest pipeline: persist each file the moment it is staged.
//!
//! Runs on the main thread because `rusqlite::Connection` is not `Sync` and
//! must never cross a thread boundary. Each message is committed on arrival
//! rather than accumulated, so the first row reaches disk seconds after the
//! first file finishes Phase A instead of after the whole corpus does.
//!
//! ## Output ordering
//!
//! NDJSON follows completion order, not filesystem order, because `par_iter`
//! is unordered. Skipped slots are emitted up front, before the drain starts,
//! so an agent watching the stream learns about them immediately. Trading
//! deterministic ordering for early persistence is deliberate: an operator
//! whose timeout fires mid-run keeps the data already written.

use super::args::IngestArgs;
use super::persist::persist_staged;
use super::plan::SlotMeta;
use super::report::{FileSuccess, IngestFileEvent, IngestSummary};
use super::stage_producer::StageMessage;
use crate::errors::AppError;
use crate::output;
use rusqlite::Connection;
use std::sync::mpsc;

/// Running counts for the run summary.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct IngestTally {
    /// Memories written or merged.
    pub(super) succeeded: usize,
    /// Files that errored during staging or persistence.
    pub(super) failed: usize,
    /// Files dropped before staging, plus duplicates rejected on write.
    pub(super) skipped: usize,
}

/// Everything the drain needs that does not change between messages.
pub(super) struct PersistContext<'a> {
    /// The invocation's arguments, for `--force-merge`, `--fail-fast` and the summary.
    pub(super) args: &'a IngestArgs,
    /// Resolved namespace for every memory written by this run.
    pub(super) namespace: &'a str,
    /// Resolved memory type for every memory written by this run.
    pub(super) memory_type: &'a str,
    /// Number of files matched by the scan, reported in the summary.
    pub(super) total: usize,
    /// Start of the run, for `elapsed_ms`.
    pub(super) started: std::time::Instant,
}

/// The reporting fields shared by every event emitted for one slot.
struct SlotEvent<'a> {
    /// Source path.
    file: &'a str,
    /// Memory name.
    name: &'a str,
    /// Whether the derived name hit the length budget.
    truncated: bool,
    /// Pre-truncation name, when truncation happened.
    original_name: Option<String>,
    /// Raw file stem, when it differs from the derived name.
    original_filename: Option<&'a str>,
}

/// Drains `results`, persisting and reporting each staged file as it arrives.
///
/// Returns the tally even when nothing was written, so the caller can emit a
/// summary unconditionally.
///
/// # Errors
/// Returns [`AppError::Validation`] when `--fail-fast` is set and a file
/// fails, after emitting the partial summary. Returns [`AppError::Internal`]
/// when a message carries an index with no matching [`SlotMeta::Process`]
/// slot, which would mean the plan and the producer disagree.
pub(super) fn drain_and_persist(
    ctx: &PersistContext<'_>,
    slots_meta: &[SlotMeta],
    results: mpsc::Receiver<StageMessage>,
    conn_or_err: &mut Result<Connection, String>,
) -> Result<IngestTally, AppError> {
    let mut tally = IngestTally::default();

    // Emit pending Skip events first so agents see them early.
    for meta in slots_meta {
        if let SlotMeta::Skip {
            file_str,
            derived_base,
            name_truncated,
            original_name,
            original_filename,
            reason,
        } = meta
        {
            emit_event(
                &SlotEvent {
                    file: file_str,
                    name: derived_base,
                    truncated: *name_truncated,
                    original_name: original_name.clone(),
                    original_filename: original_filename.as_deref(),
                },
                "skipped",
                Some(reason.clone()),
                None,
                None,
                0,
            )?;
            tally.skipped += 1;
        }
    }

    // Slot index → metadata, for O(1) lookups as messages arrive out of order.
    let meta_index: std::collections::HashMap<usize, &SlotMeta> = slots_meta
        .iter()
        .enumerate()
        .filter(|(_, m)| matches!(m, SlotMeta::Process { .. }))
        .collect();

    tracing::info!(
        target: "ingest",
        phase = "persist_start",
        files = meta_index.len(),
        "phase B starting: persisting files incrementally as Phase A completes each one",
    );

    for (idx, stage_result) in results {
        if crate::shutdown_requested() {
            tracing::info!(target: "ingest", "shutdown requested, stopping persistence loop");
            break;
        }
        let meta = meta_index.get(&idx).ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "channel idx {idx} has no corresponding Process slot"
            ))
        })?;
        let SlotMeta::Process {
            file_str,
            derived_name,
            name_truncated,
            original_name,
            original_filename,
        } = meta
        else {
            unreachable!("channel only carries Process results")
        };
        let slot = SlotEvent {
            file: file_str,
            name: derived_name,
            truncated: *name_truncated,
            original_name: original_name.clone(),
            original_filename: original_filename.as_deref(),
        };

        // If storage init failed, every file fails with the same error.
        let conn = match conn_or_err.as_mut() {
            Ok(c) => c,
            Err(err_msg) => {
                let err_msg = err_msg.clone();
                report_failure(ctx, &mut tally, &slot, err_msg)?;
                continue;
            }
        };

        match stage_result {
            Ok(parts) => {
                // GAP-SG-04/07: one source file can stage as multiple
                // sub-memories (auto-split partitions); persist and report each.
                for staged in parts {
                    let part_name = staged.name.clone();
                    let part = SlotEvent {
                        file: slot.file,
                        name: &part_name,
                        truncated: slot.truncated,
                        original_name: slot.original_name.clone(),
                        original_filename: slot.original_filename,
                    };
                    persist_one(ctx, &mut tally, conn, &part, staged)?;
                }
            }
            Err(e) => report_failure(ctx, &mut tally, &slot, format!("{e}"))?,
        }
    }

    Ok(tally)
}

/// Writes one staged part and reports the outcome.
fn persist_one(
    ctx: &PersistContext<'_>,
    tally: &mut IngestTally,
    conn: &mut Connection,
    slot: &SlotEvent<'_>,
    staged: super::stage::StagedFile,
) -> Result<(), AppError> {
    match persist_staged(
        conn,
        ctx.namespace,
        ctx.memory_type,
        staged,
        ctx.args.force_merge,
    ) {
        Ok(FileSuccess {
            memory_id,
            action,
            body_length,
            backend_invoked,
        }) => {
            output::emit_stream_record(&IngestFileEvent {
                file: slot.file,
                name: slot.name,
                status: "indexed",
                truncated: slot.truncated,
                original_name: slot.original_name.clone(),
                original_filename: slot.original_filename,
                error: None,
                memory_id: Some(memory_id),
                action: Some(action),
                body_length,
                // The only event that carries a backend: the write is what
                // resolved which embedding backend actually ran.
                backend_invoked,
                ..Default::default()
            })?;
            tally.succeeded += 1;
            Ok(())
        }
        // A duplicate is an expected outcome of re-running an ingest, not a
        // failure: it is reported as skipped and never trips --fail-fast.
        Err(ref e) if matches!(e, AppError::Duplicate(_)) => {
            emit_event(
                slot,
                "skipped",
                Some(format!("{e}")),
                None,
                Some("duplicate".to_string()),
                0,
            )?;
            tally.skipped += 1;
            Ok(())
        }
        Err(e) => report_failure(ctx, tally, slot, format!("{e}")),
    }
}

/// Reports a failed file and honours `--fail-fast`.
///
/// # Errors
/// Under `--fail-fast`, emits the partial summary and returns
/// [`AppError::Validation`] so the run aborts with the cause attached.
fn report_failure(
    ctx: &PersistContext<'_>,
    tally: &mut IngestTally,
    slot: &SlotEvent<'_>,
    error: String,
) -> Result<(), AppError> {
    emit_event(slot, "failed", Some(error.clone()), None, None, 0)?;
    tally.failed += 1;
    if ctx.args.fail_fast {
        emit_summary(ctx, *tally)?;
        return Err(AppError::Validation(
            crate::i18n::validation::ingest_aborted_on_first_failure(&error),
        ));
    }
    Ok(())
}

/// Emits one NDJSON file event.
fn emit_event(
    slot: &SlotEvent<'_>,
    status: &'static str,
    error: Option<String>,
    memory_id: Option<i64>,
    action: Option<String>,
    body_length: usize,
) -> Result<(), AppError> {
    output::emit_stream_record(&IngestFileEvent {
        file: slot.file,
        name: slot.name,
        status,
        truncated: slot.truncated,
        original_name: slot.original_name.clone(),
        original_filename: slot.original_filename,
        error,
        memory_id,
        action,
        body_length,
        backend_invoked: None,
        ..Default::default()
    })
}

/// Emits the run summary. Shared by the `--fail-fast` abort and the clean end.
pub(super) fn emit_summary(ctx: &PersistContext<'_>, tally: IngestTally) -> Result<(), AppError> {
    // GAP-SG-215: the trailer, not a record. It carries the one `agent_surface`
    // block for the whole run.
    output::emit_stream_trailer(&IngestSummary {
        summary: true,
        dir: ctx.args.dir.display().to_string(),
        pattern: ctx.args.pattern.clone(),
        recursive: ctx.args.recursive,
        files_total: ctx.total,
        files_succeeded: tally.succeeded,
        files_failed: tally.failed,
        files_skipped: tally.skipped,
        elapsed_ms: ctx.started.elapsed().as_millis() as u64,
        ..Default::default()
    })
}
