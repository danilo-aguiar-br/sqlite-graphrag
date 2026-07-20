//! Enrichment LLM prompts (GAP-CLI-ED-01, ED-05, G-T-SPLIT-01).
//!
//! Prompt text lives here so `mod.rs` stays focused on CLI dispatch and so
//! domain bias can be reviewed without scrolling a 3k-line monolith.

/// Neutral multi-domain entity-description prompt (GAP-CLI-ED-01).
/// Domain bias must come from linked memory corpus snippets and optional
/// operator domain config — never from a hard-coded software frame.
pub(crate) const ENTITY_DESCRIPTION_PROMPT_PREFIX: &str = "You are a knowledge graph annotator. Given an entity name, type, and optional corpus evidence from linked memories, write a concise one-sentence description (10-20 words) that explains what this entity IS and WHY it matters in its real domain. Use only facts supported by the provided evidence. If evidence is missing, stay conservative and describe the entity by its name and type without inventing a software, product, or configuration-file frame.\n\nEntity name: ";

/// GAP-CLI-ED-05: optional operator domain hint injected into the ED prompt.
/// `auto` / empty / `none` → no domain section.
pub(crate) fn entity_description_domain_section(domain: &str) -> String {
    let d = domain.trim();
    if d.is_empty() || d.eq_ignore_ascii_case("auto") || d.eq_ignore_ascii_case("none") {
        return String::new();
    }
    format!(
        "Operator domain hint: {d}. Prefer facts consistent with this domain when supported by evidence; do not invent details outside the evidence.\n\n"
    )
}

/// Resolve domain: CLI flag > XDG > `auto`.
pub(crate) fn resolve_entity_description_domain(cli: &str) -> String {
    let c = cli.trim();
    if !c.is_empty() && !c.eq_ignore_ascii_case("auto") {
        return c.to_string();
    }
    crate::config::get_setting("enrich.entity_description.domain")
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "auto".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_section_empty_for_auto() {
        assert!(entity_description_domain_section("auto").is_empty());
        assert!(entity_description_domain_section("none").is_empty());
        assert!(entity_description_domain_section("").is_empty());
    }

    #[test]
    fn domain_section_includes_label() {
        let s = entity_description_domain_section("fiscal");
        assert!(s.contains("fiscal"));
        assert!(s.contains("Operator domain hint"));
    }
}
