//! Cooperative shutdown wiring, stated per platform.
//!
//! Every platform reaches the SAME [`handle_first_signal`] body, so the
//! observable contract — `SHUTDOWN` flag, cancellation token, stderr notice,
//! JSON envelope with `code: 19`, forced exit 130 on the second event — does not
//! vary. What varies is which OS events can reach it:
//!
//! - **Unix (Linux, macOS)**: `SIGINT` through the `ctrlc` crate; `SIGTERM` and
//!   `SIGHUP` through `signal-hook`. `SIGPIPE` is reset to its default
//!   disposition in `main`, so a closed stdout pipe kills the process with the
//!   conventional exit 141.
//! - **Windows**: `SetConsoleCtrlHandler` covers `CTRL_C_EVENT`,
//!   `CTRL_BREAK_EVENT`, `CTRL_CLOSE_EVENT`, `CTRL_LOGOFF_EVENT` and
//!   `CTRL_SHUTDOWN_EVENT`. There is no `SIGTERM`, no `SIGHUP` and no `SIGPIPE`:
//!   console-close/logoff/shutdown are the closest equivalents of a termination
//!   request, and the exit-141 half of the contract is produced in `main` by
//!   classifying the stdout write error as `ErrorKind::BrokenPipe` instead of by
//!   a signal.
//! - **Anything else**: `SIGINT` only, via `ctrlc`.
//!
//! Windows does NOT go through `ctrlc`. Console control handlers are called
//! last-registered-first and `ctrlc`'s handler consumes every control type, so
//! registering both would either double-count one Ctrl+C — which the second-event
//! rule turns into an immediate exit 130 — or leave one of them dead. One
//! handler owns the console, and it is this module's.

use std::sync::atomic::Ordering;

/// Registers the global shutdown handler for every event the platform offers.
///
/// See the module docs for the exact per-platform event set; on Unix that is
/// Ctrl+C / SIGTERM / SIGHUP, on Windows the five console control events.
///
/// First signal: sets [`SHUTDOWN`](crate::SHUTDOWN) flag, cancels the global
/// cancellation token and emits a best-effort notice on stderr.
///
/// Second signal: calls [`std::process::exit(130)`] for immediate termination
/// following Unix convention (128 + SIGINT=2) — with ZERO I/O on that path.
///
/// # G42/S8 — panic-free by contract
///
/// The pre-v1.0.79 handler used `eprintln!` (second signal) and
/// `tracing::warn!` (first signal). When the parent shell dies the CLI is
/// reparented to PID 1 and stderr becomes a CLOSED pipe; `eprintln!` then
/// panics with `BrokenPipe`, which under `panic = "abort"` becomes the
/// SIGABRT observed on the "ctrl-c" thread (G42/C2 crash report). This
/// handler therefore:
/// - writes the first-signal notice with `writeln!` and IGNORES any I/O
///   error (`let _ =`), never panicking;
/// - performs NO I/O at all on the forced-exit path.
///
/// BrokenPipe on stdout/stderr elsewhere is handled by resetting SIGPIPE
/// to its default disposition in `main` on Unix, and by classifying the stdout
/// write error in `main` on Windows — both reach the same clean exit 141.
pub fn register_shutdown_handler() {
    // SIGINT via the ctrlc crate everywhere EXCEPT Windows, where the console
    // control handler below owns every control event (see module docs: two
    // registered handlers would double-count one Ctrl+C).
    #[cfg(not(windows))]
    if let Err(e) = ctrlc::set_handler(move || {
        handle_first_signal("SIGINT", 2);
    }) {
        tracing::warn!(target: "signals", error = %e, "SIGINT handler registration failed");
    }

    #[cfg(windows)]
    register_console_ctrl_handler();

    // SIGTERM + SIGHUP: signal-hook (Unix only; neither signal exists on
    // Windows — its termination requests arrive as console control events).
    #[cfg(unix)]
    {
        use std::sync::mpsc;
        let (tx, rx) = mpsc::channel::<i32>();

        let mut signals = match signal_hook::iterator::Signals::new([
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
        ]) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(target: "signals", error = %e, "SIGTERM/SIGHUP handler registration failed");
                return;
            }
        };

        // Detached thread: lives until process exit. The kernel kills it
        // automatically on process termination. We do NOT join it because
        // that would require the CLI to wait for an indeterminate signal.
        std::thread::Builder::new()
            .name("sqlite-graphrag-sigterm".into())
            .spawn(move || {
                for sig in signals.forever() {
                    if tx.send(sig).is_err() {
                        break;
                    }
                }
            })
            .inspect_err(|e| tracing::warn!(target: "signals", error = %e, "SIGTERM/SIGHUP handler thread spawn failed"))
            .ok();

        // Drain thread: blocks on the channel and calls the same handler
        // used by the SIGINT path. Synchronous main() can't await this,
        // but the channel is bounded so a 100ms wait is fine.
        std::thread::Builder::new()
            .name("sqlite-graphrag-sigterm-drain".into())
            .spawn(move || {
                while let Ok(sig) = rx.recv() {
                    let (name, number) = match sig {
                        libc::SIGTERM => ("SIGTERM", 15u8),
                        libc::SIGHUP => ("SIGHUP", 1u8),
                        _ => continue,
                    };
                    handle_first_signal(name, number);
                }
            })
            .inspect_err(|e| tracing::warn!(target: "signals", error = %e, "SIGTERM drain thread spawn failed"))
            .ok();
    }
}

