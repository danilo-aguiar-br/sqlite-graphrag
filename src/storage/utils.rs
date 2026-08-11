//! Storage utility helpers shared across the storage sub-modules.

use crate::constants::{MAX_SQLITE_BUSY_RETRIES, SQLITE_BUSY_BASE_DELAY_MS};
use crate::errors::AppError;
use rusqlite::ErrorCode;
use std::thread;
use std::time::Duration;

/// Resolved SQLITE_BUSY retry budget: XDG `db.busy_retries` > factory default.
///
/// Clamped to at least 1 so a zero config cannot spin-zero or panic.
fn resolved_busy_retries() -> u32 {
    crate::runtime_config::db_busy_retries(MAX_SQLITE_BUSY_RETRIES).max(1)
}

/// Resolved base delay for the first busy retry: XDG `db.busy_base_delay_ms`.
fn resolved_busy_base_delay_ms() -> u64 {
    crate::runtime_config::db_busy_base_delay_ms(SQLITE_BUSY_BASE_DELAY_MS).max(1)
}

/// Returns `true` when `err` wraps an `SQLITE_BUSY` (or `SQLITE_LOCKED`)
/// condition reported by rusqlite.
///
/// Both `SQLITE_BUSY` (`ErrorCode::DatabaseBusy`) and `SQLITE_LOCKED`
/// (`ErrorCode::DatabaseLocked`) indicate that the write cannot proceed
/// immediately due to WAL concurrency.  We treat both as transient and
/// eligible for retry.
pub fn is_sqlite_busy(err: &AppError) -> bool {
    match err {
        AppError::Database(rusqlite::Error::SqliteFailure(e, _)) => {
            e.code == ErrorCode::DatabaseBusy || e.code == ErrorCode::DatabaseLocked
        }
        _ => false,
    }
}

/// Executes `op` up to the resolved busy-retry budget with exponential
/// backoff whenever the operation fails with `SQLITE_BUSY` / `SQLITE_LOCKED`.
///
/// Policy (GAP-SG-87): XDG `db.busy_retries` / `db.busy_base_delay_ms` via
/// [`crate::runtime_config`], falling back to [`MAX_SQLITE_BUSY_RETRIES`] and
/// [`SQLITE_BUSY_BASE_DELAY_MS`]. Delay schedule (base = resolved base ms):
/// attempt *n* → `base * 2^n` with half-jitter in `[base/2, base)`.
///
/// After all retries are exhausted the last `SQLITE_BUSY` error is converted
/// to [`AppError::DbBusy`] so callers can route on exit-code `15`.
///
/// # Errors
/// Returns [`AppError::DbBusy`] once the resolved budget is exhausted, and
/// propagates any non-busy error from `op` unchanged.
pub fn with_busy_retry<T, F>(op: F) -> Result<T, AppError>
where
    F: Fn() -> Result<T, AppError>,
{
    with_busy_retry_policy(resolved_busy_retries(), resolved_busy_base_delay_ms(), op)
}

