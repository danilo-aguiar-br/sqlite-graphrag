//! Process-wide entity-embedding cache and its stats snapshot (G56).
//!
//! Entity names repeat heavily across a corpus, so this module memoises
//! `(model, text)` pairs for the lifetime of one CLI invocation and only sends
//! the misses to the backend-aware batch path.

use super::passages::embed_passages_parallel_shared;
use super::sizing::entity_embed_batch_size;
use crate::embedder::{is_openrouter_initialized, LlmBackendKind};
use crate::errors::AppError;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;

/// G56: in-process cache for entity embeddings keyed by `(model, text)`.
///
/// Schema v13 is immutable: `entity_embeddings` does not have a `text`
/// column, so a pure DB-side cache would require a schema bump. Instead
/// we keep a process-wide LRU-style map that survives within one CLI
/// invocation. The hit rate is high in `ingest` (re-embedding the same
/// canonical entity across thousands of memories) and modest in `remember`
/// (typical single-memory invocations).
///
/// Key: `blake3(model || "\0" || text)`. Value: the vector plus the instant it
/// was stored, behind an `Arc` so eviction can drop the map entry while a `Vec`
/// is still in flight.
///
/// # Bounded since v1.2.3
///
/// The map used to be unbounded and untimed: an `ingest` over a corpus with many
/// distinct entity names grew it for the whole invocation with nothing but the
/// corpus size to stop it. Both bounds now come from
/// [`crate::constants::entity_embed_cache_max_entries`] and
/// [`crate::constants::entity_embed_cache_ttl_secs`], each backed by an XDG key.
/// One memoised entity vector and the instant it entered the cache.
struct CacheEntry {
    vector: Arc<Vec<f32>>,
    stored_at: std::time::Instant,
}

/// The bounded map itself.
///
/// It keeps the `HashMap`-shaped [`Self::insert`] / [`Self::get`] pair the
/// callers already use, so the timestamp stays an implementation detail: no
/// caller has to remember to stamp an entry, and none can read an expired one.
#[derive(Default)]
pub(crate) struct EntityEmbedCacheMap {
    entries: std::collections::HashMap<u64, CacheEntry>,
}

impl EntityEmbedCacheMap {
    /// Stores `vector` under `key`, stamped with the current instant.
    pub(crate) fn insert(&mut self, key: u64, vector: Arc<Vec<f32>>) {
        self.entries.insert(
            key,
            CacheEntry {
                vector,
                stored_at: std::time::Instant::now(),
            },
        );
    }

    /// Returns the vector for `key` while it is still inside its TTL.
    ///
    /// An expired entry reads as absent instead of being removed here: the read
    /// path only holds a shared borrow, and eviction belongs to the write path
    /// (see [`Self::evict_expired_and_overflow`]).
    pub(crate) fn get(&self, key: &u64) -> Option<&Arc<Vec<f32>>> {
        let ttl = std::time::Duration::from_secs(crate::constants::entity_embed_cache_ttl_secs());
        let now = std::time::Instant::now();
        self.entries
            .get(key)
            .filter(|entry| now.duration_since(entry.stored_at) < ttl)
            .map(|entry| &entry.vector)
    }

    /// Number of entries currently held, expired ones included.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Drops expired entries, then trims back far enough to fit `incoming`.
    ///
    /// Eviction is oldest-first by insertion instant, which is the honest
    /// ordering available here: a true LRU would need a read timestamp updated
    /// under the same lock on every hit, and paying a write on the hot path to
    /// protect a cache whose whole point is to avoid work is the wrong trade.
    /// Called right before an insert batch — the only moment the map grows.
    pub(crate) fn evict_expired_and_overflow(&mut self, incoming: usize) {
        let ttl = std::time::Duration::from_secs(crate::constants::entity_embed_cache_ttl_secs());
        let now = std::time::Instant::now();
        self.entries
            .retain(|_, entry| now.duration_since(entry.stored_at) < ttl);

        let ceiling = crate::constants::entity_embed_cache_max_entries();
        // Room the incoming batch needs. A batch larger than the whole ceiling
        // can only be served by clearing everything; it still gets its vectors,
        // they just do not all survive in the cache.
        let target = ceiling.saturating_sub(incoming.min(ceiling));
        if self.entries.len() <= target {
            return;
        }
        let mut by_age: Vec<(u64, std::time::Instant)> = self
            .entries
            .iter()
            .map(|(key, entry)| (*key, entry.stored_at))
            .collect();
        by_age.sort_by_key(|(_, stored_at)| *stored_at);
        for (key, _) in by_age.into_iter().take(self.entries.len() - target) {
            self.entries.remove(&key);
        }
    }
}

static ENTITY_EMBED_CACHE: OnceLock<parking_lot::Mutex<EntityEmbedCacheMap>> = OnceLock::new();

pub(crate) fn entity_embed_cache() -> &'static parking_lot::Mutex<EntityEmbedCacheMap> {
    ENTITY_EMBED_CACHE.get_or_init(|| parking_lot::Mutex::new(EntityEmbedCacheMap::default()))
}

