//! Operations that rewrite or synthesize a memory BODY (GAP-SG-146).
//!
//! `body-enrich` expands a thin body, `body-extract` pulls structure out of an
//! unstructured one, and `deep-research-synth` writes a synthesis body plus its
//! graph. All three own the same field, so they belong together.

use super::postprocess::{persist_enriched_body, persist_memory_bindings};
use super::*;
use crate::constants::MAX_MEMORY_BODY_LEN;
use crate::entity_type::EntityType;
use crate::errors::AppError;
use crate::storage::entities::{self, NewEntity};
use crate::storage::memories;
use rusqlite::Connection;
use std::path::Path;
#[allow(clippy::too_many_arguments)]
pub(crate) fn call_body_enrich(
    conn: &Connection,
    namespace: &str,
    memory_name: &str,
    _binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
    min_output_chars: usize,
    max_output_chars: usize,
    prompt_template: Option<&Path>,
    preserve_threshold: f64,
    paths: &crate::paths::AppPaths,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<EnrichItemResult, AppError> {
    let (memory_id, body, description, memory_type): (i64, String, String, String) = conn
        .query_row(
            "SELECT id, COALESCE(body,''), COALESCE(description,''), COALESCE(type,'note') \
         FROM memories WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
            rusqlite::params![namespace, memory_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(crate::i18n::validation::memory_named_not_found(memory_name))
            }
            other => AppError::Database(other),
        })?;

    let chars_before = body.chars().count();

    // G26: gather graph context for contextualized enrichment
    let linked_entities: Vec<String> = {
        let mut stmt = conn.prepare_cached(
            "SELECT e.name FROM memory_entities me \
             JOIN entities e ON e.id = me.entity_id \
             WHERE me.memory_id = ?1 LIMIT ?2",
        )?;
        let result: Vec<String> = stmt
            .query_map(
                rusqlite::params![
                    memory_id,
                    i64::try_from(crate::constants::K_ENRICH_BODY_CONTEXT_ENTITIES_LIMIT)
                        .unwrap_or(i64::MAX)
                ],
                |r| r.get::<_, String>(0),
            )?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        result
    };

    // Load custom prompt template if provided
    let prompt_prefix = if let Some(tmpl_path) = prompt_template {
        let file_size = std::fs::metadata(tmpl_path)
            .map_err(|e| {
                AppError::Io(std::io::Error::new(
                    e.kind(),
                    format!("failed to stat prompt template: {e}"),
                ))
            })?
            .len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        std::fs::read_to_string(tmpl_path).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("failed to read prompt template: {e}"),
            ))
        })?
    } else {
        BODY_ENRICH_PROMPT_PREFIX.to_string()
    };

    // G26: build contextualized prompt with graph data
    let context_section = if !linked_entities.is_empty() || !description.is_empty() {
        let mut ctx = String::new();
        ctx.push_str(&format!(
            "\nContext:\n- Memory name: {memory_name}\n- Type: {memory_type}\n"
        ));
        if !description.is_empty() {
            ctx.push_str(&format!("- Description: {description}\n"));
        }
        ctx.push_str(&format!("- Domain: {namespace}\n"));
        if !linked_entities.is_empty() {
            ctx.push_str(&format!(
                "- Linked entities: {}\n",
                linked_entities.join(", ")
            ));
        }
        ctx
    } else {
        String::new()
    };

    let prompt = format!(
        "{prompt_prefix}{context_section}\nTarget minimum length: {min_output_chars} characters. Maximum: {max_output_chars} characters."
    );

    // The body schema uses a free-form enriched_body field
    let (value, cost, is_oauth) = match mode {
        EnrichMode::OpenRouter => {
            call_openrouter(&prompt, BODY_ENRICH_SCHEMA, &body, model, timeout)?
        }
    };

    let enriched_body = value
        .get("enriched_body")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::Validation(crate::i18n::validation::llm_missing_enriched_body_field())
        })?;

    let chars_after = enriched_body.chars().count();

    // G29 Passo 4 (v1.0.69): preservation check. Before persisting, run
    // a trigram-Jaccard similarity between the original body and the
    // LLM-rewritten body. When the score falls below
    // `args.preserve_threshold` (default 0.7 per the G29 gap), reject the
    // rewrite as a likely hallucination. The result is recorded in the
    // NDJSON stream so operators can audit what the LLM tried to do.
    let threshold = preserve_threshold;
    let verdict =
        crate::preservation::PreservationVerdict::evaluate(&body, enriched_body, threshold);
    if !verdict.is_accepted() {
        return Ok(EnrichItemResult::PreservationFailed {
            score: match verdict {
                crate::preservation::PreservationVerdict::Preserved { score, .. } => score,
                crate::preservation::PreservationVerdict::Rejected { score, .. } => score,
                crate::preservation::PreservationVerdict::Unchanged { .. } => 1.0,
            },
            threshold,
            chars_before,
            chars_after,
        });
    }

    // G29 Passo 5 (v1.0.69): idempotency via blake3 hash. Before persisting,
    // compare the hash of the original body against the hash of the enriched
    // body. Identical hashes mean the LLM produced a byte-for-byte identical
    // body (rare but possible) — treat as `Skipped` so re-running the batch
    // is safe and the queue does not get re-persisted entries.
    let old_hash = blake3::hash(body.as_bytes()).to_hex().to_string();
    let new_hash = blake3::hash(enriched_body.as_bytes()).to_hex().to_string();
    if old_hash == new_hash {
        return Ok(EnrichItemResult::Skipped {
            reason: format!(
                "enriched body hash matches original (blake3:{old_hash}); idempotency skip"
            ),
        });
    }

    // Only persist if the enriched body is genuinely longer
    if chars_after <= chars_before {
        return Ok(EnrichItemResult::Skipped {
            reason: format!(
                "enriched body ({chars_after} chars) not longer than original ({chars_before} chars)"
            ),
        });
    }

    persist_enriched_body(
        conn,
        namespace,
        memory_id,
        memory_name,
        enriched_body,
        paths,
        llm_backend,
        embedding_backend,
    )?;

    Ok(EnrichItemResult::Done {
        memory_id: Some(memory_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: Some(chars_before),
        chars_after: Some(chars_after),
        cost,
        is_oauth,
    })
}

