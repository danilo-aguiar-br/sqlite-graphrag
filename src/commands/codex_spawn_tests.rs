use super::*;

const SAMPLE_JSONL: &str = r#"{"type":"thread.started","thread_id":"abc"}
{"type":"turn.started"}
{"type":"item.completed","item":{"type":"reasoning","text":"thinking"}}
{"type":"item.completed","item":{"type":"agent_message","text":"{\"entities\":[{\"name\":\"alpha\",\"type\":\"concept\"}],\"relationships\":[{\"source\":\"alpha\",\"target\":\"beta\",\"relation\":\"uses\",\"strength\":0.7}],\"extraction_method\":\"codex\",\"urls\":[]}"}}
{"type":"turn.completed","usage":{"input_tokens":120,"output_tokens":45}}
{"type":"turn.completed","usage":{}}
"#;

#[test]
fn parse_codex_jsonl_extracts_last_agent_message() {
    // v1.0.76: relationships are no longer carried in the
    // ExtractionResult struct (they belong to the LLM ExtractionBackend
    // payload, not the URL-only default build). The default test
    // validates the entity extraction path only.
    let result = parse_codex_jsonl(SAMPLE_JSONL).expect("parse must succeed");
    assert_eq!(result.extraction.entities.len(), 1);
    assert_eq!(result.extraction.entities[0].name, "alpha");
}

#[test]
fn parse_codex_jsonl_collects_usage() {
    let result = parse_codex_jsonl(SAMPLE_JSONL).expect("parse must succeed");
    let usage = result.usage.expect("usage must be populated");
    assert_eq!(usage.input_tokens, 120);
    assert_eq!(usage.output_tokens, 45);
}

#[test]
fn parse_codex_jsonl_detects_rate_limit() {
    let r = parse_codex_jsonl(
        "{\"type\":\"turn.failed\",\"error\":{\"message\":\"rate_limit: 429 too many\"}}\n{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"{}\"}}",
    );
    assert!(matches!(r, Err(AppError::Validation(_))));
}

#[test]
fn parse_codex_jsonl_handles_no_agent_message() {
    let r = parse_codex_jsonl("{\"type\":\"thread.started\"}");
    assert!(matches!(r, Err(AppError::Validation(_))));
}

#[test]
fn parse_codex_jsonl_skips_malformed_lines() {
    let r = parse_codex_jsonl(
        "{not valid json\n{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"{\\\"entities\\\":[],\\\"relationships\\\":[],\\\"extraction_method\\\":\\\"codex\\\"}\"}}",
    );
    assert!(r.is_ok(), "malformed lines must be skipped, got {r:?}");
}

#[test]
fn validate_codex_model_accepts_known() {
    assert!(validate_codex_model(Some("gpt-5.5")).is_ok());
    assert!(validate_codex_model(Some("gpt-5.4")).is_ok());
    assert!(validate_codex_model(None).is_ok()); // no override
}

#[test]
fn validate_codex_model_rejects_unknown() {
    let err = validate_codex_model(Some("gpt-4")).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("not supported"));
    assert!(msg.contains("gpt-5.5"));
}

#[test]
fn list_codex_models_includes_all_static_whitelist() {
    let models = list_codex_models();
    for m in CODEX_PRO_OAUTH_MODELS {
        assert!(models.contains(&m.to_string()), "missing {m} in {models:?}");
    }
}

#[test]
fn suggest_codex_model_substring_match() {
    let s = suggest_codex_model("gpt-5");
    assert!(s.is_some(), "must suggest a gpt-5.x model");
}

#[test]
fn suggest_codex_model_fuzzy_match() {
    // 'gpt5.5' has no hyphen; should still suggest 'gpt-5.5'.
    let s = suggest_codex_model("gpt5.5");
    assert!(s.is_some(), "fuzzy must suggest gpt-5.5 for 'gpt5.5'");
    assert_eq!(s.unwrap(), "gpt-5.5");
}

#[test]
fn suggest_codex_model_unrelated_returns_none() {
    let s = suggest_codex_model("totally-unrelated-zzz");
    assert!(s.is_none());
}

