//! Phase B persistence: write staged files into SQLite.

use super::report::FileSuccess;
use super::stage::StagedFile;
use crate::entity_type::EntityType;
use crate::errors::AppError;
use crate::i18n::errors_msg;
use crate::paths::AppPaths;
use crate::storage::chunks as storage_chunks;
use crate::storage::connection::{ensure_db_ready, open_rw};
use crate::storage::entities::{self as entities, NewEntity};
use crate::storage::memories::{self as memories, NewMemory};
use crate::storage::{urls as storage_urls, versions};
use rusqlite::Connection;

pub(crate) fn link_staged_graph(
    tx: &Connection,
    namespace: &str,
    memory_id: i64,
    staged: &StagedFile,
) -> Result<(), AppError> {
    if staged.entities.is_empty() && staged.relationships.is_empty() {
        return Ok(());
    }
    for (idx, entity) in staged.entities.iter().enumerate() {
        let entity_id = entities::upsert_entity(tx, namespace, entity)?;
        if let Some(ref entity_embeddings) = staged.entity_embeddings {
            if let Some(entity_embedding) = entity_embeddings.get(idx) {
                entities::upsert_entity_vec(
                    tx,
                    entity_id,
                    namespace,
                    entity.entity_type,
                    entity_embedding,
                    &entity.name,
                )?;
            }
        }
        entities::link_memory_entity(tx, memory_id, entity_id)?;
    }
    let entity_types: std::collections::HashMap<&str, EntityType> = staged
        .entities
        .iter()
        .map(|entity| (entity.name.as_str(), entity.entity_type))
        .collect();

    let mut affected_entity_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for entity in &staged.entities {
        if let Some(eid) = entities::find_entity_id(tx, namespace, &entity.name)? {
            affected_entity_ids.insert(eid);
        }
    }

    for rel in &staged.relationships {
        let source_entity = NewEntity {
            name: rel.source.clone(),
            entity_type: entity_types
                .get(rel.source.as_str())
                .copied()
                .unwrap_or(EntityType::Concept),
            description: None,
        };
        let target_entity = NewEntity {
            name: rel.target.clone(),
            entity_type: entity_types
                .get(rel.target.as_str())
                .copied()
                .unwrap_or(EntityType::Concept),
            description: None,
        };
        let source_id = entities::upsert_entity(tx, namespace, &source_entity)?;
        let target_id = entities::upsert_entity(tx, namespace, &target_entity)?;
        let rel_id = entities::upsert_relationship(tx, namespace, source_id, target_id, rel)?;
        entities::link_memory_relationship(tx, memory_id, rel_id)?;
        affected_entity_ids.insert(source_id);
        affected_entity_ids.insert(target_id);
    }

    for &eid in &affected_entity_ids {
        entities::recalculate_degree(tx, eid)?;
    }
    Ok(())
}

