//! Handlers for `graph` subcommands.

use super::args::*;
use super::formats::{EdgeOut, GraphSnapshot, NodeOut, render_dot, render_json, render_mermaid, render_ndjson_streaming};
use crate::cli::GraphExportFormat;
use crate::errors::AppError;
use crate::output;
use crate::paths::AppPaths;
use crate::storage::connection::open_ro;
use crate::storage::entities;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::time::Instant;

/// Dispatch `graph` subcommands (snapshot, traverse, stats, entities, recompute-degree).
pub fn run(args: GraphArgs) -> Result<(), AppError> {
    match args.subcommand {
        None => run_entities_snapshot(
            args.db.as_deref(),
            args.namespace.as_deref(),
            args.format,
            args.json,
            args.output.as_deref(),
        ),
        Some(GraphSubcommand::Traverse(mut a)) => {
            if a.db.is_none() {
                a.db = args.db;
            }
            if a.namespace.is_none() {
                a.namespace = args.namespace;
            }
            run_traverse(a)
        }
        Some(GraphSubcommand::Stats(mut a)) => {
            if a.db.is_none() {
                a.db = args.db;
            }
            if a.namespace.is_none() {
                a.namespace = args.namespace;
            }
            run_stats(a)
        }
        Some(GraphSubcommand::Entities(mut a)) => {
            if a.db.is_none() {
                a.db = args.db;
            }
            if a.namespace.is_none() {
                a.namespace = args.namespace;
            }
            run_entities(a)
        }
        Some(GraphSubcommand::RecomputeDegree(mut a)) => {
            if a.db.is_none() {
                a.db = args.db;
            }
            if a.namespace.is_none() {
                a.namespace = args.namespace;
            }
            run_recompute_degree(a)
        }
    }
}

/// v1.1.1 (P3): summary of one degree-reconciliation pass.
///
/// `total` is every entity scanned; `updated` diverged to a non-zero real
/// degree; `zeroed` diverged to zero (no live edges); `unchanged` already
/// matched. `updated + zeroed + unchanged == total`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct RecomputeDegreeSummary {
    pub(crate) total: i64,
    pub(crate) updated: i64,
    pub(crate) zeroed: i64,
    pub(crate) unchanged: i64,
}

#[derive(Serialize)]
struct RecomputeDegreeResponse {
    namespace: Option<String>,
    dry_run: bool,
    total: i64,
    updated: i64,
    zeroed: i64,
    unchanged: i64,
    elapsed_ms: u64,
}

/// v1.1.1 (P3): recomputes `entities.degree` from the real `relationships`
/// rows inside one IMMEDIATE transaction.
///
/// Uses the SAME per-entity semantics as the canonical
/// [`entities::recalculate_degree`] helper (`COUNT(*) WHERE source_id = id OR
/// target_id = id` — a self-loop counts once), so a reconciled graph is
/// byte-identical to one maintained exclusively through link/merge/delete.
/// With `dry_run` the transaction never writes and is rolled back on drop.
pub(crate) fn recompute_degrees(
    conn: &mut rusqlite::Connection,
    namespace: Option<&str>,
    dry_run: bool,
) -> Result<RecomputeDegreeSummary, AppError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    const SELECT_BASE: &str = "SELECT e.id, e.degree, \
         (SELECT COUNT(*) FROM relationships r \
          WHERE r.source_id = e.id OR r.target_id = e.id) \
         FROM entities e";
    let rows: Vec<(i64, i64, i64)> = if let Some(ns) = namespace {
        let mut stmt = tx.prepare(&format!("{SELECT_BASE} WHERE e.namespace = ?1"))?;
        let r = stmt
            .query_map(rusqlite::params![ns], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        r
    } else {
        let mut stmt = tx.prepare(SELECT_BASE)?;
        let r = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        r
    };

    let mut summary = RecomputeDegreeSummary {
        total: rows.len() as i64,
        updated: 0,
        zeroed: 0,
        unchanged: 0,
    };
    for (id, stored, real) in rows {
        if stored == real {
            summary.unchanged += 1;
            continue;
        }
        if !dry_run {
            tx.execute(
                "UPDATE entities SET degree = ?1, updated_at = unixepoch() WHERE id = ?2",
                rusqlite::params![real, id],
            )?;
        }
        if real == 0 {
            summary.zeroed += 1;
        } else {
            summary.updated += 1;
        }
    }

    if dry_run {
        // Dropping the transaction rolls back; nothing was written anyway.
        drop(tx);
    } else {
        tx.commit()?;
    }
    Ok(summary)
}

