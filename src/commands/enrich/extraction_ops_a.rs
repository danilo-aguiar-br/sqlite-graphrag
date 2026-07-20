//! Extracted from extraction.rs (Wave C1).

use super::*;
use super::postprocess::{
    persist_enriched_body, persist_entity_description, persist_memory_bindings,
    reembed_memory_vector,
};
use rusqlite::Connection;
use crate::errors::AppError;
use crate::entity_type::EntityType;
use crate::constants::MAX_MEMORY_BODY_LEN;
use crate::storage::entities::{self};
use std::path::{Path, PathBuf};

pub(crate) fn call_memory_bindings(
    conn: &Connection,
    namespace: &str,
    memory_name: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    // GAP-CLI-QISO-04: never treat pair:/entity:/chunk: keys as memory names.
    // Cross-op claim bugs used to produce HardFailure NotFound("memory 'pair:…'").
    if super::queue::is_non_memory_key_shape(memory_name) {
        return Ok(EnrichItemResult::Skipped {
            reason: format!(
                "wrong_key_shape_for_operation:MemoryBindings: key looks like {}",
                memory_name.split(':').next().unwrap_or("prefixed")
            ),
        });
    }

    // Look up the memory
    let (memory_id, body): (i64, String) = conn.query_row(
        "SELECT id, COALESCE(body,'') FROM memories WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
        rusqlite::params![namespace, memory_name],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("memory '{memory_name}' not found")),
        other => AppError::Database(other),
    })?;

    if body.trim().is_empty() {
        return Ok(EnrichItemResult::Skipped {
            reason: "body is empty".to_string(),
        });
    }

    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            BINDINGS_PROMPT,
            BINDINGS_SCHEMA,
            &body,
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            BINDINGS_PROMPT,
            BINDINGS_SCHEMA,
            &body,
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            BINDINGS_PROMPT,
            BINDINGS_SCHEMA,
            &body,
            model,
            timeout,
        )?,
        EnrichMode::OpenRouter => {
            call_openrouter(BINDINGS_PROMPT, BINDINGS_SCHEMA, &body, model, timeout)?
        }
    };

    let empty_arr = serde_json::Value::Array(vec![]);
    let entities_val = value.get("entities").unwrap_or(&empty_arr);
    let rels_val = value.get("relationships").unwrap_or(&empty_arr);

    let (ent_count, rel_count) =
        persist_memory_bindings(conn, namespace, memory_id, entities_val, rels_val)?;

    Ok(EnrichItemResult::Done {
        memory_id: Some(memory_id),
        entity_id: None,
        entities: ent_count,
        rels: rel_count,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// Default top-K linked memory bodies injected into the ED prompt
/// (GAP-CLI-ED-02). Overridable via XDG `enrich.entity_description.corpus_top_k`.
pub(crate) const ENTITY_DESCRIPTION_CORPUS_TOP_K: usize = 5;
/// Per-body character budget for corpus snippets (GAP-CLI-ED-02).
pub(crate) const ENTITY_DESCRIPTION_SNIPPET_CHARS: usize = 400;
/// Default grounding coverage threshold for entity descriptions
/// (GAP-CLI-ED-03). Lower than body-enrich's 0.7 because descriptions
/// are short (10–20 words) relative to multi-sentence bodies.
pub(crate) const ENTITY_DESCRIPTION_GROUNDING_DEFAULT: f64 = 0.12;

/// Loads top-K linked memory body snippets for grounding (GAP-CLI-ED-02).
///
/// Shared by entity-description generation and status quality sampling (DRY).
pub(crate) fn load_entity_corpus_snippets(
    conn: &Connection,
    entity_id: i64,
    top_k: usize,
    max_chars: usize,
) -> Result<String, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT COALESCE(m.body, '') AS body
         FROM memory_entities me
         JOIN memories m ON m.id = me.memory_id
         WHERE me.entity_id = ?1 AND m.deleted_at IS NULL
         ORDER BY COALESCE(m.updated_at, m.created_at) DESC, m.id DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![entity_id, top_k as i64], |r| {
        r.get::<_, String>(0)
    })?;
    let mut snippets = Vec::with_capacity(top_k);
    for row in rows {
        let body = row.map_err(AppError::Database)?;
        let trimmed = body.trim();
        if trimmed.is_empty() {
            continue;
        }
        let snippet: String = trimmed.chars().take(max_chars).collect();
        snippets.push(snippet);
    }
    Ok(snippets.join("\n---\n"))
}

