//! LLM-based embedding backend (v1.0.76 default; reworked in v1.0.79 G42).
//!
//! `LlmEmbedding` is the production embedding client. It wraps headless
//! invocations of `claude code` or `codex` and returns f32 vectors of the
//! active dimensionality (`crate::constants::embedding_dim()`, default 1024 via DEFAULT_EMBEDDING_DIM).
//!
//! v1.0.79 (G42) changes:
//! - S1: the dimensionality is no longer hardcoded here — the single
//!   source of truth lives in `crate::constants` and the JSON schemas
//!   are generated dynamically.
//! - S2: `embed_batch` embeds N numbered texts per LLM call with the
//!   `{items:[{i,v}]}` schema, collapsing 39 subprocess spawns into 4-5.
//! - S4: the codex `--output-schema` file is a `tempfile::NamedTempFile`
//!   with a randomised name created once per client and shared across
//!   clones via `Arc` — no per-call write+delete, no PID-path races.
//! - S5: the claude model honours XDG `embedding.claude_model`
//!   (symmetric to `embedding.codex_model`). ZERO hardcoded models without
//!   a flag/XDG override.
//! - S6: `CLAUDE_CONFIG_DIR` points at an empty managed directory BY
//!   DEFAULT, because `--strict-mcp-config`/`--mcp-config '{}'` are
//!   silently ignored upstream (anthropics/claude-code#10787) and a
//!   full `~/.claude` costs ~223k cache-creation tokens per call.
//! - S7: the codex `request_user_input` failure mode maps to an
//!   actionable error instead of an opaque exit 11.
//! - BLOCO 4: every subprocess uses `kill_on_drop(true)` plus an
//!   explicit `tokio::time::timeout`, so cancellation never leaks a
//!   child and a hung LLM cannot stall the pipeline forever.
//!
//! OAuth is the only supported credential path. The constructor rejects
//! `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` in the environment — see
//! `v1.0.69 (G31) OAuth-Only Enforcement`.

mod binary;
mod builder;
mod models;
mod ops;
mod timeout;
mod types;
mod wire;

pub use binary::resolve_real_binary;
pub use builder::LlmEmbeddingBuilder;
pub use types::EmbeddingFlavour;

use crate::errors::AppError;
use models::{claude_embed_model, codex_embed_model, opencode_embed_model};
use std::sync::Arc;
use timeout::embed_timeout;
use types::CodexSchemaFiles;

/// LLM embedding client (headless claude / codex / opencode).
#[derive(Clone, Debug)]
pub struct LlmEmbedding {
    /// Which LLM headless binary to spawn.
    pub(crate) flavour: EmbeddingFlavour,
    /// Cached path to the binary to avoid PATH lookups on every call.
    pub(crate) binary: std::path::PathBuf,
    /// Model name resolved at construction time.
    pub(crate) model: String,
    /// G42/S4: lazily-created codex `--output-schema` tempfiles, shared
    /// across clones. Keyed by dim so a dim change cannot serve a stale schema.
    pub(crate) codex_schemas: Arc<parking_lot::Mutex<CodexSchemaFiles>>,
    /// Instance-scoped timeout override.
    /// Precedence: this field > XDG `embedding.timeout_secs` > default 300s.
    pub(crate) timeout_override: Option<std::time::Duration>,
}

