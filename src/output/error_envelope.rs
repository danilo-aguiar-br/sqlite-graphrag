//! Failure envelopes on stdout, with a hand-rolled fallback.
//!
//! The stdout JSON contract holds on error paths too: a machine consumer must
//! be able to parse the failure, not just read a stderr line. That is why each
//! emitter here has a second path built with `writeln!` — if `serde_json`
//! itself fails, the caller still receives a parseable envelope rather than
//! empty stdout.
//!
//! These envelopes carry `error: true`, which [`crate::agent_surface`] treats
//! as pass-through. A `--filter` can never suppress a failure.

use super::envelope::emit_json;

/// Escapes a string for inclusion in a hand-built JSON string literal.
///
/// Only the two characters that can break out of a quoted literal are
/// handled, which is exactly what the fallback paths below need: the inputs
/// are localized messages and suggestions, never arbitrary binary. A control
/// character would produce technically invalid JSON, but the alternative —
/// pulling in a serializer on the path taken *because* serialization failed —
/// defeats the point of having a fallback at all.
fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The one failure envelope every emitter below renders.
///
/// `error_class` and `retryable` carry no `skip_serializing_if`: an agent needs
/// the retry verdict on EVERY failure, and a field that vanishes when it is
/// `false` forces the reader to distinguish "not retryable" from "this build
/// does not report it". Only `suggestion` is optional, because a variant whose
/// message is already self-remediating has nothing to add.
#[derive(serde::Serialize)]
struct ErrorEnvelope<'a> {
    error: bool,
    code: i32,
    message: &'a str,
    error_class: &'a str,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<&'a str>,
    /// Flags the caller passed that the invocation could not apply.
    ///
    /// Omitted when empty, because most failures discard nothing and an always
    /// present empty array would invite a reader to treat `[]` as meaningful.
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    discarded_flags: &'a [String],
    /// GAP-SG-205: the database this process resolved, and which layer named it.
    ///
    /// Carried by the struct rather than attached by [`crate::agent_surface`],
    /// which treats a failure envelope as pass-through — deliberately, so a
    /// `--filter` can never suppress an error. Two consequences make this the
    /// right home: the invariant "a failure reaches the caller verbatim" stays
    /// literally true, and the hand-rolled fallback below still emits the target
    /// on the one path that exists BECAUSE serialization failed. Knowing which
    /// database a failed write was aimed at matters most exactly there.
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_surface: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Emits the failure envelope, with a hand-rolled fallback.
///
/// Single rendering point: the fallback path used to exist twice, once per
/// public emitter, so a field added to one envelope silently skipped the other.
///
/// Shape: `{"error": true, "code": <exit>, "message": "...", "error_class":
/// "transient|permanent|ambiguous", "retryable": <bool>, "suggestion": "..."}`.
/// A `BrokenPipe` is silenced so piping to an early-closing consumer does not
/// surface a secondary error.
#[cold]
#[inline(never)]
pub fn emit_error_envelope(
    code: i32,
    message: &str,
    error_class: &str,
    retryable: bool,
    suggestion: Option<&str>,
    discarded_flags: &[String],
) {
    let target = crate::agent_surface::target::record(crate::agent_surface::get());
    let envelope = ErrorEnvelope {
        error: true,
        code,
        message,
        error_class,
        retryable,
        suggestion,
        discarded_flags,
        agent_surface: target.clone(),
    };
    if emit_json(&envelope).is_err() {
        use std::io::Write;
        let escaped = escape(message);
        let esc_class = escape(error_class);
        let mut line = format!(
            r#"{{"error":true,"code":{code},"message":"{escaped}","error_class":"{esc_class}","retryable":{retryable}"#
        );
        if let Some(s) = suggestion {
            line.push_str(&format!(r#","suggestion":"{}""#, escape(s)));
        }
        if !discarded_flags.is_empty() {
            let list = discarded_flags
                .iter()
                .map(|f| format!(r#""{}""#, escape(f)))
                .collect::<Vec<_>>()
                .join(",");
            line.push_str(&format!(r#","discarded_flags":[{list}]"#));
        }
        // Rendered by hand for the same reason as everything else on this path:
        // the branch exists because `serde_json` already failed once, so leaning
        // on it again to serialize the target would drop exactly the field that
        // says where a failed write was pointed.
        if let Some(map) = &target {
            let members = map
                .iter()
                .map(|(k, v)| {
                    let text = v.as_str().unwrap_or_default();
                    format!(r#""{}":"{}""#, escape(k), escape(text))
                })
                .collect::<Vec<_>>()
                .join(",");
            line.push_str(&format!(r#","agent_surface":{{{members}}}"#));
        }
        line.push('}');
        let _ = writeln!(std::io::stdout().lock(), "{line}");
    }
}

/// Emits a configuration failure, which is permanent by construction.
///
/// Bootstrap failures — a missing provider key, an unreadable XDG file, a model
/// the catalogue rejects — cannot be fixed by trying again, so they are always
/// `permanent` / `retryable: false`. Callers holding a real [`AppError`] must
/// use [`emit_error_json_with_suggestion`] instead, which reads the verdict off
/// the variant.
///
/// [`AppError`]: crate::errors::AppError
#[cold]
#[inline(never)]
pub fn emit_error_json(code: i32, message: &str) {
    emit_error_envelope(code, message, "permanent", false, None, &[]);
}

/// GAP-SG-39: emits the actionable failure envelope for a classified error.
///
/// The `suggestion` tells the operator HOW to recover instead of leaving an exit
/// code without guidance, and `error_class` / `retryable` tell an agent whether
/// recovering is even possible — which is what makes a write rejection
/// observable, fixable, and safe to automate.
#[cold]
#[inline(never)]
pub fn emit_error_json_with_suggestion(
    code: i32,
    message: &str,
    error_class: &str,
    retryable: bool,
    suggestion: Option<&str>,
    discarded_flags: &[String],
) {
    emit_error_envelope(
        code,
        message,
        error_class,
        retryable,
        suggestion,
        discarded_flags,
    );
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn escape_leaves_plain_text_untouched() {
        assert_eq!(escape("database is malformed"), "database is malformed");
    }

    #[test]
    fn escape_protects_quotes_and_backslashes() {
        assert_eq!(escape(r#"say "hi""#), r#"say \"hi\""#);
        assert_eq!(escape(r"C:\path"), r"C:\\path");
    }

    #[test]
    fn escape_orders_backslash_before_quote() {
        // Escaping the quote first would then double the backslash it just
        // introduced, producing `\\"` and breaking out of the literal.
        assert_eq!(escape(r#"\""#), r#"\\\""#);
    }
}
