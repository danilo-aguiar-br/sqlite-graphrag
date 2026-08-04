// GAP-SG-150 — one NDJSON contract for every `ingest` mode.
//
// `ingest` used to describe itself with three different shapes: the standard
// pipeline emitted `IngestFileEvent`/`IngestSummary`, `--mode claude-code`
// emitted its own pair, and `--mode codex` emitted a third one that renamed the
// summary counters (`completed`/`failed`/`skipped`) and the success status
// (`done`). An agent consuming the subcommand needed three parsers and had to
// know which backend had run before it could read a line.
//
// This suite pins the unification: one line shape per event kind, published as
// `docs/schemas/ingest-file-event.schema.json` and
// `docs/schemas/ingest-summary.schema.json`, validated with the same
// `jsonschema::Validator` pattern as `tests/schema_contract_strict.rs`.
//
// The fixtures below mirror, field for field, what each emission site actually
// serialises. They are hand-written rather than produced by the structs because
// `IngestFileEvent` and `IngestSummary` are `pub(crate)` and unreachable from an
// integration test. Each fixture names its source site so a drift is traceable.

use serde_json::{json, Value};

const FILE_EVENT_SCHEMA: &str = include_str!("../docs/schemas/ingest-file-event.schema.json");
const SUMMARY_SCHEMA: &str = include_str!("../docs/schemas/ingest-summary.schema.json");
const CLAUDE_FILE_EVENT_ALIAS: &str =
    include_str!("../docs/schemas/ingest-claude-file-event.schema.json");
const CLAUDE_SUMMARY_ALIAS: &str =
    include_str!("../docs/schemas/ingest-claude-summary.schema.json");

/// Replaces every absolute `$ref` with an accept-anything subschema.
///
/// The ingest schemas point `agent_surface` at `agent-surface.schema.json` by
/// absolute URI. Compiling that as-is would make the validator try to retrieve a
/// remote resource, so this suite would fail offline for a reason that has
/// nothing to do with the ingest contract. The referenced shape is covered by
/// the agent-surface suite; here only the ingest fields matter.
fn defuse_remote_refs(node: &mut Value) {
    let remote = node
        .get("$ref")
        .and_then(Value::as_str)
        .is_some_and(|r| r.starts_with("http://") || r.starts_with("https://"));
    if remote {
        *node = json!({});
        return;
    }
    match node {
        Value::Object(map) => {
            for value in map.values_mut() {
                defuse_remote_refs(value);
            }
        }
        Value::Array(items) => {
            for item in items {
                defuse_remote_refs(item);
            }
        }
        _ => {}
    }
}

/// Parses a published schema into a compilable, offline-safe `Value`.
fn compilable(label: &str, schema_str: &str) -> Value {
    let mut schema: Value = serde_json::from_str(schema_str)
        .unwrap_or_else(|e| panic!("[{label}] schema is not valid JSON: {e}"));
    defuse_remote_refs(&mut schema);
    schema
}

/// Validates `instance` against `schema_str`, reporting every violation at once.
fn validate(label: &str, schema_str: &str, instance: &Value) {
    let schema = compilable(label, schema_str);
    let validator = jsonschema::Validator::new(&schema)
        .unwrap_or_else(|e| panic!("[{label}] schema failed to compile: {e}"));
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("  - path={} kind={:?}", e.instance_path, e.kind))
        .collect();
    assert!(
        errors.is_empty(),
        "[{label}] {n} schema violation(s):\n{list}\ninstance: {inst}",
        n = errors.len(),
        list = errors.join("\n"),
        inst = serde_json::to_string_pretty(instance).unwrap_or_default()
    );
}

// ---------------------------------------------------------------------------
// Per-file event — one schema, three producers
// ---------------------------------------------------------------------------

/// `--mode none`, success path (src/commands/ingest/persist_loop.rs).
fn event_mode_none_indexed() -> Value {
    json!({
        "file": "/corpus/adr-0042.md",
        "name": "adr-0042",
        "status": "indexed",
        "truncated": false,
        "original_filename": "adr-0042",
        "body_length": 4096,
        "memory_id": 17,
        "action": "created",
        "backend_invoked": "openrouter"
    })
}

/// `--mode none`, dry-run path (src/commands/ingest/dry_run.rs).
///
/// This site emits the legacy `skip` spelling instead of `skipped`; the schema
/// documents it as an alias rather than silently failing on real output.
fn event_mode_none_dry_run_skip() -> Value {
    json!({
        "file": "/corpus/dup.md",
        "name": "dup",
        "status": "skip",
        "truncated": false,
        "body_length": 0,
        "error": "duplicate name"
    })
}

