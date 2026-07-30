//! Claude Code extraction spawn and output parsing.

use super::types::{
    ClaudeOutputElement, ExtractionResult, EXTRACTION_PROMPT, EXTRACTION_SCHEMA,
};
use crate::errors::AppError;
use crate::spawn::env_whitelist::apply_env_whitelist;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Invokes `claude -p` for a single file and returns the extraction result.
///
/// OAuth-only enforcement (gaps.md:41-49, v1.0.69 mandate):
///
/// - `wait-timeout` for cross-platform subprocess timeout.
/// - `env_clear()` for least-privilege environment.
/// - OAuth-only flow: NO `--bare` (PROHIBITED, gaps.md:49), no API-key path.
/// - Mandatory hardening: `--strict-mcp-config --mcp-config '{}'` to zero
///   MCP servers, and `--settings '{"hooks":{}}'` to disable hooks.
/// - If `ANTHROPIC_API_KEY` is set in the environment we ABORT the spawn
///   (return a `false` command with a violation marker) — API-key path is
///   PROHIBITED in this project.
pub(crate) fn extract_with_claude(
    binary: &Path,
    file_content: &[u8],
    model: Option<&str>,
    timeout_secs: u64,
) -> Result<(ExtractionResult, f64, bool), AppError> {
    use wait_timeout::ChildExt;

    // OAuth-only guard (gaps.md:47). If `ANTHROPIC_API_KEY` is set in the
    // environment we MUST abort — that is the API-key path which is
    // explicitly PROHIBITED. Use the OAuth flow exclusively.
    if let Ok(_key) = std::env::var("ANTHROPIC_API_KEY") {
        let mut cmd = crate::spawn::failing_command();
        cmd.env_clear();
        cmd.env("PATH", "/nonexistent");
        cmd.arg("--oauth-only-violation-anthropic-api-key-set");
        return Err(AppError::Validation(
            "ANTHROPIC_API_KEY is set in the environment; \
             sqlite-graphrag operates exclusively with OAuth (Pro/Max) and \
             the API-key path is PROHIBITED (gaps.md:47). Unset the variable \
             and re-run with `claude login` already completed in this session."
                .to_string(),
        ));
    }

    let mut cmd = Command::new(binary);

    // v1.0.83 (ADR-0041): env whitelist delegated to the shared helper.
    // `ANTHROPIC_API_KEY` is INTENTIONALLY ABSENT (defence-in-depth).
    apply_env_whitelist(&mut cmd, crate::spawn::env_whitelist::is_strict_env_clear());
    crate::spawn::apply_cwd_isolation(&mut cmd)?;

    // Canonical OAuth-only command line (gaps.md:201-208 + 211-213).
    // `--bare` is PROHIBITED (gaps.md:49) — never emitted.
    //
    // GAP-META-005 (v1.0.87, ADR-0045): `--mcp-config '{}'` inline is
    // rejected by Claude Code 2.1.177. Substitute the literal for a
    // tempfile path containing `{"mcpServers":{}}`.
    let mcp_config_path = crate::spawn::preflight::write_empty_mcp_config_tempfile()?;

    cmd.arg("-p")
        .arg(EXTRACTION_PROMPT)
        .arg("--strict-mcp-config")
        .arg("--mcp-config")
        .arg(mcp_config_path.as_os_str())
        .arg("--dangerously-skip-permissions")
        .arg("--settings")
        .arg(r#"{"hooks":{}}"#)
        .arg("--output-format")
        .arg("json")
        .arg("--json-schema")
        .arg(EXTRACTION_SCHEMA)
        .arg("--max-turns")
        .arg("7")
        .arg("--no-session-persistence");

    if let Some(m) = model {
        cmd.arg("--model").arg(m);
    }

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // GAP-META-005 (v1.0.87, ADR-0045): pre-flight gate runs after argv
    // is fully built. Validates binary, argv-size, walk-up of `.mcp.json`,
    // and `CLAUDE_CONFIG_DIR` cleanliness.
    let argv_refs: Vec<std::ffi::OsString> = cmd.get_args().map(|s| s.to_os_string()).collect();
    let preflight_args = crate::spawn::preflight::PreFlightArgs {
        binary_path: binary,
        argv: &argv_refs,
        workspace_root: std::path::Path::new("."),
        mcp_config_inline_json: None,
        expected_output_bytes: 65_536,
        spawner_name: "ingest_claude",
    };
    if let Err(e) = crate::spawn::preflight::preflight_check(&preflight_args) {
        // v1.0.88 (BUG-6 fix, ADR-0046): propagate the structured
        // `PreFlightError` via the `From` impl in `errors.rs` so callers
        // receive `AppError::PreFlightFailed` (exit 16) instead of a
        // bare `std::process::exit(16)` that discards the variant name,
        // tracing context, and PT-BR i18n.
        return Err(crate::errors::AppError::from(e));
    }

    let mut child = crate::commands::claude_runner::spawn_with_memory_limit(&mut cmd).map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("failed to spawn claude: {e}"),
        ))
    })?;

    let stdin_data = file_content.to_vec();
    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Validation(crate::i18n::validation::failed_to_open_claude_stdin()))?;
    let stdin_thread = std::thread::spawn(move || -> Result<(), std::io::Error> {
        child_stdin.write_all(&stdin_data)?;
        drop(child_stdin);
        Ok(())
    });

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let status = child.wait_timeout(timeout).map_err(AppError::Io)?;

    match status {
        Some(exit_status) => {
            stdin_thread
                .join()
                .map_err(|_| AppError::Validation(crate::i18n::validation::stdin_thread_panicked()))?
                .map_err(AppError::Io)?;

            tracing::debug!(
                target: "process",
                exit_code = ?exit_status.code(),
                elapsed_ms = start.elapsed().as_millis() as u64,
                "external process completed"
            );

            let mut stdout_buf = Vec::new();
            let mut stderr_buf = Vec::new();
            if let Some(mut out) = child.stdout.take() {
                std::io::Read::read_to_end(&mut out, &mut stdout_buf).map_err(AppError::Io)?;
            }
            if let Some(mut err) = child.stderr.take() {
                std::io::Read::read_to_end(&mut err, &mut stderr_buf).map_err(AppError::Io)?;
            }

            if !exit_status.success() {
                let stdout_str = String::from_utf8_lossy(&stdout_buf);
                if let Ok(elements) = serde_json::from_str::<Vec<ClaudeOutputElement>>(&stdout_str)
                {
                    if let Some(re) = elements
                        .iter()
                        .find(|e| e.r#type.as_deref() == Some("result"))
                    {
                        if re.terminal_reason.as_deref() == Some("max_turns") {
                            tracing::warn!(
                                target: "ingest",
                                "extraction hit max_turns limit — hooks may have consumed turns"
                            );
                            return Err(AppError::Validation(
                                "claude -p hit max_turns: hooks may be consuming turns".into(),
                            ));
                        }
                        if re.is_error {
                            let err_msg = re
                                .error
                                .as_deref()
                                .or(re.result.as_deref())
                                .unwrap_or("unknown error");
                            if err_msg.contains("rate_limit") || err_msg.contains("overloaded") {
                                return Err(AppError::RateLimited {
                                    detail: err_msg.to_string(),
                                });
                            }
                            if err_msg.contains("Not logged in")
                                || err_msg.contains("authentication")
                            {
                                tracing::warn!(
                                    target: "ingest",
                                    "Claude Code authentication failed. Re-authenticate interactively with: claude"
                                );
                            }
                            return Err(AppError::Validation(
                                crate::i18n::validation::claude_p_failed(err_msg),
                            ));
                        }
                    }
                }
                let stderr_str = String::from_utf8_lossy(&stderr_buf);
                if stderr_str.contains("auth") || stderr_str.contains("login") {
                    tracing::warn!(
                        target: "ingest",
                        "Claude Code authentication may have failed. Re-authenticate with: claude"
                    );
                }
                return Err(AppError::Validation(crate::i18n::validation::process_exited(
                    "claude -p",
                    exit_status.code(),
                    stderr_str.trim(),
                )));
            }

            let stdout = String::from_utf8(stdout_buf)
                .map_err(|_| AppError::Validation(crate::i18n::validation::claude_p_stdout_not_utf8()))?;
            parse_claude_output(&stdout)
        }
        None => {
            tracing::warn!(target: "ingest", timeout_secs, "claude -p timed out, killing process");
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdin_thread.join();
            Err(AppError::Validation(
                crate::i18n::validation::process_timed_out("claude -p", timeout_secs),
            ))
        }
    }
}

/// Parses the JSON array output from `claude -p --output-format json`.
///
/// Returns `(extraction, cost_usd, is_oauth)` where `is_oauth` is true when
/// the init element reports `apiKeySource: "none"` (OAuth subscription).
/// Parses `claude -p --output-format json` output into the ingest
/// `ExtractionResult`. Delegates the shared element/error/oauth handling to
/// `claude_runner::parse_claude_output` (DRY v1.0.97), which additionally
/// covers G03 max_turns detection and the auth-failure warn, then deserialises
/// the structured value into the typed `ExtractionResult`.
pub(crate) fn parse_claude_output(stdout: &str) -> Result<(ExtractionResult, f64, bool), AppError> {
    let result = crate::commands::claude_runner::parse_claude_output_opts(stdout, true)?;
    let extraction: ExtractionResult = serde_json::from_value(result.value).map_err(|e| {
        AppError::Validation(crate::i18n::validation::failed_to_parse_extraction(
            "claude", &e,
        ))
    })?;
    Ok((extraction, result.cost_usd, result.is_oauth))
}
