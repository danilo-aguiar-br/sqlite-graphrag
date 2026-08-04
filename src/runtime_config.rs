//! Runtime configuration resolved without product environment variables.
//!
//! Precedence (G-T-XDG-04 / plan v4): **CLI flag > XDG `config set` > named default**.
//! Product `SQLITE_GRAPHRAG_*` / `OPENROUTER_*` env vars are **not** read for config.
//! OS env allowed only for process identity: `HOME`, `PATH`, `XDG_*`, locale, `NO_COLOR`.

use crate::config;
use std::sync::OnceLock;

/// Process-wide overrides captured once from CLI flags at bootstrap.
#[derive(Debug, Clone, Default)]
pub struct RuntimeOverrides {
    /// Embedding dim.
    pub embedding_dim: Option<u32>,
    /// LLM model.
    pub llm_model: Option<String>,
    /// LLM fallback.
    pub llm_fallback: Option<String>,
    /// Skip embedding on failure.
    pub skip_embedding_on_failure: bool,
    /// LLM max host concurrency.
    pub llm_max_host_concurrency: Option<usize>,
    /// LLM slot wait secs.
    pub llm_slot_wait_secs: Option<u64>,
    /// LLM slot no wait.
    pub llm_slot_no_wait: bool,
    /// Strict ENV clear.
    /// CLI `--openrouter-timeout`, in seconds.
    ///
    /// Global since v1.2.3. It used to be declared only on `enrich`, so every
    /// other embedding path — `remember`, `ingest`, `edit`, `restore`,
    /// `split-body` and both read paths — was pinned to the compiled default
    /// with no operator recourse, and a slow provider surfaced as exit 11.
    /// `None` means the flag was omitted, which is what lets flag > XDG >
    /// constant resolve instead of a clap default always winning.
    pub openrouter_timeout: Option<u64>,
    /// Log level.
    pub log_level: Option<String>,
    /// Log format.
    pub log_format: Option<String>,
    /// Lang.
    pub lang: Option<String>,
    /// Display TZ.
    pub display_tz: Option<String>,
    /// DB path.
    pub db_path: Option<String>,
}

/// Directory overrides installed BEFORE anything reads `config.toml`.
///
/// These live in their own `OnceLock` because of an ordering hazard: `main`
/// resolves the interface language during a pre-parse pass that runs before
/// [`init`], and language resolution reads the XDG key `i18n.lang` — which
/// means it reads `config.toml`, which means it needs `--config-dir` already.
/// Folding these into [`RuntimeOverrides`] would force [`init`] to run first,
/// and since both are first-wins `OnceLock`s the later call would be dropped.
#[derive(Debug, Clone, Default)]
pub struct PathOverrides {
    /// CLI `--config-dir`: directory holding `config.toml`.
    pub config_dir: Option<String>,
    /// CLI `--cache-dir`: root for lock files, models and cache artifacts.
    pub cache_dir: Option<String>,
}

static PATHS: OnceLock<PathOverrides> = OnceLock::new();

/// Install directory overrides. Idempotent first-wins.
///
/// MUST be called before any code path that can read `config.toml`.
pub fn init_paths(overrides: PathOverrides) {
    let _ = PATHS.set(overrides);
}

fn paths() -> PathOverrides {
    PATHS.get().cloned().unwrap_or_default()
}

static RUNTIME: OnceLock<RuntimeOverrides> = OnceLock::new();

/// Install CLI-captured overrides. Idempotent first-wins (main bootstrap).
pub fn init(overrides: RuntimeOverrides) {
    let _ = RUNTIME.set(overrides);
}

/// Borrow installed overrides (empty defaults if init was skipped — tests).
pub fn get() -> RuntimeOverrides {
    RUNTIME.get().cloned().unwrap_or_default()
}

