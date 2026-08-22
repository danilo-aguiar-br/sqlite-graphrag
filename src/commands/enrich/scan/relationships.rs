//! Scanners whose target is the `relationships` table.
//!
//! Two eligibility questions: which edges are strong enough to warrant weight
//! recalibration, and which still carry the generic `applies-to` relation.
//! Predicates come from [`super::super::predicates`] (GAP-SG-77).

use super::super::predicates::{generic_relation_predicate, high_weight_predicate};
use super::sql::{limit_clause, limit_param};
use crate::errors::AppError;
use rusqlite::Connection;

/// G27: Returns relationships strong enough to warrant recalibration.
///
/// The threshold is [`crate::constants::ENRICH_HIGH_WEIGHT_THRESHOLD`], never a
/// number spelled here: a doc-comment that names a value it does not read is one
/// edit away from describing the opposite of what the query does.
#[allow(clippy::type_complexity)]
pub(in crate::commands::enrich) fn scan_weight_candidates(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
) -> Result<Vec<(i64, String, String, String, f64)>, AppError> {
    let limit_sql = limit_clause(2);
    let limit_v = limit_param(limit);
    let weight_gate = high_weight_predicate();
    let sql = format!(
        "SELECT r.id, e1.name, e2.name, r.relation, r.weight \
         FROM relationships r \
         JOIN entities e1 ON e1.id = r.source_id \
         JOIN entities e2 ON e2.id = r.target_id \
         WHERE {weight_gate} AND e1.namespace = ?1 \
         ORDER BY r.weight DESC {limit_sql}"
    );
    let mut stmt = conn.prepare(&sql)?;
    // The rows must be collected: `query_map` borrows `stmt`, which dies with
    // this scope, so the iterator cannot escape to the caller.
    let rows = stmt
        .query_map(rusqlite::params![namespace, limit_v], |r| {
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

/// G27: Returns relationships with generic relation types (applies-to).
pub(in crate::commands::enrich) fn scan_generic_relations(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
) -> Result<Vec<(i64, String, String, String)>, AppError> {
    let limit_sql = limit_clause(2);
    let limit_v = limit_param(limit);
    let generic_pred = generic_relation_predicate();
    let sql = format!(
        "SELECT r.id, e1.name, e2.name, r.relation \
         FROM relationships r \
         JOIN entities e1 ON e1.id = r.source_id \
         JOIN entities e2 ON e2.id = r.target_id \
         WHERE {generic_pred} AND e1.namespace = ?1 \
         ORDER BY r.id {limit_sql}"
    );
    let mut stmt = conn.prepare(&sql)?;
    // Collected for the same reason as `scan_weight_candidates`.
    let rows = stmt
        .query_map(rusqlite::params![namespace, limit_v], |r| {
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
