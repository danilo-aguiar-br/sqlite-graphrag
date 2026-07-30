//! Embedding error classification and fallback orchestration.

use super::*;
use crate::errors::AppError;
use std::path::Path;

/// GAP-004 (v1.0.88): typed classifier for embedding error messages.
///
/// Decomposes the legacy `AppError::Embedding(String)` payload into a
/// small enum so the call sites can branch on the cause instead of
/// repeating `msg.contains(...)` literals. The classification is purely
/// lexical (case-insensitive substring match on the error message) — no
/// I/O, no retries, no telemetry, deterministic and safe under
/// `#[serial_test::serial(env)]`.
///
/// 6 variants cover the 5 known discriminators from v1.0.85 (ADR-0043)
/// plus an `Unknown` fallback for messages that do not match any marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingErrorKind {
    /// OAuth token expired or absent; no backend can authenticate.
    OAuth,
    /// OAuth usage quota exhausted on the named backend.
    Quota,
    /// LLM slot semaphore exhausted after the backoff window.
    SlotExhausted,
    /// User-requested backend differs from the one that actually executed.
    BackendMismatch,
    /// Embedding returned a zero-dimensional vector (structural bug).
    ZeroDimension,
    /// Message did not match any of the 5 markers above.
    Unknown,
}

impl EmbeddingErrorKind {
    /// Classify an embedding error message into a typed kind.
    ///
    /// Order of checks matters: `OAuth` is matched before `Quota` because
    /// both substrings can co-occur in the same message. `SlotExhausted`
    /// is checked before `Quota` because the slot-sema path is more
    /// specific (the LLM never even tried to authenticate). The checks
    /// are case-insensitive so `OAuth` and `oauth` both classify to
    /// `EmbeddingErrorKind::OAuth`.
    pub fn classify(msg: &str) -> Self {
        let m = msg.to_lowercase();
        if m.contains("oauth") {
            Self::OAuth
        } else if m.contains("quota") {
            Self::Quota
        } else if m.contains("slot exhausted") {
            Self::SlotExhausted
        } else if m.contains("backend mismatch") {
            Self::BackendMismatch
        } else if m.contains("dim") && m.contains("zero") {
            Self::ZeroDimension
        } else {
            Self::Unknown
        }
    }

    /// Stable, machine-friendly discriminator code (lowercase, kebab-safe).
    pub fn code(&self) -> &'static str {
        match self {
            Self::OAuth => "oauth",
            Self::Quota => "quota",
            Self::SlotExhausted => "slot-exhausted",
            Self::BackendMismatch => "backend-mismatch",
            Self::ZeroDimension => "zero-dimension",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for EmbeddingErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

/// G58/S1: reason an embedding call could not be completed and the caller
/// must fall back to a non-vector retrieval path (FTS5 prefix + LIKE).
///
/// Returned by [`try_embed_query_with_fallback`] so the `recall` and
/// `hybrid-search` handlers can surface a structured `vec_degraded` /
/// `warning` envelope instead of a hard `AppError::Embedding` exit 11.
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackReason {
    /// The LLM subprocess failed (rate limit, OAuth contention, quota
    /// exhausted, model unparsable response, divergent dim, etc.).
    /// Carries the original error message for observability.
    EmbeddingFailed(String),
    /// The LLM slot semaphore was exhausted: 8+ concurrent LLM
    /// subprocesses blocked the acquire beyond the backoff window
    /// (50ms + 100ms + 200ms + 400ms = 750ms total). Resolved at v1.0.85
    /// (GAP-003 / ADR-0043).
    SlotExhausted,
    /// OAuth usage quota exhausted on the named backend. The caller
    /// should retry with an alternative backend (codex ↔ claude)
    /// before falling back to FTS5-puro.
    OAuthQuota {
        /// Backend identifier.
        backend: &'static str,
    },
    /// The user requested a backend that differs from the one that
    /// actually executed the embedding (legacy "synonym for codex"
    /// bug from v1.0.83). Resolved at v1.0.84 (GAP-002).
    BackendMismatch {
        /// Requested.
        requested: &'static str,
        /// Resolved.
        resolved: &'static str,
    },
    /// The embedding returned a zero-dimensional vector, signalling a
    /// structural bug (the LLM did not produce any floats). Distinct
    /// from OAuthQuota (quota exhausted) and EmbeddingFailed
    /// (subprocess error).
    DimZero,
    /// The embedding was cancelled by an external signal (SIGTERM, etc.).
    Cancelled,
    /// The embedding exceeded its time budget. Carries the operation name
    /// and the elapsed seconds for diagnostic logging.
    Timeout {
        /// Operation.
        operation: String,
        /// Duration secs.
        duration_secs: u64,
    },
}

impl FallbackReason {
    /// Stable, machine-friendly reason code used by JSON envelopes
    /// (`vec_degraded_reason`). Mirrors the v1.0.84 contract extended
    /// at v1.0.85 with 4 new variants (GAP-003 / ADR-0043).
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::EmbeddingFailed(_) => "embedding_failed",
            Self::SlotExhausted => "slot_exhausted",
            Self::OAuthQuota { .. } => "oauth_quota",
            Self::BackendMismatch { .. } => "backend_mismatch",
            Self::DimZero => "dim_zero",
            Self::Cancelled => "cancelled",
            Self::Timeout { .. } => "timeout",
        }
    }
}

