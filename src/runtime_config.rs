//! Runtime configuration resolved without product environment variables.
//!
//! Precedence (G-T-XDG-04 / plan v4): **CLI flag > XDG `config set` > named default**.
//! Product `SQLITE_GRAPHRAG_*` / `OPENROUTER_*` env vars are **not** read for config.
//! OS env allowed only for process identity: `HOME`, `PATH`, `XDG_*`, locale, `NO_COLOR`.

use crate::config;
use std::sync::OnceLock;

/// Process-wide overrides captured once from CLI flags at bootstrap.
#[derive(Debug, Clone, Default)]
pub struct RuntimeOverrides {
    pub embedding_dim: Option<u32>,
    pub claude_binary: Option<String>,
    pub codex_binary: Option<String>,
    pub opencode_binary: Option<String>, // path as string
    pub llm_model: Option<String>,
    pub llm_fallback: Option<String>,
    pub skip_embedding_on_failure: bool,
    pub llm_max_host_concurrency: Option<usize>,
    pub llm_slot_wait_secs: Option<u64>,
    pub llm_slot_no_wait: bool,
    pub strict_env_clear: bool,
    pub log_level: Option<String>,
    pub log_format: Option<String>,
    pub lang: Option<String>,
    pub display_tz: Option<String>,
    pub db_path: Option<String>,
}

static RUNTIME: OnceLock<RuntimeOverrides> = OnceLock::new();

/// Install CLI-captured overrides. Idempotent first-wins (main bootstrap).
pub fn init(overrides: RuntimeOverrides) {
    let _ = RUNTIME.set(overrides);
}

/// Borrow installed overrides (empty defaults if init was skipped — tests).
pub fn get() -> RuntimeOverrides {
    RUNTIME.get().cloned().unwrap_or_default()
}

/// flag_opt > XDG setting > default.
pub fn resolve_string(flag: Option<&str>, xdg_key: &str, default: &str) -> String {
    if let Some(v) = flag {
        if !v.is_empty() {
            return v.to_string();
        }
    }
    if let Ok(Some(v)) = config::get_setting(xdg_key) {
        if !v.is_empty() {
            return v;
        }
    }
    default.to_string()
}

