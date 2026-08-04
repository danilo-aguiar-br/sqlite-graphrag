//! Phase 1 — RESOLVE: turn each claimed key into a target, without network.
//!
//! A key that no longer resolves, or whose text is empty, is settled here and
//! does NOT abort the batch. A target that already holds a live vector at the
//! active dim is settled as done and consumes no request.

use super::target::ReembedTarget;
use crate::commands::enrich::extraction::EnrichItemResult;
use crate::commands::enrich::queue;
use rusqlite::Connection;

/// Outcome of resolving one key without touching the network.
pub(super) enum Resolved {
    /// Terminal already: missing target, empty text, or a live vector.
    Settled(EnrichItemResult),
    /// Needs a slot in the shared embedding call.
    NeedsEmbedding {
        /// Where the vector will be written.
        target: ReembedTarget,
        /// The text to embed.
        text: String,
    },
}

/// Routes an `item_key` to its target and reads the text to embed.
///
/// Routing is by prefix, exactly as the one-row-per-call handler does:
/// `entity:` and `chunk:` are the v1.1.1 backfill shapes, a bare key is the
/// historical memory shape, so pre-v1.1.1 queue rows still resolve.
pub(super) fn resolve_key(conn: &Connection, namespace: &str, key: &str, dim: usize) -> Resolved {
    if let Some(entity_name) = key.strip_prefix("entity:") {
        return resolve_entity(conn, namespace, entity_name, dim);
    }
    if let Some(chunk_key) = key.strip_prefix("chunk:") {
        return resolve_chunk(conn, namespace, chunk_key, dim);
    }
    resolve_memory(conn, namespace, key, dim)
}

fn resolve_memory(conn: &Connection, namespace: &str, name: &str, dim: usize) -> Resolved {
    let row: Result<(i64, String, String), _> = conn.query_row(
        "SELECT id, COALESCE(body,''), COALESCE(type,'note')
             FROM memories
             WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
        rusqlite::params![namespace, name],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    );
    let (memory_id, body, memory_type) = match row {
        Ok(v) => v,
        Err(_) => {
            return Resolved::Settled(EnrichItemResult::Skipped {
                reason: crate::i18n::validation::memory_named_not_found(name),
            })
        }
    };
    if queue::memory_has_live_embedding(conn, memory_id, dim) {
        let chars = body.chars().count();
        return Resolved::Settled(EnrichItemResult::Done {
            memory_id: Some(memory_id),
            entity_id: None,
            entities: 0,
            rels: 0,
            chars_before: Some(chars),
            chars_after: Some(chars),
            cost: 0.0,
            is_oauth: true,
        });
    }
    if body.trim().is_empty() {
        return Resolved::Settled(EnrichItemResult::Skipped {
            reason: "body is empty".to_string(),
        });
    }
    let snippet: String = body.chars().take(200).collect();
    Resolved::NeedsEmbedding {
        target: ReembedTarget::Memory {
            memory_id,
            name: name.to_string(),
            memory_type,
            snippet,
        },
        text: body,
    }
}

fn resolve_entity(conn: &Connection, namespace: &str, name: &str, dim: usize) -> Resolved {
    let row: Result<(i64, String, String), _> = conn.query_row(
        "SELECT id, COALESCE(description,''), type
             FROM entities
             WHERE namespace=?1 AND name=?2",
        rusqlite::params![namespace, name],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    );
    let (entity_id, description, entity_type) = match row {
        Ok(v) => v,
        Err(_) => {
            return Resolved::Settled(EnrichItemResult::Skipped {
                reason: crate::i18n::validation::entity_named_not_found(name),
            })
        }
    };
    // Same text formula the write path uses, so backfilled vectors stay
    // comparable to the ones `remember`/`ingest` produce.
    let text = if description.is_empty() {
        name.to_string()
    } else {
        format!("{name} {description}")
    };
    if queue::entity_has_live_embedding(conn, entity_id, dim) {
        let chars = text.chars().count();
        return Resolved::Settled(EnrichItemResult::Done {
            memory_id: None,
            entity_id: Some(entity_id),
            entities: 1,
            rels: 0,
            chars_before: Some(chars),
            chars_after: Some(chars),
            cost: 0.0,
            is_oauth: true,
        });
    }
    Resolved::NeedsEmbedding {
        target: ReembedTarget::Entity {
            entity_id,
            name: name.to_string(),
            entity_type,
        },
        text,
    }
}

fn resolve_chunk(conn: &Connection, namespace: &str, chunk_key: &str, dim: usize) -> Resolved {
    let chunk_id: i64 = match chunk_key.parse() {
        Ok(v) => v,
        Err(_) => {
            return Resolved::Settled(EnrichItemResult::Skipped {
                reason: crate::i18n::validation::invalid_chunk_id_in_reembed_key(chunk_key),
            })
        }
    };
    let row: Result<(i64, i32, String), _> = conn.query_row(
        "SELECT c.memory_id, c.chunk_idx, c.chunk_text
             FROM memory_chunks c
             JOIN memories m ON m.id = c.memory_id
             WHERE c.id = ?1 AND m.namespace = ?2 AND m.deleted_at IS NULL",
        rusqlite::params![chunk_id, namespace],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    );
    let (memory_id, chunk_idx, chunk_text) = match row {
        Ok(v) => v,
        Err(_) => {
            return Resolved::Settled(EnrichItemResult::Skipped {
                reason: crate::i18n::validation::chunk_id_not_found_in_namespace(
                    chunk_id, namespace,
                ),
            })
        }
    };
    if queue::chunk_has_live_embedding(conn, chunk_id, dim) {
        let chars = chunk_text.chars().count();
        return Resolved::Settled(EnrichItemResult::Done {
            memory_id: Some(memory_id),
            entity_id: None,
            entities: 0,
            rels: 0,
            chars_before: Some(chars),
            chars_after: Some(chars),
            cost: 0.0,
            is_oauth: true,
        });
    }
    if chunk_text.trim().is_empty() {
        return Resolved::Settled(EnrichItemResult::Skipped {
            reason: "chunk text is empty".to_string(),
        });
    }
    Resolved::NeedsEmbedding {
        target: ReembedTarget::Chunk {
            chunk_id,
            memory_id,
            chunk_idx,
        },
        text: chunk_text,
    }
}
