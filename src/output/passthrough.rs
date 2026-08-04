//! Unstructured stdout writers: plain text lines and verbatim bytes.
//!
//! Neither goes through [`crate::agent_surface`], and neither can — there is
//! no envelope to reshape. `emit_raw` exists precisely so a memory body leaves
//! the process byte for byte, which is the opposite of reshaping.

use super::sink;

/// Writes `msg` followed by a newline to stdout and flushes.
///
/// A `BrokenPipe` error is silenced gracefully.
#[inline]
pub fn emit_text(msg: &str) {
    sink::write_line_lossy(msg.as_bytes());
}

/// GAP-SG-50: writes `bytes` to stdout verbatim, with no trailing newline and
/// no JSON envelope. Used by `read --format raw` so the pure memory body can be
/// piped without a `jaq -r '.body'` round-trip. A `BrokenPipe` error is
/// silenced gracefully.
#[inline]
pub fn emit_raw(bytes: &[u8]) {
    sink::write_verbatim(bytes);
}
