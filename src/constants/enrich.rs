//! Token budgets, fan-out and pacing for the `enrich` pipeline.
//!
//! Split out of the former single-file `constants.rs` in v1.2.5;
//! every item is re-exported by the parent module, so `crate::constants::X`
//! resolves exactly as before.

/// GAP-SG-185 / v1.2.4: default keyset page size for enrich scan collectors.
///
/// SQL row buffers and production page→enqueue key buffers are this wide; the
/// sidecar queue still stores one row per eligible item. Override via CLI
/// `--scan-page-size` or XDG `enrich.scan_page_size`.
pub const DEFAULT_ENRICH_SCAN_PAGE_SIZE: usize = 512;

/// Accepted range for `enrich.scan_page_size` / `--scan-page-size`.
pub const ENRICH_SCAN_PAGE_SIZE_RANGE: std::ops::RangeInclusive<usize> = 1..=4096;

/// Initial `max_tokens` budget sent on an `enrich` chat-completion request
/// (GAP-SG-70/71).
///
/// Chosen well below [`ENRICH_MAX_TOKENS_CEILING`] so a well-formed response
/// completes in one attempt for the common case; only bodies that need more
/// room trigger the growth loop below.
pub const ENRICH_INITIAL_MAX_TOKENS: u32 = 4_096;

/// Multiplier applied to `max_tokens` each time OpenRouter reports
/// `finish_reason: "length"` on an `enrich` chat-completion (GAP-SG-70/71).
pub const ENRICH_MAX_TOKENS_GROWTH_FACTOR: u32 = 2;

/// Upper bound on `max_tokens` growth for an `enrich` chat-completion
/// (GAP-SG-70/71).
///
/// Kept with margin under the ~32K-token context ceiling of
/// `deepseek/deepseek-v4-flash:nitro` (see [`crate::constants::EMBEDDING_REQUEST_MAX_TOKENS`]
/// for the equivalent embedding-side ceiling) so growth never requests a
/// budget the model cannot honour.
pub const ENRICH_MAX_TOKENS_CEILING: u32 = 16_384;

/// Maximum number of `max_tokens`-growth re-attempts after a truncated
/// (`finish_reason: "length"`) `enrich` chat-completion, before giving up and
/// returning the truncation as an error (GAP-SG-70/71).
pub const ENRICH_MAX_LENGTH_RETRIES: u32 = 2;

/// Default REST fan-out for `enrich --mode openrouter` when `--rest-concurrency`
/// is omitted (GAP-SG-141).
///
/// The clap parser clamps the flag to `1..=16`; this constant is the value used
/// when the operator passes nothing, keeping the default out of an inline
/// `unwrap_or` at the call site.
pub const DEFAULT_ENRICH_REST_CONCURRENCY: u32 = 8;

/// Lowest REST fan-out `enrich --mode openrouter` accepts (GAP-SG-266).
///
/// One means serial. Zero would mean "no worker", which is not a slower drain
/// but an absent one, so the floor is a refusal and not a preference.
pub const MIN_ENRICH_REST_CONCURRENCY: u32 = 1;

/// Highest REST fan-out `enrich --mode openrouter` accepts (GAP-SG-266).
///
/// The ceiling protects the shared OpenRouter quota, which is a HOST-scoped
/// scarcity: the key lives once in `~/.config/sqlite-graphrag/config.toml` and
/// every folder on the machine spends from it. It is enforced twice on purpose
/// — the clap parser REFUSES an out-of-range flag, and
/// `commands::enrich::events::parallelism` clamps whatever reaches it, so a
/// caller that bypasses the parser still cannot exceed the ceiling. That module
/// is an internal hook with no public page, so this is a code span and not a
/// link.
pub const MAX_ENRICH_REST_CONCURRENCY: u32 = 16;

/// Default subprocess worker count for `enrich` when `--llm-parallelism` is
/// omitted (GAP-SG-141).
///
/// `1` means serial. The flag is inert under `--mode openrouter`, where
/// [`DEFAULT_ENRICH_REST_CONCURRENCY`] governs fan-out instead.
pub const DEFAULT_ENRICH_LLM_PARALLELISM: u32 = 1;

/// Linked entities pulled into the prompt context when enriching a body.
pub const K_ENRICH_BODY_CONTEXT_ENTITIES_LIMIT: usize = 10;

/// Cooldown, in seconds, before a tripped per-worker circuit breaker allows the
/// next attempt (`enrich` parallel drain).
///
/// One minute is the smallest window in which a provider outage plausibly
/// resolves; shorter turns the breaker into a no-op, longer strands healthy
/// workers. The trip *threshold* is the per-run knob
/// (`--circuit-breaker-threshold`); this window is the host-tuning companion,
/// so it is XDG-only: `enrich.circuit_breaker_reset_secs`, resolved by
/// [`crate::runtime_config::enrich_circuit_breaker_reset_secs`].
pub const DEFAULT_ENRICH_CIRCUIT_BREAKER_RESET_SECS: u64 = 60;

