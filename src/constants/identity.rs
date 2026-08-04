//! Product identity and logging defaults.
//!
//! Split out of the former single-file `constants.rs` in v1.2.5;
//! every item is re-exported by the parent module, so `crate::constants::X`
//! resolves exactly as before.

/// Default tracing filter level when neither CLI `-v`/`-q` nor XDG `log.level`
/// is set (GAP-SG-93).
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Crate version string sourced from `CARGO_PKG_VERSION` at build time.
pub const SQLITE_GRAPHRAG_VERSION: &str = env!("CARGO_PKG_VERSION");
