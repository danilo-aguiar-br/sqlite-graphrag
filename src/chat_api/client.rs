//! Client construction and the structured-completion entry point.
//!
//! Owns the constructors, the `max_tokens` growth loop of
//! [`OpenRouterChatClient::complete`], the mandatory-reasoning fallback and
//! request assembly; the retry loop underneath lives in [`super::transport`]
//! and response finalisation in [`super::completion`].

use super::completion::{grow_max_tokens, ChatCompletion};
use super::error::{reasoning_disable_rejected, ChatError};
use super::wire::{
    ChatMessage, ChatRequest, ChatResponse, JsonSchemaSpec, ProviderPrefs, ReasoningPrefs,
    ResponseFormat,
};
use super::{
    OpenRouterChatClient, DEFAULT_CONNECT_TIMEOUT_SECS, DEFAULT_TIMEOUT_SECS, SCHEMA_NAME,
};
use crate::constants::DEFAULT_OPENROUTER_CHAT_URL;
use crate::errors::AppError;
use crate::retry::AttemptOutcome;
use secrecy::SecretBox;
use std::time::Duration;

impl OpenRouterChatClient {
    /// Builds a chat client bound to `model`, applying `timeout_secs` as the
    /// total per-request budget (wired from `--openrouter-timeout`). A value of
    /// `0` falls back to `DEFAULT_TIMEOUT_SECS` so a missing or zero flag never
    /// degrades into reqwest`'s immediate-timeout behaviour.
    pub fn new(
        api_key: SecretBox<String>,
        model: String,
        timeout_secs: u64,
    ) -> Result<Self, AppError> {
        let base_url = crate::runtime_config::openrouter_chat_url(DEFAULT_OPENROUTER_CHAT_URL);
        Self::new_with_base_url(api_key, model, timeout_secs, base_url)
    }

