//! Persistence layer for entities, relationships and their junction tables.
//!
//! The entity graph mirrors the conceptual content of memories: `entities`
//! holds nodes, `relationships` holds typed edges and `memory_entities` and
//! `memory_relationships` connect each memory to the graph slice it emitted.

mod merge;

pub use merge::{
    clear_memory_graph_bindings, count_relationships_by_relation, create_or_fetch_relationship,
    delete_entities_by_ids, delete_relationship_by_id, delete_relationships_by_ids,
    delete_relationships_by_relation, find_dangling_relationship_ids, find_entity_id,
    find_orphan_entity_ids, find_relationship, increment_degree, link_memory_entity,
    link_memory_relationship, list_entity_names_by_relation, recalculate_degree,
    unlink_memory_entity, RelationshipRow,
};

use crate::embedder::f32_to_bytes;
use crate::entity_type::normalize_entity_type;
use crate::errors::AppError;
use crate::parsers::normalize_entity_name;
use crate::storage::utils::with_busy_retry;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Input payload used to upsert a single entity.
///
/// `name` is normalized to kebab-case by the caller. `description` is
/// optional and preserved across upserts when the new value is `None`.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct NewEntity {
    /// Name of this item.
    pub name: String,
    /// Entity type label, stored as the caller wrote it.
    ///
    /// v1.2.8: plain `String` rather than a closed enum. Deserialization no
    /// longer folds unknown labels onto `concept` — that fold destroyed the
    /// caller's word before any layer could see it. Shape is normalised at the
    /// write boundary by [`normalize_entity_type`], which is where a refusal
    /// can still be reported; membership in the canonical set is advisory and
    /// only enforced under `--strict-entity-types`.
    #[serde(alias = "type")]
    pub entity_type: String,
    /// Human-readable description.
    pub description: Option<String>,
}

/// Input payload used to upsert a typed relationship between entities.
///
/// `strength` must lie within `[0.0, 1.0]` and is mapped to the `weight`
/// column of the `relationships` table.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct NewRelationship {
    /// Source side of the relationship.
    #[serde(alias = "from")]
    pub source: String,
    /// Target side of the relationship.
    #[serde(alias = "to")]
    pub target: String,
    /// Relationship type.
    #[serde(alias = "type")]
    pub relation: String,
    /// Relationship strength in `[0.0, 1.0]`. Defaults to
    /// [`crate::constants::DEFAULT_RELATION_WEIGHT`] (0.5) when omitted from
    /// graph-stdin / graph-file JSON (GAP-CLI-GRAPH-01).
    #[serde(alias = "weight", default = "default_relationship_strength")]
    pub strength: f64,
    /// Human-readable description.
    pub description: Option<String>,
}

fn default_relationship_strength() -> f64 {
    crate::constants::DEFAULT_RELATION_WEIGHT
}

/// Validates entity name against quality rules.
///
/// Rejects names with newlines, names shorter than 2 characters, and
/// ALL_CAPS abbreviations of 4 characters or fewer (common NER noise).
///
/// # Errors
///
/// Returns `Err(AppError::Validation)` when the name violates any rule.
pub fn validate_entity_name(name: &str) -> Result<(), AppError> {
    if name.len() < 2 {
        return Err(AppError::Validation(
            crate::i18n::validation::entity_name_too_short(name),
        ));
    }
    if name.contains('\n') || name.contains('\r') {
        return Err(AppError::Validation(
            "entity name must not contain newline characters".to_string(),
        ));
    }
    // v1.1.05 Bug 5: pure digit names are almost always accidental entity IDs
    // passed as `--from`/`--to` instead of names (or via `--from-id`/`--to-id`).
    // Reject them so `--create-missing` cannot pollute the graph with ghost nodes.
    if name.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::Validation(
            crate::i18n::validation::entity_name_purely_numeric(name),
        ));
    }
    if name.len() <= 4
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c == '-')
    {
        return Err(AppError::Validation(
            crate::i18n::validation::entity_name_all_caps_noise(name),
        ));
    }
    Ok(())
}

/// Fuzzy match candidate returned by [`suggest_entity_names`] / [`resolve_entity_fuzzy`].
#[derive(Debug, Clone)]
pub struct FuzzyEntityMatch {
    /// Unique identifier.
    pub id: i64,
    /// Name of this item.
    pub name: String,
    /// Similarity in `[0.0, 1.0]` (1.0 = exact).
    pub score: f64,
}

