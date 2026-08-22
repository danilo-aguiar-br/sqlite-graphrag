//! CLI argument types for `graph` subcommands.

use crate::cli::GraphExportFormat;
use serde::Serialize;
use std::path::PathBuf;

/// Optional nested subcommands. When absent, the default behavior exports
/// the full entity snapshot for backward compatibility.
#[derive(clap::Subcommand)]
pub enum GraphSubcommand {
    /// Traverse relationships from a starting entity using BFS
    Traverse(GraphTraverseArgs),
    /// Show graph statistics (node/edge counts, degree distribution)
    Stats(GraphStatsArgs),
    /// List entities stored in the graph with optional filters
    Entities(GraphEntitiesArgs),
    /// Audit the entity-type vocabulary actually present in the database
    EntityTypes(GraphEntityTypesArgs),
    /// Reconcile the cached `degree` column with the real edge counts (P3)
    RecomputeDegree(GraphRecomputeDegreeArgs),
}

/// Graph traverse format.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphTraverseFormat {
    /// JSON variant.
    Json,
}

/// Graph stats format.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphStatsFormat {
    /// JSON variant.
    Json,
    /// Text variant.
    Text,
}

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Export full entity snapshot as JSON (default)\n  \
    sqlite-graphrag graph\n\n  \
    # Traverse relationships from a starting entity\n  \
    sqlite-graphrag graph traverse --from acme-corp --depth 2\n\n  \
    # Show graph statistics as structured JSON\n  \
    sqlite-graphrag graph stats --format json\n\n  \
    # List entities filtered by type\n  \
    sqlite-graphrag graph entities --entity-type person\n\n  \
    # Export full snapshot in DOT format for Graphviz\n  \
    sqlite-graphrag graph --format dot --output graph.dot\n\n  \
NOTES:\n  \
    Without a subcommand, exports the full entity+edge snapshot.\n  \
    Use `traverse`, `stats`, or `entities` for targeted queries.")]
/// Graph args.
pub struct GraphArgs {
    /// Optional subcommand; without one, export the full entity snapshot.
    #[command(subcommand)]
    pub subcommand: Option<GraphSubcommand>,
    /// Filter by namespace. Defaults to all namespaces.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Snapshot output format.
    #[arg(long, value_enum, default_value = "json")]
    pub format: GraphExportFormat,
    /// File path to write output instead of stdout.
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
}

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Traverse relationships from an entity with default depth (2)\n  \
    sqlite-graphrag graph traverse --from acme-corp\n\n  \
    # Increase traversal depth to 3 hops\n  \
    sqlite-graphrag graph traverse --from acme-corp --depth 3\n\n  \
    # Traverse within a specific namespace\n  \
    sqlite-graphrag graph traverse --from acme-corp --namespace project-x\n\n  \
NOTES:\n  \
    Output is always JSON. The `hops` array contains each reachable entity\n  \
    with its relation, direction (inbound/outbound), weight, and depth level.\n  \
    Short nicknames (e.g. `alice` vs `alice-martins-souza`) do not exact-match;\n  \
    without `--fuzzy` the error includes ranked suggestions (v1.1.05). With\n  \
    `--fuzzy`, a clear single winner is auto-resolved and warned on stderr.")]
/// Graph traverse args.
pub struct GraphTraverseArgs {
    /// Root entity name for the traversal.
    #[arg(long)]
    pub from: String,
    /// Maximum traversal depth.
    #[arg(long, default_value_t = 2u32, value_parser = crate::parsers::parse_hops_range_u32)]
    pub depth: u32,
    /// When exact name match fails, auto-resolve a clear fuzzy match
    /// (prefix / first-token / Jaro-Winkler). Without this flag, NotFound
    /// (exit 4) includes ranked suggestions of canonical names (v1.1.05 Bug 3).
    #[arg(long, default_value_t = false)]
    pub fuzzy: bool,
    /// Namespace scope.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value = "json")]
    pub format: GraphTraverseFormat,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
}

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Show stats for all namespaces (human-readable text)\n  \
    sqlite-graphrag graph stats --format text\n\n  \
    # Show stats as structured JSON\n  \
    sqlite-graphrag graph stats --format json\n\n  \
    # Show stats for a specific namespace\n  \
    sqlite-graphrag graph stats --namespace project-x --format text\n\n  \
