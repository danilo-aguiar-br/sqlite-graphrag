//! Graph-binding and merge-support helpers for entities/relationships.
//!
//! Covers junction-table links (`memory_entities` / `memory_relationships`),
//! degree maintenance, relationship create/fetch/delete, and orphan cleanup —
//! the storage primitives used by `merge-entities`, force-merge remember/ingest
//! paths, and `prune-relations` / `cleanup-orphans`.

use crate::errors::AppError;
use crate::parsers::normalize_entity_name;
use rusqlite::{params, Connection};
use serde::Serialize;
/// Links a memory to an entity in the `memory_entities` join table.
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn link_memory_entity(
    conn: &Connection,
    memory_id: i64,
    entity_id: i64,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR IGNORE INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
        params![memory_id, entity_id],
    )?;
    Ok(())
}

/// Links a memory to a relationship in the `memory_relationships` join table.
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn link_memory_relationship(
    conn: &Connection,
    memory_id: i64,
    rel_id: i64,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR IGNORE INTO memory_relationships (memory_id, relationship_id) VALUES (?1, ?2)",
        params![memory_id, rel_id],
    )?;
    Ok(())
}

/// GAP-SG-52: removes the curated `memory_entities` binding between a memory
/// and an entity. Unlike `prune-ner` (which targets an entity across every
/// memory), this surgically unlinks a single `(memory_id, entity_id)` pair —
/// covering bindings created via `remember --graph-stdin`. Returns the number
/// of junction rows removed (0 or 1).
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn unlink_memory_entity(
    conn: &Connection,
    memory_id: i64,
    entity_id: i64,
) -> Result<u64, AppError> {
    let affected = conn.execute(
        "DELETE FROM memory_entities WHERE memory_id = ?1 AND entity_id = ?2",
        params![memory_id, entity_id],
    )?;
    Ok(affected as u64)
}

/// GAP-SG-51: clears every `memory_entities` and `memory_relationships`
/// binding for a memory so a `--force-merge --replace-graph` update can install
/// an authoritative set (including the empty set). The entities and
/// relationships themselves are preserved; only the junction rows for this
/// memory are removed. Returns `(entity_bindings_removed, relationship_bindings_removed)`.
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn clear_memory_graph_bindings(
    conn: &Connection,
    memory_id: i64,
) -> Result<(u64, u64), AppError> {
    let entities_removed = conn.execute(
        "DELETE FROM memory_entities WHERE memory_id = ?1",
        params![memory_id],
    )? as u64;
    let rels_removed = conn.execute(
        "DELETE FROM memory_relationships WHERE memory_id = ?1",
        params![memory_id],
    )? as u64;
    Ok((entities_removed, rels_removed))
}

/// Increments the `degree` counter of an entity by one.
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn increment_degree(conn: &Connection, entity_id: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE entities SET degree = degree + 1 WHERE id = ?1",
        params![entity_id],
    )?;
    Ok(())
}

/// Looks up the entity by name and namespace. Returns the id when it exists.
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn find_entity_id(
    conn: &Connection,
    namespace: &str,
    name: &str,
) -> Result<Option<i64>, AppError> {
    // Normalize the lookup name so it matches the normalized names written by
    // `upsert_entity`. Without this, an entity written through normalization
    // (e.g. "Foo Bar" -> "foo-bar") would be unreachable by its original
    // spelling, breaking delete-entity, reclassify, merge-entities, rename and
    // memory-entities lookups.
    let name = normalize_entity_name(name);
    let mut stmt =
        conn.prepare_cached("SELECT id FROM entities WHERE namespace = ?1 AND name = ?2")?;
    match stmt.query_row(params![namespace, &name], |r| r.get::<_, i64>(0)) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e)),
    }
}

