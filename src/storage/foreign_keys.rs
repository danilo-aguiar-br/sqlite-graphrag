//! `PRAGMA foreign_key_check`: measuring it, judging it, and repairing it.
//!
//! Split out of `connection.rs` when that file crossed the 800-line ceiling.
//! The split follows the seam the gate asks for rather than convenience:
//! opening a database and applying pragmas is one responsibility, deciding
//! whether the file satisfies its own foreign keys is another, and the second
//! is the one that grew — a guard, a repair, and the tests that keep the guard
//! from being wider than the action it verifies.

use crate::errors::AppError;
use rusqlite::{params, Connection};
use std::collections::{BTreeMap, BTreeSet};

/// Counts `PRAGMA foreign_key_check` rows grouped by child and parent table.
///
/// Written as a query rather than `execute_batch` on purpose: the pragma
/// reports violations as a *result set*, never as an error, so batching it
/// inside a `.sql` migration — as `V010` does — discards the answer and
/// verifies nothing.
///
/// Grouping by table pair rather than totalling is what lets the comparison
/// survive a migration that deletes one dangling row and creates another: the
/// total would match while a real regression hid inside it.
///
/// # Errors
/// Returns `Err` when the pragma cannot be prepared or read.
pub(crate) fn foreign_key_violation_counts(
    conn: &Connection,
) -> Result<BTreeMap<(String, String), usize>, AppError> {
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let mut rows = stmt.query([])?;
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let child: String = row.get(0)?;
        let parent: String = row.get(2)?;
        *counts.entry((child, parent)).or_default() += 1;
    }
    Ok(counts)
}

/// Fails when a migration left MORE rows orphaned than it found.
///
/// # Errors
/// Returns `Err` naming the first table pair whose violation count grew.
pub(crate) fn assert_migration_orphaned_nothing(
    before: &BTreeMap<(String, String), usize>,
    after: &BTreeMap<(String, String), usize>,
) -> Result<(), AppError> {
    for (pair, after_count) in after {
        let before_count = before.get(pair).copied().unwrap_or(0);
        if *after_count > before_count {
            let (child, parent) = pair;
            return Err(AppError::Internal(anyhow::anyhow!(
                "migration orphaned rows: `{child}` has {after_count} rows with no parent in \
                 `{parent}`, up from {before_count} before the migration ran. The pre-migration \
                 copy of the database is next to it, named `.bak.pre-schema-<version>.<stamp>`."
            )));
        }
    }
    Ok(())
}

/// Warns, without failing, about violations the migration inherited.
///
/// Emitted on stderr through tracing so the JSON contract on stdout is
/// untouched. Naming `cleanup-orphans` matters: the state is repairable, and a
/// warning that does not say how to repair it trains the reader to ignore it.
pub(crate) fn warn_about_pre_existing_violations(after: &BTreeMap<(String, String), usize>) {
    for ((child, parent), count) in after {
        tracing::warn!(
            target: "storage",
            child_table = %child,
            parent_table = %parent,
            rows = *count,
            "pre-existing foreign key violations left untouched by this migration; \
             run `sqlite-graphrag cleanup-orphans --dry-run` to preview the repair"
        );
    }
}

/// Every `PRAGMA foreign_key_check` row, as `(child table, rowid)`.
///
/// Table-agnostic on purpose. The guard above knows about every child table in
/// the schema — eleven of them reference `memories`, `entities`, `relationships`
/// or `memory_chunks` — so a repair that knows about only one leaves the warning
/// pointing at a command that cannot deliver, and goes stale the day a migration
/// adds the twelfth. A rebuild-and-rename of `entities` orphans four tables at
/// once, not just `relationships`, which is exactly how this state is produced.
///
/// A row reported here is unreachable by construction: with enforcement on,
/// SQLite would have cascaded it away when its parent was deleted. Removing it
/// destroys no reachable data.
///
/// # Errors
/// Returns `Err` when the pragma cannot be prepared or read.
pub fn find_foreign_key_violations(conn: &Connection) -> Result<Vec<(String, i64)>, AppError> {
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let child: String = row.get(0)?;
        // NULL only for a WITHOUT ROWID child. This schema declares none, and
        // skipping rather than failing keeps a future one from bricking the
        // repair for every other table.
        if let Some(rowid) = row.get::<_, Option<i64>>(1)? {
            out.push((child, rowid));
        }
    }
    Ok(out)
}

