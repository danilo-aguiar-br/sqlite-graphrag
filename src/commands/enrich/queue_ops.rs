//! Queue claim/failure helpers (Wave C1).

use crate::errors::AppError;

/// Runs a queue write-back (`mark_done` / `mark_skipped` / `heartbeat`) under
/// the same bounded busy-retry budget the claim path has used since GAP-SG-76,
/// and reports whether the row was actually written.
///
/// The claim was retry-protected and the write-back was not: in the parallel
/// drain and in the re-embed batch cycle its result was discarded with
/// `let _ =`. Under `--rest-concurrency 16` a SQLITE_BUSY on `mark_done`
/// silently lost the completion, the row stayed in `processing`, the
/// stale-claim sweep handed it back, and the provider was billed a second time
/// for work already paid for. Losing a write-back is not cosmetic and must not
/// be swallowed.
///
/// `drain_serial` already inspected these results and needs no wrapper.
pub(super) fn writeback<T, E, F>(what: &str, worker_id: usize, item: &str, op: F) -> bool
where
    F: Fn() -> Result<T, E>,
    AppError: From<E>,
{
    match crate::storage::utils::with_busy_retry(|| op().map_err(AppError::from)) {
        Ok(_) => true,
        Err(e) => {
            tracing::error!(
                target: "enrich",
                worker = worker_id,
                item = %item,
                writeback = %what,
                error = %e,
                "queue write-back lost after bounded retries; item will be reprocessed and re-billed"
            );
            false
        }
    }
}

/// Classifies an enrich item failure into a retry/dead-letter outcome.
///
/// This is the FALLBACK classifier: it is only consulted when the failure
/// did not already carry a typed [`crate::retry::AttemptOutcome`] computed at
/// its origin (see [`record_item_failure_typed`], fed by
/// [`crate::commands::enrich::extraction::take_last_openrouter_failure`] for
/// OpenRouter chat/embedding calls). Classification is TYPED by `AppError`
/// variant only — NEVER by matching the formatted message — per
/// `rules_rust_retry_com_backoff.md` ("NUNCA usar string matching em
/// mensagens de erro").
pub(super) fn classify_enrich_outcome(e: &AppError) -> crate::retry::AttemptOutcome {
    use crate::retry::AttemptOutcome;
    match e {
        AppError::RateLimited { .. } | AppError::Timeout { .. } | AppError::DbBusy(_) => {
            AttemptOutcome::Transient
        }
        // GAP-SG-78: a referenced entity that is not yet materialized is a
        // TRANSITORY absence — a later enrich pass creates the entity — so the
        // item is rescheduled, not dead-lettered on the first miss. Matched on
        // the typed variant, never a message substring (`rules_rust_retry:
        // NUNCA string matching`). The `--max-attempts` floor (default 8) ends
        // the item if the entity never materializes, mirroring the `Embedding`
        // floor below.
        AppError::EntityNotYetMaterialized { .. } => AttemptOutcome::Transient,
        // GAP-SG-09: errors that are genuinely PERMANENT for this item and must
        // dead-letter immediately (retrying cannot help): a structured provider
        // rejection (context-length overflow / refusal carried as ProviderError),
        // or a MEMORY that no longer exists (deleted or renamed between scan and
        // processing). Entity absence is handled above as transitory, NOT here.
        AppError::ProviderError { .. }
        | AppError::NotFound(_)
        | AppError::MemoryNotFound { .. }
        | AppError::MemoryNotFoundById { .. } => AttemptOutcome::HardFailure,
        // GAP-SG-76: SQLITE_BUSY/LOCKED is a lock-contention hiccup between the
        // queue writer and a concurrent claim — retry it; any other database
        // error (constraint violation, corruption, I/O) is permanent.
        AppError::Database(_) => {
            if crate::storage::utils::is_sqlite_busy(e) {
                AttemptOutcome::Transient
            } else {
                AttemptOutcome::HardFailure
            }
        }
        // GAP-SG-270: `re-embed` now READS the verdict computed at the failure's
        // origin. `crate::embedder::app_error_preserving_retry_class` carries
        // `EmbedError::retry_class` across the conversion to `AppError` and
        // through the fallback chain, so a PERMANENT failure dead-letters on the
        // first attempt instead of burning every `--max-attempts` retry.
        // `Success` never describes a failure: same floor as an absent verdict.
        AppError::EmbeddingClassified { retry_class, .. } => match retry_class {
            AttemptOutcome::HardFailure => AttemptOutcome::HardFailure,
            AttemptOutcome::Transient | AttemptOutcome::Success => AttemptOutcome::Transient,
        },
        // GAP-SG-73: safe floor for every embedding failure reaching the queue
        // WITHOUT an origin-typed verdict (client not initialised, task join
        // error, batch count mismatch, empty backend chain). Transient is the
        // conservative choice and is deliberately KEPT: a persistently permanent
        // failure still terminates via `--max-attempts` instead of looping.
        AppError::Embedding(_) => AttemptOutcome::Transient,
        // Every other variant — including `Validation` without an
        // origin-typed retry verdict attached — is treated as permanent.
        // Previously this branch inspected the formatted message for
        // substrings like "json" / "missing '" to guess at transience; that
        // guesswork is now unnecessary because the OpenRouter chat path
        // (the project's only supported enrich mode) attaches its retry
        // verdict directly via `ChatError::retry_class`, computed at the
        // exact HTTP status / provider code in `chat_api.rs`, and
        // `record_item_failure_typed` consumes it BEFORE ever falling back
        // to this classifier.
        _ => AttemptOutcome::HardFailure,
    }
}

