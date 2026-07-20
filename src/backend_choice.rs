//! LLM / embedding backend CLI choices (Wave C1).


/// v1.0.82 (GAP-003): LLM backend for embedding. Accepts `auto` (default —
/// detects `codex` or `claude` on the PATH), `codex` (forces codex exec), `claude`
/// (forces claude -p), or `none` (skips embedding; useful for tests).
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum LlmBackendChoice {
    Auto,
    Claude,
    Codex,
    Opencode,
    OpenRouter,
    None,
}

/// v1.0.93: embedding backend selector. Separate from `--llm-backend` which
/// controls enrichment (entity extraction, body enrichment) via subprocess.
/// `auto` tries OpenRouter if API key is available, falls back to LLM subprocess.
/// `openrouter` requires API key (exit 78 if absent).
/// `llm` forces subprocess (codex/claude/opencode) — legacy behaviour.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum EmbeddingBackendChoice {
    Auto,
    Openrouter,
    Llm,
}

impl EmbeddingBackendChoice {
    /// v1.0.93: produces a fallback chain that prepends OpenRouter when
    /// the client is initialised. Falls back to the LLM subprocess chain.
    pub fn to_chain(self, llm_choice: LlmBackendChoice) -> Vec<crate::embedder::LlmBackendKind> {
        use crate::embedder::LlmBackendKind;
        match self {
            EmbeddingBackendChoice::Openrouter => vec![LlmBackendKind::OpenRouter],
            EmbeddingBackendChoice::Llm => llm_choice.to_chain(),
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
    /// v1.0.82 (GAP-003): converts the CLI choice into an ordered chain
    /// of backends that `embedder::embed_with_fallback` iterates. The first
    /// element of the chain is the preferred backend; subsequent elements
    /// are fallbacks used when the preferred one fails with `LlmBackendError`.
    ///
    /// `Auto` produces `[Codex, Claude, None]` — codex is the default since v1.0.76+,
    /// claude is the fallback if codex fails (OAuth contention, quota), and
    /// `None` lets `embed_with_fallback` return an empty vector when
    /// `skip_on_failure` is active.
    pub fn to_chain(self) -> Vec<crate::embedder::LlmBackendKind> {
        use crate::embedder::LlmBackendKind;
        match self {
            LlmBackendChoice::Codex => vec![LlmBackendKind::Codex, LlmBackendKind::None],
            LlmBackendChoice::Claude => vec![LlmBackendKind::Claude, LlmBackendKind::None],
            LlmBackendChoice::Opencode => vec![
                LlmBackendKind::Opencode,
                LlmBackendKind::Codex,
                LlmBackendKind::Claude,
                LlmBackendKind::None,
            ],
            LlmBackendChoice::OpenRouter => vec![
                LlmBackendKind::OpenRouter,
                LlmBackendKind::Codex,
                LlmBackendKind::None,
            ],
            LlmBackendChoice::None => vec![LlmBackendKind::None],
            LlmBackendChoice::Auto => parse_fallback_chain(
                &crate::runtime_config::llm_fallback("codex,claude,none"),
            ),
        }
    }
}

fn parse_fallback_chain(s: &str) -> Vec<crate::embedder::LlmBackendKind> {
    use crate::embedder::LlmBackendKind;
    let mut chain: Vec<LlmBackendKind> = s
        .split(',')
        .filter_map(|tok| match tok.trim().to_ascii_lowercase().as_str() {
            "codex" => Some(LlmBackendKind::Codex),
            "claude" | "claude-code" => Some(LlmBackendKind::Claude),
            "opencode" => Some(LlmBackendKind::Opencode),
            "openrouter" => Some(LlmBackendKind::OpenRouter),
            "none" => Some(LlmBackendKind::None),
            _ => {
                tracing::warn!(
                    token = tok.trim(),
                    "unknown backend in --llm-fallback, skipping"
                );
                Option::None
            }
        })
        .collect();
    if chain.is_empty() {
        chain = vec![
            LlmBackendKind::Codex,
            LlmBackendKind::Claude,
            LlmBackendKind::None,
        ];
    }
    chain
}

