//! Compile-time constants shared across the crate.
//!
//! Grouped into embedding configuration, length and size limits, SQLite
//! pragmas and retrieval tuning knobs. Values are taken from the PRD and
//! must stay in sync with the migrations under `migrations/`.
//!
//! ## Dynamic concurrency permit calculation
//!
//! The maximum number of simultaneous instances can be adjusted at runtime
//! using the formula:
//!
//! ```text
//! permits = min(cpus, available_memory_mb / LLM_WORKER_RSS_MB) * 0.5
//! ```
//!
//! where `available_memory_mb` is obtained via `sysinfo::System::available_memory()`
//! converted to MiB. The result is capped at `MAX_CONCURRENT_CLI_INSTANCES`
//! and floored at 1.

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

/// Default tracing filter level when neither CLI `-v`/`-q` nor XDG `log.level`
/// is set (GAP-SG-93).
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Default OpenRouter chat completions endpoint (override via XDG
/// `network.openrouter.chat_url` or alias `network.chat_url`).
pub const DEFAULT_OPENROUTER_CHAT_URL: &str =
    "https://openrouter.ai/api/v1/chat/completions";

/// Default OpenRouter embeddings endpoint (override via XDG
/// `network.openrouter.embeddings_url` or alias `network.embed_url`).
pub const DEFAULT_OPENROUTER_EMBEDDINGS_URL: &str =
    "https://openrouter.ai/api/v1/embeddings";

/// Fail-fast probe budget for LLM backends before spawning (ms).
/// Override via XDG `llm.probe_timeout_ms`.
pub const DEFAULT_LLM_PROBE_TIMEOUT_MS: u64 = 800;

/// Per-call timeout for query embedding (recall/hybrid Auto chain).
/// Short budget so dead OAuth falls back to FTS quickly (GAP-E2E-06).
/// Override via XDG `llm.query_embed_timeout_secs`.
pub const DEFAULT_QUERY_EMBED_TIMEOUT_SECS: u64 = 3;

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

// G46: FASTEMBED_MODEL_DEFAULT removed — the fastembed model was deleted in
// v1.0.76 (LLM-only build); `schema_meta.model` now records the CLI version.

/// Default worker count for the global Rayon pool.
///
/// Each worker holds one batch-embedding call in flight, so a wider pool buys
/// no throughput on the LLM-only path and risks RSS oversubscription on a
/// 4-8 GiB host. Override via XDG `parallelism.rayon_threads`.
pub const DEFAULT_RAYON_THREADS: usize = 2;

/// Batch size for `fastembed` encoding calls.
pub const FASTEMBED_BATCH_SIZE: usize = 32;

/// Maximum byte length for a memory `name` field in kebab-case.
pub const MAX_MEMORY_NAME_LEN: usize = 80;

/// Maximum byte length for an `ingest`-derived kebab-case name.
///
/// Stricter than `MAX_MEMORY_NAME_LEN` (80) to leave headroom for collision
/// suffixes (`-2`, `-10`, ...) when multiple files derive to the same base.
/// Used exclusively by `src/commands/ingest.rs`.
pub const DERIVED_NAME_MAX_LEN: usize = 60;

/// Maximum character length for a memory `description` field.
pub const MAX_MEMORY_DESCRIPTION_LEN: usize = 500;

/// Hard upper bound on memory `body` length in bytes.
pub const MAX_MEMORY_BODY_LEN: usize = 512_000;

/// Body character count above which the body is split into chunks.
pub const MAX_BODY_CHARS_BEFORE_CHUNK: usize = 8_000;

/// Maximum attempts when a statement returns `SQLITE_BUSY`.
pub const MAX_SQLITE_BUSY_RETRIES: u32 = 5;

/// Base delay in milliseconds for the first SQLITE_BUSY retry.
///
/// Each subsequent attempt doubles the delay (exponential backoff):
/// 300 ms → 600 ms → 1200 ms → 2400 ms → 4800 ms (≈ 9.3 s total).
pub const SQLITE_BUSY_BASE_DELAY_MS: u64 = 300;

/// Query timeout applied to statements in milliseconds.
pub const QUERY_TIMEOUT_MILLIS: u64 = 5_000;

/// Jaccard threshold above which two memories are considered fuzzy duplicates.
pub const DEDUP_FUZZY_THRESHOLD: f64 = 0.8;

/// Cosine distance threshold below which two memories are semantic duplicates.
pub const DEDUP_SEMANTIC_THRESHOLD: f32 = 0.1;

/// Maximum number of hops allowed in graph traversals.
pub const MAX_GRAPH_HOPS: u32 = 2;

/// Minimum relationship weight required for traversal inclusion.
pub const MIN_RELATION_WEIGHT: f64 = 0.3;

/// Default traversal depth for `related` when `--hops` is omitted.
pub const DEFAULT_MAX_HOPS: u32 = 2;

/// Default minimum weight filter applied during graph traversal.
pub const DEFAULT_MIN_WEIGHT: f64 = 0.3;

/// Default weight assigned to newly created relationships.
pub const DEFAULT_RELATION_WEIGHT: f64 = 0.5;

/// Default `k` used by `recall` when the caller omits `--k`.
pub const DEFAULT_K_RECALL: usize = 10;

/// Default `k` for memory KNN searches when the caller omits `--k`.
pub const K_MEMORIES_DEFAULT: usize = 10;

/// Default `k` for entity KNN searches during graph expansion.
pub const K_ENTITIES_SEARCH: usize = 5;

/// Default upper bound on distinct entities persisted per memory.
///
/// Bumped from 30 → 50 in v1.0.43 to reduce semantic loss on rich documents.
/// Configurable at runtime via XDG / runtime_config (not product env).
pub const MAX_ENTITIES_PER_MEMORY: usize = 50;

/// Resolves the per-memory entity cap (flag/XDG/`runtime_config`).
///
/// v1.0.43: makes the cap (default 50) configurable without product env.
/// Stress tests showed inputs with 33-46 candidates being truncated at the old cap of 30.
/// Values outside [1, 1000] fall back to the default.
pub fn max_entities_per_memory() -> usize {
    let n = crate::runtime_config::max_entities_per_memory(MAX_ENTITIES_PER_MEMORY);
    if (1..=1_000).contains(&n) {
        n
    } else {
        MAX_ENTITIES_PER_MEMORY
    }
}

/// Upper bound on distinct relationships persisted per memory.
pub const MAX_RELATIONSHIPS_PER_MEMORY: usize = 50;

/// Resolves the per-memory relationship cap (flag/XDG/`runtime_config`).
///
/// v1.0.22: makes the cap (default 50) configurable without product env.
/// Audit found that rich documents silently hit the cap; users with dense technical corpora
/// can raise it via XDG. Values outside [1, 10000] fall back to the default.
pub fn max_relationships_per_memory() -> usize {
    let n = crate::runtime_config::max_relations_per_memory(MAX_RELATIONSHIPS_PER_MEMORY);
    if (1..=10_000).contains(&n) {
        n
    } else {
        MAX_RELATIONSHIPS_PER_MEMORY
    }
}

/// Character length of the description preview shown in `list` output.
pub const TEXT_DESCRIPTION_PREVIEW_LEN: usize = 100;

/// `PRAGMA busy_timeout` value applied on every connection.
pub const BUSY_TIMEOUT_MILLIS: i32 = 5_000;

/// `PRAGMA cache_size` value in kibibytes (negative means KiB).
pub const CACHE_SIZE_KB: i32 = -64_000;

/// `PRAGMA mmap_size` value in bytes applied to each connection.
pub const MMAP_SIZE_BYTES: i64 = 268_435_456;

/// `PRAGMA wal_autocheckpoint` threshold in pages.
pub const WAL_AUTOCHECKPOINT_PAGES: i32 = 1_000;

/// Default `k` constant used by Reciprocal Rank Fusion in `hybrid-search`.
pub const RRF_K_DEFAULT: u32 = 60;

/// Chunk size expressed in tokens for body splitting.
pub const CHUNK_SIZE_TOKENS: usize = 400;

/// Token overlap between consecutive chunks.
pub const CHUNK_OVERLAP_TOKENS: usize = 50;

/// Explicit operational guard for multi-chunk documents in `remember`.
///
/// The multi-chunk path uses serial embeddings to avoid ONNX memory amplification.
/// This limit preserves a clear operational ceiling for agents and scripts.
pub const REMEMBER_MAX_SAFE_MULTI_CHUNKS: usize = 512;

/// Ceiling on chunks per controlled micro-batch in `remember`.
///
/// The `fastembed` runtime uses `BatchLongest` padding, so oversized batches amplify
/// the cost of the longest chunk. This ceiling keeps batches small even when chunks are short.
pub const REMEMBER_MAX_CONTROLLED_BATCH_CHUNKS: usize = 4;

/// Maximum padded-token budget per controlled micro-batch in `remember`.
///
/// The budget uses `max_tokens_no_batch * batch_size`, approximating the real cost of
/// `BatchLongest` padding. Values exceeding this fall back to smaller batches or serialisation.
pub const REMEMBER_MAX_CONTROLLED_BATCH_PADDED_TOKENS: usize = 512;

/// Prefix prepended to bodies before embedding as required by E5 models.
pub const PASSAGE_PREFIX: &str = "passage: ";

/// Prefix prepended to queries before embedding as required by E5 models.
pub const QUERY_PREFIX: &str = "query: ";

/// Crate version string sourced from `CARGO_PKG_VERSION` at build time.
pub const SQLITE_GRAPHRAG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// PRD-canonical regex that validates names and namespaces. Allows 1 char `[a-z0-9]`
/// OR a 2-80 char string starting with a letter and ending with a letter/digit,
/// containing only `[a-z0-9-]`. Rejects the `__` prefix (internal reserved).
pub const NAME_SLUG_REGEX: &str = r"^[a-z][a-z0-9-]{0,78}[a-z0-9]$|^[a-z0-9]$";

static NAME_SLUG_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

/// Returns a reference to the compiled [`NAME_SLUG_REGEX`] pattern.
/// Compiled once on first call, cached via `OnceLock`.
// expect_used (audited v1.0.97): NAME_SLUG_REGEX is a const literal; a parse
// failure would be a compile-reproducible bug, never a runtime condition.
#[allow(clippy::expect_used)]
pub fn name_slug_regex() -> &'static regex::Regex {
    NAME_SLUG_RE.get_or_init(|| {
        regex::Regex::new(NAME_SLUG_REGEX).expect("NAME_SLUG_REGEX is a valid pattern")
    })
}

