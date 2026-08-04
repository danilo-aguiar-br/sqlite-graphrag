//! The one place this crate writes to stdout.
//!
//! Every emitter above funnels through here, so the `BrokenPipe` policy is
//! stated once. That policy matters: an agent piping into `head` closes the
//! read end early, and treating the resulting error as a failure would turn a
//! normal shell idiom into a non-zero exit.
//!
//! Two error disciplines coexist on purpose. Envelope writers propagate real
//! I/O failures, because a caller that asked for a payload needs to know it
//! never arrived. Stream and passthrough writers swallow them, because partial
//! output is the documented contract there and a half-written stream has no
//! meaningful recovery.

use crate::errors::AppError;
use std::io::Write;

/// Writes `bytes` followed by a newline, then flushes.
///
/// # Errors
/// Returns [`AppError::Io`] for any I/O failure except `BrokenPipe`, which is
/// reported as success.
pub(super) fn write_line(bytes: &[u8]) -> Result<(), AppError> {
    let mut out = std::io::stdout().lock();
    if let Err(e) = out
        .write_all(bytes)
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush())
    {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            return Ok(());
        }
        return Err(AppError::Io(e));
    }
    Ok(())
}

/// Writes `bytes` followed by a newline, discarding every error.
///
/// For paths whose contract already accepts partial output.
pub(super) fn write_line_lossy(bytes: &[u8]) {
    let mut out = std::io::stdout().lock();
    let _ = out
        .write_all(bytes)
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush());
}

/// Writes `bytes` verbatim — no newline, no envelope — discarding every error.
pub(super) fn write_verbatim(bytes: &[u8]) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(bytes).and_then(|()| out.flush());
}
