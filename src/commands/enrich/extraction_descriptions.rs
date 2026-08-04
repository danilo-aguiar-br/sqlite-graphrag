//! Description generation and enrichment (GAP-SG-146).
//!
//! Everything that writes a `description`, for an entity or for a memory:
//! the grounding corpus loader, its tuning constants, the entity-description
//! operation, and the memory-description enrichment that used to sit in a
//! different size-sliced file.

use super::postprocess::persist_entity_description;
use super::*;
use crate::errors::AppError;
use crate::storage::memories;
use rusqlite::Connection;
use std::path::Path;
/// Default top-K linked memory bodies injected into the ED prompt
/// (GAP-CLI-ED-02). Overridable via XDG `enrich.entity_description.corpus_top_k`.
pub(crate) const ENTITY_DESCRIPTION_CORPUS_TOP_K: usize = 5;
/// Per-body character budget for corpus snippets (GAP-CLI-ED-02).
pub(crate) const ENTITY_DESCRIPTION_SNIPPET_CHARS: usize = 400;
/// Default grounding coverage threshold for entity descriptions
/// (GAP-CLI-ED-03). Lower than body-enrich's 0.7 because descriptions
/// are short (10–20 words) relative to multi-sentence bodies.
pub(crate) const ENTITY_DESCRIPTION_GROUNDING_DEFAULT: f64 = 0.12;

/// Loads top-K linked memory body snippets for grounding (GAP-CLI-ED-02).
///
/// Shared by entity-description generation and status quality sampling (DRY).
pub(crate) fn load_entity_corpus_snippets(
    conn: &Connection,
    entity_id: i64,
    top_k: usize,
    max_chars: usize,
) -> Result<String, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT COALESCE(m.body, '') AS body
         FROM memory_entities me
         JOIN memories m ON m.id = me.memory_id
         WHERE me.entity_id = ?1 AND m.deleted_at IS NULL
         ORDER BY COALESCE(m.updated_at, m.created_at) DESC, m.id DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![entity_id, top_k as i64], |r| {
        r.get::<_, String>(0)
    })?;
    let mut snippets = Vec::with_capacity(top_k);
    for row in rows {
        let body = row.map_err(AppError::Database)?;
        let trimmed = body.trim();
        if trimmed.is_empty() {
            continue;
        }
        let snippet: String = trimmed.chars().take(max_chars).collect();
        snippets.push(snippet);
    }
    Ok(snippets.join("\n---\n"))
}

