//! Extracted from extraction.rs (Wave C1).

use crate::errors::AppError;
use std::path::Path;
use std::io::Write;

#[allow(dead_code)] // provider parity; invoked via mode match in ops
pub(crate) fn call_codex(
    binary: &Path,
    prompt: &str,
    json_schema: &str,
    input_text: &str,
    model: Option<&str>,
    timeout_secs: u64,
) -> Result<(serde_json::Value, f64, bool), AppError> {
    use wait_timeout::ChildExt;

    // G31+G32+G33 (v1.0.69): validate the model BEFORE spawn, write the
    // schema to a trusted cache path (not /tmp), and reuse the
    // consolidated JSONL parser. See `codex_spawn.rs` for the canonical
    // hardening rationale.
    crate::commands::codex_spawn::validate_codex_model(model)?;
    let schema_file = crate::commands::codex_spawn::trusted_schema_path()?;

    let args = crate::commands::codex_spawn::CodexSpawnArgs {
        binary,
        prompt,
        json_schema,
        input_text,
        model,
        timeout_secs,
        schema_path: schema_file.clone(),
    };
    let mut cmd = crate::commands::codex_spawn::build_codex_command(&args)?;

    let mut child =
        crate::commands::claude_runner::spawn_with_memory_limit(&mut cmd).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("failed to spawn codex: {e}"),
            ))
        })?;

    let full_prompt = format!("{prompt}\n\n{input_text}");
    let stdin_bytes = full_prompt.into_bytes();
    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Validation("failed to open codex stdin".into()))?;
    let stdin_thread = std::thread::spawn(move || -> Result<(), std::io::Error> {
        child_stdin.write_all(&stdin_bytes)?;
        drop(child_stdin);
        Ok(())
    });

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let status = child.wait_timeout(timeout).map_err(AppError::Io)?;
    let _ = std::fs::remove_file(&schema_file);

    match status {
        Some(exit_status) => {
            stdin_thread
                .join()
                .map_err(|_| AppError::Validation("stdin thread panicked".into()))?
                .map_err(AppError::Io)?;

            tracing::debug!(
                target: "process",
                exit_code = ?exit_status.code(),
                elapsed_ms = start.elapsed().as_millis() as u64,
                "external process completed"
            );

            let mut stdout_buf = Vec::new();
            if let Some(mut out) = child.stdout.take() {
                std::io::Read::read_to_end(&mut out, &mut stdout_buf).map_err(AppError::Io)?;
            }
            if !exit_status.success() {
                let mut stderr_buf = Vec::new();
                if let Some(mut err) = child.stderr.take() {
                    std::io::Read::read_to_end(&mut err, &mut stderr_buf).map_err(AppError::Io)?;
                }
                let stderr_str = String::from_utf8_lossy(&stderr_buf);
                tracing::warn!(
                    target: "enrich",
                    exit_code = ?exit_status.code(),
                    stderr = %stderr_str.trim(),
                    "codex process failed"
                );
                return Err(AppError::Validation(format!(
                    "codex exited with code {:?}: {}",
                    exit_status.code(),
                    stderr_str.trim()
                )));
            }
            let stdout_str = String::from_utf8(stdout_buf)
                .map_err(|_| AppError::Validation("codex stdout is not valid UTF-8".into()))?;
            // G32: use the JSONL parser, NOT serde_json::from_str on the
            // entire stdout (codex emits one event per line).
            let result = crate::commands::codex_spawn::parse_codex_jsonl(&stdout_str)?;
            // Return the raw agent_message text parsed as JSON. Different
            // operations (memory-bindings, body-enrich) use different
            // output schemas, so we let the caller pick which fields to
            // extract. The previous implementation hardcoded
            // `{entities, urls}` which broke body-enrich.
            let value: serde_json::Value =
                serde_json::from_str(&result.last_agent_text).map_err(|e| {
                    AppError::Validation(format!(
                        "codex agent_message is not valid JSON: {e}; raw={}",
                        result.last_agent_text
                    ))
                })?;
            Ok((value, 0.0, false))
        }
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdin_thread.join();
            Err(AppError::Validation(format!(
                "codex timed out after {timeout_secs} seconds"
            )))
        }
    }
}

#[allow(dead_code)] // provider parity; invoked via mode match in ops
pub(crate) fn call_opencode(
    binary: &Path,
    prompt: &str,
    json_schema: &str,
    input_text: &str,
    model: Option<&str>,
    timeout_secs: u64,
) -> Result<(serde_json::Value, f64, bool), AppError> {
    use wait_timeout::ChildExt;

    let resolved_model = crate::commands::opencode_runner::resolve_opencode_model(model);

    let augmented_prompt = if json_schema.is_empty() {
        prompt.to_string()
    } else {
        format!(
            "{prompt}\n\nIMPORTANT: You MUST respond with ONLY valid JSON (no markdown, no explanation, no code fences). \
             The JSON MUST match this schema:\n{json_schema}"
        )
    };

    let mut cmd = crate::commands::opencode_runner::build_opencode_command_sync(
        binary,
        &resolved_model,
        &augmented_prompt,
        input_text,
    )?;

    let mut child = crate::commands::opencode_runner::spawn_opencode(&mut cmd).map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("failed to spawn opencode: {e}"),
        ))
    })?;

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let status = child.wait_timeout(timeout).map_err(AppError::Io)?;

    match status {
        Some(exit_status) => {
            tracing::debug!(
                target: "process",
                exit_code = ?exit_status.code(),
                elapsed_ms = start.elapsed().as_millis() as u64,
                "opencode process completed"
            );

            let mut stdout_buf = Vec::new();
            if let Some(mut out) = child.stdout.take() {
                std::io::Read::read_to_end(&mut out, &mut stdout_buf).map_err(AppError::Io)?;
            }
            if !exit_status.success() {
                let mut stderr_buf = Vec::new();
                if let Some(mut err) = child.stderr.take() {
                    std::io::Read::read_to_end(&mut err, &mut stderr_buf).map_err(AppError::Io)?;
                }
                let stderr_str = String::from_utf8_lossy(&stderr_buf);
                tracing::warn!(
                    target: "enrich",
                    exit_code = ?exit_status.code(),
                    stderr = %stderr_str.trim(),
                    "opencode process failed"
                );
                return Err(AppError::Validation(format!(
                    "opencode exited with code {:?}: {}",
                    exit_status.code(),
                    stderr_str.trim()
                )));
            }
            let stdout_str = String::from_utf8(stdout_buf)
                .map_err(|_| AppError::Validation("opencode stdout is not valid UTF-8".into()))?;
            let (text, cost, _tokens) =
                crate::commands::opencode_runner::parse_opencode_output(&stdout_str)?;
            let value: serde_json::Value =
                crate::commands::opencode_runner::parse_json_from_opencode_text(&text).map_err(
                    |e| AppError::Validation(format!("opencode response is not valid JSON: {e}")),
                )?;
            Ok((value, cost, false))
        }
        None => {
            let _ = child.kill();
            let _ = child.wait();
            Err(AppError::Validation(format!(
                "opencode timed out after {timeout_secs} seconds"
            )))
        }
    }
}

// Tests live under extraction.rs only (avoid duplicate_mod).
