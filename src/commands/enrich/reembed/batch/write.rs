//! Phase 3 — WRITE: upsert one vector into the table its target names.

use super::target::ReembedTarget;
use crate::entity_type::EntityType;
use crate::errors::AppError;
use crate::storage::entities::{self};
use rusqlite::Connection;

/// Upserts one vector through the same storage helper the single-item path uses.
pub(super) fn write_vector(
    conn: &Connection,
    namespace: &str,
    target: &ReembedTarget,
    embedding: &[f32],
) -> Result<(), AppError> {
    match target {
        ReembedTarget::Memory {
            memory_id,
            name,
            memory_type,
            snippet,
        } => crate::storage::memories::upsert_vec(
            conn,
            *memory_id,
            namespace,
            memory_type,
            embedding,
            name,
            snippet,
        ),
        ReembedTarget::Entity {
            entity_id,
            name,
            entity_type,
        } => entities::upsert_entity_vec(
            conn,
            *entity_id,
            namespace,
            EntityType::map_to_canonical(entity_type),
            embedding,
            name,
        ),
        ReembedTarget::Chunk {
            chunk_id,
            memory_id,
            chunk_idx,
        } => crate::storage::chunks::upsert_chunk_vec(
            conn, *chunk_id, *memory_id, *chunk_idx, embedding,
        ),
    }
}
