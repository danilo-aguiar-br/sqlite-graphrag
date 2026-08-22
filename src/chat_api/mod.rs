//! HTTP client for the OpenRouter chat-completions API.
//!
//! Sends structured-output chat requests to the OpenAI-compatible endpoint
//! at `openrouter.ai/api/v1/chat/completions` and returns the parsed JSON
//! object the model produced under a strict `json_schema` `response_format`.
//!
//! This mirrors [`crate::embedding_api`] for the embeddings endpoint: same
//! retry/backoff policy (immediate abort on 401/400/404, `retry-after` on
//! 429, exponential backoff + jitter on 5xx) and the same minimal headers
//! (only `Authorization: Bearer`, no `HTTP-Referer`/`X-Title`). The shared
//! error envelope and backoff helper live in [`crate::openrouter_http`]
//! (GAP-SG-74). The submodule layout mirrors it too: `wire` holds the serde
//! shapes, `error` the failure type, `transport` the retry loop, `client`
//! the call surface and `completion` the response finalisation.
//!
//! v1.0.95 (ADR-0054): adds an OpenRouter REST transport for the `enrich`
//! JUDGE so structured extraction no longer requires a locally installed
//! `claude` / `codex` / `opencode` CLI subprocess.
//!
//! v1.1.00 (GAP-SG-70/72-chat): the OpenAI-compatible contract surfaces
//! `choices[].finish_reason` and `usage.{prompt_tokens,completion_tokens}`.
//! `finish_reason == "length"` means the response was truncated because
//! `max_tokens` was too small — not a malformed generation.
//! [`OpenRouterChatClient::complete`](crate::chat_api::OpenRouterChatClient::complete)
//! now detects this BEFORE attempting JSON repair, grows `max_tokens` and
//! re-issues the request (bounded by
//! [`crate::constants::ENRICH_MAX_LENGTH_RETRIES`]), and always reports the
//! diagnostics (`finish_reason`, token counts) to the caller via
//! [`ChatCompletion`](crate::chat_api::ChatCompletion) on success or
//! [`ChatError`](crate::chat_api::ChatError) on failure.

use secrecy::SecretBox;

// GAP-SG-17: raised from 300 to 600 — the per-request fallback budget when a
// caller passes `0`. Dense bodies near the model's ~32K-token context ceiling
// regularly need more than five minutes to generate.
const DEFAULT_TIMEOUT_SECS: u64 = 600;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;

/// Fixed `json_schema` name sent in the `response_format`. OpenRouter only
/// requires a short identifier; the actual contract is carried by `schema`.
const SCHEMA_NAME: &str = "enrich_output";

/// Sampling temperature for every request this client makes (G-PR-7).
///
/// All of them are extraction and classification over evidence the caller
/// already holds, so the useful output is the one the evidence determines.
/// Before this constant the field was absent from the request entirely and
/// each provider applied its own default, which for most is 1.0.
const EXTRACTION_TEMPERATURE: f64 = 0.0;

mod client;
mod completion;
mod error;
mod transport;
mod wire;

// Split by responsibility. Every public item is re-exported here, so
// `crate::chat_api::OpenRouterChatClient`, `ChatCompletion` and `ChatError`
// keep resolving exactly as before for every caller inside and outside this
// module.
pub use completion::ChatCompletion;
pub use error::ChatError;

/// Process-wide OpenRouter chat client. Holds the model name so that callers
/// only thread the per-item prompt/schema/input through [`Self::complete`].
pub struct OpenRouterChatClient {
    client: reqwest::Client,
    api_key: SecretBox<String>,
    model: String,
    /// Endpoint each request is POSTed to. Resolved from XDG/config at
    /// construction (default: [`DEFAULT_OPENROUTER_CHAT_URL`](crate::constants::DEFAULT_OPENROUTER_CHAT_URL)).
    base_url: String,
}

#[cfg(test)]
#[path = "../chat_api_tests.rs"]
mod tests;
