//! Handler for the `related` CLI subcommand.

use crate::constants::{
    DEFAULT_MAX_HOPS, DEFAULT_MIN_WEIGHT, K_RELATED_DEFAULT_LIMIT, TEXT_DESCRIPTION_PREVIEW_LEN,
};
use crate::errors::AppError;
use crate::graph::{GraphWalk, SqlNeighbors};
use crate::i18n::errors_msg;
use crate::output::{self, OutputFormat};
use crate::paths::AppPaths;
use crate::storage::connection::open_ro;
use rusqlite::{params, Connection};
use serde::Serialize;

/// Identifies whether the seed resolved to a memory or a bare entity.
enum SeedKind {
    Memory(i64),
    Entity(i64),
}

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # List memories connected to a memory via the entity graph (default 2 hops)\n  \
    sqlite-graphrag related onboarding\n\n  \
    # Increase hop distance and filter by relation type\n  \
    sqlite-graphrag related onboarding --max-hops 3 --relation related\n\n  \
    # Cap result count and require minimum edge weight\n  \
    sqlite-graphrag related onboarding --limit 5 --min-weight 0.5")]
/// Related args.
pub struct RelatedArgs {
    /// Memory name as a positional argument. Alternative to `--name`.
    #[arg(
        value_name = "NAME",
        conflicts_with = "name",
        help = "Memory name whose neighbours to traverse; alternative to --name"
    )]
    pub name_positional: Option<String>,
    /// Memory name as a flag. Required when the positional form is absent. Also accepts the alias `--from`.
    #[arg(long, alias = "from")]
    pub name: Option<String>,
    /// Maximum graph hop count. Also accepts the alias `--hops`.
    #[arg(long, alias = "hops", default_value_t = DEFAULT_MAX_HOPS, value_parser = crate::parsers::parse_hops_range_u32)]
    pub max_hops: u32,
    /// Filter results to a specific relation type. Canonical values:
    /// applies-to, uses, depends-on, causes, fixes, contradicts, supports,
    /// follows, related, mentions, replaces, tracked-in.
    /// Any kebab-case or snake_case string is also accepted as a custom relation.
    #[arg(long, value_parser = crate::parsers::parse_relation)]
    pub relation: Option<String>,
    /// Min weight.
    #[arg(long, default_value_t = DEFAULT_MIN_WEIGHT)]
    pub min_weight: f64,
    /// Maximum number of items.
    #[arg(long, default_value_t = K_RELATED_DEFAULT_LIMIT, value_parser = crate::parsers::parse_k_range)]
    pub limit: usize,
    /// Namespace scope.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to the SQLite database file.
    #[arg(long)]
    pub db: Option<String>,
}

#[derive(Serialize)]
struct RelatedResponse {
    /// Echo of the seed memory name resolved from `--name` or the positional argument.
    /// Added in v1.0.35 for input transparency in JSON output.
    name: String,
    /// Echo of the resolved `--max-hops` value (default 2). Added in v1.0.35.
    max_hops: u32,
    results: Vec<RelatedMemory>,
    /// Semantic alias of `results` following the v1.0.66 alias pattern (list has items/memories).
    related_memories: Vec<RelatedMemory>,
    elapsed_ms: u64,
}

#[derive(Serialize, Clone)]
struct RelatedMemory {
    memory_id: i64,
    name: String,
    namespace: String,
    #[serde(rename = "type")]
    memory_type: String,
    description: String,
    hop_distance: u32,
    source_entity: Option<String>,
    target_entity: Option<String>,
    /// Alias of `source_entity` for cross-command consistency (graph, link, deep-research use from/to).
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    /// Alias of `target_entity` for cross-command consistency.
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    relation: Option<String>,
    weight: Option<f64>,
}

