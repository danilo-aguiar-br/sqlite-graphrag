//! GAP-SG-215: an invariant declared in prose is not an invariant.
//!
//! `src/agent_surface/mod.rs` has stated since GAP-SG-142 that "NDJSON streams
//! bypass the surface", and `src/output/stream.rs` implemented that bypass in
//! `emit_json_line`. `export` never called it. It used `emit_json_compact`,
//! which routes every value through the envelope renderer, so the surface ran
//! once per LINE for two releases. Nothing — not a type, not a lint, not a test
//! — could notice, because the invariant existed only as a sentence.
//!
//! This file is the sentence made executable. It is the same countermeasure
//! `refusal_message_call_site_gate.rs` applies one level down: there, a message
//! with no call site; here, an invariant with no witness.
//!
//! # What it pins
//!
//! * A record line carries the record and nothing else — no `agent_surface`.
//! * The trailer carries the block, once, and is never reshaped.
//! * A knob that cannot act on a stream is refused with stdout still empty.
//!
//! The third matters as much as the first two: the reported defect was not only
//! that `--select name export` failed, but that it failed on the FOURTH line,
//! after three valid records had already been written to a consumer.

use serde_json::Value;

#[path = "common/mod.rs"]
mod common;

/// Member the contract keeps off every record line.
const META_KEY: &str = "agent_surface";

/// Seeds two memories so the export has records to judge.
///
/// Two rather than one: a single record cannot distinguish "the last line is
/// exempt" from "every line is exempt", and that distinction is the contract.
fn seeded() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("temp dir");
    common::init_db(&dir);
    for (name, description) in [
        ("alice-onboarding-checklist", "checklist sintético"),
        ("alice-martins-souza", "perfil sintético"),
    ] {
        common::cmd(&dir)
            .args([
                "remember",
                "--name",
                name,
                "--type",
                "note",
                "--description",
                description,
                "--body",
                "corpo sintético com texto suficiente para o registro existir",
                "--namespace",
                "global",
            ])
            .assert()
            .success();
    }
    dir
}

/// Runs `export` with the given global flags and splits stdout into lines.
fn export(dir: &tempfile::TempDir, globals: &[&str]) -> (i32, Vec<Value>, String) {
    let mut command = common::cmd(dir);
    command.args(globals).args(["export"]);
    let output = command.output().expect("spawn export");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let lines = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|e| panic!("line is not JSON ({e}): {line}"))
        })
        .collect();
    (
        output.status.code().unwrap_or(-1),
        lines,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The `required` list a published schema declares for a stream line.
fn schema_required(file: &str) -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/schemas")
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {file}: {e}"));
    let doc: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {file}: {e}"));
    doc["required"]
        .as_array()
        .unwrap_or_else(|| panic!("{file} must declare `required`"))
        .iter()
        .map(|v| {
            v.as_str()
                .expect("required entries are strings")
                .to_string()
        })
        .collect()
}

/// The gate's own premises, asserted so it cannot pass by not working.
///
/// Every test below reads "for each record line, ...". With zero record lines
/// those loops are vacuously true and the whole file turns green while the
/// stream stays broken. This pins that the fixture really produces records, and
/// that the schemas it compares against really declare a `required` list.
#[test]
fn the_gate_detects_what_it_claims_to_detect() {
    let dir = seeded();
    let (code, lines, stderr) = export(&dir, &[]);
    assert_eq!(code, 0, "baseline export failed: {stderr}");
    assert!(
        lines.len() >= 3,
        "the fixture must yield at least two records plus a trailer, got {}: \
         every per-record assertion in this file would otherwise pass vacuously",
        lines.len()
    );

    let required = schema_required("export-memory-line.schema.json");
    assert!(
        required.contains(&"name".to_string()),
        "the record schema must declare `name` required, got {required:?}"
    );
    assert!(
        schema_required("export-summary.schema.json").contains(&"summary".to_string()),
        "the trailer schema must declare `summary` required"
    );
}

#[test]
fn a_record_line_carries_the_record_and_nothing_else() {
    // Measured at 278 bytes of `agent_surface` per line before this — with no
    // flag set at all — restating one fact about the process once per memory,
    // absolute database path included, into a file the docs recommend creating
    // with `export > backup.ndjson`.
    let dir = seeded();
    let (code, lines, stderr) = export(&dir, &[]);
    assert_eq!(code, 0, "export failed: {stderr}");

    let (trailer, records) = lines.split_last().expect("a non-empty stream");
    for record in records {
        assert!(
            record.get(META_KEY).is_none(),
            "a record line must carry no {META_KEY}: {record}"
        );
    }
    assert!(
        trailer.get("summary").is_some(),
        "the last line must be the trailer: {trailer}"
    );
}

