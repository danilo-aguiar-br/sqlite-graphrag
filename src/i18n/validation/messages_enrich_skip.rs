//! Reasons an enrich item was SKIPPED rather than processed (GAP-SG-279).
//!
//! These strings are not diagnostics. They travel in the `reason` field of the
//! item envelope, which is the only channel telling the caller why an item that
//! cost nothing produced nothing. Leaving them as English literals scattered
//! across the extraction modules made the same sentence exist in five places at
//! once — `"body is empty"` was duplicated verbatim in five files and
//! `"embedding backend returned an empty vector"` in three — so a wording fix
//! could land in one of them and silently disagree with the rest.

use crate::i18n::{current, Language};

/// The memory body is empty, so there is nothing to extract from.
pub fn body_is_empty() -> String {
    match current() {
        Language::English => "body is empty".to_string(),
        Language::Portuguese => "corpo vazio".to_string(),
    }
}

/// The chunk text is empty, so there is nothing to embed.
pub fn chunk_text_is_empty() -> String {
    match current() {
        Language::English => "chunk text is empty".to_string(),
        Language::Portuguese => "texto do chunk vazio".to_string(),
    }
}

/// The embedding chain resolved to no backend and produced no vector.
///
/// An empty vector is never persisted, so the item is skipped instead of
/// writing a row that would read as embedded while carrying nothing.
pub fn embedding_backend_returned_empty_vector() -> String {
    match current() {
        Language::English => {
            "embedding backend returned an empty vector (chain resolved to none)".to_string()
        }
        Language::Portuguese => {
            "backend de embedding devolveu vetor vazio (a cadeia resolveu para nenhum)".to_string()
        }
    }
}

/// The batched re-embed path produced no outcome for this key.
pub fn reembed_batch_no_outcome() -> String {
    match current() {
        Language::English => "re-embed batch produced no outcome for this key".to_string(),
        Language::Portuguese => {
            "o lote de re-embed não produziu resultado para esta chave".to_string()
        }
    }
}

/// Re-embed is claimed in batches, so the per-item path declines it.
pub fn reembed_served_by_batch_path() -> String {
    match current() {
        Language::English => "re-embed is served by the batched claim path".to_string(),
        Language::Portuguese => "o re-embed é atendido pelo caminho de claim em lote".to_string(),
    }
}

/// The entity pair was already judged and recorded in `entity_connect_seen`.
pub fn pair_already_seen() -> String {
    match current() {
        Language::English => "pair already in entity_connect_seen".to_string(),
        Language::Portuguese => "par já registrado em entity_connect_seen".to_string(),
    }
}

/// The entity pair already carries an edge in the graph.
pub fn pair_already_related() -> String {
    match current() {
        Language::English => "pair already related".to_string(),
        Language::Portuguese => "par já relacionado".to_string(),
    }
}

/// The model read the pair and reported no relationship between them.
pub fn llm_found_no_relationship() -> String {
    match current() {
        Language::English => "LLM determined no relationship".to_string(),
        Language::Portuguese => "o LLM determinou que não há relação".to_string(),
    }
}

/// GAP-SG-279: the entity carries no description, no linked corpus and no
/// typed neighbour, so its type cannot be judged from anything but its name.
///
/// This is the honest answer, not a failure. `entity-type-validate` used to
/// send the model two lines — the name and the type under dispute — and write
/// whatever came back. For an opaque name that is a guess wearing the costume
/// of an audit, and the guess reached `UPDATE entities SET type`. Abstaining
/// before the request also costs nothing, so the caller pays for evidence or
/// pays for nothing at all.
pub fn entity_type_no_evidence(corpus_chars: usize, min_corpus_chars: usize) -> String {
    match current() {
        Language::English => format!(
            "insufficient_evidence: entity has no description and only {corpus_chars} chars of \
             linked corpus, minimum is {min_corpus_chars} (bind it to a memory or describe it \
             before asking for its type)"
        ),
        Language::Portuguese => format!(
            "insufficient_evidence: a entidade não tem descrição e tem só {corpus_chars} \
             caracteres de corpus ligado, o mínimo é {min_corpus_chars} (ligue-a a uma memória \
             ou descreva-a antes de pedir o tipo)"
        ),
    }
}

/// GAP-SG-279: the model read the evidence and declined to judge the type.
pub fn entity_type_model_abstained(corpus_chars: usize) -> String {
    match current() {
        Language::English => format!(
            "insufficient_evidence: model declined to judge the type from {corpus_chars} chars \
             of evidence"
        ),
        Language::Portuguese => format!(
            "insufficient_evidence: o modelo recusou julgar o tipo a partir de {corpus_chars} \
             caracteres de evidência"
        ),
    }
}

/// GAP-SG-279: the suggested label failed shape normalisation.
///
/// The entity keeps its current type. Turning one unusable suggestion into a
/// failed item would have the queue retry it and eventually mark it dead,
/// trading a harmless no-op for a permanent failure.
pub fn entity_type_suggestion_unusable(suggested: &str) -> String {
    match current() {
        Language::English => {
            format!("unusable_suggestion: `{suggested}` failed shape normalisation; type kept")
        }
        Language::Portuguese => {
            format!(
                "unusable_suggestion: `{suggested}` falhou na normalização de forma; tipo mantido"
            )
        }
    }
}

