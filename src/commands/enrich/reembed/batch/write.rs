//! Phase 3 — WRITE: upsert one vector into the table its target names.

use super::target::ReembedTarget;
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
            // v1.2.8: the label read back from `entities.type` travels as
            // written. It used to be folded onto a canonical kind here, which
            // could report a type the row does not hold; the column is the
            // source of truth for the vector row either way.
            entity_type,
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
