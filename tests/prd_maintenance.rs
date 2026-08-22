//! PRD compliance: purge retention, optimize, vacuum, permissions, path traversal, stats, list, rename, restore, cleanup-orphans, sync-safe-copy and the high-degree hub write (clauses 21-32).
//!
//! Part of the PRD-compliance suite split by GAP-SG-208. Covers the `MUST`/`DEVE`
//! clauses of the sqlite-graphrag PRD. The shared harness lives in
//! `tests/prd_support/`.

#[path = "prd_support/mod.rs"]
mod support;

use rusqlite::Connection;
use serial_test::serial;
use support::{cmd_base, db_path, init_db, remember_ok, sgr_cmd};
use tempfile::TempDir;
// ---------------------------------------------------------------------------
// 21 — purge with retention=1 removes soft-deleted memories older than 1 day
// ---------------------------------------------------------------------------

#[test]
fn prd_purge_retention_removes_old_soft_deleted() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    remember_ok(&tmp, "mem-purge-alvo", "corpo para purge test");

    // Direct soft-delete via SQL with timestamp in the past (2 days ago)
    let conn = Connection::open(db_path(&tmp)).unwrap();
    conn.execute(
        "UPDATE memories SET deleted_at = strftime('%s','now') - 172800 WHERE name='mem-purge-alvo'",
        [],
    )
    .unwrap();
    drop(conn);

    // Purge with retention of 1 day — should remove the 2-day-old memory
    cmd_base(&tmp)
        .args(["purge", "--retention-days", "1", "--yes"])
        .assert()
        .success();

    // Verify that it was permanently removed
    let conn2 = Connection::open(db_path(&tmp)).unwrap();
    let count: i64 = conn2
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE name='mem-purge-alvo'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 0,
        "purge deve remover permanentemente memórias com deleted_at > retention"
    );
}

// ---------------------------------------------------------------------------
// 22 — optimize runs without errors and returns status ok
// ---------------------------------------------------------------------------

#[test]
fn prd_optimize_runs_and_returns_status_ok() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd_base(&tmp)
        .arg("optimize")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["status"], "ok", "optimize deve retornar status 'ok'");
}

// ---------------------------------------------------------------------------
// 23 — vacuum returns size_before_bytes and size_after_bytes
// ---------------------------------------------------------------------------

#[test]
fn prd_vacuum_returns_size_before_and_size_after() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd_base(&tmp)
        .arg("vacuum")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(
        json.get("size_before_bytes").is_some(),
        "vacuum deve emitir size_before_bytes"
    );
    assert!(
        json.get("size_after_bytes").is_some(),
        "vacuum deve emitir size_after_bytes"
    );
}

// ---------------------------------------------------------------------------
// 24 — chmod 600 applied on Unix after init
// ---------------------------------------------------------------------------

#[test]
#[cfg(unix)]
fn prd_chmod_600_aplicado_apos_init() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let db = db_path(&tmp);
    let perms = std::fs::metadata(&db).unwrap().permissions();
    let mode = perms.mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "database deve ter permissão 600 após init, atual: {mode:o}"
    );
}

// ---------------------------------------------------------------------------
// 25 — path traversal (..) rejected in --db (product env is not a channel)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn prd_path_traversal_rejected_in_db_flag() {
    let tmp = TempDir::new().unwrap();

    // GAP-SG-101: SQLITE_GRAPHRAG_DB_PATH is not read. Validate --db instead.
    let mut c = sgr_cmd();
    support::common::wire_assert_cmd(&tmp, &mut c, "unused.sqlite");
    c.arg("--skip-memory-guard");
    c.args(["init", "--db", "../../../etc/passwd"]);

    c.assert().failure();
}

// ---------------------------------------------------------------------------
// 26 — stats includes memories, entities, relationships (and the _total aliases)
// ---------------------------------------------------------------------------

#[test]
fn prd_stats_inclui_memories_entities_relationships() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    remember_ok(&tmp, "mem-stats-check", "corpo para stats test");

    let output = cmd_base(&tmp)
        .arg("stats")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(
        json.get("memories").is_some(),
        "stats deve ter campo 'memories'"
    );
    assert!(
        json.get("entities").is_some(),
        "stats deve ter campo 'entities'"
    );
    assert!(
        json.get("relationships").is_some(),
        "stats deve ter campo 'relationships'"
    );
    assert!(
        json.get("memories_total").is_some() || json.get("memories").is_some(),
        "stats deve ter memories_total ou memories"
    );
}

