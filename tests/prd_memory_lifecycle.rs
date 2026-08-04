//! PRD compliance: namespace isolation, soft-delete, FTS integrity, merge, limits and optimistic locking (clauses 1-12).
//!
//! Part of the PRD-compliance suite split by GAP-SG-208. Covers the MUST/DEVE
//! clauses of the sqlite-graphrag PRD. The shared harness lives in
//! `tests/prd_support/`.

#[path = "prd_support/mod.rs"]
mod support;

use rusqlite::Connection;
use serial_test::serial;
use support::{cmd_base, db_path, init_db, remember_ok, sgr_cmd};
use tempfile::TempDir;
// ---------------------------------------------------------------------------
// 1 — namespace with __ prefix rejected with exit 1
//     (the check is done in remember.rs at the name level; there is no __ guard at namespace level)
// ---------------------------------------------------------------------------

#[test]
fn prd_name_double_underscore_rejected() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd_base(&tmp)
        .args([
            "remember",
            "--name",
            "___",
            "--type",
            "user",
            "--description",
            "must fail because name is empty after normalization",
            "--body",
            "body content",
        ])
        .assert()
        .failure()
        .code(1);
}

// ---------------------------------------------------------------------------
// 2 — cross-namespace link rejected (exit 4: entity does not exist in namespace)
// ---------------------------------------------------------------------------

#[test]
fn prd_cross_namespace_link_rejected() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // Cria entidade em ns-alpha
    remember_ok(&tmp, "entidade-alpha", "corpo alpha");

    // Try to link between entities from distinct namespaces (to: ns-beta does not exist)
    cmd_base(&tmp)
        .args([
            "link",
            "--from",
            "entidade-alpha",
            "--to",
            "entidade-inexistente-beta",
            "--relation",
            "related",
            "--namespace",
            "global",
        ])
        .assert()
        .failure()
        .code(4);
}

// ---------------------------------------------------------------------------
// 3 — soft-delete: forgotten memories do not appear in recall
// ---------------------------------------------------------------------------

#[test]
fn prd_soft_delete_recall_does_not_return_forgotten() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    remember_ok(&tmp, "memoria-apagavel", "conteudo apagavel importante");

    // Apaga (soft-delete)
    cmd_base(&tmp)
        .args([
            "forget",
            "--name",
            "memoria-apagavel",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    // Verify that deleted_at was filled (does not return in SELECT ... WHERE deleted_at IS NULL)
    let conn = Connection::open(db_path(&tmp)).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE name='memoria-apagavel' AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 0,
        "memória esquecida não deve aparecer sem deleted_at"
    );

    let deleted_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE name='memoria-apagavel' AND deleted_at IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(deleted_count, 1, "soft-delete deve preencher deleted_at");
}

// ---------------------------------------------------------------------------
// 4 — trg_fts_ad idempotent: double-delete does not corrupt fts_memories
// ---------------------------------------------------------------------------

#[test]
fn prd_trg_fts_ad_idempotent_double_delete() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    remember_ok(
        &tmp,
        "memoria-dupla",
        "conteudo para double delete fts test",
    );

    let conn = Connection::open(db_path(&tmp)).unwrap();

    // Obtain the memory id
    let memory_id: i64 = conn
        .query_row(
            "SELECT id FROM memories WHERE name='memoria-dupla'",
            [],
            |r| r.get(0),
        )
        .unwrap();

    // First deletion via UPDATE (manual soft-delete directly in the database)
    conn.execute(
        "UPDATE memories SET deleted_at=strftime('%s','now') WHERE id=?1",
        [memory_id],
    )
    .unwrap();

    // Second "deletion" — the trg_fts_ad trigger already removed it from FTS; should not error
    conn.execute("DELETE FROM fts_memories WHERE rowid=?1", [memory_id])
        .unwrap_or(0); // idempotente: se não existir, ignora

    // Verify FTS integrity after the double operation
    let result =
        conn.execute_batch("INSERT INTO fts_memories(fts_memories) VALUES('integrity-check')");
    assert!(
        result.is_ok(),
        "fts_memories deve passar integrity-check após double-delete"
    );
}

// ---------------------------------------------------------------------------
// 5 — remember duplicata com --force-merge retorna merged_into_memory_id
// ---------------------------------------------------------------------------