#[allow(clippy::too_many_arguments)] // Wave 5: fold into call-params struct on monólito split
pub(crate) fn call_entity_description(
    conn: &Connection,
    namespace: &str,
    entity_name: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
    grounding_threshold: f64,
    domain_label: &str,
) -> Result<EnrichItemResult, AppError> {
    let (entity_id, entity_type, old_description): (i64, String, Option<String>) = conn
        .query_row(
            "SELECT id, type, description FROM entities WHERE namespace=?1 AND name=?2",
            rusqlite::params![namespace, entity_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::EntityNotYetMaterialized {
                name: entity_name.to_string(),
                namespace: namespace.to_string(),
            },
            other => AppError::Database(other),
        })?;
    // G-PR-5 / GAP-SG-96: chars_before is the pre-existing description length,
    // never a hard-coded zero on grounding failure.
    let chars_before_existing = old_description
        .as_deref()
        .map(|s| s.chars().count())
        .unwrap_or(0);
    let old_was_lq = old_description
        .as_deref()
        .map(super::super::predicates::is_low_quality_description)
        .unwrap_or(true);

    let corpus = load_entity_corpus_snippets(
        conn,
        entity_id,
        ENTITY_DESCRIPTION_CORPUS_TOP_K,
        ENTITY_DESCRIPTION_SNIPPET_CHARS,
    )?;

    let corpus_section = if corpus.is_empty() {
        "Linked memory evidence: (none — describe conservatively from name and type only; do not invent a software/product frame).\n".to_string()
    } else {
        format!("Linked memory evidence (ground truth; prefer these facts):\n{corpus}\n")
    };

    let domain_section = super::prompts::entity_description_domain_section(domain_label);
    let prompt = format!(
        "{ENTITY_DESCRIPTION_PROMPT_PREFIX}{entity_name}\nEntity type: {entity_type}\n\n{domain_section}{corpus_section}\nGenerate a description:"
    );

    let (value, mut cost, mut is_oauth) =
        invoke_entity_description_llm(mode, binary, &prompt, model, timeout)?;

    let mut description = value
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::Validation(crate::i18n::validation::llm_missing_description_field())
        })?
        .to_string();

    // GAP-CLI-ED-03 / G-T-DRY-01 / G-PR-6: adaptive grounding.
    let threshold = if grounding_threshold > 0.0 {
        grounding_threshold
    } else {
        ENTITY_DESCRIPTION_GROUNDING_DEFAULT
    };
    let min_corpus_chars = crate::runtime_config::resolve_usize(
        None,
        "enrich.entity_description.min_corpus_chars",
        crate::preservation::DEFAULT_GROUNDING_MIN_CORPUS_CHARS,
    );

    let mut verdict = crate::preservation::PreservationVerdict::evaluate_grounding_adaptive(
        &description,
        &corpus,
        threshold,
        min_corpus_chars,
    );
    // Anti-jargon replace: old LQ + new !LQ may pass with marginal grounding.
    if !verdict.is_accepted()
        && old_was_lq
        && !super::super::predicates::is_low_quality_description(&description)
    {
        let score = match &verdict {
            crate::preservation::PreservationVerdict::Rejected { score, .. } => *score,
            crate::preservation::PreservationVerdict::Preserved { score, .. } => *score,
            crate::preservation::PreservationVerdict::Unchanged { .. } => 1.0,
        };
        if score >= (threshold * 0.25).clamp(0.0, 1.0) {
            verdict = crate::preservation::PreservationVerdict::Preserved { score, threshold };
        }
    }
    if !verdict.is_accepted() {
        let score = match verdict {
            crate::preservation::PreservationVerdict::Preserved { score, .. } => score,
            crate::preservation::PreservationVerdict::Rejected { score, .. } => score,
            crate::preservation::PreservationVerdict::Unchanged { .. } => 1.0,
        };
        return Ok(EnrichItemResult::PreservationFailed {
            score,
            threshold,
            chars_before: chars_before_existing,
            chars_after: description.chars().count(),
        });
    }

    // G-PR-2: post-filter quality — `done` requires !is_low_quality_description.
    if super::super::predicates::is_low_quality_description(&description) {
        let anti_jargon_prompt = format!(
            "{prompt}\n\nCRITICAL: Your previous draft was rejected as generic software boilerplate. \
             Write a concrete domain description using only the linked evidence. \
             Forbidden: configuration file, software component, module that, system design, chatbot."
        );
        match invoke_entity_description_llm(mode, binary, &anti_jargon_prompt, model, timeout) {
            Ok((value2, cost2, oauth2)) => {
                cost += cost2;
                is_oauth = is_oauth || oauth2;
                if let Some(d2) = value2.get("description").and_then(|v| v.as_str()) {
                    let d2 = d2.to_string();
                    let v2 = crate::preservation::PreservationVerdict::evaluate_grounding_adaptive(
                        &d2,
                        &corpus,
                        threshold,
                        min_corpus_chars,
                    );
                    if v2.is_accepted()
                        && !super::super::predicates::is_low_quality_description(&d2)
                    {
                        description = d2;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "enrich",
                    error = %e,
                    "G-PR-2 anti-jargon retry failed; keeping first draft for quality gate"
                );
            }
        }
    }

    if super::super::predicates::is_low_quality_description(&description) {
        return Ok(EnrichItemResult::Skipped {
            reason: format!(
                "quality_post_filter: description still matches low-quality predicate \
                 (orig={chars_before_existing} chars, candidate={} chars)",
                description.chars().count()
            ),
        });
    }

    persist_entity_description(conn, entity_id, &description)?;

    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: Some(entity_id),
        entities: 0,
        rels: 0,
        chars_before: Some(chars_before_existing),
        chars_after: Some(description.chars().count()),
        cost,
        is_oauth,
    })
}

/// Single LLM invocation for entity-description (DRY for G-PR-2 retry).
fn invoke_entity_description_llm(
    mode: &EnrichMode,
    _binary: &Path,
    prompt: &str,
    model: Option<&str>,
    timeout: u64,
) -> Result<(serde_json::Value, f64, bool), AppError> {
    match mode {
        EnrichMode::OpenRouter => {
            call_openrouter(prompt, ENTITY_DESCRIPTION_SCHEMA, "", model, timeout)
        }
    }
}

/// G27 P2: Enrich generic memory description via LLM.
pub(crate) fn call_description_enrich(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    _binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (mem_id, body, old_desc): (i64, String, String) = conn
        .query_row(
            "SELECT id, body, description FROM memories WHERE name = ?1 AND deleted_at IS NULL",
            rusqlite::params![item_key],
            |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
        )
        .map_err(|_| {
            AppError::NotFound(crate::i18n::validation::memory_named_not_found(item_key))
        })?;
    let snippet: String = body.chars().take(500).collect();
    let input_text = format!(
        "Memory name: {item_key}\nCurrent description: {old_desc}\nBody preview: {snippet}"
    );
    let (value, cost, is_oauth) = match mode {
        EnrichMode::OpenRouter => call_openrouter(
            DESCRIPTION_ENRICH_PROMPT,
            DESCRIPTION_ENRICH_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let new_desc = value
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or(&old_desc);
    let old_name: String = conn.query_row(
        "SELECT name FROM memories WHERE id = ?1",
        rusqlite::params![mem_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "UPDATE memories SET description = ?1 WHERE id = ?2",
        rusqlite::params![new_desc, mem_id],
    )?;
    memories::sync_fts_after_update(
        conn, mem_id, &old_name, &old_desc, &body, &old_name, new_desc, &body,
    )?;
    Ok(EnrichItemResult::Done {
        memory_id: Some(mem_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: Some(old_desc.len()),
        chars_after: Some(new_desc.len()),
        cost,
        is_oauth,
    })
}
