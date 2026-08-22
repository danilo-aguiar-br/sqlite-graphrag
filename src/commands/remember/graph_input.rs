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
    /// Declared `entity_type` labels outside the canonical set, phrased for the
    /// response `warnings` array.
    ///
    /// Filled by [`collect_noncanonical_entity_types`], never by serde. Until
    /// v1.2.8 this field carried folds, and its own doc explained that serde had
    /// already destroyed the caller's string before any layer could read it —
    /// which was precisely the defect. The label now survives deserialization
    /// intact, so what is reported here is advisory: the write went through with
    /// the label as written.
    #[serde(skip)]
    pub(super) type_warnings: Vec<String>,
}

/// Reports every declared `entity_type` outside [`CANONICAL_ENTITY_TYPES`].
///
/// Until v1.2.8 this answered a different question — "which labels did the
/// canonicalisation replace?" — because any label outside the canonical
/// thirteen was folded onto the nearest kind, and onto `concept` when nothing
/// was near (GAP-SG-47). Nothing is folded any more: the label is stored as
/// written, so the only thing left worth saying is that it is not part of the
/// recommended vocabulary. The channel stays because the caller still benefits
/// from learning that `stakeholder` and `pyramid-level` are labels only this
/// payload uses, in the turn where they can still align them.
///
/// Reads the raw payload rather than the typed value so both wire shapes and
/// the `type` alias are covered from one place, and so `--strict-entity-types`
/// and the warning always agree. The cost is one `serde_json` pass over at most
/// `limits.max_entities_per_memory` records, on a path that is about to make an
/// embedding round trip.
///
/// Labels whose *shape* is invalid are skipped here; refusing them is
/// [`crate::entity_type::normalize_entity_type`]'s job at the write boundary,
/// and reporting them twice would say the same thing in two voices.
pub(crate) fn collect_noncanonical_entity_types(raw: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    // Accepts both wire shapes: the `{entities: [...]}` envelope of
    // `--graph-stdin` / `--graph-file`, and the bare array of `--entities-file`.
    let entities = match value.get("entities").or(Some(&value)) {
        Some(serde_json::Value::Array(items)) => items,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for item in entities {
        let Some(declared) = item
            .get("entity_type")
            .or_else(|| item.get("type"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        // Test the normalised form so `Issue-Tracker` counts as canonical
        // rather than as a new label; only a genuinely unknown word warns.
        let Ok(normalized) = crate::entity_type::normalize_entity_type(declared) else {
            continue;
        };
        if !crate::entity_type::is_canonical_entity_type(&normalized) {
            let name = item
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?");
            // Phrased WITHOUT a verdict: the same sentence feeds the response
            // `warnings` (where the label was accepted) and the
            // `--strict-entity-types` refusal (where it was not). Stating the
            // outcome here made the refusal message announce that the value
            // "was accepted as written" while rejecting it.
            out.push(format!(
                "entity_type '{declared}' on entity '{name}' is outside the canonical set"
            ));
        }
    }
    out
}

/// Normalize entity type and relation labels, validating both before persistence.
///
/// The entity pass mirrors what the relation pass has always done: normalise
/// shape, then refuse what no vocabulary could accept. It runs here so every
/// graph source — `--graph-stdin`, `--graph-file`, `--entities-file` — reaches
/// the write path with labels already in storage form, and so a malformed one
/// becomes a validation exit code instead of a stored value nobody can query.
pub(super) fn normalize_and_validate_graph_input(graph: &mut GraphInput) -> Result<(), AppError> {
    for entity in &mut graph.entities {
        entity.entity_type = crate::entity_type::normalize_entity_type(&entity.entity_type)?;
    }

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

#[cfg(test)]
mod noncanonical_entity_type_visibility {
    use super::collect_noncanonical_entity_types;

    /// A label outside the canonical set must be reported — and kept.
    ///
    /// The warning names the label the caller wrote, which is now also the
    /// label the graph holds. Before v1.2.8 the same payload was rewritten to
    /// `concept` in silence.
    #[test]
    fn reports_labels_outside_the_canonical_set() {
        let raw = r#"{"entities":[
            {"name":"diretoria","entity_type":"stakeholder","description":null},
            {"name":"maturidade","entity_type":"pyramid-level","description":null}
        ]}"#;
        let warnings = collect_noncanonical_entity_types(raw);
        assert_eq!(
            warnings.len(),
            2,
            "both labels must be reported: {warnings:?}"
        );
        assert!(warnings[0].contains("stakeholder"));
        assert!(warnings[0].contains("diretoria"));
        assert!(
            !warnings[0].contains("folded"),
            "nothing is folded any more: {warnings:?}"
        );
    }

    /// A canonical label is not news, in either casing or separator.
    ///
    /// Warning on `Issue-Tracker` would train the caller to ignore the channel,
    /// which is worse than not having it.
    #[test]
    fn stays_silent_for_canonical_labels() {
        let raw = r#"{"entities":[
            {"name":"alice","entity_type":"person","description":null},
            {"name":"jira","entity_type":"Issue-Tracker","description":null},
            {"name":"repo","entity_type":"issue_tracker","description":null}
        ]}"#;
        assert!(
            collect_noncanonical_entity_types(raw).is_empty(),
            "canonical labels must not warn"
        );
    }

    /// `--entities-file` sends a bare array, not the `{entities: [...]}` envelope.
    #[test]
    fn accepts_the_bare_array_wire_shape() {
        let raw = r#"[{"name":"nome-exemplo","type":"value-element","description":null}]"#;
        let warnings = collect_noncanonical_entity_types(raw);
        assert_eq!(warnings.len(), 1, "the `type` alias must be read too");
        assert!(warnings[0].contains("value-element"));
    }

    /// Shape rejection belongs to the write boundary, not to this reporter.
    #[test]
    fn ignores_labels_whose_shape_is_invalid() {
        let raw = r#"[{"name":"x","entity_type":"   ","description":null}]"#;
        assert!(collect_noncanonical_entity_types(raw).is_empty());
    }

    /// Unparseable input is the caller's problem, reported by the real parser.
    #[test]
    fn never_fails_on_malformed_input() {
        assert!(collect_noncanonical_entity_types("not json").is_empty());
        assert!(collect_noncanonical_entity_types("").is_empty());
    }
}
