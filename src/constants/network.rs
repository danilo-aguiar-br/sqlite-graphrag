//! OpenRouter endpoints and probe budgets.
//!
//! Split out of the former single-file `constants.rs` in v1.2.5;
//! every item is re-exported by the parent module, so `crate::constants::X`
//! resolves exactly as before.

/// Default OpenRouter chat completions endpoint (override via XDG
/// `network.openrouter.chat_url` or alias `network.chat_url`).
pub const DEFAULT_OPENROUTER_CHAT_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Default OpenRouter embeddings endpoint (override via XDG
/// `network.openrouter.embeddings_url` or alias `network.embed_url`).
pub const DEFAULT_OPENROUTER_EMBEDDINGS_URL: &str = "https://openrouter.ai/api/v1/embeddings";

/// Fail-fast probe budget for LLM backends before spawning (ms).
/// Override via XDG `llm.probe_timeout_ms`.
pub const DEFAULT_LLM_PROBE_TIMEOUT_MS: u64 = 800;

/// Ceiling, in seconds, on a single `Retry-After` sleep inside the HTTP retry
/// loops of [`crate::chat_api`] and [`crate::embedding_api`].
///
/// The header is server-controlled and was honoured verbatim: a provider (or
/// anything answering in its place) replying `Retry-After: 86400` put a CLI
/// that is supposed to be born, run and die to sleep for a full day, with no
/// output and no way to tell the stall apart from a hang.
///
/// The value is anchored on the budget that already governs the TOTAL wait:
/// `enrich.rate_limit_deadline_secs` defaults to 3600s
/// ([`crate::constants::DEFAULT_RATE_LIMIT_DEADLINE_SECS`]). One step must stay
/// well under that or the deadline stops meaning anything — at 60s the worst
/// case of `openrouter_http::MAX_RETRIES` rate-limited attempts is
/// 240s, under 7% of the deadline, so the operator's budget still decides when
/// the run gives up. It is also far above any wait a healthy provider advises,
/// so the cap only fires on values that were never actionable anyway.
///
/// Distinct from [`crate::constants::ENRICH_BACKOFF_CEILING_SECS`] (900s),
/// which bounds the drain's own backoff BETWEEN items, not one HTTP attempt.
/// Coordination wait against a remote limit, so it takes no XDG key.
pub const MAX_RETRY_AFTER_SECS: u64 = 60;

/// Default per-item budget, in seconds, for an OpenRouter chat-completion when
/// `--openrouter-timeout` is omitted.
///
/// GAP-SG-17: raised from 300 to 600 because dense bodies (close to the ~32K
/// token context ceiling of the configured model) routinely take longer than
/// five minutes to generate via `deepseek-v4-flash:nitro`.
pub const DEFAULT_OPENROUTER_CHAT_TIMEOUT_SECS: u64 = 600;

/// Clamps a server-advised `Retry-After` to [`MAX_RETRY_AFTER_SECS`], warning
/// when the cap actually bites.
///
/// Lives beside the constant rather than in either transport because both HTTP
/// retry loops apply the same policy; a second copy of the expression is one
/// edit away from the chat and embedding paths disagreeing about how long a
/// remote limit may stall a one-shot process.
///
/// The warning is not decoration: a cap that trims in silence is
/// indistinguishable from a provider that asked for the shorter wait, so the
/// operator would read the retries as normal pacing instead of as a provider
/// demanding a delay this CLI refuses to grant.
pub fn clamp_retry_after_secs(requested: u64) -> u64 {
    let applied = requested.min(MAX_RETRY_AFTER_SECS);
    if applied < requested {
        tracing::warn!(
            requested_secs = requested,
            applied_secs = applied,
            "Retry-After exceeds the local ceiling and was capped; \
             a one-shot process must not sleep on a server's word alone"
        );
    }
    applied
}

#[cfg(test)]
mod retry_after_ceiling_tests {
    use super::{clamp_retry_after_secs, MAX_RETRY_AFTER_SECS};

    #[test]
    fn an_absurd_retry_after_is_capped() {
        // 86400 is the measured shape of the defect: one header value put the
        // process to sleep for a day inside a born-run-die CLI.
        assert_eq!(clamp_retry_after_secs(86_400), MAX_RETRY_AFTER_SECS);
        assert_eq!(
            clamp_retry_after_secs(u64::MAX),
            MAX_RETRY_AFTER_SECS,
            "the cap must hold for any value the header can carry"
        );
    }

    #[test]
    fn a_reasonable_retry_after_passes_through_untouched() {
        for requested in [0, 1, 2, 30, MAX_RETRY_AFTER_SECS] {
            assert_eq!(
                clamp_retry_after_secs(requested),
                requested,
                "a wait a healthy provider advises must be honoured verbatim"
            );
        }
    }
}
