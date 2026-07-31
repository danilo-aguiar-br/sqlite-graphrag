//! Codex CLI spawn + JSONL parsing helper shared by `enrich` and `ingest --mode codex`.
//!
//! G31 (v1.0.69): `enrich --mode codex` was missing five critical hardening
//! flags compared to `ingest --mode codex`. This module extracts the
//! spawn pipeline into a single helper that BOTH call-sites consume,
//! guaranteeing the same defaults everywhere.
//!
//! G32 (v1.0.69): `enrich --mode codex` used `serde_json::from_str` on the
//! raw stdout, but `codex exec --json` emits JSONL (one event per line).
//! [`parse_codex_jsonl`] iterates line-by-line, picking the last
//! `item.completed` of type `agent_message` as the assistant text.
//!
//! G33 (v1.0.69): validate the model against the ChatGPT Pro OAuth whitelist
//! stored in `~/.codex/models_cache.json` BEFORE spawning the subprocess.

use crate::errors::AppError;
use crate::extract::codex_compat::codex_supports_ask_for_approval;
use crate::extraction::{ExtractedUrl, ExtractionResult};
use crate::spawn::env_whitelist::apply_env_whitelist;
use crate::storage::entities::{NewEntity, NewRelationship};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Token usage reported by Codex on `turn.completed` events.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CodexUsage {
    /// Input tokens.
    #[serde(default)]
    pub input_tokens: u64,
    /// Cached input tokens.
    #[serde(default)]
    pub cached_input_tokens: u64,
    /// Output tokens.
    #[serde(default)]
    pub output_tokens: u64,
    /// Reasoning output tokens.
    #[serde(default)]
    pub reasoning_output_tokens: u64,
}

/// Combined result of one `codex exec` invocation.
#[derive(Debug)]
pub struct CodexResult {
    /// Extraction.
    pub extraction: ExtractionResult,
    /// Raw text of the last `item.completed` of type `agent_message` (the
    /// JSON payload the LLM produced). Callers that need a schema other
    /// than the extraction shape (e.g. body-enrich's `enriched_body`)
    /// should parse this directly.
    pub last_agent_text: String,
    /// Usage.
    pub usage: Option<CodexUsage>,
    /// Rate limited.
    pub rate_limited: bool,
    /// Schema error.
    pub schema_error: bool,
    /// Turn failed.
    pub turn_failed: bool,
    /// Failed message.
    pub failed_message: String,
}

/// Configuration for the codex spawner.
#[allow(rustdoc::broken_intra_doc_links)]
pub struct CodexSpawnArgs<'a> {
    /// Binary.
    pub binary: &'a Path,
    /// Prompt.
    pub prompt: &'a str,
    /// JSON schema.
    pub json_schema: &'a str,
    /// Input text.
    pub input_text: &'a str,
    /// Model name or identifier.
    pub model: Option<&'a str>,
    /// Timeout secs.
    pub timeout_secs: u64,
    /// Caller-provided schema path (must be inside a trusted directory
    /// that codex recognises as sandbox-safe). Use [`trusted_schema_path`]
    /// to compute one under the cache dir.
    pub schema_path: PathBuf,
}

/// Computes a schema path under the cache dir so `codex exec` accepts it
/// as part of a trusted directory (rejects `/tmp` on hardened installs).
pub fn trusted_schema_path() -> Result<PathBuf, AppError> {
    let cache = crate::paths::AppPaths::resolve(None)
        .map(|p| p.models.parent().map(|m| m.to_path_buf()))
        .ok()
        .flatten()
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&cache).map_err(AppError::Io)?;
    Ok(cache.join(format!("enrich-schema-{}.json", std::process::id())))
}

/// Models accepted by Codex CLI when using ChatGPT Pro OAuth.
///
/// Mirrored from `~/.codex/models_cache.json` (which the official CLI
/// refreshes on every login). This list is intentionally narrow; passing
/// a model not in this set with `--mode codex` returns
/// `AppError::Validation` BEFORE any OAuth turn is spent.
pub const CODEX_PRO_OAUTH_MODELS: &[&str] = &[
    "codex-auto-review",
    "gpt-5.3-codex-spark",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.5",
];

