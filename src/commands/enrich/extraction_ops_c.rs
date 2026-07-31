//! Extracted from extraction.rs (Wave C1).

use super::postprocess::persist_memory_bindings;
use super::*;
use crate::entity_type::EntityType;
use crate::errors::AppError;
use crate::storage::entities::{self, NewEntity};
use crate::storage::memories;
use rusqlite::Connection;
use std::path::Path;

/// G27 P2: Enrich generic memory description via LLM.
pub(crate) fn call_description_enrich(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    binary: &Path,
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
        .map_err(|_| AppError::NotFound(format!("memory '{item_key}' not found")))?;
    let snippet: String = body.chars().take(500).collect();
    let input_text = format!(
        "Memory name: {item_key}\nCurrent description: {old_desc}\nBody preview: {snippet}"
    );
    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            DESCRIPTION_ENRICH_PROMPT,
            DESCRIPTION_ENRICH_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            DESCRIPTION_ENRICH_PROMPT,
            DESCRIPTION_ENRICH_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            DESCRIPTION_ENRICH_PROMPT,
            DESCRIPTION_ENRICH_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
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

/// G27 P2: Classify memory into domain category via LLM.
pub(crate) fn call_domain_classify(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (mem_id, body, desc): (i64, String, String) = conn
        .query_row(
            "SELECT id, body, description FROM memories WHERE name = ?1 AND deleted_at IS NULL",
            rusqlite::params![item_key],
            |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
        )
        .map_err(|_| AppError::NotFound(format!("memory '{item_key}' not found")))?;
    let snippet: String = body.chars().take(500).collect();
    let input_text = format!("Memory: {item_key}\nDescription: {desc}\nBody preview: {snippet}");
    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            DOMAIN_CLASSIFY_PROMPT,
            DOMAIN_CLASSIFY_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            DOMAIN_CLASSIFY_PROMPT,
            DOMAIN_CLASSIFY_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            DOMAIN_CLASSIFY_PROMPT,
            DOMAIN_CLASSIFY_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::OpenRouter => call_openrouter(
            DOMAIN_CLASSIFY_PROMPT,
            DOMAIN_CLASSIFY_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let domain = value
        .get("domain")
        .and_then(|v| v.as_str())
        .unwrap_or("uncategorized");
    let metadata = format!(r#"{{"domain":"{}"}}"#, domain.replace('"', "\\\""));
    conn.execute(
        "UPDATE memories SET metadata = ?1 WHERE id = ?2",
        rusqlite::params![metadata, mem_id],
    )?;
    Ok(EnrichItemResult::Done {
        memory_id: Some(mem_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// G27 P2: Audit memory graph quality via LLM.
pub(crate) fn call_graph_audit(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (mem_id, body, desc): (i64, String, String) = conn
        .query_row(
            "SELECT id, body, description FROM memories WHERE name = ?1 AND deleted_at IS NULL",
            rusqlite::params![item_key],
            |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
        )
        .map_err(|_| AppError::NotFound(format!("memory '{item_key}' not found")))?;
    let snippet: String = body.chars().take(500).collect();
    let ent_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memory_entities WHERE memory_id = ?1",
            rusqlite::params![mem_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let input_text = format!("Memory: {item_key}\nDescription: {desc}\nEntity bindings: {ent_count}\nBody preview: {snippet}");
    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            GRAPH_AUDIT_PROMPT,
            GRAPH_AUDIT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            GRAPH_AUDIT_PROMPT,
            GRAPH_AUDIT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            GRAPH_AUDIT_PROMPT,
            GRAPH_AUDIT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::OpenRouter => call_openrouter(
            GRAPH_AUDIT_PROMPT,
            GRAPH_AUDIT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let issues = value
        .get("issues")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    Ok(EnrichItemResult::Done {
        memory_id: Some(mem_id),
        entity_id: None,
        entities: 0,
        rels: issues,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// G27 P2: Synthesize research findings into graph entities/relationships via LLM.
pub(crate) fn call_deep_research_synth(
    conn: &Connection,
    namespace: &str,
    item_key: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (mem_id, body): (i64, String) = conn
        .query_row(
            "SELECT id, body FROM memories WHERE name = ?1 AND deleted_at IS NULL",
            rusqlite::params![item_key],
            |r| Ok((r.get(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(|_| AppError::NotFound(format!("memory '{item_key}' not found")))?;
    let snippet: String = body.chars().take(2000).collect();
    let input_text = format!("Memory: {item_key}\nBody:\n{snippet}");
    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            DEEP_RESEARCH_SYNTH_PROMPT,
            DEEP_RESEARCH_SYNTH_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            DEEP_RESEARCH_SYNTH_PROMPT,
            DEEP_RESEARCH_SYNTH_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            DEEP_RESEARCH_SYNTH_PROMPT,
            DEEP_RESEARCH_SYNTH_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::OpenRouter => call_openrouter(
            DEEP_RESEARCH_SYNTH_PROMPT,
            DEEP_RESEARCH_SYNTH_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let mut ent_count = 0usize;
    let mut rel_count = 0usize;
    if let Some(ents) = value.get("entities").and_then(|v| v.as_array()) {
        for e in ents {
            let name = e.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let etype_str = e
                .get("entity_type")
                .and_then(|v| v.as_str())
                .unwrap_or("concept");
            let etype: EntityType = etype_str.parse().unwrap_or(EntityType::Concept);
            if name.len() >= 2 {
                let ne = NewEntity {
                    name: name.to_string(),
                    entity_type: etype,
                    description: None,
                };
                let _ = entities::upsert_entity(conn, namespace, &ne);
                ent_count += 1;
            }
        }
    }
    if let Some(rels) = value.get("relationships").and_then(|v| v.as_array()) {
        for r in rels {
            let src = r.get("source").and_then(|v| v.as_str()).unwrap_or_default();
            let tgt = r.get("target").and_then(|v| v.as_str()).unwrap_or_default();
            if src.is_empty() || tgt.is_empty() {
                continue;
            }
            let rel = r
                .get("relation")
                .and_then(|v| v.as_str())
                .unwrap_or("related");
            let str_ = r.get("strength").and_then(|v| v.as_f64()).unwrap_or(0.5);
            if let (Some(sid), Some(tid)) = (
                entities::find_entity_id(conn, namespace, src)?,
                entities::find_entity_id(conn, namespace, tgt)?,
            ) {
                let _ = entities::create_or_fetch_relationship(
                    conn, namespace, sid, tid, rel, str_, None,
                );
                rel_count += 1;
            }
        }
    }
    Ok(EnrichItemResult::Done {
        memory_id: Some(mem_id),
        entity_id: None,
        entities: ent_count,
        rels: rel_count,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// G27 P2: Extract structured body from unstructured text via LLM.
///
/// GAP-SG-28: when `graph_only` is set, the memory body is left UNTOUCHED and
/// the extraction instead pulls entities/relationships into the graph (additive,
/// via the same upsert path as `memory-bindings`). This is the read-only mode —
/// it never rewrites or truncates the stored body.
#[allow(clippy::too_many_arguments)]
pub(crate) fn call_body_extract(
    conn: &Connection,
    namespace: &str,
    item_key: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
    graph_only: bool,
) -> Result<EnrichItemResult, AppError> {
    // GAP-SG-28: read-only graph extraction. Reuse the bindings prompt/schema
    // and the additive persist path; the body is never modified.
    if graph_only {
        let (memory_id, body): (i64, String) = conn
            .query_row(
                "SELECT id, COALESCE(body,'') FROM memories WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
                rusqlite::params![namespace, item_key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::NotFound(format!("memory '{item_key}' not found"))
                }
                other => AppError::Database(other),
            })?;
        if body.trim().is_empty() {
            return Ok(EnrichItemResult::Skipped {
                reason: "body is empty".to_string(),
            });
        }
        let (value, cost, is_oauth) = match mode {
            EnrichMode::ClaudeCode => call_claude(
                binary,
                BINDINGS_PROMPT,
                BINDINGS_SCHEMA,
                &body,
                model,
                timeout,
            )?,
            EnrichMode::Codex => call_codex(
                binary,
                BINDINGS_PROMPT,
                BINDINGS_SCHEMA,
                &body,
                model,
                timeout,
            )?,
            EnrichMode::Opencode => call_opencode(
                binary,
                BINDINGS_PROMPT,
                BINDINGS_SCHEMA,
                &body,
                model,
                timeout,
            )?,
            EnrichMode::OpenRouter => {
                call_openrouter(BINDINGS_PROMPT, BINDINGS_SCHEMA, &body, model, timeout)?
            }
        };
        let empty_arr = serde_json::Value::Array(vec![]);
        let entities_val = value.get("entities").unwrap_or(&empty_arr);
        let rels_val = value.get("relationships").unwrap_or(&empty_arr);
        let (ent_count, rel_count) =
            persist_memory_bindings(conn, namespace, memory_id, entities_val, rels_val)?;
        return Ok(EnrichItemResult::Done {
            memory_id: Some(memory_id),
            entity_id: None,
            entities: ent_count,
            rels: rel_count,
            chars_before: None,
            chars_after: None,
            cost,
            is_oauth,
        });
    }

    let (mem_id, body, old_desc): (i64, String, String) = conn
        .query_row(
            "SELECT id, body, description FROM memories WHERE name = ?1 AND deleted_at IS NULL",
            rusqlite::params![item_key],
            |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
        )
        .map_err(|_| AppError::NotFound(format!("memory '{item_key}' not found")))?;
    let old_name: String = conn.query_row(
        "SELECT name FROM memories WHERE id = ?1",
        rusqlite::params![mem_id],
        |r| r.get(0),
    )?;
    let input_text = format!("Memory: {item_key}\nBody:\n{body}");
    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            BODY_EXTRACT_PROMPT,
            BODY_EXTRACT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            BODY_EXTRACT_PROMPT,
            BODY_EXTRACT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            BODY_EXTRACT_PROMPT,
            BODY_EXTRACT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::OpenRouter => call_openrouter(
            BODY_EXTRACT_PROMPT,
            BODY_EXTRACT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let restructured = value
        .get("restructured_body")
        .and_then(|v| v.as_str())
        .unwrap_or(&body);
    let chars_before = body.len();
    let chars_after = restructured.len();
    let new_hash = blake3::hash(restructured.as_bytes()).to_hex().to_string();
    conn.execute(
        "UPDATE memories SET body = ?1, body_hash = ?2, updated_at = unixepoch() WHERE id = ?3",
        rusqlite::params![restructured, new_hash, mem_id],
    )?;
    memories::sync_fts_after_update(
        conn,
        mem_id,
        &old_name,
        &old_desc,
        &body,
        &old_name,
        &old_desc,
        restructured,
    )?;
    Ok(EnrichItemResult::Done {
        memory_id: Some(mem_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: Some(chars_before),
        chars_after: Some(chars_after),
        cost,
        is_oauth,
    })
}
