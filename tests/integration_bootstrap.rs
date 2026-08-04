//! Database-path resolution, `init`, `health`, `stats` and removed-surface guards.
//!
//! Split out of the 2 485-line `tests/integration.rs` in v1.2.5. Each theme is
//! its own binary, so a compile error in one no longer takes the other two down
//! with it, and the shared helpers moved to `tests/common/` instead of being
//! copied three times.

#[path = "common/mod.rs"]
mod common;

#[allow(unused_imports)]
use assert_cmd::Command;
#[allow(unused_imports)]
use common::{
    cmd, home_isolated_cmd, init_db, isolated_cmd_in, seed_memory_with_entities, sgr_cmd,
};
#[allow(unused_imports)]
use tempfile::TempDir;

#[test]
fn cli_home_env_creates_db_in_target_dir() {
    // GAP-SG-101 / G-T-XDG-04 (v1.2.0): with no `--db` and no XDG `db.path`,
    // `init` resolves through the XDG DATA directory, which `directories`
    // derives from HOME as `$HOME/.local/share/sqlite-graphrag`. The pre-v1.2.0
    // contract put the file at the HOME ROOT; that expectation had been failing
    // since the release. The question under test is unchanged — "does HOME, and
    // not the working directory, decide where the default database lands?" —
    // only the expected location moved one directory deeper.
    let home_dir = TempDir::new().expect("home tempdir");
    let cwd_dir = TempDir::new().expect("cwd tempdir");
    let banco_no_home = home_dir
        .path()
        .join(".local")
        .join("share")
        .join("sqlite-graphrag")
        .join("graphrag.sqlite");
    let banco_na_raiz_do_home = home_dir.path().join("graphrag.sqlite");
    let banco_no_cwd = cwd_dir.path().join("graphrag.sqlite");

    home_isolated_cmd(cwd_dir.path())
        .env("HOME", home_dir.path())
        .arg("init")
        .assert()
        .success();

    assert!(
        banco_no_home.exists(),
        "init com HOME deve criar o banco no diretório de dados XDG derivado de HOME"
    );
    assert!(
        !banco_na_raiz_do_home.exists(),
        "init NÃO deve criar banco na raiz do HOME (G-T-XDG-04)"
    );
    assert!(
        !banco_no_cwd.exists(),
        "init com HOME NÃO deve criar banco no current_dir"
    );
}

#[test]
fn cli_home_traversal_rejected() {
    let cwd_dir = TempDir::new().expect("cwd tempdir");

    home_isolated_cmd(cwd_dir.path())
        .env("HOME", "/tmp/../etc")
        .arg("init")
        .assert()
        .failure();
}

#[test]
fn cli_product_env_db_path_is_ignored_flag_wins() {
    // GAP-SG-101 / G-T-XDG-04: SQLITE_GRAPHRAG_DB_PATH is not read.
    // --db after the subcommand is the only override; product env must not
    // create a database at the env path.
    let home_dir = TempDir::new().expect("home tempdir");
    let env_dir = TempDir::new().expect("env tempdir");
    let flag_dir = TempDir::new().expect("flag tempdir");
    let cwd_dir = TempDir::new().expect("cwd tempdir");
    let db_from_env = env_dir.path().join("from-env.sqlite");
    let db_flag = flag_dir.path().join("via-flag.sqlite");
    let banco_no_home = home_dir.path().join("graphrag.sqlite");

    home_isolated_cmd(cwd_dir.path())
        .env("HOME", home_dir.path())
        // Intentionally set: must be ignored (negative assertion).
        .env("SQLITE_GRAPHRAG_DB_PATH", &db_from_env)
        .args(["init", "--db"])
        .arg(&db_flag)
        .assert()
        .success();

    assert!(db_flag.exists(), "--db must create the explicit database");
    assert!(
        !db_from_env.exists(),
        "SQLITE_GRAPHRAG_DB_PATH must be ignored (G-T-XDG-04)"
    );
    assert!(
        !banco_no_home.exists(),
        "HOME must not be used when --db is present"
    );
}

#[test]
fn cli_flag_db_overrides_home_env() {
    let home_dir = TempDir::new().expect("home tempdir");
    let flag_dir = TempDir::new().expect("flag tempdir");
    let cwd_dir = TempDir::new().expect("cwd tempdir");
    let db_flag = flag_dir.path().join("via-flag.sqlite");
    let banco_no_home = home_dir.path().join("graphrag.sqlite");

    home_isolated_cmd(cwd_dir.path())
        .env("HOME", home_dir.path())
        .args(["init", "--db", db_flag.to_str().unwrap()])
        .assert()
        .success();

    assert!(db_flag.exists(), "flag --db deve vencer HOME");
    assert!(
        !banco_no_home.exists(),
        "HOME não deve ser usado quando --db está presente"
    );
}

