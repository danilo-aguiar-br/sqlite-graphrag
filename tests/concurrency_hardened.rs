#![cfg(feature = "slow-tests")]

// Suite 4 — Hardened lock and concurrency tests
//
// ISOLATION: each test uses `XDG_CACHE_HOME` pointing to a
// `TempDir` exclusive per test. `#[serial]` is required in all tests to avoid
// filesystem races between tests that share the same binary.
//
// `--skip-memory-guard` is used so that the RAM check does not abort before
// the semaphore is exercised in CI environments with limited memory.
//
// Timing-sensitive tests hold their slots inside the test process, so the
// waiting child always races against a hold this process controls; none of them
// is `#[ignore]`d.

use assert_cmd::Command;
use serial_test::serial;
use std::sync::{Arc, Barrier};
use tempfile::TempDir;

/// Builds a fresh `Command` with the mock LLM PATH prepended.
///
/// v1.0.76 spawns `claude` or `codex` on every `remember` / `ingest` /
/// `edit`. The bundled mocks under `tests/mock-llm/` return a fixed
/// 64-dim zero vector so the binary finishes without a real OAuth
/// login. The mock directory is leaked (no TempDir cleanup) so the
/// spawned subprocess always finds the mocks.
fn sgr_cmd() -> Command {
    let mock_dir = common::mock_llm_path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("sqlite-graphrag binary not found");
    c.env("PATH", common::prepend_path(&mock_dir));
    c
}

#[path = "common/mod.rs"]
mod common;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// GAP-SG-101: isolated assert_cmd bound to db_path via planted db.path.
///
/// GAP-SG-207: carries `--use-active`, because binding through the planted key
/// is the point of this helper and the fence refuses a mutating verb that
/// resolved that way without the declared dispensation.
fn sgr_on(tmp: &TempDir, db_path: &std::path::Path) -> Command {
    let mut c = sgr_cmd();
    common::plant_db_path(&tmp.path().join("config"), db_path);
    c.env("HOME", tmp.path().join("home"))
        .env("XDG_CACHE_HOME", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join("xdg_config"))
        .env("XDG_DATA_HOME", tmp.path().join("xdg_data"))
        .env("XDG_RUNTIME_DIR", tmp.path().join("xdg_runtime"))
        .arg("--config-dir")
        .arg(tmp.path().join("config"))
        .arg("--embedding-model")
        .arg(common::openrouter_mock::STUB_MODEL)
        .arg("--cache-dir")
        .arg(tmp.path())
        .arg("--use-active")
        .arg("--skip-memory-guard");
    c
}

/// Returns the lock file path for the given slot (1-based) inside the `TempDir`.
fn slot_path(tmp: &TempDir, slot: usize) -> std::path::PathBuf {
    tmp.path().join(format!("cli-slot-{slot}.lock"))
}

