//! GAP-SG-190: `--no-input` promised an exit code the CLI never emits.
//!
//! `src/cli/globals.rs` documented the declarative stdin refusal as failing
//! "up front with exit 65", and seven operator-facing documents repeated the
//! figure. No arm of [`sqlite_graphrag::errors::AppError::exit_code`] produces
//! 65: the refusal raises `Validation`, which maps to 1. An agent branching on
//! 65 therefore never matched the case it was told to handle, and fell through
//! to whatever its default branch did.
//!
//! These tests pin the code the binary actually returns, with a pipe attached —
//! the condition that separates the DECLARATIVE refusal from the emergent TTY
//! one, and the only one where the promise was observable.

use assert_cmd::Command;

/// Exit status of `sqlite-graphrag <args>` with `payload` on stdin.
fn exit_code_with_stdin(args: &[&str], payload: &str) -> i32 {
    let db = tempfile::Builder::new()
        .suffix(".sqlite")
        .tempfile()
        .expect("temp db");
    let mut cmd = Command::cargo_bin("sqlite-graphrag").expect("bin");
    let out = cmd
        .args(args)
        .arg("--db")
        .arg(db.path())
        .write_stdin(payload.to_string())
        .output()
        .expect("run");
    out.status.code().expect("process returned a status code")
}

#[test]
fn no_input_refuses_remember_batch_with_exit_1() {
    let code = exit_code_with_stdin(
        &["--no-input", "remember-batch"],
        "{\"name\":\"x\",\"type\":\"note\",\"description\":\"d\",\"body\":\"b\"}\n",
    );
    assert_eq!(
        code, 1,
        "the refusal must report exit 1 (Validation); 65 is not in this CLI's vocabulary"
    );
}

/// The refusal has to beat the pipe, not race it.
///
/// Without the flag this invocation would consume the NDJSON and try to write.
/// Exit 1 with a populated pipe is what proves the refusal happened before the
/// read rather than as a consequence of it.
#[test]
fn the_refusal_precedes_the_read_even_with_data_waiting() {
    let with_data = exit_code_with_stdin(&["--no-input", "remember-batch"], "{\"name\":\"y\"}\n");
    let without_data = exit_code_with_stdin(&["--no-input", "remember-batch"], "");
    assert_eq!(
        with_data, without_data,
        "a populated pipe changed the outcome, so the refusal is not declarative"
    );
    assert_eq!(with_data, 1);
}

/// The help text may not resurrect the figure the binary cannot produce.
#[test]
fn help_no_longer_advertises_exit_65() {
    let mut cmd = Command::cargo_bin("sqlite-graphrag").expect("bin");
    let out = cmd.arg("--help").output().expect("run help");
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(
        !text.contains("exit 65"),
        "help advertises exit 65, which no AppError arm returns:\n{text}"
    );
}
