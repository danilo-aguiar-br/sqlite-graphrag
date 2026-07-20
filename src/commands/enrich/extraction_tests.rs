//! Auto-extracted tests (Wave C1).

    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn parse_claude_output_valid_bindings() {
        let output = r#"[
            {"type":"system","subtype":"init"},
            {"type":"result","is_error":false,"total_cost_usd":0.01,
             "structured_output":{"entities":[{"name":"rust-lang","entity_type":"tool"}],"relationships":[]}}
        ]"#;
        let result = crate::commands::claude_runner::parse_claude_output(output)
            .expect("must parse successfully");
        assert!(result.value.get("entities").is_some());
        assert!((result.cost_usd - 0.01).abs() < f64::EPSILON);
        assert!(!result.is_oauth);
    }

    #[test]
    fn parse_claude_output_detects_oauth() {
        let output = r#"[
            {"type":"system","subtype":"init","apiKeySource":"none"},
            {"type":"result","is_error":false,"total_cost_usd":0.0,
             "structured_output":{"entities":[],"relationships":[]}}
        ]"#;
        let result = crate::commands::claude_runner::parse_claude_output(output).unwrap();
        assert!(result.is_oauth);
    }

    #[test]
    fn parse_claude_output_rate_limit_returns_error() {
        let output = r#"[
            {"type":"system","subtype":"init"},
            {"type":"result","is_error":true,"error":"rate_limit exceeded"}
        ]"#;
        let err = crate::commands::claude_runner::parse_claude_output(output).unwrap_err();
        assert!(matches!(err, AppError::RateLimited { .. }));
    }

    #[test]
    fn parse_claude_output_auth_error() {
        let output = r#"[
            {"type":"system","subtype":"init"},
            {"type":"result","is_error":true,"error":"authentication failed"}
        ]"#;
        let err = crate::commands::claude_runner::parse_claude_output(output).unwrap_err();
        assert!(format!("{err}").contains("authentication failed"));
    }

    #[cfg(unix)]
    #[test]
    fn call_codex_returns_raw_json_for_body_enrich_schema() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let binary = tmp.path().join("codex-mock");
        std::fs::write(
            &binary,
            r#"#!/usr/bin/env bash
set -euo pipefail
cat <<'JSONL'
{"type":"thread.started","thread_id":"mock-thread-0"}
{"type":"item.completed","item":{"type":"agent_message","text":"{\"enriched_body\":\"expanded body\"}"}}
{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}
JSONL
"#,
        )
        .expect("mock codex write");
        let mut perms = std::fs::metadata(&binary).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&binary, perms).expect("chmod");

        let (value, cost, is_oauth) =
            call_codex(&binary, "prompt", BODY_ENRICH_SCHEMA, "body", None, 5)
                .expect("call_codex must accept body-enrich payload");

        assert_eq!(value["enriched_body"], "expanded body");
        assert_eq!(cost, 0.0);
        assert!(!is_oauth);
    }
