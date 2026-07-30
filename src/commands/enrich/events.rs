//! NDJSON event types, preflight probe, and mode-flag validation for enrich.
//! Extracted from mod.rs (Wave C1).

use serde::Serialize;
use std::time::Instant;

use rusqlite::Connection;

use super::args::{EnrichArgs, EnrichMode, EnrichOperation};
use super::scan::scan_operation;
use super::extraction::find_codex_binary;
use crate::commands::ingest_claude::find_claude_binary;
use crate::errors::AppError;

// ---------------------------------------------------------------------------
// Internal types — raw LLM output structs
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// NDJSON event types emitted to stdout
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub(crate) struct PhaseEvent<'a> {
    pub(crate) phase: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) binary_path: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) items_total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) items_pending: Option<usize>,
    /// Active parallel LLM worker count (1 = serial). Present only on the "scan" phase event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) llm_parallelism: Option<u32>,
}

/// v1.1.06: emitted **before** the (potentially heavy) candidate SQL so hooks
/// never see a silent hang after `validate`. No product telemetry — NDJSON only.
#[derive(Debug, Serialize)]
pub(crate) struct ScanStartEvent<'a> {
    pub(crate) phase: &'static str,
    /// CLI kebab-case operation name (`entity-connect` or `cross-domain-bridges`).
    pub(crate) operation: &'a str,
    pub(crate) entities_in_namespace: i64,
    /// O(n) degree-0 + NER binding proxy (status backlog); distinct from pair enqueue count.
    pub(crate) backlog_degree0_proxy: Option<i64>,
    pub(crate) pair_algorithm: Option<&'static str>,
    pub(crate) limit: Option<usize>,
    pub(crate) scan_deadline_secs: Option<u64>,
}

/// CLI kebab-case name for an enrich operation (matches clap `--operation` values).
pub(crate) fn enrich_operation_cli_name(op: &EnrichOperation) -> &'static str {
    match op {
        EnrichOperation::MemoryBindings => "memory-bindings",
        EnrichOperation::AugmentBindings => "augment-bindings",
        EnrichOperation::EntityDescriptions => "entity-descriptions",
        EnrichOperation::BodyEnrich => "body-enrich",
        EnrichOperation::ReEmbed => "re-embed",
        EnrichOperation::WeightCalibrate => "weight-calibrate",
        EnrichOperation::RelationReclassify => "relation-reclassify",
        EnrichOperation::EntityConnect => "entity-connect",
        EnrichOperation::EntityTypeValidate => "entity-type-validate",
        EnrichOperation::DescriptionEnrich => "description-enrich",
        EnrichOperation::CrossDomainBridges => "cross-domain-bridges",
        EnrichOperation::DomainClassify => "domain-classify",
        EnrichOperation::GraphAudit => "graph-audit",
        EnrichOperation::DeepResearchSynth => "deep-research-synth",
        EnrichOperation::BodyExtract => "body-extract",
    }
}

