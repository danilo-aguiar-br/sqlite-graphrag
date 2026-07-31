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

/// Regression: re-embed `--target entities|all` enqueues `entity:{name}`
/// keys. Lookup must strip the prefix or every candidate is rejected.
#[test]
fn enqueue_candidate_accepts_entity_prefixed_reembed_key() {
    let main = open_test_db();
    main.execute_batch(
            "CREATE TABLE entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace TEXT NOT NULL,
                name TEXT NOT NULL,
                type TEXT NOT NULL DEFAULT 'concept',
                description TEXT,
                aliases TEXT NOT NULL DEFAULT '[]',
                degree INTEGER NOT NULL DEFAULT 0,
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
                UNIQUE(namespace, name)
            );
            INSERT INTO entities (namespace, name, type) VALUES ('global', 'ownership', 'concept');",
        )
        .expect("entities schema");
    let (queue, path) = open_temp_queue();
    enqueue_candidate(
        &queue,
        &main,
        "global",
        "entity:ownership",
        "entity",
        "ReEmbed",
    );
    let (key, itype, status, op): (String, String, String, String) = queue
            .query_row(
                "SELECT item_key, item_type, status, operation FROM queue WHERE item_key='entity:ownership'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("prefixed entity key must be enqueued");
    assert_eq!(key, "entity:ownership");
    assert_eq!(itype, "entity");
    assert_eq!(status, "pending");
    assert_eq!(op, "ReEmbed");
    // bare name still works (entity-descriptions path)
    enqueue_candidate(
        &queue,
        &main,
        "global",
        "ownership",
        "entity",
        "EntityDescriptions",
    );
    let n: i64 = queue
        .query_row(
            "SELECT COUNT(*) FROM queue WHERE item_key='ownership'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
    // unknown entity still rejected
    enqueue_candidate(
        &queue,
        &main,
        "global",
        "entity:does-not-exist",
        "entity",
        "ReEmbed",
    );
    let n_bad: i64 = queue
        .query_row(
            "SELECT COUNT(*) FROM queue WHERE item_key='entity:does-not-exist'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n_bad, 0);
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
        dequeue_next_pending(&conn, "MemoryBindings", "", "").unwrap(),
        DequeueOutcome::Empty
    ));
    let _ = std::fs::remove_file(&path);
}

/// CAPA-A: until-empty eligibility must ignore alien operations' pending.
#[test]
fn count_eligible_pending_isolates_operation_and_namespace() {
    let (conn, path) = open_temp_queue();
    conn.execute(
        "INSERT INTO queue (namespace, item_key, item_type, status, operation) \
             VALUES ('global', 'e1', 'entity', 'pending', 'EntityDescriptions')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO queue (namespace, item_key, item_type, status, operation) \
             VALUES ('global', 'entity:z', 'entity', 'pending', 'ReEmbed')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO queue (namespace, item_key, item_type, status, operation) \
             VALUES ('other', 'e2', 'entity', 'pending', 'EntityDescriptions')",
        [],
    )
    .unwrap();
    assert_eq!(
        count_eligible_pending(&conn, "EntityDescriptions", "global", ""),
        1
    );
    assert_eq!(count_eligible_pending(&conn, "ReEmbed", "global", ""), 1);
    assert_eq!(
        count_eligible_pending(&conn, "EntityDescriptions", "other", ""),
        1
    );
    assert_eq!(
        count_eligible_pending(&conn, "MemoryBindings", "global", ""),
        0
    );
    let _ = std::fs::remove_file(&path);
}

