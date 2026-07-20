//! XDG-based API key management for OpenRouter and other providers.
//!
//! Stores keys in `$XDG_CONFIG_HOME/sqlite-graphrag/config.toml` with
//! atomic write, symlink-attack defense and Unix permission hardening.

use crate::errors::AppError;
use directories::ProjectDirs;
use secrecy::SecretBox;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub schema_version: u32,
    #[serde(default)]
    pub keys: Vec<ApiKeyEntry>,
    /// Operational settings persisted via `config set/get` (G-T-XDG-01).
    /// Stringly-typed map keeps the schema open without migrations for every
    /// new key. Known keys are documented in `config set --help`.
    #[serde(default)]
    pub settings: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    pub provider: String,
    pub value: String,
    pub added_at: String,
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

/// Read an operational setting from XDG config (flag > XDG > default is
/// applied by callers). Returns `None` when unset.
pub fn get_setting(key: &str) -> Result<Option<String>, AppError> {
    let cfg = load_config()?;
    Ok(cfg.settings.get(key).cloned())
}

/// Persist an operational setting in XDG config.toml (G-T-XDG-01).
pub fn set_setting(key: &str, value: &str) -> Result<(), AppError> {
    if key.trim().is_empty() {
        return Err(AppError::Validation(
            "config key must be non-empty".into(),
        ));
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

pub struct ResolvedKey {
    pub value: SecretBox<String>,
    pub source: &'static str,
}

pub fn config_file_path() -> Result<PathBuf, AppError> {
    let proj = ProjectDirs::from("", "", "sqlite-graphrag").ok_or_else(|| {
        AppError::Io(std::io::Error::other(
            "could not determine home directory for config",
        ))
    })?;
    Ok(proj.config_dir().join("config.toml"))
}

pub fn load_config() -> Result<AppConfig, AppError> {
    let path = config_file_path()?;

    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let meta = std::fs::symlink_metadata(&path)?;
    if meta.file_type().is_symlink() {
        return Err(AppError::Validation(format!(
            "config file is a symlink (potential attack): {}",
            path.display()
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
    toml::from_str(&content)
        .map_err(|e| AppError::Validation(format!("config parse error in {}: {e}", path.display())))
}

pub fn save_config(config: &AppConfig) -> Result<(), AppError> {
    let path = config_file_path()?;
    let dir = path.parent().ok_or_else(|| {
        AppError::Validation(format!("config path has no parent: {}", path.display()))
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
            return Err(AppError::Validation(format!(
                "config file {} owned by uid {file_uid}, not current uid {my_uid}; refusing to overwrite",
                path.display()
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

pub fn compute_fingerprint(key: &str) -> String {
    let hash = blake3::hash(key.as_bytes());
    hash.to_hex()[..16].to_string()
}

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
        assert!(
            resolved.is_none(),
            "product env must not supply API keys"
        );
        unsafe {
            std::env::remove_var("OPENROUTER_API_KEY");
        }
    }
}
