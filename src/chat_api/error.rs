//! Chat failure type carrying the retry verdict from its origin.

use crate::errors::AppError;
use crate::retry::AttemptOutcome;

/// [`super::OpenRouterChatClient::complete`] failure (GAP-SG-72-chat /
/// GAP-SG-72 reauditor addendum).
///
/// Wraps the underlying [`AppError`] with whatever truncation diagnostics were
/// available at the point of failure. `finish_reason`/token fields are `None`
/// when the failure happened before a response was parsed (network error, a
/// permanent 4xx, or exhausted retries) — only failures that occur AFTER a
/// `ChatResponse` was successfully decoded (JSON-repair or shape-guard
/// failures) carry them.
///
/// `retry_class` is the retry verdict computed AT THE ORIGIN (the exact HTTP
/// status, or the provider's structured error `code`), never inferred
/// downstream from `source.to_string()`. The enrich queue consumes this field
/// directly instead of pattern-matching the formatted message.
#[derive(Debug)]
pub struct ChatError {
    /// Underlying cause, preserved via `source()` rather than restated.
    pub source: AppError,
    /// `choices[0].finish_reason` from the response that led to this error,
    /// when one was decoded.
    pub finish_reason: Option<String>,
    /// `usage.prompt_tokens` from the response that led to this error, when
    /// one was decoded.
    pub prompt_tokens: Option<u32>,
    /// `usage.completion_tokens` from the response that led to this error,
    /// when one was decoded.
    pub completion_tokens: Option<u32>,
    /// Typed retry verdict computed where the failure originated (HTTP
    /// status / provider code), not by matching `source`'s message.
    pub retry_class: AttemptOutcome,
}

impl ChatError {
    /// Wraps `source` with no diagnostics attached (used when no
    /// `ChatResponse` was decoded before the failure) and the `retry_class`
    /// computed by the caller at the exact HTTP status / provider code.
    pub(super) fn new(source: AppError, retry_class: AttemptOutcome) -> Self {
        Self {
            source,
            finish_reason: None,
            prompt_tokens: None,
            completion_tokens: None,
            retry_class,
        }
    }

    /// Wraps `source` with the diagnostics captured from a decoded
    /// `ChatResponse` that nonetheless failed downstream (repair or
    /// shape-guard), plus its `retry_class`.
    pub(super) fn with_diagnostics(
        source: AppError,
        finish_reason: Option<String>,
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
        retry_class: AttemptOutcome,
    ) -> Self {
        Self {
            source,
            finish_reason,
            prompt_tokens,
            completion_tokens,
            retry_class,
        }
    }
}

impl std::fmt::Display for ChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.source, f)
    }
}

impl std::error::Error for ChatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// True when an error from `execute_with_retry` indicates the model rejected
/// `reasoning.enabled=false` because reasoning is mandatory: an HTTP 400 whose
/// body mentions "reasoning" (case-insensitive). Triggers the one-shot retry
/// with the `reasoning` field omitted.
///
/// This IS a legitimate, narrowly-scoped substring check on the underlying
/// `AppError`'s message — not a retry-classification decision (that lives in
/// `ChatError.retry_class`, computed at the origin). It only decides whether
/// to attempt the mandatory-reasoning fallback shape, an orthogonal concern.
pub(super) fn reasoning_disable_rejected(err: &ChatError) -> bool {
    let msg = err.source.to_string().to_lowercase();
    msg.contains("400") && msg.contains("reasoning")
}
