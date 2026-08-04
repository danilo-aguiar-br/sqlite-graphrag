//! G29 regression test: ensures `terminal::init_console` is callable and the
//! windows-sys 0.59+ `HANDLE` type is used correctly.
//!
//! The test is compiled on ALL platforms but only exercises the Windows path
//! under `cfg(windows)`.  On non-Windows, it is a no-op that confirms the
//! function is reachable from outside the crate (public re-export check).

#![cfg_attr(not(windows), allow(dead_code))]

use sqlite_graphrag::terminal::{init_console, should_use_ansi};

/// `init_console` must be callable from any platform without panicking.
/// On non-Windows this is a no-op (UTF-8 + ANSI already supported natively);
/// on Windows it routes to `init_windows_console` which uses
/// `windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE}`.
#[test]
fn init_console_is_callable_on_current_platform() {
    init_console();
}

/// `should_use_ansi` honours `NO_COLOR` and `CLICOLOR_FORCE` env vars.
///
/// We snapshot the current `NO_COLOR` value to restore after the test,
/// because the function reads it eagerly and our test must not pollute the
/// environment for downstream tests running in the same process.
#[test]
fn should_use_ansi_respects_no_color_env() {
    let original = std::env::var_os("NO_COLOR");
    // SAFETY: tests are single-threaded with respect to env mutation here;
    // we restore the original value before returning.
    unsafe {
        std::env::set_var("NO_COLOR", "1");
    }
    assert!(
        !should_use_ansi(),
        "NO_COLOR=1 must force should_use_ansi() == false"
    );
    match original {
        Some(v) => unsafe { std::env::set_var("NO_COLOR", v) },
        None => unsafe { std::env::remove_var("NO_COLOR") },
    }
}

/// On Windows, the `HANDLE` constant from `windows-sys 0.59+` is a
/// `*mut c_void` (not `isize` as in 0.48/0.52).  The fix in `terminal.rs`
/// imports it via `use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE}`
/// and uses `.is_null()` + `!= INVALID_HANDLE_VALUE` for type-safe comparison.
///
/// This test simply references the function to make sure the build is wired
/// up; if the type check regresses, `cargo check --target x86_64-pc-windows-msvc`
/// in CI will fail before this test is even reached.
#[cfg(windows)]
#[test]
fn windows_console_init_uses_type_safe_handle_check() {
    use sqlite_graphrag::terminal::init_console;
    init_console();
}

// ---------------------------------------------------------------------------
// Cooperative shutdown must exist on BOTH platforms, not only on Unix
// ---------------------------------------------------------------------------

/// The signal module's own source, so the assertions below inspect the code
/// that ships rather than a description of it.
///
/// This suite runs on Linux, where no Windows console control event can be
/// delivered, so the Windows half is proven two ways: by `cargo check
/// --target x86_64-pc-windows-gnu` compiling the `cfg(windows)` body, and by
/// the source-level assertions here that the body registers the right events
/// and routes them into the SHARED first-signal handler.
const SIGNALS_SRC: &str = include_str!("../src/signals.rs");

/// `main`'s source, for the exit-141 half of the same contract.
const MAIN_SRC: &str = include_str!("../src/main.rs");

/// The registration entry point must exist for whatever platform compiled this
/// test — a compile-time proof that no `cfg` combination leaves the CLI with no
/// shutdown path at all.
#[test]
fn register_shutdown_handler_exists_on_the_compiled_platform() {
    let handler: fn() = sqlite_graphrag::signals::register_shutdown_handler;
    assert_ne!(
        handler as usize, 0,
        "the platform-selected registration body must be a real function"
    );
}

