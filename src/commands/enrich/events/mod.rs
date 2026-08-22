//! NDJSON event types and preflight probe for enrich.
//! Extracted from mod.rs (Wave C1).
//!
//! The NDJSON contract is split by event FAMILY, one submodule per schema in
//! `docs/schemas/`: [`phase`] (`enrich-phase`), [`item`] (`enrich-item-event`)
//! and [`summary`] (`enrich-summary`). The remaining submodules are the
//! non-event concerns this file has always also carried: [`preflight`] probes
//! the provider, [`deadline`] bounds the candidate scan and [`parallelism`]
//! sizes the drain fan-out.

mod deadline;
mod entity_type_policy;
mod item;
mod parallelism;
mod phase;
mod preflight;
mod summary;

// `is_at_default` and `wait_with_timeout` stay scoped to their own submodule:
// nothing outside `events` ever reached them.
pub(crate) use deadline::scan_operation_with_deadline;
// Only the enrich test module reaches the interrupt classifier.
#[cfg(test)]
pub(crate) use deadline::is_sqlite_interrupt;
// GAP-SG-283: the entity type vocabulary policy and the two things it
// publishes — the policy actually applied, and how many signals fed the
// decision. `install_from_args` is called from `parallelism`, the last point on
// the drain path that still holds an `EnrichArgs`.
pub(crate) use entity_type_policy::{
    apply_entity_type_policy, count_type_signals, emit_policy_event, raw_label_note, PolicyOutcome,
    UNKNOWN_TYPE_POLICIES,
};
pub(crate) use item::ItemEvent;
pub(crate) use parallelism::resolve_drain_parallelism;
pub(crate) use phase::{enrich_operation_cli_name, ConcurrencyEvent, PhaseEvent, ScanStartEvent};
pub(crate) use preflight::{run_preflight_probe, PreflightOutcome};
pub(crate) use summary::EnrichSummary;
