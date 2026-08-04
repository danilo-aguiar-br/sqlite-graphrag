//! `enrich-item-event` family: one record per candidate the drain touched.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct ItemEvent<'a> {
    /// Item identifier (memory name or entity name).
    pub(crate) item: &'a str,
    pub(crate) status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entity_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entities: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rels: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chars_before: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chars_after: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    pub(crate) index: usize,
    pub(crate) total: usize,
}
