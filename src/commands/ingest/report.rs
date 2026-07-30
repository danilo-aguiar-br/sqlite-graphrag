//! NDJSON report types for the `ingest` CLI subcommand.
//!
//! Per-file events, dry-run budget previews, stage-progress frames and the
//! final summary object are serialised to stdout/stderr as line-delimited JSON
//! for streaming consumption by agents.

use serde::Serialize;

/// Per-file NDJSON event emitted after each ingest attempt (success, skip, or error).
#[derive(Serialize)]
pub(crate) struct IngestFileEvent<'a> {
    pub file: &'a str,
    pub name: &'a str,
    pub status: &'a str,
    /// True when the derived name was truncated to fit `DERIVED_NAME_MAX_LEN`. False otherwise.
    pub truncated: bool,
    /// Original derived name before truncation; only present when `truncated=true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_name: Option<String>,
    /// Original file basename (without extension); only present when it differs from `name`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// Byte length of the body ingested; 0 when not yet read (e.g. skip or dry-run events).
    pub body_length: usize,
    /// v1.0.84 (ADR-0042): discriminator of the LLM backend that actually
    /// ran the live embedding. `"claude" | "codex" | "none"`. Absent on
    /// the wire when `None` (kept for happy-path envelope cleanliness, or
    /// when the file never reached the embed phase due to duplication/error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_invoked: Option<&'a str>,
}

/// GAP-SG-06: per-file budget assessment emitted during `--dry-run` so the
/// operator sees chunk and token counts (and how many sub-memories an
/// auto-split would create) before running a real ingest.
#[derive(Serialize)]
pub(crate) struct IngestDryRunBudget<'a> {
    pub budget: bool,
    pub file: &'a str,
    pub name: &'a str,
    pub bytes: usize,
    pub chunk_count: usize,
    pub token_count: usize,
    pub partition_count: usize,
    pub exceeds_limits: bool,
}

/// Final summary line after all files have been processed.
#[derive(Serialize)]
pub(crate) struct IngestSummary {
    pub summary: bool,
    pub dir: String,
    pub pattern: String,
    pub recursive: bool,
    pub files_total: usize,
    pub files_succeeded: usize,
    pub files_failed: usize,
    pub files_skipped: usize,
    pub elapsed_ms: u64,
}

/// Outcome of a successful per-file ingest, used to build the NDJSON event.
#[derive(Debug)]
pub(crate) struct FileSuccess {
    pub memory_id: i64,
    pub action: String,
    pub body_length: usize,
    pub backend_invoked: Option<&'static str>,
}

/// NDJSON progress event emitted to stderr after each file completes Phase A.
/// Schema version 1; consumers should check `schema_version` before parsing.
#[derive(Serialize)]
pub(crate) struct StageProgressEvent<'a> {
    pub schema_version: u8,
    pub event: &'a str,
    pub path: &'a str,
    pub ms: u64,
    pub entities: usize,
    pub relationships: usize,
}
