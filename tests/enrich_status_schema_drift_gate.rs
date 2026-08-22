//! The published `enrich` schemas must cover every key the types emit.
//!
//! `docs/schemas/enrich-*.schema.json` are CHECKED-IN artefacts. Nothing forced
//! the hand that maintains them to move: `EnrichStatus` gained
//! `sampled_without_corpus`, then `grounding_percentiles`, and the file stayed
//! frozen through both. The drift was silent because the schema declares
//! `additionalProperties: true` (Must-Ignore, RFC 7493), so an extra key
//! validates fine — the document is simply wrong about what the command emits,
//! which is worse than a hard failure because nothing complains.
//!
//! `health.schema.json` has a drift gate, and `enrich-status` did not. This is
//! that gate, written the way the health one should have been: it derives the
//! expected key set from the TYPE rather than from a hand-written list. A
//! literal list is a second copy of the truth, and the health gate's own list
//! is already pinned at "36 keys" in a comment — the exact shape of rot this
//! avoids.
//!
//! Watching only `--status` is how the NEXT drift survived. `enrich-summary`
//! promised `agent_surface`, `count`, `truncated` and `llm_parallelism` — four
//! members `enrich` has never emitted, because it writes through the unshaped
//! `emit_json_line` path documented in `src/output/stream.rs` — while omitting
//! `retyped`, which it does emit. `enrich-item-event` omitted `reason`,
//! `previous_type`, `validated_type` and `evidence_chars`, and its `status`
//! enum listed a value no call site produces while missing two that do. Both
//! files are now derived from their structs here, so the same silence cannot
//! form a third time.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Reads a published schema document under `docs/schemas/`.
fn published_schema(id: &str) -> serde_json::Value {
    let path = repo_root()
        .join("docs")
        .join("schemas")
        .join(format!("{id}.schema.json"));
    let body = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("{id}.schema.json must be valid JSON: {e}"))
}

/// Property names a published schema declares.
fn published_keys(id: &str) -> BTreeSet<String> {
    published_schema(id)["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{id}.schema.json must declare `properties`"))
        .keys()
        .cloned()
        .collect()
}

/// Property names a published schema lists under `required`.
fn published_required(id: &str) -> BTreeSet<String> {
    published_schema(id)["required"]
        .as_array()
        .unwrap_or_else(|| panic!("{id}.schema.json must declare `required`"))
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| panic!("{id}.schema.json `required` must hold strings"))
                .to_string()
        })
        .collect()
}

/// Property names the LIVE `EnrichStatus` type generates, straight from schemars.
fn generated_keys() -> BTreeSet<String> {
    let schema = schemars::schema_for!(sqlite_graphrag::commands::enrich::EnrichStatus);
    let value = serde_json::to_value(&schema).expect("generated schema must serialize");
    value["properties"]
        .as_object()
        .expect("generated schema must declare `properties`")
        .keys()
        .cloned()
        .collect()
}

#[test]
fn the_published_enrich_status_schema_declares_every_emitted_key() {
    let published = published_keys("enrich-status");
    let generated = generated_keys();

    assert!(
        !generated.is_empty(),
        "the generated schema declared no properties at all, which means this \
         gate is reading the wrong place rather than that the type is empty"
    );

    let missing: Vec<&String> = generated.difference(&published).collect();
    assert!(
        missing.is_empty(),
        "`EnrichStatus` emits key(s) the published schema does not declare: \
         {missing:?}.\nRegenerate with `cargo run --bin dump_schema -- \
         enrich-status`. The file validates anyway because it is Must-Ignore, \
         so nothing else will tell you it is stale."
    );

    let stale: Vec<&String> = published.difference(&generated).collect();
    assert!(
        stale.is_empty(),
        "the published schema declares key(s) `EnrichStatus` no longer emits: \
         {stale:?}.\nRegenerate with `cargo run --bin dump_schema -- \
         enrich-status`; a schema that promises a removed field misleads every \
         consumer that reads it."
    );
}

// ---------------------------------------------------------------------------
// The NDJSON event contracts, read from their structs
// ---------------------------------------------------------------------------

/// One member of a `Serialize` struct, as the wire sees it.
#[derive(Debug, PartialEq, Eq)]
struct WireField {
    name: String,
    /// True when `skip_serializing_if = "Option::is_none"` guards the member,
    /// which makes an empty value an ABSENT KEY rather than an explicit null.
    optional: bool,
}

/// Reads the serialized members of a `pub(crate)` struct out of its source.
///
/// `EnrichSummary` and `ItemEvent` are crate-private and derive neither
/// `JsonSchema` nor anything else an integration test could reflect over, so
/// the source text is the only place their wire shape is written down. Parsing
/// it is the same trade `docs_declared_facts_gate` makes against
/// `src/config/registry.rs`: reading the definition beats keeping a second copy
/// of it in a test.
///
/// The parser refuses a source carrying `serde(rename`, because a renamed
/// member would make the Rust identifier and the wire key differ and every
/// assertion below would then compare the wrong two things while still passing.
fn wire_fields(source: &str) -> Vec<WireField> {
    assert!(
        !source.contains("serde(rename"),
        "this parser reads the Rust identifier as the wire key; a `serde(rename` \
         attribute breaks that assumption silently, so teach the parser about it \
         before adding one"
    );

    let mut fields = Vec::new();
    let mut optional = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with("#[") {
            if trimmed.contains("skip_serializing_if") {
                optional = true;
            }
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("pub(crate) ") else {
            continue;
        };
        let Some(colon) = rest.find(':') else {
            continue;
        };
        let name = rest[..colon].trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        fields.push(WireField {
            name: name.to_string(),
            optional,
        });
        optional = false;
    }
    fields
}

