//! Localized validation messages for memory fields and CLI guards.

mod messages_a;
mod messages_b;
mod messages_c;
mod messages_embedding;

/// Portuguese translations for `AppError` Display messages.
pub mod app_error_pt;
/// Portuguese translations for runtime startup progress messages.
pub mod runtime_pt;

pub use messages_a::*;
pub use messages_b::*;
pub use messages_c::*;
pub use messages_embedding::*;