/// Score how well `query` matches a canonical entity `name`.
///
/// Prefers exact, prefix-of-kebab, first-token equality, then Jaro-Winkler
/// (rapidfuzz) so short nicknames like `alice` rank `alice-martins-souza`
/// highly.
pub fn entity_name_similarity(query: &str, name: &str) -> f64 {
    let q = query.trim().to_ascii_lowercase();
    let n = name.trim().to_ascii_lowercase();
    if q.is_empty() || n.is_empty() {
        return 0.0;
    }
    if q == n {
        return 1.0;
    }
    // Prefix of a kebab/snake name: "alice" ↔ "alice-martins-souza"
    if n.starts_with(&q) {
        let rest = &n[q.len()..];
        if rest.is_empty()
            || rest.starts_with('-')
            || rest.starts_with('_')
            || rest.starts_with(' ')
        {
            return 0.95;
        }
        // Longer shared prefix still strong
        return 0.88;
    }
    if q.starts_with(&n) && n.len() >= 3 {
        return 0.80;
    }
    let first_token = n
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
        .next()
        .unwrap_or(n.as_str());
    if first_token == q {
        return 0.92;
    }
    if n.contains(&q) && q.len() >= 3 {
        return 0.82;
    }
    rapidfuzz::distance::jaro_winkler::normalized_similarity(q.chars(), n.chars())
}

/// Rank entity names in `namespace` by fuzzy similarity to `query`.
///
/// Returns up to `limit` candidates with score ≥ `min_score`, sorted by score
/// descending (ties break alphabetically).
pub fn suggest_entity_names(
    conn: &Connection,
    namespace: &str,
    query: &str,
    limit: usize,
    min_score: f64,
) -> Result<Vec<FuzzyEntityMatch>, AppError> {
    let entities = list_entities(conn, Some(namespace))?;
    let mut scored: Vec<FuzzyEntityMatch> = entities
        .into_iter()
        .filter_map(|e| {
            let score = entity_name_similarity(query, &e.name);
            if score >= min_score {
                Some(FuzzyEntityMatch {
                    id: e.id,
                    name: e.name,
                    score,
                })
            } else {
                None
            }
        })
        .collect();
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    scored.truncate(limit.max(1));
    Ok(scored)
}

/// Resolve an entity by exact name, then optionally by fuzzy match.
///
/// * Exact match always wins.
/// * When `auto_fuzzy` is true and exactly one candidate scores ≥ `min_score`
///   (or the top candidate is ≥ 0.90 and beats the runner-up by ≥ 0.05),
///   that candidate is returned with a stderr warning.
/// * When no auto-resolution is possible, returns `Ok(None)` after the caller
///   can surface suggestions via [`suggest_entity_names`].
pub fn resolve_entity_fuzzy(
    conn: &Connection,
    namespace: &str,
    name: &str,
    auto_fuzzy: bool,
) -> Result<Option<(i64, String, bool)>, AppError> {
    if let Some(id) = find_entity_id(conn, namespace, name)? {
        return Ok(Some((id, name.to_string(), false)));
    }
    // Case-insensitive exact via list (names are normalized kebab, but callers
    // may pass mixed case).
    let normalized = crate::parsers::normalize_entity_name(name);
    if normalized != name {
        if let Some(id) = find_entity_id(conn, namespace, &normalized)? {
            return Ok(Some((id, normalized, false)));
        }
    }
    if !auto_fuzzy {
        return Ok(None);
    }
    let suggestions = suggest_entity_names(conn, namespace, name, 5, 0.75)?;
    if suggestions.is_empty() {
        return Ok(None);
    }
    let top = &suggestions[0];
    let clear_winner =
        top.score >= 0.90 && (suggestions.len() == 1 || top.score - suggestions[1].score >= 0.05);
    let single_strong = suggestions.len() == 1 && top.score >= 0.85;
    if clear_winner || single_strong {
        tracing::warn!(
            target: "entities",
            query = %name,
            resolved = %top.name,
            score = top.score,
            "fuzzy entity resolution: exact match failed; using best candidate"
        );
        return Ok(Some((top.id, top.name.clone(), true)));
    }
    Ok(None)
}

