//! Handler for the `remember-batch` CLI subcommand (G08).
//!
//! Accepts NDJSON via stdin where each line is a memory to persist.
//! One CLI invocation, one slot, one DB connection — eliminates N-process
//! contention from parallel `remember` calls.

use crate::errors::AppError;
use crate::output;
use crate::paths::AppPaths;
use crate::storage::connection::open_rw;
use crate::storage::{entities, memories, versions};
use serde::{Deserialize, Serialize};
use std::io::BufRead;

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Pipe NDJSON memories from stdin\n  \
    echo '{\"name\":\"mem-a\",\"type\":\"note\",\"description\":\"a\",\"body\":\"content\"}' | \
    sqlite-graphrag remember-batch --json\n\n  \
    # Atomic batch with --transaction\n  \
    cat memories.ndjson | sqlite-graphrag remember-batch --transaction --json")]
/// Remember batch args.
pub struct RememberBatchArgs {
    /// Apply all memories in a single transaction (all-or-nothing).
    #[arg(long)]
    pub transaction: bool,
    /// Stop processing on the first failure.
    #[arg(long)]
    pub fail_fast: bool,
    /// Apply force-merge to all memories (update existing by name).
    #[arg(long)]
    pub force_merge: bool,
    /// Validate inputs and emit preview events without persisting or embedding.
    #[arg(long)]
    pub dry_run: bool,
    /// Namespace override for all memories.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Emit NDJSON output.
    #[arg(long)]
    pub json: bool,
    /// GAP-CLI-PRIO-batch: after success, enqueue entity-descriptions for
    /// entities created/linked in this batch (hot-set priority).
    #[arg(long)]
    pub enqueue_enrich: bool,
    /// GAP-SG-216: parity with `remember --strict-entity-types`.
    ///
    /// The batch accepts the same [`crate::storage::entities::NewEntity`]
    /// payload, so it accepts the same open vocabulary — and owes its caller
    /// the same report. The visibility channel added in v1.2.8 was only ever
    /// wired into `remember`, leaving the batch silent.
    #[arg(
        long,
        default_value_t = false,
        help = "Reject a line whose declared entity_type is outside the canonical vocabulary"
    )]
    pub strict_entity_types: bool,
    /// Database path override.
    #[arg(long)]
    pub db: Option<String>,
    /// GAP-SG-35: maximum simultaneous LLM embedding subprocesses, accepted for
    /// parity with `remember`/`edit`/`ingest`/`enrich` so agents that append
    /// `--llm-parallelism` to every invocation never hit a clap error. The
    /// batch loop embeds one passage per item serially; this value bounds the
    /// embedding fan-out width where the backend supports it (clamp [1, 32]).
    #[arg(long, default_value_t = 4, value_name = "N",
          value_parser = clap::value_parser!(u64).range(1..=32))]
    pub llm_parallelism: u64,
}

#[derive(Deserialize)]
struct BatchInputLine {
    name: String,
    #[serde(default = "default_type")]
    r#type: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    entities: Vec<crate::storage::entities::NewEntity>,
    #[serde(default)]
    relationships: Vec<crate::storage::entities::NewRelationship>,
}

fn default_type() -> String {
    "note".to_string()
}

#[derive(Serialize)]
struct BatchItemEvent {
    name: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    index: usize,
    /// GAP-SG-216: declared `entity_type` labels outside the canonical set.
    ///
    /// Omitted when empty, exactly like `error` and `memory_id`, so a line with
    /// nothing to report stays byte-identical to what v1.2.8 emitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct BatchSummary {
    summary: bool,
    total: usize,
    succeeded: usize,
    failed: usize,
    elapsed_ms: u64,
    /// Entity names linked/created across the batch (GAP-CLI-PRIO-batch).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entities_created: Vec<String>,
    /// Recommended enrich ops for automation (GAP-CLI-PRIO-batch).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    enrich_recommended: Vec<String>,
    /// How many entity-descriptions were hot-enqueued when --enqueue-enrich.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    enqueued_entity_descriptions: Option<usize>,
}

