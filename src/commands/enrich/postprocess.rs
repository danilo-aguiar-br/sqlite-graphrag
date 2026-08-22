//! Persist helpers — write enrichment results to the main DB.

/* wave-c1-imports */
use crate::entity_type::{is_canonical_entity_type, normalize_entity_type};
use crate::errors::AppError;
use crate::storage::entities::{self, NewEntity, NewRelationship};
use crate::storage::memories;
use rusqlite::Connection;
use serde::Deserialize;

/// Persists entity bindings extracted by the LLM for a memory.
///
/// Creates entities via `upsert_entity`, links them to the memory via
/// `link_memory_entity`, and upserts relationships found between entities.
pub(super) fn persist_memory_bindings(
    conn: &Connection,
    namespace: &str,
    memory_id: i64,
    entities_json: &serde_json::Value,
    rels_json: &serde_json::Value,
) -> Result<(usize, usize), AppError> {
    #[derive(Deserialize)]
    struct EntityItem {
        name: String,
        entity_type: String,
    }
    #[derive(Deserialize)]
    struct RelItem {
        source: String,
        target: String,
        relation: String,
        strength: f64,
    }

    // `serde_json::from_value` takes the `Value` BY VALUE, so both calls used to
    // deep-clone the whole extracted document — every entity and every relation,
    // strings included — only to throw the copy away one line later. This runs
    // once per item inside a 16-way fan-out, so the copy was paid 16 times over
    // concurrently. Deserializing from `&Value` reads the same tree in place and
    // still hands back owned `String` fields, which is all the loops below need.
    let extracted_entities: Vec<EntityItem> = Vec::deserialize(entities_json).map_err(|e| {
        AppError::Validation(crate::i18n::validation::failed_to_parse_entities_array(&e))
    })?;

    let extracted_rels: Vec<RelItem> = Vec::deserialize(rels_json).map_err(|e| {
        AppError::Validation(crate::i18n::validation::failed_to_parse_relationships_array(&e))
    })?;

    let mut ent_count = 0usize;
    let mut rel_count = 0usize;

    // Consumed by value: nothing reads the vector after the loop, so each item's
    // `entity_type` can MOVE into `NewEntity` instead of being cloned per entity.
    for item in extracted_entities {
        // v1.2.8: the label is stored as the model wrote it. GAP-SG-47 used to
        // fold anything outside the canonical thirteen onto the nearest kind so
        // no entity was dropped; V017 removed the CHECK, so keeping the label
        // now achieves the same "never discard" goal without destroying the
        // word. `upsert_entity_preserving_type` normalises the SHAPE and is the
        // only place that may refuse — a refusal is already handled below as a
        // per-entity skip rather than a failed item.
        //
        // A non-canonical label is reported here because this drain has no
        // warnings channel: `persist_memory_bindings` answers with two counts
        // and `EnrichItemResult::Done` carries no warnings field, so a log line
        // is the only place the observation can land without changing a
        // signature shared with `extraction_bindings`.
        if let Ok(normalized) = normalize_entity_type(&item.entity_type) {
            if !is_canonical_entity_type(&normalized) {
                tracing::warn!(
                    target: "enrich",
                    entity = %item.name,
                    entity_type = %normalized,
                    "entity type is outside the canonical vocabulary; stored as written"
                );
            }
        }
        // v1.2.8: `_preserving_type` instead of `upsert_entity`. This worker
        // runs after EVERY successful write, so the plain upsert let a model
        // guess overwrite a type the caller had just declared — silently, and
        // after the envelope that reported success had already been read.
        match entities::upsert_entity_preserving_type(
            conn,
            namespace,
            &NewEntity {
                // `name` still has to be cloned: the failure arm below logs it,
                // so it must outlive the borrow handed to the upsert. Only
                // `entity_type` is free to move, and it does.
                name: item.name.clone(),
                entity_type: item.entity_type,
                description: None,
            },
        ) {
            Ok(eid) => {
                let _ = entities::link_memory_entity(conn, memory_id, eid);
                ent_count += 1;
            }
            Err(e) => {
                tracing::warn!(
                    target: "enrich",
                    entity = %item.name,
                    error = %e,
                    "entity upsert skipped"
                );
            }
        }
    }

    // Entities whose edge count changed here, so `degree` can be reconciled once
    // at the end instead of once per edge.
    let mut affected_entity_ids: std::collections::BTreeSet<i64> =
        std::collections::BTreeSet::new();

    // Same reasoning as the entity loop: nothing reads the vector afterwards, so
    // `source` and `target` move into `NewRelationship` instead of being cloned.
    for rel in extracted_rels {
        // GAP-SG-48: rewrite non-canonical relations to canonical instead of
        // accepting them raw with only a warning.
        let normalized = crate::parsers::map_to_canonical_relation(&rel.relation);

        // Normalize entity names before lookup: upsert_entity normalizes on write,
        // so the lookup must use the same normalized form to find the row.
        let src_name = crate::parsers::normalize_entity_name(&rel.source);
        let tgt_name = crate::parsers::normalize_entity_name(&rel.target);
        let src_id = entities::find_entity_id(conn, namespace, &src_name);
        let tgt_id = entities::find_entity_id(conn, namespace, &tgt_name);
        if let (Ok(Some(sid)), Ok(Some(tid))) = (src_id, tgt_id) {
            let new_rel = NewRelationship {
                source: rel.source,
                target: rel.target,
                relation: normalized,
                strength: rel.strength,
                description: None,
            };
            if entities::upsert_relationship(conn, namespace, sid, tid, &new_rel).is_ok() {
                rel_count += 1;
                affected_entity_ids.insert(sid);
                affected_entity_ids.insert(tid);
            }
        }
    }

    // `upsert_relationship` writes the edge and nothing else, so every caller
    // owns the `degree` cache. `ingest::persist` and `remember::run` already
    // reconcile it; this path did not, and it is the path that runs most.
    //
    // The omission fed back on itself. `enrich --operation entity-connect`
    // picks hubs with `ORDER BY degree DESC` and islands with `degree = 0`, both
    // read straight off that cache. Edges written here left it untouched, so the
    // next scan ranked hubs by an undercount and offered as "islands" the very
    // entities this pass had just connected. Measured on a live graph: 6 218 of
    // 15 370 entities carried a stale degree.
    for &eid in &affected_entity_ids {
        if let Err(e) = entities::recalculate_degree(conn, eid) {
            tracing::warn!(
                target: "enrich",
                entity_id = eid,
                error = %e,
                "degree reconciliation skipped"
            );
        }
    }

    Ok((ent_count, rel_count))
}

