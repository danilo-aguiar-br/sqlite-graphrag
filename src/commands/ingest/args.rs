//! CLI arguments and low-memory / parallelism resolution for `ingest`.

use crate::cli::MemoryType;
use crate::output::JsonOutputFormat;
use std::path::PathBuf;

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Ingest every Markdown file under ./docs as `document` memories\n  \
    sqlite-graphrag ingest ./docs --type document\n\n  \
    # Ingest .txt files recursively under ./notes\n  \
    sqlite-graphrag ingest ./notes --type note --pattern '*.txt' --recursive\n\n  \
    # Namespace derived names with a kebab-case prefix (projx-<derived>)\n  \
    sqlite-graphrag ingest ./docs --name-prefix projx- --dry-run\n\n  \
    # Enable automatic URL extraction (URL-regex only since v1.0.79)\n  \
    sqlite-graphrag ingest ./big-corpus --type reference --enable-ner\n\n  \
    # Preview file-to-name mapping without ingesting\n  \
    sqlite-graphrag ingest ./docs --dry-run\n\n  \
    # LLM-curated extraction via Claude Code CLI\n  \
    sqlite-graphrag ingest ./docs --mode claude-code --recursive --json\n\n  \
    # Resume interrupted claude-code ingest\n  \
    sqlite-graphrag ingest ./docs --mode claude-code --resume --json\n\n  \
    # Claude Code with budget cap and custom timeout\n  \
    sqlite-graphrag ingest ./docs --mode claude-code --max-cost-usd 5.00 --claude-timeout 600 --json\n\n  \
AUTHENTICATION:\n  \
    --mode claude-code: Uses existing Claude Code authentication.\n  \
      OAuth (Pro/Max/Team): works automatically from ~/.claude/.credentials.json\n  \
      API key: set ANTHROPIC_API_KEY for faster startup (optional)\n\n  \
    --mode codex: Uses existing Codex CLI authentication.\n  \
      Device auth: run `codex auth login` first\n  \
      API key: set OPENAI_API_KEY (optional)\n\n  \
NOTES:\n  \
    Each file becomes a separate memory. Names derive from file basenames\n  \
    (kebab-case, lowercase, ASCII). Output is NDJSON: one JSON object per file,\n  \
    followed by a final summary line with counts. Per-file errors are reported\n  \
    inline and processing continues unless --fail-fast is set.")]
