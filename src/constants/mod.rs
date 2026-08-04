//! Compile-time constants shared across the crate.
//!
//! Grouped into embedding configuration, length and size limits, SQLite
//! pragmas and retrieval tuning knobs. Values are taken from the PRD and
//! must stay in sync with the migrations under `migrations/`.
//!
//! ## Dynamic concurrency permit calculation
//!
//! The maximum number of simultaneous instances can be adjusted at runtime
//! using the formula:
//!
//! ```text
//! permits = min(cpus, available_memory_mb / LLM_WORKER_RSS_MB) * 0.5
//! ```
//!
//! where `available_memory_mb` is obtained via `sysinfo::System::available_memory()`
//! converted to MiB. The result is capped at `MAX_CONCURRENT_CLI_INSTANCES`
//! and floored at 1.
//!
//! ## Layout
//!
//! The constants live in themed submodules and are re-exported here, so every
//! `crate::constants::X` path in the crate keeps resolving unchanged. The split
//! happened in v1.2.5, when the single file had reached 960 lines against the
//! project's own 800-line ceiling (GAP-SG-89).

mod embedding;
mod enrich;
mod exit_codes;
mod identity;
mod limits;
mod network;
mod runtime;
mod search;
mod storage;

pub use embedding::*;
pub use enrich::*;
pub use exit_codes::*;
pub use identity::*;
pub use limits::*;
pub use network::*;
pub use runtime::*;
pub use search::*;
pub use storage::*;