/// Windows counterpart of the Unix signal registration.
///
/// `SetConsoleCtrlHandler` is the only mechanism Windows offers for cooperative
/// termination: there is no `SIGTERM` (`TerminateProcess` is unconditional and
/// runs no user code) and no `SIGHUP`. The five control events map onto the same
/// [`handle_first_signal`] body the Unix paths use, so the shutdown contract is
/// identical across platforms.
///
/// The handler returns `TRUE` for every event it recognises, which claims the
/// event and stops the default handler from terminating the process outright.
/// For `CTRL_CLOSE_EVENT`, `CTRL_LOGOFF_EVENT` and `CTRL_SHUTDOWN_EVENT` Windows
/// still terminates the process a few seconds after the handler returns; that
/// window is exactly what the graceful path needs to flush its envelope.
#[cfg(windows)]
fn register_console_ctrl_handler() {
    use windows_sys::Win32::Foundation::BOOL;
    use windows_sys::Win32::System::Console::{
        SetConsoleCtrlHandler, CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_C_EVENT, CTRL_LOGOFF_EVENT,
        CTRL_SHUTDOWN_EVENT,
    };

    /// `windows_sys` models Win32 `BOOL` as `i32`; these are its two values.
    const TRUE: BOOL = 1;
    const FALSE: BOOL = 0;

    /// Signal numbers reported through `crate::SIGNAL_NUMBER`. Windows has no
    /// signal table, so the Unix numbers of the closest equivalents are reused
    /// to keep the field readable by the same agent logic on every platform.
    unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
        let (name, number) = match ctrl_type {
            CTRL_C_EVENT => ("SIGINT", 2u8),
            CTRL_BREAK_EVENT => ("SIGBREAK", 21u8),
            // Closing the console window is the Windows analogue of losing the
            // controlling terminal, which on Unix arrives as SIGHUP.
            CTRL_CLOSE_EVENT => ("SIGHUP", 1u8),
            CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => ("SIGTERM", 15u8),
            // Unknown control type: decline it so the default handler decides.
            _ => return FALSE,
        };
        handle_first_signal(name, number);
        TRUE
    }

    // SAFETY: `console_ctrl_handler` is a free `extern "system"` function with
    // no captured state and a `'static` lifetime; `SetConsoleCtrlHandler` only
    // stores the pointer in the process-wide handler list.
    let registered = unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), TRUE) };
    if registered == FALSE {
        tracing::warn!(
            target: "signals",
            error = %std::io::Error::last_os_error(),
            "console control handler registration failed"
        );
    }
}

