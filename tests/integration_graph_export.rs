//! Entity graph — `--graph-stdin` ingestion, export formats and orphan cleanup.
//!
//! Split out of `tests/integration_graph.rs` by GAP-SG-210: that file held 922
//! lines, past the 800-line ceiling this project sets for itself. It was itself
//! carved out of the 2 485-line `tests/integration.rs` in v1.2.5, so this is
//! the second cut of the same tree; the shared helpers stayed in
//! `tests/common/` and were never copied.

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

// ---------------------------------------------------------------------------
// graph (export)
// ---------------------------------------------------------------------------

#[test]
fn test_graph_export_json_estrutura_correta() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    seed_memory_with_entities(
        &tmp,
        "graph-seed-json",
        r#"[
            {"name":"graph-ent-a","entity_type":"project","description":null},
            {"name":"graph-ent-b","entity_type":"tool","description":null}
        ]"#,
    );
    cmd(&tmp)
        .args([
            "link",
            "--from",
            "graph-ent-a",
            "--to",
            "graph-ent-b",
            "--relation",
            "uses",
        ])
        .assert()
        .success();

    let output = cmd(&tmp)
        .args(["graph", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["nodes"].is_array());
    assert!(json["edges"].is_array());
    assert!(json["nodes"].as_array().unwrap().len() >= 2);
    assert!(!json["edges"].as_array().unwrap().is_empty());
}

#[test]
fn test_graph_stdin_preserves_entity_type_when_creating_relationships() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let payload = r#"{
        "entities": [
            {"name": "tipo-tool", "entity_type": "tool"},
            {"name": "tipo-file", "entity_type": "file"}
        ],
        "relationships": [
            {"source": "tipo-tool", "target": "tipo-file", "relation": "uses", "strength": 0.9}
        ]
    }"#;

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "grafo-tipado",
            "--type",
            "project",
            "--description",
            "grafo tipado via stdin",
            "--graph-stdin",
        ])
        .write_stdin(payload)
        .assert()
        .success();

    let output = cmd(&tmp)
        .args(["graph", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    let tipo_tool = nodes
        .iter()
        .find(|node| node["name"] == "tipo-tool")
        .expect("tipo-tool deve existir");
    let tipo_file = nodes
        .iter()
        .find(|node| node["name"] == "tipo-file")
        .expect("tipo-file deve existir");

    assert_eq!(tipo_tool["type"], "tool");
    assert_eq!(tipo_file["type"], "file");
}

#[test]
fn test_graph_stdin_accepts_from_to_aliases_and_hyphenated_relation() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let payload = r#"{
        "entities": [
            {"name": "alias-tool", "entity_type": "tool"},
            {"name": "alias-file", "entity_type": "file"}
        ],
        "relationships": [
            {"from": "alias-tool", "to": "alias-file", "relation": "depends-on", "strength": 0.7}
        ]
    }"#;

    cmd(&tmp)
        .args([
            "remember",
            "--name",
            "grafo-aliases",
            "--type",
            "project",
            "--description",
            "grafo com aliases de relacionamento",
            "--graph-stdin",
        ])
        .write_stdin(payload)
        .assert()
        .success();

    let output = cmd(&tmp)
        .args(["graph", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let edges = json["edges"].as_array().unwrap();
    assert!(edges.iter().any(|edge| {
        edge["from"] == "alias-tool"
            && edge["to"] == "alias-file"
            && edge["relation"] == "depends_on"
    }));
}

#[test]
fn test_graph_stdin_with_skip_extraction_persists_explicit_graph() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let payload = r#"{
        "entities": [
            {"name": "skip-tool", "entity_type": "tool"},
            {"name": "skip-file", "entity_type": "file"}
        ],
        "relationships": [
            {"source": "skip-tool", "target": "skip-file", "relation": "uses", "strength": 0.8}
        ]
    }"#;

    let remember_output = cmd(&tmp)
        .args([
            "remember",
            "--name",
            "grafo-skip",
            "--type",
            "project",
            "--description",
            "grafo explicito com skip",
            "--skip-extraction",
            "--graph-stdin",
            "--json",
        ])
        .write_stdin(payload)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let remember_json: serde_json::Value = serde_json::from_slice(&remember_output).unwrap();
    assert_eq!(remember_json["entities_persisted"], 2);
    assert_eq!(remember_json["relationships_persisted"], 1);

    let output = cmd(&tmp)
        .args(["graph", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|node| node["name"] == "skip-tool" && node["type"] == "tool"));
    assert_eq!(json["edges"].as_array().unwrap().len(), 1);
}