impl std::fmt::Display for FallbackReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmbeddingFailed(msg) => write!(f, "embedding failed: {msg}"),
            Self::SlotExhausted => write!(
                f,
                "slot exhausted: failed to acquire LLM slot after backoff window (max=8 concurrent, total backoff=750ms)"
            ),
            Self::OAuthQuota { backend } => {
                write!(f, "OAuth usage quota exhausted on backend '{backend}'")
            }
            Self::BackendMismatch {
                requested,
                resolved,
            } => {
                write!(
                    f,
                    "backend mismatch: user requested '{requested}' but '{resolved}' was invoked"
                )
            }
            Self::DimZero => write!(f, "embedding returned zero-dimensional vector"),
            Self::Cancelled => write!(f, "embedding cancelled by external signal"),
            Self::Timeout {
                operation,
                duration_secs,
            } => {
                write!(
                    f,
                    "embedding timed out after {duration_secs}s during {operation}"
                )
            }
        }
    }
}

impl std::error::Error for FallbackReason {}

/// G58/S1: try to embed a query, mapping any failure to a structured
/// [`FallbackReason`] so callers can route to FTS5 + LIKE fallback instead
/// of returning exit 11 to the user.
///
/// This is the bridge between the hard-fail `embed_query_local` (used by
/// write paths where embedding failure aborts the operation) and the
/// graceful-degradation contract of `recall` / `hybrid-search` in v1.0.80.
pub fn try_embed_query_with_fallback(
    models_dir: &Path,
    query: &str,
) -> Result<(Vec<f32>, LlmBackendKind), FallbackReason> {
    match embed_query_local(models_dir, query) {
        Ok(v) => Ok((v, LlmBackendKind::None)),
        Err(e) => Err(classify_embedding_error(e)),
    }
}

/// G58 / ADR-0043 (v1.0.85): deterministic fallback for `recall` and
/// `hybrid-search`.
///
/// - On `OAuthQuota { backend }`, retry once with the alternative backend
///   (codex ↔ claude) before giving up.
/// - On `SlotExhausted`, sleep 750ms and retry once (gives the slot
///   semaphore time to release a permit from a sibling subprocess).
/// - On any other `FallbackReason`, return immediately (deterministic).
pub fn try_embed_query_with_deterministic_fallback(
    models_dir: &Path,
    query: &str,
    choice: Option<crate::cli::LlmBackendChoice>,
) -> Result<(Vec<f32>, LlmBackendKind), FallbackReason> {
    match try_embed_query_with_choice(models_dir, query, choice) {
        Ok(t) => Ok(t),
        Err(reason @ FallbackReason::OAuthQuota { backend }) => {
            let alt = match backend {
                "codex" => Some(crate::cli::LlmBackendChoice::Claude),
                "claude" => Some(crate::cli::LlmBackendChoice::Codex),
                "opencode" => Some(crate::cli::LlmBackendChoice::Codex),
                "openrouter" => Some(crate::cli::LlmBackendChoice::Codex),
                _ => None,
            };
            if let Some(alt_choice) = alt {
                try_embed_query_with_choice(models_dir, query, Some(alt_choice))
            } else {
                Err(reason)
            }
        }
        Err(reason @ FallbackReason::SlotExhausted) => {
            std::thread::sleep(std::time::Duration::from_millis(750));
            try_embed_query_with_choice(models_dir, query, choice).or(Err(reason))
        }
        Err(other) => Err(other),
    }
}

