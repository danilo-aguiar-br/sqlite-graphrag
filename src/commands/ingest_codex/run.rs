//! Main entry point for `ingest --mode codex`.

use super::binary::{find_codex_binary, validate_codex_version};
use super::extract::extract_with_codex;
use super::queue::{collect_matching_files, open_queue_db};
use super::types::*;
use crate::commands::ingest::IngestArgs;
use crate::commands::ingest_claude::ExtractionResult;
use crate::entity_type::EntityType;
use crate::errors::AppError;
use crate::output::emit_json_line as emit_json;
use crate::paths::AppPaths;
use crate::storage::connection::{ensure_db_ready, open_rw};
use crate::storage::entities::{self, NewEntity, NewRelationship};
use crate::storage::memories::{self, NewMemory};
use std::time::Instant;

/// Run `ingest --mode codex` with queue resume and per-file Codex extraction.
pub fn run_codex_ingest(args: &IngestArgs) -> Result<(), AppError> {
    let started = Instant::now();

    if !args.dir.exists() {
        return Err(AppError::Validation(
            crate::i18n::validation::directory_not_found(&args.dir.display().to_string()),
        ));
    }

    // G28-B (v1.0.68) + G30 (v1.0.69): acquire singleton before doing real
    // work so two parallel `ingest --mode codex` invocations cannot co-exist
    // on the same database. Scope includes the database hash so concurrent
    // ingest against different databases is allowed.
    let early_ns = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let early_paths = AppPaths::resolve(args.db.as_deref())?;
    let queue_path = match args.queue_db.as_deref() {
        Some(p) => std::path::PathBuf::from(p),
        None => crate::paths::sidecar_path(&early_paths.db, ".ingest-queue.sqlite"),
    };
    let _singleton = crate::lock::acquire_job_singleton(
        crate::lock::JobType::IngestCodex,
        &early_ns,
        &early_paths.db,
        args.wait_job_singleton,
        args.force_job_singleton,
    )?;

    // Stage 1: Validate binary
    let codex_binary = find_codex_binary(args.codex_binary.as_deref())?;
    let version = validate_codex_version(&codex_binary)?;
    tracing::info!(
        target: "ingest",
        binary = %codex_binary.display(),
        version = %version,
        "Codex CLI binary validated"
    );

    emit_json(&PhaseEvent {
        phase: "validate",
        codex_path: codex_binary.to_str(),
        version: Some(&version),
        dir: None,
        files_total: None,
        files_new: None,
        files_existing: None,
    });

    // Stage 2: Scan files
    let files = collect_matching_files(&args.dir, &args.pattern, args.recursive, args.max_files)?;

    let queue_conn = open_queue_db(&queue_path)?;

    if args.resume {
        let reset = queue_conn
            .execute(
                "UPDATE queue SET status='pending' WHERE status='processing'",
                [],
            )
            .map_err(|e| AppError::Validation(crate::i18n::validation::queue_resume_failed(&e)))?;
        if reset > 0 {
            tracing::info!(target: "ingest", count = reset, "reset stuck processing files to pending");
        }
    }

    if args.retry_failed {
        let count = queue_conn
            .execute(
                "UPDATE queue SET status='pending', attempt=0 WHERE status='failed'",
                [],
            )
            .map_err(|e| {
                AppError::Validation(crate::i18n::validation::queue_retry_failed_reset_failed(&e))
            })?;
        tracing::info!(target: "ingest", count, "retrying failed files");
    }

    if !args.resume && !args.retry_failed {
        queue_conn
            .execute("DELETE FROM queue", [])
            .map_err(|e| AppError::Validation(crate::i18n::validation::queue_clear_failed(&e)))?;
    }

    let mut new_count = 0usize;
    let mut existing_count = 0usize;

    if !args.retry_failed {
        for file in &files {
            let file_str = file.to_string_lossy().into_owned();
            let inserted = queue_conn
                .execute(
                    "INSERT OR IGNORE INTO queue (file_path, status) VALUES (?1, 'pending')",
                    rusqlite::params![file_str],
                )
                .map_err(|e| {
                    AppError::Validation(crate::i18n::validation::queue_insert_failed(&e))
                })?;
            if inserted > 0 {
                new_count += 1;
            } else {
                existing_count += 1;
            }
        }
    }

    emit_json(&PhaseEvent {
        phase: "scan",
        codex_path: None,
        version: None,
        dir: args.dir.to_str(),
        files_total: Some(files.len()),
        files_new: Some(new_count),
        files_existing: Some(existing_count),
    });

    if args.dry_run {
        for (idx, file) in files.iter().enumerate() {
            let (name, _truncated, _orig) =
                crate::commands::ingest::derive_kebab_name(file, args.max_name_length);
            emit_json(&FileEvent {
                file: &file.to_string_lossy(),
                name: &name,
                status: "preview",
                memory_id: None,
                entities: None,
                rels: None,
                cost_usd: None,
                input_tokens: None,
                output_tokens: None,
                elapsed_ms: None,
                error: None,
                index: idx,
                total: files.len(),
            });
        }
        emit_json(&Summary {
            summary: true,
            files_total: files.len(),
            completed: 0,
            failed: 0,
            skipped: 0,
            entities_total: 0,
            rels_total: 0,
            input_tokens_total: 0,
            output_tokens_total: 0,
            elapsed_ms: started.elapsed().as_millis() as u64,
        });
        if !args.keep_queue {
            let _ = std::fs::remove_file(&queue_path);
        }
        return Ok(());
    }

    // Stage 3: Process files
    let paths = AppPaths::resolve(args.db.as_deref())?;
    ensure_db_ready(&paths)?;
    let conn = open_rw(&paths.db)?;
    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let memory_type_str = args.r#type.as_str().to_string();

    // Write schema to temp file once (reused across all files)
    let schema_tempfile = super::extract::write_schema_tempfile()?;
    let schema_path = schema_tempfile.path().to_path_buf();

    let mut completed = 0usize;
    let mut failed = 0usize;
    let skipped_initial: usize = queue_conn
        .query_row("SELECT COUNT(*) FROM queue WHERE status='done'", [], |r| {
            r.get::<_, usize>(0)
        })
        .unwrap_or(0);
    let mut skipped = skipped_initial;
    let mut entities_total = 0usize;
    let mut rels_total = 0usize;
    let mut input_tokens_total = 0u64;
    let mut output_tokens_total = 0u64;
    let total = files.len();

    let mut backoff_secs = args.rate_limit_wait;
    let rate_limit_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3600);

    loop {
        if crate::shutdown_requested() {
            tracing::info!(target: "ingest", "shutdown requested, stopping before next file");
            break;
        }

        let pending: Option<(i64, String)> = queue_conn
            .query_row(
                "UPDATE queue SET status='processing', attempt=attempt+1 \
                 WHERE id = (SELECT id FROM queue WHERE status='pending' ORDER BY id LIMIT 1) \
                 RETURNING id, file_path",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        let (queue_id, file_path) = match pending {
            Some(p) => p,
            None => break,
        };

        let file_started = Instant::now();

        // Reject files that exceed the 10 MB stdin limit
        const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
        if let Ok(meta) = std::fs::metadata(&file_path) {
            if meta.len() > MAX_FILE_SIZE {
                let err_msg = format!("file exceeds 10MB stdin limit ({} bytes)", meta.len());
                let _ = queue_conn.execute(
                    "UPDATE queue SET status='failed', error=?1, done_at=datetime('now') WHERE id=?2",
                    rusqlite::params![err_msg, queue_id],
                );
                let current_index = completed + failed + skipped;
                failed += 1;
                emit_json(&FileEvent {
                    file: &file_path,
                    name: "",
                    status: "failed",
                    memory_id: None,
                    entities: None,
                    rels: None,
                    cost_usd: None,
                    input_tokens: None,
                    output_tokens: None,
                    elapsed_ms: Some(file_started.elapsed().as_millis() as u64),
                    error: Some(&err_msg),
                    index: current_index,
                    total,
                });
                if args.fail_fast {
                    break;
                }
                continue;
            }
        }

        let file_content = match std::fs::read(&file_path) {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("IO error: {e}");
                let _ = queue_conn.execute(
                    "UPDATE queue SET status='failed', error=?1, done_at=datetime('now') WHERE id=?2",
                    rusqlite::params![err_msg, queue_id],
                );
                let current_index = completed + failed + skipped;
                failed += 1;
                emit_json(&FileEvent {
                    file: &file_path,
                    name: "",
                    status: "failed",
                    memory_id: None,
                    entities: None,
                    rels: None,
                    cost_usd: None,
                    input_tokens: None,
                    output_tokens: None,
                    elapsed_ms: Some(file_started.elapsed().as_millis() as u64),
                    error: Some(&err_msg),
                    index: current_index,
                    total,
                });
                if args.fail_fast {
                    break;
                }
                continue;
            }
        };

        // Skip files exceeding body cap BEFORE sending to LLM to avoid wasting tokens
        if file_content.len() > crate::constants::MAX_MEMORY_BODY_LEN {
            let err_msg = format!(
                "file body exceeds {} byte limit ({} bytes) — skipping to avoid wasting LLM tokens",
                crate::constants::MAX_MEMORY_BODY_LEN,
                file_content.len()
            );
            tracing::warn!(target: "ingest", file = %file_path, size = file_content.len(), "body exceeds limit, skipping LLM extraction");
            let _ = queue_conn.execute(
                "UPDATE queue SET status='skipped', error=?1, done_at=datetime('now') WHERE id=?2",
                rusqlite::params![err_msg, queue_id],
            );
            let current_index = completed + failed + skipped;
            skipped += 1;
            emit_json(&FileEvent {
                file: &file_path,
                name: "",
                status: "skipped",
                memory_id: None,
                entities: None,
                rels: None,
                cost_usd: None,
                input_tokens: None,
                output_tokens: None,
                elapsed_ms: Some(file_started.elapsed().as_millis() as u64),
                error: Some(&err_msg),
                index: current_index,
                total,
            });
            continue;
        }

        // Retry once on cold-start failure
        let max_extract_attempts: u32 = 2;
        let mut extraction_result: Option<(ExtractionResult, Option<CodexUsage>)> = None;
        let mut last_extract_err: Option<String> = None;
        let mut last_was_rate_limited = false;

        for attempt in 1..=max_extract_attempts {
            match extract_with_codex(
                &codex_binary,
                &file_content,
                args.codex_model.as_deref(),
                args.codex_timeout,
                &schema_path,
            ) {
                Ok(result) => {
                    extraction_result = Some(result);
                    break;
                }
                Err(ref e) if matches!(e, AppError::RateLimited { .. }) => {
                    last_extract_err = Some(format!("{e}"));
                    last_was_rate_limited = true;
                    break;
                }
                Err(e) => {
                    let msg = format!("{e}");
                    if attempt < max_extract_attempts {
                        let cold_start_delay = 2 * attempt as u64;
                        tracing::warn!(
                            target: "ingest",
                            attempt,
                            delay_secs = cold_start_delay,
                            error = %msg,
                            "codex extraction failed, retrying"
                        );
                        std::thread::sleep(std::time::Duration::from_secs(cold_start_delay));
                    }
                    last_extract_err = Some(msg);
                }
            }
        }

        if let Some((extraction, usage)) = extraction_result {
            backoff_secs = args.rate_limit_wait;

            let in_tok = usage.as_ref().map(|u| u.input_tokens).unwrap_or(0);
            let out_tok = usage.as_ref().map(|u| u.output_tokens).unwrap_or(0);

            let name = &extraction.name;
            let ent_count = extraction.entities.len();
            let rel_count = 0;

            // GAP-SG-47: fold non-canonical labels onto the nearest canonical
            // kind instead of discarding the entity (no silent data loss).
            let new_entities: Vec<NewEntity> = extraction
                .entities
                .iter()
                .map(|e| NewEntity {
                    name: e.name.clone(),
                    entity_type: EntityType::map_to_canonical(&e.entity_type),
                    description: None,
                })
                .collect();

            // GAP-SG-48: rewrite non-canonical relations to canonical instead
            // of normalizing-and-accepting them raw.
            let new_relationships: Vec<NewRelationship> = extraction
                .relationships
                .iter()
                .map(|r| NewRelationship {
                    source: r.source.clone(),
                    target: r.target.clone(),
                    relation: crate::parsers::map_to_canonical_relation(&r.relation),
                    strength: r.strength,
                    description: None,
                })
                .collect();

            let body_str = String::from_utf8(file_content.clone())
                .map_err(|e| AppError::Validation(crate::i18n::validation::file_not_utf8(&e)))?;
            let body_hash = blake3::hash(body_str.as_bytes()).to_hex().to_string();
            let new_memory = NewMemory {
                name: name.clone(),
                namespace: namespace.clone(),
                memory_type: memory_type_str.clone(),
                description: extraction.description.clone(),
                body: body_str.to_string(),
                body_hash,
                session_id: None,
                source: "agent".to_string(),
                metadata: serde_json::Value::Object(serde_json::Map::new()),
            };

            // Deduplication: update existing memory instead of failing on UNIQUE
            let memory_id = match memories::find_by_name_any_state(&conn, &namespace, name)? {
                Some((existing_id, is_deleted)) => {
                    if is_deleted {
                        memories::clear_deleted_at(&conn, existing_id)?;
                    }
                    let (old_name, old_desc, old_body): (String, String, String) = conn.query_row(
                        "SELECT name, COALESCE(description,''), COALESCE(body,'') FROM memories WHERE id=?1",
                        rusqlite::params![existing_id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )?;
                    memories::update(&conn, existing_id, &new_memory, None)?;
                    memories::sync_fts_after_update(
                        &conn,
                        existing_id,
                        &old_name,
                        &old_desc,
                        &old_body,
                        &new_memory.name,
                        &new_memory.description,
                        &new_memory.body,
                    )?;
                    tracing::info!(target: "ingest", name, memory_id = existing_id, "updated existing memory (force-merge)");
                    existing_id
                }
                None => match memories::insert(&conn, &new_memory) {
                    Ok(id) => id,
                    Err(e) => {
                        let err_msg = format!("{e}");
                        let _ = queue_conn.execute(
                            "UPDATE queue SET status='failed', error=?1, done_at=datetime('now') WHERE id=?2",
                            rusqlite::params![err_msg, queue_id],
                        );
                        let current_index = completed + failed + skipped;
                        failed += 1;
                        emit_json(&FileEvent {
                            file: &file_path,
                            name,
                            status: "failed",
                            memory_id: None,
                            entities: None,
                            rels: None,
                            cost_usd: None,
                            input_tokens: Some(in_tok),
                            output_tokens: Some(out_tok),
                            elapsed_ms: Some(file_started.elapsed().as_millis() as u64),
                            error: Some(&err_msg),
                            index: current_index,
                            total,
                        });
                        input_tokens_total += in_tok;
                        output_tokens_total += out_tok;
                        if args.fail_fast {
                            break;
                        }
                        continue;
                    }
                },
            };

            for ent in &new_entities {
                if let Ok(eid) = entities::upsert_entity(&conn, &namespace, ent) {
                    let _ = entities::link_memory_entity(&conn, memory_id, eid);
                }
            }
            for rel in &new_relationships {
                crate::parsers::warn_if_non_canonical(&rel.relation);
                let src_id = entities::find_entity_id(&conn, &namespace, &rel.source);
                let tgt_id = entities::find_entity_id(&conn, &namespace, &rel.target);
                if let (Ok(Some(sid)), Ok(Some(tid))) = (src_id, tgt_id) {
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO relationships (namespace, source_id, target_id, relation, weight) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![namespace, sid, tid, rel.relation, rel.strength],
                    );
                }
            }

            let _ = queue_conn.execute(
                "UPDATE queue SET status='done', name=?1, memory_id=?2, entities=?3, rels=?4, \
                 input_tokens=?5, output_tokens=?6, elapsed_ms=?7, done_at=datetime('now') WHERE id=?8",
                rusqlite::params![
                    name,
                    memory_id,
                    ent_count,
                    rel_count,
                    in_tok,
                    out_tok,
                    file_started.elapsed().as_millis() as i64,
                    queue_id
                ],
            );

            let current_index = completed + failed + skipped;
            completed += 1;
            entities_total += ent_count;
            rels_total += rel_count;
            input_tokens_total += in_tok;
            output_tokens_total += out_tok;

            emit_json(&FileEvent {
                file: &file_path,
                name,
                status: "done",
                memory_id: Some(memory_id),
                entities: Some(ent_count),
                rels: Some(rel_count),
                cost_usd: None,
                input_tokens: Some(in_tok),
                output_tokens: Some(out_tok),
                elapsed_ms: Some(file_started.elapsed().as_millis() as u64),
                error: None,
                index: current_index,
                total,
            });
        } else if let Some(ref err_str) = last_extract_err {
            if last_was_rate_limited {
                if crate::retry::is_kill_switch_active() {
                    tracing::warn!(target: "ingest", "retry.disable is set, skipping rate-limit retry");
                } else if std::time::Instant::now() >= rate_limit_deadline {
                    tracing::error!(target: "ingest", "rate-limit retry deadline (1h) exhausted");
                } else {
                    let half = backoff_secs / 2;
                    let jitter = if half == 0 { 0 } else { fastrand::u64(0..half) };
                    let actual_wait = half + jitter;
                    tracing::warn!(target: "ingest", delay_secs = actual_wait, error_kind = "rate_limited", "Codex rate limited, backing off");
                    let _ = queue_conn.execute(
                        "UPDATE queue SET status='pending' WHERE id=?1",
                        rusqlite::params![queue_id],
                    );
                    std::thread::sleep(std::time::Duration::from_secs(actual_wait));
                    backoff_secs = (backoff_secs * 2).min(900);
                    continue;
                }
            } else {
                let _ = queue_conn.execute(
                    "UPDATE queue SET status='failed', error=?1, done_at=datetime('now') WHERE id=?2",
                    rusqlite::params![err_str, queue_id],
                );
                let current_index = completed + failed + skipped;
                failed += 1;
                emit_json(&FileEvent {
                    file: &file_path,
                    name: "",
                    status: "failed",
                    memory_id: None,
                    entities: None,
                    rels: None,
                    cost_usd: None,
                    input_tokens: None,
                    output_tokens: None,
                    elapsed_ms: Some(file_started.elapsed().as_millis() as u64),
                    error: Some(err_str),
                    index: current_index,
                    total,
                });
                if args.fail_fast {
                    break;
                }
            }
        }
    }

    // WAL checkpoint before summary
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");

    // Stage 4: Summary
    emit_json(&Summary {
        summary: true,
        files_total: total,
        completed,
        failed,
        skipped,
        entities_total,
        rels_total,
        input_tokens_total,
        output_tokens_total,
        elapsed_ms: started.elapsed().as_millis() as u64,
    });

    if !args.keep_queue && failed == 0 {
        let _ = std::fs::remove_file(&queue_path);
    }

    Ok(())
}
