//! CLI argument structs and command surface (clap-based).
//!
//! Defines `Cli` and all subcommand enums; contains no business logic.
//!
//! Split by surface: [`globals`] holds the root parser and its flags,
//! [`commands`] the subcommand enum and its classification, and
//! [`value_enums`] the `ValueEnum` types shared across argument structs.
//! Every public item is re-exported here, so `crate::cli::X` keeps resolving
//! exactly as before for every caller.

mod commands;
mod globals;
mod value_enums;

// Backend choice enums live in `backend_choice` (Wave C1).
pub use crate::backend_choice::{EmbeddingBackendChoice, LlmBackendChoice};
pub use commands::Commands;
pub use globals::Cli;
pub use value_enums::{GraphExportFormat, MemoryType};

#[cfg(test)]
#[path = "../cli_json_only_format_tests.rs"]
mod json_only_format_tests;

#[cfg(test)]
#[path = "../cli_heavy_concurrency_tests.rs"]
mod heavy_concurrency_tests;

/// GAP-SG-31/33/34/35/30: parse-time contracts for the Fase G clap fixes.
#[cfg(test)]
#[path = "../cli_fase_g_parsing_tests.rs"]
mod fase_g_parsing_tests;
