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
    OR description LIKE '%no additional information%'
    OR description LIKE '%no further information%'
    OR description LIKE '%details are not specified%'
    OR description LIKE '%no specific details%'
    OR description LIKE '%details are unknown%'
    OR description LIKE '%not enough information%'
    OR description LIKE '%insufficient information%'
    OR description LIKE '%no information is available%'
    OR description LIKE '%whose details are%'
    OR description LIKE '%fictional%'
    OR description LIKE '%may refer to%'
    OR description LIKE '%is a placeholder%'
    OR description LIKE '%hypothetical%'
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
    // G-PR-7 — VACUOUS FILLER, measured against the configured chat model when
    // an entity has no evidence. This class is worse than the software
    // boilerplate above: it is not false, so a human skims past it, and it is
    // not NULL, so the entity leaves the normal scan and `--force-redescribe`
    // never reopens it. Without these markers the entity is frozen as noise
    // with no correction path through the CLI at all.
    "no additional information",
    "no further information",
    "details are not specified",
    "no specific details",
    "details are unknown",
    "not enough information",
    "insufficient information",
    "no information is available",
    "whose details are",
    // G-PR-7 — ABSTENCAO DISFARCADA DE RESPOSTA. Measured on a live corpus:
    // 45 `person` entities are described as "a fictional software architect",
    // "a fictional software engineer", "may refer to a ...". The model was
    // LABELLING ITS OWN INVENTION and the store persisted the label, because
    // the schema gave it no way to decline. A description asserting that its
    // subject is fictional or uncertain cannot be grounded in the corpus by
    // construction.
    //
    // Safe despite the obvious false positive (a genuinely fictional
    // character) ONLY because abstention now exists: a re-description either
    // improves from real evidence or returns `sufficient_evidence: false`.
    // Do not port these markers to a build without that gate.
    "fictional",
    "may refer to",
    "is a placeholder",
    "hypothetical",
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
///
/// v1.2.8: the length cut-off is no longer spelled inside the SQL text. As a
/// bare `30` it was a constant only in name — untyped, invisible to a grep for
/// the policy it encodes, and impossible to reuse from the Rust side that
/// reports on the same idea. It now comes from
/// [`crate::constants::ENRICH_GENERIC_DESCRIPTION_MAX_CHARS`], which forced this
/// from a `const &str` into a function, matching
/// [`generic_relation_predicate`] rather than inventing a third shape.
///
/// The value is INTERPOLATED, not bound: it is a compiled-in `usize`, never
/// caller input, so GAP-SG-167 (bind anything an operator can influence) does
/// not apply — and a bound parameter here would collide with the positional
/// numbering of every caller's own `?N` placeholders.
pub(super) fn generic_description_predicate() -> String {
    let max_chars = crate::constants::ENRICH_GENERIC_DESCRIPTION_MAX_CHARS;
    format!(
        "(description LIKE '%ingested%' \
         OR description LIKE '%imported%' OR description LIKE '%added%' \
         OR length(description) < {max_chars})"
    )
}

/// Predicate for entity-descriptions scan (GAP-CLI-ED-06).
///
/// `named` is true when the caller passed an explicit `--names` /
/// `--entity-names` / `--names-file` filter. In that case eligibility is the
/// NAME, not the quality heuristic (G-PR-7): an operator who types the entity
/// has already made the judgement the heuristic exists to approximate, and
/// forcing them through a substring allowlist is what made targeted repair of
/// a fluent-but-wrong description impossible — it matched neither
/// `NULL_DESCRIPTION_PREDICATE` nor any marker, so `matched: 0` was the only
/// possible answer no matter how the operator phrased the command.
pub(super) fn entity_description_scan_predicate(force_redescribe: bool, named: bool) -> String {
    if force_redescribe && named {
        // The name filter is applied by the scanner on top of this predicate.
        "1=1".to_string()
    } else if force_redescribe {
        format!("({NULL_DESCRIPTION_PREDICATE} OR {LOW_QUALITY_DESCRIPTION_PREDICATE})")
    } else {
        NULL_DESCRIPTION_PREDICATE.to_string()
    }
}

/// `weight-calibrate`: relationships strong enough to warrant recalibration.
///
/// Built from [`crate::constants::ENRICH_HIGH_WEIGHT_THRESHOLD`] rather than
/// spelled out. The predicate used to be a `const &str` holding `r.weight >= 0.7`,
/// which is a constant in form and a literal in practice: untyped, ungreppable
/// from the Rust side, and impossible to reuse anywhere the threshold is needed
/// as a number rather than as SQL text.
///
/// This shipped in two steps within v1.2.8 — the function first, with the old
/// const kept for the one caller that was out of scope at the time, and the
/// const removed once `scan::relationships` moved over. The intermediate state
/// was held together by a test asserting the two spellings agreed, which is the
/// right way to carry a two-step change: the drift fails a test rather than
/// quietly letting the scanner and the calibration policy disagree.
pub(super) fn high_weight_predicate() -> String {
    format!(
        "r.weight >= {}",
        crate::constants::ENRICH_HIGH_WEIGHT_THRESHOLD
    )
}

/// `relation-reclassify`: relationships still using the generic `applies-to`.
///
/// v1.2.8: the literal now comes from [`crate::parsers::GENERIC_RELATION`]
/// instead of being spelled here. While it was spelled here, in snake_case, the
/// scanner that finds generic edges to reclassify saw 24 candidates in a
/// database holding 50 346 of them — the repair tool was blind to the same 95%
/// of the graph the read filters were, and for the same reason.
pub(super) fn generic_relation_predicate() -> String {
    format!("r.relation = '{}'", crate::parsers::GENERIC_RELATION)
}

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

    /// The threshold must reach the SQL from the constant, not from a number
    /// retyped inside the string.
    ///
    /// This replaces an earlier assertion that compared two spellings of the
    /// same predicate while both existed. The second spelling is gone, so the
    /// thing worth pinning is no longer that they agree — it is that the one
    /// remaining spelling is derived rather than typed.
    #[test]
    fn high_weight_predicate_uses_constant_threshold() {
        let sql = high_weight_predicate();
        assert!(
            sql.contains(&format!(
                "r.weight >= {}",
                crate::constants::ENRICH_HIGH_WEIGHT_THRESHOLD
            )),
            "predicate must be built from the constant: {sql}"
        );
    }

    /// The length cut-off must reach the SQL from the constant, not from a
    /// number retyped inside the string.
    #[test]
    fn generic_description_predicate_uses_constant_cutoff() {
        let sql = generic_description_predicate();
        assert!(sql.contains(&format!(
            "length(description) < {}",
            crate::constants::ENRICH_GENERIC_DESCRIPTION_MAX_CHARS
        )));
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
