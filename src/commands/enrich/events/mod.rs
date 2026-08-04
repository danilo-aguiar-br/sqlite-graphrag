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
pub(crate) use item::ItemEvent;
pub(crate) use parallelism::resolve_drain_parallelism;
pub(crate) use phase::{enrich_operation_cli_name, ConcurrencyEvent, PhaseEvent, ScanStartEvent};
pub(crate) use preflight::{run_preflight_probe, PreflightOutcome};
pub(crate) use summary::EnrichSummary;