/// Validates the requested model against [`CODEX_PRO_OAUTH_MODELS`].
///
/// # Errors
/// Returns [`AppError::Validation`] listing the accepted models when the
/// caller supplied a model outside the whitelist.
pub fn validate_codex_model(model: Option<&str>) -> Result<(), AppError> {
    let Some(m) = model else {
        return Ok(()); // no override; codex picks its default
    };
    if CODEX_PRO_OAUTH_MODELS.contains(&m) {
        Ok(())
    } else {
        Err(AppError::Validation(
            crate::i18n::validation::codex_model_not_supported_oauth(
                m,
                &CODEX_PRO_OAUTH_MODELS.join(", "),
            ),
        ))
    }
}

/// Returns the list of models accepted by Codex with ChatGPT Pro OAuth.
///
/// Tries to read `~/.codex/models_cache.json` (which the official CLI
/// refreshes on every login) and falls back to the static
/// [`CODEX_PRO_OAUTH_MODELS`] constant when the file is missing or
/// malformed. The returned `Vec<String>` is the union of both sources,
/// de-duplicated.
///
/// The official cache file is an object with the shape
/// `{"fetched_at": "...", "etag": "...", "client_version": "...",
/// "models": [{"slug": "gpt-5.5", ...}, ...]}` (v1.0.81 fix: previously we
/// iterated `obj.keys()` which produced bogus entries like `client_version`
/// and `etag` as "models"; now we extract only the `models` array).
pub fn list_codex_models() -> Vec<String> {
    use std::collections::BTreeSet;
    let mut out: BTreeSet<String> = CODEX_PRO_OAUTH_MODELS
        .iter()
        .map(|s| s.to_string())
        .collect();

    if let Some(home) = std::env::var_os("HOME") {
        let path = std::path::Path::new(&home)
            .join(".codex")
            .join("models_cache.json");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(obj) = value.as_object() {
                    // v1.0.81 fix: prefer the well-known `models` array
                    // (each item has a `slug` field). Fall back to keys
                    // only when `models` is absent (legacy cache format).
                    if let Some(models_arr) = obj.get("models").and_then(|m| m.as_array()) {
                        for v in models_arr {
                            if let Some(slug) = v.get("slug").and_then(|s| s.as_str()) {
                                out.insert(slug.to_string());
                            } else if let Some(s) = v.as_str() {
                                out.insert(s.to_string());
                            }
                        }
                    } else {
                        for key in obj.keys() {
                            out.insert(key.clone());
                        }
                    }
                } else if let Some(arr) = value.as_array() {
                    for v in arr {
                        if let Some(s) = v.as_str() {
                            out.insert(s.to_string());
                        }
                    }
                }
            }
        }
    }
    out.into_iter().collect()
}

/// Suggests the closest codex OAuth model to a user-supplied substring
/// (G33). Returns `None` when no candidate is close enough.
///
/// Match strategy: exact substring containment wins; otherwise Levenshtein
/// distance below `max_distance = max(2, query.len() / 3)`.
pub fn suggest_codex_model(query: &str) -> Option<String> {
    let query_lc = query.to_ascii_lowercase();
    let models = list_codex_model_lc();

    // Exact substring match wins.
    for m in &models {
        if m.contains(&query_lc) {
            return Some(m.clone());
        }
    }

    // Levenshtein fallback.
    let max_distance = (query.len() / 3).max(2);
    let mut best: Option<(usize, String)> = None;
    for m in &models {
        let d = levenshtein(query_lc.as_str(), m.as_str());
        if d <= max_distance && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
            best = Some((d, m.clone()));
        }
    }
    best.map(|(_, m)| m)
}

fn list_codex_model_lc() -> Vec<String> {
    list_codex_models()
        .into_iter()
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    if a_chars.is_empty() {
        return b_chars.len();
    }
    if b_chars.is_empty() {
        return a_chars.len();
    }
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr = vec![0; b_chars.len() + 1];
    for (i, &ac) in a_chars.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &bc) in b_chars.iter().enumerate() {
            let cost = if ac == bc { 0 } else { 1 };
            curr[j + 1] = (curr[j] + 1).min(prev[j + 1] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_chars.len()]
}

