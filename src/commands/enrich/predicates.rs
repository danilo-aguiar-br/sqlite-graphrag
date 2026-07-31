//! Shared SQL WHERE predicates for enrich scanners and backlog counters.
//!
//! GAP-SG-77: each operation-specific predicate lives in ONE place so the
//! scanner and `count_operation_backlog` cannot drift.

/// `memory-bindings`: memories with zero `memory_entities` rows.
pub(super) const UNBOUND_MEMORY_PREDICATE: &str =
    "NOT EXISTS (SELECT 1 FROM memory_entities me WHERE me.memory_id = m.id)";

/// `entity-descriptions`: entities whose description is NULL or empty.
pub(super) const NULL_DESCRIPTION_PREDICATE: &str = "(description IS NULL OR description = '')";

/// GAP-CLI-ED-06 / ED-LQ-01 / G-PR-1 (v1.2.0): high-precision SQL prefilter for
/// `--force-redescribe` low-quality candidates.
///
/// Intentionally short: broad fragments like `%software architecture%` or
/// `%is a software%` caused false positives on legitimate domain prose
/// (e.g. "software architecture decision for auth"). Full decision logic is
/// [`is_low_quality_description`], applied after the SQL scan.
/// CAPA-D (2026-07-30): compound markers only — bare `%configuration file%`
/// false-positived legitimate domain prose (e.g. clippy.toml lint config).
pub(super) const LOW_QUALITY_DESCRIPTION_PREDICATE: &str = "(
    description LIKE '%is a software component%'
    OR description LIKE '%is a software system%'
    OR description LIKE '%is a software application%'
    OR description LIKE '%in the software project%'
    OR description LIKE '%in the context of software%'
    OR description LIKE '%of the software system%'
    OR description LIKE '%in the software system%'
    OR description LIKE '%software/system design%'
    OR description LIKE '%is a configuration file%'
    OR description LIKE '%a configuration file used%'
    OR description LIKE '%generic configuration file%'
    OR description LIKE '%configuration file in the software%'
    OR description LIKE '%enhances chatbot%'
    OR description LIKE '%chatbot response%'
    OR description LIKE '%European digital identity%'
)";

/// High-precision boilerplate substrings (lowercase) used by the Rust
/// post-filter. Keep aligned with [`LOW_QUALITY_DESCRIPTION_PREDICATE`].
const LOW_QUALITY_MARKERS: &[&str] = &[
    "is a software component",
    "is a software system",
    "is a software application",
    "in the software project",
    "in the context of software",
    "of the software system",
    "in the software system",
    "software/system design",
    // CAPA-D: compound only — not bare "configuration file"
    "is a configuration file",
    "a configuration file used",
    "generic configuration file",
    "configuration file in the software",
    "enhances chatbot",
    "chatbot response",
    "european digital identity",
];

/// G-PR-2: Rust post-filter for entity-description force-redescribe.
///
/// Returns `true` when `desc` matches high-precision boilerplate patterns.
/// Empty/whitespace-only descriptions are treated as low quality so callers
/// can use a single gate; the SQL path still handles NULL/empty separately.
///
/// # Examples of intent
/// - `"X is a software component in the software project"` → true (boilerplate)
/// - `"software architecture decision for auth"` → false (legitimate prose)
pub(crate) fn is_low_quality_description(desc: &str) -> bool {
    let d = desc.trim();
    if d.is_empty() {
        return true;
    }
    let lower = d.to_ascii_lowercase();
    LOW_QUALITY_MARKERS.iter().any(|m| lower.contains(m))
}

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

/// `re-embed --target memories`: memory `m` lacks a live vector at the target dim.
///
/// CAPA (dim-migrate 2026-07-30): eligibility uses `LENGTH(embedding) = dim*4`
/// (BLOB truth), not the `dim` column alone. Rows with `dim=1024` but a 384-d
/// BLOB (CORRUPT / META_AHEAD) must remain selectable.
pub(super) fn reembed_memory_predicate(dim: usize) -> String {
    let bytes = dim * 4;
    format!(
        "NOT EXISTS (SELECT 1 FROM memory_embeddings me WHERE me.memory_id = m.id \
         AND LENGTH(me.embedding) = {bytes})"
    )
}

/// `re-embed --target entities`: entity `e` lacks a live vector at the target dim.
pub(super) fn reembed_entity_predicate(dim: usize) -> String {
    let bytes = dim * 4;
    format!(
        "NOT EXISTS (SELECT 1 FROM entity_embeddings ev WHERE ev.entity_id = e.id \
         AND LENGTH(ev.embedding) = {bytes})"
    )
}

/// `re-embed --target chunks`: chunk `c` lacks a live vector at the target dim.
pub(super) fn reembed_chunk_predicate(dim: usize) -> String {
    let bytes = dim * 4;
    format!(
        "NOT EXISTS (SELECT 1 FROM chunk_embeddings ce WHERE ce.chunk_id = c.id \
         AND LENGTH(ce.embedding) = {bytes})"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boilerplate_software_component_is_low_quality() {
        assert!(is_low_quality_description(
            "Foo is a software component in the software project"
        ));
    }

    #[test]
    fn legitimate_architecture_decision_is_not_low_quality() {
        // G-PR-1 FP case: broad `%software architecture%` must not match.
        assert!(!is_low_quality_description(
            "software architecture decision for auth"
        ));
    }

    #[test]
    fn software_development_process_is_not_low_quality() {
        assert!(!is_low_quality_description(
            "Documents the software development process for the billing team"
        ));
    }

    #[test]
    fn empty_description_is_low_quality() {
        assert!(is_low_quality_description(""));
        assert!(is_low_quality_description("   "));
    }

    #[test]
    fn chatbot_boilerplate_is_low_quality() {
        assert!(is_low_quality_description(
            "Module that enhances chatbot response quality"
        ));
    }

    #[test]
    fn configuration_file_boilerplate_is_low_quality() {
        // Compound boilerplate still matches (CAPA-D).
        assert!(is_low_quality_description(
            "Foo is a configuration file used by the build system"
        ));
        assert!(is_low_quality_description(
            "Bar is a configuration file in the software project"
        ));
        assert!(is_low_quality_description(
            "A generic configuration file placeholder"
        ));
    }

    /// CAPA-D: legitimate domain prose that merely contains the words
    /// "configuration file" must not be force-redescribe fodder.
    #[test]
    fn legitimate_configuration_file_prose_is_not_low_quality() {
        assert!(!is_low_quality_description(
            "clippy-toml is a Rust lint configuration file that shapes code quality standards and design consistency"
        ));
        assert!(!is_low_quality_description(
            "TOML configuration file for Clippy lints in this workspace"
        ));
    }

    #[test]
    fn specific_domain_prose_is_not_low_quality() {
        assert!(!is_low_quality_description(
            "OAuth2 token refresh endpoint used by the mobile client"
        ));
    }

    /// G-PR-2: scan predicate is the same gate that blocks `done` persistence.
    #[test]
    fn quality_post_filter_rejects_boilerplate_done_candidates() {
        let bad = "Foo is a software component in the software project";
        assert!(is_low_quality_description(bad));
        let good = "ICMS P05 rule for NFC-e ordered invoice sequences in Brazilian state tax";
        assert!(!is_low_quality_description(good));
    }
}