/// Updates an entity's description directly in the `entities` table, and drops
/// the vector that described the previous text.
///
/// The embedded text of an entity is `"{name} {description}"`, so rewriting the
/// description invalidates the stored vector. Nothing noticed: the re-embed
/// scanner selects on `reembed_entity_predicate`, which asks only whether a BLOB
/// exists at the active dimension — a vector for a description that no longer
/// exists satisfies both conditions perfectly. The row stayed at 100% coverage
/// while its vector answered for text the database had discarded.
///
/// Deleting the row is what makes the existing predicate true again, so the
/// backfill picks the entity up on its next pass without a second mechanism or
/// a new column to mark staleness.
pub(super) fn persist_entity_description(
    conn: &Connection,
    entity_id: i64,
    description: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE entities SET description = ?1, updated_at = unixepoch() WHERE id = ?2",
        rusqlite::params![description, entity_id],
    )?;
    conn.execute(
        "DELETE FROM entity_embeddings WHERE entity_id = ?1",
        rusqlite::params![entity_id],
    )?;
    Ok(())
}

/// One memory row, as the vector writers need it.
///
/// Every field comes out of the same `SELECT`, and three of them are `&str`:
/// `memory_name`, `memory_type` and `body` were adjacent positionals, so
/// transposing name and type wrote a valid row with the wrong payload and no
/// compiler complaint.
#[derive(Clone, Copy)]
pub(super) struct MemoryRowRef<'a> {
    /// Namespace the memory belongs to.
    pub(super) namespace: &'a str,
    /// Primary key in the `memories` table.
    pub(super) memory_id: i64,
    /// Kebab-case unique name of the memory.
    pub(super) memory_name: &'a str,
    /// Memory type discriminator (`decision`, `document`, …).
    pub(super) memory_type: &'a str,
    /// Full body text; the source of the embedding and the snippet.
    pub(super) body: &'a str,
}

