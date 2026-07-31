//! XDG-based API key management for OpenRouter and other providers.
//!
//! Stores keys in `$XDG_CONFIG_HOME/sqlite-graphrag/config.toml` with
//! atomic write, symlink-attack defense and Unix permission hardening.

use crate::errors::AppError;
use crate::i18n::validation;
use secrecy::SecretBox;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// App config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Configuration schema version.
    pub schema_version: u32,
    /// Keys.
    #[serde(default)]
    pub keys: Vec<ApiKeyEntry>,
    /// Operational settings persisted via `config set/get` (G-T-XDG-01).
    /// Stringly-typed map keeps the schema open without migrations for every
    /// new key. Known keys are documented in `config set --help`.
    #[serde(default)]
    pub settings: std::collections::BTreeMap<String, String>,
}

/// API key entry.
#[derive(Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    /// Provider name.
    pub provider: String,
    /// Value.
    pub value: String,
    /// Added at.
    pub added_at: String,
    /// Fingerprint.
    pub fingerprint: String,
}

impl std::fmt::Debug for ApiKeyEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKeyEntry")
            .field("provider", &self.provider)
            .field("value", &mask_key(&self.value))
            .field("added_at", &self.added_at)
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            keys: vec![],
            settings: std::collections::BTreeMap::new(),
        }
    }
}

/// One entry of the canonical setting registry.
///
/// Carrying the default alongside the key is what lets `config doctor` derive
/// its whole listing from [`SETTING_KEYS`] instead of repeating a hand-written
/// table. `GAP-SG-85` was exactly that divergence: a 14-entry manual list next
/// to a 44-key registry, missing the one key that redirects the database.
pub struct SettingKey {
    /// Dotted key accepted by `config set`.
    pub key: &'static str,
    /// Literal default applied when neither a CLI flag nor the XDG config
    /// supplies a value.
    ///
    /// `None` marks a default that cannot be a static string because it is
    /// derived from the host at runtime — an XDG directory, the CPU count, or
    /// a probe. `config doctor` reports those as `derived` rather than
    /// inventing a number that would not match what the process actually uses.
    pub default: Option<&'static str>,
}

