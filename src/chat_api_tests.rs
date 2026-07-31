use super::*;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_SCHEMA: &str = r#"{"type":"object"}"#;

fn key() -> SecretBox<String> {
    SecretBox::new(Box::new("test-key".to_string()))
}

/// Builds a chat-completions success body whose single choice carries the
/// model output as a JSON *string* (the double-encoding the real API uses
/// under structured outputs), optionally attaching `usage.cost` and a
/// `finish_reason` (defaults to `"stop"`).
fn success_body(content: &str, cost: Option<f64>) -> serde_json::Value {
    success_body_with_finish(content, cost, "stop")
}

/// Same as [`success_body`] but with an explicit `finish_reason`, used by
/// the GAP-SG-70 truncation tests.
fn success_body_with_finish(
    content: &str,
    cost: Option<f64>,
    finish_reason: &str,
) -> serde_json::Value {
    let mut body = json!({
        "choices": [{ "message": { "content": content }, "finish_reason": finish_reason }]
    });
    if let Some(c) = cost {
        body["usage"] = json!({ "cost": c });
    }
    body
}

async fn client_for(server: &MockServer, model: &str) -> OpenRouterChatClient {
    OpenRouterChatClient::new_with_url(
        key(),
        model.to_string(),
        format!("{}/chat/completions", server.uri()),
        30,
    )
    .expect("test client builds")
}

#[test]
fn new_builds_client_and_binds_model() {
    let client =
        OpenRouterChatClient::new(key(), "z-ai/glm-5.2".to_string(), 30).expect("client builds");
    assert_eq!(client.model(), "z-ai/glm-5.2");
}

#[test]
fn new_defaults_base_url_to_public_endpoint() {
    let client =
        OpenRouterChatClient::new(key(), "z-ai/glm-5.2".to_string(), 30).expect("client builds");
    assert_eq!(client.base_url, DEFAULT_OPENROUTER_CHAT_URL);
}

#[test]
fn request_serializes_with_strict_schema_and_disabled_reasoning() {
    let request = ChatRequest {
        model: "deepseek/deepseek-v4-flash",
        messages: vec![ChatMessage {
            role: "system",
            content: "extract".to_string(),
        }],
        response_format: ResponseFormat {
            format_type: "json_schema",
            json_schema: JsonSchemaSpec {
                name: SCHEMA_NAME,
                strict: true,
                schema: serde_json::json!({"type": "object"}),
            },
        },
        provider: ProviderPrefs {
            require_parameters: true,
        },
        reasoning: Some(ReasoningPrefs { enabled: false }),
        max_tokens: None,
    };
    let json = serde_json::to_value(&request).expect("serializes");
    assert_eq!(json["response_format"]["type"], "json_schema");
    assert_eq!(json["response_format"]["json_schema"]["strict"], true);
    assert_eq!(json["provider"]["require_parameters"], true);
    assert_eq!(json["reasoning"]["enabled"], false);
    // max_tokens omitted when None
    assert!(json.get("max_tokens").is_none());
}

#[test]
fn grow_max_tokens_uses_initial_default_when_current_is_none() {
    assert_eq!(
        grow_max_tokens(None),
        crate::constants::ENRICH_INITIAL_MAX_TOKENS
            * crate::constants::ENRICH_MAX_TOKENS_GROWTH_FACTOR
    );
}

#[test]
fn grow_max_tokens_caps_at_ceiling() {
    assert_eq!(
        grow_max_tokens(Some(crate::constants::ENRICH_MAX_TOKENS_CEILING)),
        crate::constants::ENRICH_MAX_TOKENS_CEILING
    );
    assert_eq!(
        grow_max_tokens(Some(u32::MAX)),
        crate::constants::ENRICH_MAX_TOKENS_CEILING
    );
}

