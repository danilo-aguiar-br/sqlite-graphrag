use super::*;
use std::ffi::OsString;

fn dummy_argv() -> Vec<OsString> {
    vec![
        OsString::from("/usr/bin/claude"),
        OsString::from("-p"),
        OsString::from("hello"),
    ]
}

fn dummy_args<'a>(
    binary: &'a Path,
    argv: &'a [OsString],
    inline_json: Option<&'a str>,
) -> PreFlightArgs<'a> {
    // Use a dedicated empty tempdir for workspace_root so walk-up of
    // `.mcp.json` does not pick up unrelated files in the test's CWD.
    // The tempdir is leaked (kept alive for the test lifetime) via
    // `OnceLock` to keep the API simple.
    use std::sync::OnceLock;
    static WORKSPACE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let workspace = WORKSPACE.get_or_init(|| tempfile::tempdir().expect("tempdir"));
    PreFlightArgs {
        binary_path: binary,
        argv,
        workspace_root: workspace.path(),
        mcp_config_inline_json: inline_json,
        expected_output_bytes: 1024,
        spawner_name: "test",
    }
}

#[test]
#[serial_test::serial(env)]
fn check_binary_exists_passes_when_path_valid() {
    // SAFETY: serial_test::serial(env) ensures no parallel mutation.
    let saved = std::env::var_os("CLAUDE_CONFIG_DIR");
    unsafe {
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }
    let binary = if cfg!(windows) {
        "C:\\Windows\\System32\\cmd.exe"
    } else {
        "/bin/sh"
    };
    let argv = dummy_argv();
    let args = dummy_args(Path::new(binary), &argv, None);
    let result = preflight_check(&args);
    if let Some(v) = saved {
        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", v);
        }
    }
    assert!(result.is_ok(), "preflight returned: {result:?}");
}

#[test]
fn check_binary_exists_fails_when_missing() {
    let argv = dummy_argv();
    let args = dummy_args(Path::new("/does/not/exist/claude-binary"), &argv, None);
    let err = preflight_check(&args).unwrap_err();
    assert!(
        matches!(err, PreFlightError::BinaryNotFound { .. }),
        "expected BinaryNotFound, got {err:?}"
    );
}

#[test]
#[serial_test::serial(env)]
fn check_argv_size_passes_under_limit() {
    let saved = std::env::var_os("CLAUDE_CONFIG_DIR");
    unsafe {
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }
    let argv = dummy_argv();
    let args = dummy_args(Path::new("/bin/sh"), &argv, None);
    let result = preflight_check(&args);
    if let Some(v) = saved {
        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", v);
        }
    }
    // dummy_argv() is tiny — well under ARG_MAX.
    assert!(result.is_ok(), "preflight returned: {result:?}");
}

#[test]
#[serial_test::serial(env)]
fn check_argv_size_fails_when_exceeds_arg_max() {
    let saved = std::env::var_os("CLAUDE_CONFIG_DIR");
    unsafe {
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }
    // Synthesize an argv that exceeds ARG_MAX regardless of the
    // host value. We allocate 64 MiB to leave the 4 KiB safety
    // margin well below `getconf ARG_MAX` on every supported OS.
    let huge = "x".repeat(64 * 1024 * 1024);
    let argv = vec![OsString::from("/bin/sh"), OsString::from(huge)];
    let args = dummy_args(Path::new("/bin/sh"), &argv, None);
    let err = preflight_check(&args).unwrap_err();
    if let Some(v) = saved {
        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", v);
        }
    }
    assert!(
        matches!(err, PreFlightError::ArgvExceedsArgMax { .. }),
        "expected ArgvExceedsArgMax, got {err:?}"
    );
}

#[test]
fn check_mcp_inline_json_detects_literal_braces() {
    // argv references /bin/sh (exists) so the binary check passes.
    let argv = dummy_argv();
    let args = dummy_args(Path::new("/bin/sh"), &argv, Some("{}"));
    let err = preflight_check(&args).unwrap_err();
    assert!(
        matches!(err, PreFlightError::McpConfigInlineJsonRejected(_)),
        "expected McpConfigInlineJsonRejected, got {err:?}"
    );
}

#[test]
fn check_mcp_inline_json_writes_valid_tempfile() {
    // Round-trip: write_empty_mcp_config_tempfile produces a file
    // parseable as JSON containing `mcpServers: {}`.
    let path = write_empty_mcp_config_tempfile().expect("tempfile write");
    let contents = std::fs::read_to_string(&path).expect("tempfile read");
    let parsed: serde_json::Value =
        serde_json::from_str(&contents).expect("tempfile valid JSON");
    assert!(parsed.get("mcpServers").is_some());
    assert!(parsed["mcpServers"].as_object().unwrap().is_empty());
    // Cleanup
    let _ = std::fs::remove_file(&path);
}

