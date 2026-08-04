//! Client construction and the two embedding entry points.
//!
//! Owns the three constructors and the `embed_single` / `embed_batch` calls;
//! the retry loop underneath lives in [`super::transport`].

use super::error::EmbedError;
use super::mrl::{model_default_input_type, mrl_wire_dimensions};
use super::wire::{EmbeddingInput, EmbeddingRequest};
use super::{
    OpenRouterClient, DEFAULT_CONNECT_TIMEOUT_SECS, DEFAULT_EMBED_HTTP_BATCH_SIZE,
    DEFAULT_TIMEOUT_SECS,
};
use crate::constants::DEFAULT_OPENROUTER_EMBEDDINGS_URL;
use crate::errors::AppError;
use secrecy::SecretBox;
use std::time::Duration;

impl OpenRouterClient {
    /// Builds an embedding client bound to `model`, applying `timeout_secs` as
    /// the total per-request budget.
    ///
    /// A value of `0` falls back to `DEFAULT_TIMEOUT_SECS`, mirroring
    /// [`crate::chat_api::OpenRouterChatClient::new`], so a missing or zero
    /// setting never degrades into reqwest's immediate-timeout behaviour.
    pub fn new(
        api_key: SecretBox<String>,
        model: String,
        dim: usize,
        timeout_secs: u64,
    ) -> Result<Self, AppError> {
        let base_url =
            crate::runtime_config::openrouter_embeddings_url(DEFAULT_OPENROUTER_EMBEDDINGS_URL);
        Self::new_with_base_url(api_key, model, dim, timeout_secs, base_url)
    }

    /// Build a client posting to an explicit `base_url` (XDG override, tests, gateways).
    ///
    /// `timeout_secs` follows the same zero-guard as [`Self::new`].
    pub fn new_with_base_url(
        api_key: SecretBox<String>,
        model: String,
        dim: usize,
        timeout_secs: u64,
        base_url: String,
    ) -> Result<Self, AppError> {
        let timeout_secs = if timeout_secs == 0 {
            DEFAULT_TIMEOUT_SECS
        } else {
            timeout_secs
        };
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .connect_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS))
            .user_agent(concat!("sqlite-graphrag/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| {
                AppError::Embedding(crate::i18n::validation::embedding_http_client_build_failed(
                    e,
                ))
            })?;

        let default_input_type = model_default_input_type(&model);

        Ok(Self {
            client,
            api_key,
            model,
            dim,
            default_input_type,
            base_url,
        })
    }

    /// Test-only constructor that POSTs to an arbitrary `base_url` (such as a
    /// `wiremock::MockServer`) instead of the public OpenRouter endpoint.
    /// Behaviour is otherwise identical to [`Self::new`].
    #[cfg(test)]
    pub(super) fn new_with_url(
        api_key: SecretBox<String>,
        model: String,
        dim: usize,
        timeout_secs: u64,
        base_url: String,
    ) -> Result<Self, AppError> {
        Self::new_with_base_url(api_key, model, dim, timeout_secs, base_url)
    }

    /// Default input type.
    pub fn default_input_type(&self) -> Option<&'static str> {
        self.default_input_type
    }

    /// Embed single.
    pub async fn embed_single(
        &self,
        text: &str,
        input_type: Option<&str>,
    ) -> Result<Vec<f32>, EmbedError> {
        // GAP-SG-02: reject an input that would overflow the model's token
        // window BEFORE the HTTP request, surfacing a clear Validation error
        // instead of a provider context-length rejection paid for round-trip.
        crate::memory_guard::check_embedding_input_size(text)?;

        let request = EmbeddingRequest {
            model: &self.model,
            input: EmbeddingInput::Single(text),
            dimensions: mrl_wire_dimensions(&self.model, self.dim),
            encoding_format: "float",
            input_type,
        };

        let response = self.execute_with_retry(&request).await?;

        let embedding = response
            .data
            .into_iter()
            .next()
            .ok_or_else(|| {
                AppError::Embedding(
                    crate::i18n::validation::embedding_empty_response_from_openrouter(),
                )
            })?
            .embedding;

        Ok(self.truncate_embedding(embedding)?)
    }

    /// Embed batch.
    pub async fn embed_batch(
        &self,
        texts: &[&str],
        input_type: Option<&str>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // GAP-SG-02: validate every input before any HTTP request so an
        // oversized member of the batch fails fast as Validation rather than a
        // provider context-length rejection mid-batch.
        for text in texts {
            crate::memory_guard::check_embedding_input_size(text)?;
        }

        let mut all = Vec::with_capacity(texts.len());

        let batch_size = crate::runtime_config::embedding_batch_size(DEFAULT_EMBED_HTTP_BATCH_SIZE);
        for chunk in texts.chunks(batch_size) {
            let request = EmbeddingRequest {
                model: &self.model,
                input: EmbeddingInput::Batch(chunk.to_vec()),
                dimensions: mrl_wire_dimensions(&self.model, self.dim),
                encoding_format: "float",
                input_type,
            };

            let response = self.execute_with_retry(&request).await?;

            if response.data.len() != chunk.len() {
                return Err(AppError::Embedding(
                    crate::i18n::validation::embedding_expected_count(
                        chunk.len(),
                        response.data.len(),
                    ),
                )
                .into());
            }

            let mut sorted = response.data;
            sorted.sort_by_key(|d| d.index);

            for d in sorted {
                all.push(self.truncate_embedding(d.embedding)?);
            }
        }

        Ok(all)
    }
}
