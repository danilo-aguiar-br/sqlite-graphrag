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

/// Which layer of configuration supplied the target database.
///
/// GAP-SG-205. Ordered from explicit to ambient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSource {
    /// The command line named it with `--db`.
    Argv,
    /// The XDG key `db.path` named it.
    Xdg,
    /// Nothing named it and the compiled default was used.
    Default,
}

impl TargetSource {
    /// Wire spelling for the output record.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Argv => "argv",
            Self::Xdg => "xdg",
            Self::Default => "default",
        }
    }
}

/// Whether this process is allowed to inherit its target from the environment.
///
/// GAP-SG-207. The Explicit Target Designation rule asks a verb with a side
/// effect to name its target in the argv, and to fail closed when it does not.
/// This is the record of that decision, taken once from the parsed command line.
#[derive(Debug, Clone, Copy)]
pub struct WritePolicy {
    /// The subcommand changes durable state and does not create its own target.
    pub requires_explicit_target: bool,
    /// `--use-active` was passed, dispensing the requirement on purpose.
    pub use_active: bool,
}

/// The policy this process runs under, installed before any subcommand runs.
static WRITE_POLICY: std::sync::OnceLock<WritePolicy> = std::sync::OnceLock::new();

/// Installs the target policy for this process. Idempotent, first call wins.
///
/// Called from `crate::cli::GlobalArgs` during flag validation, which is the
/// one hook that runs after the command line is parsed and before any handler
/// executes. Absent — the library used directly, or a unit test — resolution
/// stays permissive, because the rule governs the CLI contract and not the API.
pub fn install_write_policy(policy: WritePolicy) {
    let _ = WRITE_POLICY.set(policy);
}

/// Refuses a target NOBODY named, for a verb that changes durable state.
///
/// # Which layer earns a refusal, and why not all of them
///
/// The Explicit Target Designation rule asks a destructive verb to prove its
/// target came from the argv, and every layer that is not the argv is ambient.
/// Both [`TargetSource::Xdg`] and [`TargetSource::Default`] therefore fail
/// closed here.
///
/// Until this was tightened the fence fired on `Default` alone, and `Xdg` was
/// permitted on the argument that `db.path` is a first-class registry key: an
/// operator who ran `config set` DID choose the database, just once instead of
/// on every invocation. That argument is answered by the SCOPE of the key, which
/// it did not account for. `db.path` is a HOST setting — there is no per-project
/// configuration in this product — so it does not name "the database for this
/// work", it names one database for every directory on the machine. This host
/// carries sixteen of them, and the repository's own operating rules already say
/// never to reach for `config set` to solve one project's problem, precisely
/// because the change leaves the folder. A key that cannot legitimately mean
/// "this project" cannot legitimately designate this project's write target.
///
/// The read path is untouched: [`WritePolicy::requires_explicit_target`] is
/// false for idempotent verbs, so inheritance stays available exactly where it
/// costs nothing. And `--use-active` still dispenses the requirement, which is
/// the explicit opt-in the rule itself authorises — ambient authority is refused
/// as a DEFAULT, never as an impossibility.
///
/// # Errors
/// Returns [`AppError::Usage`] — exit `2` — when a mutating subcommand resolved
/// its target from any layer other than the argv, with no explicit dispensation.
fn enforce_explicit_target(source: TargetSource) -> Result<(), AppError> {
    let Some(policy) = WRITE_POLICY.get() else {
        return Ok(());
    };
    if source == TargetSource::Argv || !policy.requires_explicit_target || policy.use_active {
        return Ok(());
    }
    // Two ambient layers, two messages: the operator's next move differs. One has
    // a configured value to point at and override, the other has nothing named
    // anywhere. A single message would have to describe both and would name the
    // wrong remedy in half the cases.
    let message = match source {
        TargetSource::Xdg => validation::target_inherited_from_config(),
        _ => validation::target_not_designated(),
    };
    Err(AppError::Usage {
        message,
        // Nothing the caller typed was discarded here: the refusal is about an
        // argument that is MISSING, not one that was ignored.
        discarded_flags: Vec::new(),
    })
}

/// The target this one-shot process resolved. First resolution wins.
static RESOLVED_TARGET: std::sync::OnceLock<(PathBuf, TargetSource)> = std::sync::OnceLock::new();

/// Records the resolved target so the output layer can report it.
fn record_target(db: &std::path::Path, source: TargetSource) {
    let _ = RESOLVED_TARGET.set((db.to_path_buf(), source));
}

impl AppPaths {
    /// Which layer supplied the database this process is about to touch.
    ///
    /// Only [`TargetSource::Argv`] is an explicit designation. The other two are
    /// ambient authority: legitimate for an idempotent read, and the shape of a
    /// confused deputy for a write.
    pub fn target_source() -> Option<TargetSource> {
        RESOLVED_TARGET.get().map(|(_, source)| *source)
    }

    /// The database path this process resolved, once it has resolved one.
    pub fn resolved_target() -> Option<&'static std::path::Path> {
        RESOLVED_TARGET.get().map(|(path, _)| path.as_path())
    }

    /// Resolves the database and cache paths for this invocation.
    ///
    /// # Errors
    /// Returns [`AppError::Io`] when the home directory cannot be determined,
    /// and a validation error when a supplied path is rejected.
    pub fn resolve(db_override: Option<&str>) -> Result<Self, AppError> {
        let proj = ProjectDirs::from("", "", "sqlite-graphrag").ok_or_else(|| {
            AppError::Io(std::io::Error::other("could not determine home directory"))
        })?;

        // GAP-SG-94: one resolver for the cache root, shared with `lock` and
        // `llm_slots`, so a host can never end up with two cache directories.
        let cache_root = cache_dir()?;

        // GAP-SG-205: the target is resolved from three layers, and only the
        // first is the argv. A write verb that reaches this function without
        // `--db` mutates a database the command line never named — the confused
        // deputy the Explicit Target Designation rule describes. Recording WHICH
        // layer won is what makes that detectable at all: until v1.2.6 no
        // envelope reported the resolved target, so the wrong database could be
        // written with no trace in the output.
        let (db, source) = if let Some(p) = db_override {
            validate_path(p)?;
            (PathBuf::from(p), TargetSource::Argv)
        } else {
            match config::get_setting("db.path") {
                // An empty setting is not a designation, so it falls through to
                // the compiled default exactly as a missing one does.
                Ok(Some(cfg_path)) if !cfg_path.is_empty() => {
                    validate_path(&cfg_path)?;
                    (PathBuf::from(cfg_path), TargetSource::Xdg)
                }
                _ => (default_db_path(&proj)?, TargetSource::Default),
            }
        };
        // GAP-SG-207, checked AFTER resolution because the verdict depends on
        // WHICH layer won, and only resolution knows that. Placed here rather
        // than on each argument struct because this is the single funnel: 47
        // structs declare `--db` and all of them converge on this function.
        enforce_explicit_target(source)?;
        record_target(&db, source);

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
        let paths =
            AppPaths::resolve(Some(db_flag.to_str().expect("utf8"))).expect("resolve with flag");
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