/// Default retention period (days) used by `purge` when `--retention-days` is omitted.
pub const PURGE_RETENTION_DAYS_DEFAULT: u32 = 90;

/// Maximum number of simultaneously active namespaces (deleted_at IS NULL). Exit 5 when exceeded.
pub const MAX_NAMESPACES_ACTIVE: u32 = 100;

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

/// Initial `max_tokens` budget sent on an `enrich` chat-completion request
/// (GAP-SG-70/71).
///
/// Chosen well below [`ENRICH_MAX_TOKENS_CEILING`] so a well-formed response
/// completes in one attempt for the common case; only bodies that need more
/// room trigger the growth loop below.
pub const ENRICH_INITIAL_MAX_TOKENS: u32 = 4_096;

/// Multiplier applied to `max_tokens` each time OpenRouter reports
/// `finish_reason: "length"` on an `enrich` chat-completion (GAP-SG-70/71).
pub const ENRICH_MAX_TOKENS_GROWTH_FACTOR: u32 = 2;

/// Upper bound on `max_tokens` growth for an `enrich` chat-completion
/// (GAP-SG-70/71).
///
/// Kept with margin under the ~32K-token context ceiling of
/// `deepseek/deepseek-v4-flash:nitro` (see [`EMBEDDING_REQUEST_MAX_TOKENS`]
/// for the equivalent embedding-side ceiling) so growth never requests a
/// budget the model cannot honour.
pub const ENRICH_MAX_TOKENS_CEILING: u32 = 16_384;