#[tokio::test]
async fn complete_sends_wellformed_request_and_parses_content() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(json!({
            "model": "deepseek/deepseek-v4-flash",
            "response_format": {
                "type": "json_schema",
                "json_schema": { "name": "enrich_output", "strict": true }
            },
            "provider": { "require_parameters": true },
            "reasoning": { "enabled": false }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body(
            r#"{"entities":[],"relationships":[]}"#,
            Some(0.0023),
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, "deepseek/deepseek-v4-flash").await;
    let completion = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect("completion succeeds");

    assert_eq!(
        completion.value,
        json!({"entities": [], "relationships": []})
    );
    assert!((completion.cost_usd - 0.0023).abs() < f64::EPSILON);
    assert_eq!(completion.finish_reason.as_deref(), Some("stop"));
}

#[tokio::test]
async fn complete_defaults_cost_to_zero_when_usage_absent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(success_body(r#"{"entities":[]}"#, None)),
        )
        .mount(&server)
        .await;

    let client = client_for(&server, "z-ai/glm-5.2").await;
    let completion = client
        .complete("system", "", TEST_SCHEMA, Some(4096))
        .await
        .expect("completion succeeds");
    assert_eq!(completion.cost_usd, 0.0);
}

#[tokio::test]
async fn complete_retries_on_429_honouring_retry_after() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "1"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(success_body(r#"{"ok":true}"#, Some(0.0))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, "minimax/minimax-m3").await;
    let completion = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect("retried completion succeeds");
    assert_eq!(completion.value, json!({"ok": true}));
}

#[tokio::test]
async fn complete_retries_on_5xx_with_backoff() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(success_body(r#"{"ok":1}"#, Some(0.0))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, "openai/gpt-oss-120b").await;
    let completion = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect("retried completion succeeds");
    assert_eq!(completion.value, json!({"ok": 1}));
}

#[tokio::test]
async fn complete_401_is_permanent_without_retry() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, "z-ai/glm-5.2").await;
    let err = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect_err("401 is an error");
    assert!(err.to_string().contains("401"), "got: {err}");
    assert_eq!(err.retry_class, AttemptOutcome::HardFailure);
}

#[tokio::test]
async fn complete_400_returns_body_and_model_without_retry() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_string("schema not supported"))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, "xiaomi/mimo-v2.5").await;
    let err = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect_err("400 is an error");
    let msg = err.to_string();
    assert!(msg.contains("400"), "got: {msg}");
    assert!(msg.contains("xiaomi/mimo-v2.5"), "got: {msg}");
    assert!(msg.contains("schema not supported"), "got: {msg}");
    assert_eq!(err.retry_class, AttemptOutcome::HardFailure);
}

#[tokio::test]
async fn complete_empty_choices_errors_citing_model() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
        .mount(&server)
        .await;

    let client = client_for(&server, "minimax/minimax-m2.7").await;
    let err = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect_err("empty choices is an error");
    let msg = err.to_string();
    assert!(msg.contains("minimax/minimax-m2.7"), "got: {msg}");
    assert!(
        msg.contains("no structured content") || msg.contains("não retornou conteúdo estruturado"),
        "got: {msg}"
    );
    assert_eq!(err.finish_reason, None);
    assert_eq!(
        err.retry_class,
        AttemptOutcome::Transient,
        "no-content is a model hiccup, not a permanent rejection"
    );
}

#[tokio::test]
async fn complete_empty_content_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body("   ", Some(0.0))))
        .mount(&server)
        .await;

    let client = client_for(&server, "z-ai/glm-5.2:nitro").await;
    let err = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect_err("blank content is an error");
    let msg = err.to_string();
    assert!(
        msg.contains("no structured content") || msg.contains("não retornou conteúdo estruturado"),
        "got: {msg}"
    );
}

#[tokio::test]
async fn complete_non_json_content_errors_as_incompatible() {
    // GAP-SG-10: free text is coerced by the repair pass into a JSON string
    // (not an object), so it is rejected by the shape guard rather than the
    // strict-parse error. The message names the offending shape + model.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(success_body("this is not json", Some(0.0))),
        )
        .mount(&server)
        .await;

    let client = client_for(&server, "google/gemini-3.1-flash-lite").await;
    let err = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect_err("non-json content is an error");
    let msg = err.to_string();
    assert!(
        msg.contains("non-object JSON after repair") || msg.contains("JSON não-objeto após reparo"),
        "got: {msg}"
    );
    assert!(msg.contains("google/gemini-3.1-flash-lite"), "got: {msg}");
}

#[tokio::test]
async fn complete_repairs_markdown_fenced_object() {
    // GAP-SG-10: a model that wraps a valid object in a ```json fence (a
    // common deepseek-v4-flash:nitro defect) is repaired and parsed instead
    // of being rejected as non-JSON.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body(
            "```json\n{\"entities\":[\"rust\"],\"relationships\":[]}\n```",
            Some(0.0),
        )))
        .mount(&server)
        .await;

    let client = client_for(&server, "deepseek/deepseek-v4-flash").await;
    let completion = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect("fenced object is repaired");
    assert_eq!(
        completion.value,
        json!({"entities": ["rust"], "relationships": []})
    );
}

