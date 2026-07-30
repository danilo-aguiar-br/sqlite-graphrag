//! XDG path resolution and traversal-safe overrides.
//!
//! Resolves data directories via [`directories::ProjectDirs`] and validates
//! that user-supplied paths cannot escape the project root.
//!
//! Precedence (G-T-XDG-04): CLI flag `--db` / `db_override` → XDG setting
//! `db.path` → XDG data dir default `graphrag.sqlite` → cwd fallback.
//! Product `SQLITE_GRAPHRAG_*` env vars are **not** read.

use crate::config;
use crate::errors::AppError;
use crate::i18n::validation;
use crate::runtime_config;
use directories::ProjectDirs;
use std::path::{Component, Path, PathBuf};

/// Resolved filesystem paths used by the CLI at runtime.
#[derive(Debug, Clone)]
pub struct AppPaths {
    /// Absolute path to the SQLite database file.
    pub db: PathBuf,
    /// Directory where embedding model files are cached.
    pub models: PathBuf,
}

impl AppPaths {
    /// Resolve.
    pub fn resolve(db_override: Option<&str>) -> Result<Self, AppError> {
        let proj = ProjectDirs::from("", "", "sqlite-graphrag").ok_or_else(|| {
            AppError::Io(std::io::Error::other("could not determine home directory"))
        })?;

        // GAP-SG-94: one resolver for the cache root, shared with `lock` and
        // `llm_slots`, so a host can never end up with two cache directories.
        let cache_root = cache_dir()?;

        let db = if let Some(p) = db_override {
            validate_path(p)?;
            PathBuf::from(p)
        } else if let Ok(Some(cfg_path)) = config::get_setting("db.path") {
            if !cfg_path.is_empty() {
                validate_path(&cfg_path)?;
                PathBuf::from(cfg_path)
            } else {
                default_db_path(&proj)?
            }
        } else {
            default_db_path(&proj)?
        };

        Ok(Self {
            db,
            models: cache_root.join("models"),
        })
    }

    /// Ensure dirs.
    pub fn ensure_dirs(&self) -> Result<(), AppError> {
        for dir in [parent_or_err(&self.db)?, self.models.as_path()] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

fn default_db_path(proj: &ProjectDirs) -> Result<PathBuf, AppError> {
    // Prefer XDG data dir; fall back to cwd for bare-metal one-shot without home.
    let data = proj.data_dir();
    if data.as_os_str().is_empty() {
        return Ok(std::env::current_dir()
            .map_err(AppError::Io)?
            .join("graphrag.sqlite"));
    }
    Ok(data.join("graphrag.sqlite"))
}

fn validate_path(p: &str) -> Result<(), AppError> {
    if Path::new(p).components().any(|c| c == Component::ParentDir) {
        return Err(AppError::Validation(validation::path_traversal(p)));
    }
    Ok(())
}

/// Returns the config directory for the application.
///
/// Precedence (G-T-XDG-04): CLI `--config-dir` → OS config directory. No XDG
/// `config set` key participates: the config file lives inside this directory,
/// so consulting it here would be circular.
pub fn config_dir() -> Result<PathBuf, AppError> {
    if let Some(dir) = runtime_config::config_dir_override() {
        validate_path(&dir)?;
        return Ok(PathBuf::from(dir));
    }
    let proj = ProjectDirs::from("", "", "sqlite-graphrag").ok_or_else(|| {
        AppError::Io(std::io::Error::other(
            "could not determine home directory for config",
        ))
    })?;
    Ok(proj.config_dir().to_path_buf())
}

/// Returns the cache root for lock files, model files and other artifacts.
///
/// Precedence (G-T-XDG-04): CLI `--cache-dir` → XDG `cache.dir` → OS cache
/// directory.
///
/// GAP-SG-94: this is the SINGLE resolver for the cache root. [`crate::lock`]
/// and [`crate::llm_slots`] delegate here. Before v1.2.0 `lock` read a separate
/// key `paths.cache` while this module read `cache.dir`, so setting one moved
/// the lock files and setting the other moved the model files.
pub fn cache_dir() -> Result<PathBuf, AppError> {
    if let Some(dir) = runtime_config::cache_dir_override() {
        validate_path(&dir)?;
        return Ok(PathBuf::from(dir));
    }
    let proj = ProjectDirs::from("", "", "sqlite-graphrag").ok_or_else(|| {
        AppError::Io(std::io::Error::other(
            "could not determine cache directory for sqlite-graphrag",
        ))
    })?;
    Ok(proj.cache_dir().to_path_buf())
}

pub(crate) fn parent_or_err(path: &Path) -> Result<&Path, AppError> {
    path.parent().ok_or_else(|| {
        AppError::Validation(validation::path_no_valid_parent(
            &path.display().to_string(),
        ))
    })
}

/// Derives a sidecar file path next to the database (e.g. the enrich/ingest
/// queue), so worklist files follow `--db` instead of the process CWD. Falls
/// back to the bare filename (CWD) when `db_path` has no parent — preserving the
/// legacy default-DB layout.
pub fn sidecar_path(db_path: &Path, filename: &str) -> PathBuf {
    db_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.join(filename))
        .unwrap_or_else(|| PathBuf::from(filename))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn flag_overrides_default() {
        let tmp = TempDir::new().expect("tempdir");
        let db_flag = tmp.path().join("via-flag.sqlite");
        let paths = AppPaths::resolve(Some(db_flag.to_str().expect("utf8")))
            .expect("resolve with flag");
        assert_eq!(paths.db, db_flag);
    }

    #[test]
    fn traversal_in_flag_rejected() {
        let result = AppPaths::resolve(Some("/tmp/../etc/passwd"));
        assert!(
            matches!(result, Err(AppError::Validation(_))),
            "traversal must fail as Validation, got {result:?}"
        );
    }

    #[test]
    fn default_resolve_ok() {
        let paths = AppPaths::resolve(None).expect("default resolve");
        assert!(!paths.db.as_os_str().is_empty());
        assert!(paths.models.ends_with("models"));
    }

    #[test]
    fn sidecar_path_joins_parent() {
        let p = sidecar_path(Path::new("/data/db/graphrag.sqlite"), "enrich.queue");
        assert_eq!(p, PathBuf::from("/data/db/enrich.queue"));
    }
}
