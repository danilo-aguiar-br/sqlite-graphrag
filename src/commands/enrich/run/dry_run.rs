//! `--dry-run` reporting: preview every candidate, then close with a summary.
//!
//! Never touches the LLM or the sidecar queue, so an offline agent can inspect
//! what a real run would process without credentials.

use super::super::args::EnrichArgs;
use super::super::events::{EnrichSummary, ItemEvent};
use super::super::postprocess::take_enrich_backend;
use super::super::scan::resolve_name_filter;
use crate::output::emit_json_line as emit_json;
use std::time::Instant;

/// Emits preview events and the summary without calling the LLM.
pub(super) fn emit_preview(args: &EnrichArgs, keys: &[String], started: Instant) {
    let total = keys.len();
    // GAP-CLI-NAMES-03 / G-T-ONESHOT-01: explicit empty-match when a name
    // filter was provided but no candidates matched.
    if total == 0 {
        let name_filter = resolve_name_filter(args).unwrap_or_default();
        if !name_filter.is_empty() {
            emit_json(&serde_json::json!({
                "matched": 0,
                "hint": "no candidates matched --names/--entity-names/--memory-names for this operation; verify name space (entity vs memory) and predicates (e.g. empty description unless --force-redescribe)",
                "operation": format!("{:?}", args.operation()),
                "names_requested": name_filter,
            }));
        }
    }
    for (idx, key) in keys.iter().enumerate() {
        emit_json(&ItemEvent {
            item: key,
            status: "preview",
            memory_id: None,
            entity_id: None,
            entities: None,
            rels: None,
            chars_before: None,
            chars_after: None,
            cost_usd: None,
            elapsed_ms: None,
            error: None,
            index: idx,
            total,
        });
    }
    emit_json(&EnrichSummary {
        summary: true,
        operation: format!("{:?}", args.operation()),
        items_total: total,
        completed: 0,
        failed: 0,
        skipped: 0,
        cost_usd: 0.0,
        elapsed_ms: started.elapsed().as_millis() as u64,
        backend_invoked: take_enrich_backend(),
        waiting: 0,
        dead: 0,
        budget_exhausted: None,
        pairs_remaining_estimate: None,
        yields: None,
        preempted_for_gate: None,
    });
}