/// Ingest args.
pub struct IngestArgs {
    /// Directory containing files to ingest.
    #[arg(
        value_name = "DIR",
        help = "Directory to ingest recursively (each matching file becomes a memory)"
    )]
    pub dir: PathBuf,

    /// Memory type stored in `memories.type` for every ingested file. Defaults to `document`.
    #[arg(long, value_enum, default_value_t = MemoryType::Document)]
    pub r#type: MemoryType,

    /// Glob pattern matched against file basenames (default: `*.md`). Supports
    /// `*.<ext>`, `<prefix>*`, and exact filename match.
    #[arg(long, default_value = "*.md")]
    pub pattern: String,

    /// Recurse into subdirectories.
    #[arg(long, default_value_t = false)]
    pub recursive: bool,

    #[arg(
        long,
                value_parser = crate::parsers::parse_bool_flexible,
        action = clap::ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value = "false",
        help = "Enable automatic URL-regex extraction (URL-regex only since v1.0.79)"
    )]
    /// Enable NER.
    pub enable_ner: bool,

    /// GAP-E2E-011: generates a heuristic description from the first meaningful
    /// line of the body, instead of "ingested from `<path>`". When
    /// `--no-auto-describe` is passed, keeps the legacy behaviour.
    #[arg(
        long,
        default_value_t = true,
        overrides_with = "no_auto_describe",
        help = "Derive memory description from the first meaningful body line instead of the legacy `ingested from <path>` placeholder."
    )]
    pub auto_describe: bool,
    #[arg(
        long = "no-auto-describe",
        default_value_t = false,
        help = "Disable `--auto-describe` and fall back to the legacy `ingested from <path>` description placeholder."
    )]
    /// No auto describe.
    pub no_auto_describe: bool,

    /// Deprecated: NER is now disabled by default. Kept for backwards compatibility.
    #[arg(long, default_value_t = false, hide = true)]
    pub skip_extraction: bool,

    /// Stop on first per-file error instead of continuing with the next file.
    #[arg(long, default_value_t = false)]
    pub fail_fast: bool,

    /// Preview file-to-name mapping without loading model or persisting.
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// Maximum number of files to ingest (safety cap to prevent runaway ingestion).
    #[arg(long, default_value_t = 10_000)]
    pub max_files: usize,

    /// Namespace for the ingested memories.
    #[arg(long)]
    pub namespace: Option<String>,

    /// Database path. Falls back to XDG `db.path`, then `graphrag.sqlite`
    /// under the XDG data directory.
    #[arg(long)]
    pub db: Option<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = JsonOutputFormat::Json)]
    pub format: JsonOutputFormat,

    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,

    /// Number of files to extract+embed in parallel; default = max(1, cpus/2).min(4).
    #[arg(
        long,
        help = "Number of files to extract+embed in parallel; default = max(1, cpus/2).min(4)"
    )]
    pub ingest_parallelism: Option<usize>,

    /// Force single-threaded ingest to reduce RSS pressure.
    ///
    /// Equivalent to `--ingest-parallelism 1`, takes precedence over any
    /// explicit value. Recommended for environments with <4 GB available
    /// RAM or container/cgroup constraints. Trade-off: 3-4x longer wall
    /// time. Also available via XDG `ingest.low_memory=1`
    /// (CLI flag has higher precedence than the env var).
    #[arg(
        long,
        default_value_t = false,
        help = "Forces single-threaded ingest (--ingest-parallelism 1) to reduce RSS pressure. \
                Recommended for environments with <4 GB available RAM or container/cgroup \
                constraints. Trade-off: 3-4x longer wall time. Also honored via \
                XDG ingest.low_memory=1."
    )]
    pub low_memory: bool,

    /// Maximum process RSS in MiB; abort if exceeded during embedding.
    #[arg(long, default_value_t = crate::constants::DEFAULT_MAX_RSS_MB,
          help = "Maximum process RSS in MiB; abort if exceeded during embedding (default: 8192)")]
    pub max_rss_mb: u64,

    /// G42/S3 (v1.0.79): maximum simultaneous LLM embedding subprocesses
    /// PER FILE. Multiplies with --ingest-parallelism (files staged
    /// concurrently), hence the conservative default of 2. The effective
    /// value is further bounded by CPU count and available RAM.
    #[arg(long, default_value_t = 2, value_name = "N",
          value_parser = clap::value_parser!(u64).range(1..=32),
          help = "Maximum simultaneous LLM embedding subprocesses per file (default: 2, clamp [1,32])")]
    pub llm_parallelism: u64,

    /// Maximum character length for derived memory names from file basenames.
    ///
    /// Overrides the compile-time `DERIVED_NAME_MAX_LEN` constant (default 60).
    /// Shorter values leave more headroom for collision suffix resolution.
    #[arg(long, default_value_t = crate::constants::DERIVED_NAME_MAX_LEN,
          help = "Maximum length for derived memory names (default: 60)")]
    pub max_name_length: usize,

    /// v1.1.1 (P12): kebab-case prefix prepended to every derived memory name,
    /// AFTER the basename is normalized. Namespaces a corpus inside a shared
    /// database (e.g. `--name-prefix projx-` yields `projx-<derived>`). The
    /// derived part's budget shrinks so the final name always respects the
    /// 80-char name cap. Only supported with `--mode none`.
    #[arg(
        long,
        value_name = "PREFIX",
        help = "Kebab-case prefix applied to every derived memory name (e.g. 'projx-')"
    )]
    pub name_prefix: Option<String>,

    /// Extraction mode: `none` (body-only, default) or `claude-code`/`codex`/`opencode` (LLM-curated).
    #[arg(long, value_enum, default_value_t = IngestMode::None)]
    pub mode: IngestMode,

    /// Explicit path to the Claude Code binary (only with --mode claude-code).
    #[arg(long)]
    pub claude_binary: Option<std::path::PathBuf>,

    /// Model override for Claude Code extraction (e.g. claude-sonnet-4-6).
    #[arg(long)]
    pub claude_model: Option<String>,

    /// Resume a previously interrupted claude-code ingest from the queue DB.
    #[arg(long, default_value_t = false)]
    pub resume: bool,

    /// Retry only failed files from a previous claude-code ingest.
    #[arg(long, default_value_t = false)]
    pub retry_failed: bool,

    /// Keep the queue DB (.ingest-queue.sqlite) after completion.
    #[arg(long, default_value_t = false)]
    pub keep_queue: bool,

    /// Custom path for the ingest queue DB. Default: alongside the --db database.
    #[arg(long)]
    pub queue_db: Option<String>,

    /// Initial wait time in seconds when rate-limited (only with --mode claude-code).
    #[arg(long, default_value_t = 60)]
    pub rate_limit_wait: u64,

    /// Maximum cumulative cost in USD before aborting (only with --mode claude-code).
    #[arg(long)]
    pub max_cost_usd: Option<f64>,

    /// Timeout in seconds for each claude -p invocation (only with --mode claude-code).
    #[arg(
        long,
        default_value_t = 300,
        help = "Timeout in seconds for each claude -p invocation (default: 300)"
    )]
    pub claude_timeout: u64,

    /// Explicit path to the Codex CLI binary (only with --mode codex).
    #[arg(
        long,
        help = "Explicit path to the Codex CLI binary (only with --mode codex)"
    )]
    pub codex_binary: Option<PathBuf>,

    /// Model override for Codex extraction (e.g. o4-mini, gpt-5.1-codex).
    #[arg(
        long,
        help = "Model override for Codex extraction (e.g. o4-mini, gpt-5.1-codex)"
    )]
    pub codex_model: Option<String>,

    /// Timeout in seconds for each codex exec invocation.
    #[arg(
        long,
        default_value_t = 300,
        help = "Timeout in seconds for each codex exec invocation (default: 300)"
    )]
    pub codex_timeout: u64,

    /// Path to the `opencode` binary (override PATH lookup, only with --mode opencode).
    #[arg(long, value_name = "PATH")]
    pub opencode_binary: Option<PathBuf>,

    /// Model override for OpenCode extraction.
    #[arg(
        long,
        value_name = "MODEL",
        help = "Model override for OpenCode extraction"
    )]
    pub opencode_model: Option<String>,

    /// Timeout in seconds for each opencode run invocation.
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = 300,
        help = "Timeout in seconds for each opencode run invocation (default: 300)"
    )]
    pub opencode_timeout: u64,

    /// G30: poll for the job singleton every second for up to N seconds
    /// when another invocation holds the lock. Default: 0 (fail fast).
    #[arg(long, value_name = "SECONDS")]
    pub wait_job_singleton: Option<u64>,

    /// G30: force acquisition of the singleton lock by removing a stale
    /// lock file from a previously crashed invocation.
    #[arg(long, default_value_t = false)]
    pub force_job_singleton: bool,

    /// v1.0.93 (GAP-OR-INGEST): run `enrich --operation memory-bindings`
    /// after all files are embedded, using the active `--llm-backend`.
    #[arg(
        long,
        default_value_t = false,
        help = "Run enrich --operation memory-bindings after all files are ingested"
    )]
    pub enrich_after: bool,

    /// GAP-SG-54: update existing memories instead of skipping them. Without
    /// this flag a file whose derived name already exists is reported `skipped`;
    /// with it the existing memory's body, embedding and chunks are refreshed
    /// (the `remember --force-merge` update path applied per file).
    #[arg(
        long,
        default_value_t = false,
        help = "Update existing memories on name collision instead of skipping (idempotent re-ingest)"
    )]
    pub force_merge: bool,
}