#[tokio::test]
async fn complete_rejects_invalid_schema_before_network() {
    // No mock mounted: an unreachable URL proves we never hit the network.
    let client = OpenRouterChatClient::new_with_url(
        key(),
        "z-ai/glm-5.2".to_string(),
        "http://127.0.0.1:1/chat/completions".to_string(),
        30,
    )
    .expect("client builds");
    let err = client
        .complete("system", "input", "{not valid json", None)
        .await
        .expect_err("invalid schema is rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid JSON schema") || msg.contains("schema JSON inválido"),
        "got: {msg}"
    );
    assert_eq!(
        err.retry_class,
        AttemptOutcome::HardFailure,
        "a malformed schema is a permanent caller error"
    );
}

#[tokio::test]
async fn complete_retries_with_reasoning_omitted_when_mandatory() {
    let server = MockServer::start().await;
    // Primary attempt (reasoning.enabled=false) is rejected with a 400 whose
    // body mentions "reasoning" — the mandatory-reasoning signal that drives
    // the one-shot fallback.
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string("reasoning is mandatory for this model and cannot be disabled"),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    // Fallback attempt (reasoning field omitted) succeeds.
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body(
            r#"{"entities":[],"relationships":[]}"#,
            Some(0.0),
        )))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, "minimax/minimax-m2.7").await;
    let completion = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect("fallback completion succeeds");
    assert_eq!(
        completion.value,
        json!({"entities": [], "relationships": []})
    );

    // Exactly two requests were sent: the FIRST carries reasoning.enabled=false,
    // the SECOND (fallback) OMITS the reasoning field entirely.
    let requests = server
        .received_requests()
        .await
        .expect("request recording is enabled");
    assert_eq!(requests.len(), 2, "expected primary + fallback requests");
    let first: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("first request body is JSON");
    let second: serde_json::Value =
        serde_json::from_slice(&requests[1].body).expect("second request body is JSON");
    assert_eq!(
        first["reasoning"]["enabled"],
        json!(false),
        "primary request must send reasoning.enabled=false"
    );
    assert!(
        second.get("reasoning").is_none(),
        "fallback request must omit the reasoning field, got: {second}"
    );
}

#[tokio::test]
async fn complete_honours_configured_timeout() {
    // A 1s client timeout against a server that delays 2s proves the
    // --openrouter-timeout value is wired into the reqwest builder instead
    // of the fixed 300s default (regression: the flag was silently ignored).
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(2))
                .set_body_json(success_body(r#"{"ok":1}"#, Some(0.0))),
        )
        .mount(&server)
        .await;

    let client = OpenRouterChatClient::new_with_url(
        key(),
        "z-ai/glm-5.2".to_string(),
        format!("{}/chat/completions", server.uri()),
        1,
    )
    .expect("client builds");
    let err = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect_err("request exceeds the 1s timeout");
    let msg = err.to_string();
    // Locale-safe: EN "timed out" / PT "expirou (timeout)"
    assert!(
        msg.contains("timed out") || msg.contains("timeout") || msg.contains("expirou"),
        "got: {msg}"
    );
}

#[tokio::test]
async fn complete_surfaces_provider_error_in_200_body() {
    // GAP-SG-03: an HTTP 200 whose body is a structured OpenRouter error
    // (token/context-length overflow) must surface the REAL message, not
    // the misleading no-structured-content from empty choices.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": { "code": 400, "message": "context length exceeded" }
        })))
        .mount(&server)
        .await;

    let client = client_for(&server, "deepseek/deepseek-v4-flash").await;
    let err = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect_err("provider error must surface");
    let msg = err.to_string();
    assert!(msg.contains("context length exceeded"), "got: {msg}");
    assert!(
        !msg.contains("no structured content"),
        "must not mask as empty choices: {msg}"
    );
    assert!(
        !msg.contains("missing field"),
        "must not mask as a missing field: {msg}"
    );
    assert_eq!(
        err.retry_class,
        AttemptOutcome::HardFailure,
        "code 400 (context length exceeded) is a permanent provider rejection"
    );
}

#[tokio::test]
async fn complete_classifies_provider_error_429_code_as_transient() {
    // Reauditor addendum: the provider error's structured `code` — never
    // its message — decides the retry verdict. A 429 code inside an
    // otherwise-2xx body is exactly as transient as an HTTP-level 429.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": { "code": 429, "message": "rate limited" }
        })))
        .mount(&server)
        .await;

    let client = client_for(&server, "deepseek/deepseek-v4-flash").await;
    let err = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect_err("provider rate-limit error must surface");
    assert_eq!(err.retry_class, AttemptOutcome::Transient);
}

