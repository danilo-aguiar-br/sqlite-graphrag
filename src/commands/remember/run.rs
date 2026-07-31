//! Orchestration entry point for the `remember` command.

use super::args::RememberArgs;
use super::graph_input::{normalize_and_validate_graph_input, GraphInput};
use crate::chunking;
use crate::entity_type::EntityType;
use crate::errors::AppError;
use crate::i18n::errors_msg;
use crate::output;
use crate::paths::AppPaths;
use crate::storage::chunks as storage_chunks;
use crate::storage::connection::{ensure_schema, open_rw};
use crate::storage::entities::{self as entities, NewEntity};
use crate::storage::memories::{self as memories, NewMemory};
use crate::storage::versions;

/// Run the `remember` command: validate, embed, persist memory + graph.
pub fn run(
    args: RememberArgs,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<(), AppError> {
    use crate::constants::*;

    let inicio = std::time::Instant::now();
    let _ = args.format;
    tracing::debug!(target: "remember", name = %args.name, "persisting memory");
    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;

    // Capture the original `--name` before normalization so the JSON response can
    // surface `name_was_normalized` + `original_name` (B_4 in v1.0.32). Stored as
    // an owned String because `args.name` is moved into the response below.
    let original_name = args.name.clone();

    // Auto-normalize to kebab-case before validation (P2-H).
    // v1.0.20: also trims hyphens at the boundary (including trailing) to avoid rejection
    // after truncation by a long filename ending in a hyphen.
    let normalized_name = {
        let lower = args.name.to_lowercase().replace(['_', ' '], "-");
        let trimmed = lower.trim_matches('-').to_string();
        if trimmed != args.name {
            tracing::warn!(target: "remember",
                original = %args.name,
                normalized = %trimmed,
                "name auto-normalized to kebab-case"
            );
        }
        trimmed
    };
    let name_was_normalized = normalized_name != original_name;

    // GAP-SG-37: when --strict-name is set, refuse to silently rewrite the name.
    // The operator gets the canonical form so they can re-submit it explicitly.
    if args.strict_name && name_was_normalized {
        return Err(AppError::Validation(
            crate::i18n::validation::strict_name_not_canonical(&original_name, &normalized_name),
        ));
    }

    if normalized_name.is_empty() {
        return Err(AppError::Validation(
            crate::i18n::validation::name_empty_after_normalization(),
        ));
    }
    if normalized_name.len() > MAX_MEMORY_NAME_LEN {
        return Err(AppError::LimitExceeded(
            crate::i18n::validation::name_length(MAX_MEMORY_NAME_LEN),
        ));
    }

    if normalized_name.starts_with("__") {
        return Err(AppError::Validation(
            crate::i18n::validation::reserved_name(),
        ));
    }

    {
        let slug_re = crate::constants::name_slug_regex();
        if !slug_re.is_match(&normalized_name) {
            return Err(AppError::Validation(crate::i18n::validation::name_kebab(
                &normalized_name,
            )));
        }
    }

    if let Some(ref desc) = args.description {
        if desc.len() > MAX_MEMORY_DESCRIPTION_LEN {
            return Err(AppError::Validation(
                crate::i18n::validation::description_exceeds(MAX_MEMORY_DESCRIPTION_LEN),
            ));
        }
    }

    // GAP-SG-30: capture whether the body comes from an explicit source before
    // `args.body` is moved below; --graph-file only adopts the file's body when
    // no explicit body source was supplied.
    let body_explicitly_provided =
        args.body.is_some() || args.body_file.is_some() || args.body_stdin;

    let mut raw_body = if let Some(ref b) = args.body {
        b.clone()
    } else if let Some(ref path) = args.body_file {
        let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                let bytes = std::fs::read(path).map_err(AppError::Io)?;
                tracing::warn!(target: "remember", "body file contains invalid UTF-8; replacing invalid sequences");
                String::from_utf8_lossy(&bytes).into_owned()
            }
            Err(e) => return Err(AppError::Io(e)),
        }
    } else if args.body_stdin || args.graph_stdin {
        crate::stdin_helper::read_stdin_with_timeout(60)?
    } else {
        String::new()
    };

    let mut entities_provided_externally =
        args.entities_file.is_some() || args.relationships_file.is_some();

    let mut graph = GraphInput::default();
    if let Some(ref path) = args.entities_file {
        let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
        // v1.1.1 (P7): boundary validation with context — an invalid
        // entity_type surfaces the FromStr message (13 valid values + hints)
        // as a Validation error instead of a bare Json error (exit 20).
        graph.entities = serde_json::from_str(&content).map_err(|e| {
            AppError::Validation(crate::i18n::validation::invalid_json_in_flag(
                "--entities-file",
                &e,
            ))
        })?;
    }
    if let Some(ref path) = args.relationships_file {
        let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
        graph.relationships = serde_json::from_str(&content).map_err(|e| {
            AppError::Validation(crate::i18n::validation::invalid_json_in_flag(
                "--relationships-file",
                &e,
            ))
        })?;
    }
    if args.graph_stdin {
        graph = serde_json::from_str::<GraphInput>(&raw_body).map_err(|e| {
            AppError::Validation(crate::i18n::validation::invalid_json_payload_on_flag(
                "--graph-stdin",
                &e,
            ))
        })?;
        raw_body = graph.body.take().unwrap_or_default();
    }
    if args.graph_stdin && !graph.entities.is_empty() {
        entities_provided_externally = true;
    }
    // GAP-SG-30: graph from a file, combinable with any body source. Conflicts
    // with --graph-stdin/--entities-file/--relationships-file (enforced by clap),
    // so `graph` is still empty here when --graph-file is set.
    if let Some(ref path) = args.graph_file {
        let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
        let mut gf = serde_json::from_str::<GraphInput>(&content).map_err(|e| {
            AppError::Validation(crate::i18n::validation::invalid_json_in_flag(
                "--graph-file",
                &e,
            ))
        })?;
        graph.entities = gf.entities;
        graph.relationships = gf.relationships;
        if !body_explicitly_provided {
            raw_body = gf.body.take().unwrap_or_default();
        }
        if !graph.entities.is_empty() {
            entities_provided_externally = true;
        }
    }

    if graph.entities.len() > max_entities_per_memory() {
        return Err(AppError::LimitExceeded(errors_msg::entity_limit_exceeded(
            max_entities_per_memory(),
        )));
    }
    let mut relationships_updated = false;
    let rel_cap = max_relationships_per_memory();
    if graph.relationships.len() > rel_cap {
        tracing::warn!(target: "remember",
            count = graph.relationships.len(),
            cap = rel_cap,
            "truncating relationships to cap"
        );
        graph.relationships.truncate(rel_cap);
        relationships_updated = true;
    }
    normalize_and_validate_graph_input(&mut graph)?;

    // v1.1.2 (Gap 2): boundary validation of BOTH payload ceilings — bytes
    // (BodyTooLarge) and estimated tokens (TooManyTokens), exit 6 — reusing
    // the same guard the REST embedding client keeps as defence in depth.
    // The token cap used to fire only deep inside the embedding call.
    crate::memory_guard::check_embedding_input_size(&raw_body)?;

    // v1.0.22 P1: reject empty or whitespace-only body when no external graph is provided.
    // Without this check, empty embeddings would be persisted, breaking recall semantics.
    // GAP-08: skip this guard when --force-merge without --clear-body; the existing body
    // will be preserved from the database, so the effective body will not be empty.
    let body_will_be_preserved = args.force_merge && raw_body.trim().is_empty() && !args.clear_body;
    if !entities_provided_externally
        && graph.entities.is_empty()
        && raw_body.trim().is_empty()
        && !body_will_be_preserved
        && !args.clear_body
    {
        return Err(AppError::Validation(crate::i18n::validation::empty_body()));
    }

    let metadata: serde_json::Value = if let Some(ref m) = args.metadata {
        serde_json::from_str(m)?
    } else if let Some(ref path) = args.metadata_file {
        let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    let mut body_hash = blake3::hash(raw_body.as_bytes()).to_hex().to_string();
    let mut snippet: String = raw_body.chars().take(200).collect();

    let paths = AppPaths::resolve(args.db.as_deref())?;
    paths.ensure_dirs()?;

    // v1.0.20: use .trim().is_empty() to reject bodies that are only whitespace.
    let mut extraction_method: Option<String> = None;
    let mut extracted_urls: Vec<crate::extraction::ExtractedUrl> = Vec::with_capacity(4);
    if args.enable_ner && args.skip_extraction {
        return Err(AppError::Validation(
            crate::i18n::validation::enable_ner_skip_extraction_exclusive(),
        ));
    }
    if args.skip_extraction && !args.enable_ner {
        // v1.0.74: revert to v1.0.45 hidden no-op behavior. The v1.0.67
        // commit (9ddb17b) promoted this to a hard validation error, which
        // broke the "kept as a hidden no-op for backwards compatibility"
        // promise documented in CHANGELOG v1.0.45 and started failing
        // 5+ CI jobs whose E2E tests use this flag to skip the
        // (since-removed) GLiNER-ONNX model download in CI environments.
        tracing::warn!(
            "--skip-extraction is deprecated since v1.0.45 and has no effect (NER is disabled by default); remove this flag to silence the warning"
        );
    }
    if args.enable_ner && graph.entities.is_empty() && !raw_body.trim().is_empty() {
        match crate::extraction::extract_graph_auto(&raw_body, &paths) {
            Ok(extracted) => {
                // v1.0.76: ExtractionResult is URL + entity + elapsed_ms;
                // the LLM ExtractionBackend returns typed relationships
                // separately. The default build is URL-only extraction.
                extraction_method = Some("url-regex".to_string());
                extracted_urls = extracted.urls;
                // Convert ExtractedEntity → NewEntity (no offsets,
                // type defaults to Concept).
                graph.entities = extracted
                    .entities
                    .into_iter()
                    .map(|e| NewEntity {
                        name: e.name,
                        entity_type: crate::entity_type::EntityType::Concept,
                        description: None,
                    })
                    .collect();
                graph.relationships.clear();
                relationships_updated = false;

                if graph.entities.len() > max_entities_per_memory() {
                    graph.entities.truncate(max_entities_per_memory());
                }
                if graph.relationships.len() > max_relationships_per_memory() {
                    relationships_updated = true;
                    graph.relationships.truncate(max_relationships_per_memory());
                }
                normalize_and_validate_graph_input(&mut graph)?;
            }
            Err(e) => {
                tracing::warn!(target: "remember", error = %e, "auto-extraction failed, graceful degradation");
                extraction_method = Some("none:extraction-failed".to_string());
            }
        }
    }

    let mut conn = open_rw(&paths.db)?;
    ensure_schema(&mut conn)?;

    // --dry-run: emit planned action without any DB writes and return.
    if args.dry_run {
        let existing = memories::find_by_name(&conn, &namespace, &normalized_name)?;
        let planned_action = if existing.is_some() && args.force_merge {
            "would_update"
        } else {
            "would_create"
        };
        output::emit_json(&serde_json::json!({
            "dry_run": true,
            "name": normalized_name,
            "namespace": namespace,
            "planned_action": planned_action,
        }))?;
        return Ok(());
    }

    {
        use crate::constants::MAX_NAMESPACES_ACTIVE;
        let active_count: u32 = conn.query_row(
            "SELECT COUNT(DISTINCT namespace) FROM memories WHERE deleted_at IS NULL",
            [],
            |r| r.get::<_, i64>(0).map(|v| v as u32),
        )?;
        let ns_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM memories WHERE namespace = ?1 AND deleted_at IS NULL)",
            rusqlite::params![namespace],
            |r| r.get::<_, i64>(0).map(|v| v > 0),
        )?;
        if !ns_exists && active_count >= MAX_NAMESPACES_ACTIVE {
            return Err(AppError::NamespaceError(format!(
                "active namespace limit of {MAX_NAMESPACES_ACTIVE} reached while trying to create '{namespace}'"
            )));
        }
    }

    // M7: detect soft-deleted memory before the standard duplicate check.
    if let Some((sd_id, true)) =
        memories::find_by_name_any_state(&conn, &namespace, &normalized_name)?
    {
        if args.force_merge {
            memories::clear_deleted_at(&conn, sd_id)?;
        } else {
            return Err(AppError::Duplicate(
                errors_msg::duplicate_memory_soft_deleted(&normalized_name, &namespace),
            ));
        }
    }

    let existing_memory = memories::find_by_name(&conn, &namespace, &normalized_name)?;
    if existing_memory.is_some() && !args.force_merge {
        return Err(AppError::Duplicate(errors_msg::duplicate_memory(
            &normalized_name,
            &namespace,
        )));
    }

    // GAP-10: resolve type and description.
    // For CREATE path (new memory): both are required.
    // For UPDATE path (--force-merge on existing memory): inherit from existing row when omitted.
    let (resolved_type, resolved_description) = if existing_memory.is_none() {
        // CREATE path — both fields are mandatory.
        let t = args.r#type.ok_or_else(|| {
            AppError::Validation(crate::i18n::validation::type_and_description_required())
        })?;
        let d = args.description.clone().ok_or_else(|| {
            AppError::Validation(crate::i18n::validation::type_and_description_required())
        })?;
        (t.as_str().to_string(), d)
    } else {
        // UPDATE path (--force-merge) — inherit missing fields from stored row.
        let existing_row = memories::read_by_name(&conn, &namespace, &normalized_name)?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "memory '{normalized_name}' not found in namespace '{namespace}'"
                ))
            })?;
        let t = args
            .r#type
            .map(|v| v.as_str().to_string())
            .unwrap_or_else(|| existing_row.memory_type.clone());
        let d = args
            .description
            .clone()
            .unwrap_or_else(|| existing_row.description.clone());
        (t, d)
    };

    // GAP-08/GAP-09: protect existing body from accidental destruction during --force-merge.
    // When the caller omits a body (or passes an empty one) without --clear-body, silently
    // preserve the existing body from the database.  This prevents a common scripting mistake
    // where a cron job updates metadata fields and inadvertently wipes the stored content.
    if body_will_be_preserved {
        if let Some(existing_row) = memories::read_by_name(&conn, &namespace, &normalized_name)? {
            if !existing_row.body.is_empty() {
                tracing::debug!(target: "remember",
                    name = %normalized_name,
                    "GAP-08: empty body with --force-merge and no --clear-body; preserving existing body"
                );
                raw_body = existing_row.body;
                body_hash = blake3::hash(raw_body.as_bytes()).to_hex().to_string();
                snippet = raw_body.chars().take(200).collect();
            }
        }
    }

    let duplicate_hash_id = memories::find_by_hash(&conn, &namespace, &body_hash)?;

    output::emit_progress_i18n(
        &format!(
            "Remember stage: validated input; available memory {} MB",
            crate::memory_guard::available_memory_mb()
        ),
        &format!(
            "Stage remember: input validated; available memory {} MB",
            crate::memory_guard::available_memory_mb()
        ),
    );

    let model_max_length = crate::tokenizer::get_model_max_length();
    let total_passage_tokens = crate::tokenizer::count_passage_tokens(&raw_body)?;
    let chunks_info = chunking::split_into_chunks_hierarchical(&raw_body);
    let chunks_created = chunks_info.len();
    // GAP-SG-40: `chunks_persisted` is no longer a pre-commit estimate. It is
    // read back from `memory_chunks` AFTER the transaction commits (see below)
    // so the reported count matches the observable database state. Single-chunk
    // bodies store inline in the memories row and append no chunk rows.

    output::emit_progress_i18n(
        &format!(
            "Remember stage: tokenizer counted {total_passage_tokens} passage tokens (model max {model_max_length}); chunking produced {} chunks; process RSS {} MB",
            chunks_created,
            crate::memory_guard::current_process_memory_mb().unwrap_or(0)
        ),
        &format!(
            "Stage remember: tokenizer counted {total_passage_tokens} passage tokens (model max {model_max_length}); chunking produced {} chunks; process RSS {} MB",
            chunks_created,
            crate::memory_guard::current_process_memory_mb().unwrap_or(0)
        ),
    );

    if chunks_created > crate::constants::REMEMBER_MAX_SAFE_MULTI_CHUNKS {
        return Err(AppError::TooManyChunks {
            chunks: chunks_created,
            limit: crate::constants::REMEMBER_MAX_SAFE_MULTI_CHUNKS,
        });
    }

    let embed_out = super::embed_phase::run_embed_phase(
        &paths,
        &raw_body,
        &chunks_info,
        &graph,
        &args,
        embedding_backend,
        llm_backend,
    )?;
    let embedding = embed_out.embedding;
    let backend_invoked_passage = embed_out.backend_invoked_passage;
    let mut chunk_embeddings_cache = embed_out.chunk_embeddings_cache;
    let graph_entity_embeddings = embed_out.graph_entity_embeddings;
    let _skip_embed = embed_out.skip_embed;

    let body_for_storage = raw_body;
    let memory_type = resolved_type.as_str();
    let new_memory = NewMemory {
        namespace: namespace.clone(),
        name: normalized_name.clone(),
        memory_type: memory_type.to_string(),
        description: resolved_description.clone(),
        body: body_for_storage,
        body_hash: body_hash.clone(),
        session_id: args.session_id.clone(),
        source: "agent".to_string(),
        metadata,
    };

    let mut warnings = Vec::with_capacity(4);
    let mut entities_persisted = 0usize;
    let mut relationships_persisted = 0usize;

    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    let mut skip_reindex = false;
    let (memory_id, action, version) = match existing_memory {
        Some((existing_id, _updated_at, _current_version)) => {
            if let Some(hash_id) = duplicate_hash_id {
                if hash_id != existing_id {
                    warnings.push(format!(
                        "identical body already exists as memory id {hash_id}"
                    ));
                }
            }

            // C1 fix: capture old values for FTS5 sync before update
            let (old_fts_name, old_fts_desc, old_fts_body): (String, String, String) = tx
                .query_row(
                    "SELECT name, description, body FROM memories WHERE id = ?1",
                    rusqlite::params![existing_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?;

            // G15: skip re-indexing when body hash matches (common in --force-merge loops)
            let existing_body_hash: Option<String> = tx
                .query_row(
                    "SELECT body_hash FROM memories WHERE id = ?1",
                    rusqlite::params![existing_id],
                    |r| r.get(0),
                )
                .ok();
            let body_unchanged = existing_body_hash.as_deref() == Some(&body_hash);
            skip_reindex = body_unchanged;
            if !body_unchanged {
                storage_chunks::delete_chunks(&tx, existing_id)?;
            }

            let next_v = versions::next_version(&tx, existing_id)?;
            memories::update(&tx, existing_id, &new_memory, args.expected_updated_at)?;

            // C1 fix: sync FTS5 external-content index after update
            // (trg_fts_au trigger is absent by design due to sqlite-vec conflict)
            memories::sync_fts_after_update(
                &tx,
                existing_id,
                &old_fts_name,
                &old_fts_desc,
                &old_fts_body,
                &normalized_name,
                &resolved_description,
                &new_memory.body,
            )?;

            versions::insert_version(
                &tx,
                existing_id,
                next_v,
                &normalized_name,
                memory_type,
                &resolved_description,
                &new_memory.body,
                &serde_json::to_string(&new_memory.metadata)?,
                None,
                "edit",
            )?;
            if !body_unchanged {
                if let Some(ref emb) = embedding {
                    memories::upsert_vec(
                        &tx,
                        existing_id,
                        &namespace,
                        memory_type,
                        emb,
                        &normalized_name,
                        &snippet,
                    )?;
                }
            }
            (existing_id, "updated".to_string(), next_v)
        }
        None => {
            if let Some(hash_id) = duplicate_hash_id {
                warnings.push(format!(
                    "identical body already exists as memory id {hash_id}"
                ));
            }
            let id = memories::insert(&tx, &new_memory)?;
            versions::insert_version(
                &tx,
                id,
                1,
                &normalized_name,
                memory_type,
                &resolved_description,
                &new_memory.body,
                &serde_json::to_string(&new_memory.metadata)?,
                None,
                "create",
            )?;
            if let Some(ref emb) = embedding {
                memories::upsert_vec(
                    &tx,
                    id,
                    &namespace,
                    memory_type,
                    emb,
                    &normalized_name,
                    &snippet,
                )?;
            }
            (id, "created".to_string(), 1)
        }
    };

    // GAP-SG-51: when --force-merge --replace-graph updates an existing memory,
    // clear its prior entity/relationship bindings BEFORE re-linking the supplied
    // set. With an empty `entities`/`relationships` payload this zeroes the graph
    // for that memory without a `forget`. New bindings (if any) are linked by the
    // block further below.
    if args.replace_graph && action == "updated" {
        let (e_removed, r_removed) = entities::clear_memory_graph_bindings(&tx, memory_id)?;
        if e_removed + r_removed > 0 {
            warnings.push(format!(
                "--replace-graph cleared {e_removed} entity binding(s) and {r_removed} relationship binding(s) before re-linking"
            ));
        }
    }

    if chunks_info.len() > 1 && !skip_reindex {
        storage_chunks::insert_chunk_slices(&tx, memory_id, &new_memory.body, &chunks_info)?;

        if let Some(chunk_embeddings) = chunk_embeddings_cache.take() {
            for (i, emb) in chunk_embeddings.iter().enumerate() {
                storage_chunks::upsert_chunk_vec(&tx, i as i64, memory_id, i as i32, emb)?;
            }
        }
        output::emit_progress_i18n(
            &format!(
                "Remember stage: persisted chunk vectors; process RSS {} MB",
                crate::memory_guard::current_process_memory_mb().unwrap_or(0)
            ),
            &format!(
                "Etapa remember: vetores de chunks persistidos; RSS do processo {} MB",
                crate::memory_guard::current_process_memory_mb().unwrap_or(0)
            ),
        );
    }

    if !graph.entities.is_empty() || !graph.relationships.is_empty() {
        for entity in &graph.entities {
            let entity_id = entities::upsert_entity(&tx, &namespace, entity)?;
            let entity_embedding = &graph_entity_embeddings[entities_persisted];
            entities::upsert_entity_vec(
                &tx,
                entity_id,
                &namespace,
                entity.entity_type,
                entity_embedding,
                &entity.name,
            )?;
            entities::link_memory_entity(&tx, memory_id, entity_id)?;
            entities_persisted += 1;
        }
        let entity_types: std::collections::HashMap<&str, EntityType> = graph
            .entities
            .iter()
            .map(|entity| (entity.name.as_str(), entity.entity_type))
            .collect();

        let mut affected_entity_ids: std::collections::HashSet<i64> =
            std::collections::HashSet::new();
        for entity in &graph.entities {
            if let Some(eid) = entities::find_entity_id(&tx, &namespace, &entity.name)? {
                affected_entity_ids.insert(eid);
            }
        }

        for rel in &graph.relationships {
            let source_entity = NewEntity {
                name: rel.source.clone(),
                entity_type: entity_types
                    .get(rel.source.as_str())
                    .copied()
                    .unwrap_or(EntityType::Concept),
                description: None,
            };
            let target_entity = NewEntity {
                name: rel.target.clone(),
                entity_type: entity_types
                    .get(rel.target.as_str())
                    .copied()
                    .unwrap_or(EntityType::Concept),
                description: None,
            };
            let source_id = entities::upsert_entity(&tx, &namespace, &source_entity)?;
            let target_id = entities::upsert_entity(&tx, &namespace, &target_entity)?;
            let rel_id = entities::upsert_relationship(&tx, &namespace, source_id, target_id, rel)?;
            entities::link_memory_relationship(&tx, memory_id, rel_id)?;
            affected_entity_ids.insert(source_id);
            affected_entity_ids.insert(target_id);
            relationships_persisted += 1;
        }

        for &eid in &affected_entity_ids {
            entities::recalculate_degree(&tx, eid)?;
        }
    }
    tx.commit()?;

    super::finish::emit_remember_result(
        &conn,
        &paths,
        &args,
        &graph,
        &new_memory,
        memory_id,
        action,
        version,
        namespace,
        normalized_name,
        original_name,
        name_was_normalized,
        entities_persisted,
        relationships_persisted,
        relationships_updated,
        chunks_created,
        warnings,
        extracted_urls,
        extraction_method,
        backend_invoked_passage,
        inicio,
    )?;

    Ok(())
}
