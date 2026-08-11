//! The subcommand surface: every `Commands` variant and its classification.
//!
//! Holds the `Commands` enum, the predicates `main` uses to route a variant,
//! and the manual `Debug` impl.

use crate::commands::*;
use clap::Subcommand;

/// Every subcommand the CLI dispatches, in the order `--help` renders them.
#[derive(Subcommand)]
pub enum Commands {
    /// Initialize the database and write the schema (no model download, no subprocess)
    #[command(after_long_help = "EXAMPLES:\n  \
        # Initialize in current directory (default behavior)\n  \
        sqlite-graphrag init\n\n  \
        # Initialize at a specific path\n  \
        sqlite-graphrag init --db /path/to/graphrag.sqlite\n\n  \
        # Persist default db path via XDG config (no product env)\n  \
        sqlite-graphrag config set db.path /data/graphrag.sqlite\n  \
        sqlite-graphrag init\n\n\
        NOTES:\n  \
        - `init` is OPTIONAL: any subsequent CRUD command auto-initializes graphrag.sqlite if missing.\n  \
        - As a side effect, `init` warms a smoke-test embedding via the LLM-only one-shot pipeline.")]
    Init(init::InitArgs),
    /// Save a memory with optional entity graph
    #[command(after_long_help = "EXAMPLES:\n  \
        # Inline body\n  \
        sqlite-graphrag remember --name onboarding --type user --description \"intro\" --body \"hello\"\n\n  \
        # Body from file\n  \
        sqlite-graphrag remember --name doc1 --type document --description \"...\" --body-file ./README.md\n\n  \
        # Body from stdin (pipe)\n  \
        cat README.md | sqlite-graphrag remember --name doc1 --type document --description \"...\" --body-stdin\n\n  \
        # Enable automatic URL extraction (URL-regex only since v1.0.79)\n  \
        sqlite-graphrag remember --name rich --type note --description \"...\" --body \"...\" --enable-ner")]
    Remember(remember::RememberArgs),
    /// Batch-create memories from NDJSON stdin (one invocation, one slot)
    #[command(after_long_help = "EXAMPLES:\n  \
        # Batch create from NDJSON\n  \
        cat memories.ndjson | sqlite-graphrag remember-batch --force-merge --json\n\n  \
        # Atomic batch\n  \
        cat memories.ndjson | sqlite-graphrag remember-batch --transaction --json")]
    RememberBatch(remember_batch::RememberBatchArgs),
    /// Bulk-ingest every file under a directory as separate memories (NDJSON output)
    Ingest(Box<ingest::IngestArgs>),
    /// Search memories semantically
    #[command(after_long_help = "EXAMPLES:\n  \
        # Top 10 semantic matches (default)\n  \
        sqlite-graphrag recall \"agent memory\"\n\n  \
        # Top 3 only\n  \
        sqlite-graphrag recall \"agent memory\" -k 3\n\n  \
        # Search across all namespaces\n  \
        sqlite-graphrag recall \"agent memory\" --all-namespaces\n\n  \
        # Disable graph traversal (vector-only)\n  \
        sqlite-graphrag recall \"agent memory\" --no-graph")]
    Recall(recall::RecallArgs),
    /// Read a memory by exact name
    Read(read::ReadArgs),
    /// List memories with filters
    List(list::ListArgs),
    /// Soft-delete a memory
    Forget(forget::ForgetArgs),
    /// Permanently delete soft-deleted memories
    Purge(purge::PurgeArgs),
    /// Rename a memory preserving history
    Rename(rename::RenameArgs),
    /// Split an oversized memory body into N child memories (v1.1.03, GAP-V8)
    SplitBody(split_body::SplitBodyArgs),
    /// Edit a memory's body or description
    Edit(edit::EditArgs),
    /// List all versions of a memory
    History(history::HistoryArgs),
    /// Restore a memory to a previous version
    Restore(restore::RestoreArgs),
    /// Search using hybrid vector + full-text search
    #[command(after_long_help = "EXAMPLES:\n  \
        # Hybrid search combining KNN + FTS5 BM25 with RRF\n  \
        sqlite-graphrag hybrid-search \"agent memory architecture\"\n\n  \
        # Custom weights for vector vs full-text components\n  \
        sqlite-graphrag hybrid-search \"agent\" --weight-vec 0.7 --weight-fts 0.3")]
    HybridSearch(hybrid_search::HybridSearchArgs),
    /// Show database health
    Health(health::HealthArgs),
    /// Apply pending schema migrations
    Migrate(migrate::MigrateArgs),
    /// Resolve namespace precedence for the current invocation
    NamespaceDetect(namespace_detect::NamespaceDetectArgs),
    /// Run PRAGMA optimize on the database
    Optimize(optimize::OptimizeArgs),
    /// Show database statistics
    Stats(stats::StatsArgs),
    /// Create a checkpointed copy safe for file sync
    SyncSafeCopy(sync_safe_copy::SyncSafeCopyArgs),
    /// Back up the database using the SQLite Online Backup API
    Backup(backup::BackupArgs),
    /// Run VACUUM after checkpointing the WAL
    Vacuum(vacuum::VacuumArgs),
    /// Create an explicit relationship between two entities
    Link(link::LinkArgs),
    /// Remove a specific relationship between two entities
    Unlink(unlink::UnlinkArgs),
    /// Deep parallel multi-hop GraphRAG research
    #[command(name = "deep-research")]
    DeepResearch(deep_research::DeepResearchArgs),
    /// List memories connected via the entity graph
    Related(related::RelatedArgs),
    /// Export a graph snapshot in json, dot or mermaid
    Graph(graph_export::GraphArgs),
    /// Export memories as NDJSON (one JSON line per memory, plus a summary line)
    Export(export::ExportArgs),
    /// FTS5 full-text search index management (rebuild or check)
    Fts(fts::FtsArgs),
    /// Vector index maintenance (orphan detection, purge, stats) — G39
    Vec(vec::VecArgs),
    /// Bulk-delete all relationships of a given type (e.g. mentions)
    PruneRelations(prune_relations::PruneRelationsArgs),
    /// Remove NER bindings (memory_entities rows) for an entity or all entities
    #[command(name = "prune-ner")]
    PruneNer(prune_ner::PruneNerArgs),
    /// Inspect and manage cross-process LLM slot semaphore (GAP-004, v1.0.82)
    Slots(slots::SlotsArgs),
    /// Inspect and manage the `remember` checkpoint queue (GAP-001, v1.0.82)
    Pending(pending::PendingArgs),
    /// Health and per-entry inspection of the pending-embeddings queue (GAP-005, v1.0.82)
    Embedding(embedding::EmbeddingArgs),
    /// Batch operations over the pending-embeddings queue (GAP-005, v1.0.82)
    #[command(name = "pending-embeddings")]
    PendingEmbeddings(pending_embeddings::PendingEmbeddingsArgs),
    /// Remove entities that have no memories and no relationships
    CleanupOrphans(cleanup_orphans::CleanupOrphansArgs),
    /// List entities linked to a specific memory
    MemoryEntities(memory_entities::MemoryEntitiesArgs),
    /// Manage cached resources (embedding models, etc.)
    Cache(cache::CacheArgs),
    /// Delete an entity and all its relationships from the graph
    #[command(name = "delete-entity")]
    DeleteEntity(delete_entity::DeleteEntityArgs),
    /// Reclassify one entity or a batch of entities to a new type
    Reclassify(reclassify::ReclassifyArgs),
    /// Rename an entity preserving all relationships and memory bindings
    #[command(name = "rename-entity")]
    RenameEntity(rename_entity::RenameEntityArgs),
    /// Merge multiple source entities into a single target entity
    #[command(name = "merge-entities")]
    MergeEntities(merge_entities::MergeEntitiesArgs),
    /// Enrich graph memories and entities using an LLM provider
    Enrich(Box<enrich::EnrichArgs>),
    /// Reclassify relationship types across the graph using rules or LLM judgment
    #[command(name = "reclassify-relation")]
    ReclassifyRelation(reclassify_relation::ReclassifyRelationArgs),
    /// Normalize entity names (deduplicate, kebab-case, merge near-duplicates)
    #[command(name = "normalize-entities")]
    NormalizeEntities(normalize_entities::NormalizeEntitiesArgs),
    /// Generate shell completions for Bash, Zsh, Fish, PowerShell, or Elvish
    Completions(completions::CompletionsArgs),
    /// List every shipped JSON Schema, or emit one by id (`--name <ID>`)
    #[command(after_long_help = "EXAMPLES:\n  \
        # Catalogue: one NDJSON record per contract\n  \
        sqlite-graphrag schema\n\n  \
        # One contract by id\n  \
        sqlite-graphrag schema --name recall\n\n\
        NOTES:\n  \
        - Never opens the database and never requires an embedding API key.\n  \
        - The per-subcommand `--print-schema` flags keep working unchanged.")]
    Schema(crate::print_schema::SchemaArgs),
    /// `debug-schema` subcommand.
    #[command(name = "debug-schema", hide = true)]
    DebugSchema(debug_schema::DebugSchemaArgs),
    /// Manage API keys and diagnose provider configuration (v1.0.93)
    Config(config_cmd::ConfigArgs),
}

