//! One-row-per-call re-embed handlers (GAP-SG-141 B1).
//!
//! Moved verbatim from `extraction_ops_a.rs`. These remain the reference
//! semantics for a single `ReEmbed` queue row; the drains route the whole
//! `ReEmbed` operation through [`super::batch::call_reembed_batch`], which
//! resolves the same targets but collapses N embedding requests into one.

use crate::commands::enrich::extraction::EnrichItemResult;
use crate::commands::enrich::postprocess::{
    record_enrich_backend, reembed_memory_vector, MemoryRowRef,
};
use crate::commands::enrich::queue;
use crate::errors::AppError;
use crate::storage::entities::{self};
use rusqlite::Connection;

// GAP-SG-73: failures from `reembed_memory_vector` below reach the queue
// as bare `AppError::Embedding`, not a typed `EmbedError` — see the doc
// comment on the `AppError::Embedding` arm of `classify_enrich_outcome` in
// `queue.rs` for why the origin-typed `retry_class` is not threaded through
// here, and why Transient is the documented, deliberate safe floor.
pub(crate) fn call_reembed(
    conn: &Connection,
    namespace: &str,
    item_key: &str,
    paths: &crate::paths::AppPaths,
    backends: crate::cli::BackendChoice,
) -> Result<EnrichItemResult, AppError> {
    // v1.1.1 (P2): prefixed keys route to the entity/chunk backfill paths
    // (`re-embed --target entities|chunks|all`); bare keys keep the
    // historical memory behaviour, so pre-v1.1.1 queue rows still work.
    if let Some(entity_name) = item_key.strip_prefix("entity:") {
        return call_reembed_entity(conn, namespace, entity_name, paths, backends);
    }
    if let Some(chunk_key) = item_key.strip_prefix("chunk:") {
        return call_reembed_chunk(conn, namespace, chunk_key, paths, backends);
    }
    let memory_name = item_key;
    let (memory_id, body, memory_type): (i64, String, String) = conn
        .query_row(
            "SELECT id, COALESCE(body,''), COALESCE(type,'note')
             FROM memories
             WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
            rusqlite::params![namespace, memory_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(crate::i18n::validation::memory_named_not_found(memory_name))
            }
            other => AppError::Database(other),
        })?;

    // CAPA-C1: skip API when a live vector already exists at the active dim.
    let dim = crate::constants::embedding_dim();
    if queue::memory_has_live_embedding(conn, memory_id, dim) {
        return Ok(EnrichItemResult::Done {
            memory_id: Some(memory_id),
            entity_id: None,
            entities: 0,
            rels: 0,
            chars_before: Some(body.chars().count()),
            chars_after: Some(body.chars().count()),
            cost: 0.0,
            is_oauth: true,
        });
    }

    if body.trim().is_empty() {
        return Ok(EnrichItemResult::Skipped {
            cost: 0.0,
            reason: crate::i18n::validation::body_is_empty(),
        });
    }

    reembed_memory_vector(
        conn,
        MemoryRowRef {
            namespace,
            memory_id,
            memory_name,
            memory_type: &memory_type,
            body: &body,
        },
        paths,
        backends,
    )?;

    Ok(EnrichItemResult::Done {
        memory_id: Some(memory_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: Some(body.chars().count()),
        chars_after: Some(body.chars().count()),
        cost: 0.0,
        is_oauth: true,
    })
}

