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
/// `deepseek/deepseek-v4-flash:nitro` (see [`EMBEDDING_REQUEST_MAX_TOKENS`]
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
