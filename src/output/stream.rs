//! NDJSON streaming: one self-contained JSON record per line.

use super::sink;
use crate::errors::AppError;
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
///   stateless per record.
///
/// GAP-SG-215 took the contract decision that second bullet asked for, and
/// [`emit_stream_record`] / [`emit_stream_trailer`] below are it. `--select` and
/// `--truncate-content` act per record; everything else is refused before the
/// first byte; the summary line carries the one `agent_surface` record for the
/// whole stream. `--max-output-bytes` as a RUNNING budget across the stream is
/// still open, and still needs its own decision about what a consumer sees when
/// the budget is exhausted mid-stream — truncated line, terminator record, or
/// silent stop. Today it is refused rather than half-applied.
///
/// This function keeps its unshaped, infallible shape for the emitters that
/// genuinely want neither: `enrich` progress events and `--print-schema`
/// listings, which are diagnostics rather than a record stream with a trailer.
#[inline]
pub fn emit_json_line<T: Serialize>(value: &T) {
    if let Ok(json) = serde_json::to_string(value) {
        sink::write_line_lossy(json.as_bytes());
    }
}

/// Writes one record of a stream, with the per-record knobs applied.
///
/// GAP-SG-215. Deliberately does NOT go through `super::envelope::render`: that
/// path exists to shape and annotate ONE complete envelope, and reaching it once
/// per line is the defect this replaces. A record line leaves here carrying the
/// record and nothing else.
///
/// The command must have called [`crate::agent_surface::stream::open`] first, so
/// that an unusable knob has already been refused. Emitting without opening
/// shapes nothing rather than misbehaving, and `tests/stream_contract_gate.rs`
/// is what keeps that from being a quiet way to lose the surface.
///
/// # Errors
/// Returns `Err` when serialization fails or on a non-`BrokenPipe` I/O error.
#[inline]
pub fn emit_stream_record<T: Serialize>(value: &T) -> Result<(), AppError> {
    let surface = crate::agent_surface::get();
    if surface.select.is_empty() && surface.truncate_content == 0 {
        // Nothing to apply, so the record never becomes a `Value` at all and the
        // bytes are exactly what the command's own `Serialize` impl produces.
        // This is the path an unflagged `export` takes for every one of its
        // lines, which is why it is worth keeping free of the round-trip.
        return sink::write_line(serde_json::to_string(value)?.as_bytes());
    }
    let state = crate::agent_surface::stream::get();
    let shaped = crate::agent_surface::stream::shape_record_with(
        state,
        surface,
        serde_json::to_value(value)?,
    );
    sink::write_line(serde_json::to_string(&shaped)?.as_bytes())
}

/// Writes the line that ends a stream, carrying its one `agent_surface` record.
///
/// GAP-SG-215. Never shaped: the trailer describes the stream, so a `--select`
/// aimed at the records must neither fail on it nor rewrite it. Both were
/// measured — `--select name export` exited 2 on this line, and
/// `--select namespace export` silently deleted `summary: true` from it.
///
/// # Errors
/// Returns `Err` when serialization fails or on a non-`BrokenPipe` I/O error.
#[inline]
pub fn emit_stream_trailer<T: Serialize>(value: &T) -> Result<(), AppError> {
    let surface = crate::agent_surface::get();
    let ceiling = crate::agent_surface::universe::get();
    let shaped = crate::agent_surface::stream::trailer_with(
        crate::agent_surface::stream::get(),
        surface,
        crate::agent_surface::target::record(surface, ceiling),
        serde_json::to_value(value)?,
    );
    sink::write_line(serde_json::to_string(&shaped)?.as_bytes())
}
