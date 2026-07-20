//! CLI argument types for the `enrich` subcommand.
//! Extracted from mod.rs (Wave C1).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::DEFAULT_BODY_ENRICH_MAX_CHARS;
use super::DEFAULT_BODY_ENRICH_MIN_CHARS;

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

/// Operation to perform in the `enrich` command.
#[derive(Debug, Clone, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnrichOperation {
    /// Add missing entity/relationship bindings to memories (fully implemented).
    /// memory-bindings LINKS each memory to the EXISTING entities extracted from
    /// its body — it does not invent a new graph, it only connects what is missing. Scans
    /// only UNBOUND memories (those with zero `memory_entities`).
    MemoryBindings,
    /// GAP-SG-24/26: additive augmentation — re-run binding extraction over
    /// memories that are ALREADY bound, filtered by `--names`/`--names-file`, to
    /// merge newly-discovered entities/relationships WITHOUT removing existing
    /// links. Requires a name filter (refuses to re-scan the whole namespace).
    AugmentBindings,
    /// Fill NULL/empty entity descriptions with LLM-generated summaries (fully implemented).
    EntityDescriptions,
    /// Expand short memory bodies into richer content (fully implemented, GAP-18).
    BodyEnrich,
    /// Rebuild missing memory embeddings without rewriting the memory body.
    ReEmbed,
    /// Calibrate relationship weights using LLM analysis (fully implemented; persists weight).
    WeightCalibrate,
    /// Reclassify relationship types using LLM judgment (fully implemented; persists relation).
    RelationReclassify,
    /// Connect isolated entities by suggesting and persisting new relationships (fully implemented).
    EntityConnect,
    /// Validate entity type assignments using LLM judgment (fully implemented; persists type).
    EntityTypeValidate,
    /// Enrich memory descriptions that are generic/auto-generated (fully implemented; persists description).
    DescriptionEnrich,
    /// Identify cross-domain bridges between disconnected subgraphs.
    /// Shares the O(k) pair scan + `entity_connect_seen` drain path with
    /// `entity-connect` (v1.1.04+ / v1.1.06); status backlog proxy remains 0.
    CrossDomainBridges,
    /// Classify memories into domain categories (fully implemented; persists metadata).
    DomainClassify,
    /// Audit the graph for quality issues (scan/report; does not mutate graph structure).
    GraphAudit,
    /// Synthesize deep-research findings into graph memories (fully implemented when bindings persist).
    DeepResearchSynth,
    /// Extract structured body from unstructured text (fully implemented; persists body).
    BodyExtract,
}

/// v1.1.1 (P2): which embedding table the `re-embed` operation backfills.
///
/// `memories` is the historical behaviour (and the default, so existing
/// invocations are unchanged); `entities` and `chunks` close the retroactive
/// coverage gap for `entity_embeddings` / `chunk_embeddings`; `all` runs the
/// three scans in one invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReEmbedTarget {
    /// Memories without a live vector in `memory_embeddings` (default).
    Memories,
    /// Entities without a live vector in `entity_embeddings`.
    Entities,
    /// Chunks without a live vector in `chunk_embeddings`.
    Chunks,
    /// All three targets in a single run.
    All,
}

/// LLM provider for enrichment.
#[derive(Debug, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum EnrichMode {
    /// Use locally installed Claude Code CLI (OAuth-first).
    ClaudeCode,
    /// Use locally installed OpenAI Codex CLI.
    Codex,
    /// Use locally installed OpenCode CLI.
    #[value(name = "opencode")]
    Opencode,
    /// Use the OpenRouter chat-completions REST API (no local CLI; v1.0.95).
    #[value(name = "openrouter")]
    OpenRouter,
}

impl std::fmt::Display for EnrichMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnrichMode::ClaudeCode => write!(f, "claude-code"),
            EnrichMode::Codex => write!(f, "codex"),
            EnrichMode::Opencode => write!(f, "opencode"),
            EnrichMode::OpenRouter => write!(f, "openrouter"),
        }
    }
}

