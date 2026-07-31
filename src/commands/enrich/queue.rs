//! Enrichment queue — SQLite-backed scan/retry/dead-letter DB.

use super::args::EnrichOperation;
use crate::errors::AppError;
use rusqlite::Connection;
use serde::Serialize;

// ---------------------------------------------------------------------------
// Queue DB
// ---------------------------------------------------------------------------

/// Opens or creates the enrichment queue database (`.enrich-queue.sqlite`).
///
/// # Schema note (GAP-SG-121)
///
/// This schema is **not** the same product as the ingest sidecars
/// (`.ingest-queue.sqlite` in `ingest_claude` / `ingest_codex`), which key on
/// `file_path` for file-progress tracking. Shared connection setup lives in
/// [`crate::pragmas::apply_sidecar_queue_pragmas`]; table DDL stays separate.
///
/// GAP-SG-95: namespace-scoped queue with ternary UNIQUE
/// `(namespace, operation, item_key)`. Fresh DBs get the final schema;
/// legacy sidecars are rebuilt below when `namespace` is missing.
pub(crate) fn open_queue_db<P: AsRef<std::path::Path>>(path: P) -> Result<Connection, AppError> {
    let conn = Connection::open(path)?;
    crate::pragmas::apply_sidecar_queue_pragmas(&conn)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS queue (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            namespace   TEXT NOT NULL DEFAULT '',
            item_key    TEXT NOT NULL,
            item_type   TEXT NOT NULL DEFAULT 'memory',
            status      TEXT NOT NULL DEFAULT 'pending',
            memory_id   INTEGER,
            entity_id   INTEGER,
            entities    INTEGER DEFAULT 0,
            rels        INTEGER DEFAULT 0,
            error       TEXT,
            cost_usd    REAL DEFAULT 0.0,
            attempt     INTEGER DEFAULT 0,
            elapsed_ms  INTEGER,
            created_at  TEXT DEFAULT (datetime('now')),
            done_at     TEXT,
            error_class TEXT,
            next_retry_at TEXT,
            operation   TEXT,
            finish_reason TEXT,
            input_tokens INTEGER,
            output_tokens INTEGER,
            claimed_at  INTEGER,
            priority    INTEGER NOT NULL DEFAULT 0,
            UNIQUE (namespace, operation, item_key)
        );
        CREATE INDEX IF NOT EXISTS idx_enrich_queue_status ON queue(status);",
    )?;
    // GAP-ENRICH-BACKLOG-CONVERGE (v1.0.96): dead-letter columns. The legacy
    // `.enrich-queue.sqlite` predates these columns and `CREATE TABLE IF NOT
    // EXISTS` never alters an existing table, so add them idempotently here.
    let mut has_error_class = false;
    let mut has_next_retry_at = false;
    // GAP-SG-12/42: the `operation` column scopes queue rows to the enrich
    // operation that enqueued them, so `--status` can segment counts per
    // operation instead of conflating a shared `item_key` space. Migrated
    // idempotently here for the same reason as the v1.0.96 columns.
    let mut has_operation = false;
    // GAP-SG-72: dead-letter diagnostics carried from a typed OpenRouter
    // `ChatError` (finish_reason + token counts) so `--list-dead` can show
    // WHY an item died (e.g. truncated by max_tokens) instead of only the
    // formatted error string. Migrated idempotently for the same reason as
    // the columns above.
    let mut has_finish_reason = false;
    let mut has_input_tokens = false;
    let mut has_output_tokens = false;
    // v1.1.2 (Bug 4): `claimed_at` carries the unixepoch timestamp of the last
    // dequeue claim. `--reset-stale-claims` and the run-startup sweep use it to
    // reset rows stuck in `processing` after a kill -9 (the schema predates this
    // column, so migrate idempotently like the other dead-letter columns).
    let mut has_claimed_at = false;
    // GAP-CLI-PRIO-03: priority column (hot > normal). Higher values claim first.
    let mut has_priority = false;
    // GAP-SG-95: namespace column for multi-namespace isolation.
    let mut has_namespace = false;
    {
        let mut stmt = conn.prepare("PRAGMA table_info(queue)")?;
        let names = stmt.query_map([], |r| r.get::<_, String>(1))?;
        for name in names {
            match name?.as_str() {
                "error_class" => has_error_class = true,
                "next_retry_at" => has_next_retry_at = true,
                "operation" => has_operation = true,
                "finish_reason" => has_finish_reason = true,
                "input_tokens" => has_input_tokens = true,
                "output_tokens" => has_output_tokens = true,
                "claimed_at" => has_claimed_at = true,
                "priority" => has_priority = true,
                "namespace" => has_namespace = true,
                _ => {}
            }
        }
    }
    if !has_error_class {
        conn.execute_batch("ALTER TABLE queue ADD COLUMN error_class TEXT")?;
    }
    if !has_next_retry_at {
        conn.execute_batch("ALTER TABLE queue ADD COLUMN next_retry_at TEXT")?;
    }
    if !has_operation {
        conn.execute_batch("ALTER TABLE queue ADD COLUMN operation TEXT")?;
    }
    if !has_finish_reason {
        conn.execute_batch("ALTER TABLE queue ADD COLUMN finish_reason TEXT")?;
    }
    if !has_input_tokens {
        conn.execute_batch("ALTER TABLE queue ADD COLUMN input_tokens INTEGER")?;
    }
    if !has_output_tokens {
        conn.execute_batch("ALTER TABLE queue ADD COLUMN output_tokens INTEGER")?;
    }
    if !has_claimed_at {
        conn.execute_batch("ALTER TABLE queue ADD COLUMN claimed_at INTEGER")?;
    }
    if !has_priority {
        conn.execute_batch("ALTER TABLE queue ADD COLUMN priority INTEGER NOT NULL DEFAULT 0")?;
    }
    // GAP-CLI-QISO-01: rows with NULL operation predate per-op claim. Tag them
    // LegacyUnscoped so they are never claimable by a named operation drain
    // (fail-safe: operator re-scans to repopulate with the correct op label).
    conn.execute(
        "UPDATE queue SET operation = 'LegacyUnscoped' WHERE operation IS NULL OR operation = ''",
        [],
    )?;
    // GAP-SG-95: SQLite cannot ADD a composite UNIQUE via ALTER. When the
    // legacy global UNIQUE(item_key) schema is still present, rebuild the
    // table with UNIQUE(namespace, operation, item_key).
    if !has_namespace {
        migrate_queue_add_namespace(&conn)?;
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_enrich_queue_eligible ON queue(status, next_retry_at);
         CREATE INDEX IF NOT EXISTS idx_enrich_queue_operation ON queue(operation, status);
         CREATE INDEX IF NOT EXISTS idx_enrich_queue_memory ON queue(memory_id);
         CREATE INDEX IF NOT EXISTS idx_enrich_queue_priority ON queue(status, priority DESC, id);
         CREATE INDEX IF NOT EXISTS idx_enrich_queue_ns ON queue(namespace, operation, status)",
    )?;
    Ok(conn)
}

