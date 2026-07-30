//! Bilingual human-readable message layer.
//!
//! The CLI chooses language for stderr progress messages via:
//! 1. Explicit `--lang en|pt` flag
//! 2. XDG setting `i18n.lang` (`config set i18n.lang pt`)
//! 3. OS locale (`LC_ALL` / `LC_MESSAGES` / `LANG` — system env, not product)
//! 4. Fallback `English`
//!
//! JSON stdout is deterministic and identical across languages.

use std::sync::OnceLock;

/// Language.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Language {
    /// English variant.
    #[value(name = "en", aliases = ["english", "EN"])]
    English,
    /// Portuguese variant.
    #[value(name = "pt", aliases = ["portugues", "portuguese", "pt-BR", "pt-br", "PT"])]
    Portuguese,
}

impl Language {
    /// Parses a command-line string into a `Language` without relying on clap.
    /// Accepts the same aliases defined in `#[value(...)]`: "en", "pt", etc.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "en" | "english" => Some(Language::English),
            "pt" | "pt-br" | "portugues" | "portuguese" => Some(Language::Portuguese),
            _ => None,
        }
    }

    /// Detect language from environment variables or system locale.
    pub fn from_env_or_locale() -> Self {
        // Priority 1: XDG setting `i18n.lang` (no product env — G-T-XDG-04).
        if let Ok(Some(v)) = crate::config::get_setting("i18n.lang") {
            if !v.is_empty() {
                let lower = v.to_lowercase();
                if lower.starts_with("pt") {
                    return Language::Portuguese;
                }
                if lower.starts_with("en") {
                    return Language::English;
                }
                tracing::warn!(target: "i18n",
                    value = %v,
                    "i18n.lang setting not recognized, falling back to OS locale"
                );
            }
        }
        // Priority 2: POSIX OS locale LC_ALL > LC_MESSAGES > LANG (allowed system env).
        // We read these via std::env (not via sys_locale) because:
        // (a) `sys_locale::get_locale()` calls into native OS APIs (CFLocaleCopyCurrent
        //     on macOS, GetUserDefaultLocaleName on Windows) which cache the
        //     system locale and IGNORE env vars set at runtime by tests;
        // (b) POSIX specifies LC_ALL > LC_MESSAGES > LANG ordering and an
        //     unrecognised LC_ALL value must stop iteration (fall back to
        //     English default).
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(v) = std::env::var(var) {
                if v.is_empty() {
                    continue;
                }
                let lower = v.to_lowercase();
                if lower.starts_with("pt") {
                    return Language::Portuguese;
                }
                if lower.starts_with("en") {
                    return Language::English;
                }
                // Unrecognised value in a higher-precedence variable stops
                // iteration per POSIX.1-2017 §8.2.
                if var == "LC_ALL" {
                    return Language::English;
                }
            }
        }
        // Priority 3: cross-platform locale detection via native OS APIs.
        // Only reached when no POSIX env var is set.
        if let Some(locale) = sys_locale::get_locale() {
            let lower = locale.to_lowercase();
            if lower.starts_with("pt") {
                return Language::Portuguese;
            }
            if lower.starts_with("en") {
                return Language::English;
            }
        }
        Language::English
    }
}

static GLOBAL_LANGUAGE: OnceLock<Language> = OnceLock::new();

/// Initializes the global language. Subsequent calls are silently ignored
/// (OnceLock semantics) — guaranteeing thread-safety and determinism.
///
/// v1.0.36 (L6): early-return when already initialized so the env-fallback
/// resolver (`from_env_or_locale`) does not run a second time. Without this
/// guard, calling `init(None)` after `current()` already populated the
/// OnceLock causes `from_env_or_locale` to fire its `tracing::warn!` twice
/// for unrecognized `i18n.lang` XDG values.
pub fn init(explicit: Option<Language>) {
    if GLOBAL_LANGUAGE.get().is_some() {
        return;
    }
    let resolved = explicit.unwrap_or_else(Language::from_env_or_locale);
    let _ = GLOBAL_LANGUAGE.set(resolved);
}

/// Returns the active language, or fallback English if `init` was never called.
pub fn current() -> Language {
    *GLOBAL_LANGUAGE.get_or_init(Language::from_env_or_locale)
}

/// Translates a bilingual message by selecting the active variant.
///
/// v1.0.36 (M4): inputs are constrained to `&'static str` so the function
/// can return one of them directly without `Box::leak`. The previous
/// implementation leaked one allocation per call which accumulated in
/// long-running pipelines; this version is allocation-free. All in-tree
/// callers already pass string literals, which are `&'static str`.
pub fn tr(en: &'static str, pt: &'static str) -> &'static str {
    match current() {
        Language::English => en,
        Language::Portuguese => pt,
    }
}

