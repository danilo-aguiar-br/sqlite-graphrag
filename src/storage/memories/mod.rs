//! Persistence layer for the `memories` table and its vector companion.
//!
//! Functions here encapsulate every SQL statement touching `memories`,
//! `memory_embeddings` and the FTS5 `fts_memories` shadow table. Callers receive
//! typed [`MemoryRow`] or [`NewMemory`] values and never build SQL strings.
//!
//! One submodule per storage surface: [`rows`] holds the typed shapes, [`crud`]
//! the single-row lookups and mutations, [`listing`] the paginated reads,
//! [`vectors`] the `memory_embeddings` companion and its KNN query, [`fts`] the
//! FTS5 shadow table, and [`soft_delete`] the tombstone lifecycle.

mod crud;
mod fts;
mod listing;
mod rows;
mod soft_delete;
mod vectors;

pub use crud::{find_by_hash, find_by_name, insert, read_by_name, read_full, update};
pub use fts::{fts_search, sync_fts_after_update};
pub use listing::{count, list};
pub use rows::{MemoryRow, NewMemory};
pub use soft_delete::{clear_deleted_at, find_by_name_any_state, list_deleted_before, soft_delete};
pub use vectors::{delete_vec, knn_search, upsert_vec};

#[cfg(test)]
mod tests;