#[test]
fn check_mcp_path_missing_returns_error() {
    // Build an argv with --mcp-config pointing at a nonexistent path.
    let argv = vec![
        OsString::from("/bin/sh"),
        OsString::from("--mcp-config"),
        OsString::from("/nonexistent/path/mcp.json"),
    ];
    let args = dummy_args(Path::new("/bin/sh"), &argv, None);
    let err = preflight_check(&args).unwrap_err();
    assert!(
        matches!(err, PreFlightError::McpConfigPathMissing { .. }),
        "expected McpConfigPathMissing, got {err:?}"
    );
}

#[test]
fn check_mcp_path_invalid_json_returns_error() {
    // Write an invalid JSON tempfile then reference it.
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    std::fs::write(tmp.path(), b"this is not json").expect("write");
    let argv = vec![
        OsString::from("/bin/sh"),
        OsString::from("--mcp-config"),
        OsString::from(tmp.path().to_string_lossy().into_owned()),
    ];
    let args = dummy_args(Path::new("/bin/sh"), &argv, None);
    let err = preflight_check(&args).unwrap_err();
    assert!(
        matches!(err, PreFlightError::McpConfigPathInvalidJson { .. }),
        "expected McpConfigPathInvalidJson, got {err:?}"
    );
}

#[test]
fn check_walkup_mcp_json_passes_when_clean() {
    // Use a dedicated tempdir created for the test (guaranteed empty).
    let dir = tempfile::tempdir().expect("tempdir");
    let argv = dummy_argv();
    let args = PreFlightArgs {
        workspace_root: dir.path(),
        ..dummy_args(Path::new("/bin/sh"), &argv, None)
    };
    let result = preflight_check(&args);
    // We only assert we did NOT return WalkUpMcpJsonInvalid for a
    // clean workspace.
    if let Err(PreFlightError::WalkUpMcpJsonInvalid { .. }) = &result {
        panic!("walk-up incorrectly flagged on clean workspace");
    }
}

#[test]
fn check_walkup_mcp_json_fails_on_zod_invalid() {
    // Create a temp workspace dir with an invalid .mcp.json inside.
    let dir = tempfile::tempdir().expect("tempdir");
    let bad = dir.path().join(".mcp.json");
    std::fs::write(&bad, b"{not json").expect("write bad mcp.json");
    let argv = dummy_argv();
    let args = PreFlightArgs {
        workspace_root: dir.path(),
        ..dummy_args(Path::new("/bin/sh"), &argv, None)
    };
    let err = preflight_check(&args).unwrap_err();
    assert!(
        matches!(err, PreFlightError::WalkUpMcpJsonInvalid { .. }),
        "expected WalkUpMcpJsonInvalid, got {err:?}"
    );
}

#[test]
fn check_walkup_mcp_json_fails_on_active_mcp_servers() {
    // BUG-9 regression: a syntactically valid `.mcp.json` that
    // declares MCP servers under `mcpServers` must be rejected.
    let dir = tempfile::tempdir().expect("tempdir");
    let bad = dir.path().join(".mcp.json");
    std::fs::write(
        &bad,
        r#"{"mcpServers":{"github":{"command":"gh","args":["mcp"]}}}"#,
    )
    .expect("write bad mcp.json");
    let argv = dummy_argv();
    let args = PreFlightArgs {
        workspace_root: dir.path(),
        ..dummy_args(Path::new("/bin/sh"), &argv, None)
    };
    let err = preflight_check(&args).unwrap_err();
    assert!(
        matches!(err, PreFlightError::WalkUpMcpJsonInvalid { .. }),
        "expected WalkUpMcpJsonInvalid, got {err:?}"
    );
}

#[test]
fn check_walkup_mcp_json_passes_with_empty_mcp_servers() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ok = dir.path().join(".mcp.json");
    std::fs::write(&ok, r#"{"mcpServers":{}}"#).expect("write");
    let argv = dummy_argv();
    let args = PreFlightArgs {
        workspace_root: dir.path(),
        ..dummy_args(Path::new("/bin/sh"), &argv, None)
    };
    let result = preflight_check(&args);
    if let Err(PreFlightError::WalkUpMcpJsonInvalid { .. }) = &result {
        panic!("empty mcpServers must pass walk-up: {result:?}");
    }
}

#[test]
fn check_mcp_path_equals_form_detects_missing_file() {
    // BUG-5 regression: --mcp-config=PATH single-slot form must be
    // caught the same as the GNU --mcp-config <PATH> form.
    let argv = vec![
        OsString::from("/bin/sh"),
        OsString::from("--mcp-config=/nonexistent/path/mcp.json"),
    ];
    let args = dummy_args(Path::new("/bin/sh"), &argv, None);
    let err = preflight_check(&args).unwrap_err();
    assert!(
        matches!(err, PreFlightError::McpConfigPathMissing { .. }),
        "expected McpConfigPathMissing, got {err:?}"
    );
}

#[test]
fn check_output_buffer_warns_when_oversized() {
    let argv = dummy_argv();
    let args = PreFlightArgs {
        expected_output_bytes: 100_000, // > 65536 cap
        ..dummy_args(Path::new("/bin/sh"), &argv, None)
    };
    let err = preflight_check(&args).unwrap_err();
    assert!(
        matches!(err, PreFlightError::OutputBufferTooSmall { .. }),
        "expected OutputBufferTooSmall, got {err:?}"
    );
}

