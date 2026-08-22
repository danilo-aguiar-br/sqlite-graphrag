//! Handler for the `link` CLI subcommand.

use crate::constants::DEFAULT_RELATION_WEIGHT;
use crate::entity_type::{is_canonical_entity_type, normalize_entity_type, DEFAULT_ENTITY_TYPE};
use crate::errors::AppError;
use crate::i18n::{errors_msg, validation};
use crate::output::{self, OutputFormat};
use crate::paths::AppPaths;
use crate::storage::connection::open_rw;
use crate::storage::entities;
use crate::storage::entities::NewEntity;
use rusqlite::params;
use serde::Serialize;

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Link two existing graph entities (curated via `remember --graph-stdin` or created by `enrich`)\n  \
    sqlite-graphrag link --from oauth-flow --to refresh-tokens --relation related\n\n  \
    # Auto-create entities that don't exist yet\n  \
    sqlite-graphrag link --from concept-a --to concept-b --relation depends-on --create-missing\n\n  \
    # Specify entity type for auto-created entities\n  \
    sqlite-graphrag link --from alice --to acme-corp --relation related --create-missing --entity-type person\n\n  \
    # Use a custom (non-canonical) relation type\n  \
    sqlite-graphrag link --from module-a --to module-b --relation implements --create-missing\n\n  \
    # If the entity does not exist and --create-missing is not set, the command fails with exit 4.\n  \
    # To list current entity names:\n  \
    sqlite-graphrag graph entities | jaq '.entities[].name'\n\n  \
NOTE:\n  \
LOCK WAITING:\n  \
    The root-level --wait-lock SECONDS flag (default 30s) controls how long\n  \
    the link/unlink subcommands wait for the global CLI lock before failing\n  \
    with exit 15. In a cold start (first call in a new namespace), the lock\n  \
    acquisition may exceed the default wait. CI pipelines should pass\n  \
    --wait-lock 60 for headroom. The link command emits a tracing::info!\n  \
    diagnostic when the wait exceeds 5 seconds so operators can correlate\n  \
    cold-start latency with this CLI invocation.\n\n  \
    --from and --to expect ENTITY names (graph nodes), not memory names.\n  \
    Use --from-id / --to-id when you only have numeric entity IDs (v1.1.05).\n  \
    Purely numeric names are rejected so --create-missing cannot spawn ghosts.\n  \
    Memory names are managed via remember/read/edit/forget; entities are curated via\n  \
    remember --graph-stdin, created by enrich, or auto-created via --create-missing.")]