/// Phase B: persists one `StagedFile` to the database on the main thread.
///
/// GAP-SG-54: when `force_merge` is true an existing memory with the same name
/// is UPDATED (body/embedding/chunks/graph refreshed) instead of being rejected
/// as a duplicate. GAP-SG-55: a memory whose `body_hash` already exists under a
/// DIFFERENT name is skipped (content-level dedup) so divergent derived names do
/// not duplicate identical content.
pub(crate) fn persist_staged(
    conn: &mut Connection,
    namespace: &str,
    memory_type: &str,
    staged: StagedFile,
    force_merge: bool,
) -> Result<FileSuccess, AppError> {
    {
        let active_count: u32 = conn.query_row(
            "SELECT COUNT(DISTINCT namespace) FROM memories WHERE deleted_at IS NULL",
            [],
            |r| r.get::<_, i64>(0).map(|v| v as u32),
        )?;
        let ns_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM memories WHERE namespace = ?1 AND deleted_at IS NULL)",
            rusqlite::params![namespace],
            |r| r.get::<_, i64>(0).map(|v| v > 0),
        )?;
        if !ns_exists && active_count >= crate::constants::MAX_NAMESPACES_ACTIVE {
            return Err(AppError::NamespaceError(format!(
                "active namespace limit of {} exceeded while creating '{namespace}'",
                crate::constants::MAX_NAMESPACES_ACTIVE
            )));
        }
    }

    let existing_memory = memories::find_by_name(conn, namespace, &staged.name)?;
    let duplicate_hash_id = memories::find_by_hash(conn, namespace, &staged.body_hash)?;

    let new_memory = NewMemory {
        namespace: namespace.to_string(),
        name: staged.name.clone(),
        memory_type: memory_type.to_string(),
        description: staged.description.clone(),
        body: staged.body.clone(),
        body_hash: staged.body_hash.clone(),
        session_id: None,
        source: "agent".to_string(),
        metadata: serde_json::json!({}),
    };
    let body_length = new_memory.body.len();
    let metadata_json = serde_json::to_string(&new_memory.metadata)?;

    match existing_memory {
        Some((existing_id, _updated_at, _version)) => {
            if !force_merge {
                return Err(AppError::Duplicate(errors_msg::duplicate_memory(
                    &staged.name,
                    namespace,
                )));
            }

            // GAP-SG-54: --force-merge update path. Refresh body, embedding,
            // chunks and graph bindings of the existing memory.
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

            let (old_name, old_desc, old_body): (String, String, String) = tx.query_row(
                "SELECT name, description, body FROM memories WHERE id = ?1",
                rusqlite::params![existing_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;

            let next_v = versions::next_version(&tx, existing_id)?;
            memories::update(&tx, existing_id, &new_memory, None)?;
            memories::sync_fts_after_update(
                &tx,
                existing_id,
                &old_name,
                &old_desc,
                &old_body,
                &staged.name,
                &staged.description,
                &new_memory.body,
            )?;
            versions::insert_version(
                &tx,
                existing_id,
                next_v,
                &staged.name,
                memory_type,
                &staged.description,
                &new_memory.body,
                &metadata_json,
                None,
                "edit",
            )?;

            // Re-index chunks: drop the old slices then re-insert the staged set.
            storage_chunks::delete_chunks(&tx, existing_id)?;
            if let Some(ref emb) = staged.embedding {
                memories::upsert_vec(
                    &tx,
                    existing_id,
                    namespace,
                    memory_type,
                    emb,
                    &staged.name,
                    &staged.snippet,
                )?;
            }
            if staged.chunks_info.len() > 1 {
                storage_chunks::insert_chunk_slices(
                    &tx,
                    existing_id,
                    &new_memory.body,
                    &staged.chunks_info,
                )?;
                if let Some(ref chunk_embeddings) = staged.chunk_embeddings {
                    for (i, emb) in chunk_embeddings.iter().enumerate() {
                        storage_chunks::upsert_chunk_vec(
                            &tx,
                            i as i64,
                            existing_id,
                            i as i32,
                            emb,
                        )?;
                    }
                }
            }

            link_staged_graph(&tx, namespace, existing_id, &staged)?;
            tx.commit()?;

            Ok(FileSuccess {
                memory_id: existing_id,
                action: "updated".to_string(),
                body_length,
                backend_invoked: staged.backend_invoked,
            })
        }
        None => {
            // GAP-SG-55: identical content already stored under a different name
            // → skip creating a duplicate (reported as `skipped` by the caller).
            if let Some(hash_id) = duplicate_hash_id {
                return Err(AppError::Duplicate(format!(
                    "identical body already stored as memory id {hash_id} (dedup by body_hash); skipping '{}'",
                    staged.name
                )));
            }

            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let memory_id = memories::insert(&tx, &new_memory)?;
            versions::insert_version(
                &tx,
                memory_id,
                1,
                &staged.name,
                memory_type,
                &staged.description,
                &new_memory.body,
                &metadata_json,
                None,
                "create",
            )?;
            if let Some(ref emb) = staged.embedding {
                memories::upsert_vec(
                    &tx,
                    memory_id,
                    namespace,
                    memory_type,
                    emb,
                    &staged.name,
                    &staged.snippet,
                )?;
            }
            if staged.chunks_info.len() > 1 {
                storage_chunks::insert_chunk_slices(
                    &tx,
                    memory_id,
                    &new_memory.body,
                    &staged.chunks_info,
                )?;
                if let Some(ref chunk_embeddings) = staged.chunk_embeddings {
                    for (i, emb) in chunk_embeddings.iter().enumerate() {
                        storage_chunks::upsert_chunk_vec(&tx, i as i64, memory_id, i as i32, emb)?;
                    }
                }
            }
            link_staged_graph(&tx, namespace, memory_id, &staged)?;
            tx.commit()?;

            if !staged.urls.is_empty() {
                let url_entries: Vec<storage_urls::MemoryUrl> = staged
                    .urls
                    .into_iter()
                    .map(|u| storage_urls::MemoryUrl {
                        url: u.url,
                        offset: Some(u.start as i64),
                    })
                    .collect();
                let _ = storage_urls::insert_urls(conn, memory_id, &url_entries);
            }

            Ok(FileSuccess {
                memory_id,
                action: "created".to_string(),
                body_length,
                backend_invoked: staged.backend_invoked,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// G20: mode-conditional flag validation
// ---------------------------------------------------------------------------

/// Auto-initialises the database (matches the contract of every other CRUD
/// handler) and returns a fresh read/write connection ready for the ingest
/// loop. Errors here are recoverable per-file: the caller surfaces them as
/// failure events so `--fail-fast` and the continue-on-error path keep
/// working when, for example, the user points `--db` at an unwritable path.
pub(crate) fn init_storage(paths: &AppPaths) -> Result<Connection, AppError> {
    ensure_db_ready(paths)?;
    let conn = open_rw(&paths.db)?;
    Ok(conn)
}

