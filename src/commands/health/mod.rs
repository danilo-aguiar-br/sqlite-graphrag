//! Handler for the `health` CLI subcommand.

use crate::errors::AppError;
use crate::output;
use crate::paths::AppPaths;
use crate::storage::connection::open_ro;
use serde::Serialize;
use std::fs;
use std::time::Instant;

mod embed_stats;
mod tables;

use embed_stats::{
    chunk_embedding_health, coverage_pct, entity_embedding_health, llm_slot_info,
    memory_embedding_health,
};
use tables::table_exists;

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Check database health (connectivity, integrity, vector index)\n  \
    sqlite-graphrag health\n\n  \
    # Check health of a database at a custom path\n  \
    sqlite-graphrag health --db /path/to/graphrag.sqlite\n\n  \
    # Explicit database path\n  \
    sqlite-graphrag health --db /data/graphrag.sqlite")]
/// Health args.
pub struct HealthArgs {
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
    /// Explicit JSON flag. Accepted as a no-op because output is already JSON by default.
    #[arg(long, default_value_t = false)]
    pub json: bool,
    /// Output format: `json` or `text`. JSON is always emitted on stdout regardless of the value.
    #[arg(long, value_parser = ["json", "text"], hide = true)]
    pub format: Option<String>,
    /// Filter health report counts to a specific namespace.
    /// When omitted, counts are global (sum across all namespaces).
    /// Global checks (integrity, schema_version, journal_mode) are always reported.
    #[arg(long)]
    pub namespace: Option<String>,
}

/// Health counts.
#[derive(Serialize, schemars::JsonSchema)]
pub struct HealthCounts {
    memories: i64,
    /// Alias of `memories` for the documented contract in SKILL.md.
    memories_total: i64,
    entities: i64,
    relationships: i64,
    vec_memories: i64,
}

