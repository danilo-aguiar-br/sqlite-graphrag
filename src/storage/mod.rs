//! SQLite persistence layer: sub-modules for each domain table group.

pub mod backend;
pub mod chunks;
pub mod connection;
pub mod entities;
pub mod foreign_keys;
pub mod fusion;
pub mod memories;
pub mod pending_embeddings;
pub mod urls;
pub mod utils;
pub mod versions;
