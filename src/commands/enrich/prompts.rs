//! Enrichment LLM prompts (GAP-CLI-ED-01, ED-05, G-T-SPLIT-01).
//!
//! Prompt text lives here so `mod.rs` stays focused on CLI dispatch and so
//! domain bias can be reviewed without scrolling a 3k-line monolith.

/// Neutral multi-domain entity-description policy (GAP-CLI-ED-01).
///
/// Policy ONLY — the entity and its evidence travel in the `user` message
/// built by [`entity_description_user_text`]. The previous shape packed
/// everything into `system` and sent no `user` message at all, because
/// `chat_api/client.rs` only appends `user` when `input_text` is non-empty.
/// Measured against `deepseek/deepseek-v4-flash`, that construction returned
/// a bare end-of-sequence token, which the caller then reported as a missing
/// `description` field and routed to dead-letter.
///
/// Domain bias must come from the linked memory corpus and the optional
/// operator domain hint — never from a hard-coded software frame.
pub(crate) const ENTITY_DESCRIPTION_SYSTEM_PROMPT: &str = "You are a knowledge graph annotator. From the entity and the evidence in the user message, write a concise one-sentence description (10-20 words) explaining what the entity IS and WHY it matters in its real domain.\n\n\
Grounding rules:\n\
- Use ONLY facts stated in the provided evidence.\n\
- If the evidence does not support any factual statement about this entity, set `sufficient_evidence` to false and `description` to null. Abstaining is the correct answer, never a failure.\n\
- NEVER infer a profession, nationality, employer or biography from a personal name. A name alone is not evidence.\n\
- NEVER fall back to a software, product, framework or configuration-file frame when the evidence does not mention one.\n\
- Write the description in the SAME language as the evidence.";

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

/// Builds the `user` message: the entity under annotation and its evidence.
///
/// Kept separate from the policy prompt so the model receives the material to
/// process as content rather than as persona, which is what the `system` role
/// means to a chat model.
pub(crate) fn entity_description_user_text(
    entity_name: &str,
    entity_type: &str,
    domain_section: &str,
    corpus_section: &str,
) -> String {
    format!(
        "Entity name: {entity_name}\nEntity type: {entity_type}\n\n{domain_section}{corpus_section}"
    )
}

/// Renders the two endpoints of an edge, with their descriptions when stored.
///
/// GAP-SG-279 (class): `weight-calibrate` and `relation-reclassify` both write
/// to `relationships` after being shown nothing but two entity NAMES and the
/// label under dispute. That is the same defect `entity-type-validate` carried
/// — a judgement made from spelling, persisted as if it were an audit — and it
/// went unnoticed for the same reason: their `format!` calls looked like every
/// other one in the module.
///
/// An edge has no body of its own, so the cheapest genuine evidence available
/// is what the graph already stores ABOUT its endpoints. Both descriptions ride
/// along on the join that was running anyway, which is why this costs one extra
/// column each and no extra query.
///
/// A missing description is omitted rather than rendered as an empty field: an
/// empty "Description:" tells the model the entity has one that says nothing,
/// which is a different claim from having none.
pub(crate) fn edge_endpoints_section(
    source_name: &str,
    source_description: Option<&str>,
    target_name: &str,
    target_description: Option<&str>,
) -> String {
    let mut text = String::with_capacity(128);
    text.push_str(&format!("Source entity: {source_name}\n"));
    if let Some(d) = source_description.map(str::trim).filter(|d| !d.is_empty()) {
        text.push_str(&format!("Source description: {d}\n"));
    }
    text.push_str(&format!("Target entity: {target_name}\n"));
    if let Some(d) = target_description.map(str::trim).filter(|d| !d.is_empty()) {
        text.push_str(&format!("Target description: {d}\n"));
    }
    text
}

/// Builds the `user` message for `entity-type-validate` (GAP-SG-279).
///
/// The operation used to send `format!("Entity: {name}\nCurrent type: {type}")`
/// straight from the extraction module — two lines, assembled at the call site,
/// with no evidence in them. Moving the construction here puts it beside the
/// description prompt it now mirrors, so the two cannot drift apart unnoticed,
/// and keeps the material in the `user` role where a chat model treats it as
/// content rather than persona.
///
/// `description` and `evidence` are each omitted when empty rather than sent as
/// an empty heading: a section labelled "Description:" with nothing after it
/// reads to the model as a description that says nothing, which is a different
/// claim from having none.
pub(crate) fn entity_type_validate_user_text(
    entity_name: &str,
    current_type: &str,
    description: Option<&str>,
    evidence: &str,
) -> String {
    let mut text = format!("Entity name: {entity_name}\nCurrent type: {current_type}\n");
    if let Some(desc) = description.map(str::trim).filter(|d| !d.is_empty()) {
        text.push_str(&format!("Stored description: {desc}\n"));
    }
    let evidence = evidence.trim();
    if !evidence.is_empty() {
        text.push_str(&format!(
            "\nEvidence (ground truth; judge only from these facts):\n{evidence}\n"
        ));
    }
    text
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

    /// GAP-SG-279: the whole point is that evidence REACHES the model.
    ///
    /// A regression here is silent — the request still succeeds, the model
    /// still answers, and the answer is once again a guess from the name.
    #[test]
    fn the_user_text_carries_the_evidence() {
        let text = entity_type_validate_user_text(
            "rd_gs",
            "concept",
            Some("prefix of the generated folders"),
            "Linked memory bodies:\nrd_gs marks the generated tree",
        );
        assert!(text.contains("rd_gs"), "entity name must survive: {text}");
        assert!(
            text.contains("concept"),
            "current type must survive: {text}"
        );
        assert!(
            text.contains("prefix of the generated folders"),
            "stored description must reach the model: {text}"
        );
        assert!(
            text.contains("rd_gs marks the generated tree"),
            "corpus evidence must reach the model: {text}"
        );
    }

    /// An absent description must leave no heading behind.
    ///
    /// An empty "Stored description:" line tells the model the entity has a
    /// description that says nothing, which is a different claim from having
    /// none, and it is the claim that produces confident nonsense.
    #[test]
    fn an_absent_description_leaves_no_empty_heading() {
        for missing in [None, Some(""), Some("   ")] {
            let text = entity_type_validate_user_text("x", "concept", missing, "");
            assert!(
                !text.contains("Stored description:"),
                "empty description must not print a heading: {text:?}"
            );
            assert!(
                !text.contains("Evidence"),
                "empty evidence must not print a heading: {text:?}"
            );
        }
    }
}
