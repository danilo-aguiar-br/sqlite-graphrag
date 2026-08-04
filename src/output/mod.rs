//! Single point of terminal I/O for the CLI (stdout JSON, stderr human).
//!
//! All user-visible output must go through this module; direct `println!` in
//! other modules is forbidden.
//!
//! Split by responsibility (GAP-SG-146), roughly in order of how structured
//! the output is:
//!
//! * [`envelope`] — complete JSON payloads, and the single point where the
//!   agent-native reshaping surface is applied
//! * [`error_envelope`] — failure payloads, with a serializer-free fallback
//! * [`stream`] — NDJSON, one record per line, deliberately unshaped
//! * [`passthrough`] — plain text and verbatim bytes, no JSON at all
//! * [`human`] — stderr diagnostics via `tracing`
//! * [`sink`] — the one place bytes actually reach stdout
//! * [`format`] — `--format` value enums
//! * [`responses`] — the serializable payload types themselves
//!
//! Every public item is re-exported here, so `crate::output::emit_json` and
//! friends keep working unchanged; callers never name a submodule.

mod envelope;
mod error_envelope;
mod format;
mod human;
mod passthrough;
mod responses;
mod sink;
mod stream;

#[cfg(test)]
mod tests;

pub use envelope::{emit_json, emit_json_compact};
pub use error_envelope::{emit_error_json, emit_error_json_with_suggestion};
pub use format::{JsonOutputFormat, OutputFormat};
pub use human::{emit_error, emit_error_i18n, emit_progress, emit_progress_i18n};
pub use passthrough::{emit_raw, emit_text};
pub use responses::{RecallItem, RecallResponse, RememberResponse};
pub use stream::emit_json_line;
