//! Handler for the `reclassify` CLI subcommand (GAP-18).
//!
//! Reclassifies one entity (single mode) or a whole group of entities (batch
//! mode) by updating the `type` column in the `entities` table.
//!
//! Single mode: `--name <entity>` changes the type of one entity.
//! Batch mode: `--from-type <old> --to-type <new> --batch` changes every
//! entity in the namespace that currently has `<old>` as its type.

use crate::entity_type::normalize_entity_type;
use crate::errors::AppError;
use crate::i18n::errors_msg;
use crate::output::{self, OutputFormat};
use crate::paths::AppPaths;
use crate::storage::connection::open_rw;
use crate::storage::entities;
use rusqlite::params;
use serde::Serialize;

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Reclassify a single entity from its current type to 'tool'\n  \
    sqlite-graphrag reclassify --name tokio-runtime --new-type tool\n\n  \
    # Reclassify all 'concept' entities to 'tool' in one shot (batch)\n  \
    sqlite-graphrag reclassify --from-type concept --to-type tool --batch\n\n  \
    # Reclassify in a specific namespace\n  \
    sqlite-graphrag reclassify --name alice --new-type person --namespace my-project\n\n\
NOTE:\n  \
    Single mode requires --name and at least one of --new-type or --description.\n  \
    Batch mode requires --from-type, --to-type and --batch.\n  \
    Providing --name together with --batch is an error.\n  \
    In batch mode, --from-type is counted before the update: a value that\n  \
    matches no entity is refused instead of reported as a zero-row success.\n\n\
RECOMMENDED ENTITY TYPES (the vocabulary is open; any label is accepted):\n  \
    project, tool, person, file, concept, incident, decision,\n  \
    memory, dashboard, issue_tracker, organization, location, date")]
/// Reclassify args.
pub struct ReclassifyArgs {
    /// Entity name as a positional argument. Alternative to `--name`.
    ///
    /// GAP-SG-272: matches the spelling `read` and `related` have always accepted.
    /// It carries the SAME conflict set as `--name`, because a positional that
    /// coexisted with `--batch` would let the caller ask for one entity and get
    /// every entity of a type.
    #[arg(
        value_name = "NAME",
        conflicts_with_all = ["name", "from_type", "batch"],
        help = "Entity name (kebab-case slug); alternative to --name"
    )]
    pub name_positional: Option<String>,
    /// Entity name to reclassify (single mode). Mutually exclusive with --from-type + --batch.
    #[arg(long, conflicts_with_all = ["from_type", "batch"])]
    pub name: Option<String>,
    /// New entity type for single mode. Any label is accepted (v1.2.8); the
    /// canonical thirteen are recommended, not exhaustive.
    #[arg(long, value_name = "TYPE", visible_alias = "entity-type")]
    pub new_type: Option<String>,
    /// New description for the entity (single mode only). Ignored in batch mode.
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// Current entity type to match in batch mode. Requires --to-type and --batch.
    ///
    /// Counted before the update: a label that matches no entity is refused,
    /// never reported as a successful zero-row batch.
    #[arg(long, value_name = "TYPE", requires = "to_type", requires = "batch")]
    pub from_type: Option<String>,
    /// New entity type to assign in batch mode. Requires --from-type and --batch.
    #[arg(long, value_name = "TYPE", requires = "from_type")]
    pub to_type: Option<String>,
    /// Enable batch reclassification (--from-type to --to-type). Requires --from-type and --to-type.
    #[arg(long, default_value_t = false, requires = "from_type")]
    pub batch: bool,
    /// Namespace scope.
    #[arg(long)]
    pub namespace: Option<String>,
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
struct ReclassifyResponse {
    action: String,
    count: usize,
    /// Entities matching `--from-type` counted BEFORE the update (batch mode
    /// only). Emitted so a caller can tell a real batch from one whose
    /// `--from-type` was a typo — the open vocabulary no longer lets clap catch
    /// that for us.
    #[serde(skip_serializing_if = "Option::is_none")]
    matched_targets: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description_updated: Option<bool>,
    namespace: String,
    /// Total execution time in milliseconds from handler start to serialisation.
    elapsed_ms: u64,
}

