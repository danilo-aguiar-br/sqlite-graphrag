//! Terminal transitions shared by both drains (GAP-SG-146).
//!
//! `mark_done`, `mark_skipped` and the rate-limit release must leave the same
//! row behind whether the serial loop or a parallel worker wrote it.

use super::test_fixtures::{insert_pending, open_temp_queue};
use super::*;

/// GAP-SG-145: the serial loop and a parallel worker hold SEPARATE
/// connections to the same queue file. Both now route completion through
/// `mark_done`, so the row they leave behind must be byte-identical. Before
/// the extraction each side carried its own copy of the `UPDATE`, free to
/// drift in silence — this test is the guard against that regression.
#[test]
fn mark_done_is_identical_from_serial_and_worker_connections() {
    let (serial_conn, path) = open_temp_queue();
    // Mirrors `drain_parallel`, which opens its own handle per worker.
    let worker_conn = open_queue_db(&path).expect("worker queue db must open");

    let serial_id = insert_pending(&serial_conn, "mem-serial");
    let worker_id = insert_pending(&serial_conn, "mem-worker");

    let n_serial = mark_done(&serial_conn, serial_id, Some(11), Some(22), 3, 4, 0.5, 120).unwrap();
    let n_worker = mark_done(&worker_conn, worker_id, Some(11), Some(22), 3, 4, 0.5, 120).unwrap();
    assert_eq!(n_serial, 1);
    assert_eq!(n_worker, 1);

    type DoneRow = (String, i64, i64, i64, i64, f64, i64, bool);
    let read_row = |id: i64| -> DoneRow {
        serial_conn
            .query_row(
                "SELECT status, memory_id, entity_id, entities, rels, cost_usd, elapsed_ms, \
                 done_at IS NOT NULL FROM queue WHERE id=?1",
                rusqlite::params![id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                    ))
                },
            )
            .unwrap()
    };

    let from_serial = read_row(serial_id);
    let from_worker = read_row(worker_id);
    assert_eq!(from_serial, from_worker);
    assert_eq!(
        from_serial,
        ("done".to_string(), 11, 22, 3, 4, 0.5, 120, true)
    );
    let _ = std::fs::remove_file(&path);
}

/// GAP-SG-145: same contract for the `skipped` transition, which both drains
/// reach from the `Skipped` and `PreservationFailed` arms.
#[test]
fn mark_skipped_is_identical_from_serial_and_worker_connections() {
    let (serial_conn, path) = open_temp_queue();
    let worker_conn = open_queue_db(&path).expect("worker queue db must open");

    let serial_id = insert_pending(&serial_conn, "skip-serial");
    let worker_id = insert_pending(&serial_conn, "skip-worker");

    mark_skipped(&serial_conn, serial_id, "body is empty").unwrap();
    mark_skipped(&worker_conn, worker_id, "body is empty").unwrap();

    let read_row = |id: i64| -> (String, String, bool) {
        serial_conn
            .query_row(
                "SELECT status, COALESCE(error,''), done_at IS NOT NULL FROM queue WHERE id=?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap()
    };
    assert_eq!(read_row(serial_id), read_row(worker_id));
    assert_eq!(
        read_row(serial_id),
        ("skipped".to_string(), "body is empty".to_string(), true)
    );
    let _ = std::fs::remove_file(&path);
}

/// GAP-SG-145: the rate-limit release returns the row to `pending` WITHOUT
/// refunding the consumed attempt — refunding would let a permanently
/// throttled item outlive `--max-attempts`.
#[test]
fn requeue_rate_limited_keeps_the_consumed_attempt() {
    let (conn, path) = open_temp_queue();
    let id = insert_pending(&conn, "mem-throttled");
    // `insert_pending` leaves the schema default namespace (empty string).
    match dequeue_next_pending(&conn, "MemoryBindings", "", "").unwrap() {
        DequeueOutcome::Claimed(row) => assert_eq!(row.attempt, 1),
        DequeueOutcome::Empty => panic!("expected a claim"),
    }

    requeue_rate_limited(&conn, id).unwrap();

    let (status, attempt): (String, i64) = conn
        .query_row(
            "SELECT status, attempt FROM queue WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(attempt, 1, "attempt must NOT be refunded");
    let _ = std::fs::remove_file(&path);
}
