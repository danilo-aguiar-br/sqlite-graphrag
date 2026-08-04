//! Offline OpenRouter stub for the integration suite.
//!
//! # Why this exists
//!
//! Until the headless backends were removed, the ONLY offline embedding path
//! in this suite was `tests/mock-llm/{claude,codex}` — two shell scripts the
//! binary found on `PATH` and spawned. Nothing spawns a local CLI any more, so
//! every write path went straight to `https://openrouter.ai` and failed with
//! exit 11 (`probe openrouter: cliente não inicializado`).
//!
//! This module restores the offline path at the layer that actually carries
//! traffic today: HTTP. A [`wiremock::MockServer`] answers the two OpenRouter
//! endpoints the binary talks to, and the XDG keys `network.openrouter.*`
//! redirect the child process at it.
//!
//! # Why a background runtime
//!
//! `MockServer` is async but the integration tests are synchronous
//! (`assert_cmd`), and the server must keep answering while a SEPARATE process
//! issues requests. A `current_thread` runtime would only make progress inside
//! `block_on`, so the server would hang the moment the test returned to
//! synchronous code. The guard therefore owns a MULTI-THREAD runtime whose
//! worker threads keep polling the listener for the whole lifetime of the
//! guard.
//!
//! # Why the vector is not random
//!
//! The retired shell mock returned ZERO vectors, so cosine similarity was 0
//! for every pair and any ordering assertion was meaningless. Here the vector
//! is a deterministic bag-of-tokens projection: the same text always yields
//! the same vector, and two texts that share tokens land closer together than
//! two that share none. Similarity tests measure similarity again instead of
//! comparing noise.

#![allow(dead_code)]

use std::fs;
use std::path::Path;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// Dimensionality every stub vector carries. Must track
/// `crate::constants::DEFAULT_EMBEDDING_DIM`, which is 1024 since v1.2.0.
pub const STUB_DIM: usize = 1024;

/// API key planted in the sandbox config. Never a real credential.
pub const STUB_API_KEY: &str = "sk-or-v1-test-offline-stub";

/// Embedding model named on every sandbox invocation.
///
/// `main.rs` only constructs the OpenRouter client when `--embedding-model` is
/// present, so the sandbox must always name one. `qwen3-embedding-*` is the
/// project's canonical model AND the branch that omits the MRL `dimensions`
/// field, which keeps the stub answering at its natural width.
pub const STUB_MODEL: &str = "qwen/qwen3-embedding-8b";

/// Deterministic unit vector derived from `text`.
///
/// Lowercased alphanumeric tokens are hashed into `STUB_DIM` buckets and
/// accumulated, then the vector is L2-normalised. Properties the suite relies
/// on:
///
/// - the same text always produces the same vector (reruns are stable);
/// - texts sharing tokens produce vectors with positive cosine;
/// - the result is never the zero vector, so `validate_dim`-style guards and
///   `DimZero` fallbacks are not tripped by the stub itself.
pub fn deterministic_vector(text: &str) -> Vec<f32> {
    deterministic_vector_dim(text, STUB_DIM)
}

