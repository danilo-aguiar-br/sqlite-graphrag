//! Post-commit checks and JSON response for `remember`.

use crate::errors::AppError;
use crate::output::{self, RememberResponse};
use crate::paths::AppPaths;
use crate::storage::chunks as storage_chunks;
use crate::storage::memories::NewMemory;
use crate::storage::urls as storage_urls;
use rusqlite::Connection;
use std::time::Instant;

/// Finalize a successful remember write: chunk count, URL rows, response envelope.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_remember_result(
    conn: &Connection,
    paths: &AppPaths,
    args: &super::args::RememberArgs,
    graph: &super::graph_input::GraphInput,
    new_memory: &NewMemory,
    memory_id: i64,
    action: String,
    version: i64,
    namespace: String,
    normalized_name: String,
    original_name: String,
    name_was_normalized: bool,
    entities_persisted: usize,
    relationships_persisted: usize,
    relationships_updated: bool,
    chunks_created: usize,
    mut warnings: Vec<String>,
    extracted_urls: Vec<crate::extraction::ExtractedUrl>,
    extraction_method: Option<String>,
    backend_invoked_passage: Option<&'static str>,
    inicio: Instant,
) -> Result<(), AppError> {
    // GAP-SG-40: read back the real chunk-row count now that the write is
    // durable, so `chunks_persisted` reflects observable state (0 for inline
    // single-chunk bodies, the exact row count for multi-chunk bodies).
    let chunks_persisted = storage_chunks::count_for_memory(conn, memory_id)?;

    // GAP-SG-44: confirm the memory has a persisted embedding vector. A missing
    // vector (embedding step failed/skipped) makes the memory unsearchable
    // silently; surface it as a warning recommending re-embed instead of leaving
    // `health` to report `vec_memories_missing` later.
    if !new_memory.body.trim().is_empty() {
        let has_vec: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM memory_embeddings WHERE memory_id = ?1)",
                rusqlite::params![memory_id],
                |r| r.get::<_, i64>(0).map(|v| v > 0),
            )
            .unwrap_or(false);
        if !has_vec {
            tracing::warn!(target: "remember",
                memory_id,
                name = %normalized_name,
                "memory persisted without an embedding vector; recall will be degraded until re-embedded"
            );
            warnings.push(
                "memory persisted without an embedding vector; run `enrich --operation re-embed` to make it searchable"
                    .to_string(),
            );
        }
    }

    // GAP-SG-13: when --force-merge UPDATES an existing memory its body/graph may
    // have changed, so drop any stale enrich-queue sidecar entry keyed to it. The
    // next enrich run re-scans it cleanly. Best-effort; no-op when the queue file
    // is absent.
    if action == "updated" {
        crate::commands::enrich::cleanup_queue_entry(&paths.db, memory_id, &normalized_name);
    }

    // v1.0.24 P0-2: persist URLs in a dedicated table, outside the main transaction.
    // Failures do not propagate — non-critical path with graceful degradation.
    let urls_persisted = if !extracted_urls.is_empty() {
        let url_entries: Vec<storage_urls::MemoryUrl> = extracted_urls
            .into_iter()
            .map(|u| storage_urls::MemoryUrl {
                url: u.url,
                offset: Some(u.start as i64),
            })
            .collect();
        storage_urls::insert_urls(conn, memory_id, &url_entries)
    } else {
        0
    };

    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;

    let created_at_epoch = chrono::Utc::now().timestamp();
    let created_at_iso = crate::tz::format_iso(chrono::Utc::now());

    // GAP-CLI-PRIO-01: hot-set entity names for priority entity-descriptions.
    let entities_created: Vec<String> = graph
        .entities
        .iter()
        .map(|e| e.name.clone())
        .collect();

    // GAP-CLI-PRIO-01 / G-T-ONESHOT-02: recommend entity-descriptions when
    // entities were linked without descriptions (typical after graph-stdin).
    let mut enrich_recommended: Vec<String> = Vec::new();
    if entities_persisted > 0 {
        enrich_recommended.push("entity-descriptions".to_string());
    }

    // GAP-CLI-PRIO-02: optional priority enqueue into the enrich sidecar.
    if args.enqueue_enrich && !entities_created.is_empty() {
        match crate::commands::enrich::enqueue_priority_entity_descriptions(
            paths,
            &namespace,
            &entities_created,
        ) {
            Ok(n) => {
                tracing::info!(
                    target: "remember",
                    enqueued = n,
                    "priority entity-descriptions enqueued"
                );
            }
            Err(e) => {
                warnings.push(format!(
                    "enqueue_enrich failed (entities still listed in entities_created): {e}"
                ));
            }
        }
    }

    output::emit_json(&RememberResponse {
        memory_id,
        // Persist the normalized (kebab-case) slug as `name` since that is the
        // storage key. The original input is exposed via `original_name` only
        // when normalization actually changed something (B_4 in v1.0.32).
        name: normalized_name.clone(),
        namespace,
        action: action.clone(),
        operation: action,
        version,
        entities_persisted,
        relationships_persisted,
        relationships_truncated: relationships_updated,
        chunks_created,
        chunks_persisted,
        urls_persisted,
        extraction_method,
        merged_into_memory_id: None,
        warnings,
        created_at: created_at_epoch,
        created_at_iso,
        elapsed_ms: inicio.elapsed().as_millis() as u64,
        name_was_normalized,
        original_name: name_was_normalized.then_some(original_name),
        backend_invoked: backend_invoked_passage,
        entities_created,
        enrich_recommended,
    })?;
    Ok(())
}