/// Rebuild `queue` with a `namespace` column and ternary UNIQUE
/// `(namespace, operation, item_key)`. Legacy rows get `namespace = ''`.
fn migrate_queue_add_namespace(conn: &Connection) -> Result<(), AppError> {
    tracing::info!(target: "enrich", "migrating enrich queue to namespace-scoped UNIQUE");
    conn.execute_batch(
        "BEGIN;
         CREATE TABLE queue_v120 (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            namespace   TEXT NOT NULL DEFAULT '',
            item_key    TEXT NOT NULL,
            item_type   TEXT NOT NULL DEFAULT 'memory',
            status      TEXT NOT NULL DEFAULT 'pending',
            memory_id   INTEGER,
            entity_id   INTEGER,
            entities    INTEGER DEFAULT 0,
            rels        INTEGER DEFAULT 0,
            error       TEXT,
            cost_usd    REAL DEFAULT 0.0,
            attempt     INTEGER DEFAULT 0,
            elapsed_ms  INTEGER,
            created_at  TEXT DEFAULT (datetime('now')),
            done_at     TEXT,
            error_class TEXT,
            next_retry_at TEXT,
            operation   TEXT,
            finish_reason TEXT,
            input_tokens INTEGER,
            output_tokens INTEGER,
            claimed_at  INTEGER,
            priority    INTEGER NOT NULL DEFAULT 0,
            UNIQUE (namespace, operation, item_key)
         );
         INSERT OR IGNORE INTO queue_v120 (
            id, namespace, item_key, item_type, status, memory_id, entity_id,
            entities, rels, error, cost_usd, attempt, elapsed_ms, created_at,
            done_at, error_class, next_retry_at, operation, finish_reason,
            input_tokens, output_tokens, claimed_at, priority
         )
         SELECT
            id, '', item_key, item_type, status, memory_id, entity_id,
            entities, rels, error, cost_usd, attempt, elapsed_ms, created_at,
            done_at, error_class, next_retry_at,
            COALESCE(NULLIF(operation, ''), 'LegacyUnscoped'),
            finish_reason, input_tokens, output_tokens, claimed_at,
            COALESCE(priority, 0)
         FROM queue;
         DROP TABLE queue;
         ALTER TABLE queue_v120 RENAME TO queue;
         COMMIT;",
    )
    .map_err(|e| {
        AppError::Validation(crate::i18n::validation::queue_namespace_migration_failed(
            &e,
        ))
    })?;
    Ok(())
}

