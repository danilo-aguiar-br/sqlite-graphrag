//! Shared SQL WHERE predicates for enrich scanners and backlog counters.
//!
//! GAP-SG-77: each operation-specific predicate lives in ONE place so the
//! scanner and `count_operation_backlog` cannot drift.

/// `memory-bindings`: memories with zero `memory_entities` rows.
pub(super) const UNBOUND_MEMORY_PREDICATE: &str =
    "NOT EXISTS (SELECT 1 FROM memory_entities me WHERE me.memory_id = m.id)";

/// `entity-descriptions`: entities whose description is NULL or empty.
pub(super) const NULL_DESCRIPTION_PREDICATE: &str = "(description IS NULL OR description = '')";

/// GAP-CLI-ED-06 / ED-LQ-01 (v1.1.8): default low-quality detectors for
/// `--force-redescribe`. Grounding score sampling complements LIKE.
pub(super) const LOW_QUALITY_DESCRIPTION_PREDICATE: &str = "(
    description LIKE '%configuration file%'
    OR description LIKE '%software/system design%'
    OR description LIKE '%system design%'
    OR description LIKE '%software design%'
    OR description LIKE '%software application%'
    OR description LIKE '%in the context of software%'
    OR description LIKE '%software project%'
    OR description LIKE '%software system%'
    OR description LIKE '%software component%'
    OR description LIKE '%software architecture%'
    OR description LIKE '%software development%'
    OR description LIKE '%software engineering%'
    OR description LIKE '%enhances chatbot%'
    OR description LIKE '%chatbot response%'
    OR description LIKE '%is a software%'
    OR description LIKE '%of the software%'
    OR description LIKE '%European digital identity%'
    OR description LIKE '%in the software%'
)";

/// `body-enrich`: memory body shorter than the `?2` character threshold.
pub(super) const SHORT_BODY_PREDICATE: &str = "LENGTH(COALESCE(m.body,'')) < ?2";

/// `description-enrich`: memories with generic/auto-generated descriptions.
pub(super) const GENERIC_DESCRIPTION_PREDICATE: &str = "(description LIKE '%ingested%' \
     OR description LIKE '%imported%' OR description LIKE '%added%' \
     OR length(description) < 30)";

/// Predicate for entity-descriptions scan (GAP-CLI-ED-06).
pub(super) fn entity_description_scan_predicate(force_redescribe: bool) -> String {
    if force_redescribe {
        format!("({NULL_DESCRIPTION_PREDICATE} OR {LOW_QUALITY_DESCRIPTION_PREDICATE})")
    } else {
        NULL_DESCRIPTION_PREDICATE.to_string()
    }
}

/// `weight-calibrate`: relationships strong enough to warrant recalibration.
pub(super) const HIGH_WEIGHT_PREDICATE: &str = "r.weight >= 0.7";

/// `relation-reclassify`: relationships still using the generic `applies_to`.
pub(super) const GENERIC_RELATION_PREDICATE: &str = "r.relation = 'applies_to'";

/// `re-embed --target memories`: memory `m` lacks a live vector.
pub(super) fn reembed_memory_predicate(dim: usize) -> String {
    format!(
        "NOT EXISTS (SELECT 1 FROM memory_embeddings me WHERE me.memory_id = m.id \
         AND me.dim = {dim} AND LENGTH(me.embedding) > 0)"
    )
}

/// `re-embed --target entities`: entity `e` lacks a live vector.
pub(super) fn reembed_entity_predicate(dim: usize) -> String {
    format!(
        "NOT EXISTS (SELECT 1 FROM entity_embeddings ev WHERE ev.entity_id = e.id \
         AND ev.dim = {dim} AND LENGTH(ev.embedding) > 0)"
    )
}

/// `re-embed --target chunks`: chunk `c` lacks a live vector.
pub(super) fn reembed_chunk_predicate(dim: usize) -> String {
    format!(
        "NOT EXISTS (SELECT 1 FROM chunk_embeddings ce WHERE ce.chunk_id = c.id \
         AND ce.dim = {dim} AND LENGTH(ce.embedding) > 0)"
    )
}