/// v1.1.1 (P2): rebuilds the vector of a single entity
/// (`re-embed --target entities`).
///
/// Embeds the same text formula the write path uses (the entity name, plus
/// the description when present) so backfilled vectors are comparable to the
/// ones produced at write time by `remember`/`ingest`.
fn call_reembed_entity(
    conn: &Connection,
    namespace: &str,
    entity_name: &str,
    paths: &crate::paths::AppPaths,
    backends: crate::cli::BackendChoice,
) -> Result<EnrichItemResult, AppError> {
    let (entity_id, description, entity_type): (i64, String, String) = conn
        .query_row(
            "SELECT id, COALESCE(description,''), type
             FROM entities
             WHERE namespace=?1 AND name=?2",
            rusqlite::params![namespace, entity_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(crate::i18n::validation::entity_named_not_found(entity_name))
            }
            other => AppError::Database(other),
        })?;

    // CAPA-C1: skip API when a live vector already exists at the active dim.
    let dim = crate::constants::embedding_dim();
    if queue::entity_has_live_embedding(conn, entity_id, dim) {
        let text_len = if description.is_empty() {
            entity_name.chars().count()
        } else {
            entity_name.chars().count() + 1 + description.chars().count()
        };
        return Ok(EnrichItemResult::Done {
            memory_id: None,
            entity_id: Some(entity_id),
            entities: 1,
            rels: 0,
            chars_before: Some(text_len),
            chars_after: Some(text_len),
            cost: 0.0,
            is_oauth: true,
        });
    }

    let text = if description.is_empty() {
        entity_name.to_string()
    } else {
        format!("{entity_name} {description}")
    };
    let (embedding, backend_kind) =
        crate::embedder::embed_passage_with_embedding_choice(&paths.models, &text, backends)?;
    if embedding.is_empty() {
        return Ok(EnrichItemResult::Skipped {
            cost: 0.0,
            reason: crate::i18n::validation::embedding_backend_returned_empty_vector(),
        });
    }
    record_enrich_backend(backend_kind.as_str());
    entities::upsert_entity_vec(
        conn,
        entity_id,
        namespace,
        // v1.2.8: the label read back from `entities.type` travels as written,
        // matching the batch path in `reembed::batch::write`. It used to be
        // folded onto a canonical kind here, which could name a type the row
        // does not hold; the column is the source of truth either way.
        &entity_type,
        &embedding,
        entity_name,
    )?;
    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: Some(entity_id),
        entities: 1,
        rels: 0,
        chars_before: Some(text.chars().count()),
        chars_after: Some(text.chars().count()),
        cost: 0.0,
        is_oauth: true,
    })
}

/// v1.1.1 (P2): rebuilds the vector of a single chunk
/// (`re-embed --target chunks`). The key carries the `memory_chunks.id`.
fn call_reembed_chunk(
    conn: &Connection,
    namespace: &str,
    chunk_key: &str,
    paths: &crate::paths::AppPaths,
    backends: crate::cli::BackendChoice,
) -> Result<EnrichItemResult, AppError> {
    let chunk_id: i64 = chunk_key.parse().map_err(|_| {
        AppError::Validation(crate::i18n::validation::invalid_chunk_id_in_reembed_key(
            chunk_key,
        ))
    })?;
    let (memory_id, chunk_idx, chunk_text): (i64, i32, String) = conn
        .query_row(
            "SELECT c.memory_id, c.chunk_idx, c.chunk_text
             FROM memory_chunks c
             JOIN memories m ON m.id = c.memory_id
             WHERE c.id = ?1 AND m.namespace = ?2 AND m.deleted_at IS NULL",
            rusqlite::params![chunk_id, namespace],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(
                crate::i18n::validation::chunk_id_not_found_in_namespace(chunk_id, namespace),
            ),
            other => AppError::Database(other),
        })?;

    // CAPA-C1: skip API when a live vector already exists at the active dim.
    let dim = crate::constants::embedding_dim();
    if queue::chunk_has_live_embedding(conn, chunk_id, dim) {
        return Ok(EnrichItemResult::Done {
            memory_id: Some(memory_id),
            entity_id: None,
            entities: 0,
            rels: 0,
            chars_before: Some(chunk_text.chars().count()),
            chars_after: Some(chunk_text.chars().count()),
            cost: 0.0,
            is_oauth: true,
        });
    }

    if chunk_text.trim().is_empty() {
        return Ok(EnrichItemResult::Skipped {
            cost: 0.0,
            reason: crate::i18n::validation::chunk_text_is_empty(),
        });
    }
    let (embedding, backend_kind) =
        crate::embedder::embed_passage_with_embedding_choice(&paths.models, &chunk_text, backends)?;
    if embedding.is_empty() {
        return Ok(EnrichItemResult::Skipped {
            cost: 0.0,
            reason: crate::i18n::validation::embedding_backend_returned_empty_vector(),
        });
    }
    record_enrich_backend(backend_kind.as_str());
    crate::storage::chunks::upsert_chunk_vec(conn, chunk_id, memory_id, chunk_idx, &embedding)?;
    Ok(EnrichItemResult::Done {
        memory_id: Some(memory_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: Some(chunk_text.chars().count()),
        chars_after: Some(chunk_text.chars().count()),
        cost: 0.0,
        is_oauth: true,
    })
}