/// Priority level for hot-set entity-descriptions after `remember`
/// (GAP-CLI-PRIO-03). Higher values are claimed first.
pub const PRIORITY_HOT: i64 = 100;

/// Count pending queue rows at or above `min_priority` for an operation + namespace.
///
/// CAPA-G (2026-07-30): filter by `namespace` so HOT preemption for one ns does
/// not see pending rows belonging to another namespace on a shared sidecar.
pub(super) fn count_priority_pending(
    queue_conn: &Connection,
    operation: &str,
    namespace: &str,
    min_priority: i64,
) -> Result<i64, rusqlite::Error> {
    // GAP-SG-97: status is a string literal — must be quoted.
    // Strict operation + namespace (aligned with dequeue_next_pending).
    queue_conn.query_row(
        "SELECT COUNT(*) FROM queue \
         WHERE status='pending' \
           AND operation = ?1 \
           AND namespace = ?2 \
           AND COALESCE(priority, 0) >= ?3",
        rusqlite::params![operation, namespace, min_priority],
        |r| r.get(0),
    )
}

/// GAP-SG-12: enqueue one scan candidate, linking it to its `memory_id` and
/// tagging it with the originating `operation`. For memory-keyed operations the
/// id is resolved from `main_conn` so the cascade cleanup (GAP-SG-13) can target
/// the queue row by `memory_id` even before the item is processed. Entity/id
/// keyed operations leave `memory_id` NULL (the `item_key` carries the link).
/// `INSERT OR IGNORE` preserves the v1.0.96 invariant that a dead-letter row is
/// never resurrected by re-enqueue (item_key is UNIQUE).
///
/// v1.1.2 (Bug 4, D5): batch callers wrap a `queue_conn.transaction()` and pass
/// `&tx` here (`Transaction` derefs to `Connection`), so hundreds of candidates
/// commit in a single fsync instead of one-per-statement.
pub(super) fn enqueue_candidate(
    queue_conn: &Connection,
    main_conn: &Connection,
    namespace: &str,
    key: &str,
    item_type: &str,
    operation: &str,
) {
    // G-PR-8 / GAP-SG-102: refuse to enqueue keys that do not exist in the
    // target namespace (prevents multi-ns residual and CAPA6 dependency).
    let memory_id: Option<i64> = if item_type == "memory" {
        match main_conn.query_row(
            "SELECT id FROM memories WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
            rusqlite::params![namespace, key],
            |r| r.get(0),
        ) {
            Ok(id) => Some(id),
            Err(_) => {
                tracing::warn!(
                    target: "enrich",
                    namespace,
                    key,
                    "enqueue rejected: memory not found in namespace"
                );
                return;
            }
        }
    } else if item_type == "entity" {
        // v1.1.1 (P2) re-embed enqueues `entity:{name}` keys so drain can
        // dispatch (`call_reembed` strips the prefix). Lookup must use the
        // bare entity name — matching against the prefixed key rejects every
        // candidate ("entity not found in namespace") and leaves the ~14k
        // entity_embeddings residual stuck forever.
        let entity_name = key.strip_prefix("entity:").unwrap_or(key);
        match main_conn.query_row(
            "SELECT id FROM entities WHERE namespace=?1 AND name=?2",
            rusqlite::params![namespace, entity_name],
            |r| r.get::<_, i64>(0),
        ) {
            Ok(_) => None,
            Err(_) => {
                tracing::warn!(
                    target: "enrich",
                    namespace,
                    key,
                    "enqueue rejected: entity not found in namespace"
                );
                return;
            }
        }
    } else if item_type == "chunk" {
        // CAPA: never trust the scanner alone for chunk keys — residual
        // empty-ns / cross-ns queue rows used to land here and then fail at
        // drain with HardFailure + circuit breaker.
        let chunk_key = key.strip_prefix("chunk:").unwrap_or(key);
        let Ok(chunk_id) = chunk_key.parse::<i64>() else {
            tracing::warn!(
                target: "enrich",
                namespace,
                key,
                "enqueue rejected: invalid chunk id in re-embed key"
            );
            return;
        };
        match main_conn.query_row(
            "SELECT c.id FROM memory_chunks c
             JOIN memories m ON m.id = c.memory_id
             WHERE c.id = ?1 AND m.namespace = ?2 AND m.deleted_at IS NULL",
            rusqlite::params![chunk_id, namespace],
            |r| r.get::<_, i64>(0),
        ) {
            Ok(_) => None,
            Err(_) => {
                tracing::warn!(
                    target: "enrich",
                    namespace,
                    key,
                    "enqueue rejected: chunk not found in namespace"
                );
                return;
            }
        }
    } else {
        // entity_pair / other prefixed keys: trust the scanner; still scope by ns.
        None
    };
    if let Err(e) = queue_conn.execute(
        "INSERT OR IGNORE INTO queue \
         (namespace, item_key, item_type, status, operation, memory_id, priority) \
         VALUES (?1, ?2, ?3, 'pending', ?4, ?5, 0)",
        rusqlite::params![namespace, key, item_type, operation, memory_id],
    ) {
        tracing::warn!(target: "enrich", error = %e, "queue insert failed");
    }
}

