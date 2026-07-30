//! Embedder client getters and local-backend helpers.

use super::*;
use crate::errors::AppError;
use crate::extract::llm_embedding::LlmEmbedding;
use parking_lot::Mutex;
use std::path::Path;

/// Returns true when the process-wide OpenRouter embed client is ready.
pub fn is_openrouter_initialized() -> bool {
    OPENROUTER_CLIENT.get().is_some()
}

/// Returns the process-wide multi-thread runtime, building it on first use.
pub(crate) fn shared_runtime() -> Result<&'static tokio::runtime::Runtime, AppError> {
    if let Some(rt) = RUNTIME.get() {
        return Ok(rt);
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| {
            AppError::Embedding(crate::i18n::validation::embedding_tokio_runtime_init_failed(e))
        })?;
    let _ = RUNTIME.set(rt);
    RUNTIME.get().ok_or_else(|| {
        AppError::Embedding(crate::i18n::validation::embedding_tokio_runtime_unavailable())
    })
}

/// Initialises the LLM-embedding client on first use and returns it.
pub fn get_embedder(_models_dir: &Path) -> Result<&'static Mutex<LlmEmbedding>, AppError> {
    if let Some(e) = EMBEDDER.get() {
        return Ok(e);
    }
    let backend = LlmEmbedding::detect_available()?;
    let _ = EMBEDDER.set(Mutex::new(backend));
    EMBEDDER
        .get()
        .ok_or_else(|| {
            AppError::Embedding(crate::i18n::validation::embedding_embedder_unavailable())
        })
}

/// ADR-0042 / GAP-002: returns the process-wide Claude embedder, lazily
/// initialising it on first use. Binary and model overrides come from
/// the explicit arguments; `None` falls back to PATH/env defaults via
/// the builder.
pub fn get_claude_embedder(
    claude_binary: Option<&Path>,
    claude_model: Option<&str>,
) -> Result<&'static Mutex<LlmEmbedding>, AppError> {
    if let Some(e) = CLAUDE_EMBEDDER.get() {
        return Ok(e);
    }
    let mut builder = LlmEmbedding::with_claude_builder();
    if let Some(b) = claude_binary {
        builder = builder.override_binary(b.to_path_buf());
    }
    if let Some(m) = claude_model {
        builder = builder.override_model(m.to_string());
    }
    let backend = builder.build()?;
    let _ = CLAUDE_EMBEDDER.set(Mutex::new(backend));
    CLAUDE_EMBEDDER.get().ok_or_else(|| {
        AppError::Embedding(crate::i18n::validation::embedding_claude_embedder_unavailable())
    })
}

/// GAP-OPENCODE-001 / v1.0.90: returns the process-wide OpenCode embedder,
/// lazily initialising it on first use. Binary and model overrides come
/// from the explicit arguments; `None` falls back to PATH/env defaults via
/// the builder.
pub fn get_opencode_embedder(
    opencode_binary: Option<&Path>,
    opencode_model: Option<&str>,
) -> Result<&'static Mutex<LlmEmbedding>, AppError> {
    if let Some(e) = OPENCODE_EMBEDDER.get() {
        return Ok(e);
    }
    let mut builder = LlmEmbedding::with_opencode_builder();
    if let Some(b) = opencode_binary {
        builder = builder.override_binary(b.to_path_buf());
    }
    if let Some(m) = opencode_model {
        builder = builder.override_model(m.to_string());
    }
    let backend = builder.build()?;
    let _ = OPENCODE_EMBEDDER.set(Mutex::new(backend));
    OPENCODE_EMBEDDER.get().ok_or_else(|| {
        AppError::Embedding(crate::i18n::validation::embedding_opencode_embedder_unavailable())
    })
}

/// Get openrouter embedder.
pub fn get_openrouter_embedder(
    api_key: secrecy::SecretBox<String>,
    model: &str,
    dim: usize,
) -> Result<&'static crate::embedding_api::OpenRouterClient, AppError> {
    if let Some(c) = OPENROUTER_CLIENT.get() {
        return Ok(c);
    }
    let client = crate::embedding_api::OpenRouterClient::new(api_key, model.to_string(), dim)?;
    let _ = OPENROUTER_CLIENT.set(client);
    OPENROUTER_CLIENT.get().ok_or_else(|| {
        AppError::Embedding(crate::i18n::validation::embedding_openrouter_client_unavailable())
    })
}

