//! `enrich-phase` family: lifecycle events describing where the run is.
//!
//! Covers the generic phase marker, the pre-scan announcement that keeps hooks
//! from seeing a silent hang, and the concurrency event that separates the
//! serial scan metric from the parallel drain fan-out.

use super::super::args::EnrichOperation;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct PhaseEvent<'a> {
    pub(crate) phase: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) binary_path: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) items_total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) items_pending: Option<usize>,
    /// Value of `--llm-parallelism` as passed by the operator, when passed.
    ///
    /// Present only on the "scan" phase event, and only when the flag was given
    /// explicitly. It is NOT the effective drain fan-out: read `drain_parallelism`
    /// on the `ConcurrencyEvent` for that, since `--mode openrouter` sizes the
    /// drain from `--rest-concurrency` instead (GAP-SG-141).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) llm_parallelism: Option<u32>,
}

/// v1.1.06: emitted **before** the (potentially heavy) candidate SQL so hooks
/// never see a silent hang after `validate`. No product telemetry — NDJSON only.
#[derive(Debug, Serialize)]
pub(crate) struct ScanStartEvent<'a> {
    pub(crate) phase: &'static str,
    /// CLI kebab-case operation name (`entity-connect` or `cross-domain-bridges`).
    pub(crate) operation: &'a str,
    pub(crate) entities_in_namespace: i64,
    /// O(n) degree-0 + NER binding proxy (status backlog); distinct from pair enqueue count.
    pub(crate) backlog_degree0_proxy: Option<i64>,
    pub(crate) pair_algorithm: Option<&'static str>,
    pub(crate) limit: Option<usize>,
    pub(crate) scan_deadline_secs: Option<u64>,
}

/// CLI kebab-case name for an enrich operation (matches clap `--operation` values).
pub(crate) fn enrich_operation_cli_name(op: &EnrichOperation) -> &'static str {
    match op {
        EnrichOperation::MemoryBindings => "memory-bindings",
        EnrichOperation::AugmentBindings => "augment-bindings",
        EnrichOperation::EntityDescriptions => "entity-descriptions",
        EnrichOperation::BodyEnrich => "body-enrich",
        EnrichOperation::ReEmbed => "re-embed",
        EnrichOperation::WeightCalibrate => "weight-calibrate",
        EnrichOperation::RelationReclassify => "relation-reclassify",
        EnrichOperation::EntityConnect => "entity-connect",
        EnrichOperation::EntityTypeValidate => "entity-type-validate",
        EnrichOperation::DescriptionEnrich => "description-enrich",
        EnrichOperation::CrossDomainBridges => "cross-domain-bridges",
        EnrichOperation::DomainClassify => "domain-classify",
        EnrichOperation::GraphAudit => "graph-audit",
        EnrichOperation::DeepResearchSynth => "deep-research-synth",
        EnrichOperation::BodyExtract => "body-extract",
    }
}

/// GAP-SG-45: separates the SCAN metric (always serial — a single SQL sweep of
/// the candidate set) from the DRAIN metric (the parallel worker fan-out). The
/// legacy "scan" `PhaseEvent` reported `llm_parallelism` on the scan event,
/// conflating the two; this event makes the distinction explicit.
#[derive(Debug, Serialize)]
pub(crate) struct ConcurrencyEvent {
    pub(crate) phase: &'static str,
    pub(crate) scan_parallelism: u32,
    pub(crate) drain_parallelism: u32,
}