/// Structure representing an existing relation.
#[derive(Debug, Serialize)]
pub struct RelationshipRow {
    /// Unique identifier.
    pub id: i64,
    /// Namespace scope.
    pub namespace: String,
    /// Source ID.
    pub source_id: i64,
    /// Target ID.
    pub target_id: i64,
    /// Relationship type.
    pub relation: String,
    /// Relationship weight.
    pub weight: f64,
    /// Human-readable description.
    pub description: Option<String>,
}

/// Looks up a specific relation by (source_id, target_id, relation).
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn find_relationship(
    conn: &Connection,
    source_id: i64,
    target_id: i64,
    relation: &str,
) -> Result<Option<RelationshipRow>, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, namespace, source_id, target_id, relation, weight, description
         FROM relationships
         WHERE source_id = ?1 AND target_id = ?2 AND relation = ?3",
    )?;
    match stmt.query_row(params![source_id, target_id, relation], |r| {
        Ok(RelationshipRow {
            id: r.get(0)?,
            namespace: r.get(1)?,
            source_id: r.get(2)?,
            target_id: r.get(3)?,
            relation: r.get(4)?,
            weight: r.get(5)?,
            description: r.get(6)?,
        })
    }) {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e)),
    }
}

/// Creates a relation if it does not exist (returns action="created")
/// or returns the existing relation (action="already_exists") with updated weight.
///
/// # Errors
///
/// - [`AppError::Database`] — SQLite query or constraint failure.
/// - [`AppError::Validation`] — self-link attempt (source equals target).
pub fn create_or_fetch_relationship(
    conn: &Connection,
    namespace: &str,
    source_id: i64,
    target_id: i64,
    relation: &str,
    weight: f64,
    description: Option<&str>,
) -> Result<(i64, bool), AppError> {
    // Check if it exists first; update weight if different.
    let existing = find_relationship(conn, source_id, target_id, relation)?;
    if let Some(row) = existing {
        if (row.weight - weight).abs() > f64::EPSILON {
            conn.execute(
                "UPDATE relationships SET weight = ?1 WHERE id = ?2",
                params![weight, row.id],
            )?;
        }
        return Ok((row.id, false));
    }
    conn.execute(
        "INSERT INTO relationships (namespace, source_id, target_id, relation, weight, description)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            namespace,
            source_id,
            target_id,
            relation,
            weight,
            description
        ],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM relationships WHERE source_id = ?1 AND target_id = ?2 AND relation = ?3",
        params![source_id, target_id, relation],
        |r| r.get(0),
    )?;
    Ok((id, true))
}

/// Removes a relation by id and cleans up memory_relationships.
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn delete_relationship_by_id(conn: &Connection, relationship_id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM memory_relationships WHERE relationship_id = ?1",
        params![relationship_id],
    )?;
    conn.execute(
        "DELETE FROM relationships WHERE id = ?1",
        params![relationship_id],
    )?;
    Ok(())
}

/// Recalculates the `degree` field of an entity.
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn recalculate_degree(conn: &Connection, entity_id: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE entities
         SET degree = (SELECT COUNT(*) FROM relationships
                       WHERE source_id = entities.id OR target_id = entities.id)
         WHERE id = ?1",
        params![entity_id],
    )?;
    Ok(())
}

