//! Scanner whose target is the `memory_chunks` table.
//!
//! GAP-SG-185: keyset pages over chunk ids.

use super::super::predicates::reembed_chunk_predicate;
use super::sql::{keyset_collect, placeholder_list};
use crate::errors::AppError;
use rusqlite::Connection;

/// Chunk rows without a live vector — keyset paged (GAP-SG-185).
pub(in crate::commands::enrich) fn scan_chunks_missing_embeddings(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    name_filter: &[String],
    page_size: usize,
) -> Result<Vec<i64>, AppError> {
    let predicate = reembed_chunk_predicate(crate::constants::embedding_dim());
    keyset_collect(limit, page_size, |after, want| {
        let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
        if name_filter.is_empty() {
            let sql = format!(
                "SELECT c.id
                 FROM memory_chunks c
                 LEFT JOIN memories m ON m.id = c.memory_id
                 WHERE (m.namespace = ?1 OR m.id IS NULL)
                   AND c.id > ?2
                   AND {predicate}
                 ORDER BY c.id
                 LIMIT ?3"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params![namespace, after, limit_v], |r| {
                    let id = r.get::<_, i64>(0)?;
                    Ok((id, id))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        } else {
            let in_clause = placeholder_list(3, name_filter.len());
            let lim_idx = 3 + name_filter.len();
            let sql = format!(
                "SELECT c.id
                 FROM memory_chunks c
                 LEFT JOIN memories m ON m.id = c.memory_id
                 WHERE (m.namespace = ?1 OR m.id IS NULL)
                   AND c.id > ?2
                   AND (m.name IN ({in_clause}) OR m.id IS NULL)
                   AND {predicate}
                 ORDER BY c.id
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
                        Ok((id, id))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    })
}