/// `--mode claude-code`, success path (src/commands/ingest_claude/progress.rs).
fn event_mode_claude_indexed() -> Value {
    json!({
        "file": "/corpus/adr-0042.md",
        "name": "adr-0042-retry-budget",
        "status": "indexed",
        "truncated": false,
        "body_length": 4096,
        "memory_id": 18,
        "action": "updated",
        "entities": 7,
        "rels": 4,
        "cost_usd": 0.0123,
        "elapsed_ms": 8421,
        "index": 3,
        "total": 12
    })
}

/// `--mode claude-code`, OAuth subscription: cost is unknown and omitted.
fn event_mode_claude_indexed_no_cost() -> Value {
    json!({
        "file": "/corpus/adr-0043.md",
        "name": "adr-0043",
        "status": "indexed",
        "truncated": false,
        "body_length": 512,
        "memory_id": 19,
        "action": "created",
        "entities": 2,
        "rels": 1,
        "elapsed_ms": 5000,
        "index": 4,
        "total": 12
    })
}

/// `--mode codex`, success path (src/commands/ingest_codex/run.rs).
///
/// Carries the two fields only this mode produces: `input_tokens` and
/// `output_tokens`. It reports no `action` and no `cost_usd`, because the Codex
/// CLI response carries neither.
fn event_mode_codex_indexed() -> Value {
    json!({
        "file": "/corpus/adr-0042.md",
        "name": "adr-0042-retry-budget",
        "status": "indexed",
        "truncated": false,
        "body_length": 4096,
        "memory_id": 20,
        "entities": 7,
        "rels": 0,
        "input_tokens": 9100,
        "output_tokens": 640,
        "elapsed_ms": 6200,
        "index": 0,
        "total": 12
    })
}

/// `--mode codex`, oversized body rejected before the LLM call.
fn event_mode_codex_skipped() -> Value {
    json!({
        "file": "/corpus/huge.md",
        "name": "",
        "status": "skipped",
        "truncated": false,
        "body_length": 0,
        "error": "file body exceeds 512000 byte limit (900000 bytes)",
        "elapsed_ms": 3,
        "index": 1,
        "total": 12
    })
}

/// Every mode's dry-run preview event.
fn event_preview() -> Value {
    json!({
        "file": "/corpus/adr-0042.md",
        "name": "adr-0042",
        "status": "preview",
        "truncated": false,
        "body_length": 0,
        "index": 0,
        "total": 12
    })
}

/// Name truncation, the one path that fills `original_name` (`--mode none`).
fn event_truncated() -> Value {
    json!({
        "file": "/corpus/a-very-long-file-name.md",
        "name": "a-very-long-file-name-truncated-to-sixty-characters-abcdefgh",
        "status": "indexed",
        "truncated": true,
        "original_name": "a-very-long-file-name-truncated-to-sixty-characters-abcdefghij-and-more",
        "original_filename": "a-very-long-file-name",
        "body_length": 128,
        "memory_id": 21,
        "action": "created"
    })
}

fn all_file_events() -> Vec<(&'static str, Value)> {
    vec![
        ("mode-none/indexed", event_mode_none_indexed()),
        ("mode-none/dry-run-skip", event_mode_none_dry_run_skip()),
        ("mode-claude-code/indexed", event_mode_claude_indexed()),
        (
            "mode-claude-code/indexed-oauth",
            event_mode_claude_indexed_no_cost(),
        ),
        ("mode-codex/indexed", event_mode_codex_indexed()),
        ("mode-codex/skipped", event_mode_codex_skipped()),
        ("any-mode/preview", event_preview()),
        ("mode-none/truncated", event_truncated()),
    ]
}

#[test]
fn every_mode_file_event_matches_one_schema() {
    for (label, instance) in all_file_events() {
        validate(label, FILE_EVENT_SCHEMA, &instance);
    }
}

// ---------------------------------------------------------------------------
// Summary — one schema, three producers
// ---------------------------------------------------------------------------

/// `--mode none` (src/commands/ingest/persist_loop.rs): no graph, no LLM cost.
fn summary_mode_none() -> Value {
    json!({
        "summary": true,
        "dir": "/corpus",
        "pattern": "*.md",
        "recursive": true,
        "files_total": 12,
        "files_succeeded": 10,
        "files_failed": 1,
        "files_skipped": 1,
        "elapsed_ms": 42000,
        "backend_invoked": "openrouter"
    })
}

/// `--mode claude-code`: graph counters plus cumulative cost.
fn summary_mode_claude() -> Value {
    json!({
        "summary": true,
        "dir": "/corpus",
        "pattern": "*.md",
        "recursive": true,
        "files_total": 12,
        "files_succeeded": 12,
        "files_failed": 0,
        "files_skipped": 0,
        "elapsed_ms": 98000,
        "entities_total": 84,
        "rels_total": 51,
        "cost_usd": 0.1477
    })
}

