//! Gate: refinery's runner may only be executed through the helper that
//! disables foreign key enforcement around it.
//!
//! The defect this pins, measured on 2026-08-18 against a populated database:
//! running `V017__open_entity_type_vocabulary.sql` through a bare
//! `runner().run(conn)` upgraded the schema to 17 and left `relationships` at
//! **zero rows**, down from 213 029. Nothing errored; the migration reported
//! success.
//!
//! The chain, each link independently verifiable:
//!
//! 1. `storage::connection::open_rw` applies `apply_connection_pragmas`, which
//!    sets `PRAGMA foreign_keys = ON` (`src/pragmas.rs`).
//! 2. refinery runs every migration inside its own transaction
//!    (`refinery-core::drivers::rusqlite`, and `set_grouped` is never called).
//! 3. SQLite documents `PRAGMA foreign_keys` as "a no-op within a transaction",
//!    so the `PRAGMA foreign_keys = OFF` written at the top of V006, V008,
//!    V009, V010 and V013 never took effect — those lines are decoration.
//! 4. SQLite documents `DROP TABLE` under enforcement as performing an implicit
//!    `DELETE FROM` first, which fires `ON DELETE CASCADE` on every child.
//! 5. `entities` has four such children: `relationships`, `memory_entities`,
//!    `entity_embeddings` and `entity_connect_seen`.
//!
//! Fresh databases never exposed it, because the cascade has nothing to delete
//! while the tables are still empty — which is why the pattern survived nine
//! migrations. Only a populated database pays.
//!
//! Two ways to reintroduce the defect, and this gate closes both: writing the
//! pragma inside a `.sql` file (where it cannot work) and calling the runner
//! directly (bypassing the place where it can).

use std::path::Path;

/// The only module allowed to execute the migration runner.
const RUNNER_OWNER: &str = "src/storage/connection.rs";

/// Byte offset where a file's `#[cfg(test)]` region begins, if any.
///
/// Test setup helpers build their own empty temporary database, where the
/// cascade has nothing to reach, so they are legitimately exempt. Exempting
/// them by filename would need a hand-kept list that goes stale; asking where
/// the test region starts answers the question the list was approximating.
fn test_region_start(text: &str) -> Option<usize> {
    text.find("#[cfg(test)]")
}

/// Whole files that exist only to host tests.
///
/// Two forms, and the second is the one a filename rule keeps missing. A module
/// can be pulled in as `#[cfg(test)] #[path = "entity_test_fixtures.rs"] mod
/// test_fixtures;`, in which case the file itself carries no `#[cfg(test)]` at
/// all — the gate has to read the DECLARATION to know it is test-only. Asking
/// the sources who they include under `cfg(test)` answers that exactly, and
/// unlike a list of name suffixes it cannot fall out of date.
fn is_test_only_file(rel: &str, cfg_test_included: &[String]) -> bool {
    if rel.ends_with("_tests.rs") || rel.ends_with("/tests.rs") {
        return true;
    }
    rel.rsplit('/')
        .next()
        .is_some_and(|file| cfg_test_included.iter().any(|inc| inc == file))
}

/// Every file name pulled in by a `#[cfg(test)]` + `#[path = "..."]` module.
fn collect_cfg_test_included(files: &[std::path::PathBuf]) -> Vec<String> {
    let mut included = Vec::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.trim() != "#[cfg(test)]" {
                continue;
            }
            // The `#[path = "..."]` attribute sits between the cfg and the
            // `mod` item, so look at the next couple of lines only.
            for candidate in lines.iter().skip(idx + 1).take(2) {
                let trimmed = candidate.trim();
                let Some(rest) = trimmed.strip_prefix("#[path = \"") else {
                    continue;
                };
                if let Some(name) = rest.split('"').next() {
                    included.push(name.to_string());
                }
            }
        }
    }
    included
}

fn rust_sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn relative(path: &Path) -> String {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Proves the detector is not vacuous before trusting the two tests below.
#[test]
fn the_gate_reads_real_sources_and_finds_the_owner() {
    let mut files = Vec::new();
    rust_sources(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(),
        &mut files,
    );
    assert!(
        files.len() > 100,
        "expected to scan the whole crate, found only {} files",
        files.len()
    );

    let owner = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(RUNNER_OWNER))
        .expect("runner owner module must exist");
    assert!(
        owner.contains("run_migrations_with_foreign_keys_off"),
        "{RUNNER_OWNER} must define the guarded entry point this gate protects"
    );
    assert!(
        owner.contains("PRAGMA foreign_keys = OFF"),
        "the guarded entry point must actually disable enforcement"
    );
    assert!(
        owner.contains("foreign_key_check"),
        "the guarded entry point must verify integrity after migrating"
    );
}

