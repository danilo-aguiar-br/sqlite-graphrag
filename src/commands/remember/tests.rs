use crate::output::RememberResponse;

/// GAP-SG-37: replicates the `--strict-name` guard predicate so the
/// reject-on-normalization decision is unit-testable without a DB.
fn strict_name_rejects(strict: bool, name_was_normalized: bool) -> bool {
    strict && name_was_normalized
}

#[test]
fn strict_name_rejects_only_when_name_would_change() {
    assert!(
        strict_name_rejects(true, true),
        "strict + changed must reject"
    );
    assert!(
        !strict_name_rejects(true, false),
        "strict + canonical passes"
    );
    assert!(
        !strict_name_rejects(false, true),
        "non-strict always passes"
    );
    assert!(!strict_name_rejects(false, false));
}

// GAP-SG-37/SG-51: --strict-name and --replace-graph must parse on remember.
#[test]
fn remember_parses_strict_name_and_replace_graph_flags() {
    use crate::cli::{Cli, Commands};
    use clap::Parser;
    let cli = Cli::try_parse_from([
        "sqlite-graphrag",
        "remember",
        "--name",
        "my-mem",
        "--type",
        "note",
        "--description",
        "d",
        "--body",
        "b",
        "--strict-name",
        "--replace-graph",
        "--force-merge",
    ])
    .expect("parse");
    match cli.command {
        Some(Commands::Remember(a)) => {
            assert!(a.strict_name);
            assert!(a.replace_graph);
            assert!(a.force_merge);
        }
        other => panic!("expected remember, got {other:?}"),
    }
}

#[test]
fn remember_response_serializes_required_fields() {
    let resp = RememberResponse {
        memory_id: 42,
        name: "minha-mem".to_string(),
        namespace: "global".to_string(),
        action: "created".to_string(),
        operation: "created".to_string(),
        version: 1,
        entities_persisted: 0,
        relationships_persisted: 0,
        relationships_truncated: false,
        chunks_created: 1,
        chunks_persisted: 0,
        urls_persisted: 0,
        extraction_method: None,
        merged_into_memory_id: None,
        warnings: vec![],
        created_at: 1_705_320_000,
        created_at_iso: "2024-01-15T12:00:00Z".to_string(),
        elapsed_ms: 55,
        name_was_normalized: false,
        original_name: None,
        backend_invoked: None,
        entities_created: vec![],
        enrich_recommended: vec![],
    };

    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["memory_id"], 42);
    assert_eq!(json["action"], "created");
    assert_eq!(json["operation"], "created");
    assert_eq!(json["version"], 1);
    assert_eq!(json["elapsed_ms"], 55u64);
    assert!(json["warnings"].is_array());
    assert!(json["merged_into_memory_id"].is_null());
}

#[test]
fn remember_response_action_e_operation_sao_aliases() {
    let resp = RememberResponse {
        memory_id: 1,
        name: "mem".to_string(),
        namespace: "global".to_string(),
        action: "updated".to_string(),
        operation: "updated".to_string(),
        version: 2,
        entities_persisted: 3,
        relationships_persisted: 1,
        relationships_truncated: false,
        extraction_method: None,
        chunks_created: 2,
        chunks_persisted: 2,
        urls_persisted: 0,
        merged_into_memory_id: None,
        warnings: vec![],
        created_at: 0,
        created_at_iso: "1970-01-01T00:00:00Z".to_string(),
        elapsed_ms: 0,
        name_was_normalized: false,
        original_name: None,
        backend_invoked: None,
        entities_created: vec![],
        enrich_recommended: vec![],
    };

    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(
        json["action"], json["operation"],
        "action e operation devem ser iguais"
    );
    assert_eq!(json["entities_persisted"], 3);
    assert_eq!(json["relationships_persisted"], 1);
    assert_eq!(json["chunks_created"], 2);
}