#[allow(clippy::too_many_arguments)] // Wave 5: fold into call-params struct on monólito split
pub(crate) fn call_entity_description(
    conn: &Connection,
    namespace: &str,
    entity_name: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
    grounding_threshold: f64,
    domain_label: &str,
) -> Result<EnrichItemResult, AppError> {
    let (entity_id, entity_type): (i64, String) = conn
        .query_row(
            "SELECT id, type FROM entities WHERE namespace=?1 AND name=?2",
            rusqlite::params![namespace, entity_name],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::EntityNotYetMaterialized {
                name: entity_name.to_string(),
                namespace: namespace.to_string(),
            },
            other => AppError::Database(other),
        })?;

    let corpus = load_entity_corpus_snippets(
        conn,
        entity_id,
        ENTITY_DESCRIPTION_CORPUS_TOP_K,
        ENTITY_DESCRIPTION_SNIPPET_CHARS,
    )?;

    let corpus_section = if corpus.is_empty() {
        "Linked memory evidence: (none — describe conservatively from name and type only; do not invent a software/product frame).\n".to_string()
    } else {
        format!("Linked memory evidence (ground truth; prefer these facts):\n{corpus}\n")
    };

    let domain_section = super::prompts::entity_description_domain_section(domain_label);
    let prompt = format!(
        "{ENTITY_DESCRIPTION_PROMPT_PREFIX}{entity_name}\nEntity type: {entity_type}\n\n{domain_section}{corpus_section}\nGenerate a description:"
    );

    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => call_claude(
            binary,
            &prompt,
            ENTITY_DESCRIPTION_SCHEMA,
            "",
            model,
            timeout,
        )?,
        EnrichMode::Codex => call_codex(
            binary,
            &prompt,
            ENTITY_DESCRIPTION_SCHEMA,
            "",
            model,
            timeout,
        )?,
        EnrichMode::Opencode => call_opencode(
            binary,
            &prompt,
            ENTITY_DESCRIPTION_SCHEMA,
            "",
            model,
            timeout,
        )?,
        EnrichMode::OpenRouter => {
            call_openrouter(&prompt, ENTITY_DESCRIPTION_SCHEMA, "", model, timeout)?
        }
    };

    let description = value
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Validation("LLM result missing 'description' field".into()))?;

    // GAP-CLI-ED-03 / G-T-DRY-01: reject ungrounded descriptions when corpus exists.
    let threshold = if grounding_threshold > 0.0 {
        grounding_threshold
    } else {
        ENTITY_DESCRIPTION_GROUNDING_DEFAULT
    };
    let verdict =
        crate::preservation::PreservationVerdict::evaluate_grounding(description, &corpus, threshold);
    if !verdict.is_accepted() {
        let score = match verdict {
            crate::preservation::PreservationVerdict::Preserved { score, .. } => score,
            crate::preservation::PreservationVerdict::Rejected { score, .. } => score,
            crate::preservation::PreservationVerdict::Unchanged { .. } => 1.0,
        };
        return Ok(EnrichItemResult::PreservationFailed {
            score,
            threshold,
            chars_before: 0,
            chars_after: description.chars().count(),
        });
    }

    persist_entity_description(conn, entity_id, description)?;

    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: Some(entity_id),
        entities: 0,
        rels: 0,
        chars_before: None,
        chars_after: Some(description.chars().count()),
        cost,
        is_oauth,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn call_body_enrich(
    conn: &Connection,
    namespace: &str,
    memory_name: &str,
    binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
    min_output_chars: usize,
    max_output_chars: usize,
    prompt_template: Option<&Path>,
    preserve_threshold: f64,
    paths: &crate::paths::AppPaths,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<EnrichItemResult, AppError> {
    let (memory_id, body, description, memory_type): (i64, String, String, String) = conn
        .query_row(
            "SELECT id, COALESCE(body,''), COALESCE(description,''), COALESCE(type,'note') \
         FROM memories WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
            rusqlite::params![namespace, memory_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(format!("memory '{memory_name}' not found"))
            }
            other => AppError::Database(other),
        })?;

    let chars_before = body.chars().count();

    // G26: gather graph context for contextualized enrichment
    let linked_entities: Vec<String> = {
        let mut stmt = conn.prepare_cached(
            "SELECT e.name FROM memory_entities me \
             JOIN entities e ON e.id = me.entity_id \
             WHERE me.memory_id = ?1 LIMIT 10",
        )?;
        let result: Vec<String> = stmt
            .query_map(rusqlite::params![memory_id], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        result
    };

    // Load custom prompt template if provided
    let prompt_prefix = if let Some(tmpl_path) = prompt_template {
        let file_size = std::fs::metadata(tmpl_path)
            .map_err(|e| {
                AppError::Io(std::io::Error::new(
                    e.kind(),
                    format!("failed to stat prompt template: {e}"),
                ))
            })?
            .len();
        if file_size > MAX_MEMORY_BODY_LEN as u64 {
            return Err(AppError::BodyTooLarge {
                bytes: file_size,
                limit: MAX_MEMORY_BODY_LEN as u64,
            });
        }
        std::fs::read_to_string(tmpl_path).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("failed to read prompt template: {e}"),
            ))
        })?
    } else {
        BODY_ENRICH_PROMPT_PREFIX.to_string()
    };

    // G26: build contextualized prompt with graph data
    let context_section = if !linked_entities.is_empty() || !description.is_empty() {
        let mut ctx = String::new();
        ctx.push_str(&format!(
            "\nContext:\n- Memory name: {memory_name}\n- Type: {memory_type}\n"
        ));
        if !description.is_empty() {
            ctx.push_str(&format!("- Description: {description}\n"));
        }
        ctx.push_str(&format!("- Domain: {namespace}\n"));
        if !linked_entities.is_empty() {
            ctx.push_str(&format!(
                "- Linked entities: {}\n",
                linked_entities.join(", ")
            ));
        }
        ctx
    } else {
        String::new()
    };

    let prompt = format!(
        "{prompt_prefix}{context_section}\nTarget minimum length: {min_output_chars} characters. Maximum: {max_output_chars} characters."
    );

    // The body schema uses a free-form enriched_body field
    let (value, cost, is_oauth) = match mode {
        EnrichMode::ClaudeCode => {
            call_claude(binary, &prompt, BODY_ENRICH_SCHEMA, &body, model, timeout)?
        }
        EnrichMode::Codex => {
            call_codex(binary, &prompt, BODY_ENRICH_SCHEMA, &body, model, timeout)?
        }
        EnrichMode::Opencode => {
            call_opencode(binary, &prompt, BODY_ENRICH_SCHEMA, &body, model, timeout)?
        }
        EnrichMode::OpenRouter => {
            call_openrouter(&prompt, BODY_ENRICH_SCHEMA, &body, model, timeout)?
        }
    };

    let enriched_body = value
        .get("enriched_body")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Validation("LLM result missing 'enriched_body' field".into()))?;

    let chars_after = enriched_body.chars().count();

    // G29 Passo 4 (v1.0.69): preservation check. Before persisting, run
    // a trigram-Jaccard similarity between the original body and the
    // LLM-rewritten body. When the score falls below
    // `args.preserve_threshold` (default 0.7 per the G29 gap), reject the
    // rewrite as a likely hallucination. The result is recorded in the
    // NDJSON stream so operators can audit what the LLM tried to do.
    let threshold = preserve_threshold;
    let verdict =
        crate::preservation::PreservationVerdict::evaluate(&body, enriched_body, threshold);
    if !verdict.is_accepted() {
        return Ok(EnrichItemResult::PreservationFailed {
            score: match verdict {
                crate::preservation::PreservationVerdict::Preserved { score, .. } => score,
                crate::preservation::PreservationVerdict::Rejected { score, .. } => score,
                crate::preservation::PreservationVerdict::Unchanged { .. } => 1.0,
            },
            threshold,
            chars_before,
            chars_after,
        });
    }

    // G29 Passo 5 (v1.0.69): idempotency via blake3 hash. Before persisting,
    // compare the hash of the original body against the hash of the enriched
    // body. Identical hashes mean the LLM produced a byte-for-byte identical
    // body (rare but possible) — treat as `Skipped` so re-running the batch
    // is safe and the queue does not get re-persisted entries.
    let old_hash = blake3::hash(body.as_bytes()).to_hex().to_string();
    let new_hash = blake3::hash(enriched_body.as_bytes()).to_hex().to_string();
    if old_hash == new_hash {
        return Ok(EnrichItemResult::Skipped {
            reason: format!(
                "enriched body hash matches original (blake3:{old_hash}); idempotency skip"
            ),
        });
    }

    // Only persist if the enriched body is genuinely longer
    if chars_after <= chars_before {
        return Ok(EnrichItemResult::Skipped {
            reason: format!(
                "enriched body ({chars_after} chars) not longer than original ({chars_before} chars)"
            ),
        });
    }

    persist_enriched_body(
        conn,
        namespace,
        memory_id,
        memory_name,
        enriched_body,
        paths,
        llm_backend,
        embedding_backend,
    )?;

    Ok(EnrichItemResult::Done {
        memory_id: Some(memory_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: Some(chars_before),
        chars_after: Some(chars_after),
        cost,
        is_oauth,
    })
}

