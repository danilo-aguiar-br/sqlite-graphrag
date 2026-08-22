//! Handler for the `remember` CLI subcommand.

mod args;
mod embed_phase;
mod finish;
mod graph_input;
mod input;
mod name;
mod run;

pub use args::RememberArgs;
pub use run::run;

/// GAP-SG-216: shared with `remember-batch`, which accepts the same `NewEntity`
/// payload and owes its caller the same report.
///
/// Re-exported rather than duplicated so ONE function decides what counts as a
/// non-canonical label. Two copies would drift on the very question — "is this
/// label part of the recommended vocabulary?" — that both the warning and
/// `--strict-entity-types` answer.
pub(crate) use graph_input::collect_noncanonical_entity_types;

#[cfg(test)]
mod tests;
