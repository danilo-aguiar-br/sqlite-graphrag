//! Concurrency permits, memory budgets and slot pacing.
//!
//! Split out of the former single-file `constants.rs` in v1.2.5;
//! every item is re-exported by the parent module, so `crate::constants::X`
//! resolves exactly as before.

// G46: FASTEMBED_MODEL_DEFAULT removed — the fastembed model was deleted in
// v1.0.76 (LLM-only build); `schema_meta.model` now records the CLI version.

/// Default worker count for the global Rayon pool.
///
/// Each worker holds one batch-embedding call in flight, so a wider pool buys
/// no throughput on the LLM-only path and risks RSS oversubscription on a
/// 4-8 GiB host. Override via XDG `parallelism.rayon_threads`.
pub const DEFAULT_RAYON_THREADS: usize = 2;

/// Default value injected into ORT_NUM_THREADS when not set by the user.
pub const ORT_NUM_THREADS_DEFAULT: &str = "1";

/// Default value injected into ORT_INTRA_OP_NUM_THREADS when not set.
pub const ORT_INTRA_OP_NUM_THREADS_DEFAULT: &str = "1";

/// Default value injected into OMP_NUM_THREADS when not set by the user.
pub const OMP_NUM_THREADS_DEFAULT: &str = "1";

/// Polling interval in milliseconds used by `--wait-lock` between `try_lock_exclusive` attempts.
pub const CLI_LOCK_POLL_INTERVAL_MS: u64 = 500;

/// Maximum number of CLI instances running simultaneously.
///
/// Limits the counting
/// semaphore in [`crate::lock`] to prevent memory overload when multiple parallel
/// v1.0.75 (G18 solution): removed the rigid 4-slot ceiling. The adaptive
/// `calculate_safe_concurrency` function in [`crate::lock`]` now reports
/// the dynamic limit. This constant is preserved as a *legacy fallback*
/// when the dynamic calculation cannot be performed (e.g. when `sysinfo`
/// cannot read `/proc/meminfo`).
///
/// Operators should prefer passing `--max-concurrency` explicitly OR
/// letting the runtime compute the limit. The default ceiling is intentionally
/// higher (16) so the legacy 4-slot hard cap does not silently reappear.
pub const MAX_CONCURRENT_CLI_INSTANCES: usize = 16;

/// Memory assumed available when the LLM slot default is computed without
/// `sysinfo` at hand.
///
/// Deliberately conservative. `lock::calculate_safe_concurrency` is the source
/// of truth whenever exact memory data is available; this only keeps the
/// fallback in the same order of magnitude.
pub const LLM_SLOT_ASSUMED_AVAILABLE_MB: u32 = 4096;

/// How long the LLM slot acquirer sleeps between polls while every slot is busy.
///
/// Short enough that a freed slot is picked up promptly, long enough that a
/// waiting process does not spin on the lock.
pub const LLM_SLOT_POLL_INTERVAL_MS: u64 = 100;

/// G28-B (v1.0.68): polling interval in milliseconds used by
/// `acquire_job_singleton` between retry attempts when another invocation
/// already holds the singleton for `(job_type, namespace)`.
pub const JOB_SINGLETON_POLL_INTERVAL_MS: u64 = 1000;

/// Minimum available memory in MiB required before starting model loading.
///
/// If `sysinfo::System::available_memory() / 1_048_576` falls below this value,
/// the invocation is aborted with [`crate::errors::AppError::LowMemory`]
/// (exit code [`crate::constants::LOW_MEMORY_EXIT_CODE`]).
pub const MIN_AVAILABLE_MEMORY_MB: u64 = 2_048;

/// Maximum process RSS in MiB before aborting embedding operations.
/// Users can override via `--max-rss-mb`. Set to 8 GiB by default.
pub const DEFAULT_MAX_RSS_MB: u64 = 8_192;

/// Maximum time in seconds an instance waits to acquire a concurrency slot.
///
/// Passed as the default for `--wait-lock` in the CLI. After exhausting this limit,
/// the invocation returns [`crate::errors::AppError::AllSlotsFull`] with exit code
/// [`crate::constants::CLI_LOCK_EXIT_CODE`] (75).
pub const CLI_LOCK_DEFAULT_WAIT_SECS: u64 = 300;

/// DEFAULT expected RSS, in MiB, budgeted for one LLM/REST worker.
///
/// # This number was NOT measured empirically
///
/// It is a v1.0.75 (G18 + G23) engineering estimate for a worker that used to
/// spawn a subprocess, kept after the move to the OpenRouter REST client
/// because the REST footprint is strictly smaller and the estimate therefore
/// stays conservative. No benchmark, profile or RSS sample backs the exact
/// value 350, and no test asserts it against a measurement. Treat it as a
/// deliberately pessimistic budget, not as data.
///
/// It governs every concurrency ceiling derived from free memory
/// ([`crate::memory_guard::calculate_safe_concurrency`],
/// [`crate::llm_slots::default_max_concurrency`],
/// [`crate::embedder::effective_permits`]), so an operator who has measured the
/// real footprint on their host SHOULD override it rather than live with the
/// estimate: read it through [`llm_worker_rss_mb`], never directly.
pub const LLM_WORKER_RSS_MB: u64 = 350;

