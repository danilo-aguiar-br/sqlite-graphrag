//! Embedding error message catalog (GAP-SG-132).
//!
//! EN+PT user-facing strings for `AppError::Embedding(String)`.
//!
//! `EmbeddingErrorKind::classify` matches English substrings case-insensitively:
//! `oauth`, `quota`, `slot exhausted`, `backend mismatch`, and (`dim` + `zero`).
//! Portuguese templates MUST retain those English markers so classification
//! continues to work under either locale.

use crate::i18n::{current, Language};
use std::fmt::Display;

// ── generic wrapper ──────────────────────────────────────────────────────────

/// Wrap an external / dynamic error detail as an embedding payload.
pub fn embedding_detail(err: impl Display) -> String {
    match current() {
        Language::English => format!("{err}"),
        Language::Portuguese => format!("{err}"),
    }
}

// ── OpenRouter HTTP / client ─────────────────────────────────────────────────

/// Failed to build the reqwest HTTP client for OpenRouter embeddings.
pub fn embedding_http_client_build_failed(err: impl Display) -> String {
    match current() {
        Language::English => format!("failed to build HTTP client: {err}"),
        Language::Portuguese => format!("falha ao construir cliente HTTP: {err}"),
    }
}

/// Empty `data` array in a successful OpenRouter embedding response.
pub fn embedding_empty_response_from_openrouter() -> String {
    match current() {
        Language::English => "empty response from OpenRouter".to_string(),
        Language::Portuguese => "resposta vazia do OpenRouter".to_string(),
    }
}

/// Batch response length mismatch.
pub fn embedding_expected_count(expected: usize, got: usize) -> String {
    match current() {
        Language::English => format!("expected {expected} embeddings, got {got}"),
        Language::Portuguese => format!("esperados {expected} embeddings, obtidos {got}"),
    }
}

/// Provider returned fewer dimensions than requested.
pub fn embedding_dimension_less_than_requested(got: usize, requested: usize) -> String {
    match current() {
        Language::English => format!("embedding dimension {got} < requested {requested}"),
        Language::Portuguese => {
            format!("dimensão de embedding {got} < solicitada {requested}")
        }
    }
}

/// OpenRouter embedding request timed out.
pub fn embedding_openrouter_request_timed_out() -> String {
    match current() {
        Language::English => "OpenRouter request timed out".to_string(),
        Language::Portuguese => "requisição OpenRouter expirou (timeout)".to_string(),
    }
}

/// Transient network failure during embedding HTTP.
pub fn embedding_http_request_failed(err: impl Display) -> String {
    match current() {
        Language::English => format!("HTTP request failed: {err}"),
        Language::Portuguese => format!("requisição HTTP falhou: {err}"),
    }
}

/// Failed to read embedding response body.
pub fn embedding_failed_to_read_response_body(err: impl Display) -> String {
    match current() {
        Language::English => format!("failed to read response body: {err}"),
        Language::Portuguese => format!("falha ao ler corpo da resposta: {err}"),
    }
}

/// HTTP 200 body had neither `data` nor `error`.
pub fn embedding_openrouter_200_neither_data_nor_error() -> String {
    match current() {
        Language::English => "OpenRouter 200 response had neither data nor error".to_string(),
        Language::Portuguese => "resposta OpenRouter 200 sem data nem error".to_string(),
    }
}

/// Failed to parse embedding JSON body.
pub fn embedding_failed_to_parse_response(err: impl Display) -> String {
    match current() {
        Language::English => format!("failed to parse embedding response: {err}"),
        Language::Portuguese => format!("falha ao analisar resposta de embedding: {err}"),
    }
}

/// Invalid OpenRouter API key (HTTP 401).
pub fn embedding_openrouter_invalid_api_key_401() -> String {
    match current() {
        Language::English => "invalid OpenRouter API key (HTTP 401)".to_string(),
        Language::Portuguese => "chave de API OpenRouter inválida (HTTP 401)".to_string(),
    }
}