/// [`deterministic_vector`] at an explicit width, for the MRL `dimensions`
/// field. Models that truncate server-side (`qwen3-embedding-*`) omit that
/// field, in which case the caller passes [`STUB_DIM`].
pub fn deterministic_vector_dim(text: &str, dim: usize) -> Vec<f32> {
    let dim = dim.max(1);
    let mut v = vec![0.0f32; dim];

    for token in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
    {
        let lower = token.to_ascii_lowercase();
        // FNV-1a: tiny, dependency-free, and stable across platforms and runs.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in lower.as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // Two buckets per token so distinct tokens rarely collide entirely.
        let a = (h % dim as u64) as usize;
        let b = ((h >> 32) % dim as u64) as usize;
        v[a] += 1.0;
        v[b] += 0.5;
    }

    // A body with no alphanumeric token would otherwise be the zero vector.
    if v.iter().all(|x| *x == 0.0) {
        v[0] = 1.0;
    }

    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

/// Answers `POST /api/v1/embeddings` for both request shapes.
///
/// `input` is `String` for a single text and `[String]` for a batch (the
/// `EmbeddingInput` untagged enum on the request side). The reply echoes one
/// `data` entry per input, carrying the matching `index`, because the client
/// reassembles a batch by that field.
struct EmbeddingsResponder;

impl Respond for EmbeddingsResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = match serde_json::from_slice(&request.body) {
            Ok(v) => v,
            Err(e) => {
                return ResponseTemplate::new(400).set_body_json(serde_json::json!({
                    "error": { "code": "invalid_request", "message": format!("stub: {e}") }
                }))
            }
        };

        let inputs: Vec<String> = match body.get("input") {
            Some(serde_json::Value::String(s)) => vec![s.clone()],
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .map(|i| i.as_str().unwrap_or_default().to_string())
                .collect(),
            _ => {
                return ResponseTemplate::new(400).set_body_json(serde_json::json!({
                    "error": { "code": "invalid_request", "message": "stub: missing input" }
                }))
            }
        };

        // MRL: models that truncate client-side send an explicit width;
        // `qwen3-embedding-*` omits the field and expects the server default.
        let dim = body
            .get("dimensions")
            .and_then(serde_json::Value::as_u64)
            .map(|d| d as usize)
            .unwrap_or(STUB_DIM)
            .max(1);

        let data: Vec<serde_json::Value> = inputs
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let vec = deterministic_vector_dim(text, dim);
                serde_json::json!({ "embedding": vec, "index": i })
            })
            .collect();

        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list",
            "data": data,
            "model": body.get("model").cloned().unwrap_or(serde_json::Value::Null),
        }))
    }
}

/// Answers `POST /api/v1/chat/completions` with a schema-shaped empty result.
///
/// The enrich JUDGE asks for Structured Outputs, so the content must be a JSON
/// STRING that parses into the requested schema. Returning empty arrays keeps
/// the extraction well-formed while asserting nothing about graph content:
/// tests that need a specific verdict should mount their own `Mock` with a
/// higher priority rather than teach this default to lie.
struct ChatResponder {
    content: String,
}

impl Respond for ChatResponder {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-stub",
            "choices": [{
                "message": { "content": self.content },
                "finish_reason": "stop"
            }],
            "usage": { "cost": 0.0, "prompt_tokens": 1, "completion_tokens": 1 }
        }))
    }
}

/// Live OpenRouter stub. Keep the binding alive for as long as any child
/// process may issue a request; dropping it shuts the listener down.
#[must_use = "dropping the guard stops the stub server mid-test"]
pub struct OpenRouterStub {
    embeddings_url: String,
    chat_url: String,
    // Field order matters: the server must be dropped BEFORE the runtime that
    // owns its tasks, and Rust drops fields in declaration order.
    _server: MockServer,
    _rt: tokio::runtime::Runtime,
}

impl OpenRouterStub {
    /// Boots the stub on an ephemeral port.
    ///
    /// The default chat reply is an empty extraction (`entities` and
    /// `relationships` both empty).
    pub fn start() -> Self {
        Self::start_with_chat_content(r#"{"entities":[],"relationships":[]}"#)
    }

    /// Boots the stub with an explicit chat payload, for suites that need the
    /// JUDGE to return specific structured content.
    pub fn start_with_chat_content(chat_content: &str) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            // Four workers, not two: under a full `cargo test` the machine runs
            // many test binaries at once and a starved reactor shows up as an
            // intermittent embedding timeout in unrelated suites.
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("openrouter stub: multi-thread runtime must build");

        let content = chat_content.to_string();
        let server = rt.block_on(async move {
            let server = MockServer::start().await;

            Mock::given(method("POST"))
                .and(path("/api/v1/embeddings"))
                .respond_with(EmbeddingsResponder)
                .mount(&server)
                .await;

            Mock::given(method("POST"))
                .and(path("/api/v1/chat/completions"))
                .respond_with(ChatResponder { content })
                .mount(&server)
                .await;

            server
        });

        let uri = server.uri();
        OpenRouterStub {
            embeddings_url: format!("{uri}/api/v1/embeddings"),
            chat_url: format!("{uri}/api/v1/chat/completions"),
            _server: server,
            _rt: rt,
        }
    }

    /// Endpoint the child must POST embeddings to.
    pub fn embeddings_url(&self) -> &str {
        &self.embeddings_url
    }

    /// Endpoint the child must POST chat completions to.
    pub fn chat_url(&self) -> &str {
        &self.chat_url
    }
}

