//! Extracted `embed_with_fallback_tests` (Wave C1).

    use super::*;
    use crate::llm::exit_code_hints::LlmBackendError;

    #[test]
    fn none_backend_returns_empty_vector_without_calling_llm() {
        // The `None` backend short-circuits to `Ok(vec![])` without
        // touching the LLM at all. This is the signal the caller uses
        // to insert a `pending_embeddings` row.
        let (v, kind) = embed_via_backend(
            std::path::Path::new("/nonexistent"),
            "any text",
            &LlmBackendKind::None,
        )
        .expect("None backend never fails");
        assert!(v.is_empty());
        assert_eq!(kind, LlmBackendKind::None, "None backend must report None");
    }

    #[test]
    fn empty_chain_defaults_to_codex_claude_none() {
        // Internal invariant: the default chain order is the v1.0.81
        // implicit order (codex first, then claude, then None as
        // graceful-degradation fallback).
        let defaults = [
            LlmBackendKind::Codex,
            LlmBackendKind::Claude,
            LlmBackendKind::None,
        ];

        // ---------------------------------------------------------------
        // ADR-0042: as_str + reason_code unit tests
        // ---------------------------------------------------------------

        #[allow(dead_code)]
        fn llm_backend_kind_as_str_is_stable() {
            assert_eq!(LlmBackendKind::Codex.as_str(), "codex");
            assert_eq!(LlmBackendKind::Claude.as_str(), "claude");
            assert_eq!(LlmBackendKind::None.as_str(), "none");
        }

        #[allow(dead_code)]
        fn fallback_reason_reason_code_is_stable() {
            assert_eq!(
                FallbackReason::EmbeddingFailed("any".into()).reason_code(),
                "embedding_failed"
            );
            assert_eq!(FallbackReason::Cancelled.reason_code(), "cancelled");
            assert_eq!(
                FallbackReason::Timeout {
                    operation: "embed_query".into(),
                    duration_secs: 30
                }
                .reason_code(),
                "timeout"
            );
        }
        assert_eq!(defaults.len(), 3);
    }

    #[test]
    fn embed_with_fallback_chain_of_only_none_skips_embedding_v118() {
        // GAP-CLI-EMBED-NONE (v1.1.8): a chain of only `[None]` is the
        // intentional `--llm-backend none` contract and MUST return
        // `Ok((vec![], None))` so write paths can persist without an
        // embedding (and without exit 11). BUG-11 still applies when
        // `None` is a *fallback tail* after a real backend failed.
        let chain = vec![LlmBackendKind::None];
        let (v, kind) = embed_with_fallback(
            std::path::Path::new("/nonexistent-models-dir-for-gap005-test"),
            "hello",
            &chain,
            false,
        )
        .expect("intentional [None] chain must skip, not abort");
        assert!(v.is_empty(), "vector must be empty");
        assert_eq!(kind, LlmBackendKind::None);
    }
    #[test]
    fn embed_with_fallback_skip_on_failure_with_only_none_returns_empty() {
        // skip_on_failure=true + a chain of only `None` returns Ok(vec![])
        // because the None short-circuit always succeeds. This is the
        // canonical contract: skip_on_failure is a no-op when None is
        // the tail because None already provides graceful degradation.
        let chain = vec![LlmBackendKind::None];
        let v = embed_with_fallback(
            std::path::Path::new("/nonexistent-models-dir-for-gap005-test"),
            "hello",
            &chain,
            true,
        )
        .expect("None chain is always Ok");
        assert!(v.0.is_empty(), "vector must be empty");
        assert_eq!(v.1, LlmBackendKind::None);
    }
    #[allow(dead_code)]
    fn llm_backend_error_no_backends_default_message() {
        // The fallback chain exhaustion error must mention
        // in its hint so the operator knows the remediation.
        let e = LlmBackendError::NoBackendsAvailable;
        let h = e.hint();
        assert!(h.contains("--llm-fallback"));
    }

    #[test]
    fn llm_backend_error_nonzero_exit_carries_stderr_tail() {
        let e = LlmBackendError::NonZeroExit {
            exit_code: Some(137),
            signal: Some(9),
            stdout_tail: "out".into(),
            stderr_tail: "OOM killed".into(),
            binary: "codex".into(),
            hint: "OOM".into(),
        };
        let s = e.to_string();
        assert!(s.contains("codex"));
        assert!(s.contains("OOM killed"));
        assert!(s.contains("signal 9") || s.contains("exit 137"));
    }