/// Applies a failure outcome to a single queue row. Shared by the parallel
/// worker and the serial loop (DRY). A `HardFailure`, or a transient failure
/// whose attempt count reached `max_attempts`, lands in the dead-letter status
/// (`status='dead'`) so it is never re-selected. A transient failure below the
/// cap is rescheduled to `pending` with an exponential-backoff `next_retry_at`.
/// Returns the [`crate::retry::AttemptOutcome`] so the caller can feed the
/// existing circuit breaker.
///
/// GAP-SG-73: delegates to [`record_item_failure_typed`] with the outcome
/// computed by the untyped fallback classifier and no diagnostics — the
/// entry point for callers that only have a bare `&AppError` (subprocess
/// providers, persistence failures).
pub(super) fn record_item_failure(
    queue_conn: &rusqlite::Connection,
    queue_id: i64,
    attempt: i64,
    max_attempts: u32,
    err: &AppError,
) -> crate::retry::AttemptOutcome {
    let outcome = classify_enrich_outcome(err);
    let err_str = format!("{err}");
    record_item_failure_typed(
        queue_conn,
        queue_id,
        attempt,
        max_attempts,
        outcome,
        &err_str,
        None,
        None,
        None,
    )
}

/// GAP-SG-72/73: applies a failure outcome to a single queue row using an
/// [`crate::retry::AttemptOutcome`] the caller ALREADY computed at the
/// failure's origin (e.g. `ChatError::retry_class` from an OpenRouter chat
/// call), plus whatever truncation diagnostics (`finish_reason` and token
/// counts) were available. This is the precise counterpart to
/// [`record_item_failure`], which falls back to the untyped
/// [`classify_enrich_outcome`] classifier when no origin-typed verdict
/// exists. Both share this single write path (DRY).
// One parameter per column the failure UPDATE writes, plus the row it targets:
// the arity IS the queue schema.
#[allow(clippy::too_many_arguments)]
pub(super) fn record_item_failure_typed(
    queue_conn: &rusqlite::Connection,
    queue_id: i64,
    attempt: i64,
    max_attempts: u32,
    outcome: crate::retry::AttemptOutcome,
    err_str: &str,
    finish_reason: Option<&str>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
) -> crate::retry::AttemptOutcome {
    use crate::retry::AttemptOutcome;
    let error_class = match outcome {
        AttemptOutcome::Transient => "transient",
        AttemptOutcome::HardFailure => "permanent",
        AttemptOutcome::Success => "success",
    };

    let terminal = matches!(outcome, AttemptOutcome::HardFailure) || attempt >= max_attempts as i64;
    if terminal {
        let _ = queue_conn.execute(
            "UPDATE queue SET status='dead', error=?1, error_class=?2, done_at=datetime('now'), \
             finish_reason=?3, input_tokens=?4, output_tokens=?5 WHERE id=?6",
            rusqlite::params![
                err_str,
                error_class,
                finish_reason,
                input_tokens,
                output_tokens,
                queue_id
            ],
        );
    } else {
        let delay = crate::retry::compute_delay(
            &crate::retry::RetryConfig::llm_rate_limit(),
            attempt.max(0) as u32,
        );
        let secs = delay.as_secs().max(1);
        let modifier = format!("+{secs} seconds");
        let _ = queue_conn.execute(
            "UPDATE queue SET status='pending', error=?1, error_class=?2, next_retry_at=datetime('now', ?3), \
             finish_reason=?4, input_tokens=?5, output_tokens=?6 WHERE id=?7",
            rusqlite::params![
                err_str,
                error_class,
                modifier,
                finish_reason,
                input_tokens,
                output_tokens,
                queue_id
            ],
        );
    }
    outcome
}