/// CLI `--config-dir` only.
///
/// Deliberately does NOT consult [`config::get_setting`]: the config file lives
/// inside the directory this function resolves, so reading it here would be
/// circular. The XDG default is applied by [`crate::paths::config_dir`].
pub fn config_dir_override() -> Option<String> {
    paths()
        .config_dir
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// CLI `--cache-dir` > XDG `cache.dir` > `None` (caller applies the OS default).
pub fn cache_dir_override() -> Option<String> {
    if let Some(v) = paths().cache_dir {
        if !v.trim().is_empty() {
            return Some(v.trim().to_string());
        }
    }
    config::get_setting("cache.dir")
        .ok()
        .flatten()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// flag_opt > XDG setting > default.
pub fn resolve_string(flag: Option<&str>, xdg_key: &str, default: &str) -> String {
    if let Some(v) = flag {
        if !v.is_empty() {
            return v.to_string();
        }
    }
    if let Ok(Some(v)) = config::get_setting(xdg_key) {
        if !v.is_empty() {
            return v;
        }
    }
    default.to_string()
}

/// flag_opt > XDG setting > None.
pub fn resolve_optional_string(flag: Option<&str>, xdg_key: &str) -> Option<String> {
    if let Some(v) = flag {
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    config::get_setting(xdg_key)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

/// Parse usize from flag > XDG > default.
pub fn resolve_usize(flag: Option<usize>, xdg_key: &str, default: usize) -> usize {
    if let Some(v) = flag {
        return v;
    }
    if let Ok(Some(v)) = config::get_setting(xdg_key) {
        if let Ok(n) = v.parse::<usize>() {
            return n;
        }
    }
    default
}

/// Parse u64 from flag > XDG > default.
pub fn resolve_u64(flag: Option<u64>, xdg_key: &str, default: u64) -> u64 {
    if let Some(v) = flag {
        return v;
    }
    if let Ok(Some(v)) = config::get_setting(xdg_key) {
        if let Ok(n) = v.parse::<u64>() {
            return n;
        }
    }
    default
}

/// Parse f64 from flag > XDG > default.
pub fn resolve_f64(flag: Option<f64>, xdg_key: &str, default: f64) -> f64 {
    if let Some(v) = flag {
        return v;
    }
    if let Ok(Some(v)) = config::get_setting(xdg_key) {
        if let Ok(n) = v.parse::<f64>() {
            return n;
        }
    }
    default
}

/// Bool: CLI true wins; else XDG "1"/"true"/"yes"; else default.
pub fn resolve_bool(flag_set: bool, xdg_key: &str, default: bool) -> bool {
    if flag_set {
        return true;
    }
    if let Ok(Some(v)) = config::get_setting(xdg_key) {
        let t = v.trim().to_ascii_lowercase();
        return matches!(t.as_str(), "1" | "true" | "yes" | "on");
    }
    default
}

/// Embedding dim: CLI override > XDG `embedding.dim` > None (caller uses DB/default).
///
/// The bound comes from [`crate::constants::EMBEDDING_DIM_RANGE`] so the CLI
/// parser, this resolver and the warning text cannot disagree about what is
/// accepted.
pub fn embedding_dim_override() -> Option<u32> {
    let rt = get();
    if let Some(d) = rt.embedding_dim {
        return Some(d);
    }
    if let Ok(Some(v)) = config::get_setting("embedding.dim") {
        if let Ok(n) = v.parse::<u32>() {
            if crate::constants::EMBEDDING_DIM_RANGE.contains(&(n as usize)) {
                return Some(n);
            }
        }
    }
    None
}

/// Embedding model: CLI `--embedding-model` > XDG `embedding.model` > `None`.
///
/// `--embedding-model` documented this fallback from the day it shipped, but no
/// resolver existed, so `config set embedding.model` was accepted, stored and
/// then ignored — the invocation still died with exit 78 asking for the flag.
///
/// An empty stored value is treated as unset: a blank model name would reach
/// the OpenRouter client and fail there with a far less legible error.
pub fn embedding_model(cli: Option<&str>) -> Option<String> {
    if let Some(m) = cli.map(str::trim).filter(|m| !m.is_empty()) {
        return Some(m.to_string());
    }
    match config::get_setting("embedding.model") {
        Ok(Some(v)) if !v.trim().is_empty() => Some(v.trim().to_string()),
        _ => None,
    }
}

/// Embedding backend: CLI `--embedding-backend` > XDG `embedding.backend` > `auto`.
///
/// `--embedding-backend` advertised "optional XDG `config set
/// embedding.backend`" in its own help while the key was absent from the
/// registry, so the documented command answered exit 1. This is the sibling
/// defect of the one GAP-SG-192 fixed for `embedding.model`.
///
/// An unparseable stored value falls back to the compiled default rather than
/// aborting: a typo in a machine-wide config file must not make every
/// invocation on the host unusable, and the flag still overrides it.
pub fn embedding_backend(
    cli: Option<crate::backend_choice::EmbeddingBackendChoice>,
) -> crate::backend_choice::EmbeddingBackendChoice {
    use crate::backend_choice::EmbeddingBackendChoice as B;
    if let Some(v) = cli {
        return v;
    }
    match config::get_setting("embedding.backend") {
        Ok(Some(v)) => match v.trim().to_ascii_lowercase().as_str() {
            "openrouter" | "open-router" => B::Openrouter,
            "auto" => B::Auto,
            _ => B::Auto,
        },
        _ => B::Auto,
    }
}

/// LLM backend for embedding: CLI `--llm-backend` > XDG `llm.backend` > `open-router`.
///
/// Same defect as [`embedding_backend`]: the flag promised the key and nothing
/// registered or read it.
pub fn llm_backend(
    cli: Option<crate::backend_choice::LlmBackendChoice>,
) -> crate::backend_choice::LlmBackendChoice {
    use crate::backend_choice::LlmBackendChoice as B;
    if let Some(v) = cli {
        return v;
    }
    match config::get_setting("llm.backend") {
        Ok(Some(v)) => match v.trim().to_ascii_lowercase().as_str() {
            "none" => B::None,
            "openrouter" | "open-router" => B::OpenRouter,
            _ => B::OpenRouter,
        },
        _ => B::OpenRouter,
    }
}

/// Skip embedding on failure: runtime flag or XDG.
pub fn skip_embedding_on_failure() -> bool {
    let rt = get();
    resolve_bool(
        rt.skip_embedding_on_failure,
        "llm.skip_embedding_on_failure",
        false,
    )
}

/// Host concurrency for LLM slots.
pub fn llm_max_host_concurrency(default: usize) -> usize {
    let rt = get();
    resolve_usize(
        rt.llm_max_host_concurrency,
        "llm.max_host_concurrency",
        default,
    )
}

/// LLM slot wait secs.
pub fn llm_slot_wait_secs(default: u64) -> u64 {
    let rt = get();
    if rt.llm_slot_no_wait {
        return 0;
    }
    resolve_u64(rt.llm_slot_wait_secs, "llm.slot_wait_secs", default)
}

/// LLM slot no wait.
pub fn llm_slot_no_wait() -> bool {
    let rt = get();
    resolve_bool(rt.llm_slot_no_wait, "llm.slot_no_wait", false)
}

/// LLM model.
pub fn llm_model() -> Option<String> {
    let rt = get();
    resolve_optional_string(rt.llm_model.as_deref(), "llm.model")
}

/// LLM fallback.
pub fn llm_fallback(default: &str) -> String {
    let rt = get();
    resolve_string(rt.llm_fallback.as_deref(), "llm.fallback", default)
}

/// Effective OpenRouter CHAT budget in seconds: `--openrouter-timeout`, then
/// XDG `llm.openrouter_timeout_secs`, then `default`.
///
/// Separate from `embedding.timeout_secs`, which budgets the EMBEDDING client.
/// The same flag feeds both because one invocation talks to one provider, but
/// the two XDG keys stay distinct: a host may need a long chat budget for dense
/// bodies and a short embed budget to keep reads responsive.
pub fn openrouter_chat_timeout_secs(default: u64) -> u64 {
    let rt = get();
    resolve_u64(
        rt.openrouter_timeout,
        "llm.openrouter_timeout_secs",
        default,
    )
}

/// CLI `--openrouter-timeout` as passed, with no XDG or constant layered on.
///
/// The embedding client needs the RAW override because it resolves against its
/// own key (`embedding.timeout_secs`); handing it an already-resolved value
/// would make the flag indistinguishable from a default and silently promote
/// the constant above XDG.
pub fn openrouter_timeout_override() -> Option<u64> {
    get().openrouter_timeout
}

/// Log level.
pub fn log_level(default: &str) -> String {
    let rt = get();
    resolve_string(rt.log_level.as_deref(), "log.level", default)
}

/// Log format.
pub fn log_format(default: &str) -> String {
    let rt = get();
    resolve_string(rt.log_format.as_deref(), "log.format", default)
}

/// Max entities per memory.
pub fn max_entities_per_memory(default: usize) -> usize {
    resolve_usize(None, "limits.max_entities_per_memory", default)
}

/// Max relations per memory.
pub fn max_relations_per_memory(default: usize) -> usize {
    resolve_usize(None, "limits.max_relations_per_memory", default)
}

/// OpenRouter chat URL: XDG override or compile-time default.
/// Canonical key: `network.openrouter.chat_url`; alias: `network.chat_url`.
pub fn openrouter_chat_url(default: &str) -> String {
    resolve_string_with_aliases(
        None,
        &["network.openrouter.chat_url", "network.chat_url"],
        default,
    )
}

/// OpenRouter embeddings URL: XDG override or compile-time default.
/// Canonical key: `network.openrouter.embeddings_url`; alias: `network.embed_url`.
pub fn openrouter_embeddings_url(default: &str) -> String {
    resolve_string_with_aliases(
        None,
        &["network.openrouter.embeddings_url", "network.embed_url"],
        default,
    )
}

/// Probe timeout for fail-fast LLM backend readiness (ms).
pub fn llm_probe_timeout_ms(default: u64) -> u64 {
    resolve_u64(None, "llm.probe_timeout_ms", default)
}

/// Worker count for the global Rayon pool, from XDG `parallelism.rayon_threads`.
///
/// GAP-SG-92: the pool used to be sized by writing `RAYON_NUM_THREADS` into the
/// process environment at startup, which made an env var the configuration
/// channel and required an `unsafe` block. Reading the XDG key and handing the
/// number to `ThreadPoolBuilder` keeps the policy inside the documented
/// precedence and removes the mutation entirely.
///
/// A value of `0` is rejected in favour of `default`: Rayon treats zero as
/// "detect the host CPU count", which silently discards the cap this knob
/// exists to enforce.
pub fn rayon_threads(default: usize) -> usize {
    let n = resolve_usize(None, "parallelism.rayon_threads", default);
    if n == 0 {
        default
    } else {
        n
    }
}

/// Worker count for the shared embedding Tokio runtime, from XDG
/// `parallelism.embed_runtime_threads` (GAP-SG-141 B2).
///
/// The runtime used to be built with a hard-coded two workers while the enrich
/// drain fanned out up to sixteen blocking callers onto it. `default` is the
/// host-derived size computed by the caller; this function only applies the
/// operator override.
///
/// A value of `0` is rejected in favour of `default`: `worker_threads(0)`
/// panics in Tokio, so a typo in the config file would abort the process.
pub fn embed_runtime_worker_threads(default: usize) -> usize {
    let n = resolve_usize(None, "parallelism.embed_runtime_threads", default);
    if n == 0 {
        default
    } else {
        n
    }
}

/// GAP-SG-142: cap on emitted result elements, from `--max-items` or XDG
/// `agent_surface.max_items`.
///
/// `0` keeps every element, which is what makes the shaping surface opt-in:
/// an operator who sets nothing gets the historical envelope unchanged.
pub fn agent_surface_max_items(flag: Option<usize>) -> usize {
    resolve_usize(
        flag,
        "agent_surface.max_items",
        crate::constants::DEFAULT_AGENT_SURFACE_MAX_ITEMS,
    )
}

/// GAP-SG-142: cap on string length in characters, from `--truncate-content`
/// or XDG `agent_surface.truncate_content`. `0` disables truncation.
pub fn agent_surface_truncate_content(flag: Option<usize>) -> usize {
    resolve_usize(
        flag,
        "agent_surface.truncate_content",
        crate::constants::DEFAULT_AGENT_SURFACE_TRUNCATE_CONTENT,
    )
}

/// GAP-SG-142: cap on the serialized envelope in bytes, from
/// `--max-output-bytes` or XDG `agent_surface.max_output_bytes`.
/// `0` disables the ceiling.
pub fn agent_surface_max_output_bytes(flag: Option<usize>) -> usize {
    resolve_usize(
        flag,
        "agent_surface.max_output_bytes",
        crate::constants::DEFAULT_AGENT_SURFACE_MAX_OUTPUT_BYTES,
    )
}

/// SQLITE_BUSY retry budget, from XDG `db.busy_retries`.
pub fn db_busy_retries(default: u32) -> u32 {
    resolve_u64(None, "db.busy_retries", u64::from(default)) as u32
}

/// Base backoff for the first SQLITE_BUSY retry, from XDG `db.busy_base_delay_ms`.
pub fn db_busy_base_delay_ms(default: u64) -> u64 {
    resolve_u64(None, "db.busy_base_delay_ms", default)
}

/// Per-statement query timeout, from XDG `db.query_timeout_ms`.
pub fn db_query_timeout_ms(default: u64) -> u64 {
    resolve_u64(None, "db.query_timeout_ms", default)
}

/// Embedding batch size, from XDG `embedding.batch_size`.
///
/// Clamped to at least 1 so a `0` in the config cannot produce an empty batch
/// loop that never makes progress.
pub fn embedding_batch_size(default: usize) -> usize {
    resolve_usize(None, "embedding.batch_size", default).max(1)
}

/// GAP-SG-141 (B1): how many `ReEmbed` queue rows one claim takes, from XDG
/// `enrich.reembed_claim_batch`.
///
/// There is no CLI flag: the width is a host-tuning concern, not a per-command
/// decision, and the default already matches the 32-item chunk the OpenRouter
/// embedding path uses internally. Values outside
/// [`crate::constants::REEMBED_CLAIM_BATCH_RANGE`] are clamped rather than
/// rejected so a stale config can never stall a drain.
pub fn reembed_claim_batch() -> usize {
    let range = crate::constants::REEMBED_CLAIM_BATCH_RANGE;
    resolve_usize(
        None,
        "enrich.reembed_claim_batch",
        crate::constants::DEFAULT_REEMBED_CLAIM_BATCH,
    )
    .clamp(*range.start(), *range.end())
}

/// GAP-SG-185: keyset page size for enrich scan collectors.
///
/// Precedence: CLI `--scan-page-size` > XDG `enrich.scan_page_size` >
/// [`crate::constants::DEFAULT_ENRICH_SCAN_PAGE_SIZE`]. Values outside
/// [`crate::constants::ENRICH_SCAN_PAGE_SIZE_RANGE`] are clamped.
pub fn enrich_scan_page_size(cli: Option<usize>) -> usize {
    let range = crate::constants::ENRICH_SCAN_PAGE_SIZE_RANGE;
    resolve_usize(
        cli,
        "enrich.scan_page_size",
        crate::constants::DEFAULT_ENRICH_SCAN_PAGE_SIZE,
    )
    .clamp(*range.start(), *range.end())
}

/// Deadline a drain keeps absorbing provider rate limits, from XDG
/// `enrich.rate_limit_deadline_secs`.
///
/// There is no CLI flag: the tolerable quota window is a property of the host's
/// provider account, not of one invocation. `0` is rejected in favour of the
/// default because a zero deadline would abort on the first rate limit, which
/// is the opposite of what this budget exists for.
pub fn rate_limit_deadline_secs() -> std::time::Duration {
    let secs = resolve_u64(
        None,
        "enrich.rate_limit_deadline_secs",
        crate::constants::DEFAULT_RATE_LIMIT_DEADLINE_SECS,
    );
    let secs = if secs == 0 {
        crate::constants::DEFAULT_RATE_LIMIT_DEADLINE_SECS
    } else {
        secs
    };
    std::time::Duration::from_secs(secs)
}

/// Cooldown of a tripped per-worker circuit breaker, from XDG
/// `enrich.circuit_breaker_reset_secs`.
///
/// `0` is rejected in favour of the default: a zero cooldown lets the breaker
/// re-close immediately, which disables the protection instead of tuning it.
pub fn enrich_circuit_breaker_reset_secs() -> std::time::Duration {
    let secs = resolve_u64(
        None,
        "enrich.circuit_breaker_reset_secs",
        crate::constants::DEFAULT_ENRICH_CIRCUIT_BREAKER_RESET_SECS,
    );
    let secs = if secs == 0 {
        crate::constants::DEFAULT_ENRICH_CIRCUIT_BREAKER_RESET_SECS
    } else {
        secs
    };
    std::time::Duration::from_secs(secs)
}

/// Deadline for reading a body from stdin, from XDG `cli.stdin_timeout_secs`.
///
/// `0` is rejected in favour of the default: a zero deadline would make every
/// stdin read fail instantly, which `--no-input` already expresses deliberately.
pub fn stdin_timeout_secs() -> u64 {
    let secs = resolve_u64(
        None,
        "cli.stdin_timeout_secs",
        crate::constants::DEFAULT_STDIN_READ_TIMEOUT_SECS,
    );
    if secs == 0 {
        crate::constants::DEFAULT_STDIN_READ_TIMEOUT_SECS
    } else {
        secs
    }
}

/// Whether this invocation refuses to read stdin: CLI `--no-input` > XDG
/// `cli.no_input` > `false`.
///
/// The flag is one-way by design — passing it turns the refusal on, and a host
/// that opted in through XDG turns it off by unsetting the key rather than by a
/// `--no-input=false`, which would read as "input is allowed here" while the
/// surrounding automation assumes otherwise.
pub fn no_input(flag: bool) -> bool {
    resolve_bool(flag, "cli.no_input", false)
}

/// flag > first non-empty XDG key in `keys` > default.
fn resolve_string_with_aliases(flag: Option<&str>, keys: &[&str], default: &str) -> String {
    if let Some(v) = flag {
        if !v.is_empty() {
            return v.to_string();
        }
    }
    for key in keys {
        if let Ok(Some(v)) = config::get_setting(key) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    default.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_string_prefers_flag() {
        assert_eq!(
            resolve_string(Some("from-flag"), "nonexistent.key.xyz", "def"),
            "from-flag"
        );
    }

    #[test]
    fn resolve_string_falls_to_default() {
        assert_eq!(
            resolve_string(None, "nonexistent.key.xyz.zzz", "def"),
            "def"
        );
    }
}
