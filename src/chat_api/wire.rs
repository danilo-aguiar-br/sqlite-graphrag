//! Wire types for the OpenRouter chat-completions protocol.
//!
//! Pure serde shapes: the structured-output request body and the response
//! envelope, including the provider error object OpenRouter can embed inside
//! an HTTP 200 body.

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(super) struct ChatRequest<'a> {
    pub(super) model: &'a str,
    pub(super) messages: Vec<ChatMessage<'a>>,
    pub(super) response_format: ResponseFormat,
    pub(super) provider: ProviderPrefs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reasoning: Option<ReasoningPrefs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_tokens: Option<u32>,
}

#[derive(Serialize)]
pub(super) struct ChatMessage<'a> {
    pub(super) role: &'a str,
    pub(super) content: String,
}

#[derive(Serialize)]
pub(super) struct ResponseFormat {
    #[serde(rename = "type")]
    pub(super) format_type: &'static str,
    pub(super) json_schema: JsonSchemaSpec,
}

#[derive(Serialize)]
pub(super) struct JsonSchemaSpec {
    pub(super) name: &'static str,
    pub(super) strict: bool,
    pub(super) schema: serde_json::Value,
}

#[derive(Serialize)]
pub(super) struct ProviderPrefs {
    pub(super) require_parameters: bool,
}

#[derive(Serialize)]
pub(super) struct ReasoningPrefs {
    pub(super) enabled: bool,
}

#[derive(Deserialize)]
pub(super) struct ChatResponse {
    #[serde(default)]
    pub(super) choices: Vec<Choice>,
    #[serde(default)]
    pub(super) usage: Option<Usage>,
    /// Structured provider error. OpenRouter may return this inside an HTTP 200
    /// body (e.g. token/context-length overflow); without it the response would
    /// parse into empty `choices` and surface the misleading "no structured
    /// content" error instead of the real cause (GAP-SG-03).
    #[serde(default)]
    pub(super) error: Option<crate::openrouter_http::ApiError>,
}

#[derive(Deserialize)]
pub(super) struct Choice {
    pub(super) message: RespMessage,
    /// Why the model stopped generating: `"stop"` on a normal completion,
    /// `"length"` when `max_tokens` cut the response short (GAP-SG-70/72-chat).
    /// Absent from providers that omit it, hence `#[serde(default)]`.
    #[serde(default)]
    pub(super) finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RespMessage {
    #[serde(default)]
    pub(super) content: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct Usage {
    #[serde(default)]
    pub(super) cost: Option<f64>,
    /// Prompt token count reported by OpenRouter (GAP-SG-72-chat). Diagnostic
    /// only — never used to gate control flow, so a missing value stays `None`.
    #[serde(default)]
    pub(super) prompt_tokens: Option<u32>,
    /// Completion token count reported by OpenRouter (GAP-SG-72-chat), used
    /// alongside `finish_reason` to explain a truncated response.
    #[serde(default)]
    pub(super) completion_tokens: Option<u32>,
}
