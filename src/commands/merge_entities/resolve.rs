//! Namespace-scoped resolution of an entity ID to its name.
//!
//! v1.1.1 (P5): IDs are globally unique, so a homonym in another namespace must
//! NOT be reachable through its ID from the wrong namespace.

use crate::errors::AppError;
use rusqlite::params;

/// v1.1.1 (P5): resolves an entity ID to its name. When `enforce_namespace`
/// is true (default behaviour, same-namespace safety), the lookup also enforces
/// that the entity belongs to `namespace` — IDs are global, so a bare existence
/// check could silently cross namespaces. When false (v1.1.03 cross-namespace
/// merge), the lookup resolves the entity by its own row and returns the
/// namespace it actually lives in, so callers can audit the cross-namespace move.
pub(super) fn find_entity_name_by_id(
    conn: &rusqlite::Connection,
    namespace: &str,
    id: i64,
    enforce_namespace: bool,
) -> Result<(String, String), AppError> {
    let mut stmt = if enforce_namespace {
        conn.prepare_cached(
            "SELECT name, namespace FROM entities WHERE id = ?1 AND namespace = ?2",
        )?
    } else {
        conn.prepare_cached("SELECT name, namespace FROM entities WHERE id = ?1")?
    };
    let row = if enforce_namespace {
        stmt.query_row(params![id, namespace], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
    } else {
        stmt.query_row(params![id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
    };
    match row {
        Ok((name, ns_actual)) => Ok((name, ns_actual)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(AppError::NotFound(
            crate::i18n::validation::entity_id_not_found_in_namespace(id, namespace),
        )),
        Err(e) => Err(AppError::Database(e)),
    }
}