/// Builds the `codex exec` command with the canonical hardening flags.
///
/// G31 + OAuth-only hardening (v1.0.69, mandated by gaps.md lines 41-49):
/// the command ALWAYS uses the OAuth `auth.json` flow. The flag set is
/// the canonical one documented in gaps.md Fix A:
///
/// ```text
/// codex exec \
///   -c mcp_servers='{}' \
///   --json --output-schema <SCHEMA> \
///   --ephemeral \
///   --skip-git-repo-check \
///   --sandbox read-only \
///   --ignore-user-config \
///   --ignore-rules \
///   --ask-for-approval never \
///   -m <MODEL> \
///   -
/// ```
///
/// The combination zeroes MCP servers (via two complementary mechanisms:
/// the inline `-c mcp_servers='{}'` override AND `--ignore-user-config`),
/// disables user-defined rules, and never asks for interactive approval.
///
/// **`OPENAI_API_KEY` is FORBIDDEN** in the spawned environment (gaps.md:48).
/// OAuth flows via `~/.codex/auth.json` and `CODEX_ACCESS_TOKEN` only.
pub fn build_codex_command(args: &CodexSpawnArgs<'_>) -> Result<Command, crate::errors::AppError> {
    let full_prompt = format!("{}\n\n{}", args.prompt, args.input_text);

    // OAuth-only guard (gaps.md:48, ADR-0011). If `OPENAI_API_KEY` is set
    // in the environment we MUST abort — that is the API-key path which is
    // explicitly PROHIBITED. Use the OAuth `auth.json` flow exclusively.
    if let Ok(_key) = std::env::var("OPENAI_API_KEY") {
        let mut cmd = crate::spawn::failing_command();
        cmd.env_clear();
        cmd.env("PATH", "/nonexistent");
        cmd.arg("--oauth-only-violation-openai-api-key-set");
        cmd.arg("--oauth-only-resolution-use-codex-auth-json-or-openai-base-url");
        return Ok(cmd);
    }

    // Write the JSON schema to a path the caller controls. Callers should
    // pass a path under the cache dir (see [`trusted_schema_path`]).
    std::fs::write(&args.schema_path, args.json_schema).ok();

    let mut cmd = Command::new(args.binary);
    // v1.0.83 (ADR-0041): env whitelist delegated to the shared helper.
    // `OPENAI_API_KEY` is INTENTIONALLY ABSENT (defence-in-depth).
    // `CODEX_ACCESS_TOKEN` and `OPENAI_BASE_URL` ARE whitelisted for
    // custom providers via the canonical list in src/spawn/env_whitelist.rs.
    apply_env_whitelist(&mut cmd, crate::spawn::env_whitelist::is_strict_env_clear());
    crate::spawn::apply_cwd_isolation(&mut cmd)?;

    // v1.0.77: point CODEX_HOME at an isolated dir that only contains
    // auth.json — this prevents the codex subprocess from loading
    // ~/.codex/config.toml (which has trust_level=trusted for the project,
    // causing sandbox escalation per openai/codex#18113).
    if let Some(isolated) = prepare_isolated_codex_home_spawn() {
        cmd.env("CODEX_HOME", isolated);
    }

    // v1.0.77: `-c` TOML overrides bypass the codex exec --sandbox propagation
    // bug (openai/codex#18113). CLI flags alone are insufficient — the exec
    // subcommand may not inherit --sandbox from the parent codex command.
    cmd.arg("exec")
        .arg("-c")
        .arg("sandbox_mode='read-only'")
        .arg("-c")
        .arg("approval_policy='never'")
        .arg("--json")
        .arg("--output-schema")
        .arg(&args.schema_path)
        .arg("--ephemeral")
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("read-only")
        .arg("--ignore-user-config")
        .arg("--ignore-rules");

    // Codex 0.134+ no longer accepts `-c mcp_servers='{}'` — it parses the
    // value as a string and rejects it ("expected a map"). The
    // `--ignore-user-config` flag already discards any user-defined MCP
    // servers, so the override is redundant on all supported versions.

    // Codex 0.134+ removed --ask-for-approval entirely (Issue #26602).
    // Skip the flag on newer versions; sandbox=read-only already suppresses
    // approval prompts. See src/extract/codex_compat.rs for the probe.
    if codex_supports_ask_for_approval() {
        cmd.arg("--ask-for-approval").arg("never");
    }

    if let Some(m) = args.model {
        cmd.arg("-m").arg(m);
    }

    // `-` means: read the prompt from stdin (Codex Paperclip pattern)
    cmd.arg("-");

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Keep the prompt alive for the stdin thread spawned in `spawn_codex`.
    let _ = full_prompt; // captured by closure below

    // GAP-META-005 (v1.0.87, ADR-0045): pre-flight validation gate runs
    // AFTER argv is fully built. Validates binary existence, argv size,
    // walk-up of `.mcp.json`, and `CLAUDE_CONFIG_DIR` cleanliness.
    // Pre-flight failure aborts the spawn with exit 16 — see ADR-0045.
    let argv_refs: Vec<std::ffi::OsString> = cmd.get_args().map(|s| s.to_os_string()).collect();
    let preflight_args = crate::spawn::preflight::PreFlightArgs {
        binary_path: args.binary,
        argv: &argv_refs,
        workspace_root: std::path::Path::new("."),
        mcp_config_inline_json: None, // Codex does not use --mcp-config flag
        expected_output_bytes: 65_536,
        spawner_name: "codex_spawn",
    };
    if let Err(e) = crate::spawn::preflight::preflight_check(&preflight_args) {
        // v1.0.88 (BUG-6 fix, ADR-0046): propagate the structured
        // `PreFlightError` via the `From` impl in `errors.rs` so callers
        // receive `AppError::PreFlightFailed` (exit 16) instead of a
        // bare `std::process::exit(16)` that discards the variant name,
        // tracing context, and PT-BR i18n.
        return Err(crate::errors::AppError::from(e));
    }

    Ok(cmd)
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
/// G32 (v1.0.69): this function is the single source of truth for JSONL
/// parsing. Both `enrich` and `ingest --mode codex` consume it.
pub fn parse_codex_jsonl(stdout: &str) -> Result<CodexResult, AppError> {
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
                tracing::warn!(target: "codex_spawn", line, "skipping malformed JSONL line");
                continue;
            }
        };

        let event_type = match event.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => continue,
        };

        match event_type {
            "item.completed" => {
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
                    // Skip events that lack the recognised token fields
                    // (e.g. partial broadcasts with `{}`) so the last
                    // populated usage wins instead of being overwritten
                    // by an empty one.
                    let is_populated = u
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .map(|n| n > 0)
                        .unwrap_or(false)
                        || u.get("output_tokens")
                            .and_then(|v| v.as_u64())
                            .map(|n| n > 0)
                            .unwrap_or(false);
                    if is_populated {
                        if let Ok(parsed) = serde_json::from_value::<CodexUsage>(u.clone()) {
                            usage = Some(parsed);
                        }
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
                }
            }
            _ => {}
        }
    }

    let text = last_agent_text.ok_or_else(|| {
        AppError::Validation(crate::i18n::validation::no_agent_message_in_codex_jsonl(
            rate_limited,
            schema_error,
            turn_failed,
        ))
    })?;

    if turn_failed {
        return Err(AppError::Validation(
            crate::i18n::validation::codex_turn_failed(&failed_message),
        ));
    }
    if schema_error {
        return Err(AppError::Validation(
            "codex reported invalid_json_schema; check the --output-schema file".to_string(),
        ));
    }
    if rate_limited {
        return Err(AppError::Validation(
            crate::i18n::validation::codex_rate_limited(&failed_message),
        ));
    }

    let extraction = parse_extraction_text(&text)?;
    Ok(CodexResult {
        extraction,
        last_agent_text: text,
        usage,
        rate_limited,
        schema_error,
        turn_failed,
        failed_message,
    })
}

