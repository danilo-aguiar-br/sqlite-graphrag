//! Additional validation message catalog (v1.2.0 residual hardcode closeout).

use crate::i18n::{current, Language};


/// Memory/entity name required as positional or --name.
pub fn name_required_positional_or_flag() -> String {
    match current() {
        Language::English => {
            "name required: pass as positional argument or via --name".to_string()
        }
        Language::Portuguese => {
            "nome obrigatório: passe como argumento posicional ou via --name".to_string()
        }
    }
}

/// API key empty after read.
pub fn api_key_cannot_be_empty() -> String {
    match current() {
        Language::English => "API key cannot be empty".to_string(),
        Language::Portuguese => "chave de API não pode ser vazia".to_string(),
    }
}

/// Batch reclassify missing --from-type.
pub fn from_type_required_batch() -> String {
    match current() {
        Language::English => "--from-type is required in batch mode".to_string(),
        Language::Portuguese => "--from-type é obrigatório no modo batch".to_string(),
    }
}

/// Batch reclassify missing --to-type.
pub fn to_type_required_batch() -> String {
    match current() {
        Language::English => "--to-type is required in batch mode".to_string(),
        Language::Portuguese => "--to-type é obrigatório no modo batch".to_string(),
    }
}

/// Single-mode missing --name.
pub fn name_required_single_mode() -> String {
    match current() {
        Language::English => "--name is required in single mode".to_string(),
        Language::Portuguese => "--name é obrigatório no modo single".to_string(),
    }
}

/// Single-mode missing --target.
pub fn target_required_single_mode() -> String {
    match current() {
        Language::English => "--target is required in single mode".to_string(),
        Language::Portuguese => "--target é obrigatório no modo single".to_string(),
    }
}

/// --entity required when --memory is used.
pub fn entity_required_when_memory() -> String {
    match current() {
        Language::English => "--entity is required when --memory is used".to_string(),
        Language::Portuguese => "--entity é obrigatório quando --memory é usado".to_string(),
    }
}

/// --from required when not using --entity/--all.
pub fn from_required_without_entity_all() -> String {
    match current() {
        Language::English => "--from is required when --entity/--all is not used".to_string(),
        Language::Portuguese => {
            "--from é obrigatório quando --entity/--all não é usado".to_string()
        }
    }
}

/// --to required when not using --entity/--all.
pub fn to_required_without_entity_all() -> String {
    match current() {
        Language::English => "--to is required when --entity/--all is not used".to_string(),
        Language::Portuguese => {
            "--to é obrigatório quando --entity/--all não é usado".to_string()
        }
    }
}

/// Empty name rejected.
pub fn name_must_not_be_empty() -> String {
    match current() {
        Language::English => "name must not be empty".to_string(),
        Language::Portuguese => "nome não pode ser vazio".to_string(),
    }
}

/// Source and target names identical.
pub fn source_target_names_identical() -> String {
    match current() {
        Language::English => "source and target names are identical".to_string(),
        Language::Portuguese => "nomes de origem e destino são idênticos".to_string(),
    }
}

/// Source and target entity names identical.
pub fn source_target_entity_names_identical() -> String {
    match current() {
        Language::English => "source and target entity names are identical".to_string(),
        Language::Portuguese => "nomes de entidade de origem e destino são idênticos".to_string(),
    }
}

/// Empty version string.
pub fn empty_version_string() -> String {
    match current() {
        Language::English => "empty version string".to_string(),
        Language::Portuguese => "string de versão vazia".to_string(),
    }
}

/// Failed to open codex stdin.
pub fn failed_to_open_codex_stdin() -> String {
    match current() {
        Language::English => "failed to open codex stdin".to_string(),
        Language::Portuguese => "falha ao abrir stdin do codex".to_string(),
    }
}

/// Failed to open claude stdin.
pub fn failed_to_open_claude_stdin() -> String {
    match current() {
        Language::English => "failed to open claude stdin".to_string(),
        Language::Portuguese => "falha ao abrir stdin do claude".to_string(),
    }
}

/// Stdin writer thread panicked.
pub fn stdin_thread_panicked() -> String {
    match current() {
        Language::English => "stdin thread panicked".to_string(),
        Language::Portuguese => "thread de stdin entrou em pânico".to_string(),
    }
}

/// Codex --version not UTF-8.
pub fn codex_version_not_utf8() -> String {
    match current() {
        Language::English => "codex --version output is not UTF-8".to_string(),
        Language::Portuguese => "saída de codex --version não é UTF-8".to_string(),
    }
}