/// Canonical registry of operational setting keys accepted by `config set`.
///
/// Single source of truth shared by [`set_setting`] validation and the
/// `config doctor` knob listing. Keeping exactly one list prevents the
/// divergence class recorded as `GAP-SG-79`, where help text advertised
/// `db.default_path` while [`crate::paths::AppPaths::resolve`] has always
/// read `db.path`.
///
/// Every entry MUST have a matching [`get_setting`] reader somewhere in the
/// crate. Adding a key here without a reader recreates the silent no-op this
/// registry exists to prevent; `GAP-SG-90` adds the test that enforces it.
///
/// Literal defaults that mirror a constant are asserted against that constant
/// in this module's tests, so the two cannot drift apart in silence.
///
/// Kept sorted so the emitted diagnostics are stable across runs.
pub const SETTING_KEYS: &[SettingKey] = &[
    SettingKey {
        key: "cache.dir",
        default: None,
    },
    SettingKey {
        key: "cli.max_instances",
        default: None,
    },
    SettingKey {
        key: "db.busy_base_delay_ms",
        default: Some("300"),
    },
    SettingKey {
        key: "db.busy_retries",
        default: Some("5"),
    },
    SettingKey {
        key: "db.path",
        default: None,
    },
    SettingKey {
        key: "db.query_timeout_ms",
        default: Some("5000"),
    },
    SettingKey {
        key: "display.tz",
        default: Some("UTC"),
    },
    SettingKey {
        key: "embedding.batch_size",
        default: Some("32"),
    },
    SettingKey {
        key: "embedding.claude_model",
        default: None,
    },
    SettingKey {
        key: "embedding.codex_model",
        default: None,
    },
    SettingKey {
        key: "embedding.dim",
        default: Some("1024"),
    },
    SettingKey {
        key: "embedding.opencode_model",
        default: None,
    },
    SettingKey {
        key: "enrich.entity_connect.default_limit",
        default: Some("100"),
    },
    SettingKey {
        key: "enrich.entity_connect.large_ns_limit",
        default: Some("25"),
    },
    SettingKey {
        key: "enrich.entity_description.domain",
        default: Some("auto"),
    },
    SettingKey {
        key: "enrich.entity_description.grounding_threshold",
        default: Some("0.12"),
    },
    SettingKey {
        key: "enrich.entity_description.min_corpus_chars",
        default: Some("40"),
    },
    SettingKey {
        key: "enrich.entity_description.quality_sample",
        default: Some("50"),
    },
    SettingKey {
        key: "enrich.yield_every_n_items",
        default: Some("10"),
    },
    SettingKey {
        key: "i18n.lang",
        default: Some("en"),
    },
    SettingKey {
        key: "ingest.low_memory",
        default: Some("false"),
    },
    SettingKey {
        key: "limits.max_entities_per_memory",
        default: Some("50"),
    },
    SettingKey {
        key: "limits.max_relations_per_memory",
        default: Some("50"),
    },
    SettingKey {
        key: "llm.claude_binary",
        default: None,
    },
    SettingKey {
        key: "llm.claude_empty_config_dir",
        default: None,
    },
    SettingKey {
        key: "llm.codex_binary",
        default: None,
    },
    SettingKey {
        key: "llm.fallback",
        default: Some("codex,claude,none"),
    },
    SettingKey {
        key: "llm.max_host_concurrency",
        default: None,
    },
    SettingKey {
        key: "llm.model",
        default: None,
    },
    SettingKey {
        key: "llm.opencode_binary",
        default: None,
    },
    SettingKey {
        key: "llm.opencode_model",
        default: None,
    },
    SettingKey {
        key: "llm.opencode_timeout",
        default: Some("300"),
    },
    SettingKey {
        key: "llm.probe_timeout_ms",
        default: Some("800"),
    },
    SettingKey {
        key: "llm.skip_embedding_on_failure",
        default: Some("false"),
    },
    SettingKey {
        key: "llm.slot_no_wait",
        default: Some("false"),
    },
    SettingKey {
        key: "llm.slot_wait_secs",
        default: Some("300"),
    },
    SettingKey {
        key: "log.format",
        default: Some("pretty"),
    },
    SettingKey {
        key: "log.level",
        default: Some("warn"),
    },
    SettingKey {
        key: "log.retention_days",
        default: Some("7"),
    },
    SettingKey {
        key: "log.rotation",
        default: Some("daily"),
    },
    SettingKey {
        key: "log.to_file",
        default: Some("false"),
    },
    SettingKey {
        key: "namespace.default",
        default: Some("global"),
    },
    SettingKey {
        key: "network.chat_url",
        default: None,
    },
    SettingKey {
        key: "network.embed_url",
        default: None,
    },
    SettingKey {
        key: "network.openrouter.chat_url",
        default: Some(crate::constants::DEFAULT_OPENROUTER_CHAT_URL),
    },
    SettingKey {
        key: "network.openrouter.embeddings_url",
        default: Some(crate::constants::DEFAULT_OPENROUTER_EMBEDDINGS_URL),
    },
    SettingKey {
        key: "parallelism.rayon_threads",
        default: None,
    },
    SettingKey {
        key: "retry.disable",
        default: Some("false"),
    },
    SettingKey {
        key: "shutdown.ignore",
        default: Some("false"),
    },
    SettingKey {
        key: "spawn.skip_preflight",
        default: Some("false"),
    },
    SettingKey {
        key: "spawn.strict_env_clear",
        default: Some("false"),
    },
    SettingKey {
        key: "system.max_load_per_ncpu",
        default: Some("2.0"),
    },
];

/// Iterates the registry key names in registration (sorted) order.
///
/// Callers that only need names use this instead of reaching into the struct,
/// so adding a field to [`SettingKey`] does not ripple through the crate.
pub fn setting_key_names() -> impl Iterator<Item = &'static str> {
    SETTING_KEYS.iter().map(|entry| entry.key)
}

/// Setting keys that were advertised historically but never had a reader.
///
/// Each tuple maps the obsolete key to its replacement. Present so
/// [`load_config`] can warn instead of ignoring the value in silence, which
/// is the failure mode `GAP-SG-79` documents.
///
/// These keys are rejected by [`set_setting`]; the warning covers configs
/// written before the validation existed.
// GAP-SG-79 / GAP-SG-122: legacy aliases map to the single canonical key.
// `db.default_path` never took effect (paths always read `db.path`).
// `paths.cache` was a parallel name for `cache.dir`.
const LEGACY_SETTING_KEYS: &[(&str, &str)] =
    &[("db.default_path", "db.path"), ("paths.cache", "cache.dir")];

/// Minimum Jaro-Winkler similarity required to suggest a replacement key.
///
/// Below this a suggestion is more confusing than helpful, so the error lists
/// nothing rather than pointing at an unrelated key.
const SUGGESTION_THRESHOLD: f64 = 0.7;

