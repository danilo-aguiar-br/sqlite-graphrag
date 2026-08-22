//! Serializable response payloads for `remember` and `recall`.
//!
//! These are the JSON contract itself; the schemas under `docs/schemas/`
//! describe them and `tests/doc_contract_integration.rs` keeps the two aligned.

use serde::Serialize;

/// JSON payload emitted by the `remember` subcommand.
///
/// All fields are required by the JSON contract (see `docs/schemas/remember.schema.json`).
/// `operation` is an alias of `action` for compatibility with clients using the old field name.
///
/// # Examples
///
/// ```
/// use sqlite_graphrag::output::RememberResponse;
///
/// let resp = RememberResponse {
///     memory_id: 1,
///     name: "nota-inicial".into(),
///     namespace: "global".into(),
///     action: "created".into(),
///     operation: "created".into(),
///     version: 1,
///     entities_persisted: 0,
///     relationships_persisted: 0,
///     relationships_truncated: false,
///     chunks_created: 1,
///     chunks_persisted: 0,
///     urls_persisted: 0,
///     extraction_method: None,
///     merged_into_memory_id: None,
///     warnings: vec![],
///     created_at: 1_700_000_000,
///     created_at_iso: "2023-11-14T22:13:20Z".into(),
///     elapsed_ms: 42,
///     name_was_normalized: false,
///     original_name: None,
///     backend_invoked: None,
///     entities_created: vec![],
///     enrich_recommended: vec![],
/// };
///
/// let json = serde_json::to_string(&resp).unwrap();
/// assert!(json.contains("\"memory_id\":1"));
/// assert!(json.contains("\"elapsed_ms\":42"));
/// assert!(json.contains("\"merged_into_memory_id\":null"));
/// assert!(json.contains("\"urls_persisted\":0"));
/// assert!(json.contains("\"relationships_truncated\":false"));
/// ```
#[derive(Serialize)]
pub struct RememberResponse {
    /// Memory identifier.
    pub memory_id: i64,
    /// Name of this item.
    pub name: String,
    /// Namespace scope.
    pub namespace: String,
    /// Action.
    pub action: String,
    /// Semantic alias of `action` for compatibility with the contract documented in SKILL.md.
    pub operation: String,
    /// Version number.
    pub version: i64,
    /// Entities persisted.
    pub entities_persisted: usize,
    /// Relationships persisted.
    pub relationships_persisted: usize,
    /// True when the relationship builder hit the cap before covering all entity pairs.
    /// Callers can use this to decide whether to increase GRAPHRAG_MAX_RELATIONSHIPS_PER_MEMORY.
    pub relationships_truncated: bool,
    /// Total number of chunks the body was split into BEFORE dedup.
    ///
    /// For single-chunk bodies this equals 1 even though no row is added to
    /// the `memory_chunks` table — the memory row itself acts as the chunk.
    /// Use `chunks_persisted` to know how many rows were actually written.
    pub chunks_created: usize,
    /// Number of chunks actually written to chunks/embeddings tables. Always <= chunks_created.
    ///
    /// Equal when no chunk had identical normalized text already in DB; less when dedup skipped
    /// some. Equals zero for single-chunk bodies (the memory row is the chunk) and equals
    /// `chunks_created` for multi-chunk bodies. Added in v1.0.23 to disambiguate from
    /// `chunks_created` and reflect database state precisely.
    pub chunks_persisted: usize,
    /// Number of unique URLs inserted into `memory_urls` for this memory.
    /// Added in v1.0.24 — split URLs out of the entity graph (P0-2 fix).
    #[serde(default)]
    pub urls_persisted: usize,
    /// Extraction method used: "url-regex" when --enable-ner ran the URL-regex pass, or "none:extraction-failed" when extraction errored. None when NER is not enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extraction_method: Option<String>,
    /// Merged into memory ID.
    pub merged_into_memory_id: Option<i64>,
    /// Warnings.
    pub warnings: Vec<String>,
    /// Timestamp Unix epoch seconds.
    pub created_at: i64,
    /// RFC 3339 UTC timestamp string parallel to `created_at` for ISO 8601 parsers.
    pub created_at_iso: String,
    /// Total execution time in milliseconds from handler start to serialisation.
    pub elapsed_ms: u64,
    /// True when the user-supplied `--name` differed from the persisted slug
    /// (i.e. kebab-case normalization changed the value). Added in v1.0.32 so
    /// callers can detect normalization without parsing stderr WARN logs.
    #[serde(default)]
    pub name_was_normalized: bool,
    /// Original user-supplied `--name` value before normalization.
    /// Present only when `name_was_normalized == true`; omitted otherwise to
    /// keep the common (already-kebab) payload small.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_name: Option<String>,
    /// v1.0.84 (ADR-0042): discriminator of the embedding backend that actually
    /// ran the passage embedding. `"openrouter" | "none"`.
    /// Absent on the wire when `None` (kept for happy-path envelope cleanliness).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_invoked: Option<&'static str>,
    /// GAP-CLI-PRIO-01: entity names written/linked in this remember call
    /// (hot set for priority entity-descriptions).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities_created: Vec<String>,
    /// GAP-CLI-PRIO-01 / G-T-ONESHOT-02: enrich operations the operator
    /// should run next (e.g. `["entity-descriptions"]` after curated graph).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enrich_recommended: Vec<String>,
}