/// v1.0.84 (ADR-0042): on successful re-embed, records the active backend
/// into the shared accumulator (`ENRICH_LAST_BACKEND`) so the final
/// `EnrichSummary` can expose `backend_invoked` without changing every
/// caller's signature. Best-effort observability — concurrent enrich runs
/// may race, but `Mutex` keeps the mutation safe.
pub(super) fn reembed_memory_vector(
    conn: &Connection,
    row: MemoryRowRef<'_>,
    paths: &crate::paths::AppPaths,
    backends: crate::cli::BackendChoice,
) -> Result<(), AppError> {
    let MemoryRowRef {
        namespace,
        memory_id,
        memory_name,
        memory_type,
        body,
    } = row;
    // This text never reaches a model: it is handed to `memories::upsert_vec`,
    // which takes it as `_snippet` and discards it. The log-preview budget is
    // the right one because it is the only role that is not model input, and
    // it also keeps the width unchanged — widening a payload nothing reads
    // would be a cost with no reader.
    let snippet: String = body
        .chars()
        .take(crate::constants::ENRICH_BODY_LOG_PREVIEW_CHARS)
        .collect();
    // v1.0.82 (GAP-003): forward --llm-backend to embed_with_fallback.
    // v1.0.84 (ADR-0042): tuple (Vec<f32>, LlmBackendKind) — extrai o
    // backend that actually ran; populate the accumulator for the
    // EnrichSummary agregado.
    // v1.0.93 (GAP-OR-PROPAGATION): honour --embedding-backend openrouter.
    let (embedding, backend_kind) =
        crate::embedder::embed_passage_with_embedding_choice(&paths.models, body, backends)?;
    record_enrich_backend(backend_kind.as_str());
    memories::upsert_vec(
        conn,
        memory_id,
        namespace,
        memory_type,
        &embedding,
        memory_name,
        &snippet,
    )?;
    Ok(())
}

/// v1.0.84 (ADR-0042): process-local accumulator of the last LLM backend
/// that successfully ran a re-embed during the current enrich invocation.
/// Read by `run` once at summary emission. Scoped to a single process —
/// cross-process enrichment is gated by the per-namespace singleton, so
/// there is no concurrency hazard across DBs.
pub(super) fn record_enrich_backend(backend: &'static str) {
    if let Ok(mut guard) = ENRICH_LAST_BACKEND.lock() {
        *guard = Some(backend);
    }
}

pub(super) fn take_enrich_backend() -> Option<&'static str> {
    ENRICH_LAST_BACKEND.lock().ok().and_then(|mut g| g.take())
}

pub(super) static ENRICH_LAST_BACKEND: std::sync::Mutex<Option<&'static str>> =
    std::sync::Mutex::new(None);

