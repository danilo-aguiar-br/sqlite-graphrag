//! Unit and `wiremock` tests for the OpenRouter embedding client.

use super::error::EmbedError;
use super::mrl::{model_default_input_type, model_supports_mrl, mrl_wire_dimensions};
use super::wire::*;
use super::{OpenRouterClient, DEFAULT_TIMEOUT_SECS};
use crate::errors::AppError;
use crate::openrouter_http::ApiError;
use crate::retry::AttemptOutcome;
use secrecy::SecretBox;

#[test]
fn test_supports_mrl_detection() {
    assert!(model_supports_mrl("qwen/qwen3-embedding-8b"));
    assert!(model_supports_mrl("qwen/qwen3-embedding-4b"));
    assert!(model_supports_mrl("openai/text-embedding-3-small"));
    assert!(model_supports_mrl("openai/text-embedding-3-large"));
    assert!(model_supports_mrl("google/gemini-embedding-001"));
    assert!(model_supports_mrl("google/gemini-embedding-2"));
    assert!(model_supports_mrl(
        "nvidia/llama-nemotron-embed-vl-1b-v2:free"
    ));
    assert!(model_supports_mrl("baai/bge-m3"));

    assert!(!model_supports_mrl("perplexity/pplx-embed-v1-0.6b"));
    assert!(!model_supports_mrl("mistralai/mistral-embed-2312"));
    assert!(!model_supports_mrl("some-random-model"));
}

#[test]
fn test_mrl_wire_dimensions_qwen_omits_wire_dim() {
    // OpenRouter qwen3 rejects dimensions=384; wire must omit and truncate.
    assert_eq!(mrl_wire_dimensions("qwen/qwen3-embedding-8b", 384), None);
    assert_eq!(mrl_wire_dimensions("qwen/qwen3-embedding-4b", 512), None);
    // Other MRL models still request the configured dim on the wire.
    assert_eq!(
        mrl_wire_dimensions("openai/text-embedding-3-small", 384),
        Some(384)
    );
    assert_eq!(mrl_wire_dimensions("baai/bge-m3", 256), Some(256));
    // Non-MRL never sends dimensions.
    assert_eq!(
        mrl_wire_dimensions("mistralai/mistral-embed-2312", 1024),
        None
    );
}

#[test]
fn test_model_default_input_type() {
    assert_eq!(
        model_default_input_type("nvidia/llama-nemotron-embed-vl-1b-v2:free"),
        Some("passage")
    );
    assert_eq!(
        model_default_input_type("mistralai/mistral-embed-2312"),
        None
    );
    assert_eq!(
        model_default_input_type("qwen/qwen3-embedding-8b"),
        Some("search_document")
    );
    assert_eq!(
        model_default_input_type("openai/text-embedding-3-small"),
        Some("search_document")
    );
    assert_eq!(
        model_default_input_type("baai/bge-m3"),
        Some("search_document")
    );
}

#[test]
fn test_truncate_embedding() {
    let api_key = SecretBox::new(Box::new("test-key".to_string()));
    let client =
        OpenRouterClient::new(api_key, "test-model".into(), 3, DEFAULT_TIMEOUT_SECS).unwrap();

    let full = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let truncated = client.truncate_embedding(full).unwrap();
    assert_eq!(truncated, vec![1.0, 2.0, 3.0]);

    let exact = vec![1.0, 2.0, 3.0];
    let kept = client.truncate_embedding(exact).unwrap();
    assert_eq!(kept, vec![1.0, 2.0, 3.0]);

    let short = vec![1.0, 2.0];
    let err = client.truncate_embedding(short);
    assert!(err.is_err());
}

#[test]
fn embedding_envelope_surfaces_provider_error_not_missing_field() {
    // GAP-SG-01: a 200 body carrying an OpenRouter error object must yield
    // the REAL message, not the misleading missing-field parse failure.
    let body = r#"{"error":{"code":400,"message":"context length exceeded"}}"#;

    // Precondition: the legacy optimistic parse masked the cause. Match
    // instead of unwrap_err so EmbeddingResponse need not derive Debug.
    let legacy_err = match serde_json::from_str::<EmbeddingResponse>(body) {
        Ok(_) => panic!("legacy parse should have failed on an error body"),
        Err(e) => e.to_string(),
    };
    assert!(
        legacy_err.contains("missing field"),
        "precondition: legacy parse masks the cause as a missing field: {legacy_err}"
    );

    // The envelope captures the structured error instead.
    let env: EmbeddingEnvelope = serde_json::from_str(body).expect("envelope parses an error body");
    assert!(env.data.is_none());
    let api_err = env.error.expect("error object captured");
    assert_eq!(api_err.message, "context length exceeded");
    assert_eq!(api_err.code_string(), "400");
}