/// Deletes the rows reported by [`find_foreign_key_violations`].
///
/// The table name arrives from the pragma, so it already comes from
/// `sqlite_master`; it is still checked against the live table list before
/// being interpolated, because a name reaching SQL by string concatenation
/// deserves a witness rather than an assumption.
///
/// # Errors
/// Returns `Err` when a delete fails, or when a reported table does not exist.
pub fn delete_foreign_key_violations(
    conn: &Connection,
    violations: &[(String, i64)],
) -> Result<usize, AppError> {
    let known = real_table_names(conn)?;
    let mut removed = 0usize;
    for (table, rowid) in violations {
        if !known.contains(table) {
            return Err(AppError::Internal(anyhow::anyhow!(
                "foreign_key_check named a table `{table}` that does not exist"
            )));
        }
        let quoted = table.replace('"', "\"\"");
        removed += conn.execute(
            &format!("DELETE FROM \"{quoted}\" WHERE rowid = ?1"),
            params![rowid],
        )?;
    }
    Ok(removed)
}

/// Names of the ordinary tables in this database.
fn real_table_names(conn: &Connection) -> Result<BTreeSet<String>, AppError> {
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?;
    let names = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violations(pairs: &[(&str, &str, usize)]) -> BTreeMap<(String, String), usize> {
        pairs
            .iter()
            .map(|(c, p, n)| (((*c).to_string(), (*p).to_string()), *n))
            .collect()
    }

    /// A row that was already dangling is not evidence against this migration.
    ///
    /// This is the case that bricked whole databases: `PRAGMA foreign_key_check`
    /// scans the entire file, `ensure_db_ready` migrates on open, and nearly
    /// every subcommand calls it — so one inherited row refused every command,
    /// including `cleanup-orphans`, which is the repair.
    #[test]
    fn pre_existing_violations_do_not_fail_the_migration() {
        let before = violations(&[("relationships", "entities", 3)]);
        let after = violations(&[("relationships", "entities", 3)]);
        assert!(assert_migration_orphaned_nothing(&before, &after).is_ok());
    }

    /// Fewer than before is a repair, never a regression.
    #[test]
    fn a_migration_that_removes_dangling_rows_passes() {
        let before = violations(&[("relationships", "entities", 5)]);
        let after = violations(&[("relationships", "entities", 1)]);
        assert!(assert_migration_orphaned_nothing(&before, &after).is_ok());
    }

    /// The assertion still has to catch the cascade it was written for.
    #[test]
    fn a_migration_that_orphans_new_rows_still_fails() {
        let before = violations(&[("relationships", "entities", 1)]);
        let after = violations(&[("relationships", "entities", 2)]);
        let err = assert_migration_orphaned_nothing(&before, &after)
            .expect_err("growth must fail the migration");
        let text = err.to_string();
        assert!(text.contains("relationships"), "must name the child table");
        assert!(
            text.contains(".bak.pre-schema-"),
            "must point at the automatic pre-migration copy: {text}"
        );
    }

    /// A table pair that appears only after the migration starts from zero.
    #[test]
    fn violations_in_a_table_untouched_before_are_caught() {
        let before = violations(&[]);
        let after = violations(&[("memory_entities", "entities", 1)]);
        assert!(assert_migration_orphaned_nothing(&before, &after).is_err());
    }

    /// Grouping by table pair is what makes the comparison honest: a migration
    /// that deletes one dangling row and creates another keeps the TOTAL equal
    /// while a real regression hides inside it.
    #[test]
    fn a_swap_that_keeps_the_total_is_still_caught() {
        let before = violations(&[("relationships", "entities", 1)]);
        let after = violations(&[("memory_entities", "entities", 1)]);
        assert!(
            assert_migration_orphaned_nothing(&before, &after).is_err(),
            "equal totals must not hide a new violation in another table"
        );
    }
}
