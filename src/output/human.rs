//! Human-facing output on stderr, routed through `tracing`.
//!
//! Nothing here touches stdout. Keeping the two channels in separate modules
//! is what makes "diagnostics on stderr, payload on stdout" checkable by
//! reading a file name instead of auditing call sites.

/// Logs `msg` as a structured `tracing::info!` event (does not write to stdout).
/// v1.0.89: suppressed when stderr is not a terminal (pipe) to avoid
/// polluting JSON pipelines when the user redirects stderr with `2>&1`.
#[inline]
pub fn emit_progress(msg: &str) {
    if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        tracing::info!(target: "output", message = msg);
    }
}

/// Emits a bilingual progress message honouring `--lang` or XDG `i18n.lang`.
/// v1.0.89: suppressed when stderr is not a terminal (pipe).
pub fn emit_progress_i18n(en: &str, pt: &str) {
    if !std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        return;
    }
    use crate::i18n::{current, Language};
    match current() {
        Language::English => tracing::info!(target: "output", message = en),
        Language::Portuguese => tracing::info!(target: "output", message = pt),
    }
}

/// Emits a localised error message to stderr via the `tracing` subscriber.
///
/// ADR-0047 / BUG-12 v1.0.88: prior implementation also called `eprintln!`
/// which produced a SECOND stderr line (`Error:`/`Erro:` prefix) for the same
/// error, on top of the structured `tracing::error!` line. Operators and
/// log parsers observed duplicated stderr lines.
///
/// The tracing subscriber is configured for stderr at `main.rs:115`, so a
/// single `tracing::error!` call already produces the human-readable line.
/// Callers that want a plain stderr line without tracing (e.g. one-shot
/// scripts) should use `eprintln!` directly instead of this helper.
///
/// Centralises human-readable error output following Pattern 5
/// ([`crate::output`] is the SOLE I/O point of the CLI).
#[cold]
#[inline(never)]
pub fn emit_error(localized_msg: &str) {
    tracing::error!(target: "output", message = localized_msg);
}

/// Emits a bilingual error to stderr honouring `--lang` or XDG `i18n.lang`.
/// Usage: `output::emit_error_i18n("invariant violated", "invariante violado")`.
#[cold]
#[inline(never)]
pub fn emit_error_i18n(en: &str, pt: &str) {
    use crate::i18n::{current, Language};
    let msg = match current() {
        Language::English => en,
        Language::Portuguese => pt,
    };
    emit_error(msg);
}
