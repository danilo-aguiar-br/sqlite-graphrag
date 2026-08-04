//! Stdin reader with timeout to prevent indefinite blocking when the
//! upstream pipe is held open without sending data.
//!
//! Used by `remember` body-from-stdin and `edit` body input to enforce a
//! deadline ([`crate::constants::DEFAULT_STDIN_READ_TIMEOUT_SECS`], override
//! via XDG `cli.stdin_timeout_secs`). When the timeout fires, the spawned
//! reader thread is leaked because `std::io::stdin()` cannot be cancelled
//! from outside; this is acceptable in error scenarios because the
//! process is about to exit anyway.
//!
//! Two refusals happen before any read is attempted:
//!
//! * `--no-input` (or XDG `cli.no_input`) makes the refusal DECLARATIVE — it
//!   holds even when a pipe is attached and would have supplied data;
//! * an interactive TTY makes it EMERGENT — there is no producer, so waiting
//!   for EOF would just burn the whole deadline.

use crate::errors::AppError;
use std::io::{IsTerminal, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Whether this process refuses to read stdin (`--no-input` / XDG `cli.no_input`).
///
/// An atomic rather than a `OnceLock` so tests can exercise both branches in
/// the same binary; production installs it exactly once from CLI bootstrap.
static NO_INPUT: AtomicBool = AtomicBool::new(false);

/// Installs the resolved `--no-input` decision for the whole process.
///
/// Called from [`crate::cli::Cli::validate_flags`], the one bootstrap hook that
/// runs after XDG initialisation and before any subcommand dispatch.
pub fn install_no_input(enabled: bool) {
    NO_INPUT.store(enabled, Ordering::Release);
}

/// Whether stdin reads are refused for this invocation.
pub fn no_input() -> bool {
    NO_INPUT.load(Ordering::Acquire)
}

/// Reads stdin to a `String` with the configured deadline.
///
/// Resolves the timeout from XDG `cli.stdin_timeout_secs`, falling back to
/// [`crate::constants::DEFAULT_STDIN_READ_TIMEOUT_SECS`]. Prefer this over
/// [`read_stdin_with_timeout`] at call sites that have no reason to pick a
/// different budget.
///
/// # Errors
/// Same as [`read_stdin_with_timeout`].
pub fn read_stdin() -> Result<String, AppError> {
    read_stdin_with_timeout(crate::runtime_config::stdin_timeout_secs())
}

/// Reads stdin to a `String` with a hard deadline.
///
/// Returns `AppError::Validation` immediately when `--no-input` is in force,
/// and `AppError::Internal` immediately when stdin is attached to a terminal
/// (TTY) — the caller must redirect data via a pipe or file.
///
/// # Errors
/// Returns `AppError::Validation` when `--no-input` is in force,
/// `AppError::Internal` when stdin is a TTY, when the read does
/// not finish within `secs` seconds, or `AppError::Io` when the
/// underlying read fails.
pub fn read_stdin_with_timeout(secs: u64) -> Result<String, AppError> {
    if no_input() {
        return Err(AppError::Validation(
            crate::i18n::validation::no_input_blocks_stdin(),
        ));
    }
    if std::io::stdin().is_terminal() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "stdin is attached to a terminal; pipe data via stdin \
             (e.g. `echo ... | sqlite-graphrag ...` or `... < file`) \
             or use --body instead of the stdin body flag"
        )));
    }
    let (tx, rx) = mpsc::channel::<std::io::Result<String>>();
    thread::spawn(move || {
        let mut buf = String::new();
        let result = std::io::stdin().read_to_string(&mut buf).map(|_| buf);
        let _ = tx.send(result);
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(Ok(buf)) => Ok(buf),
        Ok(Err(e)) => Err(AppError::Io(e)),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(AppError::Internal(anyhow::anyhow!(
            "stdin read timed out after {secs}s; pipe must close within timeout window"
        ))),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(AppError::Internal(anyhow::anyhow!(
            "stdin reader thread disconnected unexpectedly"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Serialises the tests that flip the process-wide `NO_INPUT` flag.
    static NO_INPUT_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // Note: we cannot easily test the success path because tests inherit stdin
    // from the test runner. We only assert the timeout path here.
    #[test]
    fn read_stdin_with_timeout_returns_internal_error_on_timeout() {
        let _guard = NO_INPUT_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        install_no_input(false);
        // 1s is enough — stdin in test runner is typically a tty or pipe with no input.
        let start = Instant::now();
        let result = read_stdin_with_timeout(1);
        let elapsed = start.elapsed();
        // We expect either a timeout (most cases), an immediate TTY error, or a
        // successful EOF read (rare in CI environments).
        match result {
            Err(AppError::Internal(e)) => {
                let msg = e.to_string();
                // Accept both the TTY-detected error and the timeout error.
                assert!(
                    msg.contains("timed out") || msg.contains("terminal"),
                    "unexpected internal error: {msg}"
                );
                // TTY path exits immediately; timeout path takes ~1s.
                assert!(elapsed.as_secs_f64() < 2.5);
            }
            Ok(_) | Err(AppError::Io(_)) => {
                // EOF reached before timeout — also acceptable in CI environments.
            }
            Err(other) => unreachable!("stdin test: expected Internal/Io, got {other:?}"),
        }
    }

    #[test]
    fn no_input_refuses_before_the_read_is_attempted() {
        let _guard = NO_INPUT_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        install_no_input(true);
        // A 600-second budget would dominate the elapsed time if the refusal
        // happened after the read rather than before it.
        let start = Instant::now();
        let result = read_stdin_with_timeout(600);
        let elapsed = start.elapsed();
        install_no_input(false);
        match result {
            Err(AppError::Validation(msg)) => {
                assert!(msg.contains("--no-input"), "unexpected message: {msg}");
            }
            other => unreachable!("expected Validation under --no-input, got {other:?}"),
        }
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "refusal must precede the read, took {elapsed:?}"
        );
    }

    #[test]
    fn install_no_input_round_trips() {
        let _guard = NO_INPUT_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        install_no_input(true);
        assert!(no_input());
        install_no_input(false);
        assert!(!no_input());
    }

    // TTY detection cannot be simulated in unit tests because the test runner
    // always provides a non-TTY stdin (pipe). Empirical validation:
    //   cargo run --release -- remember --name h1-test  (with the stdin body flag)
    // Expected: exits in <2s with "stdin is attached to a terminal" message.
}
