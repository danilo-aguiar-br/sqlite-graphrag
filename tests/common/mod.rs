//! Test-only helpers shared by `tests/integration.rs`,
//! `tests/prd_compliance.rs`, and `tests/schema_migration_integration.rs`.
//!
//! The helpers in this module exist for ONE reason: the v1.0.76 binary
//! spawns `claude` or `codex` for every `remember` / `ingest` / `edit`,
//! and those CLIs require OAuth login plus a network round-trip. To run
//! the slow-tests hermetically on a CI runner we copy the two mock
//! scripts in `tests/mock-llm/` into a per-test temp directory and
//! prepend that directory to PATH so the binary finds the mocks first.
//!
//! `mock_llm_path` returns the directory; the caller wires it via
//! `Command::env("PATH", prepend_path)`.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Copies the bundled `claude` and `codex` mock scripts into a fresh
/// temp directory and makes them executable. Returns the directory.
///
/// Tests should call this once and prepend the returned path to PATH
/// in every `Command` they build. The directory is independent of
/// the test's own `TempDir` because the mock binaries must survive
/// for the lifetime of the spawned `sqlite-graphrag` subprocess and
/// Rust drops `TempDir` instances eagerly when they go out of scope.
#[allow(dead_code)]
pub fn mock_llm_path() -> PathBuf {
    let dir = TempDir::new()
        .expect("mock_llm_path: TempDir must be creatable")
        .keep();

    for name in &["claude", "codex"] {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("mock-llm")
            .join(name);
        let dst = dir.join(name);
        fs::copy(&src, &dst)
            .unwrap_or_else(|e| panic!("mock_llm_path: copy {src:?} -> {dst:?} failed: {e}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dst)
                .expect("mock_llm_path: stat dst")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dst, perms).expect("mock_llm_path: chmod 755");
        }
    }

    dir
}

/// Prepends `mock_dir` to the inherited PATH and returns the new PATH
/// string. Use as `cmd.env("PATH", prepend_path(&mock_dir))`.
///
/// The function does NOT set PATH globally. It returns the composite
/// value for the caller to inject per-command, which keeps tests
/// parallel-safe.
#[allow(dead_code)]
pub fn prepend_path(mock_dir: &std::path::Path) -> std::ffi::OsString {
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut entries = vec![mock_dir.to_path_buf()];
    entries.extend(std::env::split_paths(&current));
    std::env::join_paths(entries)
        .expect("prepend_path: PATH entries must not contain the separator")
}

/// Hermetic per-test environment.
///
/// GAP-SG-92 / GAP-SG-101: before v1.2.0 tests "isolated" themselves with
/// `SQLITE_GRAPHRAG_CACHE_DIR` and `SQLITE_GRAPHRAG_DB_PATH`, retired product
/// env vars that no production code reads. The temp directories were decoration
/// and the binary wrote to the developer's REAL cache and REAL database.
///
/// This guard closes both channels at once:
/// - `XDG_*` and `HOME` cover the OS defaults on Linux.
/// - `--config-dir` / `--cache-dir` cover macOS and Windows, where
///   `directories` ignores `XDG_*` entirely.
/// - `--db` covers the database, which has no env channel by design.
///
/// The returned value owns the `TempDir`. Bind it to a NAMED local
/// (`let env = common::isolated_env();`); binding to `_` drops it immediately
/// and deletes the sandbox while the test is still running.
#[must_use = "dropping the guard deletes the sandbox mid-test"]
pub struct IsolatedEnv {
    root: TempDir,
    mock_llm: PathBuf,
    db: PathBuf,
}

/// Builds an [`IsolatedEnv`] with every directory redirected into a fresh
/// `TempDir`.
#[allow(dead_code)]
pub fn isolated_env() -> IsolatedEnv {
    let root = TempDir::new().expect("isolated_env: TempDir must be creatable");
    for sub in &["home", "cache", "data", "config", "runtime", "db"] {
        fs::create_dir_all(root.path().join(sub))
            .unwrap_or_else(|e| panic!("isolated_env: mkdir {sub} failed: {e}"));
    }
    let db = root.path().join("db").join("test.sqlite");
    IsolatedEnv {
        mock_llm: mock_llm_path(),
        db,
        root,
    }
}

#[allow(dead_code)]
impl IsolatedEnv {
    /// Path to pass as `--db`.
    pub fn db(&self) -> &std::path::Path {
        &self.db
    }

    /// Sandbox root. Every artifact the binary creates must live under it.
    pub fn root(&self) -> &std::path::Path {
        self.root.path()
    }