impl LlmEmbedding {
    /// Apply a per-call timeout override (e.g. short budget for query Auto).
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        let clamped = secs.clamp(1, 3_600);
        self.timeout_override = Some(std::time::Duration::from_secs(clamped));
        self
    }

    /// Detects which LLM CLI is available on PATH and returns the
    /// matching embedding client.
    ///
    /// Prefers `codex`, then `claude`, then `opencode` (lighter context first).
    pub fn detect_available() -> Result<Self, AppError> {
        Self::oauth_only_enforce()?;

        if let Some(path) = crate::runtime_config::codex_binary()
            .map(std::path::PathBuf::from)
            .or_else(|| which::which("codex").ok())
        {
            return Ok(Self::from_parts(
                EmbeddingFlavour::Codex,
                resolve_real_binary(&path),
                codex_embed_model(),
                None,
            ));
        }
        if let Some(path) = crate::runtime_config::claude_binary()
            .map(std::path::PathBuf::from)
            .or_else(|| which::which("claude").ok())
        {
            return Ok(Self::from_parts(
                EmbeddingFlavour::Claude,
                resolve_real_binary(&path),
                claude_embed_model(),
                None,
            ));
        }
        if let Some(path) = crate::runtime_config::opencode_binary()
            .map(std::path::PathBuf::from)
            .or_else(|| which::which("opencode").ok())
        {
            return Ok(Self::from_parts(
                EmbeddingFlavour::Opencode,
                resolve_real_binary(&path),
                opencode_embed_model(),
                None,
            ));
        }
        Err(AppError::Embedding(
            crate::i18n::validation::embedding_no_llm_cli_on_path(),
        ))
    }

    /// Build from resolved parts (used by builder and detect).
    pub(crate) fn from_parts(
        flavour: EmbeddingFlavour,
        binary: std::path::PathBuf,
        model: String,
        timeout_override: Option<std::time::Duration>,
    ) -> Self {
        Self {
            flavour,
            binary,
            model,
            codex_schemas: Arc::new(parking_lot::Mutex::new(CodexSchemaFiles::default())),
            timeout_override,
        }
    }

    /// Instance-scoped timeout. Precedence:
    /// `timeout_override` field > XDG > default.
    pub(crate) fn instance_embed_timeout(&self) -> std::time::Duration {
        if let Some(d) = self.timeout_override {
            return d;
        }
        embed_timeout()
    }

    /// Instance-scoped batch timeout: base + 15s per extra item.
    pub(crate) fn instance_embed_timeout_for_batch(
        &self,
        batch_size: usize,
    ) -> std::time::Duration {
        let base = self.instance_embed_timeout();
        let extra = std::time::Duration::from_secs(15) * batch_size.saturating_sub(1) as u32;
        base + extra
    }

    /// With codex.
    pub fn with_codex() -> Result<Self, AppError> {
        Self::with_codex_builder().build()
    }

    /// With claude.
    pub fn with_claude() -> Result<Self, AppError> {
        Self::with_claude_builder().build()
    }

    /// Builder entry point for a codex-backed embedder.
    pub fn with_codex_builder() -> LlmEmbeddingBuilder {
        LlmEmbeddingBuilder::codex_default()
    }

    /// Builder entry point for a claude-backed embedder.
    pub fn with_claude_builder() -> LlmEmbeddingBuilder {
        LlmEmbeddingBuilder::claude_default()
    }

    /// With opencode.
    pub fn with_opencode() -> Result<Self, AppError> {
        Self::with_opencode_builder().build()
    }

    /// Builder entry point for an opencode-backed embedder.
    pub fn with_opencode_builder() -> LlmEmbeddingBuilder {
        LlmEmbeddingBuilder::opencode_default()
    }

    /// v1.0.69 (G31): refuse to spawn if an API key is set. The CLI must use OAuth.
    pub(crate) fn oauth_only_enforce() -> Result<(), AppError> {
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            return Err(AppError::Validation(
                crate::i18n::validation::anthropic_api_key_oauth_required(),
            ));
        }
        if std::env::var("OPENAI_API_KEY").is_ok() {
            return Err(AppError::Validation(
                crate::i18n::validation::openai_api_key_oauth_required(),
            ));
        }
        Ok(())
    }

    /// Embeds a single passage (chunk of a memory body).
    pub fn embed_passage(&self, text: &str) -> Result<Vec<f32>, AppError> {
        self.invoke_with_prefix(crate::constants::PASSAGE_PREFIX, text)
    }

    /// Embeds a single query with the query prompt prefix.
    pub fn embed_query(&self, text: &str) -> Result<Vec<f32>, AppError> {
        self.invoke_with_prefix(crate::constants::QUERY_PREFIX, text)
    }

    /// Stable label for the active embedding model (`flavour:model`).
    pub fn model_label(&self) -> String {
        format!("{}:{}", self.flavour.as_str(), self.model)
    }

    /// Returns the resolved embedding flavour of this client.
    pub fn flavour(&self) -> EmbeddingFlavour {
        self.flavour
    }
}

#[cfg(test)]
mod tests;
