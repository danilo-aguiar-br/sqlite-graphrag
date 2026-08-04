//! Memory-to-graph binding extraction (GAP-SG-146).
//!
//! One operation: turn a memory body into entities and relationships. Split out
//! of the size-sliced `extraction_ops_a.rs`, whose `_a` suffix said nothing
//! about what lived inside it.

use super::postprocess::persist_memory_bindings;
use super::*;
use crate::errors::AppError;
use rusqlite::Connection;
use std::path::Path;

pub(crate) fn call_memory_bindings(
    conn: &Connection,
    namespace: &str,
    memory_name: &str,
    _binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    // GAP-CLI-QISO-04: never treat pair:/entity:/chunk: keys as memory names.
    // Cross-op claim bugs used to produce HardFailure NotFound("memory 'pair:…'").
    if super::queue::is_non_memory_key_shape(memory_name) {
        return Ok(EnrichItemResult::Skipped {
            reason: format!(
                "wrong_key_shape_for_operation:MemoryBindings: key looks like {}",
                memory_name.split(':').next().unwrap_or("prefixed")
            ),
        });
    }

    // Look up the memory
    let (memory_id, body): (i64, String) = conn.query_row(
        "SELECT id, COALESCE(body,'') FROM memories WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
        rusqlite::params![namespace, memory_name],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(crate::i18n::validation::memory_named_not_found(memory_name)),
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

    Ok(EnrichItemResult::Done {
        memory_id: Some(memory_id),
        entity_id: None,
        entities: ent_count,
        rels: rel_count,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}