/// Maximum number of `max_tokens`-growth re-attempts after a truncated
/// (`finish_reason: "length"`) `enrich` chat-completion, before giving up and
/// returning the truncation as an error (GAP-SG-70/71).
pub const ENRICH_MAX_LENGTH_RETRIES: u32 = 2;

/// Byte budget for one auto-split partition (sub-memory) in `ingest`
/// (GAP-SG-04/07).
///
/// Chosen below the 127 KB body margin so each partition also stays under
/// [`REMEMBER_MAX_SAFE_MULTI_CHUNKS`] chunks and [`EMBEDDING_REQUEST_MAX_TOKENS`]
/// tokens, even for multibyte/CJK text (~1 cl100k token per UTF-8 char, so
/// 80 KiB / 3 bytes-per-char yields about 27K tokens, below the 30K ceiling).
pub const AUTOSPLIT_PARTITION_MAX_BYTES: usize = 80 * 1024;

/// Maximum result count from the recursive graph CTE in `recall`.
pub const K_GRAPH_MATCHES_LIMIT: usize = 20;

/// Default `--limit` for `list` when omitted.
pub const K_LIST_DEFAULT_LIMIT: usize = 100;

/// Default `--limit` for `graph entities` when omitted.
pub const K_GRAPH_ENTITIES_DEFAULT_LIMIT: usize = 50;