/// CAPA-B: force-redescribe reopens skipped/done, never dead.
#[test]
fn reopen_force_redescribe_candidates_skips_dead() {
    let (conn, path) = open_temp_queue();
    for (key, status) in [
        ("a", "skipped"),
        ("b", "done"),
        ("c", "dead"),
        ("d", "pending"),
    ] {
        conn.execute(
            "INSERT INTO queue (namespace, item_key, item_type, status, operation, attempt) \
                 VALUES ('global', ?1, 'entity', ?2, 'EntityDescriptions', 3)",
            rusqlite::params![key, status],
        )
        .unwrap();
    }
    let keys = vec![
        "a".into(),
        "b".into(),
        "c".into(),
        "d".into(),
        "missing".into(),
    ];
    let n = reopen_force_redescribe_candidates(&conn, "global", &keys);
    assert_eq!(n, 2, "only skipped+done reopen");
    let a_status: String = conn
        .query_row(
            "SELECT status FROM queue WHERE item_key='a' AND namespace='global'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let a_attempt: i64 = conn
        .query_row(
            "SELECT attempt FROM queue WHERE item_key='a' AND namespace='global'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(a_status, "pending");
    assert_eq!(a_attempt, 0);
    let c_status: String = conn
        .query_row(
            "SELECT status FROM queue WHERE item_key='c' AND namespace='global'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(c_status, "dead");
    let _ = std::fs::remove_file(&path);
}

/// CAPA-E: resume/retry scoped to op+ns.
#[test]
fn reset_processing_and_failed_scoped_to_op_ns() {
    let (conn, path) = open_temp_queue();
    conn.execute(
        "INSERT INTO queue (namespace, item_key, item_type, status, operation) VALUES
             ('global', 'ed1', 'entity', 'processing', 'EntityDescriptions'),
             ('global', 're1', 'entity', 'processing', 'ReEmbed'),
             ('other', 'ed2', 'entity', 'processing', 'EntityDescriptions'),
             ('global', 'edf', 'entity', 'failed', 'EntityDescriptions'),
             ('global', 'ref', 'entity', 'failed', 'ReEmbed')",
        [],
    )
    .unwrap();
    assert_eq!(
        reset_processing_for_op(&conn, "EntityDescriptions", "global").unwrap(),
        1
    );
    let re_status: String = conn
        .query_row("SELECT status FROM queue WHERE item_key='re1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(re_status, "processing");
    assert_eq!(
        reset_failed_for_op(&conn, "EntityDescriptions", "global").unwrap(),
        1
    );
    let ref_status: String = conn
        .query_row("SELECT status FROM queue WHERE item_key='ref'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(ref_status, "failed");
    let _ = std::fs::remove_file(&path);
}

/// CAPA-C: has_live helpers use BLOB length = dim*4.
#[test]
fn entity_has_live_embedding_checks_blob_length() {
    let main = open_test_db();
    main.execute_batch(
        "CREATE TABLE entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace TEXT NOT NULL DEFAULT 'global',
                name TEXT NOT NULL,
                type TEXT NOT NULL DEFAULT 'concept',
                description TEXT NOT NULL DEFAULT '',
                UNIQUE(namespace, name)
            );
            CREATE TABLE entity_embeddings (
                entity_id INTEGER PRIMARY KEY,
                namespace TEXT NOT NULL DEFAULT 'global',
                embedding BLOB NOT NULL,
                source TEXT,
                model TEXT,
                dim INTEGER
            );",
    )
    .unwrap();
    main.execute("INSERT INTO entities (name) VALUES ('e')", [])
        .unwrap();
    let eid = main.last_insert_rowid();
    let dim = crate::constants::embedding_dim();
    // Wrong length (half) must not count as live.
    main.execute(
        "INSERT INTO entity_embeddings (entity_id, embedding, dim) VALUES (?1, ?2, ?3)",
        rusqlite::params![eid, vec![0u8; dim * 2], dim as i64],
    )
    .unwrap();
    assert!(!entity_has_live_embedding(&main, eid, dim));
    main.execute(
        "UPDATE entity_embeddings SET embedding = ?1 WHERE entity_id = ?2",
        rusqlite::params![vec![0u8; dim * 4], eid],
    )
    .unwrap();
    assert!(entity_has_live_embedding(&main, eid, dim));
}
