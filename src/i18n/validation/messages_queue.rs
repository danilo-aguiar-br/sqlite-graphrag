//! Messages from the enrich QUEUE and its sidecar (GAP-SG-146).
//!
//! Sidecar write failures, re-embed key shapes, and the fields an enrich LLM
//! response must carry for the item to be persisted.

use crate::i18n::{current, Language};

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

/// Queue namespace migration failed.
pub fn queue_namespace_migration_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("queue namespace migration failed: {err}"),
        Language::Portuguese => {
            format!("migração de namespace da fila falhou: {err}")
        }
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

/// Requeue-dead failed.
pub fn requeue_dead_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("requeue-dead failed: {err}"),
        Language::Portuguese => format!("requeue-dead falhou: {err}"),
    }
}

/// Requeue-skipped failed.
pub fn requeue_skipped_failed(err: &impl std::fmt::Display) -> String {
    match current() {
        Language::English => format!("requeue-skipped failed: {err}"),
        Language::Portuguese => format!("requeue-skipped falhou: {err}"),
    }
}

/// Invalid chunk id in re-embed key.
pub fn invalid_chunk_id_in_reembed_key(chunk_key: &str) -> String {
    match current() {
        Language::English => format!("invalid chunk id in re-embed key: {chunk_key}"),
        Language::Portuguese => {
            format!("id de chunk inválido na chave de re-embed: {chunk_key}")
        }
    }
}

/// LLM result missing calibrated_weight.
pub fn llm_missing_calibrated_weight() -> String {
    match current() {
        Language::English => "LLM result missing 'calibrated_weight'".to_string(),
        Language::Portuguese => "resultado LLM sem 'calibrated_weight'".to_string(),
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

/// LLM result missing relation.
pub fn llm_missing_relation() -> String {
    match current() {
        Language::English => "LLM result missing 'relation'".to_string(),
        Language::Portuguese => "resultado LLM sem 'relation'".to_string(),
    }
}

/// Deep-research output file is empty.
pub fn deep_research_output_empty(path: &str) -> String {
    match current() {
        Language::English => {
            format!("deep-research --output failed: written file is empty (0 bytes): {path}")
        }
        Language::Portuguese => {
            format!("deep-research --output falhou: arquivo escrito está vazio (0 bytes): {path}")
        }
    }
}

/// Deep-research output path missing after atomic write.
pub fn deep_research_output_missing(path: &str) -> String {
    match current() {
        Language::English => {
            format!("deep-research --output failed: path does not exist after atomic write: {path}")
        }
        Language::Portuguese => format!(
            "deep-research --output falhou: caminho não existe após escrita atômica: {path}"
        ),
    }
}
