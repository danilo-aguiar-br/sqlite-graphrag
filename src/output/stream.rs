//! NDJSON streaming: one self-contained JSON record per line.

use super::sink;
use serde::Serialize;

/// Writes compact JSON to stdout, silently ignoring serialization and I/O errors.
/// Designed for NDJSON streaming where partial output is acceptable.
///
/// GAP-SG-142: NDJSON deliberately bypasses [`crate::agent_surface`]. The
/// stream contract is one record per line and the shaping surface is defined
/// over a complete envelope; filtering or capping a stream line by line would
/// change what "one record" means for every consumer already parsing it.
///
/// The exclusion is a decision, not an oversight — do not "fix" it wholesale.
/// The flags split into two groups:
///
/// * **Set-wide, never applicable here:** `--max-items`, `--sort` and
///   `--dedupe-by` need the complete result set before they can decide what to
///   emit. A stream has no complete set by construction.
/// * **Per-line, safe to add later:** `--select` and `--truncate-content` are
///   stateless per record, and `--max-output-bytes` would be genuinely
///   valuable as a running budget across the stream. Wiring those three here
///   is a deliberate future extension; it needs its own contract decision
///   about what a consumer sees when the running budget is exhausted
///   mid-stream (truncated line? terminator record? silent stop?).
#[inline]
pub fn emit_json_line<T: Serialize>(value: &T) {
    if let Ok(json) = serde_json::to_string(value) {
        sink::write_line_lossy(json.as_bytes());
    }
}