pub(crate) fn run_recompute_degree(args: GraphRecomputeDegreeArgs) -> Result<(), AppError> {
    let inicio = Instant::now();
    let paths = AppPaths::resolve(args.db.as_deref())?;
    crate::storage::connection::ensure_db_ready(&paths)?;
    let mut conn = crate::storage::connection::open_rw(&paths.db)?;

    let summary = recompute_degrees(&mut conn, args.namespace.as_deref(), args.dry_run)?;

    output::emit_json(&RecomputeDegreeResponse {
        namespace: args.namespace,
        dry_run: args.dry_run,
        total: summary.total,
        updated: summary.updated,
        zeroed: summary.zeroed,
        unchanged: summary.unchanged,
        elapsed_ms: inicio.elapsed().as_millis() as u64,
    })?;
    Ok(())
}

pub(crate) fn run_entities_snapshot(
    db: Option<&str>,
    namespace: Option<&str>,
    format: GraphExportFormat,
    json: bool,
    output_path: Option<&std::path::Path>,
) -> Result<(), AppError> {
    let inicio = Instant::now();
    let paths = AppPaths::resolve(db)?;

    crate::storage::connection::ensure_db_ready(&paths)?;

    let conn = open_ro(&paths.db)?;

    let nodes_raw = entities::list_entities(&conn, namespace)?;
    let edges_raw = entities::list_relationships_by_namespace(&conn, namespace)?;

    let id_to_name: HashMap<i64, String> =
        nodes_raw.iter().map(|n| (n.id, n.name.clone())).collect();

    let nodes: Vec<NodeOut> = nodes_raw
        .into_iter()
        .map(|n| NodeOut {
            id: n.id,
            name: n.name,
            namespace: n.namespace,
            r#type: n.kind.clone(),
            kind: n.kind,
        })
        .collect();

    let mut edges: Vec<EdgeOut> = Vec::with_capacity(edges_raw.len());
    let mut orphan_edges: usize = 0;
    for r in edges_raw {
        let from = match id_to_name.get(&r.source_id) {
            Some(n) => n.clone(),
            None => {
                orphan_edges += 1;
                tracing::warn!(target: "graph_export", source_id = r.source_id, relation = %r.relation, "edge skipped: source entity not found in id_to_name map");
                continue;
            }
        };
        let to = match id_to_name.get(&r.target_id) {
            Some(n) => n.clone(),
            None => {
                orphan_edges += 1;
                tracing::warn!(target: "graph_export", target_id = r.target_id, relation = %r.relation, "edge skipped: target entity not found in id_to_name map");
                continue;
            }
        };
        edges.push(EdgeOut {
            from,
            to,
            relation: r.relation,
            weight: r.weight,
        });
    }
    if orphan_edges > 0 {
        tracing::warn!(target: "graph_export",
            count = orphan_edges,
            "edges skipped due to orphaned entity references"
        );
    }

    let effective_format = if json {
        GraphExportFormat::Json
    } else {
        format
    };

    if effective_format == GraphExportFormat::Ndjson {
        let elapsed_ms = inicio.elapsed().as_millis() as u64;
        render_ndjson_streaming(&nodes, &edges, elapsed_ms, output_path)?;
        return Ok(());
    }

    let rendered = match effective_format {
        GraphExportFormat::Json => {
            let entities = nodes.clone();
            render_json(&GraphSnapshot {
                nodes,
                entities,
                edges,
                elapsed_ms: inicio.elapsed().as_millis() as u64,
            })?
        }
        GraphExportFormat::Dot => render_dot(&nodes, &edges),
        GraphExportFormat::Mermaid => render_mermaid(&nodes, &edges),
        GraphExportFormat::Ndjson => unreachable!("ndjson handled above"),
    };

    if let Some(path) = output_path.filter(|_| !json) {
        fs::write(path, &rendered)?;
        output::emit_progress(&format!("wrote {}", path.display()));
    } else {
        output::emit_text(&rendered);
    }

    Ok(())
}

