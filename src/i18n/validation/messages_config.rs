//! Messages about the XDG CONFIG file and stored credentials (GAP-SG-146).
//!
//! Parsing, ownership and symlink hardening of `config.toml`, plus the key
//! registry's verdicts on unknown and retired settings.

use crate::i18n::{current, Language};

/// Config file is a symlink (potential attack).
pub fn config_file_is_symlink(path: &str) -> String {
    match current() {
        Language::English => format!("config file is a symlink (potential attack): {path}"),
        Language::Portuguese => {
            format!("arquivo de config é um symlink (potencial ataque): {path}")
        }
    }
}

/// Config file owned by a different uid; refuse overwrite.
pub fn config_file_wrong_owner(path: &str, file_uid: u32, my_uid: u32) -> String {
    match current() {
        Language::English => format!(
            "config file {path} owned by uid {file_uid}, not current uid {my_uid}; refusing to overwrite"
        ),
        Language::Portuguese => format!(
            "arquivo de config {path} pertence ao uid {file_uid}, não ao uid atual {my_uid}; recusando sobrescrever"
        ),
    }
}

/// Rejects a `config set` key that was advertised historically but never read.
///
/// Distinct from [`config_key_unknown`] because the operator followed the
/// old documentation rather than mistyping, so the message names the
/// replacement directly instead of guessing.
pub fn config_key_retired(key: &str, replacement: &str) -> String {
    match current() {
        Language::English => format!(
            "config key '{key}' was never read by this binary; \
             use '{replacement}' instead"
        ),
        Language::Portuguese => format!(
            "a chave de config '{key}' nunca foi lida por este binário; \
             use '{replacement}' no lugar"
        ),
    }
}

/// Rejects a `config set` key that is not in the canonical registry.
///
/// `suggestion` carries the nearest known key when one is similar enough,
/// so a typo is actionable without the operator listing every key.
pub fn config_key_unknown(key: &str, suggestion: Option<&str>) -> String {
    match (current(), suggestion) {
        (Language::English, Some(s)) => format!(
            "unknown config key '{key}'; did you mean '{s}'? \
             list valid keys with `config doctor --json`"
        ),
        (Language::English, None) => format!(
            "unknown config key '{key}'; \
             list valid keys with `config doctor --json`"
        ),
        (Language::Portuguese, Some(s)) => format!(
            "chave de config desconhecida '{key}'; você quis dizer '{s}'? \
             liste as chaves válidas com `config doctor --json`"
        ),
        (Language::Portuguese, None) => format!(
            "chave de config desconhecida '{key}'; \
             liste as chaves válidas com `config doctor --json`"
        ),
    }
}

/// Localized description of the domain a [`ValueKind`] accepts.
///
/// `Text` and `OneOf` never reach here: the first has no domain to describe,
/// and the second is a list of literal spellings that must not be translated.
pub fn config_value_expectation(kind: crate::config::ValueKind) -> String {
    use crate::config::ValueKind as K;
    match (current(), kind) {
        (Language::English, K::Unsigned) => "a non-negative integer".to_string(),
        (Language::English, K::Float) => "a decimal number".to_string(),
        (Language::English, K::Tz) => "an IANA timezone (e.g. America/Sao_Paulo)".to_string(),
        (Language::English, K::Url) => "an http:// or https:// URL".to_string(),
        (Language::English, K::Path) => "a non-empty filesystem path".to_string(),
        (Language::English, K::LogDirective) => {
            "a tracing directive (e.g. warn, or sqlite_graphrag=debug)".to_string()
        }
        (Language::Portuguese, K::Unsigned) => "um inteiro não negativo".to_string(),
        (Language::Portuguese, K::Float) => "um número decimal".to_string(),
        (Language::Portuguese, K::Tz) => {
            "um fuso horário IANA (ex.: America/Sao_Paulo)".to_string()
        }
        (Language::Portuguese, K::Url) => "uma URL http:// ou https://".to_string(),
        (Language::Portuguese, K::Path) => {
            "um caminho de sistema de arquivos não vazio".to_string()
        }
        (Language::Portuguese, K::LogDirective) => {
            "uma diretiva de tracing (ex.: warn, ou sqlite_graphrag=debug)".to_string()
        }
        // Unreachable by construction: `ValueKind::expectation` handles both
        // before delegating. Answering with the literal spellings keeps this
        // total without a panic that a future variant could trip.
        (_, K::Bool) => "true|false (also 1|0, yes|no, on|off)".to_string(),
        (_, K::Text) => String::new(),
        (_, K::OneOf(options)) => options.join("|"),
    }
}

/// Config value outside the domain the key accepts.
///
/// GAP-SG-201: naming the expectation is the whole point. `invalid value` alone
/// sends the operator to the documentation; `expected true|false` lets them fix
/// the command they just typed.
pub fn config_value_invalid(key: &str, value: &str, expectation: &str) -> String {
    match current() {
        Language::English => {
            format!("invalid value '{value}' for config key '{key}'; expected {expectation}")
        }
        Language::Portuguese => format!(
            "valor inválido '{value}' para a chave de config '{key}'; esperado {expectation}"
        ),
    }
}

/// Config parse error at path.
pub fn config_parse_error(path: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("config parse error in {path}: {err}"),
        Language::Portuguese => format!("erro de parse de config em {path}: {err}"),
    }
}

/// Config path has no parent directory component.
pub fn config_path_no_parent(path: &str) -> String {
    match current() {
        Language::English => format!("config path has no parent: {path}"),
        Language::Portuguese => format!("caminho de config sem diretório pai: {path}"),
    }
}

/// API key empty after read.
pub fn api_key_cannot_be_empty() -> String {
    match current() {
        Language::English => "API key cannot be empty".to_string(),
        Language::Portuguese => "chave de API não pode ser vazia".to_string(),
    }
}

/// Localized message for `invalid_namespace_config`.
pub fn invalid_namespace_config(path: &str, err: &str) -> String {
    match current() {
        Language::English => {
            format!("invalid project namespace config '{path}': {err}")
        }
        Language::Portuguese => {
            format!("configuração de namespace de projeto inválida '{path}': {err}")
        }
    }
}

/// Localized message for `invalid_projects_mapping`.
pub fn invalid_projects_mapping(path: &str, err: &str) -> String {
    match current() {
        Language::English => format!("invalid projects mapping '{path}': {err}"),
        Language::Portuguese => format!("mapeamento de projetos inválido '{path}': {err}"),
    }
}