/// Locates orphan entities: no link in memory_entities and no relations.
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn find_orphan_entity_ids(
    conn: &Connection,
    namespace: Option<&str>,
) -> Result<Vec<i64>, AppError> {
    if let Some(ns) = namespace {
        let mut stmt = conn.prepare_cached(
            "SELECT e.id FROM entities e
             WHERE e.namespace = ?1
               AND NOT EXISTS (SELECT 1 FROM memory_entities me WHERE me.entity_id = e.id)
               AND NOT EXISTS (
                   SELECT 1 FROM relationships r
                   WHERE r.source_id = e.id OR r.target_id = e.id
               )",
        )?;
        let ids = stmt
            .query_map(params![ns], |r| r.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    } else {
        let mut stmt = conn.prepare_cached(
            "SELECT e.id FROM entities e
             WHERE NOT EXISTS (SELECT 1 FROM memory_entities me WHERE me.entity_id = e.id)
               AND NOT EXISTS (
                   SELECT 1 FROM relationships r
                   WHERE r.source_id = e.id OR r.target_id = e.id
               )",
        )?;
        let ids = stmt
            .query_map([], |r| r.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }
}

/// Deletes entities and their associated vectors. Returns the number of entities removed.
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn delete_entities_by_ids(conn: &Connection, entity_ids: &[i64]) -> Result<usize, AppError> {
    if entity_ids.is_empty() {
        return Ok(0);
    }
    let mut removed = 0usize;
    for id in entity_ids {
        // FK CASCADE on entity_embeddings handles cleanup automatically.
        let _ = conn.execute("DELETE FROM vec_entities WHERE entity_id = ?1", params![id]);
        let affected = conn.execute("DELETE FROM entities WHERE id = ?1", params![id])?;
        removed += affected;
    }
    Ok(removed)
}

/// Counts relationships matching the given relation type within a namespace.
///
/// Used by `prune-relations --dry-run` to preview the number of relationships
/// that would be deleted without actually modifying the database.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn count_relationships_by_relation(
    conn: &Connection,
    namespace: &str,
    relation: &str,
) -> Result<usize, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relationships WHERE namespace = ?1 AND relation = ?2",
        params![namespace, relation],
        |r| r.get(0),
    )?;
    Ok(count as usize)
}

/// Returns unique entity names involved in relationships of the given type.
///
/// Queries both source and target sides of every matching relationship row,
/// deduplicates via `DISTINCT`, and returns the names in alphabetical order.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn list_entity_names_by_relation(
    conn: &Connection,
    namespace: &str,
    relation: &str,
) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT e.name FROM entities e
         INNER JOIN relationships r ON (e.id = r.source_id OR e.id = r.target_id)
         WHERE r.namespace = ?1 AND r.relation = ?2
         ORDER BY e.name",
    )?;
    let names: Vec<String> = stmt
        .query_map(params![namespace, relation], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names)
}

/// Deletes all relationships matching a relation type within a namespace.
///
/// Operates in chunks of 1000 to avoid holding long write locks and blocking
/// WAL readers. After deletion, recalculates degree for every affected entity.
///
/// Returns `(count_deleted, affected_entity_ids)`.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn delete_relationships_by_relation(
    conn: &Connection,
    namespace: &str,
    relation: &str,
) -> Result<(usize, Vec<i64>), AppError> {
    // Step 1: collect all affected entity IDs before deletion.
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT source_id FROM relationships WHERE namespace = ?1 AND relation = ?2
         UNION
         SELECT DISTINCT target_id FROM relationships WHERE namespace = ?1 AND relation = ?2",
    )?;
    let entity_ids: Vec<i64> = stmt
        .query_map(params![namespace, relation], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    // Step 2: collect relationship IDs to delete.
    let mut id_stmt =
        conn.prepare_cached("SELECT id FROM relationships WHERE namespace = ?1 AND relation = ?2")?;
    let rel_ids: Vec<i64> = id_stmt
        .query_map(params![namespace, relation], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    // Step 3: delete in chunks of 1000 (memory_relationships + relationships).
    let mut total_deleted: usize = 0;
    for chunk in rel_ids.chunks(1000) {
        for &rel_id in chunk {
            conn.execute(
                "DELETE FROM memory_relationships WHERE relationship_id = ?1",
                params![rel_id],
            )?;
            let affected =
                conn.execute("DELETE FROM relationships WHERE id = ?1", params![rel_id])?;
            total_deleted += affected;
        }
    }

    // Step 4: recalculate degree for all affected entities.
    for &eid in &entity_ids {
        recalculate_degree(conn, eid)?;
    }

    Ok((total_deleted, entity_ids))
}