/// Extraction mode for the ingest pipeline.
#[derive(Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum IngestMode {
    /// Body-only ingestion without entity/relationship extraction (default).
    None,
    /// LLM-curated extraction via locally installed Claude Code CLI.
    ClaudeCode,
    /// LLM-curated extraction via locally installed OpenAI Codex CLI.
    Codex,
    /// LLM-curated extraction via locally installed OpenCode CLI.
    #[value(name = "opencode")]
    Opencode,
}

/// Returns true when the XDG setting `ingest.low_memory` holds a truthy value
/// (`1`, `true`, `yes`, `on`, case-insensitive). Empty or unset values evaluate
/// to false. Unrecognized non-empty values emit a `tracing::warn!` and evaluate
/// to false.
///
/// GAP-SG-83: this function was named `env_low_memory_enabled` and documented an
/// env var long after `G-T-XDG-04` moved the value to the XDG config, so the log
/// pointed operators at a variable no reader consults. No product env is read.
pub(crate) fn low_memory_setting_enabled() -> bool {
    match crate::config::get_setting("ingest.low_memory") {
        Ok(Some(v)) if v.is_empty() => false,
        Ok(Some(v)) => match v.to_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            other => {
                tracing::warn!(
                    target: "ingest",
                    value = %other,
                    "ingest.low_memory value not recognized; treating as disabled"
                );
                false
            }
        },
        _ => false,
    }
}

/// Resolves the effective ingest parallelism honoring `--low-memory` and the
/// XDG setting `ingest.low_memory`.
///
/// Precedence (G-T-XDG-04):
/// 1. `--low-memory` CLI flag forces parallelism = 1.
/// 2. XDG `ingest.low_memory` truthy forces parallelism = 1.
/// 3. Explicit `--ingest-parallelism N` (when low-memory is off).
/// 4. Default heuristic `(cpus/2).clamp(1, 4)`.
///
/// When low-memory wins and the user also passed `--ingest-parallelism N>1`,
/// emits a `tracing::warn!` advertising the override.
pub(crate) fn resolve_parallelism(
    low_memory_flag: bool,
    ingest_parallelism: Option<usize>,
) -> usize {
    let setting_flag = low_memory_setting_enabled();
    let low_memory = low_memory_flag || setting_flag;

    if low_memory {
        if let Some(n) = ingest_parallelism {
            if n > 1 {
                tracing::warn!(
                    target: "ingest",
                    requested = n,
                    "--ingest-parallelism overridden by --low-memory; using 1"
                );
            }
        }
        if low_memory_flag {
            tracing::info!(
                target: "ingest",
                source = "flag",
                "low-memory mode enabled: forcing --ingest-parallelism 1"
            );
        } else {
            tracing::info!(
                target: "ingest",
                source = "xdg",
                "low-memory mode enabled via XDG ingest.low_memory: forcing --ingest-parallelism 1"
            );
        }
        return 1;
    }

    ingest_parallelism
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|v| v.get() / 2)
                .unwrap_or(1)
                .clamp(1, 4)
        })
        .max(1)
}