/// v1.0.95 (ADR-0054): initialises the process-wide OpenRouter chat client on
/// first use and returns it. `model` is the text model the enrich JUDGE will
/// call (no default; the caller validates presence upfront).
pub fn get_openrouter_chat_client(
    api_key: secrecy::SecretBox<String>,
    model: &str,
    timeout_secs: u64,
) -> Result<&'static crate::chat_api::OpenRouterChatClient, AppError> {
    if let Some(c) = OPENROUTER_CHAT_CLIENT.get() {
        return Ok(c);
    }
    let client =
        crate::chat_api::OpenRouterChatClient::new(api_key, model.to_string(), timeout_secs)?;
    let _ = OPENROUTER_CHAT_CLIENT.set(client);
    OPENROUTER_CHAT_CLIENT.get().ok_or_else(|| {
        AppError::Embedding(
            crate::i18n::validation::embedding_openrouter_chat_client_unavailable(),
        )
    })
}

/// v1.0.95: returns the process-wide OpenRouter chat client if it has already
/// been initialised via [`get_openrouter_chat_client`]. Used by the enrich
/// JUDGE dispatch, which initialises the singleton once at startup and then
/// fetches it per item without re-threading the API key.
pub fn openrouter_chat_client() -> Option<&'static crate::chat_api::OpenRouterChatClient> {
    OPENROUTER_CHAT_CLIENT.get()
}

/// ADR-0042 / GAP-002: route a single passage through the Claude
/// embedder. Used by the Claude arm of `embed_via_backend` so the
/// fallback chain stops treating Claude as a synonym for codex.
pub fn embed_via_claude_local(
    _models_dir: &Path,
    text: &str,
    claude_binary: Option<&Path>,
    claude_model: Option<&str>,
) -> Result<Vec<f32>, AppError> {
    let _slot_guard = acquire_llm_slot_for_embedding()?;
    let embedder = get_claude_embedder(claude_binary, claude_model)?;
    embed_passage(embedder, text)
}

/// BUG-003 / v1.0.85: split of  that also
/// reports the resolved []. Always  because
/// this path constructs a Claude-flavoured embedder via
///  (no PATH probe, no silent substitution).
pub fn embed_via_claude_local_resolved(
    _models_dir: &Path,
    text: &str,
    claude_binary: Option<&Path>,
    claude_model: Option<&str>,
) -> Result<(Vec<f32>, LlmBackendKind), AppError> {
    let _slot_guard = acquire_llm_slot_for_embedding()?;
    let embedder = get_claude_embedder(claude_binary, claude_model)?;
    let v = embed_passage(embedder, text)?;
    Ok((v, LlmBackendKind::Claude))
}

/// GAP-OPENCODE-001 / v1.0.90: route a single passage through the OpenCode
/// embedder, reporting the resolved [`LlmBackendKind::Opencode`]. Constructs
/// an OpenCode-flavoured embedder via `with_opencode_builder` (no PATH probe,
/// no silent substitution).
pub fn embed_via_opencode_local_resolved(
    _models_dir: &Path,
    text: &str,
    opencode_binary: Option<&Path>,
    opencode_model: Option<&str>,
) -> Result<(Vec<f32>, LlmBackendKind), AppError> {
    let _slot_guard = acquire_llm_slot_for_embedding()?;
    let embedder = get_opencode_embedder(opencode_binary, opencode_model)?;
    let v = embed_passage(embedder, text)?;
    Ok((v, LlmBackendKind::Opencode))
}
/// Clones the embedding-client configuration. The lock is held only for
/// the duration of the clone — NEVER across I/O (G42/A3).
pub(crate) fn clone_client(embedder: &Mutex<LlmEmbedding>) -> LlmEmbedding {
    embedder.lock().clone()
}

// When true, embed_passage/embed_query use the short query timeout so Auto
// chains fail fast into FTS (GAP-E2E-06).
thread_local! {
    static QUERY_EMBED_FAST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn with_query_embed_fast<T>(f: impl FnOnce() -> T) -> T {
    QUERY_EMBED_FAST.with(|c| {
        let prev = c.replace(true);
        let out = f();
        c.set(prev);
        out
    })
}

pub(crate) fn apply_query_timeout_if_needed(client: LlmEmbedding) -> LlmEmbedding {
    if QUERY_EMBED_FAST.with(|c| c.get()) {
        let secs = crate::runtime_config::resolve_u64(
            None,
            "llm.query_embed_timeout_secs",
            crate::constants::DEFAULT_QUERY_EMBED_TIMEOUT_SECS,
        );
        client.with_timeout_secs(secs)
    } else {
        client
    }
}
