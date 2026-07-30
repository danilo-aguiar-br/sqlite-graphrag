//! Extracted from extraction.rs (Wave C1).

use super::*;
use rusqlite::Connection;
use crate::errors::AppError;
use std::path::Path;

pub(crate) fn call_weight_calibrate(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let rel_id: i64 = item_key
        .parse()
        .map_err(|_| AppError::Validation(crate::i18n::validation::invalid_relationship_id(item_key)))?;
    let (source_name, target_name, relation, current_weight): (String, String, String, f64) = conn
        .query_row(
            "SELECT e1.name, e2.name, r.relation, r.weight \
             FROM relationships r \
             JOIN entities e1 ON e1.id = r.source_id \
             JOIN entities e2 ON e2.id = r.target_id \
             WHERE r.id = ?1",
            rusqlite::params![rel_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|_| AppError::NotFound(format!("relationship {rel_id} not found")))?;

    let input_text = format!(
        "Source: {source_name}\nTarget: {target_name}\nRelation: {relation}\nCurrent weight: {current_weight}"
    );
    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            WEIGHT_CALIBRATE_PROMPT,
            WEIGHT_CALIBRATE_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            WEIGHT_CALIBRATE_PROMPT,
            WEIGHT_CALIBRATE_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            WEIGHT_CALIBRATE_PROMPT,
            WEIGHT_CALIBRATE_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::OpenRouter => call_openrouter(
            WEIGHT_CALIBRATE_PROMPT,
            WEIGHT_CALIBRATE_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };

    let calibrated = value
        .get("calibrated_weight")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| AppError::Validation(crate::i18n::validation::llm_missing_calibrated_weight()))?;

    conn.execute(
        "UPDATE relationships SET weight = ?1 WHERE id = ?2",
        rusqlite::params![calibrated, rel_id],
    )?;

    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: None,
        entities: 0,
        rels: 1,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// G27: Reclassify a generic relationship type via LLM.
pub(crate) fn call_relation_reclassify(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let rel_id: i64 = item_key
        .parse()
        .map_err(|_| AppError::Validation(crate::i18n::validation::invalid_relationship_id(item_key)))?;
    let (source_name, target_name, current_relation): (String, String, String) = conn
        .query_row(
            "SELECT e1.name, e2.name, r.relation \
             FROM relationships r \
             JOIN entities e1 ON e1.id = r.source_id \
             JOIN entities e2 ON e2.id = r.target_id \
             WHERE r.id = ?1",
            rusqlite::params![rel_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|_| AppError::NotFound(format!("relationship {rel_id} not found")))?;

    let input_text = format!(
        "Source entity: {source_name}\nTarget entity: {target_name}\nCurrent relation: {current_relation}"
    );
    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            RELATION_RECLASSIFY_PROMPT,
            RELATION_RECLASSIFY_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            RELATION_RECLASSIFY_PROMPT,
            RELATION_RECLASSIFY_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            RELATION_RECLASSIFY_PROMPT,
            RELATION_RECLASSIFY_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::OpenRouter => call_openrouter(
            RELATION_RECLASSIFY_PROMPT,
            RELATION_RECLASSIFY_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };

    let new_relation = value
        .get("relation")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Validation(crate::i18n::validation::llm_missing_relation()))?;
    let new_strength = value
        .get("strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);

    conn.execute(
        "UPDATE relationships SET relation = ?1, weight = ?2 WHERE id = ?3",
        rusqlite::params![new_relation, new_strength, rel_id],
    )?;

    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: None,
        entities: 0,
        rels: 1,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// G27 P2: Connect isolated entities via LLM-suggested relationship.
///
/// v1.1.06 (GAP-ENTITY-CONNECT-SCAN-CARTESIAN): `item_key` is
/// `pair:{id1}:{id2}` from the O(k) scan. Resolve both entities by primary key
/// — **never** re-run `scan_isolated_entity_pairs` (the old path re-executed
/// the cartesian SQL on every drain item and re-hung large namespaces).
pub(crate) fn call_entity_connect(
    conn: &Connection,
    namespace: &str,
    item_key: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (e1_id, e2_id) = match super::scan::parse_pair_key(item_key) {
        Some(ids) => ids,
        None => {
            return Ok(EnrichItemResult::Skipped {
                reason: format!(
                    "legacy or invalid entity-connect key '{item_key}' \
                     (expected pair:id1:id2); re-scan to enqueue stable pair keys"
                ),
            });
        }
    };

    let load = |id: i64| -> Result<Option<(i64, String)>, AppError> {
        match conn.query_row(
            "SELECT id, name FROM entities WHERE id = ?1 AND namespace = ?2",
            rusqlite::params![id, namespace],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ) {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::Database(e)),
        }
    };
    let (e1_id, e1_name) = match load(e1_id)? {
        Some(v) => v,
        None => {
            return Ok(EnrichItemResult::Skipped {
                reason: format!("entity id {e1_id} missing in namespace '{namespace}'"),
            });
        }
    };
    let (e2_id, e2_name) = match load(e2_id)? {
        Some(v) => v,
        None => {
            return Ok(EnrichItemResult::Skipped {
                reason: format!("entity id {e2_id} missing in namespace '{namespace}'"),
            });
        }
    };

    // Skip if already evaluated or already related (queue may be stale).
    let already_seen: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM entity_connect_seen \
         WHERE source_id = ?1 AND target_id = ?2)",
        rusqlite::params![e1_id, e2_id],
        |r| r.get(0),
    )?;
    if already_seen {
        return Ok(EnrichItemResult::Skipped {
            reason: "pair already in entity_connect_seen".into(),
        });
    }
    let already_related: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM relationships r WHERE \
           (r.source_id = ?1 AND r.target_id = ?2) OR \
           (r.source_id = ?2 AND r.target_id = ?1))",
        rusqlite::params![e1_id, e2_id],
        |r| r.get(0),
    )?;
    if already_related {
        return Ok(EnrichItemResult::Skipped {
            reason: "pair already related".into(),
        });
    }

    let input_text = format!("Entity A: {e1_name}\nEntity B: {e2_name}");
    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            ENTITY_CONNECT_PROMPT,
            ENTITY_CONNECT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            ENTITY_CONNECT_PROMPT,
            ENTITY_CONNECT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            ENTITY_CONNECT_PROMPT,
            ENTITY_CONNECT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::OpenRouter => call_openrouter(
            ENTITY_CONNECT_PROMPT,
            ENTITY_CONNECT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let relation = value
        .get("relation")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    if relation == "none" {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO entity_connect_seen (source_id, target_id, namespace, verdict, relation) \
             VALUES (?1, ?2, ?3, 'none', NULL)",
            rusqlite::params![e1_id, e2_id, namespace],
        );
        return Ok(EnrichItemResult::Skipped {
            reason: "LLM determined no relationship".into(),
        });
    }
    let strength = value
        .get("strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    conn.execute(
        "INSERT OR IGNORE INTO relationships (namespace, source_id, target_id, relation, weight) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![namespace, e1_id, e2_id, relation, strength],
    )?;
    let _ = conn.execute(
        "INSERT OR REPLACE INTO entity_connect_seen (source_id, target_id, namespace, verdict, relation) \
         VALUES (?1, ?2, ?3, 'related', ?4)",
        rusqlite::params![e1_id, e2_id, namespace, relation],
    );
    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: None,
        entities: 0,
        rels: 1,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// G27 P2: Validate entity type assignment via LLM.
