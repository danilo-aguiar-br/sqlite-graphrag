//! HTTP request/retry loop and origin-side failure classification.
//!
//! Kept apart from [`super::client`] so the retry policy — which decides what
//! is transient versus permanent — is auditable without reading the call
//! surface around it.

use super::error::ChatError;
use super::wire::{ChatRequest, ChatResponse};
use super::OpenRouterChatClient;
use crate::errors::AppError;
use crate::retry::AttemptOutcome;
use secrecy::ExposeSecret;
use std::time::Duration;

impl OpenRouterChatClient {
    /// Runs the request/retry loop, classifying every failure into a
    /// [`ChatError`] with `retry_class` set AT THE ORIGIN (the exact HTTP
    /// status, or the provider's structured error code) — never inferred
    /// downstream from a formatted message (reauditor addendum to
    /// GAP-SG-72-chat).
    pub(super) async fn execute_with_retry(
        &self,
        request: &ChatRequest<'_>,
    ) -> Result<ChatResponse, ChatError> {
        let mut last_err: Option<ChatError> = None;

        for attempt in 0..crate::openrouter_http::MAX_RETRIES {
            let result = self
                .client
                .post(&self.base_url)
                .header(
                    "Authorization",
                    format!("Bearer {}", self.api_key.expose_secret()),
                )
                .json(request)
                .send()
                .await;

            let resp = match result {
                Ok(r) => r,
                Err(e) if e.is_timeout() => {
                    return Err(ChatError::new(
                        AppError::Validation(crate::i18n::validation::openrouter_chat_timed_out()),
                        AttemptOutcome::Transient,
                    ));
                }
                Err(e) => {
                    last_err = Some(ChatError::new(
                        AppError::Validation(crate::i18n::validation::http_request_failed(&e)),
                        AttemptOutcome::Transient,
                    ));
                    crate::openrouter_http::backoff(attempt).await;
                    continue;
                }
            };

            let status = resp.status();

            if status.is_success() {
                let body = resp.text().await.map_err(|e| {
                    ChatError::new(
                        AppError::Validation(
                            crate::i18n::validation::failed_to_read_response_body(&e),
                        ),
                        AttemptOutcome::Transient,
                    )
                })?;
                match serde_json::from_str::<ChatResponse>(&body) {
                    Ok(parsed) => {
                        // A structured error object inside a 2xx body is
                        // classified by its own `code` (GAP-SG-03 surfaces
                        // the real code/message instead of letting empty
                        // choices masquerade as no-structured-content).
                        if let Some(api_err) = parsed.error {
                            let retry_class =
                                crate::openrouter_http::provider_error_retry_class(&api_err);
                            return Err(ChatError::new(
                                AppError::ProviderError {
                                    code: api_err.code_string(),
                                    message: api_err.message,
                                },
                                retry_class,
                            ));
                        }
                        return Ok(parsed);
                    }
                    Err(e) => {
                        tracing::warn!(
                            attempt,
                            body_len = body.len(),
                            "HTTP 200 but parse failed (retrying): {e}"
                        );
                        last_err = Some(ChatError::new(
                            AppError::Validation(
                                crate::i18n::validation::failed_to_parse_chat_response(&e),
                            ),
                            AttemptOutcome::Transient,
                        ));
                        crate::openrouter_http::backoff(attempt).await;
                        continue;
                    }
                }
            }

            if status.as_u16() == 401 {
                return Err(ChatError::new(
                    AppError::Validation(crate::i18n::validation::openrouter_invalid_api_key_401()),
                    AttemptOutcome::HardFailure,
                ));
            }

            if status.as_u16() == 400 || status.as_u16() == 404 {
                let body = resp.text().await.unwrap_or_default();
                return Err(ChatError::new(
                    AppError::Validation(crate::i18n::validation::openrouter_status_error(
                        &status,
                        &self.model,
                        &body,
                    )),
                    AttemptOutcome::HardFailure,
                ));
            }

            if status.as_u16() == 429 {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(2);
                tracing::warn!(
                    attempt,
                    retry_after_secs = retry_after,
                    "OpenRouter rate limited, waiting"
                );
                // GAP-SG-56: surface the Retry-After delay to the caller. If
                // every attempt is rate limited, the loop exits with this
                // RateLimited error (retryable) carrying the server-advised
                // wait, instead of a generic max-retries-exceeded message.
                last_err = Some(ChatError::new(
                    AppError::RateLimited {
                        detail: format!("OpenRouter HTTP 429 (retry-after {retry_after}s)"),
                    },
                    AttemptOutcome::Transient,
                ));
                tokio::time::sleep(Duration::from_secs(retry_after)).await;
                continue;
            }

            if status.is_server_error() {
                tracing::warn!(attempt, status = %status, "OpenRouter server error, retrying");
                last_err = Some(ChatError::new(
                    AppError::Validation(crate::i18n::validation::openrouter_server_error(&status)),
                    AttemptOutcome::Transient,
                ));
                crate::openrouter_http::backoff(attempt).await;
                continue;
            }

            let body = resp.text().await.unwrap_or_default();
            return Err(ChatError::new(
                AppError::Validation(crate::i18n::validation::unexpected_http_status(
                    &status, &body,
                )),
                crate::openrouter_http::status_retry_class(status),
            ));
        }

        // GAP-SG-72-chat addendum: exhausting every retry against a
        // transient condition (429/5xx/timeout/network) is ITSELF transient
        // — it is exactly the case the queue's `--max-attempts` backoff
        // covers, and must never be reclassified as a permanent failure.
        Err(last_err.unwrap_or_else(|| {
            ChatError::new(
                AppError::Validation(crate::i18n::validation::openrouter_chat_max_retries()),
                AttemptOutcome::Transient,
            )
        }))
    }
}
