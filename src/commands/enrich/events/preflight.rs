//! Preflight probe (G35) — single-turn readiness check that verifies the LLM
//! provider before any DB work.

use super::super::args::{EnrichArgs, EnrichMode};
use crate::errors::AppError;

/// Result of a single preflight ping (G35).
pub(crate) enum PreflightOutcome {
    /// The provider accepted the ping without errors.
    Healthy,
    /// Any other provider error (auth failure, etc.).
    Error(AppError),
}

/// Probes the configured LLM provider.
///
/// The OpenRouter JUDGE has no subprocess to ping; the preflight only
/// confirms a usable API key resolves. The chat client singleton is
/// initialised in `run()` before scan.
pub(crate) fn run_preflight_probe(args: &EnrichArgs) -> PreflightOutcome {
    match args.mode() {
        EnrichMode::OpenRouter => {
            match crate::config::resolve_api_key("openrouter", args.openrouter_api_key.as_deref()) {
                Some(_) => PreflightOutcome::Healthy,
                None => PreflightOutcome::Error(AppError::Validation(
                    "no OpenRouter API key: store one with `config add-key --provider openrouter --from-stdin` or pass --openrouter-api-key".into(),
                )),
            }
        }
    }
}
