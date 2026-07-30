//! Graph payload parsing and validation for `remember --graph-stdin` / `--graph-file`.
//!
//! Holds the curated `{body, entities, relationships}` wire shape and the
//! relation-format / strength guards applied before persistence.

use crate::errors::AppError;
use crate::storage::entities::{NewEntity, NewRelationship};
use serde::Deserialize;

/// Curated graph payload accepted on `--graph-stdin` / `--graph-file`
/// (and the optional body field when no explicit body source is given).
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct GraphInput {
    #[serde(default)]
    pub(super) body: Option<String>,
    #[serde(default)]
    pub(super) entities: Vec<NewEntity>,
    #[serde(default)]
    pub(super) relationships: Vec<NewRelationship>,
}

/// Normalize relation labels and validate strength / format for every edge.
pub(super) fn normalize_and_validate_graph_input(graph: &mut GraphInput) -> Result<(), AppError> {
    for rel in &mut graph.relationships {
        rel.relation = crate::parsers::normalize_relation(&rel.relation);
        if let Err(e) = crate::parsers::validate_relation_format(&rel.relation) {
            return Err(AppError::Validation(
                crate::i18n::validation::relation_format_for_relationship(
                    &e,
                    &rel.source,
                    &rel.target,
                ),
            ));
        }
        crate::parsers::warn_if_non_canonical(&rel.relation);
        if !(0.0..=1.0).contains(&rel.strength) {
            return Err(AppError::Validation(
                crate::i18n::validation::invalid_relationship_strength(
                    rel.strength,
                    &rel.source,
                    &rel.target,
                ),
            ));
        }
    }

    Ok(())
}