/// Build a NotFound message that includes fuzzy suggestions when available.
pub fn entity_not_found_with_suggestions(
    conn: &Connection,
    namespace: &str,
    name: &str,
) -> AppError {
    let suggestions = suggest_entity_names(conn, namespace, name, 5, 0.70).unwrap_or_default();
    if suggestions.is_empty() {
        return AppError::NotFound(
            crate::i18n::validation::entity_named_not_found_in_namespace(name, namespace),
        );
    }
    let list: Vec<String> = suggestions
        .iter()
        .map(|s| format!("{} (score={:.2})", s.name, s.score))
        .collect();
    AppError::NotFound(
        crate::i18n::validation::entity_named_not_found_with_suggestions(
            name,
            namespace,
            &list.join(", "),
        ),
    )
}

/// Upserts an entity and returns its primary key.
///
/// Uses `ON CONFLICT(namespace, name)` to keep one row per entity within a
/// namespace, refreshing `type` and `description` opportunistically.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn upsert_entity(conn: &Connection, namespace: &str, e: &NewEntity) -> Result<i64, AppError> {
    // Step 1: validate the original name — catches ALL_CAPS short noise (NER artefacts),
    // newlines, and names shorter than 2 characters before any transformation.
    validate_entity_name(&e.name)?;
    // Step 2: normalize to kebab-case ASCII (NFKD, lowercase, spaces/underscores → hyphens).
    let normalized_name = normalize_entity_name(&e.name);
    // Step 3: guard post-normalization length — a valid original could collapse to < 2 chars
    // (e.g. a single accented character that strips entirely).
    if normalized_name.chars().count() < 2 {
        return Err(AppError::Validation(
            crate::i18n::validation::entity_name_normalizes_too_short(&e.name, &normalized_name),
        ));
    }
    // Step 4: normalise the type label's SHAPE. Membership is not checked here
    // — V017 opened the vocabulary, so an unknown label is stored as written.
    let normalized_type = normalize_entity_type(&e.entity_type)?;
    conn.execute(
        "INSERT INTO entities (namespace, name, type, description)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(namespace, name) DO UPDATE SET
           type        = excluded.type,
           description = COALESCE(excluded.description, entities.description),
           updated_at  = unixepoch()",
        params![namespace, normalized_name, normalized_type, e.description],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE namespace = ?1 AND name = ?2",
        params![namespace, normalized_name],
        |r| r.get(0),
    )?;
    Ok(id)
}

/// Upserts an entity WITHOUT overwriting a type someone already committed to.
///
/// Same contract as [`upsert_entity`] except for the `type` column: a new row
/// takes the caller's type, and an existing row keeps its own unless that type
/// is the generic `concept`, in which case the caller may refine it.
///
/// This exists because [`upsert_entity`] writes `type = excluded.type`
/// unconditionally and the LLM enrichment worker runs AFTER every write. A
/// person declared as `person` in `remember --graph-stdin` was re-typed by
/// whatever the model guessed minutes later, with the graph reporting a type
/// nobody asked for and no envelope ever mentioning the change. Measured on a
/// live corpus: an area of a company stored as `person`.
///
/// The rule is deliberately asymmetric. Human write paths — `remember`, `link`,
/// `ingest`, `split_body` — keep calling [`upsert_entity`] and stay
/// authoritative. Extraction calls this one, so it can still TYPE what nobody
/// typed and can still refine `concept`, which since v1.2.8 means only "the
/// caller supplied no type" rather than "the caller's label did not fit", and
/// therefore still carries no commitment worth preserving.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure, and
/// `Err(AppError::Validation)` on a name that fails [`validate_entity_name`].
pub fn upsert_entity_preserving_type(
    conn: &Connection,
    namespace: &str,
    e: &NewEntity,
) -> Result<i64, AppError> {
    validate_entity_name(&e.name)?;
    let normalized_name = normalize_entity_name(&e.name);
    if normalized_name.chars().count() < 2 {
        return Err(AppError::Validation(
            crate::i18n::validation::entity_name_normalizes_too_short(&e.name, &normalized_name),
        ));
    }
    let normalized_type = normalize_entity_type(&e.entity_type)?;
    conn.execute(
        "INSERT INTO entities (namespace, name, type, description)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(namespace, name) DO UPDATE SET
           type        = CASE WHEN entities.type = 'concept'
                              THEN excluded.type
                              ELSE entities.type END,
           description = COALESCE(excluded.description, entities.description),
           updated_at  = unixepoch()",
        params![namespace, normalized_name, normalized_type, e.description],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE namespace = ?1 AND name = ?2",
        params![namespace, normalized_name],
        |r| r.get(0),
    )?;
    Ok(id)
}

