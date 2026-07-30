use super::*;
use rusqlite::Connection;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn setup_conn() -> Result<Connection, Box<dyn std::error::Error>> {
    crate::storage::connection::register_vec_extension();
    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA temp_store = MEMORY;",
    )?;
    crate::migrations::runner().run(&mut conn)?;
    Ok(conn)
}

fn new_memory(name: &str) -> NewMemory {
    NewMemory {
        namespace: "global".to_string(),
        name: name.to_string(),
        memory_type: "user".to_string(),
        description: "descricao de teste".to_string(),
        body: "test memory body".to_string(),
        body_hash: format!("hash-{name}"),
        session_id: None,
        source: "agent".to_string(),
        metadata: serde_json::json!({}),
    }
}

#[test]
fn insert_and_find_by_name_return_id() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-alpha");
    let id = insert(&conn, &m)?;
    assert!(id > 0);

    let found = find_by_name(&conn, "global", "mem-alpha")?;
    assert!(found.is_some());
    let (found_id, _, _) = found.ok_or("mem-alpha should exist")?;
    assert_eq!(found_id, id);
    Ok(())
}

#[test]
fn find_by_name_returns_none_when_not_found() -> TestResult {
    let conn = setup_conn()?;
    let result = find_by_name(&conn, "global", "inexistente")?;
    assert!(result.is_none());
    Ok(())
}

#[test]
fn find_by_hash_returns_correct_id() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-hash");
    let id = insert(&conn, &m)?;

    let found = find_by_hash(&conn, "global", "hash-mem-hash")?;
    assert_eq!(found, Some(id));
    Ok(())
}

#[test]
fn find_by_hash_returns_none_when_hash_not_found() -> TestResult {
    let conn = setup_conn()?;
    let result = find_by_hash(&conn, "global", "hash-inexistente")?;
    assert!(result.is_none());
    Ok(())
}

#[test]
fn find_by_hash_ignores_different_namespace() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-ns");
    insert(&conn, &m)?;

    let result = find_by_hash(&conn, "outro-namespace", "hash-mem-ns")?;
    assert!(result.is_none());
    Ok(())
}

#[test]
fn read_by_name_returns_full_memory() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-read");
    let id = insert(&conn, &m)?;

    let row = read_by_name(&conn, "global", "mem-read")?.ok_or("mem-read should exist")?;
    assert_eq!(row.id, id);
    assert_eq!(row.name, "mem-read");
    assert_eq!(row.memory_type, "user");
    assert_eq!(row.body, "test memory body");
    assert_eq!(row.namespace, "global");
    Ok(())
}

#[test]
fn read_by_name_returns_none_for_missing() -> TestResult {
    let conn = setup_conn()?;
    let result = read_by_name(&conn, "global", "nao-existe")?;
    assert!(result.is_none());
    Ok(())
}

#[test]
fn read_full_by_id_returns_memory() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-full");
    let id = insert(&conn, &m)?;

    let row = read_full(&conn, id)?.ok_or("mem-full should exist")?;
    assert_eq!(row.id, id);
    assert_eq!(row.name, "mem-full");
    Ok(())
}

#[test]
fn read_full_returns_none_for_missing_id() -> TestResult {
    let conn = setup_conn()?;
    let result = read_full(&conn, 9999)?;
    assert!(result.is_none());
    Ok(())
}

#[test]
fn update_without_optimism_modifies_fields() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-upd");
    let id = insert(&conn, &m)?;

    let mut m2 = new_memory("mem-upd");
    m2.body = "updated body".to_string();
    m2.body_hash = "hash-novo".to_string();
    let ok = update(&conn, id, &m2, None)?;
    assert!(ok);

    let row = read_full(&conn, id)?.ok_or("mem-upd should exist")?;
    assert_eq!(row.body, "updated body");
    assert_eq!(row.body_hash, "hash-novo");
    Ok(())
}

#[test]
fn update_with_correct_expected_updated_at_succeeds() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-opt");
    let id = insert(&conn, &m)?;

    let (_, updated_at, _) =
        find_by_name(&conn, "global", "mem-opt")?.ok_or("mem-opt should exist")?;

    let mut m2 = new_memory("mem-opt");
    m2.body = "optimistic body".to_string();
    m2.body_hash = "hash-optimistic".to_string();
    let ok = update(&conn, id, &m2, Some(updated_at))?;
    assert!(ok);

    let row = read_full(&conn, id)?.ok_or("mem-opt should exist after update")?;
    assert_eq!(row.body, "optimistic body");
    Ok(())
}