#[test]
fn embedding_envelope_parses_success_body() {
    let body = r#"{"data":[{"embedding":[1.0,2.0,3.0],"index":0}]}"#;
    let env: EmbeddingEnvelope =
        serde_json::from_str(body).expect("envelope parses a success body");
    assert!(env.error.is_none());
    let data = env.data.expect("data present");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].embedding, vec![1.0, 2.0, 3.0]);
}

#[test]
fn api_error_code_string_handles_number_string_and_missing() {
    let num: ApiError = serde_json::from_str(r#"{"code":429,"message":"slow down"}"#).unwrap();
    assert_eq!(num.code_string(), "429");

    let s: ApiError =
        serde_json::from_str(r#"{"code":"rate_limited","message":"slow down"}"#).unwrap();
    assert_eq!(s.code_string(), "rate_limited");

    let missing: ApiError = serde_json::from_str(r#"{"message":"oops"}"#).unwrap();
    assert_eq!(missing.code_string(), "unknown");
}

#[tokio::test]
async fn embed_single_rejects_oversized_input_before_request() {
    // GAP-SG-02 / v1.1.2 (Gap 2): an input above
    // EMBEDDING_REQUEST_MAX_TOKENS must fail as the typed TooManyTokens
    // (exit 6) WITHOUT any network call. The fake key/URL would error
    // distinctly (Embedding) if the guard let the request through.
    let api_key = SecretBox::new(Box::new("test-key".to_string()));
    let client = OpenRouterClient::new(
        api_key,
        "qwen/qwen3-embedding-8b".into(),
        384,
        DEFAULT_TIMEOUT_SECS,
    )
    .unwrap();
    let big = "word ".repeat(crate::constants::EMBEDDING_REQUEST_MAX_TOKENS + 5_000);
    match client.embed_single(&big, None).await {
        Err(EmbedError {
            source: AppError::TooManyTokens { tokens, limit },
            retry_class,
        }) => {
            assert!(tokens > limit, "tokens={tokens} limit={limit}");
            assert_eq!(limit, crate::constants::EMBEDDING_REQUEST_MAX_TOKENS as u64);
            assert_eq!(
                retry_class,
                AttemptOutcome::HardFailure,
                "an oversized input is a permanent client error"
            );
        }
        other => unreachable!("expected TooManyTokens before request, got: {other:?}"),
    }
}

async fn client_for(server: &wiremock::MockServer, model: &str) -> OpenRouterClient {
    OpenRouterClient::new_with_url(
        SecretBox::new(Box::new("test-key".to_string())),
        model.to_string(),
        384,
        DEFAULT_TIMEOUT_SECS,
        format!("{}/embeddings", server.uri()),
    )
    .expect("test client builds")
}

#[tokio::test]
async fn embed_single_401_is_hard_failure() {
    // Reauditor addendum: classification happens at the HTTP status, not
    // by matching the error message downstream.
    use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = client_for(&server, "qwen/qwen3-embedding-8b").await;
    let err = client
        .embed_single("hello", None)
        .await
        .expect_err("401 is an error");
    assert_eq!(err.retry_class, AttemptOutcome::HardFailure);
}

#[tokio::test]
async fn embed_single_exhausted_5xx_is_transient() {
    // Reauditor addendum: exhausting every retry against a persistent
    // 5xx is TRANSIENT — the caller's --max-attempts is what eventually
    // dead-letters it, never a HardFailure from this layer.
    use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let client = client_for(&server, "qwen/qwen3-embedding-8b").await;
    let err = client
        .embed_single("hello", None)
        .await
        .expect_err("persistent 5xx exhausts retries");
    assert_eq!(err.retry_class, AttemptOutcome::Transient);
}

#[tokio::test]
async fn embed_single_provider_error_code_classifies_by_code_not_message() {
    // Reauditor addendum: a 200 body carrying a structured provider error
    // is classified by its `code`, reusing the exact same classifier
    // `chat_api` uses (GAP-SG-74 DRY).
    use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": { "code": "context_length_exceeded", "message": "too many tokens" }
        })))
        .mount(&server)
        .await;

    let client = client_for(&server, "qwen/qwen3-embedding-8b").await;
    let err = client
        .embed_single("hello", None)
        .await
        .expect_err("provider error must surface");
    assert_eq!(err.retry_class, AttemptOutcome::HardFailure);
}
