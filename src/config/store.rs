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
///
/// # Declared limit: the permission check below is Unix-only, on purpose
///
/// The write side has a Windows counterpart, `restrict_to_current_user` in
/// `crate::config::permissions` — named in prose because it is private, and a
/// public doc that links a private item is denied by `[lints.rustdoc]`; the
/// READ side deliberately does not, so on Windows a `config.toml` with a loose
/// ACL — holding the OpenRouter API key — is loaded without a word, while on
/// Unix the same file draws a warning. That asymmetry is stated here rather
/// than papered over, because a silent gap is the one an operator cannot plan
/// around.
///
/// It stands for three reasons. First, this check only WARNS: it never refuses
/// the file, so what Windows loses is a log line, not a guarantee. Second, the
/// guarantee itself is on the write path, where `SetNamedSecurityInfoW`
/// installs a PROTECTED single-ACE DACL — any file this CLI wrote is already
/// restricted, and the loose-ACL case can only arise from a file some other
/// tool produced. Third, deciding "too open" from a DACL means enumerating
/// ACEs and classifying trustees in `unsafe` Win32, and this project has no
/// Windows CI: `permissions.rs` already declares its Windows branch as
/// reviewed-but-never-executed code. Adding a second unverifiable unsafe
/// surface to gain a warning is a worse trade than admitting the limit.
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

    // UNIX-ONLY BY DECLARATION, not by omission: see the `# Declared limit`
    // section on this function for why no Windows counterpart is attempted.
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

    // GAP-SG-269: Windows counterpart of the `0o600` above, applied to the
    // TEMPORARY and therefore BEFORE the rename — the same ordering Unix has had
    // all along.
    //
    // This closes a window rather than shortening one, and the reason is a
    // property of `persist` itself: it cannot move a file across filesystems,
    // and fails instead of falling back to copy-and-delete. So the rename is
    // always intra-volume, and an intra-volume move carries the file's own
    // security descriptor with it; only the inter-volume case would re-inherit
    // from the destination's parent, and `persist` makes that case impossible.
    // Hardening the temporary is therefore equivalent to hardening the target,
    // with no interval during which the key sits under the directory's
    // inheritable ACEs.
    restrict_to_current_user(tmp.path())?;

    tmp.persist(&path)
        .map_err(|e| AppError::Io(std::io::Error::other(format!("atomic persist failed: {e}"))))?;

    // Re-applied to the final path, and kept deliberately rather than trusted
    // away. The argument above says the descriptor survives the rename; this
    // call is what makes the guarantee hold even if that argument is ever wrong
    // — a different tempfile backend, a filesystem that reports one volume and
    // behaves as two. The cost is one syscall on a path that runs once per
    // `config set`; the cost of being wrong is a readable credential.
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

    // fsync parent dir for crash consistency: `persist` documents that neither
    // the contents nor the containing directory are synchronised, so the rename
    // itself is not durable until the directory is. The file's own contents were
    // covered by `sync_all` before the rename.
    //
    // Unix only, and that is not an omission. Windows exposes no supported way to
    // flush a directory entry — `FlushFileBuffers` wants a file handle, and a
    // directory handle opened for it is not a documented contract — so there is
    // nothing to call rather than something skipped.
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
