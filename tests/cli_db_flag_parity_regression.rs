//! GAP-E2E-008 regression test: every namespace-scoped subcommand must accept
//! `--db <PATH>` for parity with the rest of the CLI surface.
//!
//! The test invokes each subcommand through the already-built integration
//! binary (`CARGO_BIN_EXE_sqlite-graphrag`) with an explicit `--db <PATH>`
//! and asserts that clap accepts the flag. Nested `cargo run` is forbidden
//! here — it contende o build lock e produz falsos negativos sob paralelismo.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn sgr_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sqlite-graphrag"))
}

/// Runs `sqlite-graphrag <args>... --db <PATH>` and returns status/stdout/stderr.
fn run_with_db(subcommand_args: &[&str], db_path: &Path) -> (i32, String, String) {
    let mut cmd = sgr_bin();
    for a in subcommand_args {
        cmd.arg(a);
    }
    cmd.arg("--db").arg(db_path);

    let output = cmd
        .output()
        .expect("spawn sqlite-graphrag for cli_db_flag_parity_regression");
    let status = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (status, stdout, stderr)
}

fn init_db(db_path: &Path) -> (i32, String, String) {
    let output = sgr_bin()
        .arg("init")
        .arg("--db")
        .arg(db_path)
        .output()
        .expect("spawn sqlite-graphrag init");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn with_initialised_db<F: FnOnce(&Path)>(body: F) {
    let tmp = TempDir::new().expect("tempdir for cli_db_flag_parity_regression");
    let db_path = tmp.path().join("parity.sqlite");
    let (init_status, init_stdout, init_stderr) = init_db(&db_path);
    assert!(
        init_status == 0,
        "FATAL: `init --db {}` returned status={}; stdout={}; stderr={}; \
         cannot run parity checks.",
        db_path.display(),
        init_status,
        init_stdout,
        init_stderr
    );
    body(&db_path);
}

/// Asserts clap accepted `--db`. Only clap-style rejections (exit 2 + classic
/// clap wording) count — runtime `validation error:` / storage errors prove
/// the flag reached the handler and MUST NOT be treated as rejection.
fn assert_db_flag_accepted(label: &str, subcommand_args: &[&str], db_path: &Path) {
    let (status, stdout, stderr) = run_with_db(subcommand_args, db_path);

    let clap_rejected = status == 2
        && (stderr.contains("unexpected argument")
            || stderr.contains("unrecognized")
            || stderr.contains("unknown option")
            || stderr.contains("the following required arguments were not provided")
            || stderr.contains("argument that wasn't expected")
            || stderr.contains("Found argument")
            || stderr.contains("error: unexpected")
            || stderr.contains("error: unrecognized"));

    assert!(
        !clap_rejected,
        "REGRESSION GAP-E2E-008: subcommand `{label}` rejected `--db` flag.\n\
         stderr: {stderr}\nstdout: {stdout}\nstatus: {status}\n\
         Expected: clap accepts `--db <PATH>` as a valid argument.",
    );
}

#[test]
fn assert_db_flag_on_embedding_status() {
    with_initialised_db(|db_path| {
        assert_db_flag_accepted("embedding status", &["embedding", "status"], db_path);
    });
}

#[test]
fn assert_db_flag_on_embedding_list() {
    with_initialised_db(|db_path| {
        assert_db_flag_accepted(
            "embedding list",
            &["embedding", "list", "--limit", "10"],
            db_path,
        );
    });
}

#[test]
fn assert_db_flag_on_embedding_abandon() {
    with_initialised_db(|db_path| {
        assert_db_flag_accepted(
            "embedding abandon <id>",
            &["embedding", "abandon", "999999", "--yes"],
            db_path,
        );
    });
}

#[test]
fn assert_db_flag_on_all_namespace_subcommands() {
    with_initialised_db(|db_path| {
        assert_db_flag_accepted("embedding status", &["embedding", "status"], db_path);
        assert_db_flag_accepted(
            "embedding list",
            &["embedding", "list", "--limit", "10"],
            db_path,
        );
        assert_db_flag_accepted(
            "embedding abandon <id>",
            &["embedding", "abandon", "999999", "--yes"],
            db_path,
        );
    });
}