/// Enqueue an entity-keyed candidate with explicit priority (GAP-CLI-PRIO-02/03).
pub(super) fn enqueue_candidate_with_priority(
    queue_conn: &Connection,
    key: &str,
    item_type: &str,
    operation: &str,
    priority: i64,
) {
    // Priority hot-path historically lacked namespace; store under '' so the
    // ternary UNIQUE still applies (callers that know the ns should use
    // enqueue_candidate after validating the entity).
    if let Err(e) = queue_conn.execute(
        "INSERT OR IGNORE INTO queue \
         (namespace, item_key, item_type, status, operation, memory_id, priority) \
         VALUES ('', ?1, ?2, 'pending', ?3, NULL, ?4)",
        rusqlite::params![key, item_type, operation, priority],
    ) {
        tracing::warn!(target: "enrich", error = %e, "priority queue insert failed");
    } else {
        // If the row already existed as pending with lower priority, bump it.
        let _ = queue_conn.execute(
            "UPDATE queue SET priority = MAX(COALESCE(priority, 0), ?2), status = CASE \
                WHEN status IN ('done','skipped','dead') THEN status ELSE 'pending' END \
             WHERE item_key = ?1 AND COALESCE(priority, 0) < ?2",
            rusqlite::params![key, priority],
        );
    }
}

