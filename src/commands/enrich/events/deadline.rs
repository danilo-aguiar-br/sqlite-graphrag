//! Wall-clock deadline around the candidate scan.
//!
//! v1.1.06 (GAP-ENTITY-CONNECT-SCAN-CARTESIAN): wraps `scan_operation` in a
//! watchdog that interrupts the SQLite connection at the deadline, so a runaway
//! scan fails as a timeout instead of pinning the enrich singleton.

use super::super::args::EnrichArgs;
use super::super::scan::scan_operation;
use crate::errors::AppError;
use rusqlite::Connection;
use std::time::Instant;

/// Main entry point for the `enrich` command.
/// Run [`scan_operation`] with an optional wall-clock deadline enforced via
/// [`rusqlite::Connection::get_interrupt_handle`].
///
/// v1.1.06 (GAP-ENTITY-CONNECT-SCAN-CARTESIAN): the first enrich scan used to
/// run with no timeout; a cartesian SQL could pin the process (and the enrich
/// singleton) indefinitely. When `deadline` is `Some`, a watchdog thread calls
/// `interrupt()` at the deadline so the scan fails as
/// [`AppError::Timeout`] (exit 1) — never as exit 75.
pub(crate) fn scan_operation_with_deadline(
    conn: &Connection,
    namespace: &str,
    args: &EnrichArgs,
    deadline: Option<Instant>,
) -> Result<Vec<String>, AppError> {
    let Some(deadline) = deadline else {
        return scan_operation(conn, namespace, args);
    };

    if Instant::now() >= deadline {
        return Err(AppError::Timeout {
            operation: format!("enrich {:?} scan", args.operation()),
            duration_secs: 0,
        });
    }

    let handle = conn.get_interrupt_handle();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_w = std::sync::Arc::clone(&stop);
    let watchdog = std::thread::spawn(move || {
        while !stop_w.load(std::sync::atomic::Ordering::Relaxed) {
            if Instant::now() >= deadline {
                handle.interrupt();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(
                crate::constants::ENRICH_SCAN_WATCHDOG_POLL_MS,
            ));
        }
    });

    let scan_t0 = Instant::now();
    let result = scan_operation(conn, namespace, args);
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = watchdog.join();

    match result {
        Ok(v) => Ok(v),
        Err(AppError::Database(ref e)) if is_sqlite_interrupt(e) => Err(AppError::Timeout {
            operation: format!("enrich {:?} scan", args.operation()),
            duration_secs: scan_t0.elapsed().as_secs().max(1),
        }),
        Err(e) => Err(e),
    }
}

pub(crate) fn is_sqlite_interrupt(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(code, _) => {
            code.code == rusqlite::ErrorCode::OperationInterrupted || code.extended_code == 9
            // SQLITE_INTERRUPT
        }
        other => {
            let s = other.to_string().to_ascii_lowercase();
            s.contains("interrupt") || s.contains("cancelled")
        }
    }
}