    /// Cache root, as seen by `lock`, `llm_slots` and the model cache.
    ///
    /// `ProjectDirs::cache_dir()` appends the application directory under
    /// `XDG_CACHE_HOME`, so a test planting lock files by hand must use THIS
    /// path, not `root().join("cache")`.
    pub fn cache(&self) -> PathBuf {
        self.root.path().join("cache").join("sqlite-graphrag")
    }

    /// Config directory holding `config.toml`.
    pub fn config(&self) -> PathBuf {
        self.root.path().join("config").join("sqlite-graphrag")
    }

    /// Fully wired command with no subcommand yet.
    ///
    /// Does NOT append `--db`, because `--db` is a per-subcommand argument and
    /// must come after the subcommand. Use [`IsolatedEnv::sgr`] instead unless
    /// the subcommand does not accept `--db` (for example `config path`).
    pub fn cmd(&self) -> assert_cmd::Command {
        let mut c = assert_cmd::Command::cargo_bin("sqlite-graphrag")
            .expect("sqlite-graphrag binary not found");
        c.env("PATH", prepend_path(&self.mock_llm))
            .env("HOME", self.root.path().join("home"))
            .env("XDG_CACHE_HOME", self.root.path().join("cache"))
            .env("XDG_DATA_HOME", self.root.path().join("data"))
            .env("XDG_CONFIG_HOME", self.root.path().join("config"))
            .env("XDG_RUNTIME_DIR", self.root.path().join("runtime"))
            .arg("--config-dir")
            .arg(self.config())
            .arg("--cache-dir")
            .arg(self.cache());
        c
    }

    /// [`IsolatedEnv::cmd`] plus `<subcommand> --db <sandbox db>`, in that order.
    pub fn sgr(&self, subcommand: &str) -> assert_cmd::Command {
        let mut c = self.cmd();
        c.arg(subcommand).arg("--db").arg(&self.db);
        c
    }
}

/// Plants `db.path` under `config_dir` so commands without an explicit `--db`
/// still resolve to the sandbox database (GAP-SG-101 / G-T-XDG-04).
///
/// Prefer [`IsolatedEnv::sgr`] for new tests. This helper exists so legacy
/// suites that build `cmd(tmp).arg("init")` keep working after product env
/// was retired — without rewriting every call site to put `--db` after the
/// subcommand.
#[allow(dead_code)]
pub fn plant_db_path(config_dir: &std::path::Path, db: &std::path::Path) {
    fs::create_dir_all(config_dir).expect("plant_db_path: mkdir config");
    let db_str = db
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let cfg = format!("schema_version = 1\n\n[settings]\n\"db.path\" = \"{db_str}\"\n");
    fs::write(config_dir.join("config.toml"), cfg).expect("plant_db_path: write config.toml");
}

/// Wires an `assert_cmd::Command` for a legacy `TempDir`-based test.
///
/// Sets OS isolation (`HOME` / `XDG_*`), `--config-dir` / `--cache-dir`, and
/// plants `db.path` to `tmp/<db_name>`. Does **not** set any product
/// `SQLITE_GRAPHRAG_*` env var.
#[allow(dead_code)]
pub fn wire_assert_cmd(tmp: &TempDir, c: &mut assert_cmd::Command, db_name: &str) {
    let root = tmp.path();
    let config_dir = root.join("config");
    let cache_dir = root.join("cache");
    plant_db_path(&config_dir, &root.join(db_name));
    c.env("HOME", root.join("home"))
        .env("XDG_CACHE_HOME", root.join("xdg_cache"))
        .env("XDG_CONFIG_HOME", root.join("xdg_config"))
        .env("XDG_DATA_HOME", root.join("xdg_data"))
        .env("XDG_RUNTIME_DIR", root.join("xdg_runtime"))
        .arg("--config-dir")
        .arg(&config_dir)
        .arg("--cache-dir")
        .arg(&cache_dir);
}

/// Same isolation as [`wire_assert_cmd`] for raw `std::process::Command`
/// (benches, thread-spawned children, signal tests).
#[allow(dead_code)]
pub fn wire_std_cmd(root: &std::path::Path, c: &mut std::process::Command, db: &std::path::Path) {
    let config_dir = root.join("config");
    let cache_dir = root.join("cache");
    plant_db_path(&config_dir, db);
    c.env("HOME", root.join("home"))
        .env("XDG_CACHE_HOME", root.join("xdg_cache"))
        .env("XDG_CONFIG_HOME", root.join("xdg_config"))
        .env("XDG_DATA_HOME", root.join("xdg_data"))
        .env("XDG_RUNTIME_DIR", root.join("xdg_runtime"))
        .arg("--config-dir")
        .arg(&config_dir)
        .arg("--cache-dir")
        .arg(&cache_dir);
}
