/// Localized `validation` message wrapping `msg`.
pub fn validation(msg: &str) -> String {
    format!("erro de validação: {msg}")
}

/// Localized `usage` message wrapping `msg`.
///
/// Kept distinct from [`validation`] because the two exit with different codes:
/// a validation failure is about the DATA and exits `1`, a usage failure is
/// about the REQUEST and exits `2`.
pub fn usage(msg: &str) -> String {
    format!("erro de uso: {msg}")
}

/// Localized `duplicate` message wrapping `msg`.
pub fn duplicate(msg: &str) -> String {
    let translated = msg
        .replace("already exists in namespace", "já existe no namespace")
        .replace(
            "exists but is soft-deleted in namespace",
            "existe mas está excluída temporariamente no namespace",
        )
        .replace(
            "Use --force-merge to update.",
            "Use --force-merge para atualizar.",
        )
        .replace(
            "use --force-merge to restore and update, or `restore` to revive it",
            "use --force-merge para restaurar e atualizar, ou `restore` para revivê-la",
        )
        .replace("memory", "memória");
    format!("duplicata detectada: {translated}")
}

/// Localized `conflict` message wrapping `msg`.
pub fn conflict(msg: &str) -> String {
    let translated = msg
        .replace("optimistic lock conflict", "conflito de lock otimista")
        .replace("but current is", "mas atual é")
        .replace(
            "was modified by another process",
            "foi modificada por outro processo",
        );
    format!("conflito: {translated}")
}

/// Localized `not_found` message wrapping `msg`.
pub fn not_found(msg: &str) -> String {
    // G55 T3: add replacements for the read.rs format produced by the
    // T1 fix: `memory not found: name='X' in namespace 'Y'`.
    // The existing chain did not catch ` in namespace '` when broken
    // by the name label, leaving a bilingual hybrid. New patterns
    // must run BEFORE the catch-all `memory` → `memória` to avoid
    // being shadowed.
    // GAP-SG-143: the chain now covers the whole
    // `crate::i18n::validation::messages_not_found` catalog. Rules are ordered
    // from most specific to most generic — a generic rule that runs early
    // shadows every specific rule after it, which is how the pre-G55 hybrid
    // was produced.
    //
    // Grammatical gender is decided by ONE structural cue: the catalog quotes
    // the subject's NAME (`memory 'x'`, `entity 'y'`) and leaves ids bare
    // (`chunk 3`, `entity id=9`). A quoted subject takes the feminine
    // agreement that `memória`/`entidade` require; a bare id falls to the
    // masculine default. Plain `str::replace` cannot see the noun across a
    // varying id, so a bare-id line whose noun is feminine keeps the masculine
    // form. That is a known agreement imperfection, deliberately preferred
    // over leaving English in the output.
    let translated = msg
        // -- endpoint-qualified entities, before the generic `entity` rule
        .replace("source entity '", "entidade de origem '")
        .replace("target entity '", "entidade de destino '")
        // -- multi-word phrases, before any single-word rule
        .replace("memory not found:", "memória não encontrada:")
        .replace(
            "exists but belongs to namespace",
            "existe mas pertence ao namespace",
        )
        .replace("' not found in namespace", "' não encontrada no namespace")
        .replace("not found in namespace", "não encontrado no namespace")
        .replace(
            "not found in pending_memories",
            "não encontrado em pending_memories",
        )
        .replace("not found for memory", "não encontrada para memória")
        .replace("does not exist in namespace", "não existe no namespace")
        .replace("memory or entity", "memória ou entidade")
        .replace("Did you mean:", "Você quis dizer:")
        .replace(
            "Re-run with --fuzzy to auto-resolve a clear match, or pass the canonical name.",
            "Repita com --fuzzy para resolver automaticamente uma correspondência clara, \
             ou informe o nome canônico.",
        )
        .replace(
            "is not held (no file at",
            "não está retido (nenhum arquivo em",
        )
        .replace("no key with fingerprint", "nenhuma chave com fingerprint")
        .replace("name='", "nome='")
        // -- nouns
        .replace("relationship", "relacionamento")
        .replace("edge '", "aresta '")
        .replace("memory", "memória")
        .replace("entity", "entidade")
        // -- trailing `not found`, quoted subject first (feminine agreement)
        .replace("' not found", "' não encontrada")
        .replace("not found", "não encontrado")
        // -- residual connectors
        .replace(", not '", ", e não '")
        .replace(" in namespace '", " no namespace '")
        .replace("version", "versão")
        .replace("soft-deleted", "excluída temporariamente");
    format!("não encontrado: {translated}")
}

// G55 S2 (v1.0.80): structured variant helpers. They synthesize the
// canonical English message and feed it through the `not_found`
// replace-chain so the pt-BR translation stays in one place.
/// Localized message for `memory_not_found`.
pub fn memory_not_found(name: &str, namespace: &str) -> String {
    not_found(&format!(
        "memory not found: name='{name}' in namespace '{namespace}'"
    ))
}

/// Localized message for `memory_not_found_by_id`.
pub fn memory_not_found_by_id(id: i64) -> String {
    not_found(&format!("memory not found: id={id}"))
}

// GAP-SG-78: transitory entity absence (materialized on a later enrich
// pass). Own pt-BR string, distinct from the terminal not-found chain.
/// Localized message for `entity_not_yet_materialized`.
pub fn entity_not_yet_materialized(name: &str, namespace: &str) -> String {
    format!("entidade '{name}' ainda não materializada no namespace '{namespace}'")
}