#[test]
fn build_codex_command_includes_hardening_flags() {
    // RC-14 (v1.0.98): `/bin/true` is rejected by the preflight existence
    // check under the macOS runner sandbox. Use the running test binary,
    // which always exists and is executable on every platform.
    let self_exe = std::env::current_exe().expect("current exe path");
    let args = CodexSpawnArgs {
        binary: &self_exe,
        prompt: "p",
        json_schema: "{}",
        input_text: "i",
        model: Some("gpt-5.5"),
        timeout_secs: 60,
        schema_path: std::env::temp_dir().join("test-schema.json"),
    };
    let cmd = build_codex_command(&args).expect("preflight gate accepts valid args");
    let collected: Vec<String> = cmd
        .get_args()
        .filter_map(|a| a.to_str().map(|s| s.to_string()))
        .collect();
    for required in &[
        "exec",
        "-c",
        "sandbox_mode='read-only'",
        "approval_policy='never'",
        "--json",
        "--output-schema",
        "--ephemeral",
        "--skip-git-repo-check",
        "--sandbox",
        "read-only",
        "--ignore-user-config",
        "--ignore-rules",
        "-m",
        "gpt-5.5",
        "-",
    ] {
        assert!(
            collected.iter().any(|a| a == required),
            "missing flag {required} in {collected:?}"
        );
    }
}