/// Replaces the vector row for an entity in `entity_embeddings`.
///
/// v1.0.76: sqlite-vec was removed. Embeddings live in a regular BLOB-backed
/// table; cosine similarity is computed in pure Rust on demand. The
/// `entity_type` and `name` arguments are accepted for API compatibility
/// but are not stored — the entities table is the source of truth.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn upsert_entity_vec(
    conn: &Connection,
    entity_id: i64,
    namespace: &str,
    _entity_type: &str,
    embedding: &[f32],
    _name: &str,
) -> Result<(), AppError> {
    // v1.1.1 (P1): an empty vector means the embedding backend was skipped
    // (`--llm-backend none` without OpenRouter). Writing an empty BLOB would
    // hide the entity from the re-embed backfill scanner (the row exists but
    // carries no vector), so skip the write and leave the entity scannable.
    if embedding.is_empty() {
        tracing::debug!(
            entity_id,
            "empty entity embedding: skipping entity_embeddings row (backfill via enrich re-embed --target entities)"
        );
        return Ok(());
    }
    let embedding_bytes = f32_to_bytes(embedding);
    with_busy_retry(|| {
        conn.execute(
            "DELETE FROM entity_embeddings WHERE entity_id = ?1",
            params![entity_id],
        )?;
        conn.execute(
            "INSERT INTO entity_embeddings(entity_id, namespace, embedding, source, model, dim)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                entity_id,
                namespace,
                &embedding_bytes,
                "llm-headless",
                crate::constants::SQLITE_GRAPHRAG_VERSION,
                crate::constants::embedding_dim() as i64,
            ],
        )?;
        Ok(())
    })
}

/// Upserts a typed relationship between two entity ids.
///
/// Conflicts on `(source_id, target_id, relation)` refresh `weight` and
/// preserve a non-null `description`. Returns the `rowid` of the stored row.
///
/// # Errors
///
/// Returns `Err(AppError::Database)` on any `rusqlite` failure.
pub fn upsert_relationship(
    conn: &Connection,
    namespace: &str,
    source_id: i64,
    target_id: i64,
    rel: &NewRelationship,
) -> Result<i64, AppError> {
    // v1.2.8: canonicalised here for the same reason as
    // `create_or_fetch_relationship` — the invariant belongs to the boundary
    // that writes, not to each caller that remembers.
    let relation = crate::parsers::map_to_canonical_relation(&rel.relation);
    conn.execute(
        "INSERT INTO relationships (namespace, source_id, target_id, relation, weight, description)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(source_id, target_id, relation) DO UPDATE SET
           weight = excluded.weight,
           description = COALESCE(excluded.description, relationships.description)",
        params![
            namespace,
            source_id,
            target_id,
            relation,
            rel.strength,
            rel.description
        ],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM relationships WHERE source_id=?1 AND target_id=?2 AND relation=?3",
        params![source_id, target_id, relation],
        |r| r.get(0),
    )?;
    Ok(id)
}

/// Entity row with enough data for graph export/query.
#[derive(Debug, Serialize, Clone)]
pub struct EntityNode {
    /// Unique identifier.
    pub id: i64,
    /// Name of this item.
    pub name: String,
    /// Namespace scope.
    pub namespace: String,
    /// Kind discriminator.
    pub kind: String,
    /// Stored description, `None` when NULL or empty (G-PR-7).
    ///
    /// Carried so `graph` can export it: `entities.description` had no bulk
    /// read path in the CLI at all, which is what let a bad description-writing
    /// policy run unnoticed over a six-figure entity count.
    pub description: Option<String>,
}

