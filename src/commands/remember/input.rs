//! Body and graph input resolution for `remember`.
//!
//! Collapses the mutually exclusive body sources (`--body`, `--body-file`,
//! `--body-stdin`, `--graph-stdin`, `--graph-file`) and the graph sources
//! (`--entities-file`, `--relationships-file`, `--graph-stdin`, `--graph-file`)
//! into one resolved payload, applying the entity and relationship caps.

use super::args::RememberArgs;
use super::graph_input::{normalize_and_validate_graph_input, GraphInput};
use crate::constants::{
    max_entities_per_memory, max_relationships_per_memory, MAX_MEMORY_BODY_LEN,
};
use crate::errors::AppError;
use crate::i18n::errors_msg;

/// The body and graph a single `remember` invocation will persist.
pub(super) struct ResolvedInput {
    /// Body text before chunking.
    pub(super) raw_body: String,
    /// Entities and relationships supplied by the caller.
    pub(super) graph: GraphInput,
    /// True when entities arrived from a file or `--graph-stdin`.
    pub(super) entities_provided_externally: bool,
    /// True when the relationship list was truncated to the cap.
    pub(super) relationships_updated: bool,
}

/// Resolves every body and graph source into one payload.
pub(super) fn resolve(args: &RememberArgs) -> Result<ResolvedInput, AppError> {
    // GAP-SG-30: capture whether the body comes from an explicit source before
    // `args.body` is moved below; --graph-file only adopts the file's body when
    // no explicit body source was supplied.
    let body_explicitly_provided =
        args.body.is_some() || args.body_file.is_some() || args.body_stdin;

    let mut raw_body = if let Some(ref b) = args.body {
        b.clone()
    } else if let Some(ref path) = args.body_file {
        let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                let bytes = std::fs::read(path).map_err(AppError::Io)?;
                tracing::warn!(target: "remember", "body file contains invalid UTF-8; replacing invalid sequences");
                String::from_utf8_lossy(&bytes).into_owned()
            }
            Err(e) => return Err(AppError::Io(e)),
        }
    } else if args.body_stdin || args.graph_stdin {
        crate::stdin_helper::read_stdin()?
    } else {
        String::new()
    };

    let mut entities_provided_externally =
        args.entities_file.is_some() || args.relationships_file.is_some();

    let mut graph = GraphInput::default();
    if let Some(ref path) = args.entities_file {
        let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
        // v1.1.1 (P7): boundary validation with context — an invalid
        // entity_type surfaces the FromStr message (13 valid values + hints)
        // as a Validation error instead of a bare Json error (exit 20).
        graph.entities = serde_json::from_str(&content).map_err(|e| {
            AppError::Validation(crate::i18n::validation::invalid_json_in_flag(
                "--entities-file",
                &e,
            ))
        })?;
    }
    if let Some(ref path) = args.relationships_file {
        let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
        graph.relationships = serde_json::from_str(&content).map_err(|e| {
            AppError::Validation(crate::i18n::validation::invalid_json_in_flag(
                "--relationships-file",
                &e,
            ))
        })?;
    }
    if args.graph_stdin {
        graph = serde_json::from_str::<GraphInput>(&raw_body).map_err(|e| {
            AppError::Validation(crate::i18n::validation::invalid_json_payload_on_flag(
                "--graph-stdin",
                &e,
            ))
        })?;
        raw_body = graph.body.take().unwrap_or_default();
    }
    if args.graph_stdin && !graph.entities.is_empty() {
        entities_provided_externally = true;
    }
    // GAP-SG-30: graph from a file, combinable with any body source. Conflicts
    // with --graph-stdin/--entities-file/--relationships-file (enforced by clap),
    // so `graph` is still empty here when --graph-file is set.
    if let Some(ref path) = args.graph_file {
        let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
        let mut gf = serde_json::from_str::<GraphInput>(&content).map_err(|e| {
            AppError::Validation(crate::i18n::validation::invalid_json_in_flag(
                "--graph-file",
                &e,
            ))
        })?;
        graph.entities = gf.entities;
        graph.relationships = gf.relationships;
        if !body_explicitly_provided {
            raw_body = gf.body.take().unwrap_or_default();
        }
        if !graph.entities.is_empty() {
            entities_provided_externally = true;
        }
    }

    if graph.entities.len() > max_entities_per_memory() {
        return Err(AppError::LimitExceeded(errors_msg::entity_limit_exceeded(
            max_entities_per_memory(),
        )));
    }
    let mut relationships_updated = false;
    let rel_cap = max_relationships_per_memory();
    if graph.relationships.len() > rel_cap {
        tracing::warn!(target: "remember",
            count = graph.relationships.len(),
            cap = rel_cap,
            "truncating relationships to cap"
        );
        graph.relationships.truncate(rel_cap);
        relationships_updated = true;
    }
    normalize_and_validate_graph_input(&mut graph)?;

    Ok(ResolvedInput {
        raw_body,
        graph,
        entities_provided_externally,
        relationships_updated,
    })
}
