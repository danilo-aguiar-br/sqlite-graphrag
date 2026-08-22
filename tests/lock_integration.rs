// E2E integration tests for the sqlite-graphrag slot semaphore.
//
// GAP-SG-101: `XDG_CACHE_HOME` is the real isolation channel; the retired
// product env `SQLITE_GRAPHRAG_CACHE_DIR` that these tests used before v1.2.0
// had no reader, so the binary competed for slots in the developer's real
// cache. Because `ProjectDirs::cache_dir()` appends the application directory,
// lock files land under `<XDG_CACHE_HOME>/sqlite-graphrag`, not at the root —
// see `slots_root`.
//
// ISOLATION: every test sets `XDG_CACHE_HOME` pointing
// to an exclusive `TempDir`, ensuring lock files do not pollute
// `~/.cache/sqlite-graphrag` nor collide between tests.
//
// `#[serial]` is mandatory in all tests: although each test uses
// its own directory, the compiled binary is shared and `TempDir` is only
// released after the test ends; serializing eliminates filesystem races
// and makes timings predictable.
//
// Scenarios 4 and 5 hold the slots from inside the test process itself, so
// nothing depends on an external process winning a race: scenario 4 keeps every
// lock alive across the child invocation, and scenario 5 grants the child a 10s
// `--wait-lock` window against a 1s hold. They ran as `#[ignore]` for that
// supposed flakiness and therefore never guarded exit 75 in CI at all.

use assert_cmd::Command;
use serial_test::serial;
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

/// Directory where the binary actually writes CLI slot locks.
///
/// `ProjectDirs::cache_dir()` appends the application directory under
/// `XDG_CACHE_HOME`, so locks land one level below the sandbox root. Tests that
/// plant or inspect lock files by hand MUST go through this helper; using
/// `tmp.path()` directly looks right and silently inspects an empty directory.
fn slots_root(tmp: &TempDir) -> std::path::PathBuf {
    let dir = tmp.path().join("sqlite-graphrag");
    std::fs::create_dir_all(&dir).expect("slots_root: mkdir must succeed");
    dir
}

/// Returns the lock file path for the given slot (1-based)
/// within the provided `TempDir`, mirroring the logic of `lock.rs`.
fn slot_path(tmp: &TempDir, slot: usize) -> std::path::PathBuf {
    slots_root(tmp).join(format!("cli-slot-{slot}.lock"))
}

// ---------------------------------------------------------------------------
// Scenario 1 — slot is released after process exits
// ---------------------------------------------------------------------------
// Ensures that two sequential invocations without --max-concurrency do not conflict,
// since the first process releases the slot on exit.

#[test]
#[serial]
fn slot_released_after_process_exits() {
    let tmp = TempDir::new().expect("TempDir deve ser criado");

    // First invocation — must acquire and release slot 1.
    sgr_cmd()
        .env("XDG_CACHE_HOME", tmp.path())
        .args(["--skip-memory-guard", "namespace-detect"])
        .assert()
        .success();

    // Second invocation — must acquire the slot again without error.
    sgr_cmd()
        .env("XDG_CACHE_HOME", tmp.path())
        .args(["--skip-memory-guard", "namespace-detect"])
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// Scenario 2 — slot file is created in the configured cache dir
// ---------------------------------------------------------------------------
// Confirms that the binary creates `cli-slot-1.lock` in the directory overridden via
// `XDG_CACHE_HOME`.

#[test]
#[serial]
fn slot_file_created_in_cache_dir() {
    let tmp = TempDir::new().expect("TempDir deve ser criado");

    sgr_cmd()
        .env("XDG_CACHE_HOME", tmp.path())
        .args(["--skip-memory-guard", "namespace-detect"])
        .assert()
        .success();

    assert!(
        slot_path(&tmp, 1).exists(),
        "cli-slot-1.lock deve existir em {:?} após invocação do binário",
        tmp.path()
    );
}

// ---------------------------------------------------------------------------
// Scenario 3 — --wait-lock 0 fails immediately when all slots are busy
// ---------------------------------------------------------------------------
// Simulates N busy slots by creating and locking the lock files directly,
// then confirms that a new invocation returns exit 75 without waiting.

#[test]
#[serial]
fn wait_lock_zero_returns_75_when_slots_busy() {
    use fs4::fs_std::FileExt;
    use std::fs::OpenOptions;

    let tmp = TempDir::new().expect("TempDir deve ser criado");
    let max = 4;

    // Lock all N slots directly to simulate N running instances.
    let mut handles = Vec::new();
    for slot in 1..=max {
        let path = slot_path(&tmp, slot);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .expect("criação do lock file deve funcionar");
        file.try_lock_exclusive()
            .unwrap_or_else(|_| panic!("slot {slot} deve estar livre para testes"));
        handles.push(file);
    }

    // Invocation with all slots busy and --wait-lock 0 → exit 75.
    sgr_cmd()
        .env("XDG_CACHE_HOME", tmp.path())
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

    // Release the locks before drop(tmp).
    drop(handles);
}

// ---------------------------------------------------------------------------
// Scenario 4 — second instance receives exit 75 while slot is busy
// ---------------------------------------------------------------------------
// The locks are held by this process for the whole child invocation, and the
// child runs with `--wait-lock 0`, so the exit 75 is deterministic.

#[test]
#[serial]
fn slot_bloqueia_segunda_instancia_com_exit_75() {
    use fs4::fs_std::FileExt;
    use std::fs::OpenOptions;

    let tmp = TempDir::new().expect("TempDir deve ser criado");

    // Lock all slots (default 4) to simulate maximum saturation.
    let mut handles = Vec::new();
    for slot in 1..=4 {
        let path = slot_path(&tmp, slot);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .expect("criação do lock file deve funcionar");
        file.try_lock_exclusive().expect("slot deve estar livre");
        handles.push(file);
    }

    std::thread::sleep(std::time::Duration::from_millis(100));

    // Second instance must fail immediately with exit 75.
    sgr_cmd()
        .env("XDG_CACHE_HOME", tmp.path())
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
// Scenario 5 — --wait-lock waits and acquires the slot after release
// ---------------------------------------------------------------------------
// Costs ~1s: the locks are released after 1s while the child waits up to 10s.

#[test]
#[serial]
fn wait_lock_espera_e_adquire_slot() {
    use fs4::fs_std::FileExt;
    use std::fs::OpenOptions;

    let tmp = TempDir::new().expect("TempDir deve ser criado");

    // Lock all 4 slots.
    let mut handles = Vec::new();
    for slot in 1..=4 {
        let path = slot_path(&tmp, slot);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .expect("criação do lock file deve funcionar");
        file.try_lock_exclusive().expect("slot deve estar livre");
        handles.push(file);
    }

    // Release all after 1 second in a separate thread.
    let tmp_path = tmp.path().to_path_buf();
    let _ = tmp_path; // silence unused warning
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        drop(handles);
    });

    // --wait-lock 10 must wait for release and complete successfully.
    sgr_cmd()
        .env("XDG_CACHE_HOME", tmp.path())
        .args([
            "--skip-memory-guard",
            "--max-concurrency",
            "4",
            "--wait-lock",
            "10",
            "namespace-detect",
        ])
        .assert()
        .success();
}
