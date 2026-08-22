#![cfg(feature = "slow-tests")]

//! Suite 10 — smoke tests against the INSTALLED binary: entity graph, introspection and default-db fallback (#20–#26)
//!
//! Part of the smoke suite split by GAP-SG-210: the single file held 981 lines
//! and 26 tests, past the 800-line ceiling this project sets for itself. The
//! shared harness lives in `tests/smoke_support/`, which also documents why
//! this suite targets `~/.cargo/bin/sqlite-graphrag` instead of the build
//! output, and how it skips when nothing is installed.

#[path = "smoke_support/mod.rs"]
mod support;

use support::{
    allow_installed_version_mismatch, assert_json_or_not_found, assert_json_stdout, stderr, Env,
};

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #20: link
// ---------------------------------------------------------------------------

#[test]
#[ignore = "needs `cargo install --path .` first: validates the INSTALLED binary, not this build"]
fn smoke_20_link() {
    let env = Env::new();
    env.init();
    // Link operates on graph entities, not on memory names.
    // Create a memory with entities via --entities-file (entity_type field is required).
    let (ent_a, ent_b) = env.remember_with_entities(
        "smoke-link",
        "memória com entidades para smoke test de link",
    );
    let out = env
        .cmd()
        .args([
            "link",
            "--from",
            &ent_a,
            "--to",
            &ent_b,
            "--relation",
            "related",
        ])
        .output()
        .expect("link failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["action"], "created",
        "link deve retornar action=created: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #21: unlink
// ---------------------------------------------------------------------------

#[test]
#[ignore = "needs `cargo install --path .` first: validates the INSTALLED binary, not this build"]
fn smoke_21_unlink() {
    let env = Env::new();
    env.init();
    // Create entities, link them, then undo
    let (ent_a, ent_b) = env.remember_with_entities(
        "smoke-unlink",
        "memória com entidades para smoke test de unlink",
    );
    // Link first
    env.cmd()
        .args([
            "link",
            "--from",
            &ent_a,
            "--to",
            &ent_b,
            "--relation",
            "related",
        ])
        .output()
        .unwrap();
    // Undo the link
    let out = env
        .cmd()
        .args([
            "unlink",
            "--from",
            &ent_a,
            "--to",
            &ent_b,
            "--relation",
            "related",
        ])
        .output()
        .expect("unlink failed");
    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["action"], "deleted",
        "unlink deve retornar action=deleted: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #22: related
// ---------------------------------------------------------------------------

#[test]
#[ignore = "needs `cargo install --path .` first: validates the INSTALLED binary, not this build"]
fn smoke_22_related() {
    let env = Env::new();
    env.init();
    env.remember("smoke-related-01", "conteúdo para busca de relacionados");
    let out = env
        .cmd()
        .args(["related", "smoke-related-01"])
        .output()
        .expect("related failed");
    // Accept 0 (related found) or 4 (none related)
    assert_json_or_not_found(&out);
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #23: graph
// ---------------------------------------------------------------------------

#[test]
#[ignore = "needs `cargo install --path .` first: validates the INSTALLED binary, not this build"]
fn smoke_23_graph() {
    let env = Env::new();
    env.init();
    let out = env
        .cmd()
        .args(["graph", "--format", "json"])
        .output()
        .expect("graph failed");
    assert_json_stdout(&out);
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #24: cleanup-orphans
// ---------------------------------------------------------------------------

#[test]
#[ignore = "needs `cargo install --path .` first: validates the INSTALLED binary, not this build"]
fn smoke_24_cleanup_orphans() {
    let env = Env::new();
    env.init();
    let out = env
        .cmd()
        .arg("cleanup-orphans")
        .output()
        .expect("cleanup-orphans failed");
    assert_json_stdout(&out);
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #25: debug-schema
//
// Some legacy binaries expose `__debug_schema` instead of `debug-schema`.
// When the suite is running deliberately against an old binary,
// this test skips without failing.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "needs `cargo install --path .` first: validates the INSTALLED binary, not this build"]
fn smoke_25_debug_schema() {
    let env = Env::new();
    env.init();

    let out = env
        .cmd()
        .arg("debug-schema")
        .output()
        .expect("debug-schema failed");

    if !out.status.success() {
        let err = stderr(&out);
        if err.contains("similar subcommand exists: '__debug_schema'")
            || err.contains("a similar subcommand exists: '__debug_schema'")
        {
            let legacy = env
                .cmd()
                .arg("__debug_schema")
                .output()
                .expect("__debug_schema fallback failed");
            if legacy.status.success() {
                assert_json_stdout(&legacy);
                let json: serde_json::Value = serde_json::from_slice(&legacy.stdout).unwrap();
                assert!(
                    json["objects"].is_array() || json["migrations"].is_array(),
                    "debug-schema deve retornar informações de schema: {json}"
                );
                return;
            }
        }

        if allow_installed_version_mismatch()
            && (err.contains("unrecognized subcommand")
                || err.contains("unexpected argument")
                || err.contains("unknown subcommand")
                || err.contains("similar subcommand exists"))
        {
            eprintln!(
                "Suite 10 smoke_25: installed legacy binary does not expose debug-schema — skip graceful"
            );
            return;
        }

        panic!("debug-schema failed: {err}");
    }

    assert_json_stdout(&out);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        json["objects"].is_array() || json["migrations"].is_array(),
        "debug-schema deve retornar informações de schema: {json}"
    );
}

// ---------------------------------------------------------------------------
// Suite 10 — Smoke #26: default database contract in the current directory
// ---------------------------------------------------------------------------

#[test]
#[ignore = "needs `cargo install --path .` first: validates the INSTALLED binary, not this build"]
fn smoke_26_default_db_in_current_dir() {
    let env = Env::new();
    // GAP-SG-101 / G-T-XDG-04 (v1.2.0): the default database is resolved
    // through XDG, never through the process working directory. Product env
    // bindings were removed and resolution became `--db` > XDG `db.path` >
    // XDG data dir. This suite asserted the pre-v1.2.0 contract — a database
    // materialising in the cwd — and had been failing since that release.
    // The contract under test is still "what does `init` do with no `--db`";
    // only the expected location changed.
    let db_path = env
        .tmp
        .path()
        .join("data")
        .join("sqlite-graphrag")
        .join("graphrag.sqlite");

    assert!(
        !db_path.exists(),
        "smoke_26: banco default nao deve existir antes do init"
    );

    let init_out = env
        .cmd_default_db_in_tmp_dir()
        .arg("init")
        .output()
        .expect("init cwd failed");
    assert_json_stdout(&init_out);
    let init_json: serde_json::Value = serde_json::from_slice(&init_out.stdout).unwrap();

    assert!(
        db_path.exists(),
        "smoke_26: init deve criar graphrag.sqlite no diretorio de dados XDG"
    );
    // macOS: TempDir returns /var/... while the binary canonicalizes
    // to /private/var/...; canonicalizing both sides avoids the false
    // symlink negative.
    let reported = std::path::PathBuf::from(init_json["db_path"].as_str().unwrap());
    assert_eq!(
        reported
            .canonicalize()
            .expect("canonicalize reported db_path"),
        db_path
            .canonicalize()
            .expect("canonicalize expected db_path"),
        "smoke_26: init deve reportar o path default resolvido por XDG"
    );

    let remember_out = env
        .cmd_default_db_in_tmp_dir()
        .args([
            "remember",
            "--name",
            "smoke-cwd-default",
            "--type",
            "user",
            "--description",
            "smoke cwd default",
            "--body",
            "memoria persistida no banco default do diretorio atual",
        ])
        .output()
        .expect("remember cwd failed");
    assert_json_stdout(&remember_out);

    let read_out = env
        .cmd_default_db_in_tmp_dir()
        .args(["read", "--name", "smoke-cwd-default"])
        .output()
        .expect("read cwd failed");
    assert_json_stdout(&read_out);
    let read_json: serde_json::Value = serde_json::from_slice(&read_out.stdout).unwrap();
    assert_eq!(
        read_json["name"], "smoke-cwd-default",
        "smoke_26: read deve enxergar memoria salva no banco default"
    );

    let list_out = env
        .cmd_default_db_in_tmp_dir()
        .args(["list", "--limit", "10"])
        .output()
        .expect("list cwd failed");
    assert_json_stdout(&list_out);
    let list_json: serde_json::Value = serde_json::from_slice(&list_out.stdout).unwrap();
    let items = list_json["items"]
        .as_array()
        .expect("smoke_26: list deve retornar objeto com campo items");
    assert!(
        items.iter().any(|item| item["name"] == "smoke-cwd-default"),
        "smoke_26: list deve enxergar memoria salva em ./graphrag.sqlite"
    );
}
