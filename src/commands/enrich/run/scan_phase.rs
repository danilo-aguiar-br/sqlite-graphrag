//! The SCAN stage: announce, select candidates under a deadline, report.
//!
//! Produces the candidate key list the queue is populated from, together with
//! the `scan_start` / `scan` / `scan_meta` NDJSON events that make a long SQL
//! sweep observable instead of a silent hang.

use super::super::args::{EnrichArgs, EnrichOperation};
use super::super::events::{
    enrich_operation_cli_name, scan_operation_with_deadline, PhaseEvent, ScanStartEvent,
};
use super::super::queue::{open_queue_db, skipped_item_keys};
use super::super::scan::count_operation_backlog;
use super::budget::Budget;
use crate::errors::AppError;
use crate::output::emit_json_line as emit_json;
use rusqlite::Connection;
use std::path::Path;
use std::time::Instant;

/// Candidate keys plus the degree-0 proxy the summary later echoes.
pub(super) struct ScanOutcome {
    /// Candidate item keys, already filtered.
    pub(super) keys: Vec<String>,
    /// O(n) degree-0 + NER binding proxy, only computed for pair scans.
    #[allow(dead_code)] // dry-run path still returns ScanOutcome; production streams
    pub(super) backlog_degree0_proxy: Option<i64>,
}

/// v1.1.06: emit the pre-scan announcement so hooks never see a silent hang
/// after `validate`. Only pair scans carry the heavy SQL that motivated it.
pub(super) fn emit_scan_start(
    conn: &Connection,
    namespace: &str,
    args: &EnrichArgs,
    budget: &Budget,
    op_cli: &str,
) -> Option<i64> {
    let entities_in_namespace: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE namespace = ?1",
            rusqlite::params![namespace],
            |r| r.get(0),
        )
        .unwrap_or(0);
    // Distinct from pairs_enqueued_this_scan: status proxy of islands with NER.
    let backlog_degree0_proxy =
        count_operation_backlog(conn, &args.operation(), namespace, args.target).ok();
    emit_json(&ScanStartEvent {
        phase: "scan_start",
        operation: op_cli,
        entities_in_namespace,
        backlog_degree0_proxy,
        pair_algorithm: Some("cooccurrence+hub_island"),
        limit: args.limit,
        scan_deadline_secs: budget
            .scan_deadline
            .map(|d| d.saturating_duration_since(Instant::now()).as_secs()),
    });
    backlog_degree0_proxy
}

/// Runs the SCAN phase and emits its events.
pub(super) fn run_scan(
    conn: &Connection,
    db_path: &Path,
    namespace: &str,
    args: &EnrichArgs,
    budget: &Budget,
) -> Result<ScanOutcome, AppError> {
    let op_cli = enrich_operation_cli_name(&args.operation());
    let mut backlog_degree0_proxy: Option<i64> = None;
    if budget.pair_scan_ops {
        backlog_degree0_proxy = emit_scan_start(conn, namespace, args, budget, op_cli);
    }

    let scan_started = Instant::now();
    let mut scan_result =
        scan_operation_with_deadline(conn, namespace, args, budget.scan_deadline)?;
    // GAP-SG-69: body-enrich candidates are scanned purely by `LENGTH(body) <
    // min_output_chars`, so a short body whose rewrite the preservation guard
    // keeps rejecting is re-scanned every pass — items_total never reaches 0 and
    // `--until-empty` never converges (the detached worker reported a stuck
    // backlog for 30+ min). Exclude memories already vetoed `status='skipped'`
    // for this operation in the sidecar queue; `cleanup_queue_entry`
    // (remember/edit/forget/purge) clears the veto when the body actually
    // changes, so a genuinely updated memory is reconsidered automatically.
    if matches!(args.operation(), EnrichOperation::BodyEnrich) {
        let q_path = crate::paths::sidecar_path(db_path, ".enrich-queue.sqlite");
        if let Ok(q) = open_queue_db(&q_path) {
            if let Ok(vetoed) = skipped_item_keys(&q, &format!("{:?}", args.operation())) {
                scan_result.retain(|k| !vetoed.contains(k));
            }
        }
    }
    let total = scan_result.len();
    let scan_elapsed_ms = scan_started.elapsed().as_millis() as u64;

    emit_json(&PhaseEvent {
        phase: "scan",
        binary_path: None,
        version: None,
        items_total: Some(total),
        items_pending: Some(total),
        llm_parallelism: args.llm_parallelism,
    });
    if budget.pair_scan_ops {
        emit_json(&serde_json::json!({
            "phase": "scan_meta",
            "operation": op_cli,
            "pair_algorithm": "cooccurrence+hub_island",
            "items_total": total,
            "pairs_enqueued_this_scan": total,
            "backlog_degree0_proxy": backlog_degree0_proxy,
            "scan_elapsed_ms": scan_elapsed_ms,
            "scan_aborted_reason": serde_json::Value::Null,
        }));
    }

    Ok(ScanOutcome {
        keys: scan_result,
        backlog_degree0_proxy,
    })
}
