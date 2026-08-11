//! Localized validation messages for memory fields and CLI guards.

// GAP-SG-146: catalogs are split by MESSAGE DOMAIN. The former
// `messages_a/b/c` suffixes recorded the order the file had been sliced in,
// not what any of them held.
mod messages_agent_surface;
mod messages_cli;
mod messages_config;
mod messages_embedding;
mod messages_graph;
mod messages_naming;
mod messages_not_found;
mod messages_openrouter;
mod messages_payload;
mod messages_queue;

/// Portuguese translations for `AppError` Display messages.
pub mod app_error_pt;
/// Portuguese translations for runtime startup progress messages.
pub mod runtime_pt;

pub use messages_agent_surface::*;
pub use messages_cli::*;
pub use messages_config::*;
pub use messages_embedding::*;
pub use messages_graph::*;
pub use messages_naming::*;
pub use messages_not_found::*;
pub use messages_openrouter::*;
pub use messages_payload::*;
pub use messages_queue::*;