// scan_operation moved to scan.rs
/// G27 P2: Synthesize research findings into graph entities/relationships via LLM.
pub(crate) fn call_deep_research_synth(
    conn: &Connection,
    namespace: &str,
    item_key: &str,
    _binary: &Path,
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
        .map_err(|_| {
            AppError::NotFound(crate::i18n::validation::memory_named_not_found(item_key))
        })?;
    let snippet: String = body.chars().take(2000).collect();
    let input_text = format!("Memory: {item_key}\nBody:\n{snippet}");
    let (value, cost, is_oauth) = match mode {
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
                // Counted only when it PERSISTED. The increment used to sit
                // outside the result, so an entity the database rejected — an
                // LLM-extracted name failing validation is routine — still
                // raised the number the envelope reported. That is not a silent
                // discard, it is a false count: a caller reading
                // `entities: 5` had no way to learn that two never landed.
                //
                // A rejection stays non-fatal on purpose. Propagating it would
                // turn one bad name into a failed item, which the queue would
                // retry and eventually mark dead, trading a partial success for
                // a permanent failure.
                if entities::upsert_entity(conn, namespace, &ne).is_ok() {
                    ent_count += 1;
                }
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
                // Same correction as the entity loop above: the count follows
                // the write, not the attempt.
                if entities::create_or_fetch_relationship(
                    conn, namespace, sid, tid, rel, str_, None,
                )
                .is_ok()
                {
                    rel_count += 1;
                }
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
    _binary: &Path,
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
                    AppError::NotFound(crate::i18n::validation::memory_named_not_found(item_key))
                }
                other => AppError::Database(other),
            })?;
        if body.trim().is_empty() {
            return Ok(EnrichItemResult::Skipped {
                reason: "body is empty".to_string(),
            });
        }
        let (value, cost, is_oauth) = match mode {
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
        .map_err(|_| {
            AppError::NotFound(crate::i18n::validation::memory_named_not_found(item_key))
        })?;
    let old_name: String = conn.query_row(
        "SELECT name FROM memories WHERE id = ?1",
        rusqlite::params![mem_id],
        |r| r.get(0),
    )?;
    let input_text = format!("Memory: {item_key}\nBody:\n{body}");
    let (value, cost, is_oauth) = match mode {
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