/// `runner().run(...)` outside the owning module reintroduces the cascade.
#[test]
fn nothing_outside_the_owner_executes_the_migration_runner() {
    let mut files = Vec::new();
    rust_sources(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(),
        &mut files,
    );

    let cfg_test_included = collect_cfg_test_included(&files);
    assert!(
        !cfg_test_included.is_empty(),
        "expected at least one `#[cfg(test)] #[path = ...]` module; the reader is \
         probably broken rather than the crate being free of them"
    );

    let mut offenders = Vec::new();
    for path in &files {
        let rel = relative(path);
        if rel == RUNNER_OWNER || is_test_only_file(&rel, &cfg_test_included) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let test_region = test_region_start(&text);

        // `get_migrations()` only inspects the embedded list and never touches
        // the database, so it is not a way back into the defect.
        let mut offset = 0usize;
        for (idx, line) in text.lines().enumerate() {
            let line_start = offset;
            offset += line.len() + 1;

            if test_region.is_some_and(|start| line_start >= start) {
                continue;
            }
            if !line.contains("migrations::runner()") {
                continue;
            }
            // The call may finish on this line or chain onto the next few.
            let window: String = text[line_start..].chars().take(400).collect();
            if window.contains(".run(") || window.contains(".run_async(") {
                offenders.push(format!("{rel}:{}", idx + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the migration runner must only be executed through \
         `storage::connection::run_migrations_with_foreign_keys_off`, which disables \
         foreign key enforcement around it. Calling it directly re-enables the \
         ON DELETE CASCADE that emptied `relationships` on 2026-08-18. Offenders: {offenders:?}"
    );
}

/// A `PRAGMA foreign_keys` line inside a migration file promises a protection
/// the file cannot deliver, because refinery has already opened a transaction.
#[test]
fn no_migration_file_pretends_to_toggle_foreign_keys() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let entries = std::fs::read_dir(&dir).expect("migrations directory must exist");

    let mut sql_files = 0usize;
    let mut offenders = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "sql") {
            continue;
        }
        sql_files += 1;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            // Comments explaining why the pragma is absent are the point.
            if trimmed.starts_with("--") {
                continue;
            }
            if trimmed.to_ascii_lowercase().contains("pragma foreign_keys") {
                offenders.push(format!("{name}:{}", idx + 1));
            }
        }
    }

    assert!(
        sql_files >= 17,
        "expected at least 17 migrations, scanned {sql_files}"
    );

    // Historical files keep their inert lines: rewriting an applied migration
    // is what GAP-SG-140 is still paying for. New ones must not add more.
    let historical = [
        "V006__memory_body_limit.sql",
        "V008__expand_entity_types.sql",
        "V009__expand_memory_types.sql",
        "V010__open_relation_vocabulary.sql",
        "V013__drop_vec_use_blob_embeddings.sql",
    ];
    let unexpected: Vec<_> = offenders
        .iter()
        .filter(|o| !historical.iter().any(|h| o.starts_with(h)))
        .collect();

    assert!(
        unexpected.is_empty(),
        "a migration file cannot toggle foreign key enforcement: refinery has already \
         opened a transaction, and SQLite documents the pragma as a no-op there. Toggle it \
         in `storage::connection::run_migrations_with_foreign_keys_off` instead. \
         Offenders: {unexpected:?}"
    );
}

/// V017 exists to remove the CHECK; a CHECK on `type` would defeat it.
#[test]
fn v017_leaves_the_entity_type_column_unconstrained() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations")
        .join("V017__open_entity_type_vocabulary.sql");
    let sql = std::fs::read_to_string(&path).expect("V017 must exist");

    assert!(
        sql.contains("type        TEXT    NOT NULL,") || sql.contains("type TEXT NOT NULL,"),
        "V017 must declare `type` as unconstrained TEXT"
    );

    let type_line = sql
        .lines()
        .find(|l| l.trim_start().starts_with("type"))
        .expect("V017 must declare a `type` column");
    assert!(
        !type_line.to_ascii_uppercase().contains("CHECK"),
        "V017 must not reintroduce a CHECK on `type`: {type_line}"
    );

    // The rebuild must restore both indexes that V001 and V005 created.
    assert!(
        sql.contains("idx_entities_ns"),
        "V017 must recreate idx_entities_ns"
    );
    assert!(
        sql.contains("idx_entities_namespace_degree"),
        "V017 must recreate idx_entities_namespace_degree"
    );

    // Explicit column lists on both sides: `SELECT *` silently depends on
    // positional order, which is how V008 left a latent trap behind.
    assert!(
        !sql.contains("SELECT * FROM entities"),
        "V017 must copy with an explicit column list, never `SELECT *`"
    );
}