impl Commands {
    /// Names the subcommand for [`crate::agent_surface`] alias suppression.
    ///
    /// The suppression table used to match on the KEY alone, so `results` meant
    /// the same thing everywhere. It does not: in `recall`, `results` really is
    /// the concatenation of `direct_matches` and `graph_matches`, so dropping
    /// the halves loses nothing. In `hybrid-search` the two arrays are DISJOINT
    /// by construction — the graph expansion skips every id already fused — and
    /// they do not even hold the same type. Suppressing there deleted unique
    /// rows and then labelled them redundant, which is worse than losing them
    /// silently: the envelope asserted the removal was safe.
    ///
    /// `None` for every subcommand that declares no alias, which makes the
    /// default fail-safe: a new command is never suppressed until someone adds
    /// it to the table deliberately.
    #[must_use]
    pub fn agent_surface_slug(&self) -> Option<&'static str> {
        match self {
            Self::List(_) => Some("list"),
            Self::Graph(_) => Some("graph"),
            Self::Recall(_) => Some("recall"),
            Self::Related(_) => Some("related"),
            _ => None,
        }
    }

    /// `true` when this subcommand can change durable state.
    ///
    /// GAP-SG-205 reads it to decide whether the target database may be
    /// inherited from ambient configuration; [`crate::agent_surface::gate`]
    /// reads it to decide whether a refusal is still safe.
    ///
    /// The refusal question is the sharper one. The agent-native surface runs at
    /// OUTPUT time, after the handler has already done its work, so refusing
    /// there would hand the caller a non-zero exit for an operation that
    /// succeeded — and a caller that retries a succeeded `remember` writes the
    /// memory twice. The gate therefore stays silent on anything this reports as
    /// mutating.
    ///
    /// Read-only variants are listed EXPLICITLY and everything else answers
    /// `true`. The default has to be the conservative one: a subcommand added
    /// later and forgotten here loses a refusal it might have wanted, which
    /// costs a diagnostic, while the opposite default would let the gate fire
    /// after an unlisted write, which costs data.
    pub fn mutates(&self) -> bool {
        match self {
            Self::Recall(_)
            | Self::Read(_)
            | Self::List(_)
            | Self::History(_)
            | Self::HybridSearch(_)
            | Self::Health(_)
            | Self::NamespaceDetect(_)
            | Self::Stats(_)
            | Self::DeepResearch(_)
            | Self::Related(_)
            | Self::Export(_)
            | Self::MemoryEntities(_)
            | Self::Schema(_)
            | Self::DebugSchema(_)
            | Self::Completions(_) => false,
            // `graph` is read-only in three of its four forms; `recompute-degree`
            // rewrites the cached degree column.
            Self::Graph(args) => matches!(
                args.subcommand,
                Some(crate::commands::graph_export::GraphSubcommand::RecomputeDegree(_))
            ),
            _ => true,
        }
    }

    /// Whether this subcommand may resolve its target from ambient configuration.
    ///
    /// GAP-SG-207. [`Self::mutates`] answers "does this change durable state";
    /// this answers "is naming the target nonetheless optional for THIS
    /// invocation". The two differ, and reusing `mutates` alone would have been
    /// a defect: it lists the read-only variants explicitly and answers `true`
    /// for everything else, which is the right conservative default for the
    /// output-time refusal fence and the WRONG one here. For the fence a
    /// mistaken `true` costs a diagnostic; here it would cost a false refusal on
    /// a command that has no side effect to protect — `fts check`, `vec stats`,
    /// `pending list` and `embedding status` all read and write nothing.
    ///
    /// So the families whose subcommands split between reading and writing are
    /// classified at the SUBCOMMAND level. The Explicit Target Designation rule
    /// governs side effects, and a read inherits no authority it could misuse.
    ///
    /// Enforcement lives in [`crate::paths::AppPaths::resolve`]. That placement
    /// keeps this list short: a subcommand that never resolves a database —
    /// `config`, `completions`, `locale`, `slots`, `cache` — is exempt by
    /// construction and needs no entry here at all.
    pub fn may_inherit_target(&self) -> bool {
        use crate::commands::embedding::EmbeddingCmd;
        use crate::commands::fts::FtsSubcommand;
        use crate::commands::pending::PendingCmd;
        use crate::commands::pending_embeddings::PendingEmbeddingsCmd;
        use crate::commands::vec::VecSubcommand;

        match self {
            // Creating the XDG database when no `--db` is given IS the command,
            // so requiring the flag would invert `init` rather than protect it.
            Self::Init(_) => true,
            // Host leaves. GAP-SG-139 fixed these to accept `--db` as a no-op
            // precisely because they touch no database — but they still call
            // `AppPaths::resolve` to locate the MODELS directory, which shares
            // that resolver. Without this arm the target policy fired on
            // `cache list`, a command that reads a cache and nothing else.
            Self::Config(_) | Self::Cache(_) | Self::Slots(_) | Self::Completions(_) => true,
            Self::Fts(args) => matches!(
                args.command,
                FtsSubcommand::Check(_) | FtsSubcommand::Stats(_)
            ),
            Self::Vec(args) => matches!(
                args.command,
                VecSubcommand::OrphanList(_) | VecSubcommand::Stats(_)
            ),
            Self::Pending(args) => {
                matches!(args.cmd, PendingCmd::List(_) | PendingCmd::Show(_))
            }
            Self::Embedding(args) => {
                matches!(args.cmd, EmbeddingCmd::List(_) | EmbeddingCmd::Status(_))
            }
            Self::PendingEmbeddings(args) => matches!(
                args.cmd,
                PendingEmbeddingsCmd::List(_) | PendingEmbeddingsCmd::Status(_)
            ),
            _ => false,
        }
    }

    /// Returns true for subcommands that load the ONNX model locally.
    pub fn is_embedding_heavy(&self) -> bool {
        matches!(
            self,
            Self::Init(_)
                | Self::Remember(_)
                | Self::RememberBatch(_)
                | Self::Recall(_)
                | Self::HybridSearch(_)
                | Self::DeepResearch(_)
        )
    }

    /// Return whether this command occupies a CLI concurrency slot.
    pub fn uses_cli_slot(&self) -> bool {
        true
    }

    /// Read-only / no-embedding subcommands that MUST run without an embedding
    /// API key. `init` warms a best-effort smoke test internally and degrades to
    /// `ok_no_embedding` when the backend is unreachable; the `enrich` queue
    /// inspectors (`--status` / `--list-dead` / `--requeue-dead` /
    /// `--prune-dead-orphans`) never embed and never call the LLM. The eager
    /// OpenRouter key preflight in `main` must skip its hard-fail for these.
    pub fn tolerates_missing_embedding_key(&self) -> bool {
        match self {
            Self::Init(_) => true,
            // `schema` only writes embedded documents: no database, no key.
            Self::Schema(_) => true,
            // The host leaves must never be gated by embedding configuration:
            // they are the only way to REPAIR that configuration. Registering
            // `embedding.backend` (v1.2.5, GAP-SG-198) made the omission
            // load-bearing — `config set embedding.backend openrouter` with no
            // model stored started failing every later invocation at the
            // preflight, `config unset` included, leaving hand-editing the TOML
            // as the only exit. `cache`, `slots` and `completions` never embed
            // either, so the same reasoning covers them.
            Self::Config(_) | Self::Cache(_) | Self::Slots(_) | Self::Completions(_) => true,
            Self::Enrich(args) => {
                args.status
                    || args.list_dead
                    || args.requeue_dead
                    || args.list_skipped
                    || args.requeue_skipped
                    || args.prune_dead_orphans
                    || args.prune_dead_entity_orphans
                    || args.print_schema
            }
            _ => false,
        }
    }
}

