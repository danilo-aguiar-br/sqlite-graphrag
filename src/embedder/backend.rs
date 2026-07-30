//! Backend kind selection and raw embed-via-backend helpers.

use super::*;
use crate::errors::AppError;
use std::path::Path;


/// LLM backend kind for the fallback chain. Mirrors the CLI
/// `--llm-backend` enum so users can pass the same value to
/// `--llm-fallback` without translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LlmBackendKind {
    /// `codex exec` (default for v1.0.76+).
    Codex,
    /// `claude -p` (fallback for ChatGPT Pro OAuth unavailability).
    Claude,
    /// `opencode run` (v1.0.90).
    Opencode,
    /// OpenRouter HTTP API (v1.0.93).
    OpenRouter,
    /// No embedding — empty vector returned.
    None,
}

impl LlmBackendKind {
    /// Stable string label used in tracing and JSON envelopes. The
    /// string values are part of the public contract for `envelope.backend_invoked`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Opencode => "opencode",
            Self::OpenRouter => "openrouter",
            Self::None => "none",
        }
    }
}

/// Cheap readiness probe before spawning an LLM subprocess.
///
/// Checks binary presence on PATH and credential material on disk.
/// Does **not** perform network I/O. Failures are non-fatal for the
/// fallback chain — the caller skips to the next backend.
pub(crate) fn backend_ready_probe(backend: &LlmBackendKind) -> Result<(), AppError> {
    match backend {
        LlmBackendKind::None => Ok(()),
        LlmBackendKind::OpenRouter => {
            if OPENROUTER_CLIENT.get().is_some() {
                Ok(())
            } else {
                Err(AppError::Embedding(
                    crate::i18n::validation::embedding_openrouter_probe_not_initialised(),
                ))
            }
        }
        LlmBackendKind::Codex => {
            let bin = crate::runtime_config::codex_binary()
                .unwrap_or_else(|| "codex".into());
            if which::which(&bin).is_err() && which::which("codex").is_err() {
                return Err(AppError::Embedding(
                    crate::i18n::validation::embedding_codex_probe_binary_not_on_path(),
                ));
            }
            // OAuth material: ~/.codex/auth.json or CODEX_HOME/auth.json
            let auth = std::env::var_os("CODEX_HOME")
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|h| {
                        std::path::PathBuf::from(h).join(".codex")
                    })
                })
                .map(|p| p.join("auth.json"));
            match auth {
                Some(p) if p.is_file() => Ok(()),
                _ => Err(AppError::Embedding(
                    crate::i18n::validation::embedding_codex_probe_auth_missing(),
                )),
            }
        }
        LlmBackendKind::Claude => {
            let bin = crate::runtime_config::claude_binary()
                .unwrap_or_else(|| "claude".into());
            if which::which(&bin).is_err() && which::which("claude").is_err() {
                return Err(AppError::Embedding(
                    crate::i18n::validation::embedding_claude_probe_binary_not_on_path(),
                ));
            }
            Ok(())
        }
        LlmBackendKind::Opencode => {
            let bin = crate::runtime_config::opencode_binary()
                .unwrap_or_else(|| "opencode".into());
            if which::which(&bin).is_err() && which::which("opencode").is_err() {
                return Err(AppError::Embedding(
                    crate::i18n::validation::embedding_opencode_probe_binary_not_on_path(),
                ));
            }
            Ok(())
        }
    }
}

/// Embeds a single text via the given backend. Used by
/// `embed_with_fallback` and exposed to allow direct one-shot
/// selection without a chain.
/// Embeds a single text via the given backend. Used by
/// `embed_with_fallback` and exposed to allow direct one-shot
/// selection without a chain.
///
/// BUG-003 / v1.0.85: returns `(Vec<f32>, LlmBackendKind)`. The
/// second element reports the backend that ACTUALLY executed the
/// embedding, not the chain position requested by the caller. When
/// `LlmBackendKind::Codex` is requested but `codex` is absent from
/// PATH, `LlmEmbedding::detect_available` substitutes claude and the
/// tuple carries `LlmBackendKind::Claude` so the operator sees the
/// truth in `envelope.backend_invoked`.
pub fn embed_via_backend(
    models_dir: &Path,
    text: &str,
    backend: &LlmBackendKind,
) -> Result<(Vec<f32>, LlmBackendKind), AppError> {
    match backend {
        LlmBackendKind::None => Ok((Vec::new(), LlmBackendKind::None)),
        LlmBackendKind::Codex => embed_passage_local_resolved(models_dir, text),
        LlmBackendKind::Claude => {
            // ADR-0042 / GAP-002: route Claude through its own static
            // embedder instead of re-using the Codex path (which used
            // to silently pick Codex if PATH ordered it first).
            tracing::debug!(
                target: "embedder",
                backend = "claude",
                "embed_via_backend: forcing claude (ADR-0042 / GAP-002 fix)"
            );
            embed_via_claude_local_resolved(models_dir, text, None, None)
        }
        LlmBackendKind::Opencode => {
            tracing::debug!(
                target: "embedder",
                backend = "opencode",
                "embed_via_backend: forcing opencode (GAP-OPENCODE-001)"
            );
            embed_via_opencode_local_resolved(models_dir, text, None, None)
        }
        LlmBackendKind::OpenRouter => {
            tracing::debug!(
                target: "embedder",
                backend = "openrouter",
                "embed_via_backend: using OpenRouter API (v1.0.93)"
            );
            let client = OPENROUTER_CLIENT.get().ok_or_else(|| {
                AppError::Embedding(
                    crate::i18n::validation::embedding_openrouter_client_not_initialised(),
                )
            })?;
            // GAP-001 (v1.1.04): canonical nested-runtime guard. When called
            // from inside an existing tokio runtime (e.g. deep-research fan-out),
            // `block_in_place` parks the current worker thread and drives the
            // future via the existing handle instead of building a nested
            // runtime, which would panic with "Cannot start a runtime from
            // within a runtime".
            let vec = match tokio::runtime::Handle::try_current() {
                Ok(handle) => tokio::task::block_in_place(|| {
                    handle.block_on(client.embed_single(text, client.default_input_type()))
                })?,
                Err(_) => shared_runtime()?
                    .block_on(client.embed_single(text, client.default_input_type()))?,
            };
            Ok((vec, LlmBackendKind::OpenRouter))
        }
    }
}

