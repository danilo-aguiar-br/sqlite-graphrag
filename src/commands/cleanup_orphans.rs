//! Handler for the `cleanup-orphans` CLI subcommand.

use crate::errors::AppError;
use crate::output::{self, OutputFormat};
use crate::paths::AppPaths;
use crate::storage::connection::open_rw;
use crate::storage::entities;
use serde::Serialize;

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Remove orphan entities (no memories, no relationships) from the global namespace\n  \
    sqlite-graphrag cleanup-orphans\n\n  \
    # Preview which entities would be removed without deleting\n  \
    sqlite-graphrag cleanup-orphans --dry-run\n\n  \
    # Cleanup within a specific namespace\n  \
    sqlite-graphrag cleanup-orphans --namespace my-project --yes")]
/// Cleanup orphans args.
pub struct CleanupOrphansArgs {
    /// Namespace scope.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Show what would happen without making changes.
    #[arg(long)]
    pub dry_run: bool,
    /// Yes.
    #[arg(long)]
    pub yes: bool,
    /// Output format.
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
}

#[derive(Serialize)]
struct CleanupResponse {
    orphan_count: usize,
    deleted: usize,
    /// Relationship rows pointing at an entity id that does not exist.
    ///
    /// Reported separately from `orphan_count` because they are the mirror
    /// image of it — edges with no entity, rather than entities with no edges —
    /// and because a caller watching one number would otherwise read a repair
    /// of the other as a no-op.
    dangling_relationship_count: usize,
    dangling_relationships_deleted: usize,
    /// Rows reported by `PRAGMA foreign_key_check` across the whole file,
    /// measured before anything is deleted.
    ///
    /// Wider than `dangling_relationship_count` and overlapping it: eleven
    /// child tables reference `memories`, `entities`, `relationships` or
    /// `memory_chunks`, and a schema rebuild orphans several at once. The
    /// pragma has no notion of namespace, so this number is always whole-file.
    foreign_key_violation_count: usize,
    /// The same measurement taken again after the deletes.
    ///
    /// Reported instead of a "repaired" tally because it answers the question
    /// the operator actually has — is the file still broken — rather than
    /// asserting that the repair worked. Equals the count above under
    /// `--dry-run`, and stays above zero when `--namespace` scoped the run away
    /// from violations living elsewhere in the file.
    foreign_key_violations_remaining: usize,
    dry_run: bool,
    namespace: Option<String>,
    /// Total execution time in milliseconds from handler start to serialisation.
    elapsed_ms: u64,
}

