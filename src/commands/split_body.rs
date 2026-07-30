//! Handler for the `split-body` CLI subcommand (v1.1.03, GAP-V8).
//!
//! Splits memories whose body exceeds a character threshold into N child
//! memories, reusing the lossless section-based chunker
//! [`crate::chunking::split_body_by_sections`]. The original memory is NOT
//! soft-deleted: its `metadata` is annotated with `superseded_by_split` so
//! history and searchability are preserved. Each child memory gets a
//! canonical `replaces` graph relationship pointing back to the original.

use crate::chunking::split_body_by_sections;
use crate::constants::{DEFAULT_RELATION_WEIGHT, MAX_MEMORY_NAME_LEN};
use crate::entity_type::EntityType;
use crate::errors::AppError;
use crate::i18n::errors_msg;
use crate::output::{self, JsonOutputFormat};
use crate::paths::AppPaths;
use crate::storage::connection::open_rw;
use crate::storage::entities::{self, NewEntity};
use crate::storage::{memories, versions};
use rusqlite::params;
use serde::Serialize;

/// Character threshold below which a memory is left untouched.
pub const DEFAULT_SPLIT_THRESHOLD: usize = 25_000;

#[derive(clap::Args)]
#[command(
    about = "Split oversized memories into N child memories at Markdown section boundaries",
    after_long_help = "EXAMPLES:\n  \
        # Split a single memory by name using the default 25k char threshold\n  \
        sqlite-graphrag split-body --name big-doc\n\n  \
        # Preview the split without writing\n  \
        sqlite-graphrag split-body --name big-doc --dry-run\n\n  \
        # Batch-split every oversized memory in the current namespace\n  \
        sqlite-graphrag split-body --batch\n\n  \
        # Batch-split with a custom threshold and namespace\n  \
        sqlite-graphrag split-body --batch --threshold 50000 --namespace my-project\n\n  \
        NOTE:\n  \
            The original memory is NEVER soft-deleted. It is annotated with\n  \
            metadata.superseded_by_split=true and metadata.split_into=[child names]\n  \
            so its history and searchability are preserved. Each child memory\n  \
            gets a canonical `replaces` graph relationship to the original."
)]
/// Split body args.
pub struct SplitBodyArgs {
    /// Memory name to split (single mode). Mutually exclusive with `--batch`.
    #[arg(long, value_name = "NAME", conflicts_with = "batch")]
    pub name: Option<String>,
    /// Batch mode: split every active memory whose `LENGTH(body) > threshold`.
    #[arg(long, default_value_t = false, conflicts_with = "name")]
    pub batch: bool,
    /// Only memories with `LENGTH(body) > threshold` are split. Default 25000 chars.
    #[arg(long, value_name = "N", default_value_t = DEFAULT_SPLIT_THRESHOLD)]
    pub threshold: usize,
    #[arg(
        long,
        help = "Namespace (flag / XDG namespace.default / global)"
    )]
    /// Namespace scope.
    pub namespace: Option<String>,
    /// Preview the split(s) without writing. Emits the planned child names and
    /// body lengths.
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = JsonOutputFormat::Json)]
    pub format: JsonOutputFormat,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
}

#[derive(Serialize)]
struct ChildPlan {
    name: String,
    body_len: usize,
}

#[derive(Serialize)]
struct SplitResult {
    original: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_id: Option<i64>,
    children: Vec<ChildPlan>,
    threshold: usize,
    dry_run: bool,
    /// Total execution time in milliseconds.
    elapsed_ms: u64,
}

#[derive(Serialize)]
struct BatchResult {
    namespace: String,
    threshold: usize,
    dry_run: bool,
    split: Vec<SplitResult>,
    skipped: usize,
    /// Total execution time in milliseconds.
    elapsed_ms: u64,
}

/// Validates the parsed args and dispatches to single or batch mode.
pub fn run(args: SplitBodyArgs) -> Result<(), AppError> {
    let inicio = std::time::Instant::now();
    let _ = args.format;

    if !args.batch && args.name.is_none() {
        return Err(AppError::Validation(
            "either --name <NAME> or --batch is required".to_string(),
        ));
    }
    if args.threshold == 0 {
        return Err(AppError::Validation(
            "--threshold must be greater than zero".to_string(),
        ));
    }

    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let paths = AppPaths::resolve(args.db.as_deref())?;
    crate::storage::connection::ensure_db_ready(&paths)?;

    if args.batch {
        run_batch(&paths.db, &namespace, args.threshold, args.dry_run, inicio)
    } else if let Some(name) = args.name.as_deref() {
        let result = split_single(&paths.db, &namespace, name, args.threshold, args.dry_run)?;
        output::emit_json(&result)?;
        Ok(())
    } else {
        // Unreachable: validated at the top of run().
        Err(AppError::Validation(
            "either --name <NAME> or --batch is required".to_string(),
        ))
    }
}