/// GAP-SG-279: the model confirmed the type that was already stored.
pub fn entity_type_confirmed(current_type: &str) -> String {
    match current() {
        Language::English => format!("confirmed: `{current_type}` is already correct"),
        Language::Portuguese => format!("confirmado: `{current_type}` já está correto"),
    }
}

/// The model returned an explicit null where a description was expected.
///
/// The schema admits null precisely so the model has somewhere to put "the
/// evidence does not support any statement about this entity", so this is the
/// abstention path and not a malformed reply.
pub fn description_returned_null() -> String {
    match current() {
        Language::English => "insufficient_evidence: model returned a null description".to_string(),
        Language::Portuguese => {
            "insufficient_evidence: o modelo devolveu descrição nula".to_string()
        }
    }
}

/// The model returned a description made only of whitespace.
///
/// Kept distinct from the null case: an empty string is the model answering
/// while saying nothing, which is worth telling apart from it declining.
pub fn description_returned_empty() -> String {
    match current() {
        Language::English => {
            "insufficient_evidence: model returned an empty description".to_string()
        }
        Language::Portuguese => {
            "insufficient_evidence: o modelo devolveu descrição vazia".to_string()
        }
    }
}

/// Bounded busy-retry gave up while CLAIMING the next item to work on.
///
/// Distinct from the write-back message below on purpose: nothing was
/// processed, so nothing was lost. The operator's move is to retry later or
/// reduce concurrency, not to hunt for a half-applied change.
pub fn sqlite_busy_exhausted_on_dequeue() -> String {
    match current() {
        Language::English => {
            "SQLITE_BUSY exhausted bounded retries while dequeuing (parallel worker)".to_string()
        }
        Language::Portuguese => {
            "SQLITE_BUSY esgotou as tentativas limitadas ao retirar da fila (worker paralelo)"
                .to_string()
        }
    }
}

/// Bounded busy-retry gave up while CLAIMING a batch for re-embedding.
pub fn sqlite_busy_exhausted_on_reembed_claim() -> String {
    match current() {
        Language::English => {
            "SQLITE_BUSY exhausted bounded retries while claiming a re-embed batch".to_string()
        }
        Language::Portuguese => {
            "SQLITE_BUSY esgotou as tentativas limitadas ao reservar um lote de re-embed"
                .to_string()
        }
    }
}

/// The chat client was dispatched before it had been initialised.
///
/// Reachable only through a programming error in the dispatch order, which is
/// why the text says so: an operator who sees this cannot fix it by retrying or
/// by changing a flag, and telling them otherwise wastes their time.
pub fn chat_client_not_initialised() -> String {
    match current() {
        Language::English => {
            "OpenRouter chat client not initialised before dispatch (internal error)".to_string()
        }
        Language::Portuguese => {
            "cliente de chat da OpenRouter não inicializado antes do despacho (erro interno)"
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two abstention reasons must name the numbers that produced them.
    ///
    /// A reason that says only "insufficient evidence" tells the operator that
    /// something was missing without saying how much was there, which is the
    /// difference between a report and a shrug.
    #[test]
    fn the_abstention_reason_carries_the_measurement() {
        let msg = entity_type_no_evidence(12, 40);
        assert!(
            msg.contains("12"),
            "reason must carry the measured size: {msg}"
        );
        assert!(msg.contains("40"), "reason must carry the threshold: {msg}");
    }

    /// No reason may be empty, because an empty `reason` reaches the caller as
    /// a skip with no explanation at all — indistinguishable from a bug.
    ///
    /// The active language comes from a process-wide `OnceLock`, so this test
    /// exercises whichever one the harness resolved rather than looping over
    /// both. Every arm is a literal in the same `match`, so a missing
    /// translation is a compile error, not something a test could catch.
    #[test]
    fn no_reason_is_empty() {
        assert!(!body_is_empty().is_empty());
        assert!(!chunk_text_is_empty().is_empty());
        assert!(!embedding_backend_returned_empty_vector().is_empty());
        assert!(!reembed_batch_no_outcome().is_empty());
        assert!(!reembed_served_by_batch_path().is_empty());
        assert!(!pair_already_seen().is_empty());
        assert!(!pair_already_related().is_empty());
        assert!(!llm_found_no_relationship().is_empty());
        assert!(!entity_type_no_evidence(1, 2).is_empty());
        assert!(!entity_type_model_abstained(1).is_empty());
        assert!(!entity_type_suggestion_unusable("x").is_empty());
        assert!(!entity_type_confirmed("concept").is_empty());
    }

    /// The abstention reasons must carry the `insufficient_evidence` marker.
    ///
    /// The queue records the reason verbatim and operators grep it to tell an
    /// abstention from a refusal; a reworded prefix breaks that silently.
    #[test]
    fn the_abstention_reasons_carry_their_marker() {
        assert!(entity_type_no_evidence(0, 40).starts_with("insufficient_evidence:"));
        assert!(entity_type_model_abstained(0).starts_with("insufficient_evidence:"));
        assert!(entity_type_suggestion_unusable("x").starts_with("unusable_suggestion:"));
    }
}
