//! Embedding client types shared across the LLM embedding backend.

use serde::Deserialize;
use std::sync::Arc;

/// Lazily-created codex `--output-schema` tempfiles, shared across clones.
#[derive(Debug, Default)]
pub(crate) struct CodexSchemaFiles {
    pub(super) single: Option<(usize, Arc<tempfile::NamedTempFile>)>,
    pub(super) batch: Option<(usize, Arc<tempfile::NamedTempFile>)>,
}

/// Embedding flavour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum EmbeddingFlavour {
    /// Claude variant.
    Claude,
    /// Codex variant.
    Codex,
    /// Opencode variant.
    Opencode,
}

impl EmbeddingFlavour {
    /// Return the canonical string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Opencode => "opencode",
        }
    }
}
