//! `enrich-item-event` family: one record per candidate the drain touched.

use serde::Serialize;

/// One line per candidate the drain touched.
///
/// `Default` is derived so a construction site names only the fields that
/// actually carry a value. Before GAP-SG-279 every site spelled out all
/// thirteen members, most of them `None`, which meant adding a field was a
/// mechanical edit across eleven places — and the cost of that edit is exactly
/// why the envelope stayed silent about what this operation did for so long.
#[derive(Debug, Default, Serialize)]
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
    /// Why an item was SKIPPED, verbatim from the result that skipped it.
    ///
    /// GAP-SG-279: the queue recorded this reason all along, via `mark_skipped`,
    /// but the event carried `error: None` and nothing else — so a caller
    /// watching the stream saw `status: "skipped"` with no way to tell an
    /// abstention from a duplicate from a confirmed label without opening the
    /// sidecar database by hand.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<String>,
    /// Type label the entity carried before a reclassification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) previous_type: Option<String>,
    /// Type label written by a reclassification, already shape-normalised.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) validated_type: Option<String>,
    /// Characters of evidence a judgement was made from (GAP-SG-279).
    ///
    /// Lets a caller separate a grounded verdict from a lucky one before
    /// trusting the rewrite. Zero would mean the decision rested on the name
    /// alone, which is now refused before the request rather than reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) evidence_chars: Option<usize>,
    pub(crate) index: usize,
    pub(crate) total: usize,
}
