use super::*;
use std::io;

#[test]
fn exit_code_validation_returns_1() {
    assert_eq!(AppError::Validation("invalid field".into()).exit_code(), 1);
}

// GAP-SG-39: actionable errors carry a remediation suggestion.
#[test]
fn suggestion_present_for_actionable_variants() {
    assert!(AppError::Validation("bad name".into())
        .suggestion()
        .is_some());
    let dup = AppError::Duplicate("global/x".into());
    assert!(dup.suggestion().unwrap().contains("--force-merge"));
    let nf = AppError::MemoryNotFound {
        name: "x".into(),
        namespace: "global".into(),
    };
    assert!(nf.suggestion().is_some());
}

#[test]
fn exit_code_duplicate_returns_9() {
    assert_eq!(AppError::Duplicate("namespace/name".into()).exit_code(), 9);
}

#[test]
fn exit_code_conflict_returns_3() {
    assert_eq!(
        AppError::Conflict("updated_at changed".into()).exit_code(),
        3
    );
}

#[test]
fn exit_code_not_found_returns_4() {
    assert_eq!(AppError::NotFound("memory missing".into()).exit_code(), 4);
}

#[test]
fn exit_code_namespace_error_returns_5() {
    assert_eq!(
        AppError::NamespaceError("not resolved".into()).exit_code(),
        5
    );
}

#[test]
fn exit_code_limit_exceeded_returns_6() {
    assert_eq!(
        AppError::LimitExceeded("body too large".into()).exit_code(),
        6
    );
}

// v1.1.1 (P11): both typed ceiling variants keep the exit 6 contract.
#[test]
fn exit_code_body_too_large_and_too_many_chunks_return_6() {
    assert_eq!(
        AppError::BodyTooLarge {
            bytes: 600_000,
            limit: 512_000
        }
        .exit_code(),
        6
    );
    assert_eq!(
        AppError::TooManyChunks {
            chunks: 700,
            limit: 512
        }
        .exit_code(),
        6
    );
    assert_eq!(
        AppError::TooManyTokens {
            tokens: 35_000,
            limit: 30_000
        }
        .exit_code(),
        6
    );
}

// v1.1.2 (Gap 2): the token-cap message carries the estimated count and
// the limit, naming the constant — the third ceiling is distinguishable
// from bytes and chunks on exit 6 without substring classification.
#[test]
fn too_many_tokens_message_identifies_token_cap() {
    let err = AppError::TooManyTokens {
        tokens: 35_000,
        limit: 30_000,
    };
    let msg = err.to_string();
    assert!(msg.contains("limit exceeded"), "obtido: {msg}");
    assert!(msg.contains("35000 tokens"), "obtido: {msg}");
    assert!(msg.contains("30000-token cap"), "obtido: {msg}");
    assert!(
        msg.contains("EMBEDDING_REQUEST_MAX_TOKENS"),
        "obtido: {msg}"
    );
    assert!(!msg.contains("byte cap"), "obtido: {msg}");
    assert!(!msg.contains("chunk"), "obtido: {msg}");
}

// v1.1.1 (P11): the message identifies WHICH cap fired, with the measured
// value and the limit — bytes vs chunks are distinguishable without
// substring classification of a generic string.
#[test]
fn body_too_large_message_identifies_bytes_cap() {
    let err = AppError::BodyTooLarge {
        bytes: 600_000,
        limit: 512_000,
    };
    let msg = err.to_string();
    assert!(msg.contains("limit exceeded"), "obtido: {msg}");
    assert!(msg.contains("600000 bytes"), "obtido: {msg}");
    assert!(msg.contains("512000-byte cap"), "obtido: {msg}");
    assert!(msg.contains("MAX_MEMORY_BODY_LEN"), "obtido: {msg}");
    assert!(!msg.contains("chunk"), "obtido: {msg}");
}

