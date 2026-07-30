/// Embedding heavy must measure RAM.
pub fn embedding_heavy_must_measure_ram() -> String {
    "comando intensivo em embedding precisa medir RAM disponível".to_string()
}

/// Heavy command detected.
pub fn heavy_command_detected(available_mb: u64, safe_concurrency: usize) -> String {
    format!(
        "Comando pesado detectado; memória disponível: {available_mb} MB; \
         concorrência segura: {safe_concurrency}"
    )
}

/// Reducing concurrency.
pub fn reducing_concurrency(
    requested_concurrency: usize,
    effective_concurrency: usize,
) -> String {
    format!(
        "Reduzindo a concorrência solicitada de {requested_concurrency} para \
         {effective_concurrency} para evitar oversubscription de memória"
    )
}

/// Initializing embedding model.
pub fn initializing_embedding_model() -> &'static str {
    "Inicializando modelo de embedding (pode baixar na primeira execução)..."
}

/// Embedding chunks serially.
pub fn embedding_chunks_serially(count: usize) -> String {
    format!("Embedando {count} chunks serialmente para manter memória limitada...")
}

/// Remember step input validated.
pub fn remember_step_input_validated(available_mb: u64) -> String {
    format!("Etapa remember: entrada validada; memória disponível {available_mb} MB")
}

/// Remember step chunking completed.
pub fn remember_step_chunking_completed(
    total_passage_tokens: usize,
    model_max_length: usize,
    chunks_count: usize,
    rss_mb: u64,
) -> String {
    format!(
        "Etapa remember: tokenizer contou {total_passage_tokens} tokens de passagem \
         (máximo do modelo {model_max_length}); chunking gerou {chunks_count} chunks; \
         RSS do processo {rss_mb} MB"
    )
}

/// Remember step embeddings completed.
pub fn remember_step_embeddings_completed(rss_mb: u64) -> String {
    format!("Etapa remember: embeddings dos chunks concluídos; RSS do processo {rss_mb} MB")
}

/// Restore recomputing embedding.
pub fn restore_recomputing_embedding() -> &'static str {
    "Recalculando embedding da memória restaurada..."
}

/// Edit recomputing embedding.
pub fn edit_recomputing_embedding() -> &'static str {
    "Recalculando embedding da memória editada..."
}
