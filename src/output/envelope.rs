//! Complete JSON envelopes — and the single point where the agent-native
//! surface is applied.
//!
//! GAP-SG-142 put the reshaping here rather than in each subcommand precisely
//! because there is exactly one function pair every payload passes through.
//! One implementation covers the whole CLI and no command has to know the
//! surface exists.
//!
//! The `active()` check keeps the fast path intact: with no shaping flag set,
//! the value is serialized straight from its own `Serialize` impl and the
//! envelope is byte-for-byte what it was before the surface existed.

use super::sink;
use crate::errors::AppError;
use serde::Serialize;

/// Serializes `value` and returns its JSON text, applying the agent-native
/// surface when one is installed.
///
/// `pretty` selects indented output; compact is a single line.
fn render<T: Serialize>(value: &T, pretty: bool) -> Result<String, AppError> {
    // Two reasons to enter the layer, not one. A knob means the envelope is
    // reshaped; a resolved target means it must be annotated even though no
    // knob was set. GAP-SG-205: gating solely on `active()` is what hid the
    // target on the default path, since a caller that sets no flag is exactly
    // the caller the Explicit Target Designation rule is written for.
    if crate::agent_surface::active() || crate::agent_surface::target::is_reportable() {
        let shaped = crate::agent_surface::apply_global(serde_json::to_value(value)?)?;
        return Ok(if pretty {
            serde_json::to_string_pretty(&shaped)?
        } else {
            serde_json::to_string(&shaped)?
        });
    }
    Ok(if pretty {
        serde_json::to_string_pretty(value)?
    } else {
        serde_json::to_string(value)?
    })
}

/// Whether the envelope should be indented.
///
/// Indentation is for a human reading a terminal. Off a terminal it is bytes a
/// consumer pays for and never reads, so the envelope goes out compact.
///
/// GAP-SG-170: this also keeps `--max-output-bytes` honest. `budget::enforce`
/// measures with `serde_json::to_string`, the compact form, while emission used
/// `to_string_pretty` unconditionally. The ceiling was therefore enforced on one
/// serialization and violated on another — measured at 8 659 bytes emitted for a
/// declared cap of 8 000, with the overshoot growing alongside the cap because
/// indentation scales with content. With the surface active the answer is never
/// indented, so the byte the budget counts is the byte the caller receives.
fn should_indent() -> bool {
    use std::io::IsTerminal;
    !crate::agent_surface::active() && std::io::stdout().is_terminal()
}

/// Serializes `value` as JSON and writes it to stdout with a trailing newline.
///
/// Indented on a terminal, compact everywhere else; see `should_indent`.
///
/// Flushes stdout after writing. A `BrokenPipe` error is silenced so that
/// piping to consumers that close early (e.g. `head`) does not surface an error.
///
/// # Errors
/// Returns `Err` when serialization fails or when a non-`BrokenPipe` I/O error occurs.
#[inline]
pub fn emit_json<T: Serialize>(value: &T) -> Result<(), AppError> {
    sink::write_line(render(value, should_indent())?.as_bytes())
}

/// Serializes `value` as compact (single-line) JSON and writes it to stdout with a trailing newline.
///
/// Flushes stdout after writing. A `BrokenPipe` error is silenced.
///
/// # Errors
/// Returns `Err` when serialization fails or when a non-`BrokenPipe` I/O error occurs.
#[inline]
pub fn emit_json_compact<T: Serialize>(value: &T) -> Result<(), AppError> {
    sink::write_line(render(value, false)?.as_bytes())
}