/// GAP-SG-76: outcome of claiming the next pending queue row. Distinguishes
/// a genuinely empty backlog (`QueryReturnedNoRows`) from lock contention
/// (`SQLITE_BUSY`/`SQLITE_LOCKED`) so the caller retries briefly on the
/// latter instead of breaking out of the drain loop early. Both the serial
/// loop and the parallel worker loop share this (DRY) — previously each
/// collapsed every `query_row` error into `.ok()`, silently treating a busy
/// database the same as an empty queue.
///
/// GAP-CLI-QISO-01/02: claim is scoped to a single enrich `operation` so a
/// memory-bindings drain can never claim an entity-descriptions or
/// entity-connect row (cross-op poison → false "memory not found" dead).
pub(super) enum DequeueOutcome {
    Claimed(ClaimedRow),
    Empty,
}

/// One queue row claimed for processing (GAP-CLI-QISO-02).
#[derive(Debug, Clone)]
pub(super) struct ClaimedRow {
    pub id: i64,
    pub item_key: String,
    pub item_type: String,
    pub operation: String,
    pub attempt: i64,
}

/// Defense-in-depth after claim (GAP-CLI-QISO-03). Strict SQL already filters
/// by operation; this rejects wrong item_type / key shape before the handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ClaimCheck {
    Ok,
    RequeueWrongOp,
    SkipWrongType { reason: String },
}

/// Return true when `item_key` is not a plain memory name (pair/entity/chunk).
pub(super) fn is_non_memory_key_shape(item_key: &str) -> bool {
    item_key.starts_with("pair:")
        || item_key.starts_with("entity:")
        || item_key.starts_with("chunk:")
}