NOTES:\n  \
    Reports node_count, edge_count, avg_degree, and max_degree.\n  \
    Default format is JSON. Use `--format text` for a compact single-line summary.")]
/// Graph stats args.
pub struct GraphStatsArgs {
    /// Namespace scope.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Output format for the stats response.
    #[arg(long, value_enum, default_value = "json")]
    pub format: GraphStatsFormat,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
}

/// Graph entity-types format.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphEntityTypesFormat {
    /// JSON variant.
    Json,
    /// Text variant.
    Text,
}

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # List every entity type present, most frequent first\n  \
    sqlite-graphrag graph entity-types\n\n  \
    # Restrict the audit to one namespace\n  \
    sqlite-graphrag graph entity-types --namespace project-x\n\n  \
    # Compact human-readable summary\n  \
    sqlite-graphrag graph entity-types --format text\n\n  \
    # Only the labels outside the canonical set\n  \
    sqlite-graphrag graph entity-types --filter canonical=false\n\n\
NOTES:\n  \
    v1.2.8 opened the entity-type vocabulary, so the set of stored labels is\n  \
    no longer knowable from the source. This reports what the database\n  \
    actually holds: each `type` with its `count` and whether it belongs to\n  \
    the canonical thirteen. `--entity-type` on `graph entities` can only\n  \
    filter by a label you already guessed; this is where you find the label.")]
/// Graph entity types args.
pub struct GraphEntityTypesArgs {
    /// Namespace scope. Omit to audit ALL namespaces.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Output format for the vocabulary report.
    #[arg(long, value_enum, default_value = "json")]
    pub format: GraphEntityTypesFormat,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
}

/// One row of the entity-type vocabulary audit.
#[derive(Serialize)]
pub(crate) struct EntityTypeCount {
    /// The label exactly as stored, after V017 removed the SQL `CHECK`.
    #[serde(rename = "type")]
    pub(crate) entity_type: String,
    /// Entities carrying this label within the requested scope.
    pub(crate) count: i64,
    /// Whether the label is one of `CANONICAL_ENTITY_TYPES`. False is a normal
    /// result, not a defect: the vocabulary is open by design.
    pub(crate) canonical: bool,
}

#[derive(Serialize)]
pub(crate) struct GraphEntityTypesResponse {
    pub(crate) types: Vec<EntityTypeCount>,
    /// Distinct labels found, which is `types.len()` before any agent-surface
    /// trimming and therefore survives `--max-items`.
    pub(crate) total_types: usize,
    /// Entities summed across every label in scope.
    pub(crate) total_entities: i64,
    pub(crate) namespace: Option<String>,
    pub(crate) elapsed_ms: u64,
}

/// Field to sort entities by in `graph entities`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum EntitySortField {
    /// Sort alphabetically by entity name.
    Name,
    /// Sort by degree (total number of relationships). Use `--order desc` for most-connected-first.
    Degree,
    /// Sort by entity creation timestamp.
    CreatedAt,
}

/// Sort direction for `graph entities`.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum SortOrder {
    /// Asc variant.
    #[default]
    Asc,
    /// Desc variant.
    Desc,
}

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # List all entities (default limit applies)\n  \
    sqlite-graphrag graph entities\n\n  \
    # Filter by entity type\n  \
    sqlite-graphrag graph entities --entity-type person\n\n  \
    # Filter by namespace and type\n  \
    sqlite-graphrag graph entities --namespace project-x --entity-type concept\n\n  \
    # Paginate results (skip first 20, return next 10)\n  \
    sqlite-graphrag graph entities --offset 20 --limit 10\n\n  \
    # Sort by degree descending (most connected first)\n  \
    sqlite-graphrag graph entities --sort-by degree --order desc\n\n  \
    # Sort by creation date ascending\n  \
    sqlite-graphrag graph entities --sort-by created-at --order asc\n\n  \