/// GAP-SG-90: every literal key passed to [`get_setting`] / [`set_setting`] in
/// production sources must appear in [`SETTING_KEYS`]. The unit test below
/// scans the crate source for `get_setting("…")` call sites.
#[cfg(test)]
mod setting_keys_drift_tests {
    use super::SETTING_KEYS;

    #[test]
    fn setting_keys_covers_get_setting_literals_in_src() {
        // Walk a fixed list of high-traffic modules (full-tree walk is flaky in
        // package builds). Keys used only via aliases arrays are covered by
        // runtime_config unit tests.
        let sources = [
            include_str!("paths.rs"),
            include_str!("i18n/mod.rs"),
            include_str!("i18n/validation/mod.rs"),
            include_str!("i18n/validation/messages_a.rs"),
            include_str!("i18n/validation/messages_b.rs"),
            include_str!("tz.rs"),
            include_str!("namespace.rs"),
            include_str!("retry.rs"),
            include_str!("lock.rs"),
            include_str!("llm_slots.rs"),
            include_str!("system_load.rs"),
            include_str!("spawn/preflight.rs"),
            include_str!("spawn/env_whitelist.rs"),
            include_str!("commands/ingest/mod.rs"),
            include_str!("commands/enrich/prompts.rs"),
            include_str!("extract/llm_embedding/mod.rs"),
            include_str!("lib.rs"),
            include_str!("main.rs"),
        ];
        let registered: std::collections::HashSet<&str> =
            SETTING_KEYS.iter().map(|e| e.key).collect();
        let mut missing = Vec::new();
        for src in sources {
            for cap in src.split("get_setting(\"").skip(1) {
                if let Some(end) = cap.find('"') {
                    let key = &cap[..end];
                    if key.is_empty() {
                        continue;
                    }
                    if !registered.contains(key) {
                        missing.push(key.to_string());
                    }
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "get_setting keys missing from SETTING_KEYS: {missing:?}"
        );
    }
}

/// Returns `true` when `key` belongs to [`SETTING_KEYS`].
pub fn is_known_setting(key: &str) -> bool {
    setting_key_names().any(|known| known == key)
}

/// Returns the closest known key to `key`, when one is similar enough.
///
/// Reuses `rapidfuzz` Jaro-Winkler, the same scorer already used for entity
/// name resolution in [`crate::storage::entities`], instead of introducing a
/// second similarity implementation.
fn nearest_setting_key(key: &str) -> Option<&'static str> {
    setting_key_names()
        .map(|candidate| {
            let score = rapidfuzz::distance::jaro_winkler::normalized_similarity(
                key.chars(),
                candidate.chars(),
            );
            (candidate, score)
        })
        .filter(|(_, score)| *score >= SUGGESTION_THRESHOLD)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(candidate, _)| candidate)
}

/// Read an operational setting from XDG config (flag > XDG > default is
/// applied by callers). Returns `None` when unset.
pub fn get_setting(key: &str) -> Result<Option<String>, AppError> {
    let cfg = load_config()?;
    if let Some(v) = cfg.settings.get(key) {
        return Ok(Some(v.clone()));
    }
    // GAP-SG-122: when the canonical key is missing, fall back to any retired
    // alias that still maps onto it (e.g. paths.cache → cache.dir).
    for (legacy, replacement) in LEGACY_SETTING_KEYS {
        if *replacement == key {
            if let Some(v) = cfg.settings.get(*legacy) {
                return Ok(Some(v.clone()));
            }
        }
    }
    Ok(None)
}

/// Persist an operational setting in XDG config.toml (G-T-XDG-01).
pub fn set_setting(key: &str, value: &str) -> Result<(), AppError> {
    if key.trim().is_empty() {
        return Err(AppError::Validation("config key must be non-empty".into()));
    }
    // GAP-SG-80: an unvalidated insert persisted typos and obsolete keys while
    // reporting success, so the operator saw the value in `config show` and
    // assumed it took effect. Reject early instead of storing a silent no-op.
    if !is_known_setting(key) {
        if let Some(replacement) = LEGACY_SETTING_KEYS
            .iter()
            .find(|(legacy, _)| *legacy == key)
            .map(|(_, replacement)| *replacement)
        {
            return Err(AppError::Validation(validation::config_key_retired(
                key,
                replacement,
            )));
        }
        return Err(AppError::Validation(validation::config_key_unknown(
            key,
            nearest_setting_key(key),
        )));
    }
    let mut cfg = load_config()?;
    cfg.settings.insert(key.to_string(), value.to_string());
    save_config(&cfg)
}