/// Validate claimed row against the current CLI operation and expected type.
pub(super) fn validate_claim(
    row: &ClaimedRow,
    current_op: &str,
    expected_item_type: &str,
) -> ClaimCheck {
    if row.operation != current_op {
        return ClaimCheck::RequeueWrongOp;
    }
    // Re-embed may use entity:/chunk: prefixes; allow those types for ReEmbed.
    if current_op == "ReEmbed" {
        return ClaimCheck::Ok;
    }
    if expected_item_type == "memory" && is_non_memory_key_shape(&row.item_key) {
        return ClaimCheck::SkipWrongType {
            reason: format!(
                "wrong_key_shape_for_operation:{current_op}: key looks like {}",
                row.item_key.split(':').next().unwrap_or("prefixed")
            ),
        };
    }
    if expected_item_type == "memory" && row.item_type != "memory" {
        return ClaimCheck::SkipWrongType {
            reason: format!(
                "wrong_item_type_for_operation:{current_op}: got item_type={}",
                row.item_type
            ),
        };
    }
    if expected_item_type == "entity"
        && row.item_type != "entity"
        && !row.item_key.starts_with("entity:")
    {
        // entity-descriptions: accept item_type=entity; bare entity names ok
        if row.item_type == "entity_pair" || is_non_memory_key_shape(&row.item_key) {
            return ClaimCheck::SkipWrongType {
                reason: format!(
                    "wrong_item_type_for_operation:{current_op}: got item_type={}",
                    row.item_type
                ),
            };
        }
    }
    if expected_item_type == "entity_pair"
        && row.item_type != "entity_pair"
        && !row.item_key.starts_with("pair:")
    {
        return ClaimCheck::SkipWrongType {
            reason: format!(
                "wrong_item_type_for_operation:{current_op}: got item_type={}",
                row.item_type
            ),
        };
    }
    ClaimCheck::Ok
}

/// Put a wrongly claimed row back to pending without consuming the attempt
/// budget (defense in depth if claim filter ever races).
pub(super) fn requeue_wrong_op(
    queue_conn: &rusqlite::Connection,
    queue_id: i64,
) -> Result<(), AppError> {
    queue_conn.execute(
        "UPDATE queue SET status='pending', \
         attempt=CASE WHEN attempt > 0 THEN attempt - 1 ELSE 0 END, \
         claimed_at=NULL, error=NULL, error_class=NULL \
         WHERE id=?1",
        rusqlite::params![queue_id],
    )?;
    Ok(())
}

/// Skip a claimed row that has an incompatible item_type/key shape.
pub(super) fn skip_wrong_type(
    queue_conn: &rusqlite::Connection,
    queue_id: i64,
    reason: &str,
) -> Result<(), AppError> {
    queue_conn.execute(
        "UPDATE queue SET status='skipped', error=?1, done_at=datetime('now'), claimed_at=NULL \
         WHERE id=?2",
        rusqlite::params![reason, queue_id],
    )?;
    Ok(())
}

/// GAP-SG-145: mark a claimed queue row `done` with the per-item accounting
/// the drain collected.
///
/// The serial loop and every parallel worker previously carried a byte-for-byte
/// copy of this `UPDATE`; the twin copies were free to drift in silence. The
/// emitted SQL is unchanged, so this is a pure de-duplication with no behaviour
/// change. `entities`/`rels` are cast to `i64` here because the queue schema
/// stores counters as integers while the callers count with `usize`.
///
/// Returns the number of updated rows; callers decide whether a failed write is
/// logged (serial) or ignored (parallel worker), preserving their current
/// behaviour.
// One parameter per column the completion UPDATE writes, plus the row it
// targets: the arity IS the queue schema.
#[allow(clippy::too_many_arguments)]
pub(super) fn mark_done(
    queue_conn: &rusqlite::Connection,
    queue_id: i64,
    memory_id: Option<i64>,
    entity_id: Option<i64>,
    entities: usize,
    rels: usize,
    cost_usd: f64,
    elapsed_ms: i64,
) -> Result<usize, rusqlite::Error> {
    queue_conn.execute(
        "UPDATE queue SET status='done', memory_id=?1, entity_id=?2, entities=?3, rels=?4, cost_usd=?5, elapsed_ms=?6, done_at=datetime('now') WHERE id=?7",
        rusqlite::params![
            memory_id,
            entity_id,
            entities as i64,
            rels as i64,
            cost_usd,
            elapsed_ms,
            queue_id
        ],
    )
}

