//! FTS5 shadow table: query sanitisation, BM25 search and index sync.
//!
//! Owns every statement touching `fts_memories`, plus the sanitiser that keeps
//! raw operator input from reaching the FTS5 query parser.

use super::rows::MemoryRow;
use crate::errors::AppError;
use rusqlite::{params, Connection};

/// Preprocesses a raw user query for FTS5 `MATCH`.
///
/// Technical separators (`-`, `.`, `_`, `/`) are treated as word boundaries by
/// the `unicode61` tokenizer.  When the query contains any of these characters
/// the function builds a compound FTS5 expression:
///   1. A phrase query with the separated tokens (exact compound matching).
///   2. Individual prefix terms joined with OR (broader recall).
///
/// Queries without separators keep the original `term*` prefix behaviour.
pub(super) fn preprocess_fts_query(raw: &str) -> String {
    const SEPARATORS: &[char] = &['-', '.', '_', '/'];
    const FTS5_SYNTAX: &[char] = &['"', '*', '(', ')', '^', ':'];
    const FTS5_KEYWORDS: &[&str] = &["OR", "AND", "NOT", "NEAR"];

    let sanitized: String = raw.chars().filter(|c| !FTS5_SYNTAX.contains(c)).collect();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let is_fts_keyword = |t: &str| FTS5_KEYWORDS.iter().any(|kw| kw.eq_ignore_ascii_case(t));

    if !trimmed.chars().any(|c| SEPARATORS.contains(&c)) {
        return trimmed
            .split_whitespace()
            .filter(|t| !is_fts_keyword(t))
            .map(|t| format!("{t}*"))
            .collect::<Vec<_>>()
            .join(" ");
    }
    let tokens: Vec<&str> = trimmed
        .split(|c: char| SEPARATORS.contains(&c) || c.is_whitespace())
        .filter(|t| !t.is_empty() && !is_fts_keyword(t))
        .collect();
    if tokens.is_empty() {
        return String::new();
    }
    let phrase = format!("\"{}\"", tokens.join(" "));
    let prefix_terms: Vec<String> = tokens.iter().map(|t| format!("{t}*")).collect();
    format!("{phrase} OR {}", prefix_terms.join(" OR "))
}

/// Executes an FTS5 search against `fts_memories` with query preprocessing.
///
/// Technical separators in the query are converted to phrase + prefix OR
/// expressions so compound terms like `graphrag-precompact.sh` match correctly.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn fts_search(
    conn: &Connection,
    query: &str,
    namespace: &str,
    memory_type: Option<&str>,
    limit: usize,
) -> Result<Vec<MemoryRow>, AppError> {
    let fts_query = preprocess_fts_query(query);
    if let Some(mt) = memory_type {
        let mut stmt = conn.prepare_cached(
            "SELECT m.id, m.namespace, m.name, m.type, m.description, m.body, m.body_hash,
                    m.session_id, m.source, m.metadata, m.created_at, m.updated_at, m.deleted_at
             FROM fts_memories fts
             JOIN memories m ON m.id = fts.rowid
             WHERE fts_memories MATCH ?1 AND m.namespace = ?2 AND m.type = ?3 AND m.deleted_at IS NULL
             ORDER BY rank LIMIT ?4",
        )?;
        let rows = stmt
            .query_map(params![fts_query, namespace, mt, limit as i64], |r| {
                Ok(MemoryRow {
                    id: r.get(0)?,
                    namespace: r.get(1)?,
                    name: r.get(2)?,
                    memory_type: r.get(3)?,
                    description: r.get(4)?,
                    body: r.get(5)?,
                    body_hash: r.get(6)?,
                    session_id: r.get(7)?,
                    source: r.get(8)?,
                    metadata: r.get(9)?,
                    created_at: r.get(10)?,
                    updated_at: r.get(11)?,
                    deleted_at: r.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let mut stmt = conn.prepare_cached(
            "SELECT m.id, m.namespace, m.name, m.type, m.description, m.body, m.body_hash,
                    m.session_id, m.source, m.metadata, m.created_at, m.updated_at, m.deleted_at
             FROM fts_memories fts
             JOIN memories m ON m.id = fts.rowid
             WHERE fts_memories MATCH ?1 AND m.namespace = ?2 AND m.deleted_at IS NULL
             ORDER BY rank LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![fts_query, namespace, limit as i64], |r| {
                Ok(MemoryRow {
                    id: r.get(0)?,
                    namespace: r.get(1)?,
                    name: r.get(2)?,
                    memory_type: r.get(3)?,
                    description: r.get(4)?,
                    body: r.get(5)?,
                    body_hash: r.get(6)?,
                    session_id: r.get(7)?,
                    source: r.get(8)?,
                    metadata: r.get(9)?,
                    created_at: r.get(10)?,
                    updated_at: r.get(11)?,
                    deleted_at: r.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// Syncs FTS5 external-content index after an UPDATE on the memories table.
///
/// The AFTER UPDATE trigger (`trg_fts_au`) is intentionally absent because
/// sqlite-vec loaded via `sqlite3_auto_extension` conflicts with FTS5 inside
/// UPDATE triggers. This function performs the equivalent sync in Rust:
/// DELETE the old entry, then INSERT the new one (external-content FTS5
/// tables do not support in-place UPDATE).
#[allow(clippy::too_many_arguments)]
pub fn sync_fts_after_update(
    conn: &Connection,
    memory_id: i64,
    old_name: &str,
    old_desc: &str,
    old_body: &str,
    new_name: &str,
    new_desc: &str,
    new_body: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO fts_memories(fts_memories, rowid, name, description, body)
         VALUES('delete', ?1, ?2, ?3, ?4)",
        params![memory_id, old_name, old_desc, old_body],
    )?;
    conn.execute(
        "INSERT INTO fts_memories(rowid, name, description, body)
         VALUES(?1, ?2, ?3, ?4)",
        params![memory_id, new_name, new_desc, new_body],
    )?;
    Ok(())
}
