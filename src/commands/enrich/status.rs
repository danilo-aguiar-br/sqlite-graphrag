//! Read-only enrich maintenance paths: --status, --list-dead, --requeue-dead, prune, reset.
//! Extracted from run.rs (Wave C1).

use super::args::{EnrichArgs, EnrichOperation};
use super::queue::{
    open_queue_db, prune_dead_entity_orphans, prune_dead_orphans, reset_stale_processing_claims,
    DeadItem, DeadSummary, EnrichStatus, WaitingItem,
};
use super::scan::{
    count_operation_backlog_with_force, sample_entity_description_quality, scan_unbound_memories,
    DEFAULT_QUALITY_SAMPLE_N,
};
use crate::errors::AppError;
use crate::output::emit_json_line as emit_json;
use crate::paths::AppPaths;
use crate::storage::connection::{ensure_db_ready, open_rw};

/// Handle maintenance/read-only flags. Returns `Ok(true)` if the command fully
/// handled the request (caller should return Ok(())), or `Ok(false)` to continue
/// into the normal enrich pipeline.
pub(crate) fn try_handle_maintenance(args: &EnrichArgs) -> Result<bool, AppError> {
    if args.list_dead
        || args.requeue_dead
        || args.list_skipped
        || args.requeue_skipped
        || args.prune_dead_orphans
        || args.prune_dead_entity_orphans
    {
        let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
        let op_label = format!("{:?}", args.operation());
        let paths = AppPaths::resolve(args.db.as_deref())?;
        let queue_path = crate::paths::sidecar_path(&paths.db, ".enrich-queue.sqlite");
        let queue_conn = open_queue_db(&queue_path)?;
        // v1.1.2: prune dead ENTITY orphan rows — terminal artifacts of
        // re-extraction; no main-DB check needed (entity dead rows are not
        // recoverable, re-running re-fails the same way). Distinct from the
        // memory-scoped prune below.
        if args.prune_dead_entity_orphans {
            let pruned = prune_dead_entity_orphans(&queue_conn, &op_label)?;
            let dead_total: i64 = queue_conn
                .query_row(
                    "SELECT COUNT(*) FROM queue WHERE status='dead' \
                 AND item_type='entity' \
                 AND (operation = ?1 OR operation IS NULL)",
                    rusqlite::params![op_label],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            emit_json(&DeadSummary {
                summary: true,
                operation: op_label,
                namespace,
                action: "prune-dead-entity-orphans",
                dead_total,
                requeued: 0,
                pruned,
            });
            return Ok(true);
        }
        // GAP-SG-66: prune orphan dead rows (memory gone) — needs the main DB to
        // confirm the referenced memory is truly absent before deleting.
        if args.prune_dead_orphans {
            ensure_db_ready(&paths)?;
            let main_conn = open_rw(&paths.db)?;
            let pruned = prune_dead_orphans(&queue_conn, &main_conn, &op_label, &namespace)?;
            let dead_total: i64 = queue_conn
                .query_row(
                    "SELECT COUNT(*) FROM queue WHERE status='dead' \
                 AND (operation = ?1 OR operation IS NULL)",
                    rusqlite::params![op_label],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            emit_json(&DeadSummary {
                summary: true,
                operation: op_label,
                namespace,
                action: "prune-dead-orphans",
                dead_total,
                requeued: 0,
                pruned,
            });
            return Ok(true);
        }
        if args.list_dead {
            let mut stmt = queue_conn.prepare(
                "SELECT item_key, item_type, attempt, error_class, error, \
                     finish_reason, input_tokens, output_tokens FROM queue \
             WHERE status='dead' AND (operation = ?1 OR operation IS NULL) ORDER BY id",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![op_label], |r| {
                    Ok(DeadItem {
                        dead_item: true,
                        item_key: r.get(0)?,
                        item_type: r.get(1)?,
                        attempt: r.get(2)?,
                        error_class: r.get(3)?,
                        error: r.get(4)?,
                        finish_reason: r.get(5)?,
                        input_tokens: r.get(6)?,
                        output_tokens: r.get(7)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let dead_total = rows.len() as i64;
            for item in &rows {
                emit_json(item);
            }
            emit_json(&DeadSummary {
                summary: true,
                operation: op_label,
                namespace,
                action: "list-dead",
                dead_total,
                requeued: 0,
                pruned: 0,
            });
            return Ok(true);
        }
        // GAP-SG-96: --list-skipped mirrors --list-dead for the skipped sink.
        if args.list_skipped {
            let mut stmt = queue_conn.prepare(
                "SELECT item_key, item_type, attempt, error_class, error, \
                     finish_reason, input_tokens, output_tokens FROM queue \
             WHERE status='skipped' AND (operation = ?1 OR operation IS NULL) ORDER BY id",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![op_label], |r| {
                    Ok(DeadItem {
                        dead_item: false,
                        item_key: r.get(0)?,
                        item_type: r.get(1)?,
                        attempt: r.get(2)?,
                        error_class: r.get(3)?,
                        error: r.get(4)?,
                        finish_reason: r.get(5)?,
                        input_tokens: r.get(6)?,
                        output_tokens: r.get(7)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let dead_total = rows.len() as i64;
            for item in &rows {
                emit_json(item);
            }
            emit_json(&DeadSummary {
                summary: true,
                operation: op_label,
                namespace,
                action: "list-skipped",
                dead_total,
                requeued: 0,
                pruned: 0,
            });
            return Ok(true);
        }
        // GAP-SG-96 / G-PR-4: --requeue-skipped moves skipped -> pending.
        if args.requeue_skipped {
            let skipped_total: i64 = queue_conn
                .query_row(
                    "SELECT COUNT(*) FROM queue WHERE status='skipped' \
                 AND (operation = ?1 OR operation IS NULL)",
                    rusqlite::params![op_label],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let requeued = queue_conn
                .execute(
                    "UPDATE queue SET status='pending', attempt=0, next_retry_at=NULL, \
                 error=NULL, error_class=NULL \
                 WHERE status='skipped' AND (operation = ?1 OR operation IS NULL)",
                    rusqlite::params![op_label],
                )
                .map_err(|e| {
                    AppError::Validation(crate::i18n::validation::requeue_skipped_failed(&e))
                })? as i64;
            let _ = queue_conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            emit_json(&DeadSummary {
                summary: true,
                operation: op_label,
                namespace,
                action: "requeue-skipped",
                dead_total: skipped_total,
                requeued,
                pruned: 0,
            });
            return Ok(true);
        }
        // --requeue-dead: move dead -> pending, clearing the failure bookkeeping.
        let dead_total: i64 = queue_conn
            .query_row(
                "SELECT COUNT(*) FROM queue WHERE status='dead' \
             AND (operation = ?1 OR operation IS NULL)",
                rusqlite::params![op_label],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let requeued = queue_conn
            .execute(
                "UPDATE queue SET status='pending', attempt=0, next_retry_at=NULL, \
             error=NULL, error_class=NULL \
             WHERE status='dead' AND (operation = ?1 OR operation IS NULL)",
                rusqlite::params![op_label],
            )
            .map_err(|e| AppError::Validation(crate::i18n::validation::requeue_dead_failed(&e)))?
            as i64;
        let _ = queue_conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        emit_json(&DeadSummary {
            summary: true,
            operation: op_label,
            namespace,
            action: "requeue-dead",
            dead_total,
            requeued,
            pruned: 0,
        });
        return Ok(true);
    }

    if args.status {
        let paths = AppPaths::resolve(args.db.as_deref())?;
        ensure_db_ready(&paths)?;
        let conn = open_rw(&paths.db)?;
        let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
        let unbound_backlog = scan_unbound_memories(&conn, &namespace, None, &[])?.len();
        // GAP-SG-77: DB-semantics backlog for the queried operation (fixes the
        // false pending=0 for entity-descriptions/body-enrich/re-embed).
        // GAP-CLI-ED-STATUS-01: honour --force-redescribe on status COUNT.
        let scan_backlog = count_operation_backlog_with_force(
            &conn,
            &args.operation(),
            &namespace,
            args.target,
            args.force_redescribe,
        )?;
        let scan_backlog_empty = if matches!(args.operation(), EnrichOperation::EntityDescriptions)
        {
            Some(count_operation_backlog_with_force(
                &conn,
                &args.operation(),
                &namespace,
                args.target,
                false,
            )?)
        } else {
            None
        };
        let scan_backlog_low_quality =
            if matches!(args.operation(), EnrichOperation::EntityDescriptions)
                && args.force_redescribe
            {
                // low_quality-only = force total minus empty-only
                Some(scan_backlog - scan_backlog_empty.unwrap_or(0))
            } else if matches!(args.operation(), EnrichOperation::EntityDescriptions) {
                // Without force, still expose low-quality count for operators
                // (does not change scan_backlog classic metric unless force).
                let with_force = count_operation_backlog_with_force(
                    &conn,
                    &args.operation(),
                    &namespace,
                    args.target,
                    true,
                )?;
                Some(with_force - scan_backlog)
            } else {
                None
            };
        // Wave 2: optional grounding quality sample for entity-descriptions.
        let (quality_pct, quality_sample_n, scan_backlog_low_grounding_est) =
            if matches!(args.operation(), EnrichOperation::EntityDescriptions) {
                let sample_n = crate::runtime_config::resolve_usize(
                    args.quality_sample,
                    "enrich.entity_description.quality_sample",
                    DEFAULT_QUALITY_SAMPLE_N,
                );
                if sample_n == 0 {
                    (None, None, None)
                } else {
                    let sample = sample_entity_description_quality(
                        &conn,
                        &namespace,
                        sample_n,
                        args.entity_description_grounding_threshold,
                    )?;
                    (
                        Some(sample.quality_pct),
                        Some(sample.sampled),
                        Some(sample.low_grounding_est),
                    )
                }
            } else {
                (None, None, None)
            };
        let queue_path = crate::paths::sidecar_path(&paths.db, ".enrich-queue.sqlite");
        let queue_conn = open_queue_db(&queue_path)?;
        let op_label = format!("{:?}", args.operation());
        // GAP-SG-42: scope every count to the current operation. Rows migrated
        // before the `operation` column (NULL) are still counted so a legacy
        // queue is never reported as spuriously empty.
        let count_status = |st: &str, op: &str| -> i64 {
            queue_conn
                .query_row(
                    "SELECT COUNT(*) FROM queue WHERE status=?1 \
                 AND (operation = ?2 OR operation IS NULL)",
                    rusqlite::params![st, op],
                    |r| r.get(0),
                )
                .unwrap_or(0)
        };
        let eligible_now: i64 = queue_conn
            .query_row(
                "SELECT COUNT(*) FROM queue WHERE status='pending' \
             AND (operation = ?1 OR operation IS NULL) \
             AND (next_retry_at IS NULL OR next_retry_at <= datetime('now'))",
                rusqlite::params![op_label],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let waiting: i64 = queue_conn
            .query_row(
                "SELECT COUNT(*) FROM queue WHERE status='pending' \
             AND (operation = ?1 OR operation IS NULL) \
             AND next_retry_at IS NOT NULL AND next_retry_at > datetime('now')",
                rusqlite::params![op_label],
                |r| r.get(0),
            )
            .unwrap_or(0);
        // GAP-SG-16: enumerate the items currently in backoff with their ETA.
        let waiting_items = {
            let mut stmt = queue_conn.prepare(
                "SELECT item_key, attempt, next_retry_at, error_class FROM queue \
             WHERE status='pending' AND (operation = ?1 OR operation IS NULL) \
             AND next_retry_at IS NOT NULL AND next_retry_at > datetime('now') \
             ORDER BY next_retry_at",
            )?;
            let items: Vec<WaitingItem> = stmt
                .query_map(rusqlite::params![op_label], |r| {
                    Ok(WaitingItem {
                        item_key: r.get(0)?,
                        attempt: r.get(1)?,
                        next_retry_at: r.get(2)?,
                        error_class: r.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            items
        };
        let queue_pending = count_status("pending", &op_label);
        let queue_processing = count_status("processing", &op_label);
        let queue_done = count_status("done", &op_label);
        let queue_failed = count_status("failed", &op_label);
        let queue_skipped = count_status("skipped", &op_label);
        let queue_dead = count_status("dead", &op_label);
        // GAP-SG-15/46: distinguish empty from cooldown from not-yet-scanned.
        // GAP-CLI-QISO-06: when the DB still shows scan deficit but the only
        // queue residue is permanent dead (no pending/waiting), report
        // blocked_dead so CAPA3/gate does not loop forever on pending-scan.
        // G-PR-7: blocked_dead only when the sole non-empty residue is permanent
        // dead AND there is no skipped debt that --requeue-skipped could clear.
        // A minority of dead rows must not hide pending-scan / skipped residual.
        let state = if eligible_now > 0 {
            "draining"
        } else if waiting > 0 {
            "cooldown"
        } else if queue_pending == 0
            && queue_skipped == 0
            && queue_processing == 0
            && scan_backlog > 0
            && queue_dead > 0
        {
            "blocked_dead"
        } else if queue_pending == 0 && scan_backlog > 0 {
            "pending-scan"
        } else {
            "empty"
        };
        emit_json(&EnrichStatus {
            status_report: true,
            operation: op_label,
            namespace,
            unbound_backlog,
            scan_backlog,
            scan_backlog_empty,
            scan_backlog_low_quality,
            force_redescribe: args.force_redescribe,
            quality_pct,
            quality_sample_n,
            scan_backlog_low_grounding_est,
            queue_pending,
            queue_processing,
            queue_done,
            queue_failed,
            queue_skipped,
            queue_dead,
            eligible_now,
            waiting,
            state,
            waiting_items,
        });
        return Ok(true);
    }

    // v1.1.2 (Bug 4): standalone recovery path — reset stale `processing` claims
    // left behind by a kill -9, then exit. No scan, no LLM, no singleton: the
    // operator runs this after a crash to unblock the queue before re-running
    // enrich. Emits a JSON envelope mirroring the other maintenance paths.
    if args.reset_stale_claims {
        let paths = AppPaths::resolve(args.db.as_deref())?;
        ensure_db_ready(&paths)?;
        let queue_path = crate::paths::sidecar_path(&paths.db, ".enrich-queue.sqlite");
        let queue_conn = open_queue_db(&queue_path)?;
        let reset = reset_stale_processing_claims(&queue_conn, args.stale_claim_secs)?;
        let _ = queue_conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        tracing::info!(
            target: "enrich",
            reset,
            max_age_secs = args.stale_claim_secs,
            "reset stale processing claims"
        );
        emit_json(&serde_json::json!({
            "reset_stale_claims": true,
            "reset": reset,
            "max_age_secs": args.stale_claim_secs,
        }));
        return Ok(true);
    }

    Ok(false)
}
