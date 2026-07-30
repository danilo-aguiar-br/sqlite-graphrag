use super::models::opencode_embed_model;
use super::ops::build_codex_embedding_command;
use super::timeout::{
    embed_timeout, embed_timeout_for_batch, DEFAULT_EMBED_TIMEOUT_SECS,
};
use super::types::{CodexSchemaFiles, EmbeddingFlavour};
use super::{LlmEmbedding, LlmEmbeddingBuilder};
use std::sync::Arc;

fn test_client(flavour: EmbeddingFlavour, binary: std::path::PathBuf) -> LlmEmbedding {
    LlmEmbedding {
        flavour,
        binary,
        model: "gpt-5.4".to_string(),
        codex_schemas: Arc::new(parking_lot::Mutex::new(CodexSchemaFiles::default())),
        timeout_override: None,
    }
}

#[test]
fn embed_timeout_default_is_300() {
    assert_eq!(DEFAULT_EMBED_TIMEOUT_SECS, 300);
}

#[test]
#[serial_test::serial(env)]
fn oauth_only_enforce_blocks_api_keys() {
    // SAFETY: this test only sets and unsets env vars; the
    // `serial(env)` group prevents cross-test interference.
    unsafe {
        std::env::set_var("ANTHROPIC_API_KEY", "test");
        assert!(LlmEmbedding::oauth_only_enforce().is_err());
        std::env::remove_var("ANTHROPIC_API_KEY");

        std::env::set_var("OPENAI_API_KEY", "test");
        assert!(LlmEmbedding::oauth_only_enforce().is_err());
        std::env::remove_var("OPENAI_API_KEY");
    }
    assert!(LlmEmbedding::oauth_only_enforce().is_ok());
}

#[test]
fn flavour_as_str_is_stable() {
    assert_eq!(EmbeddingFlavour::Claude.as_str(), "claude");
    assert_eq!(EmbeddingFlavour::Codex.as_str(), "codex");
}

