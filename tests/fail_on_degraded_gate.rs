//! ACHADO I — `--fail-on-degraded`: the three cases that define the flag.
//!
//! `vec_degraded` true means a hybrid search silently became a pure BM25 search.
//! It has always exited 0, so an agent reading only `.results` cannot tell the
//! difference between "ranked by vectors and BM25" and "the provider was down".
//! The flag turns that into an exit code.
//!
//! The delicate part is not the failure, it is the EXEMPTION: `--fallback-fts-only`
//! is an operator asking for exactly the same degradation on purpose. Without a
//! discriminator the two flags are mutually unusable, so the test that matters
//! most here is the one asserting a deliberate skip does NOT fail.

use sqlite_graphrag::query_embedding::{
    degradation_failure, FALLBACK_FTS_ONLY_CODE, FALLBACK_FTS_ONLY_REASON,
};

/// Named argument positions of [`degradation_failure`], spelled the way the
/// clap field is spelled.
///
/// The call sites below pass these POSITIONALLY, which once made this whole file
/// invisible to `rg fail_on_degraded tests/`: the prose writes the flag as
/// `--fail-on-degraded` and the calls carry no parameter name, so the snake_case
/// token appeared nowhere and the suite read as missing. Naming the arguments
/// here costs nothing and keeps the obvious search honest.
const FLAG_ON: bool = true;
const FLAG_OFF: bool = false;
const DEGRADED: bool = true;
const HEALTHY: bool = false;

/// Asserts the decision for an explicitly named `fail_on_degraded` value.
fn decide(
    fail_on_degraded: bool,
    vec_degraded: bool,
    reason_code: Option<&str>,
) -> Option<sqlite_graphrag::errors::AppError> {
    degradation_failure(fail_on_degraded, vec_degraded, reason_code)
}

/// Case 1 — a REAL degradation with the flag on must fail.
#[test]
fn real_degradation_with_flag_fails() {
    for code in [
        "timeout",
        "slot_exhausted",
        "oauth_quota",
        "cancelled",
        "dim_zero",
        "backend_mismatch",
        "embedding_failed",
    ] {
        let failure = decide(FLAG_ON, DEGRADED, Some(code));
        assert!(
            failure.is_some(),
            "reason_code {code} degraded the read and must fail under --fail-on-degraded"
        );
    }
}

/// Case 2 — a DELIBERATE degradation must never fail, flag or no flag.
///
/// This is the exemption the whole discriminator exists for.
#[test]
fn deliberate_fallback_fts_only_never_fails() {
    assert!(
        decide(FLAG_ON, DEGRADED, Some(FALLBACK_FTS_ONLY_CODE)).is_none(),
        "--fallback-fts-only is the operator asking for BM25; turning their own \
         instruction into a failure makes the two flags mutually unusable"
    );
    assert!(
        decide(FLAG_OFF, DEGRADED, Some(FALLBACK_FTS_ONLY_CODE)).is_none(),
        "without the flag nothing fails either"
    );
}

/// Case 3 — ANY degradation without the flag stays exit 0.
///
/// The default path must be unchanged: same exit, same envelope.
#[test]
fn degradation_without_the_flag_is_still_exit_zero() {
    for code in [
        "timeout",
        "dim_zero",
        "embedding_failed",
        FALLBACK_FTS_ONLY_CODE,
    ] {
        assert!(
            decide(FLAG_OFF, DEGRADED, Some(code)).is_none(),
            "the flag is opt-in; reason_code {code} must not change the exit \
             code when it is off"
        );
    }
}

/// A read that did NOT degrade never fails, even with the flag on.
#[test]
fn healthy_read_never_fails() {
    assert!(decide(FLAG_ON, HEALTHY, None).is_none());
    assert!(
        decide(FLAG_ON, HEALTHY, Some("timeout")).is_none(),
        "vec_degraded false is the authority; a stale reason_code must not \
         manufacture a failure"
    );
}

/// Provider-unreachable degradation is `transient` and retryable.
///
/// The lead's requirement, and the honest advice: the invocation was fine, the
/// provider was not, so retrying is the right next step.
#[test]
fn provider_unreachable_is_transient_and_retryable() {
    for code in ["timeout", "slot_exhausted", "oauth_quota", "cancelled"] {
        let failure = decide(FLAG_ON, DEGRADED, Some(code)).expect("this reason must fail");
        assert_eq!(
            failure.error_class(),
            "transient",
            "reason_code {code} means the provider was unreachable"
        );
        assert!(
            failure.is_retryable(),
            "reason_code {code} must advise a retry"
        );
    }
}

/// A wrong shape or wrong configuration is NOT sold as retryable.
///
/// Retrying an unchanged invocation reproduces it, so `transient` would be a lie
/// that costs the operator a wasted retry loop.
#[test]
fn configuration_failure_is_not_advertised_as_retryable() {
    for code in ["dim_zero", "backend_mismatch", "embedding_failed"] {
        let failure = decide(FLAG_ON, DEGRADED, Some(code)).expect("this reason must fail");
        assert!(
            !failure.is_retryable(),
            "reason_code {code} reproduces on retry; advertising it as \
             retryable buys the operator a loop that cannot converge"
        );
        assert_eq!(failure.exit_code(), 11, "embedding failures carry exit 11");
    }
}

/// An unknown reason still fails rather than passing silently.
///
/// Fail closed: a reason this code has never seen is the one most likely to be
/// a real regression.
#[test]
fn unknown_reason_fails_closed() {
    assert!(
        decide(FLAG_ON, DEGRADED, None).is_some(),
        "a degradation with no reason code must still fail under the flag"
    );
    assert!(decide(FLAG_ON, DEGRADED, Some("something_new")).is_some());
}

/// The two representations of the deliberate skip must stay in lock-step.
///
/// The envelope carries the prose; the discriminator branches on the code. If
/// one is renamed without the other, `--fallback-fts-only` starts failing under
/// `--fail-on-degraded` and the exemption is silently gone.
#[test]
fn deliberate_skip_prose_and_code_stay_paired() {
    assert!(
        FALLBACK_FTS_ONLY_REASON.starts_with(FALLBACK_FTS_ONLY_CODE),
        "the human-readable reason ({FALLBACK_FTS_ONLY_REASON}) must remain \
         derived from the machine code ({FALLBACK_FTS_ONLY_CODE})"
    );
}