/// Default `--limit` for `related` when omitted.
pub const K_RELATED_DEFAULT_LIMIT: usize = 10;

/// Default `--limit` for `history` when omitted.
pub const K_HISTORY_DEFAULT_LIMIT: usize = 20;

/// Default weight for the vector contribution in the `hybrid-search` RRF formula.
pub const WEIGHT_VEC_DEFAULT: f64 = 1.0;

/// Default weight for the BM25 text contribution in the `hybrid-search` RRF formula.
pub const WEIGHT_FTS_DEFAULT: f64 = 1.0;

/// Character size of the body preview emitted in text/markdown formats.
pub const TEXT_BODY_PREVIEW_LEN: usize = 200;

/// Default value injected into ORT_NUM_THREADS when not set by the user.
pub const ORT_NUM_THREADS_DEFAULT: &str = "1";

/// Default value injected into ORT_INTRA_OP_NUM_THREADS when not set.
pub const ORT_INTRA_OP_NUM_THREADS_DEFAULT: &str = "1";

/// Default value injected into OMP_NUM_THREADS when not set by the user.
pub const OMP_NUM_THREADS_DEFAULT: &str = "1";

/// Exit code for partial batch failure (PRD line 1822). Conflicts with DbBusy in v1.x;
/// in v2.0.0 DbBusy migrates to 15 and this code takes 13 per PRD.
pub const BATCH_PARTIAL_FAILURE_EXIT_CODE: i32 = 13;

/// Exit code for DbBusy in v2.0.0 (migrated from 13 to free 13 for batch failure).
pub const DB_BUSY_EXIT_CODE: i32 = 15;

/// Polling interval in milliseconds used by `--wait-lock` between `try_lock_exclusive` attempts.
pub const CLI_LOCK_POLL_INTERVAL_MS: u64 = 500;

/// Process exit code returned when the lock is busy and no wait was requested (EX_TEMPFAIL).
pub const CLI_LOCK_EXIT_CODE: i32 = 75;

/// Maximum number of CLI instances running simultaneously.
///
/// Limits the counting
/// semaphore in [`crate::lock`] to prevent memory overload when multiple parallel
/// v1.0.75 (G18 solution): removed the rigid 4-slot ceiling. The adaptive
/// `calculate_safe_concurrency` function in [`crate::lock`]` now reports
/// the dynamic limit. This constant is preserved as a *legacy fallback*
/// when the dynamic calculation cannot be performed (e.g. when `sysinfo`
/// cannot read `/proc/meminfo`).
///
/// Operators should prefer passing `--max-concurrency` explicitly OR
/// letting the runtime compute the limit. The default ceiling is intentionally
/// higher (16) so the legacy 4-slot hard cap does not silently reappear.
pub const MAX_CONCURRENT_CLI_INSTANCES: usize = 16;

/// G28-B (v1.0.68): polling interval in milliseconds used by
/// `acquire_job_singleton` between retry attempts when another invocation
/// already holds the singleton for `(job_type, namespace)`.
pub const JOB_SINGLETON_POLL_INTERVAL_MS: u64 = 1000;