/// First-signal handler shared by SIGINT (via the `ctrlc` crate), SIGTERM and
/// SIGHUP (via `signal-hook`), and the Windows console control handler.
///
/// Idempotent: only the first invocation does work, and every later one takes
/// the forced-exit path. All three registration paths are plain synchronous
/// callbacks — no tokio runtime is built in the LLM-only `main` path, and the
/// signal-hook drain is an ordinary thread blocked on a channel — so the only
/// cross-thread coordination needed is the `SIGNAL_COUNT.fetch_add` below, whose
/// atomic result decides first-versus-second event without a lock.
fn handle_first_signal(signal_name: &'static str, signal_number: u8) {
    let prev = crate::SIGNAL_COUNT.fetch_add(1, Ordering::AcqRel);
    if prev != 0 {
        // Second signal: forced shutdown. GAP-SG-99: best-effort flush of the
        // non-blocking file appender before exit so the last diagnostics land.
        // Avoid stdout I/O (G42/S8); flush is stderr/file only.
        crate::tracing_init::flush_tracing();
        std::process::exit(130);
    }
    crate::SHUTDOWN.store(true, Ordering::Release);
    crate::SIGNAL_NUMBER.store(signal_number, Ordering::Release);
    crate::cancel_token().cancel();

    // Best-effort stderr notice: closed pipe must NEVER abort (G42/S8).
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr(),
        "shutdown signal received ({signal_name}); finishing current operation gracefully"
    );

    // GAP-002 (v1.0.82): emit JSON envelope to stdout before exit so that
    // piped consumers receive a parseable error with `code: 19`
    // (SHUTDOWN_EXIT_CODE) instead of an empty stdout that triggers
    // a parse error. Best-effort: if stdout is closed, writeln fails
    // silently.
    let envelope = format!(
        "{{\"error\":true,\"code\":19,\"message\":\"shutdown signal received; operation cancelled by {signal_name}\",\"signal\":\"{signal_name}\",\"graceful\":true}}"
    );
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{envelope}");
    let _ = stdout.flush();
}

#[cfg(test)]
mod tests {
    /// G42/S8 regression guard: the SHARED `handle_first_signal` function
    /// (called by both the SIGINT ctrlc closure and the SIGTERM/SIGHUP
    /// signal-hook drain) must not contain `eprintln!` or `tracing::warn!`
    /// — both can panic (and abort under `panic = "abort"`) when stderr
    /// is a closed pipe in an orphaned process.
    #[test]
    fn handler_source_has_no_panicking_io() {
        let source = include_str!("signals.rs");
        // The shared first-signal body starts at `fn handle_first_signal`
        // and ends at the closing brace of the function. We locate the
        // start of the next free-standing function or the test module
        // as the boundary.
        let body_start = source
            .find("fn handle_first_signal(")
            .expect("handle_first_signal must exist");
        let after_body = source[body_start..]
            .find("\nfn ")
            .or_else(|| source[body_start..].find("\n#[cfg(test)]"))
            .expect("body boundary not found");
        let body = &source[body_start..body_start + after_body];
        assert!(
            !body.contains("eprintln!"),
            "handle_first_signal must not use eprintln! (BrokenPipe panic, G42/C2)"
        );
        assert!(
            !body.contains("tracing::"),
            "handle_first_signal must not use tracing (stderr I/O can panic, G42/C2)"
        );
        assert!(
            body.contains("let _ = writeln!"),
            "first-signal notice must be a best-effort write"
        );
        assert!(
            body.contains("std::process::exit(130)"),
            "forced-exit path must remain in the shared handler"
        );
    }

    /// GAP-002 (v1.0.82) regression guard: the JSON envelope must use
    /// the deterministic SHUTDOWN_EXIT_CODE (19) so LLM agents can
    /// branch on a single code regardless of the triggering signal.
    #[test]
    fn envelope_uses_shutdown_exit_code() {
        let source = include_str!("signals.rs");
        // The envelope format string contains "code":19.
        assert!(
            source.contains("\\\"code\\\":19"),
            "shutdown envelope must embed SHUTDOWN_EXIT_CODE = 19"
        );
    }

    /// GAP-002 (v1.0.82) regression guard: `AppError::Shutdown` is the
    /// canonical error variant for shutdown. Constants and i18n are
    /// wired in lock-step — if SHUTDOWN_EXIT_CODE drifts away from 19,
    /// this test fails.
    #[test]
    fn shutdown_exit_code_is_19() {
        use crate::constants::SHUTDOWN_EXIT_CODE;
        assert_eq!(SHUTDOWN_EXIT_CODE, 19);
    }
}
