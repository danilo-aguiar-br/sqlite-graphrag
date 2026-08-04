//! Handler for the `merge-entities` CLI subcommand (GAP-19).
//!
//! Merges two or more source entities into a single target entity by:
//!   1. Retargeting all relationships pointing at any source to the target.
//!   2. Deduplicating relationships that become identical after the merge
//!      (same source_id + target_id + relation).
//!   3. Retargeting memory_entities bindings.
//!   4. Deleting the now-empty source entity rows.
//!
//! [`args`] holds the CLI surface, [`envelope`] the JSON report and [`resolve`]
//! the namespace-scoped ID lookup; the merge transaction itself stays here.

use crate::errors::AppError;
use crate::i18n::errors_msg;
use crate::output::{self, OutputFormat};
use crate::paths::AppPaths;
use crate::storage::connection::open_rw;
use crate::storage::entities;
use rusqlite::params;

mod args;
mod envelope;
mod resolve;

pub use args::MergeEntitiesArgs;
use envelope::MergeEntitiesResponse;
use resolve::find_entity_name_by_id;

/// Run.
pub fn run(args: MergeEntitiesArgs) -> Result<(), AppError> {
    let inicio = std::time::Instant::now();

    if args.names.is_empty() && args.ids.is_empty() {
        return Err(AppError::Validation(
            "--names or --ids must contain at least one source entity".to_string(),
        ));
    }

    // v1.1.05 Bug 4: reject self-referential merge at the earliest possible
    // point (before any DB work), so shell word-splitting mistakes fail loud.
    if let Some(target_id) = args.into_id {
        if args.ids.contains(&target_id) {
            return Err(AppError::Validation(
                crate::i18n::validation::self_merge_id_in_ids(target_id),
            ));
        }
    }
    if let Some(ref target_name) = args.into {
        if args.names.iter().any(|n| n == target_name) {
            return Err(AppError::Validation(
                crate::i18n::validation::self_merge_name_in_names(target_name),
            ));
        }
    }

    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let paths = AppPaths::resolve(args.db.as_deref())?;

    crate::storage::connection::ensure_db_ready(&paths)?;

    let mut conn = open_rw(&paths.db)?;

    // Resolve target entity — by ID (v1.1.1 P5, unambiguous) or by name.
    // Existence is validated here, BEFORE any mutation.
    let (target_id, target_name) = match args.into_id {
        Some(id) => {
            // Target is always validated in the resolved namespace, even when
            // --cross-namespace is set: cross-namespace only relaxes SOURCES.
            let (name, _ns_actual) = find_entity_name_by_id(&conn, &namespace, id, true)?;
            (id, name)
        }
        None => {
            let Some(name) = args.into.clone() else {
                return Err(AppError::Validation(
                    "--into or --into-id is required".to_string(),
                ));
            };
            let id = entities::find_entity_id(&conn, &namespace, &name)?.ok_or_else(|| {
                AppError::NotFound(errors_msg::entity_not_found(&name, &namespace))
            })?;
            (id, name)
        }
    };

    // Resolve source entity IDs — reject self-referential merge (G21),
    // by ID (v1.1.1 P5) or by name. All lookups happen BEFORE the transaction.
    // Defense-in-depth: re-check even after early parse-time guard (Bug 4).
    let mut source_ids: Vec<i64> = Vec::with_capacity(args.names.len() + args.ids.len());
    let mut source_names: Vec<String> = Vec::with_capacity(source_ids.capacity());
    if !args.ids.is_empty() {
        for &id in &args.ids {
            if id == target_id {
                return Err(AppError::Validation(
                    crate::i18n::validation::self_merge_id(id, target_id),
                ));
            }
            // v1.1.03: when --cross-namespace is set, resolve each source by its
            // own row (no namespace filter) and warn on the cross-namespace move.
            // Default (false) preserves same-namespace safety.
            let (name, ns_actual) =
                find_entity_name_by_id(&conn, &namespace, id, !args.cross_namespace)?;
            if args.cross_namespace && ns_actual != namespace {
                tracing::warn!(
                    target: "merge_entities",
                    from_id = id,
                    from_namespace = %ns_actual,
                    to_namespace = %namespace,
                    "cross-namespace merge"
                );
            }
            if !source_ids.contains(&id) {
                source_ids.push(id);
                source_names.push(name);
            }
        }
    } else {
        for name in &args.names {
            if name == &target_name {
                return Err(AppError::Validation(
                    crate::i18n::validation::self_merge_name(name, &target_name),
                ));
            }
            let id = entities::find_entity_id(&conn, &namespace, name)?.ok_or_else(|| {
                AppError::NotFound(errors_msg::entity_not_found(name, &namespace))
            })?;
            if id == target_id {
                return Err(AppError::Validation(
                    crate::i18n::validation::self_merge_name_resolves_to_target(name, target_id),
                ));
            }
            if !source_ids.contains(&id) {
                source_ids.push(id);
                source_names.push(name.clone());
            }
        }
    }

    if source_ids.is_empty() {
        return Err(AppError::Validation(
            "no valid source entities to merge (all names equal the target or were duplicates)"
                .to_string(),
        ));
    }

    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    let mut relationships_moved: usize = 0;

    for &src_id in &source_ids {
        // Step 1a: redirect source_id, ignoring UNIQUE conflicts.
        let moved_src = tx.execute(
            "UPDATE OR IGNORE relationships SET source_id = ?1 WHERE source_id = ?2",
            params![target_id, src_id],
        )?;
        tx.execute(
            "DELETE FROM relationships WHERE source_id = ?1",
            params![src_id],
        )?;
        // Step 1b: redirect target_id, ignoring UNIQUE conflicts.
        let moved_tgt = tx.execute(
            "UPDATE OR IGNORE relationships SET target_id = ?1 WHERE target_id = ?2",
            params![target_id, src_id],
        )?;
        tx.execute(
            "DELETE FROM relationships WHERE target_id = ?1",
            params![src_id],
        )?;
        relationships_moved += moved_src + moved_tgt;
    }

    // Step 2: remove self-loops introduced by the redirect (target → target).
    tx.execute("DELETE FROM relationships WHERE source_id = target_id", [])?;

    // Step 3: deduplicate relationships that now share (source, target, relation).
    // Safety net — UPDATE OR IGNORE should have handled most duplicates above.
    tx.execute(
        "DELETE FROM relationships
         WHERE id NOT IN (
             SELECT MIN(id)
             FROM relationships
             GROUP BY source_id, target_id, relation
         )",
        [],
    )?;

    // Step 4: retarget memory_entities bindings.
    // Use UPDATE OR IGNORE to skip conflicts when memory is already bound to
    // target entity. Then DELETE remaining source rows (the conflicting ones
    // that UPDATE OR IGNORE skipped). Same pattern as relationships (Step 1).
    for &src_id in &source_ids {
        tx.execute(
            "UPDATE OR IGNORE memory_entities SET entity_id = ?1 WHERE entity_id = ?2",
            params![target_id, src_id],
        )?;
        tx.execute(
            "DELETE FROM memory_entities WHERE entity_id = ?1",
            params![src_id],
        )?;
    }

    // Step 5: deduplicate memory_entities bindings (same memory + entity).
    tx.execute(
        "DELETE FROM memory_entities
         WHERE rowid NOT IN (
             SELECT MIN(rowid)
             FROM memory_entities
             GROUP BY memory_id, entity_id
         )",
        [],
    )?;

    // Step 6: delete source entities. v1.0.76: FK ON DELETE CASCADE on
    // entity_embeddings handles the vector row automatically.
    let mut entities_removed: usize = 0;
    for &src_id in &source_ids {
        let removed = tx.execute("DELETE FROM entities WHERE id = ?1", params![src_id])?;
        entities_removed += removed;
    }

    // Step 7: recalculate degree for target and all adjacent entities.
    let adjacent_ids: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT CASE WHEN source_id = ?1 THEN target_id ELSE source_id END
             FROM relationships WHERE source_id = ?1 OR target_id = ?1",
        )?;
        let ids: Vec<i64> = stmt
            .query_map(params![target_id], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids
    };
    entities::recalculate_degree(&tx, target_id)?;
    for &adj_id in &adjacent_ids {
        entities::recalculate_degree(&tx, adj_id)?;
    }

    tx.commit()?;

    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;

    let response = MergeEntitiesResponse {
        action: "merged".to_string(),
        sources: source_names,
        target: target_name,
        namespace: namespace.clone(),
        target_id,
        relationships_moved,
        entities_removed,
        elapsed_ms: inicio.elapsed().as_millis() as u64,
    };

    match args.format {
        OutputFormat::Json => output::emit_json(&response)?,
        OutputFormat::Text | OutputFormat::Markdown => {
            output::emit_text(&format!(
                "merged: {} sources into '{}' (relationships_moved={}, entities_removed={}) [{}]",
                response.sources.len(),
                response.target,
                response.relationships_moved,
                response.entities_removed,
                response.namespace
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