#[test]
fn test_graph_stdin_accepts_body_in_same_payload() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let payload = r#"{
        "body": "corpo textual enviado junto com grafo explicito",
        "entities": [
            {"name": "payload-tool", "entity_type": "tool"},
            {"name": "payload-file", "entity_type": "file"}
        ],
        "relationships": [
            {"source": "payload-tool", "target": "payload-file", "relation": "uses", "strength": 0.8}
        ]
    }"#;

    let remember_output = cmd(&tmp)
        .args([
            "remember",
            "--name",
            "grafo-com-body",
            "--type",
            "project",
            "--description",
            "grafo com body via stdin",
            "--skip-extraction",
            "--graph-stdin",
            "--json",
        ])
        .write_stdin(payload)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let remember_json: serde_json::Value = serde_json::from_slice(&remember_output).unwrap();
    assert_eq!(remember_json["entities_persisted"], 2);
    assert_eq!(remember_json["relationships_persisted"], 1);

    let read_output = cmd(&tmp)
        .args(["read", "--name", "grafo-com-body", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let read_json: serde_json::Value = serde_json::from_slice(&read_output).unwrap();
    assert_eq!(
        read_json["body"],
        "corpo textual enviado junto com grafo explicito"
    );
}

#[test]
fn test_graph_json_flag_vence_format_dot() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd(&tmp)
        .args(["graph", "--json", "--format", "dot"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["nodes"].is_array());
    assert!(json["edges"].is_array());
}

#[test]
fn test_graph_json_flag_vence_format_mermaid() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd(&tmp)
        .args(["graph", "--json", "--format", "mermaid"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["nodes"].is_array());
    assert!(json["edges"].is_array());
}

#[test]
fn test_graph_json_flag_keeps_stdout_even_with_output() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output_path = tmp.path().join("graph.dot");
    let output_path_str = output_path.to_str().unwrap();

    let output = cmd(&tmp)
        .args([
            "graph",
            "--json",
            "--format",
            "dot",
            "--output",
            output_path_str,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["nodes"].is_array());
    assert!(
        !output_path.exists(),
        "--json deve manter o contrato stdout em vez de gravar DOT"
    );
}

#[test]
fn test_graph_stats_json_flag_vence_format_text() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    let output = cmd(&tmp)
        .args(["graph", "stats", "--json", "--format", "text"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json["node_count"].is_number());
    assert!(json["edge_count"].is_number());
}

#[test]
fn test_graph_export_dot_contem_digraph() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    seed_memory_with_entities(
        &tmp,
        "graph-seed-dot",
        r#"[
            {"name":"dot-a","entity_type":"project","description":null},
            {"name":"dot-b","entity_type":"tool","description":null}
        ]"#,
    );
    cmd(&tmp)
        .args([
            "link",
            "--from",
            "dot-a",
            "--to",
            "dot-b",
            "--relation",
            "uses",
        ])
        .assert()
        .success();

    let out = cmd(&tmp)
        .args(["graph", "--format", "dot"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rendered = String::from_utf8(out).unwrap();
    assert!(rendered.contains("digraph sqlite_graphrag"));
    assert!(rendered.contains("dot-a"));
    assert!(rendered.contains("dot-b"));
    assert!(rendered.contains("uses"));
}

#[test]
fn test_graph_export_mermaid_contem_graph_lr() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    seed_memory_with_entities(
        &tmp,
        "graph-seed-mermaid",
        r#"[{"name":"mer-a","entity_type":"project","description":null}]"#,
    );

    let out = cmd(&tmp)
        .args(["graph", "--format", "mermaid"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rendered = String::from_utf8(out).unwrap();
    assert!(rendered.contains("graph LR"));
    assert!(rendered.contains("mer_a"));
}

// ---------------------------------------------------------------------------
// cleanup-orphans
// ---------------------------------------------------------------------------

#[test]
fn test_cleanup_orphans_remove_entidades_orfas() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    // Create a memory with linked entities
    seed_memory_with_entities(
        &tmp,
        "co-mem-ligada",
        r#"[{"name":"co-ent-ligada","entity_type":"project","description":null}]"#,
    );

    // Create a memory with additional entities and remove it, leaving orphan entities
    seed_memory_with_entities(
        &tmp,
        "co-mem-descartada",
        r#"[{"name":"co-ent-orfa","entity_type":"project","description":null}]"#,
    );
    cmd(&tmp)
        .args(["forget", "--name", "co-mem-descartada"])
        .assert()
        .success();
    cmd(&tmp)
        .args([
            "purge",
            "--name",
            "co-mem-descartada",
            "--retention-days",
            "0",
            "--yes",
        ])
        .assert()
        .success();

    // Dry-run counts orphans without removing
    let output = cmd(&tmp)
        .args(["cleanup-orphans", "--dry-run"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let dry: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(dry["dry_run"], true);
    assert!(dry["orphan_count"].as_u64().unwrap() >= 1);
    assert_eq!(dry["deleted"].as_u64().unwrap(), 0);

    // Real execution removes the orphans
    let output = cmd(&tmp)
        .args(["cleanup-orphans", "--yes"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let done: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(done["dry_run"], false);
    assert!(done["deleted"].as_u64().unwrap() >= 1);
}

#[test]
fn test_cleanup_orphans_without_orphans_returns_zero() {
    let tmp = TempDir::new().unwrap();
    init_db(&tmp);

    seed_memory_with_entities(
        &tmp,
        "co-limpo",
        r#"[{"name":"co-ent-limpa","entity_type":"project","description":null}]"#,
    );

    let output = cmd(&tmp)
        .args(["cleanup-orphans"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["orphan_count"], 0);
    assert_eq!(json["deleted"], 0);
}
