//! Phase A staging: read, chunk, embed and extract NER for one file.

use crate::chunking;
use crate::errors::AppError;
use crate::paths::AppPaths;
use crate::storage::entities::{NewEntity, NewRelationship};
use std::path::Path;

/// All artefacts pre-computed by Phase A (CPU-bound, runs on rayon thread pool).
/// Phase B persists these to SQLite on the main thread in submission order.
pub(crate) struct StagedFile {
    pub(crate) body: String,
    pub(crate) body_hash: String,
    pub(crate) snippet: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) embedding: Option<Vec<f32>>,
    pub(crate) chunk_embeddings: Option<Vec<Vec<f32>>>,
    pub(crate) chunks_info: Vec<crate::chunking::Chunk>,
    pub(crate) entities: Vec<NewEntity>,
    pub(crate) relationships: Vec<NewRelationship>,
    pub(crate) entity_embeddings: Option<Vec<Vec<f32>>>,
    pub(crate) urls: Vec<crate::extraction::ExtractedUrl>,
    /// v1.0.84 (ADR-0042): discriminator of the embedding backend that actually
    /// ran the body embedding. `None` when the parallel batch
    /// embed_passages_parallel_local fell back to different backends
    /// across chunks (there is no single stable discriminator).
    pub(crate) backend_invoked: Option<&'static str>,
}

/// The staging environment shared by every file of one ingest run.
///
/// These five are constant for the whole run: the caller reads them off
/// `IngestArgs` once and every Phase A worker sees the same values. Passing them
/// as five positionals put `max_rss_mb` and `llm_parallelism` — both integers —
/// side by side in the argument list, where transposing them type-checks and
/// silently caps memory at the fan-out width.
#[derive(Clone, Copy)]
pub(crate) struct StagingEnv<'a> {
    /// Resolved application paths; only `models` is read during staging.
    pub(crate) paths: &'a AppPaths,
    /// Whether to run named-entity extraction over each body.
    pub(crate) enable_ner: bool,
    /// RSS ceiling in MiB; staging aborts above it instead of thrashing.
    pub(crate) max_rss_mb: u64,
    /// Embedding fan-out width for multi-chunk bodies and entity texts.
    pub(crate) llm_parallelism: usize,
    /// Which LLM and embedding backends answer the embedding calls.
    pub(crate) backends: crate::cli::BackendChoice,
}

/// Phase A worker: reads, chunks, embeds and extracts NER for one file.
/// Never touches the database — safe to run on any rayon thread.
pub(crate) fn stage_file(
    _idx: usize,
    path: &Path,
    name: &str,
    env: StagingEnv<'_>,
    auto_describe: bool,
) -> Result<Vec<StagedFile>, AppError> {
    use crate::constants::*;

    if name.len() > MAX_MEMORY_NAME_LEN {
        return Err(AppError::LimitExceeded(
            crate::i18n::validation::name_length(MAX_MEMORY_NAME_LEN),
        ));
    }
    if name.starts_with("__") {
        return Err(AppError::Validation(
            crate::i18n::validation::reserved_name(),
        ));
    }
    {
        let slug_re = crate::constants::name_slug_regex();
        if !slug_re.is_match(name) {
            return Err(AppError::Validation(crate::i18n::validation::name_kebab(
                name,
            )));
        }
    }

    let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
    if file_size > MAX_MEMORY_BODY_LEN as u64 {
        return Err(AppError::BodyTooLarge {
            bytes: file_size,
            limit: MAX_MEMORY_BODY_LEN as u64,
        });
    }
    let raw_body = std::fs::read_to_string(path).map_err(AppError::Io)?;
    if raw_body.len() > MAX_MEMORY_BODY_LEN {
        return Err(AppError::BodyTooLarge {
            bytes: raw_body.len() as u64,
            limit: MAX_MEMORY_BODY_LEN as u64,
        });
    }
    if raw_body.trim().is_empty() {
        return Err(AppError::Validation(crate::i18n::validation::empty_body()));
    }

    let description = if auto_describe {
        crate::commands::ingest_heuristics::extract_heuristic_description(
            &raw_body,
            Some(&path.display().to_string()),
        )
    } else {
        format!("ingested from {}", path.display())
    };
    if description.len() > MAX_MEMORY_DESCRIPTION_LEN {
        return Err(AppError::Validation(
            crate::i18n::validation::description_exceeds(MAX_MEMORY_DESCRIPTION_LEN),
        ));
    }

    // GAP-SG-04/07: auto-split a body that exceeds the single-memory budgets
    // (bytes, chunk count, token count) into section-aligned sub-memories so
    // ingestion never fails on an oversized document. A body that fits returns
    // a single partition under the original name.
    // Token-ceiling enforcement for ingest lives in chunking::fits_single_partition
    // (EMBEDDING_REQUEST_MAX_TOKENS): the auto-split keeps every partition under
    // the cap, so no edge rejection is needed here (v1.1.02).
    let partitions = chunking::split_body_by_sections(&raw_body);
    let total_parts = partitions.len();
    let mut staged = Vec::with_capacity(total_parts);
    for (part_idx, part_body) in partitions.into_iter().enumerate() {
        let part_name = if total_parts == 1 {
            name.to_string()
        } else {
            format!("{name}-part-{}", part_idx + 1)
        };
        if part_name.len() > MAX_MEMORY_NAME_LEN {
            return Err(AppError::LimitExceeded(
                crate::i18n::validation::name_length(MAX_MEMORY_NAME_LEN),
            ));
        }
        let part_description = if total_parts == 1 {
            description.clone()
        } else {
            partition_description(&description, part_idx + 1, total_parts)
        };
        staged.push(stage_one_body(part_body, part_name, part_description, env)?);
    }
    Ok(staged)
}

