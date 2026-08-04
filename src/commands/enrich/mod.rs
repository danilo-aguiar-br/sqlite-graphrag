// v1.0.97: modularised into queue.rs, scan.rs, postprocess.rs, extraction.rs.
// See ADR-0056 (closes the ADR-0046 "Known Tech Debt (v1.0.89+)" item).
// v1.1.8 Wave C1: further split into schemas/args/events/run modules (≤800 LOC).

//! Handler for the `enrich` CLI subcommand (GAP-14 + GAP-18).
//!
//! Enriches the knowledge graph by running LLM-powered analysis over memories
//! and entities that are missing key structural data. Operations are:
//!
//! - `memory-bindings`: memories without `memory_entities` rows get entity extraction
//! - `entity-descriptions`: entities with NULL/empty descriptions get LLM descriptions
//! - `body-enrich`: memories with short bodies get expanded by the LLM (GAP-18)
//! - `re-embed`: memories without a vector row get re-embedded without rewriting body
//!
//! Architecture is SCAN → JUDGE (LLM) → PERSIST, with a SQLite queue DB derived
//! next to `--db` (GAP-SG-64) for resume/retry support. The shape was inherited
//! from the retired `ingest_claude.rs`, which v1.2.0 deleted along with every
//! headless-subprocess frontend.
// Workload: network-bound. JUDGE issues OpenRouter chat-completions requests
// over HTTP in-process; `--rest-concurrency` is the only fan-out knob.
//!
//! # DRY note
//!
//! GAP-SG-121: enrich vs ingest queue table shapes are different products
//! (item_key/operation vs file_path); only sidecar WAL/busy pragmas are shared
//! via [`crate::pragmas::apply_sidecar_queue_pragmas`].

mod args;
mod drain_parallel;
mod drain_serial;
mod events;
mod extraction;
// extraction_providers is a child of extraction.rs (#[path])
// extraction_ops_* are submodules of extraction.rs (#[path])
mod postprocess;
mod predicates;
mod prompts;
mod quality_sample;
mod queue;
mod queue_ops;
mod reembed;
mod run;
mod scan;
mod scan_ec;
mod scheduler;
mod schemas;
mod status;

pub(crate) const DEFAULT_RATE_LIMIT_WAIT: u64 = 60;
pub(crate) const DEFAULT_BODY_ENRICH_MIN_CHARS: usize = 500;
pub(crate) const DEFAULT_BODY_ENRICH_MAX_CHARS: usize = 2000;

// GAP-SG-149: single source of truth for the enrich knobs that both the clap
// surface (`default_value_t`) and the synthesized `EnrichArgs` of
// `ingest --enrich-after` must agree on. Before this, `enrich_after.rs`
// carried its own literals and two of them DIVERGED from the documented
// default, so the auto-pass silently ran under a different contract than the
// one `enrich --help` advertises.
/// Attempts before a queue row is dead-lettered.
pub(crate) const DEFAULT_ENRICH_MAX_ATTEMPTS: u32 = 8;
/// Seconds after which a `processing` claim is considered abandoned.
pub(crate) const DEFAULT_ENRICH_STALE_CLAIM_SECS: u64 = 1800;
/// Seconds of headroom kept below a provider rate-limit reset.
pub(crate) const DEFAULT_ENRICH_RATE_LIMIT_BUFFER_SECS: u64 = 300;
/// Consecutive hard failures that trip the circuit breaker.
pub(crate) const DEFAULT_ENRICH_CIRCUIT_BREAKER_THRESHOLD: u32 = 5;
/// Minimum similarity for an enriched body to be accepted as preserving.
pub(crate) const DEFAULT_ENRICH_PRESERVE_THRESHOLD: f64 = 0.7;
/// Minimum grounding score for a generated entity description.
pub(crate) const DEFAULT_ENRICH_GROUNDING_THRESHOLD: f64 = 0.12;

pub use args::{EnrichArgs, EnrichMode, EnrichOperation, ReEmbedTarget};
pub use queue::{cleanup_queue_entry, DeadItem, DeadSummary, EnrichStatus, WaitingItem};
pub use run::run;

use crate::errors::AppError;
use queue::{enqueue_candidate_with_priority, open_queue_db, PRIORITY_HOT};