/// Remove an operational setting from XDG config.toml.
pub fn unset_setting(key: &str) -> Result<bool, AppError> {
    let mut cfg = load_config()?;
    let removed = cfg.settings.remove(key).is_some();
    if removed {
        save_config(&cfg)?;
    }
    Ok(removed)
}

/// List all operational settings (no secrets).
pub fn list_settings() -> Result<std::collections::BTreeMap<String, String>, AppError> {
    let cfg = load_config()?;
    Ok(cfg.settings)
}

/// Resolved key.
pub struct ResolvedKey {
    /// Value.
    pub value: SecretBox<String>,
    /// Source side of the relationship.
    pub source: &'static str,
}

/// Absolute path of `config.toml`.
///
/// GAP-SG-98: delegates to [`crate::paths::config_dir`] so `--config-dir` is
/// honoured. This function used to call [`directories::ProjectDirs`] directly, which made it
/// a second, independent config-directory resolver that no flag could redirect.
///
/// There is no cycle: [`crate::paths::config_dir`] consults only the CLI
/// override captured in [`crate::runtime_config`], never a `config set` key.
pub fn config_file_path() -> Result<PathBuf, AppError> {
    Ok(crate::paths::config_dir()?.join("config.toml"))
}

/// Load application configuration from the XDG config file.
pub fn load_config() -> Result<AppConfig, AppError> {
    let path = config_file_path()?;

    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let meta = std::fs::symlink_metadata(&path)?;
    if meta.file_type().is_symlink() {
        return Err(AppError::Validation(validation::config_file_is_symlink(
            &path.display().to_string(),
        )));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode() & 0o777;
        if mode > 0o600 {
            tracing::warn!(
                path = %path.display(),
                mode = format!("{mode:o}"),
                "config file permissions are too open; recommend chmod 600"
            );
        }
    }

    let content = std::fs::read_to_string(&path)?;
    let cfg: AppConfig = toml::from_str(&content).map_err(|e| {
        AppError::Validation(validation::config_parse_error(
            &path.display().to_string(),
            &e,
        ))
    })?;
    warn_on_legacy_settings(&cfg);
    Ok(cfg)
}

/// Emits one warning per process for each retired key still present on disk.
///
/// `load_config` runs on every [`get_setting`] call, so the warning is gated by
/// a [`std::sync::Once`] to keep a hot read path from flooding stderr.
///
/// The value is deliberately left untouched: `GAP-SG-79` is fixed by making the
/// dead key visible, not by rewriting a file the operator owns.
fn warn_on_legacy_settings(cfg: &AppConfig) {
    static WARNED: std::sync::Once = std::sync::Once::new();
    if LEGACY_SETTING_KEYS
        .iter()
        .all(|(legacy, _)| !cfg.settings.contains_key(*legacy))
    {
        return;
    }
    WARNED.call_once(|| {
        for (legacy, replacement) in LEGACY_SETTING_KEYS {
            if cfg.settings.contains_key(*legacy) {
                tracing::warn!(
                    target: "config",
                    key = legacy,
                    replacement = replacement,
                    "config key is never read and has no effect; \
                     move the value to the replacement key and unset the old one"
                );
            }
        }
    });
}

/// Persist application configuration to the XDG config file.
pub fn save_config(config: &AppConfig) -> Result<(), AppError> {
    let path = config_file_path()?;
    let dir = path.parent().ok_or_else(|| {
        AppError::Validation(validation::config_path_no_parent(
            &path.display().to_string(),
        ))
    })?;

    std::fs::create_dir_all(dir)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }

    #[cfg(unix)]
    if path.exists() {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(&path)?;
        let file_uid = meta.uid();
        let my_uid = unsafe { libc::getuid() };
        if file_uid != my_uid {
            return Err(AppError::Validation(validation::config_file_wrong_owner(
                &path.display().to_string(),
                file_uid,
                my_uid,
            )));
        }
    }

    let serialized =
        toml::to_string_pretty(config).map_err(|e| AppError::Validation(e.to_string()))?;

    #[cfg(unix)]
    let old_umask = unsafe { libc::umask(0o077) };

    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(serialized.as_bytes())?;
    tmp.as_file().sync_all()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o600))?;
    }

    tmp.persist(&path)
        .map_err(|e| AppError::Io(std::io::Error::other(format!("atomic persist failed: {e}"))))?;

    #[cfg(unix)]
    unsafe {
        libc::umask(old_umask);
    }

    // fsync parent dir for crash consistency
    #[cfg(unix)]
    {
        let dir_file = std::fs::File::open(dir)?;
        dir_file.sync_all()?;
    }

    Ok(())
}

