//! Codex extraction spawn and output parsing.

use super::types::*;
use crate::commands::ingest_claude::ExtractionResult;
use crate::errors::AppError;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

pub(crate) fn write_schema_tempfile() -> Result<tempfile::NamedTempFile, AppError> {
    let mut f = tempfile::NamedTempFile::new().map_err(AppError::Io)?;
    std::io::Write::write_all(&mut f, EXTRACTION_SCHEMA_CODEX.as_bytes()).map_err(AppError::Io)?;
    std::io::Write::flush(&mut f).map_err(AppError::Io)?;
    Ok(f)
}

/// Invokes `codex exec` for a single file and returns the extraction result.
///
/// Uses `wait-timeout` for cross-platform subprocess timeout, `env_clear()`
/// for least-privilege environment, and reads prompt + file content from
/// stdin using the `-` argument (Codex Paperclip pattern).
///
/// # Errors
///
/// Returns `AppError::Validation` on extraction failure, rate limiting, or
/// schema errors. Returns `AppError::Io` on process spawn/IO failures.
pub(crate) fn extract_with_codex(
    binary: &Path,
    file_content: &[u8],
    model: Option<&str>,
    timeout_secs: u64,
    schema_file: &Path,
) -> Result<(ExtractionResult, Option<CodexUsage>), AppError> {
    use wait_timeout::ChildExt;

    // G31 Passo C (v1.0.69): delegate command construction to the shared
    // `codex_spawn::build_codex_command` helper so `enrich` and `ingest` stay
    // perfectly aligned on the canonical seven hardening flags. The local
    // function still owns the stdin pump + JSONL parsing (see below).
    let _ = timeout_secs; // currently unused; consumed by the helper when it spawns the process
    let _ = file_content; // pumped into stdin below, see `stdin_pump` thread
    let _ = schema_file; // helper reuses the temp file at the given path
    let prompt = String::new(); // empty prompt — helper appends file_content via args.input_text
    let mut cmd = crate::commands::codex_spawn::build_codex_command(
        &crate::commands::codex_spawn::CodexSpawnArgs {
            binary,
            prompt: &prompt,
            json_schema: "", // caller writes the schema directly via `schema_file`
            input_text: "",
            model,
            timeout_secs,
            schema_path: schema_file.to_path_buf(),
        },
    )?;

    // `build_codex_command` writes the JSON schema to `schema_path` and
    // appends `input_text` to the prompt via Paperclip stdin. For `ingest`
    // we want the schema content already on disk (the caller pre-wrote
    // EXTRACTION_SCHEMA_CODEX into the named tempfile), and the document
    // content goes through stdin via a dedicated thread (see below). Strip
    // the file the helper just rewrote — our caller pre-wrote it.
    let _ = std::fs::write(
        schema_file,
        EXTRACTION_SCHEMA_CODEX,
    );

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = crate::commands::claude_runner::spawn_with_memory_limit(&mut cmd).map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("failed to spawn codex: {e}"),
        ))
    })?;

    // Build stdin: prompt + document content (strict UTF-8 to surface encoding bugs early)
    let file_utf8 = String::from_utf8(file_content.to_vec())
        .map_err(|e| AppError::Validation(crate::i18n::validation::file_not_utf8(&e)))?;
    let stdin_payload = format!("{EXTRACTION_PROMPT}\n\n---\n\nDocument content:\n\n{file_utf8}");
    let stdin_bytes = stdin_payload.into_bytes();

    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Validation(crate::i18n::validation::failed_to_open_codex_stdin()))?;
    let stdin_thread = std::thread::spawn(move || -> Result<(), std::io::Error> {
        child_stdin.write_all(&stdin_bytes)?;
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
                let stderr_str = String::from_utf8_lossy(&stderr_buf);
                let stdout_str = String::from_utf8_lossy(&stdout_buf);
                // Check if stdout has JSONL with an error event before falling back
                if let Ok((result, usage)) = parse_codex_output(&stdout_str) {
                    return Ok((result, usage));
                }
                if stderr_str.contains("401")
                    || stderr_str.contains("Unauthorized")
                    || stderr_str.contains("auth")
                {
                    tracing::warn!(
                        target: "ingest",
                        "Codex CLI authentication expired. Re-authenticate with: codex auth login"
                    );
                }
                return Err(AppError::Validation(crate::i18n::validation::process_exited(
                    "codex exec",
                    exit_status.code(),
                    stderr_str.trim(),
                )));
            }

            let stdout = String::from_utf8(stdout_buf)
                .map_err(|_| AppError::Validation(crate::i18n::validation::codex_exec_stdout_not_utf8()))?;
            parse_codex_output(&stdout)
        }
        None => {
            tracing::warn!(target: "ingest", timeout_secs, "codex exec timed out, killing process");
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdin_thread.join();
            Err(AppError::Validation(
                crate::i18n::validation::process_timed_out("codex exec", timeout_secs),
            ))
        }
    }
}

