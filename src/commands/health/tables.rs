//! Table discovery and row-count helpers for health checks.

/// Candidate table names for memory embeddings (canonical first, legacy second).
pub(super) const MEMORY_EMBEDDING_TABLES: &[&str] = &["memory_embeddings", "vec_memories"];
/// Candidate table names for entity embeddings (canonical first, legacy second).
pub(super) const ENTITY_EMBEDDING_TABLES: &[&str] = &["entity_embeddings", "vec_entities"];
/// Candidate table names for chunk embeddings (canonical first, legacy second).
pub(super) const CHUNK_EMBEDDING_TABLES: &[&str] = &["chunk_embeddings", "vec_chunks"];

/// Checks whether a table (including virtual ones) exists in sqlite_master.
pub(super) fn table_exists(conn: &rusqlite::Connection, table_name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table', 'shadow') AND name = ?1",
        rusqlite::params![table_name],
        |r| r.get::<_, i64>(0),
    )
    .unwrap_or(0)
        > 0
}

/// Returns the first candidate table that exists in the connection.
pub(super) fn first_existing_table<'a>(
    conn: &rusqlite::Connection,
    candidates: &'a [&'a str],
) -> Option<&'a str> {
    candidates
        .iter()
        .copied()
        .find(|name| table_exists(conn, name))
}

/// Counts rows in `table_name`, returning 0 on error.
pub(super) fn count_rows(conn: &rusqlite::Connection, table_name: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table_name}"), [], |r| {
        r.get(0)
    })
    .unwrap_or(0)
}
