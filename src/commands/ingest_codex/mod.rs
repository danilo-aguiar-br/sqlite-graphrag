//! Handler for `ingest --mode codex`.
//!
//! Orchestrates the locally installed Codex CLI binary to extract
//! domain-specific entities and relationships from each file.

mod binary;
mod extract;
mod queue;
mod run;
mod types;

pub use binary::find_codex_binary;
pub use run::run_codex_ingest;

#[cfg(test)]
#[path = "../ingest_codex_tests.rs"]
mod tests;
