//! GAP-SG-185 v1.2.4: page→enqueue streaming for high-volume enrich scans.
//!
//! Production enqueue walks keyset pages and never retains the full key list.
//! Dry-run and unit tests keep [`super::scan_operation`] (full `Vec`).

use super::super::args::{EnrichArgs, EnrichOperation, ReEmbedTarget};
use super::super::predicates::{
    entity_description_scan_predicate, is_low_quality_description, reembed_chunk_predicate,
    reembed_entity_predicate, reembed_memory_predicate, UNBOUND_MEMORY_PREDICATE,
};
use super::name_filter::resolve_name_filter;
use super::sql::{keyset_for_each, keyset_for_each_selected};
use crate::errors::AppError;
use rusqlite::Connection;

/// Walk candidate keys in pages, invoking `on_page` for each page.
///
/// Returns the total number of keys delivered. Peak key-buffer RSS is O(page_size)
/// for keyset-backed operations without a name filter; non-keyset ops (and name
/// filters) deliver via a single full collect page.
pub(in crate::commands::enrich) fn scan_operation_for_each<F>(
    conn: &Connection,
    namespace: &str,
    args: &EnrichArgs,
    mut on_page: F,
) -> Result<usize, AppError>
where
    F: FnMut(Vec<String>) -> Result<(), AppError>,
{
    let name_filter = resolve_name_filter(args)?;
    let page_size = args.scan_page_size().max(1);
    let limit = args.limit;

    // Name filters are small explicit subsets; reuse the full collect path so
    // keyset page fullness is not corrupted by post-fetch filtering.
    if !name_filter.is_empty() {
        return deliver_full(conn, namespace, args, &mut on_page);
    }

    match args.operation() {
        EnrichOperation::MemoryBindings => {
            for_each_unbound(conn, namespace, limit, page_size, &mut on_page)
        }
        EnrichOperation::BodyEnrich => for_each_short_body(
            conn,
            namespace,
            args.min_output_chars,
            limit,
            page_size,
            &mut on_page,
        ),
        EnrichOperation::ReEmbed => {
            for_each_reembed(conn, namespace, args, page_size, &mut on_page)
        }
        EnrichOperation::EntityDescriptions => for_each_entity_descriptions(
            conn,
            namespace,
            args.force_redescribe,
            limit,
            page_size,
            &mut on_page,
        ),
        EnrichOperation::DomainClassify
        | EnrichOperation::GraphAudit
        | EnrichOperation::DeepResearchSynth
        | EnrichOperation::BodyExtract => {
            for_each_all_memory_names(conn, namespace, limit, page_size, &mut on_page)
        }
        _ => deliver_full(conn, namespace, args, &mut on_page),
    }
}

fn deliver_full<F>(
    conn: &Connection,
    namespace: &str,
    args: &EnrichArgs,
    on_page: &mut F,
) -> Result<usize, AppError>
where
    F: FnMut(Vec<String>) -> Result<(), AppError>,
{
    let keys = super::scan_operation(conn, namespace, args)?;
    let n = keys.len();
    if n > 0 {
        on_page(keys)?;
    }
    Ok(n)
}

