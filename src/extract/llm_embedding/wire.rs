//! Wire-format helpers for LLM embedding responses.
//!
//! Owns the JSON schemas sent to headless LLM backends (Claude `--json-schema`,
//! Codex `--output-schema`) and the multi-backend response parsers that accept
//! Claude single-object JSON, Codex JSONL, and OpenCode NDJSON.
//!
//! Dimension fields (`minItems` / `maxItems`) pin the active embedding dim so
//! the client rejects silent truncation/padding (G42/C5). This is the LLM-side
//! counterpart of OpenRouter MRL wire dimension handling.

use serde::Deserialize;

/// G42/S1: single-vector JSON schema generated from the active dim.
pub(super) fn build_single_schema(dim: usize) -> String {
    format!(
        r#"{{"type":"object","properties":{{"embedding":{{"type":"array","items":{{"type":"number"}},"minItems":{dim},"maxItems":{dim}}}}},"required":["embedding"],"additionalProperties":false}}"#
    )
}

/// G42/S2: batch JSON schema `{items:[{i,v}]}`. The `items` array length
/// is deliberately unconstrained so ONE schema file serves every batch
/// size (index coverage is validated in Rust after parsing).
pub(super) fn build_batch_schema(dim: usize) -> String {
    format!(
        r#"{{"type":"object","properties":{{"items":{{"type":"array","items":{{"type":"object","properties":{{"i":{{"type":"integer"}},"v":{{"type":"array","items":{{"type":"number"}},"minItems":{dim},"maxItems":{dim}}}}},"required":["i","v"],"additionalProperties":false}}}}}},"required":["items"],"additionalProperties":false}}"#
    )
}

/// Claude / single-vector wire response: `{ "embedding": [f32; dim] }`.
#[derive(Debug, Deserialize)]
pub(super) struct EmbeddingResponse {
    pub(super) embedding: Vec<f32>,
}

/// Batch wire response: `{ "items": [{ "i": 1-based, "v": [f32; dim] }] }`.
#[derive(Debug, Deserialize)]
pub(super) struct BatchEmbeddingResponse {
    pub(super) items: Vec<BatchEmbeddingItem>,
}

/// One item inside a batch embedding wire response.
#[derive(Debug, Deserialize)]
pub(super) struct BatchEmbeddingItem {
    pub(super) i: usize,
    pub(super) v: Vec<f32>,
}