/// Progress message emitted after pruning relationships.
///
/// English-only: this string is emitted to stderr as a progress notice and
/// does not vary by language because the prune-relations command targets
/// agent-first pipelines where deterministic output matters.
pub fn relations_pruned(count: usize, relation: &str, namespace: &str) -> String {
    format!("pruned {count} '{relation}' relationships in namespace '{namespace}'")
}

/// Progress message for dry-run preview of prune-relations.
///
/// English-only: emitted to stderr as a progress notice.
pub fn prune_dry_run(count: usize, relation: &str) -> String {
    format!("dry run: {count} '{relation}' relationships would be removed")
}

/// Warning message when --yes is not passed for destructive prune-relations.
///
/// English-only: emitted to stderr as a progress notice.
pub fn prune_requires_yes() -> String {
    "destructive operation requires --yes flag; use --dry-run to preview".to_string()
}

/// Localized prefix for error messages displayed to the end user.
pub fn error_prefix() -> &'static str {
    match current() {
        Language::English => "Error",
        Language::Portuguese => "Erro",
    }
}

/// Error messages for `AppError` variants — always English.
///
/// These strings end up inside `AppError` inner fields and may appear in
/// deterministic JSON stdout (e.g. ingest NDJSON). Portuguese translations
/// for stderr live in `pub mod app_error_pt` and are applied by
/// `localized_message_for(Language::Portuguese)`.
pub mod errors_msg {
    /// Localized message for `memory_not_found`.
    pub fn memory_not_found(nome: &str, namespace: &str) -> String {
        format!("memory '{nome}' not found in namespace '{namespace}'")
    }

    /// Localized message for `memory_or_entity_not_found`.
    pub fn memory_or_entity_not_found(name: &str, namespace: &str) -> String {
        format!("memory or entity '{name}' not found in namespace '{namespace}'")
    }

    /// Localized message for `database_not_found`.
    pub fn database_not_found(path: &str) -> String {
        format!("database not found at {path}. Run 'sqlite-graphrag init' first.")
    }

    /// Localized message for `entity_not_found`.
    pub fn entity_not_found(nome: &str, namespace: &str) -> String {
        format!("entity \"{nome}\" does not exist in namespace \"{namespace}\"")
    }

    /// Localized message for `relationship_not_found`.
    pub fn relationship_not_found(de: &str, rel: &str, para: &str, namespace: &str) -> String {
        format!(
            "relationship \"{de}\" --[{rel}]--> \"{para}\" does not exist in namespace \"{namespace}\""
        )
    }

    /// Localized message for `duplicate_memory`.
    pub fn duplicate_memory(nome: &str, namespace: &str) -> String {
        format!(
            "memory '{nome}' already exists in namespace '{namespace}'. Use --force-merge to update."
        )
    }

    /// Localized message for `duplicate_memory_soft_deleted`.
    pub fn duplicate_memory_soft_deleted(name: &str, namespace: &str) -> String {
        format!(
            "memory '{name}' exists but is soft-deleted in namespace '{namespace}'; \
             use --force-merge to restore and update, or `restore` to revive it"
        )
    }

    /// Localized message for `optimistic_lock_conflict`.
    pub fn optimistic_lock_conflict(expected: i64, current_ts: i64) -> String {
        format!(
            "optimistic lock conflict: expected updated_at={expected}, but current is {current_ts}"
        )
    }

    /// Localized message for `version_not_found`.
    pub fn version_not_found(versao: i64, nome: &str) -> String {
        format!("version {versao} not found for memory '{nome}'")
    }

    /// Localized message for `no_recall_results`.
    pub fn no_recall_results(max_distance: f32, query: &str, namespace: &str) -> String {
        format!(
            "no results within --max-distance {max_distance} for query '{query}' in namespace '{namespace}'"
        )
    }

    /// Localized message for `soft_deleted_memory_not_found`.
    pub fn soft_deleted_memory_not_found(nome: &str, namespace: &str) -> String {
        format!("soft-deleted memory '{nome}' not found in namespace '{namespace}'")
    }

    /// Localized message for `concurrent_process_conflict`.
    pub fn concurrent_process_conflict() -> String {
        "optimistic lock conflict: memory was modified by another process".to_string()
    }

    /// Localized message for `entity_limit_exceeded`.
    pub fn entity_limit_exceeded(max: usize) -> String {
        format!("entities exceed limit of {max}")
    }

    /// Localized message for `relationship_limit_exceeded`.
    pub fn relationship_limit_exceeded(max: usize) -> String {
        format!("relationships exceed limit of {max}")
    }
}

/// Localized validation messages for memory fields.
pub mod validation;

#[cfg(test)]
mod tests;