#[tokio::test]
async fn complete_classifies_exhausted_5xx_retries_as_transient() {
    // GAP-SG-72-chat addendum: exhausting every retry against a
    // persistent 5xx is a TRANSIENT outcome (the queue's --max-attempts
    // is what eventually dead-letters it), never a HardFailure.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let client = client_for(&server, "openai/gpt-oss-120b").await;
    let err = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect_err("persistent 5xx exhausts retries");
    assert_eq!(err.retry_class, AttemptOutcome::Transient);
}

#[tokio::test]
async fn complete_regrows_max_tokens_and_retries_on_length_truncation() {
    // GAP-SG-70: the first response is truncated (finish_reason="length")
    // with content that is NOT valid JSON on its own (proving the retry
    // happens BEFORE json_repair, not as a repair fallback). The second
    // response (after max_tokens grows) is well-formed and finishes
    // normally.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(success_body_with_finish(
                r#"{"entities":["trunc"#,
                Some(0.001),
                "length",
            )),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(success_body_with_finish(
                r#"{"entities":["rust"],"relationships":[]}"#,
                Some(0.002),
                "stop",
            )),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, "deepseek/deepseek-v4-flash:nitro").await;
    let completion = client
        .complete("system", "input", TEST_SCHEMA, Some(64))
        .await
        .expect("second attempt with grown max_tokens succeeds");

    assert_eq!(
        completion.value,
        json!({"entities": ["rust"], "relationships": []})
    );
    assert_eq!(completion.finish_reason.as_deref(), Some("stop"));

    let requests = server
        .received_requests()
        .await
        .expect("request recording is enabled");
    assert_eq!(requests.len(), 2, "expected exactly one regrowth retry");
    let first: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("first request body is JSON");
    let second: serde_json::Value =
        serde_json::from_slice(&requests[1].body).expect("second request body is JSON");
    assert_eq!(first["max_tokens"], json!(64));
    assert_eq!(
        second["max_tokens"],
        json!(64 * crate::constants::ENRICH_MAX_TOKENS_GROWTH_FACTOR),
        "max_tokens must grow by ENRICH_MAX_TOKENS_GROWTH_FACTOR before the retry"
    );
}

#[tokio::test]
async fn complete_captures_finish_reason_and_tokens_on_success() {
    // GAP-SG-72-chat: a normal (non-truncated) completion still reports
    // finish_reason and both token counts to the caller.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "content": r#"{"ok":true}"# },
                "finish_reason": "stop"
            }],
            "usage": { "cost": 0.001, "prompt_tokens": 120, "completion_tokens": 30 }
        })))
        .mount(&server)
        .await;

    let client = client_for(&server, "z-ai/glm-5.2").await;
    let completion = client
        .complete("system", "input", TEST_SCHEMA, None)
        .await
        .expect("completion succeeds");

    assert_eq!(completion.finish_reason.as_deref(), Some("stop"));
    assert_eq!(completion.prompt_tokens, Some(120));
    assert_eq!(completion.completion_tokens, Some(30));
}

#[tokio::test]
async fn complete_gives_up_after_exhausting_length_retries() {
    // GAP-SG-70: every attempt (primary + ENRICH_MAX_LENGTH_RETRIES
    // regrowth retries) reports finish_reason="length" with unparsable
    // content, so complete() must give up and return a ChatError instead
    // of retrying forever.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(success_body_with_finish(
                r#"[1, 2, 3"#,
                Some(0.0),
                "length",
            )),
        )
        .mount(&server)
        .await;

    let client = client_for(&server, "deepseek/deepseek-v4-flash:nitro").await;
    let err = client
        .complete("system", "input", TEST_SCHEMA, Some(64))
        .await
        .expect_err("exhausted length retries must fail");
    assert_eq!(err.finish_reason.as_deref(), Some("length"));
    assert_eq!(
        err.retry_class,
        AttemptOutcome::Transient,
        "a repeatedly truncated response is a bounded-retry hiccup, not permanent"
    );

    let requests = server
        .received_requests()
        .await
        .expect("request recording is enabled");
    assert_eq!(
        requests.len() as u32,
        crate::constants::ENRICH_MAX_LENGTH_RETRIES + 1,
        "expected the primary attempt plus every regrowth retry"
    );
}
