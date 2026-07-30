//! Embedding model name resolution via XDG config (no product env).

/// G42/S5: claude embedding model with XDG override, symmetric to the
/// codex `embedding.codex_model` introduced in v1.0.78.
pub(super) fn claude_embed_model() -> String {
    // Precedence: XDG embedding.claude_model > runtime llm.model > default
    if let Some(m) = crate::config::get_setting("embedding.claude_model")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
    {
        return m;
    }
    if let Some(m) = crate::runtime_config::llm_model() {
        return m;
    }
    tracing::info!(
        target: "llm_embedding",
        "no model specified; defaulting to claude-sonnet-4-6"
    );
    "claude-sonnet-4-6".to_string()
}

pub(super) fn codex_embed_model() -> String {
    // Precedence: XDG embedding.codex_model > runtime llm.model > default
    if let Some(m) = crate::config::get_setting("embedding.codex_model")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
    {
        return m;
    }
    if let Some(m) = crate::runtime_config::llm_model() {
        return m;
    }
    tracing::info!(
        target: "llm_embedding",
        "no model specified; defaulting to gpt-5.5"
    );
    "gpt-5.5".to_string()
}

pub(super) fn opencode_embed_model() -> String {
    // Precedence: XDG embedding.opencode_model > llm.opencode_model > default
    // Does NOT fall back to llm.model (cross-backend contamination).
    if let Some(m) = crate::config::get_setting("embedding.opencode_model")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
    {
        return m;
    }
    crate::runtime_config::resolve_string(None, "llm.opencode_model", "opencode/big-pickle")
}
