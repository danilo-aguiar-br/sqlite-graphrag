//! Resolved targets and the pending-embed bookkeeping they feed.
//!
//! What a claimed key becomes once it is routed to a row, and the slot that
//! carries its text into the shared embedding call.

use crate::commands::enrich::extraction::EnrichItemResult;

/// One resolved re-embed target: what to embed, and where the vector goes.
///
/// Resolution is deliberately separated from embedding so a key that cannot be
/// resolved costs nothing and cannot poison the request the other keys share.
#[derive(Debug)]
pub(super) enum ReembedTarget {
    /// A whole memory body, keyed by a bare `item_key`.
    Memory {
        /// `memories.id` receiving the vector.
        memory_id: i64,
        /// Memory name, mirrored into the vector row.
        name: String,
        /// Canonical memory type, mirrored into the vector row.
        memory_type: String,
        /// First 200 chars of the body, mirrored into the vector row.
        snippet: String,
    },
    /// A single entity, keyed by `entity:NAME`.
    Entity {
        /// `entities.id` receiving the vector.
        entity_id: i64,
        /// Entity name, mirrored into the vector row.
        name: String,
        /// Raw entity type, folded to canonical on write.
        entity_type: String,
    },
    /// A single memory chunk, keyed by `chunk:ID`.
    Chunk {
        /// `memory_chunks.id` receiving the vector.
        chunk_id: i64,
        /// Owning memory, mirrored into the vector row.
        memory_id: i64,
        /// Position of the chunk inside its memory.
        chunk_idx: i32,
    },
}

/// A resolved target plus the text to embed and the number of characters it
/// carries (reported back as `chars_before`/`chars_after`).
#[derive(Debug)]
pub(super) struct PendingEmbed {
    /// Index into the caller's key slice, so results map back one-to-one.
    pub(super) slot: usize,
    /// Where the resulting vector is written.
    pub(super) target: ReembedTarget,
    /// The exact text handed to the embedder.
    pub(super) text: String,
}

/// Builds the `Done` result the single-item handlers would have produced.
pub(super) fn done_result(target: &ReembedTarget, chars: usize) -> EnrichItemResult {
    match target {
        ReembedTarget::Memory { memory_id, .. } => EnrichItemResult::Done {
            memory_id: Some(*memory_id),
            entity_id: None,
            entities: 0,
            rels: 0,
            chars_before: Some(chars),
            chars_after: Some(chars),
            cost: 0.0,
            is_oauth: true,
        },
        ReembedTarget::Entity { entity_id, .. } => EnrichItemResult::Done {
            memory_id: None,
            entity_id: Some(*entity_id),
            entities: 1,
            rels: 0,
            chars_before: Some(chars),
            chars_after: Some(chars),
            cost: 0.0,
            is_oauth: true,
        },
        ReembedTarget::Chunk { memory_id, .. } => EnrichItemResult::Done {
            memory_id: Some(*memory_id),
            entity_id: None,
            entities: 0,
            rels: 0,
            chars_before: Some(chars),
            chars_after: Some(chars),
            cost: 0.0,
            is_oauth: true,
        },
    }
}