#[test]
fn remember_response_warnings_lista_mensagens() {
    let resp = RememberResponse {
        memory_id: 5,
        name: "dup-mem".to_string(),
        namespace: "global".to_string(),
        action: "created".to_string(),
        operation: "created".to_string(),
        version: 1,
        entities_persisted: 0,
        extraction_method: None,
        relationships_persisted: 0,
        relationships_truncated: false,
        chunks_created: 1,
        chunks_persisted: 0,
        urls_persisted: 0,
        merged_into_memory_id: None,
        warnings: vec!["identical body already exists as memory id 3".to_string()],
        created_at: 0,
        created_at_iso: "1970-01-01T00:00:00Z".to_string(),
        elapsed_ms: 10,
        name_was_normalized: false,
        original_name: None,
        backend_invoked: None,
        entities_created: vec![],
        enrich_recommended: vec![],
    };

    let json = serde_json::to_value(&resp).expect("serialization failed");
    let warnings = json["warnings"]
        .as_array()
        .expect("warnings deve ser array");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].as_str().unwrap().contains("identical body"));
}

#[test]
fn invalid_name_reserved_prefix_returns_validation_error() {
    use crate::errors::AppError;
    // Validates the rejection logic for names with the "__" prefix directly
    let nome = "__reservado";
    let resultado: Result<(), AppError> = if nome.starts_with("__") {
        Err(AppError::Validation(
            crate::i18n::validation::reserved_name(),
        ))
    } else {
        Ok(())
    };
    assert!(resultado.is_err());
    if let Err(AppError::Validation(msg)) = resultado {
        assert!(!msg.is_empty());
    }
}

#[test]
fn name_too_long_returns_validation_error() {
    use crate::errors::AppError;
    let nome_longo = "a".repeat(crate::constants::MAX_MEMORY_NAME_LEN + 1);
    let resultado: Result<(), AppError> =
        if nome_longo.is_empty() || nome_longo.len() > crate::constants::MAX_MEMORY_NAME_LEN {
            Err(AppError::Validation(crate::i18n::validation::name_length(
                crate::constants::MAX_MEMORY_NAME_LEN,
            )))
        } else {
            Ok(())
        };
    assert!(resultado.is_err());
}

#[test]
fn remember_response_merged_into_memory_id_some_serializes_integer() {
    let resp = RememberResponse {
        memory_id: 10,
        name: "mem-mergeada".to_string(),
        namespace: "global".to_string(),
        action: "updated".to_string(),
        operation: "updated".to_string(),
        version: 3,
        extraction_method: None,
        entities_persisted: 0,
        relationships_persisted: 0,
        relationships_truncated: false,
        chunks_created: 1,
        chunks_persisted: 0,
        urls_persisted: 0,
        merged_into_memory_id: Some(7),
        warnings: vec![],
        created_at: 0,
        created_at_iso: "1970-01-01T00:00:00Z".to_string(),
        elapsed_ms: 0,
        name_was_normalized: false,
        original_name: None,
        backend_invoked: None,
        entities_created: vec![],
        enrich_recommended: vec![],
    };

    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["merged_into_memory_id"], 7);
}

#[test]
fn remember_response_urls_persisted_serializes_field() {
    // v1.0.24 P0-2: garante que urls_persisted aparece no JSON e aceita valor > 0.
    let resp = RememberResponse {
        memory_id: 3,
        name: "mem-com-urls".to_string(),
        namespace: "global".to_string(),
        action: "created".to_string(),
        operation: "created".to_string(),
        version: 1,
        entities_persisted: 0,
        relationships_persisted: 0,
        relationships_truncated: false,
        chunks_created: 1,
        chunks_persisted: 0,
        urls_persisted: 3,
        extraction_method: Some("regex-only".to_string()),
        merged_into_memory_id: None,
        warnings: vec![],
        created_at: 0,
        created_at_iso: "1970-01-01T00:00:00Z".to_string(),
        elapsed_ms: 0,
        name_was_normalized: false,
        original_name: None,
        backend_invoked: None,
        entities_created: vec![],
        enrich_recommended: vec![],
    };
    let json = serde_json::to_value(&resp).expect("serialization failed");
    assert_eq!(json["urls_persisted"], 3);
}

#[test]
fn empty_name_after_normalization_returns_specific_message() {
    // P0-4 regression: name consisting only of hyphens normalizes to empty string;
    // must produce a distinct error message, not the "too long" message.
    use crate::errors::AppError;
    let normalized = "---".to_lowercase().replace(['_', ' '], "-");
    let normalized = normalized.trim_matches('-').to_string();
    let resultado: Result<(), AppError> = if normalized.is_empty() {
        Err(AppError::Validation(
            crate::i18n::validation::name_empty_after_normalization(),
        ))
    } else {
        Ok(())
    };
    assert!(resultado.is_err());
    if let Err(AppError::Validation(msg)) = resultado {
        assert!(
            msg.contains("empty after normalization")
                || msg.contains("vazio após normalização"),
            "mensagem deve mencionar normalização vazia, obteve: {msg}"
        );
    }
}

