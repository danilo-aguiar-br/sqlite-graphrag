//! CLI surface of the `deep-research` subcommand.

/// Arguments for the `deep-research` subcommand.
#[derive(clap::Args)]
#[command(
    about = "Deep parallel multi-hop GraphRAG research via query decomposition",
    after_long_help = "CONTRACT:\n  \
        stdout = pretty JSON envelope only (machine-readable).\n  \
        stderr = tracing / progress / diagnostics only.\n  \
        Never redirect with `&>` or `2>&1` into the same file as stdout — that\n  \
        contaminates the JSON and breaks jaq/jq. Prefer:\n  \
        sqlite-graphrag deep-research \"q\" > out.json 2>/dev/null\n  \
        or --output out.json (atomic write via atomwrite algorithm).\n\n\
EXAMPLES:\n  \
        # Basic deep research (single-token queries auto-expand into aspects)\n  \
        sqlite-graphrag deep-research \"danilo\"\n\n  \
        # With custom parameters\n  \
        sqlite-graphrag deep-research \"auth\" --k 20 --max-hops 3 --max-sub-queries 7\n\n  \
        # Include full memory bodies in output\n  \
        sqlite-graphrag deep-research \"auth\" --with-bodies\n\n  \
        # Manual sub-queries (one query per line)\n  \
        sqlite-graphrag deep-research \"danilo\" --sub-query-strategy manual \\\n  \
          --sub-queries-file aspects.txt\n\n  \
        # Atomic JSON file (crash-safe; preferred for large --with-bodies runs)\n  \
        sqlite-graphrag deep-research \"auth\" --output /tmp/dr.json\n\n  \
        # Tune RRF and graph scoring\n  \
        sqlite-graphrag deep-research \"auth and deployment\" --rrf-k 60 --graph-decay 0.7"
)]
pub struct DeepResearchArgs {
    /// Research query to decompose and search.
    #[arg(
        value_name = "QUERY",
        allow_hyphen_values = true,
        help = "Research query to decompose and search"
    )]
    pub query: String,
    /// Results per sub-query (Recall@20 captures 95%+ relevant hits).
    #[arg(
        long,
        short,
        aliases = ["limit", "top-k"],
        default_value_t = 20,
        value_parser = crate::parsers::parse_k_range,
        help = "Results per sub-query (Recall@20 captures 95%+ relevant hits)"
    )]
    pub k: usize,
    /// Maximum sub-queries from decomposition (covers complex multi-hop queries).
    #[arg(
        long,
        default_value_t = 7,
        value_parser = crate::parsers::parse_sub_queries_range,
        help = "Maximum sub-queries (covers complex multi-hop queries)"
    )]
    pub max_sub_queries: usize,
    /// Multi-hop graph traversal depth (sweet spot: 2-3 hops).
    #[arg(
        long,
        default_value_t = 3,
        value_parser = crate::parsers::parse_hops_range_usize,
        help = "Multi-hop graph traversal depth (sweet spot: 2-3 hops)"
    )]
    pub max_hops: usize,
    /// Minimum edge weight for graph traversal.
    #[arg(
        long,
        default_value_t = 0.3,
        help = "Minimum edge weight for graph traversal"
    )]
    pub min_weight: f64,
    /// Maximum concurrent sub-queries (default: min(cpus, 8)).
    #[arg(long, help = "Maximum concurrent sub-queries (default: min(cpus, 8))")]
    pub max_concurrency: Option<usize>,
    /// Timeout per sub-query in seconds.
    #[arg(long, default_value_t = 30, help = "Timeout per sub-query in seconds")]
    pub timeout: u64,
    /// Include full memory bodies in results.
    #[arg(
        long,
        default_value_t = false,
        help = "Include full memory bodies in results"
    )]
    pub with_bodies: bool,
    /// Maximum results after deduplication.
    #[arg(
        long,
        default_value_t = 50,
        value_parser = crate::parsers::parse_k_range,
        help = "Maximum results after deduplication"
    )]
    pub max_results: usize,
    /// RRF k parameter controlling score smoothing (higher = less weight on top ranks).
    #[arg(
        long,
        default_value_t = 60.0,
        help = "RRF k parameter (higher = less weight on top ranks)"
    )]
    pub rrf_k: f64,
    /// Decay factor applied to graph scores per hop (score = seed_score * decay^hop).
    #[arg(
        long,
        default_value_t = 0.7,
        help = "Graph score decay factor per hop (0.0-1.0)"
    )]
    pub graph_decay: f64,
    /// Minimum score threshold for graph-expanded results (filters noise).
    #[arg(
        long,
        default_value_t = 0.05,
        help = "Minimum score threshold for graph-expanded results"
    )]
    pub graph_min_score: f64,
    /// Limit top-k neighbours followed per entity per hop (None = unlimited).
    #[arg(
        long,
        help = "Limit neighbours per entity per hop for graph traversal (default: unlimited)"
    )]
    pub max_neighbors_per_hop: Option<usize>,
    /// Namespace (flag / XDG namespace.default / global).
    #[arg(long, help = "Namespace (flag / XDG namespace.default / global)")]
    pub namespace: Option<String>,
    /// Research mode. `none` (local heuristic) is the only accepted value.
    ///
    /// The doc used to offer `claude-code` and `codex` while `value_parser`
    /// accepted neither, so the help advertised two values clap rejected on
    /// sight. v1.2.0 removed both backends; the flag survives, hidden, so
    /// existing `--mode none` invocations keep parsing.
    #[arg(long, default_value = "none", value_parser = ["none"], hide = true)]
    pub mode: String,
    /// Maximum LLM cost in USD. Inert while `--mode` accepts only `none`.
    ///
    /// Kept as an accepted flag so scripted invocations that pass it do not
    /// break, and reported as inert by the handler rather than silently ignored.
    #[arg(
        long,
        value_name = "USD",
        help = "Max LLM cost in USD (inert: no LLM research mode is available)"
    )]
    pub max_cost_usd: Option<f64>,
    /// JSON output (always on, kept for consistency).
    #[arg(long, hide = true)]
    pub json: bool,
    /// Database path.
    #[arg(long)]
    pub db: Option<String>,
    /// Sub-query strategy: `heuristic` (default, syntactic + single-token aspects)
    /// or `manual` (requires `--sub-queries-file`).
    #[arg(
        long,
        default_value = "heuristic",
        value_parser = ["heuristic", "manual"],
        help = "Sub-query strategy: heuristic (default) or manual"
    )]
    pub sub_query_strategy: String,
    /// Path to a UTF-8 text file with one sub-query per line (required when
    /// `--sub-query-strategy manual`). Empty lines and `#` comments are ignored.
    #[arg(
        long,
        value_name = "PATH",
        help = "File with one sub-query per line (manual strategy)"
    )]
    pub sub_queries_file: Option<std::path::PathBuf>,
    /// Write the JSON envelope atomically to this path (tempfile→fsync→rename).
    /// When set, stdout receives a short confirmation JSON
    /// `{ "written": "<path>", "bytes": N, "blake3": "..." }` instead of the full
    /// envelope — preventing shell redirect truncation of multi-MB payloads.
    #[arg(
        short = 'o',
        long,
        value_name = "PATH",
        help = "Atomic JSON output path (atomwrite algorithm; short -o)"
    )]
    pub output: Option<std::path::PathBuf>,
}
