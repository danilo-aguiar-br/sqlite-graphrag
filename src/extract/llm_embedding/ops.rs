//! Embedding orchestration and headless LLM subprocess invokers.

use super::timeout::extract_exit_info;
use super::types::EmbeddingFlavour;
use super::wire::{
    build_batch_schema, build_single_schema, parse_llm_json, BatchEmbeddingResponse,
    EmbeddingResponse,
};
use super::LlmEmbedding;
use crate::errors::AppError;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

impl LlmEmbedding {

    /// LLM call. Returns `(global_index, vector)` pairs. Async — this
    /// is the unit of work scheduled by the bounded fan-out in
    /// `crate::embedder`.
    ///
    /// Cancel safety: the future owns its subprocess via
    /// `kill_on_drop(true)`, so dropping it (e.g. losing a
    /// `tokio::select!` race against a cancellation token) kills the
    /// child and leaks nothing.
    pub async fn embed_batch_async(
        &self,
        prefix: &str,
        batch: &[(usize, String)],
    ) -> Result<Vec<(usize, Vec<f32>)>, AppError> {
        let dim = crate::constants::embedding_dim();
        if batch.is_empty() {
            return Ok(Vec::new());
        }
        if batch.len() == 1 {
            let (idx, text) = (&batch[0].0, &batch[0].1);
            let v = self.invoke_single_async(prefix, text, dim).await?;
            return Ok(vec![(*idx, v)]);
        }

        let mut prompt = format!(
            "Generate {dim}-dimensional semantic embedding vectors for each numbered text below.\n\
             Return a JSON object with an \"items\" array containing EXACTLY {n} items.\n\
             Each item has \"i\" (the 1-based index) and \"v\" (the {dim}-float vector, values between -1 and 1).\n\n",
            n = batch.len()
        );
        for (pos, (_, text)) in batch.iter().enumerate() {
            prompt.push_str(&format!("{}: {prefix}{text}\n", pos + 1));
        }

        // BUG-TIMEOUT-HARDCODE-001: batch timeout is now instance-scoped
        // (no more std::env::set_var which was unsafe in multi-thread).
        let _batch_timeout = self.instance_embed_timeout_for_batch(batch.len());
        let stdout = match self.flavour {
            EmbeddingFlavour::Claude => {
                self.invoke_claude(&prompt, &build_batch_schema(dim))
                    .await?
            }
            EmbeddingFlavour::Codex => {
                let schema = self.codex_schema_file(dim, true)?;
                self.invoke_codex(&prompt, schema.path()).await?
            }
            EmbeddingFlavour::Opencode => {
                let opencode_prompt = format!(
                    "You are a batch embedding function. For each numbered text item below, \
                     generate an array of exactly {dim} floating-point numbers between -1 and 1 \
                     representing its semantic meaning. Output ONLY a JSON object with key \"items\" \
                     containing an array of objects, each with \"i\" (the 1-based index) and \
                     \"v\" (the {dim}-element float array). No markdown, no explanation.\n\n\
                     {prompt}"
                );
                self.invoke_opencode(&opencode_prompt).await?
            }
        };
        let parsed: BatchEmbeddingResponse = parse_llm_json(&stdout).map_err(|e| {
            AppError::Embedding(crate::i18n::validation::embedding_llm_batch_parse_failed(
                e, &stdout,
            ))
        })?;
        if parsed.items.len() != batch.len() {
            return Err(AppError::Embedding(
                crate::i18n::validation::embedding_llm_batch_item_count(
                    parsed.items.len(),
                    batch.len(),
                ),
            ));
        }
        let mut out: Vec<Option<Vec<f32>>> = vec![None; batch.len()];
        for item in parsed.items {
            if item.i == 0 || item.i > batch.len() {
                return Err(AppError::Embedding(
                    crate::i18n::validation::embedding_llm_batch_index_out_of_range(
                        item.i,
                        batch.len(),
                    ),
                ));
            }
            if item.v.len() != dim {
                return Err(AppError::Embedding(
                    crate::i18n::validation::embedding_llm_batch_item_dims(
                        item.i,
                        item.v.len(),
                        dim,
                    ),
                ));
            }
            out[item.i - 1] = Some(item.v);
        }
        let mut result = Vec::with_capacity(batch.len());
        for (pos, slot) in out.into_iter().enumerate() {
            let v = slot.ok_or_else(|| {
                AppError::Embedding(crate::i18n::validation::embedding_llm_batch_missing_item(
                    pos + 1,
                ))
            })?;
            result.push((batch[pos].0, v));
        }
        Ok(result)
    }

