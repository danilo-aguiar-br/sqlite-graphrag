//! Schema, enqueue and maintenance of the sidecar queue (GAP-SG-146).
//!
//! Table creation and idempotent migration, enqueue paths, orphan pruning,
//! scoped resets and the backlog counters.

use super::test_fixtures::{open_temp_queue, open_test_db};
use super::*;

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

#[test]
fn open_queue_db_alter_is_idempotent() {
    // A hand-built `/tmp/...` literal made this test Unix-only; `tempfile`
    // resolves the platform's temp directory and cleans up on drop.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("idempotent.sqlite");
    let path = path.to_string_lossy().into_owned();
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
fn prune_dead_entity_orphans_removes_only_entity_dead_rows() {
    let (queue, path) = open_temp_queue();
    // Entity dead row -> pruned (terminal artifact, no recovery path).
    queue
        .execute(
            "INSERT INTO queue (namespace, item_key, item_type, status, operation, error_class) \
                 VALUES ('global', 'entity:foo', 'entity', 'dead', 'ReEmbed', 'permanent')",
            [],
        )
        .unwrap();
    // Memory dead row -> untouched (wrong item_type).
    queue
        .execute(
            "INSERT INTO queue (namespace, item_key, item_type, status, operation, error_class) \
                 VALUES ('global', 'mem-dead', 'memory', 'dead', 'MemoryBindings', 'permanent')",
            [],
        )
        .unwrap();
    // Entity pending row -> untouched (not dead).
    queue
        .execute(
            "INSERT INTO queue (namespace, item_key, item_type, status, operation) \
                 VALUES ('global', 'entity:bar', 'entity', 'pending', 'ReEmbed')",
            [],
        )
        .unwrap();

    let pruned = prune_dead_entity_orphans(&queue, "ReEmbed", "global").unwrap();
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
            "INSERT INTO queue (namespace, item_key, item_type, status, operation, error_class) \
                 VALUES ('global', 'gone', 'memory', 'dead', 'MemoryBindings', 'permanent')",
            [],
        )
        .unwrap();
    // Live dead memory row (memory exists) -> kept.
    queue
        .execute(
            "INSERT INTO queue (namespace, item_key, item_type, status, operation, error_class) \
                 VALUES ('global', 'alive', 'memory', 'dead', 'MemoryBindings', 'permanent')",
            [],
        )
        .unwrap();
    // Entity dead row -> never touched (key is not a memory name).
    queue
        .execute(
            "INSERT INTO queue (namespace, item_key, item_type, status, operation) \
                 VALUES ('global', 'some-entity', 'entity', 'dead', 'EntityDescriptions')",
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

/// A prune asked for one namespace must never delete another namespace's dead
/// rows.
///
/// The queue SELECT used to be unscoped while the memory existence check was
/// scoped: a dead row belonging to `other` was checked against `global`'s
/// memories, found absent, and deleted as an orphan — silent data loss on a
/// shared sidecar. `dead-other` below is EXACTLY that row: it is an orphan
/// relative to `global` and a live row relative to `other`.
#[test]
fn prune_dead_orphans_never_crosses_into_another_namespace() {
    let main = open_test_db();
    main.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('other', 'dead-other', 'b')",
        [],
    )
    .unwrap();
    let (queue, path) = open_temp_queue();
    queue
        .execute(
            "INSERT INTO queue (namespace, item_key, item_type, status, operation, error_class) \
                 VALUES ('other', 'dead-other', 'memory', 'dead', 'MemoryBindings', 'permanent')",
            [],
        )
        .unwrap();

    let pruned = prune_dead_orphans(&queue, &main, "MemoryBindings", "global").unwrap();
    assert_eq!(pruned, 0, "a prune scoped to `global` must not see `other`");

    let survivors: i64 = queue
        .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get(0))
        .unwrap();
    assert_eq!(survivors, 1, "the `other` dead row must survive intact");
    let _ = std::fs::remove_file(&path);
}

/// Same invariant for the entity-keyed prune, which consults no main DB at all
/// and therefore deleted every namespace's entity dead rows unconditionally.
#[test]
fn prune_dead_entity_orphans_never_crosses_into_another_namespace() {
    let (queue, path) = open_temp_queue();
    queue
        .execute(
            "INSERT INTO queue (namespace, item_key, item_type, status, operation, error_class) \
                 VALUES ('other', 'entity:foo', 'entity', 'dead', 'ReEmbed', 'permanent')",
            [],
        )
        .unwrap();

    let pruned = prune_dead_entity_orphans(&queue, "ReEmbed", "global").unwrap();
    assert_eq!(pruned, 0, "a prune scoped to `global` must not see `other`");

    let survivors: i64 = queue
        .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get(0))
        .unwrap();
    assert_eq!(survivors, 1, "the `other` dead row must survive intact");
    let _ = std::fs::remove_file(&path);
}

fn busy_error() -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error {
            code: rusqlite::ErrorCode::DatabaseBusy,
            extended_code: 5,
        },
        None,
    )
}

/// A write-back that never lands must be REPORTED, not discarded.
///
/// The parallel drain and the re-embed batch cycle called `mark_done` as
/// `let _ = mark_done(...)`. A SQLITE_BUSY there lost the completion in
/// silence: the row stayed claimed, the stale-claim sweep handed it back, and
/// the provider was billed again for work already paid for.
#[test]
fn writeback_reports_a_completion_it_could_not_persist() {
    let attempts = std::cell::Cell::new(0usize);
    let landed = writeback("mark_done", 0, "some-item", || {
        attempts.set(attempts.get() + 1);
        Err::<(), _>(busy_error())
    });
    assert!(!landed, "a lost write-back must be reported as lost");
    assert!(
        attempts.get() > 1,
        "the write-back must be retried under SQLITE_BUSY, not attempted once; got {}",
        attempts.get()
    );
}

/// The success path stays a plain success, so the retry wrapper costs nothing
/// when the queue is uncontended.
#[test]
fn writeback_reports_a_completion_that_landed() {
    let attempts = std::cell::Cell::new(0usize);
    let landed = writeback("mark_done", 0, "some-item", || {
        attempts.set(attempts.get() + 1);
        Ok::<_, rusqlite::Error>(1usize)
    });
    assert!(landed);
    assert_eq!(attempts.get(), 1, "a landing write-back must not retry");
}

#[test]
fn queue_db_schema_creates_correctly() {
    // A hand-built `/tmp/...` literal made this test Unix-only; `tempfile`
    // resolves the platform's temp directory and cleans up on drop.
    let dir = tempfile::tempdir().expect("tempdir");
    let tmp_path = dir.path().join("schema.sqlite");
    let tmp_path = tmp_path.to_string_lossy().into_owned();
    let conn = open_queue_db(&tmp_path).expect("queue db must open");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
    let _ = std::fs::remove_file(&tmp_path);
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