/// Run.
pub fn run(args: RememberBatchArgs, backends: crate::cli::BackendChoice) -> Result<(), AppError> {
    let crate::cli::BackendChoice {
        llm: llm_backend,
        embedding: embedding_backend,
    } = backends;
    let start = std::time::Instant::now();
    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let paths = AppPaths::resolve(args.db.as_deref())?;
    paths.ensure_dirs()?;
    crate::storage::connection::ensure_db_ready(&paths)?;
    let mut conn = open_rw(&paths.db)?;

    // Declarative refusal (`--no-input`): fail before touching stdin, even when
    // a pipe is attached and would have supplied NDJSON.
    if crate::stdin_helper::no_input() {
        return Err(AppError::Validation(
            crate::i18n::validation::no_input_blocks_stdin(),
        ));
    }
    let stdin = std::io::stdin();
    let lines: Vec<String> = stdin
        .lock()
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .collect();

    let total = lines.len();
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    if args.dry_run {
        for (idx, line) in lines.iter().enumerate() {
            match serde_json::from_str::<BatchInputLine>(line) {
                Ok(input) => {
                    let normalized_name = crate::parsers::normalize_entity_name(&input.name);
                    if normalized_name.is_empty() {
                        failed += 1;
                        output::emit_json(&BatchItemEvent {
                            name: String::new(),
                            status: "failed".to_string(),
                            memory_id: None,
                            error: Some(format!("line {idx}: name normalizes to empty string")),
                            index: idx,
                            warnings: Vec::new(),
                        })?;
                        continue;
                    }
                    // GAP-SG-216: a dry run that cannot predict the refusal is
                    // not a dry run. The labels are read off the raw line, so
                    // the preview says exactly what the real pass would.
                    let type_warnings =
                        crate::commands::remember::collect_noncanonical_entity_types(line);
                    if args.strict_entity_types && !type_warnings.is_empty() {
                        failed += 1;
                        output::emit_json(&BatchItemEvent {
                            name: normalized_name,
                            status: "would_fail_strict_entity_types".to_string(),
                            memory_id: None,
                            error: Some(crate::i18n::validation::strict_entity_type_folded(
                                &type_warnings,
                            )),
                            index: idx,
                            warnings: type_warnings,
                        })?;
                        continue;
                    }
                    let existing = memories::find_by_name(&conn, &namespace, &normalized_name)?;
                    let action = if existing.is_some() {
                        if args.force_merge {
                            "would_update"
                        } else {
                            "would_fail_duplicate"
                        }
                    } else {
                        "would_create"
                    };
                    succeeded += 1;
                    output::emit_json(&BatchItemEvent {
                        name: normalized_name,
                        status: action.to_string(),
                        memory_id: existing.map(|(id, _, _)| id),
                        error: None,
                        index: idx,
                        warnings: type_warnings,
                    })?;
                }
                Err(e) => {
                    failed += 1;
                    output::emit_json(&BatchItemEvent {
                        name: String::new(),
                        status: "failed".to_string(),
                        memory_id: None,
                        error: Some(format!("line {idx}: invalid JSON: {e}")),
                        index: idx,
                        warnings: Vec::new(),
                    })?;
                }
            }
        }

        output::emit_json(&BatchSummary {
            summary: true,
            total,
            succeeded,
            failed,
            elapsed_ms: start.elapsed().as_millis() as u64,
            entities_created: vec![],
            enrich_recommended: vec![],
            enqueued_entity_descriptions: None,
        })?;
        return Ok(());
    }

    let mut all_entities: Vec<String> = Vec::new();
    if args.transaction {
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for (idx, line) in lines.iter().enumerate() {
            match process_line(
                &tx,
                &namespace,
                line,
                idx,
                args.force_merge,
                &paths,
                crate::cli::BackendChoice::new(llm_backend, embedding_backend),
                args.strict_entity_types,
            ) {
                Ok((event, ent_names)) => {
                    output::emit_json(&event)?;
                    all_entities.extend(ent_names);
                    succeeded += 1;
                }
                Err(e) => {
                    failed += 1;
                    output::emit_json(&BatchItemEvent {
                        name: String::new(),
                        status: "failed".to_string(),
                        memory_id: None,
                        error: Some(format!("{e}")),
                        index: idx,
                        warnings: Vec::new(),
                    })?;
                    if args.fail_fast {
                        break;
                    }
                }
            }
        }
        if failed == 0 || !args.fail_fast {
            tx.commit()?;
        }
    } else {
        for (idx, line) in lines.iter().enumerate() {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            match process_line(
                &tx,
                &namespace,
                line,
                idx,
                args.force_merge,
                &paths,
                crate::cli::BackendChoice::new(llm_backend, embedding_backend),
                args.strict_entity_types,
            ) {
                Ok((event, ent_names)) => {
                    tx.commit()?;
                    output::emit_json(&event)?;
                    all_entities.extend(ent_names);
                    succeeded += 1;
                }
                Err(e) => {
                    drop(tx);
                    failed += 1;
                    output::emit_json(&BatchItemEvent {
                        name: String::new(),
                        status: "failed".to_string(),
                        memory_id: None,
                        error: Some(format!("{e}")),
                        index: idx,
                        warnings: Vec::new(),
                    })?;
                    if args.fail_fast {
                        break;
                    }
                }
            }
        }
    }

    // Dedup entity names for hot-set enqueue (GAP-CLI-PRIO-batch).
    all_entities.sort();
    all_entities.dedup();
    let mut enrich_recommended = Vec::new();
    if !all_entities.is_empty() {
        enrich_recommended.push("entity-descriptions".to_string());
    }
    let mut enqueued_entity_descriptions = None;
    if args.enqueue_enrich && !all_entities.is_empty() {
        match crate::commands::enrich::enqueue_priority_entity_descriptions(
            &paths,
            &namespace,
            &all_entities,
        ) {
            Ok(n) => enqueued_entity_descriptions = Some(n),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "remember-batch: enqueue_enrich failed (entities still listed in entities_created)"
                );
            }
        }
    }

    output::emit_json(&BatchSummary {
        summary: true,
        total,
        succeeded,
        failed,
        elapsed_ms: start.elapsed().as_millis() as u64,
        entities_created: all_entities,
        enrich_recommended,
        enqueued_entity_descriptions,
    })?;

    Ok(())
}

