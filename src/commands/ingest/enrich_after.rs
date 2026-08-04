//! Optional `--enrich-after` phase: run `enrich --operation memory-bindings`
//! once the ingest has persisted at least one memory.
//!
//! The pass is best-effort by design. A failure here means the corpus is in
//! the database but not yet bound to the graph, which is recoverable by
//! running `enrich` again; aborting the whole ingest over it would be worse.
//! The outcome is therefore reported as an NDJSON event rather than raised.

use super::args::IngestArgs;
use crate::errors::AppError;
use crate::output;

/// Runs the post-ingest binding pass and reports its outcome.
///
/// # Errors
/// Propagates only stdout/serialization failures. A failing `enrich` run is
/// logged and reported as `enrich_phase_failed`, never returned as an error.
pub(super) fn run(
    args: &IngestArgs,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<(), AppError> {
    output::emit_json_compact(&serde_json::json!({
        "event": "enrich_phase_started",
        "operation": "memory-bindings"
    }))?;
    let enrich_args = crate::commands::enrich::EnrichArgs {
        operation: Some(crate::commands::enrich::EnrichOperation::MemoryBindings),
        // GAP-SG-149: `None` resolves through `EnrichArgs::mode()`, whose
        // documented default is OpenRouter. Pinning `ClaudeCode` here made
        // `ingest --enrich-after` spawn a headless subprocess while every
        // other entry point defaulted to the REST path.
        mode: None,
        limit: None,
        scan_page_size: None,
        target: crate::commands::enrich::ReEmbedTarget::Memories,
        dry_run: false,
        namespace: args.namespace.clone(),
        openrouter_model: None,
        openrouter_api_key: None,
        // `openrouter_timeout` is no longer a field here: the flag is GLOBAL
        // since v1.2.3 and resolves through `runtime_config`, so this synthetic
        // `EnrichArgs` inherits whatever the operator passed to `ingest`
        // instead of pinning a value. GAP-SG-149 remains honoured — nothing
        // here shadows XDG.
        openrouter_base_url: None,
        db: args.db.clone(),
        json: false,
        resume: false,
        retry_failed: false,
        reset_stale_claims: false,
        stale_claim_secs: crate::commands::enrich::DEFAULT_ENRICH_STALE_CLAIM_SECS,
        max_cost_usd: args.max_cost_usd,
        llm_parallelism: Some(args.llm_parallelism as u32),
        wait_job_singleton: args.wait_job_singleton,
        force_job_singleton: args.force_job_singleton,
        names: Vec::new(),
        names_file: None,
        preflight_check: false,
        rate_limit_buffer: crate::commands::enrich::DEFAULT_ENRICH_RATE_LIMIT_BUFFER_SECS,
        max_load_check: true,
        circuit_breaker_threshold:
            crate::commands::enrich::DEFAULT_ENRICH_CIRCUIT_BREAKER_THRESHOLD,
        preserve_threshold: crate::commands::enrich::DEFAULT_ENRICH_PRESERVE_THRESHOLD,
        entity_description_grounding_threshold:
            crate::commands::enrich::DEFAULT_ENRICH_GROUNDING_THRESHOLD,
        force_redescribe: false,
        quality_sample: None,
        entity_names: Vec::new(),
        memory_names: Vec::new(),
        anchor_memory: None,
        entity_description_domain: "auto".to_string(),
        yield_every_n_items: None,
        ops_gate: false,
        min_output_chars: crate::commands::enrich::DEFAULT_BODY_ENRICH_MIN_CHARS,
        max_output_chars: crate::commands::enrich::DEFAULT_BODY_ENRICH_MAX_CHARS,
        preserve_check: true,
        prompt_template: None,
        until_empty: false,
        max_runtime: None,
        max_attempts: crate::commands::enrich::DEFAULT_ENRICH_MAX_ATTEMPTS,
        status: false,
        rest_concurrency: crate::constants::DEFAULT_ENRICH_REST_CONCURRENCY,
        // enrich-after runs a plain memory-bindings pass; dead-letter,
        // backoff-ignore and graph-only flags stay at their defaults.
        list_dead: false,
        requeue_dead: false,
        list_skipped: false,
        requeue_skipped: false,
        prune_dead_orphans: false,
        prune_dead_entity_orphans: false,
        ignore_backoff: false,
        body_extract_graph_only: false,
        print_schema: false,
    };
    match crate::commands::enrich::run(&enrich_args, llm_backend, embedding_backend) {
        Ok(()) => {
            output::emit_json_compact(&serde_json::json!({
                "event": "enrich_phase_completed"
            }))?;
        }
        Err(e) => {
            tracing::warn!(error = %e, "enrich --operation memory-bindings failed after ingest");
            output::emit_json_compact(&serde_json::json!({
                "event": "enrich_phase_failed",
                "error": e.to_string()
            }))?;
        }
    }
    Ok(())
}
