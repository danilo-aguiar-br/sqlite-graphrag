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
    pub fn resolve(db_override: Option<&str>) -> Result<Self, AppError> {
        let proj = ProjectDirs::from("", "", "sqlite-graphrag").ok_or_else(|| {
            AppError::Io(std::io::Error::other("could not determine home directory"))
        })?;

        // Cache always under XDG cache (or ProjectDirs); optional XDG setting cache.dir
        let cache_root = if let Ok(Some(override_dir)) = config::get_setting("cache.dir") {
            if !override_dir.is_empty() {
                validate_path(&override_dir)?;
                PathBuf::from(override_dir)
            } else {
                proj.cache_dir().to_path_buf()
            }
        } else {
            proj.cache_dir().to_path_buf()
        };

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

/// Returns the XDG config directory for the application.
pub fn config_dir() -> Result<PathBuf, AppError> {
    let proj = ProjectDirs::from("", "", "sqlite-graphrag").ok_or_else(|| {
        AppError::Io(std::io::Error::other(
            "could not determine home directory for config",
        ))
    })?;
    Ok(proj.config_dir().to_path_buf())
}

pub(crate) fn parent_or_err(path: &Path) -> Result<&Path, AppError> {
    path.parent().ok_or_else(|| {
        AppError::Validation(format!(
            "path '{}' has no valid parent component",
            path.display()
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
