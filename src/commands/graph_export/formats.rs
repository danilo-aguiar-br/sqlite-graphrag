//! Snapshot export formatters for the `graph` command.
//!
//! Renders entity/edge snapshots as pretty JSON, NDJSON streams, Graphviz DOT,
//! and Mermaid diagrams. Types here (`NodeOut`, `EdgeOut`, `GraphSnapshot`) are
//! shared by the default snapshot path and the format-specific renderers.

use crate::errors::AppError;
use crate::output;
use serde::Serialize;
use std::fs;

#[derive(Serialize, Clone)]
pub(crate) struct NodeOut {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) namespace: String,
    /// Deprecated alias of `type` kept for backward-compat with pre-v1.0.35 clients.
    /// New consumers MUST read `type` instead. Will be removed in a future major release.
    pub(crate) kind: String,
    /// Canonical entity classification (organization, concept, person, etc.).
    /// Mirrors `kind` while the deprecation window is active.
    #[serde(rename = "type")]
    pub(crate) r#type: String,
}

#[derive(Serialize)]
pub(crate) struct EdgeOut {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) relation: String,
    pub(crate) weight: f64,
}

#[derive(Serialize)]
pub(crate) struct GraphSnapshot {
    pub(crate) nodes: Vec<NodeOut>,
    pub(crate) entities: Vec<NodeOut>,
    pub(crate) edges: Vec<EdgeOut>,
    pub(crate) elapsed_ms: u64,
}

/// Renders the single-envelope JSON snapshot as pretty-printed text.
///
/// Applies the agent-native surface (`--select`, `--filter`, `--max-items`, …)
/// exactly like `output::emit_json` does, so the `--output PATH` destination and
/// stdout never diverge. With no shaping flag installed the value is serialized
/// straight from its own `Serialize` impl and the output is byte-identical to
/// what it was before the surface existed.
pub(crate) fn render_json(snapshot: &GraphSnapshot) -> Result<String, AppError> {
    if crate::agent_surface::active() {
        let shaped = crate::agent_surface::apply_global(serde_json::to_value(snapshot)?);
        return Ok(serde_json::to_string_pretty(&shaped)?);
    }
    Ok(serde_json::to_string_pretty(snapshot)?)
}

/// Streams the graph as NDJSON: one object per node, one per edge, then a summary.
///
/// Each line is flushed immediately so consumers can process incrementally.
/// When `output_path` is `Some`, lines are written to the file; otherwise to stdout.
pub(crate) fn render_ndjson_streaming(
    nodes: &[NodeOut],
    edges: &[EdgeOut],
    elapsed_ms: u64,
    output_path: Option<&std::path::Path>,
) -> Result<(), AppError> {
    #[derive(serde::Serialize)]
    struct NdjsonNode<'a> {
        kind: &'static str,
        id: i64,
        name: &'a str,
        namespace: &'a str,
        #[serde(rename = "type")]
        r#type: &'a str,
    }
    #[derive(serde::Serialize)]
    struct NdjsonEdge<'a> {
        kind: &'static str,
        from: &'a str,
        to: &'a str,
        relation: &'a str,
        weight: f64,
    }
    #[derive(serde::Serialize)]
    struct NdjsonSummary {
        kind: &'static str,
        nodes: usize,
        edges: usize,
        elapsed_ms: u64,
    }

    use std::io::Write as IoWrite;

    let mut buf: Vec<u8> = Vec::with_capacity(4096);

    let emit_line =
        |buf: &mut Vec<u8>, line: &str, path: Option<&std::path::Path>| -> Result<(), AppError> {
            buf.clear();
            buf.extend_from_slice(line.as_bytes());
            buf.push(b'\n');
            if let Some(p) = path {
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)
                    .map_err(AppError::Io)?;
                f.write_all(buf).map_err(AppError::Io)?;
            } else {
                output::emit_text(line);
            }
            Ok(())
        };

    // Truncate the output file once before starting (avoids re-opening with append for every line).
    if let Some(p) = output_path {
        fs::write(p, b"")?;
    }

    for node in nodes {
        let obj = NdjsonNode {
            kind: "node",
            id: node.id,
            name: &node.name,
            namespace: &node.namespace,
            r#type: &node.r#type,
        };
        let line = serde_json::to_string(&obj)?;
        emit_line(&mut buf, &line, output_path)?;
    }

    for edge in edges {
        let obj = NdjsonEdge {
            kind: "edge",
            from: &edge.from,
            to: &edge.to,
            relation: &edge.relation,
            weight: edge.weight,
        };
        let line = serde_json::to_string(&obj)?;
        emit_line(&mut buf, &line, output_path)?;
    }

    let summary = NdjsonSummary {
        kind: "summary",
        nodes: nodes.len(),
        edges: edges.len(),
        elapsed_ms,
    };
    let line = serde_json::to_string(&summary)?;
    emit_line(&mut buf, &line, output_path)?;

    Ok(())
}

pub(crate) fn sanitize_dot_id(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn render_dot(nodes: &[NodeOut], edges: &[EdgeOut]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(nodes.len() * 80 + edges.len() * 60 + 300);
    out.push_str("digraph sqlite_graphrag {\n");
    out.push_str("  graph [bgcolor=\"white\", fontname=\"Helvetica Neue\", fontsize=12, rankdir=LR, nodesep=0.8, ranksep=1.2];\n");
    out.push_str("  node [shape=box, style=\"filled,rounded\", fillcolor=\"#F2F2F7\", fontname=\"Helvetica Neue\", fontsize=11, color=\"#C7C7CC\"];\n");
    out.push_str("  edge [fontname=\"Helvetica Neue\", fontsize=9, color=\"#8E8E93\"];\n");
    for node in nodes {
        let node_id = sanitize_dot_id(&node.name);
        let escaped = node.name.replace('"', "\\\"");
        let _ = writeln!(out, "  {node_id} [label=\"{escaped}\"];");
    }
    for edge in edges {
        let from = sanitize_dot_id(&edge.from);
        let to = sanitize_dot_id(&edge.to);
        let label = edge.relation.replace('"', "\\\"");
        let _ = writeln!(out, "  {from} -> {to} [label=\"{label}\"];");
    }
    out.push_str("}\n");
    out
}

pub(crate) fn sanitize_mermaid_id(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn render_mermaid(nodes: &[NodeOut], edges: &[EdgeOut]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(nodes.len() * 50 + edges.len() * 40 + 200);
    out.push_str("%%{init: {'theme': 'neutral', 'themeVariables': {'primaryColor': '#F2F2F7', 'primaryTextColor': '#1C1C1E', 'primaryBorderColor': '#C7C7CC', 'lineColor': '#8E8E93'}}}%%\n");
    out.push_str("graph LR\n");
    for node in nodes {
        let id = sanitize_mermaid_id(&node.name);
        let escaped = node.name.replace('"', "\\\"");
        let _ = writeln!(out, "  {id}[\"{escaped}\"]");
    }
    for edge in edges {
        let from = sanitize_mermaid_id(&edge.from);
        let to = sanitize_mermaid_id(&edge.to);
        let label = edge.relation.replace('|', "\\|");
        let _ = writeln!(out, "  {from} -->|{label}| {to}");
    }
    out
}
