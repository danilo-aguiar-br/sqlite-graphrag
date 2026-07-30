//! CLI argument structs and command surface (clap-based).
//!
//! Defines `Cli` and all subcommand enums; contains no business logic.

use crate::commands::*;
use crate::i18n::{current, Language};
use clap::{Parser, Subcommand};

/// Returns the maximum simultaneous invocations allowed by the CPU heuristic.
fn max_concurrency_ceiling() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get() * 2)
        .unwrap_or(8)
}

/// Graph export format.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum GraphExportFormat {
    /// JSON variant.
    Json,
    /// DOT variant.
    Dot,
    /// Mermaid variant.
    Mermaid,
    /// Stream one JSON object per entity, then one per edge, then a summary line.
    Ndjson,
}

// Backend choice enums live in `backend_choice` (Wave C1).
pub use crate::backend_choice::{EmbeddingBackendChoice, LlmBackendChoice};

#[derive(Parser)]
#[command(name = "sqlite-graphrag")]
#[command(version)]
#[command(about = "Local GraphRAG memory for LLMs in a single SQLite file")]
#[command(arg_required_else_help = true)]
#[command(after_help = "DATABASE PATH (GAP-SG-32):\n  \
    `--db` is a PER-SUBCOMMAND flag, so it must come AFTER the subcommand:\n    \
    sqlite-graphrag remember --db ./graphrag.sqlite --name mem --type note ...\n  \
    Placing it before the subcommand (e.g. `sqlite-graphrag --db x.sqlite remember`) is rejected.\n  \
    Prefer `--db` on every invocation (one-shot agents). Optional XDG defaults:\n    \
    `sqlite-graphrag config set db.path ./graphrag.sqlite`\n  \
    Product environment variables are not read at runtime; use flags + `config set/get`.")]
/// CLI.
pub struct Cli {
    /// Maximum number of simultaneous CLI invocations allowed (default: 4).
    ///
    /// Caps the counting semaphore used for CLI concurrency slots. The value must
    /// stay within [1, 2×nCPUs]. Values above the ceiling are rejected with exit 2.
    #[arg(long, global = true, value_name = "N")]
    pub max_concurrency: Option<usize>,

    /// Wait up to SECONDS for a free concurrency slot before giving up (exit 75).
    ///
    /// Useful in retrying agent pipelines: the process polls every 500 ms until a
    /// slot opens or the timeout expires. Default: 300s (5 minutes).
    #[arg(long, global = true, value_name = "SECONDS")]
    pub wait_lock: Option<u64>,

    /// Skip the available-memory check before loading the model.
    ///
    /// Exclusive use in automated tests where real allocation does not occur.
    #[arg(long, global = true, hide = true, default_value_t = false)]
    pub skip_memory_guard: bool,

