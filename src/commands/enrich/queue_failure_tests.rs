//! Failure classification and dead-letter accounting (GAP-SG-146).
//!
//! Everything that decides whether a failed item retries or dies: the typed
//! `AppError` classifier and the two `record_item_failure` write paths.

use super::test_fixtures::{insert_pending, open_temp_queue};
use super::*;

#[test]
fn classify_database_busy_is_transient_non_busy_is_hard() {
    let busy = AppError::Database(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
        Some("database is locked".into()),
    ));
    assert_eq!(
        classify_enrich_outcome(&busy),
        crate::retry::AttemptOutcome::Transient
    );
    let constraint = AppError::Database(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
        Some("UNIQUE constraint failed".into()),
    ));
    assert_eq!(
        classify_enrich_outcome(&constraint),
        crate::retry::AttemptOutcome::HardFailure
    );
}

#[test]
fn classify_embedding_error_is_transient_floor() {
    assert_eq!(
        classify_enrich_outcome(&AppError::Embedding("dimension mismatch".into())),
        crate::retry::AttemptOutcome::Transient
    );
}

// GAP-SG-78: entity absence is Transient (own typed variant); memory
// absence and the untyped NotFound string stay HardFailure. No substring.
#[test]
fn classify_entity_not_yet_materialized_is_transient() {
    assert_eq!(
        classify_enrich_outcome(&AppError::EntityNotYetMaterialized {
            name: "acme".into(),
            namespace: "global".into(),
        }),
        crate::retry::AttemptOutcome::Transient
    );
}

#[test]
fn classify_memory_absence_stays_hard_failure() {
    assert_eq!(
        classify_enrich_outcome(&AppError::MemoryNotFound {
            name: "mem-x".into(),
            namespace: "global".into(),
        }),
        crate::retry::AttemptOutcome::HardFailure
    );
    assert_eq!(
        classify_enrich_outcome(&AppError::MemoryNotFoundById { id: 42 }),
        crate::retry::AttemptOutcome::HardFailure
    );
    assert_eq!(
        classify_enrich_outcome(&AppError::NotFound("gone".into())),
        crate::retry::AttemptOutcome::HardFailure
    );
}

#[test]
fn classify_provider_error_and_not_found_are_hard() {
    assert_eq!(
        classify_enrich_outcome(&AppError::ProviderError {
            code: "400".into(),
            message: "context length exceeded".into(),
        }),
        crate::retry::AttemptOutcome::HardFailure
    );
    assert_eq!(
        classify_enrich_outcome(&AppError::NotFound("memory 'gone' not found".into())),
        crate::retry::AttemptOutcome::HardFailure
    );
}

#[test]
fn classify_rate_limit_is_transient() {
    let e = AppError::RateLimited {
        detail: "429".into(),
    };
    assert_eq!(
        classify_enrich_outcome(&e),
        crate::retry::AttemptOutcome::Transient
    );
}

#[test]
fn classify_timeout_and_dbbusy_are_transient() {
    let t = AppError::Timeout {
        operation: "judge".into(),
        duration_secs: 30,
    };
    let b = AppError::DbBusy("locked".into());
    assert_eq!(
        classify_enrich_outcome(&t),
        crate::retry::AttemptOutcome::Transient
    );
    assert_eq!(
        classify_enrich_outcome(&b),
        crate::retry::AttemptOutcome::Transient
    );
}

#[test]
fn classify_validation_and_parse_are_hard_failure() {
    let v = AppError::Validation("failed to parse entities array: bad".into());
    assert_eq!(
        classify_enrich_outcome(&v),
        crate::retry::AttemptOutcome::HardFailure
    );
}

#[test]
fn classify_validation_never_infers_transience_from_message() {
    // GAP-SG-73: the fallback classifier is TYPED-only now. Messages
    // that used to be sniffed for "json" / "missing '" substrings and
    // treated as Transient are HardFailure here — the OpenRouter chat
    // path (the project's only supported enrich mode) attaches its own
    // typed `ChatError::retry_class` for these exact shape failures
    // BEFORE `record_item_failure_typed` ever falls back to this
    // classifier, so no message-based guessing survives in the fallback.
    for msg in [
        "model 'x' returned non-object JSON after repair (got string)",
        "model 'x' returned content that could not be parsed even after JSON repair",
        "model 'x' returned no structured content",
        "LLM result missing 'description' field",
        "LLM result missing 'enriched_body' field",
    ] {
        assert_eq!(
            classify_enrich_outcome(&AppError::Validation(msg.into())),
            crate::retry::AttemptOutcome::HardFailure,
            "expected hard failure for: {msg}"
        );
    }
}