fn run_batch(
    db_path: &std::path::Path,
    namespace: &str,
    threshold: usize,
    dry_run: bool,
    inicio: std::time::Instant,
) -> Result<(), AppError> {
    let conn = open_rw(db_path)?;
    let mut stmt =
        conn.prepare("SELECT name FROM memories WHERE namespace = ?1 AND deleted_at IS NULL AND LENGTH(body) > ?2")?;
    let mut rows = stmt.query(params![namespace, threshold as i64])?;
    let mut names: Vec<String> = Vec::new();
    while let Some(row) = rows.next()? {
        names.push(row.get::<_, String>(0)?);
    }
    drop(rows);
    drop(stmt);
    drop(conn);

    let mut split: Vec<SplitResult> = Vec::new();
    let mut skipped = 0usize;
    for name in &names {
        match split_single(db_path, namespace, name, threshold, dry_run) {
            Ok(r) => split.push(r),
            Err(AppError::NotFound(_)) => {
                // Race: memory was removed between scan and split. Skip quietly.
                skipped += 1;
            }
            Err(e) => return Err(e),
        }
    }

    output::emit_json(&BatchResult {
        namespace: namespace.to_string(),
        threshold,
        dry_run,
        split,
        skipped,
        elapsed_ms: inicio.elapsed().as_millis() as u64,
    })?;
    Ok(())
}

