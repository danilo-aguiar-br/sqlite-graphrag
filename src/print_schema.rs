//! Agent-native R-AN-01: emit a command's JSON Schema and exit without work.
//!
//! Two surfaces share this module:
//!
//! * the per-subcommand `--print-schema` flag, which emits the one schema its
//!   subcommand documents;
//! * the top-level `schema` subcommand, which lists every contract shipped in
//!   `docs/schemas/` and emits any of them by id.
//!
//! Both write **compact** JSON to stdout and return successfully without
//! opening the database, calling an LLM, or performing other side effects.
//!
//! Schemas are embedded at compile time so the installed binary does not
//! depend on a source-tree checkout.

use crate::errors::AppError;
use crate::output;

/// Declares every schema id, its enum variant and its embedded source.
///
/// A macro is used because the three projections (`name`, `embedded`, `ALL`)
/// must stay in lockstep across 74 contracts; writing them by hand is three
/// chances for an id to drift away from the file it claims to describe.
macro_rules! schema_ids {
    ($($variant:ident => $id:literal;)*) => {
        /// Identifiers for every JSON Schema shipped under `docs/schemas/`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum SchemaId {
            $(
                #[doc = concat!("`", $id, "` → `docs/schemas/", $id, ".schema.json`")]
                $variant,
            )*
        }

        impl SchemaId {
            /// Every declared schema id, in the order written above.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            /// Canonical id, identical to the file stem under `docs/schemas/`.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $id,)*
                }
            }

            /// Embedded pretty JSON Schema source (from the repository at build time).
            const fn embedded(self) -> &'static str {
                match self {
                    $(Self::$variant => include_str!(
                        concat!("../docs/schemas/", $id, ".schema.json")
                    ),)*
                }
            }

            /// Resolves a canonical id to its variant, or `None` when unknown.
            pub fn from_id(id: &str) -> Option<Self> {
                match id {
                    $($id => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

schema_ids! {
    // GAP-SG-160: the block the agent-native surface injects into every
    // reshaped envelope. Shared definition, referenced by the per-command
    // schemas rather than duplicated in each of them.
    AgentSurface => "agent-surface";
    Backup => "backup";
    CleanupOrphans => "cleanup-orphans";
    ConfigList => "config-list";
    DebugSchema => "debug-schema";
    DeepResearch => "deep-research";
    DeepResearchOutputAck => "deep-research-output-ack";
    DeleteEntity => "delete-entity";
    Edit => "edit";
    EmbeddingList => "embedding-list";
    EmbeddingStatus => "embedding-status";
    EnrichItemEvent => "enrich-item-event";
    EnrichPhase => "enrich-phase";
    EnrichStatus => "enrich-status";
    EnrichSummary => "enrich-summary";
    EntitiesInput => "entities-input";
    ErrorEnvelope => "error-envelope";
    ExportMemoryLine => "export-memory-line";
    ExportSummary => "export-summary";
    Forget => "forget";
    FtsCheck => "fts-check";
    FtsRebuild => "fts-rebuild";
    FtsStats => "fts-stats";
    Graph => "graph";
    GraphEntities => "graph-entities";
    GraphRecomputeDegree => "graph-recompute-degree";
    GraphStats => "graph-stats";
    GraphTraverse => "graph-traverse";
    Health => "health";
    History => "history";
    HybridSearch => "hybrid-search";
    IngestClaudeFileEvent => "ingest-claude-file-event";
    IngestClaudePhase => "ingest-claude-phase";
    IngestClaudeSummary => "ingest-claude-summary";
    IngestFileEvent => "ingest-file-event";
    IngestSummary => "ingest-summary";
    Init => "init";
    Link => "link";
    List => "list";
    MemoryEntities => "memory-entities";
    MemoryEntitiesReverse => "memory-entities-reverse";
    MergeEntities => "merge-entities";
    Migrate => "migrate";
    MigrateRehash => "migrate-rehash";
    MigrateToLlmOnly => "migrate-to-llm-only";
    NamespaceDetect => "namespace-detect";
    NormalizeEntities => "normalize-entities";
    Optimize => "optimize";
    PendingList => "pending-list";
    PruneNer => "prune-ner";
    PruneRelations => "prune-relations";
    Purge => "purge";
    Read => "read";
    Recall => "recall";
    Reclassify => "reclassify";
    ReclassifyRelation => "reclassify-relation";
    Related => "related";
    RelationshipsInput => "relationships-input";
    Remember => "remember";
    RememberBatch => "remember-batch";
    RememberBatchSummary => "remember-batch-summary";
    Rename => "rename";
    RenameEntity => "rename-entity";
    Restore => "restore";
    ShutdownEnvelope => "shutdown-envelope";
    SlotsStatus => "slots-status";
    SplitBody => "split-body";
    Stats => "stats";
    SyncSafeCopy => "sync-safe-copy";
    Unlink => "unlink";
    Vacuum => "vacuum";
    VecOrphanList => "vec-orphan-list";
    VecPurgeOrphan => "vec-purge-orphan";
    VecStats => "vec-stats";
}

/// Minimum Jaro-Winkler similarity required to suggest a replacement id.
///
/// Mirrors the threshold [`crate::config::registry`] uses for setting keys, so
/// a typo gets the same quality of hint on both surfaces.
const SUGGESTION_THRESHOLD: f64 = 0.7;

impl SchemaId {
    /// Returns the closest known id to `id`, when one is similar enough.
    ///
    /// Reuses the `rapidfuzz` Jaro-Winkler scorer already used for setting keys
    /// and entity names rather than introducing a third similarity metric.
    pub fn nearest(id: &str) -> Option<&'static str> {
        Self::ALL
            .iter()
            .map(|candidate| {
                let score = rapidfuzz::distance::jaro_winkler::normalized_similarity(
                    id.chars(),
                    candidate.name().chars(),
                );
                (candidate.name(), score)
            })
            .filter(|(_, score)| *score >= SUGGESTION_THRESHOLD)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(candidate, _)| candidate)
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

/// Arguments for the top-level `schema` subcommand.
///
/// Deliberately a subcommand rather than a global flag: clap propagates a
/// global argument downward only, so four subcommands already defining
/// `--print-schema` would collide on the same id. A subcommand also matches the
/// surface the sibling CLIs in this toolchain expose.
#[derive(Debug, clap::Args)]
pub struct SchemaArgs {
    /// Schema id to emit. Omit to list the whole catalogue as NDJSON.
    #[arg(long, value_name = "ID")]
    pub name: Option<String>,

    /// No-op; JSON is always emitted on stdout by `schema`.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,

    /// GAP-SG-139: accepted as a no-op for agent uniformity (no graph I/O).
    #[command(flatten)]
    pub db_noop: crate::cli_db_noop::DbNoopArgs,
}

/// Runs the `schema` subcommand: catalogue listing or single-document emit.
///
/// Never opens the database and never requires an embedding API key.
///
/// # Errors
/// Returns [`AppError::NotFound`] when `--name` is not a known id, or the
/// error surfaced by [`emit`] for a known one.
pub fn run(args: SchemaArgs) -> Result<(), AppError> {
    args.db_noop.ignore();
    let _ = args.json;
    match args.name.as_deref() {
        Some(id) => match SchemaId::from_id(id) {
            Some(schema) => emit(schema),
            None => Err(AppError::NotFound(
                crate::i18n::validation::unknown_schema_id(id, SchemaId::nearest(id)),
            )),
        },
        None => {
            emit_catalog();
            Ok(())
        }
    }
}

/// Writes one NDJSON record per known schema: `{"id": …, "invoke": …}`.
///
/// NDJSON rather than a single array so the line count equals the contract
/// count, which is what the anti-drift gate asserts.
fn emit_catalog() {
    for schema in SchemaId::ALL {
        let id = schema.name();
        output::emit_json_line(&serde_json::json!({
            "id": id,
            "invoke": format!("sqlite-graphrag schema --name {id}"),
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_schemas_parse_as_json_objects() {
        for id in SchemaId::ALL {
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

    #[test]
    fn every_id_round_trips_through_from_id() {
        for id in SchemaId::ALL {
            assert_eq!(SchemaId::from_id(id.name()), Some(*id));
        }
        assert_eq!(SchemaId::from_id("no-such-schema-at-all"), None);
    }

    #[test]
    fn ids_are_unique_and_sorted() {
        let names: Vec<&str> = SchemaId::ALL.iter().map(|id| id.name()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted, "SchemaId::ALL must be sorted and unique");
    }

    #[test]
    fn nearest_suggests_a_close_id_and_nothing_for_gibberish() {
        assert_eq!(SchemaId::nearest("enrich-statu"), Some("enrich-status"));
        assert_eq!(SchemaId::nearest("zzzzzzzzzzzzzzzz"), None);
    }

    /// Anti-drift gate: the enum must cover `docs/schemas/` exactly.
    ///
    /// A file added without a variant is unreachable from the CLI — the very
    /// gap this surface exists to close — and a variant without a file would
    /// not compile, so only the first direction needs a runtime assertion.
    /// Both are asserted anyway so a future refactor cannot quietly invert it.
    #[test]
    fn schema_ids_cover_every_file_in_docs_schemas() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/schemas");
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("docs/schemas must be readable: {e}"));
        let mut on_disk: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.strip_suffix(".schema.json").map(str::to_string)
            })
            .collect();
        on_disk.sort();
        assert!(
            !on_disk.is_empty(),
            "walk found zero schema files under {} — the walk itself is broken, \
             which is exactly how this guard would go silently blind",
            dir.display()
        );

        let declared: std::collections::HashSet<&str> =
            SchemaId::ALL.iter().map(|id| id.name()).collect();
        let missing: Vec<&String> = on_disk
            .iter()
            .filter(|id| !declared.contains(id.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "schema files with no SchemaId variant (unreachable from the CLI): {missing:?}"
        );

        let on_disk_set: std::collections::HashSet<&str> =
            on_disk.iter().map(String::as_str).collect();
        let orphaned: Vec<&str> = declared
            .iter()
            .copied()
            .filter(|id| !on_disk_set.contains(id))
            .collect();
        assert!(
            orphaned.is_empty(),
            "SchemaId variants with no file under docs/schemas: {orphaned:?}"
        );
    }
}