/// Arguments for the `enrich` subcommand.
#[derive(clap::Args)]
#[command(
    about = "Enrich graph memories and entities using an LLM provider",
    after_long_help = "EXAMPLES:\n  \
    # Add missing entity bindings to all unbound memories\n  \
    sqlite-graphrag enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini\n\n  \
    # Fill entity descriptions (dry-run preview, no tokens spent)\n  \
    sqlite-graphrag enrich --operation entity-descriptions --dry-run --json\n\n  \
    # Expand short memory bodies (GAP-18)\n  \
    sqlite-graphrag enrich --operation body-enrich --min-output-chars 600\n\n  \
    # Rebuild only missing memory embeddings without rewriting bodies\n  \
    sqlite-graphrag enrich --operation re-embed --limit 100\n\n  \
    # Resume an interrupted body-enrich run\n  \
    sqlite-graphrag enrich --operation body-enrich --resume --json\n\n  \
    # Retry only failed items from a previous run\n  \
    sqlite-graphrag enrich --operation memory-bindings --retry-failed --json\n\n  \
    # Converge the whole backlog (internal scan+drain loop, no bash wrapper)\n  \
    sqlite-graphrag enrich --operation memory-bindings --mode openrouter \\\n    \
      --openrouter-model deepseek/deepseek-v4-flash:nitro --until-empty --max-runtime 600\n\n  \
    # Inspect / resurrect dead-letter items\n  \
    sqlite-graphrag enrich --operation memory-bindings --list-dead\n  \
    sqlite-graphrag enrich --operation memory-bindings --requeue-dead\n\n  \
    # Read-only status (no LLM, no singleton)\n  \
    sqlite-graphrag enrich --operation memory-bindings --status\n\n\
    OPERATIONS NOTE:\n  \
    memory-bindings LINKS each memory to the EXISTING entities extracted from its\n  \
    body — it does not invent a new graph, it connects what is missing. It scans\n  \
    only UNBOUND memories. To re-run extraction over ALREADY-bound memories and\n  \
    MERGE newly-found entities/relationships additively (without removing links),\n  \
    use --operation augment-bindings with --names/--names-file.\n\n\
    DEAD-LETTER SIDECAR (.enrich-queue.sqlite):\n  \
    A SQLite sidecar tracks each work item across runs. Schema (table `queue`):\n  \
    item_key (UNIQUE name/id), item_type (memory|entity), operation, memory_id,\n  \
    status (pending|processing|done|skipped|dead), attempt, error, error_class,\n  \
    next_retry_at (backoff cooldown). --until-empty loops scan→drain internally\n  \
    until eligible items are exhausted; transient failures (incl. malformed/non-\n  \
    JSON LLM output, GAP-SG-09) reschedule with backoff until --max-attempts, then\n  \
    land in status='dead'. Use --status to see the queue, --list-dead to inspect\n  \
    the sink, --requeue-dead to retry it, and --ignore-backoff to skip cooldowns.\n  \
    --names/--names-file also remedy a cooldown by targeting a specific subset.\n\n\
    EXIT CODES:\n  \
    0  success\n  \
    1  validation error (bad args, binary not found)\n  \
    14 I/O error"
)]
#[derive(Clone)]
pub struct EnrichArgs {
    /// Enrichment operation to run. Required for write operations; optional for
    /// the read-only queue inspectors (`--status` / `--list-dead` /
    /// `--requeue-dead`), where it defaults to `memory-bindings` when omitted
    /// (GAP-SG-31).
    #[arg(
        long,
        short = 'o',
        value_enum,
        value_name = "OPERATION",
        // GAP-CLI-DRY-01: dry-run is read-only (no LLM); still needs --operation.
        required_unless_present_any = ["status", "list_dead", "requeue_dead", "prune_dead_orphans", "prune_dead_entity_orphans"]
    )]
    pub operation: Option<EnrichOperation>,

    /// LLM provider to use. Required for write operations; not needed for the
    /// read-only queue inspectors (`--status` / `--list-dead` /
    /// `--requeue-dead`), which never call the LLM (GAP-SG-31).
    ///
    /// GAP-CLI-DRY-01 (v1.1.8): also optional for `--dry-run` (preview only).
    #[arg(
        long,
        value_enum,
        required_unless_present_any = ["status", "list_dead", "requeue_dead", "prune_dead_orphans", "prune_dead_entity_orphans", "dry_run"]
    )]
    pub mode: Option<EnrichMode>,

    /// Maximum number of items to process in this run. Omit for all.
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,

    /// v1.1.1 (P2): embedding table backfilled by `--operation re-embed`.
    /// `memories` (default) preserves the historical behaviour; `entities`
    /// and `chunks` rebuild `entity_embeddings` / `chunk_embeddings`; `all`
    /// covers the three tables in one run. The scan also selects rows whose
    /// stored `dim` diverges from the configured `--embedding-dim` (P10),
    /// so legacy-dimension vectors are regenerated, not only missing ones.
    /// Ignored (rejected) for every other operation.
    #[arg(long, value_enum, value_name = "TARGET", default_value_t = ReEmbedTarget::Memories)]
    pub target: ReEmbedTarget,

    /// Preview items without calling the LLM (zero tokens consumed).
    #[arg(long)]
    pub dry_run: bool,

    /// Namespace to operate on. Default: global.
    #[arg(long)]
    pub namespace: Option<String>,

    // -- Provider flags (Claude) --
    /// Path to the Claude Code binary. Default: auto-detect from PATH.
    #[arg(long, value_name = "PATH")]
    pub claude_binary: Option<PathBuf>,

    /// Claude model to use (e.g. claude-sonnet-4-6).
    #[arg(long, value_name = "MODEL")]
    pub claude_model: Option<String>,

    /// Timeout per item in seconds when using Claude Code. Default: 300.
    #[arg(long, value_name = "SECONDS", default_value_t = 300)]
    pub claude_timeout: u64,

    // -- Provider flags (Codex) --
    /// Path to the Codex CLI binary. Default: auto-detect from PATH.
    #[arg(long, value_name = "PATH")]
    pub codex_binary: Option<PathBuf>,

    /// Codex model to use (e.g. o4-mini).
    #[arg(long, value_name = "MODEL")]
    pub codex_model: Option<String>,

    /// Timeout per item in seconds when using Codex. Default: 300.
    #[arg(long, value_name = "SECONDS", default_value_t = 300)]
    pub codex_timeout: u64,

    // -- Provider flags (OpenCode) --
    /// Path to the OpenCode binary. Default: auto-detect from PATH.
    #[arg(long, value_name = "PATH")]
    pub opencode_binary: Option<PathBuf>,

    /// OpenCode model to use.
    #[arg(long, value_name = "MODEL")]
    pub opencode_model: Option<String>,

    /// Timeout per item in seconds when using OpenCode. Default: 300.
    #[arg(
        long,
        value_name = "SECONDS",
                default_value_t = 300
    )]
    pub opencode_timeout: u64,

    // -- Provider flags (OpenRouter, v1.0.95) --
    /// OpenRouter text model to use (REQUIRED with --mode openrouter; no default).
    #[arg(long, value_name = "MODEL")]
    pub openrouter_model: Option<String>,

    /// OpenRouter API key. Falls back to OPENROUTER_API_KEY env or stored config.
    #[arg(
        long,
        value_name = "KEY",
                hide_env_values = true
    )]
    pub openrouter_api_key: Option<String>,

    /// Timeout per item in seconds when using OpenRouter. Default: 600.
    ///
    /// GAP-SG-17: raised from 300 to 600 because dense bodies (close to the
    /// ~32K-token context ceiling of the configured model) routinely take
    /// longer than five minutes to generate via `deepseek-v4-flash:nitro`.
    /// Raise it further for very large corpora; lower it for short snippets.
    #[arg(long, value_name = "SECONDS", default_value_t = 600)]
    pub openrouter_timeout: u64,

    /// Optional OpenRouter base URL override (reserved; defaults to the public API).
    #[arg(long, value_name = "URL")]
    pub openrouter_base_url: Option<String>,

    // -- Cost controls --
    /// Abort when cumulative cost exceeds this USD budget (API key only; ignored for OAuth).
    #[arg(long, value_name = "USD")]
    pub max_cost_usd: Option<f64>,

    // -- Queue controls --
    /// Resume a previously interrupted run (skip already-done items).
    #[arg(long)]
    pub resume: bool,

    /// Retry only items that failed in a previous run.
    #[arg(long)]
    pub retry_failed: bool,

    /// v1.1.2 (Bug 4): reset `processing` rows whose `claimed_at` is older than
    /// `--stale-claim-secs` (default 1800s = 30 min) back to `pending`, then exit.
    /// Recover rows orphaned by a kill -9 mid-enrich without a full re-scan.
    #[arg(long)]
    pub reset_stale_claims: bool,

    /// v1.1.2 (Bug 4): age threshold (seconds) for `--reset-stale-claims` and
    /// the run-startup stale sweep. Default 1800 (30 min).
    #[arg(long, value_name = "SECONDS", default_value_t = 1800)]
    pub stale_claim_secs: u64,

    /// GAP-ENRICH-BACKLOG-CONVERGE: loop scan→drain internally until the queue
    /// empties of eligible items or --max-runtime elapses; removes the need for
    /// an external bash retry loop.
    #[arg(long)]
    pub until_empty: bool,

    /// GAP-ENRICH-BACKLOG-CONVERGE: wall-clock ceiling in seconds for
    /// --until-empty. Defaults to 3600 when omitted.
    #[arg(long, value_name = "SECONDS")]
    pub max_runtime: Option<u64>,

    /// GAP-ENRICH-BACKLOG-CONVERGE: attempts per item before it becomes a
    /// dead-letter (status='dead'). Range 1..=20. Default 8.
    ///
    /// GAP-SG-21: the default was raised from 5 to 8 because GAP-SG-09 now
    /// reclassifies malformed / non-JSON LLM output as TRANSIENT (retryable)
    /// rather than a permanent HardFailure. A flaky structured-output model
    /// (e.g. deepseek-v4-flash:nitro) may emit several bad generations in a row
    /// even after JSON repair (GAP-SG-10) recovers most of them; the extra
    /// attempts give the backlog room to converge before an item is parked in
    /// the dead-letter sink. Permanent faults (ProviderError, NotFound) still
    /// dead-letter on the first attempt regardless of this value.
    #[arg(long, value_name = "N", default_value_t = 8, value_parser = clap::value_parser!(u32).range(1..=20))]
    pub max_attempts: u32,

    /// GAP-ENRICH-BACKLOG-CONVERGE: read-only mode — report queue and backlog
    /// counts without calling the LLM or acquiring the singleton.
    ///
    /// Field semantics (v1.1.03 clarification):
    /// - `scan_backlog` = candidates a fresh scan WOULD select from the database
    ///   (REAL pending work, same WHERE predicate as the scanners).
    /// - `queue_pending` = a COMPUTED COUNT over the sidecar queue, NOT a physical
    ///   queue of rows to process — it stays non-zero even after a clean drain.
    /// - `eligible_now == 0` WITH `queue_pending > 0` means COOLDOWN (rate-limit
    ///   backoff), NOT a deadlock: items are parked on `next_retry_at`.
    /// - `eligible_now > 0` stuck against `state: "draining"` IS a deadlock —
    ///   stale `processing` claims hold the state. Run `--reset-stale-claims`
    ///   to clear processing claims older than the threshold.
    #[arg(long)]
    pub status: bool,

    /// GAP-SG-23: list every dead-letter item (status='dead') for the current
    /// operation with its error_class, attempt count and last error message.
    /// Read-only — no LLM, no singleton. Use it to inspect what `--requeue-dead`
    /// would resurrect before running it.
    #[arg(long)]
    pub list_dead: bool,

    /// GAP-SG-11/14: resurrect dead-letter items — move every `status='dead'`
    /// row back to `pending`, zeroing `attempt`, `next_retry_at`, `error` and
    /// `error_class`. Distinct from `--retry-failed`, which only resets the
    /// legacy `status='failed'` rows; dead-letter rows are the terminal sink of
    /// the v1.0.96 converge loop and are never re-selected without this flag.
    /// No LLM call or singleton is taken — it only rewrites queue statuses.
    #[arg(long)]
    pub requeue_dead: bool,

    /// GAP-SG-66: prune ORPHAN dead-letter rows — remove every `status='dead'`
    /// memory row whose `item_key` (the memory name) no longer exists in the
    /// main DB for this namespace. These are terminal "not found" failures that
    /// `--requeue-dead` can never recover (re-processing re-fails the same way),
    /// so they inflate `queue_dead` forever. Read-only on the main DB; deletes
    /// only confirmed-orphan rows from the queue sidecar. Entity-keyed dead rows
    /// are left untouched. No LLM, no singleton — like `--list-dead`.
    #[arg(long)]
    pub prune_dead_orphans: bool,

    /// v1.1.2: prune dead ENTITY orphan rows — remove every `status='dead'`
    /// `item_type='entity'` row from the queue sidecar. Distinct from
    /// `--prune-dead-orphans` (memory-keyed, consults the main DB): entity dead
    /// rows are terminal artifacts of re-extraction and have no recovery path,
    /// so no main-DB check is needed. Required because the v1.1.1 re-embed bug
    /// left 14680 entity-keyed dead-letter rows that the memory-scoped pruner
    /// cannot reach. No LLM, no singleton — like `--list-dead`.
    #[arg(long, conflicts_with = "prune_dead_orphans")]
    pub prune_dead_entity_orphans: bool,

    /// GAP-SG-16: ignore the per-item backoff cooldown (`next_retry_at`) when
    /// selecting candidates, so items waiting on exponential backoff are
    /// processed immediately. Use to drain a backlog whose cooldown windows are
    /// long but the provider has recovered. Without it, `--status` reports such
    /// items under `waiting` and they are skipped until their `next_retry_at`.
    #[arg(long)]
    pub ignore_backoff: bool,

    /// GAP-SG-28: read-only `body-extract` — extract entities/relationships into
    /// the graph WITHOUT rewriting (or truncating) the memory body. The default
    /// `body-extract` restructures the stored body in place; with this flag the
    /// body is left untouched and only graph bindings are persisted (additive,
    /// via the same upsert path as `memory-bindings`). Ignored for every other
    /// operation.
    #[arg(long)]
    pub body_extract_graph_only: bool,

    /// GAP-ENRICH-BACKLOG-CONVERGE: REST concurrency for --mode openrouter
    /// (clamp 1..=16, default 8). Distinct from the legacy --llm-parallelism.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..=16))]
    pub rest_concurrency: Option<u32>,

    // -- body-enrich specific flags (GAP-18) --
    /// Minimum output character count for body-enrich. Default: 500.
    #[arg(long, value_name = "CHARS", default_value_t = DEFAULT_BODY_ENRICH_MIN_CHARS)]
    pub min_output_chars: usize,

    /// Maximum output character count for body-enrich. Default: 2000.
    #[arg(long, value_name = "CHARS", default_value_t = DEFAULT_BODY_ENRICH_MAX_CHARS)]
    pub max_output_chars: usize,

    /// Check that enriched body preserves all facts from the original (LLM judge). Default: true.
    #[arg(long, default_value_t = true)]
    pub preserve_check: bool,

    /// Path to a custom prompt template file for body-enrich.
    #[arg(long, value_name = "PATH")]
    pub prompt_template: Option<PathBuf>,

    /// Number of parallel LLM workers (default 1 = serial).
    /// Each worker claims items atomically from the queue DB via UPDATE...RETURNING.
    /// Range: 1–32. For 2321 entities, --llm-parallelism 4 reduces wall time ~4×.
    #[arg(long, default_value_t = 1, value_name = "N", value_parser = clap::value_parser!(u32).range(1..=32))]
    pub llm_parallelism: u32,

    // -- Output / infra --
    /// Emit NDJSON output. Always true; flag accepted for compatibility.
    #[arg(long)]
    pub json: bool,

    /// Database path override.
    #[arg(long)]
    pub db: Option<String>,

    /// G30: poll for the job singleton every second for up to N seconds
    /// when another invocation holds the lock. Default: 0 (fail fast).
    #[arg(long, value_name = "SECONDS")]
    pub wait_job_singleton: Option<u64>,

    /// G30: force acquisition of the singleton lock by removing a stale
    /// lock file from a previously crashed invocation. Use only when you
    /// are certain no other `enrich`/`ingest` is running.
    #[arg(long, default_value_t = false)]
    pub force_job_singleton: bool,

    /// Select a subset of names to enrich instead of the full candidate set.
    /// Comma-separated, e.g. `--names a,b,c`. Semantics depend on the
    /// operation (GAP-CLI-NAMES-01): entity-keyed ops (`entity-descriptions`,
    /// `entity-connect`, …) treat values as **entity** names; memory-keyed
    /// ops (`memory-bindings`, `body-enrich`, …) treat values as **memory**
    /// names. Prefer `--entity-names` / `--memory-names` for an explicit
    /// namespace. Empty when omitted (processes all candidates).
    ///
    /// GAP-SG-18: also a cooldown remedy — when `--status` shows items under
    /// `waiting` (parked on `next_retry_at` backoff), pass the exact names here
    /// to re-enqueue and process just that subset on the next run instead of
    /// waiting for every cooldown to elapse. REQUIRED for `--operation
    /// augment-bindings`, which refuses to re-scan the whole namespace.
    #[arg(long, value_name = "NAMES", value_delimiter = ',')]
    pub names: Vec<String>,

    /// G37: read the subset of memory names from a file (one per line).
    /// Lines starting with `#` and empty lines are ignored. Combined with
    /// `--names` (union) when both are set.
    #[arg(long, value_name = "PATH")]
    pub names_file: Option<PathBuf>,

    /// G35: probe the LLM provider with a 1-turn ping before processing
    /// the batch. Aborts with a clear error if the rate-limit window is
    /// closed (avoids burning N turns only to fail on item 1).
    #[arg(long, default_value_t = false)]
    pub preflight_check: bool,

    /// G35: if a preflight probe or in-flight call hits the Claude rate
    /// limit, fall back to `--fallback-mode` (typically `codex`) instead
    /// of failing the batch. Ignored when `--mode` is already `codex`.
    #[arg(long, value_enum)]
    pub fallback_mode: Option<EnrichMode>,

    /// G35: number of seconds before the OAuth rate-limit reset at which
    /// the preflight probe should refuse to start. Default 300 (5 min).
    #[arg(long, value_name = "SECONDS", default_value_t = 300)]
    pub rate_limit_buffer: u64,

    /// G28-D: refuse to start when the 1-minute load average exceeds
    /// `2 × ncpus` (or XDG `enrich.max_load_per_ncpu` if set).
    /// Set to false to skip the check on contended CI runners.
    #[arg(long, default_value_t = true)]
    pub max_load_check: bool,

    /// G28-D: when the system is saturated, abort the job after this
    /// many consecutive HardFailure outcomes. Default 5.
    #[arg(long, value_name = "N", default_value_t = 5)]
    pub circuit_breaker_threshold: u32,

    /// G29 Step 4: minimum trigram-Jaccard similarity between the
    /// original body and the LLM-rewritten body for the rewrite to be
    /// accepted. Scores below the threshold are rejected and emitted as
    /// `EnrichItemResult::PreservationFailed`. Default 0.7 (per the G29
    /// gap specification). Ignored when `--operation` is not
    /// `body-enrich`.
    #[arg(long, value_name = "FLOAT", default_value_t = 0.7)]
    pub preserve_threshold: f64,

    /// GAP-CLI-ED-03: minimum grounding coverage of an entity description
    /// against linked memory bodies. Uses trigram coverage (`|A∩B|/|A|`),
    /// not symmetric Jaccard, because descriptions are short. Default 0.12.
    /// Set to 0 to use the compiled default. Only applies to
    /// `entity-descriptions`.
    #[arg(long, value_name = "FLOAT", default_value_t = 0.12)]
    pub entity_description_grounding_threshold: f64,

    /// GAP-CLI-ED-06: re-scan entities whose description is empty OR matches
    /// low-quality patterns (e.g. generic "configuration file" / software
    /// jargon). Default false (write-once for non-empty descriptions).
    #[arg(long, default_value_t = false)]
    pub force_redescribe: bool,

    /// Wave 2 / GAP-CLI-OBS-04: sample N entities with descriptions and report
    /// `quality_pct` / `scan_backlog_low_grounding_est` on `--status`.
    /// Precedence: this flag > XDG `enrich.entity_description.quality_sample`
    /// > default 50. Set 0 to disable sampling.
    #[arg(long, value_name = "N")]
    pub quality_sample: Option<usize>,

    /// GAP-CLI-NAMES-02: explicit entity name filter (entity-descriptions,
    /// entity-connect, …). Alias of `--names` for entity-keyed ops.
    #[arg(long, value_name = "NAMES", value_delimiter = ',')]
    pub entity_names: Vec<String>,

    /// GAP-CLI-NAMES-02: explicit memory name filter (memory-bindings,
    /// body-enrich, …). Alias of `--names` for memory-keyed ops.
    #[arg(long, value_name = "NAMES", value_delimiter = ',')]
    pub memory_names: Vec<String>,

    /// GAP-CLI-EC-03: limit entity-connect pair scan to the subgraph of
    /// entities linked to this memory name.
    #[arg(long, value_name = "NAME")]
    pub anchor_memory: Option<String>,

    /// GAP-CLI-ED-05: optional domain label for entity-descriptions
    /// (`auto` = no hint, `none` = force neutral, or free-form e.g. `fiscal`).
    /// Precedence: this flag > XDG `enrich.entity_description.domain` > `auto`.
    #[arg(long, value_name = "LABEL", default_value = "auto")]
    pub entity_description_domain: String,

    /// GAP-CLI-PRIO-05: cooperative yield every N processed items so hot-set
    /// entity-descriptions can preempt long entity-connect drains. 0 disables.
    /// XDG key: `enrich.yield_every_n_items` (default 10).
    #[arg(long, value_name = "N")]
    pub yield_every_n_items: Option<usize>,

    /// GAP-CLI-OBS-02 / PRIO-04: run gate ops in order
    /// memory-bindings → entity-descriptions (before entity-connect).
    #[arg(long, default_value_t = false)]
    pub ops_gate: bool,

    /// G33 Step 3: when set, validate `--codex-model` against the
    /// ChatGPT Pro OAuth accepted-model list and abort with a
    /// suggestion when the value is unknown. Default true (fail fast
    /// to avoid burning OAuth turns). Set to false to opt out.
    #[arg(long, default_value_t = true)]
    pub codex_model_validate: bool,

    /// G33 Step 3: when set together with an invalid `--codex-model`,
    /// automatically substitute the supplied default (e.g. `gpt-5.5`)
    /// instead of aborting. The substitution is recorded in the NDJSON
    /// stream as `provider_substituted: true` for traceability.
    #[arg(long, value_name = "MODEL")]
    pub codex_model_fallback: Option<String>,
}

impl EnrichArgs {
    /// GAP-SG-31: resolved enrichment operation.
    ///
    /// `operation` is `Option` so the read-only queue inspectors
    /// (`--status` / `--list-dead` / `--requeue-dead`) can run without it.
    /// Write paths always carry a value (enforced by
    /// `required_unless_present_any` at parse time); the read-only paths fall
    /// back to `memory-bindings`, the most common queue, when it is omitted.
    pub(crate) fn operation(&self) -> EnrichOperation {
        self.operation
            .clone()
            .unwrap_or(EnrichOperation::MemoryBindings)
    }

    /// GAP-SG-31: resolved LLM provider. `mode` is `Option` for the read-only
    /// inspectors that never call the LLM; write paths always carry a value
    /// (enforced by `required_unless_present_any`). The fallback is only ever
    /// observed by read-only code that does not actually invoke the provider.
    pub(crate) fn mode(&self) -> EnrichMode {
        self.mode.clone().unwrap_or(EnrichMode::OpenRouter)
    }
}