/// Localized message for `namespace_error`.
pub fn namespace_error(msg: &str) -> String {
    format!("namespace não resolvido: {msg}")
}

/// Localized message for `limit_exceeded`.
pub fn limit_exceeded(msg: &str) -> String {
    let translated = msg
        .replace("exceeds limit of", "excede limite de")
        .replace("body exceeds", "corpo excede")
        .replace("entities exceed limit", "entidades excedem limite")
        .replace(
            "relationships exceed limit",
            "relacionamentos excedem limite",
        );
    format!("limite excedido: {translated}")
}

// v1.1.1 (P11): typed ceiling variants. Own pt-BR strings mirroring
// the English `#[error]` text of `BodyTooLarge`/`TooManyChunks`,
// naming the constant so the operator knows WHICH cap fired.
/// Localized message for `body_too_large`.
pub fn body_too_large(bytes: u64, limit: u64) -> String {
    format!(
        "limite excedido: corpo tem {bytes} bytes, acima do teto de {limit} bytes \
         (MAX_MEMORY_BODY_LEN); divida o conteúdo em múltiplas memórias"
    )
}

/// Too many chunks.
pub fn too_many_chunks(chunks: usize, limit: usize) -> String {
    format!(
        "limite excedido: documento produz {chunks} chunks, acima do teto de {limit} \
         chunks (REMEMBER_MAX_SAFE_MULTI_CHUNKS); divida o documento antes da escrita"
    )
}

// v1.1.2 (Gap 2): third typed payload ceiling — token cap, mirroring
// the English `#[error]` text of `TooManyTokens`.
/// Too many tokens.
pub fn too_many_tokens(tokens: u64, limit: u64) -> String {
    format!(
        "limite excedido: corpo tem {tokens} tokens (estimado), acima do teto de \
         {limit} tokens (EMBEDDING_REQUEST_MAX_TOKENS); divida o conteúdo em \
         múltiplas memórias"
    )
}

/// Database.
pub fn database(err: &str) -> String {
    format!("erro de banco de dados: {err}")
}

/// Embedding.
pub fn embedding(msg: &str) -> String {
    format!("erro de embedding: {msg}")
}

/// VEC extension.
pub fn vec_extension(msg: &str) -> String {
    format!("extensão sqlite-vec falhou: {msg}")
}

/// Provider error.
pub fn provider_error(code: &str, message: &str) -> String {
    format!("erro do provedor (código {code}): {message}")
}

/// DB busy.
pub fn db_busy(msg: &str) -> String {
    format!("banco ocupado: {msg}")
}

/// Batch partial failure.
pub fn batch_partial_failure(total: usize, failed: usize) -> String {
    format!("falha parcial em batch: {failed} de {total} itens falharam")
}

/// IO.
pub fn io(err: &str) -> String {
    format!("erro de I/O: {err}")
}

/// Internal.
pub fn internal(err: &str) -> String {
    format!("erro interno: {err}")
}

/// JSON.
pub fn json(err: &str) -> String {
    format!("erro de JSON: {err}")
}

/// Lock busy.
pub fn lock_busy(msg: &str) -> String {
    format!("lock ocupado: {msg}")
}

/// All slots full.
pub fn all_slots_full(max: usize, waited_secs: u64) -> String {
    format!(
        "todos os {max} slots de concorrência ocupados após aguardar {waited_secs}s \
         (exit 75); use --max-concurrency ou aguarde outras invocações terminarem"
    )
}

/// Job singleton locked.
pub fn job_singleton_locked(job_type: &str, namespace: &str) -> String {
    format!(
        "job {job_type} para o namespace '{namespace}' já está em execução (exit 75); \
         aguarde a conclusão ou passe --wait-job-singleton <SEGUNDOS>"
    )
}

/// Embedding singleton locked.
pub fn embedding_singleton_locked(namespace: &str) -> String {
    format!(
        "singleton de embedding para o namespace '{namespace}' já está retido (exit 75); \
         outra CLI está chamando o LLM neste banco; passe --wait-embed-singleton <SEGUNDOS> para aguardar"
    )
}

/// Low memory.
pub fn low_memory(available_mb: u64, required_mb: u64) -> String {
    format!(
        "memória disponível ({available_mb}MB) abaixo do mínimo requerido ({required_mb}MB) \
         para carregar o modelo; aborte outras cargas ou use --skip-memory-guard (exit 77)"
    )
}

/// Shutdown.
pub fn shutdown(signal: &str) -> String {
    format!("sinal de desligamento recebido: {signal}; operação cancelada pelo usuário (exit 19)")
}

// `preflight_failed` lived here until v1.2.3. It localised the exit-16 MCP
// preflight error for a `src/spawn/` module and an `AppError` variant that no
// longer exist, and it advertised `config set spawn.skip_preflight=1` — a key
// that was itself removed in the same change. A message nothing can emit is a
// translated promise about a code path the product no longer has.
/// Localized message for `binary_not_found`.
pub fn binary_not_found(name: &str) -> String {
    format!("binário não encontrado: {name} — instale e adicione ao PATH")
}

/// Rate limited.
pub fn rate_limited(detail: &str) -> String {
    format!("taxa de requisição excedida: {detail}")
}

/// Timeout.
pub fn timeout(operation: &str, secs: u64) -> String {
    format!("timeout após {secs}s: {operation}")
}