/// OpenRouter hard-failure status with body.
pub fn embedding_openrouter_returned(status: impl Display, body: &str) -> String {
    match current() {
        Language::English => format!("OpenRouter returned {status}: {body}"),
        Language::Portuguese => format!("OpenRouter retornou {status}: {body}"),
    }
}

/// OpenRouter 5xx during embedding.
pub fn embedding_openrouter_server_error(status: impl Display) -> String {
    match current() {
        Language::English => format!("OpenRouter server error: {status}"),
        Language::Portuguese => format!("erro de servidor OpenRouter: {status}"),
    }
}

/// Unexpected HTTP status with body snippet.
pub fn embedding_unexpected_http(status: impl Display, body: &str) -> String {
    match current() {
        Language::English => format!("unexpected HTTP {status}: {body}"),
        Language::Portuguese => format!("HTTP inesperado {status}: {body}"),
    }
}

/// Max retries exhausted for OpenRouter embedding request.
pub fn embedding_openrouter_max_retries() -> String {
    match current() {
        Language::English => "max retries exceeded for OpenRouter request".to_string(),
        Language::Portuguese => {
            "número máximo de tentativas excedido para requisição OpenRouter".to_string()
        }
    }
}

// ── runtime / singleton getters ──────────────────────────────────────────────

/// Tokio multi-thread runtime failed to initialise.
pub fn embedding_tokio_runtime_init_failed(err: impl Display) -> String {
    match current() {
        Language::English => format!("tokio runtime init failed: {err}"),
        Language::Portuguese => format!("falha na inicialização do runtime tokio: {err}"),
    }
}

/// Tokio runtime missing after set.
pub fn embedding_tokio_runtime_unavailable() -> String {
    match current() {
        Language::English => "tokio runtime unavailable after initialisation".to_string(),
        Language::Portuguese => "runtime tokio indisponível após inicialização".to_string(),
    }
}

/// Default embedder singleton missing after set.
pub fn embedding_embedder_unavailable() -> String {
    match current() {
        Language::English => "embedder unavailable after initialisation".to_string(),
        Language::Portuguese => "embedder indisponível após inicialização".to_string(),
    }
}

/// Claude embedder singleton missing after set.
pub fn embedding_claude_embedder_unavailable() -> String {
    match current() {
        Language::English => "claude embedder unavailable after initialisation".to_string(),
        Language::Portuguese => "embedder claude indisponível após inicialização".to_string(),
    }
}

/// OpenCode embedder singleton missing after set.
pub fn embedding_opencode_embedder_unavailable() -> String {
    match current() {
        Language::English => "opencode embedder unavailable after initialisation".to_string(),
        Language::Portuguese => "embedder opencode indisponível após inicialização".to_string(),
    }
}

/// OpenRouter embed client singleton missing after set.
pub fn embedding_openrouter_client_unavailable() -> String {
    match current() {
        Language::English => "openrouter client unavailable after initialisation".to_string(),
        Language::Portuguese => "cliente openrouter indisponível após inicialização".to_string(),
    }
}

/// OpenRouter chat client singleton missing after set.
pub fn embedding_openrouter_chat_client_unavailable() -> String {
    match current() {
        Language::English => "openrouter chat client unavailable after initialisation".to_string(),
        Language::Portuguese => {
            "cliente de chat openrouter indisponível após inicialização".to_string()
        }
    }
}

// ── backend probes ───────────────────────────────────────────────────────────

/// OpenRouter probe: client not initialised.
pub fn embedding_openrouter_probe_not_initialised() -> String {
    match current() {
        Language::English => "openrouter probe: client not initialised (skip)".to_string(),
        Language::Portuguese => "probe openrouter: cliente não inicializado (skip)".to_string(),
    }
}

/// Codex probe: binary missing from PATH.
pub fn embedding_codex_probe_binary_not_on_path() -> String {
    match current() {
        Language::English => "codex probe: binary not on PATH (skip)".to_string(),
        Language::Portuguese => "probe codex: binário não está no PATH (skip)".to_string(),
    }
}

