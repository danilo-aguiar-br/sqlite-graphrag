//! `enrich-summary` family: the single closing record of a drain run.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct EnrichSummary {
    pub(crate) summary: bool,
    pub(crate) operation: String,
    pub(crate) items_total: usize,
    pub(crate) completed: usize,
    pub(crate) failed: usize,
    pub(crate) skipped: usize,
    pub(crate) cost_usd: f64,
    pub(crate) elapsed_ms: u64,
    /// v1.0.84 (ADR-0042): discriminator of the embedding backend that actually ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) backend_invoked: Option<&'static str>,
    /// GAP-SG-15: items still parked on backoff when the run ended.
    pub(crate) waiting: i64,
    /// GAP-SG-15: dead-letter items at the end of the run.
    pub(crate) dead: i64,
    /// GAP-CLI-EC-08: true when soft/max runtime budget ended the run early.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) budget_exhausted: Option<bool>,
    /// GAP-CLI-EC-08: estimated pairs still unscanned (status proxy).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pairs_remaining_estimate: Option<i64>,
    /// GAP-CLI-PRIO-05: cooperative yields performed this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) yields: Option<u64>,
    /// Wave 3 / EC-09: true when entity-connect yielded to hot entity-descriptions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) preempted_for_gate: Option<bool>,
}