#[test]
fn the_trailer_carries_the_record_for_the_whole_stream() {
    let dir = seeded();
    let (code, lines, stderr) = export(&dir, &[]);
    assert_eq!(code, 0, "export failed: {stderr}");

    let trailer = lines.last().expect("a non-empty stream");
    let meta = trailer
        .get(META_KEY)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("the trailer must carry the stream's record: {trailer}"));
    assert_eq!(meta.get("stream"), Some(&Value::Bool(true)));
    assert!(
        meta.get("db_path_resolved").is_some(),
        "GAP-SG-205's target record moves to the trailer, it does not vanish: {trailer}"
    );
}

#[test]
fn an_unflagged_record_line_matches_its_published_schema() {
    let dir = seeded();
    let (code, lines, stderr) = export(&dir, &[]);
    assert_eq!(code, 0, "export failed: {stderr}");

    let required = schema_required("export-memory-line.schema.json");
    let (_, records) = lines.split_last().expect("a non-empty stream");
    for record in records {
        let object = record.as_object().expect("a record is an object");
        for key in &required {
            assert!(
                object.contains_key(key),
                "record is missing required key {key}: {record}"
            );
        }
    }
}

#[test]
fn a_projection_aimed_at_the_records_never_reaches_the_trailer() {
    // The reported defect and its silent twin, in one test. `--select name`
    // exited 2 on the trailer after three good records; `--select namespace`
    // exited 0 and deleted `summary: true` from it, leaving a truncated export
    // indistinguishable from a complete one.
    let dir = seeded();
    for key in ["name", "namespace"] {
        let (code, lines, stderr) = export(&dir, &["--select", key]);
        assert_eq!(
            code, 0,
            "--select {key} must not fail on the trailer: {stderr}"
        );

        let (trailer, records) = lines.split_last().expect("a non-empty stream");
        for record in records {
            assert_eq!(
                record.as_object().map(|o| o.len()),
                Some(1),
                "--select {key} must leave one member on a record: {record}"
            );
            assert!(record.get(key).is_some(), "the projected key must survive");
        }
        for required in schema_required("export-summary.schema.json") {
            assert!(
                trailer.get(&required).is_some(),
                "--select {key} deleted {required} from the trailer: {trailer}"
            );
        }
    }
}

#[test]
fn a_knob_that_cannot_act_is_refused_with_stdout_still_clean() {
    // Refusing mid-stream is worse than refusing: the consumer has already been
    // handed records it must now discard, and the terminator it keys on never
    // arrives. Every refusal here has to land before the first record.
    let dir = seeded();
    for globals in [
        vec!["--count-only"],
        vec!["--max-items", "2"],
        vec!["--sort", "name"],
        vec!["--dedupe-by", "name"],
        vec!["--max-output-bytes", "4096"],
        vec!["--filter", "type=note"],
    ] {
        let (code, lines, _) = export(&dir, &globals);
        assert_eq!(code, 2, "{globals:?} must be refused on a stream");
        assert_eq!(
            lines.len(),
            1,
            "{globals:?} must refuse before any record reaches stdout, got {} lines",
            lines.len()
        );
        let envelope = &lines[0];
        assert_eq!(envelope.get("error"), Some(&Value::Bool(true)));
        let discarded = envelope["discarded_flags"]
            .as_array()
            .expect("a refusal names the flags it could not honour");
        assert!(
            discarded.iter().any(|f| f == globals[0]),
            "{globals:?} must name itself in discarded_flags, got {discarded:?}"
        );
    }
}

#[test]
fn a_per_record_knob_still_works_on_a_stream() {
    // The contract keeps a feature rather than removing one. `--select` over a
    // stream is the largest payload reduction this binary can offer, and it is
    // exactly what GAP-SG-142 exists to spare an agent from doing in `jaq`.
    let dir = seeded();
    let (code, lines, stderr) = export(&dir, &["--truncate-content", "8"]);
    assert_eq!(code, 0, "--truncate-content must act per record: {stderr}");

    let (trailer, records) = lines.split_last().expect("a non-empty stream");
    for record in records {
        let body = record["body"].as_str().expect("a record carries a body");
        assert!(
            body.chars().count() <= 8,
            "body was not shortened: {body:?}"
        );
    }
    let meta = trailer[META_KEY]
        .as_object()
        .expect("the trailer carries the record");
    assert_eq!(
        meta.get("records_truncated"),
        Some(&Value::from(records.len())),
        "truncation is never silent, and the trailer is where it is now reported: {trailer}"
    );
    assert_eq!(trailer.get("truncated"), Some(&Value::Bool(true)));
}

/// Every `.rs` file under `root`, recursively.
fn rust_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// `true` when the line CALLS a stream emitter, rather than importing one.
///
/// `src/output/mod.rs` re-exports the emitters with `pub use`, which mentions
/// every name without ever emitting a record. Matching on the bare name would
/// report the module that publishes the API as the one abusing it.
fn calls_an_emitter(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("use ") || trimmed.starts_with("pub use ") {
        return false;
    }
    line.contains("emit_stream_record(") || line.contains("emit_stream_trailer(")
}

/// `true` when the text resolves the surface.
fn opens_the_stream(source: &str) -> bool {
    source.contains("stream::open(")
}

