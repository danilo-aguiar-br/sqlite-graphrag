//! GAP-005 (v1.0.82): DAO for the `pending_embeddings` table.
//!
//! Queue of memories persisted with a NULL embedding for later reprocessing
//! via `embedding retry --backend <KIND>` or `enrich --operation re-embed --pending-only`.

use rusqlite::{params, Connection};

use crate::errors::AppError;

/// Pending embedding status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingEmbeddingStatus {
    /// Pending variant.
    Pending,
    /// In progress variant.
    InProgress,
    /// Done variant.
    Done,
    /// Abandoned variant.
    Abandoned,
}

impl PendingEmbeddingStatus {
    /// Return the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }
}

/// Pending embedding.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingEmbedding {
    /// Pending ID.
    pub pending_id: i64,
    /// Memory identifier.
    pub memory_id: i64,
    /// Namespace scope.
    pub namespace: String,
    /// Name of this item.
    pub name: String,
    /// Backend chain.
    pub backend_chain: String,
    /// Last error.
    pub last_error: Option<String>,
    /// Last exit code.
    pub last_exit_code: Option<i32>,
    /// Last stderr tail.
    pub last_stderr_tail: Option<String>,
    /// Attempt count.
    pub attempt_count: i32,
    /// Status value.
    pub status: PendingEmbeddingStatus,
    /// Creation timestamp.
    pub created_at: i64,
    /// Last-update timestamp.
    pub updated_at: i64,
}

/// Inserts a new `pending_embeddings` entry with status `pending`.
// One parameter per column of the `pending_embeddings` row this writes: the
// arity IS the schema, and a struct here would be that row spelled a second time.
#[allow(clippy::too_many_arguments)]
pub fn insert(
    conn: &Connection,
    memory_id: i64,
    namespace: &str,
    name: &str,
    backend_chain: &str,
    last_error: Option<&str>,
    last_exit_code: Option<i32>,
    last_stderr_tail: Option<&str>,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO pending_embeddings
            (memory_id, namespace, name, backend_chain, last_error,
             last_exit_code, last_stderr_tail, attempt_count, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 'pending')",
        params![
            memory_id,
            namespace,
            name,
            backend_chain,
            last_error,
            last_exit_code,
            last_stderr_tail,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Update status.
pub fn update_status(
    conn: &Connection,
    pending_id: i64,
    status: PendingEmbeddingStatus,
    last_error: Option<&str>,
    last_exit_code: Option<i32>,
    last_stderr_tail: Option<&str>,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE pending_embeddings
         SET status = ?1,
             last_error = COALESCE(?2, last_error),
             last_exit_code = COALESCE(?3, last_exit_code),
             last_stderr_tail = COALESCE(?4, last_stderr_tail),
             attempt_count = attempt_count + 1,
             updated_at = unixepoch()
         WHERE pending_id = ?5",
        params![
            status.as_str(),
            last_error,
            last_exit_code,
            last_stderr_tail,
            pending_id
        ],
    )?;
    Ok(())
}

/// Counts the entries [`list_by_status`] would page through.
///
/// GAP-SG-201: `embedding list --limit N` is a page of a countable set, and the
/// output surface cannot tell a page from a corpus without the size of the
/// corpus. The `WHERE` clause mirrors [`list_by_status`] exactly.
///
/// # Errors
/// Returns [`AppError`] when the query fails.
pub fn count_by_status(
    conn: &Connection,
    status: PendingEmbeddingStatus,
) -> Result<usize, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pending_embeddings WHERE status = ?1",
        params![status.as_str()],
        |row| row.get(0),
    )?;
    Ok(usize::try_from(count).unwrap_or(0))
}

