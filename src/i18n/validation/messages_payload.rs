//! Messages about PAYLOAD size and shape (GAP-SG-146).
//!
//! Body and description ceilings, empty inputs, batch line contracts, file
//! caps and the host load guard.

use crate::i18n::{current, Language};

/// Localized message for `body_exceeds`.
pub fn body_exceeds(max: usize) -> String {
    match current() {
        Language::English => format!("body exceeds {max} bytes"),
        Language::Portuguese => format!("corpo excede {max} bytes"),
    }
}

/// Localized message for `description_exceeds`.
pub fn description_exceeds(max: usize) -> String {
    match current() {
        Language::English => format!("description must be <= {max} chars"),
        Language::Portuguese => format!("descrição deve ter no máximo {max} caracteres"),
    }
}

/// Localized message for `empty_body`.
pub fn empty_body() -> String {
    match current() {
        Language::English => "body cannot be empty: provide --body, --body-file, or --body-stdin with content, or supply a graph via --entities-file/--graph-stdin".to_string(),
        Language::Portuguese => "o corpo não pode estar vazio: forneça --body, --body-file ou --body-stdin com conteúdo, ou um grafo via --entities-file/--graph-stdin".to_string(),
    }
}

/// Localized message for `empty_query`.
pub fn empty_query() -> String {
    match current() {
        Language::English => "query cannot be empty".to_string(),
        Language::Portuguese => "a consulta não pode estar vazia".to_string(),
    }
}

/// File bytes are not valid UTF-8.
pub fn file_not_utf8(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("file is not valid UTF-8: {err}"),
        Language::Portuguese => format!("arquivo não é UTF-8 válido: {err}"),
    }
}

/// Invalid memory source string.
pub fn invalid_memory_source(other: &str, expected: &str) -> String {
    match current() {
        Language::English => {
            format!("invalid memory source: {other}; expected one of {expected}")
        }
        Language::Portuguese => {
            format!("fonte de memória inválida: {other}; esperado um de {expected}")
        }
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

/// Batch line: invalid JSON.
pub fn batch_line_invalid_json(index: usize, err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("line {index}: invalid JSON: {err}"),
        Language::Portuguese => format!("linha {index}: JSON inválido: {err}"),
    }
}

/// Batch line: name normalizes empty.
pub fn batch_line_name_empty(index: usize) -> String {
    match current() {
        Language::English => format!("line {index}: name normalizes to empty string"),
        Language::Portuguese => {
            format!("linha {index}: nome normaliza para string vazia")
        }
    }
}

/// Batch line: type/description required.
pub fn batch_line_type_description_required(index: usize) -> String {
    match current() {
        Language::English => format!(
            "line {index}: --type and --description are required when creating a new memory"
        ),
        Language::Portuguese => format!(
            "linha {index}: --type e --description são obrigatórios ao criar uma nova memória"
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
