//! GAP-SG-270: carries `EmbedError::retry_class` across the conversion to
//! `AppError`.
//!
//! [`crate::embedding_api::EmbedError`] states its retry verdict at the exact
//! HTTP status / structured provider code, but `From<EmbedError> for AppError`
//! unwraps down to `source` and DROPS that verdict. Every embedding failure then
//! arrived at the enrich queue as the untyped `AppError::Embedding` bucket,
//! whose conservative floor retries a PERMANENT failure until `--max-attempts`
//! runs out.
//!
//! The embedder is the single place where the fallback chain performs that
//! conversion, so the verdict is re-attached HERE — via
//! [`crate::errors::AppError::EmbeddingClassified`] — instead of being
//! reconstructed by the consumer.

use crate::embedding_api::EmbedError;
use crate::errors::AppError;
use crate::retry::AttemptOutcome;

/// Converts an [`EmbedError`] into an [`AppError`] WITHOUT losing its
/// origin-computed retry verdict (GAP-SG-270).
///
/// Only the untyped `AppError::Embedding` bucket is re-wrapped as
/// [`AppError::EmbeddingClassified`]. Every other source variant already
/// classifies itself (`RateLimited`, `Timeout` and `DbBusy` are transient;
/// `ProviderError` and `NotFound` are permanent), so it is forwarded untouched
/// and the existing typed classification keeps winning.
pub(crate) fn app_error_preserving_retry_class(err: EmbedError) -> AppError {
    let EmbedError {
        source,
        retry_class,
    } = err;
    match source {
        AppError::Embedding(message) => AppError::EmbeddingClassified {
            message,
            retry_class,
        },
        already_typed => already_typed,
    }
}

/// Reads back the retry verdict an [`AppError`] carries, if it carries one.
///
/// Returns `None` for every variant that never held a verdict, which keeps the
/// caller on its pre-existing untyped path instead of inventing a class.
pub(crate) fn retry_class_of(err: &AppError) -> Option<AttemptOutcome> {
    match err {
        AppError::EmbeddingClassified { retry_class, .. } => Some(*retry_class),
        _ => None,
    }
}

/// Rebuilds an embedding failure with a NEW message while keeping the retry
/// verdict `previous` carried.
///
/// The fallback chain restates the last backend error as an embedding detail
/// string; doing that with a bare `AppError::Embedding` threw the verdict away
/// one step after it had just been preserved.
pub(crate) fn embedding_error_with_class_of(message: String, previous: &AppError) -> AppError {
    match retry_class_of(previous) {
        Some(retry_class) => AppError::EmbeddingClassified {
            message,
            retry_class,
        },
        None => AppError::Embedding(message),
    }
}