/// v1.1.2 (Bug 4): reset `processing` rows whose `claimed_at` is older than
/// `max_age_secs`. A row stuck in `processing` after a kill -9 never clears its
/// claim (no heartbeat, no done_at), so a subsequent run never re-selects it and
/// the backlog appears permanently drained. Returns the number of rows reset to
/// `pending`.
pub fn reset_stale_processing_claims(
    conn: &Connection,
    max_age_secs: u64,
) -> Result<usize, AppError> {
    let reset = conn.execute(
        "UPDATE queue SET status='pending', claimed_at=NULL \
         WHERE status='processing' AND claimed_at IS NOT NULL \
         AND CAST(strftime('%s','now') AS INTEGER) - claimed_at > ?1",
        rusqlite::params![max_age_secs as i64],
    )?;
    Ok(reset)
}

/// v1.1.2 (Bug 4): refresh `claimed_at` on the currently-processed row so a slow
/// LLM call (body-enrich can take 60s+) is not mistaken for a stale claim by a
/// concurrent sweep. Called by the worker loop right after the dequeue claim.
pub fn heartbeat(conn: &Connection, queue_id: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE queue SET claimed_at = CAST(strftime('%s','now') AS INTEGER) WHERE id = ?1",
        rusqlite::params![queue_id],
    )?;
    Ok(())
}

/// GAP-SG-69: item_keys vetoed `status='skipped'` for an operation. The
/// body-enrich scan selects candidates purely by `LENGTH(body) <
/// min_output_chars`, so a short body whose rewrite the preservation guard keeps
/// rejecting would be re-scanned every pass and `--until-empty` would never
/// converge. Callers exclude these keys so the scan returns only actionable
/// items; `cleanup_queue_entry` clears the veto when the body actually changes,
/// restoring the memory as a candidate.
pub(super) fn skipped_item_keys(
    conn: &Connection,
    operation: &str,
) -> Result<std::collections::HashSet<String>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT item_key FROM queue WHERE status='skipped' AND (operation = ?1 OR operation IS NULL)",
    )?;
    let keys = stmt
        .query_map(rusqlite::params![operation], |r| r.get::<_, String>(0))?
        .collect::<Result<std::collections::HashSet<String>, _>>()?;
    Ok(keys)
}

/// Queue `item_type` for an operation: entity-keyed operations use `"entity"`,
/// entity-pair operations use `"entity_pair"`, every other (memory/id-keyed)
/// operation uses `"memory"`.
pub(super) fn item_type_for(operation: &EnrichOperation) -> &'static str {
    match operation {
        EnrichOperation::EntityDescriptions => "entity",
        // v1.1.06: entity-connect enqueues `pair:{id1}:{id2}` keys — never
        // treat them as memory names (prune_dead_orphans only reaps memory).
        EnrichOperation::EntityConnect | EnrichOperation::CrossDomainBridges => "entity_pair",
        _ => "memory",
    }
}

