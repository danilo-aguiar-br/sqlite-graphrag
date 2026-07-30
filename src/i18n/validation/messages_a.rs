use crate::i18n::{current, Language};

/// Localized message for `name_length`.
pub fn name_length(max: usize) -> String {
    match current() {
        Language::English => format!("name must be 1-{max} chars"),
        Language::Portuguese => format!("nome deve ter entre 1 e {max} caracteres"),
    }
}

/// Localized message for `reserved_name`.
pub fn reserved_name() -> String {
    match current() {
        Language::English => {
            "names and namespaces starting with __ are reserved for internal use".to_string()
        }
        Language::Portuguese => {
            "nomes e namespaces iniciados com __ são reservados para uso interno".to_string()
        }
    }
}

/// Localized message for `name_kebab`.
pub fn name_kebab(nome: &str) -> String {
    match current() {
        Language::English => format!(
            "name must be kebab-case slug (lowercase letters, digits, hyphens): '{nome}'"
        ),
        Language::Portuguese => {
            format!("nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{nome}'")
        }
    }
}

/// Localized message for `description_exceeds`.
pub fn description_exceeds(max: usize) -> String {
    match current() {
        Language::English => format!("description must be <= {max} chars"),
        Language::Portuguese => format!("descrição deve ter no máximo {max} caracteres"),
    }
}

/// Localized message for `body_exceeds`.
pub fn body_exceeds(max: usize) -> String {
    match current() {
        Language::English => format!("body exceeds {max} bytes"),
        Language::Portuguese => format!("corpo excede {max} bytes"),
    }
}

/// Localized message for `new_name_length`.
pub fn new_name_length(max: usize) -> String {
    match current() {
        Language::English => format!("new-name must be 1-{max} chars"),
        Language::Portuguese => format!("novo nome deve ter entre 1 e {max} caracteres"),
    }
}

/// Localized message for `new_name_kebab`.
pub fn new_name_kebab(nome: &str) -> String {
    match current() {
        Language::English => format!(
            "new-name must be kebab-case slug (lowercase letters, digits, hyphens): '{nome}'"
        ),
        Language::Portuguese => format!(
            "novo nome deve estar em kebab-case (minúsculas, dígitos, hífens): '{nome}'"
        ),
    }
}

/// Localized message for `namespace_length`.
pub fn namespace_length() -> String {
    match current() {
        Language::English => "namespace must be 1-80 chars".to_string(),
        Language::Portuguese => "namespace deve ter entre 1 e 80 caracteres".to_string(),
    }
}

/// Localized message for `namespace_format`.
pub fn namespace_format() -> String {
    match current() {
        Language::English => "namespace must be alphanumeric + hyphens/underscores".to_string(),
        Language::Portuguese => {
            "namespace deve ser alfanumérico com hífens/sublinhados".to_string()
        }
    }
}