/// Run.
pub fn run(args: CleanupOrphansArgs) -> Result<(), AppError> {
    let started = std::time::Instant::now();
    let paths = AppPaths::resolve(args.db.as_deref())?;

    crate::storage::connection::ensure_db_ready(&paths)?;

    let mut conn = open_rw(&paths.db)?;

    let orphan_ids = entities::find_orphan_entity_ids(&conn, args.namespace.as_deref())?;
    let orphan_count = orphan_ids.len();

    // Dangling edges are the state `PRAGMA foreign_key_check` reports on every
    // migration, and until now this command — the one named for orphans — did
    // not touch them, so there was no supported repair for it at all.
    let dangling_ids = entities::find_dangling_relationship_ids(&conn, args.namespace.as_deref())?;
    let dangling_relationship_count = dangling_ids.len();

    // Whole-file, every child table. The migration guard warns about this exact
    // set and names this command as the repair, so the command has to be able
    // to deliver on more than one of the eleven tables involved.
    let violations = crate::storage::foreign_keys::find_foreign_key_violations(&conn)?;
    let foreign_key_violation_count = violations.len();

    let (deleted, dangling_relationships_deleted) = if args.dry_run {
        (0, 0)
    } else {
        let total = orphan_count + dangling_relationship_count + foreign_key_violation_count;
        if total > 0 && !args.yes {
            return Err(AppError::Validation(
                crate::i18n::validation::refuse_delete_orphans_without_yes(total),
            ));
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // Edges first: removing them can leave an entity with no edges, and
        // that entity is only an orphan by this command's own definition after
        // the edge is gone. Doing it in the other order would need a second
        // pass to reach the same state.
        let edges_removed = entities::delete_relationships_by_ids(&tx, &dangling_ids)?;
        let removed = entities::delete_entities_by_ids(&tx, &orphan_ids)?;
        // Last, and only unscoped: the pragma cannot be filtered by namespace,
        // so a namespaced run reports the wider damage without touching rows
        // that belong to projects sharing this file.
        if args.namespace.is_none() {
            // Re-read inside the transaction: the two deletes above already
            // removed part of the set, and deleting a rowid twice would report
            // a repair that did not happen.
            let left = crate::storage::foreign_keys::find_foreign_key_violations(&tx)?;
            crate::storage::foreign_keys::delete_foreign_key_violations(&tx, &left)?;
        }
        tx.commit()?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        (removed, edges_removed)
    };

    // Measured again rather than inferred: a post-condition that reports what
    // the code intended, instead of what the file now holds, verifies nothing.
    //
    // Skipped under --dry-run, where nothing was written and the answer is the
    // first measurement by definition. Re-scanning there would spend a second
    // full pass over every child table to report a number that could only
    // differ because of somebody else's writes — noise attributed to a preview
    // that touched nothing.
    let foreign_key_violations_remaining = if args.dry_run {
        foreign_key_violation_count
    } else {
        crate::storage::foreign_keys::find_foreign_key_violations(&conn)?.len()
    };

    let response = CleanupResponse {
        orphan_count,
        deleted,
        dangling_relationship_count,
        dangling_relationships_deleted,
        foreign_key_violation_count,
        foreign_key_violations_remaining,
        dry_run: args.dry_run,
        namespace: args.namespace.clone(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    };

    match args.format {
        OutputFormat::Json => output::emit_json(&response)?,
        OutputFormat::Text | OutputFormat::Markdown => {
            let ns = response.namespace.as_deref().unwrap_or("<all>");
            output::emit_text(&format!(
                "orphans: {} entities found, {} deleted; {} dangling relationships found, {} deleted; \
                 {} foreign key violations found, {} still remaining (dry_run={}) [{}]",
                response.orphan_count,
                response.deleted,
                response.dangling_relationship_count,
                response.dangling_relationships_deleted,
                response.foreign_key_violation_count,
                response.foreign_key_violations_remaining,
                response.dry_run,
                ns
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_response_serializes_dry_run_true() {
        let resp = CleanupResponse {
            orphan_count: 5,
            deleted: 0,
            dangling_relationship_count: 2,
            dangling_relationships_deleted: 0,
            foreign_key_violation_count: 2,
            // A dry run repairs nothing, so the second measurement must match
            // the first. Anything else would mean the preview mutated the file.
            foreign_key_violations_remaining: 2,
            dry_run: true,
            namespace: Some("global".to_string()),
            elapsed_ms: 12,
        };
        let json = serde_json::to_value(&resp).expect("serialization failed");
        assert_eq!(json["orphan_count"], 5);
        assert_eq!(json["deleted"], 0);
        assert_eq!(json["dry_run"], true);
        assert_eq!(json["namespace"], "global");
        assert!(json["elapsed_ms"].is_number());
    }

    #[test]
    fn cleanup_response_deleted_zero_when_dry_run() {
        let resp = CleanupResponse {
            orphan_count: 10,
            deleted: 0,
            dangling_relationship_count: 0,
            dangling_relationships_deleted: 0,
            foreign_key_violation_count: 0,
            foreign_key_violations_remaining: 0,
            dry_run: true,
            namespace: None,
            elapsed_ms: 5,
        };
        assert_eq!(resp.deleted, 0, "dry_run must keep deleted at 0");
        assert_eq!(resp.orphan_count, 10);
    }

    #[test]
    fn cleanup_response_namespace_none_serializes_null() {
        let resp = CleanupResponse {
            orphan_count: 0,
            deleted: 0,
            dangling_relationship_count: 0,
            dangling_relationships_deleted: 0,
            foreign_key_violation_count: 0,
            foreign_key_violations_remaining: 0,
            dry_run: false,
            namespace: None,
            elapsed_ms: 1,
        };
        let json = serde_json::to_value(&resp).expect("serialization failed");
        assert!(
            json["namespace"].is_null(),
            "namespace None must serialize as null"
        );
    }

    #[test]
    fn cleanup_response_deleted_equals_orphan_count_when_executed() {
        let resp = CleanupResponse {
            orphan_count: 3,
            deleted: 3,
            dangling_relationship_count: 4,
            dangling_relationships_deleted: 4,
            foreign_key_violation_count: 4,
            // An executed repair must end with the file satisfying its own
            // foreign keys; a non-zero here is the signal that it did not.
            foreign_key_violations_remaining: 0,
            dry_run: false,
            namespace: Some("projeto".to_string()),
            elapsed_ms: 20,
        };
        assert_eq!(
            resp.deleted, resp.orphan_count,
            "when running without dry_run, deleted must equal orphan_count"
        );
    }
}