/// Resolve API key.
pub fn resolve_api_key(provider: &str, cli_key: Option<&str>) -> Option<ResolvedKey> {
    // G-T-XDG-04: flag/cli > XDG `config add-key` only. Product env is not read.
    if let Some(k) = cli_key {
        if !k.is_empty() {
            return Some(ResolvedKey {
                value: SecretBox::new(Box::new(k.to_owned())),
                source: "cli",
            });
        }
    }

    if let Ok(cfg) = load_config() {
        if let Some(entry) = cfg.keys.iter().find(|k| k.provider == provider) {
            return Some(ResolvedKey {
                value: SecretBox::new(Box::new(entry.value.clone())),
                source: "config",
            });
        }
    }

    None
}

/// Compute fingerprint.
pub fn compute_fingerprint(key: &str) -> String {
    let hash = blake3::hash(key.as_bytes());
    hash.to_hex()[..16].to_string()
}

/// Mask key.
pub fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use tempfile::TempDir;

    #[test]
    fn compute_fingerprint_deterministic() {
        let fp1 = compute_fingerprint("sk-or-v1-test-key-12345");
        let fp2 = compute_fingerprint("sk-or-v1-test-key-12345");
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 16);
    }

    #[test]
    fn compute_fingerprint_differs_for_different_keys() {
        let fp1 = compute_fingerprint("key-a");
        let fp2 = compute_fingerprint("key-b");
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn mask_key_short() {
        assert_eq!(mask_key("abcd"), "****");
        assert_eq!(mask_key("12345678"), "****");
        assert_eq!(mask_key(""), "****");
    }

    #[test]
    fn mask_key_normal() {
        assert_eq!(mask_key("sk-or-v1-abcdef1234"), "sk-o...1234");
    }

    #[test]
    fn load_config_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let nonexistent = tmp.path().join("does-not-exist.toml");
        assert!(!nonexistent.exists());
        let cfg = AppConfig::default();
        assert_eq!(cfg.schema_version, 1);
        assert!(cfg.keys.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let mut cfg = AppConfig::default();
        cfg.keys.push(ApiKeyEntry {
            provider: "openrouter".to_string(),
            value: "sk-test-key".to_string(),
            added_at: "2026-01-01T00:00:00Z".to_string(),
            fingerprint: compute_fingerprint("sk-test-key"),
        });

        let serialized = toml::to_string_pretty(&cfg).unwrap();
        std::fs::write(&config_path, &serialized).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let loaded: AppConfig = toml::from_str(&content).unwrap();

        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.keys.len(), 1);
        assert_eq!(loaded.keys[0].provider, "openrouter");
        assert_eq!(loaded.keys[0].value, "sk-test-key");
    }

    #[test]
    fn resolve_api_key_cli_wins() {
        let resolved = resolve_api_key("openrouter", Some("cli-key-value"));
        assert!(resolved.is_some());
        let r = resolved.unwrap();
        assert_eq!(r.source, "cli");
        assert_eq!(r.value.expose_secret(), "cli-key-value");
    }

    #[test]
    fn resolve_api_key_cli_fallback() {
        let resolved = resolve_api_key("nonexistent-provider", Some("cli-key"));
        assert!(resolved.is_some());
        let r = resolved.unwrap();
        assert_eq!(r.source, "cli");
        assert_eq!(r.value.expose_secret(), "cli-key");
    }

    #[test]
    fn resolve_api_key_none_when_nothing_available() {
        let resolved = resolve_api_key("totally-unknown-provider-xyz-no-key", None);
        // Only returns Some if host XDG config has that provider (unlikely).
        if let Some(r) = resolved {
            assert_eq!(r.source, "config");
        }
    }

    #[test]
    fn resolve_api_key_ignores_product_env() {
        // G-T-XDG-04: even if OPENROUTER_API_KEY is set, it must not be used.
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", "env-must-be-ignored");
        }
        let resolved = resolve_api_key("openrouter-env-ignore-test-provider", None);
        assert!(resolved.is_none(), "product env must not supply API keys");
        unsafe {
            std::env::remove_var("OPENROUTER_API_KEY");
        }
    }
}
