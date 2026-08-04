//! Sidecar queue preparation and candidate enqueue.
//!
//! Brings the `.enrich-queue.sqlite` sidecar to a clean starting state for this
//! operation and namespace — stale claims swept, resume/retry applied, stale
//! rows cleared, force-redescribe reopened, satisfied re-embeds reconciled —
//! and then inserts the scanned candidates in one transaction.

use super::super::args::{EnrichArgs, EnrichOperation};
use super::super::queue::{
    count_eligible_pending, enqueue_candidate, item_type_for_key,
    reconcile_satisfied_reembed_pending, reopen_force_redescribe_candidates, reset_failed_for_op,
    reset_processing_for_op, reset_stale_processing_claims,
};
use crate::errors::AppError;
use rusqlite::Connection;

/// Sweeps stale claims and applies the resume / retry / clear policy.
pub(super) fn prepare_queue(
    queue_conn: &Connection,
    conn: &Connection,
    args: &EnrichArgs,
    namespace: &str,
    op_label: &str,
    scan_keys: &[String],
) -> Result<(), AppError> {
    // v1.1.2 (Bug 4): sweep stale `processing` claims left by a previous kill -9
    // BEFORE the singleton/drain starts, on EVERY run (not only --resume). A row
    // orphaned mid-LLM-call never clears its claimed_at, so without this sweep the
    // next run would never re-select it and the backlog would silently shrink.
    {
        let stale_reset = reset_stale_processing_claims(queue_conn, args.stale_claim_secs)?;
        if stale_reset > 0 {
            tracing::info!(
                target: "enrich",
                count = stale_reset,
                max_age_secs = args.stale_claim_secs,
                "reset stale processing claims (older than threshold)"
            );
        }
    }

    // CAPA-E: resume / retry-failed scoped to this operation + namespace only.
    if args.resume {
        let reset = reset_processing_for_op(queue_conn, op_label, namespace)
            .map_err(|e| AppError::Validation(crate::i18n::validation::queue_resume_failed(&e)))?;
        if reset > 0 {
            tracing::info!(target: "enrich", count = reset, "reset stuck processing items to pending");
        }
    }

    if args.retry_failed {
        let count = reset_failed_for_op(queue_conn, op_label, namespace).map_err(|e| {
            AppError::Validation(crate::i18n::validation::queue_retry_failed_reset_failed(&e))
        })?;
        tracing::info!(target: "enrich", count, "retrying failed items");
    }
    if !args.resume && !args.retry_failed && !args.until_empty {
        queue_conn
            .execute(
                "DELETE FROM queue WHERE operation = ?1 \
                 AND (namespace = ?2 OR namespace = '' OR namespace IS NULL)",
                rusqlite::params![op_label, namespace],
            )
            .map_err(|e| AppError::Validation(crate::i18n::validation::queue_clear_failed(&e)))?;
    }

    // CAPA-B/F: force-redescribe reopens skipped/done for scan keys once per run
    // (before first enqueue only). until-empty re-scans must NOT reopen or
    // preservation_failed loops until max-runtime.
    if args.force_redescribe && matches!(args.operation(), EnrichOperation::EntityDescriptions) {
        let reopened = reopen_force_redescribe_candidates(queue_conn, namespace, scan_keys);
        if reopened > 0 {
            tracing::info!(
                target: "enrich",
                reopened,
                scan = scan_keys.len(),
                "force-redescribe reopened skipped/done candidates (once per run)"
            );
        }
    }

    // CAPA-C2: drop ReEmbed pending that already have a live vector at active dim.
    if matches!(args.operation(), EnrichOperation::ReEmbed) {
        match reconcile_satisfied_reembed_pending(conn, queue_conn, namespace) {
            Ok(n) if n > 0 => {
                tracing::info!(
                    target: "enrich",
                    reconciled = n,
                    "re-embed reconciled pending rows with live embeddings"
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(target: "enrich", error = %e, "re-embed reconcile failed");
            }
        }
    }

    Ok(())
}

/// Populate queue (GAP-SG-12: tag rows with the operation + link memory_id).
///
/// v1.1.2 (Bug 4, D5): batch every INSERT in a single transaction so hundreds
/// of candidates commit with one fsync instead of one-per-statement. The
/// memory_id resolution SELECT runs against the main DB (read-only here) and
/// stays outside the queue transaction.
pub(super) fn enqueue_batch(
    queue_conn: &mut Connection,
    conn: &Connection,
    namespace: &str,
    keys: &[String],
    item_type: &'static str,
    op_label: &str,
) -> Result<(), AppError> {
    let tx = queue_conn.transaction()?;
    // v1.1.2 (Bug 4): `Transaction` derefs to `Connection`, so `&*tx` yields
    // the `&Connection` the existing enqueue_candidate signature expects.
    let tx_conn: &Connection = &tx;
    for key in keys.iter() {
        // v1.1.1 (P2): re-embed keys may be prefixed (`entity:` / `chunk:`);
        // derive the row item_type from the key so prune-dead-orphans never
        // mistakes an entity/chunk row for an orphaned memory.
        let it = item_type_for_key(key, item_type);
        enqueue_candidate(tx_conn, conn, namespace, key, it, op_label);
    }
    tx.commit()?;
    Ok(())
}

/// CAPA-H: scan length vs actual pending after enqueue (INSERT OR IGNORE).
pub(super) fn log_enqueue_result(
    queue_conn: &Connection,
    op_label: &str,
    namespace: &str,
    scan_len: usize,
) {
    let pending_now = count_eligible_pending(queue_conn, op_label, namespace, "");
    tracing::info!(
        target: "enrich",
        scan = scan_len,
        pending_after_enqueue = pending_now,
        "enqueue complete"
    );
}

/// Enqueue one page of candidate keys in a single transaction (GAP-SG-185 v1.2.4).
pub(super) fn enqueue_page(
    queue_conn: &mut Connection,
    conn: &Connection,
    namespace: &str,
    keys: &[String],
    item_type: &'static str,
    op_label: &str,
) -> Result<(), AppError> {
    if keys.is_empty() {
        return Ok(());
    }
    enqueue_batch(queue_conn, conn, namespace, keys, item_type, op_label)
}

/// CAPA-B/F: reopen force-redescribe candidates for one page of keys.
pub(super) fn reopen_force_redescribe_page(
    queue_conn: &Connection,
    namespace: &str,
    keys: &[String],
) -> usize {
    if keys.is_empty() {
        return 0;
    }
    reopen_force_redescribe_candidates(queue_conn, namespace, keys)
}
