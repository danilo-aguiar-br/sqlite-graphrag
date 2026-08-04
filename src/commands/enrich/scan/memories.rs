//! Scanners whose target is the `memories` table.
//!
//! Covers the four memory-shaped eligibility questions: which memories have no
//! entity binding, which already have one and can be augmented, which have a
//! body below the enrichment threshold, and which lack a live vector.
//! Every predicate comes from [`super::super::predicates`], the single source
//! of truth shared with `count_operation_backlog` (GAP-SG-77).
//!
//! GAP-SG-185: unbounded scans walk keyset pages (`id > last`) so each
//! `query_map` collect is O(page_size).

use super::super::predicates::{reembed_memory_predicate, UNBOUND_MEMORY_PREDICATE};
use super::sql::{keyset_collect, limit_clause, limit_param, placeholder_list};
use crate::errors::AppError;
use rusqlite::Connection;

/// Returns memories without any `memory_entities` binding.
///
/// Yields `(id, name)`: the body is filtered on in SQL but never selected.
/// `page_size` is the GAP-SG-185 keyset page width (flag / XDG / default).
pub(in crate::commands::enrich) fn scan_unbound_memories(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
    page_size: usize,
) -> Result<Vec<(i64, String)>, AppError> {
    keyset_collect(limit, page_size, |after, want| {
        let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
        if name_filter.is_empty() {
            let sql = format!(
                "SELECT m.id, m.name
                 FROM memories m
                 WHERE m.namespace = ?1
                   AND m.deleted_at IS NULL
                   AND m.id > ?2
                   AND {UNBOUND_MEMORY_PREDICATE}
                 ORDER BY m.id
                 LIMIT ?3"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params![namespace, after, limit_v], |r| {
                    let id = r.get::<_, i64>(0)?;
                    Ok((id, (id, r.get::<_, String>(1)?)))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        } else {
            let in_clause = placeholder_list(3, name_filter.len());
            let lim_idx = 3 + name_filter.len();
            let sql = format!(
                "SELECT m.id, m.name
                 FROM memories m
                 WHERE m.namespace = ?1
                   AND m.deleted_at IS NULL
                   AND m.id > ?2
                   AND m.name IN ({in_clause})
                   AND {UNBOUND_MEMORY_PREDICATE}
                 ORDER BY m.id
                 LIMIT ?{lim_idx}"
            );
            let mut params_vec: Vec<&dyn rusqlite::ToSql> =
                Vec::with_capacity(3 + name_filter.len());
            params_vec.push(&namespace);
            params_vec.push(&after);
            for n in name_filter {
                params_vec.push(n);
            }
            params_vec.push(&limit_v);
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(
                    rusqlite::params_from_iter(params_vec.iter().copied()),
                    |r| {
                        let id = r.get::<_, i64>(0)?;
                        Ok((id, (id, r.get::<_, String>(1)?)))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    })
}

/// GAP-SG-24/26: already-bound memory names for additive augmentation.
pub(in crate::commands::enrich) fn scan_bound_memories_for_augment(
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
    let limit_v = limit_param(limit);
    let in_clause = placeholder_list(2, name_filter.len());
    let limit_sql = limit_clause(name_filter.len() + 2);
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
         {limit_sql}"
    );
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(2 + name_filter.len());
    params_vec.push(&namespace);
    for n in name_filter {
        params_vec.push(n);
    }
    params_vec.push(&limit_v);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            rusqlite::params_from_iter(params_vec.iter().copied()),
            |r| r.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Memories whose body length is below the configured minimum (body-enrich).
pub(in crate::commands::enrich) fn scan_short_body_memories(
    conn: &Connection,
    namespace: &str,
    min_chars: usize,
    limit: Option<usize>,
    name_filter: &[String],
    page_size: usize,
) -> Result<Vec<(i64, String)>, AppError> {
    let min_chars_i64 = min_chars as i64;
    // Inline the length check so keyset `id > ?2` does not collide with the
    // hard-coded `?2` inside SHORT_BODY_PREDICATE.
    keyset_collect(limit, page_size, |after, want| {
        let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
        if name_filter.is_empty() {
            let sql = "SELECT m.id, m.name
                 FROM memories m
                 WHERE m.namespace = ?1
                   AND m.deleted_at IS NULL
                   AND m.id > ?2
                   AND LENGTH(COALESCE(m.body,'')) < ?3
                 ORDER BY m.id
                 LIMIT ?4";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt
                .query_map(
                    rusqlite::params![namespace, after, min_chars_i64, limit_v],
                    |r| {
                        let id = r.get::<_, i64>(0)?;
                        Ok((id, (id, r.get::<_, String>(1)?)))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        } else {
            let in_clause = placeholder_list(3, name_filter.len());
            let min_idx = 3 + name_filter.len();
            let lim_idx = min_idx + 1;
            let sql = format!(
                "SELECT m.id, m.name
                 FROM memories m
                 WHERE m.namespace = ?1
                   AND m.deleted_at IS NULL
                   AND m.id > ?2
                   AND m.name IN ({in_clause})
                   AND LENGTH(COALESCE(m.body,'')) < ?{min_idx}
                 ORDER BY m.id
                 LIMIT ?{lim_idx}"
            );
            let mut params_vec: Vec<&dyn rusqlite::ToSql> =
                Vec::with_capacity(4 + name_filter.len());
            params_vec.push(&namespace);
            params_vec.push(&after);
            for n in name_filter {
                params_vec.push(n);
            }
            params_vec.push(&min_chars_i64);
            params_vec.push(&limit_v);
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(
                    rusqlite::params_from_iter(params_vec.iter().copied()),
                    |r| {
                        let id = r.get::<_, i64>(0)?;
                        Ok((id, (id, r.get::<_, String>(1)?)))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    })
}

/// Live memories without a live vector in `memory_embeddings` (re-embed).
pub(in crate::commands::enrich) fn scan_memories_without_embeddings(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
    page_size: usize,
) -> Result<Vec<(i64, String)>, AppError> {
    let predicate = reembed_memory_predicate(crate::constants::embedding_dim());
    keyset_collect(limit, page_size, |after, want| {
        let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
        if name_filter.is_empty() {
            let sql = format!(
                "SELECT m.id, m.name
                 FROM memories m
                 WHERE m.namespace = ?1
                   AND m.deleted_at IS NULL
                   AND m.id > ?2
                   AND {predicate}
                 ORDER BY m.id
                 LIMIT ?3"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params![namespace, after, limit_v], |r| {
                    let id = r.get::<_, i64>(0)?;
                    Ok((id, (id, r.get::<_, String>(1)?)))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        } else {
            let in_clause = placeholder_list(3, name_filter.len());
            let lim_idx = 3 + name_filter.len();
            let sql = format!(
                "SELECT m.id, m.name
                 FROM memories m
                 WHERE m.namespace = ?1
                   AND m.deleted_at IS NULL
                   AND m.id > ?2
                   AND m.name IN ({in_clause})
                   AND {predicate}
                 ORDER BY m.id
                 LIMIT ?{lim_idx}"
            );
            let mut params_vec: Vec<&dyn rusqlite::ToSql> =
                Vec::with_capacity(3 + name_filter.len());
            params_vec.push(&namespace);
            params_vec.push(&after);
            for n in name_filter {
                params_vec.push(n);
            }
            params_vec.push(&limit_v);
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(
                    rusqlite::params_from_iter(params_vec.iter().copied()),
                    |r| {
                        let id = r.get::<_, i64>(0)?;
                        Ok((id, (id, r.get::<_, String>(1)?)))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    })
}

/// Whole-namespace memory names (GAP-SG-27).
pub(in crate::commands::enrich) fn scan_all_memory_names(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
    page_size: usize,
) -> Result<Vec<String>, AppError> {
    keyset_collect(limit, page_size, |after, want| {
        let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
        let sql = "SELECT id, name FROM memories \
                   WHERE namespace=?1 AND deleted_at IS NULL AND id > ?2 \
                   ORDER BY id LIMIT ?3";
        let mut stmt = conn.prepare(sql)?;
        let names_wanted: Option<std::collections::HashSet<&str>> = if name_filter.is_empty() {
            None
        } else {
            Some(name_filter.iter().map(String::as_str).collect())
        };
        let mut page: Vec<(i64, String)> = Vec::new();
        let rows = stmt.query_map(rusqlite::params![namespace, after, limit_v], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, name) = row?;
            match &names_wanted {
                Some(wanted) if !wanted.contains(name.as_str()) => continue,
                _ => page.push((id, name)),
            }
        }
        Ok(page)
    })
}
