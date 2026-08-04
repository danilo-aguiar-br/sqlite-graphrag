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

/// Default per-item budget, in seconds, for an OpenRouter chat-completion when
/// `--openrouter-timeout` is omitted.
///
/// GAP-SG-17: raised from 300 to 600 because dense bodies (close to the ~32K
/// token context ceiling of the configured model) routinely take longer than
/// five minutes to generate via `deepseek-v4-flash:nitro`.
pub const DEFAULT_OPENROUTER_CHAT_TIMEOUT_SECS: u64 = 600;
