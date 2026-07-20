//! Auto-extracted tests (Wave C1).

    use super::*;

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

    #[test]
    fn classify_provider_error_and_not_found_are_hard() {
        assert_eq!(
            classify_enrich_outcome(&AppError::ProviderError {
                code: "400".into(),
                message: "context length exceeded".into(),
            }),
            crate::retry::AttemptOutcome::HardFailure
        );
        assert_eq!(
            classify_enrich_outcome(&AppError::NotFound("memory 'gone' not found".into())),
            crate::retry::AttemptOutcome::HardFailure
        );
    }

    #[test]
    fn open_queue_db_migrates_operation_column() {
        let (conn, path) = open_temp_queue();
        drop(conn);
        let conn = open_queue_db(&path).expect("second open is idempotent");
        let cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(queue)").unwrap();
            stmt.query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert!(cols.iter().any(|c| c == "operation"));
        assert!(cols.iter().any(|c| c == "memory_id"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn enqueue_candidate_tags_operation_and_memory_id() {
        let main = open_test_db();
        main.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'mem-x', 'body')",
            [],
        )
        .unwrap();
        let mem_id: i64 = main
            .query_row("SELECT id FROM memories WHERE name='mem-x'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let (queue, path) = open_temp_queue();
        enqueue_candidate(&queue, &main, "global", "mem-x", "memory", "MemoryBindings");
        let (op, mid): (String, i64) = queue
            .query_row(
                "SELECT operation, memory_id FROM queue WHERE item_key='mem-x'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(op, "MemoryBindings");
        assert_eq!(mid, mem_id);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn requeue_dead_resurrects_dead_rows() {
        let (conn, path) = open_temp_queue();
        conn.execute(
            "INSERT INTO queue (item_key, item_type, status, operation, attempt, error, error_class, next_retry_at) \
             VALUES ('mem-dead', 'memory', 'dead', 'MemoryBindings', 8, 'boom', 'permanent', datetime('now'))",
            [],
        )
        .unwrap();
        let n = conn
            .execute(
                "UPDATE queue SET status='pending', attempt=0, next_retry_at=NULL, \
                 error=NULL, error_class=NULL \
                 WHERE status='dead' AND (operation = ?1 OR operation IS NULL)",
                rusqlite::params!["MemoryBindings"],
            )
            .unwrap();
        assert_eq!(n, 1);
        let (status, attempt, nra): (String, i64, Option<String>) = conn
            .query_row(
                "SELECT status, attempt, next_retry_at FROM queue WHERE item_key='mem-dead'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "pending");
        assert_eq!(attempt, 0);
        assert!(nra.is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn skipped_item_keys_excludes_only_skipped_for_operation() {
        // GAP-SG-69: the body-enrich scan must drop memories already vetoed
        // `status='skipped'` so `--until-empty` converges instead of re-scanning a
        // non-expandable short body forever (the detached worker reported a
        // stuck backlog for 30+ min).
        let (conn, path) = open_temp_queue();
        conn.execute(
            "INSERT INTO queue (item_key, item_type, status, operation) VALUES ('mem-vetoed', 'memory', 'skipped', 'BodyEnrich')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO queue (item_key, item_type, status, operation) VALUES ('mem-pending', 'memory', 'pending', 'BodyEnrich')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO queue (item_key, item_type, status, operation) VALUES ('mem-other-op', 'memory', 'skipped', 'MemoryBindings')",
            [],
        )
        .unwrap();
        let keys = skipped_item_keys(&conn, "BodyEnrich").unwrap();
        assert!(
            keys.contains("mem-vetoed"),
            "vetoed BodyEnrich item must be excluded from scan"
        );
        assert!(
            !keys.contains("mem-pending"),
            "pending item is still actionable"
        );
        assert!(
            !keys.contains("mem-other-op"),
            "skipped item from another operation must not leak"
        );
        assert_eq!(keys.len(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn cascade_cleanup_delete_targets_memory_id_and_name() {
        let (conn, path) = open_temp_queue();
        conn.execute(
            "INSERT INTO queue (item_key, item_type, status, memory_id) VALUES ('by-id', 'memory', 'done', 42)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO queue (item_key, item_type, status) VALUES ('by-name', 'memory', 'pending')",
            [],
        )
        .unwrap();
        let removed = conn
            .execute(
                "DELETE FROM queue WHERE memory_id = ?1 OR item_key = ?2",
                rusqlite::params![42_i64, "by-name"],
            )
            .unwrap();
        assert_eq!(removed, 2);
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn item_type_for_maps_entity_and_memory() {
        assert_eq!(
            item_type_for(&EnrichOperation::EntityDescriptions),
            "entity"
        );
        assert_eq!(item_type_for(&EnrichOperation::MemoryBindings), "memory");
        assert_eq!(item_type_for(&EnrichOperation::AugmentBindings), "memory");
        assert_eq!(item_type_for(&EnrichOperation::BodyExtract), "memory");
        assert_eq!(
            item_type_for(&EnrichOperation::EntityConnect),
            "entity_pair"
        );
        assert_eq!(
            item_type_for(&EnrichOperation::CrossDomainBridges),
            "entity_pair"
        );
    }

    // v1.1.1 (P2): prefixed re-embed keys override the operation default so
    // prune_dead_orphans never reaps entity/chunk rows as orphaned memories.
    #[test]
    fn item_type_for_key_honours_reembed_prefixes() {
        assert_eq!(item_type_for_key("plain-memory-name", "memory"), "memory");
        assert_eq!(
            item_type_for_key("entity:tokio-runtime", "memory"),
            "entity"
        );
        assert_eq!(item_type_for_key("chunk:42", "memory"), "chunk");
        assert_eq!(item_type_for_key("some-entity", "entity"), "entity");
        // v1.1.06: pair keys must not be treated as memory names.
        assert_eq!(item_type_for_key("pair:1:2", "memory"), "entity_pair");
    }

    #[test]
    fn prune_dead_orphans_removes_only_orphan_memory_rows() {
        let main = open_test_db();
        // One live memory whose dead row must be KEPT (it still exists).
        main.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'alive', 'b')",
            [],
        )
        .unwrap();
        let (queue, path) = open_temp_queue();
        // Orphan dead memory row (no matching memory) -> pruned.
        queue
            .execute(
                "INSERT INTO queue (item_key, item_type, status, operation, error_class) \
                 VALUES ('gone', 'memory', 'dead', 'MemoryBindings', 'permanent')",
                [],
            )
            .unwrap();
        // Live dead memory row (memory exists) -> kept.
        queue
            .execute(
                "INSERT INTO queue (item_key, item_type, status, operation, error_class) \
                 VALUES ('alive', 'memory', 'dead', 'MemoryBindings', 'permanent')",
                [],
            )
            .unwrap();
        // Entity dead row -> never touched (key is not a memory name).
        queue
            .execute(
                "INSERT INTO queue (item_key, item_type, status, operation) \
                 VALUES ('some-entity', 'entity', 'dead', 'EntityDescriptions')",
                [],
            )
            .unwrap();

        let pruned = prune_dead_orphans(&queue, &main, "MemoryBindings", "global").unwrap();
        assert_eq!(pruned, 1, "only the orphan memory row is pruned");

        let remaining: Vec<String> = {
            let mut stmt = queue
                .prepare("SELECT item_key FROM queue ORDER BY item_key")
                .unwrap();
            stmt.query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(remaining, vec!["alive", "some-entity"]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn prune_dead_entity_orphans_removes_only_entity_dead_rows() {
        let (queue, path) = open_temp_queue();
        // Entity dead row -> pruned (terminal artifact, no recovery path).
        queue
            .execute(
                "INSERT INTO queue (item_key, item_type, status, operation, error_class) \
                 VALUES ('entity:foo', 'entity', 'dead', 'ReEmbed', 'permanent')",
                [],
            )
            .unwrap();
        // Memory dead row -> untouched (wrong item_type).
        queue
            .execute(
                "INSERT INTO queue (item_key, item_type, status, operation, error_class) \
                 VALUES ('mem-dead', 'memory', 'dead', 'MemoryBindings', 'permanent')",
                [],
            )
            .unwrap();
        // Entity pending row -> untouched (not dead).
        queue
            .execute(
                "INSERT INTO queue (item_key, item_type, status, operation) \
                 VALUES ('entity:bar', 'entity', 'pending', 'ReEmbed')",
                [],
            )
            .unwrap();

        let pruned = prune_dead_entity_orphans(&queue, "ReEmbed").unwrap();
        assert_eq!(pruned, 1, "only the entity dead row is pruned");

        let remaining: Vec<String> = {
            let mut stmt = queue
                .prepare("SELECT item_key FROM queue ORDER BY item_key")
                .unwrap();
            stmt.query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(remaining, vec!["entity:bar", "mem-dead"]);
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

    // v1.1.2 (Bug 4, D5): enqueuing N candidates inside one transaction makes
    // the batch atomic — a second connection sees NONE of them before commit and
    // ALL of them after. (Within the same connection SQLite always shows
    // read-your-own-writes, so atomicity is observed cross-connection.)
    #[test]
    fn enqueue_batch_is_atomic() {
        let main = open_test_db();
        main.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'm1', 'b'), ('global', 'm2', 'b'), ('global', 'm3', 'b')",
            [],
        )
        .unwrap();
        let (mut queue, path) = open_temp_queue();
        // Independent reader on the same sidecar file: sees only committed rows.
        let reader = Connection::open(&path).unwrap();

        let tx = queue.transaction().unwrap();
        let tx_conn: &Connection = &tx;
        for key in ["m1", "m2", "m3"] {
            enqueue_candidate(tx_conn, &main, "global", key, "memory", "MemoryBindings");
        }
        // A second connection sees nothing until commit.
        let before: i64 = reader
            .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get::<_, i64>(0))
            .unwrap();
        assert_eq!(before, 0, "rows must not be visible before commit");
        tx.commit().unwrap();

        let after: i64 = reader
            .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get::<_, i64>(0))
            .unwrap();
        assert_eq!(after, 3, "all three rows are visible after commit");
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

    /// GAP-CLI-QISO-01/05: MemoryBindings drain must not claim EntityDescriptions.
    #[test]
    fn dequeue_filters_by_operation_isolation() {
        let (conn, path) = open_temp_queue();
        let _ed_id = insert_pending_op(&conn, "aeroporto-guarulhos", "entity", "EntityDescriptions");
        let mb_id = insert_pending_op(&conn, "yt-memory", "memory", "MemoryBindings");

        match dequeue_next_pending(&conn, "MemoryBindings", "").unwrap() {
            DequeueOutcome::Claimed(row) => {
                assert_eq!(row.id, mb_id);
                assert_eq!(row.item_key, "yt-memory");
                assert_eq!(row.operation, "MemoryBindings");
            }
            DequeueOutcome::Empty => panic!("expected MB claim"),
        }
        assert!(matches!(
            dequeue_next_pending(&conn, "MemoryBindings", "").unwrap(),
            DequeueOutcome::Empty
        ));
        match dequeue_next_pending(&conn, "EntityDescriptions", "").unwrap() {
            DequeueOutcome::Claimed(row) => {
                assert_eq!(row.item_key, "aeroporto-guarulhos");
                assert_eq!(row.operation, "EntityDescriptions");
            }
            DequeueOutcome::Empty => panic!("expected ED claim"),
        }
        let _ = std::fs::remove_file(&path);
    }

    /// GAP-CLI-QISO-01/05: entity-connect pair keys are invisible to MB claim.
    #[test]
    fn dequeue_does_not_claim_pair_keys_under_memory_bindings() {
        let (conn, path) = open_temp_queue();
        let _pair = insert_pending_op(&conn, "pair:21560:159670", "entity_pair", "EntityConnect");
        assert!(matches!(
            dequeue_next_pending(&conn, "MemoryBindings", "").unwrap(),
            DequeueOutcome::Empty
        ));
        match dequeue_next_pending(&conn, "EntityConnect", "").unwrap() {
            DequeueOutcome::Claimed(row) => {
                assert_eq!(row.item_key, "pair:21560:159670");
            }
            DequeueOutcome::Empty => panic!("expected EC claim"),
        }
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
            dequeue_next_pending(&conn, "MemoryBindings", "").unwrap(),
            DequeueOutcome::Empty
        ));
        let _ = std::fs::remove_file(&path);
    }
