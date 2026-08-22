//! Provider readiness: binary resolution, host saturation and the ping probe.
//!
//! Everything that must hold about the LLM provider before a single candidate
//! is scanned — which binary will be spawned, whether the host can take the
//! load, and whether the provider answers a one-turn ping.

use super::super::args::{EnrichArgs, EnrichMode, EnrichOperation};
use super::super::events::{run_preflight_probe, PhaseEvent, PreflightOutcome};
use crate::errors::AppError;
use crate::output::emit_json_line as emit_json;
use std::path::PathBuf;

/// Validate the provider binary upfront, only for LLM-backed write operations.
///
/// GAP-CLI-DRY-01: dry-run never spawns a provider — skip binary resolution.
/// Emits the `validate` phase event as a side effect, exactly as the inline
/// version did.
pub(super) fn resolve_provider_binary(args: &EnrichArgs) -> Result<Option<PathBuf>, AppError> {
    if args.dry_run || matches!(args.operation(), EnrichOperation::ReEmbed) {
        return Ok(None);
    }
    Ok(Some(match args.mode() {
        EnrichMode::OpenRouter => {
            // v1.0.95: the OpenRouter JUDGE is a REST call, not a spawned
            // binary. The chat client singleton was initialised at the top
            // of run(); this placeholder path threads through the dispatch
            // but is never dereferenced by the OpenRouter arm.
            emit_json(&PhaseEvent {
                phase: "validate",
                binary_path: None,
                version: None,
                items_total: None,
                items_pending: None,
                llm_parallelism: None,
            });
            PathBuf::new()
        }
    }))
}

/// G28-D: refuse to start when the system is saturated. This check
/// is BEFORE preflight so we never spend an OAuth turn on a host
/// that is already at the limit.
pub(super) fn check_system_load(args: &EnrichArgs) -> Result<(), AppError> {
    if args.max_load_check
        && !args.no_max_load_check
        && !args.dry_run
        && crate::system_load::is_system_saturated()
    {
        let load = crate::system_load::load_average_one();
        let n = crate::system_load::ncpus();
        return Err(AppError::Validation(
            crate::i18n::validation::system_load_exceeded(load, n),
        ));
    }
    Ok(())
}

/// G35: preflight probe — issue a single ping turn to verify the
/// provider is healthy before scanning N candidates. If the probe
/// fails with a rate-limit error, optionally fall back to a
/// different mode (typically codex) instead of failing the entire
/// batch. The probe itself consumes 1 OAuth turn, so it stays
/// opt-in (default off) to keep --dry-run and CI flows zero-cost.
pub(super) fn run_preflight(args: &EnrichArgs) -> Result<(), AppError> {
    if !(args.preflight_check
        && !args.dry_run
        && !matches!(args.operation(), EnrichOperation::ReEmbed))
    {
        return Ok(());
    }
    let preflight_result = run_preflight_probe(args);
    match preflight_result {
        PreflightOutcome::Healthy => {
            tracing::info!(target: "enrich", mode = ?args.mode(), "preflight probe healthy");
            Ok(())
        }
        PreflightOutcome::Error(e) => Err(e),
    }
}
