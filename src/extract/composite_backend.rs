//! Composite extraction backend (v1.0.75 — G21 orchestration)
//!
//! Runs multiple backends in parallel and merges their outputs.
//! Was selected by an extraction-backend switch that no longer exists: the flag
//! was removed in v1.0.79 together with the fastembed pipeline.

use super::{
    BackendHealth, BackendKind, ExtractionBackend, ExtractionHints, ExtractionOutput, SharedBackend,
};
use crate::errors::AppError;
use async_trait::async_trait;
use std::time::Instant;

/// Composite backend.
pub struct CompositeBackend {
    backends: Vec<SharedBackend>,
}

impl CompositeBackend {
    /// Create a new instance.
    pub fn new(backends: Vec<SharedBackend>) -> Self {
        Self { backends }
    }
}

#[async_trait]
impl ExtractionBackend for CompositeBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Composite
    }

    fn model_name(&self) -> String {
        self.backends
            .iter()
            .map(|b| b.model_name())
            .collect::<Vec<_>>()
            .join("+")
    }

    async fn extract(
        &self,
        content: &str,
        hints: &ExtractionHints,
    ) -> Result<ExtractionOutput, AppError> {
        let start = Instant::now();
        let mut merged = ExtractionOutput {
            backend: self.kind().as_str().to_string(),
            ..Default::default()
        };
        let mut first_embedding: Option<Vec<f32>> = None;
        let mut any_error: Option<AppError> = None;

        for backend in &self.backends {
            match backend.extract(content, hints).await {
                Ok(out) => {
                    for entity in out.entities {
                        if !merged.entities.iter().any(|e| e.name == entity.name) {
                            merged.entities.push(entity);
                        }
                    }
                    for rel in out.relationships {
                        let exists = merged.relationships.iter().any(|r| {
                            r.source == rel.source
                                && r.target == rel.target
                                && r.relation == rel.relation
                        });
                        if !exists {
                            merged.relationships.push(rel);
                        }
                    }
                    if first_embedding.is_none() && out.embedding.is_some() {
                        first_embedding = out.embedding;
                    }
                }
                Err(err) => {
                    if any_error.is_none() {
                        any_error = Some(err);
                    }
                }
            }
        }

        merged.embedding = first_embedding;
        merged.elapsed_ms = start.elapsed().as_millis() as u64;

        if merged.entities.is_empty() && merged.relationships.is_empty() {
            if let Some(err) = any_error {
                return Err(err);
            }
        }
        Ok(merged)
    }

    async fn health(&self) -> Result<BackendHealth, AppError> {
        let mut healthy = true;
        let mut messages = Vec::new();
        for backend in &self.backends {
            match backend.health().await {
                Ok(h) => {
                    if !h.healthy {
                        healthy = false;
                    }
                    messages.push(format!(
                        "{}:{}",
                        h.kind.as_str(),
                        if h.healthy { "ok" } else { "degraded" }
                    ));
                }
                Err(err) => {
                    healthy = false;
                    messages.push(format!("err:{err}"));
                }
            }
        }
        Ok(BackendHealth {
            kind: self.kind(),
            healthy,
            model_name: self.model_name(),
            message: messages.join(" "),
        })
    }
}

/// Factory that builds the default backend for the current build configuration.
pub fn default_backend() -> SharedBackend {
    use std::sync::Arc;
    Arc::new(super::llm_backend::LlmBackend::new(
        super::llm_backend::LlmExtractorConfig::default(),
    ))
}

/// Factory that builds a backend from a CLI flag.
pub fn backend_from_kind(kind: BackendKind) -> SharedBackend {
    use std::sync::Arc;
    match kind {
        BackendKind::Llm => default_backend(),
        BackendKind::Embedding => Arc::new(super::embedding_backend::EmbeddingBackend::new()),
        BackendKind::None => Arc::new(super::none_backend::NoneBackend::new()),
        BackendKind::Composite => {
            let llm: SharedBackend = default_backend();
            let embedding: SharedBackend =
                Arc::new(super::embedding_backend::EmbeddingBackend::new());
            Arc::new(CompositeBackend::new(vec![llm, embedding]))
        }
    }
}