/// Link args.
pub struct LinkArgs {
    /// Source ENTITY name (graph node, not memory). Entities are curated via
    /// `remember --graph-stdin`, created by `enrich`, or auto-created via `--create-missing`.
    /// Use `graph entities` to list available entity names. Also accepts the alias `--name`.
    /// Conflicts with `--from-id`.
    #[arg(
        long,
        alias = "name",
        required_unless_present = "from_id",
        conflicts_with = "from_id"
    )]
    pub from: Option<String>,
    /// Source entity ID (unambiguous alternative to `--from`). Conflicts with `--from`.
    #[arg(long, value_name = "ID")]
    pub from_id: Option<i64>,
    /// Target ENTITY name (graph node, not memory). See `--from` for sourcing entity names.
    /// Conflicts with `--to-id`.
    #[arg(long, required_unless_present = "to_id", conflicts_with = "to_id")]
    pub to: Option<String>,
    /// Target entity ID (unambiguous alternative to `--to`). Conflicts with `--to`.
    #[arg(long, value_name = "ID")]
    pub to_id: Option<i64>,
    /// Relation type between entities. Canonical values: applies-to, uses,
    /// depends-on, causes, fixes, contradicts, supports, follows, related,
    /// mentions, replaces, tracked-in. Any kebab-case or snake_case string
    /// is also accepted as a custom relation.
    #[arg(long, value_parser = crate::parsers::parse_relation, value_name = "RELATION")]
    pub relation: String,
    /// Relationship weight in `[0.0, 1.0]`; defaults to
    /// `DEFAULT_RELATION_WEIGHT` (0.5). Also accepts the alias `--strength`.
    ///
    /// v1.2.8 added the alias because the same property carries two names on
    /// the two wire shapes: `remember --relationships-file` and the extraction
    /// schemas call it `strength`, while graph output and this flag call it
    /// `weight` (`docs/schemas/relationships-input.schema.json` documents the
    /// mapping). A caller who learned the input schema looked for `--strength`,
    /// did not find it, and concluded `link` could not weight an edge at all —
    /// then modelled dozens of edges around a capability that was always here.
    /// One alias is cheaper than that misreading.
    #[arg(long, alias = "strength")]
    pub weight: Option<f64>,
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
    /// Auto-create entities when they do not exist. Created entities default to
    /// type `concept` unless `--entity-type` specifies a different type.
    #[arg(long, default_value_t = false)]
    pub create_missing: bool,
    /// Entity type assigned to auto-created entities (only effective with `--create-missing`).
    ///
    /// The vocabulary is open (v1.2.8): any label is stored as written, after
    /// shape normalisation (trim, lowercase, hyphen to underscore). The
    /// canonical thirteen — concept, dashboard, date, decision, file, incident,
    /// issue_tracker, location, memory, organization, person, project, tool —
    /// are RECOMMENDED, not exhaustive; a non-canonical label is accepted and
    /// reported as a warning in the response envelope. Defaults to
    /// `DEFAULT_ENTITY_TYPE`.
    #[arg(long, default_value = DEFAULT_ENTITY_TYPE, value_name = "TYPE")]
    pub entity_type: String,
    /// Reject non-canonical relation types with exit 1.
    ///
    /// When set, any relation not in the canonical list causes an immediate error.
    /// Canonical values: applies-to, uses, depends-on, causes, fixes, contradicts,
    /// supports, follows, related, mentions, replaces, tracked-in.
    #[arg(
        long,
        default_value_t = false,
        help = "Reject non-canonical relation types with exit 1"
    )]
    pub strict_relations: bool,
}

