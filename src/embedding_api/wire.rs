//! Wire types for the OpenRouter embeddings protocol.
//!
//! Pure serde shapes: what goes on the request body and what comes back,
//! including the envelope that carries a provider error instead of data.

use crate::openrouter_http::ApiError;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(super) struct EmbeddingRequest<'a> {
    pub(super) model: &'a str,
    pub(super) input: EmbeddingInput<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) dimensions: Option<usize>,
    pub(super) encoding_format: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) input_type: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum EmbeddingInput<'a> {
    Single(&'a str),
    Batch(Vec<&'a str>),
}

#[derive(Deserialize)]
pub(super) struct EmbeddingResponse {
    pub(super) data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
pub(super) struct EmbeddingData {
    pub(super) embedding: Vec<f32>,
    pub(super) index: usize,
}

/// Envelope that captures BOTH shapes the OpenRouter embeddings endpoint can
/// return: the success payload (`data`) and the structured error object
/// (`error`). OpenRouter sometimes returns the error object inside an HTTP 200
/// body (e.g. token/context-length overflow); a direct parse to
/// [`EmbeddingResponse`] would fail with a misleading missing-field error,
/// masking the real cause. Both fields are optional so the branch is decided
/// by inspection, not by a parse failure.
#[derive(Deserialize)]
pub(super) struct EmbeddingEnvelope {
    #[serde(default)]
    pub(super) data: Option<Vec<EmbeddingData>>,
    #[serde(default)]
    pub(super) error: Option<ApiError>,
}
