//! CLI surface of the `merge-entities` subcommand.

use crate::output::OutputFormat;

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Merge two source entities into a target\n  \
    sqlite-graphrag merge-entities --names auth,authentication --into auth-service\n\n  \
    # Merge three sources into one target across a namespace\n  \
    sqlite-graphrag merge-entities --names svc-a,svc-b,old-svc --into canonical-service --namespace my-project\n\n  \
    # Merge by ID (unambiguous when homonyms exist across namespaces)\n  \
    sqlite-graphrag merge-entities --ids 12,17 --into-id 3\n\n\
NOTE:\n  \
    --names is a comma-separated list of source entity names.\n  \
    --into is the target entity name and must already exist.\n  \
    --ids / --into-id select entities by ID; IDs are globally unique so they\n  \
    disambiguate homonyms. They conflict with --names / --into respectively\n  \
    and must belong to the resolved namespace.\n  \
    Source entities are deleted after the merge; the target is preserved.\n  \
    Duplicate relationships (same endpoints + relation) are removed automatically.\n  \
    Run `sqlite-graphrag cleanup-orphans` afterwards if sources had no other links.")]
/// Merge entities args.
pub struct MergeEntitiesArgs {
    /// Comma-separated list of source entity names to merge into the target.
    #[arg(
        long,
        value_delimiter = ',',
        value_name = "NAMES",
        required_unless_present = "ids",
        conflicts_with = "ids"
    )]
    pub names: Vec<String>,
    /// v1.1.1 (P5): comma-separated list of source entity IDs. IDs are
    /// globally unique, so they disambiguate homonyms across namespaces.
    /// Conflicts with --names; every ID must belong to the resolved namespace.
    #[arg(long, value_delimiter = ',', value_name = "IDS")]
    pub ids: Vec<i64>,
    /// Target entity name. Must already exist. All source relationships are redirected here.
    #[arg(
        long,
        value_name = "TARGET",
        required_unless_present = "into_id",
        conflicts_with = "into_id"
    )]
    pub into: Option<String>,
    /// v1.1.1 (P5): target entity ID. Unambiguous alternative to --into.
    #[arg(long, value_name = "TARGET_ID")]
    pub into_id: Option<i64>,
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
    /// v1.1.03: allow merging source entities from OTHER namespaces into the
    /// target. Default false preserves same-namespace safety. When true, each
    /// --ids source is resolved by its own row (no namespace filter); target
    /// must still exist in the resolved namespace.
    #[arg(long, default_value_t = false, hide = false)]
    pub cross_namespace: bool,
}
