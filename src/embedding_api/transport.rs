//! HTTP request/retry loop and origin-side failure classification.
//!
//! Kept apart from [`super::client`] so the retry policy — which decides what
//! is transient versus permanent — is auditable without reading the call
//! surface around it.

use super::error::EmbedError;
use super::wire::{EmbeddingEnvelope, EmbeddingRequest, EmbeddingResponse};
use super::OpenRouterClient;
use crate::errors::AppError;
use crate::retry::AttemptOutcome;
use secrecy::ExposeSecret;
use std::time::Duration;

impl OpenRouterClient {
    /// Runs the request/retry loop, classifying every failure into an
    /// [`EmbedError`] with `retry_class` set AT THE ORIGIN (the exact HTTP
    /// status, or the provider's structured error code) via the same
    /// classifiers [`crate::chat_api::OpenRouterChatClient`] uses
    /// (GAP-SG-74 DRY) — never inferred downstream from a formatted message
    /// (reauditor addendum).
    pub(super) async fn execute_with_retry(
        &self,
        request: &EmbeddingRequest<'_>,
    ) -> Result<EmbeddingResponse, EmbedError> {
        let mut last_err: Option<EmbedError> = None;

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
                    return Err(EmbedError::new(
                        AppError::Embedding(
                            crate::i18n::validation::embedding_openrouter_request_timed_out(),
                        ),
                        AttemptOutcome::Transient,
                    ));
                }
                Err(e) => {
                    last_err = Some(EmbedError::new(
                        AppError::Embedding(
                            crate::i18n::validation::embedding_http_request_failed(e),
                        ),
                        AttemptOutcome::Transient,
                    ));
                    crate::openrouter_http::backoff(attempt).await;
                    continue;
                }
            };

            let status = resp.status();

            if status.is_success() {
                let body = resp.text().await.map_err(|e| {
                    EmbedError::new(
                        AppError::Embedding(
                            crate::i18n::validation::embedding_failed_to_read_response_body(e),
                        ),
                        AttemptOutcome::Transient,
                    )
                })?;
                match serde_json::from_str::<EmbeddingEnvelope>(&body) {
                    Ok(env) => {
                        // A structured error object inside a 2xx body is
                        // classified by its own `code` (GAP-SG-01 surfaces
                        // the real code/message instead of masking it as a
                        // parse failure).
                        if let Some(api_err) = env.error {
                            let retry_class =
                                crate::openrouter_http::provider_error_retry_class(&api_err);
                            return Err(EmbedError::new(
                                AppError::ProviderError {
                                    code: api_err.code_string(),
                                    message: api_err.message,
                                },
                                retry_class,
                            ));
                        }
                        match env.data {
                            Some(data) => return Ok(EmbeddingResponse { data }),
                            None => {
                                tracing::warn!(
                                    attempt,
                                    body_len = body.len(),
                                    "HTTP 200 with neither data nor error (retrying)"
                                );
                                last_err = Some(EmbedError::new(
                                    AppError::Embedding(
                                        crate::i18n::validation::embedding_openrouter_200_neither_data_nor_error(),
                                    ),
                                    AttemptOutcome::Transient,
                                ));
                                crate::openrouter_http::backoff(attempt).await;
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            attempt,
                            body_len = body.len(),
                            "HTTP 200 but JSON unparseable (retrying): {e}"
                        );
                        last_err = Some(EmbedError::new(
                            AppError::Embedding(
                                crate::i18n::validation::embedding_failed_to_parse_response(e),
                            ),
                            AttemptOutcome::Transient,
                        ));
                        crate::openrouter_http::backoff(attempt).await;
                        continue;
                    }
                }
            }

            if status.as_u16() == 401 {
                return Err(EmbedError::new(
                    AppError::Embedding(
                        crate::i18n::validation::embedding_openrouter_invalid_api_key_401(),
                    ),
                    AttemptOutcome::HardFailure,
                ));
            }

            if status.as_u16() == 400 || status.as_u16() == 404 {
                let body = resp.text().await.unwrap_or_default();
                return Err(EmbedError::new(
                    AppError::Embedding(crate::i18n::validation::embedding_openrouter_returned(
                        status, &body,
                    )),
                    AttemptOutcome::HardFailure,
                ));
            }

            if status.as_u16() == 429 {
                // The header is server-controlled, so it is clamped BEFORE it
                // reaches the sleep AND before it reaches the error detail: the
                // caller must be told the wait this process will actually
                // serve, not the one a provider asked for and never got.
                let retry_after = crate::constants::clamp_retry_after_secs(
                    resp.headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(2),
                );
                tracing::warn!(
                    attempt,
                    retry_after_secs = retry_after,
                    "OpenRouter rate limited, waiting"
                );
                // GAP-SG-56: surface the Retry-After delay to the caller. If
                // every attempt is rate limited, the loop exits with this
                // RateLimited error (retryable) carrying the server-advised
                // wait, instead of a generic max-retries-exceeded message.
                last_err = Some(EmbedError::new(
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
                last_err = Some(EmbedError::new(
                    AppError::Embedding(
                        crate::i18n::validation::embedding_openrouter_server_error(status),
                    ),
                    AttemptOutcome::Transient,
                ));
                crate::openrouter_http::backoff(attempt).await;
                continue;
            }

            let body = resp.text().await.unwrap_or_default();
            return Err(EmbedError::new(
                AppError::Embedding(crate::i18n::validation::embedding_unexpected_http(
                    status, &body,
                )),
                crate::openrouter_http::status_retry_class(status),
            ));
        }

        // Reauditor addendum: exhausting every retry against a transient
        // condition (429/5xx/timeout/network) is ITSELF transient — it is
        // exactly the case the queue's re-embed `--max-attempts` backoff
        // covers, and must never be reclassified as a permanent failure.
        Err(last_err.unwrap_or_else(|| {
            EmbedError::new(
                AppError::Embedding(crate::i18n::validation::embedding_openrouter_max_retries()),
                AttemptOutcome::Transient,
            )
        }))
    }
}
