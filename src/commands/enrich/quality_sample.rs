//! Quality sample and backlog counts (Wave C1).

#![allow(clippy::empty_line_after_doc_comments)]

use super::args::{EnrichOperation, ReEmbedTarget};
use super::predicates::{entity_description_scan_predicate, *};
use super::scan::sql::{limit_clause, limit_param};
use crate::errors::AppError;
use rusqlite::Connection;

/// Samples entities whose type label a model should re-examine.
///
/// `type_filter` narrows the sample to one stored label. v1.2.8 added it for a
/// concrete reason: this is the operation that can propose a better label for
/// the entities the closed vocabulary collapsed into `concept` — 10 902 of
/// 15 744 on this workspace's database — and until now it sampled the whole
/// namespace at random, so reaching them meant paying for every entity in the
/// graph. The label is BOUND, never pasted into the SQL text (GAP-SG-167), and
/// matched literally, because the vocabulary is open and the caller may be
/// hunting for a label no enum knows.
pub(super) fn scan_entities_for_type_validation(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
    type_filter: Option<&str>,
) -> Result<Vec<(i64, String, String)>, AppError> {
    // GAP-SG-167: the cap is BOUND, never pasted into the SQL text.
    let limit_sql = limit_clause(3);
    let sql = format!(
        "SELECT id, name, type FROM entities WHERE namespace = ?1 \
         AND (?2 IS NULL OR type = ?2) \
         ORDER BY (id * 2654435761) % 2147483647 {limit_sql}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            rusqlite::params![namespace, type_filter, limit_param(limit)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Scan for memories with generic descriptions (ingested, imported, etc).
pub(super) fn scan_generic_descriptions(
    conn: &Connection,
    namespace: &str,
    limit: Option<usize>,
) -> Result<Vec<(i64, String, String)>, AppError> {
    // GAP-SG-167: the cap is BOUND, never pasted into the SQL text.
    let limit_sql = limit_clause(2);
    let generic_pred = generic_description_predicate();
    let sql = format!(
        "SELECT id, name, description FROM memories WHERE namespace = ?1 AND deleted_at IS NULL \
         AND {generic_pred} \
         ORDER BY (id * 2654435761) % 2147483647 {limit_sql}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![namespace, limit_param(limit)], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Default sample size for entity-description grounding quality (Wave 2).
///
/// DEBT (v1.2.8): this belongs in `src/constants/`, beside the other numbers
/// that shape enrich behaviour. It is a tuning knob an operator overrides with
/// `--quality-sample` or `enrich.entity_description.quality_sample`, so its
/// compiled default is policy, and policy kept in a private module constant is
/// invisible to anyone reading the constants module to learn what the defaults
/// are. Not moved here because `src/constants/` was outside the scope this
/// change was allowed to touch.
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
    /// G-PR-7: entities carrying a description while having NO linked corpus
    /// worth grounding against. Reported separately because they used to be
    /// counted as perfect quality, which is what kept the defect invisible.
    pub without_corpus: u32,
    /// G-PR-7: shape of the grounding distribution, `None` on an empty sample.
    pub percentiles: Option<GroundingPercentiles>,
}

/// Quantiles of the grounding coverage distribution over the sample.
///
/// The mean cannot answer the only question that matters when choosing
/// [`super::DEFAULT_ENRICH_GROUNDING_THRESHOLD`]: how much of the corpus sits
/// just BELOW the line. Two corpora with the same mean — one tightly clustered,
/// one bimodal — demand opposite thresholds, and averaging erases the
/// difference. A p25 of 0.28 against a threshold of 0.30 says plainly that the
/// line rejects a quarter of the corpus; no single scalar says that.
///
/// Every score the sampler computed used to be discarded here, which is why the
/// threshold was set by intuition rather than by measurement.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(super) struct GroundingPercentiles {
    pub p10: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p90: f64,
}

/// Nearest-rank percentile over an ALREADY SORTED slice.
///
/// Nearest-rank rather than interpolated: every value returned is a score some
/// entity actually scored, so an operator can look it up. Interpolation would
/// invent a number that no description produced.
fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let rank = (q * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
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
    // Already resolved by `EnrichArgs::entity_description_grounding_threshold`
    // (flag > XDG > compiled default). The sampler MUST use the same number the
    // write path uses, or the metric measures a threshold nothing enforces.
    let threshold = grounding_threshold;

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

    let mut without_corpus: u32 = 0;
    let mut sampled: u32 = 0;
    let mut grounded: u32 = 0;
    // Capacity is the number of rows the query CAN return, not the number the
    // caller asked for. `total_with_description` is already known here, and the
    // `LIMIT ?2` above means a request for 4096 over a namespace holding 12
    // descriptions yields 12 — reserving 4096 would waste the difference on
    // every status call. Sizing from the request alone is also how an
    // unvalidated `--quality-sample` reached the allocator (see
    // `parsers::parse_quality_sample_range`); this clamp holds even if that
    // ceiling is ever widened.
    let capacity = sample_n.min(usize::try_from(total_with_description).unwrap_or(0));
    let mut scores: Vec<f64> = Vec::with_capacity(capacity);

    let min_corpus_chars = crate::runtime_config::resolve_usize(
        None,
        "enrich.entity_description.min_corpus_chars",
        crate::preservation::DEFAULT_GROUNDING_MIN_CORPUS_CHARS,
    );

    for row in rows {
        let (entity_id, description) = row.map_err(AppError::Database)?;
        // Same loader as the write path, on purpose: the sampler must not
        // measure a corpus the writer never saw.
        let corpus = super::extraction::load_entity_evidence(conn, entity_id)?;
        sampled = sampled.saturating_add(1);

        // G-PR-7: an entity with a description but no corpus is the WORST
        // case, not the best one. Asking `evaluate_grounding` here returned
        // `Preserved { score: 1.0 }` for exactly that population, so it
        // counted once in `accept_rate` and again as a perfect 1.0 in
        // `mean_score`. The sampler was computing quality with the same
        // inverted gate that produced the bad descriptions, which is why
        // `quality_pct` never moved and `low_grounding_est` understated the
        // backlog. Score it as zero and count it apart.
        if !crate::preservation::corpus_is_sufficient(&corpus, min_corpus_chars) {
            without_corpus = without_corpus.saturating_add(1);
            scores.push(0.0);
            continue;
        }

        let verdict = crate::preservation::PreservationVerdict::evaluate_grounding(
            &description,
            &corpus,
            threshold,
        );
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

    // Sorted IN PLACE — the sample is already the only copy of these numbers,
    // and cloning it to keep insertion order would buy nothing: nothing
    // downstream cares which entity produced which score, only the shape.
    // `total_cmp` rather than `partial_cmp().unwrap()`: a NaN would panic the
    // status command, and a status command must never be the thing that breaks.
    scores.sort_unstable_by(f64::total_cmp);
    let percentiles = (!scores.is_empty()).then(|| GroundingPercentiles {
        p10: percentile(&scores, 0.10),
        p25: percentile(&scores, 0.25),
        p50: percentile(&scores, 0.50),
        p75: percentile(&scores, 0.75),
        p90: percentile(&scores, 0.90),
    });

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
        // The two halves are NAMED (v1.2.8): as anonymous `0.5` literals nothing
        // stated that they are meant to be equal, and nothing would have caught
        // a pair that stopped summing to one — at which point the figure is no
        // longer a weighted mean and `low_grounding_est`, derived from
        // `1.0 - quality_pct`, silently stops estimating a count.
        let accept_rate = f64::from(grounded) / f64::from(sampled);
        (crate::constants::ENRICH_QUALITY_ACCEPT_RATE_WEIGHT * accept_rate
            + crate::constants::ENRICH_QUALITY_MEAN_SCORE_WEIGHT * mean_score)
            .clamp(0.0, 1.0)
    };
    let low_frac = (1.0 - quality_pct).clamp(0.0, 1.0);
    let low_grounding_est = (low_frac * total_with_description as f64).round() as i64;

    Ok(DescriptionQualitySample {
        percentiles,
        sampled,
        grounded,
        quality_pct,
        low_grounding_est,
        total_with_description,
        without_corpus,
    })
}

/// Unfiltered backlog: counts what a scan with no `--entity-type` would select.
///
/// DEBT (v1.2.8): passes `None` for the type filter, so for
/// `entity-type-validate` this still reports the whole namespace even when the
/// caller narrowed the scan. Fixing it here means changing `run::scan_phase`,
/// which was outside the scope this change was allowed to touch; the path that
/// reports the number to an operator — `enrich --status` — goes through
/// [`count_operation_backlog_with_force`] and is fixed.
pub(super) fn count_operation_backlog(
    conn: &Connection,
    operation: &EnrichOperation,
    namespace: &str,
    reembed_target: ReEmbedTarget,
) -> Result<i64, AppError> {
    count_operation_backlog_with_force(conn, operation, namespace, reembed_target, false, None)
}

/// GAP-CLI-ED-STATUS-01 (v1.1.8): same as [`count_operation_backlog`] but
/// honours `--force-redescribe` for entity-descriptions so status and scan
/// share one predicate (GAP-SG-77).
///
/// `type_filter` (v1.2.8) carries `--entity-type` through to the
/// `entity-type-validate` count. Without it the count answered a different
/// question than the scan beside it: the scan applies the label filter
/// (`scan_entities_for_type_validation`), the count did not, so
/// `--entity-type concept` reported the size of the whole namespace as the
/// backlog of a run that would only ever touch one label — 15 744 instead of
/// 10 902 on this workspace's database. The label is BOUND, never pasted into
/// the SQL text (GAP-SG-167), and matched literally, exactly as the scan
/// matches it.
pub(super) fn count_operation_backlog_with_force(
    conn: &Connection,
    operation: &EnrichOperation,
    namespace: &str,
    reembed_target: ReEmbedTarget,
    force_redescribe: bool,
    type_filter: Option<&str>,
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
                let pred = entity_description_scan_predicate(true, false);
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
                let pred = entity_description_scan_predicate(false, false);
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
            let high_weight_pred = high_weight_predicate();
            let sql = format!(
                "SELECT COUNT(*) FROM relationships r \
                 JOIN entities e1 ON e1.id = r.source_id \
                 WHERE {high_weight_pred} AND e1.namespace = ?1"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::RelationReclassify => {
            let generic_pred = generic_relation_predicate();
            let sql = format!(
                "SELECT COUNT(*) FROM relationships r \
                 JOIN entities e1 ON e1.id = r.source_id \
                 WHERE {generic_pred} AND e1.namespace = ?1"
            );
            conn.query_row(&sql, rusqlite::params![namespace], |r| r.get::<_, i64>(0))?
        }
        EnrichOperation::EntityTypeValidate => {
            // Mirrors scan_entities_for_type_validation, INCLUDING its optional
            // label filter — the same `(?2 IS NULL OR type = ?2)` shape, so the
            // two cannot answer different questions about the same run.
            conn.query_row(
                "SELECT COUNT(*) FROM entities WHERE namespace = ?1 \
                 AND (?2 IS NULL OR type = ?2)",
                rusqlite::params![namespace, type_filter],
                |r| r.get::<_, i64>(0),
            )?
        }
        EnrichOperation::DescriptionEnrich => {
            let generic_pred = generic_description_predicate();
            let sql = format!(
                "SELECT COUNT(*) FROM memories \
                 WHERE namespace = ?1 AND deleted_at IS NULL \
                 AND {generic_pred}"
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

#[cfg(test)]
mod tests {
    /// The blend behind `quality_pct` is a weighted mean, and a weighted mean
    /// whose weights do not sum to one is not a mean at all: the result leaves
    /// the 0..1 range the `clamp` pretends to guarantee, and
    /// `low_grounding_est`, computed as `1.0 - quality_pct` times the corpus
    /// size, stops estimating a count of entities. Nothing else in the codebase
    /// would notice — the figure would simply be wrong on every status call.
    #[test]
    fn quality_blend_weights_sum_to_one() {
        let sum = crate::constants::ENRICH_QUALITY_ACCEPT_RATE_WEIGHT
            + crate::constants::ENRICH_QUALITY_MEAN_SCORE_WEIGHT;
        assert!(
            (sum - 1.0).abs() < f64::EPSILON,
            "quality blend weights must sum to 1.0, got {sum}"
        );
    }
}