    /// v1.0.83 (ADR-0041): strict env-clear mode for compliance environments.
    ///
    /// When enabled, the LLM subprocess receives ONLY `PATH` — no
    /// `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`
    /// or other custom-provider credentials are forwarded. Defaults to
    /// the standard v1.0.83 whitelist that preserves custom-provider
    /// credentials (ADR-0041). Prefer the flag; optional XDG
    /// `spawn.strict_env_clear=1` via `config set`.
    #[arg(
        long,
        global = true,
        hide = true,
        default_value_t = false,
        value_parser = clap::builder::BoolishValueParser::new(),
            )]
    pub strict_env_clear: bool,

    /// v1.0.84 (ADR-0042 / GAP-002): resolve and print the LLM backend that
    /// WOULD be invoked for embedding (binary path + model + flavour),
    /// then exit 0 without executing the subprocess. Useful for CI
    /// audit and sanity-check of `--llm-backend` before long sessions.
    ///
    /// Prefer the flag; optional XDG `llm.dry_run_backend=1` via `config set`.
    #[arg(
        long,
        global = true,
        hide = true,
        default_value_t = false,
        value_parser = clap::builder::BoolishValueParser::new(),
            )]
    pub dry_run_backend: bool,

    /// Language for human-facing stderr messages. Accepts `en` or `pt`.
    ///
    /// Without the flag, detection uses XDG `i18n.lang` then OS locale
    /// (`LC_ALL`/`LC_MESSAGES`/`LANG`). JSON stdout stays deterministic and
    /// identical across languages; only human-facing strings are affected.
    #[arg(long, global = true, value_enum, value_name = "LANG")]
    pub lang: Option<crate::i18n::Language>,

    /// Time zone for `*_iso` fields in JSON output (for example `America/Sao_Paulo`).
    ///
    /// Accepts any IANA time zone name. Without the flag, it falls back to
    /// XDG `display.tz`; if unset, UTC is used. Integer epoch fields
    /// are not affected.
    #[arg(long, global = true, value_name = "IANA")]
    pub tz: Option<chrono_tz::Tz>,

    /// Directory holding `config.toml`. Overrides the OS config directory.
    ///
    /// Precedence (G-T-XDG-04): this flag > OS default. It deliberately does
    /// NOT consult a `config set` key, because the config file itself lives in
    /// this directory and reading it to find itself would be circular.
    /// Hidden: it exists for hermetic test isolation and sandboxed hosts.
    #[arg(long, global = true, hide = true, value_name = "DIR")]
    pub config_dir: Option<std::path::PathBuf>,

    /// Directory for lock files, model files and other cache artifacts.
    ///
    /// Precedence (G-T-XDG-04): this flag > XDG `cache.dir` > OS default.
    /// Hidden for the same reason as `--config-dir`.
    #[arg(long, global = true, hide = true, value_name = "DIR")]
    pub cache_dir: Option<std::path::PathBuf>,

    /// Increase logging verbosity (-v=info, -vv=debug, -vvv=trace).
    ///
    /// Overrides XDG `log.level` when present. Logs are emitted
    /// to stderr; JSON stdout is unaffected.
    #[arg(short = 'v', long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress non-error tracing on stderr (sets log level to `error`).
    ///
    /// Prefer this in pipelines that capture stdout JSON (`> out.json`).
    /// Never combine stdout and stderr into the same file (`&>` / `2>&1`) —
    /// that contaminates the JSON envelope (v1.1.05 Bug 2). Conflicts with
    /// `-v` / `--verbose` only in spirit: quiet wins when both are present.
    #[arg(short = 'q', long, global = true, default_value_t = false)]
    pub quiet: bool,

    /// v1.0.75 (G21 solution): extraction backend selector. Accepts
    /// `llm` (default), `embedding` (legacy), `none`, or `both` (composite).
    /// The `llm` backend invokes claude code / codex CLI headless to extract
    /// entities and relationships; `embedding` is a permanent stub since
    /// v1.0.79 (legacy fastembed pipeline removed) that returns a clear
    /// migration error.
    #[arg(long, global = true, value_name = "KIND", default_value = "llm")]
    pub extraction_backend: Option<String>,

    /// Embedding dimensionality override (default 1024 since v1.2.0).
    ///
    /// Precedence: this flag > XDG `embedding.dim` >
    /// the `dim` recorded in the database `schema_meta` > 1024. Existing
    /// databases keep their recorded dimensionality automatically; use
    /// this flag only to migrate a corpus to a new dimensionality
    /// (followed by `enrich --operation re-embed`). Range: [8, 4096].
    #[arg(long, global = true, value_name = "N", value_parser = clap::value_parser!(u64).range(8..=4096))]
    pub embedding_dim: Option<u64>,

    /// v1.0.82 (GAP-003) / v1.0.84 (ADR-0042): LLM backend for embedding.
    /// Accepts `auto` (detects via PATH, codex-first), `codex` (forces
    /// `codex exec`), `claude` (forces `claude -p`; since v1.0.84 does NOT fall back to
    /// codex — emits `AppError::Validation` if `claude` is absent),
    /// `opencode` (forces `opencode run`), or `none`
    /// (skips embedding; useful for tests). Prefer the flag; optional XDG
    /// XDG `llm.backend` via `config set`.
    #[arg(long, global = true, value_enum, default_value_t = LlmBackendChoice::Auto)]
    pub llm_backend: LlmBackendChoice,

    /// v1.0.82 (GAP-003): model to invoke on the chosen backend.
    /// Prefer the flag; optional XDG `llm.model`. The default depends
    /// on the backend (codex: `gpt-5.5`; claude: `claude-sonnet-4-6`).
    #[arg(
        long,
        global = true,
        value_name = "MODEL",
            )]
    pub llm_model: Option<String>,

    /// v1.0.82 (GAP-003): path to the `claude` binary (overrides
    /// PATH detection). Prefer the flag; optional XDG `llm.claude_binary`.
    #[arg(
        long,
        global = true,
        value_name = "PATH",
            )]
    pub claude_binary: Option<std::path::PathBuf>,

    /// v1.0.89 (GAP-1): path to the `codex` binary (overrides
    /// PATH detection). Prefer the flag; optional XDG `llm.codex_binary`.
    #[arg(
        long,
        global = true,
        value_name = "PATH",
            )]
    pub codex_binary: Option<std::path::PathBuf>,

    /// v1.0.90 (GAP-OPENCODE-001): path to the `opencode` binary (overrides
    /// PATH detection). Prefer the flag; optional XDG `llm.opencode_binary`.
    #[arg(
        long,
        global = true,
        value_name = "PATH",
            )]
    pub opencode_binary: Option<std::path::PathBuf>,

    /// v1.0.82 (GAP-005): chain of LLM backends tried in order
    /// when the primary fails. Default `codex,claude,none`. Prefer the
    /// flag; optional XDG `llm.fallback`.
    #[arg(
        long,
        global = true,
        default_value = "codex,claude,none",
            )]
    pub llm_fallback: String,

    /// v1.0.82 (GAP-005): persists with a NULL embedding when all
    /// backends in the chain fail. The memory stays in `pending_embeddings`
    /// for reprocessing via `embedding retry`. Prefer the flag; optional XDG
    /// XDG `llm.skip_embedding_on_failure`.
    #[arg(
        long,
        global = true,
        default_value_t = false,
        value_parser = clap::builder::BoolishValueParser::new(),
            )]
    pub skip_embedding_on_failure: bool,

    /// v1.0.82 (GAP-004): host-wide limit of concurrent LLM
    /// subprocesses. Default derived from `ncpus`. Prefer the flag; optional XDG
    /// XDG `llm.max_host_concurrency`.
    #[arg(
        long,
        global = true,
        value_name = "N",
            )]
    pub llm_max_host_concurrency: Option<u32>,

    /// v1.0.82 (GAP-004): seconds to wait for a free LLM slot
    /// before failing with exit 75. Default 30s. Prefer the flag; optional XDG
    /// XDG `llm.slot_wait_secs`.
    #[arg(
        long,
        global = true,
        value_name = "SECONDS",
            )]
    pub llm_slot_wait_secs: Option<u64>,

    /// v1.0.82 (GAP-004): if set, fails immediately (exit 75)
    /// when no LLM slot is free. Prefer the flag; optional XDG
    /// XDG `llm.slot_no_wait`.
    #[arg(
        long,
        global = true,
        default_value_t = false,
        value_parser = clap::builder::BoolishValueParser::new(),
            )]
    pub llm_slot_no_wait: bool,

    /// v1.0.93: embedding backend selector. `auto` tries OpenRouter API if key
    /// available, falls back to LLM subprocess. `openrouter` requires API key.
    /// `llm` forces subprocess. Prefer the flag; optional XDG `embedding.backend`.
    #[arg(long, global = true, value_enum, default_value_t = EmbeddingBackendChoice::Auto)]
    pub embedding_backend: EmbeddingBackendChoice,

    /// v1.0.93: embedding model for the OpenRouter API. Required when
    /// `--embedding-backend openrouter`. Prefer the flag; optional XDG `embedding.model`.
    #[arg(
        long,
        global = true,
        value_name = "MODEL",
            )]
    pub embedding_model: Option<String>,

    /// v1.0.93: OpenRouter API key (prefer env var or config.toml over CLI flag
    /// to avoid shell history exposure). Prefer `config set-key openrouter`.
    #[arg(
        long,
        global = true,
        value_name = "KEY",
        hide = true,
                hide_env_values = true
    )]
    pub openrouter_api_key: Option<String>,

    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[cfg(test)]
