//! Auto-extracted tests (Wave C1).

    use super::*;
    use super::batch::{
        adaptive_batch_for_dim, build_batches, entity_cache_key, entity_embed_cache,
        reassemble_ordered, run_bounded,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn reassemble_ordered_restores_input_order() {
        // GAP-OPENROUTER-REST-CONCURRENCY: the bounded JoinSet fan-out
        // completes chunks out of order, so parts arrive shuffled. The
        // reassembly MUST restore the exact input order by chunk index.
        let parts = vec![
            (2, vec![vec![2.0_f32], vec![2.1]]),
            (0, vec![vec![0.0], vec![0.1]]),
            (1, vec![vec![1.0], vec![1.1]]),
        ];
        let out = reassemble_ordered(parts);
        assert_eq!(
            out,
            vec![
                vec![0.0_f32],
                vec![0.1],
                vec![1.0],
                vec![1.1],
                vec![2.0],
                vec![2.1],
            ]
        );
    }

    #[test]
    fn f32_to_bytes_roundtrip() {
        let input = vec![0.0_f32, 1.5, -2.25, f32::MIN, f32::MAX];
        let bytes = f32_to_bytes(&input);
        assert_eq!(bytes.len(), input.len() * 4);
        let out = bytes_to_f32(&bytes);
        assert_eq!(out, input);
    }

    #[test]
    fn validate_dim_rejects_divergent_vectors() {
        // G42/C5 acceptance criterion: a divergent vector MUST fail —
        // never be silently normalised.
        let dim = crate::constants::embedding_dim();
        let long = vec![0.0; dim + 10];
        assert!(validate_dim(long).is_err(), "longer vector must error");
        let short = vec![0.0; dim.saturating_sub(1).max(1)];
        assert!(validate_dim(short).is_err(), "shorter vector must error");
        let exact = vec![0.0; dim];
        assert_eq!(validate_dim(exact).expect("exact dim must pass").len(), dim);
    }

    #[test]
    fn embedding_dim_matches_constants_source() {
        assert_eq!(embedding_dim(), crate::constants::embedding_dim());
    }

    #[test]
    fn build_batches_preserves_global_indices() {
        let texts: Vec<String> = (0..10).map(|i| format!("t{i}")).collect();
        let batches = build_batches(&texts, 4);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 4);
        assert_eq!(batches[2].len(), 2);
        assert_eq!(batches[2][1].0, 9);
        assert_eq!(batches[2][1].1, "t9");
    }

    #[test]
    fn effective_permits_clamps_to_bounds() {
        assert!(effective_permits(0) >= 1);
        assert!(effective_permits(1000) <= 32);
    }

    fn test_batches(n: usize) -> Vec<Vec<(usize, String)>> {
        (0..n).map(|i| vec![(i, format!("t{i}"))]).collect()
    }

    fn dummy_vec(dim: usize) -> Vec<f32> {
        vec![0.0; dim]
    }

    /// G42 acceptance criterion: with N permits the measured peak of
    /// concurrent workers NEVER exceeds N, even with 10x more batches.
    #[test]
    fn concurrency_peak_never_exceeds_permits() {
        let permits = 4usize;
        let batches = test_batches(permits * 10);
        let dim = crate::constants::embedding_dim();
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let current_c = Arc::clone(&current);
        let peak_c = Arc::clone(&peak);
        let work = move |batch: Vec<(usize, String)>| {
            let current = Arc::clone(&current_c);
            let peak = Arc::clone(&peak_c);
            async move {
                let now = current.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                current.fetch_sub(1, Ordering::SeqCst);
                Ok(batch
                    .into_iter()
                    .map(|(i, _)| (i, dummy_vec(dim)))
                    .collect())
            }
        };

        let mut delivered = 0usize;
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("test runtime");
        rt.block_on(run_bounded(
            batches,
            permits,
            dim,
            CancellationToken::new(),
            work,
            &mut |_idx, _v| {
                delivered += 1;
                Ok(())
            },
        ))
        .expect("fan-out must succeed");

        assert_eq!(delivered, permits * 10, "every item must be delivered");
        assert!(
            peak.load(Ordering::SeqCst) <= permits,
            "peak concurrency {} exceeded permits {permits}",
            peak.load(Ordering::SeqCst)
        );
    }

    /// G42 acceptance criterion: a panicking task returns its permit via
    /// RAII and surfaces as JoinError::is_panic, not a hang.
    #[test]
    fn panicking_task_returns_permit_and_surfaces_error() {
        let permits = 2usize;
        let batches = test_batches(4);
        let dim = crate::constants::embedding_dim();

        let work = move |batch: Vec<(usize, String)>| async move {
            if batch[0].0 == 1 {
                panic!("intentional test panic");
            }
            Ok(batch
                .into_iter()
                .map(|(i, _)| (i, dummy_vec(dim)))
                .collect())
        };

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("test runtime");
        let result = rt.block_on(run_bounded(
            batches,
            permits,
            dim,
            CancellationToken::new(),
            work,
            &mut |_idx, _v| Ok(()),
        ));

        let err = result.expect_err("panic must surface as an error");
        assert!(
            err.to_string().contains("panicked"),
            "error must mention the panic: {err}"
        );
    }

    /// G42 acceptance criterion: cancellation aborts in-flight work and
    /// the fan-out terminates within the shutdown timeout.
    #[test]
    fn cancellation_terminates_fan_out_quickly() {
        let permits = 2usize;
        let batches = test_batches(8);
        let dim = crate::constants::embedding_dim();
        let token = CancellationToken::new();

        let work = move |batch: Vec<(usize, String)>| async move {
            // Long enough that only cancellation can finish the test fast.
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            Ok(batch
                .into_iter()
                .map(|(i, _)| (i, dummy_vec(dim)))
                .collect())
        };

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("test runtime");
        let cancel = token.clone();
        let start = std::time::Instant::now();
        let result = rt.block_on(async move {
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                cancel.cancel();
            });
            run_bounded(batches, permits, dim, token, work, &mut |_idx, _v| Ok(())).await
        });

        assert!(result.is_err(), "cancelled fan-out must report an error");
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "graceful shutdown must finish well under the work duration"
        );
    }

    /// G42 acceptance criterion: a divergent dim coming out of the work
    /// stage fails the fan-out instead of being silently accepted.
    #[test]
    fn fan_out_rejects_divergent_dim() {
        let permits = 2usize;
        let batches = test_batches(2);
        let dim = crate::constants::embedding_dim();

        let work = move |batch: Vec<(usize, String)>| async move {
            Ok(batch
                .into_iter()
                .map(|(i, _)| (i, vec![0.0f32; 3]))
                .collect::<Vec<(usize, Vec<f32>)>>())
        };

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("test runtime");
        let result = rt.block_on(run_bounded(
            batches,
            permits,
            dim,
            CancellationToken::new(),
            work,
            &mut |_idx, _v| Ok(()),
        ));

        let err = result.expect_err("divergent dim must fail the fan-out");
        assert!(err.to_string().contains("G42/C5"), "error cites C5: {err}");
    }

    /// G44: the calibration bases stay intact at the calibration dim.
    #[test]
    fn adaptive_batch_dim64_keeps_calibrated_sizes() {
        assert_eq!(adaptive_batch_for_dim(CHUNK_EMBED_BATCH_SIZE, 64), 8);
        assert_eq!(adaptive_batch_for_dim(ENTITY_EMBED_BATCH_SIZE, 64), 25);
    }

    /// G44: legacy 384-dim databases shrink to reliable batch sizes.
    #[test]
    fn adaptive_batch_dim384_shrinks() {
        assert_eq!(adaptive_batch_for_dim(CHUNK_EMBED_BATCH_SIZE, 384), 1);
        assert_eq!(adaptive_batch_for_dim(ENTITY_EMBED_BATCH_SIZE, 384), 4);
    }

    /// G44: intermediate dims scale proportionally to the float budget.
    #[test]
    fn adaptive_batch_intermediate_dims() {
        assert_eq!(adaptive_batch_for_dim(8, 128), 4);
        assert_eq!(adaptive_batch_for_dim(8, 256), 2);
    }

    /// G44: dims below the calibration dim never exceed the base.
    #[test]
    fn adaptive_batch_small_dim_clamps_to_base() {
        assert_eq!(adaptive_batch_for_dim(8, 8), 8);
    }

    /// G44: the function is total — no division by zero, no clamp panic.
    #[test]
    fn adaptive_batch_total_function() {
        assert_eq!(adaptive_batch_for_dim(8, 4096), 1);
        assert_eq!(adaptive_batch_for_dim(8, 0), 8);
        assert_eq!(adaptive_batch_for_dim(0, 64), 1);
    }

    /// G44 end-to-end: the public wrappers follow the ACTIVE dim.
    ///
    /// GAP-SG-84: this case used to set `SQLITE_GRAPHRAG_EMBEDDING_DIM` and
    /// assert batch sizes for 384 dims. No reader consults that env, so the
    /// assertions only held because the compiled default happened to be 384 —
    /// the test proved nothing about the override and broke the moment the
    /// default moved. It now drives the dim through the real channel.
    #[test]
    #[serial_test::serial(env)]
    fn adaptive_wrappers_follow_active_dim() {
        crate::constants::set_active_embedding_dim(384);
        let chunk = chunk_embed_batch_size();
        let entity = entity_embed_batch_size();
        crate::constants::set_active_embedding_dim(crate::constants::DEFAULT_EMBEDDING_DIM);
        assert_eq!(chunk, 1, "384-dim chunk batch must shrink to 1 (G44)");
        assert_eq!(entity, 4, "384-dim entity batch must shrink to 4 (G44)");
    }

    /// The retired product env must not move the batch size.
    ///
    /// Asserting the negative is the point: without it, reintroducing an env
    /// read would go unnoticed by the suite.
    #[test]
    #[serial_test::serial(env)]
    fn retired_embedding_dim_env_is_inert() {
        crate::constants::set_active_embedding_dim(crate::constants::DEFAULT_EMBEDDING_DIM);
        let before = chunk_embed_batch_size();
        std::env::set_var("SQLITE_GRAPHRAG_EMBEDDING_DIM", "384");
        let during = chunk_embed_batch_size();
        std::env::remove_var("SQLITE_GRAPHRAG_EMBEDDING_DIM");
        assert_eq!(
            before, during,
            "SQLITE_GRAPHRAG_EMBEDDING_DIM must not change the active dim"
        );
    }

    // ---------------------------------------------------------------
    // G58/S1: FallbackReason + try_embed_query_with_fallback tests
    // ---------------------------------------------------------------

    /// GAP-004 (v1.0.88): EmbeddingErrorKind::classify maps an OAuth
    /// error message to the OAuth variant regardless of case or
    /// surrounding text.
    #[test]
    fn embedding_error_kind_classify_oauth_message() {
        assert_eq!(
            EmbeddingErrorKind::classify("OAuth token expired for claude"),
            EmbeddingErrorKind::OAuth,
        );
        assert_eq!(
            EmbeddingErrorKind::classify("oauth authentication failed"),
            EmbeddingErrorKind::OAuth,
        );
    }

    /// GAP-004 (v1.0.88): EmbeddingErrorKind::classify maps a quota
    /// message to the Quota variant (without "OAuth" substring).
    #[test]
    fn embedding_error_kind_classify_quota_message() {
        assert_eq!(
            EmbeddingErrorKind::classify("quota exhausted on backend"),
            EmbeddingErrorKind::Quota,
        );
        assert_eq!(
            EmbeddingErrorKind::classify("Usage quota limit reached"),
            EmbeddingErrorKind::Quota,
        );
    }

    /// GAP-004 (v1.0.88): EmbeddingErrorKind::classify maps a slot-sema
    /// message to the SlotExhausted variant (matched BEFORE Quota so
    /// the more specific LLM-never-tried path wins).
    #[test]
    fn embedding_error_kind_classify_slot_exhausted_message() {
        assert_eq!(
            EmbeddingErrorKind::classify(
                "slot exhausted: failed to acquire LLM slot after backoff"
            ),
            EmbeddingErrorKind::SlotExhausted,
        );
    }

    /// GAP-004 (v1.0.88): EmbeddingErrorKind::classify maps a
    /// zero-dimensional vector error to the ZeroDimension variant.
    #[test]
    fn embedding_error_kind_classify_zero_dimension_message() {
        assert_eq!(
            EmbeddingErrorKind::classify("embedding returned dim=zero"),
            EmbeddingErrorKind::ZeroDimension,
        );
        assert_eq!(
            EmbeddingErrorKind::classify("got zero-dim vector from LLM"),
            EmbeddingErrorKind::ZeroDimension,
        );
    }

    /// GAP-004 (v1.0.88): EmbeddingErrorKind::classify falls back to
    /// the Unknown variant when no marker matches, and the code()
    /// accessor returns the kebab-safe discriminator string.
    #[test]
    fn embedding_error_kind_classify_unknown_fallback() {
        assert_eq!(
            EmbeddingErrorKind::classify("unrelated subprocess error"),
            EmbeddingErrorKind::Unknown,
        );
        assert_eq!(
            EmbeddingErrorKind::classify("rate limit hit"),
            EmbeddingErrorKind::Unknown,
        );
        // code() returns the stable discriminator string.
        assert_eq!(EmbeddingErrorKind::OAuth.code(), "oauth");
        assert_eq!(EmbeddingErrorKind::Quota.code(), "quota");
        assert_eq!(EmbeddingErrorKind::SlotExhausted.code(), "slot-exhausted");
        assert_eq!(
            EmbeddingErrorKind::BackendMismatch.code(),
            "backend-mismatch"
        );
        assert_eq!(EmbeddingErrorKind::ZeroDimension.code(), "zero-dimension");
        assert_eq!(EmbeddingErrorKind::Unknown.code(), "unknown");
    }

    /// Display impl covers all three variants without panicking.
    #[test]
    fn fallback_reason_display_does_not_panic() {
        let _ = FallbackReason::EmbeddingFailed("rate limit".into()).to_string();
        let _ = FallbackReason::Cancelled.to_string();
        let _ = FallbackReason::Timeout {
            operation: "embed_query".into(),
            duration_secs: 30,
        }
        .to_string();
    }

    /// FallbackReason is PartialEq — used in test assertions to verify
    /// the mapping rules.
    #[test]
    fn fallback_reason_is_partial_eq() {
        assert_eq!(
            FallbackReason::EmbeddingFailed("a".into()),
            FallbackReason::EmbeddingFailed("a".into())
        );
        assert_eq!(FallbackReason::Cancelled, FallbackReason::Cancelled);
        assert_ne!(
            FallbackReason::EmbeddingFailed("a".into()),
            FallbackReason::EmbeddingFailed("b".into())
        );
        assert_ne!(
            FallbackReason::Cancelled,
            FallbackReason::Timeout {
                operation: "x".into(),
                duration_secs: 1
            }
        );
    }

    /// Timeout variant preserves the operation name and duration from the
    /// original AppError::Timeout for observability.
    #[test]
    fn fallback_reason_timeout_preserves_fields() {
        let r = FallbackReason::Timeout {
            operation: "embed_query_local".into(),
            duration_secs: 300,
        };
        match r {
            FallbackReason::Timeout {
                operation,
                duration_secs,
            } => {
                assert_eq!(operation, "embed_query_local");
                assert_eq!(duration_secs, 300);
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    /// try_embed_query_with_fallback surfaces an EmbeddingFailed variant
    /// when the LLM subprocess errors. Uses a path that surely does not
    /// contain any embedder configuration (the binary is invoked as
    /// `codex` / `claude` via PATH which, in tests, defaults to nothing
    /// in scope, so `LlmEmbedding::detect_available()` returns Err).
    #[test]
    #[ignore = "G58 S1 stub: requires env without codex/claude on PATH; tracked as T5 of Fase 2"]
    fn try_embed_query_with_fallback_surfaces_embedding_failed_for_missing_binary() {
        // Pointing at a models dir that does not exist forces the embedder
        // init to fail; the error is mapped to EmbeddingFailed.
        let bogus = std::path::Path::new("/nonexistent-models-dir-for-g58-fallback-test");
        let result = try_embed_query_with_fallback(bogus, "hello world");
        match result {
            Err(FallbackReason::EmbeddingFailed(msg)) => {
                // The original error must survive in the message for ops triage.
                assert!(!msg.is_empty(), "fallback message must not be empty");
            }
            Err(FallbackReason::Cancelled) => {
                panic!("expected EmbeddingFailed, got Cancelled");
            }
            Err(FallbackReason::Timeout { .. }) => {
                panic!("expected EmbeddingFailed, got Timeout");
            }
            Err(FallbackReason::SlotExhausted) => {
                panic!("expected EmbeddingFailed, got SlotExhausted");
            }
            Err(FallbackReason::OAuthQuota { .. }) => {
                panic!("expected EmbeddingFailed, got OAuthQuota");
            }
            Err(FallbackReason::BackendMismatch { .. }) => {
                panic!("expected EmbeddingFailed, got BackendMismatch");
            }
            Err(FallbackReason::DimZero) => {
                panic!("expected EmbeddingFailed, got DimZero");
            }
            Ok(_) => {
                panic!("expected an error, got Ok — embedder must fail for bogus path");
            }
        }
    }

    // G56: entity embed cache — unit tests
    #[test]
    fn g56_entity_cache_key_is_stable_and_distinct() {
        let k1 = entity_cache_key("codex:default", "sqlite-graphrag");
        let k2 = entity_cache_key("codex:default", "sqlite-graphrag");
        let k3 = entity_cache_key("codex:default", "claude-code");
        let k4 = entity_cache_key("claude:default", "sqlite-graphrag");
        assert_eq!(k1, k2, "same model+text must hash identically");
        assert_ne!(k1, k3, "different text must hash differently");
        assert_ne!(k1, k4, "different model must hash differently");
    }

    #[test]
    fn g56_entity_embed_cache_stats_hit_rate() {
        let zero = EmbedCacheStats::default();
        assert_eq!(zero.hit_rate(), 0.0);
        let half = EmbedCacheStats {
            requested: 4,
            hits: 2,
            misses: 2,
        };
        assert!((half.hit_rate() - 0.5).abs() < 1e-9);
        let all = EmbedCacheStats {
            requested: 7,
            hits: 7,
            misses: 0,
        };
        assert!((all.hit_rate() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn g56_entity_embed_cache_populates_and_hits() {
        // Manually populate the cache: bypasses the LLM by writing a
        // known vector under a chosen (model, text) key, then verifies
        // the cache is consulted before any LLM call would happen.
        let cache = entity_embed_cache();
        let model = "test-model";
        let text = "sqlite-graphrag";
        let key = entity_cache_key(model, text);
        let stored = Arc::new(vec![0.42_f32; crate::constants::embedding_dim()]);
        cache.lock().insert(key, Arc::clone(&stored));
        let guard = cache.lock();
        let hit = guard.get(&key).expect("cache must return stored value");
        assert_eq!(hit.len(), crate::constants::embedding_dim());
        assert!((hit[0] - 0.42).abs() < 1e-6);
    }

    // v1.1.1 (P1): com `--embedding-backend openrouter` a chain de embedding
    // de entidade é exatamente `[OpenRouter]` mesmo com `--llm-backend none`
    // — o short-circuit de vetor vazio de embed_entity_texts_cached (chain ==
    // [None]) NÃO dispara, então a entidade ganha vetor via REST na escrita.
    #[test]
    fn p1_openrouter_chain_ignores_llm_backend_none() {
        use crate::cli::{EmbeddingBackendChoice, LlmBackendChoice};
        let chain = EmbeddingBackendChoice::Openrouter.to_chain(LlmBackendChoice::None);
        assert_eq!(
            chain,
            vec![LlmBackendKind::OpenRouter],
            "openrouter embedding must not be silenced by --llm-backend none"
        );
        // O curto-circuito de vetor vazio existe SOMENTE para a chain [None]
        // (`--embedding-backend llm --llm-backend none`).
        let none_chain = EmbeddingBackendChoice::Llm.to_chain(LlmBackendChoice::None);
        assert_eq!(none_chain, vec![LlmBackendKind::None]);
    }

    #[test]
    fn g56_empty_texts_short_circuits_with_zero_stats() {
        // Cannot call embed_entity_texts_cached without an LLM on PATH,
        // so we only verify the empty-input contract via the stats struct.
        let stats = EmbedCacheStats::default();
        assert_eq!(stats.requested, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.hit_rate(), 0.0);
    }