#[derive(Serialize)]
struct LinkResponse {
    action: String,
    from: String,
    to: String,
    relation: String,
    weight: f64,
    namespace: String,
    /// Total execution time in milliseconds from handler start to serialisation.
    elapsed_ms: u64,
    /// Entity names that were auto-created by `--create-missing`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    created_entities: Vec<String>,
    /// Non-fatal warnings (e.g. non-canonical relation type).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

/// Run.
pub fn run(args: LinkArgs) -> Result<(), AppError> {
    let started = std::time::Instant::now();
    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let paths = AppPaths::resolve(args.db.as_deref())?;

    crate::storage::connection::ensure_db_ready(&paths)?;

    let mut conn = open_rw(&paths.db)?;

    // v1.1.05 Bug 5: resolve by name OR by explicit --from-id / --to-id.
    // Name path still validates BEFORE normalization (BUG-13 ALL_CAPS guard)
    // and rejects purely numeric names so --create-missing cannot spawn ghosts.
    let (norm_from, source_id_pre, from_is_id) = match (args.from_id, args.from.as_ref()) {
        (Some(id), _) => {
            let (name, _) = resolve_entity_name_by_id(&conn, &namespace, id)?;
            (name, Some(id), true)
        }
        (None, Some(from_name)) => {
            if let Err(msg) = crate::storage::entities::validate_entity_name(from_name) {
                return Err(AppError::Validation(msg.to_string()));
            }
            let norm = crate::parsers::normalize_entity_name(from_name);
            (norm, None, false)
        }
        (None, None) => {
            return Err(AppError::Validation(
                "--from or --from-id is required".to_string(),
            ));
        }
    };

    let (norm_to, target_id_pre, to_is_id) = match (args.to_id, args.to.as_ref()) {
        (Some(id), _) => {
            let (name, _) = resolve_entity_name_by_id(&conn, &namespace, id)?;
            (name, Some(id), true)
        }
        (None, Some(to_name)) => {
            if let Err(msg) = crate::storage::entities::validate_entity_name(to_name) {
                return Err(AppError::Validation(msg.to_string()));
            }
            let norm = crate::parsers::normalize_entity_name(to_name);
            (norm, None, false)
        }
        (None, None) => {
            return Err(AppError::Validation(
                "--to or --to-id is required".to_string(),
            ));
        }
    };

    tracing::debug!(
        target: "link",
        from = %norm_from,
        to = %norm_to,
        relation = %args.relation,
        "creating relationship"
    );

    if norm_from == norm_to {
        return Err(AppError::Validation(validation::self_referential_link()));
    }
    if let (Some(a), Some(b)) = (source_id_pre, target_id_pre) {
        if a == b {
            return Err(AppError::Validation(validation::self_referential_link()));
        }
    }

    let weight = args.weight.unwrap_or(DEFAULT_RELATION_WEIGHT);
    if !(0.0..=1.0).contains(&weight) {
        return Err(AppError::Validation(validation::invalid_link_weight(
            weight,
        )));
    }
    if weight >= 0.95 {
        tracing::warn!(target: "link",
            weight = weight,
            "weight >= 0.95 compresses the scoring range; consider using a value below 0.95"
        );
    }
    if weight <= 0.05 {
        tracing::warn!(target: "link",
            weight = weight,
            "weight <= 0.05 may be too weak to influence traversal; consider using a value above 0.05"
        );
    }

    let mut warnings: Vec<String> = Vec::with_capacity(2);
    let is_canonical = crate::parsers::is_canonical_relation(&args.relation);
    if !is_canonical {
        if args.strict_relations {
            return Err(AppError::Validation(validation::non_canonical_relation(
                &args.relation,
                &crate::parsers::CANONICAL_RELATIONS.join(", "),
            )));
        }
        warnings.push(format!("non-canonical relation '{}'", args.relation));
        tracing::warn!(target: "link",
            relation = %args.relation,
            "non-canonical relation accepted; consider using a well-known value"
        );
    }
    let relation_str = &args.relation;

    // v1.2.8: the entity-type vocabulary is open. Shape is still enforced (an
    // unusable label is refused here, before any write); membership is only
    // advisory, so a non-canonical label is accepted with the same warning
    // treatment the relation vocabulary has had since v1.0.49.
    let entity_type = normalize_entity_type(&args.entity_type)?;
    if !is_canonical_entity_type(&entity_type) {
        warnings.push(format!("non-canonical entity type '{entity_type}'"));
        tracing::warn!(target: "link",
            entity_type = %entity_type,
            "non-canonical entity type accepted; consider using a well-known value"
        );
    }

    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    let mut created_entities: Vec<String> = Vec::with_capacity(2);

    if entity_type == "memory" {
        tracing::warn!(target: "link",
            entity_type = "memory",
            "entity_type 'memory' may conflict with memory table semantics; consider using 'concept' or another type"
        );
    }

    // ID path never creates missing entities — IDs must already exist.
    let source_id = if let Some(id) = source_id_pre {
        let _ = from_is_id;
        id
    } else {
        match entities::find_entity_id(&tx, &namespace, &norm_from)? {
            Some(id) => id,
            None if args.create_missing => {
                let new_entity = NewEntity {
                    name: norm_from.clone(),
                    entity_type: entity_type.clone(),
                    description: None,
                };
                created_entities.push(norm_from.clone());
                entities::upsert_entity(&tx, &namespace, &new_entity)?
            }
            None => {
                return Err(AppError::NotFound(errors_msg::entity_not_found(
                    &norm_from, &namespace,
                )));
            }
        }
    };

    let target_id = if let Some(id) = target_id_pre {
        let _ = to_is_id;
        id
    } else {
        match entities::find_entity_id(&tx, &namespace, &norm_to)? {
            Some(id) => id,
            None if args.create_missing => {
                let new_entity = NewEntity {
                    name: norm_to.clone(),
                    entity_type: entity_type.clone(),
                    description: None,
                };
                created_entities.push(norm_to.clone());
                entities::upsert_entity(&tx, &namespace, &new_entity)?
            }
            None => {
                return Err(AppError::NotFound(errors_msg::entity_not_found(
                    &norm_to, &namespace,
                )));
            }
        }
    };

    let (rel_id, was_created) = entities::create_or_fetch_relationship(
        &tx,
        &namespace,
        source_id,
        target_id,
        relation_str,
        weight,
        None,
    )?;

    let actual_weight: f64 = tx.query_row(
        "SELECT weight FROM relationships WHERE id = ?1",
        params![rel_id],
        |r| r.get(0),
    )?;

    if was_created {
        entities::recalculate_degree(&tx, source_id)?;
        entities::recalculate_degree(&tx, target_id)?;
    }
    tx.commit()?;

    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;

    let action = if was_created {
        "created".to_string()
    } else {
        "already_exists".to_string()
    };

    let response = LinkResponse {
        action: action.clone(),
        from: norm_from.clone(),
        to: norm_to.clone(),
        relation: relation_str.to_string(),
        weight: actual_weight,
        namespace: namespace.clone(),
        elapsed_ms: started.elapsed().as_millis() as u64,
        created_entities,
        warnings,
    };

    match args.format {
        OutputFormat::Json => output::emit_json(&response)?,
        OutputFormat::Text | OutputFormat::Markdown => {
            output::emit_text(&format!(
                "{}: {} --[{}]--> {} [{}]",
                action, response.from, response.relation, response.to, response.namespace
            ));
        }
    }

    Ok(())
}

/// Resolve entity id → (name, namespace) enforcing namespace membership.
fn resolve_entity_name_by_id(
    conn: &rusqlite::Connection,
    namespace: &str,
    id: i64,
) -> Result<(String, String), AppError> {
    let mut stmt = conn
        .prepare_cached("SELECT name, namespace FROM entities WHERE id = ?1 AND namespace = ?2")?;
    match stmt.query_row(params![id, namespace], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    }) {
        Ok(row) => Ok(row),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(AppError::NotFound(
            crate::i18n::validation::entity_id_not_found_in_namespace(id, namespace),
        )),
        Err(e) => Err(AppError::Database(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(clap::Parser)]
    struct TestCli {
        #[command(flatten)]
        args: LinkArgs,
    }

    #[test]
    fn clap_accepts_from_id_to_id() {
        use clap::Parser;
        let ok = match TestCli::try_parse_from([
            "t",
            "--from-id",
            "1",
            "--to-id",
            "2",
            "--relation",
            "supports",
        ]) {
            Ok(v) => v,
            Err(e) => panic!("from-id/to-id must parse: {e}"),
        };
        assert_eq!(ok.args.from_id, Some(1));
        assert_eq!(ok.args.to_id, Some(2));
    }

    #[test]
    fn clap_rejects_from_combined_with_from_id() {
        use clap::Parser;
        match TestCli::try_parse_from([
            "t",
            "--from",
            "a",
            "--from-id",
            "1",
            "--to",
            "b",
            "--relation",
            "supports",
        ]) {
            Ok(_) => panic!("expected argument conflict"),
            Err(err) => assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict),
        }
    }

    #[test]
    fn link_rejects_purely_numeric_name_validation() {
        // Unit-level: validate_entity_name rejects digit-only names.
        assert!(crate::storage::entities::validate_entity_name("89975").is_err());
    }

    #[test]
    fn link_response_without_redundant_aliases() {
        // P1-O: source/target fields were removed from the JSON response.
        let resp = LinkResponse {
            action: "created".to_string(),
            from: "entity-a".to_string(),
            to: "entity-b".to_string(),
            relation: "uses".to_string(),
            weight: 1.0,
            namespace: "default".to_string(),
            elapsed_ms: 0,
            created_entities: vec![],
            warnings: vec![],
        };
        let json = serde_json::to_value(&resp).expect("serialization must work");
        assert_eq!(json["from"], "entity-a");
        assert_eq!(json["to"], "entity-b");
        assert!(
            json.get("source").is_none(),
            "field 'source' was removed in P1-O"
        );
        assert!(
            json.get("target").is_none(),
            "field 'target' was removed in P1-O"
        );
    }

    #[test]
    fn link_response_serializes_all_fields() {
        let resp = LinkResponse {
            action: "already_exists".to_string(),
            from: "origin".to_string(),
            to: "destination".to_string(),
            relation: "mentions".to_string(),
            weight: 0.8,
            namespace: "test".to_string(),
            elapsed_ms: 5,
            created_entities: vec![],
            warnings: vec![],
        };
        let json = serde_json::to_value(&resp).expect("serialization must work");
        assert!(json.get("action").is_some());
        assert!(json.get("from").is_some());
        assert!(json.get("to").is_some());
        assert!(json.get("relation").is_some());
        assert!(json.get("weight").is_some());
        assert!(json.get("namespace").is_some());
        assert!(json.get("elapsed_ms").is_some());
    }

    #[test]
    fn link_response_omits_created_entities_when_empty() {
        let resp = LinkResponse {
            action: "created".to_string(),
            from: "a".to_string(),
            to: "b".to_string(),
            relation: "uses".to_string(),
            weight: 1.0,
            namespace: "global".to_string(),
            elapsed_ms: 0,
            created_entities: vec![],
            warnings: vec![],
        };
        let json = serde_json::to_value(&resp).expect("serialization");
        assert!(
            json.get("created_entities").is_none(),
            "empty vec must be omitted"
        );
    }

    #[test]
    fn link_response_includes_created_entities_when_present() {
        let resp = LinkResponse {
            action: "created".to_string(),
            from: "new-a".to_string(),
            to: "new-b".to_string(),
            relation: "depends-on".to_string(),
            weight: 0.5,
            namespace: "test".to_string(),
            elapsed_ms: 1,
            created_entities: vec!["new-a".to_string(), "new-b".to_string()],
            warnings: vec![],
        };
        let json = serde_json::to_value(&resp).expect("serialization");
        let created = json["created_entities"].as_array().expect("must be array");
        assert_eq!(created.len(), 2);
        assert_eq!(created[0], "new-a");
        assert_eq!(created[1], "new-b");
    }

    #[test]
    fn link_response_includes_warnings_when_non_canonical() {
        let resp = LinkResponse {
            action: "created".to_string(),
            from: "a".to_string(),
            to: "b".to_string(),
            relation: "implements".to_string(),
            weight: 0.5,
            namespace: "global".to_string(),
            elapsed_ms: 0,
            created_entities: vec![],
            warnings: vec!["non-canonical relation 'implements'".to_string()],
        };
        let json = serde_json::to_value(&resp).expect("serialization");
        let w = json["warnings"]
            .as_array()
            .expect("warnings must be present");
        assert_eq!(w.len(), 1);
        assert!(w[0].as_str().unwrap().contains("implements"));
    }

    #[test]
    fn link_response_omits_warnings_when_empty() {
        let resp = LinkResponse {
            action: "created".to_string(),
            from: "a".to_string(),
            to: "b".to_string(),
            relation: "uses".to_string(),
            weight: 0.5,
            namespace: "global".to_string(),
            elapsed_ms: 0,
            created_entities: vec![],
            warnings: vec![],
        };
        let json = serde_json::to_value(&resp).expect("serialization");
        assert!(
            json.get("warnings").is_none(),
            "empty warnings must be omitted"
        );
    }
}
