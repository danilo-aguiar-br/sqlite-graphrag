//! Builder for [`super::LlmEmbedding`] (ADR-0042 / GAP-002).

use super::binary::resolve_real_binary;
use super::models::{claude_embed_model, codex_embed_model, opencode_embed_model};
use super::types::EmbeddingFlavour;
use super::LlmEmbedding;
use crate::errors::AppError;

/// ADR-0042 / GAP-002: builder for [`LlmEmbedding`] that lets callers
/// override the binary path and model without having to remember the
/// env-var names per flavour. Replaces the duplicated `with_codex` /
/// `with_claude` bodies that diverged in v1.0.82 (GAP-002: the Claude
/// arm of `embed_via_backend` re-did the PATH probe via
/// `LlmEmbedding::detect_available` and could silently pick `codex`).
#[derive(Clone, Debug)]
pub struct LlmEmbeddingBuilder {
    /// Selected backend flavour.
    pub(crate) flavour: EmbeddingFlavour,
    /// Optional binary path override.
    pub(crate) binary_override: Option<std::path::PathBuf>,
    /// Optional model name override.
    pub(crate) model_override: Option<String>,
    /// Optional per-call timeout override.
    pub(crate) timeout_override: Option<std::time::Duration>,
}

impl LlmEmbeddingBuilder {
    /// Convenience: produce a Claude-backed builder pre-configured with
    /// the canonical default binary + model.
    /// Convenience: produce a Claude-backed builder pre-configured with
    /// the canonical default binary + model.
    pub fn claude_default() -> Self {
        Self {
            flavour: EmbeddingFlavour::Claude,
            binary_override: None,
            model_override: None,
            timeout_override: None,
        }
    }

    /// Convenience: produce a Codex-backed builder pre-configured with
    /// the canonical default binary + model.
    pub fn codex_default() -> Self {
        Self {
            flavour: EmbeddingFlavour::Codex,
            binary_override: None,
            model_override: None,
            timeout_override: None,
        }
    }

    /// Convenience: produce an OpenCode-backed builder pre-configured with
    /// the canonical default binary + model.
    pub fn opencode_default() -> Self {
        Self {
            flavour: EmbeddingFlavour::Opencode,
            binary_override: None,
            model_override: None,
            timeout_override: None,
        }
    }
    /// Override the binary path (skips the `which::which` PATH probe).
    pub fn override_binary(mut self, binary: std::path::PathBuf) -> Self {
        self.binary_override = Some(binary);
        self
    }

    /// Override the model name (skips the env-var lookup).
    pub fn override_model(mut self, model: String) -> Self {
        self.model_override = Some(model);
        self
    }

    /// Override the per-call embedding timeout (skips env-var lookup).
    /// Minimum 1s enables fail-fast Auto chain probes (GAP-E2E-06).
    pub fn override_timeout(mut self, secs: u64) -> Self {
        let clamped = secs.clamp(1, 3_600);
        self.timeout_override = Some(std::time::Duration::from_secs(clamped));
        self
    }

    /// Build the [`LlmEmbedding`]. Enforces OAuth-only and resolves the
    /// binary/model via the override or the env-var defaults.
    pub fn build(self) -> Result<LlmEmbedding, AppError> {
        LlmEmbedding::oauth_only_enforce()?;
        let binary = match self.binary_override {
            Some(path) => resolve_real_binary(&path),
            None => {
                // Wave 4: flag/XDG via runtime_config — no product env reads.
                let (xdg_bin, which_name) = match self.flavour {
                    EmbeddingFlavour::Codex => (
                        crate::runtime_config::codex_binary(),
                        "codex",
                    ),
                    EmbeddingFlavour::Claude => (
                        crate::runtime_config::claude_binary(),
                        "claude",
                    ),
                    EmbeddingFlavour::Opencode => (
                        crate::runtime_config::opencode_binary(),
                        "opencode",
                    ),
                };
                let path = xdg_bin
                    .map(std::path::PathBuf::from)
                    .or_else(|| which::which(which_name).ok())
                    .ok_or_else(|| {
                        AppError::Embedding(
                            crate::i18n::validation::embedding_binary_not_found_on_path(which_name),
                        )
                    })?;
                resolve_real_binary(&path)
            }
        };
        let model = match self.model_override {
            Some(m) => m,
            None => match self.flavour {
                EmbeddingFlavour::Codex => codex_embed_model(),
                EmbeddingFlavour::Claude => claude_embed_model(),
                EmbeddingFlavour::Opencode => opencode_embed_model(),
            },
        };
        Ok(LlmEmbedding::from_parts(
            self.flavour,
            binary,
            model,
            self.timeout_override,
        ))
    }
}