/// Claude --version not UTF-8.
pub fn claude_version_not_utf8() -> String {
    match current() {
        Language::English => "claude --version output is not UTF-8".to_string(),
        Language::Portuguese => "saída de claude --version não é UTF-8".to_string(),
    }
}

/// Codex exec stdout not UTF-8.
pub fn codex_exec_stdout_not_utf8() -> String {
    match current() {
        Language::English => "codex exec stdout is not valid UTF-8".to_string(),
        Language::Portuguese => "stdout de codex exec não é UTF-8 válido".to_string(),
    }
}

/// Codex stdout not UTF-8.
pub fn codex_stdout_not_utf8() -> String {
    match current() {
        Language::English => "codex stdout is not valid UTF-8".to_string(),
        Language::Portuguese => "stdout do codex não é UTF-8 válido".to_string(),
    }
}

/// Claude -p stdout not UTF-8.
pub fn claude_p_stdout_not_utf8() -> String {
    match current() {
        Language::English => "claude -p stdout is not valid UTF-8".to_string(),
        Language::Portuguese => "stdout de claude -p não é UTF-8 válido".to_string(),
    }
}

/// Opencode stdout not UTF-8.
pub fn opencode_stdout_not_utf8() -> String {
    match current() {
        Language::English => "opencode stdout is not valid UTF-8".to_string(),
        Language::Portuguese => "stdout do opencode não é UTF-8 válido".to_string(),
    }
}

/// Codex output missing agent_message.
pub fn codex_no_agent_message() -> String {
    match current() {
        Language::English => "codex output contained no agent_message item".to_string(),
        Language::Portuguese => "saída do codex não continha item agent_message".to_string(),
    }
}

/// Codex agent_message not JSON object.
pub fn codex_agent_message_not_json_object() -> String {
    match current() {
        Language::English => "codex agent_message is not a JSON object".to_string(),
        Language::Portuguese => "agent_message do codex não é um objeto JSON".to_string(),
    }
}

/// Claude output missing result element.
pub fn claude_output_missing_result() -> String {
    match current() {
        Language::English => "claude output missing 'result' element".to_string(),
        Language::Portuguese => "saída do claude sem elemento 'result'".to_string(),
    }
}

/// LLM result missing description field.
pub fn llm_missing_description_field() -> String {
    match current() {
        Language::English => "LLM result missing 'description' field".to_string(),
        Language::Portuguese => "resultado LLM sem campo 'description'".to_string(),
    }
}

/// LLM result missing enriched_body field.
pub fn llm_missing_enriched_body_field() -> String {
    match current() {
        Language::English => "LLM result missing 'enriched_body' field".to_string(),
        Language::Portuguese => "resultado LLM sem campo 'enriched_body'".to_string(),
    }
}

/// LLM result missing calibrated_weight.
pub fn llm_missing_calibrated_weight() -> String {
    match current() {
        Language::English => "LLM result missing 'calibrated_weight'".to_string(),
        Language::Portuguese => "resultado LLM sem 'calibrated_weight'".to_string(),
    }
}

/// LLM result missing relation.
pub fn llm_missing_relation() -> String {
    match current() {
        Language::English => "LLM result missing 'relation'".to_string(),
        Language::Portuguese => "resultado LLM sem 'relation'".to_string(),
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

/// Max retries exceeded for OpenRouter chat.
pub fn openrouter_chat_max_retries() -> String {
    match current() {
        Language::English => "max retries exceeded for OpenRouter chat request".to_string(),
        Language::Portuguese => {
            "número máximo de tentativas excedido para requisição de chat OpenRouter".to_string()
        }
    }
}

/// ANTHROPIC_API_KEY set — OAuth required.
pub fn anthropic_api_key_oauth_required() -> String {
    match current() {
        Language::English => "ANTHROPIC_API_KEY is set; v1.0.76 requires OAuth. unset it and use `claude login` instead."
            .to_string(),
        Language::Portuguese => "ANTHROPIC_API_KEY está definida; v1.0.76 exige OAuth. remova-a e use `claude login`."
            .to_string(),
    }
}

/// OPENAI_API_KEY set — OAuth required.
pub fn openai_api_key_oauth_required() -> String {
    match current() {
        Language::English => "OPENAI_API_KEY is set; v1.0.76 requires OAuth. unset it and use `codex login` instead."
            .to_string(),
        Language::Portuguese => "OPENAI_API_KEY está definida; v1.0.76 exige OAuth. remova-a e use `codex login`."
            .to_string(),
    }
}
