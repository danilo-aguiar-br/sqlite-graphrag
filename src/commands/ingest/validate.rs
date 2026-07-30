//! Mode-conditional flag validation for `ingest` (G20).

use super::args::{IngestArgs, IngestMode};
use crate::errors::AppError;

pub(crate) fn is_at_default<T: PartialEq>(value: T, default: T) -> bool {
    value == default
}

/// G20: validate that flags for one LLM provider were not passed when
/// the operator selected a different provider (or no provider). Flags
/// silently discarded by the wrong mode are surfaced as
///  BEFORE any DB work, so the operator gets
/// an actionable error instead of a surprise at runtime.
///
/// Mode-specific matrices:
/// - `mode=none` rejects: claude_binary, claude_model,
///   claude_timeout!=300, max_cost_usd, resume, retry_failed, keep_queue,
///   codex_binary, codex_model, codex_timeout!=300
/// - `mode=claude-code` rejects: codex_binary, codex_model, codex_timeout!=300
/// - `mode=codex` rejects: claude_binary, claude_model, claude_timeout!=300,
///   max_cost_usd, resume, retry_failed, keep_queue
pub(crate) fn validate_mode_conditional_flags_ingest(args: &IngestArgs) -> Result<(), AppError> {
    const DEFAULT_TIMEOUT: u64 = 300;
    const DEFAULT_RATE_LIMIT_WAIT: u64 = 60;

    let mut conflicts: Vec<String> = Vec::new();

    let is_local_mode = args.mode == IngestMode::None;

    // v1.1.1 (P12): --name-prefix is only applied by the local staging path;
    // rejecting it under LLM modes avoids a silently unprefixed corpus.
    if args.name_prefix.is_some() && !is_local_mode {
        return Err(AppError::Validation(
            "--name-prefix is not supported with --mode claude-code/codex/opencode; \
             use --mode none (default)"
                .to_string(),
        ));
    }

    if is_local_mode {
        if args.claude_binary.is_some() {
            conflicts.push("--claude-binary is ignored when --mode is none".to_string());
        }
        if args.claude_model.is_some() {
            conflicts.push("--claude-model is ignored when --mode is none".to_string());
        }
        if !is_at_default(args.claude_timeout, DEFAULT_TIMEOUT) {
            conflicts.push(format!(
                "--claude-timeout={} is ignored when --mode is none (remove the flag to use the default 300s)",
                args.claude_timeout
            ));
        }
        if args.codex_binary.is_some() {
            conflicts.push("--codex-binary is ignored when --mode is none".to_string());
        }
        if args.codex_model.is_some() {
            conflicts.push("--codex-model is ignored when --mode is none".to_string());
        }
        if !is_at_default(args.codex_timeout, DEFAULT_TIMEOUT) {
            conflicts.push(format!(
                "--codex-timeout={} is ignored when --mode is none (remove the flag to use the default 300s)",
                args.codex_timeout
            ));
        }
        if args.opencode_binary.is_some() {
            conflicts.push("--opencode-binary is ignored when --mode is none".to_string());
        }
        if args.opencode_model.is_some() {
            conflicts.push("--opencode-model is ignored when --mode is none".to_string());
        }
        if !is_at_default(args.opencode_timeout, DEFAULT_TIMEOUT) {
            conflicts.push(format!(
                "--opencode-timeout={} is ignored when --mode is none (remove the flag to use the default 300s)",
                args.opencode_timeout
            ));
        }
        if args.max_cost_usd.is_some() {
            conflicts.push("--max-cost-usd is ignored when --mode is none (cost is only tracked for LLM-backed modes)".to_string());
        }
        if args.resume {
            conflicts.push("--resume is ignored when --mode is none (the queue DB is only used by LLM-backed modes)".to_string());
        }
        if args.retry_failed {
            conflicts.push("--retry-failed is ignored when --mode is none".to_string());
        }
        if args.keep_queue {
            conflicts.push("--keep-queue is ignored when --mode is none".to_string());
        }
        if !is_at_default(args.rate_limit_wait, DEFAULT_RATE_LIMIT_WAIT) {
            conflicts.push(format!(
                "--rate-limit-wait={} is ignored when --mode is none",
                args.rate_limit_wait
            ));
        }
    }

    match args.mode {
        IngestMode::ClaudeCode => {
            if args.codex_binary.is_some() {
                conflicts.push("--codex-binary is ignored when --mode=claude-code".to_string());
            }
            if args.codex_model.is_some() {
                conflicts.push("--codex-model is ignored when --mode=claude-code".to_string());
            }
            if !is_at_default(args.codex_timeout, DEFAULT_TIMEOUT) {
                conflicts.push(format!(
                    "--codex-timeout={} is ignored when --mode=claude-code (remove the flag to use the default 300s)",
                    args.codex_timeout
                ));
            }
            if args.opencode_binary.is_some() {
                conflicts.push("--opencode-binary is ignored when --mode=claude-code".to_string());
            }
            if args.opencode_model.is_some() {
                conflicts.push("--opencode-model is ignored when --mode=claude-code".to_string());
            }
            if !is_at_default(args.opencode_timeout, DEFAULT_TIMEOUT) {
                conflicts.push(format!(
                    "--opencode-timeout={} is ignored when --mode=claude-code (remove the flag to use the default 300s)",
                    args.opencode_timeout
                ));
            }
        }
        IngestMode::Codex => {
            if args.claude_binary.is_some() {
                conflicts.push("--claude-binary is ignored when --mode=codex".to_string());
            }
            if args.claude_model.is_some() {
                conflicts.push("--claude-model is ignored when --mode=codex".to_string());
            }
            if !is_at_default(args.claude_timeout, DEFAULT_TIMEOUT) {
                conflicts.push(format!(
                    "--claude-timeout={} is ignored when --mode=codex (remove the flag to use the default 300s)",
                    args.claude_timeout
                ));
            }
            if args.max_cost_usd.is_some() {
                conflicts.push(
                    "--max-cost-usd is ignored when --mode=codex (OAuth-first; cost is metered by your subscription)"
                        .to_string(),
                );
            }
            if args.resume {
                conflicts.push("--resume is only valid for --mode=claude-code".to_string());
            }
            if args.retry_failed {
                conflicts.push("--retry-failed is only valid for --mode=claude-code".to_string());
            }
            if args.keep_queue {
                conflicts.push("--keep-queue is only valid for --mode=claude-code".to_string());
            }
            if args.opencode_binary.is_some() {
                conflicts.push("--opencode-binary is ignored when --mode=codex".to_string());
            }
            if args.opencode_model.is_some() {
                conflicts.push("--opencode-model is ignored when --mode=codex".to_string());
            }
            if !is_at_default(args.opencode_timeout, DEFAULT_TIMEOUT) {
                conflicts.push(format!(
                    "--opencode-timeout={} is ignored when --mode=codex (remove the flag to use the default 300s)",
                    args.opencode_timeout
                ));
            }
        }
        IngestMode::Opencode => {
            if args.claude_binary.is_some() {
                conflicts.push("--claude-binary is ignored when --mode=opencode".to_string());
            }
            if args.claude_model.is_some() {
                conflicts.push("--claude-model is ignored when --mode=opencode".to_string());
            }
            if !is_at_default(args.claude_timeout, DEFAULT_TIMEOUT) {
                conflicts.push(format!(
                    "--claude-timeout={} is ignored when --mode=opencode (remove the flag to use the default 300s)",
                    args.claude_timeout
                ));
            }
            if args.codex_binary.is_some() {
                conflicts.push("--codex-binary is ignored when --mode=opencode".to_string());
            }
            if args.codex_model.is_some() {
                conflicts.push("--codex-model is ignored when --mode=opencode".to_string());
            }
            if !is_at_default(args.codex_timeout, DEFAULT_TIMEOUT) {
                conflicts.push(format!(
                    "--codex-timeout={} is ignored when --mode=opencode (remove the flag to use the default 300s)",
                    args.codex_timeout
                ));
            }
            if args.max_cost_usd.is_some() {
                conflicts.push(
                    "--max-cost-usd is ignored when --mode=opencode (OAuth-first; cost is metered by your subscription)"
                        .to_string(),
                );
            }
            if args.resume {
                conflicts.push("--resume is only valid for --mode=claude-code".to_string());
            }
            if args.retry_failed {
                conflicts.push("--retry-failed is only valid for --mode=claude-code".to_string());
            }
            if args.keep_queue {
                conflicts.push("--keep-queue is only valid for --mode=claude-code".to_string());
            }
        }
        IngestMode::None => {}
    }

    if !conflicts.is_empty() {
        return Err(AppError::Validation(
            crate::i18n::validation::mode_flag_conflicts(
                &format!("{:?}", args.mode),
                &conflicts.join("\n  - "),
            ),
        ));
    }

    Ok(())
}
