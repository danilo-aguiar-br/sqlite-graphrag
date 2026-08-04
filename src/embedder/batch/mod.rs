//! Batch sizing, bounded parallel fan-out and entity-embed cache.
//!
//! G42/S2–S3 / G44 / G56: adaptive batch sizes, `Arc<Semaphore>` permit
//! accounting, OpenRouter REST batch fan-out, and the process-wide entity
//! embedding cache. Split from the root embedder module (R-SRP-01) so the
//! single-vector paths stay independent of the multi-text orchestration.
//!
//! Responsibilities are one per submodule: [`sizing`] decides how much work
//! goes into a unit, [`fan_out`] runs those units concurrently, [`passages`]
//! is the corpus-level entry point, and [`entity_cache`] memoises entity-name
//! vectors for the life of the process.

mod entity_cache;
mod fan_out;
mod passages;
mod sizing;

pub use entity_cache::{embed_entity_texts_cached, EmbedCacheStats};

// GAP-SG-163: the shim stays re-exported so external consumers keep compiling,
// but the deprecation must reach them rather than warn us about our own
// re-export. The allow is scoped to the re-export, never to the definition.
#[allow(deprecated)]
pub use passages::embed_passages_parallel_with_embedding_choice;
pub use sizing::{
    chunk_embed_batch_size, effective_permits, entity_embed_batch_size, CHUNK_EMBED_BATCH_SIZE,
    EMBED_BATCH_CALIBRATION_DIM, ENTITY_EMBED_BATCH_SIZE,
};

pub(crate) use passages::embed_passages_parallel_shared;

// Test-only surface: `src/embedder_tests.rs` reaches these helpers through
// `super::batch::`, and nothing in a release build does.
#[cfg(test)]
pub(crate) use entity_cache::{entity_cache_key, entity_embed_cache};
#[cfg(test)]
pub(crate) use sizing::adaptive_batch_for_dim;

#[cfg(test)]
mod tests;