pub(crate) fn run_traverse(args: GraphTraverseArgs) -> Result<(), AppError> {
    let inicio = Instant::now();
    let _ = args.format;
    let paths = AppPaths::resolve(args.db.as_deref())?;

    crate::storage::connection::ensure_db_ready(&paths)?;

    let conn = open_ro(&paths.db)?;
    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;

    // v1.1.05 Bug 3: exact match first; with --fuzzy auto-resolve clear
    // nickname/prefix hits; without it, NotFound includes ranked suggestions.
    let (from_id, resolved_name) =
        match entities::resolve_entity_fuzzy(&conn, &namespace, &args.from, args.fuzzy)? {
            Some((id, name, was_fuzzy)) => {
                if was_fuzzy {
                    tracing::warn!(
                        target: "graph_export",
                        query = %args.from,
                        resolved = %name,
                        "traverse: fuzzy-resolved entity name"
                    );
                }
                (id, name)
            }
            None => {
                return Err(entities::entity_not_found_with_suggestions(
                    &conn, &namespace, &args.from,
                ));
            }
        };

    let all_rels = entities::list_relationships_by_namespace(&conn, Some(&namespace))?;
    let all_entities = entities::list_entities(&conn, Some(&namespace))?;
    let id_to_name: HashMap<i64, String> = all_entities
        .iter()
        .map(|e| (e.id, e.name.clone()))
        .collect();

    let mut hops: Vec<TraverseHop> = Vec::with_capacity(16);
    let mut visited: std::collections::HashSet<i64> =
        std::collections::HashSet::with_capacity(args.depth as usize * 10);
    let mut frontier: Vec<(i64, u32)> = vec![(from_id, 0)];

    while let Some((current_id, current_depth)) = frontier.pop() {
        if current_depth >= args.depth || visited.contains(&current_id) {
            continue;
        }
        visited.insert(current_id);

        for rel in &all_rels {
            if rel.source_id == current_id {
                if let Some(target_name) = id_to_name.get(&rel.target_id) {
                    hops.push(TraverseHop {
                        entity: target_name.clone(),
                        relation: rel.relation.clone(),
                        direction: "outbound".to_string(),
                        weight: rel.weight,
                        depth: current_depth + 1,
                    });
                    frontier.push((rel.target_id, current_depth + 1));
                }
            } else if rel.target_id == current_id {
                if let Some(source_name) = id_to_name.get(&rel.source_id) {
                    hops.push(TraverseHop {
                        entity: source_name.clone(),
                        relation: rel.relation.clone(),
                        direction: "inbound".to_string(),
                        weight: rel.weight,
                        depth: current_depth + 1,
                    });
                    frontier.push((rel.source_id, current_depth + 1));
                }
            }
        }
    }

    output::emit_json(&GraphTraverseResponse {
        from: resolved_name,
        namespace,
        depth: args.depth,
        hops,
        elapsed_ms: inicio.elapsed().as_millis() as u64,
    })?;

    Ok(())
}

pub(crate) fn run_stats(args: GraphStatsArgs) -> Result<(), AppError> {
    let inicio = Instant::now();
    let paths = AppPaths::resolve(args.db.as_deref())?;

    crate::storage::connection::ensure_db_ready(&paths)?;

    let conn = open_ro(&paths.db)?;
    let ns = args.namespace.as_deref();

    let node_count: i64 = if let Some(n) = ns {
        conn.query_row(
            "SELECT COUNT(*) FROM entities WHERE namespace = ?1",
            rusqlite::params![n],
            |r| r.get(0),
        )?
    } else {
        conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?
    };

    let edge_count: i64 = if let Some(n) = ns {
        conn.query_row(
            "SELECT COUNT(*) FROM relationships r
             JOIN entities s ON s.id = r.source_id
             WHERE s.namespace = ?1",
            rusqlite::params![n],
            |r| r.get(0),
        )?
    } else {
        conn.query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))?
    };

    let max_degree: i64 = if let Some(n) = ns {
        conn.query_row(
            "SELECT COALESCE(MAX(degree), 0) FROM entities WHERE namespace = ?1",
            rusqlite::params![n],
            |r| r.get(0),
        )?
    } else {
        conn.query_row("SELECT COALESCE(MAX(degree), 0) FROM entities", [], |r| {
            r.get(0)
        })?
    };

    // avg_degree = 2 * edge_count / node_count (each edge contributes 2 to total degree sum).
    let avg_degree = if node_count > 0 {
        2.0 * (edge_count as f64) / (node_count as f64)
    } else {
        0.0
    };

    let resp = GraphStatsResponse {
        namespace: args.namespace,
        node_count,
        edge_count,
        avg_degree,
        max_degree,
        elapsed_ms: inicio.elapsed().as_millis() as u64,
    };

    let effective_format = if args.json {
        GraphStatsFormat::Json
    } else {
        args.format
    };

    match effective_format {
        GraphStatsFormat::Json => output::emit_json(&resp)?,
        GraphStatsFormat::Text => {
            output::emit_text(&format!(
                "nodes={} edges={} avg_degree={:.2} max_degree={} namespace={}",
                resp.node_count,
                resp.edge_count,
                resp.avg_degree,
                resp.max_degree,
                resp.namespace.as_deref().unwrap_or("all"),
            ));
        }
    }

    Ok(())
}

