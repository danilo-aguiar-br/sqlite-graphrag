//! GAP-SG-92 / GAP-SG-101: proof that `common::IsolatedEnv` really isolates.
//!
//! Every other test in the suite is only as trustworthy as this file. Before
//! v1.2.0 the suite "isolated" itself with the retired product env vars
//! `SQLITE_GRAPHRAG_CACHE_DIR` and `SQLITE_GRAPHRAG_DB_PATH`, which no production
//! code reads (`src/paths.rs` documents this normatively). The `TempDir`s were
//! decoration: the binary happily wrote to the developer's real cache and real
//! database, which is why `graph_traverse_regression` failed on duplicates
//! accumulated across runs.
//!
//! These tests assert the two halves that matter:
//!   1. everything the binary creates lands under the sandbox root, and
//!   2. the real `HOME` is not touched at all.
//!
//! Half 2 is the one that was missing. A helper can redirect the sandbox
//! correctly and still leak a second write to the real directory.

#[path = "common/mod.rs"]
mod common;

use std::path::Path;

/// Recursively collects every file under `dir`, relative to it.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(dir) {
                out.push(rel.display().to_string());
            }
        }
    }
    out.sort();
    out
}

/// Snapshot of a real user directory: existence plus modification time.
///
/// Comparing mtime is what proves the suite did not write. Comparing only
/// existence would pass even if the binary appended to an existing file.
fn mtime_snapshot(paths: &[std::path::PathBuf]) -> Vec<(String, Option<std::time::SystemTime>)> {
    paths
        .iter()
        .map(|p| {
            (
                p.display().to_string(),
                std::fs::metadata(p).and_then(|m| m.modified()).ok(),
            )
        })
        .collect()
}

fn real_user_dirs() -> Vec<std::path::PathBuf> {
    let Some(proj) = directories::ProjectDirs::from("", "", "sqlite-graphrag") else {
        return Vec::new();
    };
    vec![
        proj.data_dir().to_path_buf(),
        proj.cache_dir().to_path_buf(),
        proj.config_dir().to_path_buf(),
    ]
}

#[test]
fn isolated_env_redirects_every_directory() {
    let env = common::isolated_env();

    // A write path: `init` creates the database, `config set` creates
    // config.toml, and acquiring a CLI slot creates a lock file. Together they
    // exercise the data dir, the config dir and the cache dir.
    env.cmd()
        .args(["config", "set", "log.level", "debug", "--json"])
        .assert()
        .success();

    env.sgr("init")
        .args(["--namespace", "isolation"])
        .assert()
        .success();

    let produced = files_under(env.root());

    assert!(
        produced.iter().any(|f| f.contains("config.toml")),
        "config.toml must be created inside the sandbox; found {produced:?}"
    );
    assert!(
        produced.iter().any(|f| f.contains("test.sqlite")),
        "the database must be created inside the sandbox; found {produced:?}"
    );
    assert!(
        produced.iter().any(|f| f.contains("cli-slot")),
        "CLI slot locks must be created inside the sandbox; found {produced:?}"
    );
}

#[test]
fn isolated_env_does_not_touch_the_real_home() {
    let dirs = real_user_dirs();
    if dirs.is_empty() {
        // No home directory resolvable (bare container). Nothing to protect.
        return;
    }
    let before = mtime_snapshot(&dirs);

    let env = common::isolated_env();
    env.cmd()
        .args(["config", "set", "log.level", "debug", "--json"])
        .assert()
        .success();
    env.sgr("init")
        .args(["--namespace", "isolation"])
        .assert()
        .success();

    let after = mtime_snapshot(&dirs);

    assert_eq!(
        before, after,
        "the suite must not create or modify anything under the real user \
         directories; a difference here means the isolation channel is inert \
         again (GAP-SG-101)"
    );
}

#[test]
fn cache_accessor_matches_where_the_binary_actually_writes() {
    // `ProjectDirs::cache_dir()` appends the application directory under
    // `XDG_CACHE_HOME`. A test planting lock files by hand must use
    // `IsolatedEnv::cache()`, not `root().join("cache")`. This test pins that
    // contract so the helper and the binary cannot drift apart.
    let env = common::isolated_env();
    env.cmd()
        .args(["--skip-memory-guard", "namespace-detect", "--json"])
        .assert()
        .success();

    let locks = files_under(&env.cache());
    assert!(
        locks.iter().any(|f| f.contains("cli-slot")),
        "IsolatedEnv::cache() must be the directory the binary writes locks \
         into; found {locks:?} under {}",
        env.cache().display()
    );
}