// GAP-SG-73: failures from `reembed_memory_vector` below reach the queue
// as bare `AppError::Embedding`, not a typed `EmbedError` — see the doc
// comment on the `AppError::Embedding` arm of `classify_enrich_outcome` in
// `queue.rs` for why the origin-typed `retry_class` is not threaded through
// here, and why Transient is the documented, deliberate safe floor.
pub(crate) fn call_reembed(
    conn: &Connection,
    namespace: &str,
    item_key: &str,
    paths: &crate::paths::AppPaths,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<EnrichItemResult, AppError> {
    // v1.1.1 (P2): prefixed keys route to the entity/chunk backfill paths
    // (`re-embed --target entities|chunks|all`); bare keys keep the
    // historical memory behaviour, so pre-v1.1.1 queue rows still work.
    if let Some(entity_name) = item_key.strip_prefix("entity:") {
        return call_reembed_entity(
            conn,
            namespace,
            entity_name,
            paths,
            llm_backend,
            embedding_backend,
        );
    }
    if let Some(chunk_key) = item_key.strip_prefix("chunk:") {
        return call_reembed_chunk(
            conn,
            namespace,
            chunk_key,
            paths,
            llm_backend,
            embedding_backend,
        );
    }
    let memory_name = item_key;
    let (memory_id, body, memory_type): (i64, String, String) = conn
        .query_row(
            "SELECT id, COALESCE(body,''), COALESCE(type,'note')
             FROM memories
             WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
            rusqlite::params![namespace, memory_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(format!("memory '{memory_name}' not found"))
            }
            other => AppError::Database(other),
        })?;

    if body.trim().is_empty() {
        return Ok(EnrichItemResult::Skipped {
            reason: "body is empty".to_string(),
        });
    }

    reembed_memory_vector(
        conn,
        namespace,
        memory_id,
        memory_name,
        &memory_type,
        &body,
        paths,
        llm_backend,
        embedding_backend,
    )?;

    Ok(EnrichItemResult::Done {
        memory_id: Some(memory_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: Some(body.chars().count()),
        chars_after: Some(body.chars().count()),
        cost: 0.0,
        is_oauth: true,
    })
}

/// v1.1.1 (P2): rebuilds the vector of a single entity
/// (`re-embed --target entities`).
///
/// Embeds the same text formula the write path uses (the entity name, plus
/// the description when present) so backfilled vectors are comparable to the
/// ones produced at write time by `remember`/`ingest`.
fn call_reembed_entity(
    conn: &Connection,
    namespace: &str,
    entity_name: &str,
    paths: &crate::paths::AppPaths,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<EnrichItemResult, AppError> {
    let (entity_id, description, entity_type): (i64, String, String) = conn
        .query_row(
            "SELECT id, COALESCE(description,''), type
             FROM entities
             WHERE namespace=?1 AND name=?2",
            rusqlite::params![namespace, entity_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(format!("entity '{entity_name}' not found"))
            }
            other => AppError::Database(other),
        })?;

    let text = if description.is_empty() {
        entity_name.to_string()
    } else {
        format!("{entity_name} {description}")
    };
    let (embedding, backend_kind) = crate::embedder::embed_passage_with_embedding_choice(
        &paths.models,
        &text,
        embedding_backend,
        llm_backend,
    )?;
    if embedding.is_empty() {
        return Ok(EnrichItemResult::Skipped {
            reason: "embedding backend returned an empty vector (chain resolved to none)"
                .to_string(),
        });
    }
    super::postprocess::record_enrich_backend(backend_kind.as_str());
    entities::upsert_entity_vec(
        conn,
        entity_id,
        namespace,
        EntityType::map_to_canonical(&entity_type),
        &embedding,
        entity_name,
    )?;
    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: Some(entity_id),
        entities: 1,
        rels: 0,
        chars_before: Some(text.chars().count()),
        chars_after: Some(text.chars().count()),
        cost: 0.0,
        is_oauth: true,
    })
}