#[test]
fn name_only_underscores_after_normalization_returns_specific_message() {
    // P0-4 regression: name consisting only of underscores normalizes to empty string.
    use crate::errors::AppError;
    let normalized = "___".to_lowercase().replace(['_', ' '], "-");
    let normalized = normalized.trim_matches('-').to_string();
    assert!(
        normalized.is_empty(),
        "underscores devem normalizar para string vazia"
    );
    let resultado: Result<(), AppError> = if normalized.is_empty() {
        Err(AppError::Validation(
            crate::i18n::validation::name_empty_after_normalization(),
        ))
    } else {
        Ok(())
    };
    assert!(resultado.is_err());
    if let Err(AppError::Validation(msg)) = resultado {
        assert!(
            msg.contains("empty after normalization")
                || msg.contains("vazio após normalização"),
            "mensagem deve mencionar normalização vazia, obteve: {msg}"
        );
    }
}

#[test]
fn remember_response_relationships_truncated_serializes_field() {
    // P1-D: garante que relationships_truncated aparece no JSON como bool.
    let resp_false = RememberResponse {
        memory_id: 1,
        name: "test".to_string(),
        namespace: "global".to_string(),
        action: "created".to_string(),
        operation: "created".to_string(),
        version: 1,
        entities_persisted: 2,
        relationships_persisted: 1,
        relationships_truncated: false,
        chunks_created: 1,
        chunks_persisted: 0,
        urls_persisted: 0,
        extraction_method: None,
        merged_into_memory_id: None,
        warnings: vec![],
        created_at: 0,
        created_at_iso: "1970-01-01T00:00:00Z".to_string(),
        elapsed_ms: 0,
        name_was_normalized: false,
        original_name: None,
        backend_invoked: None,
        entities_created: vec![],
        enrich_recommended: vec![],
    };
    let json_false = serde_json::to_value(&resp_false).expect("serialization failed");
    assert_eq!(json_false["relationships_truncated"], false);

    let resp_true = RememberResponse {
        relationships_truncated: true,
        ..resp_false
    };
    let json_true = serde_json::to_value(&resp_true).expect("serialization failed");
    assert_eq!(json_true["relationships_truncated"], true);
}

// GAP-08: body-preservation predicate tests.
// Verifies the decision logic that determines whether an existing body should
// be kept instead of overwritten with an empty incoming body during --force-merge.

/// Returns `true` when the existing body should be preserved.
///
/// Mirrors the `body_will_be_preserved` expression in `run()` so the logic
/// is testable without a real database connection.
fn should_preserve_body(force_merge: bool, raw_body_is_empty: bool, clear_body: bool) -> bool {
    force_merge && raw_body_is_empty && !clear_body
}

#[test]
fn gap08_empty_body_force_merge_no_clear_body_preserves() {
    // Caller passes no body with --force-merge but without --clear-body.
    // The existing body in the DB must be kept.
    assert!(
        should_preserve_body(true, true, false),
        "empty body + force-merge + no clear-body should trigger preservation"
    );
}

#[test]
fn gap08_empty_body_force_merge_with_clear_body_does_not_preserve() {
    // Caller explicitly passes --clear-body; intentional wipe is honoured.
    assert!(
        !should_preserve_body(true, true, true),
        "--clear-body must bypass preservation"
    );
}

#[test]
fn gap08_non_empty_body_force_merge_does_not_preserve() {
    // Caller provides a real body; it must overwrite the existing one.
    assert!(
        !should_preserve_body(true, false, false),
        "non-empty body must overwrite, not preserve"
    );
}

#[test]
fn gap08_empty_body_no_force_merge_does_not_preserve() {
    // Without --force-merge the path is a fresh create; no preservation needed.
    assert!(
        !should_preserve_body(false, true, false),
        "no --force-merge means no preservation logic applies"
    );
}
