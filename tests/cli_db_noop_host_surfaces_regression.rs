//! GAP-SG-139: host/XDG leaves must accept `--db <PATH>` as a no-op.
//!
//! Agents that append `--db` to every one-shot invocation must not receive
//! clap exit 2 on `config`, `slots`, `cache`, `codex-models`, or `completions`.
//! These surfaces never open the graph path: the sentinel file must not be
//! created after a successful parse/run.
//!
//! Exit status is measured **without** piping through `head` (pipeline exit
//! would mask clap status 2).

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn bin() -> Command {
    // Prefer the built test binary when available; fall back to cargo run.
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_sqlite-graphrag") {
        Command::new(path)
    } else {
        let mut c = Command::new(env!("CARGO"));
        c.arg("run")
            .arg("--quiet")
            .arg("--bin")
            .arg("sqlite-graphrag")
            .arg("--");
        c
    }
}

fn run_args(args: &[&str]) -> (i32, String, String) {
    let mut cmd = bin();
    for a in args {
        cmd.arg(a);
    }
    let output = cmd
        .output()
        .expect("spawn sqlite-graphrag for GAP-SG-139 host surface test");
    let status = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (status, stdout, stderr)
}

fn clap_rejected(stderr: &str) -> bool {
    stderr.contains("unexpected argument")
        || stderr.contains("unknown option")
        || stderr.contains("unrecognized")
}

fn sentinel_path(tmp: &TempDir) -> PathBuf {
    tmp.path().join("gap-sg-139-must-not-be-created.sqlite")
}

/// Assert clap accepted `--db` and the sentinel path was not created (no-op).
fn assert_db_noop(label: &str, args: &[&str], sentinel: &Path) {
    assert!(
        !sentinel.exists(),
        "precondition: sentinel must not exist before {label}"
    );
    let (status, stdout, stderr) = run_args(args);
    assert!(
        !clap_rejected(&stderr),
        "GAP-SG-139 REGRESSION: `{label}` rejected --db\nstatus={status}\nstderr={stderr}\nstdout={stdout}"
    );
    assert!(
        !sentinel.exists(),
        "GAP-SG-139 REGRESSION: `{label}` opened/created --db path {} (must be no-op)",
        sentinel.display()
    );
    // status 2 is clap usage; host surfaces should not hit that for --db alone.
    assert_ne!(
        status, 2,
        "GAP-SG-139 REGRESSION: `{label}` exit 2 (likely clap)\nstderr={stderr}\nstdout={stdout}"
    );
}

#[test]
fn host_surfaces_accept_db_noop_without_creating_path() {
    let tmp = TempDir::new().expect("tempdir");
    let sentinel = sentinel_path(&tmp);
    let s = sentinel.to_str().expect("utf8 path");

    // Read-only / non-destructive leaves (safe without init).
    assert_db_noop("config doctor", &["config", "doctor", "--db", s], &sentinel);
    assert_db_noop("config path", &["config", "path", "--db", s], &sentinel);
    assert_db_noop("config list", &["config", "list", "--db", s], &sentinel);
    assert_db_noop(
        "config list-keys",
        &["config", "list-keys", "--db", s],
        &sentinel,
    );
    assert_db_noop(
        "config get",
        &["config", "get", "embedding.dim", "--db", s],
        &sentinel,
    );
    assert_db_noop("slots status", &["slots", "status", "--db", s], &sentinel);
    assert_db_noop(
        "slots cleanup dry-run",
        &["slots", "cleanup", "--dry-run", "--db", s],
        &sentinel,
    );
    assert_db_noop("cache list", &["cache", "list", "--db", s], &sentinel);
    assert_db_noop("cache stats", &["cache", "stats", "--db", s], &sentinel);
    assert_db_noop("codex-models", &["codex-models", "--db", s], &sentinel);
    assert_db_noop(
        "completions bash",
        &["completions", "bash", "--db", s],
        &sentinel,
    );

    // Destructive / mutating leaves: only prove clap accepts --db.
    // Use impossible fingerprint / missing slot so domain fails after parse.
    assert_db_noop(
        "config remove-key",
        &["config", "remove-key", "deadbeef-gap-sg-139", "--db", s],
        &sentinel,
    );
    assert_db_noop(
        "slots release",
        &["slots", "release", "--slot-id", "0", "--yes", "--db", s],
        &sentinel,
    );
    // cache clear-models without --yes → validation after clap accepts --db
    let (status, stdout, stderr) = run_args(&["cache", "clear-models", "--db", s]);
    assert!(
        !clap_rejected(&stderr),
        "cache clear-models rejected --db\nstatus={status}\nstderr={stderr}\nstdout={stdout}"
    );
    assert!(!sentinel.exists());
    assert_ne!(status, 2);

    // config set/unset: use a known key then unset (XDG only; still no graph).
    assert_db_noop(
        "config set",
        &["config", "set", "log.level", "info", "--db", s],
        &sentinel,
    );
    assert_db_noop(
        "config unset",
        &["config", "unset", "log.level", "--db", s],
        &sentinel,
    );
}

#[test]
fn config_doctor_with_db_emits_json_exit_zero() {
    let tmp = TempDir::new().expect("tempdir");
    let sentinel = sentinel_path(&tmp);
    let s = sentinel.to_str().expect("utf8");
    let (status, stdout, stderr) = run_args(&["config", "doctor", "--db", s]);
    assert_eq!(
        status, 0,
        "config doctor --db must exit 0\nstderr={stderr}\nstdout={stdout}"
    );
    assert!(
        stdout.contains("product_env_reads") || stdout.contains("config_path"),
        "expected JSON doctor payload, got: {stdout}"
    );
    assert!(!sentinel.exists(), "doctor must not create --db path");
    assert!(!clap_rejected(&stderr));
}

#[test]
fn slots_status_with_db_exit_zero() {
    let tmp = TempDir::new().expect("tempdir");
    let sentinel = sentinel_path(&tmp);
    let s = sentinel.to_str().expect("utf8");
    let (status, stdout, stderr) = run_args(&["slots", "status", "--db", s]);
    assert_eq!(
        status, 0,
        "slots status --db must exit 0\nstderr={stderr}\nstdout={stdout}"
    );
    assert!(
        stdout.contains("slots_status") || stdout.contains("max_concurrency"),
        "expected slots JSON, got: {stdout}"
    );
    assert!(!sentinel.exists());
}
