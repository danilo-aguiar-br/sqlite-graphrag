// GAP-001 regression test (v1.1.04): calling the synchronous embedding path
// from INSIDE an active Tokio runtime must NOT panic with
// "Cannot start a runtime from within a runtime".
//
// Before the A2 fix, `embedder.rs` created T2 via `shared_runtime().block_on()`
// on the same thread driven by T1, causing a nested-runtime panic whenever
// deep-research tried to embed sub-queries with T1 already active.
//
// We use `EmbeddingBackendChoice::Llm` + `LlmBackendChoice::None` so that the
// chain is `[LlmBackendKind::None]` — `embed_with_fallback` returns an empty
// vector without spawning a subprocess or touching the network, so `try_embed_query_with_embedding_choice`
// returns `Err(FallbackReason::DimZero)` instead of panicking. The result does
// not matter: what matters is that the call returns (Ok or Err), never aborts.
//
// COVERAGE LIMITATION: this is a non-panic SMOKE TEST. The `None` path
// returns early in `embedder.rs` (~line 984) WITHOUT exercising the
// `block_in_place` guards of the OpenRouter path (single, serial batch, JoinSet
// fan-out), since those only fire when `OPENROUTER_CLIENT` is
// initialized (which requires network and the `OPENROUTER_API_KEY` key). The
// PRIMARY protection against the original bug is A1 (`compute_sub_embeddings` BEFORE
// building T1 in `deep_research.rs`); A2 (`block_in_place` guards in the
// embedder) is defense in depth for future callers. Exercising the exact
// OpenRouter path would require a network/key fixture, out of scope for CI.

use sqlite_graphrag::cli::{BackendChoice, EmbeddingBackendChoice, LlmBackendChoice};
use sqlite_graphrag::embedder::try_embed_query_with_embedding_choice;

#[test]
fn embedding_inside_active_runtime_does_not_panic() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    // Before the fix this panics. Now it must return Ok or Err (FallbackReason),
    // never abort the process.
    let outcome = std::panic::catch_unwind(|| {
        rt.block_on(async {
            let _ = try_embed_query_with_embedding_choice(
                std::path::Path::new("/tmp/nonexistent-models"),
                "query de teste",
                BackendChoice::new(LlmBackendChoice::None, EmbeddingBackendChoice::Auto),
            );
        })
    });

    assert!(
        outcome.is_ok(),
        "embedding dentro de runtime ativo não deve panica (regressão GAP-001)"
    );
}