/// GAP-SG-45: separates the SCAN metric (always serial — a single SQL sweep of
/// the candidate set) from the DRAIN metric (the parallel worker fan-out). The
/// legacy "scan" `PhaseEvent` reported `llm_parallelism` on the scan event,
/// conflating the two; this event makes the distinction explicit.
#[derive(Debug, Serialize)]
pub(crate) struct ConcurrencyEvent {
    pub(crate) phase: &'static str,
    pub(crate) scan_parallelism: u32,
    pub(crate) drain_parallelism: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct ItemEvent<'a> {
    /// Item identifier (memory name or entity name).
    pub(crate) item: &'a str,
    pub(crate) status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entity_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entities: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rels: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chars_before: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chars_after: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    pub(crate) index: usize,
    pub(crate) total: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct EnrichSummary {
    pub(crate) summary: bool,
    pub(crate) operation: String,
    pub(crate) items_total: usize,
    pub(crate) completed: usize,
    pub(crate) failed: usize,
    pub(crate) skipped: usize,
    pub(crate) cost_usd: f64,
    pub(crate) elapsed_ms: u64,
    /// v1.0.84 (ADR-0042): discriminator of the LLM backend that actually ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) backend_invoked: Option<&'static str>,
    /// GAP-SG-15: items still parked on backoff when the run ended.
    pub(crate) waiting: i64,
    /// GAP-SG-15: dead-letter items at the end of the run.
    pub(crate) dead: i64,
    /// GAP-CLI-EC-08: true when soft/max runtime budget ended the run early.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) budget_exhausted: Option<bool>,
    /// GAP-CLI-EC-08: estimated pairs still unscanned (status proxy).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pairs_remaining_estimate: Option<i64>,
    /// GAP-CLI-PRIO-05: cooperative yields performed this run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) yields: Option<u64>,
    /// Wave 3 / EC-09: true when entity-connect yielded to hot entity-descriptions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) preempted_for_gate: Option<bool>,
}

// ---------------------------------------------------------------------------
// Queue DB
// ---------------------------------------------------------------------------

// Queue functions and structs moved to queue.rs

// LLM call_claude and call_openrouter moved to extraction.rs

// ---------------------------------------------------------------------------
// Preflight probe (G35) — single-turn ping to verify the LLM provider
// ---------------------------------------------------------------------------

/// Result of a single preflight ping (G35).
pub(crate) enum PreflightOutcome {
    /// The provider accepted the ping without rate-limit or other errors.
    Healthy,
    /// The provider rejected the ping due to OAuth rate limit. The
    /// `suggestion` field is a human hint that callers can embed in the
    /// user-facing error.
    RateLimited {
        reason: String,
        suggestion: &'static str,
    },
    /// Any other provider error (binary missing, auth failure, etc.).
    Error(AppError),
}

/// Probes the configured LLM provider with a 1-turn ping.
///
/// - Claude: `claude -p "ping" --max-turns 1 --strict-mcp-config --mcp-config '{}'`
/// - Codex:  `codex exec -c mcp_servers='{}' "ping" --json`
///
/// The probe intentionally avoids spawning any MCP server children (G28-A)
/// to keep its own process footprint at the minimum.
pub(crate) fn run_preflight_probe(args: &EnrichArgs) -> PreflightOutcome {
    let timeout = std::time::Duration::from_secs(args.rate_limit_buffer.max(60));

    match args.mode() {
        EnrichMode::ClaudeCode => {
            let bin = match find_claude_binary(args.claude_binary.as_deref()) {
                Ok(b) => b,
                Err(e) => return PreflightOutcome::Error(e),
            };
            // v1.0.88 (BUG-3 fix, ADR-0046): write the empty MCP config to a
            // tempfile (Claude Code 2.1.177 rejects the inline `{}`
            // form) and run the preflight gate before spawn, mirroring
            // the canonical pattern in `claude_runner::build_claude_command`.
            let mcp_config_path = match crate::spawn::preflight::write_empty_mcp_config_tempfile() {
                Ok(p) => p,
                Err(e) => {
                    return PreflightOutcome::Error(AppError::Io(e));
                }
            };
            let mut cmd = std::process::Command::new(&bin);
            crate::spawn::env_whitelist::apply_env_whitelist(
                &mut cmd,
                crate::spawn::env_whitelist::is_strict_env_clear(),
            );
            if let Err(e) = crate::spawn::apply_cwd_isolation(&mut cmd) {
                return PreflightOutcome::Error(e);
            }
            cmd.arg("-p")
                .arg("ping")
                .arg("--max-turns")
                .arg("1")
                .arg("--strict-mcp-config")
                .arg("--mcp-config")
                .arg(mcp_config_path.as_os_str())
                .arg("--dangerously-skip-permissions")
                .arg("--settings")
                .arg("{\"hooks\":{}}")
                .arg("--output-format")
                .arg("json")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let child = match crate::commands::claude_runner::spawn_with_memory_limit(&mut cmd) {
                Ok(c) => c,
                Err(e) => {
                    return PreflightOutcome::Error(AppError::Io(e));
                }
            };
            let output = match wait_with_timeout(child, timeout) {
                Ok(out) => out,
                Err(e) => return PreflightOutcome::Error(e),
            };
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("hit your session limit")
                    || stderr.contains("rate_limit")
                    || stderr.contains("429")
                {
                    return PreflightOutcome::RateLimited {
                        reason: stderr.trim().to_string(),
                        suggestion:
                            "wait for the OAuth window to reset or use --fallback-mode codex",
                    };
                }
                return PreflightOutcome::Error(AppError::Validation(crate::i18n::validation::preflight_probe_failed(stderr.trim())));
            }
            PreflightOutcome::Healthy
        }
        EnrichMode::Codex => {
            let bin = match find_codex_binary(args.codex_binary.as_deref()) {
                Ok(b) => b,
                Err(e) => return PreflightOutcome::Error(e),
            };
            crate::commands::codex_spawn::validate_codex_model(args.codex_model.as_deref())
                .map_err(PreflightOutcome::Error)
                .ok();
            let schema = "{}";
            let schema_path = match crate::commands::codex_spawn::trusted_schema_path() {
                Ok(p) => p,
                Err(e) => return PreflightOutcome::Error(e),
            };
            let spawn_args = crate::commands::codex_spawn::CodexSpawnArgs {
                binary: &bin,
                prompt: "ping",
                json_schema: schema,
                input_text: "",
                model: args.codex_model.as_deref(),
                timeout_secs: args.rate_limit_buffer.max(60),
                schema_path: schema_path.clone(),
            };
            let mut cmd = match crate::commands::codex_spawn::build_codex_command(&spawn_args) {
                Ok(c) => c,
                Err(e) => return PreflightOutcome::Error(e),
            };
            let child = match crate::commands::claude_runner::spawn_with_memory_limit(&mut cmd) {
                Ok(c) => c,
                Err(e) => return PreflightOutcome::Error(AppError::Io(e)),
            };
            let output = match wait_with_timeout(child, timeout) {
                Ok(out) => out,
                Err(e) => return PreflightOutcome::Error(e),
            };
            let _ = std::fs::remove_file(&schema_path);
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("rate_limit")
                    || stderr.contains("429")
                    || stderr.contains("Too Many Requests")
                {
                    return PreflightOutcome::RateLimited {
                        reason: stderr.trim().to_string(),
                        suggestion: "wait for the rate-limit window to reset",
                    };
                }
                return PreflightOutcome::Error(AppError::Validation(crate::i18n::validation::preflight_probe_failed(stderr.trim())));
            }
            PreflightOutcome::Healthy
        }
        EnrichMode::Opencode => {
            let bin = match crate::commands::opencode_runner::find_opencode_binary_with_override(
                args.opencode_binary.as_deref(),
            ) {
                Ok(b) => b,
                Err(e) => return PreflightOutcome::Error(e),
            };
            let model =
                crate::commands::opencode_runner::resolve_opencode_model(args.opencode_model.as_deref());
            let mut cmd =
                match crate::commands::opencode_runner::build_opencode_command_sync(&bin, &model, "ping", "")
                {
                    Ok(c) => c,
                    Err(e) => return PreflightOutcome::Error(e),
                };
            let child = match crate::commands::opencode_runner::spawn_opencode(&mut cmd) {
                Ok(c) => c,
                Err(e) => return PreflightOutcome::Error(AppError::Io(e)),
            };
            let output = match wait_with_timeout(child, timeout) {
                Ok(out) => out,
                Err(e) => return PreflightOutcome::Error(e),
            };
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("rate_limit")
                    || stderr.contains("429")
                    || stderr.contains("Too Many Requests")
                {
                    return PreflightOutcome::RateLimited {
                        reason: stderr.trim().to_string(),
                        suggestion: "wait for the rate-limit window to reset",
                    };
                }
                return PreflightOutcome::Error(AppError::Validation(crate::i18n::validation::preflight_probe_failed(stderr.trim())));
            }
            PreflightOutcome::Healthy
        }
        EnrichMode::OpenRouter => {
            // v1.0.95: the OpenRouter JUDGE has no subprocess to ping; the
            // preflight only confirms a usable API key resolves. The chat
            // client singleton is initialised in run() before scan.
            match crate::config::resolve_api_key("openrouter", args.openrouter_api_key.as_deref()) {
                Some(_) => PreflightOutcome::Healthy,
                None => PreflightOutcome::Error(AppError::Validation(
                    "OPENROUTER_API_KEY not found for --mode openrouter preflight".into(),
                )),
            }
        }
    }
}

