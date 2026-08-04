//! JSON envelope reported after a merge.

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct MergeEntitiesResponse {
    pub(super) action: String,
    pub(super) sources: Vec<String>,
    pub(super) target: String,
    pub(super) namespace: String,
    /// v1.1.1 (P5): resolved target entity ID, echoed for unambiguous auditing.
    pub(super) target_id: i64,
    pub(super) relationships_moved: usize,
    pub(super) entities_removed: usize,
    /// Total execution time in milliseconds from handler start to serialisation.
    pub(super) elapsed_ms: u64,
}