#[test]
#[serial_test::serial(env)]
fn check_claude_config_dir_fails_when_settings_has_active_mcps() {
    // SAFETY: serial_test::serial(env) ensures no parallel mutation.
    let dir = tempfile::tempdir().expect("tempdir");
    let settings = dir.path().join("settings.json");
    std::fs::write(
        &settings,
        r#"{"mcpServers":{"github":{"command":"gh","args":["mcp"]}}}"#,
    )
    .expect("write settings.json");
    unsafe {
        std::env::set_var("CLAUDE_CONFIG_DIR", dir.path());
    }
    let argv = dummy_argv();
    let args = dummy_args(Path::new("/bin/sh"), &argv, None);
    let err = preflight_check(&args);
    unsafe {
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }
    if let Err(PreFlightError::ClaudeConfigDirNotEmpty { reason, .. }) = err {
        assert_eq!(reason, "mcpServers");
    } else {
        panic!("expected ClaudeConfigDirNotEmpty mcpServers, got {err:?}");
    }
}

#[test]
#[serial_test::serial(env)]
fn check_claude_config_dir_passes_when_settings_empty() {
    // SAFETY: serial_test::serial(env) ensures no parallel mutation.
    let dir = tempfile::tempdir().expect("tempdir");
    let settings = dir.path().join("settings.json");
    std::fs::write(&settings, r#"{"mcpServers":{},"hooks":{}}"#).expect("write");
    unsafe {
        std::env::set_var("CLAUDE_CONFIG_DIR", dir.path());
    }
    let argv = dummy_argv();
    let args = dummy_args(Path::new("/bin/sh"), &argv, None);
    let result = preflight_check(&args);
    unsafe {
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }
    assert!(result.is_ok(), "empty MCPs and hooks must pass: {result:?}");
}

#[test]
#[serial_test::serial(env)]
fn check_claude_config_dir_passes_when_no_settings_json() {
    // SAFETY: serial_test::serial(env) ensures no parallel mutation.
    let dir = tempfile::tempdir().expect("tempdir");
    // Populate with non-MCP files only (CLAUDE.md, commands/, etc).
    std::fs::write(dir.path().join("CLAUDE.md"), "# project notes").expect("write");
    unsafe {
        std::env::set_var("CLAUDE_CONFIG_DIR", dir.path());
    }
    let argv = dummy_argv();
    let args = dummy_args(Path::new("/bin/sh"), &argv, None);
    let result = preflight_check(&args);
    unsafe {
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }
    assert!(
        result.is_ok(),
        "populated dir without settings.json must pass: {result:?}"
    );
}

#[test]
#[serial_test::serial(env)]
fn check_claude_config_dir_passes_when_settings_has_only_hooks() {
    // Hooks are tolerated because the spawners override
    // `--settings '{"hooks":{}}'` at the CLI boundary; only MCP
    // servers are flagged as a hard error.
    let dir = tempfile::tempdir().expect("tempdir");
    let settings = dir.path().join("settings.json");
    std::fs::write(&settings, r#"{"hooks":{"PreToolUse":[]}}"#).expect("write");
    unsafe {
        std::env::set_var("CLAUDE_CONFIG_DIR", dir.path());
    }
    let argv = dummy_argv();
    let args = dummy_args(Path::new("/bin/sh"), &argv, None);
    let result = preflight_check(&args);
    unsafe {
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }
    assert!(result.is_ok(), "hooks must be tolerated: {result:?}");
}

#[test]
fn preflight_check_runs_all_guards_in_order() {
    // Valid path + clean argv + clean workspace + no inline JSON.
    let dir = tempfile::tempdir().expect("tempdir");
    let argv = dummy_argv();
    let args = PreFlightArgs {
        workspace_root: dir.path(),
        ..dummy_args(Path::new("/bin/sh"), &argv, None)
    };
    assert!(preflight_check(&args).is_ok());
}

#[test]
fn preflight_check_short_circuits_on_first_failure() {
    // Invalid binary + bad inline JSON — should report BinaryNotFound
    // first (cheap in-memory check) NOT the McpConfigInlineJsonRejected
    // (also cheap, but binary is checked earlier in the order).
    let argv = dummy_argv();
    let args = dummy_args(Path::new("/does/not/exist/at/all"), &argv, Some("{}"));
    let err = preflight_check(&args).unwrap_err();
    assert!(
        matches!(err, PreFlightError::BinaryNotFound { .. }),
        "expected BinaryNotFound (short-circuit), got {err:?}"
    );
}

#[test]
#[serial_test::serial(env)]
fn app_error_preflight_failed_has_exit_code_16() {
    // Cross-check the integration: AppError::PreFlightFailed maps to
    // exit code 16 (validated by this test, not by preflight itself).
    use crate::errors::AppError;
    let err: AppError = crate::spawn::preflight::PreFlightError::BinaryNotFound {
        path: "/bin/test".into(),
    }
    .into();
    assert_eq!(err.exit_code(), 16);
    assert!(err.is_permanent());
    assert!(!err.is_retryable());
}