/// Parses the agent_message text as an `ExtractionResult` JSON payload.
///
/// The schema is shared by both `enrich` and `ingest --mode codex`; the
/// `text` is the JSON value the assistant returned, not a wrapper object.
pub fn parse_extraction_text(text: &str) -> Result<ExtractionResult, AppError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| {
        AppError::Validation(crate::i18n::validation::failed_to_parse_codex_agent_message_json(&e))
    })?;
    let obj = value.as_object().ok_or_else(|| {
        AppError::Validation(crate::i18n::validation::codex_agent_message_not_json_object())
    })?;

    let mut entities: Vec<NewEntity> = Vec::new();
    if let Some(arr) = obj.get("entities").and_then(|v| v.as_array()) {
        for e in arr {
            if let Some(name) = e.get("name").and_then(|v| v.as_str()) {
                // Accept either "type" or "entity_type" from the LLM payload
                // and fall back to "concept" when the LLM omits it.
                let entity_type_str = e
                    .get("type")
                    .or_else(|| e.get("entity_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("concept");
                // GAP-SG-47: fold non-canonical labels onto the nearest
                // canonical kind (preserves aliases/case instead of collapsing
                // every miss straight to concept).
                let entity_type = crate::entity_type::EntityType::map_to_canonical(entity_type_str);
                entities.push(NewEntity {
                    name: name.to_string(),
                    entity_type,
                    description: None,
                });
            }
        }
    }

    let mut relationships: Vec<NewRelationship> = Vec::new();
    if let Some(arr) = obj.get("relationships").and_then(|v| v.as_array()) {
        for r in arr {
            let from = r.get("source").or_else(|| r.get("from"));
            let to = r.get("target").or_else(|| r.get("to"));
            let rel = r.get("relation").and_then(|v| v.as_str());
            if let (Some(from_v), Some(to_v), Some(rel_v)) = (
                from.and_then(|v| v.as_str()),
                to.and_then(|v| v.as_str()),
                rel,
            ) {
                relationships.push(NewRelationship {
                    source: from_v.to_string(),
                    target: to_v.to_string(),
                    // GAP-SG-48: rewrite non-canonical relations to canonical.
                    relation: crate::parsers::map_to_canonical_relation(rel_v),
                    strength: r.get("strength").and_then(|v| v.as_f64()).unwrap_or(0.5),
                    description: None,
                });
            }
        }
    }

    let urls: Vec<ExtractedUrl> = obj
        .get("urls")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|u| {
                    let url = u.get("url")?.as_str()?.to_string();
                    let start = u.get("start").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let end = u
                        .get("end")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(start as u64) as usize;
                    Some(ExtractedUrl { url, start, end })
                })
                .collect()
        })
        .unwrap_or_default();

    // v1.0.76: ExtractionResult no longer carries relationships or
    // relationships_truncated fields; those are LLM backend output
    // (see `ExtractionOutput` in src/extract/mod.rs). The default
    // build extracts URLs + entities only; relationships are an
    // LLM-side concern.
    //
    // Convert `NewEntity` (storage-side) to `ExtractedEntity`
    // (extraction-side). The LLM payload doesn't include byte offsets
    // (the chunker is responsible for that), so start/end are 0.
    let entities_ext: Vec<crate::extraction::ExtractedEntity> = entities
        .into_iter()
        .map(|e| crate::extraction::ExtractedEntity {
            name: e.name,
            entity_type: e.entity_type.as_str().to_string(),
            start: 0,
            end: 0,
        })
        .collect();

    Ok(ExtractionResult {
        entities: entities_ext,
        urls,
        elapsed_ms: 0,
    })
}

fn prepare_isolated_codex_home_spawn() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let real_auth = std::path::Path::new(&home).join(".codex/auth.json");
    if !real_auth.exists() {
        return None;
    }
    let isolated =
        std::env::temp_dir().join(format!("sqlite-graphrag-codex-home-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&isolated);
    let target = isolated.join("auth.json");
    if !target.exists() {
        let _ = std::fs::copy(&real_auth, &target);
    }
    Some(isolated)
}
#[cfg(test)]
#[path = "codex_spawn_tests.rs"]
mod tests;