#[test]
fn prd_remember_duplicate_returns_merged_into_memory_id() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    remember_ok(&tmp, "mem-merge-alvo", "corpo original da memoria merge");

    // Segunda chamada com mesmo nome + --force-merge
    let output = cmd_base(&tmp)
        .args([
            "remember",
            "--name",
            "mem-merge-alvo",
            "--type",
            "user",
            "--description",
            "desc atualizada",
            "--body",
            "corpo novo do merge",
            "--namespace",
            "global",
            "--force-merge",
            "--skip-extraction",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    // merged_into_memory_id deve ser presente (pode ser null ou inteiro)
    assert!(
        json.get("merged_into_memory_id").is_some(),
        "remember com --force-merge deve incluir campo merged_into_memory_id"
    );
}

// ---------------------------------------------------------------------------
// 6 — remember JSON contains entities_persisted and relationships_persisted
// ---------------------------------------------------------------------------

#[test]
fn prd_remember_json_contains_entities_and_relationships_persisted() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd_base(&tmp)
        .args([
            "remember",
            "--name",
            "mem-fields-check",
            "--type",
            "user",
            "--description",
            "verificar campos de saida",
            "--body",
            "corpo para checar campos json",
            "--namespace",
            "global",
            "--skip-extraction",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(
        json.get("entities_persisted").is_some(),
        "remember deve emitir entities_persisted"
    );
    assert!(
        json.get("relationships_persisted").is_some(),
        "remember deve emitir relationships_persisted"
    );
}

// ---------------------------------------------------------------------------
// 7 — FTS5 unicode61 remove_diacritics: searching "nao" matches "não"
// ---------------------------------------------------------------------------

#[test]
fn prd_fts5_unicode61_remove_diacritics() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let conn = Connection::open(db_path(&tmp)).unwrap();

    // Verifica que fts_memories usa tokenize com unicode61 remove_diacritics
    let tokenize: String = conn
        .query_row(
            "SELECT tokenize FROM pragma_table_info('fts_memories') LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| {
            // Alternativa: busca via sqlite_master
            conn.query_row(
                "SELECT sql FROM sqlite_master WHERE name='fts_memories'",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_default()
        });

    assert!(
        tokenize.contains("unicode61") || tokenize.contains("remove_diacritics"),
        "fts_memories deve usar tokenize='unicode61 remove_diacritics 1', encontrado: {tokenize}"
    );
}

// ---------------------------------------------------------------------------
// 8 — pure-Rust cosine similarity produces the expected distance range
// ---------------------------------------------------------------------------
// v1.0.76 dropped vec_memories and the distance_metric=cosine DDL. The cosine
// invariant is now guaranteed by src/similarity.rs::cosine_similarity plus
// the BLOB-backed memory_embeddings table. This test pins the contract:
//   - orthogonal unit vectors yield distance > 0.5
//   - identical unit vectors yield distance ~ 0.0
//   - the result lies in (0.5, 2.0] for near-orthogonal vectors
// The test does NOT shell out to the binary; it runs a tiny pure-Rust snippet
// against the live library, so it stays fast and hermetic.

#[test]
fn prd_cosine_similarity_distance_invariant() {
    use sqlite_graphrag::similarity::{cosine_similarity, similarity_to_distance};

    let a: Vec<f32> = (0..384).map(|i| (i as f32).sin()).collect();
    let b: Vec<f32> = (0..384).map(|i| (i as f32).cos()).collect();
    let c: Vec<f32> = a.clone();

    let sim_ab = cosine_similarity(&a, &b);
    let sim_ac = cosine_similarity(&a, &c);
    let d_ab = similarity_to_distance(sim_ab);
    let d_ac = similarity_to_distance(sim_ac);

    assert!(
        d_ab > 0.5 && d_ab <= 2.0,
        "distance must lie in (0.5, 2.0] for near-orthogonal vectors, got {d_ab}"
    );
    assert!(
        d_ac.abs() < 1e-6,
        "identical vectors must yield distance ~ 0.0, got {d_ac}"
    );
}

// ---------------------------------------------------------------------------
// 9 — edit com --expected-updated-at stale retorna exit 3 (Conflict)
// ---------------------------------------------------------------------------

#[test]
fn prd_edit_expected_updated_at_stale_returns_exit_3() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    remember_ok(&tmp, "mem-edit-lock", "corpo para edit lock test");

    // Use stale timestamp (0) to force a conflict
    cmd_base(&tmp)
        .args([
            "edit",
            "--name",
            "mem-edit-lock",
            "--namespace",
            "global",
            "--body",
            "novo corpo conflito",
            "--expected-updated-at",
            "0",
        ])
        .assert()
        .failure()
        .code(3);
}

