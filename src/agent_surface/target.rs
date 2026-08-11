//! GAP-SG-205: the resolved database target, reported on every envelope that
//! resolved one.
//!
//! # Why this is not just another knob record
//!
//! The Explicit Target Designation rule asks for one thing: a verb with a side
//! effect must name its target in the argv, and the target it actually resolved
//! must appear in the output. The second half is what makes the first half
//! auditable — without it, a write that landed in the wrong database leaves no
//! trace to find.
//!
//! v1.2.6 emitted the two members from `super::base_meta`, which sounded
//! right and was not: `base_meta` runs downstream of TWO short-circuits —
//! `crate::output::envelope::emit_json` skips the whole layer when
//! [`super::active`] is false, and [`super::apply`] returns early when
//! [`super::AgentSurface::is_noop`] is true. So the target appeared only for a
//! caller that had already set some unrelated flag, and vanished on the default
//! path every agent actually uses:
//!
//! ```text
//! remember --db T                  → no agent_surface block at all
//! remember --db T --max-items 50   → db_path_source: "argv"
//! ```
//!
//! Hanging a universal contract off an optional block is the same proxy mistake
//! this release already paid for twice in [`super::gate`]. The cure is not to
//! move the field somewhere else; it is to stop conditioning the block on
//! "some knob is set" and condition it on "there is something to report". A
//! resolved target is something to report.
//!
//! # Why the members live inside `agent_surface`
//!
//! Measured, not assumed: 66 of the 74 published schemas close their root with
//! `additionalProperties: false`, and all 66 already declare `agent_surface`.
//! A new root member would therefore break 66 contracts, while the existing
//! block absorbs the record at zero schema cost.
//!
//! # When nothing is reported
//!
//! Absent means the process never resolved a target, which is the honest answer
//! for `config`, `completions` and `locale` — they touch no database at all. It
//! never means "resolved but omitted"; that distinction is the whole point.

use super::AgentSurface;
use crate::paths::{AppPaths, TargetSource};
use serde_json::{json, Map, Value};

/// Member naming which configuration layer supplied the target.
pub const SOURCE_KEY: &str = "db_path_source";

/// Member carrying the absolute path this process resolved.
pub const RESOLVED_KEY: &str = "db_path_resolved";

/// Member recording that an ambient target was accepted on purpose.
///
/// Present only when the caller passed the dispensation flag, so its absence
/// beside a non-`argv` source is itself the signal that nothing explicit
/// authorised the inheritance.
pub const DISPENSATION_KEY: &str = "db_path_dispensation";

/// Wire spelling of the dispensation, matching the flag that grants it.
pub const DISPENSATION_VALUE: &str = "use-active";

/// Writes the target record into `meta`, when this process resolved a target.
///
/// Idempotent and total: calling it on a record that already carries the
/// members overwrites them with the same values, so both the shaping path and
/// the inert path can call it without coordinating.
pub fn insert_into(meta: &mut Map<String, Value>, surface: &AgentSurface) {
    let Some(source) = AppPaths::target_source() else {
        return;
    };
    meta.insert(SOURCE_KEY.into(), json!(source.as_str()));
    if let Some(path) = AppPaths::resolved_target() {
        meta.insert(RESOLVED_KEY.into(), json!(path.to_string_lossy().as_ref()));
    }
    // Recorded only where it changed the outcome. On an `argv` target the
    // dispensation was never consulted, so reporting it would suggest the
    // caller leaned on an escape hatch it did not need.
    if surface.use_active && source != TargetSource::Argv {
        meta.insert(DISPENSATION_KEY.into(), json!(DISPENSATION_VALUE));
    }
}

/// `true` when this process has a target worth reporting.
///
/// `crate::output::envelope` asks before deciding whether the layer has to run
/// at all, so an envelope from a host-only subcommand keeps the zero-cost path.
#[must_use]
pub fn is_reportable() -> bool {
    // Either fact is enough. Today every command that declares a ceiling also
    // resolves a database, so the second test is redundant in practice — and
    // relying on that coincidence is precisely how the target came to depend on
    // an unrelated flag. Asking about both keeps the two independent.
    AppPaths::target_source().is_some() || super::universe::get().is_some()
}

/// Builds the record on its own, for envelopes that carry no shaping record.
///
/// Returns `None` when there is no target, which lets the caller skip the
/// insertion entirely rather than attach an empty block.
#[must_use]
pub fn record(surface: &AgentSurface) -> Option<Map<String, Value>> {
    let mut meta = Map::new();
    insert_into(&mut meta, surface);
    // The query ceiling rides along, for the same reason the target does: it is
    // a fact about the PROCESS rather than about the reshaping. Without it
    // `deep-research "x"` with no knob reported which database it opened and
    // stayed silent about having cut the ranking to five, which is half a
    // contract and the harder half to notice is missing.
    super::insert_query_ceiling(&mut meta);
    (!meta.is_empty()).then_some(meta)
}