/// Cross-platform wait with timeout (no extra crate dependency).
pub(crate) fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: std::time::Duration,
) -> Result<std::process::Output, AppError> {
    use wait_timeout::ChildExt;
    let start = std::time::Instant::now();
    let Some(exit) = child.wait_timeout(timeout).map_err(AppError::Io)? else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(AppError::Validation(crate::i18n::validation::preflight_probe_timed_out(start.elapsed().as_secs())));
    };
    let mut stdout = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        std::io::Read::read_to_end(&mut out, &mut stdout).map_err(AppError::Io)?;
    }
    let mut stderr = Vec::new();
    if let Some(mut err) = child.stderr.take() {
        std::io::Read::read_to_end(&mut err, &mut stderr).map_err(AppError::Io)?;
    }
    Ok(std::process::Output {
        status: exit,
        stdout,
        stderr,
    })
}

// Scan functions moved to scan.rs

// Persist functions moved to postprocess.rs

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// G20: mode-conditional flag validation
// ---------------------------------------------------------------------------

/// True when a scalar value matches its declared default. Used to
/// distinguish "operator passed an explicit override" from "clap filled
/// the default" for flags with default_value_t.
pub(crate) fn is_at_default<T: PartialEq>(value: T, default: T) -> bool {
    value == default
}