/// Reads the serialized members of a struct from a file under `src/`.
fn wire_fields_of(relative: &str) -> Vec<WireField> {
    let path = repo_root().join(relative);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let fields = wire_fields(&source);
    assert!(
        fields.len() >= 10,
        "the parser found {} member(s) in {relative}, too few to be the real \
         struct — every assertion built on this would be passing on an empty set",
        fields.len()
    );
    fields
}

/// Asserts a published schema declares exactly the members a struct serializes.
fn assert_contract_matches_struct(id: &str, relative: &str) {
    let fields = wire_fields_of(relative);
    let declared = published_keys(id);
    let emitted: BTreeSet<String> = fields.iter().map(|f| f.name.clone()).collect();

    let missing: Vec<&String> = emitted.difference(&declared).collect();
    assert!(
        missing.is_empty(),
        "{relative} serializes key(s) `{id}.schema.json` does not declare: \
         {missing:?}. The schema also sets `additionalProperties: false`, so a \
         consumer validating a real line against it REJECTS output the command \
         is correct to emit."
    );

    let stale: Vec<&String> = declared.difference(&emitted).collect();
    assert!(
        stale.is_empty(),
        "`{id}.schema.json` declares key(s) {relative} never serializes: \
         {stale:?}. A promised member that never arrives sends a consumer \
         looking for a value that does not exist."
    );

    let required_by_struct: BTreeSet<String> = fields
        .iter()
        .filter(|f| !f.optional)
        .map(|f| f.name.clone())
        .collect();
    assert_eq!(
        published_required(id),
        required_by_struct,
        "`{id}.schema.json` disagrees with {relative} about which members are \
         always present. A member guarded by `skip_serializing_if` is ABSENT, \
         not null, so listing it as required makes every run that omits it fail \
         validation; leaving a always-emitted member out lets a consumer treat \
         it as optional and never handle it."
    );
}

#[test]
fn the_published_enrich_summary_schema_matches_the_summary_struct() {
    assert_contract_matches_struct("enrich-summary", "src/commands/enrich/events/summary.rs");
}

#[test]
fn the_published_enrich_item_event_schema_matches_the_item_struct() {
    assert_contract_matches_struct("enrich-item-event", "src/commands/enrich/events/item.rs");
}

/// The `status` enum must list every value a call site actually writes.
///
/// The published enum offered `not_yet_implemented`, which no call site
/// produces, and omitted `retyped` and `preservation_failed`, which two do.
/// Because the document is `additionalProperties: false` AND enumerates this
/// member, a consumer validating a real `retyped` line rejected correct output
/// — the strictest possible failure mode for the one field a reader branches on.
#[test]
fn the_item_event_status_enum_lists_every_status_a_call_site_writes() {
    let declared: BTreeSet<String> = published_schema("enrich-item-event")["properties"]["status"]
        ["enum"]
        .as_array()
        .expect("enrich-item-event.schema.json must enumerate `status`")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("status enum must hold strings")
                .to_string()
        })
        .collect();

    let mut emitted = BTreeSet::new();
    let dir = repo_root().join("src/commands/enrich");
    for path in rust_sources(&dir) {
        let source = std::fs::read_to_string(&path).expect("enrich source must be readable");
        for hit in source.match_indices("status: \"") {
            let rest = &source[hit.0 + "status: \"".len()..];
            if let Some(end) = rest.find('"') {
                emitted.insert(rest[..end].to_string());
            }
        }
    }

    assert!(
        !emitted.is_empty(),
        "the scan found no `status: \"…\"` literal under src/commands/enrich, \
         which means it is reading the wrong place rather than that the command \
         emits no status"
    );
    assert_eq!(
        declared, emitted,
        "the `status` enum in enrich-item-event.schema.json disagrees with the \
         literals under src/commands/enrich. A value emitted but not enumerated \
         makes a validating consumer REJECT correct output; a value enumerated \
         but never emitted makes a consumer branch on a state that cannot occur."
    );
}

/// Every `.rs` file under `dir`, recursively.
fn rust_sources(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            found.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
    found
}

// ---------------------------------------------------------------------------
// The parser is itself tested, so a silent parser cannot fake a pass
// ---------------------------------------------------------------------------

#[test]
fn the_field_parser_separates_required_members_from_skipped_ones() {
    let source = r#"
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct Example {
    pub(crate) summary: bool,
    /// A doc comment mentioning pub(crate) in prose must not become a field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) retyped: Option<usize>,
    pub(crate) waiting: i64,
}
"#;
    let fields = wire_fields(source);
    assert_eq!(
        fields
            .iter()
            .map(|f| (f.name.as_str(), f.optional))
            .collect::<Vec<_>>(),
        vec![("summary", false), ("retyped", true), ("waiting", false)]
    );
}

#[test]
fn the_field_parser_ignores_the_struct_header_and_lifetimes() {
    let source = r#"
pub(crate) struct ItemEvent<'a> {
    pub(crate) item: &'a str,
    pub(crate) index: usize,
}
"#;
    let fields = wire_fields(source);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "item");
    assert_eq!(fields[1].name, "index");
}

#[test]
fn the_field_parser_does_not_carry_a_skip_attribute_to_the_next_member() {
    let source = r#"
pub(crate) struct Example {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) a: Option<usize>,
    pub(crate) b: usize,
}
"#;
    let fields = wire_fields(source);
    assert!(fields[0].optional);
    assert!(
        !fields[1].optional,
        "an attribute belongs to the member it precedes; leaking it forward \
         would mark an always-emitted member optional and silence the very \
         disagreement this gate exists to report"
    );
}
