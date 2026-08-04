//! Global flag surface: the root `Cli` parser and its validation.
//!
//! Every flag that applies before a subcommand is chosen lives here, together
//! with the cross-flag validation `main` runs before dispatching.

use super::commands::Commands;
use crate::backend_choice::{EmbeddingBackendChoice, LlmBackendChoice};
use crate::i18n::{current, Language};
use clap::Parser;

/// Returns the maximum simultaneous invocations allowed by the CPU heuristic.
fn max_concurrency_ceiling() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get() * 2)
        .unwrap_or(8)
}

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

    // `--strict-env-clear` lived here until v1.2.2. It controlled which
    // environment variables an LLM subprocess inherited, and this release
    // removed every LLM subprocess. The flag still parsed, still flowed into
    // `RuntimeOverrides`, and was read by nothing — the inverse of the dead
    // configuration channel this release also closed: there the text promised a
    // channel the code ignored, here a flag promised an effect it could no
    // longer have. Both are removed rather than documented.
    /// Fail instead of degrading when the query embedding cannot be produced.
    ///
    /// `recall` and `hybrid-search` fall back to FTS5-only ranking when the
    /// provider is unreachable, raise `vec_degraded` on the envelope, and exit
    /// `0`. That is the right default for a human reading results, and the wrong
    /// one for an agent that parses `.results` and never looks at the flags: it
    /// silently receives a keyword search where it asked for a hybrid one.
    ///
    /// Under this flag a degraded read exits non-zero with the usual error
    /// envelope, so the retry verdict travels with it. A degradation the caller
    /// ASKED for with `--fallback-fts-only` is deliberate and never fails.
    #[arg(long, global = true, default_value_t = false)]
    pub fail_on_degraded: bool,

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

    // `--extraction-backend` lived here until v1.2.2, and was the most
    // advertised dead flag in the crate: ten shipped documents described its
    // four values while `src/` held exactly ONE mention of it — this very
    // declaration. It never reached `RuntimeOverrides`, let alone a consumer.
    // Its doc still described selecting between a headless `claude`/`codex`
    // extractor and a `fastembed` pipeline, and BOTH were removed from the
    // product. Found by asking who READS the field rather than how many times
    // the identifier appears, which is the check `tests/inert_flag_guard.rs`
    // now performs for the whole `Cli` struct.
    /// Embedding dimensionality override (default 1024 since v1.2.0).
    ///
    /// Precedence: this flag > XDG `embedding.dim` >
    /// the `dim` recorded in the database `schema_meta` > 1024. Existing
    /// databases keep their recorded dimensionality automatically; use
    /// this flag only to migrate a corpus to a new dimensionality
    /// (followed by `enrich --operation re-embed`). Range: [8, 4096].
    #[arg(long, global = true, value_name = "N", value_parser = clap::value_parser!(u64).range(8..=4096))]
    pub embedding_dim: Option<u64>,

    /// LLM backend for embedding. Accepts `openrouter` (OpenRouter REST)
    /// or `none` (skips embedding; useful for tests). Prefer the flag;
    /// optional XDG `llm.backend` via `config set`.
    ///
    /// Kept `Option` with no `default_value_t`, for the reason `--llm-fallback`
    /// already documents below: a clap default makes the field always `Some`,
    /// which silently swallows the XDG layer the doc promises. The default
    /// lives in [`crate::runtime_config::llm_backend`] instead, so
    /// flag > XDG > `open-router` actually resolves.
    #[arg(long, global = true, value_enum)]
    pub llm_backend: Option<LlmBackendChoice>,

    /// v1.0.82 (GAP-003): model to invoke on the chosen backend.
    /// Prefer the flag; optional XDG `llm.model`.
    #[arg(long, global = true, value_name = "MODEL")]
    pub llm_model: Option<String>,

    /// Chain of LLM backends tried in order when the primary fails.
    ///
    /// Defaults to `none`. The default lives in the runtime registry rather
    /// than in `default_value` here on purpose: a clap default makes the field
    /// always `Some`, which silently swallows the XDG layer the doc promises —
    /// `config set llm.fallback` would have been read by nothing. Leaving it
    /// `None` when unset is what lets flag > XDG > constant actually resolve.
    #[arg(long, global = true)]
    pub llm_fallback: Option<String>,

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

    // GAP-SG-204 (drive-by): the rendered text read "optional XDG XDG
    // `llm.max_host_concurrency`" — a duplicated word from a mechanical edit —
    // and described the ceiling as covering "LLM subprocesses", a thing v1.2.0
    // removed. It bounds concurrent host SLOTS.
    /// Host-wide ceiling of concurrent LLM slots. Default derived from `ncpus`.
    ///
    /// Prefer the flag; optional XDG `llm.max_host_concurrency`.
    #[arg(long, global = true, value_name = "N")]
    pub llm_max_host_concurrency: Option<u32>,

    /// v1.0.82 (GAP-004): seconds to wait for a free LLM slot
    /// before failing with exit 75. Default 30s. Prefer the flag; optional XDG
    /// XDG `llm.slot_wait_secs`.
    #[arg(long, global = true, value_name = "SECONDS")]
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

    /// Embedding backend selector.
    ///
    /// `openrouter` uses the REST API and requires a stored key. `auto` resolves
    /// to the same path when a key is reachable and degrades to no embedding
    /// when it is not. There is no subprocess backend: generation happens over
    /// HTTP, in-process, one shot.
    ///
    /// Prefer the flag; optional XDG `config set embedding.backend`.
    ///
    /// Kept `Option` with no `default_value_t`: a clap default makes the field
    /// always `Some` and the XDG layer promised right above would be read by
    /// nothing. The default lives in
    /// [`crate::runtime_config::embedding_backend`] instead.
    #[arg(long, global = true, value_enum)]
    pub embedding_backend: Option<EmbeddingBackendChoice>,

    /// v1.0.93: embedding model for the OpenRouter API. Required when
    /// `--embedding-backend openrouter`. Prefer the flag; optional XDG `embedding.model`.
    #[arg(long, global = true, value_name = "MODEL")]
    pub embedding_model: Option<String>,

    /// OpenRouter API key for a single invocation.
    ///
    /// Prefer `config add-key --provider openrouter --from-stdin`, which stores
    /// the key at rest under XDG with mode 0600 and keeps it out of both the
    /// shell history and the process table. No environment variable supplies
    /// this value: the product never reads one (G-T-XDG-04).
    #[arg(long, global = true, value_name = "KEY", hide = true)]
    pub openrouter_api_key: Option<String>,

    /// Per-request budget, in seconds, for every OpenRouter call.
    ///
    /// Global because the deadline binds the EMBEDDING client too, and that
    /// client is built once per process at startup. Declared only on `enrich`,
    /// the flag reached the chat path and nothing else: `remember`, `ingest`,
    /// `edit`, `restore` and `split-body` were pinned to the compiled default
    /// with no way to widen it, and a slow provider turned into exit 11 with no
    /// operator recourse. `enrich --openrouter-timeout <N>` keeps working
    /// unchanged, because a clap global argument accepts being written at the
    /// subcommand position.
    ///
    /// Kept optional so an EXPLICIT value is distinguishable from an omitted
    /// one, which is what lets flag > XDG > constant resolve instead of the
    /// flag always winning with a default nobody asked for.
    #[arg(long, global = true, value_name = "SECONDS")]
    pub openrouter_timeout: Option<u64>,

    /// GAP-SG-142: keep only these keys in each result object (comma separated).
    ///
    /// Accepts dotted paths (`stats.total`). Keys missing from an element are
    /// skipped rather than emitted as `null`, so a projection never invents
    /// fields. Envelopes without a result array are projected themselves.
    /// `--fields` is an accepted spelling of the same flag.
    #[arg(
        long,
        visible_alias = "fields",
        global = true,
        value_name = "KEYS",
        value_delimiter = ','
    )]
    pub select: Vec<String>,

    /// GAP-SG-142: keep only result elements satisfying `EXPR`.
    ///
    /// Grammar: `key=value`, `key!=value`, `key~substring` (case-insensitive
    /// containment). `==` is a synonym of `=`. Repeat the flag to conjoin
    /// predicates with AND. A malformed expression fails fast with exit 2 so a
    /// typo is never mistaken for an empty result set. Failure envelopes are
    /// never filtered: `error: true` / `ok: false` always reaches the caller.
    #[arg(long, global = true, value_name = "EXPR")]
    pub filter: Vec<String>,

    /// GAP-SG-142: emit at most N result elements.
    ///
    /// Distinct from the per-subcommand `--limit` and from `-k`, which bound
    /// the *query*; this bounds only what is written to stdout, after
    /// filtering. Precedence: this flag > XDG `agent_surface.max_items` > 0
    /// (no cap).
    ///
    /// The name is not a stylistic choice and no `--limit` alias is offered:
    /// eight subcommands (`related`, `pending`, `pending-embeddings`, `list`,
    /// `export`, `embedding`, `graph entities`, `enrich`) already declare their
    /// own `--limit`, and a global argument sharing that long flag would give
    /// clap two definitions for one name inside those subcommands.
    ///
    /// Applies to EVERY array in the envelope, not only the primary one: an
    /// agent asking for two nodes must not be handed sixty thousand edges
    /// alongside them. `--select` stays on the primary array — see the module
    /// documentation of [`crate::agent_surface`] for why projecting a
    /// heterogeneous secondary array would erase it rather than shrink it.
    #[arg(long, global = true, value_name = "N")]
    pub max_items: Option<usize>,

    /// GAP-SG-142: sort result elements ascending by this key (dotted path).
    ///
    /// Numbers compare numerically, everything else as text. Elements without
    /// the key keep their relative order at the end of the list.
    #[arg(long, global = true, value_name = "KEY")]
    pub sort: Option<String>,

    /// GAP-SG-142: drop later result elements repeating this key's value.
    ///
    /// Elements lacking the key are always kept, since they were never proven
    /// duplicate.
    #[arg(long, global = true, value_name = "KEY")]
    pub dedupe_by: Option<String>,

    /// GAP-SG-142: replace the payload with `{"count": N}`.
    ///
    /// `N` is the number of result elements left after `--filter`,
    /// `--dedupe-by` and `--max-items`.
    #[arg(long, global = true, default_value_t = false)]
    pub count_only: bool,

    /// GAP-SG-142: shorten every string longer than N characters.
    ///
    /// Counts characters, never bytes, so a UTF-8 sequence is never split.
    /// Truncation is recorded under `agent_surface.content_truncated` and
    /// raises the top-level `truncated` flag. Precedence: this flag > XDG
    /// `agent_surface.truncate_content` > 0 (disabled).
    #[arg(long, global = true, value_name = "N")]
    pub truncate_content: Option<usize>,

    /// GAP-SG-142: cap the serialized envelope at N bytes.
    ///
    /// Enforced by dropping trailing result elements until the payload fits —
    /// never by slicing the JSON text, which would not parse. What was dropped
    /// is recorded under `agent_surface.output_truncated` / `dropped`.
    /// Precedence: this flag > XDG `agent_surface.max_output_bytes` > 0
    /// (no ceiling).
    #[arg(long, global = true, value_name = "N")]
    pub max_output_bytes: Option<usize>,

    /// Refuse to read stdin anywhere in this invocation.
    ///
    /// The refusal is declarative, not emergent: without the flag, a stdin path
    /// only fails once the read is attempted (immediately on a TTY, after the
    /// deadline otherwise). With it, `--body-stdin`, `--graph-stdin`,
    /// `remember-batch` and every other stdin reader fail up front with exit 1
    /// (`AppError::Validation`), even when a pipe is attached and would have
    /// supplied data.
    ///
    /// Precedence: this flag > XDG `cli.no_input` > `false`.
    // Until v1.2.4 the paragraph above promised exit 65, and seven
    // operator-facing documents repeated it. No arm of `AppError::exit_code`
    // returns 65, so an agent branching on it never matched this case. The
    // correction lives in a plain comment, not a doc comment: clap renders doc
    // comments as help text, and naming the wrong code — even to disown it —
    // puts the number back in front of the reader `tests/no_input_exit_contract.rs`
    // is there to keep it away from.
    #[arg(long, global = true, default_value_t = false)]
    pub no_input: bool,

    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

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
        self.install_agent_surface()?;
        // Installed here rather than at each call site so the guarantee holds
        // for every stdin reader, present and future, from a single point.
        crate::stdin_helper::install_no_input(crate::runtime_config::no_input(self.no_input));
        Ok(())
    }

    /// GAP-SG-142: resolves the agent-native output surface and installs it
    /// process-wide, so [`crate::output`] can reshape every envelope from a
    /// single point.
    ///
    /// Runs from [`Self::validate_flags`] because that is the one bootstrap
    /// hook already invoked after language and XDG initialisation and before
    /// any subcommand dispatch — exactly the window where a malformed
    /// `--filter` must abort with exit 2 rather than be mistaken for an empty
    /// result set.
    ///
    /// # Errors
    /// Returns the localized parse error of the first malformed `--filter`.
    fn install_agent_surface(&self) -> Result<(), String> {
        let mut filters = Vec::with_capacity(self.filter.len());
        for raw in &self.filter {
            filters.push(crate::agent_surface::filter::FilterExpr::parse(raw)?);
        }
        crate::agent_surface::init(crate::agent_surface::AgentSurface {
            // Alias suppression is only correct for the subcommand that declared
            // the alias, so the surface has to know which one emitted the
            // envelope. Resolvable here at zero cost because `validate_flags`
            // already runs on the parsed `Cli`, after dispatch is decided and
            // before any command runs.
            command: self
                .command
                .as_ref()
                .and_then(Commands::agent_surface_slug)
                .map(str::to_string),
            select: self.select.clone(),
            filters,
            sort: self.sort.clone(),
            dedupe_by: self.dedupe_by.clone(),
            max_items: crate::runtime_config::agent_surface_max_items(self.max_items),
            count_only: self.count_only,
            truncate_content: crate::runtime_config::agent_surface_truncate_content(
                self.truncate_content,
            ),
            max_output_bytes: crate::runtime_config::agent_surface_max_output_bytes(
                self.max_output_bytes,
            ),
        });
        Ok(())
    }
}