#[test]
fn too_many_chunks_message_identifies_chunk_cap() {
    let err = AppError::TooManyChunks {
        chunks: 700,
        limit: 512,
    };
    let msg = err.to_string();
    assert!(msg.contains("limit exceeded"), "obtido: {msg}");
    assert!(msg.contains("700 chunks"), "obtido: {msg}");
    assert!(msg.contains("512-chunk cap"), "obtido: {msg}");
    assert!(
        msg.contains("REMEMBER_MAX_SAFE_MULTI_CHUNKS"),
        "obtido: {msg}"
    );
    assert!(!msg.contains("byte cap"), "obtido: {msg}");
}

#[test]
fn typed_limit_variants_are_permanent_with_suggestion() {
    let body = AppError::BodyTooLarge { bytes: 1, limit: 1 };
    let chunks = AppError::TooManyChunks {
        chunks: 1,
        limit: 1,
    };
    assert!(body.is_permanent());
    assert!(chunks.is_permanent());
    assert!(!body.is_retryable());
    assert!(!chunks.is_retryable());
    assert!(body.suggestion().unwrap().contains("MAX_MEMORY_BODY_LEN"));
    assert!(chunks
        .suggestion()
        .unwrap()
        .contains("REMEMBER_MAX_SAFE_MULTI_CHUNKS"));
    let tokens = AppError::TooManyTokens {
        tokens: 1,
        limit: 1,
    };
    assert!(tokens.is_permanent());
    assert!(!tokens.is_retryable());
    assert!(tokens
        .suggestion()
        .unwrap()
        .contains("EMBEDDING_REQUEST_MAX_TOKENS"));
}

#[test]
fn typed_limit_variants_localize_to_pt() {
    let body = AppError::BodyTooLarge {
        bytes: 600_000,
        limit: 512_000,
    };
    let pt = body.localized_message_for(crate::i18n::Language::Portuguese);
    assert!(pt.contains("limite excedido"), "obtido: {pt}");
    assert!(pt.contains("600000"), "obtido: {pt}");
    let chunks = AppError::TooManyChunks {
        chunks: 700,
        limit: 512,
    };
    let pt = chunks.localized_message_for(crate::i18n::Language::Portuguese);
    assert!(pt.contains("limite excedido"), "obtido: {pt}");
    assert!(pt.contains("700"), "obtido: {pt}");
    let tokens = AppError::TooManyTokens {
        tokens: 35_000,
        limit: 30_000,
    };
    let pt = tokens.localized_message_for(crate::i18n::Language::Portuguese);
    assert!(pt.contains("limite excedido"), "obtido: {pt}");
    assert!(pt.contains("35000"), "obtido: {pt}");
    assert!(pt.contains("EMBEDDING_REQUEST_MAX_TOKENS"), "obtido: {pt}");
}

#[test]
fn exit_code_embedding_returns_11() {
    assert_eq!(AppError::Embedding("model failure".into()).exit_code(), 11);
}

#[test]
fn exit_code_vec_extension_returns_12() {
    assert_eq!(
        AppError::VecExtension("extension did not load".into()).exit_code(),
        12
    );
}

#[test]
fn exit_code_db_busy_returns_15() {
    assert_eq!(AppError::DbBusy("retries exhausted".into()).exit_code(), 15);
}

#[test]
fn exit_code_batch_partial_failure_returns_13() {
    assert_eq!(
        AppError::BatchPartialFailure {
            total: 10,
            failed: 3
        }
        .exit_code(),
        13
    );
}

#[test]
fn display_batch_partial_failure_includes_counts() {
    let err = AppError::BatchPartialFailure {
        total: 50,
        failed: 7,
    };
    let msg = err.to_string();
    assert!(msg.contains("7"));
    assert!(msg.contains("50"));
    // to_string() uses the English #[error] attr; PT is in localized_message_for
    assert!(msg.contains("batch partial failure"));
}

#[test]
fn exit_code_io_returns_14() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
    assert_eq!(AppError::Io(io_err).exit_code(), 14);
}

#[test]
fn exit_code_internal_returns_20() {
    let anyhow_err = anyhow::anyhow!("unexpected internal error");
    assert_eq!(AppError::Internal(anyhow_err).exit_code(), 20);
}

