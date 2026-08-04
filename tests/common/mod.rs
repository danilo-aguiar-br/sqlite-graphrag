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

pub mod openrouter_mock;
#[allow(unused_imports)]
pub use openrouter_mock::{
    global_stub, write_sandbox_config, write_sandbox_config_without_key, OpenRouterStub,
};

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
/// `TempDir`, and an offline OpenRouter stub already wired into its config.
///
/// The stub replaces the retired `tests/mock-llm/` shell scripts: since the
/// headless backends were removed, HTTP is the only transport left, so the
/// offline seam moved from `PATH` to `network.openrouter.*`.
#[allow(dead_code)]
pub fn isolated_env() -> IsolatedEnv {
    let root = TempDir::new().expect("isolated_env: TempDir must be creatable");
    for sub in &["home", "cache", "data", "config", "runtime", "db"] {
        fs::create_dir_all(root.path().join(sub))
            .unwrap_or_else(|e| panic!("isolated_env: mkdir {sub} failed: {e}"));
    }
    let db = root.path().join("db").join("test.sqlite");
    let env = IsolatedEnv {
        mock_llm: mock_llm_path(),
        db,
        root,
    };
    write_sandbox_config(&env.config(), None);
    env
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
            .arg(self.cache())
            // The OpenRouter client is only constructed when a model is named
            // (`main.rs`), so without this the sandbox would still fall through
            // to "client not initialised" and exit 11.
            .arg("--embedding-model")
            .arg(openrouter_mock::STUB_MODEL);
        c
    }

    /// Endpoint the stub answers embeddings on, for tests that assert on it.
    pub fn embeddings_url(&self) -> &str {
        global_stub().embeddings_url()
    }

    /// Endpoint the stub answers chat completions on.
    pub fn chat_url(&self) -> &str {
        global_stub().chat_url()
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
    // Also plants the offline OpenRouter endpoints and a test key: since the
    // headless backends were removed, a sandbox without them reaches the real
    // api.openrouter.ai and dies with exit 11.
    write_sandbox_config(config_dir, Some(db));
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
        .arg(&cache_dir)
        .arg("--embedding-model")
        .arg(openrouter_mock::STUB_MODEL);
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
        .arg(&cache_dir)
        .arg("--embedding-model")
        .arg(openrouter_mock::STUB_MODEL);
}

// ---------------------------------------------------------------------------
// Integration-suite command builders (v1.2.5)
// ---------------------------------------------------------------------------
//
// These lived at the top of `tests/integration.rs` until that 2 485-line file
// was split into `integration_bootstrap`, `integration_memory_crud` and
// `integration_graph`. Three copies of the same five helpers would have been
// the alternative, so they moved here — which is what this module is for.

/// Builds a fresh `Command` with the mock LLM PATH prepended.
///
/// The bundled mocks under `tests/mock-llm/` stand in for the subprocess
/// backends the pre-v1.2.0 binary spawned on every write. The mock directory is
/// leaked (no `TempDir` cleanup) so the spawned subprocess always finds it.
#[allow(dead_code)]
pub fn sgr_cmd() -> assert_cmd::Command {
    let mock_dir = mock_llm_path();
    let mut c = assert_cmd::Command::cargo_bin("sqlite-graphrag")
        .expect("sqlite-graphrag binary not found");
    c.env("PATH", prepend_path(&mock_dir));
    c
}

/// Isolated `Command` with a per-test `TempDir` database and shared model cache.
#[allow(dead_code)]
pub fn cmd(tmp: &TempDir) -> assert_cmd::Command {
    // GAP-SG-101: product env is not read (G-T-XDG-04).
    let mut c = sgr_cmd();
    wire_assert_cmd(tmp, &mut c, "test.sqlite");
    c
}

/// Runs `init` against the per-test database.
#[allow(dead_code)]
pub fn init_db(tmp: &TempDir) {
    cmd(tmp).arg("init").assert().success();
}

/// Isolated command pinned to a per-directory `graphrag.sqlite`.
///
/// GAP-SG-101 / G-T-XDG-04 (v1.2.0): the invocation directory stopped being an
/// authority for database resolution. The order became `--db` > XDG `db.path` >
/// XDG data dir, so a helper that only redirects `XDG_*` now sends every write
/// to `XDG_DATA_HOME/sqlite-graphrag/graphrag.sqlite` instead of `dir`. The
/// question the callers ask — "does each directory get its OWN database, and
/// does CRUD round-trip against that file?" — is unchanged; only the channel
/// that pins the file moved from the cwd to `db.path`.
#[allow(dead_code)]
pub fn isolated_cmd_in(dir: &std::path::Path) -> assert_cmd::Command {
    let mut c = sgr_cmd();
    c.current_dir(dir);
    c.env("HOME", dir.join("home"));
    c.env("XDG_CACHE_HOME", dir.join("cache"));
    c.env("XDG_CONFIG_HOME", dir.join("config_home"));
    c.env("XDG_DATA_HOME", dir.join("data"));
    plant_db_path(&dir.join("config"), &dir.join("graphrag.sqlite"));
    c.arg("--config-dir").arg(dir.join("config"));
    c.arg("--cache-dir").arg(dir.join("cache"));
    // Offline OpenRouter stub: the client is only built when a model is named.
    c.arg("--embedding-model").arg(openrouter_mock::STUB_MODEL);
    c
}

/// Isolated helper that lets database resolution fall back to `HOME`.
///
/// Uses `env_clear` so CI environment variables cannot leak into the case.
#[allow(dead_code)]
pub fn home_isolated_cmd(cwd: &std::path::Path) -> assert_cmd::Command {
    let mock_dir = mock_llm_path();
    let mut c = assert_cmd::Command::cargo_bin("sqlite-graphrag").expect("bin");
    c.env_clear();
    // PATH must carry the mock dir AFTER env_clear.
    c.env("PATH", prepend_path(&mock_dir));
    if let Ok(home_var) = std::env::var("HOME") {
        c.env("HOME", home_var);
    }
    c.current_dir(cwd);
    c.env("XDG_CACHE_HOME", cwd.join("cache"));
    c
}

// ---------------------------------------------------------------------------
// Helpers para testes de grafo (link, unlink, related, graph, cleanup-orphans)
// ---------------------------------------------------------------------------

/// Creates a memory with entities attached via entities-file to populate the graph.
#[allow(dead_code)]
pub fn seed_memory_with_entities(
    tmp: &TempDir,
    memory_name: &str,
    entities_json: &str,
) -> std::path::PathBuf {
    let entities_path = tmp.path().join(format!("entities-{memory_name}.json"));
    std::fs::write(&entities_path, entities_json).unwrap();

    cmd(tmp)
        .args([
            "remember",
            "--name",
            memory_name,
            "--type",
            "project",
            "--description",
            "seed memory for graph tests",
            "--body",
            "body",
            "--entities-file",
            entities_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    entities_path
}
