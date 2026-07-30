//! Scan functions — select candidates for each enrichment operation.

use super::args::{EnrichArgs, EnrichOperation, ReEmbedTarget};
use crate::errors::AppError;
use rusqlite::Connection;
use std::path::Path;

pub(super) use super::quality_sample::*;
pub(super) use super::scan_ec::*;

#[cfg(test)]
#[path = "scan_tests_a.rs"]
mod tests_a;
#[cfg(test)]
#[path = "scan_tests_b.rs"]
mod tests_b;

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

/// `entity-descriptions`: empty-only predicate (kept for local docs; scan uses predicates::entity_description_scan_predicate).
#[allow(dead_code)]
const NULL_DESCRIPTION_PREDICATE: &str = "(description IS NULL OR description = '')";

/// `body-enrich`: memory body shorter than the `?2` character threshold.
const SHORT_BODY_PREDICATE: &str = "LENGTH(COALESCE(m.body,'')) < ?2";

// GAP-SG-109: do not redeclare GENERIC_DESCRIPTION_PREDICATE here — the
// single source of truth is `predicates::GENERIC_DESCRIPTION_PREDICATE`.

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
        AppError::Validation(crate::i18n::validation::failed_to_read_names_file(
            &path.display().to_string(),
            &e,
        ))
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
///
/// With `--force-redescribe`, also selects high-precision SQL low-quality
/// candidates, then applies [`super::predicates::is_low_quality_description`]
/// so legitimate prose that still hits a LIKE prefilter is dropped (G-PR-2).
pub(super) fn scan_entities_without_description(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
    force_redescribe: bool,
) -> Result<Vec<(i64, String, String)>, AppError> {
    // Over-fetch slightly when force-redescribe so the Rust post-filter can
    // drop false positives without under-filling an explicit LIMIT.
    let sql_limit = limit.map(|n| {
        if force_redescribe {
            n.saturating_mul(2).max(n.saturating_add(32))
        } else {
            n
        }
    });
    let limit_clause = sql_limit
        .map(|n| format!("LIMIT {n}"))
        .unwrap_or_default();
    // GAP-SG-77 / GAP-CLI-ED-06: status COUNT and scan MUST share one predicate.
    // Without force: NULL/empty only. With --force-redescribe: empty OR low-quality LIKEs.
    let desc_pred = super::predicates::entity_description_scan_predicate(force_redescribe);

    // Always fetch description so force-redescribe can post-filter; empty path
    // ignores it.
    let rows: Vec<(i64, String, String, String)> = if name_filter.is_empty() {
        let sql = format!(
            "SELECT id, name, type, COALESCE(description, '')
             FROM entities
             WHERE namespace = ?1
               AND {desc_pred}
             ORDER BY id
             {limit_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let mapped = stmt.query_map(rusqlite::params![namespace], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    } else {
        let placeholders: Vec<String> = (2..=name_filter.len() + 1)
            .map(|i| format!("?{i}"))
            .collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT id, name, type, COALESCE(description, '')
             FROM entities
             WHERE namespace = ?1
               AND name IN ({in_clause})
               AND {desc_pred}
             ORDER BY id
             {limit_clause}"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + name_filter.len());
        params_vec.push(&namespace);
        for n in name_filter {
            params_vec.push(n);
        }
        let mut stmt = conn.prepare(&sql)?;
        let mapped = stmt.query_map(
            rusqlite::params_from_iter(params_vec.iter().copied()),
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };

    let mut out: Vec<(i64, String, String)> = Vec::with_capacity(rows.len());
    for (id, name, ty, desc) in rows {
        if force_redescribe {
            // Keep empty/NULL (already selected) and true low-quality only.
            let empty = desc.trim().is_empty();
            if !empty && !super::predicates::is_low_quality_description(&desc) {
                continue;
            }
        }
        out.push((id, name, ty));
        if let Some(n) = limit {
            if out.len() >= n {
                break;
            }
        }
    }
    Ok(out)
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
            let rows = scan_entities_without_description(
                conn,
                namespace,
                args.limit,
                &name_filter,
                args.force_redescribe,
            )?;
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
