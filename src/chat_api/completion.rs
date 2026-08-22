//! Response finalisation: content extraction, JSON repair and the
//! `max_tokens` growth policy.
//!
//! Everything that turns a decoded [`super::wire::ChatResponse`] into the
//! caller-facing [`ChatCompletion`], plus the truncation arithmetic that
//! decides how much room the next attempt gets (GAP-SG-10 / GAP-SG-70/71).

use super::error::ChatError;
use super::wire::ChatResponse;
use super::OpenRouterChatClient;
use crate::errors::AppError;
use crate::retry::AttemptOutcome;

/// Successful [`super::OpenRouterChatClient::complete`] result (GAP-SG-72-chat).
///
/// `finish_reason`, `prompt_tokens` and `completion_tokens` are the raw
/// diagnostics OpenRouter attached to the response that ultimately succeeded
/// (after any `max_tokens` growth retries — see [`Self::value`] and the
/// module docs). They are `None` only when the provider omitted them.
#[derive(Debug)]
pub struct ChatCompletion {
    /// Model output parsed as JSON (guaranteed to be a JSON object).
    pub value: serde_json::Value,
    /// Cost in USD read from `usage.cost`, or `0.0` when the provider omitted it.
    pub cost_usd: f64,
    /// `choices[0].finish_reason` from the response that produced `value`.
    pub finish_reason: Option<String>,
    /// `usage.prompt_tokens` from the response that produced `value`.
    pub prompt_tokens: Option<u32>,
    /// `usage.completion_tokens` from the response that produced `value`.
    pub completion_tokens: Option<u32>,
}

impl OpenRouterChatClient {
    /// Extracts content, repairs/parses it as JSON, and enforces the
    /// object-shape guard, attaching `finish_reason`/token diagnostics to any
    /// failure.
    ///
    /// Every failure branch below (missing content, JSON-repair failure,
    /// non-object shape) classifies as `AttemptOutcome::Transient`. This is a
    /// deliberate, acknowledged tension with `rules_rust_retry_com_backoff.md`
    /// (`NUNCA retentar erros de parsing ou deserialização` /
    /// `NUNCA retentar erros de deserialização`): those rules target DETERMINISTIC parse
    /// errors, where retrying the identical input reproduces the identical
    /// failure. Here the "input" is `deepseek-v4-flash:nitro` sampling
    /// variance — the SAME prompt can legitimately produce well-formed JSON
    /// on the next generation (see GAP-SG-10). So this is a typed, bounded
    /// hiccup, not a retry-forever loophole: it is capped by `--max-attempts`
    /// (GAP-SG-09/GAP-SG-21) and dead-letters once attempts are exhausted.
    pub(super) fn finish_completion(
        &self,
        response: ChatResponse,
        finish_reason: Option<String>,
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
    ) -> Result<ChatCompletion, ChatError> {
        let content = response
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .filter(|c| !c.trim().is_empty())
            .ok_or_else(|| {
                AppError::Validation(crate::i18n::validation::model_no_structured_content(
                    &self.model,
                ))
            })
            .map_err(|e| {
                ChatError::with_diagnostics(
                    e,
                    finish_reason.clone(),
                    prompt_tokens,
                    completion_tokens,
                    AttemptOutcome::Transient,
                )
            })?;

        // GAP-SG-10: deepseek-v4-flash:nitro and similar models do not honour
        // `json_schema` strict mode reliably — they wrap output in markdown
        // fences, add trailing commas, or omit quotes around keys. Try a strict
        // parse first (zero cost for well-formed JSON), then fall back to the
        // repair pass (a Rust port of `json_repair`) before giving up.
        let value = crate::json_repair::repair_to_value(&content).map_err(|e| {
            ChatError::with_diagnostics(
                AppError::Validation(crate::i18n::validation::model_json_parse_failed(
                    &self.model,
                    &e,
                )),
                finish_reason.clone(),
                prompt_tokens,
                completion_tokens,
                AttemptOutcome::Transient,
            )
        })?;

        // GAP-SG-10: `llm_json` coerces aggressively — free text becomes a JSON
        // string, empty input becomes `{}`, a lone delimiter becomes `null`. The
        // enrich JUDGE contract is ALWAYS a JSON object, so a non-object result
        // here is a malformed/refused generation, NOT a usable value. Reject it
        // (the enrich classifier reclassifies this as a transient model hiccup,
        // GAP-SG-09) instead of letting a coerced scalar masquerade as a
        // valid-but-empty result downstream.
        if !value.is_object() {
            return Err(ChatError::with_diagnostics(
                AppError::Validation(crate::i18n::validation::model_non_object_json(
                    &self.model,
                    json_shape_name(&value),
                )),
                finish_reason,
                prompt_tokens,
                completion_tokens,
                AttemptOutcome::Transient,
            ));
        }

        let cost = response.usage.and_then(|u| u.cost).unwrap_or(0.0);

        Ok(ChatCompletion {
            value,
            cost_usd: cost,
            finish_reason,
            prompt_tokens,
            completion_tokens,
        })
    }
}

/// Grows `current` for the next `max_tokens` retry after a truncated
/// (`finish_reason: "length"`) response (GAP-SG-70/71). When `current` is
/// `None` the caller left the provider default in place, so growth starts
/// from [`crate::constants::ENRICH_INITIAL_MAX_TOKENS`] instead of an unknown
/// base. The result is always capped at
/// [`crate::constants::ENRICH_MAX_TOKENS_CEILING`].
pub(super) fn grow_max_tokens(current: Option<u32>) -> u32 {
    let base = current.unwrap_or(crate::constants::ENRICH_INITIAL_MAX_TOKENS);
    base.saturating_mul(crate::constants::ENRICH_MAX_TOKENS_GROWTH_FACTOR)
        .min(crate::constants::ENRICH_MAX_TOKENS_CEILING)
}

/// Names the JSON shape of `value` for diagnostics (GAP-SG-10). Used when the
/// repaired model output is not the object the enrich JUDGE contract requires.
fn json_shape_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}
