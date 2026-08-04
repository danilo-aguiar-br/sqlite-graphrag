//! Re-embed handlers for the `ReEmbed` enrich operation (GAP-SG-141 B1).
//!
//! - [`batch`] is the production path. It claims N rows in one statement,
//!   resolves them without touching the network, issues ONE embedding request
//!   for the survivors, and writes the vectors back in a single transaction.
//! - [`single`] keeps the historical one-row-per-call handlers, moved verbatim
//!   out of `extraction_ops_a.rs`. Nothing in the shipped binary calls them any
//!   more, so they are compiled under `cfg(test)` only, where they serve as the
//!   differential oracle the batch path is checked against: same key, same
//!   database, same resulting vector.
//!
//! Both resolve the same three target shapes from an `item_key`: `entity:NAME`,
//! `chunk:ID`, and a bare memory name.

pub(super) mod batch;
#[cfg(test)]
pub(super) mod single;

pub(super) use batch::{run_reembed_cycle, ReembedCycle, ReembedCycleCtx, ReembedTally};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