/// List by status.
pub fn list_by_status(
    conn: &Connection,
    status: PendingEmbeddingStatus,
    limit: usize,
) -> Result<Vec<PendingEmbedding>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT pending_id, memory_id, namespace, name, backend_chain,
                last_error, last_exit_code, last_stderr_tail,
                attempt_count, status, created_at, updated_at
         FROM pending_embeddings
         WHERE status = ?1
         ORDER BY updated_at ASC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![status.as_str(), limit as i64], |row| {
        Ok(PendingEmbedding {
            pending_id: row.get(0)?,
            memory_id: row.get(1)?,
            namespace: row.get(2)?,
            name: row.get(3)?,
            backend_chain: row.get(4)?,
            last_error: row.get(5)?,
            last_exit_code: row.get(6)?,
            last_stderr_tail: row.get(7)?,
            attempt_count: row.get(8)?,
            status: parse_status(&row.get::<_, String>(9)?).map_err(|e| -> rusqlite::Error {
                rusqlite::Error::FromSqlConversionFailure(
                    9,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Abandon.
pub fn abandon(conn: &Connection, pending_id: i64) -> Result<(), AppError> {
    update_status(
        conn,
        pending_id,
        PendingEmbeddingStatus::Abandoned,
        None,
        None,
        None,
    )
}

/// Delete.
pub fn delete(conn: &Connection, pending_id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM pending_embeddings WHERE pending_id = ?1",
        params![pending_id],
    )?;
    Ok(())
}

fn parse_status(s: &str) -> Result<PendingEmbeddingStatus, AppError> {
    match s {
        "pending" => Ok(PendingEmbeddingStatus::Pending),
        "in_progress" => Ok(PendingEmbeddingStatus::InProgress),
        "done" => Ok(PendingEmbeddingStatus::Done),
        "abandoned" => Ok(PendingEmbeddingStatus::Abandoned),
        other => Err(AppError::Validation(
            crate::i18n::validation::unknown_pending_embeddings_status(other),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("pragma");
        crate::migrations::runner()
            .run(&mut conn)
            .expect("migrations apply");
        conn
    }

    fn insert_test_memory(conn: &Connection, name: &str) -> i64 {
        conn.execute(
            "INSERT INTO memories (name, namespace, type, description, body, body_hash, source)
             VALUES (?1, 'global', 'note', 'desc', 'body', 'h', 'agent')",
            params![name],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn insert_records_pending_with_full_diagnostics() {
        let conn = fresh_db();
        let mid = insert_test_memory(&conn, "p");
        let id = insert(
            &conn,
            mid,
            "global",
            "p",
            "openrouter,none",
            Some("exit 137 SIGKILL"),
            Some(137),
            Some("OOM killed by kernel"),
        )
        .unwrap();
        let p = list_by_status(&conn, PendingEmbeddingStatus::Pending, 10)
            .unwrap()
            .into_iter()
            .find(|p| p.pending_id == id)
            .expect("pending found");
        assert_eq!(p.backend_chain, "openrouter,none");
        assert_eq!(p.last_exit_code, Some(137));
        assert_eq!(p.last_stderr_tail.as_deref(), Some("OOM killed by kernel"));
    }

    #[test]
    fn update_status_increments_attempt_count() {
        let conn = fresh_db();
        let mid = insert_test_memory(&conn, "p");
        let id = insert(&conn, mid, "global", "p", "openrouter", None, None, None).unwrap();
        update_status(
            &conn,
            id,
            PendingEmbeddingStatus::InProgress,
            None,
            None,
            None,
        )
        .unwrap();
        let p = list_by_status(&conn, PendingEmbeddingStatus::InProgress, 10)
            .unwrap()
            .into_iter()
            .find(|p| p.pending_id == id)
            .expect("found");
        assert_eq!(p.attempt_count, 1);
    }

    #[test]
    fn abandon_sets_status() {
        let conn = fresh_db();
        let mid = insert_test_memory(&conn, "p");
        let id = insert(&conn, mid, "global", "p", "openrouter", None, None, None).unwrap();
        abandon(&conn, id).unwrap();
        let abandoned = list_by_status(&conn, PendingEmbeddingStatus::Abandoned, 10).unwrap();
        assert!(abandoned.iter().any(|p| p.pending_id == id));
    }

    #[test]
    fn delete_removes_row() {
        let conn = fresh_db();
        let mid = insert_test_memory(&conn, "p");
        let id = insert(&conn, mid, "global", "p", "openrouter", None, None, None).unwrap();
        delete(&conn, id).unwrap();
        let pending = list_by_status(&conn, PendingEmbeddingStatus::Pending, 10).unwrap();
        assert!(pending.iter().all(|p| p.pending_id != id));
    }
}