/// Codex probe: auth.json missing.
pub fn embedding_codex_probe_auth_missing() -> String {
    match current() {
        Language::English => {
            "codex probe: auth.json missing (skip; use --llm-backend none or login)".to_string()
        }
        Language::Portuguese => {
            "probe codex: auth.json ausente (skip; use --llm-backend none ou login)".to_string()
        }
    }
}

/// Claude probe: binary missing from PATH.
pub fn embedding_claude_probe_binary_not_on_path() -> String {
    match current() {
        Language::English => "claude probe: binary not on PATH (skip)".to_string(),
        Language::Portuguese => "probe claude: binário não está no PATH (skip)".to_string(),
    }
}

/// OpenCode probe: binary missing from PATH.
pub fn embedding_opencode_probe_binary_not_on_path() -> String {
    match current() {
        Language::English => "opencode probe: binary not on PATH (skip)".to_string(),
        Language::Portuguese => "probe opencode: binário não está no PATH (skip)".to_string(),
    }
}

/// OpenRouter client not initialised for embed path.
pub fn embedding_openrouter_client_not_initialised() -> String {
    match current() {
        Language::English => {
            "OpenRouter client not initialised; call get_openrouter_embedder first".to_string()
        }
        Language::Portuguese => {
            "cliente OpenRouter não inicializado; chame get_openrouter_embedder primeiro"
                .to_string()
        }
    }
}

// ── dimension / validation ───────────────────────────────────────────────────

/// Vector length diverges from configured embedding dim (G42/C5).
pub fn embedding_has_dims_expected(got: usize, expected: usize) -> String {
    match current() {
        Language::English => format!(
            "embedding has {got} dims, expected {expected}; \
             refusing to truncate or pad silently (G42/C5)"
        ),
        Language::Portuguese => format!(
            "embedding tem {got} dims, esperado {expected}; \
             recusando truncar ou preencher silenciosamente (G42/C5)"
        ),
    }
}

/// LLM returned wrong dim for a specific fan-out item.
pub fn embedding_llm_item_dims(got: usize, idx: usize, expected: usize) -> String {
    match current() {
        Language::English => format!(
            "LLM returned {got} dims for item {idx}, expected {expected}; \
             refusing to truncate or pad silently (G42/C5)"
        ),
        Language::Portuguese => format!(
            "LLM retornou {got} dims para item {idx}, esperado {expected}; \
             recusando truncar ou preencher silenciosamente (G42/C5)"
        ),
    }
}

/// KNN search dim mismatch (memories / entities).
pub fn embedding_knn_search_dim_mismatch(got: usize, expected: usize) -> String {
    match current() {
        Language::English => {
            format!("knn_search embedding has {got} dims, expected {expected}")
        }
        Language::Portuguese => {
            format!("embedding knn_search tem {got} dims, esperado {expected}")
        }
    }
}

/// KNN search dim mismatch (chunks).
pub fn embedding_knn_search_chunks_dim_mismatch(got: usize, expected: usize) -> String {
    match current() {
        Language::English => {
            format!("knn_search_chunks embedding has {got} dims, expected {expected}")
        }
        Language::Portuguese => {
            format!("embedding knn_search_chunks tem {got} dims, esperado {expected}")
        }
    }
}

// ── slot / fan-out / batch ───────────────────────────────────────────────────

/// LLM slot semaphore exhausted (marker: `slot exhausted`).
pub fn embedding_slot_exhausted(err: impl Display) -> String {
    match current() {
        Language::English => format!("slot exhausted: {err} (fall back to FTS5)"),
        // Keep English marker "slot exhausted" for EmbeddingErrorKind::classify.
        Language::Portuguese => {
            format!("slot exhausted: {err} (fall back para FTS5)")
        }
    }
}