/// Individual item returned by the `recall` query.
///
/// The `memory_type` field is serialised as `"type"` in JSON to maintain
/// compatibility with external clients — the Rust name uses `memory_type`
/// to avoid conflict with the reserved keyword.
///
/// # Examples
///
/// ```
/// use sqlite_graphrag::output::RecallItem;
///
/// let item = RecallItem {
///     memory_id: 7,
///     name: "nota-rust".into(),
///     namespace: "global".into(),
///     memory_type: "user".into(),
///     description: "aprendizado de Rust".into(),
///     snippet: "ownership e borrowing".into(),
///     distance: 0.12,
///     score: 0.88,
///     source: "direct".into(),
///     graph_depth: None,
/// };
///
/// let json = serde_json::to_string(&item).unwrap();
/// // Rust field `memory_type` appears as `"type"` in JSON.
/// assert!(json.contains("\"type\":\"user\""));
/// assert!(!json.contains("memory_type"));
/// assert!(json.contains("\"distance\":0.12"));
/// ```
#[derive(Serialize, Clone)]
pub struct RecallItem {
    /// Memory identifier.
    pub memory_id: i64,
    /// Name of this item.
    pub name: String,
    /// Namespace scope.
    pub namespace: String,
    /// Memory type classification.
    #[serde(rename = "type")]
    pub memory_type: String,
    /// Human-readable description.
    pub description: String,
    /// Snippet.
    pub snippet: String,
    /// Distance metric value.
    pub distance: f32,
    /// Cosine similarity in `[0.0, 1.0]` derived as `1.0 - distance` and clamped
    /// to that interval. Always populated to satisfy the documented contract
    /// (M-A5 in v1.0.40); higher means more similar. For graph hits the value
    /// reflects the hop-derived distance proxy and should be interpreted
    /// alongside `graph_depth` rather than as a true cosine score.
    pub score: f32,
    /// Source side of the relationship.
    pub source: String,
    /// Number of graph hops between this match and the seed memories.
    ///
    /// Set to `None` for direct vector matches (where `distance` is meaningful)
    /// and to `Some(N)` for traversal results, with `N=0` when the depth could
    /// not be tracked precisely. Added in v1.0.23 to disambiguate graph results
    /// from the `distance: 0.0` placeholder previously used for graph entries.
    /// Field is omitted from JSON output when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_depth: Option<u32>,
}

impl RecallItem {
    /// Computes the similarity score from a vector distance, clamped to
    /// `[0.0, 1.0]`. Cosine distance returned by sqlite-vec lives in `[0, 2]`
    /// in theory but the embedder produces unit-norm vectors so the practical
    /// range is `[0, 1]`. Centralized so every constructor keeps the contract.
    #[inline]
    pub fn score_from_distance(distance: f32) -> f32 {
        let raw = 1.0 - distance;
        if raw.is_nan() {
            0.0
        } else {
            raw.clamp(0.0, 1.0)
        }
    }
}

/// Full response envelope returned by the `recall` subcommand.
///
/// Contains both direct vector matches and graph-traversal matches, plus the
/// aggregated `results` list that merges both for callers that do not need
/// to distinguish the source.
#[derive(Serialize)]
pub struct RecallResponse {
    /// Search query text.
    pub query: String,
    /// Maximum number of results to return.
    pub k: usize,
    /// Direct matches.
    pub direct_matches: Vec<RecallItem>,
    /// Graph matches.
    pub graph_matches: Vec<RecallItem>,
    /// Aggregated alias of `direct_matches` + `graph_matches` for the contract documented in SKILL.md.
    pub results: Vec<RecallItem>,
    /// Total execution time in milliseconds from handler start to serialisation.
    pub elapsed_ms: u64,
    /// G58 (v1.0.80): `true` when the live query embedding failed and the
    /// handler fell back to FTS5 BM25 + LIKE prefix. Symmetric to
    /// `fts_degraded` in `hybrid-search`. Absent on the wire when false.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub vec_degraded: bool,
    /// G58 (v1.0.80): human-readable description of the embedding failure
    /// that triggered the fallback. Absent on the wire when `vec_degraded`
    /// is false or the failure had no message.
    #[serde(skip_serializing_if = "std::option::Option::is_none")]
    pub vec_error: Option<String>,
    /// G58 (v1.0.80): advisory warning echoed for callers that branch on
    /// top-level status. Distinguishes a FTS5-only fallback from a clean
    /// hybrid response so downstream pipelines can lower their confidence.
    #[serde(skip_serializing_if = "std::option::Option::is_none")]
    pub warning: Option<String>,
    /// v1.0.84 (ADR-0042): discriminator of the embedding backend that actually
    /// ran the live embedding. `"openrouter" | "none"`. Absent
    /// on the wire when `None` (kept for happy-path envelope cleanliness).
    #[serde(skip_serializing_if = "std::option::Option::is_none")]
    pub backend_invoked: Option<&'static str>,
    /// Operator-facing PROSE for the degradation, not a closed set.
    ///
    /// The name says `reason` and the published document said `enum` for four
    /// releases, but what lands here is `FallbackReason`'s `Display` —
    /// `"embedding failed: {msg}"`, carrying the provider's own message. Any
    /// new provider error is a new string, so no enum could ever have held.
    /// GAP-SG-290 measured this; the machine-readable half now travels beside
    /// it in [`Self::vec_degraded_code`] rather than replacing this field,
    /// because the envelope has always carried the prose and changing it would
    /// break consumers reading it.
    #[serde(skip_serializing_if = "std::option::Option::is_none")]
    pub vec_degraded_reason: Option<String>,
    /// v1.2.8 (GAP-SG-290): stable, machine-readable code for the degradation.
    ///
    /// This is `FallbackReason::reason_code()` — the eight-value set a consumer
    /// can actually match on: the seven from that method plus
    /// `FALLBACK_FTS_ONLY_CODE` for the degradation an operator ASKED for.
    /// Absent on the wire when `vec_degraded` is false, so the happy-path
    /// envelope is byte-identical and no existing consumer sees a new field.
    #[serde(skip_serializing_if = "std::option::Option::is_none")]
    pub vec_degraded_code: Option<&'static str>,
}
