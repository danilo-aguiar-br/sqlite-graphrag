//! Pre-flight guards that run BEFORE any database access.
//!
//! Schema introspection, the `--ops-gate` recursion, mode-conditional flag
//! validation, the `--target` scope check, maintenance short-circuits and the
//! OpenRouter chat-client singleton. Each of these can end the invocation on
//! its own, so they are settled before a connection is ever opened.

use super::super::args::{EnrichArgs, EnrichMode, EnrichOperation, ReEmbedTarget};
use super::super::scheduler;
use crate::errors::AppError;

/// Runs every pre-DB guard.
///
/// Returns `Ok(true)` when the invocation was fully handled here and the caller
/// must return immediately (schema dump, `--ops-gate` fan-out, or a maintenance
/// flag), or `Ok(false)` to continue into the normal enrich pipeline.
pub(super) fn handle_pre_db_guards(
    args: &EnrichArgs,
    backends: crate::cli::BackendChoice,
) -> Result<bool, AppError> {
    // R-AN-01: schema introspection must not open the DB or call the LLM.
    if args.print_schema {
        crate::print_schema::emit(crate::print_schema::SchemaId::EnrichStatus)?;
        return Ok(true);
    }

    // GAP-CLI-PRIO-04 / OBS-02: --ops-gate runs quality gate ops first, in order.
    if args.ops_gate {
        for op in scheduler::gate_ops_order() {
            let mut gate_args = args.clone();
            gate_args.operation = Some(op);
            gate_args.ops_gate = false; // prevent recursion
            super::run(&gate_args, backends)?;
        }
        return Ok(true);
    }

    // G20: mode-conditional flag validation BEFORE any DB access.
    // Surfaces flags that the wrong mode would silently discard.

    // v1.1.1 (P2): --target only means something for re-embed. Fail loud
    // instead of silently ignoring it under another operation.
    if args.target != ReEmbedTarget::Memories
        && !matches!(args.operation(), EnrichOperation::ReEmbed)
    {
        let target_label = match args.target {
            ReEmbedTarget::Memories => "memories",
            ReEmbedTarget::Entities => "entities",
            ReEmbedTarget::Chunks => "chunks",
            ReEmbedTarget::All => "all",
        };
        return Err(AppError::Validation(
            crate::i18n::validation::reembed_target_only(target_label),
        ));
    }

    if super::super::status::try_handle_maintenance(args)? {
        return Ok(true);
    }

    // v1.0.95 (ADR-0054): when the JUDGE is OpenRouter the model is mandatory
    // (no default) and the API key must resolve BEFORE any network or DB work.
    // The chat client singleton is initialised here so every per-item dispatch
    // fetches it without re-threading the key.
    //
    // GAP-CLI-DRY-01 (v1.1.8): dry-run never calls the LLM — skip provider
    // key/model resolution so offline agents can preview candidates without
    // credentials. `--status` already returns earlier above.
    if args.mode() == EnrichMode::OpenRouter && !args.dry_run {
        let model = args.openrouter_model.as_deref().ok_or_else(|| {
            AppError::Validation(crate::i18n::validation::openrouter_model_required())
        })?;
        let resolved =
            crate::config::resolve_api_key("openrouter", args.openrouter_api_key.as_deref())
                .ok_or_else(|| {
                    AppError::Validation(crate::i18n::validation::openrouter_api_key_not_found())
                })?;
        crate::embedder::get_openrouter_chat_client(
            resolved.value,
            model,
            args.openrouter_chat_timeout_secs(),
        )?;
    }

    Ok(false)
}
