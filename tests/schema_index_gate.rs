//! Gate: the schema index and the schema directory agree (GAP-SG-271, v1.2.8).
//!
//! `docs/schemas/README.md` is the INDEX of the published contract catalogue: it
//! is where a reader looks up which document describes a command's envelope. The
//! directory beside it is the catalogue itself. They are two files, so they can
//! disagree, and until this gate they did — silently, in both directions.
//!
//! # What the closing audit of v1.2.8 measured
//!
//! Removing the `pending` family deleted `pending-list.schema.json` and left two
//! table rows pointing at it, one in each language half of the index. A reader
//! following either row reached nothing. `docs_consistency`,
//! `docs_command_coverage` and `docs_declared_facts_gate` all passed over it,
//! because none of them compares the index against the directory — they compare
//! documents against each other and against the binary.
//!
//! # Why the check is only on table rows
//!
//! Prose legitimately NAMES a document in order to say it is gone: the index now
//! records that `pending-list.schema.json` was removed with the family. A rule
//! reading every backticked filename would fail on the very sentence written to
//! prevent the confusion. A table row is different — it is a live mapping,
//! offered as a place to go, so it must resolve.
//!
//! The reverse direction is checked too, and matters more over time: a schema
//! added to the directory but never indexed is a contract nobody can find.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The index document, and the directory it indexes.
const INDEX: &str = "docs/schemas/README.md";
const DIR: &str = "docs/schemas";

/// Schema filenames cited inside a Markdown TABLE ROW of the index.
///
/// A row is any line whose first non-space character is a pipe. Header and
/// separator rows carry no `.schema.json` token, so they fall out on their own.
fn indexed_schemas(markdown: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for line in markdown.lines() {
        if !line.trim_start().starts_with('|') {
            continue;
        }
        for token in line.split('`') {
            if token.ends_with(".schema.json") && !token.contains('/') && !token.contains(' ') {
                found.insert(token.to_string());
            }
        }
    }
    found
}

/// Every `*.schema.json` file actually present in the catalogue directory.
fn schemas_on_disk(dir: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path: PathBuf = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with(".schema.json") {
                found.insert(name.to_string());
            }
        }
    }
    found
}

#[test]
fn every_indexed_schema_exists_on_disk() {
    let markdown = std::fs::read_to_string(INDEX).expect("read the schema index");
    let indexed = indexed_schemas(&markdown);
    let on_disk = schemas_on_disk(Path::new(DIR));

    let dangling: Vec<_> = indexed.difference(&on_disk).cloned().collect();
    assert!(
        dangling.is_empty(),
        "{INDEX} has table row(s) pointing at schema file(s) that do not exist in {DIR}: {dangling:?}\n\
         A row is a live mapping and must resolve. To record that a document was removed, \
         say so in PROSE — prose is not checked, precisely so the removal can be documented."
    );
}

#[test]
fn every_schema_on_disk_is_indexed() {
    let markdown = std::fs::read_to_string(INDEX).expect("read the schema index");
    let indexed = indexed_schemas(&markdown);
    let on_disk = schemas_on_disk(Path::new(DIR));

    let unlisted: Vec<_> = on_disk.difference(&indexed).cloned().collect();
    assert!(
        unlisted.is_empty(),
        "{DIR} contains schema file(s) with no table row in {INDEX}: {unlisted:?}\n\
         A contract nobody can find in the index is a contract an agent will not read."
    );
}

#[test]
fn the_index_is_actually_being_read() {
    // A gate that parsed nothing would pass both assertions above by comparing
    // two empty sets, which is exactly how this class of check goes blind.
    let markdown = std::fs::read_to_string(INDEX).expect("read the schema index");
    let indexed = indexed_schemas(&markdown);
    let on_disk = schemas_on_disk(Path::new(DIR));
    assert!(
        indexed.len() > 50,
        "expected the published catalogue in the index, parsed {} entr(ies)",
        indexed.len()
    );
    assert!(
        on_disk.len() > 50,
        "expected the published catalogue on disk, found {} file(s)",
        on_disk.len()
    );
}

#[test]
fn the_row_parser_finds_rows_and_ignores_prose() {
    let sample = "\
# Catalogue

| Command | Schema |
| --- | --- |
| `health` | `health.schema.json` |
| `recall` | `recall.schema.json` |

- `removed-thing.schema.json` was REMOVED in v1.2.8 and nothing points at it
";
    let found = indexed_schemas(sample);
    assert!(
        found.contains("health.schema.json") && found.contains("recall.schema.json"),
        "table rows must be read: {found:?}"
    );
    assert!(
        !found.contains("removed-thing.schema.json"),
        "prose must NOT be read, or a removal note would fail the gate: {found:?}"
    );
    assert_eq!(found.len(), 2, "nothing else may be picked up: {found:?}");
}