/// v1.1.1 (P2): rebuilds the vector of a single chunk
/// (`re-embed --target chunks`). The key carries the `memory_chunks.id`.
fn call_reembed_chunk(
    conn: &Connection,
    namespace: &str,
    chunk_key: &str,
    paths: &crate::paths::AppPaths,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<EnrichItemResult, AppError> {
    let chunk_id: i64 = chunk_key.parse().map_err(|_| {
        AppError::Validation(format!("invalid chunk id in re-embed key: {chunk_key}"))
    })?;
    let (memory_id, chunk_idx, chunk_text): (i64, i32, String) = conn
        .query_row(
            "SELECT c.memory_id, c.chunk_idx, c.chunk_text
             FROM memory_chunks c
             JOIN memories m ON m.id = c.memory_id
             WHERE c.id = ?1 AND m.namespace = ?2 AND m.deleted_at IS NULL",
            rusqlite::params![chunk_id, namespace],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!(
                "chunk {chunk_id} not found in namespace '{namespace}'"
            )),
            other => AppError::Database(other),
        })?;

    if chunk_text.trim().is_empty() {
        return Ok(EnrichItemResult::Skipped {
            reason: "chunk text is empty".to_string(),
        });
    }
    let (embedding, backend_kind) = crate::embedder::embed_passage_with_embedding_choice(
        &paths.models,
        &chunk_text,
        embedding_backend,
        llm_backend,
    )?;
    if embedding.is_empty() {
        return Ok(EnrichItemResult::Skipped {
            reason: "embedding backend returned an empty vector (chain resolved to none)"
                .to_string(),
        });
    }
    super::postprocess::record_enrich_backend(backend_kind.as_str());
    crate::storage::chunks::upsert_chunk_vec(conn, chunk_id, memory_id, chunk_idx, &embedding)?;
    Ok(EnrichItemResult::Done {
        memory_id: Some(memory_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: Some(chunk_text.chars().count()),
        chars_after: Some(chunk_text.chars().count()),
        cost: 0.0,
        is_oauth: true,
    })
}

// scan_operation moved to scan.rs

// ---------------------------------------------------------------------------
// Codex stub provider
// ---------------------------------------------------------------------------

/// Locates the Codex CLI binary.
pub(crate) fn find_codex_binary(explicit: Option<&Path>) -> Result<PathBuf, AppError> {
    if let Some(p) = explicit {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        return Err(AppError::Validation(format!(
            "Codex binary not found at explicit path: {}",
            p.display()
        )));
    }

    if let Some(env_path) = crate::runtime_config::codex_binary() {
        let p = PathBuf::from(&env_path);
        if p.exists() {
            return Ok(p);
        }
    }

    let name = if cfg!(windows) { "codex.exe" } else { "codex" };
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Ok(crate::extract::llm_embedding::resolve_real_binary(
                    &candidate,
                ));
            }
        }
    }

    Err(AppError::Validation(
        "Codex CLI binary not found in PATH. Install it or specify --codex-binary".to_string(),
    ))
}
