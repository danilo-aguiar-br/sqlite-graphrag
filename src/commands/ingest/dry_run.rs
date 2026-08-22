//! `--dry-run` preview: report what an ingest would do, touch nothing.
//!
//! Runs after the name plan is resolved and before the database is opened or
//! any model is loaded, so the operator can inspect name derivation, skipped
//! files and chunk/token budgets at zero cost.

use super::args::IngestArgs;
use super::plan::SlotMeta;
use super::report::{IngestDryRunBudget, IngestFileEvent, IngestSummary};
use crate::chunking;
use crate::errors::AppError;
use crate::output;

/// Emits one NDJSON event per slot, then the run summary.
///
/// Processable slots additionally get a budget line (GAP-SG-06) reporting
/// chunk count, token count and how many sub-memories an auto-split would
/// create, so chunk or token overflow surfaces before a real ingest rather
/// than halfway through one.
///
/// # Errors
/// Propagates serialization and stdout failures from [`crate::output`].
pub(super) fn emit_preview(
    args: &IngestArgs,
    slots_meta: &[SlotMeta],
    total: usize,
    started: std::time::Instant,
) -> Result<(), AppError> {
    for meta in slots_meta {
        match meta {
            SlotMeta::Skip {
                file_str,
                derived_base,
                name_truncated,
                original_name,
                original_filename,
                reason,
            } => {
                output::emit_stream_record(&IngestFileEvent {
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
                    ..Default::default()
                })?;
            }
            SlotMeta::Process {
                file_str,
                derived_name,
                name_truncated,
                original_name,
                original_filename,
            } => {
                output::emit_stream_record(&IngestFileEvent {
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
                    ..Default::default()
                })?;
                emit_budget(file_str, derived_name)?;
            }
        }
    }

    // GAP-SG-215: the trailer, not a record. It carries the one `agent_surface`
    // block for the whole preview.
    output::emit_stream_trailer(&IngestSummary {
        summary: true,
        dir: args.dir.to_string_lossy().into_owned(),
        pattern: args.pattern.clone(),
        recursive: args.recursive,
        files_total: total,
        files_succeeded: 0,
        files_failed: 0,
        files_skipped: 0,
        elapsed_ms: started.elapsed().as_millis() as u64,
        ..Default::default()
    })
}

/// Reports the chunk/token budget of one file.
///
/// An unreadable file is a warning, not an error: the preview should still
/// describe every other file rather than abort on one bad path.
fn emit_budget(file_str: &str, derived_name: &str) -> Result<(), AppError> {
    match std::fs::read_to_string(file_str) {
        Ok(body) => {
            let budget = chunking::assess_body_budget(&body);
            output::emit_stream_record(&IngestDryRunBudget {
                budget: true,
                file: file_str,
                name: derived_name,
                bytes: budget.bytes,
                chunk_count: budget.chunk_count,
                token_count: budget.approx_tokens,
                partition_count: budget.partition_count,
                exceeds_limits: budget.exceeds_limits,
            })
        }
        Err(e) => {
            tracing::warn!(
                target: "ingest",
                file = %file_str,
                "dry-run: could not read file for budget assessment: {e}"
            );
            Ok(())
        }
    }
}