/// Classify an embedding [`AppError`] into a typed [`FallbackReason`].
///
/// v1.0.85 (ADR-0043): discriminates the 4 new causes (SlotExhausted,
/// OAuthQuota, BackendMismatch, DimZero) from the legacy generic
/// EmbeddingFailed bucket. The classification is purely lexical
/// (substring match on the message) — no I/O, no retries, no
/// telemetry, deterministic and `#[serial_test::serial(env)]`-safe.
pub fn classify_embedding_error(err: AppError) -> FallbackReason {
    match err {
        AppError::Timeout {
            operation,
            duration_secs,
        } => FallbackReason::Timeout {
            operation,
            duration_secs,
        },
        AppError::Embedding(msg) => match EmbeddingErrorKind::classify(&msg) {
            // GAP-004 (v1.0.88): typed-discriminator dispatch.
            // The lexical classifier picks the discriminator; the arms below
            // enrich the result with the backend name and the
            // requested/resolved pair that the JSON envelope needs.
            //
            // Note: `Cancelled` and `EmbeddingFailed(msg)` are not in the
            // 6-variant enum (they have no lexical marker) so we keep them
            // as explicit guards at the head of the match.
            EmbeddingErrorKind::SlotExhausted => FallbackReason::SlotExhausted,
            EmbeddingErrorKind::OAuth => {
                let backend = if msg.contains("codex") {
                    "codex"
                } else if msg.contains("claude") || msg.contains("anthropic-ratelimit") {
                    // G45-CR5: anthropic-ratelimit-* headers are emitted only by
                    // the Claude CLI subprocess; treat them as claude quota
                    // signals even when the message text omits the word
                    // "claude" explicitly.
                    "claude"
                } else if msg.contains("opencode") {
                    "opencode"
                } else {
                    "unknown"
                };
                FallbackReason::OAuthQuota { backend }
            }
            EmbeddingErrorKind::Quota => {
                let backend = if msg.contains("codex") {
                    "codex"
                } else if msg.contains("claude") || msg.contains("anthropic-ratelimit") {
                    "claude"
                } else if msg.contains("opencode") {
                    "opencode"
                } else {
                    "unknown"
                };
                FallbackReason::OAuthQuota { backend }
            }
            EmbeddingErrorKind::BackendMismatch => {
                // The `msg.contains("claude")` arm is intentionally
                // placed BEFORE the OAuth arm so that a backend-mismatch
                // message that mentions both "claude" and "codex" maps to
                // BackendMismatch (the more specific failure mode).
                let (requested, resolved) =
                    if msg.contains("requested claude") && msg.contains("but codex") {
                        ("claude", "codex")
                    } else if msg.contains("requested codex") && msg.contains("but claude") {
                        ("codex", "claude")
                    } else if msg.contains("requested claude") {
                        ("claude", "unknown")
                    } else if msg.contains("requested codex") {
                        ("codex", "unknown")
                    } else {
                        ("unknown", "unknown")
                    };
                FallbackReason::BackendMismatch {
                    requested,
                    resolved,
                }
            }
            EmbeddingErrorKind::ZeroDimension => FallbackReason::DimZero,
            EmbeddingErrorKind::Unknown => {
                if msg.contains("cancelled") {
                    FallbackReason::Cancelled
                } else {
                    FallbackReason::EmbeddingFailed(msg)
                }
            }
        },
        e => FallbackReason::EmbeddingFailed(e.to_string()),
    }
}
// backends before giving up. The chain order matches the user-supplied
// `--llm-fallback` list (default: codex, claude, none).
// =============================================================================