NOTES:\n  \
    Output is always JSON with `entities`, `total_count`, `limit`, and `offset` fields.\n  \
    Entity types are free-form strings; the canonical ones (e.g. `person`,\n  \
    `organization`, `location`) are recommended rather than exhaustive.")]
/// Graph entities args.
pub struct GraphEntitiesArgs {
    /// Namespace scope.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Filter by entity type. Any stored label is accepted (v1.2.8); the
    /// thirteen canonical types are recommended, not exhaustive. A label no
    /// entity carries returns an empty list, which is a result and not an
    /// error.
    #[arg(long, value_name = "TYPE")]
    pub entity_type: Option<String>,
    /// Maximum number of results to return.
    #[arg(long, default_value_t = crate::constants::K_GRAPH_ENTITIES_DEFAULT_LIMIT, value_parser = crate::parsers::parse_k_range)]
    pub limit: usize,
    /// Number of results to skip for pagination.
    #[arg(long, default_value_t = 0usize)]
    pub offset: usize,
    /// Sort entities by this field. When omitted, the default order is by name ascending.
    #[arg(long, value_enum, help = "Sort entities by field")]
    pub sort_by: Option<EntitySortField>,
    /// Sort direction: `asc` (default) or `desc`.
    #[arg(long, value_enum, default_value_t = SortOrder::Asc, help = "Sort order")]
    pub order: SortOrder,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
}

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Preview divergences without writing (recommended first run)\n  \
    sqlite-graphrag graph recompute-degree --dry-run\n\n  \
    # Reconcile every namespace\n  \
    sqlite-graphrag graph recompute-degree\n\n  \
    # Reconcile a single namespace\n  \
    sqlite-graphrag graph recompute-degree --namespace project-x\n\n\
NOTES:\n  \
    `entities.degree` is a derived cache (incremented on link, recalculated\n  \
    on merge/delete) that drifts when edges are written by paths that skip\n  \
    the recalculation. This command recomputes every entity's degree from\n  \
    the real `relationships` rows (same semantics as the canonical\n  \
    `recalculate_degree` helper: COUNT(*) WHERE source_id = id OR\n  \
    target_id = id) inside one transaction. Entities left with zero edges\n  \
    are zeroed. Envelope: {total, updated, zeroed, unchanged}.")]
/// Graph recompute degree args.
pub struct GraphRecomputeDegreeArgs {
    /// Namespace to reconcile. Omit to reconcile ALL namespaces.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Report divergences without writing anything.
    #[arg(long)]
    pub dry_run: bool,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct TraverseHop {
    pub(crate) entity: String,
    pub(crate) relation: String,
    pub(crate) direction: String,
    pub(crate) weight: f64,
    pub(crate) depth: u32,
}

#[derive(Serialize)]
pub(crate) struct GraphTraverseResponse {
    pub(crate) from: String,
    pub(crate) namespace: String,
    pub(crate) depth: u32,
    pub(crate) hops: Vec<TraverseHop>,
    pub(crate) elapsed_ms: u64,
}

#[derive(Serialize)]
pub(crate) struct GraphStatsResponse {
    pub(crate) namespace: Option<String>,
    pub(crate) node_count: i64,
    pub(crate) edge_count: i64,
    pub(crate) avg_degree: f64,
    pub(crate) max_degree: i64,
    pub(crate) elapsed_ms: u64,
}

#[derive(Serialize)]
pub(crate) struct EntityItem {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) entity_type: String,
    pub(crate) namespace: String,
    pub(crate) created_at: String,
    /// Total number of relationships (inbound + outbound) for this entity.
    pub(crate) degree: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct GraphEntitiesResponse {
    pub(crate) entities: Vec<EntityItem>,
    pub(crate) total_count: i64,
    pub(crate) limit: usize,
    pub(crate) offset: usize,
    pub(crate) namespace: Option<String>,
    pub(crate) elapsed_ms: u64,
}