/// JoinSet join error while embedding.
pub fn embedding_task_join_error(err: impl Display) -> String {
    match current() {
        Language::English => format!("embedding task join error: {err}"),
        Language::Portuguese => format!("erro de join na tarefa de embedding: {err}"),
    }
}

/// Entity embed cache produced a null slot.
pub fn embedding_entity_cache_null() -> String {
    match current() {
        Language::English => "entity embed cache produced null result".to_string(),
        Language::Portuguese => "cache de embed de entidade produziu resultado nulo".to_string(),
    }
}

/// Fan-out lost an item index.
pub fn embedding_fanout_lost_index(idx: usize) -> String {
    match current() {
        Language::English => format!("embedding fan-out lost item index {idx}"),
        Language::Portuguese => {
            format!("fan-out de embedding perdeu índice do item {idx}")
        }
    }
}

/// Semaphore closed mid fan-out.
pub fn embedding_semaphore_closed() -> String {
    match current() {
        Language::English => "semaphore closed".to_string(),
        Language::Portuguese => "semáforo fechado".to_string(),
    }
}

/// Embedding cancelled by shutdown signal (keep "cancelled" for batch matcher).
pub fn embedding_cancelled_by_shutdown() -> String {
    match current() {
        Language::English => "embedding cancelled by shutdown signal".to_string(),
        // Keep English "cancelled" so batch collector still matches.
        Language::Portuguese => "embedding cancelled por sinal de shutdown".to_string(),
    }
}

/// Embedding JoinSet task panicked.
pub fn embedding_task_panicked(err: impl Display) -> String {
    match current() {
        Language::English => format!("embedding task panicked: {err}"),
        Language::Portuguese => format!("tarefa de embedding entrou em pânico: {err}"),
    }
}

// ── LLM subprocess embedding ─────────────────────────────────────────────────

/// No codex/claude/opencode on PATH.
pub fn embedding_no_llm_cli_on_path() -> String {
    match current() {
        Language::English => {
            "no LLM CLI found on PATH: install `codex` (0.130+), `claude` (Claude Code 2.1+), or `opencode` (1.17+)"
                .to_string()
        }
        Language::Portuguese => {
            "nenhum CLI de LLM encontrado no PATH: instale `codex` (0.130+), `claude` (Claude Code 2.1+) ou `opencode` (1.17+)"
                .to_string()
        }
    }
}

/// Named binary not found on PATH.
pub fn embedding_binary_not_found_on_path(which_name: &str) -> String {
    match current() {
        Language::English => format!("`{which_name}` not found on PATH"),
        Language::Portuguese => format!("`{which_name}` não encontrado no PATH"),
    }
}

/// LLM batch embedding JSON parse failed.
pub fn embedding_llm_batch_parse_failed(err: impl Display, raw: &str) -> String {
    match current() {
        Language::English => {
            format!("LLM batch embedding response parse failed: {err}; raw={raw}")
        }
        Language::Portuguese => {
            format!("falha ao analisar resposta de embedding em lote LLM: {err}; raw={raw}")
        }
    }
}

/// LLM batch returned wrong item count.
pub fn embedding_llm_batch_item_count(got: usize, expected: usize) -> String {
    match current() {
        Language::English => {
            format!("LLM batch returned {got} items, expected {expected} (G42/S2 coverage check)")
        }
        Language::Portuguese => format!(
            "lote LLM retornou {got} itens, esperados {expected} (verificação de cobertura G42/S2)"
        ),
    }
}

/// LLM batch item index out of range.
pub fn embedding_llm_batch_index_out_of_range(i: usize, max: usize) -> String {
    match current() {
        Language::English => {
            format!("LLM batch item index {i} out of range 1..={max}")
        }
        Language::Portuguese => {
            format!("índice de item do lote LLM {i} fora do intervalo 1..={max}")
        }
    }
}