/// Poll interval, in milliseconds, of the watchdog that interrupts an
/// over-budget enrich scan.
///
/// Bounds how far past its deadline a scan can run. Fifty milliseconds is
/// imperceptible against a scan measured in seconds while keeping the watchdog
/// thread idle. Coordination wait, so it takes no XDG key.
pub const ENRICH_SCAN_WATCHDOG_POLL_MS: u64 = 50;

/// Nap, in seconds, taken by an `--until-empty` drain when every remaining item
/// is still serving its backoff.
///
/// The loop has nothing to claim, so it sleeps rather than re-querying SQLite in
/// a tight loop; the real stopping conditions are `--max-runtime` and
/// convergence. Coordination wait, so it takes no XDG key.
pub const ENRICH_UNTIL_EMPTY_IDLE_NAP_SECS: u64 = 1;

/// Ceiling, in seconds, on the exponential backoff a rate-limited drain waits
/// between attempts.
///
/// The doubling in both drains used to stop at a bare `900` written twice, once
/// per drain. Two literals of the same policy is one edit away from two
/// policies, and the serial and parallel paths would then disagree about how
/// long a rate limit is allowed to stall a run. Coordination wait against a
/// remote limit, so it takes no XDG key: the tunable the operator actually has
/// is `enrich.rate_limit_deadline_secs`, which bounds the whole wait rather
/// than one step of it.
pub const ENRICH_BACKOFF_CEILING_SECS: u64 = 900;

/// Default wall-clock budget, in seconds, for one `enrich` run when
/// `--max-runtime` is omitted.
///
/// Previously an unnamed `3600` inside `unwrap_or`, contradicted by its own
/// doc-comment two lines above; naming it is what lets the help text and the
/// default be read from the same place.
pub const DEFAULT_ENRICH_MAX_RUNTIME_SECS: u64 = 3_600;

/// Number of chars of a memory body shown to the model as a PREVIEW.
///
/// GAP-SG-279 measured six body truncations across the enrich modules carrying
/// three different literals — 500, 2000 and 200 — none of them named. The
/// divergence was invisible because each site read as a local decision; taken
/// together they meant the same operation class showed the model wildly
/// different amounts of the same corpus. Preview is the smallest of the three
/// roles: enough to identify a memory, never enough to reason from.
pub const ENRICH_BODY_PREVIEW_CHARS: usize = 500;

/// Number of chars of a memory body sent when the body ITSELF is the subject.
///
/// Used where the model must reason over the body rather than recognise it —
/// synthesis and extraction — so it is four times the preview budget.
pub const ENRICH_BODY_SUBJECT_CHARS: usize = 2_000;

/// Number of chars of a memory body kept in a LOG or diagnostic line.
///
/// Smallest of the three roles: this text is never sent to a model, it only
/// has to let a human recognise which memory a line refers to.
pub const ENRICH_BODY_LOG_PREVIEW_CHARS: usize = 200;

/// Minimum description length, in chars, below which a description is judged
/// generic and eligible for rewriting.
///
/// Lived inside the text of `GENERIC_DESCRIPTION_PREDICATE` as a bare `30`. A
/// number embedded in a SQL string is neither typed nor greppable: changing the
/// policy meant editing prose, and nothing connected it to the quality report
/// that acts on the same idea.
pub const ENRICH_GENERIC_DESCRIPTION_MAX_CHARS: usize = 30;

/// Relationship weight at or above which an edge counts as HIGH weight.
///
/// Same defect as the constant above: it lived as `0.7` inside a
/// `const &str` holding SQL, so it was formally a constant and practically a
/// literal — untyped, and impossible to reuse from the Rust side.
pub const ENRICH_HIGH_WEIGHT_THRESHOLD: f64 = 0.7;

/// Weight given to the accept rate when blending it with mean grounding score
/// into a single quality figure.
///
/// The blend was written as `0.5 * accept_rate + 0.5 * mean_score` with both
/// halves anonymous. Naming one of them states that the two signals are
/// deliberately equal rather than accidentally so, and gives the pair a single
/// place to change.
pub const ENRICH_QUALITY_ACCEPT_RATE_WEIGHT: f64 = 0.5;

/// Weight given to the mean grounding score in the same blend.
///
/// Must sum to one with [`ENRICH_QUALITY_ACCEPT_RATE_WEIGHT`]; the test beside
/// the blend asserts it.
pub const ENRICH_QUALITY_MEAN_SCORE_WEIGHT: f64 = 0.5;
