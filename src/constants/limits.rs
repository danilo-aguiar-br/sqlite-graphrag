//! Length, size and count ceilings enforced on stored data.
//!
//! Split out of the former single-file `constants.rs` in v1.2.5;
//! every item is re-exported by the parent module, so `crate::constants::X`
//! resolves exactly as before.

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

/// Byte budget for one auto-split partition (sub-memory) in `ingest`
/// (GAP-SG-04/07).
///
/// Chosen below the 127 KB body margin so each partition also stays under
/// [`REMEMBER_MAX_SAFE_MULTI_CHUNKS`] chunks and [`crate::constants::EMBEDDING_REQUEST_MAX_TOKENS`]
/// tokens, even for multibyte/CJK text (~1 cl100k token per UTF-8 char, so
/// 80 KiB / 3 bytes-per-char yields about 27K tokens, below the 30K ceiling).
pub const AUTOSPLIT_PARTITION_MAX_BYTES: usize = 80 * 1024;

/// Degree above which `health` reports an entity as a super-hub.
///
/// A hub this wide makes graph traversal fan out badly, so the check exists to
/// prompt a `prune-relations` or `merge-entities` pass.
pub const HEALTH_SUPER_HUB_DEGREE_THRESHOLD: i64 = 50;

/// How many super-hubs `health` names in its warning string.
///
/// This bounds the *sample* shown to a human. It must never bound the reported
/// count, which is measured separately over the whole graph.
pub const HEALTH_SUPER_HUB_SAMPLE_LIMIT: usize = 5;

/// Character size of the body preview emitted in text/markdown formats.
pub const TEXT_BODY_PREVIEW_LEN: usize = 200;
