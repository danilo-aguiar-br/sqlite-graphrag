//! CLI surface of the `hybrid-search` subcommand.

use crate::cli::MemoryType;
use crate::errors::AppError;
use crate::output::JsonOutputFormat;

/// Arguments for the `hybrid-search` subcommand.
///
/// When `--namespace` is omitted the search runs against the `global` namespace,
/// which is the default namespace used by `remember` when no `--namespace` flag
/// is provided. Pass an explicit `--namespace` value to search a different
/// isolated namespace.
#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Basic hybrid search combining FTS5 + vector via RRF\n  \
    sqlite-graphrag hybrid-search \"postgres migration deadlock\" --k 10\n\n  \
    # Tune RRF weights to favor keyword matches over semantic similarity\n  \
    sqlite-graphrag hybrid-search \"jwt auth\" --weight-fts 1.5 --weight-vec 0.5 --k 5\n\n  \
    # Add graph traversal matches (entities connected to top results)\n  \
    sqlite-graphrag hybrid-search \"frontend architecture\" --with-graph --k 10\n\n  \
    # Graph traversal with custom depth and minimum edge weight\n  \
    sqlite-graphrag hybrid-search \"auth design\" --with-graph --max-hops 3 --min-weight 0.5 --k 10\n\n  \
NOTES:\n  \
    --with-graph enables entity graph traversal seeded by the top RRF results.\n  \
    Graph matches appear in the `graph_matches` array (separate from `results`).\n  \
    Without --with-graph, `graph_matches` is always empty.")]
pub struct HybridSearchArgs {
    #[arg(
        allow_hyphen_values = true,
        help = "Hybrid search query (vector KNN + FTS5 BM25 fused via RRF)"
    )]
    /// Search query text.
    pub query: String,
    /// Maximum number of fused results to return after RRF combines vector + FTS5 candidates.
    ///
    /// Validated to the inclusive range `1..=4096` (the upper bound matches `sqlite-vec`'s knn
    /// limit). Each underlying search fetches `k * 2` candidates before fusion.
    #[arg(short = 'k', long, aliases = ["limit", "top-k"], default_value = "10", value_parser = crate::parsers::parse_k_range)]
    pub k: usize,
    /// Rrf k.
    #[arg(long, default_value = "60")]
    pub rrf_k: u32,
    /// Weight VEC.
    #[arg(long, default_value = "1.0")]
    pub weight_vec: f32,
    /// Weight FTS.
    #[arg(long, default_value = "1.0")]
    pub weight_fts: f32,
    /// Filter by memory.type. Note: distinct from graph entity_type
    /// (project/tool/person/file/concept/incident/decision/memory/dashboard/issue_tracker/organization/location/date)
    /// used in --entities-file.
    #[arg(long, value_enum)]
    pub r#type: Option<MemoryType>,
    /// Namespace scope.
    #[arg(long)]
    pub namespace: Option<String>,
    /// With graph.
    #[arg(long)]
    pub with_graph: bool,
    /// Cap the size of `graph_matches` to at most N entries; `0` removes the cap.
    ///
    /// Unlike the `recall` flag of the same name this one is ACTIVE by default
    /// ([`crate::constants::DEFAULT_HYBRID_MAX_GRAPH_RESULTS`], overridable via
    /// XDG `search.hybrid.max_graph_results`). The traversal is seeded by the
    /// fused results AND by the entities nearest the query embedding, so its
    /// size follows the graph, not `--k`: an uncapped `--k 3` measured a
    /// 1 112 925 byte envelope.
    #[arg(long, value_name = "N")]
    pub max_graph_results: Option<usize>,
    /// G58 (v1.0.80): skip the live query embedding and serve FTS5 BM25 only.
    /// Useful in CI/CD with tight OAuth quota and in deterministic tests.
    #[arg(long, help = "Skip live query embedding; serve FTS5 BM25 only")]
    pub fallback_fts_only: bool,
    /// Graph traversal depth (requires --with-graph; default 2 when active).
    #[arg(long)]
    pub max_hops: Option<u32>,
    /// Minimum edge weight for graph traversal (requires --with-graph; default 0.3 when active).
    #[arg(long)]
    pub min_weight: Option<f64>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = JsonOutputFormat::Json)]
    pub format: JsonOutputFormat,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
    /// Accept `--json` as a no-op because output is already JSON by default.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
}

impl HybridSearchArgs {
    /// G20: reject graph-specific flags when `--with-graph` is not active.
    ///
    /// G48: `Option<T>` detects an explicitly provided flag even when the value
    /// equals the old default (pre-fix, `--max-hops 2` was silently accepted).
    pub(super) fn validate_graph_flags(&self) -> Result<(), AppError> {
        if !self.with_graph {
            if self.max_hops.is_some() {
                return Err(AppError::Validation(
                    "--max-hops requires --with-graph to be active".to_string(),
                ));
            }
            if self.min_weight.is_some() {
                return Err(AppError::Validation(
                    "--min-weight requires --with-graph to be active".to_string(),
                ));
            }
        }
        Ok(())
    }
}