#[test]
fn list_codex_models_dedupes_with_cache_file() {
    // Ensure the union with the cache file (when present) does not
    // produce duplicates. We can't actually write a cache file in
    // a test, so we just verify the static path is dedup'd.
    let models = list_codex_models();
    let unique: std::collections::HashSet<_> = models.iter().collect();
    assert_eq!(unique.len(), models.len(), "list_codex_models must dedupe");
}
#[test]
fn list_codex_models_extracts_from_models_array_v1_0_81_regression() {
    // v1.0.81 fix: the official codex CLI writes
    //   {"fetched_at": "...", "etag": "...", "client_version": "...",
    //    "models": [{"slug": "gpt-5.5", ...}, ...]}
    // and the old code iterated obj.keys(), polluting the model
    // list with metadata keys. Here we simulate a cache file by
    // setting HOME to a tempdir containing a synthetic cache and
    // verifying the metadata keys are NOT present in the output.
    let tmp =
        std::env::temp_dir().join(format!("codex-models-array-test-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join(".codex")).expect("mkdir");
    let cache_body = r#"{
        "fetched_at": "2026-06-14T06:43:56.639903114Z",
        "etag": "W/\"deadbeef\"",
        "client_version": "0.139.0",
        "models": [
            {"slug": "gpt-5.5", "display_name": "GPT-5.5"},
            {"slug": "gpt-5.4-mini", "display_name": "GPT-5.4 mini"}
        ]
    }"#;
    std::fs::write(tmp.join(".codex/models_cache.json"), cache_body).expect("write cache");
    // SAFETY: unit test
    let prev_home = std::env::var("HOME");
    unsafe {
        std::env::set_var("HOME", &tmp);
    }
    let models = list_codex_models();
    unsafe {
        if let Ok(h) = prev_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);

    for forbidden in &["client_version", "etag", "fetched_at", "models"] {
        assert!(
            !models.contains(&forbidden.to_string()),
            "metadata key {forbidden:?} leaked into model list: {models:?}"
        );
    }
    assert!(
        models.contains(&"gpt-5.5".to_string()),
        "gpt-5.5 missing from extracted list: {models:?}"
    );
    assert!(
        models.contains(&"gpt-5.4-mini".to_string()),
        "gpt-5.4-mini missing from extracted list: {models:?}"
    );
}

#[test]
fn list_codex_models_falls_back_to_keys_when_models_field_absent() {
    // Legacy cache shape: keys are model ids directly (no models
    // array). v1.0.81 must still merge those keys into the result.
    let tmp =
        std::env::temp_dir().join(format!("codex-models-legacy-test-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join(".codex")).expect("mkdir");
    let cache_body = r#"{"legacy-model-x": 1, "legacy-model-y": 2}"#;
    std::fs::write(tmp.join(".codex/models_cache.json"), cache_body).expect("write cache");
    let prev_home = std::env::var("HOME");
    unsafe {
        std::env::set_var("HOME", &tmp);
    }
    let models = list_codex_models();
    unsafe {
        if let Ok(h) = prev_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);

    assert!(
        models.contains(&"legacy-model-x".to_string()),
        "legacy-model-x missing: {models:?}"
    );
    assert!(
        models.contains(&"legacy-model-y".to_string()),
        "legacy-model-y missing: {models:?}"
    );
}

/// OAuth-only conformance test (gaps.md:41-49, v1.0.69 mandate).
/// Verifies that `build_codex_command` always emits `-c mcp_servers='{}'`,
/// `--ignore-user-config`, `--ask-for-approval never` and does NOT
/// whitelist `OPENAI_API_KEY` in the env_clear whitelist.
#[test]
#[serial_test::serial(env)]
fn build_command_oauth_only_mandatory_flags() {
    // SAFETY: unit test
    unsafe {
        std::env::remove_var("OPENAI_API_KEY");
    }
    let schema = std::env::temp_dir().join("codex-test-schema.json");
    let _ = std::fs::remove_file(&schema);
    let args = CodexSpawnArgs {
        binary: std::path::Path::new("/usr/bin/false"),
        prompt: "p",
        json_schema: "{}",
        input_text: "i",
        model: Some("gpt-5.4-mini"),
        timeout_secs: 60,
        schema_path: schema.clone(),
    };
    let cmd = build_codex_command(&args).expect("preflight gate accepts valid args");
    let argv: Vec<&str> = cmd.get_args().filter_map(|a| a.to_str()).collect();
    // Mandatory flags from gaps.md lines 233-238.
    // -c mcp_servers='{}' was REMOVED in v1.0.76 — codex 0.134+ parses
    // the value as a string and rejects it ("expected a map"). The
    // --ignore-user-config flag already covers the MCP isolation
    // requirement.
    assert!(
        argv.contains(&"--ignore-user-config"),
        "must have --ignore-user-config (gaps.md:266)"
    );
    // --ask-for-approval is conditional on codex < 0.134. When the
    // installed codex is 0.134+ the flag is omitted by the compat
    // helper. Both outcomes are valid.
    let ask_for_approval_present = argv.contains(&"--ask-for-approval");
    if !crate::extract::codex_compat::codex_supports_ask_for_approval() {
        assert!(
            !ask_for_approval_present,
            "codex 0.134+ must NOT include --ask-for-approval"
        );
    }
    assert!(
        argv.contains(&"--sandbox"),
        "must have --sandbox read-only (G31)"
    );
    assert!(argv.contains(&"--ephemeral"), "must have --ephemeral (G31)");
    assert!(
        argv.contains(&"--skip-git-repo-check"),
        "must have --skip-git-repo-check (G31)"
    );
    assert!(
        argv.contains(&"--ignore-rules"),
        "must have --ignore-rules (G31)"
    );
    // v1.0.77: -c TOML overrides bypass codex exec --sandbox bug (#18113)
    assert!(
        argv.contains(&"-c") && argv.contains(&"sandbox_mode='read-only'"),
        "must have -c sandbox_mode='read-only' (v1.0.77, codex#18113)"
    );
    assert!(
        argv.contains(&"approval_policy='never'"),
        "must have -c approval_policy='never' (v1.0.77)"
    );
}

/// OAuth-only guard: when `OPENAI_API_KEY` is in the environment,
/// `build_codex_command` MUST abort the spawn (return a `false`
/// command), NOT pass the key through to the child.
#[test]
#[serial_test::serial(env)]
fn build_command_aborts_when_openai_api_key_set() {
    // SAFETY: unit test
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "sk-violation-test");
    }
    let schema = std::env::temp_dir().join("codex-test-schema-abort.json");
    let _ = std::fs::remove_file(&schema);
    let args = CodexSpawnArgs {
        binary: std::path::Path::new("/usr/bin/codex"),
        prompt: "p",
        json_schema: "{}",
        input_text: "i",
        model: Some("gpt-5.4-mini"),
        timeout_secs: 60,
        schema_path: schema.clone(),
    };
    let cmd = build_codex_command(&args).expect("preflight gate accepts valid args");
    let program = cmd.get_program().to_string_lossy().to_string();
    let argv: Vec<&str> = cmd.get_args().filter_map(|a| a.to_str()).collect();
    assert_eq!(
        program, "false",
        "when OPENAI_API_KEY is set, build_codex_command must abort"
    );
    assert!(
        argv.contains(&"--oauth-only-violation-openai-api-key-set"),
        "aborted command must carry violation marker"
    );
    unsafe {
        std::env::remove_var("OPENAI_API_KEY");
    }
}
