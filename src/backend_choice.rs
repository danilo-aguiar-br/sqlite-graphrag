//! LLM / embedding backend CLI choices (Wave C1).

/// LLM backend for embedding. Accepts `openrouter` (OpenRouter REST) or
/// `none` (skips embedding; useful for tests).
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum LlmBackendChoice {
    /// Open router variant.
    OpenRouter,
    /// None variant.
    None,
}

/// v1.0.93: embedding backend selector. Separate from `--llm-backend` which
/// controls enrichment (entity extraction, body enrichment).
/// `auto` uses OpenRouter when a client is initialised.
/// `openrouter` requires API key (exit 78 if absent).
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum EmbeddingBackendChoice {
    /// Auto variant.
    Auto,
    /// Openrouter variant.
    Openrouter,
}

impl EmbeddingBackendChoice {
    /// v1.0.93: produces a fallback chain that prepends OpenRouter when
    /// the client is initialised.
    pub fn to_chain(self, llm_choice: LlmBackendChoice) -> Vec<crate::embedder::LlmBackendKind> {
        use crate::embedder::LlmBackendKind;
        match self {
            EmbeddingBackendChoice::Openrouter => vec![LlmBackendKind::OpenRouter],
            EmbeddingBackendChoice::Auto => {
                if crate::embedder::is_openrouter_initialized() {
                    let mut chain = vec![LlmBackendKind::OpenRouter];
                    chain.extend(llm_choice.to_chain());
                    chain
                } else {
                    llm_choice.to_chain()
                }
            }
        }
    }
}

impl LlmBackendChoice {
    /// Converts the CLI choice into an ordered chain of backends that
    /// `embedder::embed_with_fallback` iterates. The first element of the
    /// chain is the preferred backend; subsequent elements are fallbacks
    /// used when the preferred one fails with `LlmBackendError`.
    pub fn to_chain(self) -> Vec<crate::embedder::LlmBackendKind> {
        use crate::embedder::LlmBackendKind;
        match self {
            LlmBackendChoice::OpenRouter => {
                vec![LlmBackendKind::OpenRouter, LlmBackendKind::None]
            }
            LlmBackendChoice::None => vec![LlmBackendKind::None],
        }
    }
}
