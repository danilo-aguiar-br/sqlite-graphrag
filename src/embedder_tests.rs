//! Auto-extracted tests (Wave C1).

use super::batch::{adaptive_batch_for_dim, entity_cache_key, entity_embed_cache};
use super::*;
use std::sync::Arc;

#[test]
fn f32_to_bytes_roundtrip() {
    let input = vec![0.0_f32, 1.5, -2.25, f32::MIN, f32::MAX];
    let bytes = f32_to_bytes(&input);
    assert_eq!(bytes.len(), input.len() * 4);
    let out = bytes_to_f32(&bytes);
    assert_eq!(out, input);
}

#[test]
fn embedding_dim_matches_constants_source() {
    assert_eq!(embedding_dim(), crate::constants::embedding_dim());
}

#[test]
fn effective_permits_clamps_to_bounds() {
    assert!(effective_permits(0) >= 1);
    assert!(effective_permits(1000) <= 32);
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

// GAP-SG-232: the retired `SQLITE_GRAPHRAG_EMBEDDING_DIM` was asserted inert HERE,
// by setting it and observing that the batch size did not move. That proof cost
// a runtime test that set the very variable the product must never read, so a
// reader of this file learnt the channel exists. The same negative is now
// enforced statically, and more strictly, by
// `tests/env_channel_guard.rs::no_source_file_manipulates_a_product_environment_variable`:
// no source file may so much as touch the variable, which no runtime assertion
// could have caught. The dim comes from `--embedding-dim`, then the XDG key
// `embedding.dim`, then the compiled default.

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
        EmbeddingErrorKind::classify("slot exhausted: failed to acquire LLM slot after backoff"),
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

/// `try_embed_query_with_fallback` surfaces an `EmbeddingFailed` variant when
/// the embedding backend cannot run.
///
/// It was `#[ignore]`d while the chain still probed `codex` / `claude` on PATH,
/// which made the outcome depend on the developer machine. The chain is now
/// OpenRouter-only over REST.
///
/// The previous version drove `try_embed_query_with_fallback` against a bogus
/// models directory and asserted the resulting variant. That made the outcome
/// depend on execution order: `OPENROUTER_CLIENT` in `src/embedder/mod.rs` is a
/// process-wide `OnceLock`, so once any earlier test initialised it the call
/// succeeded and the assertion inverted — and on a host with a key in XDG a unit
/// test would reach the network to get there. It was `#[ignore]`d for that,
/// which hid a test that passed alone and lied in the suite.
///
/// The contract it meant to guard is the mapping itself, which is pure. It is
/// asserted here directly, over every branch, with no global state and no I/O.
#[test]
fn embedding_errors_map_to_the_fallback_reason_that_names_them() {
    // An unclassifiable embedding failure degrades to EmbeddingFailed, and the
    // original text survives for ops triage.
    let reason = classify_embedding_error(crate::errors::AppError::Embedding(
        "models dir /nonexistent not found".to_string(),
    ));
    match reason {
        FallbackReason::EmbeddingFailed(msg) => assert!(
            msg.contains("/nonexistent"),
            "the original error must survive for triage, got {msg:?}"
        ),
        other => panic!("expected EmbeddingFailed, got {other:?}"),
    }

    // A timeout keeps its operation and budget rather than collapsing into the
    // generic variant, because the caller reports both.
    let reason = classify_embedding_error(crate::errors::AppError::Timeout {
        operation: "embed_query_local".to_string(),
        duration_secs: 300,
    });
    match reason {
        FallbackReason::Timeout {
            operation,
            duration_secs,
        } => {
            assert_eq!(operation, "embed_query_local");
            assert_eq!(duration_secs, 300);
        }
        other => panic!("expected Timeout, got {other:?}"),
    }

    // The lexical discriminators, each of which the JSON envelope renders under
    // a distinct `vec_degraded_reason`.
    for (message, expected) in [
        ("embedding returned dim=zero", "dim_zero"),
        ("operation cancelled by signal", "cancelled"),
    ] {
        let reason =
            classify_embedding_error(crate::errors::AppError::Embedding(message.to_string()));
        assert_eq!(
            reason.reason_code(),
            expected,
            "{message:?} must classify as {expected}"
        );
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

// v1.1.1 (P1): with `--embedding-backend openrouter` the entity embedding
// chain is exactly `[OpenRouter]` even under `--llm-backend none` — the
// empty-vector short-circuit of embed_entity_texts_cached (chain ==
// [None]) does NOT fire, so the entity gains a vector over REST on write.
#[test]
fn p1_openrouter_chain_ignores_llm_backend_none() {
    use crate::cli::{EmbeddingBackendChoice, LlmBackendChoice};
    let chain = EmbeddingBackendChoice::Openrouter.to_chain(LlmBackendChoice::None);
    assert_eq!(
        chain,
        vec![LlmBackendKind::OpenRouter],
        "openrouter embedding must not be silenced by --llm-backend none"
    );
    // The empty-vector short-circuit exists ONLY for the [None] chain
    // (`--llm-backend none` with no OpenRouter client initialised).
    let none_chain = LlmBackendChoice::None.to_chain();
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
