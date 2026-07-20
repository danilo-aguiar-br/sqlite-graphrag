//! LLM invocation — per-operation call helpers and result types.

use super::*;
use super::prompts;
use super::scan;
use super::queue;
/* wave-c1-imports */
use crate::errors::AppError;
use std::path::Path;


// ---------------------------------------------------------------------------
// LLM invocation — Claude Code
// ---------------------------------------------------------------------------

/// Calls `claude -p` via the shared `claude_runner` module (G02).
///
/// Returns `(output_value, cost_usd, is_oauth)`.
pub(crate) fn call_claude(
    binary: &Path,
    prompt: &str,
    json_schema: &str,
    input_text: &str,
    model: Option<&str>,
    timeout_secs: u64,
) -> Result<(serde_json::Value, f64, bool), AppError> {
    let result = crate::commands::claude_runner::run_claude(
        binary,
        prompt,
        json_schema,
        input_text,
        model,
        timeout_secs,
        7,
    )?;
    Ok((result.value, result.cost_usd, result.is_oauth))
}

/// GAP-SG-72/73 (v1.1.00): per-item failure diagnostics captured from a
/// [`crate::chat_api::ChatError`] returned by [`call_openrouter`]. The
/// `retry_class` is computed AT THE ORIGIN by `chat_api.rs` (the exact HTTP
/// status / provider code), never inferred downstream by matching the
/// formatted error string. `finish_reason` and the token counts are the raw
/// truncation diagnostics OpenRouter attached to the failing response, when
/// one was decoded.
pub(crate) struct OpenRouterFailureDiagnostics {
    pub(crate) retry_class: crate::retry::AttemptOutcome,
    pub(crate) finish_reason: Option<String>,
    pub(crate) prompt_tokens: Option<i64>,
    pub(crate) completion_tokens: Option<i64>,
}

// GAP-SG-72/73: `call_openrouter` returns the same `(Value, f64, bool)` tuple
// shape as the subprocess providers (`call_claude`/`call_codex`/
// `call_opencode`) so every `call_*` helper above can keep matching on
// `mode` uniformly. That tuple has no room for `ChatError`'s typed
// `retry_class` / truncation diagnostics, so they are stashed here on
// failure and drained by the caller in `mod.rs` right after every
// `call_result` (mirrors the `ENRICH_LAST_BACKEND` accumulator in
// `postprocess.rs`). `thread_local` — NOT a process-wide `Mutex` — because
// the parallel worker loop runs one item per OS thread at a time: a
// process-wide slot would let a diagnostic from one worker's item leak into
// another worker's unrelated failure.
thread_local! {
    static LAST_OPENROUTER_FAILURE: std::cell::RefCell<Option<OpenRouterFailureDiagnostics>> =
        const { std::cell::RefCell::new(None) };
}

/// Drains the diagnostics stashed by the most recent [`call_openrouter`]
/// failure on THIS thread. Callers must invoke this unconditionally right
/// after every `call_result` (success or failure) so a diagnostic never
/// survives past the item that produced it — see the doc comment on
/// [`OpenRouterFailureDiagnostics`].
pub(crate) fn take_last_openrouter_failure() -> Option<OpenRouterFailureDiagnostics> {
    LAST_OPENROUTER_FAILURE.with(|cell| cell.borrow_mut().take())
}

/// v1.0.95 (ADR-0054): route a single JUDGE turn through the OpenRouter
/// chat-completions REST API. Unlike the subprocess runners there is no
/// `binary` argument: the process-wide chat client (initialised in `run()`
/// before scan) is fetched from the singleton and driven synchronously via
/// the shared tokio runtime. Returns `(value, cost_usd, is_oauth=false)`
/// where `cost_usd` is read from the response `usage.cost`.
///
/// v1.1.00 (GAP-SG-70/72/73): `complete` now returns a typed
/// `Result<ChatCompletion, ChatError>` carrying `finish_reason` / token
/// diagnostics and an origin-computed `retry_class`. On success those
/// diagnostics are simply discarded (the item succeeded); on failure they
/// are stashed via [`take_last_openrouter_failure`] so the queue recorder in
/// `mod.rs` can call `record_item_failure_typed` with the precise verdict
/// instead of falling back to the untyped `classify_enrich_outcome` message
/// sniffing.
pub(crate) fn call_openrouter(
    prompt: &str,
    json_schema: &str,
    input_text: &str,
    model: Option<&str>,
    timeout_secs: u64,
) -> Result<(serde_json::Value, f64, bool), AppError> {
    // `model` is bound into the client singleton at init; `timeout_secs` is
    // enforced by the reqwest builder. Both remain in the signature for
    // parity with the subprocess runners.
    let _ = (model, timeout_secs);
    let client = crate::embedder::openrouter_chat_client().ok_or_else(|| {
        AppError::Validation(
            "OpenRouter chat client not initialised before dispatch (internal error)".into(),
        )
    })?;
    let runtime = crate::embedder::shared_runtime()?;
    match runtime.block_on(client.complete(
        prompt,
        input_text,
        json_schema,
        Some(crate::constants::ENRICH_INITIAL_MAX_TOKENS),
    )) {
        Ok(completion) => Ok((completion.value, completion.cost_usd, false)),
        Err(chat_err) => {
            LAST_OPENROUTER_FAILURE.with(|cell| {
                *cell.borrow_mut() = Some(OpenRouterFailureDiagnostics {
                    retry_class: chat_err.retry_class,
                    finish_reason: chat_err.finish_reason.clone(),
                    prompt_tokens: chat_err.prompt_tokens.map(i64::from),
                    completion_tokens: chat_err.completion_tokens.map(i64::from),
                });
            });
            Err(chat_err.source)
        }
    }
}

// ---------------------------------------------------------------------------
// Internal result type for a single item call
// ---------------------------------------------------------------------------

pub(crate) enum EnrichItemResult {
    Done {
        memory_id: Option<i64>,
        entity_id: Option<i64>,
        entities: usize,
        rels: usize,
        chars_before: Option<usize>,
        chars_after: Option<usize>,
        cost: f64,
        is_oauth: bool,
    },
    Skipped {
        reason: String,
    },
    /// G29 Step 4 (v1.0.69): the LLM rewrite diverged from the original
    /// body beyond the configured `--preserve-threshold` and was rejected
    /// before persistence. The trigram-Jaccard score and threshold are
    /// emitted in the NDJSON stream for operator audit.
    PreservationFailed {
        score: f64,
        threshold: f64,
        chars_before: usize,
        chars_after: usize,
    },
}

// ---------------------------------------------------------------------------
// Per-operation call helpers (SCAN + JUDGE + PERSIST in one unit)
// ---------------------------------------------------------------------------

// Wave C1: operation helpers are children of this module so `use super::*` works.
#[path = "extraction_ops_a.rs"]
mod ops_a;
#[path = "extraction_ops_b.rs"]
mod ops_b;
#[path = "extraction_ops_c.rs"]
mod ops_c;
#[path = "extraction_providers.rs"]
mod providers;

pub(super) use ops_a::*;
pub(super) use ops_b::*;
pub(super) use ops_c::*;
pub(super) use providers::*;

#[cfg(test)]
#[path = "extraction_tests.rs"]
mod tests;
