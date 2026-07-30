//! Embedding-table health, coverage percentages, and LLM slot status.

use super::tables::{
    count_rows, first_existing_table, CHUNK_EMBEDDING_TABLES, ENTITY_EMBEDDING_TABLES,
    MEMORY_EMBEDDING_TABLES,
};

/// Returns `(max, active, stale)` LLM embedding slot counts for this host.
pub(super) fn llm_slot_info() -> (u32, u32, u32) {
    let max = crate::llm_slots::default_max_concurrency();
    let status = crate::llm_slots::read_status(max);
    let stale = crate::llm_slots::find_stale_slots(max);
    (status.max, status.active, stale.len() as u32)
}

/// Memory embedding completeness: `(table_ok, total, missing, orphaned)`.
pub(super) fn memory_embedding_health(conn: &rusqlite::Connection) -> (bool, i64, i64, i64) {
    let Some(table_name) = first_existing_table(conn, MEMORY_EMBEDDING_TABLES) else {
        return (false, 0, 0, 0);
    };

    let total = count_rows(conn, table_name);
    let missing = conn
        .query_row(
            &format!(
                "SELECT COUNT(*)
                 FROM memories m
                 LEFT JOIN {table_name} me ON me.memory_id = m.id
                 WHERE me.memory_id IS NULL AND m.deleted_at IS NULL"
            ),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let orphaned = conn
        .query_row(
            &format!(
                "SELECT COUNT(*)
                 FROM {table_name} me
                 LEFT JOIN memories m ON m.id = me.memory_id
                 WHERE m.id IS NULL OR m.deleted_at IS NOT NULL"
            ),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    (true, total, missing, orphaned)
}

/// v1.1.1 (P6a): completeness check for entity vectors, mirroring the
/// `vec_memories_missing` pattern. Returns `(table_ok, missing)` where
/// `missing` counts entities without a row in the embedding table —
/// COVERAGE, distinct from the mere table-existence reported by
/// `vec_entities_ok`.
pub(super) fn entity_embedding_health(conn: &rusqlite::Connection) -> (bool, i64) {
    let Some(table_name) = first_existing_table(conn, ENTITY_EMBEDDING_TABLES) else {
        return (false, 0);
    };
    let missing = conn
        .query_row(
            &format!(
                "SELECT COUNT(*)
                 FROM entities e
                 LEFT JOIN {table_name} ee ON ee.entity_id = e.id
                 WHERE ee.entity_id IS NULL"
            ),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    (true, missing)
}

/// v1.1.1 (P6a): completeness check for chunk vectors. Returns
/// `(table_ok, missing)` where `missing` counts `memory_chunks` rows without
/// a row in the chunk embedding table.
pub(super) fn chunk_embedding_health(conn: &rusqlite::Connection) -> (bool, i64) {
    let Some(table_name) = first_existing_table(conn, CHUNK_EMBEDDING_TABLES) else {
        return (false, 0);
    };
    let missing = conn
        .query_row(
            &format!(
                "SELECT COUNT(*)
                 FROM memory_chunks c
                 LEFT JOIN {table_name} ce ON ce.chunk_id = c.id
                 WHERE ce.chunk_id IS NULL"
            ),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    (true, missing)
}

/// v1.1.1 (P6a): coverage percentage in [0.0, 100.0]. 100.0 when there is
/// nothing to cover (total 0); 0.0 when the vector table itself is absent
/// but source rows exist.
pub(super) fn coverage_pct(table_ok: bool, total: i64, missing: i64) -> f64 {
    if total <= 0 {
        return 100.0;
    }
    if !table_ok {
        return 0.0;
    }
    let covered = (total - missing).max(0) as f64;
    (covered / total as f64) * 100.0
}