/// Lists entities, filtering by namespace if provided.
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn list_entities(
    conn: &Connection,
    namespace: Option<&str>,
) -> Result<Vec<EntityNode>, AppError> {
    if let Some(ns) = namespace {
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, namespace, type, description FROM entities WHERE namespace = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![ns], |r| {
                Ok(EntityNode {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    namespace: r.get(2)?,
                    kind: r.get(3)?,
                    description: r
                        .get::<_, Option<String>>(4)?
                        .filter(|d| !d.trim().is_empty()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, namespace, type, description FROM entities ORDER BY namespace, id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(EntityNode {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    namespace: r.get(2)?,
                    kind: r.get(3)?,
                    description: r
                        .get::<_, Option<String>>(4)?
                        .filter(|d| !d.trim().is_empty()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// Lists relations filtered by namespace (of source/target entities).
///
/// # Errors
///
/// Returns [`AppError::Database`] when the underlying SQLite operation fails.
pub fn list_relationships_by_namespace(
    conn: &Connection,
    namespace: Option<&str>,
) -> Result<Vec<RelationshipRow>, AppError> {
    if let Some(ns) = namespace {
        let mut stmt = conn.prepare_cached(
            "SELECT r.id, r.namespace, r.source_id, r.target_id, r.relation, r.weight, r.description
             FROM relationships r
             JOIN entities se ON se.id = r.source_id AND se.namespace = ?1
             JOIN entities te ON te.id = r.target_id AND te.namespace = ?1
             ORDER BY r.id",
        )?;
        let rows = stmt
            .query_map(params![ns], |r| {
                Ok(RelationshipRow {
                    id: r.get(0)?,
                    namespace: r.get(1)?,
                    source_id: r.get(2)?,
                    target_id: r.get(3)?,
                    relation: r.get(4)?,
                    weight: r.get(5)?,
                    description: r.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let mut stmt = conn.prepare_cached(
            "SELECT id, namespace, source_id, target_id, relation, weight, description
             FROM relationships ORDER BY id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(RelationshipRow {
                    id: r.get(0)?,
                    namespace: r.get(1)?,
                    source_id: r.get(2)?,
                    target_id: r.get(3)?,
                    relation: r.get(4)?,
                    weight: r.get(5)?,
                    description: r.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// Searches the `entity_embeddings` table for the k nearest neighbours
/// using pure-Rust cosine similarity.
///
/// v1.0.76: sqlite-vec was removed. The full table scan + in-process
/// cosine is O(N × D) per call. For namespaces with more than ~10k
/// entities, the operator should rely on FTS5 (`hybrid-search`) for
/// coarse filtering before reaching this function.
///
/// # Errors
///
/// - [`AppError::Database`] — SQLite query failure.
/// - [`AppError::Embedding`] — invalid or mismatched embedding dimension.
pub fn knn_search(
    conn: &Connection,
    embedding: &[f32],
    namespace: &str,
    k: usize,
) -> Result<Vec<(i64, f32)>, AppError> {
    if embedding.len() != crate::constants::embedding_dim() {
        return Err(AppError::Embedding(
            crate::i18n::validation::embedding_knn_search_dim_mismatch(
                embedding.len(),
                crate::constants::embedding_dim(),
            ),
        ));
    }
    let mut stmt = conn.prepare_cached(
        "SELECT entity_id, embedding FROM entity_embeddings WHERE namespace = ?1",
    )?;
    let mut scored: Vec<(i64, f32)> = stmt
        .query_map(params![namespace], |r| {
            let id: i64 = r.get(0)?;
            let bytes: Vec<u8> = r.get(1)?;
            Ok((id, bytes))
        })?
        .filter_map(|row| {
            row.ok().and_then(|(id, bytes)| {
                let stored = crate::embedder::bytes_to_f32(&bytes);
                if stored.len() != embedding.len() {
                    return None;
                }
                let score = crate::similarity::cosine_similarity(embedding, &stored);
                Some((id, score))
            })
        })
        .collect();
    // `cosine_similarity` returns a value in [-1.0, 1.0]; 1.0 is the
    // best match. Sort descending and truncate to `k`.
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);
    Ok(scored)
}

// GAP-SG-146: test modules named for what they exercise. `test_fixtures`
// holds the schema bootstrap both halves used to duplicate.
#[cfg(test)]
#[path = "entity_crud_tests.rs"]
mod crud_tests;
#[cfg(test)]
#[path = "entity_name_validation_tests.rs"]
mod name_validation_tests;
#[cfg(test)]
#[path = "entity_relationship_tests.rs"]
mod relationship_tests;
#[cfg(test)]
#[path = "entity_test_fixtures.rs"]
mod test_fixtures;
#[cfg(test)]
#[path = "entity_vector_tests.rs"]
mod vector_tests;
