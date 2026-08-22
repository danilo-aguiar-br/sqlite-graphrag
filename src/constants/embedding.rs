//! Embedding dimensionality, batching and vector-cache tuning.
//!
//! Split out of the former single-file `constants.rs` in v1.2.5;
//! every item is re-exported by the parent module, so `crate::constants::X`
//! resolves exactly as before.

/// Default embedding vector dimensionality for a NEWLY created database.
///
/// Sized for `qwen/qwen3-embedding-8b`, the model the OpenRouter REST backend
/// uses today. Matryoshka Representation Learning (MRL, arXiv 2205.13147) lets
/// a prefix of the native vector stand on its own, so 1024 is a real truncation
/// point rather than a lossy resize.
///
/// This value governs `init` only. An existing database keeps the width
/// recorded in `schema_meta.dim`, which [`crate::storage::connection`] adopts on
/// every open — so raising this default can never silently reinterpret vectors
/// already on disk. Widening a populated database is a deliberate migration
/// that must re-embed every row; the previous default was 384, generated
/// against `multilingual-e5-small`.
///
/// Precedence for the active dim is documented on [`embedding_dim`].
pub const DEFAULT_EMBEDDING_DIM: usize = 1024;

// `DEFAULT_QUERY_EMBED_TIMEOUT_SECS` lived here until v1.2.3, at 3 seconds.
// Its doc justified the short budget with "dead OAuth falls back to FTS
// quickly" — a chain the product no longer has. The mechanism that consumed it
// (`apply_query_timeout_if_needed`, `with_timeout_secs`) went out with the
// headless backends, leaving a constant nothing read and a documented XDG key
// nothing honoured, while the operator kept being told a per-query budget
// existed. There is now ONE embedding budget, resolved as
// `--openrouter-timeout` > XDG `embedding.timeout_secs` >
// `DEFAULT_EMBEDDING_HTTP_TIMEOUT_SECS`, and it governs reads and writes alike.

/// Accepted range for any embedding dimensionality, override or recorded.
///
/// Declared once because the bound is checked on the CLI/XDG override, on the
/// value adopted from `schema_meta`, and in the warning text. Three separate
/// literals would be three chances to drift.
pub const EMBEDDING_DIM_RANGE: std::ops::RangeInclusive<usize> = 8..=4096;

/// Active embedding dimensionality for this process. `0` means unresolved.
static ACTIVE_EMBEDDING_DIM: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Resolves the active embedding dimensionality (single source of truth).
///
/// Precedence (G-T-XDG-04):
/// 1. CLI `--embedding-dim` / XDG `embedding.dim` via [`crate::runtime_config`];
/// 2. the value recorded via [`set_active_embedding_dim`] — from `schema_meta`;
/// 3. [`DEFAULT_EMBEDDING_DIM`].
pub fn embedding_dim() -> usize {
    if let Some(dim) = embedding_dim_from_runtime() {
        return dim;
    }
    let active = ACTIVE_EMBEDDING_DIM.load(std::sync::atomic::Ordering::Acquire);
    if active != 0 {
        return active;
    }
    DEFAULT_EMBEDDING_DIM
}

/// Reads the CLI `--embedding-dim` flag or the XDG key `embedding.dim`.
///
/// Values outside [`EMBEDDING_DIM_RANGE`] are rejected with a warning rather
/// than clamped: a clamped width would still mismatch the stored vectors, and
/// `cosine_similarity` reports a dimension mismatch as `0.0` with no error, so
/// the search would go quiet instead of failing.
pub fn embedding_dim_from_runtime() -> Option<usize> {
    let n = crate::runtime_config::embedding_dim_override()? as usize;
    if EMBEDDING_DIM_RANGE.contains(&n) {
        Some(n)
    } else {
        tracing::warn!(
            value = n,
            min = *EMBEDDING_DIM_RANGE.start(),
            max = *EMBEDDING_DIM_RANGE.end(),
            "embedding.dim override out of range; ignoring"
        );
        None
    }
}