#[path = "cli_json_only_format_tests.rs"]
mod json_only_format_tests;


impl Cli {
    /// Validates concurrency flags and returns a localised descriptive error if invalid.
    ///
    /// Requires that `crate::i18n::init()` has already been called (happens before this
    /// function in the `main` flow). In English it emits EN messages; in Portuguese it emits PT.
    pub fn validate_flags(&self) -> Result<(), String> {
        if let Some(n) = self.max_concurrency {
            if n == 0 {
                return Err(match current() {
                    Language::English => "--max-concurrency must be >= 1".to_string(),
                    Language::Portuguese => "--max-concurrency deve ser >= 1".to_string(),
                });
            }
            let teto = max_concurrency_ceiling();
            if n > teto {
                return Err(match current() {
                    Language::English => format!(
                        "--max-concurrency {n} exceeds the ceiling of {teto} (2×nCPUs) on this system"
                    ),
                    Language::Portuguese => format!(
                        "--max-concurrency {n} excede o teto de {teto} (2×nCPUs) neste sistema"
                    ),
                });
            }
        }
        Ok(())
    }
}

impl Commands {
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

/// GAP-E2E-010 (v1.0.89): `codex-models` accepts `--json` as a no-op so
/// agents that append `--json` to every subcommand never see clap errors.
/// The handler in `main.rs` always emits JSON on stdout; this flag is
/// accepted and ignored for parity with the rest of the CLI surface.
///
/// GAP-SG-139: also accepts `--db` as a no-op for agent uniformity (host surface).
#[derive(Debug, clap::Args)]
pub struct CodexModelsArgs {
    /// No-op; JSON is always emitted on stdout by `codex-models`.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// GAP-SG-139: accepted as a no-op for agent uniformity (no graph I/O).
    #[command(flatten)]
    pub db_noop: crate::cli_db_noop::DbNoopArgs,
}

/// Commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Initialize database and download embedding model
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
    /// List codex OAuth models accepted by ChatGPT Pro (G33).
    ///
    /// GAP-E2E-010 (v1.0.89): accepts `--json` as a no-op (JSON is always
    /// emitted on stdout) so the flag never breaks agent pipelines that
    /// append `--json` to every invocation.
    #[command(name = "codex-models")]
    CodexModels(CodexModelsArgs),
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
    /// `debug-schema` subcommand.
    #[command(name = "debug-schema", hide = true)]
    DebugSchema(debug_schema::DebugSchemaArgs),
    /// Manage API keys and diagnose provider configuration (v1.0.93)
    Config(config_cmd::ConfigArgs),
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
            Self::CodexModels(_) => "CodexModels",
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
            Self::DebugSchema(_) => "DebugSchema",
            Self::Config(_) => "Config",
        };
        f.write_str(name)
    }
}

/// Memory type.
#[derive(Copy, Clone, Debug, Default, clap::ValueEnum)]
pub enum MemoryType {
    /// User variant.
    User,
    /// Feedback variant.
    Feedback,
    /// Project variant.
    Project,
    /// Reference variant.
    Reference,
    /// Decision variant.
    Decision,
    /// Incident variant.
    Incident,
    /// Skill variant.
    Skill,
    /// Document variant.
    #[default]
    Document,
    /// Note variant.
    Note,
}

#[cfg(test)]
#[path = "cli_heavy_concurrency_tests.rs"]
mod heavy_concurrency_tests;


impl MemoryType {
    /// Return the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
            Self::Decision => "decision",
            Self::Incident => "incident",
            Self::Skill => "skill",
            Self::Document => "document",
            Self::Note => "note",
        }
    }
}

/// GAP-SG-31/33/34/35/30: parse-time contracts for the Fase G clap fixes.
#[cfg(test)]
#[path = "cli_fase_g_parsing_tests.rs"]
mod fase_g_parsing_tests;
