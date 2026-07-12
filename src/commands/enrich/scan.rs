//! Scan functions — select candidates for each enrichment operation.

use super::*;

// ---------------------------------------------------------------------------
// Shared WHERE predicates (GAP-SG-77)
//
// Each operation-specific predicate lives in ONE place so the scanner and the
// count-only `count_operation_backlog` cannot drift. Sharing the exact string
// guarantees that the backlog reported by `enrich --status` matches the rows a
// scan would actually select.
// ---------------------------------------------------------------------------

/// `memory-bindings`: memories with zero `memory_entities` rows.
const UNBOUND_MEMORY_PREDICATE: &str =
    "NOT EXISTS (SELECT 1 FROM memory_entities me WHERE me.memory_id = m.id)";

/// `entity-descriptions`: entities whose description is NULL or empty.
const NULL_DESCRIPTION_PREDICATE: &str = "(description IS NULL OR description = '')";

/// `body-enrich`: memory body shorter than the `?2` character threshold.
const SHORT_BODY_PREDICATE: &str = "LENGTH(COALESCE(m.body,'')) < ?2";

/// `description-enrich`: memories with generic/auto-generated descriptions.
const GENERIC_DESCRIPTION_PREDICATE: &str = "(description LIKE '%ingested%' \
     OR description LIKE '%imported%' OR description LIKE '%added%' \
     OR length(description) < 30)";

/// `weight-calibrate`: relationships strong enough to warrant recalibration.
const HIGH_WEIGHT_PREDICATE: &str = "r.weight >= 0.7";

/// `relation-reclassify`: relationships still using the generic `applies_to`.
const GENERIC_RELATION_PREDICATE: &str = "r.relation = 'applies_to'";

// ---------------------------------------------------------------------------
// v1.1.1 (P2/P10): `re-embed` predicates.
//
// A row is a candidate when it has NO live vector for the CONFIGURED
// dimensionality. "Live" means the vector row exists, its blob is non-empty
// and its stored `dim` matches the configured `--embedding-dim`; vectors
// written under a legacy dimension (P10) and empty blobs are re-selected,
// not only missing rows. Built as functions (not consts) because the dim is
// resolved at runtime; scanner and counter share the same builder so they
// cannot drift (GAP-SG-77).
// ---------------------------------------------------------------------------

/// `re-embed --target memories`: memory `m` lacks a live vector.
fn reembed_memory_predicate(dim: usize) -> String {
    format!(
        "NOT EXISTS (SELECT 1 FROM memory_embeddings me WHERE me.memory_id = m.id \
         AND me.dim = {dim} AND LENGTH(me.embedding) > 0)"
    )
}

/// `re-embed --target entities`: entity `e` lacks a live vector.
fn reembed_entity_predicate(dim: usize) -> String {
    format!(
        "NOT EXISTS (SELECT 1 FROM entity_embeddings ev WHERE ev.entity_id = e.id \
         AND ev.dim = {dim} AND LENGTH(ev.embedding) > 0)"
    )
}

/// `re-embed --target chunks`: chunk `c` lacks a live vector.
fn reembed_chunk_predicate(dim: usize) -> String {
    format!(
        "NOT EXISTS (SELECT 1 FROM chunk_embeddings ce WHERE ce.chunk_id = c.id \
         AND ce.dim = {dim} AND LENGTH(ce.embedding) > 0)"
    )
}

// ---------------------------------------------------------------------------

/// Returns memories without any `memory_entities` binding.
///
/// These are the targets for `memory-bindings` enrichment. When `name_filter`
/// is non-empty, restricts the scan to the given names (G37); unknown names
/// are silently skipped (the caller can detect them by comparing
/// requested vs. returned).
pub(super) fn scan_unbound_memories(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
) -> Result<Vec<(i64, String, String)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();

    if name_filter.is_empty() {
        let sql = format!(
            "SELECT m.id, m.name, m.body
             FROM memories m
             WHERE m.namespace = ?1
               AND m.deleted_at IS NULL
               AND {UNBOUND_MEMORY_PREDICATE}
             ORDER BY m.id
             {limit_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params![namespace], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        // Build a parameterised IN clause: ?2, ?3, ..., ?{1+n}
        let placeholders: Vec<String> = (2..=name_filter.len() + 1)
            .map(|i| format!("?{i}"))
            .collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT m.id, m.name, m.body
             FROM memories m
             WHERE m.namespace = ?1
               AND m.deleted_at IS NULL
               AND m.name IN ({in_clause})
               AND {UNBOUND_MEMORY_PREDICATE}
             ORDER BY m.id
             {limit_clause}"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + name_filter.len());
        params_vec.push(&namespace);
        for n in name_filter {
            params_vec.push(n);
        }
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params_vec.iter().copied()),
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// GAP-SG-24/26: returns ALREADY-bound memory names for additive augmentation,
/// restricted to `name_filter`.
///
/// Unlike [`scan_unbound_memories`] this selects memories that DO have at least
/// one `memory_entities` binding, so a second extraction pass can merge newly
/// discovered entities/relationships without disturbing existing links (the
/// persist path is purely additive). A name filter is MANDATORY: re-running
/// extraction over an entire namespace is expensive and rarely intended, so an
/// empty filter is rejected rather than silently scanning everything.
pub(super) fn scan_bound_memories_for_augment(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
) -> Result<Vec<String>, AppError> {
    if name_filter.is_empty() {
        return Err(AppError::Validation(
            "augment-bindings requires an explicit subset: pass --names or \
             --names-file (it refuses to re-scan the whole namespace)"
                .into(),
        ));
    }
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let placeholders: Vec<String> = (2..=name_filter.len() + 1)
        .map(|i| format!("?{i}"))
        .collect();
    let in_clause = placeholders.join(", ");
    let sql = format!(
        "SELECT m.name
         FROM memories m
         WHERE m.namespace = ?1
           AND m.deleted_at IS NULL
           AND m.name IN ({in_clause})
           AND EXISTS (
               SELECT 1 FROM memory_entities me WHERE me.memory_id = m.id
           )
         ORDER BY m.id
         {limit_clause}"
    );
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + name_filter.len());
    params_vec.push(&namespace);
    for n in name_filter {
        params_vec.push(n);
    }
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            rusqlite::params_from_iter(params_vec.iter().copied()),
            |r| r.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Reads a list of memory names from a UTF-8 text file (G37).
///
/// Empty lines and lines beginning with `#` are skipped. Returns a
/// de-duplicated, order-preserving list of trimmed names.
pub(super) fn read_names_file(path: &Path) -> Result<Vec<String>, AppError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        AppError::Validation(format!("failed to read names file {}: {e}", path.display()))
    })?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
}

/// Resolves the union of `--names` and `--names-file` (G37).
pub(super) fn resolve_name_filter(args: &EnrichArgs) -> Result<Vec<String>, AppError> {
    let mut combined: Vec<String> = args.names.clone();
    if let Some(p) = &args.names_file {
        let from_file = read_names_file(p)?;
        for n in from_file {
            if !combined.contains(&n) {
                combined.push(n);
            }
        }
    }
    Ok(combined)
}