// ADR-0046 / BUG-11 v1.0.88: specialisation of `embed_via_backend` that
// refuses to SILENTLY DEGRADE to `LlmBackendKind::None` after all real
// backends (Codex, Claude) have failed. The previous behaviour
// (`Ok((Vec::new(), None))`) caused the `remember` write path to persist
// memories with zero-dimensional embeddings — breaking `recall` and
// `hybrid-search` while returning exit 0 (BUG-11 CRITICAL).
//
// When `--llm-backend none` is explicitly requested (i.e. `last_err` is
// None AND the chain was a single-element `[None]`), pass
// `skip_on_failure = true` to `embed_with_fallback` to consume the empty
// vector via the pending-embeddings retry queue instead of persisting
// directly. This helper is the right hook for `remember`/`edit`/`ingest`.
/// Embed via backend strict.
pub fn embed_via_backend_strict(
    models_dir: &Path,
    text: &str,
    backend: &LlmBackendKind,
    last_err: Option<&AppError>,
    skip_on_failure: bool,
) -> Result<(Vec<f32>, LlmBackendKind), AppError> {
    use crate::llm::exit_code_hints::LlmBackendError;
    match backend {
        LlmBackendKind::None => {
            // GAP-CLI-EMBED-NONE (v1.1.8): an intentional chain of only
            // `[None]` (`--llm-backend none`) MUST skip embedding with an
            // empty vector — matching the CLI help contract "skips embedding;
            // useful for tests". When `None` is reached *after* a real
            // backend failed (`last_err.is_some()`), honour
            // `skip_on_failure` or propagate the prior error (BUG-11).
            // Intentional none-only chain, or skip-on-failure after a prior error.
            if last_err.is_none() || skip_on_failure {
                Ok((Vec::new(), LlmBackendKind::None))
            } else {
                Err(match last_err {
                    Some(e) => AppError::Embedding(
                        crate::i18n::validation::embedding_detail(e),
                    ),
                    None => AppError::Embedding(crate::i18n::validation::embedding_detail(
                        LlmBackendError::NoBackendsAvailable,
                    )),
                })
            }
        }
        LlmBackendKind::Codex => embed_passage_local_resolved(models_dir, text),
        LlmBackendKind::Claude => {
            tracing::debug!(
                target: "embedder",
                backend = "claude",
                "embed_via_backend_strict: forcing claude (ADR-0042 / GAP-002 fix)"
            );
            embed_via_claude_local_resolved(models_dir, text, None, None)
        }
        LlmBackendKind::Opencode => {
            tracing::debug!(
                target: "embedder",
                backend = "opencode",
                "embed_via_backend_strict: forcing opencode (GAP-OPENCODE-001)"
            );
            embed_via_opencode_local_resolved(models_dir, text, None, None)
        }
        LlmBackendKind::OpenRouter => embed_via_backend(models_dir, text, backend),
    }
}

/// Legacy one-shot wrapper around `embed_via_backend` that discards
/// the resolved backend. Kept for call sites that only care about
/// the vector and ignore the executed-backend signal. New code
/// should prefer `embed_via_backend` directly.
pub fn embed_via_backend_legacy(
    models_dir: &Path,
    text: &str,
    backend: &LlmBackendKind,
) -> Result<Vec<f32>, AppError> {
    embed_via_backend(models_dir, text, backend).map(|(v, _)| v)
}

/// F 32 to bytes.
pub fn f32_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

/// Bytes to f 32.
pub fn bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    out
}

/// Returns the dimensionality of the embedding space. Used to
/// validate LLM responses and to size the in-memory cache.
pub fn embedding_dim() -> usize {
    crate::constants::embedding_dim()
}

/// G42/C5: a vector with a divergent dimensionality is an ERROR, never
/// silently truncated or zero-padded (the pre-v1.0.79 `normalise_dim`
/// masked malformed LLM responses).
pub(crate) fn validate_dim(v: Vec<f32>) -> Result<Vec<f32>, AppError> {
    let dim = crate::constants::embedding_dim();
    if v.len() != dim {
        return Err(AppError::Embedding(
            crate::i18n::validation::embedding_has_dims_expected(v.len(), dim),
        ));
    }
    Ok(v)
}

