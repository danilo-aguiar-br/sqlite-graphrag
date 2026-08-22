//! Wall-clock budgets for the run.
//!
//! Derives the overall deadline from `--max-runtime` and the separate, tighter
//! deadline that bounds the candidate scan, so a runaway pair scan cannot pin
//! the enrich singleton (v1.1.06, GAP-ENTITY-CONNECT-SCAN-CARTESIAN).

use super::super::args::{EnrichArgs, EnrichOperation};
use std::time::Instant;

/// Soft ceiling for pair scans when no explicit short budget is set.
const ENTITY_CONNECT_SCAN_SOFT_CEILING_SECS: u64 = 120;

/// The two deadlines plus the flag telling whether this operation scans pairs.
pub(super) struct Budget {
    /// Deadline covering scan plus drain.
    pub(super) until_deadline: Instant,
    /// Tighter deadline handed to the candidate scan, when one applies.
    pub(super) scan_deadline: Option<Instant>,
    /// True for `entity-connect` / `cross-domain-bridges`.
    pub(super) pair_scan_ops: bool,
}

/// v1.1.06 (GAP-ENTITY-CONNECT-SCAN-CARTESIAN): wall-clock deadline covers
/// the **first** scan (and later rescans), not only the drain loop tail.
/// Default 3600s matches --max-runtime; entity-connect also gets a soft
/// ceiling so a hung SQL cannot pin the singleton forever when the operator
/// omits --max-runtime without --until-empty.
pub(super) fn resolve(args: &EnrichArgs) -> Budget {
    // The fallback is the SAME constant the `--max-runtime` help text and the
    // arg default advertise: spelled as a bare `3600` here it could drift from
    // the documented default without anything failing.
    let max_runtime_secs = args
        .max_runtime
        .unwrap_or(crate::constants::DEFAULT_ENRICH_MAX_RUNTIME_SECS);
    let until_deadline = Instant::now() + std::time::Duration::from_secs(max_runtime_secs);
    let pair_scan_ops = matches!(
        args.operation(),
        EnrichOperation::EntityConnect | EnrichOperation::CrossDomainBridges
    );
    let scan_deadline = if pair_scan_ops {
        let soft =
            Instant::now() + std::time::Duration::from_secs(ENTITY_CONNECT_SCAN_SOFT_CEILING_SECS);
        Some(soft.min(until_deadline))
    } else if args.until_empty || args.max_runtime.is_some() {
        Some(until_deadline)
    } else {
        None
    };
    Budget {
        until_deadline,
        scan_deadline,
        pair_scan_ops,
    }
}
