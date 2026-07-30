//! Entity and URL extraction pipeline (v1.0.76).
//!
//! v1.0.76: the default build is **LLM-only**. v1.0.79: the legacy GLiNER
//! NER pipeline (`extraction_gliner.rs`, `ner-legacy` feature) was REMOVED
//! entirely. The build extracts:
//!
//! - **URLs** via regex (always available, no model needed).
//! - **Entities** via the `ExtractionBackend` trait (LLM headless).
//!   The default backend is `LlmBackend` (claude / codex), which produces
//!   structured entities and relationships via tool-use JSON.
//!
//! The `extract_graph_auto` function below is the entry point used by
//! `remember`, `ingest`, and `enrich`. With the default feature set, it
//! runs the regex URL pass; entity extraction happens through the LLM
//! extraction backend selected at the command layer.

use serde::{Deserialize, Serialize};

/// One URL extracted from a body. Always produced by the regex path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedUrl {
    /// URL value.
    pub url: String,
    /// Start offset (inclusive).
    pub start: usize,
    /// End offset (exclusive).
    pub end: usize,
}

/// One named-entity mention. Produced via the LLM extraction backend
/// (the legacy GLiNER NER build was removed in v1.0.79).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedEntity {
    /// Name of this item.
    pub name: String,
    /// Entity type label.
    pub entity_type: String,
    /// Start offset (inclusive).
    pub start: usize,
    /// End offset (exclusive).
    pub end: usize,
}

/// Full extraction result: URLs (regex), entities (LLM), and the
/// relationships between them. The LLM backend also returns typed
/// relationships directly in `ExtractionOutput`; this struct is the
/// regex-only baseline that `remember` and `ingest` consume.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExtractionResult {
    /// Extracted entities.
    pub entities: Vec<ExtractedEntity>,
    /// Extracted URLs.
    pub urls: Vec<ExtractedUrl>,
    /// Wall-clock latency in milliseconds.
    pub elapsed_ms: u64,
}

/// Trait abstraction for any extractor. Implemented by the LLM backend
/// (the legacy GLiNER backend was removed in v1.0.79).
pub trait Extractor: Send + Sync {
    /// Return the name of this item.
    fn name(&self) -> &'static str;
    /// Extract structured data from the given body text.
    fn extract(&self, body: &str) -> Result<ExtractionResult, crate::errors::AppError>;
}

/// Regex-only extractor: URLs and nothing else. Used as a fast
/// pre-pass before the (slower) LLM extractor in `extract_graph_auto`.
pub struct RegexExtractor;

impl Extractor for RegexExtractor {
    fn name(&self) -> &'static str {
        "regex"
    }
    fn extract(&self, body: &str) -> Result<ExtractionResult, crate::errors::AppError> {
        Ok(ExtractionResult {
            entities: Vec::new(),
            urls: extract_urls(body),
            elapsed_ms: 0,
        })
    }
}

/// Extracts URLs from `body` using a substring scan. UTF-8 safe; offsets
/// are byte indices into the input.
pub fn extract_urls(body: &str) -> Vec<ExtractedUrl> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor < body.len() {
        let hay = &body[cursor..];
        // Find the next URL boundary, considering both schemes.
        let http_at = hay.find("http://");
        let https_at = hay.find("https://");
        let (rel_start, scheme_len) = match (http_at, https_at) {
            (Some(a), Some(b)) => {
                if a <= b {
                    (a, 7)
                } else {
                    (b, 8)
                }
            }
            (Some(a), None) => (a, 7),
            (None, Some(b)) => (b, 8),
            (None, None) => break,
        };
        let abs_start = cursor + rel_start;
        let after_scheme = abs_start + scheme_len;
        let mut end = after_scheme;
        for (i, c) in body[after_scheme..].char_indices() {
            if c.is_whitespace() || matches!(c, ')' | ']' | '}' | '"' | '\'' | '<') {
                end = after_scheme + i;
                break;
            }
            end = after_scheme + i + c.len_utf8();
        }
        out.push(ExtractedUrl {
            url: body[abs_start..end].to_string(),
            start: abs_start,
            end,
        });
        cursor = end;
    }
    out
}

/// Top-level extraction entry point used by `remember`, `ingest`, and
/// `enrich`. Runs the regex URL pass (always available); the legacy GLiNER
/// delegation was removed in v1.0.79 together with the `ner-legacy` feature.
pub fn extract_graph_auto(
    body: &str,
    _paths: &crate::paths::AppPaths,
) -> Result<ExtractionResult, crate::errors::AppError> {
    let start = std::time::Instant::now();
    let urls = extract_urls(body);
    Ok(ExtractionResult {
        entities: Vec::new(),
        urls,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_urls_finds_http_and_https() {
        let body = "see https://example.com/foo and http://bar.baz/qux end";
        let urls = extract_urls(body);
        assert_eq!(urls.len(), 2, "got {urls:?} for body {body:?}");
        assert_eq!(urls[0].url, "https://example.com/foo");
        assert_eq!(urls[1].url, "http://bar.baz/qux");
    }

    #[test]
    fn extract_urls_handles_trailing_punctuation() {
        let body = "see https://example.com/foo).";
        let urls = extract_urls(body);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com/foo");
    }

    #[test]
    fn extract_urls_empty_body() {
        assert!(extract_urls("").is_empty());
    }

    #[test]
    fn regex_extractor_returns_only_urls() {
        let result = RegexExtractor.extract("see https://example.com").unwrap();
        assert_eq!(result.entities.len(), 0);
        assert_eq!(result.urls.len(), 1);
    }
}