#[test]
fn update_with_wrong_expected_updated_at_returns_false() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-conflict");
    let id = insert(&conn, &m)?;

    let mut m2 = new_memory("mem-conflict");
    m2.body = "must not appear".to_string();
    m2.body_hash = "hash-x".to_string();
    let ok = update(&conn, id, &m2, Some(0))?;
    assert!(!ok);

    let row = read_full(&conn, id)?.ok_or("mem-conflict should exist")?;
    assert_eq!(row.body, "test memory body");
    Ok(())
}

#[test]
fn update_missing_id_returns_false() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("fantasma");
    let ok = update(&conn, 9999, &m, None)?;
    assert!(!ok);
    Ok(())
}

#[test]
fn soft_delete_marks_deleted_at() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-del");
    insert(&conn, &m)?;

    let ok = soft_delete(&conn, "global", "mem-del")?;
    assert!(ok);

    let result = find_by_name(&conn, "global", "mem-del")?;
    assert!(result.is_none());

    let result_read = read_by_name(&conn, "global", "mem-del")?;
    assert!(result_read.is_none());
    Ok(())
}

#[test]
fn soft_delete_returns_false_when_not_found() -> TestResult {
    let conn = setup_conn()?;
    let ok = soft_delete(&conn, "global", "nao-existe")?;
    assert!(!ok);
    Ok(())
}

#[test]
fn double_soft_delete_returns_false_on_second_call() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-del2");
    insert(&conn, &m)?;

    soft_delete(&conn, "global", "mem-del2")?;
    let ok = soft_delete(&conn, "global", "mem-del2")?;
    assert!(!ok);
    Ok(())
}

#[test]
fn list_returns_memories_from_namespace() -> TestResult {
    let conn = setup_conn()?;
    insert(&conn, &new_memory("mem-list-a"))?;
    insert(&conn, &new_memory("mem-list-b"))?;

    let rows = list(&conn, "global", None, 10, 0, false)?;
    assert!(rows.len() >= 2);
    let nomes: Vec<_> = rows.iter().map(|r| r.name.as_str()).collect();
    assert!(nomes.contains(&"mem-list-a"));
    assert!(nomes.contains(&"mem-list-b"));
    Ok(())
}

#[test]
fn list_with_type_filter_returns_only_correct_type() -> TestResult {
    let conn = setup_conn()?;
    insert(&conn, &new_memory("mem-user"))?;

    let mut m2 = new_memory("mem-feedback");
    m2.memory_type = "feedback".to_string();
    insert(&conn, &m2)?;

    let rows_user = list(&conn, "global", Some("user"), 10, 0, false)?;
    assert!(rows_user.iter().all(|r| r.memory_type == "user"));

    let rows_fb = list(&conn, "global", Some("feedback"), 10, 0, false)?;
    assert!(rows_fb.iter().all(|r| r.memory_type == "feedback"));
    Ok(())
}

#[test]
fn list_exclui_soft_deleted() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-excluida");
    insert(&conn, &m)?;
    soft_delete(&conn, "global", "mem-excluida")?;

    let rows = list(&conn, "global", None, 10, 0, false)?;
    assert!(rows.iter().all(|r| r.name != "mem-excluida"));
    Ok(())
}

#[test]
fn list_pagination_works() -> TestResult {
    let conn = setup_conn()?;
    for i in 0..5 {
        insert(&conn, &new_memory(&format!("mem-pag-{i}")))?;
    }

    let pagina1 = list(&conn, "global", None, 2, 0, false)?;
    let pagina2 = list(&conn, "global", None, 2, 2, false)?;
    assert!(pagina1.len() <= 2);
    assert!(pagina2.len() <= 2);
    if !pagina1.is_empty() && !pagina2.is_empty() {
        assert_ne!(pagina1[0].id, pagina2[0].id);
    }
    Ok(())
}

#[test]
#[serial_test::serial(env)]
fn upsert_vec_and_delete_vec_work() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-vec");
    let id = insert(&conn, &m)?;

    let embedding: Vec<f32> = vec![0.1; crate::constants::embedding_dim()];
    upsert_vec(
        &conn, id, "global", "user", &embedding, "mem-vec", "snippet",
    )?;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    assert_eq!(count, 1);

    delete_vec(&conn, id)?;

    let count_after: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    assert_eq!(count_after, 0);
    Ok(())
}

