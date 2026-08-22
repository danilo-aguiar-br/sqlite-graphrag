//! Messages about GRAPH edges and their endpoints (GAP-SG-146).
//!
//! Relation vocabulary, edge weights and the self-reference guard.

use crate::i18n::{current, Language};

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

/// Invalid relationship id.
pub fn invalid_relationship_id(item_key: &str) -> String {
    match current() {
        Language::English => format!("invalid relationship id: {item_key}"),
        Language::Portuguese => format!("id de relacionamento inválido: {item_key}"),
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

/// Non-canonical relation under --strict-relations.
pub fn non_canonical_relation(relation: &str, allowed: &str) -> String {
    match current() {
        Language::English => format!(
            "non-canonical relation '{relation}': use --strict-relations=false or choose from: {allowed}"
        ),
        Language::Portuguese => format!(
            "relação não canônica '{relation}': use --strict-relations=false ou escolha entre: {allowed}"
        ),
    }
}

/// Batch reclassification whose `--from-type` matches no entity at all.
///
/// v1.2.8: with a closed enum, a typo in `--from-type` was refused by clap. The
/// vocabulary is open now, so the same typo would parse, match nothing, and
/// report success over zero rows. Counting the targets first turns that silent
/// no-op back into a refusal.
pub fn reclassify_batch_no_targets(from_type: &str, namespace: &str) -> String {
    match current() {
        Language::English => format!(
            "--from-type '{from_type}' matches no entity in namespace '{namespace}'; \
             list the stored types with `graph entities` before retrying"
        ),
        Language::Portuguese => format!(
            "--from-type '{from_type}' não corresponde a nenhuma entidade no namespace '{namespace}'; \
             liste os tipos gravados com `graph entities` antes de repetir"
        ),
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

/// Localized message for `self_referential_link`.
pub fn self_referential_link() -> String {
    match current() {
        Language::English => "--from and --to must be different entities — self-referential relationships are not supported".to_string(),
        Language::Portuguese => "--from e --to devem ser entidades diferentes — relacionamentos auto-referenciais não são suportados".to_string(),
    }
}