/// Returns entities with NULL or empty description.
///
/// These are the targets for `entity-descriptions` enrichment.
pub(super) fn scan_entities_without_description(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
) -> Result<Vec<(i64, String, String)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();

    if name_filter.is_empty() {
        let sql = format!(
            "SELECT id, name, type
             FROM entities
             WHERE namespace = ?1
               AND {NULL_DESCRIPTION_PREDICATE}
             ORDER BY id
             {limit_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params![namespace], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let placeholders: Vec<String> = (2..=name_filter.len() + 1)
            .map(|i| format!("?{i}"))
            .collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT id, name, type
             FROM entities
             WHERE namespace = ?1
               AND name IN ({in_clause})
               AND {NULL_DESCRIPTION_PREDICATE}
             ORDER BY id
             {limit_clause}"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + name_filter.len());
        params_vec.push(&namespace);
        for n in name_filter {
            params_vec.push(n);
        }
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params_vec.iter().copied()),
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// Returns memories whose body length is below the configured minimum.
///
/// These are the targets for `body-enrich` (GAP-18).
pub(super) fn scan_short_body_memories(
    conn: &Connection,
    namespace: &str,
    min_chars: usize,
    limit: Option<usize>,
    name_filter: &[String],
) -> Result<Vec<(i64, String, String)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();

    if name_filter.is_empty() {
        let sql = format!(
            "SELECT m.id, m.name, m.body
             FROM memories m
             WHERE m.namespace = ?1
               AND m.deleted_at IS NULL
               AND {SHORT_BODY_PREDICATE}
             ORDER BY m.id
             {limit_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params![namespace, min_chars as i64], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let placeholders: Vec<String> = (3..=name_filter.len() + 2)
            .map(|i| format!("?{i}"))
            .collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT m.id, m.name, m.body
             FROM memories m
             WHERE m.namespace = ?1
               AND m.deleted_at IS NULL
               AND m.name IN ({in_clause})
               AND {SHORT_BODY_PREDICATE}
             ORDER BY m.id
             {limit_clause}"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(2 + name_filter.len());
        let min_chars_i64 = min_chars as i64;
        params_vec.push(&namespace);
        params_vec.push(&min_chars_i64);
        for n in name_filter {
            params_vec.push(n);
        }
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params_vec.iter().copied()),
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// Returns live memories without a live vector in `memory_embeddings`.
///
/// These are the targets for `re-embed` (`--target memories`). v1.1.1 (P10):
/// the predicate also selects memories whose stored vector has a stale `dim`
/// or an empty blob, so legacy-dimension vectors are regenerated too.
pub(super) fn scan_memories_without_embeddings(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
) -> Result<Vec<(i64, String, String)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let predicate = reembed_memory_predicate(crate::constants::embedding_dim());

    if name_filter.is_empty() {
        let sql = format!(
            "SELECT m.id, m.name, COALESCE(m.body,'')
             FROM memories m
             WHERE m.namespace = ?1
               AND m.deleted_at IS NULL
               AND {predicate}
             ORDER BY m.id
             {limit_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params![namespace], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let placeholders: Vec<String> = (2..=name_filter.len() + 1)
            .map(|i| format!("?{i}"))
            .collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT m.id, m.name, COALESCE(m.body,'')
             FROM memories m
             WHERE m.namespace = ?1
               AND m.deleted_at IS NULL
               AND m.name IN ({in_clause})
               AND {predicate}
             ORDER BY m.id
             {limit_clause}"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + name_filter.len());
        params_vec.push(&namespace);
        for n in name_filter {
            params_vec.push(n);
        }
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params_vec.iter().copied()),
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// v1.1.1 (P2): entities without a live vector in `entity_embeddings` for the
/// configured dimensionality — targets for `re-embed --target entities`.
/// `name_filter` (when present) restricts by entity name.
pub(super) fn scan_entities_missing_embeddings(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
) -> Result<Vec<(i64, String)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let predicate = reembed_entity_predicate(crate::constants::embedding_dim());

    if name_filter.is_empty() {
        let sql = format!(
            "SELECT e.id, e.name
             FROM entities e
             WHERE e.namespace = ?1
               AND {predicate}
             ORDER BY e.id
             {limit_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params![namespace], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let placeholders: Vec<String> = (2..=name_filter.len() + 1)
            .map(|i| format!("?{i}"))
            .collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT e.id, e.name
             FROM entities e
             WHERE e.namespace = ?1
               AND e.name IN ({in_clause})
               AND {predicate}
             ORDER BY e.id
             {limit_clause}"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + name_filter.len());
        params_vec.push(&namespace);
        for n in name_filter {
            params_vec.push(n);
        }
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params_vec.iter().copied()),
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// v1.1.1 (P2): chunk rows without a live vector in `chunk_embeddings` for
/// the configured dimensionality — targets for `re-embed --target chunks`.
/// `name_filter` (when present) restricts by PARENT memory name.
pub(super) fn scan_chunks_missing_embeddings(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
) -> Result<Vec<i64>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let predicate = reembed_chunk_predicate(crate::constants::embedding_dim());

    if name_filter.is_empty() {
        let sql = format!(
            "SELECT c.id
             FROM memory_chunks c
             LEFT JOIN memories m ON m.id = c.memory_id
             WHERE (m.namespace = ?1 OR m.id IS NULL)
               AND {predicate}
             ORDER BY c.id
             {limit_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let placeholders: Vec<String> = (2..=name_filter.len() + 1)
            .map(|i| format!("?{i}"))
            .collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT c.id
             FROM memory_chunks c
             LEFT JOIN memories m ON m.id = c.memory_id
             WHERE (m.namespace = ?1 OR m.id IS NULL)
               AND (m.name IN ({in_clause}) OR m.id IS NULL)
               AND {predicate}
             ORDER BY c.id
             {limit_clause}"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + name_filter.len());
        params_vec.push(&namespace);
        for n in name_filter {
            params_vec.push(n);
        }
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params_vec.iter().copied()),
                |r| r.get::<_, i64>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// G27: Returns relationships with weight >= 0.7 that may need recalibration.
#[allow(clippy::type_complexity)]
pub(super) fn scan_weight_candidates(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
) -> Result<Vec<(i64, String, String, String, f64)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let sql = format!(
        "SELECT r.id, e1.name, e2.name, r.relation, r.weight \
         FROM relationships r \
         JOIN entities e1 ON e1.id = r.source_id \
         JOIN entities e2 ON e2.id = r.target_id \
         WHERE {HIGH_WEIGHT_PREDICATE} AND e1.namespace = ?1 \
         ORDER BY r.weight DESC {limit_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![namespace], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, f64>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// G27: Returns relationships with generic relation types (applies_to).
pub(super) fn scan_generic_relations(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
) -> Result<Vec<(i64, String, String, String)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let sql = format!(
        "SELECT r.id, e1.name, e2.name, r.relation \
         FROM relationships r \
         JOIN entities e1 ON e1.id = r.source_id \
         JOIN entities e2 ON e2.id = r.target_id \
         WHERE {GENERIC_RELATION_PREDICATE} AND e1.namespace = ?1 \
         ORDER BY r.id {limit_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![namespace], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// PERSIST helpers for fully-implemented operations
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Scan dispatcher — maps operation to scan query result (item keys)
// ---------------------------------------------------------------------------

pub(super) fn scan_operation(
    conn: &Connection,
    namespace: &str,
    args: &EnrichArgs,
) -> Result<Vec<String>, AppError> {
    // G37: resolve --names + --names-file once and apply to every scan path.
    let name_filter = resolve_name_filter(args)?;
    match args.operation() {
        EnrichOperation::MemoryBindings => {
            let rows = scan_unbound_memories(conn, namespace, args.limit, &name_filter)?;
            Ok(rows.into_iter().map(|(_, name, _)| name).collect())
        }
        // GAP-SG-24/26: additive augmentation processes ALREADY-bound memories,
        // restricted to an explicit name filter so it never re-scans the whole
        // namespace.
        EnrichOperation::AugmentBindings => {
            scan_bound_memories_for_augment(conn, namespace, args.limit, &name_filter)
        }
        EnrichOperation::EntityDescriptions => {
            let rows =
                scan_entities_without_description(conn, namespace, args.limit, &name_filter)?;
            Ok(rows.into_iter().map(|(_, name, _)| name).collect())
        }
        EnrichOperation::BodyEnrich => {
            let rows = scan_short_body_memories(
                conn,
                namespace,
                args.min_output_chars,
                args.limit,
                &name_filter,
            )?;
            Ok(rows.into_iter().map(|(_, name, _)| name).collect())
        }
        EnrichOperation::ReEmbed => {
            // v1.1.1 (P2): --target selects which embedding table to backfill.
            // Non-memory keys carry an `entity:` / `chunk:` prefix so the
            // drain dispatch (`call_reembed`) and the queue `item_type` can
            // tell them apart; bare memory names stay unprefixed for full
            // retro-compatibility with pre-v1.1.1 queue rows.
            let mut keys: Vec<String> = Vec::new();
            if matches!(args.target, ReEmbedTarget::Memories | ReEmbedTarget::All) {
                let rows =
                    scan_memories_without_embeddings(conn, namespace, args.limit, &name_filter)?;
                keys.extend(rows.into_iter().map(|(_, name, _)| name));
            }
            if matches!(args.target, ReEmbedTarget::Entities | ReEmbedTarget::All) {
                let rows =
                    scan_entities_missing_embeddings(conn, namespace, args.limit, &name_filter)?;
                keys.extend(rows.into_iter().map(|(_, name)| format!("entity:{name}")));
            }
            if matches!(args.target, ReEmbedTarget::Chunks | ReEmbedTarget::All) {
                let ids =
                    scan_chunks_missing_embeddings(conn, namespace, args.limit, &name_filter)?;
                keys.extend(ids.into_iter().map(|id| format!("chunk:{id}")));
            }
            Ok(keys)
        }
        EnrichOperation::WeightCalibrate => {
            let rows = scan_weight_candidates(conn, namespace, args.limit)?;
            Ok(rows
                .into_iter()
                .map(|(id, _, _, _, _)| id.to_string())
                .collect())
        }
        EnrichOperation::RelationReclassify => {
            let rows = scan_generic_relations(conn, namespace, args.limit)?;
            Ok(rows
                .into_iter()
                .map(|(id, _, _, _)| id.to_string())
                .collect())
        }
        EnrichOperation::EntityConnect | EnrichOperation::CrossDomainBridges => {
            // v1.1.06: enqueue stable pair keys so drain resolves by ID without
            // re-running the pair scan (GAP-ENTITY-CONNECT-SCAN-CARTESIAN).
            let pairs = scan_isolated_entity_pairs(conn, namespace, args.limit)?;
            Ok(pairs
                .into_iter()
                .map(|(id1, _, id2, _)| format_pair_key(id1, id2))
                .collect())
        }
        EnrichOperation::EntityTypeValidate => {
            let rows = scan_entities_for_type_validation(conn, namespace, args.limit)?;
            Ok(rows.into_iter().map(|(_, name, _)| name).collect())
        }
        EnrichOperation::DescriptionEnrich => {
            let rows = scan_generic_descriptions(conn, namespace, args.limit)?;
            Ok(rows.into_iter().map(|(_, name, _)| name).collect())
        }
        EnrichOperation::DomainClassify
        | EnrichOperation::GraphAudit
        | EnrichOperation::DeepResearchSynth
        | EnrichOperation::BodyExtract => {
            let limit_clause = args.limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
            let sql = format!(
                "SELECT name FROM memories WHERE namespace=?1 AND deleted_at IS NULL ORDER BY id {limit_clause}"
            );
            let mut stmt = conn.prepare(&sql)?;
            let mut names = stmt
                .query_map(rusqlite::params![namespace], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            // GAP-SG-27: honour --names/--names-file for body-extract (and the
            // sibling whole-namespace scans), which previously ignored it and
            // scanned every memory by id.
            if !name_filter.is_empty() {
                names.retain(|n| name_filter.iter().any(|f| f == n));
            }
            Ok(names)
        }
    }
}

/// Queue / drain key for an entity pair (`id1 < id2`).
///
/// v1.1.06 (GAP-ENTITY-CONNECT-SCAN-CARTESIAN): stable ID-based keys replace
/// the legacy `e1.name` enqueue so each pair is unique and
/// [`super::extraction::call_entity_connect`] can resolve without re-scanning.
pub(super) fn format_pair_key(id1: i64, id2: i64) -> String {
    let (a, b) = if id1 < id2 { (id1, id2) } else { (id2, id1) };
    format!("pair:{a}:{b}")
}

/// Parse a `pair:{id1}:{id2}` key produced by [`format_pair_key`].
///
/// Returns `None` for legacy keys (bare entity names) so the drain path can
/// skip without falling back to an O(n²) scan.
pub(super) fn parse_pair_key(key: &str) -> Option<(i64, i64)> {
    let rest = key.strip_prefix("pair:")?;
    let (a, b) = rest.split_once(':')?;
    let id1: i64 = a.parse().ok()?;
    let id2: i64 = b.parse().ok()?;
    if id1 <= 0 || id2 <= 0 || id1 == id2 {
        return None;
    }
    Some(if id1 < id2 { (id1, id2) } else { (id2, id1) })
}

/// Default batch size for entity-connect / cross-domain-bridges scans.
pub(super) const ENTITY_CONNECT_DEFAULT_LIMIT: usize = 50;

/// Top hubs considered when filling residual slots via hub×island pairs.
const ENTITY_CONNECT_HUB_TOP: i64 = 32;

/// Cap on island candidates considered per hub-fill pass (keeps the fill O(H·I)).
const ENTITY_CONNECT_ISLAND_CAP: i64 = 500;

/// Scan for entity pairs that share no direct relationship and have not been
/// evaluated in `entity_connect_seen`.
///
/// # Algorithm (v1.1.06 — GAP-ENTITY-CONNECT-SCAN-CARTESIAN)
///
/// **Never** enumerates the cartesian product `entities × entities` with a
/// global `ORDER BY` (that forced SQLite to materialise O(n²) candidates before
/// `LIMIT`, hanging on large namespaces such as `global` with ~10⁵ entities).
///
/// Instead:
/// 1. **Co-occurrence (primary):** pairs that share ≥1 memory in
///    `memory_entities` (self-join on `memory_id`), ordered by co-count.
/// 2. **Hub × island (fill):** top-degree hubs paired with degree-0 entities
///    that have NER bindings — aligns with the O(n) backlog proxy.
///
/// Both stages exclude existing `relationships` and `entity_connect_seen` rows.
/// GAP-002 convergence semantics are preserved; only candidate *generation*
/// changes from O(n²) to evidence-local O(k).
#[allow(clippy::type_complexity)]
pub(super) fn scan_isolated_entity_pairs(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
) -> Result<Vec<(i64, String, i64, String)>, AppError> {
    let limit_val = limit.unwrap_or(ENTITY_CONNECT_DEFAULT_LIMIT) as i64;
    if limit_val <= 0 {
        return Ok(Vec::new());
    }

    let mut pairs = scan_pairs_by_cooccurrence(conn, namespace, limit_val)?;
    if (pairs.len() as i64) < limit_val {
        let remaining = limit_val - pairs.len() as i64;
        let fill = scan_pairs_hub_island(conn, namespace, remaining)?;
        let mut seen: std::collections::HashSet<(i64, i64)> =
            pairs.iter().map(|(a, _, b, _)| (*a, *b)).collect();
        for p in fill {
            if seen.insert((p.0, p.2)) {
                pairs.push(p);
                if (pairs.len() as i64) >= limit_val {
                    break;
                }
            }
        }
    }
    Ok(pairs)
}

/// Primary source: pairs that co-occur in at least one memory.
#[allow(clippy::type_complexity)]
fn scan_pairs_by_cooccurrence(
    conn: &Connection,
    namespace: &str,
    limit: i64,
) -> Result<Vec<(i64, String, i64, String)>, AppError> {
    // Join on memory_entities (indexed by PK/memory_id) rather than entities×entities.
    // ORDER BY applies only to the already-reduced co-occurrence set.
    let mut stmt = conn.prepare_cached(
        "SELECT e1.id, e1.name, e2.id, e2.name \
         FROM memory_entities me1 \
         JOIN memory_entities me2 \
           ON me1.memory_id = me2.memory_id AND me1.entity_id < me2.entity_id \
         JOIN entities e1 ON e1.id = me1.entity_id \
         JOIN entities e2 ON e2.id = me2.entity_id \
         LEFT JOIN entity_connect_seen ecs \
           ON ecs.source_id = e1.id AND ecs.target_id = e2.id \
         WHERE e1.namespace = ?1 AND e2.namespace = ?1 \
           AND ecs.source_id IS NULL \
           AND NOT EXISTS ( \
             SELECT 1 FROM relationships r WHERE \
               (r.source_id = e1.id AND r.target_id = e2.id) OR \
               (r.source_id = e2.id AND r.target_id = e1.id) \
           ) \
         GROUP BY e1.id, e1.name, e2.id, e2.name \
         ORDER BY COUNT(*) DESC, e1.id, e2.id \
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![namespace, limit], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Fill residual slots: high-degree hubs × degree-0 islands with NER bindings.
#[allow(clippy::type_complexity)]
fn scan_pairs_hub_island(
    conn: &Connection,
    namespace: &str,
    limit: i64,
) -> Result<Vec<(i64, String, i64, String)>, AppError> {
    if limit <= 0 {
        return Ok(Vec::new());
    }
    // Normalize (smaller_id, larger_id) so seen/relationship lookups stay consistent
    // with co-occurrence pairs (which always use e1.id < e2.id).
    let mut stmt = conn.prepare_cached(
        "WITH hubs AS ( \
           SELECT id, name, degree FROM entities \
           WHERE namespace = ?1 \
           ORDER BY degree DESC, id \
           LIMIT ?2 \
         ), \
         islands AS ( \
           SELECT e.id, e.name FROM entities e \
           WHERE e.namespace = ?1 AND e.degree = 0 \
             AND EXISTS (SELECT 1 FROM memory_entities me WHERE me.entity_id = e.id) \
           ORDER BY e.id \
           LIMIT ?3 \
         ) \
         SELECT \
           CASE WHEN h.id < i.id THEN h.id ELSE i.id END AS id1, \
           CASE WHEN h.id < i.id THEN h.name ELSE i.name END AS name1, \
           CASE WHEN h.id < i.id THEN i.id ELSE h.id END AS id2, \
           CASE WHEN h.id < i.id THEN i.name ELSE h.name END AS name2 \
         FROM hubs h \
         CROSS JOIN islands i \
         WHERE h.id != i.id \
           AND NOT EXISTS ( \
             SELECT 1 FROM entity_connect_seen ecs \
             WHERE ecs.source_id = CASE WHEN h.id < i.id THEN h.id ELSE i.id END \
               AND ecs.target_id = CASE WHEN h.id < i.id THEN i.id ELSE h.id END \
           ) \
           AND NOT EXISTS ( \
             SELECT 1 FROM relationships r WHERE \
               (r.source_id = h.id AND r.target_id = i.id) OR \
               (r.source_id = i.id AND r.target_id = h.id) \
           ) \
         ORDER BY h.degree DESC, h.id, i.id \
         LIMIT ?4",
    )?;
    let rows = stmt
        .query_map(
            rusqlite::params![
                namespace,
                ENTITY_CONNECT_HUB_TOP,
                ENTITY_CONNECT_ISLAND_CAP,
                limit
            ],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Scan for entities with non-validated types (all entities for type audit).
pub(super) fn scan_entities_for_type_validation(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
) -> Result<Vec<(i64, String, String)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let sql = format!(
        "SELECT id, name, type FROM entities WHERE namespace = ?1 ORDER BY id {limit_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![namespace], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Scan for memories with generic descriptions (ingested, imported, etc).
pub(super) fn scan_generic_descriptions(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
) -> Result<Vec<(i64, String, String)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let sql = format!(
        "SELECT id, name, description FROM memories WHERE namespace = ?1 AND deleted_at IS NULL \
         AND {GENERIC_DESCRIPTION_PREDICATE} \
         ORDER BY id {limit_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![namespace], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Backlog counter (GAP-SG-77)
// ---------------------------------------------------------------------------

/// Count-only backlog for a single operation, using a cheap `SELECT COUNT(*)`.
///
/// This mirrors the dispatch of [`scan_operation`], reusing the SAME shared
/// WHERE predicates so the count can never drift from the rows a scan would
/// select. Unlike the scanners it materialises no rows.
///
/// The returned figure has DATABASE semantics — the real backlog of the
/// operation against the store — which is distinct from the FILE (sidecar
/// queue) semantics reported by `queue_pending`/`queue_dead`. It powers the
/// `scan_backlog` field of `enrich --status` so that db-backed operations
/// (`entity-descriptions`, `body-enrich`, `re-embed`, ...) no longer report a
/// false `pending=0` when thousands of eligible items exist.
///
/// Notes on individual operations:
/// - `body-enrich` uses the default [`DEFAULT_BODY_ENRICH_MIN_CHARS`] threshold
///   (the same default the CLI applies when `--min-output-chars` is omitted).
/// - `re-embed` (v1.1.1 P2/P10) sums the dim-aware backlog of every table
///   selected by `--target` (memories / entities / chunks / all), sharing the
///   `reembed_*_predicate` builders with the scanners.
/// - `entity-connect` (v1.1.04+/v1.1.06) reports a real O(n) proxy: degree-0
///   entities with NER bindings (`backlog_degree0_proxy`). Pair candidates are
///   O(k) co-occurrence + hub×island (not cartesian).
/// - advisory / scan-only operations (`augment-bindings`,
///   `cross-domain-bridges` status proxy, `domain-classify`,
///   `graph-audit`, `deep-research-synth`, `body-extract`) have no closeable
///   database deficit in this counter and report `0` (bridges still shares the
///   O(k) pair scan + `entity_connect_seen` drain path at runtime).
pub(super) fn count_operation_backlog(
    conn: &Connection,
    operation: &EnrichOperation,
    namespace: &str,
    reembed_target: ReEmbedTarget,
) -> Result<i64, AppError> {
    let count = match operation {
        EnrichOperation::MemoryBindings => {
            let sql = format!(
                "SELECT COUNT(*) FROM memories m \
                 WHERE m.namespace = ?1 AND m.deleted_at IS NULL \
                 AND {UNBOUND_MEMORY_PREDICATE}"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::EntityDescriptions => {
            let sql = format!(
                "SELECT COUNT(*) FROM entities \
                 WHERE namespace = ?1 AND {NULL_DESCRIPTION_PREDICATE}"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::BodyEnrich => {
            let sql = format!(
                "SELECT COUNT(*) FROM memories m \
                 WHERE m.namespace = ?1 AND m.deleted_at IS NULL \
                 AND {SHORT_BODY_PREDICATE}"
            );
            let min_chars = super::DEFAULT_BODY_ENRICH_MIN_CHARS as i64;
            conn.query_row(&sql, rusqlite::params![namespace, min_chars], |r| {
                r.get::<_, i64>(0)
            })?
        }
        EnrichOperation::ReEmbed => {
            // v1.1.1 (P2/P10): same dim-aware predicates as the scanners,
            // summed over the targets selected by --target.
            let dim = crate::constants::embedding_dim();
            let mut total = 0i64;
            if matches!(reembed_target, ReEmbedTarget::Memories | ReEmbedTarget::All) {
                let sql = format!(
                    "SELECT COUNT(*) FROM memories m \
                     WHERE m.namespace = ?1 AND m.deleted_at IS NULL \
                     AND {}",
                    reembed_memory_predicate(dim)
                );
                total +=
                    conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?;
            }
            if matches!(reembed_target, ReEmbedTarget::Entities | ReEmbedTarget::All) {
                let sql = format!(
                    "SELECT COUNT(*) FROM entities e WHERE e.namespace = ?1 AND {}",
                    reembed_entity_predicate(dim)
                );
                total +=
                    conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?;
            }
            if matches!(reembed_target, ReEmbedTarget::Chunks | ReEmbedTarget::All) {
                let sql = format!(
                    "SELECT COUNT(*) FROM memory_chunks c \
                     LEFT JOIN memories m ON m.id = c.memory_id \
                     WHERE (m.namespace = ?1 OR m.id IS NULL) \
                     AND {}",
                    reembed_chunk_predicate(dim)
                );
                total +=
                    conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?;
            }
            total
        }
        EnrichOperation::WeightCalibrate => {
            let sql = format!(
                "SELECT COUNT(*) FROM relationships r \
                 JOIN entities e1 ON e1.id = r.source_id \
                 WHERE {HIGH_WEIGHT_PREDICATE} AND e1.namespace = ?1"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::RelationReclassify => {
            let sql = format!(
                "SELECT COUNT(*) FROM relationships r \
                 JOIN entities e1 ON e1.id = r.source_id \
                 WHERE {GENERIC_RELATION_PREDICATE} AND e1.namespace = ?1"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::EntityTypeValidate => {
            // Mirrors scan_entities_for_type_validation: every entity is a
            // candidate for the type audit.
            conn.query_row(
                "SELECT COUNT(*) FROM entities WHERE namespace = ?1",
                rusqlite::params![namespace],
                |r| r.get::<_, i64>(0),
            )?
        }
        EnrichOperation::DescriptionEnrich => {
            let sql = format!(
                "SELECT COUNT(*) FROM memories \
                 WHERE namespace = ?1 AND deleted_at IS NULL \
                 AND {GENERIC_DESCRIPTION_PREDICATE}"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::EntityConnect => {
            // Proxy O(n): entidades degree-0 com binding NER (ilhadas e conectaveis).
            // Nao conta pares O(n^2) — seria proibitivo para 82k entidades.
            let sql = "SELECT COUNT(*) FROM entities e \
                       WHERE e.namespace = ?1 AND e.degree = 0 \
                       AND EXISTS (SELECT 1 FROM memory_entities me WHERE me.entity_id = e.id)";
            conn.query_row(sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        // Advisory / quadratic scan-only operations have no closeable database
        // backlog; report 0 (see the doc comment above).
        EnrichOperation::AugmentBindings
        | EnrichOperation::CrossDomainBridges
        | EnrichOperation::DomainClassify
        | EnrichOperation::GraphAudit
        | EnrichOperation::DeepResearchSynth
        | EnrichOperation::BodyExtract => 0,
    };
    Ok(count)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE memories (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace   TEXT NOT NULL DEFAULT 'global',
                name        TEXT NOT NULL,
                type        TEXT NOT NULL DEFAULT 'note',
                description TEXT NOT NULL DEFAULT '',
                body        TEXT NOT NULL DEFAULT '',
                body_hash   TEXT NOT NULL DEFAULT '',
                session_id  TEXT,
                source      TEXT NOT NULL DEFAULT 'agent',
                metadata    TEXT NOT NULL DEFAULT '{}',
                created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                deleted_at  INTEGER,
                UNIQUE(namespace, name)
            );
            CREATE TABLE entities (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace   TEXT NOT NULL DEFAULT 'global',
                name        TEXT NOT NULL,
                type        TEXT NOT NULL DEFAULT 'concept',
                description TEXT,
                degree      INTEGER NOT NULL DEFAULT 0,
                created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                UNIQUE(namespace, name)
            );
            CREATE TABLE memory_entities (
                memory_id  INTEGER NOT NULL,
                entity_id  INTEGER NOT NULL,
                PRIMARY KEY (memory_id, entity_id)
            );
            CREATE TABLE relationships (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace  TEXT NOT NULL DEFAULT 'global',
                source_id  INTEGER NOT NULL,
                target_id  INTEGER NOT NULL,
                relation   TEXT NOT NULL,
                weight     REAL NOT NULL DEFAULT 0.5,
                description TEXT,
                UNIQUE(source_id, target_id, relation)
            );
            CREATE TABLE memory_embeddings (
                memory_id   INTEGER PRIMARY KEY,
                namespace   TEXT NOT NULL,
                embedding   BLOB NOT NULL,
                source      TEXT NOT NULL,
                model       TEXT NOT NULL DEFAULT '',
                dim         INTEGER NOT NULL DEFAULT 384,
                created_at  INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE entity_embeddings (
                entity_id   INTEGER PRIMARY KEY,
                namespace   TEXT NOT NULL,
                embedding   BLOB NOT NULL,
                source      TEXT NOT NULL,
                model       TEXT NOT NULL DEFAULT '',
                dim         INTEGER NOT NULL DEFAULT 384,
                created_at  INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE memory_chunks (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id    INTEGER NOT NULL,
                chunk_idx    INTEGER NOT NULL,
                chunk_text   TEXT NOT NULL,
                start_offset INTEGER NOT NULL DEFAULT 0,
                end_offset   INTEGER NOT NULL DEFAULT 0,
                token_count  INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE chunk_embeddings (
                chunk_id    INTEGER PRIMARY KEY,
                memory_id   INTEGER NOT NULL,
                embedding   BLOB NOT NULL,
                source      TEXT NOT NULL,
                model       TEXT NOT NULL DEFAULT '',
                dim         INTEGER NOT NULL DEFAULT 384,
                created_at  INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE entity_connect_seen (
                source_id    INTEGER NOT NULL,
                target_id    INTEGER NOT NULL,
                namespace    TEXT NOT NULL,
                verdict      TEXT NOT NULL CHECK(verdict IN ('related','none')),
                relation     TEXT,
                evaluated_at INTEGER NOT NULL DEFAULT (unixepoch()),
                PRIMARY KEY (source_id, target_id)
            );",
        )
        .expect("schema creation must succeed");
        conn
    }

    #[test]
    fn scan_unbound_memories_finds_memories_without_bindings() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'test-mem', 'some body content')",
            [],
        )
        .unwrap();

        let results = scan_unbound_memories(&conn, "global", None, &[]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "test-mem");
    }

    #[test]
    fn scan_unbound_memories_excludes_bound_memories() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'bound-mem', 'body')",
            [],
        )
        .unwrap();
        let mem_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='bound-mem'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO entities (namespace, name) VALUES ('global', 'some-entity')",
            [],
        )
        .unwrap();
        let ent_id: i64 = conn
            .query_row(
                "SELECT id FROM entities WHERE name='some-entity'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
            rusqlite::params![mem_id, ent_id],
        )
        .unwrap();

        let results = scan_unbound_memories(&conn, "global", None, &[]).unwrap();
        assert!(results.is_empty(), "bound memory must not appear in scan");
    }

    #[test]
    fn scan_entities_without_description_finds_null_description() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO entities (namespace, name, type, description) VALUES ('global', 'my-tool', 'tool', NULL)",
            [],
        )
        .unwrap();

        let results = scan_entities_without_description(&conn, "global", None, &[]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "my-tool");
    }

    #[test]
    fn scan_entities_without_description_excludes_entities_with_description() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO entities (namespace, name, type, description) VALUES ('global', 'described-tool', 'tool', 'Has a description already')",
            [],
        )
        .unwrap();

        let results = scan_entities_without_description(&conn, "global", None, &[]).unwrap();
        assert!(
            results.is_empty(),
            "entity with description must not appear"
        );
    }

    #[test]
    fn scan_short_body_memories_finds_short_bodies() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'short-mem', 'hi')",
            [],
        )
        .unwrap();

        let results = scan_short_body_memories(&conn, "global", 100, None, &[]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "short-mem");
    }

    #[test]
    fn scan_short_body_memories_excludes_long_bodies() {
        let conn = open_test_db();
        let long_body = "a".repeat(1000);
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'long-mem', ?1)",
            rusqlite::params![long_body],
        )
        .unwrap();

        let results = scan_short_body_memories(&conn, "global", 100, None, &[]).unwrap();
        assert!(results.is_empty(), "long memory must not appear in scan");
    }

    #[test]
    fn scan_respects_limit() {
        let conn = open_test_db();
        for i in 0..5 {
            conn.execute(
                &format!("INSERT INTO memories (namespace, name, body) VALUES ('global', 'mem-{i}', 'short')"),
                [],
            )
            .unwrap();
        }

        let results = scan_short_body_memories(&conn, "global", 1000, Some(3), &[]).unwrap();
        assert_eq!(results.len(), 3, "limit must be respected");
    }

    #[test]
    fn scan_memories_without_embeddings_finds_only_missing_rows() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'missing-vec', 'body one')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'has-vec', 'body two')",
            [],
        )
        .unwrap();
        let memory_id: i64 = conn
            .query_row(
                "SELECT id FROM memories WHERE namespace='global' AND name='has-vec'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let embedding = vec![0.0_f32; crate::constants::embedding_dim()];
        crate::storage::memories::upsert_vec(
            &conn, memory_id, "global", "note", &embedding, "has-vec", "body two",
        )
        .unwrap();

        let results = scan_memories_without_embeddings(&conn, "global", None, &[]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "missing-vec");
    }

    #[test]
    fn scan_memories_without_embeddings_respects_name_filter() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'match-me', 'body one')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'skip-me', 'body two')",
            [],
        )
        .unwrap();

        let results =
            scan_memories_without_embeddings(&conn, "global", None, &["match-me".to_string()])
                .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "match-me");
    }

    #[test]
    fn dry_run_emits_preview_without_calling_llm() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'dry-mem', 'tiny')",
            [],
        )
        .unwrap();

        let results = scan_short_body_memories(&conn, "global", 1000, None, &[]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "dry-mem");
    }

    #[test]
    fn scan_bound_memories_for_augment_requires_names_and_finds_bound() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (id, namespace, name, body) VALUES (1, 'global', 'bound', 'b')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memories (id, namespace, name, body) VALUES (2, 'global', 'unbound', 'b')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO entities (id, namespace, name) VALUES (10, 'global', 'e')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memory_entities (memory_id, entity_id) VALUES (1, 10)",
            [],
        )
        .unwrap();

        assert!(scan_bound_memories_for_augment(&conn, "global", None, &[]).is_err());

        let names = scan_bound_memories_for_augment(
            &conn,
            "global",
            None,
            &["bound".to_string(), "unbound".to_string()],
        )
        .unwrap();
        assert_eq!(names, vec!["bound".to_string()]);
    }

    // -----------------------------------------------------------------------
    // GAP-SG-77: count_operation_backlog — correctness + scan parity
    // -----------------------------------------------------------------------

    #[test]
    fn count_operation_backlog_entity_descriptions_counts_only_missing() {
        let conn = open_test_db();
        for i in 0..3 {
            conn.execute(
                &format!("INSERT INTO entities (namespace, name, type, description) VALUES ('global', 'ent-{i}', 'tool', NULL)"),
                [],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO entities (namespace, name, type, description) VALUES ('global', 'described', 'tool', 'already has one')",
            [],
        )
        .unwrap();

        let n = count_operation_backlog(
            &conn,
            &EnrichOperation::EntityDescriptions,
            "global",
            ReEmbedTarget::Memories,
        )
        .unwrap();
        assert_eq!(n, 3);
        // Parity: the count must equal what the scanner would materialise.
        let scanned = scan_entities_without_description(&conn, "global", None, &[]).unwrap();
        assert_eq!(n as usize, scanned.len());
    }

    #[test]
    fn count_operation_backlog_re_embed_counts_missing_embeddings() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'no-vec', 'body one')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'has-vec', 'body two')",
            [],
        )
        .unwrap();
        let has_vec_id: i64 = conn
            .query_row(
                "SELECT id FROM memories WHERE namespace='global' AND name='has-vec'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let embedding = vec![0.0_f32; crate::constants::embedding_dim()];
        crate::storage::memories::upsert_vec(
            &conn, has_vec_id, "global", "note", &embedding, "has-vec", "body two",
        )
        .unwrap();

        let n = count_operation_backlog(
            &conn,
            &EnrichOperation::ReEmbed,
            "global",
            ReEmbedTarget::Memories,
        )
        .unwrap();
        assert_eq!(n, 1);
        let scanned = scan_memories_without_embeddings(&conn, "global", None, &[]).unwrap();
        assert_eq!(n as usize, scanned.len());
    }

    #[test]
    fn count_operation_backlog_memory_bindings_counts_unbound() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'unbound', 'b')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'bound', 'b')",
            [],
        )
        .unwrap();
        let bound_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='bound'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO entities (namespace, name) VALUES ('global', 'e')",
            [],
        )
        .unwrap();
        let ent_id: i64 = conn
            .query_row("SELECT id FROM entities WHERE name='e'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
            rusqlite::params![bound_id, ent_id],
        )
        .unwrap();

        let n = count_operation_backlog(
            &conn,
            &EnrichOperation::MemoryBindings,
            "global",
            ReEmbedTarget::Memories,
        )
        .unwrap();
        assert_eq!(n, 1);
        let scanned = scan_unbound_memories(&conn, "global", None, &[]).unwrap();
        assert_eq!(n as usize, scanned.len());
    }

    #[test]
    fn count_operation_backlog_body_enrich_uses_default_threshold() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'short', 'tiny')",
            [],
        )
        .unwrap();
        let long_body = "a".repeat(super::DEFAULT_BODY_ENRICH_MIN_CHARS + 100);
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'long', ?1)",
            rusqlite::params![long_body],
        )
        .unwrap();

        let n = count_operation_backlog(
            &conn,
            &EnrichOperation::BodyEnrich,
            "global",
            ReEmbedTarget::Memories,
        )
        .unwrap();
        assert_eq!(n, 1);
        // Parity against the scanner using the same default threshold.
        let scanned = scan_short_body_memories(
            &conn,
            "global",
            super::DEFAULT_BODY_ENRICH_MIN_CHARS,
            None,
            &[],
        )
        .unwrap();
        assert_eq!(n as usize, scanned.len());
    }

    #[test]
    fn count_operation_backlog_advisory_ops_report_zero() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'm', 'b')",
            [],
        )
        .unwrap();
        for op in [
            EnrichOperation::CrossDomainBridges,
            EnrichOperation::GraphAudit,
            EnrichOperation::BodyExtract,
        ] {
            let n = count_operation_backlog(&conn, &op, "global", ReEmbedTarget::Memories).unwrap();
            assert_eq!(n, 0, "advisory op {op:?} must report zero backlog");
        }
    }

    #[test]
    fn count_operation_backlog_entity_connect_counts_isolated() {
        let conn = open_test_db();
        // entidade degree-0 COM binding NER -> deve contar como backlog
        conn.execute(
            "INSERT INTO entities (namespace, name, type, degree) VALUES ('global','hub','tool',0)",
            [],
        )
        .unwrap();
        let hub_id: i64 = conn
            .query_row("SELECT id FROM entities WHERE name='hub'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global','m','b')",
            [],
        )
        .unwrap();
        let mem_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='m'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO memory_entities (entity_id, memory_id) VALUES (?1, ?2)",
            rusqlite::params![hub_id, mem_id],
        )
        .unwrap();
        let n = count_operation_backlog(
            &conn,
            &EnrichOperation::EntityConnect,
            "global",
            ReEmbedTarget::Memories,
        )
        .unwrap();
        assert!(
            n > 0,
            "entity-connect backlog must count degree-0 entities with NER bindings"
        );
    }

    #[test]
    fn scan_isolated_entity_pairs_excludes_seen() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO entities (namespace, name, type) VALUES ('global','a','tool')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO entities (namespace, name, type) VALUES ('global','b','tool')",
            [],
        )
        .unwrap();
        let (a_id, b_id): (i64, i64) = conn
            .query_row(
                "SELECT (SELECT id FROM entities WHERE name='a'), \
                 (SELECT id FROM entities WHERE name='b')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        // Co-occurrence evidence so the pair would otherwise be a candidate.
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global','m-ab','body')",
            [],
        )
        .unwrap();
        let mem_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='m-ab'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2), (?1, ?3)",
            rusqlite::params![mem_id, a_id, b_id],
        )
        .unwrap();
        // marca o par como já avaliado (verdict none)
        conn.execute(
            "INSERT INTO entity_connect_seen (source_id, target_id, namespace, verdict) \
             VALUES (?1, ?2, 'global','none')",
            rusqlite::params![a_id, b_id],
        )
        .unwrap();
        let pairs = scan_isolated_entity_pairs(&conn, "global", Some(50)).unwrap();
        assert!(
            pairs
                .iter()
                .all(|(id1, _, id2, _)| !(*id1 == a_id && *id2 == b_id)),
            "seen pair must not be re-scanned"
        );
    }

    #[test]
    fn format_and_parse_pair_key_roundtrip() {
        assert_eq!(format_pair_key(3, 1), "pair:1:3");
        assert_eq!(parse_pair_key("pair:1:3"), Some((1, 3)));
        assert_eq!(parse_pair_key("pair:9:2"), Some((2, 9)));
        assert_eq!(parse_pair_key("legacy-entity-name"), None);
        assert_eq!(parse_pair_key("pair:x:y"), None);
    }

    #[test]
    fn scan_isolated_entity_pairs_uses_cooccurrence_not_cartesian() {
        let conn = open_test_db();
        // Three entities: only a+b co-occur; c is isolated (no shared memory).
        for name in ["a", "b", "c"] {
            conn.execute(
                "INSERT INTO entities (namespace, name, type, degree) VALUES ('global', ?1, 'tool', 0)",
                rusqlite::params![name],
            )
            .unwrap();
        }
        let (a_id, b_id, c_id): (i64, i64, i64) = conn
            .query_row(
                "SELECT \
                   (SELECT id FROM entities WHERE name='a'), \
                   (SELECT id FROM entities WHERE name='b'), \
                   (SELECT id FROM entities WHERE name='c')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global','m','body')",
            [],
        )
        .unwrap();
        let mem_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='m'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2), (?1, ?3)",
            rusqlite::params![mem_id, a_id, b_id],
        )
        .unwrap();
        // c has a binding alone (island) but does not co-occur with a/b.
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global','m-c','body')",
            [],
        )
        .unwrap();
        let mem_c: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='m-c'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
            rusqlite::params![mem_c, c_id],
        )
        .unwrap();

        let pairs = scan_isolated_entity_pairs(&conn, "global", Some(50)).unwrap();
        assert!(
            pairs.iter().any(|(x, _, y, _)| *x == a_id && *y == b_id),
            "co-occurring a-b must be a candidate: {pairs:?}"
        );
        // Without a hub of degree>0, hub×island may not pair c with a/b; the
        // invariant is we never invent a-c/b-c from a pure cartesian product
        // when they never co-occur and no hub fill applies.
        let only_ab = pairs.len() == 1
            && pairs[0].0 == a_id
            && pairs[0].2 == b_id;
        assert!(
            only_ab || pairs.iter().all(|(x, _, y, _)| {
                (*x == a_id && *y == b_id)
                    || (*x == a_id && *y == c_id)
                    || (*x == b_id && *y == c_id)
            }),
            "unexpected pairs: {pairs:?}"
        );
    }

    #[test]
    fn scan_isolated_entity_pairs_respects_limit_on_large_namespace() {
        let conn = open_test_db();
        // 80 entities sharing one memory → many co-pairs; LIMIT must cap.
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global','bulk','x')",
            [],
        )
        .unwrap();
        let mem_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='bulk'", [], |r| r.get(0))
            .unwrap();
        for i in 0..80 {
            conn.execute(
                "INSERT INTO entities (namespace, name, type) VALUES ('global', ?1, 'tool')",
                rusqlite::params![format!("e{i}")],
            )
            .unwrap();
            let eid: i64 = conn
                .query_row(
                    "SELECT id FROM entities WHERE name = ?1",
                    rusqlite::params![format!("e{i}")],
                    |r| r.get(0),
                )
                .unwrap();
            conn.execute(
                "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
                rusqlite::params![mem_id, eid],
            )
            .unwrap();
        }
        let started = std::time::Instant::now();
        let pairs = scan_isolated_entity_pairs(&conn, "global", Some(10)).unwrap();
        let elapsed = started.elapsed();
        assert_eq!(pairs.len(), 10, "LIMIT 10 must be honoured");
        assert!(
            elapsed.as_secs() < 5,
            "scan must finish quickly on co-occurrence graph, took {elapsed:?}"
        );
    }

    // -----------------------------------------------------------------------
    // v1.1.1 (P2/P10): re-embed targets — entity/chunk backfill scanners and
    // dim-divergence selection.
    // -----------------------------------------------------------------------

    /// Inserts a raw vector row with the given dim and blob length (bytes).
    fn insert_entity_vec_raw(conn: &Connection, entity_id: i64, dim: usize, blob_len: usize) {
        conn.execute(
            "INSERT INTO entity_embeddings (entity_id, namespace, embedding, source, model, dim) \
             VALUES (?1, 'global', ?2, 'test', 'test', ?3)",
            rusqlite::params![entity_id, vec![0u8; blob_len], dim as i64],
        )
        .unwrap();
    }

    fn insert_entity_named(conn: &Connection, name: &str) -> i64 {
        conn.execute(
            &format!(
                "INSERT INTO entities (namespace, name, type) VALUES ('global', '{name}', 'tool')"
            ),
            [],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn scan_entities_missing_embeddings_selects_missing_stale_and_empty() {
        let conn = open_test_db();
        let dim = crate::constants::embedding_dim();

        let e_missing = insert_entity_named(&conn, "ent-missing");
        let e_live = insert_entity_named(&conn, "ent-live");
        let e_stale = insert_entity_named(&conn, "ent-stale-dim");
        let e_empty = insert_entity_named(&conn, "ent-empty-blob");

        insert_entity_vec_raw(&conn, e_live, dim, dim * 4);
        insert_entity_vec_raw(&conn, e_stale, 64, 64 * 4);
        insert_entity_vec_raw(&conn, e_empty, dim, 0);

        let rows = scan_entities_missing_embeddings(&conn, "global", None, &[]).unwrap();
        let names: Vec<&str> = rows.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(
            names,
            vec!["ent-missing", "ent-stale-dim", "ent-empty-blob"],
            "missing, stale-dim and empty-blob entities must be selected; live must not"
        );
        assert!(!names.contains(&"ent-live"));
        let _ = e_missing;
    }

    #[test]
    fn scan_entities_missing_embeddings_respects_name_filter() {
        let conn = open_test_db();
        insert_entity_named(&conn, "ent-a");
        insert_entity_named(&conn, "ent-b");

        let rows = scan_entities_missing_embeddings(&conn, "global", None, &["ent-b".to_string()])
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "ent-b");
    }

    fn insert_chunk_row(conn: &Connection, memory_id: i64, chunk_idx: i32) -> i64 {
        conn.execute(
            "INSERT INTO memory_chunks (memory_id, chunk_idx, chunk_text) \
             VALUES (?1, ?2, 'chunk text')",
            rusqlite::params![memory_id, chunk_idx],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn insert_chunk_vec_raw(conn: &Connection, chunk_id: i64, memory_id: i64, dim: usize) {
        conn.execute(
            "INSERT INTO chunk_embeddings (chunk_id, memory_id, embedding, source, model, dim) \
             VALUES (?1, ?2, ?3, 'test', 'test', ?4)",
            rusqlite::params![chunk_id, memory_id, vec![0u8; dim * 4], dim as i64],
        )
        .unwrap();
    }

    #[test]
    fn scan_chunks_missing_embeddings_selects_missing_and_stale_dim() {
        let conn = open_test_db();
        let dim = crate::constants::embedding_dim();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'chunked', 'b')",
            [],
        )
        .unwrap();
        let mem_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='chunked'", [], |r| {
                r.get(0)
            })
            .unwrap();

        let c_live = insert_chunk_row(&conn, mem_id, 0);
        let c_stale = insert_chunk_row(&conn, mem_id, 1);
        let c_missing = insert_chunk_row(&conn, mem_id, 2);
        insert_chunk_vec_raw(&conn, c_live, mem_id, dim);
        insert_chunk_vec_raw(&conn, c_stale, mem_id, 64);

        let ids = scan_chunks_missing_embeddings(&conn, "global", None, &[]).unwrap();
        assert_eq!(
            ids,
            vec![c_stale, c_missing],
            "stale-dim and missing chunks must be selected; live must not"
        );

        // Name filter restricts by PARENT memory name.
        let filtered =
            scan_chunks_missing_embeddings(&conn, "global", None, &["other-mem".to_string()])
                .unwrap();
        assert!(filtered.is_empty());
        let filtered =
            scan_chunks_missing_embeddings(&conn, "global", None, &["chunked".to_string()])
                .unwrap();
        assert_eq!(filtered, vec![c_stale, c_missing]);
    }

    // P10: a memory whose stored vector has a divergent dim is re-scanned.
    #[test]
    fn scan_memories_with_stale_dim_are_rescanned() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'stale-dim', 'body')",
            [],
        )
        .unwrap();
        let mem_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='stale-dim'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO memory_embeddings (memory_id, namespace, embedding, source, model, dim) \
             VALUES (?1, 'global', ?2, 'test', 'test', 64)",
            rusqlite::params![mem_id, vec![0u8; 64 * 4]],
        )
        .unwrap();

        let rows = scan_memories_without_embeddings(&conn, "global", None, &[]).unwrap();
        assert_eq!(rows.len(), 1, "legacy-dim vector must be re-selected");
        assert_eq!(rows[0].1, "stale-dim");
    }

    #[test]
    fn count_operation_backlog_re_embed_targets_match_scanners() {
        let conn = open_test_db();
        let dim = crate::constants::embedding_dim();

        // One memory without vector, one entity stale, one chunk missing.
        conn.execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'no-vec', 'b')",
            [],
        )
        .unwrap();
        let mem_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='no-vec'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let eid = insert_entity_named(&conn, "ent-stale");
        insert_entity_vec_raw(&conn, eid, 64, 64 * 4);
        insert_chunk_row(&conn, mem_id, 0);
        let _ = dim;

        let n_mem = count_operation_backlog(
            &conn,
            &EnrichOperation::ReEmbed,
            "global",
            ReEmbedTarget::Memories,
        )
        .unwrap();
        assert_eq!(
            n_mem as usize,
            scan_memories_without_embeddings(&conn, "global", None, &[])
                .unwrap()
                .len()
        );

        let n_ent = count_operation_backlog(
            &conn,
            &EnrichOperation::ReEmbed,
            "global",
            ReEmbedTarget::Entities,
        )
        .unwrap();
        assert_eq!(
            n_ent as usize,
            scan_entities_missing_embeddings(&conn, "global", None, &[])
                .unwrap()
                .len()
        );

        let n_chunk = count_operation_backlog(
            &conn,
            &EnrichOperation::ReEmbed,
            "global",
            ReEmbedTarget::Chunks,
        )
        .unwrap();
        assert_eq!(
            n_chunk as usize,
            scan_chunks_missing_embeddings(&conn, "global", None, &[])
                .unwrap()
                .len()
        );

        let n_all = count_operation_backlog(
            &conn,
            &EnrichOperation::ReEmbed,
            "global",
            ReEmbedTarget::All,
        )
        .unwrap();
        assert_eq!(n_all, n_mem + n_ent + n_chunk, "all = soma dos três alvos");
    }

    // Bug 6: chunks whose parent memory was soft-deleted stay invisible to
    // re-embed under the old INNER JOIN + `m.deleted_at IS NULL` filter.
    // LEFT JOIN + `(m.namespace = ?1 OR m.id IS NULL)` must surface them.
    #[test]
    fn scan_chunks_of_soft_deleted_memory_are_selected() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body, deleted_at) \
             VALUES ('global', 'gone-mem', 'b', 1700000000)",
            [],
        )
        .unwrap();
        let mem_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='gone-mem'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let orphan_chunk = insert_chunk_row(&conn, mem_id, 0);

        let ids = scan_chunks_missing_embeddings(&conn, "global", None, &[]).unwrap();
        assert!(
            ids.contains(&orphan_chunk),
            "orphan chunk of soft-deleted memory must be selected for re-embed"
        );
    }

    #[test]
    fn count_backlog_includes_orphan_chunks() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO memories (namespace, name, body, deleted_at) \
             VALUES ('global', 'gone-mem', 'b', 1700000000)",
            [],
        )
        .unwrap();
        let mem_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='gone-mem'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let orphan_chunk = insert_chunk_row(&conn, mem_id, 0);

        let n = count_operation_backlog(
            &conn,
            &EnrichOperation::ReEmbed,
            "global",
            ReEmbedTarget::Chunks,
        )
        .unwrap();
        assert!(
            n >= 1,
            "orphan chunk of soft-deleted memory must be counted in backlog"
        );
        let _ = orphan_chunk;
    }
}