pub(crate) fn call_entity_type_validate(
    conn: &Connection,
    namespace: &str,
    item_key: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (ent_id, ent_name, ent_type): (i64, String, String) = conn
        .query_row(
            "SELECT id, name, type FROM entities WHERE namespace = ?1 AND name = ?2",
            rusqlite::params![namespace, item_key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::EntityNotYetMaterialized {
                name: item_key.to_string(),
                namespace: namespace.to_string(),
            },
            other => AppError::Database(other),
        })?;
    let input_text = format!("Entity: {ent_name}\nCurrent type: {ent_type}");
    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            ENTITY_TYPE_VALIDATE_PROMPT,
            ENTITY_TYPE_VALIDATE_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            ENTITY_TYPE_VALIDATE_PROMPT,
            ENTITY_TYPE_VALIDATE_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            ENTITY_TYPE_VALIDATE_PROMPT,
            ENTITY_TYPE_VALIDATE_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
        EnrichMode::OpenRouter => call_openrouter(
            ENTITY_TYPE_VALIDATE_PROMPT,
            ENTITY_TYPE_VALIDATE_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let validated_type = value
        .get("validated_type")
        .and_then(|v| v.as_str())
        .unwrap_or(&ent_type);
    let was_correct = value
        .get("was_correct")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    if !was_correct {
        conn.execute(
            "UPDATE entities SET type = ?1 WHERE id = ?2",
            rusqlite::params![validated_type, ent_id],
        )?;
    }
    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: Some(ent_id),
        entities: 1,
        rels: 0,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}
