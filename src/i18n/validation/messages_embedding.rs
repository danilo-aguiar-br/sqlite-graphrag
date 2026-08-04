//! Embedding error message catalog (GAP-SG-132).
//!
//! EN+PT user-facing strings for `AppError::Embedding(String)`.
//!
//! `EmbeddingErrorKind::classify` matches English substrings case-insensitively:
//! `oauth`, `quota`, `slot exhausted`, `backend mismatch`, and (`dim` + `zero`).
//! Portuguese templates MUST retain those English markers so classification
//! continues to work under either locale.

use crate::i18n::{current, Language};
use std::fmt::Display;

// ── generic wrapper ──────────────────────────────────────────────────────────

/// Wrap an external / dynamic error detail as an embedding payload.
pub fn embedding_detail(err: impl Display) -> String {
    match current() {
        Language::English => format!("{err}"),
        Language::Portuguese => format!("{err}"),
    }
}

// ── OpenRouter HTTP / client ─────────────────────────────────────────────────

/// Failed to build the reqwest HTTP client for OpenRouter embeddings.
pub fn embedding_http_client_build_failed(err: impl Display) -> String {
    match current() {
        Language::English => format!("failed to build HTTP client: {err}"),
        Language::Portuguese => format!("falha ao construir cliente HTTP: {err}"),
    }
}

/// Empty `data` array in a successful OpenRouter embedding response.
pub fn embedding_empty_response_from_openrouter() -> String {
    match current() {
        Language::English => "empty response from OpenRouter".to_string(),
        Language::Portuguese => "resposta vazia do OpenRouter".to_string(),
    }
}

/// Batch response length mismatch.
pub fn embedding_expected_count(expected: usize, got: usize) -> String {
    match current() {
        Language::English => format!("expected {expected} embeddings, got {got}"),
        Language::Portuguese => format!("esperados {expected} embeddings, obtidos {got}"),
    }
}

/// Provider returned fewer dimensions than requested.
pub fn embedding_dimension_less_than_requested(got: usize, requested: usize) -> String {
    match current() {
        Language::English => format!("embedding dimension {got} < requested {requested}"),
        Language::Portuguese => {
            format!("dimensão de embedding {got} < solicitada {requested}")
        }
    }
}

/// OpenRouter embedding request timed out.
pub fn embedding_openrouter_request_timed_out() -> String {
    match current() {
        Language::English => "OpenRouter request timed out".to_string(),
        Language::Portuguese => "requisição OpenRouter expirou (timeout)".to_string(),
    }
}

/// Transient network failure during embedding HTTP.
pub fn embedding_http_request_failed(err: impl Display) -> String {
    match current() {
        Language::English => format!("HTTP request failed: {err}"),
        Language::Portuguese => format!("requisição HTTP falhou: {err}"),
    }
}

/// Failed to read embedding response body.
pub fn embedding_failed_to_read_response_body(err: impl Display) -> String {
    match current() {
        Language::English => format!("failed to read response body: {err}"),
        Language::Portuguese => format!("falha ao ler corpo da resposta: {err}"),
    }
}

/// HTTP 200 body had neither `data` nor `error`.
pub fn embedding_openrouter_200_neither_data_nor_error() -> String {
    match current() {
        Language::English => "OpenRouter 200 response had neither data nor error".to_string(),
        Language::Portuguese => "resposta OpenRouter 200 sem data nem error".to_string(),
    }
}

/// Failed to parse embedding JSON body.
pub fn embedding_failed_to_parse_response(err: impl Display) -> String {
    match current() {
        Language::English => format!("failed to parse embedding response: {err}"),
        Language::Portuguese => format!("falha ao analisar resposta de embedding: {err}"),
    }
}

/// Invalid OpenRouter API key (HTTP 401).
pub fn embedding_openrouter_invalid_api_key_401() -> String {
    match current() {
        Language::English => "invalid OpenRouter API key (HTTP 401)".to_string(),
        Language::Portuguese => "chave de API OpenRouter inválida (HTTP 401)".to_string(),
    }
}

/// OpenRouter hard-failure status with body.
pub fn embedding_openrouter_returned(status: impl Display, body: &str) -> String {
    match current() {
        Language::English => format!("OpenRouter returned {status}: {body}"),
        Language::Portuguese => format!("OpenRouter retornou {status}: {body}"),
    }
}

/// OpenRouter 5xx during embedding.
pub fn embedding_openrouter_server_error(status: impl Display) -> String {
    match current() {
        Language::English => format!("OpenRouter server error: {status}"),
        Language::Portuguese => format!("erro de servidor OpenRouter: {status}"),
    }
}

/// Unexpected HTTP status with body snippet.
pub fn embedding_unexpected_http(status: impl Display, body: &str) -> String {
    match current() {
        Language::English => format!("unexpected HTTP {status}: {body}"),
        Language::Portuguese => format!("HTTP inesperado {status}: {body}"),
    }
}

