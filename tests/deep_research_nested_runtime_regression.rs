// Teste de regressão GAP-001 (v1.1.04): chamar o path de embedding síncrono
// de DENTRO de um runtime Tokio ativo NÃO deve panica com
// "Cannot start a runtime from within a runtime".
//
// Antes do fix A2, `embedder.rs` criava T2 via `shared_runtime().block_on()`
// na mesma thread dirigida por T1, causando nested-runtime panic sempre que
// o deep-research tentava embeddar sub-queries com T1 já ativo.
//
// Usamos `EmbeddingBackendChoice::Llm` + `LlmBackendChoice::None` para que a
// chain seja `[LlmBackendKind::None]` — `embed_with_fallback` retorna um vetor
// vazio sem subir subprocesso nem rede, então `try_embed_query_with_embedding_choice`
// devolve `Err(FallbackReason::DimZero)` em vez de panicar. O resultado não
// importa: o que importa é que a chamada retorna (Ok ou Err), nunca aborta.
//
// LIMITAÇÃO DE COBERTURA: este é um SMOKE-TEST de não-panic. O caminho `None`
// retorna cedo em `embedder.rs` (~linha 984) SEM exercitar as guardas
// `block_in_place` do caminho OpenRouter (single, serial batch, JoinSet
// fan-out), pois essas só disparam quando `OPENROUTER_CLIENT` está
// inicializado (o que exige rede e chave `OPENROUTER_API_KEY`). A proteção
// PRIMÁRIA contra o bug original é a A1 (`compute_sub_embeddings` ANTES de
// construir T1 em `deep_research.rs`); a A2 (guardas `block_in_place` no
// embedder) é defesa em profundidade para callers futuros. Exercitar o caminho
// OpenRouter exato exigiria um fixture de rede/key, fora do escopo do CI.

use sqlite_graphrag::cli::{EmbeddingBackendChoice, LlmBackendChoice};
use sqlite_graphrag::embedder::try_embed_query_with_embedding_choice;

#[test]
fn embedding_inside_active_runtime_does_not_panic() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    // Antes do fix isto panica. Agora deve retornar Ok ou Err (FallbackReason),
    // nunca abortar o processo.
    let outcome = std::panic::catch_unwind(|| {
        rt.block_on(async {
            let _ = try_embed_query_with_embedding_choice(
                std::path::Path::new("/tmp/nonexistent-models"),
                "query de teste",
                EmbeddingBackendChoice::Auto,
                LlmBackendChoice::None,
            );
        })
    });

    assert!(
        outcome.is_ok(),
        "embedding dentro de runtime ativo não deve panica (regressão GAP-001)"
    );
}
