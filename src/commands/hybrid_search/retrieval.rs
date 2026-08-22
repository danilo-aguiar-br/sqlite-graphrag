//! The two candidate sources feeding the fusion: the live query embedding plus
//! vector KNN, and the FTS5 BM25 lookup with its corruption auto-rebuild.
//!
//! Each entry point returns the degradation flags it observed so the caller can
//! assemble the response envelope without re-deriving them.

use super::HybridSearchArgs;
use crate::errors::AppError;
use crate::storage::memories;
use rusqlite::Connection;

/// Live query embedding plus the flags describing whether it succeeded.
///
/// Re-exported from [`crate::query_embedding`], which owns the resolution
/// `recall` and `hybrid-search` share.
pub(super) use crate::query_embedding::QueryEmbedding;

/// FTS5 candidates plus the flags describing how they were obtained.
///
/// The tuple is `(rows, fts_degraded, fts_error, fts_auto_rebuilt)`.
pub(super) type FtsCandidates = (Vec<memories::MemoryRow>, bool, Option<String>, bool);

/// G58 (v1.0.80): when the live embedding fails, skip the KNN half of the RRF
/// and serve FTS5-only results. The RRF degenerates to a pure BM25 ranking and
/// the envelope surfaces `vec_degraded` + `vec_error` + `warning`.
///
/// Thin adapter over [`crate::query_embedding::resolve_query_embedding`]: the
/// logic is shared with `recall` so a degradation means the same thing in both
/// envelopes, and only the argument shape is local to this command.
pub(super) fn resolve_query_embedding(
    args: &HybridSearchArgs,
    models_dir: &std::path::Path,
    backends: crate::cli::BackendChoice,
) -> QueryEmbedding {
    crate::query_embedding::resolve_query_embedding(
        args.fallback_fts_only,
        models_dir,
        &args.query,
        backends,
        "hybrid_search",
    )
}

/// Vector KNN candidates, or an empty list when no embedding is available.
pub(super) fn vector_candidates(
    conn: &Connection,
    embedding: Option<&Vec<f32>>,
    namespaces: &[String],
    memory_type: Option<&str>,
    k: usize,
) -> Result<Vec<(i64, f32)>, AppError> {
    match embedding {
        Some(emb) => memories::knn_search(conn, emb, namespaces, memory_type, k * 2),
        None => Ok(Vec::new()),
    }
}

/// FTS5 candidates, transparently rebuilding a corrupted index once before
/// giving up and degrading to vec-only.
pub(super) fn fts_candidates(
    conn: &Connection,
    args: &HybridSearchArgs,
    namespace: &str,
    memory_type: Option<&str>,
) -> FtsCandidates {
    if args.weight_fts == 0.0 {
        return (vec![], false, None, false);
    }
    match memories::fts_search(conn, &args.query, namespace, memory_type, args.k * 2) {
        Ok(r) => (r, false, None, false),
        Err(e) => {
            let err_msg = e.to_string();
            let is_malformed = err_msg.contains("malformed") || err_msg.contains("corrupt");
            if is_malformed {
                tracing::warn!(target: "hybrid_search", "FTS5 index corrupted, attempting auto-rebuild");
                if conn
                    .execute_batch("INSERT INTO fts_memories(fts_memories) VALUES('rebuild');")
                    .is_ok()
                {
                    match memories::fts_search(
                        conn,
                        &args.query,
                        namespace,
                        memory_type,
                        args.k * 2,
                    ) {
                        Ok(r) => (r, false, None, true),
                        Err(e2) => {
                            tracing::error!(target: "hybrid_search", error = %e2, "FTS5 auto-rebuild failed to recover");
                            (vec![], true, Some(e2.to_string()), true)
                        }
                    }
                } else {
                    (vec![], true, Some(err_msg), false)
                }
            } else {
                tracing::warn!(target: "hybrid_search", error = %e, "FTS5 query failed, falling back to vec-only");
                (vec![], true, Some(err_msg), false)
            }
        }
    }
}