/// Health check.
#[derive(Serialize, schemars::JsonSchema)]
pub struct HealthCheck {
    name: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

/// Health response.
#[derive(Serialize, schemars::JsonSchema)]
pub struct HealthResponse {
    status: String,
    /// Namespace filter applied to the counts. None means global (sum across all namespaces).
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace: Option<String>,
    integrity: String,
    integrity_ok: bool,
    schema_ok: bool,
    vec_memories_ok: bool,
    vec_memories_missing: i64,
    vec_memories_orphaned: i64,
    vec_entities_ok: bool,
    /// v1.1.1 (P6a): entities without a row in entity_embeddings/vec_entities.
    /// Completeness (coverage), distinct from the table-existence consistency
    /// reported by `vec_entities_ok`.
    vec_entities_missing: i64,
    vec_chunks_ok: bool,
    /// v1.1.1 (P6a): memory_chunks rows without a row in chunk_embeddings/vec_chunks.
    vec_chunks_missing: i64,
    /// v1.1.1 (P6a): vector coverage percentages in [0.0, 100.0] — fraction of
    /// source rows (active memories / entities / chunks) that have a vector.
    /// 100.0 when there is nothing to cover.
    vec_memories_coverage_pct: f64,
    vec_entities_coverage_pct: f64,
    vec_chunks_coverage_pct: f64,
    fts_ok: bool,
    /// Whether a live FTS5 MATCH query against fts_memories succeeded.
    fts_query_ok: bool,
    model_ok: bool,
    counts: HealthCounts,
    db_path: String,
    db_size_bytes: u64,
    /// MAX(version) from refinery_schema_history — number of the last applied migration.
    /// Distinct from PRAGMA schema_version (SQLite DDL counter) and PRAGMA user_version
    /// (canonical SCHEMA_USER_VERSION from __debug_schema).
    schema_version: u32,
    /// List of entities referenced by memories but absent from the entities table.
    /// Empty in a healthy DB. Per the contract documented in SKILL.md.
    missing_entities: Vec<String>,
    /// WAL file size in MB (0.0 if WAL does not exist or journal_mode != wal).
    wal_size_mb: f64,
    /// SQLite journaling mode (wal, delete, truncate, persist, memory, off).
    journal_mode: String,
    /// SQLite version string, e.g. `"3.46.0"`.
    sqlite_version: String,
    /// Fraction of relationships that use the `mentions` relation type (0.0–1.0).
    /// Omitted when there are no relationships in the database.
    #[serde(skip_serializing_if = "Option::is_none")]
    mentions_ratio: Option<f64>,
    /// Human-readable warning when `mentions` relationships dominate the graph (ratio > 0.5).
    /// Omitted when the ratio is within acceptable bounds or there are no relationships.
    #[serde(skip_serializing_if = "Option::is_none")]
    mentions_warning: Option<String>,
    /// The relation type with the highest edge count in the namespace.
    /// Omitted when there are no relationships in the database.
    #[serde(skip_serializing_if = "Option::is_none")]
    top_relation: Option<String>,
    /// Fraction of all edges occupied by `top_relation` (0.0–1.0).
    /// Omitted when there are no relationships in the database.
    #[serde(skip_serializing_if = "Option::is_none")]
    top_relation_ratio: Option<f64>,
    /// Fraction of relationships that use the `applies_to` relation type (0.0–1.0).
    /// Omitted when there are no relationships or when `applies_to` is absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    applies_to_ratio: Option<f64>,
    /// Human-readable warning when a single relation type occupies more than 40 % of edges.
    /// Omitted when concentration is within acceptable bounds or there are no relationships.
    #[serde(skip_serializing_if = "Option::is_none")]
    relation_concentration_warning: Option<String>,
    /// Number of entities whose name differs from its normalized kebab-case form.
    #[serde(skip_serializing_if = "Option::is_none")]
    non_normalized_count: Option<i64>,
    /// Warning when non-normalized entities are detected.
    #[serde(skip_serializing_if = "Option::is_none")]
    normalization_warning: Option<String>,
    /// Number of entities with degree exceeding the super-hub threshold (default 50).
    #[serde(skip_serializing_if = "Option::is_none")]
    super_hub_count: Option<i64>,
    /// Warning listing top super-hub entity names.
    #[serde(skip_serializing_if = "Option::is_none")]
    super_hub_warning: Option<String>,
    /// Name of the entity with the highest connection count in the namespace.
    /// Omitted when there are no entities in the database.
    #[serde(skip_serializing_if = "Option::is_none")]
    top_hub_entity: Option<String>,
    /// Number of connections (degree) of `top_hub_entity`.
    /// Omitted when there are no entities in the database.
    #[serde(skip_serializing_if = "Option::is_none")]
    top_hub_degree: Option<i64>,
    /// Human-readable warning when `top_hub_entity` exceeds 50 connections.
    /// Omitted when degree is within acceptable bounds or there are no entities.
    #[serde(skip_serializing_if = "Option::is_none")]
    hub_warning: Option<String>,
    /// Total LLM embedding slots available on this host.
    #[serde(skip_serializing_if = "Option::is_none")]
    llm_slots_total: Option<u32>,
    /// LLM embedding slots currently occupied (slot file exists).
    #[serde(skip_serializing_if = "Option::is_none")]
    llm_slots_occupied: Option<u32>,
    /// LLM embedding slots held by dead processes (stale).
    #[serde(skip_serializing_if = "Option::is_none")]
    llm_slots_stale: Option<u32>,
    checks: Vec<HealthCheck>,
    elapsed_ms: u64,
}

/// Run.
pub fn run(args: HealthArgs) -> Result<(), AppError> {
    let start = Instant::now();
    let _ = args.json; // --json is a no-op because output is already JSON by default
    let _ = args.format; // --format is a no-op; JSON is always emitted on stdout
    let paths = AppPaths::resolve(args.db.as_deref())?;
    // GAP-E2E-002: resolve --namespace for counts filtering.
    // Global checks (integrity, schema_version, journal_mode) remain namespace-agnostic.
    let namespace_filter = match args.namespace.as_deref() {
        Some(ns) => Some(crate::namespace::resolve_namespace(Some(ns))?),
        None => None,
    };

    // BUG-AUDIT-1 (v1.0.88): refuse to silently bootstrap an empty database
    // when the operator passes a typo'd or non-existent path. `health` must
    // observe the database as-is, never mutate it.
    if !paths.db.exists() {
        let msg = format!(
            "database not found at {}; `health` does not auto-create the database — \
             run `sqlite-graphrag init --db {}` first or pass an existing path",
            paths.db.display(),
            paths.db.display(),
        );
        tracing::warn!(target: "health", db_path = %paths.db.display(), "database path does not exist; refusing to bootstrap");
        output::emit_json(&serde_json::json!({
            "error": true,
            "code": 4,
            "message": msg,
            "db_path": paths.db.display().to_string(),
        }))?;
        return Err(AppError::NotFound(msg));
    }

    let conn = open_ro(&paths.db)?;

    let integrity: String = conn.query_row("PRAGMA integrity_check;", [], |r| r.get(0))?;
    let integrity_ok = integrity == "ok";
    tracing::info!(target: "health", integrity_ok = %integrity_ok, "PRAGMA integrity_check complete");

    if !integrity_ok {
        let db_size_bytes = fs::metadata(&paths.db).map(|m| m.len()).unwrap_or(0);
        output::emit_json(&HealthResponse {
            status: "degraded".to_string(),
            namespace: None,
            integrity: integrity.clone(),
            integrity_ok: false,
            schema_ok: false,
            vec_memories_ok: false,
            vec_memories_missing: 0,
            vec_memories_orphaned: 0,
            vec_entities_ok: false,
            vec_entities_missing: 0,
            vec_chunks_ok: false,
            vec_chunks_missing: 0,
            vec_memories_coverage_pct: 0.0,
            vec_entities_coverage_pct: 0.0,
            vec_chunks_coverage_pct: 0.0,
            fts_ok: false,
            fts_query_ok: false,
            model_ok: false,
            counts: HealthCounts {
                memories: 0,
                memories_total: 0,
                entities: 0,
                relationships: 0,
                vec_memories: 0,
            },
            db_path: paths.db.display().to_string(),
            db_size_bytes,
            schema_version: 0,
            sqlite_version: "unknown".to_string(),
            missing_entities: vec![],
            wal_size_mb: 0.0,
            journal_mode: "unknown".to_string(),
            mentions_ratio: None,
            mentions_warning: None,
            top_relation: None,
            top_relation_ratio: None,
            applies_to_ratio: None,
            relation_concentration_warning: None,
            non_normalized_count: None,
            normalization_warning: None,
            super_hub_count: None,
            super_hub_warning: None,
            top_hub_entity: None,
            top_hub_degree: None,
            hub_warning: None,
            llm_slots_total: None,
            llm_slots_occupied: None,
            llm_slots_stale: None,
            checks: vec![HealthCheck {
                name: "integrity".to_string(),
                ok: false,
                detail: Some(integrity),
            }],
            elapsed_ms: start.elapsed().as_millis() as u64,
        })?;
        return Err(AppError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
            Some("integrity check failed".to_string()),
        )));
    }

    // GAP-E2E-002: filter memory count by namespace when --namespace is set.
    let memories_count: i64 = match &namespace_filter {
        Some(ns) => conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE deleted_at IS NULL AND namespace = ?1",
            rusqlite::params![ns],
            |r| r.get(0),
        )?,
        None => conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )?,
    };
    let entities_count: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
    let relationships_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))?;
    let (vec_memories_ok, vec_memories_count, vec_memories_missing, vec_memories_orphaned) =
        memory_embedding_health(&conn);

    let mentions_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relationships WHERE relation = 'mentions'",
        [],
        |r| r.get(0),
    )?;
    let (mentions_ratio, mentions_warning) = if relationships_count > 0 {
        let ratio = mentions_count as f64 / relationships_count as f64;
        let warning = if ratio > 0.5 {
            Some(format!(
                "mentions relationships dominate graph at {:.1}% ({}/{} total); consider running prune-relations --relation mentions --dry-run",
                ratio * 100.0,
                mentions_count,
                relationships_count
            ))
        } else {
            None
        };
        (Some(ratio), warning)
    } else {
        (None, None)
    };

    // Relation concentration: find the most frequent relation type and check threshold.
    let (top_relation, top_relation_ratio, applies_to_ratio, relation_concentration_warning) =
        if relationships_count > 0 {
            // Identify the relation with the highest edge count.
            let (top_rel, top_count): (String, i64) = conn
                .query_row(
                    "SELECT relation, COUNT(*) AS cnt
                     FROM relationships
                     GROUP BY relation
                     ORDER BY cnt DESC
                     LIMIT 1",
                    [],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
                )
                .unwrap_or_else(|_| ("unknown".to_string(), 0));

            let top_ratio = top_count as f64 / relationships_count as f64;

            // Compute applies_to ratio separately (may be 0 if absent).
            let applies_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM relationships WHERE relation = 'applies_to'",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let at_ratio = if applies_count > 0 {
                Some(applies_count as f64 / relationships_count as f64)
            } else {
                None
            };

            let concentration_warning = if top_ratio > 0.40 {
                Some(format!(
                    "relation '{}' dominates graph at {:.1}% ({}/{} total); consider running prune-relations --relation {} --dry-run",
                    top_rel,
                    top_ratio * 100.0,
                    top_count,
                    relationships_count,
                    top_rel,
                ))
            } else {
                None
            };

            (
                Some(top_rel),
                Some(top_ratio),
                at_ratio,
                concentration_warning,
            )
        } else {
            (None, None, None, None)
        };

    let status = "ok";

    let schema_version: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM refinery_schema_history",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0) as u32;

    let schema_ok = schema_version > 0;

    // Checks vector tables via sqlite_master (consistency: table exists)
    // and counts source rows without a vector (completeness: coverage).
    let (vec_entities_ok, vec_entities_missing) = entity_embedding_health(&conn);
    let (vec_chunks_ok, vec_chunks_missing) = chunk_embedding_health(&conn);

    // v1.1.1 (P6a): coverage percentages. The memory total is global (the
    // vec_memories_missing count above is namespace-agnostic too).
    let memories_total_global: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memories WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    let chunks_total: i64 = conn
        .query_row("SELECT COUNT(*) FROM memory_chunks", [], |r| r.get(0))
        .unwrap_or(0);
    let vec_memories_coverage_pct =
        coverage_pct(vec_memories_ok, memories_total_global, vec_memories_missing);
    let vec_entities_coverage_pct =
        coverage_pct(vec_entities_ok, entities_count, vec_entities_missing);
    let vec_chunks_coverage_pct = coverage_pct(vec_chunks_ok, chunks_total, vec_chunks_missing);

    tracing::info!(target: "health", vec_memories_ok = %vec_memories_ok, vec_entities_ok = %vec_entities_ok, vec_missing = vec_memories_missing, vec_orphaned = vec_memories_orphaned, "vector table checks complete");
    let fts_ok = table_exists(&conn, "fts_memories");

    // Verifies that FTS5 can execute a MATCH query (catches index corruption distinct from table absence).
    let fts_query_ok = if fts_ok {
        conn.query_row(
            "SELECT COUNT(*) FROM fts_memories WHERE fts_memories MATCH 'a' LIMIT 1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .is_ok()
    } else {
        false
    };

    tracing::info!(target: "health", fts_ok = %fts_ok, fts_query_ok = %fts_query_ok, "FTS5 checks complete");

    // Captures the SQLite runtime version for observability.
    let sqlite_version: String = conn
        .query_row("SELECT sqlite_version()", [], |r| r.get(0))
        .unwrap_or_else(|_| "unknown".to_string());

    // Detects orphan entities referenced by memories but absent from the entities table.
    let mut missing_entities: Vec<String> = Vec::with_capacity(4);
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT me.entity_id
         FROM memory_entities me
         LEFT JOIN entities e ON e.id = me.entity_id
         WHERE e.id IS NULL",
    )?;
    let orphans: Vec<i64> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for id in orphans {
        missing_entities.push(format!("entity_id={id}"));
    }

    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
        .unwrap_or_else(|_| "unknown".to_string());

    let wal_size_mb = fs::metadata(format!("{}-wal", paths.db.display()))
        .map(|m| m.len() as f64 / 1024.0 / 1024.0)
        .unwrap_or(0.0);

    // Database file size in bytes
    let db_size_bytes = fs::metadata(&paths.db).map(|m| m.len()).unwrap_or(0);

    // G46: the ONNX model cache no longer exists in the LLM-only build
    // (v1.0.76+). model_ok now reports whether an LLM CLI (claude or codex)
    // is reachable on PATH — the real prerequisite for embedding generation.
    let model_ok = crate::commands::ingest_claude::find_claude_binary(None).is_ok()
        || crate::commands::ingest_codex::find_codex_binary(None).is_ok();
    tracing::info!(target: "health", model_ok = %model_ok, "LLM CLI availability check complete");

    // Builds the checks array for detailed diagnostics
    let mut checks: Vec<HealthCheck> = Vec::with_capacity(8);

    // At this point integrity_ok is always true (corrupt DB returned early above).
    checks.push(HealthCheck {
        name: "integrity".to_string(),
        ok: true,
        detail: None,
    });

    checks.push(HealthCheck {
        name: "schema_version".to_string(),
        ok: schema_ok,
        detail: if schema_ok {
            None
        } else {
            Some(format!("schema_version={schema_version} (expected >0)"))
        },
    });

    checks.push(HealthCheck {
        name: "vec_memories".to_string(),
        ok: vec_memories_ok,
        detail: if vec_memories_ok {
            None
        } else {
            Some("memory_embeddings/vec_memories table missing from sqlite_master".to_string())
        },
    });

    checks.push(HealthCheck {
        name: "vec_entities".to_string(),
        ok: vec_entities_ok,
        detail: if vec_entities_ok {
            None
        } else {
            Some("entity_embeddings/vec_entities table missing from sqlite_master".to_string())
        },
    });

    checks.push(HealthCheck {
        name: "vec_chunks".to_string(),
        ok: vec_chunks_ok,
        detail: if vec_chunks_ok {
            None
        } else {
            Some("chunk_embeddings/vec_chunks table missing from sqlite_master".to_string())
        },
    });

    checks.push(HealthCheck {
        name: "fts_memories".to_string(),
        ok: fts_ok,
        detail: if fts_ok {
            None
        } else {
            Some("fts_memories table missing from sqlite_master".to_string())
        },
    });

    checks.push(HealthCheck {
        name: "fts_query".to_string(),
        ok: fts_query_ok,
        detail: if fts_query_ok {
            None
        } else {
            Some("FTS5 MATCH query failed — run 'sqlite-graphrag fts rebuild'".to_string())
        },
    });

    checks.push(HealthCheck {
        name: "llm_cli".to_string(),
        ok: model_ok,
        detail: if model_ok {
            None
        } else {
            Some(
                "no LLM CLI found on PATH; install 'claude' (Claude Code) or 'codex' \
                 (Codex CLI) — required for embedding generation since v1.0.76"
                    .to_string(),
            )
        },
    });

    // G24: detect non-normalized entity names
    let (non_normalized_count, normalization_warning) = {
        let mut stmt = conn.prepare_cached("SELECT name FROM entities")?;
        let names: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        let count = names
            .iter()
            .filter(|n| crate::parsers::normalize_entity_name(n) != **n)
            .count() as i64;
        let warning = if count > 0 {
            Some(format!(
                "run 'normalize-entities --yes' to fix {count} non-normalized entities"
            ))
        } else {
            None
        };
        (Some(count), warning)
    };

    // G25: detect super-hub entities (degree > 50)
    let (super_hub_count, super_hub_warning) = {
        let mut stmt = conn.prepare_cached(
            "SELECT e.name, COUNT(r.id) as deg FROM entities e \
             LEFT JOIN relationships r ON e.id = r.source_id OR e.id = r.target_id \
             GROUP BY e.id HAVING deg > 50 ORDER BY deg DESC LIMIT 5",
        )?;
        let hubs: Vec<(String, i64)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        let count = hubs.len() as i64;
        let warning = if count > 0 {
            let names: Vec<String> = hubs
                .iter()
                .map(|(n, d)| format!("{n} (degree {d})"))
                .collect();
            Some(format!("super-hubs detected: {}", names.join(", ")))
        } else {
            None
        };
        (Some(count), warning)
    };

    // G25 (extended): identify the single highest-degree entity for programmatic use.
    let (top_hub_entity, top_hub_degree, hub_warning) = {
        let result: Option<(String, i64)> = conn
            .query_row(
                "SELECT e.name, COUNT(r.id) AS degree
                 FROM entities e
                 LEFT JOIN relationships r ON e.id = r.source_id OR e.id = r.target_id
                 GROUP BY e.id
                 ORDER BY degree DESC
                 LIMIT 1",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
            )
            .ok();
        match result {
            Some((name, degree)) => {
                let warning = if degree > 50 {
                    Some(format!(
                        "entity '{name}' has {degree} connections; consider splitting or using --max-neighbors-per-hop"
                    ))
                } else {
                    None
                };
                (Some(name), Some(degree), warning)
            }
            None => (None, None, None),
        }
    };

    let llm_slots = llm_slot_info();
    let response = HealthResponse {
        status: status.to_string(),
        namespace: namespace_filter.clone(),
        integrity,
        integrity_ok,
        schema_ok,
        vec_memories_ok,
        vec_memories_missing,
        vec_memories_orphaned,
        vec_entities_ok,
        vec_entities_missing,
        vec_chunks_ok,
        vec_chunks_missing,
        vec_memories_coverage_pct,
        vec_entities_coverage_pct,
        vec_chunks_coverage_pct,
        fts_ok,
        fts_query_ok,
        model_ok,
        counts: HealthCounts {
            memories: memories_count,
            memories_total: memories_count,
            entities: entities_count,
            relationships: relationships_count,
            vec_memories: vec_memories_count,
        },
        db_path: paths.db.display().to_string(),
        db_size_bytes,
        schema_version,
        sqlite_version,
        missing_entities,
        wal_size_mb,
        journal_mode,
        mentions_ratio,
        mentions_warning,
        top_relation,
        top_relation_ratio,
        applies_to_ratio,
        relation_concentration_warning,
        non_normalized_count,
        normalization_warning,
        super_hub_count,
        super_hub_warning,
        top_hub_entity,
        top_hub_degree,
        hub_warning,
        llm_slots_total: Some(llm_slots.0),
        llm_slots_occupied: Some(llm_slots.1),
        llm_slots_stale: Some(llm_slots.2),
        checks,
        elapsed_ms: start.elapsed().as_millis() as u64,
    };
    output::emit_json(&response)?;
    Ok(())
}
#[cfg(test)]
#[path = "../health_tests.rs"]
mod tests;
