//! Scan functions — select candidates for each enrichment operation.
//!
//! One submodule per scan TARGET: [`memories`], [`entities`], [`chunks`] and
//! [`relationships`] each answer the eligibility question for their own table,
//! while [`name_filter`] resolves the `--names`/`--names-file` subset every
//! scanner applies. Entity-pair scanning lives in `super::scan_ec` and the
//! backlog counters in `super::quality_sample`; both are re-exported here so
//! `scan::` stays the single import surface for the rest of `enrich`.
//!
//! Every SQL predicate is read from `super::predicates`, never redeclared, so
//! a scan and the `count_operation_backlog` that reports its size cannot drift
//! (GAP-SG-77).

use super::args::{EnrichArgs, EnrichOperation, ReEmbedTarget};
use crate::errors::AppError;
use rusqlite::Connection;

mod chunks;
mod entities;
mod memories;
mod name_filter;
mod relationships;
pub(in crate::commands::enrich) mod sql;
mod stream;

pub(in crate::commands::enrich) use stream::scan_operation_for_each;

pub(super) use super::quality_sample::*;
pub(super) use super::scan_ec::*;

pub(super) use chunks::*;
pub(super) use entities::*;
pub(super) use memories::*;
pub(super) use name_filter::*;
pub(super) use relationships::*;

// GAP-SG-146: test modules named by the scanner family they cover.
#[cfg(test)]
#[path = "../scan_backlog_tests.rs"]
mod backlog_tests;
#[cfg(test)]
#[path = "../scan_candidate_tests.rs"]
mod candidate_tests;
#[cfg(test)]
#[path = "../scan_entity_pair_tests.rs"]
mod entity_pair_tests;
#[cfg(test)]
#[path = "../scan_reembed_target_tests.rs"]
mod reembed_target_tests;
#[cfg(test)]
#[path = "../scan_rss_tests.rs"]
mod rss_tests;
#[cfg(test)]
#[path = "../scan_test_fixtures.rs"]
mod test_fixtures;

// ---------------------------------------------------------------------------
// Scan dispatcher — maps operation to scan query result (item keys)
// ---------------------------------------------------------------------------

pub(super) fn scan_operation(
    conn: &Connection,
    namespace: &str,
    args: &EnrichArgs,
) -> Result<Vec<String>, AppError> {
    // G37: resolve --names + --names-file once and apply to every scan path.
    let name_filter = resolve_name_filter(args)?;
    // GAP-SG-185: keyset page width (flag > XDG > default).
    let page_size = args.scan_page_size();
    match args.operation() {
        EnrichOperation::MemoryBindings => {
            let rows = scan_unbound_memories(conn, namespace, args.limit, &name_filter, page_size)?;
            Ok(rows.into_iter().map(|(_, name)| name).collect())
        }
        // GAP-SG-24/26: additive augmentation processes ALREADY-bound memories,
        // restricted to an explicit name filter so it never re-scans the whole
        // namespace.
        EnrichOperation::AugmentBindings => {
            scan_bound_memories_for_augment(conn, namespace, args.limit, &name_filter)
        }
        EnrichOperation::EntityDescriptions => {
            let rows = scan_entities_without_description(
                conn,
                namespace,
                args.limit,
                &name_filter,
                args.force_redescribe,
            )?;
            Ok(rows.into_iter().map(|(_, name, _)| name).collect())
        }
        EnrichOperation::BodyEnrich => {
            let rows = scan_short_body_memories(
                conn,
                namespace,
                args.min_output_chars,
                args.limit,
                &name_filter,
                page_size,
            )?;
            Ok(rows.into_iter().map(|(_, name)| name).collect())
        }
        EnrichOperation::ReEmbed => {
            // v1.1.1 (P2): --target selects which embedding table to backfill.
            // Non-memory keys carry an `entity:` / `chunk:` prefix so the
            // drain dispatch (`call_reembed`) and the queue `item_type` can
            // tell them apart; bare memory names stay unprefixed for full
            // retro-compatibility with pre-v1.1.1 queue rows.
            let mut keys: Vec<String> = Vec::new();
            if matches!(args.target, ReEmbedTarget::Memories | ReEmbedTarget::All) {
                let rows = scan_memories_without_embeddings(
                    conn,
                    namespace,
                    args.limit,
                    &name_filter,
                    page_size,
                )?;
                keys.extend(rows.into_iter().map(|(_, name)| name));
            }
            if matches!(args.target, ReEmbedTarget::Entities | ReEmbedTarget::All) {
                let rows = scan_entities_missing_embeddings(
                    conn,
                    namespace,
                    args.limit,
                    &name_filter,
                    page_size,
                )?;
                keys.extend(rows.into_iter().map(|(_, name)| format!("entity:{name}")));
            }
            if matches!(args.target, ReEmbedTarget::Chunks | ReEmbedTarget::All) {
                let ids = scan_chunks_missing_embeddings(
                    conn,
                    namespace,
                    args.limit,
                    &name_filter,
                    page_size,
                )?;
                keys.extend(ids.into_iter().map(|id| format!("chunk:{id}")));
            }
            Ok(keys)
        }
        EnrichOperation::WeightCalibrate => {
            let rows = scan_weight_candidates(conn, namespace, args.limit)?;
            Ok(rows
                .into_iter()
                .map(|(id, _, _, _, _)| id.to_string())
                .collect())
        }
        EnrichOperation::RelationReclassify => {
            let rows = scan_generic_relations(conn, namespace, args.limit)?;
            Ok(rows
                .into_iter()
                .map(|(id, _, _, _)| id.to_string())
                .collect())
        }
        EnrichOperation::EntityConnect | EnrichOperation::CrossDomainBridges => {
            // v1.1.06: enqueue stable pair keys so drain resolves by ID without
            // re-running the pair scan (GAP-ENTITY-CONNECT-SCAN-CARTESIAN).
            let pairs = scan_isolated_entity_pairs(conn, namespace, args.limit)?;
            Ok(pairs
                .into_iter()
                .map(|(id1, _, id2, _)| format_pair_key(id1, id2))
                .collect())
        }
        EnrichOperation::EntityTypeValidate => {
            let rows = scan_entities_for_type_validation(conn, namespace, args.limit)?;
            Ok(rows.into_iter().map(|(_, name, _)| name).collect())
        }
        EnrichOperation::DescriptionEnrich => {
            let rows = scan_generic_descriptions(conn, namespace, args.limit)?;
            Ok(rows.into_iter().map(|(_, name, _)| name).collect())
        }
        EnrichOperation::DomainClassify
        | EnrichOperation::GraphAudit
        | EnrichOperation::DeepResearchSynth
        | EnrichOperation::BodyExtract => {
            scan_all_memory_names(conn, namespace, args.limit, &name_filter, page_size)
        }
    }
}
