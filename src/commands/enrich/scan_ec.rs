//! Entity-connect pair scan (Wave C1).

use crate::errors::AppError;
use rusqlite::Connection;

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