/// LLM batch item dimension mismatch (G42/C5).
pub fn embedding_llm_batch_item_dims(i: usize, got: usize, expected: usize) -> String {
    match current() {
        Language::English => format!(
            "LLM batch item {i} returned {got} dims, expected {expected}; \
             refusing to truncate or pad silently (G42/C5)"
        ),
        Language::Portuguese => format!(
            "item {i} do lote LLM retornou {got} dims, esperado {expected}; \
             recusando truncar ou preencher silenciosamente (G42/C5)"
        ),
    }
}

/// LLM batch response missing an item index.
pub fn embedding_llm_batch_missing_item(index: usize) -> String {
    match current() {
        Language::English => {
            format!("LLM batch response is missing item index {index} (G42/S2 coverage check)")
        }
        Language::Portuguese => format!(
            "resposta do lote LLM sem índice de item {index} (verificação de cobertura G42/S2)"
        ),
    }
}

/// Single LLM embedding JSON parse failed.
pub fn embedding_llm_parse_failed(err: impl Display, raw: &str) -> String {
    match current() {
        Language::English => {
            format!("LLM embedding response parse failed: {err}; raw={raw}")
        }
        Language::Portuguese => {
            format!("falha ao analisar resposta de embedding LLM: {err}; raw={raw}")
        }
    }
}

/// Single LLM embedding dimension mismatch (G42/C5).
pub fn embedding_llm_returned_dims(got: usize, expected: usize) -> String {
    match current() {
        Language::English => format!(
            "LLM returned {got} dims, expected {expected}; \
             refusing to truncate or pad silently (G42/C5)"
        ),
        Language::Portuguese => format!(
            "LLM retornou {got} dims, esperado {expected}; \
             recusando truncar ou preencher silenciosamente (G42/C5)"
        ),
    }
}

/// Schema tempfile create failed.
pub fn embedding_schema_tempfile_create_failed(err: impl Display) -> String {
    match current() {
        Language::English => format!("schema tempfile create failed: {err}"),
        Language::Portuguese => {
            format!("falha ao criar tempfile de schema: {err}")
        }
    }
}

/// Schema tempfile write failed.
pub fn embedding_schema_tempfile_write_failed(err: impl Display) -> String {
    match current() {
        Language::English => format!("schema tempfile write failed: {err}"),
        Language::Portuguese => {
            format!("falha ao escrever tempfile de schema: {err}")
        }
    }
}

/// Claude OAuth usage quota exhausted (markers: `OAuth`, `quota`).
pub fn embedding_oauth_usage_quota_exhausted_claude(snippet: &str) -> String {
    match current() {
        // Keep English "OAuth" + "quota" markers for EmbeddingErrorKind::classify.
        Language::English => {
            format!("OAuth usage quota exhausted: claude rate_limit detected in stdout: {snippet}")
        }
        Language::Portuguese => format!(
            "OAuth usage quota exhausted: rate_limit do claude detectado no stdout: {snippet}"
        ),
    }
}

/// Codex stdin write failed.
pub fn embedding_codex_stdin_write_failed(err: impl Display) -> String {
    match current() {
        Language::English => format!("codex stdin write failed: {err}"),
        Language::Portuguese => format!("falha ao escrever no stdin do codex: {err}"),
    }
}

// ── dry-run backend guards ───────────────────────────────────────────────────

/// `--llm-backend codex` but codex missing (claude detected).
pub fn embedding_dry_run_codex_not_on_path() -> String {
    match current() {
        Language::English => "`--llm-backend codex` requested but `codex` was not found on PATH \
             (a `claude` binary was detected; refusing silent fallback per ADR-0042). \
             Install `codex` (>= 0.130) or pass `--llm-backend claude` explicitly."
            .to_string(),
        Language::Portuguese => {
            "`--llm-backend codex` solicitado mas `codex` não encontrado no PATH \
             (binário `claude` detectado; recusando fallback silencioso por ADR-0042). \
             Instale `codex` (>= 0.130) ou passe `--llm-backend claude` explicitamente."
                .to_string()
        }
    }
}

