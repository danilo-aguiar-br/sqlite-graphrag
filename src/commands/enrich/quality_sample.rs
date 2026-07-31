//! Quality sample and backlog counts (Wave C1).

#![allow(clippy::empty_line_after_doc_comments)]

use super::args::{EnrichOperation, ReEmbedTarget};
use super::predicates::{entity_description_scan_predicate, *};
use crate::errors::AppError;
use rusqlite::Connection;

pub(super) fn scan_entities_for_type_validation(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
) -> Result<Vec<(i64, String, String)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let sql = format!(
        "SELECT id, name, type FROM entities WHERE namespace = ?1 ORDER BY (id * 2654435761) % 2147483647 {limit_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![namespace], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Scan for memories with generic descriptions (ingested, imported, etc).
pub(super) fn scan_generic_descriptions(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
) -> Result<Vec<(i64, String, String)>, AppError> {
    let limit_clause = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let sql = format!(
        "SELECT id, name, description FROM memories WHERE namespace = ?1 AND deleted_at IS NULL \
         AND {GENERIC_DESCRIPTION_PREDICATE} \
         ORDER BY (id * 2654435761) % 2147483647 {limit_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![namespace], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// Default sample size for entity-description grounding quality (Wave 2).
pub(super) const DEFAULT_QUALITY_SAMPLE_N: usize = 50;

// ---------------------------------------------------------------------------
// Backlog counter (GAP-SG-77)
// ---------------------------------------------------------------------------

/// Count-only backlog for a single operation, using a cheap `SELECT COUNT(*)`.
///
/// This mirrors the dispatch of [`scan_operation`], reusing the SAME shared
/// WHERE predicates so the count can never drift from the rows a scan would
/// select. Unlike the scanners it materialises no rows.
///
/// The returned figure has DATABASE semantics — the real backlog of the
/// operation against the store — which is distinct from the FILE (sidecar
/// queue) semantics reported by `queue_pending`/`queue_dead`. It powers the
/// `scan_backlog` field of `enrich --status` so that db-backed operations
/// (`entity-descriptions`, `body-enrich`, `re-embed`, ...) no longer report a
/// false `pending=0` when thousands of eligible items exist.

/// Result of sampling entity-description grounding against linked memory corpus.
pub(super) struct DescriptionQualitySample {
    pub sampled: u32,
    #[allow(dead_code)]
    pub grounded: u32,
    pub quality_pct: f64,
    pub low_grounding_est: i64,
    #[allow(dead_code)]
    pub total_with_description: i64,
}

/// Sample entity descriptions and measure grounding coverage vs linked memories
/// (Wave 2 ED-LQ-SCAN / OBS-04).
pub(super) fn sample_entity_description_quality(
    conn: &Connection,
    namespace: &str,
    sample_n: usize,
    grounding_threshold: f64,
) -> Result<DescriptionQualitySample, AppError> {
    let sample_n = sample_n.max(1);
    let threshold = if grounding_threshold > 0.0 {
        grounding_threshold
    } else {
        super::extraction::ENTITY_DESCRIPTION_GROUNDING_DEFAULT
    };

    let total_with_description: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities \
         WHERE namespace = ?1 \
           AND description IS NOT NULL AND description != ''",
        rusqlite::params![namespace],
        |r| r.get(0),
    )?;

    // G-PR-6 / GAP-SG Phase 5: stable but non-sequential sample (ORDER BY id
    // biased toward oldest rows). Hash-modulo keeps the sample deterministic
    // across runs for the same namespace size.
    let mut stmt = conn.prepare(
        "SELECT id, description FROM entities \
         WHERE namespace = ?1 \
           AND description IS NOT NULL AND description != '' \
         ORDER BY (id * 2654435761) % 2147483647 \
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![namespace, sample_n as i64], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })?;

    let top_k = crate::runtime_config::resolve_usize(
        None,
        "enrich.entity_description.corpus_top_k",
        super::extraction::ENTITY_DESCRIPTION_CORPUS_TOP_K,
    );
    let max_chars = crate::runtime_config::resolve_usize(
        None,
        "enrich.entity_description.snippet_chars",
        super::extraction::ENTITY_DESCRIPTION_SNIPPET_CHARS,
    );

    let mut sampled: u32 = 0;
    let mut grounded: u32 = 0;
    let mut scores: Vec<f64> = Vec::with_capacity(sample_n);

    for row in rows {
        let (entity_id, description) = row.map_err(AppError::Database)?;
        let corpus =
            super::extraction::load_entity_corpus_snippets(conn, entity_id, top_k, max_chars)?;
        let verdict = crate::preservation::PreservationVerdict::evaluate_grounding(
            &description,
            &corpus,
            threshold,
        );
        sampled = sampled.saturating_add(1);
        if verdict.is_accepted() {
            grounded = grounded.saturating_add(1);
        }
        let score = match verdict {
            crate::preservation::PreservationVerdict::Preserved { score, .. } => score,
            crate::preservation::PreservationVerdict::Rejected { score, .. } => score,
            crate::preservation::PreservationVerdict::Unchanged { .. } => 1.0,
        };
        scores.push(score);
    }

    // Use scores for a mean-weighted quality signal when the binary accept
    // rate alone would hide near-threshold failures.
    let mean_score = if scores.is_empty() {
        1.0
    } else {
        scores.iter().sum::<f64>() / scores.len() as f64
    };
    let quality_pct = if sampled == 0 {
        1.0
    } else {
        // Blend accept-rate with mean Jaccard so operators see continuous signal.
        let accept_rate = f64::from(grounded) / f64::from(sampled);
        (0.5 * accept_rate + 0.5 * mean_score).clamp(0.0, 1.0)
    };
    let low_frac = (1.0 - quality_pct).clamp(0.0, 1.0);
    let low_grounding_est = (low_frac * total_with_description as f64).round() as i64;

    Ok(DescriptionQualitySample {
        sampled,
        grounded,
        quality_pct,
        low_grounding_est,
        total_with_description,
    })
}

pub(super) fn count_operation_backlog(
    conn: &Connection,
    operation: &EnrichOperation,
    namespace: &str,
    reembed_target: ReEmbedTarget,
) -> Result<i64, AppError> {
    count_operation_backlog_with_force(conn, operation, namespace, reembed_target, false)
}

/// GAP-CLI-ED-STATUS-01 (v1.1.8): same as [`count_operation_backlog`] but
/// honours `--force-redescribe` for entity-descriptions so status and scan
/// share one predicate (GAP-SG-77).
pub(super) fn count_operation_backlog_with_force(
    conn: &Connection,
    operation: &EnrichOperation,
    namespace: &str,
    reembed_target: ReEmbedTarget,
    force_redescribe: bool,
) -> Result<i64, AppError> {
    let count = match operation {
        EnrichOperation::MemoryBindings => {
            let sql = format!(
                "SELECT COUNT(*) FROM memories m \
                 WHERE m.namespace = ?1 AND m.deleted_at IS NULL \
                 AND {UNBOUND_MEMORY_PREDICATE}"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::EntityDescriptions => {
            // G-PR-2: with --force-redescribe, SQL is a high-precision prefilter;
            // re-apply `is_low_quality_description` so status COUNT matches scan.
            if force_redescribe {
                let pred = entity_description_scan_predicate(true);
                let sql = format!(
                    "SELECT COALESCE(description, '') FROM entities \
                     WHERE namespace = ?1 AND {pred}"
                );
                let mut stmt = conn.prepare(&sql)?;
                let descs =
                    stmt.query_map(rusqlite::params![namespace], |r| r.get::<_, String>(0))?;
                let mut n = 0i64;
                for d in descs {
                    let desc = d?;
                    if desc.trim().is_empty() || is_low_quality_description(&desc) {
                        n += 1;
                    }
                }
                n
            } else {
                let pred = entity_description_scan_predicate(false);
                let sql = format!(
                    "SELECT COUNT(*) FROM entities \
                     WHERE namespace = ?1 AND {pred}"
                );
                conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
            }
        }
        EnrichOperation::BodyEnrich => {
            let sql = format!(
                "SELECT COUNT(*) FROM memories m \
                 WHERE m.namespace = ?1 AND m.deleted_at IS NULL \
                 AND {SHORT_BODY_PREDICATE}"
            );
            let min_chars = super::DEFAULT_BODY_ENRICH_MIN_CHARS as i64;
            conn.query_row(&sql, rusqlite::params![namespace, min_chars], |r| {
                r.get::<_, i64>(0)
            })?
        }
        EnrichOperation::ReEmbed => {
            // v1.1.1 (P2/P10): same dim-aware predicates as the scanners,
            // summed over the targets selected by --target.
            let dim = crate::constants::embedding_dim();
            let mut total = 0i64;
            if matches!(reembed_target, ReEmbedTarget::Memories | ReEmbedTarget::All) {
                let sql = format!(
                    "SELECT COUNT(*) FROM memories m \
                     WHERE m.namespace = ?1 AND m.deleted_at IS NULL \
                     AND {}",
                    reembed_memory_predicate(dim)
                );
                total +=
                    conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?;
            }
            if matches!(reembed_target, ReEmbedTarget::Entities | ReEmbedTarget::All) {
                let sql = format!(
                    "SELECT COUNT(*) FROM entities e WHERE e.namespace = ?1 AND {}",
                    reembed_entity_predicate(dim)
                );
                total +=
                    conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?;
            }
            if matches!(reembed_target, ReEmbedTarget::Chunks | ReEmbedTarget::All) {
                let sql = format!(
                    "SELECT COUNT(*) FROM memory_chunks c \
                     LEFT JOIN memories m ON m.id = c.memory_id \
                     WHERE (m.namespace = ?1 OR m.id IS NULL) \
                     AND {}",
                    reembed_chunk_predicate(dim)
                );
                total +=
                    conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?;
            }
            total
        }
        EnrichOperation::WeightCalibrate => {
            let sql = format!(
                "SELECT COUNT(*) FROM relationships r \
                 JOIN entities e1 ON e1.id = r.source_id \
                 WHERE {HIGH_WEIGHT_PREDICATE} AND e1.namespace = ?1"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::RelationReclassify => {
            let sql = format!(
                "SELECT COUNT(*) FROM relationships r \
                 JOIN entities e1 ON e1.id = r.source_id \
                 WHERE {GENERIC_RELATION_PREDICATE} AND e1.namespace = ?1"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::EntityTypeValidate => {
            // Mirrors scan_entities_for_type_validation: every entity is a
            // candidate for the type audit.
            conn.query_row(
                "SELECT COUNT(*) FROM entities WHERE namespace = ?1",
                rusqlite::params![namespace],
                |r| r.get::<_, i64>(0),
            )?
        }
        EnrichOperation::DescriptionEnrich => {
            let sql = format!(
                "SELECT COUNT(*) FROM memories \
                 WHERE namespace = ?1 AND deleted_at IS NULL \
                 AND {GENERIC_DESCRIPTION_PREDICATE}"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::EntityConnect => {
            // Proxy O(n): entidades degree-0 com binding NER (ilhadas e conectaveis).
            // Do not count O(n^2) pairs — prohibitive for ~82k entities.
            let sql = "SELECT COUNT(*) FROM entities e \
                       WHERE e.namespace = ?1 AND e.degree = 0 \
                       AND EXISTS (SELECT 1 FROM memory_entities me WHERE me.entity_id = e.id)";
            conn.query_row(sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        // Advisory / quadratic scan-only operations have no closeable database
        // backlog; report 0 (see the doc comment above).
        EnrichOperation::AugmentBindings
        | EnrichOperation::CrossDomainBridges
        | EnrichOperation::DomainClassify
        | EnrichOperation::GraphAudit
        | EnrichOperation::DeepResearchSynth
        | EnrichOperation::BodyExtract => 0,
    };
    Ok(count)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