/// Persists an enriched memory body (body-enrich, GAP-18).
///
/// Uses `memories::update` to set the new body and `sync_fts_after_update`
/// to keep FTS5 in sync. Also re-embeds the memory for recall accuracy.
pub(super) fn persist_enriched_body(
    conn: &Connection,
    namespace: &str,
    memory_id: i64,
    memory_name: &str,
    new_body: &str,
    paths: &crate::paths::AppPaths,
    backends: crate::cli::BackendChoice,
) -> Result<(), AppError> {
    // Read current values for FTS sync
    let (old_name, old_desc, old_body): (String, String, String) = conn.query_row(
        "SELECT name, COALESCE(description,''), COALESCE(body,'') FROM memories WHERE id=?1",
        rusqlite::params![memory_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    let memory_type: String = conn.query_row(
        "SELECT type FROM memories WHERE id=?1",
        rusqlite::params![memory_id],
        |r| r.get(0),
    )?;

    let description: String = conn.query_row(
        "SELECT COALESCE(description,'') FROM memories WHERE id=?1",
        rusqlite::params![memory_id],
        |r| r.get(0),
    )?;

    let body_hash = blake3::hash(new_body.as_bytes()).to_hex().to_string();

    let new_memory = memories::NewMemory {
        namespace: namespace.to_string(),
        name: memory_name.to_string(),
        memory_type: memory_type.clone(),
        description: description.clone(),
        body: new_body.to_string(),
        body_hash,
        session_id: None,
        source: "agent".to_string(),
        metadata: serde_json::json!({
            "operation": "body-enrich",
            "orig_chars": old_body.chars().count(),
            "new_chars": new_body.chars().count(),
        }),
    };

    // G29 audit: insert a new immutable version BEFORE the update so the
    // enriched body is reachable through `history --name <X>` and
    // `restore --version N` can roll back to the pre-enrich state.
    let next_version = crate::storage::versions::next_version(conn, memory_id)?;
    let version_metadata = serde_json::json!({
        "operation": "body-enrich",
        "orig_chars": old_body.chars().count(),
        "new_chars": new_body.chars().count(),
    })
    .to_string();
    crate::storage::versions::insert_version(
        conn,
        memory_id,
        next_version,
        memory_name,
        &memory_type,
        &description,
        new_body,
        &version_metadata,
        Some("enrich"),
        "edit",
    )?;

    memories::update(conn, memory_id, &new_memory, None)?;
    memories::sync_fts_after_update(
        conn,
        memory_id,
        &old_name,
        &old_desc,
        &old_body,
        &new_memory.name,
        &new_memory.description,
        &new_memory.body,
    )?;

    // Re-embed for recall accuracy
    if let Err(e) = reembed_memory_vector(
        conn,
        MemoryRowRef {
            namespace,
            memory_id,
            memory_name,
            memory_type: &memory_type,
            body: new_body,
        },
        paths,
        backends,
    ) {
        tracing::warn!(target: "enrich", memory = %memory_name, error = %e, "vec upsert failed after body-enrich");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        // GAP-SG-119: use DEFAULT_EMBEDDING_DIM (not legacy 384) for new fixtures.
        let dim = crate::constants::DEFAULT_EMBEDDING_DIM;
        conn.execute_batch(&format!(
            "CREATE TABLE memories (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace   TEXT NOT NULL DEFAULT 'global',
                name        TEXT NOT NULL,
                type        TEXT NOT NULL DEFAULT 'note',
                description TEXT NOT NULL DEFAULT '',
                body        TEXT NOT NULL DEFAULT '',
                body_hash   TEXT NOT NULL DEFAULT '',
                session_id  TEXT,
                source      TEXT NOT NULL DEFAULT 'agent',
                metadata    TEXT NOT NULL DEFAULT '{{}}',
                created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                deleted_at  INTEGER,
                UNIQUE(namespace, name)
            );
            CREATE TABLE entities (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace   TEXT NOT NULL DEFAULT 'global',
                name        TEXT NOT NULL,
                type        TEXT NOT NULL DEFAULT 'concept',
                description TEXT,
                degree      INTEGER NOT NULL DEFAULT 0,
                created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                UNIQUE(namespace, name)
            );
            CREATE TABLE memory_entities (
                memory_id  INTEGER NOT NULL,
                entity_id  INTEGER NOT NULL,
                PRIMARY KEY (memory_id, entity_id)
            );
            CREATE TABLE relationships (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace  TEXT NOT NULL DEFAULT 'global',
                source_id  INTEGER NOT NULL,
                target_id  INTEGER NOT NULL,
                relation   TEXT NOT NULL,
                weight     REAL NOT NULL DEFAULT 0.5,
                description TEXT,
                UNIQUE(source_id, target_id, relation)
            );
            CREATE TABLE memory_embeddings (
                memory_id   INTEGER PRIMARY KEY,
                namespace   TEXT NOT NULL,
                embedding   BLOB NOT NULL,
                source      TEXT NOT NULL,
                model       TEXT NOT NULL DEFAULT '',
                dim         INTEGER NOT NULL DEFAULT {dim},
                created_at  INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE entity_embeddings (
                entity_id   INTEGER PRIMARY KEY,
                namespace   TEXT NOT NULL,
                embedding   BLOB NOT NULL,
                source      TEXT NOT NULL,
                model       TEXT NOT NULL DEFAULT '',
                dim         INTEGER NOT NULL DEFAULT {dim},
                created_at  INTEGER NOT NULL DEFAULT (unixepoch())
            );",
        ))
        .expect("schema creation must succeed");
        conn
    }

    /// `upsert_relationship` writes the edge and nothing else, so every caller
    /// owns the `degree` cache. This path did not reconcile it, and it is the
    /// path that runs most.
    ///
    /// The omission fed back on itself: `entity-connect` picks hubs with
    /// `ORDER BY degree DESC` and islands with `degree = 0`, both read off that
    /// cache, so a pass would offer as "islands" the entities it had just
    /// connected. Measured on a live graph, 6 218 of 15 370 entities carried a
    /// stale degree.
    #[test]
    fn persisted_relationships_reconcile_the_degree_cache() {
        let conn = open_test_db();
        conn.execute("INSERT INTO memories (name) VALUES ('m')", [])
            .unwrap();
        let memory_id: i64 = conn
            .query_row("SELECT id FROM memories WHERE name='m'", [], |r| r.get(0))
            .unwrap();

        let entities_json = serde_json::json!([
            {"name": "hub", "entity_type": "concept"},
            {"name": "leaf-a", "entity_type": "concept"},
            {"name": "leaf-b", "entity_type": "concept"},
        ]);
        let rels_json = serde_json::json!([
            {"source": "hub", "target": "leaf-a", "relation": "uses", "strength": 0.8},
            {"source": "hub", "target": "leaf-b", "relation": "uses", "strength": 0.8},
        ]);

        let (_ents, rels) =
            persist_memory_bindings(&conn, "global", memory_id, &entities_json, &rels_json)
                .unwrap();
        assert_eq!(
            rels, 2,
            "both edges must persist for the check to mean anything"
        );

        let stored: i64 = conn
            .query_row("SELECT degree FROM entities WHERE name='hub'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let real: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM relationships r \
                 JOIN entities e ON e.id = r.source_id OR e.id = r.target_id \
                 WHERE e.name = 'hub'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            stored, real,
            "the stored degree ({stored}) must match the edges actually written ({real}); \
             leaving it at zero is what made entity-connect treat connected entities as islands"
        );
    }

    #[test]
    fn persist_entity_description_updates_db() {
        let conn = open_test_db();
        conn.execute(
            "INSERT INTO entities (namespace, name, type) VALUES ('global', 'tokio-runtime', 'tool')",
            [],
        )
        .unwrap();
        let eid: i64 = conn
            .query_row(
                "SELECT id FROM entities WHERE name='tokio-runtime'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        // A vector describing the OLD text, so the staleness is observable.
        conn.execute(
            "INSERT INTO entity_embeddings (entity_id, namespace, embedding, source, model)
             VALUES (?1, 'global', X'00', 'test', 'test')",
            rusqlite::params![eid],
        )
        .unwrap();

        persist_entity_description(&conn, eid, "Async runtime for Rust applications").unwrap();

        // The embedded text is `"{name} {description}"`, so a rewritten
        // description leaves the old vector answering for text that no longer
        // exists. Dropping the row is what makes `reembed_entity_predicate`
        // true again, so the backfill reclaims the entity on its next pass.
        let vectors_left: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_embeddings WHERE entity_id = ?1",
                rusqlite::params![eid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            vectors_left, 0,
            "rewriting the description must invalidate the vector that described the old text"
        );

        let desc: String = conn
            .query_row(
                "SELECT description FROM entities WHERE id=?1",
                rusqlite::params![eid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(desc, "Async runtime for Rust applications");
    }
}
