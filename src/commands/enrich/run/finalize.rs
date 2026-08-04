//! Run teardown: graceful shutdown, WAL checkpoints, summary, sidecar cleanup.
//!
//! Everything that happens after the drain loop breaks — releasing in-flight
//! claims on SIGTERM, reporting the convergence state (waiting versus dead
//! versus truly empty) and deciding whether the sidecar queue may be deleted.

use super::super::args::EnrichArgs;
use super::super::events::EnrichSummary;
use super::super::postprocess::take_enrich_backend;
use super::super::queue::reset_processing_for_op;
use crate::output::emit_json_line as emit_json;
use rusqlite::Connection;
use std::path::Path;
use std::time::Instant;

/// Tallies the drain produced, threaded into the closing summary.
pub(super) struct FinalTally<'a> {
    pub(super) counters: &'a super::super::drain_parallel::DrainCounters,
    pub(super) items_total: usize,
    pub(super) started: Instant,
    pub(super) until_deadline: Instant,
    pub(super) pair_scan_ops: bool,
    pub(super) backlog_degree0_proxy: Option<i64>,
    pub(super) yield_count: u64,
    pub(super) preempted_for_gate: bool,
}

/// v1.1.2 (Bug 4 / Omissão 3): SIGTERM graceful cleanup. When a shutdown was
/// requested mid-drain (the worker/serial loops already broke out), reset the
/// in-flight `processing` claims back to `pending` so the NEXT run re-selects
/// them — without this a kill recycles the rows via the startup stale-claim
/// sweep (which waits `stale_claim_secs`), delaying recovery. Scoped to the
/// enrich run (not the global signal handler) so unrelated code paths keep
/// their existing exit semantics. Best-effort: errors are logged, not fatal.
/// CAPA-E: only this operation + namespace (do not unstick alien ops).
pub(super) fn release_on_shutdown(queue_conn: &Connection, op_label: &str, namespace: &str) {
    if crate::shutdown_requested() {
        let reset = reset_processing_for_op(queue_conn, op_label, namespace).unwrap_or(0);
        let _ = queue_conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        tracing::info!(
            target: "enrich",
            reset,
            "graceful shutdown: WAL checkpointed, processing claims reset"
        );
    }
}

/// Checkpoints both databases, emits the closing summary and removes the
/// sidecar queue when it holds nothing worth carrying to the next run.
pub(super) fn finish(
    conn: &Connection,
    queue_conn: &Connection,
    queue_path: &Path,
    args: &EnrichArgs,
    op_label: &str,
    namespace: &str,
    tally: FinalTally<'_>,
) {
    release_on_shutdown(queue_conn, op_label, namespace);

    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    let _ = queue_conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");

    // GAP-SG-15: report items still in cooldown (waiting) and dead-lettered
    // alongside completed, so `--until-empty` makes the convergence state
    // explicit (cooldown vs. dead vs. truly empty) instead of just "done".
    let waiting_final: i64 = queue_conn
        .query_row(
            "SELECT COUNT(*) FROM queue WHERE status='pending' \
             AND (operation = ?1 OR operation IS NULL) \
             AND next_retry_at IS NOT NULL AND next_retry_at > datetime('now')",
            rusqlite::params![op_label],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let dead_final: i64 = queue_conn
        .query_row(
            "SELECT COUNT(*) FROM queue WHERE status='dead' \
             AND (operation = ?1 OR operation IS NULL)",
            rusqlite::params![op_label],
            |r| r.get(0),
        )
        .unwrap_or(0);

    emit_json(&EnrichSummary {
        summary: true,
        operation: format!("{:?}", args.operation()),
        items_total: tally.items_total,
        completed: tally.counters.completed,
        failed: tally.counters.failed,
        skipped: tally.counters.skipped,
        cost_usd: tally.counters.cost_total,
        elapsed_ms: tally.started.elapsed().as_millis() as u64,
        backend_invoked: take_enrich_backend(),
        waiting: waiting_final,
        dead: dead_final,
        budget_exhausted: if tally.pair_scan_ops && Instant::now() >= tally.until_deadline {
            Some(true)
        } else {
            None
        },
        pairs_remaining_estimate: tally.backlog_degree0_proxy,
        yields: if tally.yield_count > 0 {
            Some(tally.yield_count)
        } else {
            None
        },
        preempted_for_gate: if tally.preempted_for_gate {
            Some(true)
        } else {
            None
        },
    });

    if tally.counters.failed == 0 {
        // GAP-ENRICH-BACKLOG-CONVERGE: keep the queue file when dead-letter rows
        // exist so `enrich --status` can still report them on the next run.
        let dead: i64 = queue_conn
            .query_row("SELECT COUNT(*) FROM queue WHERE status='dead'", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        // GAP-SG-69: keep the sidecar queue while it still holds `skipped`
        // verdicts. Those rows tell the next scan which short bodies are
        // non-expandable; removing the file would lose the veto and the
        // body-enrich backlog would never converge. cleanup_queue_entry clears
        // a row when its memory is edited/forgotten, so the veto is not permanent.
        let skipped_remaining: i64 = queue_conn
            .query_row(
                "SELECT COUNT(*) FROM queue WHERE status='skipped'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if dead == 0 && skipped_remaining == 0 {
            let _ = std::fs::remove_file(queue_path);
        }
    }
}