/// Run.
pub fn run(args: RelatedArgs) -> Result<(), AppError> {
    let started = std::time::Instant::now();
    let name = args
        .name_positional
        .as_deref()
        .or(args.name.as_deref())
        .ok_or_else(|| {
            AppError::Validation(
                "name required: pass as positional argument or via --name".to_string(),
            )
        })?
        .to_string();

    if name.trim().is_empty() {
        return Err(AppError::Validation(
            crate::i18n::validation::name_must_not_be_empty(),
        ));
    }

    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let paths = AppPaths::resolve(args.db.as_deref())?;

    crate::storage::connection::ensure_db_ready(&paths)?;

    let conn = open_ro(&paths.db)?;

    // Locate the seed: try memory first, fall back to bare entity.
    let seed = match conn.query_row(
        "SELECT id FROM memories WHERE namespace = ?1 AND name = ?2 AND deleted_at IS NULL",
        params![namespace, name],
        |r| r.get::<_, i64>(0),
    ) {
        Ok(id) => SeedKind::Memory(id),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            match crate::storage::entities::find_entity_id(&conn, &namespace, &name)? {
                Some(id) => SeedKind::Entity(id),
                None => {
                    return Err(AppError::NotFound(errors_msg::memory_or_entity_not_found(
                        &name, &namespace,
                    )))
                }
            }
        }
        Err(e) => return Err(AppError::Database(e)),
    };

    // Collect seed entity IDs depending on seed kind.
    let (seed_memory_id, seed_entity_ids): (i64, Vec<i64>) = match &seed {
        SeedKind::Memory(id) => {
            let mem_id = *id;
            let mut stmt =
                conn.prepare_cached("SELECT entity_id FROM memory_entities WHERE memory_id = ?1")?;
            let rows: Vec<i64> = stmt
                .query_map(params![mem_id], |r| r.get(0))?
                .collect::<Result<Vec<i64>, _>>()?;
            (mem_id, rows)
        }
        SeedKind::Entity(entity_id) => {
            // For a bare entity seed there is no corresponding memory to skip.
            // Use a sentinel -1 so dedup never matches a real memory_id.
            (-1, vec![*entity_id])
        }
    };

    let relation_filter = args.relation;
    if let Some(ref r) = relation_filter {
        crate::parsers::warn_if_non_canonical(r);
    }
    let results = traverse_related(
        &conn,
        seed_memory_id,
        &seed_entity_ids,
        &namespace,
        args.max_hops,
        args.min_weight,
        relation_filter.as_deref(),
        args.limit,
    )?;
    // GAP-SG-201: `related --limit` stops a breadth-first walk rather than
    // paging a table, so there is no universe to count and no page to refuse.
    // Reported so the caller sees the bound instead of inferring completeness.
    crate::agent_surface::universe::record(crate::agent_surface::universe::QueryCeiling {
        applied: args.limit,
        offset: 0,
        source: crate::agent_surface::universe::CeilingSource::Flag,
        kind: crate::agent_surface::universe::CeilingKind::TopK,
        universe_total: None,
    });

    match args.format {
        OutputFormat::Json => {
            let related_memories = results.clone();
            output::emit_json(&RelatedResponse {
                name: name.clone(),
                max_hops: args.max_hops,
                results,
                related_memories,
                elapsed_ms: started.elapsed().as_millis() as u64,
            })?;
        }
        OutputFormat::Text => {
            for item in &results {
                if item.description.is_empty() {
                    output::emit_text(&format!(
                        "{}. {} ({})",
                        item.hop_distance, item.name, item.namespace
                    ));
                } else {
                    let preview: String = item
                        .description
                        .chars()
                        .take(TEXT_DESCRIPTION_PREVIEW_LEN)
                        .collect();
                    output::emit_text(&format!(
                        "{}. {} ({}): {}",
                        item.hop_distance, item.name, item.namespace, preview
                    ));
                }
            }
        }
        OutputFormat::Markdown => {
            for item in &results {
                if item.description.is_empty() {
                    output::emit_text(&format!(
                        "- **{}** ({}) — hop {}",
                        item.name, item.namespace, item.hop_distance
                    ));
                } else {
                    let preview: String = item
                        .description
                        .chars()
                        .take(TEXT_DESCRIPTION_PREVIEW_LEN)
                        .collect();
                    output::emit_text(&format!(
                        "- **{}** ({}) — hop {}: {}",
                        item.name, item.namespace, item.hop_distance, preview
                    ));
                }
            }
        }
    }

    Ok(())
}

