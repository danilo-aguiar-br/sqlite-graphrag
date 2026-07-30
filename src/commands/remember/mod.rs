//! Handler for the `remember` CLI subcommand.

mod args;
mod embed_phase;
mod finish;
mod graph_input;
mod run;

pub use args::RememberArgs;
pub use run::run;

#[cfg(test)]
mod tests;
