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
        sqlite-graphrag remember --name rich --type note --description \"...\" --body \"...\" --enable-ner\n\n  \
        # Positional name, the same form edit/read/forget/history accept\n  \
        sqlite-graphrag remember onboarding --type user --description \"intro\" --body \"hello\"\n\n\
        NOTES:\n  \
        - The name may be given positionally OR via --name, never both.\n  \
        - Pick exactly one body source: --body, --body-file, --body-stdin or --graph-stdin.\n  \
        - --graph-file combines with any of the first three.\n\n\
        ENTITY TYPES (for graph entities, NOT the memory --type):\n  \
        The entity vocabulary is OPEN since v1.2.8: any label is stored as\n  \
        you write it, and none is ever rewritten into another.\n  \
        RECOMMENDED labels: concept, tool, person, file, project, decision,\n  \
        incident, organization, location, date, dashboard, issue_tracker,\n  \
        memory. A label outside them is accepted and reported in the\n  \
        response `warnings` array, never substituted.\n  \
        Pass --strict-entity-types to refuse anything outside the thirteen.\n  \
        Shape is still checked: a label cannot be empty, digits only,\n  \
        contain a line break, or exceed 64 characters.\n  \
        The memory --type is a DIFFERENT and CLOSED vocabulary (user,\n  \
        feedback, project, reference, decision, incident, skill, document,\n  \
        note); the three names in both mean different things.\n  \
        Inspect what your database actually uses: sqlite-graphrag graph entity-types\n  \
        Wire contract: sqlite-graphrag schema --name graph-input")]
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
    /// GAP-SG-274: `graph` reports TWO slugs, because it emits two shapes.
    ///
    /// The NDJSON snapshot emits one self-contained record per line, discriminated
    /// by a `kind` field valued `node`, `edge` or `summary`; every other form of
    /// `graph` emits a single envelope in which `kind` is instead the deprecated
    /// alias of an entity's `type`. One slug for both made the vocabulary layer
    /// blind to a distinction [`Self::streams`] was already computing three
    /// methods away, which is why a field name meaning two different things had
    /// to be excluded everywhere rather than scoped where it is unambiguous.
    #[must_use]
    pub fn agent_surface_slug(&self) -> Option<&'static str> {
        match self {
            Self::List(_) => Some("list"),
            Self::Graph(args) => Some(if Self::is_graph_ndjson(args) {
                "graph-ndjson"
            } else {
                "graph"
            }),
            Self::Recall(_) => Some("recall"),
            Self::Related(_) => Some("related"),
            _ => None,
        }
    }

    /// `true` when this invocation is the NDJSON snapshot form of `graph`.
    ///
    /// GAP-SG-274. Both [`Self::streams`] and [`Self::agent_surface_slug`] need
    /// this answer, and before it was named they disagreed: one computed it, the
    /// other ignored the args entirely. Asking the question in one place is what
    /// stops the two from drifting apart again — the same argument the
    /// [`Self::persists`] conjunction records for its own pair.
    fn is_graph_ndjson(args: &graph_export::GraphArgs) -> bool {
        !args.json
            && args.subcommand.is_none()
            && args.format == crate::cli::GraphExportFormat::Ndjson
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
            // `graph` is read-only in four of its five forms; `recompute-degree`
            // rewrites the cached degree column.
            Self::Graph(args) => matches!(
                args.subcommand,
                Some(crate::commands::graph_export::GraphSubcommand::RecomputeDegree(_))
            ),
            _ => true,
        }
    }

    /// `true` when this subcommand actually persists, so its envelope is a receipt.
    ///
    /// GAP-SG-206. Neither half answers this on its own. [`Self::mutates`] lists
    /// the read-only variants explicitly and answers `true` for all the rest, so
    /// it reports `true` for `config list-keys`, `embedding list` and `fts check`,
    /// which write nothing — right default for a refusal fence, wrong one for
    /// withholding an answer. [`Self::may_inherit_target`] classifies exactly
    /// those split families at the subcommand level.
    ///
    /// The conjunction is not a new list: `Cli::install_write_policy` already
    /// asks precisely this question to decide whether the target must be named
    /// in the argv, which is the same question — "did something get written".
    /// Naming it once is what stops a third hand-written copy from drifting away
    /// from the other two.
    ///
    /// That hook is a plain code span rather than an intra-doc link because it is
    /// private, and `rustdoc::private_intra_doc_links` — denied in `Cargo.toml`
    /// — rejects a link from public documentation to an item the public
    /// documentation does not contain. `tests/rustdoc_link_gate.rs` now catches
    /// that class, which `cargo test` and `cargo clippy` are both blind to.
    pub fn persists(&self) -> bool {
        self.mutates() && !self.may_inherit_target()
    }

    /// `true` when this subcommand emits one self-contained record per line.
    ///
    /// GAP-SG-209. [`crate::agent_surface::gate`] reads it to refuse the knobs
    /// that need a complete set, because the surface runs once per emitted
    /// envelope and a stream has no complete set by construction. Measured:
    /// `--count-only export --limit 10` answered with eleven `{"count":1}` lines.
    ///
    /// The property belongs to the SUBCOMMAND and not to the emitting function,
    /// which is the distinction that makes this a list rather than a flag on
    /// `emit_json_compact`. That function is also how `config path`, `slots
    /// release` and `embedding list` emit ONE envelope; keying the refusal off it
    /// would have rejected `--count-only config path`, which is perfectly
    /// answerable.
    ///
    /// Streaming variants are listed EXPLICITLY and everything else answers
    /// `false`. The conservative default is the opposite of [`Self::mutates`]
    /// here, and deliberately so: a subcommand added later and forgotten keeps
    /// exactly today's behaviour, while the opposite default would refuse flags
    /// on a command that can honour them perfectly well.
    ///
    /// GAP-SG-229 added `graph --format ndjson`, which had been streaming since
    /// v1.0.35 without ever answering `true` here. The consequence was worse than
    /// a missing refusal: `render_ndjson_streaming` returns before the surface
    /// layer runs, so `--select`, `--filter`, `--sort` and `--dedupe-by` were
    /// ACCEPTED and then IGNORED, with no refusal and no warning — the exact
    /// shape of "flag aceita e silenciosamente ignorada" this project catalogues.
    ///
    /// The format has to be read from `args`, and it can be: this predicate
    /// matches on the parsed arguments exactly as [`Self::mutates`] already does
    /// for `graph recompute-degree`. GAP-SG-274 gave
    /// [`Self::agent_surface_slug`] the same reach through the shared
    /// `is_graph_ndjson` helper, so the two no longer disagree about which shape
    /// of `graph` is in front of them. That helper is a plain code span rather
    /// than an intra-doc link because it is private, and
    /// `rustdoc::private_intra_doc_links` — denied in `Cargo.toml` — rejects a
    /// link from public documentation to an item the public documentation does
    /// not contain, exactly as [`Self::persists`] records for its own hook. The
    /// `--json` override is mirrored from
    /// `graph_export::handlers`, where it promotes the format to `Json` and turns
    /// the streaming path off entirely; forgetting it here would refuse
    /// whole-set knobs on an invocation that emits a single envelope.
    ///
    /// `dot` and `mermaid` stay outside: they are rendered text, not JSON, so
    /// there is no record for a knob to act on.
    pub fn streams(&self) -> bool {
        match self {
            Self::Export(_) | Self::Ingest(_) => true,
            Self::Graph(args) => Self::is_graph_ndjson(args),
            _ => false,
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
    /// `embedding list` and `embedding status` all read and write nothing.
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