// One over, and every parameter is an independent knob the caller reads off
// `RelatedArgs`: there is no pair here that travels together anywhere else.
// Four seeds plus four traversal knobs. The knobs come from four unrelated CLI
// flags with no shared lifetime, so a struct would be a bag named after this one
// call rather than after a concept the caller already holds.
#[allow(clippy::too_many_arguments)]
fn traverse_related(
    conn: &Connection,
    seed_memory_id: i64,
    seed_entity_ids: &[i64],
    namespace: &str,
    max_hops: u32,
    min_weight: f64,
    relation_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<RelatedMemory>, AppError> {
    if seed_entity_ids.is_empty() || max_hops == 0 {
        return Ok(Vec::new());
    }

    // Bidirectional BFS: users reason about "related" without caring which end of
    // the edge their memory sits on. The walk records the edge that FIRST reached
    // each entity, which under FIFO order is the shortest-path edge.
    let walk = GraphWalk::bidirectional(min_weight, max_hops)
        .with_relation_filter(relation_filter.map(str::to_string));
    let outcome = walk.run(&SqlNeighbors::with_names(conn, namespace), seed_entity_ids)?;

    let entity_hop = outcome.depth;
    // Per-entity edge info: source_name, target_name, relation, weight.
    let entity_edge: crate::hash::AHashMap<i64, (String, String, String, f64)> = outcome
        .arrival
        .into_iter()
        .map(|(id, edge)| {
            (
                id,
                (
                    edge.source_name.unwrap_or_default(),
                    edge.target_name.unwrap_or_default(),
                    edge.relation,
                    edge.weight,
                ),
            )
        })
        .collect();

    // For each discovered entity (hop >= 1) find its memories, skipping the seed memory.
    let mut out: Vec<RelatedMemory> = Vec::with_capacity(limit);
    let mut dedup_ids: crate::hash::AHashSet<i64> =
        crate::hash::AHashSet::with_capacity_and_hasher(limit, Default::default());
    dedup_ids.insert(seed_memory_id);

    // Sort entities by hop ASC, weight DESC, entity_id ASC so we emit closer
    // entities first — and emit the SAME ones on every run.
    //
    // The `entity_id` term is not cosmetic. `entity_hop` is a `HashMap` with
    // `RandomState`, seeded per PROCESS, so `.iter()` yields a different order
    // in every invocation. `sort_by` is STABLE, so any tie inherited that order
    // verbatim — and in a graph `(hop, weight)` ties are the norm, because
    // weights cluster on 0.5 and 1.0. Measured before this line existed: eight
    // identical `related` invocations against the same database returned EIGHT
    // different result sets, not merely reordered, since the top-k cut fell in a
    // different place each time. Exit 0 throughout, so nothing announced it.
    //
    // The tie-break makes the comparator TOTAL, which is what determinism needs;
    // switching the container to `BTreeMap` would also work and would cost
    // O(log n) on a walk that crosses hubs of degree 7515 to buy nothing this
    // does not already buy.
    let mut ordered_entities: Vec<(i64, u32)> = entity_hop
        .iter()
        .filter(|(id, _)| !seed_entity_ids.contains(id))
        .map(|(id, hop)| (*id, *hop))
        .collect();
    ordered_entities.sort_by(|a, b| {
        let weight_a = entity_edge.get(&a.0).map(|e| e.3).unwrap_or(0.0);
        let weight_b = entity_edge.get(&b.0).map(|e| e.3).unwrap_or(0.0);
        a.1.cmp(&b.1)
            .then_with(|| {
                weight_b
                    .partial_cmp(&weight_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.0.cmp(&b.0))
    });

    for (entity_id, hop) in ordered_entities {
        let mut stmt = conn.prepare_cached(
            // `ORDER BY m.id` states the order this loop already depended on.
            // Without it the row order is whatever plan SQLite picks, which is
            // stable for a given database and schema and is NOT a contract —
            // an index added later reorders the output of a read-only command.
            "SELECT m.id, m.name, m.namespace, m.type, m.description
             FROM memory_entities me
             JOIN memories m ON m.id = me.memory_id
             WHERE me.entity_id = ?1 AND m.deleted_at IS NULL
             ORDER BY m.id",
        )?;
        let rows = stmt
            .query_map(params![entity_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        for (mid, name, ns, mtype, desc) in rows {
            if !dedup_ids.insert(mid) {
                continue;
            }
            let edge = entity_edge.get(&entity_id);
            let src = edge.map(|e| e.0.clone());
            let tgt = edge.map(|e| e.1.clone());
            out.push(RelatedMemory {
                memory_id: mid,
                name,
                namespace: ns,
                memory_type: mtype,
                description: desc,
                hop_distance: hop,
                source_entity: src.clone(),
                target_entity: tgt.clone(),
                from: src,
                to: tgt,
                relation: edge.map(|e| e.2.clone()),
                weight: edge.map(|e| e.3),
            });
            if out.len() >= limit {
                return Ok(out);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_related_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("failed to open in-memory db");
        conn.execute_batch(
            "CREATE TABLE memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                namespace TEXT NOT NULL DEFAULT 'global',
                type TEXT NOT NULL DEFAULT 'fact',
                description TEXT NOT NULL DEFAULT '',
                deleted_at INTEGER
            );
            CREATE TABLE entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace TEXT NOT NULL,
                name TEXT NOT NULL
            );
            CREATE TABLE relationships (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                namespace TEXT NOT NULL,
                source_id INTEGER NOT NULL,
                target_id INTEGER NOT NULL,
                relation TEXT NOT NULL DEFAULT 'related_to',
                weight REAL NOT NULL DEFAULT 1.0
            );
            CREATE TABLE memory_entities (
                memory_id INTEGER NOT NULL,
                entity_id INTEGER NOT NULL
            );",
        )
        .expect("failed to create test tables");
        conn
    }

    fn insert_memory(conn: &rusqlite::Connection, name: &str, namespace: &str) -> i64 {
        conn.execute(
            "INSERT INTO memories (name, namespace) VALUES (?1, ?2)",
            rusqlite::params![name, namespace],
        )
        .expect("failed to insert memory");
        conn.last_insert_rowid()
    }

    fn insert_entity(conn: &rusqlite::Connection, name: &str, namespace: &str) -> i64 {
        conn.execute(
            "INSERT INTO entities (name, namespace) VALUES (?1, ?2)",
            rusqlite::params![name, namespace],
        )
        .expect("failed to insert entity");
        conn.last_insert_rowid()
    }

    fn link_memory_entity(conn: &rusqlite::Connection, memory_id: i64, entity_id: i64) {
        conn.execute(
            "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
            rusqlite::params![memory_id, entity_id],
        )
        .expect("failed to link memory-entity");
    }

    fn insert_relationship(
        conn: &rusqlite::Connection,
        namespace: &str,
        source_id: i64,
        target_id: i64,
        relation: &str,
        weight: f64,
    ) {
        conn.execute(
            "INSERT INTO relationships (namespace, source_id, target_id, relation, weight)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![namespace, source_id, target_id, relation, weight],
        )
        .expect("failed to insert relationship");
    }

    #[test]
    fn related_response_serializes_results_and_elapsed_ms() {
        let mem = RelatedMemory {
            memory_id: 1,
            name: "neighbor-mem".to_string(),
            namespace: "global".to_string(),
            memory_type: "document".to_string(),
            description: "desc".to_string(),
            hop_distance: 1,
            source_entity: Some("entity-a".to_string()),
            target_entity: Some("entity-b".to_string()),
            from: Some("entity-a".to_string()),
            to: Some("entity-b".to_string()),
            relation: Some("related_to".to_string()),
            weight: Some(0.9),
        };
        let resp = RelatedResponse {
            name: "seed-mem".to_string(),
            max_hops: 2,
            related_memories: vec![mem.clone()],
            results: vec![mem],
            elapsed_ms: 7,
        };
        let json = serde_json::to_value(&resp).expect("serialization failed");
        assert!(json["results"].is_array());
        assert_eq!(json["results"].as_array().unwrap().len(), 1);
        assert_eq!(json["elapsed_ms"], 7u64);
        assert_eq!(json["results"][0]["type"], "document");
        assert_eq!(json["results"][0]["hop_distance"], 1);
    }

    #[test]
    fn traverse_related_returns_empty_without_seed_entities() {
        let conn = setup_related_db();
        let result = traverse_related(&conn, 1, &[], "global", 2, 0.0, None, 10)
            .expect("traverse_related failed");
        assert!(result.is_empty());
    }

    #[test]
    fn traverse_related_returns_empty_with_max_hops_zero() {
        let conn = setup_related_db();
        let mem_id = insert_memory(&conn, "seed", "global");
        let ent_id = insert_entity(&conn, "global", "ent");
        let result = traverse_related(&conn, mem_id, &[ent_id], "global", 0, 0.0, None, 10)
            .expect("traverse_related failed");
        assert!(result.is_empty());
    }

    #[test]
    fn traverse_related_discovers_neighbor_memory_via_graph() {
        let conn = setup_related_db();
        let seed_id = insert_memory(&conn, "seed", "global");
        let ent_a = insert_entity(&conn, "global", "ent-a");
        let ent_b = insert_entity(&conn, "global", "ent-b");
        let neighbor_id = insert_memory(&conn, "neighbor", "global");
        link_memory_entity(&conn, seed_id, ent_a);
        link_memory_entity(&conn, neighbor_id, ent_b);
        insert_relationship(&conn, "global", ent_a, ent_b, "related_to", 1.0);
        let result = traverse_related(&conn, seed_id, &[ent_a], "global", 2, 0.0, None, 10)
            .expect("traverse_related failed");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "neighbor");
    }

    #[test]
    fn traverse_related_respects_limit() {
        let conn = setup_related_db();
        let seed_id = insert_memory(&conn, "seed", "global");
        let ent_seed = insert_entity(&conn, "global", "ent-seed");
        link_memory_entity(&conn, seed_id, ent_seed);
        for i in 0..5 {
            let ent_id = insert_entity(&conn, "global", &format!("ent-{i}"));
            let mem_id = insert_memory(&conn, &format!("mem-{i}"), "global");
            link_memory_entity(&conn, mem_id, ent_id);
            insert_relationship(&conn, "global", ent_seed, ent_id, "related_to", 1.0);
        }
        let result = traverse_related(&conn, seed_id, &[ent_seed], "global", 1, 0.0, None, 3)
            .expect("traverse_related failed");
        assert_eq!(
            result.len(),
            3,
            "limit=3 must constrain to at most 3 results"
        );
    }

    #[test]
    fn related_memory_optional_null_fields_serialized() {
        let mem = RelatedMemory {
            memory_id: 99,
            name: "no-relation".to_string(),
            namespace: "ns".to_string(),
            memory_type: "concept".to_string(),
            description: "".to_string(),
            hop_distance: 2,
            source_entity: None,
            target_entity: None,
            from: None,
            to: None,
            relation: None,
            weight: None,
        };
        let json = serde_json::to_value(&mem).expect("serialization failed");
        assert!(json["source_entity"].is_null());
        assert!(json["target_entity"].is_null());
        assert!(json["relation"].is_null());
        assert!(json["weight"].is_null());
        assert_eq!(json["hop_distance"], 2);
    }
}