/// GAP-SG-145: mark a claimed queue row `skipped` with a human-readable reason.
///
/// Shared by the `Skipped` and `PreservationFailed` arms of both drains, which
/// each held their own copy of this statement. Distinct from
/// [`skip_wrong_type`], which additionally clears `claimed_at` because it
/// rejects a row that was never handed to a handler.
pub(super) fn mark_skipped(
    queue_conn: &rusqlite::Connection,
    queue_id: i64,
    reason: &str,
) -> Result<usize, rusqlite::Error> {
    queue_conn.execute(
        "UPDATE queue SET status='skipped', error=?1, done_at=datetime('now') WHERE id=?2",
        rusqlite::params![reason, queue_id],
    )
}

/// GAP-SG-145: release a claimed row back to `pending` before a rate-limit
/// sleep, so another worker (or this one after the backoff) can reclaim it.
///
/// The consumed `attempt` is deliberately NOT refunded: the call was really
/// issued and the provider really rejected it, so refunding would let a
/// permanently throttled item retry forever past `--max-attempts`.
pub(super) fn requeue_rate_limited(
    queue_conn: &rusqlite::Connection,
    queue_id: i64,
) -> Result<usize, rusqlite::Error> {
    queue_conn.execute(
        "UPDATE queue SET status='pending' WHERE id=?1",
        rusqlite::params![queue_id],
    )
}

/// GAP-CLI-QISO-01: claim the next pending row **for a single operation + namespace**.
///
/// `operation` must match the Debug label used at enqueue (`"EntityDescriptions"`,
/// `"MemoryBindings"`, …). Rows with `LegacyUnscoped` / other ops are never claimed.
///
/// CAPA (dim-migrate 2026-07-30): claim MUST filter `namespace = ?2`. Without it,
/// a drain for `ai-sdd` claimed `global`/empty-ns keys and failed with
/// `chunk N not found in namespace 'ai-sdd'`, tripped the circuit breaker, and
/// produced zero successful re-embeds on multi-namespace DBs.
pub(super) fn dequeue_next_pending(
    queue_conn: &rusqlite::Connection,
    operation: &str,
    namespace: &str,
    backoff_clause: &str,
) -> Result<DequeueOutcome, AppError> {
    // GAP-CLI-PRIO-03/04: claim highest priority first (hot > normal), then id.
    // GAP-CLI-QISO-01: strict operation filter — no OR operation IS NULL.
    // CAPA: strict namespace filter — no OR namespace = '' (legacy residual
    // empty-ns rows must be re-enqueued under the correct namespace, not claimed
    // under whatever process happens to be draining).
    let dequeue_sql = format!(
        "UPDATE queue SET status='processing', attempt=attempt+1, \
         claimed_at=CAST(strftime('%s','now') AS INTEGER) \
         WHERE id = (SELECT id FROM queue WHERE status='pending' \
                     AND operation = ?1 \
                     AND namespace = ?2 \
                     {backoff_clause} \
                     ORDER BY COALESCE(priority, 0) DESC, id ASC LIMIT 1) \
         RETURNING id, item_key, item_type, COALESCE(operation,''), attempt"
    );
    match queue_conn.query_row(
        &dequeue_sql,
        rusqlite::params![operation, namespace],
        |row| {
            Ok(ClaimedRow {
                id: row.get(0)?,
                item_key: row.get(1)?,
                item_type: row.get(2)?,
                operation: row.get(3)?,
                attempt: row.get(4)?,
            })
        },
    ) {
        Ok(claimed) => Ok(DequeueOutcome::Claimed(claimed)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(DequeueOutcome::Empty),
        Err(e) => Err(AppError::Database(e)),
    }
}

/// GAP-SG-141 (B1): claim up to `limit` pending rows in ONE statement, for a
/// single operation + namespace.
///
/// Exists solely for `re-embed`, whose handler can embed N texts in a single
/// REST call: claiming one row per request made the drain issue ~32x more
/// requests than the payload needed. The 13 chat-backed operations keep using
/// [`dequeue_next_pending`], which is unchanged — they have no batch handler,
/// so claiming several rows for them would only widen the blast radius of a
/// crash.
///
/// The `operation` and `namespace` filters are as strict as the single-row
/// claim's, for the same reason documented there: without them a drain claims
/// keys belonging to another namespace and every re-embed fails with
/// `not found in namespace`.
///
/// Rows are consumed with `query_map`, NOT `query_row`: `RETURNING` emits one
/// row per updated record and `query_row` would silently discard every row
/// past the first, leaving the rest claimed-but-unprocessed until the stale
/// claim sweep. Multi-row `RETURNING` needs SQLite 3.35+; the bundled
/// `rusqlite` build embeds the 3.50 series.
///
/// Returns the claimed rows in claim order. An empty vector means the backlog
/// for this operation + namespace is empty.
pub(super) fn dequeue_batch_pending(
    queue_conn: &rusqlite::Connection,
    operation: &str,
    namespace: &str,
    backoff_clause: &str,
    limit: usize,
) -> Result<Vec<ClaimedRow>, AppError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let dequeue_sql = format!(
        "UPDATE queue SET status='processing', attempt=attempt+1, \
         claimed_at=CAST(strftime('%s','now') AS INTEGER) \
         WHERE id IN (SELECT id FROM queue WHERE status='pending' \
                      AND operation = ?1 \
                      AND namespace = ?2 \
                      {backoff_clause} \
                      ORDER BY COALESCE(priority, 0) DESC, id ASC LIMIT ?3) \
         RETURNING id, item_key, item_type, COALESCE(operation,''), attempt"
    );
    let mut stmt = queue_conn.prepare(&dequeue_sql)?;
    let rows = stmt.query_map(
        rusqlite::params![operation, namespace, limit as i64],
        |row| {
            Ok(ClaimedRow {
                id: row.get(0)?,
                item_key: row.get(1)?,
                item_type: row.get(2)?,
                operation: row.get(3)?,
                attempt: row.get(4)?,
            })
        },
    )?;
    let mut claimed = Vec::with_capacity(limit);
    for row in rows {
        claimed.push(row?);
    }
    Ok(claimed)
}

