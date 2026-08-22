//! GAP-SG-216: the behaviour behind `--strict-entity-types`, the reported folds
//! and the positional name.
//!
//! Every fixture here is synthetic (`alice`). The defect was observed on a real
//! corpus; reproducing it needs only the PROPERTY — a declared `entity_type`
//! outside the canonical thirteen — never the observed data.

use serde_json::Value;

#[path = "common/mod.rs"]
mod common;

/// Graph payload declaring three labels that do not survive canonicalisation.
///
/// `practice` and `problem` reach `EntityType::Concept` through the catch-all
/// arm and `artifact` through the explicit file-like arm, so the three together
/// cover both paths a fold can take.
const THREE_FOLDS: &str = r#"{"body":"corpo sintético com texto suficiente para o registro existir",
 "entities":[
   {"name":"alice-praxis","entity_type":"practice","description":null},
   {"name":"alice-artefato","entity_type":"artifact","description":null},
   {"name":"alice-questao","entity_type":"problem","description":null}],
 "relationships":[]}"#;

/// Graph payload whose labels all survive, one of them only after normalisation.
const NO_FOLD: &str = r#"{"body":"corpo sintético com texto suficiente para o registro existir",
 "entities":[
   {"name":"alice-martins-souza","entity_type":"person","description":null},
   {"name":"alice-tracker","entity_type":"Issue-Tracker","description":null}],
 "relationships":[]}"#;

fn seeded() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("temp dir");
    common::init_db(&dir);
    dir
}

/// Runs `remember` with the given args and a graph payload on stdin.
fn remember(dir: &tempfile::TempDir, args: &[&str], stdin: &str) -> (i32, Value) {
    let output = common::cmd(dir)
        .arg("remember")
        .args(args)
        .arg("--graph-stdin")
        .write_stdin(stdin.to_string())
        .output()
        .expect("spawn remember");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    // A refusal prints its envelope on stdout; the trailing tracing line goes to
    // stderr. Parse the first JSON line either way.
    let line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or_else(|| panic!("no JSON on stdout.\nstdout: {stdout}\nstderr: {stderr}"));
    (
        output.status.code().unwrap_or(-1),
        serde_json::from_str(line).expect("stdout line is JSON"),
    )
}

#[test]
fn strict_entity_types_refuses_a_fold_and_names_it() {
    let dir = seeded();
    let (code, envelope) = remember(
        &dir,
        &[
            "--name",
            "alice-strict",
            "--type",
            "note",
            "--description",
            "d",
            "--strict-entity-types",
        ],
        THREE_FOLDS,
    );
    assert_ne!(code, 0, "a fold under --strict-entity-types must refuse");
    let message = envelope["message"].as_str().expect("a message");
    for named in ["practice", "artifact", "problem", "concept", "file"] {
        assert!(
            message.contains(named),
            "the refusal must name {named}: {message}"
        );
    }
    assert!(
        message.contains("--strict-entity-types"),
        "the refusal must name the flag that caused it: {message}"
    );
}

#[test]
fn a_fold_without_the_flag_still_succeeds_and_still_warns() {
    // GAP-SG-47 stays intact: never dropping a node is the right call, and the
    // flag is opt-in precisely so extraction keeps working unchanged.
    let dir = seeded();
    let (code, envelope) = remember(
        &dir,
        &[
            "--name",
            "alice-lenient",
            "--type",
            "note",
            "--description",
            "d",
        ],
        THREE_FOLDS,
    );
    assert_eq!(code, 0, "folding is the default: {envelope}");
    let warnings = envelope["warnings"].as_array().expect("a warnings array");
    assert_eq!(warnings.len(), 3, "one warning per fold: {warnings:?}");
    assert_eq!(envelope["entities_persisted"], 3);
}

#[test]
fn a_label_that_only_needed_normalising_is_not_a_fold() {
    // Warning on `Issue-Tracker` would train the caller to ignore the channel,
    // and refusing it under the strict flag would be a false positive.
    let dir = seeded();
    let (code, envelope) = remember(
        &dir,
        &[
            "--name",
            "alice-canonical",
            "--type",
            "note",
            "--description",
            "d",
            "--strict-entity-types",
        ],
        NO_FOLD,
    );
    assert_eq!(
        code, 0,
        "canonical labels must pass the strict gate: {envelope}"
    );
    assert!(envelope["warnings"]
        .as_array()
        .expect("a warnings array")
        .is_empty());
}

#[test]
fn dry_run_reports_the_folds_and_the_parsed_counts() {
    // The defect: until v1.2.8 this envelope carried four members and answered
    // `warnings: null` to a payload with three folds — the one mode whose whole
    // purpose is to say what would happen was the one that would not say.
    let dir = seeded();
    let (code, envelope) = remember(
        &dir,
        &[
            "--name",
            "alice-dry",
            "--type",
            "note",
            "--description",
            "d",
            "--dry-run",
        ],
        THREE_FOLDS,
    );
    assert_eq!(code, 0, "dry run must not fail: {envelope}");
    assert_eq!(envelope["dry_run"], Value::Bool(true));
    assert_eq!(envelope["planned_action"], "would_create");
    assert_eq!(envelope["entities_parsed"], 3);
    assert_eq!(envelope["relationships_parsed"], 0);
    assert_eq!(
        envelope["warnings"].as_array().map(Vec::len),
        Some(3),
        "the dry run must report every fold the real run would: {envelope}"
    );
}