/// GAP-CLI-PRIO-02: enqueue entity-descriptions for a hot set of entity names
/// into the enrich sidecar queue with elevated priority.
pub fn enqueue_priority_entity_descriptions(
    paths: &crate::paths::AppPaths,
    namespace: &str,
    entity_names: &[String],
) -> Result<usize, AppError> {
    let _ = namespace; // queue keys are entity names; namespace is scoped by DB path
    let queue_path = crate::paths::sidecar_path(&paths.db, ".enrich-queue.sqlite");
    let queue = open_queue_db(&queue_path)?;
    let mut n = 0usize;
    for name in entity_names {
        enqueue_candidate_with_priority(&queue, name, "entity", "EntityDescriptions", PRIORITY_HOT);
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::events::{
        enrich_operation_cli_name, is_sqlite_interrupt, scan_operation_with_deadline,
    };
    use super::schemas::{BINDINGS_SCHEMA, BODY_ENRICH_SCHEMA, ENTITY_DESCRIPTION_SCHEMA};
    use super::*;
    use crate::errors::AppError;
    use rusqlite::{Connection, ErrorCode};
    use std::time::{Duration, Instant};

    #[test]
    fn bindings_schema_is_valid_json() {
        let _: serde_json::Value =
            serde_json::from_str(BINDINGS_SCHEMA).expect("BINDINGS_SCHEMA must be valid JSON");
    }

    #[test]
    fn entity_description_schema_is_valid_json() {
        let _: serde_json::Value = serde_json::from_str(ENTITY_DESCRIPTION_SCHEMA)
            .expect("ENTITY_DESCRIPTION_SCHEMA must be valid JSON");
    }

    #[test]
    fn body_enrich_schema_is_valid_json() {
        let _: serde_json::Value = serde_json::from_str(BODY_ENRICH_SCHEMA)
            .expect("BODY_ENRICH_SCHEMA must be valid JSON");
    }

    // v1.1.06 — GAP-ENTITY-CONNECT-SCAN-CARTESIAN observability + interrupt

    #[test]
    fn enrich_operation_cli_name_pair_ops_are_kebab_case() {
        assert_eq!(
            enrich_operation_cli_name(&EnrichOperation::EntityConnect),
            "entity-connect"
        );
        assert_eq!(
            enrich_operation_cli_name(&EnrichOperation::CrossDomainBridges),
            "cross-domain-bridges"
        );
        assert_eq!(
            enrich_operation_cli_name(&EnrichOperation::EntityDescriptions),
            "entity-descriptions"
        );
    }

    #[test]
    fn is_sqlite_interrupt_detects_operation_interrupted() {
        let ffi_err = rusqlite::ffi::Error {
            code: ErrorCode::OperationInterrupted,
            extended_code: 9,
        };
        let err = rusqlite::Error::SqliteFailure(ffi_err, Some("interrupted".into()));
        assert!(is_sqlite_interrupt(&err));

        let busy = rusqlite::ffi::Error {
            code: ErrorCode::DatabaseBusy,
            extended_code: 5,
        };
        let busy_err = rusqlite::Error::SqliteFailure(busy, None);
        assert!(!is_sqlite_interrupt(&busy_err));
    }

    #[test]
    fn scan_deadline_already_elapsed_returns_timeout() {
        // Past deadline must fail fast without running SQL (exit path → Timeout).
        use clap::Parser;
        let cli = crate::cli::Cli::try_parse_from([
            "sqlite-graphrag",
            "enrich",
            "--operation",
            "entity-connect",
            "--mode",
            "openrouter",
            "--openrouter-model",
            "test/model",
            "--dry-run",
            "--limit",
            "1",
        ])
        .expect("parse enrich args");
        let Some(crate::cli::Commands::Enrich(args)) = cli.command else {
            panic!("expected Commands::Enrich");
        };
        let conn = Connection::open_in_memory().unwrap();
        let past = Instant::now() - Duration::from_secs(1);
        let err = scan_operation_with_deadline(&conn, "global", &args, Some(past))
            .expect_err("elapsed deadline must Timeout");
        match err {
            AppError::Timeout { .. } => {}
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[test]
    fn interrupt_handle_maps_long_query_to_sqlite_interrupt() {
        // Live SQLite: watchdog interrupt aborts a recursive CTE (same mechanism
        // as scan_operation_with_deadline). Confirms rusqlite InterruptHandle.
        let conn = Connection::open_in_memory().unwrap();
        let handle = conn.get_interrupt_handle();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_w = std::sync::Arc::clone(&stop);
        let watchdog = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            if !stop_w.load(std::sync::atomic::Ordering::Relaxed) {
                handle.interrupt();
            }
        });
        let result = conn.query_row(
            "WITH RECURSIVE t(x) AS (
                 SELECT 1
                 UNION ALL
                 SELECT x + 1 FROM t WHERE x < 500000000
             )
             SELECT COUNT(*) FROM t",
            [],
            |r| r.get::<_, i64>(0),
        );
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = watchdog.join();
        let err = result.expect_err("recursive CTE must be interrupted");
        assert!(
            is_sqlite_interrupt(&err),
            "expected SQLITE_INTERRUPT, got {err:?}"
        );
    }
}