/// flag_opt > XDG setting > None.
pub fn resolve_optional_string(flag: Option<&str>, xdg_key: &str) -> Option<String> {
    if let Some(v) = flag {
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    config::get_setting(xdg_key).ok().flatten().filter(|s| !s.is_empty())
}

/// Parse usize from flag > XDG > default.
pub fn resolve_usize(flag: Option<usize>, xdg_key: &str, default: usize) -> usize {
    if let Some(v) = flag {
        return v;
    }
    if let Ok(Some(v)) = config::get_setting(xdg_key) {
        if let Ok(n) = v.parse::<usize>() {
            return n;
        }
    }
    default
}

/// Parse u64 from flag > XDG > default.
pub fn resolve_u64(flag: Option<u64>, xdg_key: &str, default: u64) -> u64 {
    if let Some(v) = flag {
        return v;
    }
    if let Ok(Some(v)) = config::get_setting(xdg_key) {
        if let Ok(n) = v.parse::<u64>() {
            return n;
        }
    }
    default
}

/// Parse f64 from flag > XDG > default.
pub fn resolve_f64(flag: Option<f64>, xdg_key: &str, default: f64) -> f64 {
    if let Some(v) = flag {
        return v;
    }
    if let Ok(Some(v)) = config::get_setting(xdg_key) {
        if let Ok(n) = v.parse::<f64>() {
            return n;
        }
    }
    default
}

/// Bool: CLI true wins; else XDG "1"/"true"/"yes"; else default.
pub fn resolve_bool(flag_set: bool, xdg_key: &str, default: bool) -> bool {
    if flag_set {
        return true;
    }
    if let Ok(Some(v)) = config::get_setting(xdg_key) {
        let t = v.trim().to_ascii_lowercase();
        return matches!(t.as_str(), "1" | "true" | "yes" | "on");
    }
    default
}

/// Embedding dim: CLI override > XDG `embedding.dim` > None (caller uses DB/default).
pub fn embedding_dim_override() -> Option<u32> {
    let rt = get();
    if let Some(d) = rt.embedding_dim {
        return Some(d);
    }
    if let Ok(Some(v)) = config::get_setting("embedding.dim") {
        if let Ok(n) = v.parse::<u32>() {
            if (8..=4096).contains(&n) {
                return Some(n);
            }
        }
    }
    None
}

/// Skip embedding on failure: runtime flag or XDG.
pub fn skip_embedding_on_failure() -> bool {
    let rt = get();
    resolve_bool(
        rt.skip_embedding_on_failure,
        "llm.skip_embedding_on_failure",
        false,
    )
}

/// Host concurrency for LLM slots.
pub fn llm_max_host_concurrency(default: usize) -> usize {
    let rt = get();
    resolve_usize(
        rt.llm_max_host_concurrency,
        "llm.max_host_concurrency",
        default,
    )
}

pub fn llm_slot_wait_secs(default: u64) -> u64 {
    let rt = get();
    if rt.llm_slot_no_wait {
        return 0;
    }
    resolve_u64(rt.llm_slot_wait_secs, "llm.slot_wait_secs", default)
}

pub fn llm_slot_no_wait() -> bool {
    let rt = get();
    resolve_bool(rt.llm_slot_no_wait, "llm.slot_no_wait", false)
}

pub fn claude_binary() -> Option<String> {
    let rt = get();
    resolve_optional_string(rt.claude_binary.as_deref(), "llm.claude_binary")
}

pub fn codex_binary() -> Option<String> {
    let rt = get();
    resolve_optional_string(rt.codex_binary.as_deref(), "llm.codex_binary")
}

pub fn opencode_binary() -> Option<String> {
    let rt = get();
    resolve_optional_string(rt.opencode_binary.as_deref(), "llm.opencode_binary")
}

pub fn llm_model() -> Option<String> {
    let rt = get();
    resolve_optional_string(rt.llm_model.as_deref(), "llm.model")
}

pub fn llm_fallback(default: &str) -> String {
    let rt = get();
    resolve_string(rt.llm_fallback.as_deref(), "llm.fallback", default)
}

pub fn log_level(default: &str) -> String {
    let rt = get();
    resolve_string(rt.log_level.as_deref(), "log.level", default)
}

pub fn log_format(default: &str) -> String {
    let rt = get();
    resolve_string(rt.log_format.as_deref(), "log.format", default)
}

pub fn max_entities_per_memory(default: usize) -> usize {
    resolve_usize(None, "limits.max_entities_per_memory", default)
}

pub fn max_relations_per_memory(default: usize) -> usize {
    resolve_usize(None, "limits.max_relations_per_memory", default)
}

/// OpenRouter chat URL: XDG override or compile-time default.
/// Canonical key: `network.openrouter.chat_url`; alias: `network.chat_url`.
pub fn openrouter_chat_url(default: &str) -> String {
    resolve_string_with_aliases(
        None,
        &["network.openrouter.chat_url", "network.chat_url"],
        default,
    )
}

/// OpenRouter embeddings URL: XDG override or compile-time default.
/// Canonical key: `network.openrouter.embeddings_url`; alias: `network.embed_url`.
pub fn openrouter_embeddings_url(default: &str) -> String {
    resolve_string_with_aliases(
        None,
        &[
            "network.openrouter.embeddings_url",
            "network.embed_url",
        ],
        default,
    )
}

/// Probe timeout for fail-fast LLM backend readiness (ms).
pub fn llm_probe_timeout_ms(default: u64) -> u64 {
    resolve_u64(None, "llm.probe_timeout_ms", default)
}

/// flag > first non-empty XDG key in `keys` > default.
fn resolve_string_with_aliases(flag: Option<&str>, keys: &[&str], default: &str) -> String {
    if let Some(v) = flag {
        if !v.is_empty() {
            return v.to_string();
        }
    }
    for key in keys {
        if let Ok(Some(v)) = config::get_setting(key) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    default.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_string_prefers_flag() {
        assert_eq!(
            resolve_string(Some("from-flag"), "nonexistent.key.xyz", "def"),
            "from-flag"
        );
    }

    #[test]
    fn resolve_string_falls_to_default() {
        assert_eq!(
            resolve_string(None, "nonexistent.key.xyz.zzz", "def"),
            "def"
        );
    }
}