/// Localized message for `path_traversal`.
pub fn path_traversal(p: &str) -> String {
    match current() {
        Language::English => format!("path traversal rejected: {p}"),
        Language::Portuguese => format!("traversal de caminho rejeitado: {p}"),
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

/// Localized message for `invalid_tz`.
pub fn invalid_tz(v: &str) -> String {
    match current() {
        Language::English => format!(
            "display.tz invalid: '{v}'; use an IANA name like 'America/Sao_Paulo'"
        ),
        Language::Portuguese => format!(
            "display.tz inválido: '{v}'; use um nome IANA como 'America/Sao_Paulo'"
        ),
    }
}

/// Localized message for `empty_query`.
pub fn empty_query() -> String {
    match current() {
        Language::English => "query cannot be empty".to_string(),
        Language::Portuguese => "a consulta não pode estar vazia".to_string(),
    }
}

/// Localized message for `empty_body`.
pub fn empty_body() -> String {
    match current() {
        Language::English => "body cannot be empty: provide --body, --body-file, or --body-stdin with content, or supply a graph via --entities-file/--graph-stdin".to_string(),
        Language::Portuguese => "o corpo não pode estar vazio: forneça --body, --body-file ou --body-stdin com conteúdo, ou um grafo via --entities-file/--graph-stdin".to_string(),
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

/// Localized message for `self_referential_link`.
pub fn self_referential_link() -> String {
    match current() {
        Language::English => "--from and --to must be different entities — self-referential relationships are not supported".to_string(),
        Language::Portuguese => "--from e --to devem ser entidades diferentes — relacionamentos auto-referenciais não são suportados".to_string(),
    }
}

/// Localized message for `invalid_link_weight`.
pub fn invalid_link_weight(weight: f64) -> String {
    match current() {
        Language::English => {
            format!("--weight: must be between 0.0 and 1.0 (actual: {weight})")
        }
        Language::Portuguese => {
            format!("--weight: deve estar entre 0.0 e 1.0 (atual: {weight})")
        }
    }
}

/// Localized message for `sync_destination_equals_source`.
pub fn sync_destination_equals_source() -> String {
    match current() {
        Language::English => {
            "destination path must differ from the source database path".to_string()
        }
        Language::Portuguese => {
            "caminho de destino deve ser diferente do caminho do banco de dados fonte"
                .to_string()
        }
    }
}

// ── R-I18N-01 onda 1: reusable Validation catalog helpers ──────────────
// Prefer these over ad-hoc `AppError::Validation(format!(...))` so EN/PT
// stay in one place. English is the default agent language; PT follows
// `current()` when locale is Portuguese.

/// Generic "missing / required field" helper.
pub fn missing_field(field: &str) -> String {
    match current() {
        Language::English => format!("missing required field: {field}"),
        Language::Portuguese => format!("campo obrigatório ausente: {field}"),
    }
}

/// Directory path does not exist.
pub fn directory_not_found(path: &str) -> String {
    match current() {
        Language::English => format!("directory not found: {path}"),
        Language::Portuguese => format!("diretório não encontrado: {path}"),
    }
}

/// Path exists but is not a directory.
pub fn not_a_directory(path: &str) -> String {
    match current() {
        Language::English => format!("path is not a directory: {path}"),
        Language::Portuguese => format!("caminho não é um diretório: {path}"),
    }
}

/// `--max-files` cap exceeded (generic form used by ingest-claude/codex).
pub fn max_files_exceeded(found: usize, max: usize) -> String {
    match current() {
        Language::English => {
            format!("found {found} files, exceeds --max-files cap of {max}")
        }
        Language::Portuguese => {
            format!("encontrados {found} arquivos, excede o limite --max-files de {max}")
        }
    }
}

/// `--max-files` cap exceeded with pattern-matching wording (ingest).
pub fn max_files_exceeded_matching(found: usize, max: usize) -> String {
    match current() {
        Language::English => format!(
            "found {found} files matching pattern, exceeds --max-files cap of {max} \
             (raise the cap or narrow the pattern)"
        ),
        Language::Portuguese => format!(
            "encontrados {found} arquivos no padrão, excede o limite --max-files de {max} \
             (aumente o limite ou restrinja o padrão)"
        ),
    }
}

/// `--max-files` cap exceeded with all-or-nothing abort (ingest-opencode).
pub fn max_files_exceeded_all_or_nothing(found: usize, max: usize) -> String {
    match current() {
        Language::English => format!(
            "found {found} files exceeding --max-files cap of {max}; aborting (all-or-nothing)"
        ),
        Language::Portuguese => format!(
            "encontrados {found} arquivos excedendo o limite --max-files de {max}; \
             abortando (tudo-ou-nada)"
        ),
    }
}

/// File bytes are not valid UTF-8.
pub fn file_not_utf8(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("file is not valid UTF-8: {err}"),
        Language::Portuguese => format!("arquivo não é UTF-8 válido: {err}"),
    }
}

/// Queue resume (`status=processing` → `pending`) failed.
pub fn queue_resume_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("queue resume failed: {err}"),
        Language::Portuguese => format!("falha ao retomar a fila: {err}"),
    }
}

/// Queue retry-failed reset failed.
pub fn queue_retry_failed_reset_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("queue retry-failed reset failed: {err}"),
        Language::Portuguese => {
            format!("falha ao redefinir itens com falha da fila: {err}")
        }
    }
}

/// Queue clear failed.
pub fn queue_clear_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("queue clear failed: {err}"),
        Language::Portuguese => format!("falha ao limpar a fila: {err}"),
    }
}

/// Queue insert failed.
pub fn queue_insert_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("queue insert failed: {err}"),
        Language::Portuguese => format!("falha ao inserir na fila: {err}"),
    }
}

/// Invalid JSON for a CLI flag that takes a file path (e.g. `--entities-file`).
pub fn invalid_json_in_flag(flag: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("invalid JSON in {flag}: {err}"),
        Language::Portuguese => format!("JSON inválido em {flag}: {err}"),
    }
}