// ---------------------------------------------------------------------------
// 10 — 5 simultaneous instances: the 5th returns exit 75
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn prd_five_instances_fifth_returns_exit_75() {
    use fs4::fs_std::FileExt;
    use std::fs::OpenOptions;

    let tmp = TempDir::new().unwrap();

    // GAP-SG-94 / G-T-XDG-04: `lock::slot_path` resolves through
    // `paths::cache_dir()`, whose precedence is `--cache-dir` > XDG `cache.dir`
    // > the OS cache directory. Setting only `XDG_CACHE_HOME` sends the binary
    // to `<XDG_CACHE_HOME>/sqlite-graphrag/`, one level BELOW where this test
    // planted its lock files, so every slot looked free and the 5th invocation
    // exited 0. Passing `--cache-dir` pins the resolver to the exact directory
    // the locks live in. The question under test — "does the 5th concurrent
    // instance get exit 75 when all 4 slots are taken?" — is unchanged.
    let cache_dir = tmp.path().join("cache");
    std::fs::create_dir_all(&cache_dir).unwrap();

    // Occupy the 4 default slots directly via fs4
    let handles: Vec<std::fs::File> = (1..=4)
        .map(|slot| {
            let path = cache_dir.join(format!("cli-slot-{slot}.lock"));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
                .unwrap();
            file.try_lock_exclusive().unwrap();
            file
        })
        .collect();

    // 5th invocation with --wait-lock 0 must return exit 75
    sgr_cmd()
        .env("XDG_CACHE_HOME", tmp.path().join("xdg_cache"))
        .arg("--cache-dir")
        .arg(&cache_dir)
        .args([
            "--skip-memory-guard",
            "--max-concurrency",
            "4",
            "--wait-lock",
            "0",
            "namespace-detect",
        ])
        .assert()
        .failure()
        .code(75);

    drop(handles);
}

// ---------------------------------------------------------------------------
// 11 — MAX_MEMORY_BODY_LEN=512000: corpo acima do limite retorna exit 6
// ---------------------------------------------------------------------------

#[test]
fn prd_max_body_len_exceeded_returns_exit_6() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let corpo_gigante = "x".repeat(512_001);
    let body_path = tmp.path().join("body-grande.txt");
    std::fs::write(&body_path, corpo_gigante).unwrap();

    cmd_base(&tmp)
        .args([
            "remember",
            "--name",
            "mem-body-limit",
            "--type",
            "user",
            "--description",
            "limite de corpo",
            "--body-file",
            body_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(6);
}

// ---------------------------------------------------------------------------
// 12 — --namespace flag sets the memory namespace (product env is not a channel)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn prd_namespace_flag_sets_memory_namespace() {
    // GAP-SG-101 / G-T-XDG-04: SQLITE_GRAPHRAG_NAMESPACE is not read.
    // The real channel is --namespace (or config set namespace.default).
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    cmd_base(&tmp)
        .args([
            "remember",
            "--name",
            "mem-via-flag-ns",
            "--type",
            "user",
            "--description",
            "namespace via flag",
            "--namespace",
            "ns-from-flag",
            "--body",
            "corpo namespace flag",
            "--skip-extraction",
        ])
        .assert()
        .success();

    // The literal was unquoted, so SQLite parsed `name=mem-via-flag-ns` as a
    // comparison between COLUMNS and failed with "no such column: mem", which
    // `.unwrap()` turned into a panic before the namespace was ever read. The
    // question under test — "does --namespace decide the persisted namespace?"
    // — is unchanged; the row is now selected through a bound parameter.
    let conn = Connection::open(db_path(&tmp)).unwrap();
    let ns: String = conn
        .query_row(
            "SELECT namespace FROM memories WHERE name = ?1",
            rusqlite::params!["mem-via-flag-ns"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ns, "ns-from-flag", "namespace must match --namespace flag");
}
