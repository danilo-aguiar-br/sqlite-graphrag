//! Phase A of the ingest pipeline: read, chunk, embed and extract, in parallel.
//!
//! The producer runs on its own OS thread rather than on the caller's.
//! `pool.install` blocks until every rayon worker finishes, so calling it on
//! the main thread would restore the two-phase behaviour this pipeline exists
//! to remove: with the old design a 50-file corpus at 27 s/file spent ~22 min
//! staging before the first database write, blowing past the caller's timeout
//! with nothing persisted.
//!
//! Results leave through a bounded channel. The bound is what makes the
//! pipeline safe on a large corpus: a producer that outruns the consumer
//! blocks on `send` instead of piling staged files up in memory.

use super::args::{resolve_parallelism, IngestArgs};
use super::plan::ProcessItem;
use super::report::StageProgressEvent;
use super::stage::{stage_file, StagedFile, StagingEnv};
use crate::errors::AppError;
use crate::paths::AppPaths;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::sync::mpsc;

/// One staged file leaving Phase A, tagged with its slot index.
///
/// The index is required because `par_iter` completes out of order, so the
/// consumer cannot infer which slot a message belongs to from arrival order.
pub(super) type StageMessage = (usize, Result<Vec<StagedFile>, AppError>);

/// A running Phase A: the channel to drain and the thread to join.
pub(super) struct StageProducer {
    /// Receives one message per staged file, in completion order.
    pub(super) results: mpsc::Receiver<StageMessage>,
    /// Joined by the caller once the channel is drained.
    pub(super) handle: std::thread::JoinHandle<()>,
}

/// Per-file staging knobs, resolved once from the CLI arguments.
///
/// Grouped into a struct because they are copied into the rayon closure
/// together and passing eight loose parameters through the spawn boundary
/// obscures which ones are staging policy and which are plumbing.
#[derive(Clone, Copy)]
struct StageOptions {
    /// Whether the URL-regex extraction pass runs.
    enable_ner: bool,
    /// Whether a missing description is synthesised from the body.
    auto_describe: bool,
    /// Per-worker RSS ceiling.
    max_rss_mb: u64,
    /// Embedding fan-out inside a single file.
    llm_parallelism: usize,
}

/// Validates the parallelism flags and returns the resolved worker count.
///
/// # Errors
/// Returns [`AppError::Validation`] when `--ingest-parallelism N>1` is
/// combined with `--low-memory`, which ask for opposite things.
pub(super) fn resolve_worker_count(args: &IngestArgs) -> Result<usize, AppError> {
    if args.low_memory {
        if let Some(n) = args.ingest_parallelism {
            if n > 1 {
                return Err(AppError::Validation(
                    "--ingest-parallelism N>1 conflicts with --low-memory; use one or the other"
                        .to_string(),
                ));
            }
        }
    }
    // Honors --low-memory and the XDG setting `ingest.low_memory`
    // (both force parallelism = 1).
    Ok(resolve_parallelism(
        args.low_memory,
        args.ingest_parallelism,
    ))
}

/// Validates the extraction flags that Phase A depends on.
///
/// # Errors
/// Returns [`AppError::Validation`] when `--enable-ner` and
/// `--skip-extraction` are combined.
pub(super) fn validate_extraction_flags(args: &IngestArgs) -> Result<(), AppError> {
    if args.enable_ner && args.skip_extraction {
        return Err(AppError::Validation(
            crate::i18n::validation::enable_ner_skip_extraction_exclusive(),
        ));
    }
    if args.skip_extraction && !args.enable_ner {
        // v1.0.74: revert to v1.0.45 hidden no-op behavior. The v1.0.67
        // commit (9ddb17b) promoted this to a hard validation error, which
        // broke the "kept as a hidden no-op for backwards compatibility"
        // promise documented in CHANGELOG v1.0.45 and started failing
        // 5+ CI jobs whose E2E tests use this flag to skip the
        // (since-removed) GLiNER-ONNX model download in CI environments.
        tracing::warn!(
            "--skip-extraction is deprecated since v1.0.45 and has no effect (NER is disabled by default); remove this flag to silence the warning"
        );
    }
    Ok(())
}

/// Starts Phase A and returns immediately, before any file is staged.
///
/// # Errors
/// Returns [`AppError::Internal`] when the rayon pool cannot be built.
pub(super) fn spawn(
    args: &IngestArgs,
    process_items: Vec<ProcessItem>,
    paths: &AppPaths,
    parallelism: usize,
    backends: crate::cli::BackendChoice,
) -> Result<StageProducer, AppError> {
    let crate::cli::BackendChoice {
        llm: llm_backend,
        embedding: embedding_backend,
    } = backends;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(parallelism)
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("rayon pool: {e}")))?;

    let options = StageOptions {
        enable_ner: args.enable_ner,
        auto_describe: args.auto_describe && !args.no_auto_describe,
        max_rss_mb: args.max_rss_mb,
        llm_parallelism: args.llm_parallelism as usize,
    };

    // Bounded channel: the producer never gets more than parallelism*2 items
    // ahead of the consumer.
    let channel_bound = (parallelism * 2).max(1);
    let (tx, rx) = mpsc::sync_channel::<StageMessage>(channel_bound);

    let paths_owned = paths.clone();
    let handle = std::thread::spawn(move || {
        pool.install(|| {
            process_items.into_par_iter().for_each(|item| {
                if crate::shutdown_requested() {
                    return;
                }
                let t0 = std::time::Instant::now();
                let result = stage_file(
                    item.idx,
                    &item.path,
                    &item.derived_name,
                    StagingEnv {
                        paths: &paths_owned,
                        enable_ner: options.enable_ner,
                        max_rss_mb: options.max_rss_mb,
                        llm_parallelism: options.llm_parallelism,
                        backends: crate::cli::BackendChoice::new(llm_backend, embedding_backend),
                    },
                    options.auto_describe,
                );
                emit_progress(&item.file_str, &result, t0.elapsed().as_millis() as u64);

                // Blocking send applies backpressure: if Phase B is slower,
                // Phase A workers wait here instead of accumulating staged
                // files in memory. If the receiver is dropped (fail_fast
                // abort), ignore.
                let _ = tx.send((item.idx, result));
            });
            // Explicit drop of tx signals Phase B (rx iteration) to stop.
            drop(tx);
        });
    });

    Ok(StageProducer {
        results: rx,
        handle,
    })
}

/// Reports one staged file on stderr so a long run shows progress.
///
/// Without this a 50-file NER run looks frozen for minutes at a time; the
/// event carries the per-file cost so the operator can tell slow from stuck.
fn emit_progress(file_str: &str, result: &Result<Vec<StagedFile>, AppError>, elapsed_ms: u64) {
    let (entities, relationships) = match result {
        Ok(parts) => (
            parts.iter().map(|sf| sf.entities.len()).sum::<usize>(),
            parts.iter().map(|sf| sf.relationships.len()).sum::<usize>(),
        ),
        Err(_) => (0, 0),
    };
    let progress = StageProgressEvent {
        schema_version: 1,
        event: "file_extracted",
        path: file_str,
        ms: elapsed_ms,
        entities,
        relationships,
    };
    if let Ok(line) = serde_json::to_string(&progress) {
        tracing::info!(target: "ingest_progress", "{}", line);
    }
}