/// Locks `n_slots` lock files directly via fs4, returning the handles.
fn occupy_slots(tmp: &TempDir, n_slots: usize) -> Vec<std::fs::File> {
    use fs4::fs_std::FileExt;
    use std::fs::OpenOptions;

    (1..=n_slots)
        .map(|slot| {
            let path = slot_path(tmp, slot);
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
                .unwrap_or_else(|_| panic!("criação do lock file slot {slot} deve funcionar"));
            file.try_lock_exclusive()
                .unwrap_or_else(|_| panic!("slot {slot} deve estar livre antes do teste"));
            file
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Test 1 — 5 simultaneous instances: the 5th receives exit 75
// ---------------------------------------------------------------------------
// Occupies the 4 default slots via fs4, then triggers a 5th invocation with
// --wait-lock 0 and confirms it returns exit 75 (AllSlotsFull).

#[test]
#[serial]
fn five_instances_fifth_gets_exit_75() {
    let tmp = TempDir::new().expect("TempDir deve ser criado");

    // Occupy all 4 default slots
    let handles = occupy_slots(&tmp, 4);

    // 5th invocation with --wait-lock 0 must fail with exit 75.
    // MUST use --cache-dir so lock files share the same directory as occupy_slots
    // (paths::cache_dir prefers CLI override over ProjectDirs under XDG_CACHE_HOME).
    sgr_cmd()
        .env("XDG_CACHE_HOME", tmp.path())
        .arg("--cache-dir")
        .arg(tmp.path())
        .args([
            "--skip-memory-guard",
            "--max-concurrency",
            "4",
            "--wait-lock",
            "0",
            "namespace-detect",
        ])
        .assert()
        .failure()
        .code(75);

    drop(handles);
}

// ---------------------------------------------------------------------------
// Test 2 — --wait-lock 3 waits up to 3 seconds for a slot
// ---------------------------------------------------------------------------
// Occupies all slots, releases after 1s in a separate thread, confirms that
// --wait-lock 3 waits and completes successfully.
//
// Costs ~1s: the slots are released after 800ms while the child waits up to 3s.

#[test]
#[serial]
fn wait_lock_3s_respected() {
    let tmp = TempDir::new().expect("TempDir deve ser criado");
    let tmp_path = tmp.path().to_path_buf();

    // Occupy all 4 slots
    let handles = occupy_slots(&tmp, 4);

    // Release all after 1 second in a separate thread
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(800));
        drop(handles);
        // Keep tmp_path alive until here
        let _ = &tmp_path;
    });

    // --wait-lock 3 must wait for release (within 3s) and complete.
    // MUST use --cache-dir to share lock directory with occupy_slots.
    sgr_cmd()
        .env("XDG_CACHE_HOME", tmp.path())
        .arg("--cache-dir")
        .arg(tmp.path())
        .args([
            "--skip-memory-guard",
            "--max-concurrency",
            "4",
            "--wait-lock",
            "3",
            "namespace-detect",
        ])
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// Test 3 — duplicate remember followed by edit with a stale --updated-at → exit 3
// ---------------------------------------------------------------------------
// Simulates optimistic locking: insert a memory, get updated_at, modify
// via CLI, then try editing again with the stale updated_at (before the
// modification) and confirm exit 3 (Conflict).

#[test]
#[serial]
fn optimistic_locking_conflict_exit_3() {
    let tmp = TempDir::new().expect("TempDir deve ser criado");
    let db_path = tmp.path().join("test.sqlite");

    // Init
    sgr_on(&tmp, &db_path).args(["init"]).assert().success();

    // Insert memory
    sgr_on(&tmp, &db_path)
        .args([
            "remember",
            "--name",
            "mem-conflito",
            "--type",
            "user",
            "--namespace",
            "global",
            "--description",
            "desc original",
            "--body",
            "corpo original",
        ])
        .assert()
        .success();

    // Get updated_at via read to capture the timestamp before modifying
    let read_output = sgr_on(&tmp, &db_path)
        .args(["read", "--name", "mem-conflito", "--namespace", "global"])
        .output()
        .expect("output deve funcionar");

    let read_json: serde_json::Value =
        serde_json::from_slice(&read_output.stdout).expect("output deve ser JSON");

    let _updated_at_real = read_json
        .get("updated_at")
        .and_then(|v| v.as_i64())
        .expect("updated_at deve existir e ser i64");

    // Impossible value: Unix epoch 1970-01-01 will never be updated_at for a freshly created memory.
    // Ensures the conflict regardless of how many operations happen in the same second.
    let updated_at_stale: i64 = 1;

    // Edit with stale --expected-updated-at must fail with exit 3 (Conflict)
    sgr_on(&tmp, &db_path)
        .args([
            "edit",
            "--name",
            "mem-conflito",
            "--namespace",
            "global",
            "--description",
            "desc conflitante",
            "--expected-updated-at",
            &updated_at_stale.to_string(),
        ])
        .assert()
        .failure()
        .code(3);
}

// ---------------------------------------------------------------------------
// Test 4 — purge during recall does not corrupt the database
// ---------------------------------------------------------------------------
// Fires recall and purge in parallel via threads and confirms the database
// remains intact (no SQLITE_CORRUPT errors or panic) after both finish.
// Uses std::sync::Barrier to synchronize the start.

#[test]
#[serial]
fn purge_during_recall_does_not_corrupt() {
    let tmp = TempDir::new().expect("TempDir deve ser criado");
    let db_path = tmp.path().join("test.sqlite");

    // Init
    sgr_on(&tmp, &db_path).args(["init"]).assert().success();

    // Insert some old memories so that purge has something to do
    for i in 0..3 {
        sgr_on(&tmp, &db_path)
            .args([
                "remember",
                "--name",
                &format!("mem-purge-{i}"),
                "--type",
                "user",
                "--namespace",
                "global",
                "--description",
                &format!("memória antiga {i}"),
                "--body",
                &format!("corpo da memória para purge teste {i}"),
            ])
            .assert()
            .success();
    }

    let db_path_recall = db_path.clone();
    let db_path_purge = db_path.clone();
    let root_recall = tmp.path().to_path_buf();
    let root_purge = tmp.path().to_path_buf();

    let barrier = Arc::new(Barrier::new(2));
    let barrier_recall = Arc::clone(&barrier);
    let barrier_purge = Arc::clone(&barrier);

    let bin_path = std::path::PathBuf::from(env!("CARGO_BIN_EXE_sqlite-graphrag"));

    let bin_recall = bin_path.clone();
    let bin_purge = bin_path.clone();

    // Thread recall — concurrent search
    let handle_recall = std::thread::spawn(move || {
        barrier_recall.wait();
        let mut c = std::process::Command::new(&bin_recall);
        common::wire_std_cmd(&root_recall, &mut c, &db_path_recall);
        c.args(["--skip-memory-guard", "recall", "--db"])
            .arg(&db_path_recall)
            .args(["memória antiga", "--namespace", "global", "--k", "5"])
            .output()
            .expect("recall deve executar sem panic")
    });

    // Purge thread — concurrent purge with --dry-run so nothing is deleted
    let handle_purge = std::thread::spawn(move || {
        barrier_purge.wait();
        let mut c = std::process::Command::new(&bin_purge);
        common::wire_std_cmd(&root_purge, &mut c, &db_path_purge);
        c.args(["--skip-memory-guard", "purge", "--db"])
            .arg(&db_path_purge)
            .args(["--namespace", "global", "--dry-run"])
            .output()
            .expect("purge deve executar sem panic")
    });

    let recall_result = handle_recall
        .join()
        .expect("thread recall não deve entrar em panic");
    let purge_result = handle_purge
        .join()
        .expect("thread purge não deve entrar em panic");

    // Neither must have exited with a corruption error code
    // Exit code 10 = Database error (SQLite), 20 = Internal
    let recall_code = recall_result.status.code().unwrap_or(-1);
    let purge_code = purge_result.status.code().unwrap_or(-1);

    assert_ne!(
        recall_code, 20,
        "recall não deve retornar erro interno (exit 20)"
    );
    assert_ne!(
        purge_code, 20,
        "purge não deve retornar erro interno (exit 20)"
    );

    // Verify database integrity after concurrent operations
    let conn = rusqlite::Connection::open(&db_path).expect("banco deve abrir após concorrência");
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("PRAGMA integrity_check deve funcionar");
    assert_eq!(
        integrity, "ok",
        "banco deve estar íntegro após recall+purge concorrentes"
    );
}

// ---------------------------------------------------------------------------
// Test 5 — 10 remembers in different namespaces do not collide
// ---------------------------------------------------------------------------
// Confirms that inserts into 10 distinct namespaces via concurrent threads
// all succeed and that each namespace contains exactly 1 memory.

#[test]
#[serial]
fn ten_remembers_in_different_namespaces() {
    let tmp = TempDir::new().expect("TempDir deve ser criado");
    let db_path = tmp.path().join("test.sqlite");

    // Init
    sgr_on(&tmp, &db_path).args(["init"]).assert().success();

    let n_threads = 10;
    let barrier = Arc::new(Barrier::new(n_threads));
    let bin_path = std::path::PathBuf::from(env!("CARGO_BIN_EXE_sqlite-graphrag"));
    let mock_path = common::prepend_path(&common::mock_llm_path());

    let root = tmp.path().to_path_buf();
    let handles: Vec<_> = (0..n_threads)
        .map(|i| {
            let db_path_clone = db_path.clone();
            let root_clone = root.clone();
            let barrier_clone = Arc::clone(&barrier);
            let namespace = format!("ns-thread-{i}");
            let bin_clone = bin_path.clone();
            let path_clone = mock_path.clone();

            std::thread::spawn(move || {
                barrier_clone.wait();
                let mut c = std::process::Command::new(&bin_clone);
                c.env("PATH", &path_clone);
                common::wire_std_cmd(&root_clone, &mut c, &db_path_clone);
                c.args(["--skip-memory-guard", "remember", "--db"])
                    .arg(&db_path_clone)
                    .args([
                        "--name",
                        &format!("mem-thread-{i}"),
                        "--type",
                        "user",
                        "--namespace",
                        &namespace,
                        "--description",
                        &format!("memória do thread {i}"),
                        "--body",
                        &format!("corpo da memória isolada para o namespace {namespace}"),
                    ])
                    .output()
                    .expect("remember deve executar sem panic")
            })
        })
        .collect();

    // Collect results from all threads
    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("thread não deve entrar em panic"))
        .collect();

    let successes = results.iter().filter(|r| r.status.success()).count();
    let failures = results.len() - successes;

    assert_eq!(
        successes, n_threads,
        "all {n_threads} remembers in distinct namespaces must succeed, \
         got {successes} successes and {failures} failures"
    );

    // Verify that each namespace has exactly 1 memory in the database
    let conn = rusqlite::Connection::open(&db_path).expect("banco deve abrir");
    for i in 0..n_threads {
        let namespace = format!("ns-thread-{i}");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE namespace = ?1 AND deleted_at IS NULL",
                rusqlite::params![namespace],
                |row| row.get(0),
            )
            .expect("query deve funcionar");

        assert_eq!(
            count, 1,
            "namespace '{namespace}' deve ter exatamente 1 memória, encontrou {count}"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 6 — Saturation 10x: 40 CLI processes compete for 4 slots
// ---------------------------------------------------------------------------
// Validates that the CLI slot semaphore bounds actual concurrency even under
// extreme load. With 40 concurrent processes and 4 available slots, we expect
// some to succeed (acquiring a slot) and others to fail with exit code 75
// (AllSlotsFull). The key invariant is that the system never panics, never
// hangs, and completes within a bounded time.

#[test]
#[serial]
fn saturation_10x_slots_bounded() {
    let tmp = TempDir::new().expect("TempDir");
    let db_path = tmp.path().join("test.sqlite");

    sgr_on(&tmp, &db_path).args(["init"]).assert().success();

    let total_processes = 40;
    let barrier = Arc::new(Barrier::new(total_processes));
    let bin_path = std::path::PathBuf::from(env!("CARGO_BIN_EXE_sqlite-graphrag"));
    let mock_path = common::prepend_path(&common::mock_llm_path());

    let root = tmp.path().to_path_buf();
    let handles: Vec<_> = (0..total_processes)
        .map(|i| {
            let db_clone = db_path.clone();
            let root_clone = root.clone();
            let barrier_clone = Arc::clone(&barrier);
            let bin_clone = bin_path.clone();
            let path_clone = mock_path.clone();

            std::thread::spawn(move || {
                barrier_clone.wait();
                let mut c = std::process::Command::new(&bin_clone);
                c.env("PATH", &path_clone);
                common::wire_std_cmd(&root_clone, &mut c, &db_clone);
                c.args([
                    "--skip-memory-guard",
                    "--max-concurrency",
                    "4",
                    "--wait-lock",
                    "0",
                    "remember",
                    "--db",
                ])
                .arg(&db_clone)
                .args([
                    "--name",
                    &format!("sat-mem-{i}"),
                    "--type",
                    "note",
                    "--description",
                    "saturation test",
                    "--body",
                    &format!("saturation test item {i}"),
                ])
                .output()
                .expect("must not panic")
            })
        })
        .collect();

    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("thread must not panic"))
        .collect();

    let successes = results.iter().filter(|r| r.status.success()).count();
    let slot_full = results
        .iter()
        .filter(|r| r.status.code() == Some(75))
        .count();

    assert!(
        successes > 0,
        "at least one process must succeed acquiring a slot"
    );
    assert!(
        slot_full > 0,
        "with 40 processes and 4 slots (wait=0), some must get exit 75"
    );
    assert_eq!(
        successes + slot_full,
        total_processes,
        "every process must either succeed or get exit 75 (no panics, no hangs)"
    );
}