// FIX-1 (v1.0.89): manual `Debug` impl so test panic messages that print
// `{:?}` on a captured `Commands` variant compile without requiring every
// contained subcommand arg struct to derive `Debug`. The Debug output is
// only used in test assertions for diagnostic messages; we emit the variant
// name only — arg payload is intentionally omitted.
impl std::fmt::Debug for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Init(_) => "Init",
            Self::Health(_) => "Health",
            Self::Stats(_) => "Stats",
            Self::List(_) => "List",
            Self::Read(_) => "Read",
            Self::Edit(_) => "Edit",
            Self::Rename(_) => "Rename",
            Self::SplitBody(_) => "SplitBody",
            Self::Restore(_) => "Restore",
            Self::History(_) => "History",
            Self::Forget(_) => "Forget",
            Self::Purge(_) => "Purge",
            Self::Remember(_) => "Remember",
            Self::RememberBatch(_) => "RememberBatch",
            Self::Recall(_) => "Recall",
            Self::HybridSearch(_) => "HybridSearch",
            Self::Enrich(_) => "Enrich",
            Self::Ingest(_) => "Ingest",
            Self::Optimize(_) => "Optimize",
            Self::Migrate(_) => "Migrate",
            Self::SyncSafeCopy(_) => "SyncSafeCopy",
            Self::Backup(_) => "Backup",
            Self::Vacuum(_) => "Vacuum",
            Self::Link(_) => "Link",
            Self::Unlink(_) => "Unlink",
            Self::DeepResearch(_) => "DeepResearch",
            Self::Related(_) => "Related",
            Self::Graph(_) => "Graph",
            Self::Export(_) => "Export",
            Self::Fts(_) => "Fts",
            Self::Vec(_) => "Vec",
            Self::PruneRelations(_) => "PruneRelations",
            Self::PruneNer(_) => "PruneNer",
            Self::Slots(_) => "Slots",
            Self::Pending(_) => "Pending",
            Self::Embedding(_) => "Embedding",
            Self::PendingEmbeddings(_) => "PendingEmbeddings",
            Self::CleanupOrphans(_) => "CleanupOrphans",
            Self::MemoryEntities(_) => "MemoryEntities",
            Self::Cache(_) => "Cache",
            Self::DeleteEntity(_) => "DeleteEntity",
            Self::Reclassify(_) => "Reclassify",
            Self::RenameEntity(_) => "RenameEntity",
            Self::ReclassifyRelation(_) => "ReclassifyRelation",
            Self::NormalizeEntities(_) => "NormalizeEntities",
            Self::MergeEntities(_) => "MergeEntities",
            Self::NamespaceDetect(_) => "NamespaceDetect",
            Self::Completions(_) => "Completions",
            Self::Schema(_) => "Schema",
            Self::DebugSchema(_) => "DebugSchema",
            Self::Config(_) => "Config",
        };
        f.write_str(name)
    }
}
