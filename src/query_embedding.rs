//! The one place a read path turns a query string into a live embedding.
//!
//! `recall` and `hybrid-search` both need the same three-way outcome — a
//! vector, a deliberate skip, or a degradation — and both need to report it the
//! same way on their envelope. They used to carry byte-identical copies of that
//! logic, including the reason string, which is duplicated knowledge rather than
//! merely similar code: changing the degradation contract meant remembering to
//! edit two files, and nothing failed if you edited one.
//!
//! Keeping the resolution here also keeps the envelope honest. `vec_degraded`
//! is what tells a caller that a hybrid search silently became a pure FTS5
//! search, and a single implementation is what guarantees both commands raise
//! it under exactly the same conditions.

/// Live query embedding plus the flags describing whether it succeeded.
///
/// `backend_invoked` names the backend that actually ran, and is `None` both
/// when the caller opted out and when every attempt failed.
///
/// Was a four-element tuple until v1.2.5. It became a struct when `reason_code`
/// was added because the field next to it, `backend_invoked`, has the SAME type:
/// two adjacent `Option<&'static str>` in a tuple swap silently, the compiler
/// accepts it, and the only symptom is a degradation classified as the wrong
/// error. A named field makes that swap impossible to write.
///
/// `reason_code` is the machine-readable half of `error`, and it exists because
/// [`degradation_failure`] derives the error class from the code and never from
/// the prose. Until v1.2.5 this resolver logged the code and threw it away, so
/// no caller could satisfy that contract — which is why `--fail-on-degraded`
/// shipped declared, documented and never once consulted.
pub struct QueryEmbedding {
    /// The query vector, or `None` when the read degraded to FTS5-only.
    pub embedding: Option<Vec<f32>>,
    /// Whether the read fell back to BM25 alone.
    pub degraded: bool,
    /// Operator-facing prose for the degradation.
    pub error: Option<String>,
    /// Backend that actually produced the vector.
    pub backend_invoked: Option<&'static str>,
    /// Stable code for the degradation, `None` when nothing degraded.
    pub reason_code: Option<&'static str>,
}

/// Machine-readable `vec_error` for a degradation the operator asked for.
///
/// Named rather than inlined because it is a value consumers match on: it is the
/// one `vec_error` that means "nothing went wrong", so a caller distinguishing a
/// deliberate skip from a real failure compares against this exact string.
pub const FALLBACK_FTS_ONLY_REASON: &str = "fallback_fts_only requested";

/// `reason_code` recorded for a degradation the operator asked for.
///
/// The prose in [`FALLBACK_FTS_ONLY_REASON`] is what a human reads; this is what
/// [`degradation_failure`] branches on. Two representations because the envelope
/// has always carried the prose and changing it would break consumers.
pub const FALLBACK_FTS_ONLY_CODE: &str = "fallback_fts_only";

/// Decides whether a degraded read must become a non-zero exit.
///
/// Returns `None` — the read stands, exit 0, envelope untouched — when any of:
/// - `fail_on_degraded` is off, which is the default and the historical
///   behaviour byte for byte;
/// - nothing degraded;
/// - the degradation was REQUESTED with `--fallback-fts-only`.
///
/// That third case is the whole point of the discriminator. `--fallback-fts-only`
/// is an operator saying "skip the provider, BM25 is what I want"; turning their
/// own instruction into a failure would make the two flags mutually unusable.
///
/// # Error classification
///
/// The class is derived from `reason_code`, never from the message prose, so a
/// reworded string cannot silently reclassify a failure:
/// - `timeout`, `slot_exhausted`, `oauth_quota`, `cancelled` — the provider was
///   unreachable or too slow. [`crate::errors::AppError::Timeout`] is retryable, so
///   `error_class` is `transient` and `retryable` is `true`: retrying is exactly
///   the right advice.
/// - anything else (`dim_zero`, `backend_mismatch`, `embedding_failed`) — the
///   configuration or the response shape is wrong, and retrying an unchanged
///   invocation reproduces it. [`crate::errors::AppError::Embedding`] carries exit 11.
pub fn degradation_failure(
    fail_on_degraded: bool,
    vec_degraded: bool,
    reason_code: Option<&str>,
) -> Option<crate::errors::AppError> {
    if !fail_on_degraded || !vec_degraded {
        return None;
    }
    let code = reason_code.unwrap_or("unknown");
    if code == FALLBACK_FTS_ONLY_CODE {
        return None;
    }
    // Built here rather than in `i18n::validation` because the operator-facing
    // half of this message is the `vec_error` the envelope ALREADY carries,
    // localised at its own source; this string only names the discriminator.
    let detail = format!("query embedding degraded to FTS5-only ({code})");
    match code {
        "timeout" | "slot_exhausted" | "oauth_quota" | "cancelled" => {
            Some(crate::errors::AppError::Timeout {
                operation: detail,
                duration_secs: 0,
            })
        }
        _ => Some(crate::errors::AppError::Embedding(detail)),
    }
}

