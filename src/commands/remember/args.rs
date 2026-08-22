//! CLI arguments for the `remember` command.

use crate::cli::MemoryType;
use crate::output::JsonOutputFormat;

#[derive(clap::Args)]
/// Remember args.
///
/// GAP-SG-216: this struct carried an `after_long_help` block from v1.0.x until
/// v1.2.8 and NONE of it ever reached a terminal. `Commands::Remember` in
/// `crate::cli::commands` declares the same attribute on the enum variant, and
/// clap keeps that one — silently, with no warning and no lint. The dead block
/// is where the false claim "reference, skill, document, note, user, feedback
/// are MEMORY types only — NOT valid for entities" survived two minor releases
/// after v1.1.8 made the parser accept all six.
///
/// The authoritative block now lives beside the variant, where it renders, and
/// `tests/remember_input_contract_gate.rs` reads THAT text. Do not add a second
/// `after_long_help` here: a help attribute nothing renders is prose that can
/// only rot.
pub struct RememberArgs {
    /// GAP-SG-216: memory name as a positional argument, the same form `edit`,
    /// `read`, `forget`, `history`, `related`, `rename`, `restore` and
    /// `memory-entities` already accept. `remember` alone refused it, and the
    /// refusal arrived as clap's generic `unexpected argument`, which names
    /// neither `--name` nor the reason.
    #[arg(
        value_name = "NAME",
        conflicts_with = "name",
        help = "Memory name in kebab-case; alternative to --name"
    )]
    pub name_positional: Option<String>,
    /// Memory name in kebab-case (lowercase letters, digits, hyphens).
    /// Acts as unique key within the namespace; collisions trigger merge or rejection.
    ///
    /// Optional at the clap layer only because the positional form is the
    /// alternative; `super::name::resolve` refuses when NEITHER is given.
    #[arg(long)]
    pub name: Option<String>,
    #[arg(
        long,
        value_enum,
        long_help = "Memory kind stored in `memories.type`. Required when creating a new memory. Optional with --force-merge: if omitted the existing memory type is inherited. This is NOT the graph `entity_type` used in `--entities-file`. Valid values: user, feedback, project, reference, decision, incident, skill, document, note."
    )]
    /// Item.
    pub r#type: Option<MemoryType>,
    /// Short description (≤500 chars) summarizing the memory for use in `list` and `recall` snippets.
    /// Required when creating a new memory. Optional with --force-merge: if omitted the existing description is inherited.
    ///
    /// GAP-SG-33: `allow_hyphen_values` lets a description that begins with a
    /// hyphen (e.g. `"- bullet"`) be accepted as a value instead of being
    /// mistaken for a flag.
    #[arg(long, allow_hyphen_values = true)]
    pub description: Option<String>,
    /// Inline body content. Mutually exclusive with --body-file, --body-stdin, --graph-stdin.
    /// Maximum 512000 bytes; rejected if empty without an external graph.
    ///
    /// GAP-SG-33: `allow_hyphen_values` lets a body that begins with a hyphen
    /// (e.g. a markdown bullet list) be accepted as a value.
    #[arg(
        long,
        allow_hyphen_values = true,
        help = "Inline body content (max 500 KB / 512000 bytes and 30000 estimated tokens; split dense bodies into multiple memories at ~25000 tokens, or use --body-file)",
        conflicts_with_all = ["body_file", "body_stdin", "graph_stdin"]
    )]
    pub body: Option<String>,
    #[arg(
        long,
        help = "Read body from a file instead of --body",
        conflicts_with_all = ["body", "body_stdin", "graph_stdin"]
    )]
    /// Body file.
    pub body_file: Option<std::path::PathBuf>,
    /// Read body from stdin until EOF. Useful in pipes (echo "..." | sqlite-graphrag remember ...).
    /// Mutually exclusive with --body, --body-file, --graph-stdin.
    #[arg(
        long,
        conflicts_with_all = ["body", "body_file", "graph_stdin"]
    )]
    pub body_stdin: bool,
    #[arg(
        long,
        help = "JSON file containing entities to associate with this memory"
    )]
    /// Entities file.
    pub entities_file: Option<std::path::PathBuf>,
    #[arg(
        long,
        help = "JSON file containing relationships to associate with this memory"
    )]
    /// Relationships file.
    pub relationships_file: Option<std::path::PathBuf>,
    #[arg(
        long,
        help = "Read graph JSON (body + entities + relationships) from stdin",
        conflicts_with_all = [
            "body",
            "body_file",
            "body_stdin",
            "entities_file",
            "relationships_file",
            "graph_file"
        ]
    )]
    /// Graph stdin.
    pub graph_stdin: bool,
    /// GAP-SG-30: read graph JSON (`{body, entities, relationships}`) from a
    /// FILE instead of stdin, so a curated graph can combine with a body
    /// supplied via --body / --body-file / --body-stdin (which previously
    /// conflicted with --graph-stdin over the single stdin). The file's `body`
    /// field is used only when no other body source is given; otherwise the
    /// body source wins and only the file's entities/relationships are applied.
    #[arg(
        long,
        value_name = "PATH",
        help = "Read graph JSON (body + entities + relationships) from a file (combines with --body/--body-file/--body-stdin)",
        conflicts_with_all = ["graph_stdin", "entities_file", "relationships_file"]
    )]
    pub graph_file: Option<std::path::PathBuf>,
    #[arg(long, help = "Namespace (flag / XDG namespace.default / global)")]
    /// Namespace scope.
    pub namespace: Option<String>,
    /// Inline JSON object with arbitrary metadata key-value pairs. Mutually exclusive with --metadata-file.
    #[arg(long)]
    pub metadata: Option<String>,
    /// Metadata file.
    #[arg(long, help = "JSON file containing metadata key-value pairs")]
    pub metadata_file: Option<std::path::PathBuf>,
    /// Force merge.
    #[arg(long)]
    pub force_merge: bool,
    #[arg(
        long,
        value_name = "EPOCH_OR_RFC3339",
        value_parser = crate::parsers::parse_expected_updated_at,
        long_help = "Optimistic lock: reject if updated_at does not match. \
Accepts Unix epoch (e.g. 1700000000) or RFC 3339 (e.g. 2026-04-19T12:00:00Z)."
    )]
    /// Expected updated at.
    pub expected_updated_at: Option<i64>,
    #[arg(
        long,
                value_parser = crate::parsers::parse_bool_flexible,
        action = clap::ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value = "false",
        help = "Enable automatic URL-regex extraction from body (URL-regex only since v1.0.79)"
    )]
    /// Enable NER.
    pub enable_ner: bool,
    /// Skip extraction.
    #[arg(long, hide = true)]
    pub skip_extraction: bool,
    /// Explicitly clear the body content (set to empty string). Required to distinguish
    /// intentional body clearing from accidental omission during --force-merge.
    /// Without this flag, an empty body passed to --force-merge preserves the existing body.
    #[arg(
        long,
        default_value_t = false,
        help = "Explicitly clear body content during --force-merge (without this flag, an empty body is ignored and the existing body is kept)"
    )]
    pub clear_body: bool,
    /// Validate input and report planned actions without persisting.
    #[arg(
        long,
        default_value_t = false,
        help = "Validate input and report planned actions without persisting"
    )]
    pub dry_run: bool,
    /// GAP-SG-37: reject (instead of silently normalizing) when the supplied
    /// --name is not already canonical kebab-case. Use this when the literal
    /// name matters and a silent transform would surprise downstream lookups.
    #[arg(
        long,
        default_value_t = false,
        help = "Reject the write if --name would be normalized to kebab-case (preserve-name guard)"
    )]
    pub strict_name: bool,
    /// GAP-SG-216: reject a declared `entity_type` outside the thirteen
    /// canonical kinds.
    ///
    /// The sibling of [`Self::strict_name`]: one field guards the name the
    /// caller typed, the other the taxonomy. An open vocabulary stays the
    /// default because LLM extraction depends on it — an extractor cannot be
    /// asked to emit only thirteen labels, while a caller who typed one can opt
    /// into being refused.
    #[arg(
        long,
        default_value_t = false,
        help = "Reject the write if a declared entity_type is outside the canonical vocabulary"
    )]
    pub strict_entity_types: bool,
    /// GAP-SG-51: with --force-merge, REPLACE the memory's entity/relationship
    /// bindings with the supplied set instead of merging additively. Combined
    /// with an empty `entities`/`relationships` payload this clears all bindings
    /// without deleting the memory.
    #[arg(
        long,
        default_value_t = false,
        help = "With --force-merge, replace (not merge) the memory's graph bindings; empty entities clears them"
    )]
    pub replace_graph: bool,
    /// Optional opaque session identifier for tracing memory provenance across multi-agent runs.
    #[arg(long)]
    pub session_id: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = JsonOutputFormat::Json)]
    pub format: JsonOutputFormat,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
    /// Maximum process RSS in MiB; abort if exceeded during embedding.
    #[arg(long, default_value_t = crate::constants::DEFAULT_MAX_RSS_MB,
          help = "Maximum process RSS in MiB; abort if exceeded during embedding (default: 8192)")]
    pub max_rss_mb: u64,
    /// G42/S3 (v1.0.79): maximum simultaneous LLM embedding subprocesses.
    /// The effective value is further bounded by CPU count and available
    /// RAM (permits = min(N, cpus, ram_livre*0.5/350MB), clamp [1, 32]).
    #[arg(long, default_value_t = 4, value_name = "N",
          value_parser = clap::value_parser!(u64).range(1..=32),
          help = "Maximum simultaneous LLM embedding subprocesses (default: 4, clamp [1,32])")]
    pub llm_parallelism: u64,
    /// GAP-CLI-PRIO-02: after write, enqueue entity-descriptions for the
    /// entities linked in this call (priority hot set). Default false —
    /// operators enable when they want automatic priority enrich.
    #[arg(long, default_value_t = false)]
    pub enqueue_enrich: bool,
}