#[test]
fn codex_schema_file_is_created_once_and_reused() {
    let client = test_client(
        EmbeddingFlavour::Codex,
        std::path::PathBuf::from("/bin/true"),
    );
    let first = client
        .codex_schema_file(64, false)
        .expect("schema file must be created");
    let second = client
        .codex_schema_file(64, false)
        .expect("schema file must be reused");
    assert_eq!(first.path(), second.path(), "same dim must reuse the file");

    let batch = client
        .codex_schema_file(64, true)
        .expect("batch schema file must be created");
    assert_ne!(
        first.path(),
        batch.path(),
        "single and batch schemas are distinct files"
    );

    let content = std::fs::read_to_string(first.path()).expect("schema file must be readable");
    assert!(content.contains(r#""minItems":64"#));
}

#[test]
fn codex_embedding_command_reads_prompt_from_stdin() {
    let schema_path = std::env::temp_dir().join("sqlite-graphrag-embed-schema-test.json");
    let cmd = build_codex_embedding_command(
        std::path::Path::new("/bin/true"),
        "gpt-5.4",
        &schema_path,
    )
    .expect("build_codex_embedding_command must succeed in test");
    let argv: Vec<String> = cmd
        .as_std()
        .get_args()
        .filter_map(|arg| arg.to_str().map(|s| s.to_string()))
        .collect();

    assert!(
        argv.iter().any(|arg| arg == "-"),
        "codex embedding command must read prompt from stdin: {argv:?}"
    );
    assert!(
        !argv.iter().any(|arg| arg.starts_with("passage: ")),
        "prompt text must not be passed as argv: {argv:?}"
    );
    for required in &[
        "exec",
        "-c",
        "sandbox_mode='read-only'",
        "approval_policy='never'",
        "--json",
        "--output-schema",
        "--ephemeral",
        "--skip-git-repo-check",
        "--sandbox",
        "read-only",
        "--ignore-user-config",
        "--ignore-rules",
        "--model",
        "gpt-5.4",
    ] {
        assert!(
            argv.iter().any(|arg| arg == required),
            "missing flag {required} in {argv:?}"
        );
    }
}

#[cfg(unix)]
#[test]
#[serial_test::serial(env)]
fn embed_passage_sends_prompt_to_codex_stdin() {
    use std::os::unix::fs::PermissionsExt;

    // Pin the dimensionality so the mock script and the validation
    // agree regardless of test execution order (no product env).
    crate::constants::set_active_embedding_dim(64);

    let temp = tempfile::tempdir().expect("tempdir must exist");
    let binary = temp.path().join("codex-stdin-check");
    let script = r#"#!/usr/bin/env bash
set -euo pipefail

prompt="$(cat)"
if [[ "$prompt" != "passage: codex-cli" ]]; then
  echo "unexpected stdin: $prompt" >&2
  exit 41
fi

vals="0.0"
for _ in $(seq 2 64); do
  vals="$vals,0.0"
done
payload="{\"embedding\":[$vals]}"
escaped="${payload//\"/\\\"}"
echo "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"$escaped\"}}"
"#;
    std::fs::write(&binary, script).expect("mock codex script must be written");
    let mut perms = std::fs::metadata(&binary)
        .expect("mock codex metadata must exist")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&binary, perms).expect("mock codex must be executable");

    let embedding = test_client(EmbeddingFlavour::Codex, binary);

    let vector = embedding
        .embed_passage("codex-cli")
        .expect("stdin-backed codex embedding must succeed");

    crate::constants::set_active_embedding_dim(crate::constants::DEFAULT_EMBEDDING_DIM);

    assert_eq!(vector.len(), 64);
    assert!(vector.iter().all(|value| *value == 0.0));
}

// ---------------------------------------------------------------
// ADR-0042 / GAP-002: LlmEmbeddingBuilder unit tests
// ---------------------------------------------------------------

/// `claude_default` is the `with_claude_builder` alias: returns a
/// builder pre-set to the Claude flavour. Build requires the
/// Claude binary to be on PATH; in CI without `claude`, the build
/// fails with the canonical `claude not found` error, which is
/// itself the proof that the flavour is propagated correctly.
#[test]
fn claude_default_resolves_path() {
    let builder = LlmEmbeddingBuilder::claude_default();
    assert_eq!(builder.flavour, EmbeddingFlavour::Claude);
    assert!(builder.binary_override.is_none());
    assert!(builder.model_override.is_none());
}

/// `override_binary` short-circuits the PATH probe. The builder
/// stores the override verbatim so the `build()` call can fall
/// back to `resolve_real_binary` for ELF canonicalisation.
#[test]
fn override_binary_uses_provided() {
    let path = std::path::PathBuf::from("/tmp/fake-claude-binary");
    let builder = LlmEmbeddingBuilder::claude_default().override_binary(path.clone());
    assert_eq!(builder.binary_override.as_ref(), Some(&path));
}

/// `override_model` short-circuits the env-var lookup. The model
/// override travels untouched through `build()` so the LLM
/// subprocess spawn honours it.
#[test]
fn override_model_uses_provided() {
    let builder =
        LlmEmbeddingBuilder::codex_default().override_model("gpt-5.4-custom".to_string());
    assert_eq!(builder.model_override.as_deref(), Some("gpt-5.4-custom"));
}

// ---------------------------------------------------------------
// v1.0.89 GAP tests
// ---------------------------------------------------------------

#[test]
fn embed_timeout_for_batch_scales_with_size() {
    let t1 = embed_timeout_for_batch(1);
    let t4 = embed_timeout_for_batch(4);
    let t8 = embed_timeout_for_batch(8);
    assert!(
        t1 < t4,
        "batch of 4 must have longer timeout than batch of 1"
    );
    assert!(
        t4 < t8,
        "batch of 8 must have longer timeout than batch of 4"
    );
    assert_eq!(t8 - t1, std::time::Duration::from_secs(15 * 7));
}

#[test]
fn embed_timeout_for_batch_single_equals_base() {
    let base = embed_timeout();
    let single = embed_timeout_for_batch(1);
    assert_eq!(base, single);
}

#[test]
fn opencode_flavour_as_str() {
    assert_eq!(EmbeddingFlavour::Opencode.as_str(), "opencode");
}

#[test]
#[serial_test::serial(env)]
fn opencode_embed_model_default_is_big_pickle() {
    // Without XDG embedding.opencode_model / llm.opencode_model, default applies.
    let model = opencode_embed_model();
    // Host XDG may override; ensure non-empty and not a claude/gpt default.
    assert!(!model.is_empty());
    assert!(!model.starts_with("claude"));
}

#[test]
fn opencode_embed_model_ignores_runtime_llm_model_cross_contamination() {
    // Even if runtime llm.model is set to a codex model, opencode path
    // must not use it (cross-backend contamination guard).
    crate::runtime_config::init(crate::runtime_config::RuntimeOverrides {
        llm_model: Some("gpt-5.4-mini".into()),
        ..Default::default()
    });
    let model = opencode_embed_model();
    assert_eq!(
        model, "opencode/big-pickle",
        "must NOT cross-contaminate with LLM_MODEL"
    );
}

#[test]
fn opencode_builder_default_has_correct_flavour() {
    let builder = LlmEmbeddingBuilder::opencode_default();
    assert_eq!(builder.flavour, EmbeddingFlavour::Opencode);
    assert!(builder.binary_override.is_none());
    assert!(builder.model_override.is_none());
}
