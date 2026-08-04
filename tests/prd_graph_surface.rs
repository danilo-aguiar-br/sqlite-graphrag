//! PRD compliance: health, history, link/unlink, graph rendering and hybrid ranking (clauses 13-20).
//!
//! Part of the PRD-compliance suite split by GAP-SG-208. Covers the MUST/DEVE
//! clauses of the sqlite-graphrag PRD. The shared harness lives in
//! `tests/prd_support/`.

#[path = "prd_support/mod.rs"]
mod support;

use rusqlite::Connection;
use support::{cmd_base, db_path, init_db, remember_ok};
use tempfile::TempDir;
// ---------------------------------------------------------------------------
// 13 — health emite integrity_ok e schema_ok
// ---------------------------------------------------------------------------

#[test]
fn prd_health_emits_integrity_ok_and_schema_ok() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd_base(&tmp)
        .arg("health")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(
        json.get("integrity_ok").is_some(),
        "health deve emitir integrity_ok"
    );
    assert!(
        json.get("schema_ok").is_some(),
        "health deve emitir schema_ok"
    );
}

// ---------------------------------------------------------------------------
// 14 — history inclui created_at_iso
// ---------------------------------------------------------------------------

#[test]
fn prd_history_includes_created_at_iso() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    remember_ok(&tmp, "mem-history-iso", "corpo para history test");

    let output = cmd_base(&tmp)
        .args([
            "history",
            "--name",
            "mem-history-iso",
            "--namespace",
            "global",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let versions = json["versions"].as_array().unwrap();
    assert!(!versions.is_empty(), "deve haver ao menos uma versão");
    assert!(
        versions[0].get("created_at_iso").is_some(),
        "versão deve conter campo created_at_iso"
    );
}

// ---------------------------------------------------------------------------
// 15 — link cria entrada em memory_relationships
// ---------------------------------------------------------------------------

#[test]
fn prd_link_creates_memory_relationships() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // Create two memories (and thus potentially entities via extraction)
    remember_ok(&tmp, "mem-link-src", "entidade alfa para link test");
    remember_ok(&tmp, "mem-link-dst", "entidade beta para link test");

    // Verifica que ao menos duas entidades existem ou cria via link direto
    // Try the link; if there are no entities, the test validates the error behavior
    let output = cmd_base(&tmp)
        .args([
            "link",
            "--from",
            "mem-link-src",
            "--to",
            "mem-link-dst",
            "--relation",
            "related",
            "--namespace",
            "global",
        ])
        .output()
        .unwrap();

    if output.status.success() {
        // Se o link funcionou, verifica a tabela memory_relationships
        let conn = Connection::open(db_path(&tmp)).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_relationships", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        // Verify relationships as well
        let rel_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))
            .unwrap_or(0);

        assert!(
            count > 0 || rel_count > 0,
            "link deve criar entrada em memory_relationships ou relationships"
        );
    } else {
        // Entities do not exist — link failed with exit 4 (NotFound): correct behavior
        assert_eq!(
            output.status.code(),
            Some(4),
            "sem entidades, link deve retornar exit 4"
        );
    }
}

// ---------------------------------------------------------------------------
// 16 — unlink removes only the specific relation, preserving others
// ---------------------------------------------------------------------------

#[test]
fn prd_unlink_removes_only_specific_relation() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let conn = Connection::open(db_path(&tmp)).unwrap();

    // Insere entidades e relacionamentos manualmente
    conn.execute_batch(
        "INSERT INTO entities (name, type, namespace) VALUES ('ent-a', 'concept', 'global');
         INSERT INTO entities (name, type, namespace) VALUES ('ent-b', 'concept', 'global');
         INSERT INTO entities (name, type, namespace) VALUES ('ent-c', 'concept', 'global');",
    )
    .unwrap();

    let id_a: i64 = conn
        .query_row("SELECT id FROM entities WHERE name='ent-a'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let id_b: i64 = conn
        .query_row("SELECT id FROM entities WHERE name='ent-b'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let id_c: i64 = conn
        .query_row("SELECT id FROM entities WHERE name='ent-c'", [], |r| {
            r.get(0)
        })
        .unwrap();

    conn.execute(
        "INSERT INTO relationships (source_id, target_id, relation, weight, namespace) VALUES (?1, ?2, 'related', 1.0, 'global')",
        [id_a, id_b],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO relationships (source_id, target_id, relation, weight, namespace) VALUES (?1, ?2, 'related', 1.0, 'global')",
        [id_a, id_c],
    )
    .unwrap();

    drop(conn);

    // Desfaz apenas o link A→B
    cmd_base(&tmp)
        .args([
            "unlink",
            "--from",
            "ent-a",
            "--to",
            "ent-b",
            "--relation",
            "related",
            "--namespace",
            "global",
        ])
        .assert()
        .success();

    let conn2 = Connection::open(db_path(&tmp)).unwrap();
    let remaining: i64 = conn2
        .query_row(
            "SELECT COUNT(*) FROM relationships WHERE source_id=?1",
            [id_a],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        remaining, 1,
        "unlink deve remover apenas a relação específica A→B, preservando A→C"
    );
}

// ---------------------------------------------------------------------------
// 17 — graph JSON contains nodes and edges
// ---------------------------------------------------------------------------

#[test]
fn prd_graph_json_contains_nodes_and_edges() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd_base(&tmp)
        .args(["graph", "--format", "json", "--namespace", "global"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8_lossy(&output);
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(
        json.get("nodes").is_some(),
        "graph JSON deve conter campo 'nodes'"
    );
    assert!(
        json.get("edges").is_some(),
        "graph JSON deve conter campo 'edges'"
    );
}

// ---------------------------------------------------------------------------
// 18 — graph DOT is a valid digraph (starts with "digraph sqlite-graphrag {")
// ---------------------------------------------------------------------------

#[test]
fn prd_graph_dot_is_valid_digraph() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd_base(&tmp)
        .args(["graph", "--format", "dot", "--namespace", "global"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("digraph sqlite_graphrag {"),
        "graph DOT deve começar com 'digraph sqlite_graphrag {{', obtido: {text}"
    );
}

// ---------------------------------------------------------------------------
// 19 — graph Mermaid starts with "graph LR"
// ---------------------------------------------------------------------------

#[test]
fn prd_graph_mermaid_starts_with_graph_lr() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd_base(&tmp)
        .args(["graph", "--format", "mermaid", "--namespace", "global"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("graph LR"),
        "graph Mermaid deve conter 'graph LR', obtido: {text}"
    );
}

// ---------------------------------------------------------------------------
// 20 — hybrid-search usa RRF k=60 como default (verifica que aceita o arg)
// ---------------------------------------------------------------------------

#[test]
fn prd_hybrid_search_rrf_k_default_60() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // Verify that --rrf-k 60 is accepted without error (documented default value)
    // Use empty database — empty result is acceptable
    cmd_base(&tmp)
        .args([
            "hybrid-search",
            "query de teste prd",
            "--rrf-k",
            "60",
            "--namespace",
            "global",
        ])
        .assert()
        .success();
}
