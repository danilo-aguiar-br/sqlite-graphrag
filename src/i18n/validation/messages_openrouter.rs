//! Messages from the OpenRouter REST transport (GAP-SG-146).
//!
//! HTTP client construction, request and response failures, and the
//! structured-output contract the chat endpoint must honour.

use crate::i18n::{current, Language};

/// OpenRouter API key could not be resolved.
pub fn openrouter_api_key_not_found() -> String {
    match current() {
        Language::English => "OpenRouter API key not found; store it via \
             `config add-key --provider openrouter`, or pass --openrouter-api-key \
             (product env is deprecated)"
            .to_string(),
        Language::Portuguese => "chave de API OpenRouter não encontrada; armazene via \
             `config add-key --provider openrouter`, ou passe --openrouter-api-key \
             (env de produto está depreciada)"
            .to_string(),
    }
}

/// Max retries exceeded for OpenRouter chat.
pub fn openrouter_chat_max_retries() -> String {
    match current() {
        Language::English => "max retries exceeded for OpenRouter chat request".to_string(),
        Language::Portuguese => {
            "número máximo de tentativas excedido para requisição de chat OpenRouter".to_string()
        }
    }
}

/// OpenRouter chat timed out.
pub fn openrouter_chat_timed_out() -> String {
    match current() {
        Language::English => "OpenRouter chat request timed out".to_string(),
        Language::Portuguese => "requisição de chat OpenRouter expirou (timeout)".to_string(),
    }
}

/// Invalid OpenRouter API key HTTP 401.
pub fn openrouter_invalid_api_key_401() -> String {
    match current() {
        Language::English => "invalid OpenRouter API key (HTTP 401)".to_string(),
        Language::Portuguese => "chave de API OpenRouter inválida (HTTP 401)".to_string(),
    }
}

/// OpenRouter mode requires an explicit model flag.
pub fn openrouter_model_required() -> String {
    match current() {
        Language::English => {
            "--mode openrouter requires --openrouter-model (no default model is allowed)"
                .to_string()
        }
        Language::Portuguese => {
            "--mode openrouter exige --openrouter-model (nenhum modelo padrão é permitido)"
                .to_string()
        }
    }
}

/// OpenRouter 5xx server error.
pub fn openrouter_server_error(status: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("OpenRouter server error: {status}"),
        Language::Portuguese => format!("erro de servidor OpenRouter: {status}"),
    }
}

/// OpenRouter returned a non-success status for a model.
pub fn openrouter_status_error(status: &impl std::fmt::Display, model: &str, body: &str) -> String {
    match current() {
        Language::English => {
            format!("OpenRouter returned {status} for model '{model}': {body}")
        }
        Language::Portuguese => {
            format!("OpenRouter retornou {status} para o modelo '{model}': {body}")
        }
    }
}

/// Failed to build the HTTP client.
pub fn http_client_build_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to build HTTP client: {err}"),
        Language::Portuguese => format!("falha ao construir cliente HTTP: {err}"),
    }
}

/// HTTP request failed (transport-level).
pub fn http_request_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("HTTP request failed: {err}"),
        Language::Portuguese => format!("requisição HTTP falhou: {err}"),
    }
}

/// Unexpected HTTP status with body snippet.
pub fn unexpected_http_status(status: &impl std::fmt::Display, body: &str) -> String {
    match current() {
        Language::English => format!("unexpected HTTP {status}: {body}"),
        Language::Portuguese => format!("HTTP inesperado {status}: {body}"),
    }
}

/// Failed to parse chat response JSON.
pub fn failed_to_parse_chat_response(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to parse chat response: {err}"),
        Language::Portuguese => format!("falha ao parsear resposta de chat: {err}"),
    }
}

/// Failed to read HTTP response body.
pub fn failed_to_read_response_body(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to read response body: {err}"),
        Language::Portuguese => format!("falha ao ler corpo da resposta: {err}"),
    }
}

/// Invalid JSON schema for an OpenRouter request body.
pub fn invalid_json_schema_for_request(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => {
            format!("invalid JSON schema for OpenRouter request: {err}")
        }
        Language::Portuguese => {
            format!("schema JSON inválido para requisição OpenRouter: {err}")
        }
    }
}

/// Embedded schema JSON is invalid.
pub fn embedded_schema_invalid_json(name: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("embedded schema for {name} is not valid JSON: {err}"),
        Language::Portuguese => {
            format!("schema embutido para {name} não é JSON válido: {err}")
        }
    }
}

/// Model content could not be parsed even after JSON repair.
pub fn model_json_parse_failed(model: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!(
            "model '{model}' returned content that could not be parsed even after \
             JSON repair: {err}"
        ),
        Language::Portuguese => format!(
            "modelo '{model}' retornou conteúdo que não pôde ser parseado mesmo após \
             reparo de JSON: {err}"
        ),
    }
}

/// Model returned non-object JSON after repair.
pub fn model_non_object_json(model: &str, shape: &str) -> String {
    match current() {
        Language::English => format!(
            "model '{model}' returned non-object JSON after repair (got {shape}); \
             likely a refusal or malformed structured output"
        ),
        Language::Portuguese => format!(
            "modelo '{model}' retornou JSON não-objeto após reparo (obteve {shape}); \
             provavelmente uma recusa ou saída estruturada malformada"
        ),
    }
}

/// Model returned no structured content.
pub fn model_no_structured_content(model: &str) -> String {
    match current() {
        Language::English => format!(
            "model '{model}' returned no structured content (incompatible with \
             structured outputs, or refused the request)"
        ),
        Language::Portuguese => format!(
            "modelo '{model}' não retornou conteúdo estruturado (incompatível com \
             saídas estruturadas, ou recusou a requisição)"
        ),
    }
}

/// Failed to parse an ExtractionResult from a provider.
pub fn failed_to_parse_extraction(provider: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => {
            format!("failed to deserialize {provider} output as ExtractionResult: {err}")
        }
        Language::Portuguese => {
            format!("falha ao deserializar saída de {provider} como ExtractionResult: {err}")
        }
    }
}

/// Failed to parse entities array.
pub fn failed_to_parse_entities_array(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to parse entities array: {err}"),
        Language::Portuguese => format!("falha ao parsear array de entidades: {err}"),
    }
}

/// Failed to parse relationships array.
pub fn failed_to_parse_relationships_array(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to parse relationships array: {err}"),
        Language::Portuguese => {
            format!("falha ao parsear array de relacionamentos: {err}")
        }
    }
}