#[test]
fn exit_code_json_returns_20() {
    let json_err = serde_json::from_str::<serde_json::Value>("invalid json {{").unwrap_err();
    assert_eq!(AppError::Json(json_err).exit_code(), 20);
}

#[test]
fn exit_code_lock_busy_returns_75() {
    assert_eq!(
        AppError::LockBusy("another active instance".into()).exit_code(),
        75
    );
}

#[test]
fn display_validation_includes_message() {
    let err = AppError::Validation("invalid id".into());
    assert!(err.to_string().contains("invalid id"));
    assert!(err.to_string().contains("validation error"));
}

#[test]
fn display_duplicate_includes_message() {
    let err = AppError::Duplicate("proj/mem".into());
    assert!(err.to_string().contains("proj/mem"));
    assert!(err.to_string().contains("duplicate detected"));
}

#[test]
fn display_not_found_includes_message() {
    let err = AppError::NotFound("id 42".into());
    assert!(err.to_string().contains("id 42"));
    assert!(err.to_string().contains("not found"));
}

#[test]
fn display_embedding_includes_message() {
    let err = AppError::Embedding("wrong dimension".into());
    assert!(err.to_string().contains("wrong dimension"));
    assert!(err.to_string().contains("embedding error"));
}

#[test]
fn display_lock_busy_includes_message() {
    let err = AppError::LockBusy("pid 1234".into());
    assert!(err.to_string().contains("pid 1234"));
    assert!(err.to_string().contains("lock busy"));
}

#[test]
fn from_io_error_converts_correctly() {
    let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "permission denied");
    let app_err: AppError = io_err.into();
    assert_eq!(app_err.exit_code(), 14);
    assert!(app_err.to_string().contains("IO error"));
}

#[test]
fn from_anyhow_error_converts_correctly() {
    let anyhow_err = anyhow::anyhow!("internal detail");
    let app_err: AppError = anyhow_err.into();
    assert_eq!(app_err.exit_code(), 20);
    assert!(app_err.to_string().contains("internal detail"));
}

#[test]
fn from_serde_json_error_converts_correctly() {
    let json_err = serde_json::from_str::<serde_json::Value>("{bad_field}").unwrap_err();
    let app_err: AppError = json_err.into();
    assert_eq!(app_err.exit_code(), 20);
    assert!(app_err.to_string().contains("json error"));
}

#[test]
fn exit_code_lock_busy_matches_constant() {
    assert_eq!(
        AppError::LockBusy("test".into()).exit_code(),
        crate::constants::CLI_LOCK_EXIT_CODE
    );
}

#[test]
fn localized_message_en_equals_to_string() {
    let err = AppError::NotFound("mem-x".into());
    assert_eq!(
        err.localized_message_for(crate::i18n::Language::English),
        err.to_string()
    );
}

// Detailed Portuguese-specific assertions live in `src/i18n.rs`
// (the bilingual module). Here we only verify that delegation is wired
// correctly, without embedding PT strings in this English-only file.

#[test]
fn localized_message_pt_differs_from_en() {
    let err = AppError::NotFound("mem-x".into());
    let en = err.localized_message_for(crate::i18n::Language::English);
    let pt = err.localized_message_for(crate::i18n::Language::Portuguese);
    assert_ne!(en, pt, "PT and EN must produce distinct messages");
    assert!(pt.contains("mem-x"), "PT must include the variant payload");
}

