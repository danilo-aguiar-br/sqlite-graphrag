//! Agent-native R-AN-01: emit a command's JSON Schema and exit without work.
//!
//! When a subcommand is invoked with `--print-schema`, the handler must write
//! the corresponding file from `docs/schemas/*.schema.json` to stdout as
//! **compact** JSON and return successfully without opening the database,
//! calling an LLM, or performing other side effects.
//!
//! Schemas are embedded at compile time so the installed binary does not
//! depend on a source-tree checkout.

use crate::errors::AppError;
use crate::output;

/// Identifiers for schemas that currently support `--print-schema`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaId {
    /// `list` → `docs/schemas/list.schema.json`
    List,
    /// `recall` → `docs/schemas/recall.schema.json`
    Recall,
    /// `enrich --status` → `docs/schemas/enrich-status.schema.json`
    EnrichStatus,
    /// `config list` → `docs/schemas/config-list.schema.json`
    ConfigList,
}

impl SchemaId {
    /// Human-readable command name for error messages.
    pub const fn name(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Recall => "recall",
            Self::EnrichStatus => "enrich-status",
            Self::ConfigList => "config-list",
        }
    }

    /// Embedded pretty JSON Schema source (from the repository at build time).
    const fn embedded(self) -> &'static str {
        match self {
            Self::List => include_str!("../docs/schemas/list.schema.json"),
            Self::Recall => include_str!("../docs/schemas/recall.schema.json"),
            Self::EnrichStatus => include_str!("../docs/schemas/enrich-status.schema.json"),
            Self::ConfigList => include_str!("../docs/schemas/config-list.schema.json"),
        }
    }
}

/// Emit the compact JSON Schema for `id` to stdout and flush.
///
/// Stdout contains only the schema document (one compact JSON line) so agents
/// can pipe the output into a validator without filtering tracing noise.
///
/// # Errors
/// Returns [`AppError::Validation`] if the embedded schema is not valid JSON
/// (should never happen for the checked-in files) or an I/O error from stdout.
pub fn emit(id: SchemaId) -> Result<(), AppError> {
    let value: serde_json::Value = serde_json::from_str(id.embedded()).map_err(|e| {
        AppError::Validation(crate::i18n::validation::embedded_schema_invalid_json(
            id.name(),
            &e,
        ))
    })?;
    // Compact form: strip pretty-print whitespace from the source files.
    output::emit_json_compact(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_schemas_parse_as_json_objects() {
        for id in [
            SchemaId::List,
            SchemaId::Recall,
            SchemaId::EnrichStatus,
            SchemaId::ConfigList,
        ] {
            let v: serde_json::Value = serde_json::from_str(id.embedded())
                .unwrap_or_else(|e| panic!("{}: {e}", id.name()));
            assert!(v.is_object(), "{} schema must be a JSON object", id.name());
            assert!(
                v.get("$schema").is_some() || v.get("type").is_some(),
                "{} schema must look like a JSON Schema document",
                id.name()
            );
        }
    }
}