/// Builds a partition description by appending a `(part i/n)` marker, trimming
/// the base (on a char boundary) when the marker would push it past
/// [`crate::constants::MAX_MEMORY_DESCRIPTION_LEN`].
pub(crate) fn partition_description(base: &str, part: usize, total: usize) -> String {
    let suffix = format!(" (part {part}/{total})");
    let max = crate::constants::MAX_MEMORY_DESCRIPTION_LEN;
    if base.len() + suffix.len() <= max {
        return format!("{base}{suffix}");
    }
    let mut cut = max.saturating_sub(suffix.len()).min(base.len());
    while cut > 0 && !base.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{}", &base[..cut], suffix)
}

/// Stages a single body (one memory) into a [`StagedFile`]: NER extraction,
/// chunking, embedding and entity embedding. Extracted from `stage_file` so the
/// GAP-SG-04/07 auto-split path stages each partition independently.
pub(crate) fn stage_one_body(
    raw_body: String,
    name: String,
    description: String,
    env: StagingEnv<'_>,
) -> Result<StagedFile, AppError> {
    use crate::constants::*;

    let StagingEnv {
        paths,
        enable_ner,
        max_rss_mb,
        llm_parallelism,
        backends,
    } = env;

    let mut extracted_entities: Vec<NewEntity> = Vec::with_capacity(30);
    let mut extracted_relationships: Vec<NewRelationship> = Vec::with_capacity(50);
    let mut extracted_urls: Vec<crate::extraction::ExtractedUrl> = Vec::with_capacity(4);
    if enable_ner {
        match crate::extraction::extract_graph_auto(&raw_body, paths) {
            Ok(extracted) => {
                extracted_urls = extracted.urls;
                // v1.0.76: ExtractionResult.entities is now
                // Vec<ExtractedEntity>, not Vec<NewEntity>. Convert
                // via name + type only; start/end offsets are not
                // carried forward into the storage layer.
                extracted_entities = extracted
                    .entities
                    .into_iter()
                    .map(|e| NewEntity {
                        name: e.name,
                        // Extraction reports a span, never a kind, so nothing is
                        // being folded here: this is the "caller supplied no
                        // type" case the default exists for.
                        entity_type: crate::entity_type::DEFAULT_ENTITY_TYPE.to_string(),
                        description: None,
                    })
                    .collect();
                // v1.0.76: relationships are no longer in the
                // ExtractionResult struct; the LLM backend returns
                // them in its own payload. The default build is
                // URL-only extraction.
                extracted_relationships.clear();

                if extracted_entities.len() > max_entities_per_memory() {
                    extracted_entities.truncate(max_entities_per_memory());
                }
                if extracted_relationships.len() > max_relationships_per_memory() {
                    extracted_relationships.truncate(max_relationships_per_memory());
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "ingest",
                    file = %name,
                    "auto-extraction failed (graceful degradation): {e:#}"
                );
            }
        }
    }

    for rel in &mut extracted_relationships {
        rel.relation = crate::parsers::normalize_relation(&rel.relation);
        if let Err(e) = crate::parsers::validate_relation_format(&rel.relation) {
            return Err(AppError::Validation(
                crate::i18n::validation::relation_format_for_relationship(
                    &e,
                    &rel.source,
                    &rel.target,
                ),
            ));
        }
        crate::parsers::warn_if_non_canonical(&rel.relation);
        if !(0.0..=1.0).contains(&rel.strength) {
            return Err(AppError::Validation(
                crate::i18n::validation::invalid_relationship_strength(
                    rel.strength,
                    &rel.source,
                    &rel.target,
                ),
            ));
        }
    }

    let body_hash = blake3::hash(raw_body.as_bytes()).to_hex().to_string();
    let snippet: String = raw_body.chars().take(200).collect();

    let chunks_info = chunking::split_into_chunks_hierarchical(&raw_body);
    if chunks_info.len() > REMEMBER_MAX_SAFE_MULTI_CHUNKS {
        return Err(AppError::TooManyChunks {
            chunks: chunks_info.len(),
            limit: REMEMBER_MAX_SAFE_MULTI_CHUNKS,
        });
    }

    let mut chunk_embeddings_opt: Option<Vec<Vec<f32>>> = None;
    let skip_embed = crate::embedder::should_skip_embedding_on_failure();
    // v1.0.84 (ADR-0042): the tuple (Vec<f32>, LlmBackendKind) carries the
    // backend that actually ran, which populates `backend_invoked` in the
    // per-file NDJSON envelope.
    let (embedding, backend_invoked): (Option<Vec<f32>>, Option<&'static str>) = if chunks_info
        .len()
        == 1
    {
        match crate::embedder::embed_passage_with_embedding_choice(
            &paths.models,
            &raw_body,
            backends,
        ) {
            Ok((v, k)) => (Some(v), Some(k.as_str())),
            // v1.1.2 (Gap 2): typed payload rejections are permanent and
            // must not be swallowed by --skip-embedding-on-failure.
            Err(
                e @ (AppError::Validation(_)
                | AppError::BodyTooLarge { .. }
                | AppError::TooManyTokens { .. }),
            ) => return Err(e),
            Err(e) if skip_embed => {
                tracing::warn!(error = %e, file = %name, "ingest: embedding failed; --skip-embedding-on-failure active, persisting without embedding");
                (None, None)
            }
            Err(e) => return Err(e),
        }
    } else {
        // G42/S2+S3 (v1.0.79): batched bounded fan-out replaces the
        // serial per-chunk subprocess loop.
        let chunk_texts: Vec<String> = chunks_info
            .iter()
            .map(|c| chunking::chunk_text(&raw_body, c).to_string())
            .collect();
        if let Some(rss) = crate::memory_guard::current_process_memory_mb() {
            if rss > max_rss_mb {
                tracing::error!(
                    target: "ingest",
                    rss_mb = rss,
                    max_rss_mb = max_rss_mb,
                    file = %name,
                    "RSS exceeded --max-rss-mb threshold; aborting to prevent system instability"
                );
                return Err(AppError::LowMemory {
                    available_mb: crate::memory_guard::available_memory_mb(),
                    required_mb: max_rss_mb,
                });
            }
        }
        match crate::embedder::embed_passages_parallel_shared(
            &paths.models,
            std::sync::Arc::from(chunk_texts),
            llm_parallelism,
            crate::embedder::chunk_embed_batch_size(),
            backends,
        ) {
            Ok(chunk_embeddings) => {
                let aggregated = chunking::aggregate_embeddings(&chunk_embeddings);
                chunk_embeddings_opt = Some(chunk_embeddings);
                // v1.0.84 (ADR-0042): parallel batch does not return a per-call
                // discriminator. Conservatively, we populate None here.
                (Some(aggregated), None)
            }
            Err(
                e @ (AppError::Validation(_)
                | AppError::BodyTooLarge { .. }
                | AppError::TooManyTokens { .. }),
            ) => return Err(e),
            Err(e) if skip_embed => {
                tracing::warn!(error = %e, file = %name, "ingest: chunk embedding failed; --skip-embedding-on-failure active, persisting without embedding");
                (None, None)
            }
            Err(e) => return Err(e),
        }
    };

    // G42/S2+A4 (v1.0.79): entity names use the short-text batch profile.
    let entity_texts: Vec<String> = extracted_entities
        .iter()
        .map(|entity| match &entity.description {
            Some(desc) => format!("{} {}", entity.name, desc),
            None => entity.name.clone(),
        })
        .collect();
    // G56 (v1.0.80): ingest reuses canonical entity names across many
    // memories (e.g. `sqlite-graphrag`, `claude-code`); the in-process
    // cache collapses the repeated LLM calls into one per unique text.
    let entity_embeddings_opt = match crate::embedder::embed_entity_texts_cached(
        &paths.models,
        &entity_texts,
        llm_parallelism,
        backends,
    ) {
        Ok((entity_embeddings, embed_cache_stats)) => {
            if embed_cache_stats.hits > 0 {
                tracing::debug!(
                    hits = embed_cache_stats.hits,
                    misses = embed_cache_stats.misses,
                    requested = embed_cache_stats.requested,
                    "G56: entity embed cache hit (ingest)"
                );
            }
            Some(entity_embeddings)
        }
        Err(e) if skip_embed => {
            tracing::warn!(error = %e, file = %name, "ingest: entity embedding failed; --skip-embedding-on-failure active");
            None
        }
        Err(e) => return Err(e),
    };

    Ok(StagedFile {
        body: raw_body,
        body_hash,
        snippet,
        name,
        description,
        embedding,
        chunk_embeddings: chunk_embeddings_opt,
        chunks_info,
        entities: extracted_entities,
        relationships: extracted_relationships,
        entity_embeddings: entity_embeddings_opt,
        urls: extracted_urls,
        backend_invoked,
    })
}