/// Process-wide stub, booted on first use.
///
/// One listener per TEST BINARY, not per test: `cargo test` already gives each
/// integration file its own process, and a per-test server would mean hundreds
/// of sockets and runtimes for no isolation gain — the stub is stateless.
/// A `OnceLock` also lets the free functions below reach it without threading a
/// handle through every call site.
pub fn global_stub() -> &'static OpenRouterStub {
    static STUB: std::sync::OnceLock<OpenRouterStub> = std::sync::OnceLock::new();
    STUB.get_or_init(OpenRouterStub::start)
}

/// Writes the sandbox `config.toml`: optional `db.path`, the stub endpoints,
/// and a test API key.
///
/// The child is redirected through the SAME XDG channel production uses — no
/// product environment variable is invented here, because none is read on the
/// hot path since G-T-XDG-04.
pub fn write_sandbox_config(config_dir: &Path, db: Option<&Path>) {
    write_sandbox_config_inner(config_dir, db, true);
}

/// Same sandbox, minus the API key.
///
/// Exists for the cases whose SUBJECT is the missing-key failure: planting a
/// key there would make the command succeed and the assertion vacuous.
pub fn write_sandbox_config_without_key(config_dir: &Path, db: Option<&Path>) {
    write_sandbox_config_inner(config_dir, db, false);
}

fn write_sandbox_config_inner(config_dir: &Path, db: Option<&Path>, with_key: bool) {
    fs::create_dir_all(config_dir).expect("write_sandbox_config: mkdir config");
    let stub = global_stub();

    // MERGE, never clobber: helpers here run on every command construction, so
    // a plain rewrite would erase whatever the test itself planted with
    // `config set` on the previous step.
    let file = config_dir.join("config.toml");
    let mut settings: Vec<String> = Vec::new();
    let mut had_db = false;
    if let Ok(existing) = fs::read_to_string(&file) {
        for line in existing.lines() {
            let t = line.trim();
            if !(t.starts_with('"') && t.contains('=')) {
                continue;
            }
            if t.starts_with("\"network.openrouter.") {
                continue; // ours; re-emitted below with the live port
            }
            if t.starts_with("\"db.path\"") {
                had_db = true;
                if db.is_some() {
                    continue; // caller supplied a fresher one
                }
            }
            settings.push(t.to_string());
        }
    }
    let _ = had_db;
    if let Some(db) = db {
        let db_str = db
            .display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        settings.push(format!("\"db.path\" = \"{db_str}\""));
    }
    settings.push(format!(
        "\"network.openrouter.embeddings_url\" = \"{}\"",
        stub.embeddings_url()
    ));
    settings.push(format!(
        "\"network.openrouter.chat_url\" = \"{}\"",
        stub.chat_url()
    ));

    let keys = if with_key {
        let fingerprint = blake3::hash(STUB_API_KEY.as_bytes()).to_hex().to_string();
        format!(
            "\n[[keys]]\nprovider = \"openrouter\"\nvalue = \"{STUB_API_KEY}\"\nadded_at = \"2026-01-01T00:00:00Z\"\nfingerprint = \"{fingerprint}\"\n"
        )
    } else {
        String::new()
    };
    let cfg = format!(
        "schema_version = 1\n\n[settings]\n{}\n{}",
        settings.join("\n"),
        keys,
    );
    fs::write(&file, cfg).expect("write_sandbox_config: write config.toml");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_is_deterministic_and_unit_length() {
        let a = deterministic_vector("agent memory architecture");
        let b = deterministic_vector("agent memory architecture");
        assert_eq!(a, b, "same text must yield the same vector");
        assert_eq!(a.len(), STUB_DIM);
        let norm: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "vector must be L2-normalised");
    }

    #[test]
    fn shared_tokens_score_higher_than_disjoint_ones() {
        let base = deterministic_vector("jwt authentication rotation");
        let near = deterministic_vector("jwt authentication policy");
        let far = deterministic_vector("kubernetes ingress controller");
        let cos = |x: &[f32], y: &[f32]| -> f32 { x.iter().zip(y).map(|(a, b)| a * b).sum() };
        assert!(
            cos(&base, &near) > cos(&base, &far),
            "overlapping text must be closer than unrelated text"
        );
    }

    #[test]
    fn empty_text_is_not_the_zero_vector() {
        let v = deterministic_vector("   ");
        assert!(v.iter().any(|x| *x != 0.0), "stub must never return zeros");
    }
}