#[test]
fn record_item_failure_hard_marks_dead() {
    let (conn, path) = open_temp_queue();
    let id = insert_pending(&conn, "mem-hard");
    let outcome = record_item_failure(
        &conn,
        id,
        1,
        5,
        &AppError::Validation("invalid body".into()),
    );
    assert_eq!(outcome, crate::retry::AttemptOutcome::HardFailure);
    let status: String = conn
        .query_row(
            "SELECT status FROM queue WHERE id=?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "dead");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn record_item_failure_transient_at_cap_marks_dead() {
    let (conn, path) = open_temp_queue();
    let id = insert_pending(&conn, "mem-cap");
    let outcome = record_item_failure(
        &conn,
        id,
        5,
        5,
        &AppError::RateLimited {
            detail: "429".into(),
        },
    );
    assert_eq!(outcome, crate::retry::AttemptOutcome::Transient);
    let status: String = conn
        .query_row(
            "SELECT status FROM queue WHERE id=?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "dead");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn record_item_failure_transient_reschedules_pending() {
    let (conn, path) = open_temp_queue();
    let id = insert_pending(&conn, "mem-transient");
    let outcome = record_item_failure(
        &conn,
        id,
        1,
        5,
        &AppError::RateLimited {
            detail: "429".into(),
        },
    );
    assert_eq!(outcome, crate::retry::AttemptOutcome::Transient);
    let (status, future): (String, i64) = conn
        .query_row(
            "SELECT status, (next_retry_at > datetime('now')) FROM queue WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(future, 1, "next_retry_at must be in the future");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn record_item_failure_typed_persists_diagnostics_on_dead_letter() {
    let (conn, path) = open_temp_queue();
    let id = insert_pending(&conn, "mem-diag");
    let outcome = record_item_failure_typed(
        &conn,
        id,
        1,
        5,
        crate::retry::AttemptOutcome::HardFailure,
        "truncated response",
        Some("length"),
        Some(120),
        Some(4096),
    );
    assert_eq!(outcome, crate::retry::AttemptOutcome::HardFailure);
    let (status, finish_reason, input_tokens, output_tokens): (
        String,
        Option<String>,
        Option<i64>,
        Option<i64>,
    ) = conn
        .query_row(
            "SELECT status, finish_reason, input_tokens, output_tokens FROM queue WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(status, "dead");
    assert_eq!(finish_reason.as_deref(), Some("length"));
    assert_eq!(input_tokens, Some(120));
    assert_eq!(output_tokens, Some(4096));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn record_item_failure_typed_reschedules_transient_below_max_attempts() {
    // GAP-SG-72-chat: a transient failure (e.g. a truncated OpenRouter
    // response) below max_attempts must stay `pending` with a
    // future `next_retry_at`, not go straight to `dead` — and it must
    // still persist the finish_reason/token diagnostics for later
    // inspection via `--list-dead` / `--status`.
    let (conn, path) = open_temp_queue();
    let id = insert_pending(&conn, "mem-retry");
    let outcome = record_item_failure_typed(
        &conn,
        id,
        1,
        5,
        crate::retry::AttemptOutcome::Transient,
        "truncated response",
        Some("length"),
        Some(120),
        Some(64),
    );
    assert_eq!(outcome, crate::retry::AttemptOutcome::Transient);
    let (status, error_class, finish_reason, next_retry_at): (
        String,
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT status, error_class, finish_reason, next_retry_at FROM queue WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(error_class, "transient");
    assert_eq!(finish_reason.as_deref(), Some("length"));
    assert!(
        next_retry_at.is_some(),
        "a rescheduled item must carry a next_retry_at"
    );
    let _ = std::fs::remove_file(&path);
}
