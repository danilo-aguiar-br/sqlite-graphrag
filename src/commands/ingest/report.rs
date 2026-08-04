//! NDJSON report types for the `ingest` CLI subcommand.
//!
//! Per-file events, dry-run budget previews, stage-progress frames and the
//! final summary object are serialised to stdout/stderr as line-delimited JSON
//! for streaming consumption by agents.
//!
//! GAP-SG-150: these types are the SINGLE contract for every `ingest` mode.
//! `--mode none` serialises
//! [`IngestFileEvent`] and [`IngestSummary`], so an agent consuming the
//! subcommand parses ONE shape regardless of which backend ran. Fields a given
//! mode does not produce are `Option` and skipped on the wire, never emitted as
//! a useless `null`.

use serde::Serialize;

/// Per-file NDJSON event emitted after each ingest attempt (success, skip, or error).
#[derive(Serialize, Default)]
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
    /// v1.0.84 (ADR-0042): discriminator of the embedding backend that actually
    /// ran the live embedding. `"openrouter" | "none"`. Absent on
    /// the wire when `None` (kept for happy-path envelope cleanliness, or
    /// when the file never reached the embed phase due to duplication/error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_invoked: Option<&'a str>,
    // GAP-SG-148 item 5: the fields below exist so `--mode claude-code` reports
    // through THIS type instead of a parallel one. They are `Option` and
    // skipped when absent, so a line emitted by the standard pipeline is
    // byte-identical to what it was before the modes were unified.
    /// Entities extracted from the file by the LLM modes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<usize>,
    /// Relationships extracted from the file by the LLM modes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rels: Option<usize>,
    /// LLM cost of this file in USD; absent on an OAuth subscription.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// Wall-clock time spent on this file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    /// Files already resolved before this one, for progress rendering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// Total files in the run, for progress rendering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    // GAP-SG-150: token counters exist only on `--mode codex`, whose CLI is the
    // only backend that reports usage per turn. They are `Option` so a line from
    // the standard pipeline or from `--mode claude-code` stays byte-identical.
    /// Prompt tokens billed for this file; only `--mode codex` reports them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Completion tokens billed for this file; only `--mode codex` reports them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
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
#[derive(Serialize, Default)]
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
    // GAP-SG-148 item 5: LLM-mode totals, absent from the standard pipeline's
    // line because it extracts no graph and pays no per-file LLM cost.
    /// Entities written across the whole run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities_total: Option<usize>,
    /// Relationships written across the whole run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rels_total: Option<usize>,
    /// Cumulative LLM cost of the run in USD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    // GAP-SG-150: run totals of the per-file token counters; `--mode codex` only.
    /// Prompt tokens billed across the whole run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens_total: Option<u64>,
    /// Completion tokens billed across the whole run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens_total: Option<u64>,
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

// GAP-SG-148 item 5, completed: the NDJSON `status` vocabulary lives here, with
// the types that carry it. It used to exist twice — privately in `ingest_claude`
// and again in `ingest_codex` — which is how two pipelines can drift apart while
// each looks internally consistent.