/// Invalid JSON payload on a stdin flag (e.g. `--graph-stdin`).
pub fn invalid_json_payload_on_flag(flag: &str, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("invalid JSON payload on {flag}: {err}"),
        Language::Portuguese => format!("payload JSON inválido em {flag}: {err}"),
    }
}

/// Relation format error scoped to a source→target edge.
pub fn relation_format_for_relationship(
    err: &impl std::fmt::Display,
    source: &str,
    target: &str,
) -> String {
    match current() {
        Language::English => format!("{err} for relationship '{source}' -> '{target}'"),
        Language::Portuguese => {
            format!("{err} para relacionamento '{source}' -> '{target}'")
        }
    }
}

/// Relationship strength outside [0.0, 1.0].
pub fn invalid_relationship_strength(strength: f64, source: &str, target: &str) -> String {
    match current() {
        Language::English => format!(
            "invalid strength {strength} for relationship '{source}' -> '{target}'; \
             expected value in [0.0, 1.0]"
        ),
        Language::Portuguese => format!(
            "força inválida {strength} para relacionamento '{source}' -> '{target}'; \
             valor esperado em [0.0, 1.0]"
        ),
    }
}

/// Ingest aborted on first failure (`--fail-fast` / default all-or-nothing path).
pub fn ingest_aborted_on_first_failure(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("ingest aborted on first failure: {err}"),
        Language::Portuguese => format!("ingest abortado na primeira falha: {err}"),
    }
}

/// Name normalized to empty (only hyphens/underscores/spaces).
pub fn name_empty_after_normalization() -> String {
    match current() {
        Language::English => {
            "name cannot be empty after normalization (input was blank or contained only hyphens/underscores/spaces)"
                .to_string()
        }
        Language::Portuguese => {
            "nome não pode ficar vazio após normalização (entrada em branco ou só com hífens/sublinhados/espaços)"
                .to_string()
        }
    }
}

/// `--strict-name` refused auto-normalization.
pub fn strict_name_not_canonical(original: &str, normalized: &str) -> String {
    match current() {
        Language::English => format!(
            "--strict-name is set but '{original}' is not canonical kebab-case; \
             re-run with --name '{normalized}' (or drop --strict-name to allow auto-normalization)"
        ),
        Language::Portuguese => format!(
            "--strict-name está ativo mas '{original}' não é kebab-case canônico; \
             reexecute com --name '{normalized}' (ou remova --strict-name para permitir auto-normalização)"
        ),
    }
}

/// `--enable-ner` and `--skip-extraction` conflict.
pub fn enable_ner_skip_extraction_exclusive() -> String {
    match current() {
        Language::English => {
            "--enable-ner and --skip-extraction are mutually exclusive; remove one"
                .to_string()
        }
        Language::Portuguese => {
            "--enable-ner e --skip-extraction são mutuamente exclusivos; remova um"
                .to_string()
        }
    }
}

/// CREATE path requires `--type` and `--description`.
pub fn type_and_description_required() -> String {
    match current() {
        Language::English => {
            "--type and --description are required when creating a new memory".to_string()
        }
        Language::Portuguese => {
            "--type e --description são obrigatórios ao criar uma nova memória".to_string()
        }
    }
}

/// External CLI binary missing at an explicit path.
pub fn binary_not_found_at_path(tool: &str, path: &str) -> String {
    match current() {
        Language::English => format!("{tool} binary not found at explicit path: {path}"),
        Language::Portuguese => {
            format!("binário {tool} não encontrado no caminho explícito: {path}")
        }
    }
}

/// Executable not found in PATH.
pub fn executable_not_in_path(binary: &str) -> String {
    match current() {
        Language::English => format!(
            "executable '{binary}' not found in PATH; ensure Codex CLI is installed"
        ),
        Language::Portuguese => format!(
            "executável '{binary}' não encontrado no PATH; certifique-se de que o Codex CLI está instalado"
        ),
    }
}

/// Tool version below the minimum required.
pub fn version_below_minimum(tool: &str, actual: &str, min: &str) -> String {
    match current() {
        Language::English => {
            format!("{tool} version {actual} is below minimum required {min}")
        }
        Language::Portuguese => {
            format!("versão {actual} de {tool} está abaixo do mínimo exigido {min}")
        }
    }
}

/// External process timed out.
pub fn process_timed_out(cmd: &str, secs: u64) -> String {
    match current() {
        Language::English => format!("{cmd} timed out after {secs} seconds"),
        Language::Portuguese => format!("{cmd} expirou após {secs} segundos"),
    }
}

