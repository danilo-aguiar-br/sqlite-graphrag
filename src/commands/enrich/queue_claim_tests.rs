//! Claiming rows and keeping claims isolated (GAP-SG-146).
//!
//! Single-row and batch claims, the strict operation/namespace filters, the
//! post-claim shape validation, heartbeats and stale-claim recovery.

use super::test_fixtures::{insert_pending, insert_pending_op, open_temp_queue};
use super::*;

/// GAP-CLI-QISO-01/05: entity-connect pair keys are invisible to MB claim.
#[test]
fn dequeue_does_not_claim_pair_keys_under_memory_bindings() {
    let (conn, path) = open_temp_queue();
    let _pair = insert_pending_op(&conn, "pair:21560:159670", "entity_pair", "EntityConnect");
    assert!(matches!(
        dequeue_next_pending(&conn, "MemoryBindings", "", "").unwrap(),
        DequeueOutcome::Empty
    ));
    match dequeue_next_pending(&conn, "EntityConnect", "", "").unwrap() {
        DequeueOutcome::Claimed(row) => {
            assert_eq!(row.item_key, "pair:21560:159670");
        }
        DequeueOutcome::Empty => panic!("expected EC claim"),
    }
    let _ = std::fs::remove_file(&path);
}

/// GAP-CLI-QISO-01/05: MemoryBindings drain must not claim EntityDescriptions.
#[test]
fn dequeue_filters_by_operation_isolation() {
    let (conn, path) = open_temp_queue();
    let _ed_id = insert_pending_op(&conn, "aeroporto-guarulhos", "entity", "EntityDescriptions");
    let mb_id = insert_pending_op(&conn, "yt-memory", "memory", "MemoryBindings");

    match dequeue_next_pending(&conn, "MemoryBindings", "", "").unwrap() {
        DequeueOutcome::Claimed(row) => {
            assert_eq!(row.id, mb_id);
            assert_eq!(row.item_key, "yt-memory");
            assert_eq!(row.operation, "MemoryBindings");
        }
        DequeueOutcome::Empty => panic!("expected MB claim"),
    }
    assert!(matches!(
        dequeue_next_pending(&conn, "MemoryBindings", "", "").unwrap(),
        DequeueOutcome::Empty
    ));
    match dequeue_next_pending(&conn, "EntityDescriptions", "", "").unwrap() {
        DequeueOutcome::Claimed(row) => {
            assert_eq!(row.item_key, "aeroporto-guarulhos");
            assert_eq!(row.operation, "EntityDescriptions");
        }
        DequeueOutcome::Empty => panic!("expected ED claim"),
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn dequeue_next_pending_distinguishes_empty_from_claimed() {
    let (conn, path) = open_temp_queue();
    let id = insert_pending(&conn, "mem-dequeue");
    let claimed =
        dequeue_next_pending(&conn, "MemoryBindings", "", "").expect("dequeue must succeed");
    match claimed {
        DequeueOutcome::Claimed(row) => {
            assert_eq!(row.id, id);
            assert_eq!(row.item_key, "mem-dequeue");
            assert_eq!(row.operation, "MemoryBindings");
        }
        DequeueOutcome::Empty => panic!("expected a claimed row"),
    }
    let empty =
        dequeue_next_pending(&conn, "MemoryBindings", "", "").expect("dequeue must succeed");
    assert!(matches!(empty, DequeueOutcome::Empty));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn dequeue_next_pending_isolates_by_namespace() {
    // CAPA: drain for ns A must never claim pending rows enqueued under ns B.
    let (conn, path) = open_temp_queue();
    conn.execute(
        "INSERT INTO queue (namespace, item_key, item_type, status, operation)
             VALUES (\"global\", \"chunk:1\", \"chunk\", \"pending\", \"ReEmbed\")",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO queue (namespace, item_key, item_type, status, operation)
             VALUES (\"ai-sdd\", \"entity:x\", \"entity\", \"pending\", \"ReEmbed\")",
        [],
    )
    .unwrap();
    // Claiming under ai-sdd must not surface the global chunk key.
    match dequeue_next_pending(&conn, "ReEmbed", "ai-sdd", "").unwrap() {
        DequeueOutcome::Claimed(row) => {
            assert_eq!(row.item_key, "entity:x");
        }
        DequeueOutcome::Empty => panic!("expected ai-sdd claim"),
    }
    // global still pending
    let still: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM queue WHERE namespace=\"global\" AND status=\"pending\"",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(still, 1);
    // wrong ns empty
    assert!(matches!(
        dequeue_next_pending(&conn, "ReEmbed", "ai-research", "").unwrap(),
        DequeueOutcome::Empty
    ));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn dequeue_skips_future_retry_and_dead() {
    let (conn, path) = open_temp_queue();
    let eligible = insert_pending(&conn, "mem-eligible");
    let waiting = insert_pending(&conn, "mem-waiting");
    conn.execute(
        "UPDATE queue SET next_retry_at=datetime('now', '+3600 seconds') WHERE id=?1",
        rusqlite::params![waiting],
    )
    .unwrap();
    let dead = insert_pending(&conn, "mem-dead");
    conn.execute(
        "UPDATE queue SET status='dead' WHERE id=?1",
        rusqlite::params![dead],
    )
    .unwrap();

    let claimed: Option<i64> = conn
        .query_row(
            "UPDATE queue SET status='processing', attempt=attempt+1 \
                 WHERE id = (SELECT id FROM queue WHERE status='pending' \
                               AND (next_retry_at IS NULL OR next_retry_at <= datetime('now')) \
                             ORDER BY id LIMIT 1) \
                 RETURNING id",
            [],
            |r| r.get(0),
        )
        .ok();
    assert_eq!(claimed, Some(eligible));

    let second: Option<i64> = conn
        .query_row(
            "UPDATE queue SET status='processing', attempt=attempt+1 \
                 WHERE id = (SELECT id FROM queue WHERE status='pending' \
                               AND (next_retry_at IS NULL OR next_retry_at <= datetime('now')) \
                             ORDER BY id LIMIT 1) \
                 RETURNING id",
            [],
            |r| r.get(0),
        )
        .ok();
    assert_eq!(second, None);
    let _ = std::fs::remove_file(&path);
}

// v1.1.2 (Bug 4): a fresh claim (within the threshold) is preserved so an
// in-flight worker is not preempted by the sweep.
#[test]
fn fresh_processing_claim_is_preserved() {
    let (conn, path) = open_temp_queue();
    let id = insert_pending(&conn, "mem-fresh");
    // Claim taken now.
    conn.execute(
            "UPDATE queue SET status='processing', claimed_at = CAST(strftime('%s','now') AS INTEGER) WHERE id=?1",
            rusqlite::params![id],
        )
        .unwrap();
    let reset = reset_stale_processing_claims(&conn, 1800).unwrap();
    assert_eq!(reset, 0, "a fresh claim must not be reset");
    let status: String = conn
        .query_row(
            "SELECT status FROM queue WHERE id=?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "processing");
    let _ = std::fs::remove_file(&path);
}

// v1.1.2 (Bug 4): heartbeat refreshes claimed_at so a slow LLM call is not
// mistaken for a stale claim.
#[test]
fn heartbeat_updates_claimed_at() {
    let (conn, path) = open_temp_queue();
    let id = insert_pending(&conn, "mem-heartbeat");
    // Stale claim taken 2 hours ago.
    conn.execute(
            "UPDATE queue SET status='processing', claimed_at = CAST(strftime('%s','now') AS INTEGER) - 7200 WHERE id=?1",
            rusqlite::params![id],
        )
        .unwrap();
    heartbeat(&conn, id).unwrap();
    let claimed_at: Option<i64> = conn
        .query_row(
            "SELECT claimed_at FROM queue WHERE id=?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap();
    let claimed_at = claimed_at.expect("claimed_at must be set after heartbeat");
    let now: i64 = conn
        .query_row("SELECT CAST(strftime('%s','now') AS INTEGER)", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap();
    assert!(
        now - claimed_at <= 5,
        "claimed_at must be within 5s of now after heartbeat"
    );
    // And the row is no longer stale under a 1800s threshold.
    let reset = reset_stale_processing_claims(&conn, 1800).unwrap();
    assert_eq!(reset, 0, "fresh claim survives the sweep after heartbeat");
    let _ = std::fs::remove_file(&path);
}

/// GAP-CLI-QISO-01: LegacyUnscoped rows are never claimable by named ops.
#[test]
fn legacy_unscoped_rows_are_not_claimed() {
    let (conn, path) = open_temp_queue();
    conn.execute(
        "INSERT INTO queue (item_key, item_type, status, operation) \
             VALUES ('orphan-legacy', 'memory', 'pending', 'LegacyUnscoped')",
        [],
    )
    .unwrap();
    assert!(matches!(
        dequeue_next_pending(&conn, "MemoryBindings", "", "").unwrap(),
        DequeueOutcome::Empty
    ));
    let _ = std::fs::remove_file(&path);
}

// v1.1.2 (Bug 4): a `processing` row whose claimed_at is older than the
// threshold is reset to `pending` so a kill -9 does not strand it forever.
#[test]
fn stale_processing_claim_is_reset_after_threshold() {
    let (conn, path) = open_temp_queue();
    let id = insert_pending(&conn, "mem-stale");
    // Simulate a claim taken long ago (2 hours back).
    conn.execute(
            "UPDATE queue SET status='processing', claimed_at = CAST(strftime('%s','now') AS INTEGER) - 7200 WHERE id=?1",
            rusqlite::params![id],
        )
        .unwrap();
    let reset = reset_stale_processing_claims(&conn, 1800).unwrap();
    assert_eq!(reset, 1, "a stale claim older than the threshold is reset");
    let (status, claimed_at): (String, Option<i64>) = conn
        .query_row(
            "SELECT status, claimed_at FROM queue WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "pending");
    assert!(claimed_at.is_none(), "claimed_at must be cleared on reset");
    let _ = std::fs::remove_file(&path);
}

/// GAP-CLI-QISO-03/04: validate_claim rejects wrong type and key shape.
#[test]
fn validate_claim_rejects_wrong_type_and_key_shape() {
    let pair = ClaimedRow {
        id: 1,
        item_key: "pair:1:2".into(),
        item_type: "entity_pair".into(),
        operation: "MemoryBindings".into(),
        attempt: 1,
    };
    match validate_claim(&pair, "MemoryBindings", "memory") {
        ClaimCheck::SkipWrongType { reason } => {
            assert!(reason.contains("wrong_"), "reason={reason}");
        }
        other => panic!("expected SkipWrongType, got {other:?}"),
    }
    let entity = ClaimedRow {
        id: 2,
        item_key: "aeroporto-guarulhos".into(),
        item_type: "entity".into(),
        operation: "EntityDescriptions".into(),
        attempt: 1,
    };
    assert_eq!(
        validate_claim(&entity, "MemoryBindings", "memory"),
        ClaimCheck::RequeueWrongOp
    );
    assert_eq!(
        validate_claim(&entity, "EntityDescriptions", "entity"),
        ClaimCheck::Ok
    );
    assert!(is_non_memory_key_shape("pair:1:2"));
    assert!(is_non_memory_key_shape("entity:99"));
    assert!(!is_non_memory_key_shape("plain-memory-name"));
}

/// GAP-SG-76/v1.1.00 fix: proves the enrich drain loops' composition
/// `with_busy_retry(|| dequeue_next_pending(...))` is BOUNDED under
/// sustained lock contention instead of the previous
/// `loop { ... continue; }`, which retried `SQLITE_BUSY` forever. A
/// second connection holds an exclusive write lock for the whole test;
/// the queue connection under test has `busy_timeout=0` so SQLite
/// reports `SQLITE_BUSY` immediately instead of blocking internally,
/// isolating the bounded backoff as the only source of delay.
///
/// The schedule is DECLARED here rather than resolved from XDG, and the
/// difference is not cosmetic. `with_busy_retry` reads `db.busy_retries` and
/// `db.busy_base_delay_ms`, so this test used to assert against the compiled
/// constant while the code under test used the operator's value: on a
/// workstation carrying `12` and `600 ms` it attempted twelve times, asserted
/// five, and spent roughly half an hour of exponential backoff before saying so.
/// A test whose verdict AND duration depend on the developer's configuration
/// proves nothing about the code.
#[test]
fn with_busy_retry_bounds_dequeue_under_sustained_contention() {
    let (conn, path) = open_temp_queue();
    insert_pending(&conn, "mem-busy");
    conn.pragma_update(None, "busy_timeout", 0i64)
        .expect("busy_timeout override must succeed");

    // Second connection holds an EXCLUSIVE write lock so every dequeue
    // attempt on `conn` observes SQLITE_BUSY, never SQLITE_LOCKED-then-
    // clears-up.
    let blocker = Connection::open(&path).expect("blocker connection must open");
    blocker
        .execute_batch("BEGIN EXCLUSIVE;")
        .expect("exclusive lock must be acquired");

    let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let calls_clone = std::sync::Arc::clone(&calls);
    /// Attempts this test declares, independent of any XDG setting.
    const ATTEMPTS: u32 = 5;
    /// One millisecond of base delay: the subject is the BOUND, not the wait.
    const BASE_DELAY_MS: u64 = 1;

    let result: Result<DequeueOutcome, AppError> =
        crate::storage::utils::with_busy_retry_policy(ATTEMPTS, BASE_DELAY_MS, || {
            calls_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            dequeue_next_pending(&conn, "MemoryBindings", "", "")
        });

    assert!(
        matches!(result, Err(AppError::DbBusy(_))),
        "sustained SQLITE_BUSY must convert to DbBusy, not hang or silently report Empty"
    );
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        ATTEMPTS,
        "must attempt exactly the declared budget, never retry unbounded"
    );

    blocker
        .execute_batch("ROLLBACK;")
        .expect("releasing the exclusive lock must succeed");
    let _ = std::fs::remove_file(&path);
}
