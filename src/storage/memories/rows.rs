//! Typed row shapes exchanged with the `memories` table.

use serde::{Deserialize, Serialize};

/// Input payload for inserting or updating a memory.
///
/// `body_hash` must be the BLAKE3 digest of `body`. The `metadata` field is
/// stored as a TEXT column containing JSON.
#[derive(Debug, Serialize, Deserialize)]
pub struct NewMemory {
    /// Namespace scope.
    pub namespace: String,
    /// Name of this item.
    pub name: String,
    /// Memory type classification.
    pub memory_type: String,
    /// Human-readable description.
    pub description: String,
    /// Full text body.
    pub body: String,
    /// Body hash.
    pub body_hash: String,
    /// Session ID.
    pub session_id: Option<String>,
    /// Source side of the relationship.
    pub source: String,
    /// Arbitrary metadata.
    pub metadata: serde_json::Value,
}

/// Fully materialized row from the `memories` table.
///
/// Returned by `read_by_name`, `read_full`, `list` and `fts_search`.
/// The `metadata` field is kept as a JSON string to avoid double parsing.
#[derive(Debug, Serialize)]
pub struct MemoryRow {
    /// Unique identifier.
    pub id: i64,
    /// Namespace scope.
    pub namespace: String,
    /// Name of this item.
    pub name: String,
    /// Memory type classification.
    pub memory_type: String,
    /// Human-readable description.
    pub description: String,
    /// Full text body.
    pub body: String,
    /// Body hash.
    pub body_hash: String,
    /// Session ID.
    pub session_id: Option<String>,
    /// Source side of the relationship.
    pub source: String,
    /// Arbitrary metadata.
    pub metadata: String,
    /// Creation timestamp.
    pub created_at: i64,
    /// Last-update timestamp.
    pub updated_at: i64,
    /// Unix epoch when the memory was soft-deleted, or `None` for active memories.
    /// Surfaced in `list --include-deleted --json` so LLM consumers can distinguish
    /// active from soft-deleted rows without a second SQL query (v1.0.37 H7+M9 fix).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
}