/// Parse an LLM JSON response of type `T`. The three backends emit
/// different shapes:
/// - Claude (with `--output-format json`): single JSON object on stdout.
/// - Codex (with `--json`): JSONL stream with one event per line; the
///   `agent_message` event's `text` field is the JSON payload.
/// - OpenCode (with `--format json`): NDJSON `type == "text"` parts.
///
/// This helper accepts all three shapes and returns the parsed value (or an
/// error describing the first mismatch).
pub(super) fn parse_llm_json<T: serde::de::DeserializeOwned>(stdout: &str) -> Result<T, String> {
    // Strategy 1: try the whole stdout as JSON (Claude path).
    if let Ok(parsed) = serde_json::from_str::<T>(stdout) {
        return Ok(parsed);
    }
    // Strategy 3: walk NDJSON and collect `.part.text` from `type == "text"`
    // events (OpenCode path: `opencode run --format json`).
    let mut opencode_texts: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(text) = event
                .get("part")
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str())
            {
                opencode_texts.push(text.to_string());
            }
        }
    }
    if !opencode_texts.is_empty() {
        let combined = opencode_texts.concat();
        if let Ok(parsed) = serde_json::from_str::<T>(&combined) {
            return Ok(parsed);
        }
    }
    // Strategy 2: walk the JSONL line by line and pick the last
    // `item.completed` of type `agent_message` (Codex path).
    let mut last_agent_text: Option<String> = None;
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("type").and_then(|t| t.as_str()) != Some("item.completed") {
            continue;
        }
        let item = match event.get("item") {
            Some(i) => i,
            None => continue,
        };
        if item.get("type").and_then(|t| t.as_str()) != Some("agent_message") {
            continue;
        }
        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
            last_agent_text = Some(text.to_string());
        }
    }
    let text = last_agent_text
        .ok_or_else(|| "no agent_message found in codex JSONL output".to_string())?;
    serde_json::from_str::<T>(&text)
        .map_err(|e| format!("codex agent_message text does not match schema: {e}; raw={text}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_schema_embeds_active_dim() {
        let schema = build_single_schema(64);
        assert!(schema.contains(r#""minItems":64"#));
        assert!(schema.contains(r#""maxItems":64"#));
        let parsed: serde_json::Value =
            serde_json::from_str(&schema).expect("single schema must be valid JSON");
        assert_eq!(parsed["properties"]["embedding"]["minItems"], 64);
    }

    #[test]
    fn batch_schema_is_valid_json_and_unbounded_items() {
        let schema = build_batch_schema(64);
        let parsed: serde_json::Value =
            serde_json::from_str(&schema).expect("batch schema must be valid JSON");
        // The items array must NOT constrain its length so one schema
        // file serves every batch size (G42/S4).
        assert!(parsed["properties"]["items"].get("minItems").is_none());
        assert_eq!(
            parsed["properties"]["items"]["items"]["properties"]["v"]["minItems"],
            64
        );
    }

    #[test]
    fn parse_llm_json_accepts_claude_json() {
        let stdout = r#"{"embedding":[0.0,1.0,2.0]}"#;

        let parsed: EmbeddingResponse = parse_llm_json(stdout).expect("claude JSON must parse");

        assert_eq!(parsed.embedding, vec![0.0, 1.0, 2.0]);
    }

    #[test]
    fn parse_llm_json_accepts_codex_jsonl() {
        let stdout = r#"{"type":"thread.started","thread_id":"mock-thread-0"}
{"type":"item.completed","item":{"type":"agent_message","text":"{\"embedding\":[0.0,1.0,2.0]}"}}
{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#;

        let parsed: EmbeddingResponse = parse_llm_json(stdout).expect("codex JSONL must parse");

        assert_eq!(parsed.embedding, vec![0.0, 1.0, 2.0]);
    }

    #[test]
    fn parse_llm_json_rejects_jsonl_without_agent_message() {
        let stdout = r#"{"type":"thread.started","thread_id":"mock-thread-0"}"#;

        let err = parse_llm_json::<EmbeddingResponse>(stdout)
            .expect_err("missing agent_message must fail");

        assert!(err.contains("no agent_message"));
    }

    #[test]
    fn parse_llm_json_accepts_batch_response() {
        let stdout = r#"{"items":[{"i":1,"v":[0.0,1.0]},{"i":2,"v":[2.0,3.0]}]}"#;

        let parsed: BatchEmbeddingResponse = parse_llm_json(stdout).expect("batch JSON must parse");

        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].i, 1);
        assert_eq!(parsed.items[1].v, vec![2.0, 3.0]);
    }

    #[test]
    fn parse_llm_json_accepts_opencode_ndjson() {
        let stdout = r#"{"type":"step_start","timestamp":1234,"sessionID":"ses_test","part":{"type":"step-start"}}
{"type":"text","timestamp":1235,"sessionID":"ses_test","part":{"type":"text","text":"{\"embedding\":[0.1,0.2,0.3]}"}}
{"type":"step_finish","timestamp":1236,"sessionID":"ses_test","part":{"type":"step-finish","tokens":{"total":100,"input":90,"output":10,"reasoning":0},"cost":0}}"#;

        let parsed: EmbeddingResponse = parse_llm_json(stdout).expect("opencode NDJSON must parse");
        assert_eq!(parsed.embedding, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn parse_llm_json_accepts_opencode_batch_ndjson() {
        let stdout = r#"{"type":"step_start","timestamp":1234,"sessionID":"ses_test","part":{"type":"step-start"}}
{"type":"text","timestamp":1235,"sessionID":"ses_test","part":{"type":"text","text":"{\"items\":[{\"i\":1,\"v\":[0.1,0.2]},{\"i\":2,\"v\":[0.3,0.4]}]}"}}
{"type":"step_finish","timestamp":1236,"sessionID":"ses_test","part":{"type":"step-finish","tokens":{"total":100,"input":90,"output":10,"reasoning":0},"cost":0}}"#;

        let parsed: BatchEmbeddingResponse =
            parse_llm_json(stdout).expect("opencode batch NDJSON must parse");
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].i, 1);
        assert_eq!(parsed.items[1].v, vec![0.3, 0.4]);
    }
}