/// Run.
pub fn run(args: ReclassifyArgs) -> Result<(), AppError> {
    let started = std::time::Instant::now();
    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let paths = AppPaths::resolve(args.db.as_deref())?;

    crate::storage::connection::ensure_db_ready(&paths)?;

    let mut conn = open_rw(&paths.db)?;

    let mut matched_targets: Option<usize> = None;

    let count = if args.batch {
        // Batch mode: --from-type + --to-type + --batch
        let from_type = args.from_type.as_deref().ok_or_else(|| {
            AppError::Validation(crate::i18n::validation::from_type_required_batch())
        })?;
        let to_type = args.to_type.as_deref().ok_or_else(|| {
            AppError::Validation(crate::i18n::validation::to_type_required_batch())
        })?;
        let from_type = normalize_entity_type(from_type)?;
        let to_type = normalize_entity_type(to_type)?;

        // v1.2.8: count the targets BEFORE mutating. While the vocabulary was a
        // closed enum, a typo in --from-type was refused by clap; now it parses
        // and would quietly update zero rows, which reads as success. Failing
        // closed on an empty match restores the refusal at the only layer that
        // can still see it.
        let targets: i64 = conn.query_row(
            "SELECT COUNT(*) FROM entities WHERE type = ?1 AND namespace = ?2",
            params![from_type, namespace],
            |r| r.get(0),
        )?;
        if targets == 0 {
            return Err(AppError::Validation(
                crate::i18n::validation::reclassify_batch_no_targets(&from_type, &namespace),
            ));
        }
        matched_targets = Some(targets as usize);

        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let affected = tx.execute(
            "UPDATE entities SET type = ?1, updated_at = unixepoch()
             WHERE type = ?2 AND namespace = ?3",
            params![to_type, from_type, namespace],
        )?;
        tx.commit()?;
        affected
    } else {
        // Single mode: name (positional or --name) + --new-type
        //
        // GAP-SG-272: `or` cannot mask a conflict here, because clap already
        // refused the invocation that supplied both spellings.
        let entity_name = args
            .name_positional
            .as_deref()
            .or(args.name.as_deref())
            .ok_or_else(|| {
                AppError::Validation(crate::i18n::validation::name_required_single_mode())
            })?;
        if args.new_type.is_none() && args.description.is_none() {
            return Err(AppError::Validation(
                crate::i18n::validation::reclassify_needs_type_or_description(),
            ));
        }

        // Verify entity exists.
        entities::find_entity_id(&conn, &namespace, entity_name)?.ok_or_else(|| {
            AppError::NotFound(errors_msg::entity_not_found(entity_name, &namespace))
        })?;

        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut affected = 0;
        if let Some(ref new_type) = args.new_type {
            let new_type = normalize_entity_type(new_type)?;
            affected = tx.execute(
                "UPDATE entities SET type = ?1, updated_at = unixepoch()
                 WHERE name = ?2 AND namespace = ?3",
                params![new_type, entity_name, namespace],
            )?;
        }
        if let Some(ref desc) = args.description {
            let rows = tx.execute(
                "UPDATE entities SET description = ?1, updated_at = unixepoch()
                 WHERE name = ?2 AND namespace = ?3",
                params![desc, entity_name, namespace],
            )?;
            if affected == 0 {
                affected = rows;
            }
        }
        tx.commit()?;
        affected
    };

    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;

    let response = ReclassifyResponse {
        action: "reclassified".to_string(),
        count,
        matched_targets,
        description_updated: if args.description.is_some() {
            Some(true)
        } else {
            None
        },
        namespace: namespace.clone(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    };

    match args.format {
        OutputFormat::Json => output::emit_json(&response)?,
        OutputFormat::Text | OutputFormat::Markdown => {
            output::emit_text(&format!(
                "reclassified: {} entities [{}]",
                response.count, response.namespace
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(clap::Parser)]
    struct TestCli {
        #[command(flatten)]
        args: ReclassifyArgs,
    }

    #[test]
    fn entity_type_flag_is_a_visible_alias_of_new_type() {
        // G47: llms-full.txt and external docs promise `--entity-type`; the
        // flag was only reachable as --new-type, breaking the documented CLI.
        use clap::Parser;
        let cli = TestCli::try_parse_from(["reclassify", "--name", "e", "--entity-type", "tool"])
            .expect("--entity-type must parse as an alias of --new-type");
        assert!(cli.args.new_type.is_some());
    }

    #[test]
    fn reclassify_response_serializes_all_fields() {
        let resp = ReclassifyResponse {
            action: "reclassified".to_string(),
            count: 5,
            matched_targets: None,
            description_updated: None,
            namespace: "global".to_string(),
            elapsed_ms: 12,
        };
        let json = serde_json::to_value(&resp).expect("serialization failed");
        assert_eq!(json["action"], "reclassified");
        assert_eq!(json["count"], 5);
        assert_eq!(json["namespace"], "global");
        assert!(json["elapsed_ms"].is_number());
        assert!(json.get("description_updated").is_none());
    }

    #[test]
    fn reclassify_response_count_zero_is_valid() {
        let resp = ReclassifyResponse {
            action: "reclassified".to_string(),
            count: 0,
            matched_targets: None,
            description_updated: None,
            namespace: "my-project".to_string(),
            elapsed_ms: 3,
        };
        let json = serde_json::to_value(&resp).expect("serialization failed");
        assert_eq!(json["count"], 0);
        assert_eq!(json["action"], "reclassified");
    }

    #[test]
    fn reclassify_response_action_is_reclassified() {
        let resp = ReclassifyResponse {
            action: "reclassified".to_string(),
            count: 1,
            matched_targets: None,
            description_updated: None,
            namespace: "ns".to_string(),
            elapsed_ms: 1,
        };
        assert_eq!(resp.action, "reclassified");
    }

    #[test]
    fn reclassify_response_description_updated_present_when_set() {
        let resp = ReclassifyResponse {
            action: "reclassified".to_string(),
            count: 1,
            matched_targets: None,
            description_updated: Some(true),
            namespace: "global".to_string(),
            elapsed_ms: 2,
        };
        let json = serde_json::to_value(&resp).expect("serialization failed");
        assert_eq!(json["description_updated"], true);
    }
}
