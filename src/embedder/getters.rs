//! Embedder client getters and local-backend helpers.

use super::*;
use crate::constants::{EMBED_RUNTIME_MAX_WORKER_THREADS, EMBED_RUNTIME_MIN_WORKER_THREADS};
use crate::errors::AppError;

/// Returns true when the process-wide OpenRouter embed client is ready.
pub fn is_openrouter_initialized() -> bool {
    OPENROUTER_CLIENT.get().is_some()
}

/// Host-derived worker count for the shared embedding runtime, before the XDG
/// override is applied (GAP-SG-141 B2).
///
/// Clamped between [`EMBED_RUNTIME_MIN_WORKER_THREADS`] and
/// [`EMBED_RUNTIME_MAX_WORKER_THREADS`]; a host that cannot report its
/// parallelism falls back to the minimum.
fn default_embed_runtime_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(EMBED_RUNTIME_MIN_WORKER_THREADS)
        .clamp(
            EMBED_RUNTIME_MIN_WORKER_THREADS,
            EMBED_RUNTIME_MAX_WORKER_THREADS,
        )
}

/// Returns the process-wide multi-thread runtime, building it on first use.
///
/// The worker count is never a literal: it comes from
/// [`crate::runtime_config::embed_runtime_worker_threads`], which layers XDG
/// `parallelism.embed_runtime_threads` over [`default_embed_runtime_threads`].
/// A hard-coded two workers starved the reactor once the enrich drain began
/// issuing up to sixteen concurrent blocking calls against it.
pub(crate) fn shared_runtime() -> Result<&'static tokio::runtime::Runtime, AppError> {
    if let Some(rt) = RUNTIME.get() {
        return Ok(rt);
    }
    let workers =
        crate::runtime_config::embed_runtime_worker_threads(default_embed_runtime_threads())
            .max(EMBED_RUNTIME_MIN_WORKER_THREADS);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .map_err(|e| {
            AppError::Embedding(crate::i18n::validation::embedding_tokio_runtime_init_failed(e))
        })?;
    let _ = RUNTIME.set(rt);
    RUNTIME.get().ok_or_else(|| {
        AppError::Embedding(crate::i18n::validation::embedding_tokio_runtime_unavailable())
    })
}

/// Initialises the process-wide OpenRouter embedding client on first use and
/// returns it.
///
/// The per-request timeout resolves in the documented precedence:
/// `timeout_override` (the `--openrouter-timeout` flag) first, then XDG
/// `embedding.timeout_secs`, then the client's own default.
///
/// FIRST INITIALISER WINS. The client lives in a `OnceLock`, so a later call
/// with a different timeout returns the already-built client unchanged. This is
/// sound under the one-shot CLI contract: a single invocation runs a single
/// subcommand, so exactly one timeout is in play per process. Nothing here
/// attempts to rebuild or swap the client, which would race with in-flight
/// requests for no benefit.
pub fn get_openrouter_embedder(
    api_key: secrecy::SecretBox<String>,
    model: &str,
    dim: usize,
    timeout_override: Option<u64>,
) -> Result<&'static crate::embedding_api::OpenRouterClient, AppError> {
    if let Some(c) = OPENROUTER_CLIENT.get() {
        return Ok(c);
    }
    let timeout_secs = crate::runtime_config::resolve_u64(
        timeout_override,
        "embedding.timeout_secs",
        crate::constants::DEFAULT_EMBEDDING_HTTP_TIMEOUT_SECS,
    );
    let client =
        crate::embedding_api::OpenRouterClient::new(api_key, model.to_string(), dim, timeout_secs)?;
    let _ = OPENROUTER_CLIENT.set(client);
    OPENROUTER_CLIENT.get().ok_or_else(|| {
        AppError::Embedding(crate::i18n::validation::embedding_openrouter_client_unavailable())
    })
}

/// v1.0.95 (ADR-0054): initialises the process-wide OpenRouter chat client on
/// first use and returns it. `model` is the text model the enrich JUDGE will
/// call (no default; the caller validates presence upfront).
pub fn get_openrouter_chat_client(
    api_key: secrecy::SecretBox<String>,
    model: &str,
    timeout_secs: u64,
) -> Result<&'static crate::chat_api::OpenRouterChatClient, AppError> {
    if let Some(c) = OPENROUTER_CHAT_CLIENT.get() {
        return Ok(c);
    }
    let client =
        crate::chat_api::OpenRouterChatClient::new(api_key, model.to_string(), timeout_secs)?;
    let _ = OPENROUTER_CHAT_CLIENT.set(client);
    OPENROUTER_CHAT_CLIENT.get().ok_or_else(|| {
        AppError::Embedding(crate::i18n::validation::embedding_openrouter_chat_client_unavailable())
    })
}