/// `true` when the text emits stream records without ever resolving the surface.
///
/// Split out so the self-check below can drive it with fixtures rather than
/// trusting a filesystem walk to have found the defect.
fn emits_without_opening(source: &str) -> bool {
    source.lines().any(calls_an_emitter) && !opens_the_stream(source)
}

/// The unit a stream contract belongs to: one COMMAND, not one file.
///
/// `ingest` opens its stream in `src/commands/ingest/run.rs` and emits from
/// `persist_loop.rs`, `dry_run.rs` and `enrich_after.rs` — three sibling modules
/// that are one command and one stream. Judging per file reports all three as
/// offenders and says nothing true. A command implemented as a directory is that
/// directory; one implemented as a single file is that file.
fn command_unit(path: &std::path::Path, repo: &std::path::Path) -> String {
    let relative = path
        .strip_prefix(repo)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let Some(tail) = relative.strip_prefix("src/commands/") else {
        return relative;
    };
    match tail.split_once('/') {
        Some((directory, _)) => format!("src/commands/{directory}"),
        None => relative,
    }
}

#[test]
fn every_stream_emitter_resolves_the_surface_before_the_first_record() {
    // GAP-SG-229. The three behavioural tests above prove the contract for
    // `export`, and prove it well — but they prove it for `export` ALONE, by
    // invoking that one binary path. `graph --format ndjson` had been streaming
    // since v1.0.35 and satisfied none of it, because no test named it.
    //
    // The failure mode is specific and silent. `emit_stream_record` falls back to
    // `StreamState::inert()` when the command never called `stream::open`, and an
    // inert state carries an EMPTY compiled projection. So `--select name` did not
    // fail loudly on the unopened stream: it shaped every record down to `{}`.
    // Measured on the pre-fix binary, that is exactly what came out.
    //
    // A structural scan is the right shape for this because the property is a
    // property of the CALL SITE, not of any one command's output. Adding another
    // behavioural case would pin the command that was just fixed and stay blind
    // to the fourth streaming surface, which is the mistake this test exists to
    // stop repeating.
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&repo.join("src"), &mut files);
    files.sort();

    // Fold the tree into command units first, then judge each unit as a whole.
    let mut emits: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut opens: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for path in &files {
        // `src/output/stream.rs` DEFINES the emitters; it is the callee, never a
        // call site, so it has no surface of its own to resolve.
        if path.ends_with("output/stream.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let unit = command_unit(path, &repo);
        if text.lines().any(calls_an_emitter) {
            emits.insert(unit.clone());
        }
        if opens_the_stream(&text) {
            opens.insert(unit);
        }
    }
    let offenders: Vec<String> = emits.difference(&opens).cloned().collect();

    assert!(
        offenders.is_empty(),
        "these file(s) emit stream records without calling \
         `agent_surface::stream::open` first. An unopened stream falls back to \
         an inert state whose compiled projection is empty, so `--select` shapes \
         every record to `{{}}` instead of projecting it, and the whole-set \
         refusals never run. Resolve the surface before the first record, as \
         `src/commands/export.rs::open_stream` does.\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_call_site_scan_detects_what_it_claims_to_detect() {
    // The detector, driven by both shapes. Without this the scan above could
    // pass by matching nothing at all.
    assert!(
        emits_without_opening("output::emit_stream_record(&obj)?;"),
        "an emitter with no `open` is the defect this scan exists to find"
    );
    assert!(
        !emits_without_opening(
            "crate::agent_surface::stream::open(surface, &sample, total)?;\n\
             output::emit_stream_record(&obj)?;"
        ),
        "resolving the surface first is the correct shape and must stay quiet"
    );
    assert!(
        !emits_without_opening("let x = 1;"),
        "a file that streams nothing is not an offender"
    );
    // The re-export shape, verbatim from `src/output/mod.rs:42`. Matching the
    // bare name here reported the module that PUBLISHES the emitters as the one
    // abusing them.
    assert!(
        !emits_without_opening(
            "pub use stream::{emit_json_line, emit_stream_record, emit_stream_trailer};"
        ),
        "importing an emitter is not calling one"
    );
    // The grouping, pinned to the layout that produced three false reports:
    // `ingest` opens in `run.rs` and emits from three sibling modules.
    let repo = std::path::Path::new("/repo");
    assert_eq!(
        command_unit(
            std::path::Path::new("/repo/src/commands/ingest/persist_loop.rs"),
            repo
        ),
        command_unit(
            std::path::Path::new("/repo/src/commands/ingest/run.rs"),
            repo
        ),
        "sibling modules of one command are one stream and must fold together"
    );
    assert_ne!(
        command_unit(std::path::Path::new("/repo/src/commands/export.rs"), repo),
        command_unit(
            std::path::Path::new("/repo/src/commands/ingest/run.rs"),
            repo
        ),
        "two different commands must stay distinct, or one command's `open` \
         would excuse another's omission"
    );
}