#[test]
#[serial_test::serial(env)]
fn upsert_vec_replaces_existing_vector() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-vec-upsert");
    let id = insert(&conn, &m)?;

    let emb1: Vec<f32> = vec![0.1; crate::constants::embedding_dim()];
    upsert_vec(&conn, id, "global", "user", &emb1, "mem-vec-upsert", "s1")?;

    let emb2: Vec<f32> = vec![0.9; crate::constants::embedding_dim()];
    upsert_vec(&conn, id, "global", "user", &emb2, "mem-vec-upsert", "s2")?;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    assert_eq!(count, 1);
    Ok(())
}

// v1.1.1 (P1): an empty embedding must NOT create a vector row, so the
// memory stays visible to `enrich re-embed`.
#[test]
fn upsert_vec_empty_embedding_skips_row() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-vec-vazia");
    let id = insert(&conn, &m)?;

    upsert_vec(&conn, id, "global", "user", &[], "mem-vec-vazia", "s")?;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    assert_eq!(count, 0, "empty embedding must not persist a row");
    Ok(())
}

#[test]
#[serial_test::serial(env)]
fn knn_search_returns_results_by_distance() -> TestResult {
    let conn = setup_conn()?;

    // emb_a: predominantemente positivo — cosseno alto com a query toda-uns
    let ma = new_memory("mem-knn-a");
    let id_a = insert(&conn, &ma)?;
    let emb_a: Vec<f32> = vec![1.0; crate::constants::embedding_dim()];
    upsert_vec(&conn, id_a, "global", "user", &emb_a, "mem-knn-a", "s")?;

    // emb_b: predominantemente negativo — cosseno baixo com a query toda-uns
    let mb = new_memory("mem-knn-b");
    let id_b = insert(&conn, &mb)?;
    let emb_b: Vec<f32> = vec![-1.0; crate::constants::embedding_dim()];
    upsert_vec(&conn, id_b, "global", "user", &emb_b, "mem-knn-b", "s")?;

    let query: Vec<f32> = vec![1.0; crate::constants::embedding_dim()];
    let results = knn_search(&conn, &query, &["global".to_string()], None, 2)?;
    assert!(!results.is_empty());
    assert_eq!(results[0].0, id_a);
    Ok(())
}

#[test]
#[serial_test::serial(env)]
fn knn_search_with_type_filter_restricts_result() -> TestResult {
    let conn = setup_conn()?;

    let ma = new_memory("mem-knn-tipo-user");
    let id_a = insert(&conn, &ma)?;
    let emb: Vec<f32> = vec![1.0; crate::constants::embedding_dim()];
    upsert_vec(
        &conn,
        id_a,
        "global",
        "user",
        &emb,
        "mem-knn-tipo-user",
        "s",
    )?;

    let mut mb = new_memory("mem-knn-tipo-fb");
    mb.memory_type = "feedback".to_string();
    let id_b = insert(&conn, &mb)?;
    upsert_vec(
        &conn,
        id_b,
        "global",
        "feedback",
        &emb,
        "mem-knn-tipo-fb",
        "s",
    )?;

    let query: Vec<f32> = vec![1.0; crate::constants::embedding_dim()];
    let results_user = knn_search(&conn, &query, &["global".to_string()], Some("user"), 5)?;
    assert!(results_user.iter().all(|(id, _)| *id == id_a));

    let results_fb = knn_search(&conn, &query, &["global".to_string()], Some("feedback"), 5)?;
    assert!(results_fb.iter().all(|(id, _)| *id == id_b));
    Ok(())
}

#[test]
fn fts_search_finds_by_prefix_in_body() -> TestResult {
    let conn = setup_conn()?;
    let mut m = new_memory("mem-fts");
    m.body = "linguagem de programacao rust".to_string();
    insert(&conn, &m)?;

    conn.execute_batch(
        "INSERT INTO fts_memories(rowid, name, description, body)
         SELECT id, name, description, body FROM memories WHERE deleted_at IS NULL",
    )?;

    let rows = fts_search(&conn, "programacao", "global", None, 10)?;
    assert!(!rows.is_empty());
    assert!(rows.iter().any(|r| r.name == "mem-fts"));
    Ok(())
}