/// Tries each LLM backend in `chain` in order, returning the first
/// successful embedding. On failure, the diagnostic tail of the last
/// error is preserved in the returned `AppError::Embedding` so the
/// operator can see WHY every backend failed.
///
/// If `skip_on_failure` is `true` AND every backend fails, the function
/// returns `Ok(Vec::new())` (an empty vector) to signal "persist
/// without embedding" — the call site is then responsible for writing
/// a `pending_embeddings` row that can be retried later by the
/// `embedding retry` subcommand.
///
/// Defaults the chain to `[codex, claude, none]` when `chain` is
/// empty, matching the v1.0.81 behaviour where codex was the
/// implicit default and claude was the implicit fallback.
pub fn embed_with_fallback(
    models_dir: &Path,
    text: &str,
    chain: &[LlmBackendKind],
    skip_on_failure: bool,
) -> Result<(Vec<f32>, LlmBackendKind), AppError> {
    use crate::llm::exit_code_hints::LlmBackendError;
    let effective: Vec<LlmBackendKind> = if chain.is_empty() {
        vec![
            LlmBackendKind::Codex,
            LlmBackendKind::Claude,
            LlmBackendKind::Opencode,
            LlmBackendKind::None,
        ]
    } else {
        chain.to_vec()
    };

    let mut last_err: Option<AppError> = None;
    for backend in &effective {
        // GAP-E2E-06 / v1.1.8: fail-fast credential/binary probe so Auto
        // does not burn ~20s on a dead Codex/Claude before FTS fallback.
        if let Err(probe_err) = backend_ready_probe(backend) {
            tracing::warn!(
                target: "embedding",
                backend = ?backend,
                error = %probe_err,
                "embed_with_fallback: backend probe failed, skipping"
            );
            last_err = Some(probe_err);
            continue;
        }
        // BUG-003 / v1.0.85: propagar o backend REAL retornado por
        // embed_via_backend (que pode diferir do chain position quando
        // LlmEmbedding::detect_available substitui codex por claude).
        // O tuple `(_, requested_kind)` é descartado — só queremos o
        // backend resolvido na primeira posição.
        // ADR-0046 / BUG-11 v1.0.88: use `embed_via_backend_strict` so the
        // sentinel `None` backend propagates the last real error instead
        // of silently degrading to `Ok((Vec::new(), None))`. This is the
        // path that caused preflight rejections to be swallowed by the
        // chain's default trailing `None`.
        match embed_via_backend_strict(
            models_dir,
            text,
            backend,
            last_err.as_ref(),
            skip_on_failure,
        ) {
            Ok((v, resolved_kind)) => return Ok((v, resolved_kind)),
            Err(e) => {
                // ADR-0011: Validation errors (OAuth-only enforcement) are
                // FATAL — propagate immediately without trying the next
                // backend. This prevents the fallback chain from swallowing
                // OAuth violations via the trailing `None` sentinel.
                if matches!(e, AppError::Validation(_)) {
                    return Err(e);
                }
                tracing::warn!(
                    target: "embedding",
                    backend = ?backend,
                    error = %e,
                    "embed_with_fallback: backend failed, trying next"
                );
                last_err = Some(e);
            }
        }
    }
    if skip_on_failure {
        // Signal "persist with no embedding" via an empty vector paired
        // with `None` so callers know the chain exhausted without a hit.
        // Caller is responsible for writing a `pending_embeddings` row
        // that can be retried later by the `embedding retry` subcommand.
        return Ok((Vec::new(), LlmBackendKind::None));
    }
    Err(last_err.unwrap_or_else(|| {
        AppError::Embedding(crate::i18n::validation::embedding_detail(
            LlmBackendError::NoBackendsAvailable,
        ))
    }))
}