/// Records the dimensionality found in the opened database (`schema_meta.dim`).
///
/// Out-of-range values are ignored. A CLI flag or XDG override still wins over
/// this value — see the precedence documented on [`embedding_dim`].
pub fn set_active_embedding_dim(dim: usize) {
    if EMBEDDING_DIM_RANGE.contains(&dim) {
        ACTIVE_EMBEDDING_DIM.store(dim, std::sync::atomic::Ordering::Release);
    }
}

/// Batch size for `fastembed` encoding calls.
pub const FASTEMBED_BATCH_SIZE: usize = 32;

/// GAP-SG-141 (B1): how many `ReEmbed` queue rows a single claim takes.
///
/// Deliberately aligned with the 32-item chunk width that
/// [`crate::embedder::embed_passages_parallel_shared`] uses
/// internally on the OpenRouter path: with 32 or fewer texts that function
/// issues exactly ONE serial REST call, so one claim becomes one request.
/// Raising this above 32 splits the claim into several requests again and
/// buys nothing; the range clamp below still allows it for hosts that
/// deliberately trade request count for claim overhead.
///
/// Override via XDG `enrich.reembed_claim_batch`.
pub const DEFAULT_REEMBED_CLAIM_BATCH: usize = 32;

/// Accepted range for `enrich.reembed_claim_batch`.
///
/// The floor of 1 degenerates to the historical one-row-per-claim behaviour.
/// The ceiling bounds how many rows a single worker can strand in
/// `processing` if it dies mid-batch.
pub const REEMBED_CLAIM_BATCH_RANGE: std::ops::RangeInclusive<usize> = 1..=256;

/// Prefix prepended to bodies before embedding as required by E5 models.
pub const PASSAGE_PREFIX: &str = "passage: ";

/// Prefix prepended to queries before embedding as required by E5 models.
pub const QUERY_PREFIX: &str = "query: ";

/// Maximum tokens accepted by an embedding input before chunking.
pub const EMBEDDING_MAX_TOKENS: usize = 512;

/// Maximum token count for a SINGLE embedding request input (GAP-SG-02).
///
/// The `qwen/qwen3-embedding-8b` model used by the OpenRouter backend accepts
/// roughly 32K tokens of context. This ceiling rejects an input above a safe
/// margin BEFORE the HTTP request, using the conservative cl100k_base proxy in
/// [`crate::tokenizer::count_tokens`] (which emits at least as many tokens as
/// Qwen for the same text). Distinct from [`EMBEDDING_MAX_TOKENS`] (512), which
/// is the per-chunk ceiling that drives chunking.
pub const EMBEDDING_REQUEST_MAX_TOKENS: usize = 30_000;

/// Default total per-request budget, in seconds, for an OpenRouter embeddings
/// HTTP call when neither `--openrouter-timeout` nor XDG
/// `embedding.timeout_secs` supplies a value (GAP-SG-141 B3).
///
/// Deliberately far below the chat-side budget: an embeddings response is a
/// fixed-size vector, so a call that has not returned within this window is
/// stalled rather than slow.
pub const DEFAULT_EMBEDDING_HTTP_TIMEOUT_SECS: u64 = 30;

/// Lower bound on Tokio worker threads for the shared embedding runtime
/// (GAP-SG-141 B2).
///
/// Two threads keep a blocking `block_on` caller from starving the reactor on a
/// single-core host, which is the historical hard-coded value.
pub const EMBED_RUNTIME_MIN_WORKER_THREADS: usize = 2;

/// Upper bound on Tokio worker threads for the shared embedding runtime
/// (GAP-SG-141 B2).
///
/// The runtime only drives HTTP polling, so worker threads past this point add
/// scheduling overhead without adding throughput; the concurrency that matters
/// is the request fan-out (`--rest-concurrency`, `--llm-parallelism`), not the
/// reactor width.
pub const EMBED_RUNTIME_MAX_WORKER_THREADS: usize = 8;

