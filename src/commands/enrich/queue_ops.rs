//! Queue claim/failure helpers (Wave C1).

use crate::errors::AppError;


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
        // the typed variant, never a message substring (rules_rust_retry: NUNCA
        // string matching). The `--max-attempts` floor (default 8) still ends
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
        // GAP-SG-73: safe floor for the `re-embed` operation. `AppError::Embedding`
        // reaches here only via `embed_with_fallback`'s backend-chain resolution
        // (`crate::embedder`), which discards the origin-typed
        // `EmbedError::retry_class` through `From<EmbedError> for AppError` before
        // the error surfaces to the queue. Extracting the precise verdict would
        // require bypassing the fallback chain to call the OpenRouter embedding
        // client directly — out of scope here (touches `embedder.rs`, which is
        // off-limits, and removes the multi-backend fallback safety net).
        // Transient is the conservative choice: a persistently permanent failure
        // still terminates via `--max-attempts` instead of retrying forever.
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
    if expected_item_type == "entity" && row.item_type != "entity" && !row.item_key.starts_with("entity:") {
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

/// GAP-CLI-QISO-01: claim the next pending row **for a single operation**.
///
/// `operation` must match the Debug label used at enqueue (`"EntityDescriptions"`,
/// `"MemoryBindings"`, …). Rows with `LegacyUnscoped` / other ops are never claimed.
pub(super) fn dequeue_next_pending(
    queue_conn: &rusqlite::Connection,
    operation: &str,
    backoff_clause: &str,
) -> Result<DequeueOutcome, AppError> {
    // GAP-CLI-PRIO-03/04: claim highest priority first (hot > normal), then id.
    // GAP-CLI-QISO-01: strict operation filter — no OR operation IS NULL.
    let dequeue_sql = format!(
        "UPDATE queue SET status='processing', attempt=attempt+1, \
         claimed_at=CAST(strftime('%s','now') AS INTEGER) \
         WHERE id = (SELECT id FROM queue WHERE status='pending' \
                     AND operation = ?1 {backoff_clause} \
                     ORDER BY COALESCE(priority, 0) DESC, id ASC LIMIT 1) \
         RETURNING id, item_key, item_type, COALESCE(operation,''), attempt"
    );
    match queue_conn.query_row(&dequeue_sql, rusqlite::params![operation], |row| {
        Ok(ClaimedRow {
            id: row.get(0)?,
            item_key: row.get(1)?,
            item_type: row.get(2)?,
            operation: row.get(3)?,
            attempt: row.get(4)?,
        })
    }) {
        Ok(claimed) => Ok(DequeueOutcome::Claimed(claimed)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(DequeueOutcome::Empty),
        Err(e) => Err(AppError::Database(e)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