/// External process exited with a non-zero code.
pub fn process_exited(cmd: &str, code: Option<i32>, stderr: &str) -> String {
    match current() {
        Language::English => format!("{cmd} exited with code {code:?}: {stderr}"),
        Language::Portuguese => {
            format!("{cmd} encerrou com código {code:?}: {stderr}")
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

/// HTTP request failed (transport-level).
pub fn http_request_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("HTTP request failed: {err}"),
        Language::Portuguese => format!("requisição HTTP falhou: {err}"),
    }
}

/// Failed to read HTTP response body.
pub fn failed_to_read_response_body(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to read response body: {err}"),
        Language::Portuguese => format!("falha ao ler corpo da resposta: {err}"),
    }
}

/// Failed to parse chat response JSON.
pub fn failed_to_parse_chat_response(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("failed to parse chat response: {err}"),
        Language::Portuguese => format!("falha ao parsear resposta de chat: {err}"),
    }
}

/// OpenRouter returned a non-success status for a model.
pub fn openrouter_status_error(
    status: &impl std::fmt::Display,
    model: &str,
    body: &str,
) -> String {
    match current() {
        Language::English => {
            format!("OpenRouter returned {status} for model '{model}': {body}")
        }
        Language::Portuguese => {
            format!("OpenRouter retornou {status} para o modelo '{model}': {body}")
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

/// Unexpected HTTP status with body snippet.
pub fn unexpected_http_status(status: &impl std::fmt::Display, body: &str) -> String {
    match current() {
        Language::English => format!("unexpected HTTP {status}: {body}"),
        Language::Portuguese => format!("HTTP inesperado {status}: {body}"),
    }
}

/// `--target` only applies to re-embed operation.
pub fn reembed_target_only(target: &str) -> String {
    match current() {
        Language::English => {
            format!("--target {target} only applies to --operation re-embed")
        }
        Language::Portuguese => {
            format!("--target {target} só se aplica a --operation re-embed")
        }
    }
}

/// System load average exceeds 2× ncpus.
pub fn system_load_exceeded(load: f64, ncpus: usize) -> String {
    match current() {
        Language::English => format!(
            "system load average {load:.2} exceeds 2x ncpus ({ncpus}); \
             pass --no-max-load-check to override (not recommended)"
        ),
        Language::Portuguese => format!(
            "carga média do sistema {load:.2} excede 2x ncpus ({ncpus}); \
             passe --no-max-load-check para sobrescrever (não recomendado)"
        ),
    }
}

/// Preflight rate-limit with a distinct fallback provider.
pub fn preflight_rate_limit_fallback(mode: &str, reason: &str, fallback: &str) -> String {
    match current() {
        Language::English => format!(
            "preflight detected rate limit on {mode}: {reason}; \
             re-invoke with `--mode {fallback}` to use the fallback provider"
        ),
        Language::Portuguese => format!(
            "preflight detectou limite de taxa em {mode}: {reason}; \
             reexecute com `--mode {fallback}` para usar o provedor de fallback"
        ),
    }
}

/// Preflight rate-limit when fallback equals current mode.
pub fn preflight_rate_limit_same_mode(mode: &str, reason: &str) -> String {
    match current() {
        Language::English => format!(
            "preflight detected rate limit on {mode}: {reason}; \
             --fallback-mode matches --mode, no recovery possible"
        ),
        Language::Portuguese => format!(
            "preflight detectou limite de taxa em {mode}: {reason}; \
             --fallback-mode coincide com --mode, sem recuperação possível"
        ),
    }
}

/// Preflight rate-limit with operator suggestion (no fallback configured).
pub fn preflight_rate_limit_suggestion(mode: &str, reason: &str, suggestion: &str) -> String {
    match current() {
        Language::English => format!(
            "preflight detected rate limit on {mode}: {reason}; \
             {suggestion}; pass --fallback-mode codex to recover"
        ),
        Language::Portuguese => format!(
            "preflight detectou limite de taxa em {mode}: {reason}; \
             {suggestion}; passe --fallback-mode codex para recuperar"
        ),
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

/// OpenRouter API key could not be resolved.
pub fn openrouter_api_key_not_found() -> String {
    match current() {
        Language::English => {
            "OpenRouter API key not found; store it via \
             `config add-key --provider openrouter`, or pass --openrouter-api-key \
             (product env is deprecated)"
                .to_string()
        }
        Language::Portuguese => {
            "chave de API OpenRouter não encontrada; armazene via \
             `config add-key --provider openrouter`, ou passe --openrouter-api-key \
             (env de produto está depreciada)"
                .to_string()
        }
    }
}