    pub(crate) fn invoke_with_prefix(&self, prefix: &str, text: &str) -> Result<Vec<f32>, AppError> {
        let dim = crate::constants::embedding_dim();
        let inner = self.invoke_single_async(prefix, text, dim);
        // v1.0.79 (G42/A2): reuse the process-wide multi-thread runtime
        // instead of building a current-thread runtime PER CALL. Inside
        // an existing runtime (tests, async commands) block_in_place
        // keeps the worker pool healthy.
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(inner)),
            Err(_) => crate::embedder::shared_runtime()?.block_on(inner),
        }
    }

    async fn invoke_single_async(
        &self,
        prefix: &str,
        text: &str,
        dim: usize,
    ) -> Result<Vec<f32>, AppError> {
        let prompt = format!("{prefix}{text}");
        let stdout = match self.flavour {
            EmbeddingFlavour::Claude => {
                self.invoke_claude(&prompt, &build_single_schema(dim))
                    .await?
            }
            EmbeddingFlavour::Codex => {
                let schema = self.codex_schema_file(dim, false)?;
                self.invoke_codex(&prompt, schema.path()).await?
            }
            EmbeddingFlavour::Opencode => {
                let opencode_prompt = format!(
                    "You are an embedding function. Given the input text, output a JSON object \
                     with a single key \"embedding\" containing an array of exactly {dim} \
                     floating-point numbers between -1 and 1 that represent the semantic meaning \
                     of the text. Output ONLY the JSON object, nothing else.\n\n\
                     Input text: \"{prompt}\""
                );
                self.invoke_opencode(&opencode_prompt).await?
            }
        };
        let parsed: EmbeddingResponse = parse_llm_json(&stdout).map_err(|e| {
            AppError::Embedding(crate::i18n::validation::embedding_llm_parse_failed(
                e, &stdout,
            ))
        })?;
        if parsed.embedding.len() != dim {
            return Err(AppError::Embedding(
                crate::i18n::validation::embedding_llm_returned_dims(
                    parsed.embedding.len(),
                    dim,
                ),
            ));
        }
        Ok(parsed.embedding)
    }

    /// G42/S4: returns the lazily-created, process-shared codex schema
    /// tempfile for the requested mode. `NamedTempFile` randomises the
    /// filename (no PID-based collisions) and removes the file on drop
    /// of the last `Arc` clone.
    pub(crate) fn codex_schema_file(
        &self,
        dim: usize,
        batch: bool,
    ) -> Result<Arc<tempfile::NamedTempFile>, AppError> {
        let mut guard = self.codex_schemas.lock();
        let slot = if batch {
            &mut guard.batch
        } else {
            &mut guard.single
        };
        if let Some((cached_dim, file)) = slot {
            if *cached_dim == dim {
                return Ok(Arc::clone(file));
            }
        }
        let content = if batch {
            build_batch_schema(dim)
        } else {
            build_single_schema(dim)
        };
        let file = tempfile::Builder::new()
            .prefix("sqlite-graphrag-embed-schema-")
            .suffix(".json")
            .tempfile()
            .map_err(|e| {
                AppError::Embedding(
                    crate::i18n::validation::embedding_schema_tempfile_create_failed(e),
                )
            })?;
        std::fs::write(file.path(), content).map_err(|e| {
            AppError::Embedding(crate::i18n::validation::embedding_schema_tempfile_write_failed(e))
        })?;
        let file = Arc::new(file);
        *slot = Some((dim, Arc::clone(&file)));
        Ok(file)
    }



    async fn invoke_claude(&self, prompt: &str, schema: &str) -> Result<String, AppError> {
        // v1.0.69 hardening: --strict-mcp-config --mcp-config <PATH> --settings
        // '{"hooks":{}}' --dangerously-skip-permissions.
        //
        // v1.0.76 hardening: Claude Code 2.1+ renamed --output-schema to
        // --json-schema and accepts the schema as an inline JSON string
        // (NOT a file path). Also pass --output-format json so the
        // response is a single JSON object on stdout.
        //
        // v1.0.79 (G42/S6): CLAUDE_CONFIG_DIR points at an empty managed
        // directory BY DEFAULT — the MCP-isolation flags above are
        // silently ignored upstream (anthropics/claude-code#10787) and a
        // populated ~/.claude costs ~223k cache-creation tokens per call.
        //
        // v1.0.88 (BUG-2 fix, ADR-0046): the inline `--mcp-config '{}'`
        // form was rejected by Claude Code 2.1.177 (ADR-0045 Bug 2).
        // Substitute a tempfile path produced by
        // `write_empty_mcp_config_tempfile()` and run the full
        // preflight gate BEFORE `Command::spawn()`, mirroring what
        // `invoke_codex` already does for the codex backend.
        let spawn_dir = crate::spawn::spawn_isolation_dir()?;
        let mcp_config_path = crate::spawn::preflight::write_empty_mcp_config_tempfile()?;
        let argv_refs: [std::ffi::OsString; 0] = [];
        let preflight_args = crate::spawn::preflight::PreFlightArgs {
            binary_path: &self.binary,
            argv: &argv_refs,
            workspace_root: &spawn_dir,
            mcp_config_inline_json: None,
            expected_output_bytes: 65_536,
            spawner_name: "llm_embedding",
        };
        crate::spawn::preflight::preflight_check(&preflight_args)?;
        let mut cmd = Command::new(&self.binary);
        cmd.arg("-p")
            .arg(prompt)
            .arg("--model")
            .arg(&self.model)
            .arg("--json-schema")
            .arg(schema)
            .arg("--output-format")
            .arg("json")
            .arg("--strict-mcp-config")
            .arg("--mcp-config")
            .arg(mcp_config_path.as_os_str())
            .arg("--settings")
            .arg(r#"{"hooks":{}}"#)
            .arg("--dangerously-skip-permissions")
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", std::env::var("HOME").unwrap_or_default())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // BLOCO 4: cancellation (dropped future) must kill the child.
            .kill_on_drop(true);
        // GAP-SPAWN-001: isolate CWD so child never inherits .mcp.json
        cmd.current_dir(&spawn_dir);
        cmd.env("CLAUDE_CONFIG_DIR", &spawn_dir);
        if let Some(config_dir) = claude_embedding_config_dir() {
            cmd.env("CLAUDE_CONFIG_DIR", &config_dir);
        }
        let binary_str = self.binary.to_string_lossy().into_owned();
        let output = match tokio::time::timeout(self.instance_embed_timeout(), cmd.output()).await {
            Err(_elapsed) => {
                return Err(crate::llm::exit_code_hints::into_legacy_embedding(
                    &crate::llm::exit_code_hints::LlmBackendError::Timeout {
                        secs: self.instance_embed_timeout().as_secs(),
                        binary: binary_str.clone(),
                    },
                ));
            }
            Ok(Err(e)) => {
                return Err(crate::llm::exit_code_hints::into_legacy_embedding(
                    &crate::llm::exit_code_hints::LlmBackendError::SpawnFailed {
                        binary: binary_str.clone(),
                        source: e.to_string(),
                    },
                ));
            }
            Ok(Ok(o)) => o,
        };
        // G45-CR5 / ADR-0043 (v1.0.85): parse the JSON envelope from
        // `claude -p --output-format json` and detect OAuth quota
        // exhaustion by looking for the `rate_limit_error` or
        // `usage` overflow markers before checking the subprocess
        // exit status. This lets the deterministic fallback in
        // hybrid-search and recall swap to codex immediately.
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
            let is_rate_limited = parsed
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                && parsed
                    .get("result")
                    .and_then(|v| v.as_str())
                    .map(|s| {
                        s.contains("rate limit")
                            || s.contains("quota")
                            || s.contains("anthropic-ratelimit")
                    })
                    .unwrap_or(false);
            if is_rate_limited {
                let snippet: String = parsed
                    .get("result")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .chars()
                    .take(120)
                    .collect();
                return Err(AppError::Embedding(
                    crate::i18n::validation::embedding_oauth_usage_quota_exhausted_claude(
                        &snippet,
                    ),
                ));
            }
        }
        if !output.status.success() {
            let (exit_code, signal) = if let Some(code) = output.status.code() {
                (Some(code), None)
            } else {
                extract_exit_info(&output.status)
            };
            let stdout_tail = crate::llm::exit_code_hints::LlmBackendError::truncate_tail(
                &output.stdout,
                crate::llm::exit_code_hints::DIAG_TAIL_BYTES,
            );
            let stderr_tail = crate::llm::exit_code_hints::LlmBackendError::truncate_tail(
                &output.stderr,
                crate::llm::exit_code_hints::DIAG_TAIL_BYTES,
            );
            let mut hint = crate::llm::exit_code_hints::diagnose_exit_code(exit_code, signal);
            // v1.0.89 (GAP-5): detect expired OAuth and suggest actionable fix.
            if stderr_tail.contains("401")
                || stderr_tail.contains("Unauthorized")
                || stderr_tail.contains("expired")
                || stderr_tail.contains("login")
                || stdout_tail.contains("401")
                || stdout_tail.contains("Unauthorized")
            {
                hint.push_str(" | Claude OAuth token may be expired; run `claude login` to renew");
            }
            return Err(crate::llm::exit_code_hints::into_legacy_embedding(
                &crate::llm::exit_code_hints::LlmBackendError::NonZeroExit {
                    exit_code,
                    signal,
                    stdout_tail,
                    stderr_tail,
                    binary: binary_str,
                    hint,
                },
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    async fn invoke_codex(
        &self,
        prompt: &str,
        schema_path: &std::path::Path,
    ) -> Result<String, AppError> {
        let binary_str = self.binary.to_string_lossy().into_owned();
        let mut cmd = build_codex_embedding_command(&self.binary, &self.model, schema_path)?;

        // GAP-META-005 (v1.0.87, ADR-0045): pre-flight gate before spawn.
        // `tokio::process::Command` does not expose `get_args()`, so we
        // skip the argv-size check here and rely on binary + workspace
        // root + output buffer guards. Embedding prompts are bounded by
        // the schema validator so argv overflow is not a real risk here.
        //
        // v1.0.88 (BUG-7 fix, ADR-0046): propagate the preflight error
        // directly via `AppError::PreFlightFailed` (via the `From`
        // impl added in `errors.rs`) so callers and operators see the
        // structured `PreFlightError` variant and the canonical exit
        // code 16. The previous implementation wrapped the error in
        // `LlmBackendError::SpawnFailed`, which mapped to a different
        // exit code and masked the preflight signal.
        let argv_refs: [std::ffi::OsString; 0] = [];
        let preflight_args = crate::spawn::preflight::PreFlightArgs {
            binary_path: &self.binary,
            argv: &argv_refs,
            workspace_root: std::path::Path::new("."),
            mcp_config_inline_json: None,
            expected_output_bytes: 65_536,
            spawner_name: "llm_embedding",
        };
        crate::spawn::preflight::preflight_check(&preflight_args)?;
        let _ = binary_str; // silenced: preflight does not need it

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Err(crate::llm::exit_code_hints::into_legacy_embedding(
                    &crate::llm::exit_code_hints::LlmBackendError::SpawnFailed {
                        binary: binary_str,
                        source: e.to_string(),
                    },
                ));
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .await
                .map_err(|e| {
                    AppError::Embedding(
                        crate::i18n::validation::embedding_codex_stdin_write_failed(e),
                    )
                })?;
            drop(stdin);
        }
        let output =
            match tokio::time::timeout(self.instance_embed_timeout(), child.wait_with_output())
                .await
            {
                Err(_elapsed) => {
                    return Err(crate::llm::exit_code_hints::into_legacy_embedding(
                        &crate::llm::exit_code_hints::LlmBackendError::Timeout {
                            secs: self.instance_embed_timeout().as_secs(),
                            binary: binary_str,
                        },
                    ));
                }
                Ok(Err(e)) => {
                    return Err(crate::llm::exit_code_hints::into_legacy_embedding(
                        &crate::llm::exit_code_hints::LlmBackendError::SpawnFailed {
                            binary: binary_str,
                            source: format!("codex wait failed: {e}"),
                        },
                    ));
                }
                Ok(Ok(o)) => o,
            };
        if !output.status.success() {
            let (exit_code, signal) = if let Some(code) = output.status.code() {
                (Some(code), None)
            } else {
                extract_exit_info(&output.status)
            };
            let stdout_tail = crate::llm::exit_code_hints::LlmBackendError::truncate_tail(
                &output.stdout,
                crate::llm::exit_code_hints::DIAG_TAIL_BYTES,
            );
            let stderr_tail = crate::llm::exit_code_hints::LlmBackendError::truncate_tail(
                &output.stderr,
                crate::llm::exit_code_hints::DIAG_TAIL_BYTES,
            );
            let hint = crate::llm::exit_code_hints::diagnose_exit_code(exit_code, signal);
            // G42/S7: the headless spawn can still hit interactive
            // prompts on some codex builds; keep the legacy request_user_input
            // branch as a special-case hint, and stamp the diagnostic
            // tail on top of the canonical NonZeroExit envelope.
            let mut combined_hint = hint;
            if stderr_tail.contains("request_user_input") {
                combined_hint.push_str(
                    " | codex requested interactive input in a headless embedding call; \
                     upgrade codex (>= 0.134) or switch the embedding backend to claude",
                );
            }
            return Err(crate::llm::exit_code_hints::into_legacy_embedding(
                &crate::llm::exit_code_hints::LlmBackendError::NonZeroExit {
                    exit_code,
                    signal,
                    stdout_tail,
                    stderr_tail,
                    binary: binary_str,
                    hint: combined_hint,
                },
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    async fn invoke_opencode(&self, prompt: &str) -> Result<String, AppError> {
        let binary_str = self.binary.to_string_lossy().into_owned();
        let spawn_dir = crate::spawn::spawn_isolation_dir()?;
        let mut cmd = Command::new(&self.binary);
        cmd.current_dir(&spawn_dir);
        cmd.arg("run")
            .arg("--format")
            .arg("json")
            .arg("-m")
            .arg(&self.model)
            .arg("--dangerously-skip-permissions")
            .arg(prompt)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", std::env::var("HOME").unwrap_or_default())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        crate::commands::opencode_runner::propagate_opencode_env(&mut cmd);

        let output = match tokio::time::timeout(self.instance_embed_timeout(), cmd.output()).await {
            Err(_elapsed) => {
                return Err(crate::llm::exit_code_hints::into_legacy_embedding(
                    &crate::llm::exit_code_hints::LlmBackendError::Timeout {
                        secs: self.instance_embed_timeout().as_secs(),
                        binary: binary_str.clone(),
                    },
                ));
            }
            Ok(Err(e)) => {
                return Err(crate::llm::exit_code_hints::into_legacy_embedding(
                    &crate::llm::exit_code_hints::LlmBackendError::SpawnFailed {
                        binary: binary_str.clone(),
                        source: e.to_string(),
                    },
                ));
            }
            Ok(Ok(o)) => o,
        };
        if !output.status.success() {
            let (exit_code, signal) = if let Some(code) = output.status.code() {
                (Some(code), None)
            } else {
                extract_exit_info(&output.status)
            };
            let stdout_tail = crate::llm::exit_code_hints::LlmBackendError::truncate_tail(
                &output.stdout,
                crate::llm::exit_code_hints::DIAG_TAIL_BYTES,
            );
            let stderr_tail = crate::llm::exit_code_hints::LlmBackendError::truncate_tail(
                &output.stderr,
                crate::llm::exit_code_hints::DIAG_TAIL_BYTES,
            );
            let hint = crate::llm::exit_code_hints::diagnose_exit_code(exit_code, signal);
            return Err(crate::llm::exit_code_hints::into_legacy_embedding(
                &crate::llm::exit_code_hints::LlmBackendError::NonZeroExit {
                    exit_code,
                    signal,
                    stdout_tail,
                    stderr_tail,
                    binary: binary_str,
                    hint,
                },
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

}

/// G42/S6: resolves the empty `CLAUDE_CONFIG_DIR` used for embedding
/// subprocesses.
///
/// - XDG `llm.claude_empty_config_dir` is honoured when set and
///   pointing at a directory (same contract as G28-A in claude_runner);
/// - otherwise a managed directory is created at
///   `~/.local/state/sqlite-graphrag/claude-empty-config` (mode 0700).
///   If `~/.claude/.credentials.json` exists (Linux OAuth storage) it is
///   copied in so authentication still works; on macOS credentials live
///   in the Keychain and the empty dir is sufficient.
///
/// Returns `None` only when HOME is unset AND no override is given —
/// in that case the subprocess falls back to claude's own default.
pub(super) fn claude_embedding_config_dir() -> Option<std::path::PathBuf> {
    if let Ok(Some(dir)) = crate::config::get_setting("llm.claude_empty_config_dir") {
        let path = std::path::PathBuf::from(dir);
        if path.is_dir() {
            return Some(path);
        }
        tracing::warn!(
            target: "embedding",
            path = %path.display(),
            "llm.claude_empty_config_dir is set but not a directory; \
             falling back to the managed empty config dir"
        );
    }
    let home = std::env::var("HOME").ok()?;
    let dir = std::path::Path::new(&home)
        .join(".local/state/sqlite-graphrag")
        .join("claude-empty-config");
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    // Linux stores OAuth credentials on disk; copy them so the isolated
    // config dir still authenticates. Best-effort: macOS uses Keychain.
    // v1.0.89: ALWAYS copy (was: skip if target exists). OAuth tokens
    // expire and the stale copy causes 401 until manually deleted.
    let creds = std::path::Path::new(&home).join(".claude/.credentials.json");
    if creds.exists() {
        let target = dir.join(".credentials.json");
        let _ = std::fs::copy(&creds, &target);
    }
    Some(dir)
}

pub(crate) fn build_codex_embedding_command(
    binary: &std::path::Path,
    model: &str,
    schema_path: &std::path::Path,
) -> Result<Command, AppError> {
    let spawn_dir = crate::spawn::spawn_isolation_dir()?;
    let mut cmd = Command::new(binary);
    cmd.current_dir(&spawn_dir);
    cmd.arg("exec")
        .arg("-c")
        .arg("sandbox_mode='read-only'")
        .arg("-c")
        .arg("approval_policy='never'")
        .arg("--json")
        .arg("--output-schema")
        .arg(schema_path)
        .arg("--ephemeral")
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("read-only")
        .arg("--ignore-user-config")
        .arg("--ignore-rules");
    if crate::extract::codex_compat::codex_supports_ask_for_approval() {
        cmd.arg("--ask-for-approval").arg("never");
    }
    // v1.0.89: use the real CODEX_HOME (~/.codex) instead of an isolated
    // per-PID directory. The isolated dir caused cold-start overhead (codex
    // creates ~6 SQLite databases on first run) that regularly exceeded
    // the 30s embedding timeout. The --ignore-user-config + --ephemeral
    // flags already prevent config pollution; CODEX_HOME only needs auth.
    cmd.arg("--model")
        .arg(model)
        .arg("-")
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", std::env::var("HOME").unwrap_or_default());
    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        cmd.env("CODEX_HOME", codex_home);
    } else if let Ok(home) = std::env::var("HOME") {
        let default_home = std::path::Path::new(&home).join(".codex");
        if default_home.exists() {
            cmd.env("CODEX_HOME", &default_home);
        }
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // BLOCO 4: cancellation (dropped future) must kill the child.
        .kill_on_drop(true);
    Ok(cmd)
}

// prepare_isolated_codex_home removed in v1.0.89: the per-PID isolated
// CODEX_HOME caused cold-start overhead that exceeded the 30s embedding
// timeout. The real ~/.codex is now used directly (see build_codex_embedding_command).