pub(crate) fn entity_cache_key(model: &str, text: &str) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(model.as_bytes());
    hasher.update(b"\0");
    hasher.update(text.as_bytes());
    let h = hasher.finalize();
    let bytes = h.as_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// G56: embeds entity-name texts through a process-wide cache.
///
/// Skips any `(model, text)` pair already produced in this CLI invocation
/// and only spawns subprocesses for the cache misses. Returns vectors in
/// the same order as `texts`.
///
/// Designed for entity-name batches (short texts). For chunk embeds use
/// [`super::embed_passages_parallel_local`] directly — chunks are unique per
/// memory and cache hit rate is negligible.
pub fn embed_entity_texts_cached(
    models_dir: &Path,
    texts: &[String],
    parallelism: usize,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
    llm_backend: crate::cli::LlmBackendChoice,
) -> Result<(Vec<Vec<f32>>, EmbedCacheStats), AppError> {
    if texts.is_empty() {
        return Ok((Vec::new(), EmbedCacheStats::default()));
    }
    // GAP-OR-ENTITY-EMBED: resolve the SAME chain the chunk path uses so the
    // entity embedding honours `--embedding-backend`/`--llm-backend` instead
    // of always forcing the codex subprocess (the old G56 code path).
    let chain = embedding_backend.to_chain(llm_backend);

    // `none` short-circuit: when the resolved chain is exactly `[None]`
    // (`--embedding-backend llm --llm-backend none`) skip every backend and
    // return empty vectors WITHOUT spawning a subprocess. Empties are never
    // cached so a later call with a real backend in the same process is not
    // poisoned; they count as misses for stats parity with the chunk path.
    if chain.as_slice() == [LlmBackendKind::None] {
        let out: Vec<Vec<f32>> = texts.iter().map(|_| Vec::new()).collect();
        return Ok((
            out,
            EmbedCacheStats {
                requested: texts.len(),
                hits: 0,
                misses: texts.len(),
            },
        ));
    }

    // Cache model label reflects the EFFECTIVE embedding backend: vectors
    // carry that model's dim/MRL profile, so the key must not collide across
    // dimensionalities. This cache is process-local.
    let routed_openrouter =
        chain.first() == Some(&LlmBackendKind::OpenRouter) && is_openrouter_initialized();
    let model = if routed_openrouter {
        format!("openrouter:{}", crate::constants::embedding_dim())
    } else {
        format!("none:{}", crate::constants::embedding_dim())
    };
    let cache = entity_embed_cache();
    let mut hits: Vec<Option<Arc<Vec<f32>>>> = vec![None; texts.len()];
    let mut miss_indices: Vec<usize> = Vec::with_capacity(texts.len());
    {
        let guard = cache.lock();
        for (i, text) in texts.iter().enumerate() {
            let key = entity_cache_key(&model, text);
            // `get` already filters out entries past their TTL.
            match guard.get(&key) {
                Some(vector) => hits[i] = Some(Arc::clone(vector)),
                None => miss_indices.push(i),
            }
        }
    }
    let miss_count = miss_indices.len();
    if miss_count > 0 {
        let miss_texts: Vec<String> = miss_indices.iter().map(|&i| texts[i].clone()).collect();
        // GAP-OR-ENTITY-EMBED: route misses through the backend-aware batch
        // helper (same one the chunk path uses). With OpenRouter this hits the
        // REST `embed_batch` (~200ms) instead of the codex subprocess (~120s).
        let mut miss_vecs = embed_passages_parallel_shared(
            models_dir,
            Arc::from(miss_texts),
            parallelism,
            entity_embed_batch_size(),
            embedding_backend,
            llm_backend,
        )?;
        let mut guard = cache.lock();
        guard.evict_expired_and_overflow(miss_count);
        for (slot, &orig_idx) in miss_indices.iter().enumerate() {
            // MOVE the freshly produced vector into the `Arc` instead of
            // cloning it: the batch result is dead after this loop, so the copy
            // it used to make was a full duplicate of every miss vector.
            let vector = Arc::new(std::mem::take(&mut miss_vecs[slot]));
            let key = entity_cache_key(&model, &texts[orig_idx]);
            guard.insert(key, Arc::clone(&vector));
            hits[orig_idx] = Some(vector);
        }
    }
    let mut out = Vec::with_capacity(texts.len());
    for hit in hits.into_iter() {
        let v = hit.ok_or_else(|| {
            AppError::Embedding(crate::i18n::validation::embedding_entity_cache_null())
        })?;
        out.push((*v).clone());
    }
    Ok((
        out,
        EmbedCacheStats {
            requested: texts.len(),
            hits: texts.len() - miss_count,
            misses: miss_count,
        },
    ))
}

/// G56: stats snapshot returned by [`embed_entity_texts_cached`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct EmbedCacheStats {
    /// Requested.
    pub requested: usize,
    /// Hits.
    pub hits: usize,
    /// Misses.
    pub misses: usize,
}

impl EmbedCacheStats {
    /// Hit rate as a fraction in `[0.0, 1.0]`. Returns 0.0 when nothing was requested.
    pub fn hit_rate(&self) -> f64 {
        if self.requested == 0 {
            0.0
        } else {
            self.hits as f64 / self.requested as f64
        }
    }
}