// ---------------------------------------------------------------------------
// GAP-SG-101 expanded contract: flags win, product env is inert
// ---------------------------------------------------------------------------

#[test]
fn db_flag_wins_over_planted_config_path() {
    let env = common::isolated_env();
    let other = env.root().join("other.sqlite");

    // Planted default via IsolatedEnv::sgr is env.db(); --db must override it.
    env.cmd()
        .args(["init", "--db"])
        .arg(&other)
        .assert()
        .success();

    assert!(
        other.exists(),
        "--db must create the explicit database at {}",
        other.display()
    );
    assert!(
        !env.db().exists(),
        "planted/default sandbox db must not be created when --db points elsewhere"
    );
}

#[test]
fn product_env_db_path_is_ignored() {
    let env = common::isolated_env();
    let decoy = env.root().join("decoy-from-env.sqlite");

    // Intentionally set the retired product env. Binary must ignore it.
    env.sgr("init")
        .env("SQLITE_GRAPHRAG_DB_PATH", &decoy)
        .assert()
        .success();

    assert!(
        env.db().exists(),
        "--db from IsolatedEnv::sgr must create the sandbox database"
    );
    assert!(
        !decoy.exists(),
        "SQLITE_GRAPHRAG_DB_PATH must be ignored (G-T-XDG-04)"
    );
}

#[test]
fn product_env_cache_dir_is_ignored() {
    let env = common::isolated_env();
    let decoy_cache = env.root().join("decoy-cache");

    env.cmd()
        .env("SQLITE_GRAPHRAG_CACHE_DIR", &decoy_cache)
        .args(["--skip-memory-guard", "namespace-detect", "--json"])
        .assert()
        .success();

    let locks = files_under(&env.cache());
    assert!(
        locks.iter().any(|f| f.contains("cli-slot")),
        "locks must land under --cache-dir sandbox; found {locks:?}"
    );
    assert!(
        !decoy_cache.exists() || files_under(&decoy_cache).is_empty(),
        "SQLITE_GRAPHRAG_CACHE_DIR must not receive lock files"
    );
}

#[test]
fn config_dir_isolation_writes_config_toml_only_in_sandbox() {
    let env = common::isolated_env();
    env.cmd()
        .args(["config", "set", "log.level", "warn", "--json"])
        .assert()
        .success();

    let cfg = env.config().join("config.toml");
    assert!(
        cfg.exists(),
        "config.toml must live under --config-dir ({})",
        env.config().display()
    );
    let body = std::fs::read_to_string(&cfg).expect("read config.toml");
    assert!(
        body.contains("log.level") || body.contains("warn"),
        "config.toml must contain the set key; body={body}"
    );
}

#[test]
fn cache_dir_isolation_holds_cli_slot_locks() {
    let env = common::isolated_env();
    env.sgr("init").assert().success();

    let under_cache = files_under(&env.cache());
    assert!(
        under_cache.iter().any(|f| f.contains("cli-slot")),
        "CLI slot locks must be under IsolatedEnv::cache(); found {under_cache:?}"
    );
    // Nothing under root/cache that is NOT under root/cache/sqlite-graphrag
    // is required; the accessor already points at the app dir.
    assert!(
        env.cache().ends_with("sqlite-graphrag")
            || env.cache().components().any(|c| c.as_os_str() == "cache"),
        "cache accessor must resolve inside the sandbox"
    );
}

#[test]
fn namespace_flag_is_honoured_by_namespace_detect() {
    let env = common::isolated_env();
    let out = env
        .cmd()
        .args([
            "--skip-memory-guard",
            "namespace-detect",
            "--namespace",
            "flag-ns-iso",
            "--json",
        ])
        .output()
        .expect("namespace-detect");
    assert!(out.status.success(), "namespace-detect must succeed");
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("namespace-detect JSON");
    assert_eq!(json["namespace"], "flag-ns-iso");
    assert_eq!(json["source"], "explicit_flag");
}

