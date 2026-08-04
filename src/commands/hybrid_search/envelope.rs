//! Serialisation shapes of the `hybrid-search` response.
//!
//! Only the wire contract lives here: the per-result item, the RRF weight pair
//! and the top-level envelope with its degradation flags.

use crate::output::RecallItem;

/// Hybrid search item.
#[derive(serde::Serialize)]
pub struct HybridSearchItem {
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
    /// Full text body.
    pub body: String,
    /// Snippet.
    pub snippet: String,
    /// Combined score.
    pub combined_score: f64,
    /// Alias of `combined_score` for the documented contract in SKILL.md.
    pub score: f64,
    /// Source of the match: always "hybrid" (RRF of vec + fts). Added in v2.0.1.
    pub source: String,
    /// VEC rank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vec_rank: Option<usize>,
    /// FTS rank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fts_rank: Option<usize>,
    /// Combined RRF score — explicit alias of `combined_score` for integration contracts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rrf_score: Option<f64>,
    /// RRF score normalized to [0.0, 1.0] for cross-method comparability.
    pub normalized_score: f64,
    /// Raw KNN distance from the vector index (lower = more similar).
    ///
    /// Present when the result came from the vector search path; `None` when the
    /// result appeared only in the FTS5 results and was not ranked by the KNN index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vec_distance: Option<f64>,
    /// Raw BM25 score from the FTS5 index. Currently always `None`; reserved for
    /// a future release when the FTS5 BM25 score is exposed by the storage layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fts_bm25: Option<f64>,
}

/// RRF weights used in hybrid search: vec (vector) and fts (text).
#[derive(serde::Serialize)]
pub struct Weights {
    /// VEC.
    pub vec: f32,
    /// FTS.
    pub fts: f32,
}

/// Hybrid search response.
#[derive(serde::Serialize)]
pub struct HybridSearchResponse {
    /// Search query text.
    pub query: String,
    /// Maximum number of results to return.
    pub k: usize,
    /// RRF k parameter used in the combined ranking.
    pub rrf_k: u32,
    /// Weights applied to vec and fts sources in the RRF fusion.
    pub weights: Weights,
    /// Results.
    pub results: Vec<HybridSearchItem>,
    /// Graph matches.
    pub graph_matches: Vec<RecallItem>,
    /// Ceiling applied to `graph_matches`; `null` when the cap is disabled.
    ///
    /// Echoed because the cap is ACTIVE by default
    /// ([`crate::constants::DEFAULT_HYBRID_MAX_GRAPH_RESULTS`]): a truncation the
    /// caller cannot see is worse than no truncation, since a short
    /// `graph_matches` would otherwise be indistinguishable from a small
    /// neighbourhood.
    pub max_graph_results: Option<usize>,
    /// True when FTS5 failed and the response is vec-only.
    ///
    /// Omitted from JSON when `false` to keep the happy-path envelope clean.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fts_degraded: bool,
    /// Human-readable description of the FTS5 failure when `fts_degraded` is true.
    ///
    /// Omitted from JSON when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fts_error: Option<String>,
    /// True when the FTS5 index was corrupted and successfully auto-rebuilt during this request.
    ///
    /// Omitted from JSON when `false` to keep the happy-path envelope clean.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fts_auto_rebuilt: bool,
    /// G58 (v1.0.80): symmetric to `fts_degraded`; `true` when the live query
    /// embedding failed and the response degraded to FTS5-only. Absent on the
    /// wire when false.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub vec_degraded: bool,
    /// G58 (v1.0.80): human-readable description of the embedding failure
    /// that triggered the fallback. Absent on the wire when `vec_degraded` is
    /// false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vec_error: Option<String>,
    /// G58 (v1.0.80): advisory warning echoed for callers that branch on
    /// top-level status. Distinguishes a FTS5-only fallback from a clean
    /// hybrid response so downstream pipelines can lower their confidence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// v1.0.84 (ADR-0042): discriminator of the embedding backend that actually
    /// ran the live embedding. `"openrouter" | "none"`. Absent
    /// on the wire when `None` (kept for happy-path envelope cleanliness).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_invoked: Option<&'static str>,
    /// v1.0.84 (ADR-0042): reason code discriminating the degradation
    /// (`"embedding_failed" | "cancelled" | "timeout"`). Absent when
    /// `vec_degraded` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vec_degraded_reason: Option<String>,
    /// Total execution time in milliseconds from handler start to serialisation.
    pub elapsed_ms: u64,
}