/// CAPA-A (2026-07-30): count pending rows eligible for **this** operation +
/// namespace only — same isolation as [`dequeue_next_pending`].
///
/// `--until-empty` previously counted *all* pending rows across operations,
/// so alien ReEmbed zombies kept EntityDescriptions spinning until max-runtime
/// with `completed=0`. Strict op+ns (no `OR operation IS NULL`) matches claim.
///
/// `backoff_clause` is the same fragment drain uses (may be empty or
/// `AND (next_retry_at IS NULL OR …)`); it is interpolated, not bound.
pub(super) fn count_eligible_pending(
    queue_conn: &rusqlite::Connection,
    operation: &str,
    namespace: &str,
    backoff_clause: &str,
) -> i64 {
    let sql = format!(
        "SELECT COUNT(*) FROM queue WHERE status='pending' \
         AND operation = ?1 AND namespace = ?2 {backoff_clause}"
    );
    queue_conn
        .query_row(&sql, rusqlite::params![operation, namespace], |r| r.get(0))
        .unwrap_or(0)
}

/// CAPA-B/F: reopen `skipped`/`done` EntityDescriptions rows for force-redescribe
/// scan candidates so `INSERT OR IGNORE` is not a silent no-op.
///
/// Call **once per process** before the first enqueue — not on every
/// `--until-empty` re-scan — or preservation_failed items loop forever.
/// Never reopens `dead` (use `--requeue-dead`).
///
/// Returns the number of rows updated to `pending`.
pub(super) fn reopen_force_redescribe_candidates(
    queue_conn: &rusqlite::Connection,
    namespace: &str,
    keys: &[String],
) -> usize {
    if keys.is_empty() {
        return 0;
    }
    let mut total = 0usize;
    for key in keys {
        match queue_conn.execute(
            "UPDATE queue SET status='pending', attempt=0, next_retry_at=NULL, \
             error=NULL, error_class=NULL, claimed_at=NULL, done_at=NULL \
             WHERE operation = 'EntityDescriptions' \
               AND namespace = ?1 \
               AND item_key = ?2 \
               AND status IN ('skipped', 'done')",
            rusqlite::params![namespace, key],
        ) {
            Ok(n) => total = total.saturating_add(n),
            Err(e) => {
                tracing::warn!(
                    target: "enrich",
                    error = %e,
                    key,
                    "force-redescribe reopen failed for key"
                );
            }
        }
    }
    total
}