/// DEFAULT entry ceiling for the process-wide entity-embedding cache.
///
/// The cache used to be an unbounded `HashMap`: a long `ingest` over a corpus
/// with many distinct entity names grew it for the whole invocation with no
/// eviction, so its memory was bounded only by the corpus. At 1024 dims one
/// vector costs ~4 KiB, so 10 000 entries is ~40 MiB — the point where the
/// cache stops paying for itself.
///
/// Read it through [`entity_embed_cache_max_entries`], never directly.
pub const ENTITY_EMBED_CACHE_MAX_ENTRIES: usize = 10_000;

/// Entity-cache entry ceiling: XDG `embedding.entity_cache_max_entries` or
/// [`ENTITY_EMBED_CACHE_MAX_ENTRIES`]. `0` falls back to the default.
pub fn entity_embed_cache_max_entries() -> usize {
    crate::config::get_setting("embedding.entity_cache_max_entries")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(ENTITY_EMBED_CACHE_MAX_ENTRIES)
}

/// DEFAULT time-to-live, in seconds, of one entity-embedding cache entry.
///
/// A cached vector is only valid while the embedding model and dimensionality
/// behind it are unchanged. The key already carries both, so the TTL guards the
/// other axis: a long-running drain must not keep a vector alive for hours after
/// its source text stopped being relevant.
///
/// Read it through [`entity_embed_cache_ttl_secs`], never directly.
pub const ENTITY_EMBED_CACHE_TTL_SECS: u64 = 3_600;

/// Entity-cache TTL in seconds: XDG `embedding.entity_cache_ttl_secs` or
/// [`ENTITY_EMBED_CACHE_TTL_SECS`]. `0` falls back to the default.
pub fn entity_embed_cache_ttl_secs() -> u64 {
    crate::config::get_setting("embedding.entity_cache_ttl_secs")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(ENTITY_EMBED_CACHE_TTL_SECS)
}

/// Extra timeout, in seconds, granted per item beyond the first in a batched
/// embedding call (GAP-4).
///
/// The base budget is the already-configurable `embedding.timeout_secs`; this
/// is only how that budget scales with batch width, so it takes no key of its
/// own — a second knob governing the same deadline would be two ways to say
/// one thing.
pub const EMBED_TIMEOUT_PER_EXTRA_BATCH_ITEM_SECS: u64 = 15;

/// Pause, in milliseconds, before retrying a query embedding that lost the race
/// for an LLM slot.
///
/// Long enough for a sibling invocation to finish and release its slot, short
/// enough to stay inside the query path's own budget. Contention backoff, not a
/// deadline, so it takes no XDG key.
pub const EMBED_SLOT_RETRY_DELAY_MS: u64 = 750;

/// Lowest REST fan-out width the batched passage embedder will use
/// (v1.2.8, plan step 6).
///
/// One means serial: a single batch is one REST call, so the `JoinSet` would
/// only add latency. Zero would mean "no worker", which is not a narrower
/// fan-out but an absent one, so the floor is a refusal and not a preference.
pub const MIN_EMBED_PASSAGE_FAN_OUT: usize = 1;

/// Highest REST fan-out width the batched passage embedder will use
/// (v1.2.8, plan step 6).
///
/// Kept inside the range Cloudflare tolerates in front of OpenRouter, and
/// deliberately equal to [`crate::constants::MAX_ENRICH_REST_CONCURRENCY`]:
/// both bound concurrent requests against the SAME host-scoped quota, whose key
/// lives once in `~/.config/sqlite-graphrag/config.toml` and is spent by every
/// folder on the machine. The joint ceiling from
/// [`crate::constants::joint_parallelism_ceiling`] still applies on top, because
/// only the PRODUCT of process count and per-process width describes the load.
pub const MAX_EMBED_PASSAGE_FAN_OUT: usize = 16;
