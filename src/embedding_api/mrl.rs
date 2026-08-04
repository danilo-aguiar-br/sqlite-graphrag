//! Matryoshka (MRL) policy and dimension truncation.
//!
//! Decides whether a model accepts a `dimensions` field on the wire, what
//! default input type it expects, and how a longer native vector is cut down
//! to the configured dimensionality.

use super::OpenRouterClient;
use crate::errors::AppError;

pub(super) fn model_supports_mrl(model: &str) -> bool {
    model.contains("qwen3-embedding")
        || model.contains("text-embedding-3")
        || model.contains("gemini-embedding")
        || model.contains("llama-nemotron-embed")
        || model.contains("bge-m3")
}

/// Dimensions to put on the OpenRouter wire for MRL models.
///
/// Returns `None` when the provider should return the native full vector and
/// the client applies MRL prefix truncation to `dim` via [`OpenRouterClient::truncate_embedding`].
///
/// Qwen3 on OpenRouter rejects intermediate dims such as 384 with provider
/// code 20015 ("The parameter is invalid") while still serving the full 4096-d
/// vector without a `dimensions` field. Requesting native size + client
/// truncate preserves the configured project default (1024) without failing the request when intermediate dims are rejected.
pub(super) fn mrl_wire_dimensions(model: &str, dim: usize) -> Option<usize> {
    if !model_supports_mrl(model) {
        return None;
    }
    if model.contains("qwen3-embedding") {
        return None;
    }
    Some(dim)
}

pub(super) fn model_default_input_type(model: &str) -> Option<&'static str> {
    if model.contains("llama-nemotron-embed") {
        Some("passage")
    } else if model.contains("mistral-embed") {
        None
    } else {
        Some("search_document")
    }
}

impl OpenRouterClient {
    pub(super) fn truncate_embedding(&self, embedding: Vec<f32>) -> Result<Vec<f32>, AppError> {
        if embedding.len() < self.dim {
            return Err(AppError::Embedding(
                crate::i18n::validation::embedding_dimension_less_than_requested(
                    embedding.len(),
                    self.dim,
                ),
            ));
        }
        if embedding.len() == self.dim {
            Ok(embedding)
        } else {
            Ok(embedding[..self.dim].to_vec())
        }
    }
}
