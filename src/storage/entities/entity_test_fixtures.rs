//! Shared fixtures for the entity storage tests (GAP-SG-146).
//!
//! `tests_a.rs` and `tests_b.rs` each carried their own copy of the schema
//! bootstrap and the entity builder.

use super::*;
use crate::constants::embedding_dim;
use crate::entity_type::EntityType;
use crate::storage::connection::register_vec_extension;
use rusqlite::Connection;
use tempfile::TempDir;

/// Shared alias for a test that propagates errors with `?`.
pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn setup_db() -> Result<(TempDir, Connection), Box<dyn std::error::Error>> {
    register_vec_extension();
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("test.db");
    let mut conn = Connection::open(&db_path)?;
    crate::migrations::runner().run(&mut conn)?;
    Ok((tmp, conn))
}

pub(super) fn new_entity_helper(name: &str) -> NewEntity {
    NewEntity {
        name: name.to_string(),
        entity_type: EntityType::Project,
        description: None,
    }
}

pub(super) fn embedding_zero() -> Vec<f32> {
    vec![0.0f32; embedding_dim()]
}

pub(super) fn insert_memory(conn: &Connection) -> Result<i64, Box<dyn std::error::Error>> {
    conn.execute(
        "INSERT INTO memories (namespace, name, type, description, body, body_hash)
         VALUES ('global', 'test-mem', 'user', 'desc', 'body', 'hash1')",
        [],
    )?;
    Ok(conn.last_insert_rowid())
}