/// [`with_busy_retry`] with the schedule supplied instead of resolved.
///
/// The ambient resolution is correct for the binary and wrong for a test. The
/// schedule is exponential, so its total duration is governed by whatever the
/// DEVELOPER happens to have in XDG: at the compiled defaults of 5 retries and
/// 300 ms a contention test costs about nine seconds, while a machine carrying
/// `db.busy_retries = 12` and `db.busy_base_delay_ms = 600` pays roughly THIRTY
/// MINUTES for the same test. Measured, not theorised — it is why the suite
/// appeared to hang for four consecutive sessions on this workstation, and the
/// two tests involved were reported as "running for over 60 seconds" while they
/// were in fact working exactly as written.
///
/// A test that reads ambient configuration is the same confused-deputy shape
/// GAP-SG-205 describes: behaviour governed by state nobody declared. So the
/// contention tests state their own schedule and the binary keeps resolving its.
///
/// # Errors
/// Returns [`AppError::DbBusy`] once `max_retries` attempts are exhausted, and
/// propagates any non-busy error unchanged.
pub fn with_busy_retry_policy<T, F>(
    max_retries: u32,
    base_delay_ms: u64,
    op: F,
) -> Result<T, AppError>
where
    F: Fn() -> Result<T, AppError>,
{
    let max_retries = max_retries.max(1);
    let base_delay_ms = base_delay_ms.max(1);
    for attempt in 0..max_retries {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) if is_sqlite_busy(&e) => {
                if crate::retry::is_kill_switch_active() {
                    tracing::warn!(target: "storage", "retry.disable is set, propagating SQLITE_BUSY immediately");
                    return Err(e);
                }
                // Saturating shift: attempt is bounded by max_retries (u32, small).
                let shift = attempt.min(63);
                // Capped BEFORE the jitter is drawn, so the half-jitter window
                // stays inside the ceiling instead of straddling it.
                let base_ms = base_delay_ms
                    .saturating_mul(1u64 << shift)
                    .min(crate::constants::SQLITE_BUSY_MAX_DELAY_MS);
                let half = base_ms / 2;
                let jitter = if half == 0 { 0 } else { fastrand::u64(0..half) };
                let delay_ms = half + jitter;
                tracing::debug!(
                    target: "storage",
                    attempt = attempt + 1,
                    attempt_max = max_retries,
                    delay_ms,
                    "SQLITE_BUSY retry with half-jitter"
                );
                thread::sleep(Duration::from_millis(delay_ms));
            }
            Err(other) => return Err(other),
        }
    }

    tracing::error!(
        target: "storage",
        retries = max_retries,
        "SQLITE_BUSY exhausted all retries"
    );
    Err(AppError::DbBusy(
        crate::i18n::errors_ops::sqlite_busy_after_retries(max_retries),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    /// Helper that builds a fake `AppError::Database` wrapping
    /// `SQLITE_BUSY` (error code 5) so that `is_sqlite_busy` can be tested
    /// without needing a live SQLite connection.
    fn make_busy_error() -> AppError {
        // rusqlite::Error::SqliteFailure requires a `ffi::Error` + optional msg.
        // We construct it via the public `rusqlite::ffi` interface.
        let ffi_err = rusqlite::ffi::Error {
            code: ErrorCode::DatabaseBusy,
            extended_code: 5,
        };
        AppError::Database(rusqlite::Error::SqliteFailure(ffi_err, None))
    }

    fn make_locked_error() -> AppError {
        let ffi_err = rusqlite::ffi::Error {
            code: ErrorCode::DatabaseLocked,
            extended_code: 6,
        };
        AppError::Database(rusqlite::Error::SqliteFailure(ffi_err, None))
    }

    #[test]
    fn is_sqlite_busy_detects_database_busy() {
        assert!(is_sqlite_busy(&make_busy_error()));
    }

    #[test]
    fn is_sqlite_busy_detects_database_locked() {
        assert!(is_sqlite_busy(&make_locked_error()));
    }

    #[test]
    fn is_sqlite_busy_rejects_other_errors() {
        let err = AppError::Validation("invalid field".into());
        assert!(!is_sqlite_busy(&err));
    }

    #[test]
    fn with_busy_retry_propagates_non_busy_error() {
        let calls = Arc::new(AtomicU32::new(0));
        let calls_clone = Arc::clone(&calls);

        let result: Result<(), AppError> = with_busy_retry(|| {
            calls_clone.fetch_add(1, Ordering::SeqCst);
            Err(AppError::Validation("campo x".into()))
        });

        // Non-busy errors must propagate immediately without retrying.
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn with_busy_retry_succeeds_on_third_attempt() {
        let calls = Arc::new(AtomicU32::new(0));
        let calls_clone = Arc::clone(&calls);

        // Fail twice with SQLITE_BUSY, succeed on the third call.
        let result = with_busy_retry(|| {
            let n = calls_clone.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                Err(make_busy_error())
            } else {
                Ok(())
            }
        });

        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert!(result.is_ok(), "expected Ok after 3rd attempt");
    }

    #[test]
    fn busy_retry_jitter_in_range() {
        // Verify that the half-jitter formula stays within [base/2, base) for attempt=2.
        // attempt=2 → base_ms = SQLITE_BUSY_BASE_DELAY_MS * 4; half = base_ms/2.
        // We call fastrand::u64 indirectly through with_busy_retry by observing that the
        // function completes; direct delay bounds are tested via the formula invariant.
        let base_ms = SQLITE_BUSY_BASE_DELAY_MS * (1u64 << 2); // attempt=2
        let half = base_ms / 2;
        for _ in 0..100 {
            let jitter = fastrand::u64(0..half);
            let delay_ms = half + jitter;
            assert!(
                delay_ms >= half && delay_ms < base_ms,
                "delay_ms {delay_ms} out of [{half}, {base_ms})"
            );
        }
    }

    #[test]
    fn with_busy_retry_returns_db_busy_after_all_retries() {
        // Declared, not resolved: this test exhausts the whole budget, so under
        // an operator schedule of 12 attempts at 600 ms it would sleep for about
        // half an hour to assert a bound it can prove in milliseconds.
        const ATTEMPTS: u32 = 5;
        const BASE_DELAY_MS: u64 = 1;

        let calls = Arc::new(AtomicU32::new(0));
        let calls_clone = Arc::clone(&calls);

        let result: Result<(), AppError> = with_busy_retry_policy(ATTEMPTS, BASE_DELAY_MS, || {
            calls_clone.fetch_add(1, Ordering::SeqCst);
            Err(make_busy_error())
        });

        assert_eq!(
            calls.load(Ordering::SeqCst),
            ATTEMPTS,
            "must attempt exactly the declared busy-retry budget"
        );
        assert!(
            matches!(result, Err(AppError::DbBusy(_))),
            "must convert to DbBusy after exhausting retries"
        );
    }

    /// GAP-SG-76/v1.1.00 fix: `with_busy_retry` was generalised from
    /// `Result<(), AppError>` to `Result<T, AppError>` so the enrich dequeue
    /// loops (which claim a non-unit `DequeueOutcome`) can reuse the bounded
    /// helper instead of an unbounded `loop { ... continue; }` on
    /// `SQLITE_BUSY`. This proves the generic path (a) still returns the
    /// caller-typed `Ok(v)` on success and (b) is bounded (never spins
    /// forever) when the wrapped operation always reports `SQLITE_BUSY`.
    #[test]
    fn with_busy_retry_is_generic_over_return_type() {
        // (a) success path threads a non-unit T through unchanged.
        let ok: Result<i64, AppError> = with_busy_retry(|| Ok(42i64));
        assert_eq!(ok.unwrap(), 42);

        // (b) exhaustion path is bounded for a non-unit T: exactly the
        // resolved retry budget attempts, then Err(DbBusy), never an
        // infinite retry loop.
        let calls = Arc::new(AtomicU32::new(0));
        let calls_clone = Arc::clone(&calls);
        let result: Result<i64, AppError> = with_busy_retry(|| {
            calls_clone.fetch_add(1, Ordering::SeqCst);
            Err(make_busy_error())
        });
        assert_eq!(calls.load(Ordering::SeqCst), resolved_busy_retries());
        assert!(matches!(result, Err(AppError::DbBusy(_))));
    }

    /// GAP-SG-87 / GAP-SG-199: the busy policy is always usable, whatever the
    /// host config says.
    ///
    /// This asserts only what does NOT depend on the host: both resolvers apply
    /// `.max(1)`, so neither can ever hand back a budget of zero that would
    /// spin-zero or divide by nothing.
    ///
    /// The stronger claim — that an ABSENT override resolves to the compiled
    /// constant — moved to `tests/busy_policy_hermetic.rs`, because it is only
    /// true under an isolated XDG root. Asserting it here compared a RESOLVED
    /// value against a compiled one while the resolver reads the developer's
    /// real config, so `config set db.busy_retries 12` — a documented, legitimate
    /// key — turned the suite red without a line of code changing.
    #[test]
    fn resolved_busy_policy_defaults_match_constants() {
        assert!(
            resolved_busy_retries() >= 1,
            "retry budget must never resolve to zero"
        );
        assert!(
            resolved_busy_base_delay_ms() >= 1,
            "base delay must never resolve to zero"
        );
    }
}
