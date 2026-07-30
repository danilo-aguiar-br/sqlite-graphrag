//! Embedding call timeout helpers (XDG `embedding.timeout_secs`).

use std::process::ExitStatus;

/// Default per-LLM-call timeout in seconds. Set to 300 to align with the
/// ingest, enrich, opencode and llm_backend defaults, which already use a
/// 300-second per-call budget. Override via XDG `embedding.timeout_secs`
/// (`config set embedding.timeout_secs <secs>`).
pub(super) const DEFAULT_EMBED_TIMEOUT_SECS: u64 = 300;

pub(super) fn embed_timeout() -> std::time::Duration {
    let secs = crate::runtime_config::resolve_u64(
        None,
        "embedding.timeout_secs",
        DEFAULT_EMBED_TIMEOUT_SECS,
    );
    let secs = if (10..=3_600).contains(&secs) {
        secs
    } else {
        DEFAULT_EMBED_TIMEOUT_SECS
    };
    std::time::Duration::from_secs(secs)
}

/// v1.0.89 (GAP-4): scales the per-call timeout with batch size.
/// A single-item batch uses the base timeout (120s default).
/// Each additional item adds 15s to account for the LLM generating
/// more embedding vectors in the same call.
#[cfg(test)]
pub(super) fn embed_timeout_for_batch(batch_size: usize) -> std::time::Duration {
    let base = embed_timeout();
    let extra = std::time::Duration::from_secs(15) * batch_size.saturating_sub(1) as u32;
    base + extra
}

/// Cross-platform helper: extracts `(exit_code, signal)` from an
/// `ExitStatus` whose `.code()` returned `None`. On Unix this means
/// the process was killed by a signal; on Windows processes always
/// have an exit code so this branch returns `(None, None)`.
pub(super) fn extract_exit_info(status: &ExitStatus) -> (Option<i32>, Option<i32>) {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        (None, status.signal())
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        (None, None)
    }
}
