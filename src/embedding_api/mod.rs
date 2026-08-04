//! HTTP client for the OpenRouter embeddings API.
//!
//! Sends embedding requests to the OpenAI-compatible endpoint at
//! `openrouter.ai/api/v1/embeddings` and returns dense `Vec<f32>`
//! vectors. Handles retry with exponential backoff + jitter for
//! transient failures (429, 5xx) and immediate abort for permanent
//! errors (401, 400).

use secrecy::SecretBox;

// Default lives in constants; production clients resolve via runtime_config.

const DEFAULT_TIMEOUT_SECS: u64 = crate::constants::DEFAULT_EMBEDDING_HTTP_TIMEOUT_SECS;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
// Factory default for OpenRouter embed batching; runtime uses XDG
// `embedding.batch_size` via [`crate::runtime_config::embedding_batch_size`].
const DEFAULT_EMBED_HTTP_BATCH_SIZE: usize = crate::constants::FASTEMBED_BATCH_SIZE;

mod client;
mod error;
mod mrl;
#[cfg(test)]
mod tests;
mod transport;
mod wire;

// GAP-SG-146: split by responsibility. Every public item is re-exported here,
// so `crate::embedding_api::OpenRouterClient` and `EmbedError` keep resolving
// exactly as before for every caller inside and outside this module.
pub use error::EmbedError;

/// Open router client.
pub struct OpenRouterClient {
    client: reqwest::Client,
    api_key: SecretBox<String>,
    model: String,
    dim: usize,
    default_input_type: Option<&'static str>,
    /// Endpoint each request is POSTed to. Resolved from XDG/config at
    /// construction (default: [`DEFAULT_OPENROUTER_EMBEDDINGS_URL`]).
    base_url: String,
}