#[test]
fn fts_search_with_type_filter() -> TestResult {
    let conn = setup_conn()?;
    let mut m = new_memory("mem-fts-tipo");
    m.body = "linguagem especial para filtro".to_string();
    insert(&conn, &m)?;

    let mut m2 = new_memory("mem-fts-feedback");
    m2.memory_type = "feedback".to_string();
    m2.body = "linguagem especial para filtro".to_string();
    insert(&conn, &m2)?;

    conn.execute_batch(
        "INSERT INTO fts_memories(rowid, name, description, body)
         SELECT id, name, description, body FROM memories WHERE deleted_at IS NULL",
    )?;

    let rows_user = fts_search(&conn, "especial", "global", Some("user"), 10)?;
    assert!(rows_user.iter().all(|r| r.memory_type == "user"));

    let rows_fb = fts_search(&conn, "especial", "global", Some("feedback"), 10)?;
    assert!(rows_fb.iter().all(|r| r.memory_type == "feedback"));
    Ok(())
}

#[test]
fn fts_search_excludes_deleted() -> TestResult {
    let conn = setup_conn()?;
    let mut m = new_memory("mem-fts-del");
    m.body = "deleted fts content".to_string();
    insert(&conn, &m)?;

    conn.execute_batch(
        "INSERT INTO fts_memories(rowid, name, description, body)
         SELECT id, name, description, body FROM memories WHERE deleted_at IS NULL",
    )?;

    soft_delete(&conn, "global", "mem-fts-del")?;

    let rows = fts_search(&conn, "deleted", "global", None, 10)?;
    assert!(rows.iter().all(|r| r.name != "mem-fts-del"));
    Ok(())
}

#[test]
fn list_deleted_before_returns_correct_ids() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-purge");
    insert(&conn, &m)?;
    soft_delete(&conn, "global", "mem-purge")?;

    let ids = list_deleted_before(&conn, "global", i64::MAX)?;
    assert!(!ids.is_empty());

    let ids_antes = list_deleted_before(&conn, "global", 0)?;
    assert!(ids_antes.is_empty());
    Ok(())
}

#[test]
fn find_by_name_returns_correct_max_version() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-ver");
    let id = insert(&conn, &m)?;

    let (_, _, v0) = find_by_name(&conn, "global", "mem-ver")?.ok_or("mem-ver should exist")?;
    assert_eq!(v0, 0);

    conn.execute(
        "INSERT INTO memory_versions (memory_id, version, name, type, description, body, metadata, change_reason)
         VALUES (?1, 1, 'mem-ver', 'user', 'desc', 'body', '{}', 'create')",
        params![id],
    )?;

    let (_, _, v1) =
        find_by_name(&conn, "global", "mem-ver")?.ok_or("mem-ver should exist after insert")?;
    assert_eq!(v1, 1);
    Ok(())
}

#[test]
fn insert_com_metadata_json() -> TestResult {
    let conn = setup_conn()?;
    let mut m = new_memory("mem-meta");
    m.metadata = serde_json::json!({"chave": "valor", "numero": 42});
    let id = insert(&conn, &m)?;

    let row = read_full(&conn, id)?.ok_or("mem-meta should exist")?;
    let meta: serde_json::Value = serde_json::from_str(&row.metadata)?;
    assert_eq!(meta["chave"], "valor");
    assert_eq!(meta["numero"], 42);
    Ok(())
}

#[test]
fn insert_com_session_id() -> TestResult {
    let conn = setup_conn()?;
    let mut m = new_memory("mem-session");
    m.session_id = Some("sessao-xyz".to_string());
    let id = insert(&conn, &m)?;

    let row = read_full(&conn, id)?.ok_or("mem-session should exist")?;
    assert_eq!(row.session_id, Some("sessao-xyz".to_string()));
    Ok(())
}

