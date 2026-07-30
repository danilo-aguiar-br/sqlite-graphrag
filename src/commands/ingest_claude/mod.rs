//! Handler for `ingest --mode claude-code`.
//!
//! Orchestrates the locally installed Claude Code CLI binary (`claude -p`)
//! to extract domain-specific entities and relationships from each file,
//! then persists them via the same pipeline as `remember --graph-stdin`.
//!
//! Architecture: P1 One-Shot per file — each file spawns a separate
//! `claude -p` process with `--json-schema` for guaranteed structured output.
//! A SQLite queue DB tracks progress for resume/retry support.
// Workload: Subprocess I/O-bound (claude -p headless with network wait)

mod binary;
mod extract;
mod queue;
mod run;
mod types;

pub use binary::find_claude_binary;
pub use run::run_claude_ingest;
pub use types::{ExtractedEntity, ExtractedRelationship, ExtractionResult};

#[cfg(test)]
#[path = "../ingest_claude_tests.rs"]
mod tests;
