//! Embedding generation for the GraphRAG memory.
//!
//! v1.0.76: the default build is **LLM-only** — the binary does NOT bundle
//! fastembed / ort / ndarray / tokenizers. All embeddings are produced
//! by the OpenRouter REST embeddings API and stored as a BLOB in
//! `memory_embeddings(memory_id, embedding, source)`. Vector similarity is
//! computed in pure Rust at query time.
//!
//! # Workload classification (G42/S3, BLOCK 1 — MANDATORY)
//!
//! LLM embedding is **I/O-bound**: each call waits on a network
//! round-trip to the OpenRouter REST API while the local CPU stays
//! idle. Concurrency
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
//! `LLM_WORKER_RSS_MB = 350` (`crate::constants`): the historical
//! per-worker RSS budget, retained as the RAM bound on the permit
//! formula.
//!
use std::sync::OnceLock;

/// Process-wide OpenRouter embedding client.
pub(crate) static OPENROUTER_CLIENT: OnceLock<crate::embedding_api::OpenRouterClient> =
    OnceLock::new();

/// v1.0.95 (ADR-0054): process-wide OpenRouter chat-completions client for
/// the `enrich` JUDGE. Distinct from `OPENROUTER_CLIENT` (embeddings) because
/// the chat client binds a text model, not an embedding model.
pub(crate) static OPENROUTER_CHAT_CLIENT: OnceLock<crate::chat_api::OpenRouterChatClient> =
    OnceLock::new();

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
    chunk_embed_batch_size, effective_permits, embed_entity_texts_cached, entity_embed_batch_size,
    EmbedCacheStats, CHUNK_EMBED_BATCH_SIZE, EMBED_BATCH_CALIBRATION_DIM, ENTITY_EMBED_BATCH_SIZE,
};

// GAP-SG-163: see the note in `batch/mod.rs` — the allow belongs to the
// re-export so the warning lands on external callers, not on us.
#[allow(deprecated)]
pub use batch::embed_passages_parallel_with_embedding_choice;
// GAP-SG-147: zero-copy entry point for in-crate callers that already own the
// corpus. Deliberately not `pub`: the borrowed-slice wrapper above stays the
// public surface so no downstream signature breaks.
pub(crate) use batch::embed_passages_parallel_shared;
pub use fallback::{
    classify_embedding_error, embed_with_fallback, try_embed_query_with_deterministic_fallback,
    try_embed_query_with_fallback, EmbeddingErrorKind, FallbackReason,
};
pub use getters::{
    get_openrouter_chat_client, get_openrouter_embedder, is_openrouter_initialized,
    openrouter_chat_client,
};
pub use passage::{
    embed_passage_or_skip, embed_passage_with_choice, embed_passage_with_embedding_choice,
    should_skip_embedding_on_failure, try_embed_query_with_choice,
    try_embed_query_with_embedding_choice,
};

// Crate-visible helpers used across submodules.
pub(crate) use backend::backend_ready_probe;
pub(crate) use getters::shared_runtime;

#[cfg(test)]
#[path = "../embedder_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../embedder_fallback_tests.rs"]
mod embed_with_fallback_tests;