/// `--llm-backend claude` but claude missing (codex detected).
pub fn embedding_dry_run_claude_not_on_path() -> String {
    match current() {
        Language::English => "`--llm-backend claude` requested but `claude` was not found on PATH \
             (a `codex` binary was detected; refusing silent fallback per ADR-0042). \
             Install `claude` (Claude Code >= 2.1) or pass `--llm-backend codex` explicitly."
            .to_string(),
        Language::Portuguese => {
            "`--llm-backend claude` solicitado mas `claude` não encontrado no PATH \
             (binário `codex` detectado; recusando fallback silencioso por ADR-0042). \
             Instale `claude` (Claude Code >= 2.1) ou passe `--llm-backend codex` explicitamente."
                .to_string()
        }
    }
}

/// `--llm-backend opencode` but auto-detect resolved another flavour.
pub fn embedding_dry_run_opencode_resolved_other(flavour: &str) -> String {
    match current() {
        Language::English => format!(
            "`--llm-backend opencode` requested but auto-detect resolved `{flavour}` \
             (opencode has lower priority than codex/claude in detect_available). \
             Pass `--llm-backend auto`, `--opencode-binary <path>`, or \
             `config set llm.opencode_binary <path>`."
        ),
        Language::Portuguese => format!(
            "`--llm-backend opencode` solicitado mas auto-detect resolveu `{flavour}` \
             (opencode tem prioridade menor que codex/claude em detect_available). \
             Passe `--llm-backend auto`, `--opencode-binary <path>`, ou \
             `config set llm.opencode_binary <path>`."
        ),
    }
}

/// `--llm-backend opencode` but opencode not on PATH.
pub fn embedding_dry_run_opencode_not_on_path() -> String {
    match current() {
        Language::English => {
            "`--llm-backend opencode` requested but `opencode` was not found on PATH. \
             Install `opencode` (>= 1.17) or pass `--llm-backend auto` to auto-detect."
                .to_string()
        }
        Language::Portuguese => {
            "`--llm-backend opencode` solicitado mas `opencode` não encontrado no PATH. \
             Instale `opencode` (>= 1.17) ou passe `--llm-backend auto` para auto-detect."
                .to_string()
        }
    }
}

// ── opencode runner ──────────────────────────────────────────────────────────

/// OpenCode NDJSON had no text events.
pub fn embedding_opencode_no_text_events() -> String {
    match current() {
        Language::English => "opencode returned no text events in NDJSON output".to_string(),
        Language::Portuguese => {
            "opencode não retornou eventos de texto na saída NDJSON".to_string()
        }
    }
}

/// OpenCode timed out.
pub fn embedding_opencode_timed_out(timeout_secs: u64) -> String {
    match current() {
        Language::English => format!("opencode timed out after {timeout_secs}s"),
        Language::Portuguese => {
            format!("opencode expirou (timeout) após {timeout_secs}s")
        }
    }
}

/// Failed to spawn opencode.
pub fn embedding_failed_to_spawn_opencode(err: impl Display) -> String {
    match current() {
        Language::English => format!("failed to spawn opencode: {err}"),
        Language::Portuguese => format!("falha ao iniciar opencode: {err}"),
    }
}

/// OpenCode non-zero exit with tails.
pub fn embedding_opencode_exited(status: impl Display, stderr: &str, stdout: &str) -> String {
    match current() {
        Language::English => {
            format!("opencode exited with {status}: stderr={stderr}, stdout={stdout}")
        }
        Language::Portuguese => {
            format!("opencode saiu com {status}: stderr={stderr}, stdout={stdout}")
        }
    }
}

/// OpenCode JSON parse failed.
pub fn embedding_opencode_json_parse_failed(err: impl Display) -> String {
    match current() {
        Language::English => format!("opencode JSON parse failed: {err}"),
        Language::Portuguese => format!("falha ao analisar JSON do opencode: {err}"),
    }
}