/// `--mode codex`: graph counters plus token totals, and no cost figure.
fn summary_mode_codex() -> Value {
    json!({
        "summary": true,
        "dir": "/corpus",
        "pattern": "*.md",
        "recursive": true,
        "files_total": 12,
        "files_succeeded": 11,
        "files_failed": 1,
        "files_skipped": 0,
        "elapsed_ms": 76000,
        "entities_total": 77,
        "rels_total": 0,
        "input_tokens_total": 101_200,
        "output_tokens_total": 7_040
    })
}

fn all_summaries() -> Vec<(&'static str, Value)> {
    vec![
        ("mode-none/summary", summary_mode_none()),
        ("mode-claude-code/summary", summary_mode_claude()),
        ("mode-codex/summary", summary_mode_codex()),
    ]
}

#[test]
fn every_mode_summary_matches_one_schema() {
    for (label, instance) in all_summaries() {
        validate(label, SUMMARY_SCHEMA, &instance);
    }
}

// ---------------------------------------------------------------------------
// The unification is the point: the old per-mode names must stay rejected
// ---------------------------------------------------------------------------

/// The pre-unification `--mode codex` summary used `completed`/`failed`/
/// `skipped` and omitted `dir`/`pattern`/`recursive`. If that shape ever
/// validates again, the three contracts have grown back.
#[test]
fn legacy_codex_summary_is_rejected() {
    let legacy = json!({
        "summary": true,
        "files_total": 12,
        "completed": 11,
        "failed": 1,
        "skipped": 0,
        "entities_total": 77,
        "rels_total": 0,
        "input_tokens_total": 101_200,
        "output_tokens_total": 7_040,
        "elapsed_ms": 76000
    });
    let schema = compilable("legacy-codex-summary", SUMMARY_SCHEMA);
    let validator = jsonschema::Validator::new(&schema).expect("summary schema compiles");
    assert!(
        !validator.is_valid(&legacy),
        "the pre-unification codex summary shape still validates; \
         the per-mode contracts have grown back"
    );
}

/// The pre-unification `--mode codex` success status was `done`.
#[test]
fn legacy_codex_done_status_is_rejected() {
    let mut legacy = event_mode_codex_indexed();
    legacy["status"] = json!("done");
    let schema = compilable("legacy-codex-done", FILE_EVENT_SCHEMA);
    let validator = jsonschema::Validator::new(&schema).expect("event schema compiles");
    assert!(
        !validator.is_valid(&legacy),
        "status `done` still validates; `--mode codex` has drifted back off the \
         shared `indexed` vocabulary"
    );
}

// ---------------------------------------------------------------------------
// The claude-specific schema files survive only as verbatim aliases
// ---------------------------------------------------------------------------

/// Asserts an alias schema still carries exactly the canonical shape.
///
/// `$id`, `title` and `description` are expected to differ — everything that
/// constrains an instance must not.
fn assert_alias_of(alias_label: &str, alias_str: &str, canonical_str: &str) {
    let alias: Value = serde_json::from_str(alias_str)
        .unwrap_or_else(|e| panic!("[{alias_label}] alias schema is not valid JSON: {e}"));
    let canonical: Value =
        serde_json::from_str(canonical_str).expect("canonical schema is valid JSON");
    for key in ["type", "required", "additionalProperties", "properties"] {
        assert_eq!(
            alias.get(key),
            canonical.get(key),
            "[{alias_label}] `{key}` drifted away from the canonical schema; \
             the alias must stay a verbatim copy or be retired"
        );
    }
}

#[test]
fn claude_file_event_schema_is_a_verbatim_alias() {
    assert_alias_of(
        "ingest-claude-file-event",
        CLAUDE_FILE_EVENT_ALIAS,
        FILE_EVENT_SCHEMA,
    );
}

#[test]
fn claude_summary_schema_is_a_verbatim_alias() {
    assert_alias_of(
        "ingest-claude-summary",
        CLAUDE_SUMMARY_ALIAS,
        SUMMARY_SCHEMA,
    );
}

/// Every mode's events must also validate against the deprecated aliases, so a
/// consumer pinned to the old `$id` is not broken by the unification.
#[test]
fn deprecated_aliases_still_accept_every_mode() {
    for (label, instance) in all_file_events() {
        validate(
            &format!("alias/{label}"),
            CLAUDE_FILE_EVENT_ALIAS,
            &instance,
        );
    }
    for (label, instance) in all_summaries() {
        validate(&format!("alias/{label}"), CLAUDE_SUMMARY_ALIAS, &instance);
    }
}