/// Windows has no SIGTERM and no SIGHUP; `SetConsoleCtrlHandler` is the only
/// cooperative termination mechanism it offers, and all five control events
/// must be covered — close, logoff and shutdown are the ones a service or a
/// user closing the window actually sends.
#[test]
fn windows_registers_a_console_control_handler_for_every_event() {
    assert!(
        SIGNALS_SRC.contains("fn register_console_ctrl_handler()"),
        "the Windows registration path must exist"
    );
    assert!(
        SIGNALS_SRC.contains("SetConsoleCtrlHandler"),
        "Windows shutdown must go through SetConsoleCtrlHandler"
    );
    for event in [
        "CTRL_C_EVENT",
        "CTRL_BREAK_EVENT",
        "CTRL_CLOSE_EVENT",
        "CTRL_LOGOFF_EVENT",
        "CTRL_SHUTDOWN_EVENT",
    ] {
        assert!(
            SIGNALS_SRC.contains(event),
            "console control event {event} is not handled"
        );
    }
}

/// Both platforms must converge on `handle_first_signal`, otherwise the flag,
/// the cancellation token and the `code: 19` envelope would differ per OS.
#[test]
fn both_platforms_route_into_the_shared_first_signal_handler() {
    let windows_body = SIGNALS_SRC
        .split("fn register_console_ctrl_handler()")
        .nth(1)
        .expect("the Windows registration body must exist")
        .split("\nfn ")
        .next()
        .expect("the body ends at the next free-standing function");
    assert!(
        windows_body.contains("handle_first_signal(name, number)"),
        "the Windows handler must call the SHARED first-signal body"
    );

    let unix_gate = SIGNALS_SRC
        .split("#[cfg(unix)]")
        .nth(1)
        .expect("the Unix registration body must exist");
    assert!(
        unix_gate.contains("signal_hook::consts::SIGTERM")
            && unix_gate.contains("signal_hook::consts::SIGHUP"),
        "the Unix path must still cover SIGTERM and SIGHUP"
    );
}

/// Registering `ctrlc` AND a console control handler would deliver one Ctrl+C
/// twice, and the second-event rule turns that into an immediate exit 130.
/// Windows therefore owns the console alone.
#[test]
fn windows_does_not_also_register_the_ctrlc_crate() {
    assert!(
        SIGNALS_SRC.contains("#[cfg(not(windows))]"),
        "ctrlc registration must be gated away from Windows so a single \
         Ctrl+C cannot be counted twice"
    );
}

/// Windows has no SIGPIPE, so the exit-141 contract cannot come from a signal
/// there; `main` classifies the stdout write error instead. The module doc must
/// say so, and the code must actually do it.
#[test]
fn broken_pipe_reaches_exit_141_without_sigpipe() {
    assert_eq!(
        sqlite_graphrag::constants::BROKEN_PIPE_EXIT_CODE,
        141,
        "141 is 128 + SIGPIPE(13), the shell-visible convention"
    );
    assert!(
        MAIN_SRC.contains("ErrorKind::BrokenPipe"),
        "main must classify a closed stdout pipe explicitly"
    );
    assert!(
        MAIN_SRC.contains("BROKEN_PIPE_EXIT_CODE"),
        "the 141 exit must come from the named constant, not a literal"
    );
    assert!(
        SIGNALS_SRC.contains("no `SIGPIPE`"),
        "the module doc must state that Windows has no SIGPIPE"
    );
}

/// The module doc used to promise "Cross-platform signal handling: SIGINT,
/// SIGTERM, SIGHUP" while `cfg(unix)` gated two of the three away. A doc that
/// over-promises is the defect this suite closes, so it is asserted directly.
#[test]
fn module_doc_states_the_per_platform_event_set() {
    let header: String = SIGNALS_SRC
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !header.contains("Cross-platform signal handling: SIGINT, SIGTERM, SIGHUP"),
        "the old blanket claim must not come back: Windows delivers none of the three"
    );
    assert!(
        header.contains("Windows") && header.contains("Unix"),
        "the doc must name what each platform actually delivers"
    );
    assert!(
        header.contains("SetConsoleCtrlHandler"),
        "the doc must name the Windows mechanism"
    );
}