/// Parses JSONL output from `codex exec --json`.
///
/// Event format (DOTS notation):
/// - `thread.started` — session init
/// - `turn.started` — model turn begins
/// - `item.completed` — message or tool call; last `agent_message` wins
/// - `turn.completed` — includes usage stats
/// - `turn.failed` — error with optional rate-limit indicator
/// - `error` — schema or validation error
///
/// # Errors
///
/// Returns `AppError::Validation` when no agent_message is found, when the
/// turn failed, or when the extracted JSON cannot be parsed as `ExtractionResult`.
pub(crate) fn parse_codex_output(stdout: &str) -> Result<(ExtractionResult, Option<CodexUsage>), AppError> {
    let mut last_agent_text: Option<String> = None;
    let mut usage: Option<CodexUsage> = None;
    let mut rate_limited = false;
    let mut schema_error = false;
    let mut turn_failed = false;
    let mut failed_message = String::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let event: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!(target: "ingest", line, "codex output: skipping malformed JSONL line");
                continue;
            }
        };

        let event_type = match event.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => continue,
        };

        match event_type {
            "item.completed" => {
                // Last agent_message wins (reasoning / tool calls may appear before)
                if let Some(item) = event.get("item") {
                    if item.get("type").and_then(|t| t.as_str()) == Some("agent_message") {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            last_agent_text = Some(text.to_string());
                        }
                    }
                }
            }
            "turn.completed" => {
                if let Some(u) = event.get("usage") {
                    if let Ok(parsed) = serde_json::from_value::<CodexUsage>(u.clone()) {
                        usage = Some(parsed);
                    }
                }
            }
            "turn.failed" => {
                turn_failed = true;
                if let Some(err) = event.get("error") {
                    let msg = err
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown error");
                    failed_message = msg.to_string();
                    if msg.contains("rate_limit")
                        || msg.contains("429")
                        || msg.contains("Too Many Requests")
                    {
                        rate_limited = true;
                    }
                }
            }
            "error" => {
                if let Some(msg) = event.get("message").and_then(|m| m.as_str()) {
                    if msg.contains("invalid_json_schema") || msg.contains("schema") {
                        schema_error = true;
                    }
                    tracing::warn!(target: "ingest", error_msg = msg, "codex error event received");
                }
            }
            _ => {
                // Gracefully skip unknown event types (thread.started, turn.started, etc.)
            }
        }
    }

    if rate_limited {
        return Err(AppError::RateLimited {
            detail: failed_message,
        });
    }

    if schema_error {
        return Err(AppError::Validation(
            "codex rejected the output schema (invalid_json_schema)".to_string(),
        ));
    }

    if turn_failed {
        return Err(AppError::Validation(
            crate::i18n::validation::codex_turn_failed(&failed_message),
        ));
    }

    let text = last_agent_text.ok_or_else(|| {
        AppError::Validation(crate::i18n::validation::codex_no_agent_message())
    })?;

    let extraction: ExtractionResult = serde_json::from_str(&text).map_err(|e| {
        AppError::Validation(crate::i18n::validation::failed_to_parse_codex_agent_message(
            &e, &text,
        ))
    })?;

    Ok((extraction, usage))
}