/// Splits a single memory by name into N child memories.
///
/// Reuses the lossless [`split_body_by_sections`] chunker so concatenating
/// every child body reproduces the original body. The original memory is kept
/// active and its `metadata` is annotated with `superseded_by_split` and the
/// list of child names. Each child is inserted as a new memory (auto-embedded
/// downstream by the remember path or `enrich --operation re-embed`) and linked
/// to the original entity via a canonical `replaces` relationship.
fn split_single(
    db_path: &std::path::Path,
    namespace: &str,
    name: &str,
    threshold: usize,
    dry_run: bool,
) -> Result<SplitResult, AppError> {
    let start = std::time::Instant::now();

    let mut conn = open_rw(db_path)?;
    let row = memories::read_by_name(&conn, namespace, name)?
        .ok_or_else(|| AppError::NotFound(errors_msg::memory_not_found(name, namespace)))?;

    if row.body.len() <= threshold {
        return Ok(SplitResult {
            original: name.to_string(),
            original_id: Some(row.id),
            children: Vec::new(),
            threshold,
            dry_run,
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }

    let partitions = split_body_by_sections(&row.body);
    if partitions.len() < 2 {
        // Body crosses the threshold but the chunker kept it as a single
        // partition (e.g. under byte/chunk/token budgets). Nothing to split.
        return Ok(SplitResult {
            original: name.to_string(),
            original_id: Some(row.id),
            children: Vec::new(),
            threshold,
            dry_run,
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }

    // Plan child names first so we can surface them in dry-run and metadata.
    let child_names: Vec<String> = (0..partitions.len())
        .map(|i| format!("{name}-part-{i}"))
        .collect();

    for child in &child_names {
        validate_child_name(child, name)?;
    }

    let children_plan: Vec<ChildPlan> = partitions
        .iter()
        .zip(child_names.iter())
        .map(|(body, n)| ChildPlan {
            name: n.clone(),
            body_len: body.len(),
        })
        .collect();

    if dry_run {
        return Ok(SplitResult {
            original: name.to_string(),
            original_id: Some(row.id),
            children: children_plan,
            threshold,
            dry_run: true,
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }

    // Parse existing metadata (stored as TEXT JSON) and merge our markers.
    let mut metadata: serde_json::Value =
        serde_json::from_str(&row.metadata).unwrap_or_else(|_| serde_json::json!({}));
    let map = match metadata.as_object_mut() {
        Some(m) => m,
        None => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "metadata for memory '{name}' is not a JSON object"
            )));
        }
    };
    map.insert("superseded_by_split".to_string(), serde_json::json!(true));
    map.insert(
        "split_into".to_string(),
        serde_json::to_value(&child_names)?,
    );
    let metadata_str = serde_json::to_string(&metadata)?;

    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    // Insert each child as a new memory of type `document` (auto-embedded by
    // downstream paths). We do NOT call the embedding pipeline here to keep
    // the split atomic and side-effect-free; re-embed can be run separately.
    for (body_part, child_name) in partitions.iter().zip(child_names.iter()) {
        let child_hash = blake3_hash(body_part);
        let new_memory = memories::NewMemory {
            namespace: namespace.to_string(),
            name: child_name.clone(),
            memory_type: "document".to_string(),
            description: format!("Partição de '{name}' (split-body)"),
            body: body_part.clone(),
            body_hash: child_hash,
            session_id: row.session_id.clone(),
            source: "system".to_string(),
            metadata: serde_json::json!({ "split_from": name }),
        };
        memories::insert(&tx, &new_memory)?;
    }

    // Annotate the original memory metadata (no soft-delete: history preserved).
    let affected = tx.execute(
        "UPDATE memories SET metadata = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![row.id, metadata_str],
    )?;
    if affected == 0 {
        return Err(AppError::Conflict(format!(
            "memory '{name}' was modified concurrently; retry"
        )));
    }

    // Record a version snapshot of the metadata annotation.
    let next_v = versions::next_version(&tx, row.id)?;
    versions::insert_version(
        &tx,
        row.id,
        next_v,
        &row.name,
        &row.memory_type,
        &row.description,
        &row.body,
        &metadata_str,
        None,
        "edit",
    )?;

    // Graph: each child entity `replaces` the original. Auto-create both
    // endpoints so split-body is self-contained (no dependency on prior NER).
    let original_entity_name = crate::parsers::normalize_entity_name(name);
    let original_entity = NewEntity {
        name: original_entity_name.clone(),
        entity_type: EntityType::Memory,
        description: None,
    };
    let original_entity_id = entities::upsert_entity(&tx, namespace, &original_entity)?;

    for child_name in &child_names {
        let child_entity_name = crate::parsers::normalize_entity_name(child_name);
        let child_entity = NewEntity {
            name: child_entity_name.clone(),
            entity_type: EntityType::Memory,
            description: None,
        };
        let child_entity_id = entities::upsert_entity(&tx, namespace, &child_entity)?;
        let (_, was_created) = entities::create_or_fetch_relationship(
            &tx,
            namespace,
            child_entity_id,
            original_entity_id,
            "replaces",
            DEFAULT_RELATION_WEIGHT,
            None,
        )?;
        if was_created {
            entities::recalculate_degree(&tx, child_entity_id)?;
            entities::recalculate_degree(&tx, original_entity_id)?;
        }
    }

    tx.commit()?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;

    Ok(SplitResult {
        original: name.to_string(),
        original_id: Some(row.id),
        children: children_plan,
        threshold,
        dry_run: false,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

fn validate_child_name(child: &str, parent: &str) -> Result<(), AppError> {
    if child.is_empty() || child.len() > MAX_MEMORY_NAME_LEN {
        return Err(AppError::Validation(crate::i18n::validation::child_name_exceeds_max(child, parent, MAX_MEMORY_NAME_LEN)));
    }
    let slug_re = crate::constants::name_slug_regex();
    if !slug_re.is_match(child) {
        return Err(AppError::Validation(crate::i18n::validation::child_name_not_kebab(child)));
    }
    Ok(())
}

/// Computes the BLAKE3 hex digest of `body`, matching the project convention.
fn blake3_hash(body: &str) -> String {
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    hasher.update(body.as_bytes());
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::memories::{insert, NewMemory};
    use rusqlite::Connection;
    use tempfile::TempDir;

    /// `split_single` needs a `db_path` on disk so it can reopen a connection
    /// for the transaction. This helper inserts the oversized memory and returns
    /// the temp dir + db path.
    fn setup_with_memory(name: &str, body_len: usize) -> (TempDir, std::path::PathBuf) {
        crate::storage::connection::register_vec_extension();
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let mut conn = Connection::open(&db_path).unwrap();
        crate::migrations::runner().run(&mut conn).unwrap();
        let body = body_with_headers(body_len);
        insert(
            &conn,
            &NewMemory {
                namespace: "global".to_string(),
                name: name.to_string(),
                memory_type: "document".to_string(),
                description: "desc".to_string(),
                body,
                body_hash: format!("hash-{name}"),
                session_id: None,
                source: "agent".to_string(),
                metadata: serde_json::json!({}),
            },
        )
        .unwrap();
        drop(conn);
        (dir, db_path)
    }

    /// Builds a Markdown body with many ATX sections so the section splitter
    /// produces multiple partitions well above the 80 KiB byte budget.
    fn body_with_headers(total_bytes: usize) -> String {
        let mut body = String::new();
        let mut i = 0;
        while body.len() < total_bytes {
            body.push_str(&format!(
                "# Section {i}\n\n{}\n\n",
                "body content text here. ".repeat(200)
            ));
            i += 1;
        }
        body
    }

    #[test]
    fn split_body_divides_long_memory_into_parts() {
        let (_dir, db_path) = setup_with_memory("big-doc", 100_000);
        let result = split_single(&db_path, "global", "big-doc", 25_000, false).unwrap();
        assert!(
            result.children.len() >= 2,
            "expected 2+ children, got {}",
            result.children.len()
        );
        // Verify each child memory was actually persisted.
        let conn = Connection::open(&db_path).unwrap();
        for child in &result.children {
            let row = crate::storage::memories::read_by_name(&conn, "global", &child.name)
                .unwrap()
                .expect("child memory should exist");
            assert_eq!(row.memory_type, "document");
            assert!(
                row.body.len() <= crate::constants::AUTOSPLIT_PARTITION_MAX_BYTES,
                "child body {} exceeds partition budget",
                row.body.len()
            );
        }
    }

    #[test]
    fn split_body_marks_original_as_superseded() {
        let (_dir, db_path) = setup_with_memory("super-test", 100_000);
        split_single(&db_path, "global", "super-test", 25_000, false).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let row = crate::storage::memories::read_by_name(&conn, "global", "super-test")
            .unwrap()
            .expect("original memory must remain");
        let metadata: serde_json::Value = serde_json::from_str(&row.metadata).unwrap();
        assert_eq!(
            metadata
                .get("superseded_by_split")
                .and_then(|v| v.as_bool()),
            Some(true),
            "metadata.superseded_by_split must be true after split"
        );
        let split_into = metadata
            .get("split_into")
            .and_then(|v| v.as_array())
            .expect("metadata.split_into must be an array");
        assert!(
            split_into.len() >= 2,
            "metadata.split_into must list 2+ children"
        );
    }

    #[test]
    fn split_body_creates_replaces_relations() {
        let (_dir, db_path) = setup_with_memory("rel-source", 100_000);
        let result = split_single(&db_path, "global", "rel-source", 25_000, false).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let original_entity = crate::parsers::normalize_entity_name("rel-source");
        let original_id =
            crate::storage::entities::find_entity_id(&conn, "global", &original_entity)
                .unwrap()
                .expect("original entity must exist");

        let mut count = 0;
        for child in &result.children {
            let child_entity = crate::parsers::normalize_entity_name(&child.name);
            let child_id = crate::storage::entities::find_entity_id(&conn, "global", &child_entity)
                .unwrap()
                .expect("child entity must exist");
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM relationships
                     WHERE source_id = ?1 AND target_id = ?2 AND relation = 'replaces')",
                    rusqlite::params![child_id, original_id],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(
                exists,
                "child '{}' must `replaces` the original",
                child.name
            );
            count += 1;
        }
        assert!(count >= 2, "expected 2+ replaces relations, got {count}");
    }

    #[test]
    fn split_body_preserves_history() {
        let (_dir, db_path) = setup_with_memory("hist-keep", 100_000);
        split_single(&db_path, "global", "hist-keep", 25_000, false).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        // read_by_name filters deleted_at IS NULL: original must remain visible.
        let row = crate::storage::memories::read_by_name(&conn, "global", "hist-keep")
            .unwrap()
            .expect("original must remain active (not soft-deleted)");
        assert!(row.deleted_at.is_none(), "deleted_at must stay NULL");
    }

    #[test]
    fn dry_run_writes_nothing() {
        let (_dir, db_path) = setup_with_memory("dry-only", 100_000);
        let result = split_single(&db_path, "global", "dry-only", 25_000, true).unwrap();
        assert!(result.dry_run);
        assert!(result.children.len() >= 2);

        let conn = Connection::open(&db_path).unwrap();
        // No child should have been persisted.
        for child in &result.children {
            assert!(
                crate::storage::memories::read_by_name(&conn, "global", &child.name)
                    .unwrap()
                    .is_none(),
                "dry-run must not persist child '{}'",
                child.name
            );
        }
        let row = crate::storage::memories::read_by_name(&conn, "global", "dry-only")
            .unwrap()
            .expect("original must exist");
        let metadata: serde_json::Value = serde_json::from_str(&row.metadata).unwrap();
        assert!(
            metadata.get("superseded_by_split").is_none(),
            "dry-run must not annotate metadata"
        );
    }

    #[test]
    fn below_threshold_returns_empty_children() {
        let (_dir, db_path) = setup_with_memory("small-doc", 1_000);
        let result = split_single(&db_path, "global", "small-doc", 25_000, false).unwrap();
        assert!(result.children.is_empty());
    }

    #[test]
    fn validate_child_name_rejects_too_long_parent() {
        // A parent name so long that even "{name}-part-0" exceeds MAX_MEMORY_NAME_LEN.
        let long_parent = "a".repeat(MAX_MEMORY_NAME_LEN);
        let child = format!("{long_parent}-part-0");
        let err = validate_child_name(&child, &long_parent).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