#[test]
fn product_env_namespace_is_ignored() {
    let env = common::isolated_env();
    let out = env
        .cmd()
        .env("SQLITE_GRAPHRAG_NAMESPACE", "env-ns-must-be-ignored")
        .args(["--skip-memory-guard", "namespace-detect", "--json"])
        .output()
        .expect("namespace-detect");
    assert!(out.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("namespace-detect JSON");
    assert_ne!(
        json["namespace"], "env-ns-must-be-ignored",
        "SQLITE_GRAPHRAG_NAMESPACE must not drive resolution"
    );
    let source = json["source"].as_str().unwrap_or("");
    assert!(
        source == "default" || source == "xdg_config",
        "source must not be environment; got {source}"
    );
}

#[test]
fn embedding_dim_flag_or_config_channel_not_product_env() {
    // Product env SQLITE_GRAPHRAG_EMBEDDING_DIM is inert. Setting it must not
    // crash init and must not redirect the database.
    let env = common::isolated_env();
    env.sgr("init")
        .env("SQLITE_GRAPHRAG_EMBEDDING_DIM", "384")
        .assert()
        .success();
    assert!(
        env.db().exists(),
        "init must still create the sandbox db when product embedding-dim env is set"
    );
}

#[test]
fn sgr_places_db_flag_after_subcommand() {
    // Contract of IsolatedEnv::sgr: `--db` comes AFTER the subcommand name.
    let env = common::isolated_env();
    // health --db <path> is the documented order; if --db were global-only
    // before the subcommand this would fail clap parsing.
    env.sgr("init").assert().success();
    env.sgr("health")
        .arg("--json")
        .assert()
        .success();
    assert!(env.db().exists());
}

#[test]
fn product_env_log_level_is_ignored() {
    // Product env SQLITE_GRAPHRAG_LOG_LEVEL is inert (G-T-XDG-04). A nonsense
    // value must not break init/list under IsolatedEnv.
    let env = common::isolated_env();
    env.sgr("init")
        .env("SQLITE_GRAPHRAG_LOG_LEVEL", "not-a-real-level")
        .assert()
        .success();
    env.sgr("list")
        .env("SQLITE_GRAPHRAG_LOG_LEVEL", "not-a-real-level")
        .arg("--json")
        .assert()
        .success();
    assert!(
        env.db().exists(),
        "init/list must succeed while product log-level env is set"
    );
}

#[test]
fn product_env_llm_backend_is_ignored_when_flag_sets_none() {
    // SQLITE_GRAPHRAG_LLM_BACKEND is retired. When the operator passes
    // `--llm-backend none`, that flag must win even if the product env
    // claims a live backend (claude/codex).
    let env = common::isolated_env();
    let out = env
        .cmd()
        .env("SQLITE_GRAPHRAG_LLM_BACKEND", "claude")
        .args([
            "--skip-memory-guard",
            "--llm-backend",
            "none",
            "--dry-run-backend",
        ])
        .output()
        .expect("dry-run-backend");
    assert!(
        out.status.success(),
        "dry-run-backend must succeed; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("dry-run-backend JSON");
    assert_eq!(
        json["backend"], "none",
        "flag --llm-backend none must win over SQLITE_GRAPHRAG_LLM_BACKEND; got {json}"
    );

    // Offline path: init with --llm-backend none still creates the sandbox DB.
    env.sgr("init")
        .env("SQLITE_GRAPHRAG_LLM_BACKEND", "claude")
        .args(["--llm-backend", "none"])
        .assert()
        .success();
    assert!(env.db().exists());
}

#[test]
fn config_set_db_path_under_isolated_xdg() {
    // `config set db.path` under IsolatedEnv's --config-dir must redirect
    // subsequent commands that omit --db (G-T-XDG-04).
    let env = common::isolated_env();
    let alt = env.root().join("db").join("via-config.sqlite");

    env.cmd()
        .args(["config", "set", "db.path"])
        .arg(alt.to_str().expect("utf8 path"))
        .arg("--json")
        .assert()
        .success();

    // No --db: init must honour XDG db.path from the isolated config dir.
    env.cmd()
        .args(["--skip-memory-guard", "init"])
        .assert()
        .success();

    assert!(
        alt.exists(),
        "init without --db must create the database at config-set db.path ({})",
        alt.display()
    );
    assert!(
        !env.db().exists(),
        "IsolatedEnv default db path must stay untouched when db.path points elsewhere"
    );

    // health/list without --db resolve the same config path.
    env.cmd()
        .args(["--skip-memory-guard", "health", "--json"])
        .assert()
        .success();
    env.cmd()
        .args(["--skip-memory-guard", "list", "--json"])
        .assert()
        .success();
}
