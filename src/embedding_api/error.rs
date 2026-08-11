//! Embedding failure type carrying the retry verdict from its origin.

use crate::errors::AppError;
use crate::retry::AttemptOutcome;

/// [`crate::embedding_api::OpenRouterClient::embed_single`] / [`crate::embedding_api::OpenRouterClient::embed_batch`]
/// failure (reauditor addendum, mirrors [`crate::chat_api::ChatError`]).
///
/// `retry_class` is the retry verdict computed AT THE ORIGIN (the exact HTTP
/// status, or the provider's structured error `code`) via the same
/// `openrouter_http::status_retry_class` /
/// `openrouter_http::provider_error_retry_class` classifiers (private helpers)
/// [`crate::chat_api::OpenRouterChatClient`] uses (GAP-SG-74 DRY) — never
/// inferred downstream from `source.to_string()`. The enrich `re-embed`
/// consumer reads this field directly instead of pattern-matching the
/// formatted message.
#[derive(Debug)]
pub struct EmbedError {
    /// Underlying cause, preserved via `source()` rather than restated.
    pub source: AppError,
    /// Typed retry verdict computed where the failure originated (HTTP
    /// status / provider code), not by matching `source`'s message.
    pub retry_class: AttemptOutcome,
}

impl EmbedError {
    pub(super) fn new(source: AppError, retry_class: AttemptOutcome) -> Self {
        Self {
            source,
            retry_class,
        }
    }
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.source, f)
    }
}

impl std::error::Error for EmbedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Converts a bare `AppError` into an `EmbedError` with `retry_class:
/// HardFailure`. Used by the `?` operator on call sites that predate the
/// origin-typed classification (the GAP-SG-02 oversized-input guard, the
/// dimension-mismatch guard in `OpenRouterClient::truncate_embedding`, and
/// the batch-size-mismatch check) — all of those are genuine permanent
/// client/config errors, never transient. Every `EmbedError` constructed
/// inside `execute_with_retry` uses `EmbedError::new` explicitly with a
/// retry verdict computed at the exact HTTP status / provider code instead.
impl From<AppError> for EmbedError {
    fn from(source: AppError) -> Self {
        Self::new(source, AttemptOutcome::HardFailure)
    }
}

/// Unwraps `EmbedError` back down to its `source`, discarding `retry_class`.
/// Lets the many pre-existing `?`-based callers of [`crate::embedding_api::OpenRouterClient::embed_single`]
/// / [`crate::embedding_api::OpenRouterClient::embed_batch`] (in [`crate::embedder`]) keep compiling
/// unchanged; callers that need the typed retry verdict (the enrich
/// `re-embed` path) should match on `EmbedError` directly instead of relying
/// on this conversion.
impl From<EmbedError> for AppError {
    fn from(err: EmbedError) -> Self {
        err.source
    }
}