#[test]
fn localized_message_pt_delegates_to_app_error_pt_helper() {
    use crate::i18n::validation::app_error_pt as pt;

    let cases: Vec<(AppError, String)> = vec![
        (AppError::Validation("x".into()), pt::validation("x")),
        (AppError::Duplicate("x".into()), pt::duplicate("x")),
        (AppError::Conflict("x".into()), pt::conflict("x")),
        (AppError::NotFound("x".into()), pt::not_found("x")),
        (
            AppError::NamespaceError("x".into()),
            pt::namespace_error("x"),
        ),
        (AppError::LimitExceeded("x".into()), pt::limit_exceeded("x")),
        (AppError::Embedding("x".into()), pt::embedding("x")),
        (AppError::VecExtension("x".into()), pt::vec_extension("x")),
        (AppError::DbBusy("x".into()), pt::db_busy("x")),
        (
            AppError::BatchPartialFailure {
                total: 10,
                failed: 3,
            },
            pt::batch_partial_failure(10, 3),
        ),
        (AppError::LockBusy("x".into()), pt::lock_busy("x")),
        (
            AppError::AllSlotsFull {
                max: 4,
                waited_secs: 60,
            },
            pt::all_slots_full(4, 60),
        ),
        (
            AppError::LowMemory {
                available_mb: 100,
                required_mb: 500,
            },
            pt::low_memory(100, 500),
        ),
        (
            AppError::BinaryNotFound {
                name: "claude".into(),
            },
            pt::binary_not_found("claude"),
        ),
        (
            AppError::RateLimited {
                detail: "429".into(),
            },
            pt::rate_limited("429"),
        ),
        (
            AppError::Timeout {
                operation: "op".into(),
                duration_secs: 30,
            },
            pt::timeout("op", 30),
        ),
    ];

    for (err, expected) in cases {
        let actual = err.localized_message_for(crate::i18n::Language::Portuguese);
        assert_eq!(actual, expected, "delegation mismatch");
    }
}

#[test]
fn is_retryable_transient_errors() {
    assert!(AppError::DbBusy("x".into()).is_retryable());
    assert!(AppError::LockBusy("x".into()).is_retryable());
    assert!(AppError::AllSlotsFull {
        max: 4,
        waited_secs: 60
    }
    .is_retryable());
    assert!(AppError::LowMemory {
        available_mb: 100,
        required_mb: 500
    }
    .is_retryable());
    assert!(AppError::RateLimited {
        detail: "429".into()
    }
    .is_retryable());
    assert!(AppError::Timeout {
        operation: "op".into(),
        duration_secs: 30
    }
    .is_retryable());
}

#[test]
fn is_retryable_permanent_errors() {
    assert!(!AppError::Validation("x".into()).is_retryable());
    assert!(!AppError::NotFound("x".into()).is_retryable());
    assert!(!AppError::Duplicate("x".into()).is_retryable());
    assert!(!AppError::Conflict("x".into()).is_retryable());
    assert!(!AppError::BinaryNotFound { name: "x".into() }.is_retryable());
}

#[test]
fn exit_code_new_variants() {
    assert_eq!(AppError::BinaryNotFound { name: "x".into() }.exit_code(), 1);
    assert_eq!(AppError::RateLimited { detail: "x".into() }.exit_code(), 1);
    assert_eq!(
        AppError::Timeout {
            operation: "x".into(),
            duration_secs: 5
        }
        .exit_code(),
        1
    );
}

// GAP-SG-78: EntityNotYetMaterialized is a transitory absence (the entity is
// materialized on a later enrich pass), NOT a terminal not-found.
#[test]
fn entity_not_yet_materialized_exit_code_is_4() {
    let e = AppError::EntityNotYetMaterialized {
        name: "acme".into(),
        namespace: "global".into(),
    };
    assert_eq!(e.exit_code(), 4);
}

#[test]
fn entity_not_yet_materialized_is_retryable_not_permanent() {
    let e = AppError::EntityNotYetMaterialized {
        name: "acme".into(),
        namespace: "global".into(),
    };
    assert!(e.is_retryable());
    assert!(!e.is_permanent());
}

#[test]
fn entity_not_yet_materialized_user_message_non_empty() {
    let e = AppError::EntityNotYetMaterialized {
        name: "acme".into(),
        namespace: "global".into(),
    };
    assert!(!e
        .localized_message_for(crate::i18n::Language::English)
        .is_empty());
    assert!(!e
        .localized_message_for(crate::i18n::Language::Portuguese)
        .is_empty());
}

#[test]
fn app_error_size_does_not_exceed_budget() {
    let size = std::mem::size_of::<AppError>();
    assert!(
        size <= 128,
        "AppError is {size} bytes — exceeds 128-byte budget; \
         consider boxing large variants to reduce memcpy cost in Result propagation"
    );
}