// ---------------------------------------------------------------------------
// init
// ---------------------------------------------------------------------------

#[test]
fn test_init_creates_sqlite_file() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("test.sqlite");
    assert!(!db_path.exists(), "banco nao deve existir antes do init");

    cmd(&tmp).arg("init").assert().success();

    assert!(db_path.exists(), "banco deve existir apos o init");
}

#[test]
fn test_init_creates_local_db_in_invocation_directory() {
    let pasta_a = TempDir::new().unwrap();
    let pasta_b = TempDir::new().unwrap();
    let banco_a = pasta_a.path().join("graphrag.sqlite");
    let banco_b = pasta_b.path().join("graphrag.sqlite");

    assert!(
        !banco_a.exists(),
        "banco local nao deve existir antes do init em a"
    );
    assert!(
        !banco_b.exists(),
        "banco local nao deve existir antes do init em b"
    );

    isolated_cmd_in(pasta_a.path())
        .arg("init")
        .assert()
        .success();
    isolated_cmd_in(pasta_b.path())
        .arg("init")
        .assert()
        .success();

    assert!(banco_a.exists(), "init deve criar graphrag.sqlite em a");
    assert!(banco_b.exists(), "init deve criar graphrag.sqlite em b");
}

#[test]
fn test_crud_uses_graphrag_sqlite_in_invocation_directory() {
    let pasta = TempDir::new().unwrap();
    let banco = pasta.path().join("graphrag.sqlite");

    assert!(
        !banco.exists(),
        "banco local nao deve existir antes do init no diretorio da invocacao"
    );

    isolated_cmd_in(pasta.path()).arg("init").assert().success();

    assert!(
        banco.exists(),
        "init deve criar graphrag.sqlite no diretorio da invocacao"
    );

    isolated_cmd_in(pasta.path())
        .args([
            "remember",
            "--name",
            "memory-cwd",
            "--type",
            "user",
            "--description",
            "crud cwd",
            "--body",
            "conteudo salvo no banco local da pasta atual",
        ])
        .assert()
        .success();

    let read_output = isolated_cmd_in(pasta.path())
        .args(["read", "--name", "memory-cwd"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let read_json: serde_json::Value = serde_json::from_slice(&read_output).unwrap();
    assert_eq!(read_json["name"], "memory-cwd");
    assert_eq!(read_json["description"], "crud cwd");

    let list_output = isolated_cmd_in(pasta.path())
        .arg("list")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: serde_json::Value = serde_json::from_slice(&list_output).unwrap();
    let itens = list_json["items"].as_array().unwrap();
    assert!(
        itens.iter().any(|item| item["name"] == "memory-cwd"),
        "list deve ler a memoria persistida em ./graphrag.sqlite"
    );

    isolated_cmd_in(pasta.path())
        .args(["forget", "--name", "memory-cwd"])
        .assert()
        .success();

    let purge_output = isolated_cmd_in(pasta.path())
        .args(["purge", "--retention-days", "0", "--yes"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let purge_json: serde_json::Value = serde_json::from_slice(&purge_output).unwrap();
    assert_eq!(purge_json["purged_count"], 1);
}

#[test]
fn test_remember_without_init_creates_migrated_local_db() {
    let pasta = TempDir::new().unwrap();
    let banco = pasta.path().join("graphrag.sqlite");

    assert!(
        !banco.exists(),
        "banco local nao deve existir antes do remember"
    );

    isolated_cmd_in(pasta.path())
        .args([
            "remember",
            "--name",
            "memory-without-init",
            "--type",
            "user",
            "--description",
            "create sem init",
            "--body",
            "conteudo salvo sem init explicito",
            "--skip-extraction",
            "--json",
        ])
        .assert()
        .success();

    assert!(
        banco.exists(),
        "remember deve criar graphrag.sqlite migrado no cwd"
    );

    let read_output = isolated_cmd_in(pasta.path())
        .args(["read", "--name", "memory-without-init", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let read_json: serde_json::Value = serde_json::from_slice(&read_output).unwrap();
    assert_eq!(read_json["name"], "memory-without-init");
    assert_eq!(read_json["body"], "conteudo salvo sem init explicito");
}

#[test]
fn test_init_returns_json_with_status_ok() {
    let tmp = TempDir::new().unwrap();
    let output = cmd(&tmp)
        .arg("init")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["status"], "ok");
    // Until v1.2.4 this asserted the CRATE VERSION, freezing the divergence
    // `init.schema.json` documented against: the schema described `model` as
    // the embedding model name while the handler filled it with
    // `SQLITE_GRAPHRAG_VERSION`. The version still reaches the database via
    // `schema_meta.sqlite-graphrag_version`; the envelope field now answers
    // what it is named after — the model this invocation would embed with,
    // resolved as `--embedding-model` > XDG `embedding.model` > `"none"`.
    assert_eq!(json["model"], common::openrouter_mock::STUB_MODEL);
    assert_ne!(json["model"], env!("CARGO_PKG_VERSION"));
    assert!(json["dim"].as_u64().unwrap() > 0);
}

// ---------------------------------------------------------------------------
// health
// ---------------------------------------------------------------------------

#[test]
fn test_health_does_not_auto_init_when_missing() {
    let tmp = TempDir::new().unwrap();
    // v1.0.x contract: `health` no longer auto-creates the database; with a
    // missing DB it guards with exit 4 and tells the operator to run `init`
    // first (it must never silently create the file).
    let assert = cmd(&tmp).arg("health").assert().failure().code(4);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        stderr.contains("does not auto-create"),
        "expected the no-auto-create guard, got stderr: {stderr}"
    );
}

#[test]
fn test_health_ok_after_init() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd(&tmp)
        .arg("health")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["integrity"], "ok");
}

// ---------------------------------------------------------------------------
// daemon — REMOVED in v1.0.79 (the CLI is 100% one-shot)
// ---------------------------------------------------------------------------

/// v1.0.79 regression guard: the `daemon` subcommand was fully removed
/// (ADR-0021; code deleted in v1.0.79). Invoking it must fail with the
/// clap unknown-subcommand error (exit 2), never start any process.
#[test]
fn test_daemon_subcommand_is_removed() {
    let tmp = TempDir::new().unwrap();

    sgr_cmd()
        .current_dir(tmp.path())
        .env("XDG_CACHE_HOME", tmp.path().join("cache"))
        .args(["daemon", "--ping", "--json"])
        .assert()
        .failure()
        .code(2);
}

/// v1.0.79 regression guard: the top-level help must not advertise the
/// removed `daemon` subcommand.
#[test]
fn test_help_does_not_list_daemon() {
    let tmp = TempDir::new().unwrap();

    let output = sgr_cmd()
        .current_dir(tmp.path())
        .env("XDG_CACHE_HOME", tmp.path().join("cache"))
        .args(["--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let help = String::from_utf8(output).unwrap();
    assert!(
        !help.contains("daemon"),
        "top-level help must not list the removed daemon subcommand"
    );
}

// ---------------------------------------------------------------------------
// namespace-detect
// ---------------------------------------------------------------------------

#[test]
fn test_namespace_detect_returns_global_without_local_config() {
    let tmp = TempDir::new().unwrap();

    let output = isolated_cmd_in(tmp.path())
        .arg("namespace-detect")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["namespace"], "global");
    assert_eq!(json["source"], "default");
}

// ---------------------------------------------------------------------------
// sync-safe-copy
// ---------------------------------------------------------------------------

#[test]
fn test_sync_safe_copy_creates_consistent_snapshot() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);
    let dest = tmp.path().join("snapshot.sqlite");

    let output = cmd(&tmp)
        .args(["sync-safe-copy", "--dest", dest.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["status"], "ok");
    assert!(dest.exists());
    assert!(std::fs::metadata(dest).unwrap().len() > 0);
}

// ---------------------------------------------------------------------------
// stats
// ---------------------------------------------------------------------------

#[test]
fn test_stats_returns_counts() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "stat-mem",
            "--type",
            "user",
            "--description",
            "desc",
            "--body",
            "corpo da stat",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .arg("stats")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["memories"].as_i64().unwrap() >= 1);
    assert!(json["db_size_bytes"].as_u64().unwrap() > 0);
    assert_eq!(json["schema_version"], 16);
}

#[test]
fn test_stats_auto_inits_when_missing() {
    let tmp = TempDir::new().unwrap();
    cmd(&tmp).arg("stats").assert().success();
}