/// Per-worker RSS budget in MiB: XDG `llm.worker_rss_mb` or
/// [`LLM_WORKER_RSS_MB`].
///
/// The knob exists because the default is an estimate and not a measurement
/// (see [`LLM_WORKER_RSS_MB`]). `0` is rejected in favour of the default: a zero
/// budget would make every `available_mb / per_worker` division either panic or
/// authorise unbounded concurrency.
pub fn llm_worker_rss_mb() -> u64 {
    crate::config::get_setting("llm.worker_rss_mb")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(LLM_WORKER_RSS_MB)
}

/// DEFAULT joint ceiling on `max_concurrency × llm_parallelism` for one host.
///
/// The two knobs are validated independently — `--max-concurrency` against
/// `2 × nCPUs` and `--llm-parallelism` against 32 — so nothing used to stop
/// their PRODUCT from authorising `2 × nCPUs × 32` in-flight workers, which on a
/// 16-core host is 1024. This constant is the missing joint bound; the per-knob
/// ceilings stay exactly as they are and this one only clamps the product.
///
/// Read it through [`max_total_llm_workers`], never directly.
pub const MAX_TOTAL_LLM_WORKERS: usize = 64;

/// Joint worker ceiling: XDG `parallelism.max_total_workers` or
/// [`MAX_TOTAL_LLM_WORKERS`]. `0` falls back to the default.
pub fn max_total_llm_workers() -> usize {
    crate::config::get_setting("parallelism.max_total_workers")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(MAX_TOTAL_LLM_WORKERS)
}

/// Per-process fan-out width still allowed once `max_concurrency` processes are
/// counted against [`max_total_llm_workers`].
///
/// Pure and total: `max_concurrency` of `0` is read as `1`, and the result never
/// drops below `1` — a joint cap that forbade all work would be a deadlock, not
/// a safety bound.
pub fn joint_parallelism_ceiling_for(max_concurrency: usize) -> usize {
    (max_total_llm_workers() / max_concurrency.max(1)).max(1)
}

/// Joint fan-out ceiling published by `main` once `--max-concurrency` resolves.
///
/// `0` means "never published", which is the case for every unit test and for
/// any embedded consumer of the library that does not go through `main`.
static JOINT_PARALLELISM_CEILING: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Publishes the joint fan-out ceiling derived from the resolved
/// `--max-concurrency` (called once, from `main`).
pub fn set_joint_parallelism_ceiling(ceiling: usize) {
    JOINT_PARALLELISM_CEILING.store(ceiling.max(1), std::sync::atomic::Ordering::Release);
}

/// Joint fan-out ceiling in force for this process.
///
/// Falls back to [`max_total_llm_workers`] when `main` never published one, so a
/// library consumer is bounded by the joint cap alone rather than by an
/// accidental `1`.
pub fn joint_parallelism_ceiling() -> usize {
    let published = JOINT_PARALLELISM_CEILING.load(std::sync::atomic::Ordering::Acquire);
    if published == 0 {
        max_total_llm_workers()
    } else {
        published
    }
}

/// Minimum interval, in seconds, between two `/proc/loadavg` reads.
///
/// The saturation check is consulted before every spawn decision, so an
/// unthrottled read would issue one syscall per decision for a value that
/// changes on a one-minute average. Throttle, not a deadline, so it takes no
/// XDG key.
pub const SYSTEM_LOAD_REFRESH_INTERVAL_SECS: u64 = 1;

/// Deadline, in seconds, a drain keeps absorbing provider rate limits before
/// giving up on the run.
///
/// One hour is long enough to ride out a provider quota window without a human,
/// and short enough that a wedged run does not hold a job singleton overnight.
/// Shared by `enrich` (serial and parallel drains) and `ingest-codex`, which had
/// three independent copies of the same literal.
///
/// Operational policy, so it is configurable: XDG
/// `enrich.rate_limit_deadline_secs`, resolved by
/// [`crate::runtime_config::rate_limit_deadline_secs`].
pub const DEFAULT_RATE_LIMIT_DEADLINE_SECS: u64 = 3_600;

/// Deadline, in seconds, for reading a memory body from stdin.
///
/// The `stdin_helper` doc comment has promised "default 60s" since the module
/// was written while every call site passed the literal; this constant makes
/// the promise real. Sixty seconds is generous for a pipe that is already
/// producing and short enough that a held-open pipe fails inside an agent turn.
///
/// Operational policy, so it is configurable: XDG `cli.stdin_timeout_secs`,
/// resolved by [`crate::runtime_config::stdin_timeout_secs`].
pub const DEFAULT_STDIN_READ_TIMEOUT_SECS: u64 = 60;

/// Poll interval, in seconds, of the `deadlock-detection` watchdog thread.
///
/// Short enough to catch a deadlock inside an interactive test, long enough to
/// keep tracing quiet during normal operation. Diagnostic scaffolding behind a
/// cargo feature, never a production deadline, so it takes no XDG key.
pub const DEADLOCK_CHECK_INTERVAL_SECS: u64 = 10;

/// Pause, in milliseconds, appended to a cooperative yield between enrich
/// batches (PRIO-05).
///
/// `yield_now` alone is advisory and some schedulers ignore it; one millisecond
/// guarantees the descheduling without measurably slowing the drain. Scheduler
/// hint, so it takes no XDG key.
pub const COOPERATIVE_YIELD_SLEEP_MS: u64 = 1;
