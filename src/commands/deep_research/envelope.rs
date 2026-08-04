//! Result shapes of `deep-research`: sub-query records, evidence chains,
//! graph context and the top-level JSON envelope.

use serde::Serialize;

#[derive(Serialize)]
pub(in crate::commands) struct SubQuery {
    pub(super) id: usize,
    pub(super) text: String,
    pub(super) source: &'static str,
}

#[derive(Serialize)]
pub(super) struct DeepResult {
    pub(super) name: String,
    pub(super) score: f64,
    pub(super) source: String,
    pub(super) sub_query_ids: Vec<usize>,
    pub(super) snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) body: Option<String>,
    pub(super) hop_distance: Option<usize>,
}

/// A node in a reconstructed evidence path.
#[derive(Serialize, Clone)]
pub(in crate::commands) struct EvidenceNode {
    pub(super) entity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) relation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) weight: Option<f64>,
}

/// A directed evidence chain reconstructed from BFS predecessors.
///
/// Fields:
/// - `from`: name of the seed (source) entity.
/// - `to`: name of the terminal (target) entity.
/// - `path`: ordered list of intermediate nodes from `from` to `to`.
/// - `total_weight`: product of edge weights along the path.
/// - `sub_query_ids`: which sub-queries produced this chain.
#[derive(Serialize)]
pub(in crate::commands) struct EvidenceChain {
    pub(super) from: String,
    pub(super) to: String,
    pub(super) path: Vec<EvidenceNode>,
    pub(super) total_weight: f64,
    pub(super) depth: usize,
    pub(super) sub_query_ids: Vec<usize>,
}

#[derive(Serialize)]
pub(super) struct ResearchStats {
    pub(super) sub_queries_total: usize,
    pub(super) sub_queries_completed: usize,
    pub(super) sub_queries_failed: usize,
    pub(super) sub_queries_timed_out: usize,
    pub(super) unique_memories_found: usize,
    pub(super) evidence_chains_found: usize,
    pub(super) elapsed_ms: u64,
    pub(super) vec_degraded: bool,
}

#[derive(Serialize)]
pub(super) struct GraphContextEntity {
    pub(super) name: String,
    pub(super) entity_type: String,
    pub(super) degree: u32,
}

#[derive(Serialize)]
pub(super) struct GraphContextRel {
    pub(super) from: String,
    pub(super) to: String,
    pub(super) relation: String,
    pub(super) weight: f64,
}

#[derive(Serialize)]
pub(super) struct GraphContext {
    pub(super) entities: Vec<GraphContextEntity>,
    pub(super) relationships: Vec<GraphContextRel>,
}

#[derive(Serialize)]
pub(super) struct DeepResearchResponse {
    pub(super) query: String,
    pub(super) sub_queries: Vec<SubQuery>,
    pub(super) results: Vec<DeepResult>,
    pub(super) evidence_chains: Vec<EvidenceChain>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) graph_context: Option<GraphContext>,
    pub(super) stats: ResearchStats,
}

/// Aggregated hit data: (score, source_label, snippet, body, hop_distance, sub_query_ids).
pub(super) type MergedHit = (f64, String, String, String, Option<usize>, Vec<usize>);

/// Intermediate result from a single sub-query execution.
pub(in crate::commands) struct SubQueryResult {
    pub(super) sub_query_id: usize,
    /// (memory_id, score, source_label, snippet, body, hop_distance)
    pub(super) hits: Vec<(i64, f64, String, String, String, Option<usize>)>,
    /// Evidence chains reconstructed from BFS.
    pub(super) chains: Vec<EvidenceChain>,
}
