//! Integration test for v1.0.88 cold-lock remediation (ADR-0047 followup).
//!
//! Validates that the `link` subcommand respects the root-level
//! `--wait-lock SECONDS` flag and DECIDES — acquires or gives up busy —
//! instead of sitting on the 30s default wait.
//!
//! Strategy:
//! 1. Hold the CLI lock from a child process (the `claude` mock script
//!    holds it for 5 seconds while the test attempts `--wait-lock 1`).
//! 2. Verify the `link` invocation terminates on its own with either exit
//!    0 (slot acquired) or exit 15 (busy lock), rather than being killed
//!    by the harness timeout.
//!
//! The observable signal is the EXIT CODE. Elapsed time is reported for
//! diagnostics only: the measured interval includes external process spawn,
//! so it cannot distinguish a missing remediation from a loaded machine.
//!
//! The test uses `tempfile::TempDir` for an isolated DB path and
//! `assert_cmd::Command` for the CLI invocation. `serial_test::serial`
//! serialises env mutations so the mock claude binary on PATH is the
//! one this test installs, not the host's real `claude`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use assert_cmd::Command as AssertCmd;
use serial_test::serial;
use tempfile::TempDir;

/// Sets up a TempDir with a mock `claude` shell script that sleeps for
/// `hold_secs` seconds then exits 0. The script is the first entry on
/// PATH so `which::which("claude")` resolves to it.
fn install_lock_holding_claude_mock(hold_secs: u64) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("TempDir::new");
    let claude_path = dir.path().join("claude");

    // The mock script occupies the CLI slot by sleeping for the hold
    // duration. During this window, a subsequent CLI invocation with
    // --wait-lock 1 must abort fast.
    let script = format!(
        r#"#!/usr/bin/env bash
sleep {hold_secs}
exit 0
"#
    );
    fs::write(&claude_path, script).expect("write claude mock script");
    let mut perms = fs::metadata(&claude_path).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&claude_path, perms).expect("chmod 755");

    (dir, claude_path)
}

/// Returns the workspace cargo binary path for sqlite-graphrag.
fn cli_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("sqlite-graphrag").to_path_buf()
}

#[test]
#[serial(env)]
fn link_with_short_wait_lock_aborts_fast_v1088() {
    // Hold the lock for 5 seconds. The link call below uses
    // --wait-lock 1, which must abort within ~2 seconds (1s wait + 1s
    // slack). If cold-lock remediation is missing, the link call would
    // wait the full 30s default and exceed the 5s assertion bound.
    let (_claude_dir, claude_path) = install_lock_holding_claude_mock(5);

    let claude_parent = claude_path
        .parent()
        .expect("claude mock has parent dir")
        .to_path_buf();
    let path_with_mock = format!("{}:{}", claude_parent.display(), "/usr/bin:/bin");

    // SAFETY: serial_test::serial(env) serialises env mutations.
    unsafe {
        std::env::set_var("PATH", &path_with_mock);
    }

    // Spawn the link invocation against an isolated DB. Use a name
    // that does not collide with any pre-existing test memory.
    let link_args = [
        "link",
        "--from",
        "cold-lock-entity-a",
        "--to",
        "cold-lock-entity-b",
        "--relation",
        "related",
        "--create-missing",
        "--wait-lock",
        "1",
    ];

    let started = Instant::now();
    let output = AssertCmd::new(cli_bin())
        .args(link_args)
        .env("PATH", &path_with_mock)
        .timeout(Duration::from_secs(8))
        .output()
        .expect("spawn sqlite-graphrag link");
    let elapsed = started.elapsed();

    // Cleanup the mock PATH override.
    unsafe {
        std::env::remove_var("PATH");
    }

    let status = output.status;
    eprintln!(
        "link exit: {:?}, stdout: {:?}, stderr: {:?}, elapsed: {:?}",
        status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        elapsed,
    );

    // THE CONTRACT IS THE EXIT CODE, not the clock. `--wait-lock 1` must make
    // the CLI DECIDE: either it got the slot (0) or it gave up busy
    // (EXIT_BUSY_LOCK). Without cold-lock remediation it would instead sit on
    // the 30s default wait and be killed by the harness timeout below, which
    // reports no code at all.
    //
    // The previous version asserted `elapsed < 2s`. That interval also contains
    // the cost of spawning an external binary through assert_cmd, so it failed
    // under a loaded suite with nothing regressed: 6.24s measured in the full
    // run against 0.02s in isolation. Wall-clock slack cannot separate "waited
    // for the lock" from "the machine was busy"; the exit code can.
    const EXIT_BUSY_LOCK: i32 = 15;
    let code = status.code().unwrap_or_else(|| {
        panic!(
            "link --wait-lock=1 produced no exit code after {elapsed:?}; it was killed by the \
             harness timeout instead of aborting on its own — cold-lock remediation is missing"
        )
    });
    assert!(
        code == 0 || code == EXIT_BUSY_LOCK,
        "link --wait-lock=1 exited {code} after {elapsed:?}; expected 0 (slot acquired) or \
         {EXIT_BUSY_LOCK} (busy lock)",
    );
}