/// v1.0.95: returns the process-wide OpenRouter chat client if it has already
/// been initialised via [`get_openrouter_chat_client`]. Used by the enrich
/// JUDGE dispatch, which initialises the singleton once at startup and then
/// fetches it per item without re-threading the API key.
pub fn openrouter_chat_client() -> Option<&'static crate::chat_api::OpenRouterChatClient> {
    OPENROUTER_CHAT_CLIENT.get()
}

#[cfg(test)]
mod runtime_sizing_tests {
    use super::*;

    /// Collects every `.rs` file under `dir`, recursively.
    fn rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// Drops whole-line `//` comments so prose quoting the banned call shape —
    /// including the comment right below — is not read as a call site.
    fn strip_line_comments(source: &str) -> String {
        source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// No runtime in the CRATE may fix its reactor width to a literal.
    ///
    /// SCOPE IS THE CRATE, never this one file. The previous version read its
    /// own source through `include_str!`, so it could only ever police the
    /// runtime built a few lines above it. `src/commands/deep_research` built a
    /// second runtime with `.worker_threads(2)` and the guard stayed green
    /// throughout — a one-file guard against a crate-wide invariant is not a
    /// guard. Every `.rs` file under `src/` is walked instead; files whose name
    /// carries `test` are fixtures and assertions, not runtime construction.
    ///
    /// The historical bug was `.worker_threads(2)`: a fixed reactor width that
    /// the enrich drain then oversubscribed. Any literal digit is that defect
    /// returning, wherever it is written.
    #[test]
    fn runtime_worker_count_is_never_a_literal() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_sources(&root, &mut files);
        assert!(
            files.len() > 50,
            "source walk found only {} files under {}; the guard would pass vacuously",
            files.len(),
            root.display()
        );

        let mut offenders: Vec<String> = Vec::new();
        for path in &files {
            let is_test_file = path
                .file_name()
                .map(|n| n.to_string_lossy().contains("test"))
                .unwrap_or(false);
            if is_test_file {
                continue;
            }
            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let code = strip_line_comments(&source);
            for call in code.split(".worker_threads(").skip(1) {
                let arg = call.split(')').next().unwrap_or_default().trim();
                if arg.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    offenders.push(format!(
                        "{}: .worker_threads({arg})",
                        path.strip_prefix(&root).unwrap_or(path).display()
                    ));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "worker_threads must be resolved through runtime_config (or left to \
             Tokio's own core-count default), never written as a literal:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn host_default_stays_within_the_named_bounds() {
        let n = default_embed_runtime_threads();
        assert!(
            (EMBED_RUNTIME_MIN_WORKER_THREADS..=EMBED_RUNTIME_MAX_WORKER_THREADS).contains(&n),
            "host-derived worker count {n} escaped its clamp"
        );
    }

    #[test]
    fn zero_override_falls_back_to_the_default() {
        // `worker_threads(0)` panics in Tokio, so the reader must never let a
        // zero through.
        assert_eq!(
            crate::runtime_config::embed_runtime_worker_threads(0),
            0,
            "the reader returns the caller's default verbatim when no override is set"
        );
        // The builder therefore applies its own floor on top.
        let workers = crate::runtime_config::embed_runtime_worker_threads(0)
            .max(EMBED_RUNTIME_MIN_WORKER_THREADS);
        assert!(workers >= EMBED_RUNTIME_MIN_WORKER_THREADS);
    }

    #[test]
    fn embed_timeout_flag_outranks_xdg_and_default() {
        // The flag short-circuits before any XDG lookup, so this holds without
        // touching the operator's config file.
        assert_eq!(
            crate::runtime_config::resolve_u64(
                Some(77),
                "embedding.timeout_secs",
                crate::constants::DEFAULT_EMBEDDING_HTTP_TIMEOUT_SECS,
            ),
            77
        );
    }

    #[test]
    fn shared_runtime_builds() {
        assert!(shared_runtime().is_ok());
    }
}
