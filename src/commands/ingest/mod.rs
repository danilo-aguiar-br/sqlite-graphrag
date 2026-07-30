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

// Submodules (R-SRP-01): scan_fs = walk/name derivation; report = NDJSON types;
// args/stage/persist/validate/run = ingest pipeline stages.
mod args;
mod persist;
mod report;
mod run;
mod scan_fs;
mod stage;
mod validate;

pub use args::{IngestArgs, IngestMode};
pub use run::run;

// Used by LLM-mode ingest frontends (`ingest_claude` / `ingest_codex` / `ingest_opencode`).
pub(crate) use scan_fs::{collect_files, derive_kebab_name};

#[cfg(test)]
#[path = "../ingest_tests.rs"]
mod tests;
