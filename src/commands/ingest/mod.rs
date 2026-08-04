//! Handler for the `ingest` CLI subcommand.
//!
//! Bulk-ingests every file under a directory that matches a glob pattern.
//! Each matched file is persisted as a separate memory using the same
//! validation, chunking, embedding and persistence pipeline as `remember`,
//! but executed in-process so the ONNX model is loaded only once per
//! invocation. This is the v1.0.32 Onda 4B (finding A2) refactor that
//! replaced a fork-spawn-per-file pipeline (every file paid the ~17s ONNX
//! cold-start cost) with an in-process loop reusing the warm embedder
//! (daemon when available, in-process `Embedder::new` otherwise).
//!
//! Memory names are derived from file basenames (kebab-case, lowercase,
//! ASCII alphanumerics + hyphens). Output is line-delimited JSON: one
//! object per processed file (success or error), followed by a final
//! summary object. Designed for streaming consumption by agents.
//!
//! ## Incremental pipeline (v1.0.43)
//!
//! Phase A runs on a rayon thread pool (size = `--ingest-parallelism`):
//! read + chunk + embed + NER per file. Results are sent immediately via a
//! bounded `mpsc::sync_channel` to Phase B so persistence starts as soon
//! as the first file completes — no waiting for all files to finish Phase A.
//!
//! Phase B runs on the main thread: receives staged files from the channel,
//! writes to SQLite per-file (WAL absorbs individual commits), and emits
//! NDJSON progress events to stderr as each file is persisted. `Connection`
//! is not `Sync` so it never crosses thread boundaries.
//!
//! This fixes B1: with the old 2-phase design, a 50-file corpus with 27s/file
//! NER would spend ~22min in Phase A alone, exceeding the user's 900s timeout
//! before Phase B (and any DB writes) could begin. With this pipeline, the
//! first file is committed within seconds of starting.

// Submodules (R-SRP-01), split by responsibility rather than by size.
// Pipeline stages, in execution order: validate (flag checks) -> scan_fs
// (walk) -> plan (name resolution) -> dry_run (preview) -> stage_producer
// (Phase A) -> stage (per-file work) -> persist_loop (Phase B) -> persist
// (per-file write) -> enrich_after (optional binding pass). `run` orchestrates
// them; `args` and `report` carry the CLI surface and the NDJSON types.
mod args;
mod dry_run;
mod enrich_after;
mod persist;
mod persist_loop;
mod plan;
mod report;
mod run;
mod scan_fs;
mod stage;
mod stage_producer;
mod validate;

pub use args::{IngestArgs, IngestMode};
pub use run::run;

#[cfg(test)]
#[path = "../ingest_tests.rs"]
mod tests;