#[test]
fn dry_run_emits_an_empty_warnings_array_rather_than_omitting_it() {
    // Absent and empty are different answers to "did anything fold?", and a
    // consumer should never have to tell them apart.
    let dir = seeded();
    let (code, envelope) = remember(
        &dir,
        &[
            "--name",
            "alice-dry-clean",
            "--type",
            "note",
            "--description",
            "d",
            "--dry-run",
        ],
        NO_FOLD,
    );
    assert_eq!(code, 0, "dry run must not fail: {envelope}");
    assert_eq!(envelope["warnings"], serde_json::json!([]));
    assert_eq!(envelope["entities_parsed"], 2);
}

#[test]
fn the_dry_run_envelope_satisfies_its_published_schema() {
    let dir = seeded();
    let (_, envelope) = remember(
        &dir,
        &[
            "--name",
            "alice-dry-schema",
            "--type",
            "note",
            "--description",
            "d",
            "--dry-run",
        ],
        THREE_FOLDS,
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/schemas/remember-dry-run.schema.json");
    let text = std::fs::read_to_string(&path).expect("read remember-dry-run schema");
    let schema: Value = serde_json::from_str(&text).expect("parse schema");
    let object = envelope.as_object().expect("an object");
    for key in schema["required"].as_array().expect("required list") {
        let key = key.as_str().expect("required entries are strings");
        assert!(
            object.contains_key(key),
            "dry-run envelope is missing required key {key}: {envelope}"
        );
    }
    // `additionalProperties: false` is only meaningful if nothing extra ships.
    let declared = schema["properties"].as_object().expect("properties");
    for key in object.keys() {
        assert!(
            declared.contains_key(key),
            "dry-run emitted {key}, which its schema does not declare"
        );
    }
}

#[test]
fn the_positional_name_creates_the_memory() {
    let dir = seeded();
    let (code, envelope) = remember(
        &dir,
        &["alice-posicional", "--type", "note", "--description", "d"],
        NO_FOLD,
    );
    assert_eq!(code, 0, "the positional name must work: {envelope}");
    assert_eq!(envelope["name"], "alice-posicional");
}

#[test]
fn the_positional_name_and_the_flag_together_are_refused() {
    // clap owns this one via `conflicts_with`, and the point of the test is that
    // the conflict is DECLARED rather than resolved by silent precedence.
    let dir = seeded();
    let output = common::cmd(&dir)
        .args([
            "remember",
            "alice-posicional",
            "--name",
            "alice-flag",
            "--type",
            "note",
            "--description",
            "d",
            "--body",
            "corpo sintético suficiente",
        ])
        .output()
        .expect("spawn remember");
    assert_eq!(output.status.code(), Some(2), "a conflict is a usage error");
}

#[test]
fn neither_the_positional_nor_the_flag_is_refused_by_name() {
    // The refusal that clap cannot make, because neither arg is required alone.
    let dir = seeded();
    let output = common::cmd(&dir)
        .args([
            "remember",
            "--type",
            "note",
            "--description",
            "d",
            "--body",
            "corpo sintético suficiente",
        ])
        .output()
        .expect("spawn remember");
    assert_ne!(output.status.code(), Some(0), "a nameless write must fail");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("--name"),
        "the refusal must name the flag: {combined}"
    );
}

#[test]
fn remember_batch_reports_a_fold_per_line() {
    // The visibility channel added in v1.2.8 was wired only into `remember`,
    // so the batch folded in total silence through the same `NewEntity`.
    let dir = seeded();
    let line = r#"{"name":"alice-batch","type":"note","description":"d","body":"corpo sintético suficiente para existir","entities":[{"name":"alice-praxis","entity_type":"practice","description":null}],"relationships":[]}"#;
    let output = common::cmd(&dir)
        .args(["remember-batch", "--json"])
        .write_stdin(format!("{line}\n"))
        .output()
        .expect("spawn remember-batch");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let events: Vec<Value> = stdout
        .lines()
        .filter(|l| l.trim_start().starts_with('{'))
        .map(|l| serde_json::from_str(l).expect("NDJSON line"))
        .collect();
    let item = events
        .iter()
        .find(|e| e.get("index").is_some())
        .unwrap_or_else(|| panic!("no per-item event: {stdout}"));
    let warnings = item["warnings"]
        .as_array()
        .unwrap_or_else(|| panic!("the batch item must report its fold: {item}"));
    assert_eq!(warnings.len(), 1, "one warning for one fold: {warnings:?}");
    assert!(warnings[0].as_str().expect("a string").contains("practice"));
}

#[test]
fn remember_batch_honours_the_strict_flag() {
    let dir = seeded();
    let line = r#"{"name":"alice-batch-strict","type":"note","description":"d","body":"corpo sintético suficiente para existir","entities":[{"name":"alice-praxis","entity_type":"practice","description":null}],"relationships":[]}"#;
    let output = common::cmd(&dir)
        .args(["remember-batch", "--json", "--strict-entity-types"])
        .write_stdin(format!("{line}\n"))
        .output()
        .expect("spawn remember-batch");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        stdout.contains("strict-entity-types"),
        "the batch refusal must name the flag: {stdout}"
    );
}