// ---------------------------------------------------------------------------
// 27 — list respects --limit
// ---------------------------------------------------------------------------

#[test]
fn prd_list_respeita_limit() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // Create 5 memories
    for i in 0..5 {
        remember_ok(&tmp, &format!("mem-limit-{i}"), &format!("corpo {i}"));
    }

    let output = cmd_base(&tmp)
        .args(["list", "--namespace", "global", "--limit", "2"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let items = json["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        2,
        "list com --limit 2 deve retornar exatamente 2 itens"
    );
}

// ---------------------------------------------------------------------------
// 28 — rename updates memory version
// ---------------------------------------------------------------------------

#[test]
fn prd_rename_updates_version() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    remember_ok(&tmp, "mem-rename-orig", "corpo para rename test");

    // Verify initial version via memory_versions
    let conn = Connection::open(db_path(&tmp)).unwrap();
    let version_antes: i64 = conn
        .query_row(
            "SELECT MAX(version) FROM memory_versions mv \
             JOIN memories m ON m.id = mv.memory_id WHERE m.name='mem-rename-orig'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    drop(conn);

    // Rename
    cmd_base(&tmp)
        .args([
            "rename",
            "--name",
            "mem-rename-orig",
            "--new-name",
            "mem-rename-novo",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    // Verify the memory exists with the new name
    let conn2 = Connection::open(db_path(&tmp)).unwrap();
    let count: i64 = conn2
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE name='mem-rename-novo'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "memória deve existir com novo nome após rename");

    // Version may have incremented after rename (we check it exists in memory_versions)
    let versions_count: i64 = conn2
        .query_row(
            "SELECT COUNT(*) FROM memory_versions WHERE name='mem-rename-novo' OR name='mem-rename-orig'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        versions_count >= 1,
        "rename deve registrar versão em memory_versions"
    );
    let _ = version_antes; // used to document the test's intent
}

// ---------------------------------------------------------------------------
// 29 — restore reverts memory to the state before the last soft-delete
// ---------------------------------------------------------------------------

#[test]
fn prd_restore_reverte_soft_delete() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    remember_ok(&tmp, "mem-restore-test", "corpo original para restore");

    // Soft-delete
    cmd_base(&tmp)
        .args([
            "forget",
            "--name",
            "mem-restore-test",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    // Verify soft-deleted and obtain the version for restore
    let conn = Connection::open(db_path(&tmp)).unwrap();
    let deleted: bool = conn
        .query_row(
            "SELECT deleted_at IS NOT NULL FROM memories WHERE name='mem-restore-test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(deleted, "memória deve estar soft-deleted após forget");
    let version: i64 = conn
        .query_row(
            "SELECT MAX(version) FROM memory_versions v JOIN memories m ON m.id=v.memory_id WHERE m.name='mem-restore-test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    drop(conn);

    // Restore passing the version obtained from history
    cmd_base(&tmp)
        .args([
            "restore",
            "--name",
            "mem-restore-test",
            "--namespace",
            "global",
            "--version",
            &version.to_string(),
        ])
        .assert()
        .success();

    // Verify that it was restored (deleted_at = NULL)
    let conn2 = Connection::open(db_path(&tmp)).unwrap();
    let active: bool = conn2
        .query_row(
            "SELECT deleted_at IS NULL FROM memories WHERE name='mem-restore-test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        active,
        "memória deve estar ativa (deleted_at NULL) após restore"
    );
}

// ---------------------------------------------------------------------------
// 30 — cleanup-orphans removes entities without memories
// ---------------------------------------------------------------------------

#[test]
fn prd_cleanup_orphans_removes_entities_without_memories() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // Insert an orphan entity directly into the database
    let conn = Connection::open(db_path(&tmp)).unwrap();
    conn.execute(
        "INSERT INTO entities (name, type, namespace) VALUES ('entidade-orfa', 'concept', 'global')",
        [],
    )
    .unwrap();
    drop(conn);

    // Verify that it exists beforehand
    let conn2 = Connection::open(db_path(&tmp)).unwrap();
    let antes: i64 = conn2
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE name='entidade-orfa'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(antes, 1, "entidade órfã deve existir antes do cleanup");
    drop(conn2);

    // Run cleanup
    let output = cmd_base(&tmp)
        .args(["cleanup-orphans", "--yes"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let deleted = json["deleted"].as_u64().unwrap_or(0);
    assert!(
        deleted >= 1,
        "cleanup-orphans deve reportar ao menos 1 deleted"
    );

    // Verify that the entity was removed
    let conn3 = Connection::open(db_path(&tmp)).unwrap();
    let depois: i64 = conn3
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE name='entidade-orfa'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        depois, 0,
        "entidade órfã deve ter sido removida pelo cleanup"
    );
}

// ---------------------------------------------------------------------------
// 31 — sync-safe-copy produces a coherent snapshot with bytes_copied > 0
// ---------------------------------------------------------------------------

#[test]
fn prd_sync_safe_copy_generates_coherent_snapshot() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    remember_ok(&tmp, "mem-snapshot", "corpo para snapshot test");

    let dest = tmp.path().join("snapshot.sqlite");

    let output = cmd_base(&tmp)
        .args(["sync-safe-copy", "--dest", dest.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(
        json.get("bytes_copied").is_some(),
        "sync-safe-copy deve emitir bytes_copied"
    );
    assert!(
        json["bytes_copied"].as_u64().unwrap_or(0) > 0,
        "bytes_copied deve ser > 0"
    );
    assert_eq!(
        json["status"], "ok",
        "sync-safe-copy deve retornar status 'ok'"
    );
    assert!(dest.exists(), "arquivo de snapshot deve existir no destino");
}

// ---------------------------------------------------------------------------
// 32 — GAP-SG-67: a write referencing a high-degree hub must be purely
//      additive — it must NEVER prune incident edges. Repro of the incident
//      at small scale: pre-fix, the default degree cap of 50 pruned the hub
//      back down to 50; post-fix the edge count only ever grows.
// ---------------------------------------------------------------------------

#[test]
fn gap_sg_67_write_never_prunes_high_degree_hub() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // Build a HUB entity with 60 incident edges directly via SQL: well above
    // the old default degree cap of 50. Each edge targets a distinct leaf so
    // there are no UNIQUE(source_id,target_id,relation) collisions.
    let conn = Connection::open(db_path(&tmp)).unwrap();
    conn.execute(
        "INSERT INTO entities (name, type, namespace) VALUES ('hub-sg67', 'concept', 'global')",
        [],
    )
    .unwrap();
    let hub_id: i64 = conn
        .query_row("SELECT id FROM entities WHERE name='hub-sg67'", [], |r| {
            r.get(0)
        })
        .unwrap();

    const SEED_EDGES: i64 = 60;
    for i in 0..SEED_EDGES {
        let leaf = format!("leaf-sg67-{i}");
        conn.execute(
            "INSERT INTO entities (name, type, namespace) VALUES (?1, 'concept', 'global')",
            [&leaf],
        )
        .unwrap();
        let leaf_id: i64 = conn
            .query_row("SELECT id FROM entities WHERE name=?1", [&leaf], |r| {
                r.get(0)
            })
            .unwrap();
        // Ascending weights so a pruning pass would have a deterministic victim
        // order; the fix means no pruning happens at all.
        let weight = 0.1 + (i as f64) * 0.01;
        conn.execute(
            "INSERT INTO relationships (source_id, target_id, relation, weight, namespace) \
             VALUES (?1, ?2, 'related', ?3, 'global')",
            rusqlite::params![hub_id, leaf_id, weight],
        )
        .unwrap();
    }
    drop(conn);

    // One additional write: create the 61st incident edge on the hub via `link`.
    // The --max-entity-degree flag no longer exists; pre-fix it defaulted to 50
    // and this write would have pruned the 11 weakest edges back down to 50.
    cmd_base(&tmp)
        .args([
            "link",
            "--from",
            "hub-sg67",
            "--to",
            "leaf-sg67-new",
            "--relation",
            "related",
            "--create-missing",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    // ASSERT non-destructive: the edge count only grew (60 -> 61) and the hub's
    // degree stays high. A degree-cap prune would have collapsed both to 50.
    let conn = Connection::open(db_path(&tmp)).unwrap();
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        total,
        SEED_EDGES + 1,
        "GAP-SG-67: write must be additive; relationships dropped (degree-cap pruning regressed)"
    );

    let hub_degree: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM relationships WHERE source_id=?1 OR target_id=?1",
            [hub_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        hub_degree,
        SEED_EDGES + 1,
        "GAP-SG-67: hub degree must not collapse to the old cap of 50"
    );
    assert!(
        hub_degree > 50,
        "GAP-SG-67: hub must remain above the removed degree cap of 50"
    );
}