/// G20: validate that flags for one LLM provider were not passed when
/// the operator selected a different provider. Flags silently discarded
/// by the wrong mode are surfaced as AppError::Validation BEFORE any
/// DB work, so the operator gets an actionable error instead of a
/// surprise at runtime.
///
/// Detection rules:
/// - For `Option<PathBuf>` / `Option<String>`: is_some() means explicit
/// - For scalar fields with default_value_t: value != default means explicit
/// - For boolean fields: true means explicit (default is false)
///
/// Mode-specific matrices:
/// - mode=claude-code rejects: codex_binary, codex_model, codex_timeout != 300
/// - mode=codex rejects: claude_binary, claude_model, claude_timeout != 300, max_cost_usd
pub(crate) fn validate_mode_conditional_flags_enrich(args: &EnrichArgs) -> Result<(), AppError> {
    const DEFAULT_TIMEOUT: u64 = 300;

    let mut conflicts: Vec<String> = Vec::new();

    match args.mode() {
        EnrichMode::ClaudeCode => {
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
        }
        EnrichMode::Codex => {
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
                    "--max-cost-usd is ignored when --mode=codex (OAuth-first; cost is metered by your subscription, not the call)"
                        .to_string(),
                );
            }
        }
        EnrichMode::Opencode => {
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
            if args.max_cost_usd.is_some() {
                conflicts.push(
                    "--max-cost-usd is ignored when --mode=opencode (OAuth-first; cost is metered by your subscription, not the call)"
                        .to_string(),
                );
            }
        }
        EnrichMode::OpenRouter => {
            if args.claude_binary.is_some() {
                conflicts.push("--claude-binary is ignored when --mode=openrouter".to_string());
            }
            if args.claude_model.is_some() {
                conflicts.push("--claude-model is ignored when --mode=openrouter".to_string());
            }
            if args.codex_binary.is_some() {
                conflicts.push("--codex-binary is ignored when --mode=openrouter".to_string());
            }
            if args.codex_model.is_some() {
                conflicts.push("--codex-model is ignored when --mode=openrouter".to_string());
            }
            if args.opencode_binary.is_some() {
                conflicts.push("--opencode-binary is ignored when --mode=openrouter".to_string());
            }
            if args.opencode_model.is_some() {
                conflicts.push("--opencode-model is ignored when --mode=openrouter".to_string());
            }
            if !is_at_default(args.claude_timeout, DEFAULT_TIMEOUT) {
                conflicts.push(format!(
                    "--claude-timeout={} is ignored when --mode=openrouter (remove the flag to use the default 300s)",
                    args.claude_timeout
                ));
            }
            if !is_at_default(args.codex_timeout, DEFAULT_TIMEOUT) {
                conflicts.push(format!(
                    "--codex-timeout={} is ignored when --mode=openrouter (remove the flag to use the default 300s)",
                    args.codex_timeout
                ));
            }
            if !is_at_default(args.opencode_timeout, DEFAULT_TIMEOUT) {
                conflicts.push(format!(
                    "--opencode-timeout={} is ignored when --mode=openrouter (remove the flag to use the default 300s)",
                    args.opencode_timeout
                ));
            }
        }
    }

    if !conflicts.is_empty() {
        return Err(AppError::Validation(crate::i18n::validation::mode_flag_conflicts(
                &args.mode().to_string(),
                &conflicts.join("\n  - "),
            )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------

/// Main entry point for the `enrich` command.
/// Run [`scan_operation`] with an optional wall-clock deadline enforced via
/// [`rusqlite::Connection::get_interrupt_handle`].
///
/// v1.1.06 (GAP-ENTITY-CONNECT-SCAN-CARTESIAN): the first enrich scan used to
/// run with no timeout; a cartesian SQL could pin the process (and the enrich
/// singleton) indefinitely. When `deadline` is `Some`, a watchdog thread calls
/// `interrupt()` at the deadline so the scan fails as
/// [`AppError::Timeout`] (exit 1) — never as exit 75.
pub(crate) fn scan_operation_with_deadline(
    conn: &Connection,
    namespace: &str,
    args: &EnrichArgs,
    deadline: Option<Instant>,
) -> Result<Vec<String>, AppError> {
    let Some(deadline) = deadline else {
        return scan_operation(conn, namespace, args);
    };

    if Instant::now() >= deadline {
        return Err(AppError::Timeout {
            operation: format!("enrich {:?} scan", args.operation()),
            duration_secs: 0,
        });
    }

    let handle = conn.get_interrupt_handle();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_w = std::sync::Arc::clone(&stop);
    let watchdog = std::thread::spawn(move || {
        while !stop_w.load(std::sync::atomic::Ordering::Relaxed) {
            if Instant::now() >= deadline {
                handle.interrupt();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });

    let scan_t0 = Instant::now();
    let result = scan_operation(conn, namespace, args);
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = watchdog.join();

    match result {
        Ok(v) => Ok(v),
        Err(AppError::Database(ref e)) if is_sqlite_interrupt(e) => Err(AppError::Timeout {
            operation: format!("enrich {:?} scan", args.operation()),
            duration_secs: scan_t0.elapsed().as_secs().max(1),
        }),
        Err(e) => Err(e),
    }
}

pub(crate) fn is_sqlite_interrupt(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(code, _) => {
            code.code == rusqlite::ErrorCode::OperationInterrupted || code.extended_code == 9
            // SQLITE_INTERRUPT
        }
        other => {
            let s = other.to_string().to_ascii_lowercase();
            s.contains("interrupt") || s.contains("cancelled")
        }
    }
}

/// Resolve drain worker count and emit concurrency warnings (G19/G28-D/G34).
pub(crate) fn resolve_drain_parallelism(args: &EnrichArgs) -> usize {
    let parallelism = if args.mode() == EnrichMode::OpenRouter {
        let rest = args.rest_concurrency.unwrap_or(8).clamp(1, 16) as usize;
        tracing::info!(
            target: "enrich",
            concurrency = rest,
            source = "rest_concurrency",
            "OpenRouter REST concurrency (clamp 1..=16)"
        );
        rest
    } else {
        let p = args.llm_parallelism.clamp(1, 32) as usize;
        tracing::info!(
            target: "enrich",
            concurrency = p,
            source = "llm_parallelism",
            "LLM subprocess parallelism (clamp 1..=32)"
        );
        p
    };
    if parallelism > 1 {
        tracing::info!(
            target: "enrich",
            llm_parallelism = parallelism,
            "parallel LLM processing with bounded thread pool"
        );
    }
    if parallelism > 4 {
        match args.mode() {
            EnrichMode::ClaudeCode => {
                tracing::warn!(
                    target: "enrich",
                    llm_parallelism = parallelism,
                    recommended_max = 4,
                    mode = "claude-code",
                    "llm_parallelism above 4 multiplies Claude Code subprocess fan-out; \
                     consider combining with `config set llm.claude_empty_config_dir <path>` \
                     to cut MCP children (G28-A)"
                );
            }
            EnrichMode::Codex if parallelism > 16 => {
                tracing::warn!(
                    target: "enrich",
                    llm_parallelism = parallelism,
                    recommended_max = 16,
                    mode = "codex",
                    "llm_parallelism above 16 risks OAuth rate-limit on Codex;                      consider --llm-parallelism 8 for safer concurrency"
                );
            }
            EnrichMode::Codex => {}
            EnrichMode::Opencode if parallelism > 16 => {
                tracing::warn!(
                    target: "enrich",
                    llm_parallelism = parallelism,
                    recommended_max = 16,
                    mode = "opencode",
                    "llm_parallelism above 16 risks OAuth rate-limit on OpenCode;                      consider --llm-parallelism 8 for safer concurrency"
                );
            }
            EnrichMode::Opencode => {}
            EnrichMode::OpenRouter => {}
        }
    }
    parallelism
}