/// Max retries exhausted for OpenRouter embedding request.
pub fn embedding_openrouter_max_retries() -> String {
    match current() {
        Language::English => "max retries exceeded for OpenRouter request".to_string(),
        Language::Portuguese => {
            "número máximo de tentativas excedido para requisição OpenRouter".to_string()
        }
    }
}

// ── runtime / singleton getters ──────────────────────────────────────────────

/// Tokio multi-thread runtime failed to initialise.
pub fn embedding_tokio_runtime_init_failed(err: impl Display) -> String {
    match current() {
        Language::English => format!("tokio runtime init failed: {err}"),
        Language::Portuguese => format!("falha na inicialização do runtime tokio: {err}"),
    }
}

/// Tokio runtime missing after set.
pub fn embedding_tokio_runtime_unavailable() -> String {
    match current() {
        Language::English => "tokio runtime unavailable after initialisation".to_string(),
        Language::Portuguese => "runtime tokio indisponível após inicialização".to_string(),
    }
}

/// OpenRouter embed client singleton missing after set.
pub fn embedding_openrouter_client_unavailable() -> String {
    match current() {
        Language::English => "openrouter client unavailable after initialisation".to_string(),
        Language::Portuguese => "cliente openrouter indisponível após inicialização".to_string(),
    }
}

/// OpenRouter chat client singleton missing after set.
pub fn embedding_openrouter_chat_client_unavailable() -> String {
    match current() {
        Language::English => "openrouter chat client unavailable after initialisation".to_string(),
        Language::Portuguese => {
            "cliente de chat openrouter indisponível após inicialização".to_string()
        }
    }
}

// ── backend probes ───────────────────────────────────────────────────────────

/// OpenRouter probe: client not initialised.
pub fn embedding_openrouter_probe_not_initialised() -> String {
    match current() {
        Language::English => "openrouter probe: client not initialised (skip)".to_string(),
        Language::Portuguese => "probe openrouter: cliente não inicializado (skip)".to_string(),
    }
}

/// OpenRouter client not initialised for embed path.
pub fn embedding_openrouter_client_not_initialised() -> String {
    match current() {
        Language::English => {
            "OpenRouter client not initialised; call get_openrouter_embedder first".to_string()
        }
        Language::Portuguese => {
            "cliente OpenRouter não inicializado; chame get_openrouter_embedder primeiro"
                .to_string()
        }
    }
}

// ── dimension / validation ───────────────────────────────────────────────────

/// KNN search dim mismatch (memories / entities).
pub fn embedding_knn_search_dim_mismatch(got: usize, expected: usize) -> String {
    match current() {
        Language::English => {
            format!("knn_search embedding has {got} dims, expected {expected}")
        }
        Language::Portuguese => {
            format!("embedding knn_search tem {got} dims, esperado {expected}")
        }
    }
}

/// KNN search dim mismatch (chunks).
pub fn embedding_knn_search_chunks_dim_mismatch(got: usize, expected: usize) -> String {
    match current() {
        Language::English => {
            format!("knn_search_chunks embedding has {got} dims, expected {expected}")
        }
        Language::Portuguese => {
            format!("embedding knn_search_chunks tem {got} dims, esperado {expected}")
        }
    }
}

// ── slot / fan-out / batch ───────────────────────────────────────────────────

/// LLM slot semaphore exhausted (marker: `slot exhausted`).
pub fn embedding_slot_exhausted(err: impl Display) -> String {
    match current() {
        Language::English => format!("slot exhausted: {err} (fall back to FTS5)"),
        // Keep English marker "slot exhausted" for EmbeddingErrorKind::classify.
        Language::Portuguese => {
            format!("slot exhausted: {err} (fall back para FTS5)")
        }
    }
}

/// JoinSet join error while embedding.
pub fn embedding_task_join_error(err: impl Display) -> String {
    match current() {
        Language::English => format!("embedding task join error: {err}"),
        Language::Portuguese => format!("erro de join na tarefa de embedding: {err}"),
    }
}

/// Entity embed cache produced a null slot.
pub fn embedding_entity_cache_null() -> String {
    match current() {
        Language::English => "entity embed cache produced null result".to_string(),
        Language::Portuguese => "cache de embed de entidade produziu resultado nulo".to_string(),
    }
}

// ── LLM subprocess embedding ─────────────────────────────────────────────────

// ── dry-run backend guards ───────────────────────────────────────────────────

// ── opencode runner ──────────────────────────────────────────────────────────

/// The legacy ONNX-backed embedding extraction backend was removed in
/// v1.0.79; `extract` on it is unreachable and reports why.
pub fn legacy_embedding_backend_removed(model_name: &str) -> String {
    match current() {
        Language::English => format!(
            "legacy embedding backend ({model_name}) was removed in v1.0.79; use the llm backend"
        ),
        Language::Portuguese => format!(
            "o backend de embedding legado ({model_name}) foi removido na v1.0.79; use o backend llm"
        ),
    }
}