/// CAPA-E: reset `processing` → `pending` scoped to one operation + namespace
/// (`--resume` and graceful SIGTERM). Avoids stealing in-flight claims of
/// other ops sharing the same sidecar.
pub(super) fn reset_processing_for_op(
    queue_conn: &rusqlite::Connection,
    operation: &str,
    namespace: &str,
) -> Result<usize, AppError> {
    let n = queue_conn.execute(
        "UPDATE queue SET status='pending', claimed_at=NULL \
         WHERE status='processing' AND operation = ?1 AND namespace = ?2",
        rusqlite::params![operation, namespace],
    )?;
    Ok(n)
}

/// CAPA-E: reset `failed` → `pending` for one operation + namespace (`--retry-failed`).
pub(super) fn reset_failed_for_op(
    queue_conn: &rusqlite::Connection,
    operation: &str,
    namespace: &str,
) -> Result<usize, AppError> {
    let n = queue_conn.execute(
        "UPDATE queue SET status='pending', attempt=0, next_retry_at=NULL, \
         error=NULL, error_class=NULL, claimed_at=NULL \
         WHERE status='failed' AND operation = ?1 AND namespace = ?2",
        rusqlite::params![operation, namespace],
    )?;
    Ok(n)
}

/// CAPA-C: true when a live embedding BLOB of the target dim exists
/// (`LENGTH(embedding) = dim*4`), not merely a matching `dim` column.
pub(super) fn entity_has_live_embedding(
    main_conn: &rusqlite::Connection,
    entity_id: i64,
    dim: usize,
) -> bool {
    let bytes = (dim * 4) as i64;
    main_conn
        .query_row(
            "SELECT 1 FROM entity_embeddings \
             WHERE entity_id = ?1 AND LENGTH(embedding) = ?2 LIMIT 1",
            rusqlite::params![entity_id, bytes],
            |_| Ok(()),
        )
        .is_ok()
}

/// CAPA-C: live memory embedding at target dim (BLOB length).
pub(super) fn memory_has_live_embedding(
    main_conn: &rusqlite::Connection,
    memory_id: i64,
    dim: usize,
) -> bool {
    let bytes = (dim * 4) as i64;
    main_conn
        .query_row(
            "SELECT 1 FROM memory_embeddings \
             WHERE memory_id = ?1 AND LENGTH(embedding) = ?2 LIMIT 1",
            rusqlite::params![memory_id, bytes],
            |_| Ok(()),
        )
        .is_ok()
}

/// CAPA-C: live chunk embedding at target dim (BLOB length).
pub(super) fn chunk_has_live_embedding(
    main_conn: &rusqlite::Connection,
    chunk_id: i64,
    dim: usize,
) -> bool {
    let bytes = (dim * 4) as i64;
    main_conn
        .query_row(
            "SELECT 1 FROM chunk_embeddings \
             WHERE chunk_id = ?1 AND LENGTH(embedding) = ?2 LIMIT 1",
            rusqlite::params![chunk_id, bytes],
            |_| Ok(()),
        )
        .is_ok()
}

