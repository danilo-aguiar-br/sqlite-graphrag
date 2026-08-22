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

/// The `llm` + `embedding` pair, named once.
///
/// GAP-SG-265: these two selectors are resolved together in `main`, travel
/// together down every write path, and are consumed together by
/// `embed_passage_with_embedding_choice`. Passing them as two positional
/// parameters cost one argument slot in each of the signatures that carry them,
/// and several of those signatures sat one slot over the
/// `clippy::too_many_arguments` threshold for exactly that reason.
///
/// This is a plain aggregate on purpose: the two fields keep their own types,
/// so nothing that reads them changes, and the struct is `Copy` so threading it
/// through a call chain stays as cheap as threading the two enums was.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BackendChoice {
    /// Which LLM backend answers enrichment calls.
    pub llm: LlmBackendChoice,
    /// Which backend computes embeddings.
    pub embedding: EmbeddingBackendChoice,
}

impl BackendChoice {
    /// Builds the pair from the two CLI selectors.
    pub fn new(llm: LlmBackendChoice, embedding: EmbeddingBackendChoice) -> Self {
        Self { llm, embedding }
    }
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
