//! Location, load and atomic persistence of `config.toml`.
//!
//! Owns the on-disk representation: symlink refusal, permission warning,
//! tempfile-fsync-rename persistence and the platform hardening call-outs.

use super::permissions::restrict_to_current_user;
use super::registry::LEGACY_SETTING_KEYS;
use super::AppConfig;
use crate::errors::AppError;
use crate::i18n::validation;
use std::path::PathBuf;

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

    // GAP-SG-144: Windows counterpart of the `0o700` above.
    //
    // ASYMMETRY, DELIBERATE: this one WARNS, while the file below ABORTS. Do
    // not "uniformise" them. The directory is defence in depth, not the primary
    // guarantee: the file DACL sets PROTECTED_DACL_SECURITY_INFORMATION, which
    // severs parent inheritance by construction, so a locked-down file stays
    // locked down even if the directory restriction failed. Its remaining value
    // is narrowing the TOCTOU window described on `restrict_to_current_user`,
    // which is worth a warning but not worth losing the operator's config.
    if let Err(e) = restrict_to_current_user(dir) {
        tracing::warn!(
            path = %dir.display(),
            error = %e,
            "could not restrict config directory to the current user; \
             the config file DACL remains the primary protection"
        );
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

    // GAP-SG-144: Windows counterpart of the `0o600` above. Applied AFTER the
    // rename, because the DACL must land on the file that now carries the API
    // key, not on the temporary that no longer exists.
    //
    // FAIL-CLOSED, unlike the directory above: this file holds the OpenRouter
    // API key. Keeping it after the restriction failed produces exactly the
    // state GAP-SG-144 exists to eliminate — a readable credential — and a
    // warning in a log nobody reads is not a mitigation. Best-effort is the
    // right policy for performance hardening, not for secret protection.
    restrict_to_current_user(&path)?;

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

#[cfg(test)]
mod tests {
    use crate::config::{compute_fingerprint, ApiKeyEntry, AppConfig};
    use tempfile::TempDir;

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
}