/// Resolves the query embedding, degrading to FTS5-only instead of failing.
///
/// When the live embedding cannot be produced — timeout, rate limit,
/// unreachable provider — the read still returns results, ranked by BM25 alone.
/// The caller surfaces that through `vec_degraded` and `vec_error` on the
/// envelope, so the degradation is visible rather than silent.
///
/// `--fallback-fts-only` takes the same path deliberately and never contacts the
/// provider at all.
///
/// `log_target` is the tracing target of the calling subcommand, so a degraded
/// read stays attributable to the command the operator actually ran.
pub fn resolve_query_embedding(
    fallback_fts_only: bool,
    models_dir: &std::path::Path,
    query: &str,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    llm_backend: crate::cli::LlmBackendChoice,
    log_target: &'static str,
) -> QueryEmbedding {
    if fallback_fts_only {
        return QueryEmbedding {
            embedding: None,
            degraded: true,
            error: Some(FALLBACK_FTS_ONLY_REASON.to_string()),
            backend_invoked: None,
            // The code the operator ASKED for. `degradation_failure` matches on
            // it to keep `--fallback-fts-only` from turning into a failure.
            reason_code: Some(FALLBACK_FTS_ONLY_CODE),
        };
    }
    match crate::embedder::try_embed_query_with_embedding_choice(
        models_dir,
        query,
        embedding_backend,
        llm_backend,
    ) {
        Ok((v, backend)) => QueryEmbedding {
            embedding: Some(v),
            degraded: false,
            error: None,
            backend_invoked: Some(backend.as_str()),
            reason_code: None,
        },
        Err(reason) => {
            let msg = reason.to_string();
            let code = reason.reason_code();
            tracing::warn!(
                target: "query_embedding",
                command = log_target,
                fallback_reason = %msg,
                reason_code = %code,
                "live embedding failed; falling back to FTS5"
            );
            QueryEmbedding {
                embedding: None,
                degraded: true,
                error: Some(msg),
                backend_invoked: None,
                reason_code: Some(code),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opting_out_reports_the_named_reason_and_never_names_a_backend() {
        let resolved = resolve_query_embedding(
            true,
            std::path::Path::new("/nonexistent"),
            "anything",
            crate::cli::EmbeddingBackendChoice::Openrouter,
            crate::cli::LlmBackendChoice::None,
            "recall",
        );
        assert!(
            resolved.embedding.is_none(),
            "opting out must not produce a vector"
        );
        assert!(
            resolved.degraded,
            "opting out is still a degradation for the caller"
        );
        assert_eq!(resolved.error.as_deref(), Some(FALLBACK_FTS_ONLY_REASON));
        assert!(
            resolved.backend_invoked.is_none(),
            "no provider was contacted, so none may be reported as invoked"
        );
        // O código é o que impede `--fail-on-degraded` de transformar a escolha do
        // operador em falha. Se ele parar de vir, `degradation_failure` cai no ramo
        // "unknown" e `--fallback-fts-only` passa a reprovar com as duas flags juntas.
        assert_eq!(
            resolved.reason_code,
            Some(FALLBACK_FTS_ONLY_CODE),
            "opting out must carry its own code, not an absent one"
        );
    }

    /// Degradation the caller ASKED for never becomes a failure, flag or no flag.
    ///
    /// The pair that closes the loop: the test above proves the code arrives,
    /// and this one proves what the code is there to decide. Without it, someone
    /// could drop `reason_code` from the opt-out branch and only one would break.
    #[test]
    fn opting_out_survives_fail_on_degraded() {
        let resolved = resolve_query_embedding(
            true,
            std::path::Path::new("/nonexistent"),
            "anything",
            crate::cli::EmbeddingBackendChoice::Openrouter,
            crate::cli::LlmBackendChoice::None,
            "recall",
        );
        assert!(
            degradation_failure(true, resolved.degraded, resolved.reason_code).is_none(),
            "--fallback-fts-only com --fail-on-degraded deve continuar saindo 0"
        );
    }
}