// Still one over after folding the backend pair: the remaining seven are the
// transaction, the line and its index, and the three per-run flags the caller
// resolved from argv — none of which travel together anywhere else.
// One parameter per decision a single NDJSON line depends on. The backend pair
// is already aggregated as `BackendChoice`; what remains does not form a second
// group with a name.
#[allow(clippy::too_many_arguments)]
fn process_line(
    tx: &rusqlite::Transaction<'_>,
    namespace: &str,
    line: &str,
    index: usize,
    force_merge: bool,
    paths: &AppPaths,
    backends: crate::cli::BackendChoice,
    strict_entity_types: bool,
) -> Result<(BatchItemEvent, Vec<String>), AppError> {
    let mut input: BatchInputLine = serde_json::from_str(line).map_err(|e| {
        AppError::Validation(crate::i18n::validation::batch_line_invalid_json(index, &e))
    })?;

    // GAP-SG-216: read the labels off the RAW line, from the same function the
    // `remember` path uses, so one definition of "outside the vocabulary"
    // serves both the warning and the refusal.
    let type_warnings = crate::commands::remember::collect_noncanonical_entity_types(line);
    if strict_entity_types && !type_warnings.is_empty() {
        return Err(AppError::Validation(
            crate::i18n::validation::strict_entity_type_folded(&type_warnings),
        ));
    }

    // Normalise the shape of every label before anything is written, so an
    // unusable one is a validation exit code instead of a stored value.
    for entity in &mut input.entities {
        entity.entity_type = crate::entity_type::normalize_entity_type(&entity.entity_type)?;
    }

    let normalized_name = crate::parsers::normalize_entity_name(&input.name);
    if normalized_name.is_empty() {
        return Err(AppError::Validation(
            crate::i18n::validation::batch_line_name_empty(index),
        ));
    }

    // v1.1.2 (Gap 2): boundary validation of BOTH payload ceilings per NDJSON
    // line — bytes (BodyTooLarge) and estimated tokens (TooManyTokens), exit 6 —
    // so an oversized item fails typed BEFORE any row is written.
    crate::memory_guard::check_embedding_input_size(&input.body)?;

    let body_hash = blake3::hash(input.body.as_bytes()).to_hex().to_string();

    let existing = memories::find_by_name(tx, namespace, &normalized_name)?;

    // GAP-E2E-05: parity with `remember` — description required when creating.
    if existing.is_none() && input.description.trim().is_empty() {
        return Err(AppError::Validation(
            crate::i18n::validation::batch_line_type_description_required(index),
        ));
    }

    let (memory_id, batch_action) = if let Some((existing_id, _updated_at, _version)) = existing {
        if !force_merge {
            // DRY: the other three duplicate call sites already route through
            // this builder; this one had drifted to its own wording.
            return Err(AppError::Duplicate(
                crate::i18n::errors_msg::duplicate_memory(&normalized_name, namespace),
            ));
        }
        let snippet: String = input.body.chars().take(200).collect();
        // Capture old FTS values BEFORE the UPDATE for sync_fts_after_update
        // (trg_fts_au trigger is absent by design due to sqlite-vec conflict).
        let (old_fts_name, old_fts_desc, old_fts_body): (String, String, String) = tx.query_row(
            "SELECT name, description, body FROM memories WHERE id = ?1",
            rusqlite::params![existing_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        memories::update(
            tx,
            existing_id,
            &memories::NewMemory {
                namespace: namespace.to_string(),
                name: normalized_name.clone(),
                memory_type: input.r#type.clone(),
                description: input.description.clone(),
                body: input.body.clone(),
                body_hash,
                session_id: None,
                source: "agent".to_string(),
                metadata: serde_json::json!({}),
            },
            None,
        )?;
        memories::sync_fts_after_update(
            tx,
            existing_id,
            &old_fts_name,
            &old_fts_desc,
            &old_fts_body,
            &normalized_name,
            &input.description,
            &input.body,
        )?;
        let next_v = versions::next_version(tx, existing_id)?;
        versions::insert_version(
            tx,
            existing_id,
            next_v,
            &normalized_name,
            &input.r#type,
            &input.description,
            &input.body,
            "{}",
            None,
            "edit",
        )?;

        let skip_embed = crate::embedder::should_skip_embedding_on_failure();
        match crate::embedder::embed_passage_with_embedding_choice(
            &paths.models,
            &input.body,
            backends,
        ) {
            Ok((embedding, _backend)) => {
                memories::upsert_vec(
                    tx,
                    existing_id,
                    namespace,
                    &input.r#type,
                    &embedding,
                    &normalized_name,
                    &snippet,
                )?;
            }
            // v1.1.2 (Gap 2): typed payload rejections are permanent and
            // must not be swallowed by --skip-embedding-on-failure.
            Err(
                e @ (AppError::Validation(_)
                | AppError::BodyTooLarge { .. }
                | AppError::TooManyTokens { .. }),
            ) => return Err(e),
            Err(e) if skip_embed => {
                tracing::warn!(error = %e, "remember-batch: embedding failed; --skip-embedding-on-failure active, persisting without embedding");
            }
            Err(e) => return Err(e),
        }
        (existing_id, "updated")
    } else {
        let new_mem = memories::NewMemory {
            namespace: namespace.to_string(),
            name: normalized_name.clone(),
            memory_type: input.r#type.clone(),
            description: input.description.clone(),
            body: input.body.clone(),
            body_hash,
            session_id: None,
            source: "agent".to_string(),
            metadata: serde_json::json!({}),
        };
        let id = memories::insert(tx, &new_mem)?;
        versions::insert_version(
            tx,
            id,
            1,
            &normalized_name,
            &input.r#type,
            &input.description,
            &input.body,
            "{}",
            None,
            "create",
        )?;

        let snippet: String = input.body.chars().take(200).collect();
        let skip_embed = crate::embedder::should_skip_embedding_on_failure();
        match crate::embedder::embed_passage_with_embedding_choice(
            &paths.models,
            &input.body,
            backends,
        ) {
            Ok((embedding, _backend)) => {
                memories::upsert_vec(
                    tx,
                    id,
                    namespace,
                    &input.r#type,
                    &embedding,
                    &normalized_name,
                    &snippet,
                )?;
            }
            Err(
                e @ (AppError::Validation(_)
                | AppError::BodyTooLarge { .. }
                | AppError::TooManyTokens { .. }),
            ) => return Err(e),
            Err(e) if skip_embed => {
                tracing::warn!(error = %e, "remember-batch: embedding failed; --skip-embedding-on-failure active, persisting without embedding");
            }
            Err(e) => return Err(e),
        }
        (id, "created")
    };

    // Persist graph entities and relationships if provided
    for entity in &input.entities {
        let entity_id = entities::upsert_entity(tx, namespace, entity)?;
        let entity_text = match &entity.description {
            Some(desc) => format!("{} {}", entity.name, desc),
            None => entity.name.clone(),
        };
        let skip_embed = crate::embedder::should_skip_embedding_on_failure();
        match crate::embedder::embed_entity_texts_cached(
            &paths.models,
            std::slice::from_ref(&entity_text),
            1,
            backends,
        ) {
            Ok((entity_embedding_vec, _stats)) => {
                if let Some(entity_embedding) = entity_embedding_vec.into_iter().next() {
                    entities::upsert_entity_vec(
                        tx,
                        entity_id,
                        namespace,
                        &entity.entity_type,
                        &entity_embedding,
                        &entity.name,
                    )?;
                }
            }
            Err(e) if skip_embed => {
                tracing::warn!(error = %e, "remember-batch: entity embedding failed; --skip-embedding-on-failure active");
            }
            Err(e) => return Err(e),
        }
        entities::link_memory_entity(tx, memory_id, entity_id)?;
    }

    for rel in &input.relationships {
        let src_name = crate::parsers::normalize_entity_name(&rel.source);
        let tgt_name = crate::parsers::normalize_entity_name(&rel.target);
        if let (Some(src_id), Some(tgt_id)) = (
            entities::find_entity_id(tx, namespace, &src_name)?,
            entities::find_entity_id(tx, namespace, &tgt_name)?,
        ) {
            entities::create_or_fetch_relationship(
                tx,
                namespace,
                src_id,
                tgt_id,
                &rel.relation,
                rel.strength,
                rel.description.as_deref(),
            )?;
        }
    }

    let mut created_entity_names: Vec<String> = Vec::with_capacity(input.entities.len());
    for entity in &input.entities {
        created_entity_names.push(crate::parsers::normalize_entity_name(&entity.name));
    }
    Ok((
        BatchItemEvent {
            name: normalized_name,
            status: batch_action.to_string(),
            memory_id: Some(memory_id),
            error: None,
            index,
            warnings: type_warnings,
        },
        created_entity_names,
    ))
}