/// CAPA-C2: mark pending ReEmbed rows `done` when the main DB already has a
/// live vector at the active dim — clears zombie pending without API calls.
///
/// Workload: serial SQLite I/O (orchestrator only; run before parallel drain).
pub(super) fn reconcile_satisfied_reembed_pending(
    main_conn: &rusqlite::Connection,
    queue_conn: &rusqlite::Connection,
    namespace: &str,
) -> Result<usize, AppError> {
    let dim = crate::constants::embedding_dim();
    let mut stmt = queue_conn.prepare(
        "SELECT id, item_key FROM queue \
         WHERE status='pending' AND operation='ReEmbed' AND namespace=?1",
    )?;
    let rows: Vec<(i64, String)> = stmt
        .query_map(rusqlite::params![namespace], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut reconciled = 0usize;
    for (id, key) in rows {
        let satisfied = if let Some(name) = key.strip_prefix("entity:") {
            match main_conn.query_row(
                "SELECT id FROM entities WHERE namespace=?1 AND name=?2",
                rusqlite::params![namespace, name],
                |r| r.get::<_, i64>(0),
            ) {
                Ok(eid) => entity_has_live_embedding(main_conn, eid, dim),
                Err(_) => false,
            }
        } else if let Some(chunk_key) = key.strip_prefix("chunk:") {
            match chunk_key.parse::<i64>() {
                Ok(cid) => chunk_has_live_embedding(main_conn, cid, dim),
                Err(_) => false,
            }
        } else {
            match main_conn.query_row(
                "SELECT id FROM memories WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
                rusqlite::params![namespace, key],
                |r| r.get::<_, i64>(0),
            ) {
                Ok(mid) => memory_has_live_embedding(main_conn, mid, dim),
                Err(_) => false,
            }
        };
        if !satisfied {
            continue;
        }
        let n = queue_conn.execute(
            "UPDATE queue SET status='done', done_at=datetime('now'), claimed_at=NULL, \
             error='reconciled: live embedding already present' \
             WHERE id=?1 AND status='pending'",
            rusqlite::params![id],
        )?;
        reconciled = reconciled.saturating_add(n);
    }
    Ok(reconciled)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// GAP-SG-270: proves the origin-typed retry verdict survives the whole path
/// from `EmbedError` to the queue classifier.
#[cfg(test)]
mod retry_class_preservation_tests {
    use super::classify_enrich_outcome;
    use crate::embedder::app_error_preserving_retry_class;
    use crate::embedding_api::EmbedError;
    use crate::errors::AppError;
    use crate::retry::AttemptOutcome;

    /// The failure the OpenRouter transport produces, with its origin verdict.
    fn embed_failure(retry_class: AttemptOutcome) -> EmbedError {
        EmbedError {
            source: AppError::Embedding("openrouter returned status 400".to_string()),
            retry_class,
        }
    }

    #[test]
    fn permanent_embed_failure_reaches_the_queue_as_permanent() {
        let err = app_error_preserving_retry_class(embed_failure(AttemptOutcome::HardFailure));
        assert_eq!(
            classify_enrich_outcome(&err),
            AttemptOutcome::HardFailure,
            "a permanent embedding failure must dead-letter instead of burning --max-attempts"
        );
        // The operator-facing contract is unchanged by the extra field.
        assert_eq!(
            err.to_string(),
            "embedding error: openrouter returned status 400"
        );
        assert_eq!(err.exit_code(), 11);
    }

    #[test]
    fn transient_embed_failure_stays_transient() {
        let err = app_error_preserving_retry_class(embed_failure(AttemptOutcome::Transient));
        assert_eq!(classify_enrich_outcome(&err), AttemptOutcome::Transient);
    }

    #[test]
    fn embed_failure_without_a_verdict_keeps_the_conservative_transient_floor() {
        let err = AppError::Embedding("client not initialised".to_string());
        assert_eq!(
            classify_enrich_outcome(&err),
            AttemptOutcome::Transient,
            "an absent verdict must NOT be promoted to permanent"
        );
    }

    #[test]
    fn already_typed_sources_are_forwarded_untouched() {
        let err = app_error_preserving_retry_class(EmbedError {
            source: AppError::RateLimited {
                detail: "429".to_string(),
            },
            retry_class: AttemptOutcome::Transient,
        });
        assert!(matches!(err, AppError::RateLimited { .. }));
        assert_eq!(classify_enrich_outcome(&err), AttemptOutcome::Transient);
    }
}