/// Builds the `ORDER BY` clause fragment from sort options.
///
/// Returns a static SQL fragment such as `ORDER BY e.name ASC`.
pub(crate) fn build_order_by(sort_by: Option<EntitySortField>, order: SortOrder) -> &'static str {
    // The combinations are enumerated as static strings to avoid
    // format!() allocations in the hot path and satisfy the borrow checker
    // when the string is used inside conn.prepare().
    match (sort_by, order) {
        (None, SortOrder::Asc) | (Some(EntitySortField::Name), SortOrder::Asc) => {
            "ORDER BY e.name ASC"
        }
        (Some(EntitySortField::Name), SortOrder::Desc) => "ORDER BY e.name DESC",
        (Some(EntitySortField::Degree), SortOrder::Asc) => "ORDER BY degree ASC",
        (Some(EntitySortField::Degree), SortOrder::Desc) => "ORDER BY degree DESC",
        (Some(EntitySortField::CreatedAt), SortOrder::Asc) => "ORDER BY e.created_at ASC",
        (Some(EntitySortField::CreatedAt), SortOrder::Desc) => "ORDER BY e.created_at DESC",
        // Fallback: None/Desc → sort by name desc (consistent with dir variable).
        (None, SortOrder::Desc) => "ORDER BY e.name DESC",
    }
}

pub(crate) fn run_entities(args: GraphEntitiesArgs) -> Result<(), AppError> {
    let inicio = Instant::now();
    let paths = AppPaths::resolve(args.db.as_deref())?;

    crate::storage::connection::ensure_db_ready(&paths)?;

    let conn = open_ro(&paths.db)?;

    let row_to_item = |r: &rusqlite::Row<'_>| -> rusqlite::Result<EntityItem> {
        let ts: i64 = r.get(4)?;
        let created_at = chrono::DateTime::from_timestamp(ts, 0)
            .unwrap_or_default()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        Ok(EntityItem {
            id: r.get(0)?,
            name: r.get(1)?,
            entity_type: r.get(2)?,
            namespace: r.get(3)?,
            created_at,
            degree: r.get(5)?,
            description: r.get(6)?,
        })
    };

    let limit_i = args.limit as i64;
    let offset_i = args.offset as i64;
    let order_clause = build_order_by(args.sort_by, args.order);

    let base_select = "SELECT e.id, e.name, COALESCE(e.type, ''), e.namespace, e.created_at,
                        (SELECT COUNT(*) FROM relationships r
                         WHERE r.source_id = e.id OR r.target_id = e.id) AS degree,
                        e.description
                 FROM entities e";

    let (total_count, items) = match (
        args.namespace.as_deref(),
        args.entity_type.map(|et| et.as_str()),
    ) {
        (Some(ns), Some(et)) => {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM entities WHERE namespace = ?1 AND type = ?2",
                rusqlite::params![ns, et],
                |r| r.get(0),
            )?;
            let sql = format!(
                "{base_select} WHERE e.namespace = ?1 AND e.type = ?2 {order_clause} LIMIT ?3 OFFSET ?4"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params![ns, et, limit_i, offset_i], row_to_item)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (count, rows)
        }
        (Some(ns), None) => {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM entities WHERE namespace = ?1",
                rusqlite::params![ns],
                |r| r.get(0),
            )?;
            let sql =
                format!("{base_select} WHERE e.namespace = ?1 {order_clause} LIMIT ?2 OFFSET ?3");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params![ns, limit_i, offset_i], row_to_item)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (count, rows)
        }
        (None, Some(et)) => {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM entities WHERE type = ?1",
                rusqlite::params![et],
                |r| r.get(0),
            )?;
            let sql = format!("{base_select} WHERE e.type = ?1 {order_clause} LIMIT ?2 OFFSET ?3");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params![et, limit_i, offset_i], row_to_item)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (count, rows)
        }
        (None, None) => {
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
            let sql = format!("{base_select} {order_clause} LIMIT ?1 OFFSET ?2");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params![limit_i, offset_i], row_to_item)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (count, rows)
        }
    };

    output::emit_json(&GraphEntitiesResponse {
        entities: items,
        total_count,
        limit: args.limit,
        offset: args.offset,
        namespace: args.namespace,
        elapsed_ms: inicio.elapsed().as_millis() as u64,
    })
}