    /// Build a client posting to an explicit `base_url` (XDG override, tests, gateways).
    pub fn new_with_base_url(
        api_key: SecretBox<String>,
        model: String,
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
                AppError::Validation(crate::i18n::validation::http_client_build_failed(&e))
            })?;

        Ok(Self {
            client,
            api_key,
            model,
            base_url,
        })
    }

    /// Test-only constructor that POSTs to an arbitrary `base_url`.
    #[cfg(test)]
    pub fn new_with_url(
        api_key: SecretBox<String>,
        model: String,
        base_url: String,
        timeout_secs: u64,
    ) -> Result<Self, AppError> {
        Self::new_with_base_url(api_key, model, timeout_secs, base_url)
    }

    /// Returns the model bound to this client.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Runs a single structured-output completion, transparently growing
    /// `max_tokens` and re-issuing the request when the model truncates its
    /// output (GAP-SG-70).
    ///
    /// `schema_str` is the JSON Schema (as a string) the model must honour
    /// under `strict: true`. When `input_text` is empty only the system
    /// message is sent. `max_tokens` seeds the first attempt; `None` lets the
    /// provider apply its own default.
    ///
    /// Returns [`ChatCompletion`] on success or [`ChatError`] on failure; both
    /// carry `finish_reason`/token diagnostics when a response was decoded.
    ///
    /// # Errors
    ///
    /// Returns [`ChatError`] when: the schema is invalid JSON; the HTTP
    /// request fails or exhausts retries; the provider returns a permanent
    /// error (401/400/404, or a structured `error` object in a 2xx body); the
    /// response carries no usable content; the content cannot be parsed as
    /// JSON even after repair; the parsed JSON is not an object; or the
    /// response is truncated (`finish_reason: "length"`) after
    /// [`crate::constants::ENRICH_MAX_LENGTH_RETRIES`] `max_tokens` growth
    /// attempts are exhausted.
    pub async fn complete(
        &self,
        system_prompt: &str,
        input_text: &str,
        schema_str: &str,
        max_tokens: Option<u32>,
    ) -> Result<ChatCompletion, ChatError> {
        // A malformed schema is a permanent caller/config error — classified
        // explicitly (no blanket `From<AppError>` conversion exists for this
        // type; every `ChatError` states its `retry_class` at construction).
        let schema: serde_json::Value = serde_json::from_str(schema_str).map_err(|e| {
            ChatError::new(
                AppError::Validation(crate::i18n::validation::invalid_json_schema_for_request(&e)),
                AttemptOutcome::HardFailure,
            )
        })?;

        let mut current_max_tokens = max_tokens;

        for length_attempt in 0..=crate::constants::ENRICH_MAX_LENGTH_RETRIES {
            let response = self
                .complete_one_attempt(&schema, system_prompt, input_text, current_max_tokens)
                .await?;

            let finish_reason = response
                .choices
                .first()
                .and_then(|c| c.finish_reason.clone());
            let prompt_tokens = response.usage.as_ref().and_then(|u| u.prompt_tokens);
            let completion_tokens = response.usage.as_ref().and_then(|u| u.completion_tokens);

            let truncated = finish_reason.as_deref() == Some("length");
            let retries_left = length_attempt < crate::constants::ENRICH_MAX_LENGTH_RETRIES;

            if truncated && retries_left {
                let next_max_tokens = grow_max_tokens(current_max_tokens);
                tracing::warn!(
                    model = %self.model,
                    attempt = length_attempt,
                    previous_max_tokens = ?current_max_tokens,
                    next_max_tokens,
                    "OpenRouter completion truncated (finish_reason=length); \
                     retrying with a larger max_tokens budget"
                );
                current_max_tokens = Some(next_max_tokens);
                continue;
            }

            if truncated {
                tracing::warn!(
                    model = %self.model,
                    max_length_retries = crate::constants::ENRICH_MAX_LENGTH_RETRIES,
                    max_tokens = ?current_max_tokens,
                    "OpenRouter completion still truncated after exhausting \
                     max_tokens growth"
                );
            }

            return self.finish_completion(
                response,
                finish_reason,
                prompt_tokens,
                completion_tokens,
            );
        }

        unreachable!("loop always returns within ENRICH_MAX_LENGTH_RETRIES + 1 iterations")
    }

    /// Runs one HTTP attempt (including the mandatory-reasoning fallback) and
    /// returns the decoded [`ChatResponse`] without inspecting `finish_reason`
    /// or extracting content — that happens in [`Self::complete`] so the
    /// `max_tokens` growth loop can re-issue the request first.
    async fn complete_one_attempt(
        &self,
        schema: &serde_json::Value,
        system_prompt: &str,
        input_text: &str,
        max_tokens: Option<u32>,
    ) -> Result<ChatResponse, ChatError> {
        // First attempt sends reasoning.enabled=false (token savings on the
        // ~9 models that allow disabling). The ~4 reasoning-mandatory models
        // (e.g. minimax-m2.7, gpt-oss-120b) reject it with HTTP 400 mentioning
        // "reasoning"; on that specific failure we retry ONCE with the
        // reasoning field omitted so the model uses its mandatory default. Any
        // other error, or a second failure, propagates the original error.
        let primary = self.build_request(
            schema.clone(),
            system_prompt,
            input_text,
            max_tokens,
            Some(ReasoningPrefs { enabled: false }),
        );
        match self.execute_with_retry(&primary).await {
            Ok(r) => Ok(r),
            Err(first_err) => {
                if reasoning_disable_rejected(&first_err) {
                    tracing::warn!(
                        model = %self.model,
                        "model rejected reasoning.enabled=false (mandatory); \
                         retrying once with reasoning omitted"
                    );
                    let fallback = self.build_request(
                        schema.clone(),
                        system_prompt,
                        input_text,
                        max_tokens,
                        None,
                    );
                    match self.execute_with_retry(&fallback).await {
                        Ok(r) => Ok(r),
                        Err(_) => Err(first_err),
                    }
                } else {
                    Err(first_err)
                }
            }
        }
    }

    /// Builds a `ChatRequest` for one attempt. `reasoning` is `Some` on the
    /// primary attempt (`enabled:false`) and `None` on the mandatory-reasoning
    /// fallback, where the field is omitted entirely.
    fn build_request<'a>(
        &'a self,
        schema: serde_json::Value,
        system_prompt: &str,
        input_text: &str,
        max_tokens: Option<u32>,
        reasoning: Option<ReasoningPrefs>,
    ) -> ChatRequest<'a> {
        let mut messages = Vec::with_capacity(2);
        messages.push(ChatMessage {
            role: "system",
            content: system_prompt.to_string(),
        });
        if !input_text.is_empty() {
            messages.push(ChatMessage {
                role: "user",
                content: input_text.to_string(),
            });
        }
        ChatRequest {
            model: &self.model,
            messages,
            response_format: ResponseFormat {
                format_type: "json_schema",
                json_schema: JsonSchemaSpec {
                    name: SCHEMA_NAME,
                    strict: true,
                    schema,
                },
            },
            provider: ProviderPrefs {
                require_parameters: true,
            },
            reasoning,
            max_tokens,
        }
    }
}