/// Minimum available memory in MiB required before starting model loading.
///
/// If `sysinfo::System::available_memory() / 1_048_576` falls below this value,
/// the invocation is aborted with [`crate::errors::AppError::LowMemory`]
/// (exit code [`LOW_MEMORY_EXIT_CODE`]).
pub const MIN_AVAILABLE_MEMORY_MB: u64 = 2_048;

/// Maximum process RSS in MiB before aborting embedding operations.
/// Users can override via `--max-rss-mb`. Set to 8 GiB by default.
pub const DEFAULT_MAX_RSS_MB: u64 = 8_192;

/// Maximum time in seconds an instance waits to acquire a concurrency slot.
///
/// Passed as the default for `--max-wait-secs` in the CLI. After exhausting this limit,
/// the invocation returns [`crate::errors::AppError::AllSlotsFull`] with exit code
/// [`CLI_LOCK_EXIT_CODE`] (75).
pub const CLI_LOCK_DEFAULT_WAIT_SECS: u64 = 300;

/// v1.0.75 (G18 + G23): expected RSS in MiB for an LLM-only worker that
/// spawns a `claude -p` or `codex exec` subprocess. Much lower than the
/// embedding cost because the ONNX model is not loaded per-worker.
pub const LLM_WORKER_RSS_MB: u64 = 350;

/// Process exit code returned when available memory is below [`MIN_AVAILABLE_MEMORY_MB`].
///
/// Value `77` is `EX_NOPERM` in glibc sysexits, reused here to indicate
/// "insufficient system resource to proceed".
pub const LOW_MEMORY_EXIT_CODE: i32 = 77;

/// Process exit code returned when a duplicate memory or entity is detected (exit 9).
///
/// Moved from `2` to `9` in v1.0.52 to free exit code `2` for future use and align
/// with the PRD exit code contract. Shell callers and LLM agents must use `9` from
/// this version onwards.
pub const DUPLICATE_EXIT_CODE: i32 = 9;

/// Process exit code returned when shutdown is requested via SIGINT/SIGTERM/SIGHUP
/// (v1.0.82, GAP-002 final).
///
/// The shell sees this code INSTEAD of the legacy `128 + signal` (130/143/129) so
/// that LLM agents and orchestrators can branch on a single deterministic value
/// when the operation was cancelled by the user. The signal name is preserved in
/// the JSON envelope emitted before exit (`{"code":19,"signal":"SIGINT",...}`).
pub const SHUTDOWN_EXIT_CODE: i32 = 19;

/// Canonical value of `PRAGMA user_version` written after migrations.
///
/// **Why 50 instead of `CURRENT_SCHEMA_VERSION` (15)?**
/// `user_version` is a 32-bit integer that SQLite reserves for application use.
/// We deliberately set it to a project-specific marker (50 = decimal) so external
/// inspection tools (`sqlite3 db.sqlite "PRAGMA user_version"`, the `file` command,
/// SQLite browser GUIs) can distinguish a sqlite-graphrag database from a generic
/// SQLite file at a glance. The application-level schema version (15, matching
/// `CURRENT_SCHEMA_VERSION`) is stored in the `schema_meta` table and exposed via
/// `health --json`/`stats --json`. Bumping migrations does NOT change this constant.
/// Refinery uses its own `refinery_schema_history` table for migration bookkeeping.
pub const SCHEMA_USER_VERSION: i64 = 50;

/// Current schema version, equal to the highest migration number in `migrations/Vnnn__*.sql`.
///
/// Added in v1.0.27 as a runtime and test sanity check.
/// Must be bumped in sync with new Refinery migrations; the unit test
/// `schema_version_matches_migrations_count` validates this automatically.
pub const CURRENT_SCHEMA_VERSION: u32 = 16;

#[cfg(test)]
mod tests_schema_version {
    use super::CURRENT_SCHEMA_VERSION;

    #[test]
    fn schema_version_matches_migrations_count() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let migrations_dir = std::path::Path::new(manifest_dir).join("migrations");
        let count = std::fs::read_dir(&migrations_dir)
            .expect("migrations directory must exist")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with('V'))
            .count() as u32;
        assert_eq!(
            CURRENT_SCHEMA_VERSION, count,
            "CURRENT_SCHEMA_VERSION ({CURRENT_SCHEMA_VERSION}) must equal the number of V*.sql migrations ({count})"
        );
    }
}
