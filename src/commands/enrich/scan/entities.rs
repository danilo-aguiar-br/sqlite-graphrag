//! Scanners whose target is the `entities` table.
//!
//! Two eligibility questions: which entities carry no usable description, and
//! which lack a live vector at the configured dimensionality. Predicates come
//! from [`super::super::predicates`] so the scan and `count_operation_backlog`
//! cannot drift (GAP-SG-77).
//!
//! GAP-SG-185: missing-embedding scans use keyset pages.

use super::super::predicates::{
    entity_description_scan_predicate, is_low_quality_description, reembed_entity_predicate,
};
use super::sql::{keyset_collect, limit_clause, limit_param, placeholder_list};
use crate::errors::AppError;
use rusqlite::Connection;

/// Applies the force-redescribe post-filter while STREAMING the SQL rows.
/// `named` is true when the operator passed an explicit name filter.
///
/// G-PR-7: without it this post-filter SILENTLY UNDID the SQL predicate. The
/// query already resolves to `1=1` under `--force-redescribe` plus a name
/// filter, and then this loop re-applied `is_low_quality_description` to every
/// row it returned — two gates in series with only the first one opened.
///
/// Measured: `--force-redescribe --entity-names a-carney,adriana-bruno` scanned
/// `items_total: 0` while a third entity in the SAME invocation processed fine,
/// because that one happened to carry a low-quality marker. A fluent,
/// confident, WRONG description — the exact class targeted repair exists for —
/// matched no marker and was unreachable by any phrasing of the command.
fn filter_description_candidates<I>(
    rows: I,
    limit: Option<usize>,
    force_redescribe: bool,
    named: bool,
) -> Result<Vec<(i64, String, String)>, AppError>
where
    I: Iterator<Item = rusqlite::Result<(i64, String, String, String)>>,
{
    let mut out: Vec<(i64, String, String)> = Vec::new();
    for row in rows {
        let (id, name, ty, desc) = row?;
        if force_redescribe && !named {
            let empty = desc.trim().is_empty();
            if !empty && !is_low_quality_description(&desc) {
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

/// Returns entities with NULL or empty description.
pub(in crate::commands::enrich) fn scan_entities_without_description(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
    force_redescribe: bool,
) -> Result<Vec<(i64, String, String)>, AppError> {
    let sql_limit = limit.map(|n| {
        if force_redescribe {
            n.saturating_mul(2).max(n.saturating_add(32))
        } else {
            n
        }
    });
    let limit_v = limit_param(sql_limit);
    // G-PR-7: with an explicit name filter the operator's choice IS the
    // eligibility rule; the quality heuristic only decides who to visit when
    // nobody was named.
    let desc_pred = entity_description_scan_predicate(force_redescribe, !name_filter.is_empty());

    if name_filter.is_empty() {
        let limit_sql = limit_clause(2);
        let sql = format!(
            "SELECT id, name, type, COALESCE(description, '')
             FROM entities
             WHERE namespace = ?1
               AND {desc_pred}
             ORDER BY id
             {limit_sql}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let mapped = stmt.query_map(rusqlite::params![namespace, limit_v], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        filter_description_candidates(mapped, limit, force_redescribe, false)
    } else {
        // v1.2.8: match the raw name AND its kebab-ASCII form. Entities are
        // stored normalised, so `--entity-names "Relatório Anual"` matched zero
        // rows against `relatorio-anual` and reported `matched: 0` — the same
        // one-sided normalisation that made relation filters blind.
        let name_filter = &super::name_filter::widen_name_filter(name_filter);
        let in_clause = placeholder_list(2, name_filter.len());
        let limit_sql = limit_clause(name_filter.len() + 2);
        let sql = format!(
            "SELECT id, name, type, COALESCE(description, '')
             FROM entities
             WHERE namespace = ?1
               AND name IN ({in_clause})
               AND {desc_pred}
             ORDER BY id
             {limit_sql}"
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(2 + name_filter.len());
        params_vec.push(&namespace);
        for n in name_filter {
            params_vec.push(n);
        }
        params_vec.push(&limit_v);
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
        filter_description_candidates(mapped, limit, force_redescribe, true)
    }
}

/// Entities without a live vector — keyset paged (GAP-SG-185).
pub(in crate::commands::enrich) fn scan_entities_missing_embeddings(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
    page_size: usize,
) -> Result<Vec<(i64, String)>, AppError> {
    let predicate = reembed_entity_predicate(crate::constants::embedding_dim());
    keyset_collect(limit, page_size, |after, want| {
        let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
        if name_filter.is_empty() {
            let sql = format!(
                "SELECT e.id, e.name
                 FROM entities e
                 WHERE e.namespace = ?1
                   AND e.id > ?2
                   AND {predicate}
                 ORDER BY e.id
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
            // Same widening as `scan_entity_description_candidates`.
            let name_filter = &super::name_filter::widen_name_filter(name_filter);
            let in_clause = placeholder_list(3, name_filter.len());
            let lim_idx = 3 + name_filter.len();
            let sql = format!(
                "SELECT e.id, e.name
                 FROM entities e
                 WHERE e.namespace = ?1
                   AND e.id > ?2
                   AND e.name IN ({in_clause})
                   AND {predicate}
                 ORDER BY e.id
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
