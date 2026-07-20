//! Auto-extracted tests (Wave C1).

    use super::*;

    #[allow(dead_code)]
    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE memories (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace   TEXT NOT NULL DEFAULT 'global',
                name        TEXT NOT NULL,
                type        TEXT NOT NULL DEFAULT 'note',
                description TEXT NOT NULL DEFAULT '',
                body        TEXT NOT NULL DEFAULT '',
                body_hash   TEXT NOT NULL DEFAULT '',
                session_id  TEXT,
                source      TEXT NOT NULL DEFAULT 'agent',
                metadata    TEXT NOT NULL DEFAULT '{}',
                created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                deleted_at  INTEGER,
                UNIQUE(namespace, name)
            );",
        )
        .expect("schema creation must succeed");
        conn
    }

    fn open_temp_queue() -> (Connection, String) {
        let path = format!(
            "/tmp/test-enrich-dl-{}-{}.sqlite",
            std::process::id(),
            fastrand::u64(..)
        );
        let conn = open_queue_db(&path).expect("queue db must open");
        (conn, path)
    }

    fn insert_pending(conn: &Connection, key: &str) -> i64 {
        insert_pending_op(conn, key, "memory", "MemoryBindings")
    }

    fn insert_pending_op(conn: &Connection, key: &str, item_type: &str, operation: &str) -> i64 {
        conn.execute(
            "INSERT INTO queue (item_key, item_type, status, operation) VALUES (?1, ?2, 'pending', ?3)",
            rusqlite::params![key, item_type, operation],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn queue_db_schema_creates_correctly() {
        let tmp_path = format!("/tmp/test-enrich-queue-{}.sqlite", std::process::id());
        let conn = open_queue_db(&tmp_path).expect("queue db must open");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        let _ = std::fs::remove_file(&tmp_path);
    }

    #[test]
    fn classify_rate_limit_is_transient() {
        let e = AppError::RateLimited {
            detail: "429".into(),
        };
        assert_eq!(
            classify_enrich_outcome(&e),
            crate::retry::AttemptOutcome::Transient
        );
    }

    #[test]
    fn classify_timeout_and_dbbusy_are_transient() {
        let t = AppError::Timeout {
            operation: "judge".into(),
            duration_secs: 30,
        };
        let b = AppError::DbBusy("locked".into());
        assert_eq!(
            classify_enrich_outcome(&t),
            crate::retry::AttemptOutcome::Transient
        );
        assert_eq!(
            classify_enrich_outcome(&b),
            crate::retry::AttemptOutcome::Transient
        );
    }

    #[test]
    fn classify_validation_and_parse_are_hard_failure() {
        let v = AppError::Validation("failed to parse entities array: bad".into());
        assert_eq!(
            classify_enrich_outcome(&v),
            crate::retry::AttemptOutcome::HardFailure
        );
    }

    #[test]
    fn open_queue_db_alter_is_idempotent() {
        let path = format!(
            "/tmp/test-enrich-idem-{}-{}.sqlite",
            std::process::id(),
            fastrand::u64(..)
        );
        let _ = open_queue_db(&path).expect("first open");
        let conn = open_queue_db(&path).expect("second open is idempotent");
        let cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(queue)").unwrap();
            stmt.query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert!(cols.iter().any(|c| c == "error_class"));
        assert!(cols.iter().any(|c| c == "next_retry_at"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn record_item_failure_hard_marks_dead() {
        let (conn, path) = open_temp_queue();
        let id = insert_pending(&conn, "mem-hard");
        let outcome = record_item_failure(
            &conn,
            id,
            1,
            5,
            &AppError::Validation("invalid body".into()),
        );
        assert_eq!(outcome, crate::retry::AttemptOutcome::HardFailure);
        let status: String = conn
            .query_row(
                "SELECT status FROM queue WHERE id=?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "dead");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn record_item_failure_transient_reschedules_pending() {
        let (conn, path) = open_temp_queue();
        let id = insert_pending(&conn, "mem-transient");
        let outcome = record_item_failure(
            &conn,
            id,
            1,
            5,
            &AppError::RateLimited {
                detail: "429".into(),
            },
        );
        assert_eq!(outcome, crate::retry::AttemptOutcome::Transient);
        let (status, future): (String, i64) = conn
            .query_row(
                "SELECT status, (next_retry_at > datetime('now')) FROM queue WHERE id=?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "pending");
        assert_eq!(future, 1, "next_retry_at must be in the future");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn record_item_failure_transient_at_cap_marks_dead() {
        let (conn, path) = open_temp_queue();
        let id = insert_pending(&conn, "mem-cap");
        let outcome = record_item_failure(
            &conn,
            id,
            5,
            5,
            &AppError::RateLimited {
                detail: "429".into(),
            },
        );
        assert_eq!(outcome, crate::retry::AttemptOutcome::Transient);
        let status: String = conn
            .query_row(
                "SELECT status FROM queue WHERE id=?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "dead");
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

    #[test]
    fn classify_validation_never_infers_transience_from_message() {
        // GAP-SG-73: the fallback classifier is TYPED-only now. Messages
        // that used to be sniffed for "json" / "missing '" substrings and
        // treated as Transient are HardFailure here — the OpenRouter chat
        // path (the project's only supported enrich mode) attaches its own
        // typed `ChatError::retry_class` for these exact shape failures
        // BEFORE `record_item_failure_typed` ever falls back to this
        // classifier, so no message-based guessing survives in the fallback.
        for msg in [
            "model 'x' returned non-object JSON after repair (got string)",
            "model 'x' returned content that could not be parsed even after JSON repair",
            "model 'x' returned no structured content",
            "LLM result missing 'description' field",
            "LLM result missing 'enriched_body' field",
        ] {
            assert_eq!(
                classify_enrich_outcome(&AppError::Validation(msg.into())),
                crate::retry::AttemptOutcome::HardFailure,
                "expected hard failure for: {msg}"
            );
        }
    }

    #[test]
    fn classify_embedding_error_is_transient_floor() {
        assert_eq!(
            classify_enrich_outcome(&AppError::Embedding("dimension mismatch".into())),
            crate::retry::AttemptOutcome::Transient
        );
    }

    // GAP-SG-78: entity absence is Transient (own typed variant); memory
    // absence and the untyped NotFound string stay HardFailure. No substring.
    #[test]
    fn classify_entity_not_yet_materialized_is_transient() {
        assert_eq!(
            classify_enrich_outcome(&AppError::EntityNotYetMaterialized {
                name: "acme".into(),
                namespace: "global".into(),
            }),
            crate::retry::AttemptOutcome::Transient
        );
    }

    #[test]
    fn classify_memory_absence_stays_hard_failure() {
        assert_eq!(
            classify_enrich_outcome(&AppError::MemoryNotFound {
                name: "mem-x".into(),
                namespace: "global".into(),
            }),
            crate::retry::AttemptOutcome::HardFailure
        );
        assert_eq!(
            classify_enrich_outcome(&AppError::MemoryNotFoundById { id: 42 }),
            crate::retry::AttemptOutcome::HardFailure
        );
        assert_eq!(
            classify_enrich_outcome(&AppError::NotFound("gone".into())),
            crate::retry::AttemptOutcome::HardFailure
        );
    }

    #[test]
    fn classify_database_busy_is_transient_non_busy_is_hard() {
        let busy = AppError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            Some("database is locked".into()),
        ));
        assert_eq!(
            classify_enrich_outcome(&busy),
            crate::retry::AttemptOutcome::Transient
        );
        let constraint = AppError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
            Some("UNIQUE constraint failed".into()),
        ));
        assert_eq!(
            classify_enrich_outcome(&constraint),
            crate::retry::AttemptOutcome::HardFailure
        );
    }

    #[test]
    fn record_item_failure_typed_persists_diagnostics_on_dead_letter() {
        let (conn, path) = open_temp_queue();
        let id = insert_pending(&conn, "mem-diag");
        let outcome = record_item_failure_typed(
            &conn,
            id,
            1,
            5,
            crate::retry::AttemptOutcome::HardFailure,
            "truncated response",
            Some("length"),
            Some(120),
            Some(4096),
        );
        assert_eq!(outcome, crate::retry::AttemptOutcome::HardFailure);
        let (status, finish_reason, input_tokens, output_tokens): (
            String,
            Option<String>,
            Option<i64>,
            Option<i64>,
        ) = conn
            .query_row(
                "SELECT status, finish_reason, input_tokens, output_tokens FROM queue WHERE id=?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(status, "dead");
        assert_eq!(finish_reason.as_deref(), Some("length"));
        assert_eq!(input_tokens, Some(120));
        assert_eq!(output_tokens, Some(4096));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn record_item_failure_typed_reschedules_transient_below_max_attempts() {
        // GAP-SG-72-chat: a transient failure (e.g. a truncated OpenRouter
        // response) below max_attempts must stay `pending` with a
        // future `next_retry_at`, not go straight to `dead` — and it must
        // still persist the finish_reason/token diagnostics for later
        // inspection via `--list-dead` / `--status`.
        let (conn, path) = open_temp_queue();
        let id = insert_pending(&conn, "mem-retry");
        let outcome = record_item_failure_typed(
            &conn,
            id,
            1,
            5,
            crate::retry::AttemptOutcome::Transient,
            "truncated response",
            Some("length"),
            Some(120),
            Some(64),
        );
        assert_eq!(outcome, crate::retry::AttemptOutcome::Transient);
        let (status, error_class, finish_reason, next_retry_at): (
            String,
            String,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT status, error_class, finish_reason, next_retry_at FROM queue WHERE id=?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(status, "pending");
        assert_eq!(error_class, "transient");
        assert_eq!(finish_reason.as_deref(), Some("length"));
        assert!(
            next_retry_at.is_some(),
            "a rescheduled item must carry a next_retry_at"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// GAP-SG-76/v1.1.00 fix: proves the enrich drain loops' composition
    /// `with_busy_retry(|| dequeue_next_pending(...))` is BOUNDED under
    /// sustained lock contention instead of the previous
    /// `loop { ... continue; }`, which retried `SQLITE_BUSY` forever. A
    /// second connection holds an exclusive write lock for the whole test;
    /// the queue connection under test has `busy_timeout=0` so SQLite
    /// reports `SQLITE_BUSY` immediately instead of blocking internally,
    /// isolating `with_busy_retry`'s own bounded backoff (5 attempts) as the
    /// only source of delay.
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
        let result: Result<DequeueOutcome, AppError> =
            crate::storage::utils::with_busy_retry(|| {
                calls_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                dequeue_next_pending(&conn, "MemoryBindings", "")
            });

        assert!(
            matches!(result, Err(AppError::DbBusy(_))),
            "sustained SQLITE_BUSY must convert to DbBusy, not hang or silently report Empty"
        );
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            crate::constants::MAX_SQLITE_BUSY_RETRIES,
            "must attempt exactly MAX_SQLITE_BUSY_RETRIES times, never retry unbounded"
        );

        blocker
            .execute_batch("ROLLBACK;")
            .expect("releasing the exclusive lock must succeed");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn dequeue_next_pending_distinguishes_empty_from_claimed() {
        let (conn, path) = open_temp_queue();
        let id = insert_pending(&conn, "mem-dequeue");
        let claimed = dequeue_next_pending(&conn, "MemoryBindings", "").expect("dequeue must succeed");
        match claimed {
            DequeueOutcome::Claimed(row) => {
                assert_eq!(row.id, id);
                assert_eq!(row.item_key, "mem-dequeue");
                assert_eq!(row.operation, "MemoryBindings");
            }
            DequeueOutcome::Empty => panic!("expected a claimed row"),
        }
        let empty = dequeue_next_pending(&conn, "MemoryBindings", "").expect("dequeue must succeed");
        assert!(matches!(empty, DequeueOutcome::Empty));
        let _ = std::fs::remove_file(&path);
    }