#[test]
fn delete_vec_for_nonexistent_id_does_not_fail() -> TestResult {
    let conn = setup_conn()?;
    let result = delete_vec(&conn, 99999);
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn preprocess_fts_query_no_separators() {
    assert_eq!(preprocess_fts_query("hello"), "hello*");
    assert_eq!(preprocess_fts_query("hello world"), "hello* world*");
}

#[test]
fn preprocess_fts_query_with_hyphens() {
    let result = preprocess_fts_query("graphrag-precompact");
    assert!(result.contains("\"graphrag precompact\""));
    assert!(result.contains("graphrag*"));
    assert!(result.contains("precompact*"));
}

#[test]
fn preprocess_fts_query_with_dots() {
    let result = preprocess_fts_query("v1.0.44");
    assert!(result.contains("\"v1 0 44\""));
    assert!(result.contains("v1*"));
    assert!(result.contains("44*"));
}

#[test]
fn preprocess_fts_query_with_mixed_separators() {
    let result = preprocess_fts_query("graphrag-precompact.sh");
    assert!(result.contains("\"graphrag precompact sh\""));
    assert!(result.contains("graphrag*"));
}

#[test]
fn preprocess_fts_query_empty_and_whitespace() {
    assert_eq!(preprocess_fts_query(""), "");
    assert_eq!(preprocess_fts_query("  "), "");
}

#[test]
fn preprocess_fts_query_strips_quotes() {
    let result = preprocess_fts_query(r#"hello "world"#);
    assert!(result.contains("hello*"));
    assert!(result.contains("world*"));
}

#[test]
fn preprocess_fts_query_strips_asterisks() {
    assert_eq!(preprocess_fts_query("test*"), "test*");
}

#[test]
fn preprocess_fts_query_strips_parens() {
    let result = preprocess_fts_query("(hello)");
    assert!(result.contains("hello*"));
    assert!(!result.contains('('));
}

#[test]
fn preprocess_fts_query_filters_fts_keywords() {
    let result = preprocess_fts_query("foo OR bar");
    assert!(result.contains("foo*"));
    assert!(result.contains("bar*"));
    assert!(!result.contains("OR*"));
}

#[test]
fn preprocess_fts_query_only_fts_keywords() {
    assert_eq!(preprocess_fts_query("OR AND NOT"), "");
}

#[test]
fn preprocess_fts_query_keywords_with_separators() {
    let result = preprocess_fts_query("hello-OR-world");
    assert!(result.contains("hello*"));
    assert!(result.contains("world*"));
    assert!(!result.contains("OR*"));
}

#[test]
fn fts_search_finds_compound_term_with_hyphen() -> TestResult {
    let conn = setup_conn()?;
    let mut m = new_memory("mem-compound");
    m.body = "the graphrag-precompact script runs daily".to_string();
    insert(&conn, &m)?;
    conn.execute_batch(
        "INSERT INTO fts_memories(rowid, name, description, body)
         SELECT id, name, description, body FROM memories WHERE deleted_at IS NULL",
    )?;
    let rows = fts_search(&conn, "graphrag-precompact", "global", None, 10)?;
    assert!(!rows.is_empty(), "should find compound hyphenated term");
    Ok(())
}

#[test]
fn find_by_name_any_state_returns_deleted_flag() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-soft-del");
    let id = insert(&conn, &m)?;
    conn.execute(
        "UPDATE memories SET deleted_at = unixepoch() WHERE id = ?1",
        rusqlite::params![id],
    )?;
    let result = find_by_name_any_state(&conn, "global", "mem-soft-del")?;
    assert_eq!(result, Some((id, true)));
    Ok(())
}

#[test]
fn find_by_name_any_state_returns_not_deleted() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-active");
    let id = insert(&conn, &m)?;
    let result = find_by_name_any_state(&conn, "global", "mem-active")?;
    assert_eq!(result, Some((id, false)));
    Ok(())
}

#[test]
fn find_by_name_any_state_returns_none_when_absent() -> TestResult {
    let conn = setup_conn()?;
    let result = find_by_name_any_state(&conn, "global", "does-not-exist")?;
    assert!(result.is_none());
    Ok(())
}

#[test]
fn clear_deleted_at_restores_memory() -> TestResult {
    let conn = setup_conn()?;
    let m = new_memory("mem-restore");
    let id = insert(&conn, &m)?;
    conn.execute(
        "UPDATE memories SET deleted_at = unixepoch() WHERE id = ?1",
        rusqlite::params![id],
    )?;
    // Soft-deleted: find_by_name should return None.
    assert!(find_by_name(&conn, "global", "mem-restore")?.is_none());
    clear_deleted_at(&conn, id)?;
    // Restored: find_by_name should return Some again.
    let found = find_by_name(&conn, "global", "mem-restore")?;
    assert!(found.is_some());
    assert_eq!(found.unwrap().0, id);
    Ok(())
}
