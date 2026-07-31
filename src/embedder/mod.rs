//! Embedding generation for the GraphRAG memory.
//!
//! v1.0.76: the default build is **LLM-only** — the binary does NOT bundle
//! fastembed / ort / ndarray / tokenizers. All embeddings are produced
//! by a headless invocation of `claude code` or `codex` (OAuth, no MCP,
//! no hooks) and stored as a BLOB in `memory_embeddings(memory_id, embedding,
//! source)`. Vector similarity is computed in pure Rust at query time.
//!
//! # Workload classification (G42/S3, BLOCK 1 — MANDATORY)
//!
//! LLM embedding is **I/O-bound + subprocess-bound**: each call waits
//! 5-60s on a network round-trip through a headless `claude -p` /
//! `codex exec` subprocess while the local CPU stays idle. Concurrency
//! therefore uses **tokio** (async I/O concurrency) and NEVER rayon
//! (reserved for CPU-bound work).
//!
//! # Permit formula (G42/S3, BLOCO 2)
//!
//! ```text
//! permits = clamp(--llm-parallelism, 1, 32)
//!           .min(available_parallelism())
//!           .min(available_ram_mb * 0.5 / LLM_WORKER_RSS_MB)
//! ```
//!
//! `LLM_WORKER_RSS_MB = 350` (`crate::constants`): `claude -p` and
//! `codex exec` are node processes with a typical Maximum RSS of
//! 200-400 MB (measured via `/usr/bin/time -l` on macOS /
//! `/usr/bin/time -v` on Linux), so the RAM bound is pertinent.
//!
//! # Locking contract (G42/A3 fix)
//!
//! The process-wide `Mutex<LlmEmbedding>` protects ONLY the cheap clone
//! of the client configuration (flavour + binary path + model + shared
//! schema tempfiles). It is NEVER held across network I/O — the
//! v1.0.76-v1.0.78 `flush_group` held it for the whole sequential
//! embedding loop, which is why `--llm-parallelism 8` measured an
//! effective parallelism of 1.

use crate::extract::llm_embedding::LlmEmbedding;
use parking_lot::Mutex;
use std::sync::OnceLock;

/// Process-wide LLM-embedding client behind a Mutex.
///
/// The lock guards configuration cloning only (see module docs); the
/// actual LLM I/O happens on clones, outside the lock.
///
/// ADR-0042 / GAP-002: process-wide Claude-backed LLM-embedding client
/// behind a `Mutex`. Distinct from `EMBEDDER` so the Claude path of
/// `embed_via_backend` no longer re-probes PATH via `detect_available`
/// (the v1.0.82 bug where requesting Claude could resolve to Codex).
pub(crate) static CLAUDE_EMBEDDER: OnceLock<Mutex<LlmEmbedding>> = OnceLock::new();
pub(crate) static OPENCODE_EMBEDDER: OnceLock<Mutex<LlmEmbedding>> = OnceLock::new();
pub(crate) static OPENROUTER_CLIENT: OnceLock<crate::embedding_api::OpenRouterClient> =
    OnceLock::new();

/// v1.0.95 (ADR-0054): process-wide OpenRouter chat-completions client for
/// the `enrich` JUDGE. Distinct from `OPENROUTER_CLIENT` (embeddings) because
/// the chat client binds a text model, not an embedding model.
pub(crate) static OPENROUTER_CHAT_CLIENT: OnceLock<crate::chat_api::OpenRouterChatClient> =
    OnceLock::new();

pub(crate) static EMBEDDER: OnceLock<Mutex<LlmEmbedding>> = OnceLock::new();

/// Process-wide multi-thread tokio runtime for embedding I/O.
///
/// G42/A2 fix: v1.0.76-v1.0.78 built a current-thread runtime PER CALL.
/// One runtime per process amortises the setup and hosts the bounded
/// fan-out of `embed_texts_parallel`.
pub(crate) static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

// Batch sizing + parallel fan-out (R-SRP-01).
mod backend;
mod batch;
mod fallback;
mod getters;
mod passage;

pub use backend::{
    bytes_to_f32, embed_via_backend, embed_via_backend_legacy, embed_via_backend_strict,
    embedding_dim, f32_to_bytes, LlmBackendKind,
};
pub use batch::{
    chunk_embed_batch_size, effective_permits, embed_entity_texts_cached,
    embed_passages_controlled_local, embed_passages_parallel_local,
    embed_passages_parallel_with_embedding_choice, embed_texts_parallel, embed_texts_parallel_with,
    entity_embed_batch_size, EmbedCacheStats, CHUNK_EMBED_BATCH_SIZE, EMBED_BATCH_CALIBRATION_DIM,
    ENTITY_EMBED_BATCH_SIZE,
};
pub use fallback::{
    classify_embedding_error, embed_with_fallback, try_embed_query_with_deterministic_fallback,
    try_embed_query_with_fallback, EmbeddingErrorKind, FallbackReason,
};
pub use getters::{
    embed_via_claude_local, embed_via_claude_local_resolved, embed_via_opencode_local_resolved,
    get_claude_embedder, get_embedder, get_opencode_embedder, get_openrouter_chat_client,
    get_openrouter_embedder, is_openrouter_initialized, openrouter_chat_client,
};
pub use passage::{
    embed_passage, embed_passage_local, embed_passage_local_resolved, embed_passage_or_skip,
    embed_passage_with_choice, embed_passage_with_embedding_choice, embed_passages_controlled,
    embed_query, embed_query_local, should_skip_embedding_on_failure, try_embed_query_with_choice,
    try_embed_query_with_embedding_choice,
};

// Crate-visible helpers used across submodules.
pub(crate) use backend::{backend_ready_probe, validate_dim};
pub(crate) use getters::{
    apply_query_timeout_if_needed, clone_client, shared_runtime, with_query_embed_fast,
};
pub(crate) use passage::acquire_llm_slot_for_embedding;

#[cfg(test)]
#[path = "../embedder_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../embedder_fallback_tests.rs"]
mod embed_with_fallback_tests;