fn for_each_unbound<F>(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    page_size: usize,
    on_page: &mut F,
) -> Result<usize, AppError>
where
    F: FnMut(Vec<String>) -> Result<(), AppError>,
{
    keyset_for_each(
        limit,
        page_size,
        &mut |after, want| {
            let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
            let sql = format!(
                "SELECT m.id, m.name FROM memories m
                 WHERE m.namespace = ?1 AND m.deleted_at IS NULL AND m.id > ?2
                   AND {UNBOUND_MEMORY_PREDICATE}
                 ORDER BY m.id LIMIT ?3"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params![namespace, after, limit_v], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        },
        // `&mut F` implements `FnMut` whenever `F` does, so the reborrow is
        // handed straight to `keyset_for_each` — wrapping it in a closure only
        // added a layer clippy rejects as redundant.
        &mut *on_page,
    )
}

fn for_each_short_body<F>(
    conn: &Connection,
    namespace: &str,
    min_chars: usize,
    limit: Option<usize>,
    page_size: usize,
    on_page: &mut F,
) -> Result<usize, AppError>
where
    F: FnMut(Vec<String>) -> Result<(), AppError>,
{
    let min_chars_i64 = min_chars as i64;
    keyset_for_each(
        limit,
        page_size,
        &mut |after, want| {
            let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
            let sql = "SELECT m.id, m.name FROM memories m
             WHERE m.namespace = ?1 AND m.deleted_at IS NULL AND m.id > ?2
               AND LENGTH(COALESCE(m.body,'')) < ?3
             ORDER BY m.id LIMIT ?4";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt
                .query_map(
                    rusqlite::params![namespace, after, min_chars_i64, limit_v],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        },
        // `&mut F` implements `FnMut` whenever `F` does, so the reborrow is
        // handed straight to `keyset_for_each` — wrapping it in a closure only
        // added a layer clippy rejects as redundant.
        &mut *on_page,
    )
}

fn for_each_all_memory_names<F>(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    page_size: usize,
    on_page: &mut F,
) -> Result<usize, AppError>
where
    F: FnMut(Vec<String>) -> Result<(), AppError>,
{
    keyset_for_each(
        limit,
        page_size,
        &mut |after, want| {
            let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
            let sql = "SELECT id, name FROM memories
                   WHERE namespace=?1 AND deleted_at IS NULL AND id > ?2
                   ORDER BY id LIMIT ?3";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt
                .query_map(rusqlite::params![namespace, after, limit_v], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        },
        // `&mut F` implements `FnMut` whenever `F` does, so the reborrow is
        // handed straight to `keyset_for_each` — wrapping it in a closure only
        // added a layer clippy rejects as redundant.
        &mut *on_page,
    )
}

/// Streams entities whose description is missing or, under `--force-redescribe`,
/// too low-quality to keep.
///
/// The quality test lives in Rust, not in SQL, so a fetched row can still be
/// rejected — which is exactly the case
/// [`super::sql::keyset_for_each_selected`] exists for. Judging page fullness on
/// rows scanned rather than rows kept is what stops a page thinned by rejections
/// from being read as the end of the table.
fn for_each_entity_descriptions<F>(
    conn: &Connection,
    namespace: &str,
    force_redescribe: bool,
    limit: Option<usize>,
    page_size: usize,
    on_page: &mut F,
) -> Result<usize, AppError>
where
    F: FnMut(Vec<String>) -> Result<(), AppError>,
{
    // Unfiltered paging path: no operator named anyone, so the heuristic decides.
    let desc_pred = entity_description_scan_predicate(force_redescribe, false);
    keyset_for_each_selected(
        limit,
        page_size,
        &mut |after, want| {
            let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
            let sql = format!(
                "SELECT id, name, COALESCE(description, '') FROM entities
                 WHERE namespace = ?1 AND id > ?2 AND {desc_pred}
                 ORDER BY id LIMIT ?3"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params![namespace, after, limit_v], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows
                .into_iter()
                .map(|(id, name, desc)| {
                    let keep = !force_redescribe
                        || desc.trim().is_empty()
                        || is_low_quality_description(&desc);
                    (id, keep.then_some(name))
                })
                .collect())
        },
        &mut *on_page,
    )
}

fn for_each_reembed<F>(
    conn: &Connection,
    namespace: &str,
    args: &EnrichArgs,
    page_size: usize,
    on_page: &mut F,
) -> Result<usize, AppError>
where
    F: FnMut(Vec<String>) -> Result<(), AppError>,
{
    let mut total = 0usize;
    let limit = args.limit;
    if matches!(args.target, ReEmbedTarget::Memories | ReEmbedTarget::All) {
        let pred = reembed_memory_predicate(crate::constants::embedding_dim());
        let n = keyset_for_each(
            limit,
            page_size,
            &mut |after, want| {
                let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
                let sql = format!(
                    "SELECT m.id, m.name FROM memories m
                 WHERE m.namespace = ?1 AND m.deleted_at IS NULL AND m.id > ?2
                   AND {pred}
                 ORDER BY m.id LIMIT ?3"
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt
                    .query_map(rusqlite::params![namespace, after, limit_v], |r| {
                        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            },
            &mut *on_page,
        )?;
        total = total.saturating_add(n);
    }
    if matches!(args.target, ReEmbedTarget::Entities | ReEmbedTarget::All) {
        let pred = reembed_entity_predicate(crate::constants::embedding_dim());
        let n = keyset_for_each(
            limit,
            page_size,
            &mut |after, want| {
                let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
                let sql = format!(
                    "SELECT e.id, e.name FROM entities e
                 WHERE e.namespace = ?1 AND e.id > ?2 AND {pred}
                 ORDER BY e.id LIMIT ?3"
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt
                    .query_map(rusqlite::params![namespace, after, limit_v], |r| {
                        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            },
            |page| on_page(page.into_iter().map(|n| format!("entity:{n}")).collect()),
        )?;
        total = total.saturating_add(n);
    }
    if matches!(args.target, ReEmbedTarget::Chunks | ReEmbedTarget::All) {
        let pred = reembed_chunk_predicate(crate::constants::embedding_dim());
        let n = keyset_for_each(
            limit,
            page_size,
            &mut |after, want| {
                let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
                let sql = format!(
                    "SELECT c.id FROM memory_chunks c
                 LEFT JOIN memories m ON m.id = c.memory_id
                 WHERE (m.namespace = ?1 OR m.id IS NULL) AND c.id > ?2 AND {pred}
                 ORDER BY c.id LIMIT ?3"
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt
                    .query_map(rusqlite::params![namespace, after, limit_v], |r| {
                        let id = r.get::<_, i64>(0)?;
                        Ok((id, id))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            },
            |page| on_page(page.into_iter().map(|id| format!("chunk:{id}")).collect()),
        )?;
        total = total.saturating_add(n);
    }
    Ok(total)
}

#[cfg(test)]
mod entity_description_stream_tests {
    use super::*;

    /// Minimal `entities` shape the description scan touches.
    fn seeded_conn(rows: &[(&str, Option<&str>)]) -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE entities (
                 id INTEGER PRIMARY KEY,
                 namespace TEXT NOT NULL,
                 name TEXT NOT NULL,
                 type TEXT NOT NULL DEFAULT 'concept',
                 description TEXT
             );",
        )
        .expect("schema");
        for (name, description) in rows {
            conn.execute(
                "INSERT INTO entities (namespace, name, description) VALUES ('global', ?1, ?2)",
                rusqlite::params![name, description],
            )
            .expect("seed row");
        }
        conn
    }

    fn streamed(conn: &Connection, force: bool, limit: Option<usize>, page: usize) -> Vec<String> {
        let mut seen = Vec::new();
        for_each_entity_descriptions(conn, "global", force, limit, page, &mut |names| {
            seen.extend(names);
            Ok(())
        })
        .expect("stream walk");
        seen
    }

    /// A page whose rows are all rejected must not be read as end-of-table.
    ///
    /// This is the failure the plain `keyset_for_each` would have produced: with
    /// `page_size = 2`, the middle page holds two acceptable descriptions, gets
    /// thinned to zero, and `n < want` ends the scan — losing every entity
    /// behind it. The tail here sits behind exactly such a page.
    #[test]
    fn a_fully_rejected_page_does_not_end_the_scan() {
        let conn = seeded_conn(&[
            ("alpha", None),
            ("beta", Some("")),
            (
                "gamma",
                Some("a genuinely specific description of the parser"),
            ),
            ("delta", Some("another honest sentence about the scheduler")),
            ("epsilon", None),
        ]);

        let names = streamed(&conn, true, None, 2);

        assert!(
            names.contains(&"epsilon".to_string()),
            "the tail behind a fully rejected page was lost: {names:?}"
        );
        assert_eq!(names, vec!["alpha", "beta", "epsilon"]);
    }

    /// Streaming must deliver exactly what the full collect delivers, at every
    /// page width — including one narrower than the number of rejections.
    #[test]
    fn streaming_matches_the_full_scan_at_every_page_width() {
        let rows: Vec<(String, Option<String>)> = (0..40)
            .map(|i| {
                let name = format!("entity-{i:02}");
                let description = match i % 3 {
                    0 => None,
                    1 => Some(String::new()),
                    _ => Some(format!("a specific description number {i} of real prose")),
                };
                (name, description)
            })
            .collect();
        let borrowed: Vec<(&str, Option<&str>)> = rows
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_deref()))
            .collect();
        let conn = seeded_conn(&borrowed);

        let reference = streamed(&conn, true, None, 4096);
        for page in [1usize, 2, 3, 7, 40, 100] {
            assert_eq!(
                streamed(&conn, true, None, page),
                reference,
                "page width {page} diverged from the single-page walk"
            );
        }
    }

    /// `--limit N` counts items DELIVERED, not rows scanned.
    ///
    /// Decrementing the budget by rows looked at would return fewer than N
    /// usable entities whenever the post-filter rejected any of them.
    #[test]
    fn limit_counts_delivered_items_not_scanned_rows() {
        let conn = seeded_conn(&[
            ("a", None),
            (
                "b",
                Some("a real and sufficiently specific description here"),
            ),
            ("c", None),
            (
                "d",
                Some("another real description that must not be rewritten"),
            ),
            ("e", None),
        ]);

        assert_eq!(streamed(&conn, true, Some(2), 2), vec!["a", "c"]);
        assert_eq!(streamed(&conn, true, Some(3), 2), vec!["a", "c", "e"]);
    }
}
