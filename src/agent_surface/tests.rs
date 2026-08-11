//! Unit coverage for the agent-native reshaping surface (GAP-SG-142).
//!
//! Split by responsibility in v1.2.6. The file had reached 799 of the 800
//! physical lines `tests/file_size_ceiling_gate.rs` allows, so the next test
//! written here would have turned that gate red — and this change adds several.
//!
//! The fixtures stay in this root because every child shares them; each child
//! owns one knob or one invariant, so a reader looking for `--filter` behaviour
//! opens `filters.rs` instead of scrolling past the budget arithmetic.

mod aliases;
mod budget;
mod filters;
mod gate;
mod ordering;
mod projection;
mod surface_state;
mod target;
mod truncation;
mod vocabulary;

use super::filter::{FilterExpr, FilterOp};
use super::*;
use serde_json::json;

fn envelope() -> Value {
    json!({
        "query": "rust",
        "elapsed_ms": 12,
        "results": [
            { "name": "alpha", "score": 0.9, "type": "note", "snippet": "abcdefghij" },
            { "name": "beta",  "score": 0.5, "type": "note", "snippet": "klmnopqrst" },
            { "name": "gamma", "score": 0.7, "type": "decision", "snippet": "uvwxyz" },
            { "name": "beta",  "score": 0.1, "type": "note", "snippet": "duplicate" }
        ]
    })
}

fn surface() -> AgentSurface {
    AgentSurface::default()
}

/// Shapes and unwraps.
///
/// `super::apply` became fallible in v1.2.6, but the overwhelming majority of
/// these tests assert on a shaped envelope, not on a refusal. Shadowing the name
/// here keeps those assertions reading as they did while making the unwrap a
/// single, named decision instead of forty-seven scattered ones. Tests that
/// assert a refusal call [`try_apply`].
/// States NO resolved target, which is what keeps these assertions deterministic.
///
/// `paths` and `agent_surface` unit tests share one test binary, and the target
/// lives in a process-wide `OnceLock`. Reading it here would make "is the
/// envelope byte-identical when the surface is inert" depend on whether a
/// `paths` test happened to resolve a database first. Tests whose subject IS the
/// target pass their own record.
fn apply(surface: &AgentSurface, value: Value) -> Value {
    super::apply_with_target(surface, value, None)
        .expect("this surface must not refuse this envelope")
}

/// The fallible form, for the tests whose subject IS the refusal.
fn try_apply(surface: &AgentSurface, value: Value) -> Result<Value, crate::errors::AppError> {
    super::apply_with_target(surface, value, None)
}

/// A surface that knows which subcommand emitted the envelope.
///
/// Alias suppression needs it: a member is derived only for the subcommand that
/// declared it so, and the same `results` name means different things in
/// `recall`, `related` and `hybrid-search`.
fn surface_for(command: &str) -> AgentSurface {
    AgentSurface {
        command: Some(command.to_string()),
        ..AgentSurface::default()
    }
}

fn results(value: &Value) -> &Vec<Value> {
    value
        .get("results")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("envelope must keep its results array: {value}"))
}

/// The `list` envelope: `memories` is a clone of `items` (v1.0.66 alias).
fn list_envelope() -> Value {
    let items = json!([
        { "name": "alpha", "memory_type": "skill" },
        { "name": "beta",  "memory_type": "note" },
        { "name": "gamma", "memory_type": "skill" }
    ]);
    json!({ "total_count": 3, "items": items, "memories": items, "elapsed_ms": 1 })
}

impl AgentSurface {
    fn clone_with_sort(&self, key: &str) -> Self {
        let mut next = self.clone();
        next.sort = Some(key.to_string());
        next
    }
}