/// v1.1.1 (P2): per-key `item_type` override for the re-embed targets.
///
/// Re-embed keys are prefixed with `entity:` / `chunk:` when `--target`
/// selects a non-memory table; the queue row must carry the real item type
/// so `prune_dead_orphans` (which only reaps `item_type='memory'` rows)
/// never mistakes an entity/chunk key for an orphaned memory name.
/// Unprefixed keys keep the operation-level default.
///
/// v1.1.06: `pair:{id1}:{id2}` → `"entity_pair"`.
pub(super) fn item_type_for_key(key: &str, default: &'static str) -> &'static str {
    if key.starts_with("pair:") {
        "entity_pair"
    } else if key.starts_with("entity:") {
        "entity"
    } else if key.starts_with("chunk:") {
        "chunk"
    } else {
        default
    }
}

/// GAP-SG-13: remove a memory's enrich-queue entry when the memory is deleted or
/// force-merged, so the dead-letter / pending sidecar never references a row
/// that no longer exists. Best-effort and a no-op when the queue file is absent
/// (the common case after a clean run, which removes it). Targets BOTH
/// `memory_id` (populated at enqueue for memory ops, GAP-SG-12) and `item_key`
/// (the memory name) so pending rows enqueued before id resolution are also
/// cleared. Errors are logged, never propagated — cleanup must not fail the
/// caller's delete/upsert.
pub fn cleanup_queue_entry(db_path: &std::path::Path, memory_id: i64, name: &str) {
    let queue_path = crate::paths::sidecar_path(db_path, ".enrich-queue.sqlite");
    if !queue_path.exists() {
        return;
    }
    match open_queue_db(&queue_path) {
        Ok(conn) => {
            if let Err(e) = conn.execute(
                "DELETE FROM queue WHERE memory_id = ?1 OR item_key = ?2",
                rusqlite::params![memory_id, name],
            ) {
                tracing::warn!(target: "enrich", error = %e, memory_id, "enrich-queue cleanup failed");
            }
        }
        Err(e) => {
            tracing::warn!(target: "enrich", error = %e, "enrich-queue cleanup skipped (open failed)");
        }
    }
}

/// GAP-SG-66: prune ORPHAN dead-letter rows — `status='dead'` memory rows whose
/// `item_key` (the memory name) no longer exists in the main DB for `namespace`.
///
/// These are terminal "not found" failures (the memory was renamed/purged after
/// being enqueued): re-processing them just re-fails with the same not-found
/// error, so `--requeue-dead` can never recover them and they inflate
/// `queue_dead` forever. Read-only on the main DB; deletes only the
/// confirmed-orphan rows from the queue sidecar. Entity-keyed dead rows
/// (`item_type='entity'`) are left untouched — their key is an entity name, not
/// a memory name. Returns the number of rows pruned.
pub(super) fn prune_dead_orphans(
    queue_conn: &Connection,
    main_conn: &Connection,
    operation: &str,
    namespace: &str,
) -> Result<i64, AppError> {
    let dead: Vec<(i64, String)> = {
        let mut stmt = queue_conn.prepare(
            "SELECT id, item_key FROM queue \
             WHERE status='dead' AND item_type='memory' \
             AND (operation = ?1 OR operation IS NULL) ORDER BY id",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![operation], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let mut pruned = 0_i64;
    for (id, name) in dead {
        let exists = main_conn
            .query_row(
                "SELECT 1 FROM memories WHERE namespace=?1 AND name=?2 AND deleted_at IS NULL",
                rusqlite::params![namespace, name],
                |_| Ok(()),
            )
            .is_ok();
        if !exists {
            queue_conn.execute("DELETE FROM queue WHERE id=?1", rusqlite::params![id])?;
            pruned += 1;
        }
    }
    if pruned > 0 {
        let _ = queue_conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }
    Ok(pruned)
}

/// v1.1.2: prune dead ENTITY orphan rows — remove every `status='dead'`
/// `item_type='entity'` row from the queue sidecar. Unlike
/// [`prune_dead_orphans`], this does NOT consult the main DB: entity dead rows
/// are terminal artifacts of re-extraction/rename and have no recovery path
/// (re-running them re-fails the same way). Returns the number of rows pruned.
pub(super) fn prune_dead_entity_orphans(
    queue_conn: &Connection,
    operation: &str,
) -> Result<i64, AppError> {
    let pruned = queue_conn.execute(
        "DELETE FROM queue \
         WHERE status='dead' AND item_type='entity' \
         AND (operation = ?1 OR operation IS NULL)",
        rusqlite::params![operation],
    )? as i64;
    if pruned > 0 {
        let _ = queue_conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }
    Ok(pruned)
}

// ---------------------------------------------------------------------------
// GAP-ENRICH-BACKLOG-CONVERGE — dead-letter classification + queue failure sink
// ---------------------------------------------------------------------------

/// Read-only `enrich --status` report (no LLM, no singleton).
///
/// GAP-SG-42: all queue counts are scoped to the current `--operation` (rows
/// migrated before the `operation` column, which are NULL, are still counted so
/// a legacy queue is not silently reported as empty).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct EnrichStatus {
    pub(super) status_report: bool,
    pub(super) operation: String,
    pub(super) namespace: String,
    pub(super) unbound_backlog: usize,
    /// GAP-SG-77: DATABASE-semantics backlog for the queried operation, computed
    /// by `scan::count_operation_backlog` via a `SELECT COUNT(*)` over the real
    /// store. This is distinct from `queue_pending`/`queue_dead` (FILE/sidecar
    /// queue semantics) and from the legacy `unbound_backlog` (memory-bindings
    /// only). It fixes the false `pending=0` that db-backed operations
    /// (entity-descriptions/body-enrich/re-embed) previously reported.
    pub(super) scan_backlog: i64,
    /// GAP-CLI-ED-STATUS-02: empty-description-only backlog (entity-descriptions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scan_backlog_empty: Option<i64>,
    /// GAP-CLI-ED-STATUS-02: low-quality description backlog (entity-descriptions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scan_backlog_low_quality: Option<i64>,
    /// Whether `--force-redescribe` was active for this status report.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) force_redescribe: bool,
    /// Wave 2 / GAP-CLI-OBS-04: fraction of sampled descriptions grounded
    /// against linked memory bodies (`grounding_coverage` ≥ threshold).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) quality_pct: Option<f64>,
    /// Sample size used for `quality_pct` (entities inspected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) quality_sample_n: Option<u32>,
    /// Extrapolated count of low-grounding descriptions in the namespace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) scan_backlog_low_grounding_est: Option<i64>,
    pub(super) queue_pending: i64,
    pub(super) queue_processing: i64,
    pub(super) queue_done: i64,
    pub(super) queue_failed: i64,
    pub(super) queue_skipped: i64,
    pub(super) queue_dead: i64,
    pub(super) eligible_now: i64,
    pub(super) waiting: i64,
    /// GAP-SG-15/46: coarse backlog state, disambiguating an empty queue from a
    /// not-yet-scanned backlog and from a cooldown wait.
    /// `draining` | `cooldown` | `pending-scan` | `blocked_dead` (scan deficit
    /// remains but only permanent dead queue rows remain — requeue/prune) |
    /// `empty`.
    pub(super) state: &'static str,
    /// GAP-SG-16: per-item `next_retry_at` for every pending row currently in
    /// backoff, so an operator can see exactly when each will become eligible.
    pub(super) waiting_items: Vec<WaitingItem>,
}

/// GAP-SG-16: one pending queue row waiting on its backoff cooldown.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct WaitingItem {
    pub(super) item_key: String,
    pub(super) attempt: i64,
    pub(super) next_retry_at: Option<String>,
    pub(super) error_class: Option<String>,
}

/// GAP-SG-23: one dead-letter row reported by `--list-dead`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DeadItem {
    pub(super) dead_item: bool,
    pub(super) item_key: String,
    pub(super) item_type: String,
    pub(super) attempt: i64,
    pub(super) error_class: Option<String>,
    pub(super) error: Option<String>,
    /// GAP-SG-72: `choices[0].finish_reason` from the OpenRouter response
    /// that produced this failure, when one was decoded (e.g. `"length"`
    /// for a max_tokens truncation). `None` for subprocess-provider modes
    /// or failures that never reached a decoded response.
    pub(super) finish_reason: Option<String>,
    /// GAP-SG-72: `usage.prompt_tokens` from the same response, when known.
    pub(super) input_tokens: Option<i64>,
    /// GAP-SG-72: `usage.completion_tokens` from the same response, when known.
    pub(super) output_tokens: Option<i64>,
}

/// GAP-SG-23/11: summary footer for `--list-dead` and `--requeue-dead`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DeadSummary {
    pub(super) summary: bool,
    pub(super) operation: String,
    pub(super) namespace: String,
    /// `list-dead` | `requeue-dead` | `prune-dead-orphans`
    pub(super) action: &'static str,
    pub(super) dead_total: i64,
    pub(super) requeued: i64,
    /// GAP-SG-66: `prune-dead-orphans` — dead rows removed because their
    /// referenced memory no longer exists in the main DB for the namespace.
    /// Zero for `list-dead` / `requeue-dead`.
    pub(super) pruned: i64,
}

// claim/failure helpers in queue_ops.rs
pub(super) use super::queue_ops::*;

#[cfg(test)]
#[path = "queue_tests_a.rs"]
mod tests_a;
#[cfg(test)]
#[path = "queue_tests_b.rs"]
mod tests_b;
