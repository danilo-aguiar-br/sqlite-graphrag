//! LLM invocation — per-operation call helpers and result types.

use super::postprocess;
use super::prompts;
use super::queue;
use super::scan;
use crate::errors::AppError;

// Re-export for child ops modules that use `use super::*`.
pub(crate) use super::args::EnrichMode;
pub(crate) use super::prompts::ENTITY_DESCRIPTION_PROMPT_PREFIX;
pub(crate) use super::schemas::{
    BINDINGS_PROMPT, BINDINGS_SCHEMA, BODY_ENRICH_PROMPT_PREFIX, BODY_ENRICH_SCHEMA,
    BODY_EXTRACT_PROMPT, BODY_EXTRACT_SCHEMA, DEEP_RESEARCH_SYNTH_PROMPT,
    DEEP_RESEARCH_SYNTH_SCHEMA, DESCRIPTION_ENRICH_PROMPT, DESCRIPTION_ENRICH_SCHEMA,
    DOMAIN_CLASSIFY_PROMPT, DOMAIN_CLASSIFY_SCHEMA, ENTITY_CONNECT_PROMPT, ENTITY_CONNECT_SCHEMA,
    ENTITY_DESCRIPTION_SCHEMA, ENTITY_TYPE_VALIDATE_PROMPT, ENTITY_TYPE_VALIDATE_SCHEMA,
    GRAPH_AUDIT_PROMPT, GRAPH_AUDIT_SCHEMA, RELATION_RECLASSIFY_PROMPT, RELATION_RECLASSIFY_SCHEMA,
    WEIGHT_CALIBRATE_PROMPT, WEIGHT_CALIBRATE_SCHEMA,
};

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

// GAP-SG-72/73: `call_openrouter` returns a `(Value, f64, bool)` tuple.
// That tuple has no room for `ChatError`'s typed
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
/// chat-completions REST API. The process-wide chat client (initialised in `run()`
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
    // enforced by the reqwest builder.
    let _ = (model, timeout_secs);
    let client = crate::embedder::openrouter_chat_client().ok_or_else(|| {
        AppError::Validation(
            "OpenRouter chat client not initialised before dispatch (internal error)".into(),
        )
    })?;
    // GAP-001 (v1.1.04): canonical nested-runtime guard. This was the last
    // `block_on` in the crate without it — calling it from inside a Tokio
    // context panicked with "cannot start a runtime from within a runtime".
    // Reachable today from any async caller of the enrich drain, and from the
    // `#[tokio::test]` harness. Inside an existing runtime, `block_in_place`
    // moves the blocking wait off the worker so the pool stays healthy.
    let completion = client.complete(
        prompt,
        input_text,
        json_schema,
        Some(crate::constants::ENRICH_INITIAL_MAX_TOKENS),
    );
    let outcome = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(completion)),
        Err(_) => crate::embedder::shared_runtime()?.block_on(completion),
    };
    match outcome {
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
// GAP-SG-146: children are named for what they DO. The former `_a`/`_b`/`_c`
// suffixes recorded how the file had been sliced by size, not what lived in it,
// which is exactly the drift the gap records.
#[path = "extraction_bindings.rs"]
mod bindings;
#[path = "extraction_body.rs"]
mod body;
#[path = "extraction_descriptions.rs"]
mod descriptions;
#[path = "extraction_graph.rs"]
mod graph;

pub(super) use bindings::*;
pub(super) use body::*;
pub(super) use descriptions::*;
pub(super) use graph::*;
