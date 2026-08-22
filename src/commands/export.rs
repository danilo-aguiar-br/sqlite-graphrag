//! Handler for the `export` CLI subcommand.

use crate::cli::MemoryType;
use crate::errors::AppError;
use crate::output;
use crate::paths::AppPaths;
use crate::storage::connection::open_ro;
use serde::Serialize;

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Export all memories as NDJSON\n  \
    sqlite-graphrag export\n\n  \
    # Export only decision memories from a namespace\n  \
    sqlite-graphrag export --type decision --namespace my-project\n\n  \
    # Export including soft-deleted memories\n  \
    sqlite-graphrag export --include-deleted\n\n  \
    # Pipe to file for backup\n  \
    sqlite-graphrag export > backup.ndjson\n\n\
    STREAM CONTRACT (GAP-SG-215):\n  \
    Output is one self-contained record per line, followed by one summary line.\n  \
    A record line carries the record and nothing else. The summary line carries\n  \
    the single agent-surface record for the whole stream and is never reshaped.\n\n  \
    Per-record knobs act here: --select and --truncate-content.\n  \
    Whole-set knobs are refused with exit 2 before the first line, because they\n  \
    cannot mean anything per record: --count-only, --sort, --dedupe-by,\n  \
    --max-output-bytes and --max-items. Narrow the query with --limit instead.\n  \
    --filter is refused too: the summary counts what the QUERY returned, so a\n  \
    predicate applied here would leave that count describing rows you never got.\n  \
    Use --type and --namespace to narrow at the source.")]
/// Export args.
pub struct ExportArgs {
    /// Namespace (flag / XDG namespace.default / global).
    #[arg(long, help = "Namespace (flag / XDG namespace.default / global)")]
    pub namespace: Option<String>,
    /// Filter by memory type.
    #[arg(long, value_enum)]
    pub r#type: Option<MemoryType>,
    /// Include soft-deleted memories in the export.
    #[arg(long, default_value_t = false)]
    pub include_deleted: bool,
    /// Maximum number of memories to export (default: 100000).
    #[arg(long, default_value_t = DEFAULT_EXPORT_LIMIT, value_parser = crate::parsers::parse_list_limit_range)]
    pub limit: usize,
    /// Offset for pagination.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
    /// Path to graphrag.sqlite. Overrides the XDG `db.path` setting.
    #[arg(long)]
    pub db: Option<String>,
}

/// Page size `export` uses when the caller names none.
///
/// Named because GAP-SG-201 has to tell a ceiling the caller CHOSE from one a
/// constant supplied, and comparing against a literal in two places is how those
/// two facts drift apart.
const DEFAULT_EXPORT_LIMIT: usize = 100_000;

#[derive(Serialize)]
struct ExportMemoryLine {
    name: String,
    r#type: String,
    memory_type: String,
    description: String,
    body: String,
    namespace: String,
    created_at_iso: String,
    updated_at_iso: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted_at_iso: Option<String>,
}

#[derive(Serialize)]
struct ExportSummary {
    summary: bool,
    exported: usize,
    namespace: String,
    elapsed_ms: u64,
}

/// Exports memories as NDJSON (one JSON line per memory, followed by a summary line).
pub fn run(args: ExportArgs) -> Result<(), AppError> {
    let start = std::time::Instant::now();
    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;
    let paths = AppPaths::resolve(args.db.as_deref())?;
    crate::storage::connection::ensure_db_ready(&paths)?;
    let conn = open_ro(&paths.db)?;

    let deleted_filter = if args.include_deleted {
        ""
    } else {
        "AND m.deleted_at IS NULL"
    };

    let limit_i64 = args.limit as i64;
    let offset_i64 = args.offset as i64;
    let type_str: Option<String> = args.r#type.map(|t| t.as_str().to_string());

    let rows = fetch_rows(
        &conn,
        &namespace,
        &type_str,
        deleted_filter,
        limit_i64,
        offset_i64,
    )?;

    // GAP-SG-201: declared BEFORE the first emission, because the output surface
    // reads it while shaping each envelope. `export` paginates like `list` and
    // `graph entities` do, and until now declared nothing — so `--filter` and
    // `--count-only` here judged a page while reporting `query_limited: null`,
    // the exact silence the ceiling exists to break. The default limit of 100 000
    // is wider than most corpora, so this is usually a report and not a cut.
    let total_count = count_rows(&conn, &namespace, &type_str, deleted_filter)?;
    crate::agent_surface::universe::record(crate::agent_surface::universe::QueryCeiling {
        applied: args.limit,
        offset: args.offset,
        source: if args.limit == DEFAULT_EXPORT_LIMIT {
            crate::agent_surface::universe::CeilingSource::Default
        } else {
            crate::agent_surface::universe::CeilingSource::Flag
        },
        kind: crate::agent_surface::universe::CeilingKind::Pagination,
        universe_total: Some(total_count),
    });

    let exported = rows.len();

    // GAP-SG-215: resolve the whole stream's request BEFORE the first line goes
    // out. `--select name export` used to emit three correct records and then
    // exit 2 on the fourth, the summary — a refusal delivered on top of output
    // the caller had already started consuming. Opening here means a refusal
    // arrives with stdout still untouched.
    open_stream(&rows)?;

    for line in &rows {
        output::emit_stream_record(line)?;
    }

    output::emit_stream_trailer(&ExportSummary {
        summary: true,
        exported,
        namespace: namespace.clone(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    })?;

    Ok(())
}

/// Hands the surface a bounded prefix of the records it is about to shape.
///
/// The prefix is the memory bound, and it is not a small one to give up: a
/// record measured ~24 KB as a `Value`, so materializing all of them at the
/// default `--limit 100000` would cost roughly 2.4 GB to answer a question about
/// which field names exist. `agent_surface::stream` receives the real row count
/// alongside the prefix and declares the bound on the trailer.
///
/// Skipped entirely when no key needs resolving: without `--select` the
/// vocabulary is never consulted, so serializing even one row would be work done
/// for nobody. The refusals still run — `open` reaches them with an empty
/// sample, which is exactly right, since none of them asks about field names.
fn open_stream(rows: &[ExportMemoryLine]) -> Result<(), AppError> {
    let surface = crate::agent_surface::get();
    let sample = if surface.select.is_empty() {
        Vec::new()
    } else {
        rows.iter()
            .take(crate::agent_surface::stream::SAMPLE_RECORDS)
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?
    };
    crate::agent_surface::stream::open(surface, &sample, rows.len())
}

fn fetch_rows(
    conn: &rusqlite::Connection,
    namespace: &str,
    type_str: &Option<String>,
    deleted_filter: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<ExportMemoryLine>, AppError> {
    let rows = if let Some(t) = type_str {
        let sql = format!(
            "SELECT m.name, m.type, m.description, m.body, m.namespace, \
                    m.created_at, m.updated_at, m.deleted_at \
             FROM memories m \
             WHERE m.namespace = ?1 {deleted_filter} AND m.type = ?2 \
             ORDER BY m.name \
             LIMIT ?3 OFFSET ?4"
        );
        let mut stmt = conn.prepare(&sql)?;
        let result = stmt
            .query_map(rusqlite::params![namespace, t, limit, offset], map_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        let sql = format!(
            "SELECT m.name, m.type, m.description, m.body, m.namespace, \
                    m.created_at, m.updated_at, m.deleted_at \
             FROM memories m \
             WHERE m.namespace = ?1 {deleted_filter} \
             ORDER BY m.name \
             LIMIT ?2 OFFSET ?3"
        );
        let mut stmt = conn.prepare(&sql)?;
        let result = stmt
            .query_map(rusqlite::params![namespace, limit, offset], map_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };
    Ok(rows)
}

/// Counts the rows [`fetch_rows`] pages through, for the GAP-SG-201 ceiling.
///
/// The WHERE clauses mirror [`fetch_rows`] exactly, including `deleted_filter`.
/// A divergence would declare a universe that describes a different set than the
/// one being paged, which is worse than declaring none: the surface would refuse,
/// or decline to refuse, on a comparison against the wrong number.
fn count_rows(
    conn: &rusqlite::Connection,
    namespace: &str,
    type_str: &Option<String>,
    deleted_filter: &str,
) -> Result<usize, AppError> {
    let count: i64 = if let Some(t) = type_str {
        let sql = format!(
            "SELECT COUNT(*) FROM memories m \
             WHERE m.namespace = ?1 {deleted_filter} AND m.type = ?2"
        );
        conn.query_row(&sql, rusqlite::params![namespace, t], |r| r.get(0))?
    } else {
        let sql =
            format!("SELECT COUNT(*) FROM memories m WHERE m.namespace = ?1 {deleted_filter}");
        conn.query_row(&sql, rusqlite::params![namespace], |r| r.get(0))?
    };
    Ok(usize::try_from(count).unwrap_or(0))
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExportMemoryLine> {
    let memory_type_val: String = row.get(1)?;
    Ok(ExportMemoryLine {
        name: row.get(0)?,
        r#type: memory_type_val.clone(),
        memory_type: memory_type_val,
        description: row.get(2)?,
        body: row.get(3)?,
        namespace: row.get(4)?,
        created_at_iso: crate::tz::epoch_to_iso(row.get::<_, i64>(5)?),
        updated_at_iso: crate::tz::epoch_to_iso(row.get::<_, i64>(6)?),
        deleted_at_iso: row.get::<_, Option<i64>>(7)?.map(crate::tz::epoch_to_iso),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_line_emits_both_type_and_memory_type() {
        let line = ExportMemoryLine {
            name: "test".to_string(),
            r#type: "document".to_string(),
            memory_type: "document".to_string(),
            description: "desc".to_string(),
            body: "body".to_string(),
            namespace: "global".to_string(),
            created_at_iso: "2025-01-01T00:00:00Z".to_string(),
            updated_at_iso: "2025-01-01T00:00:00Z".to_string(),
            deleted_at_iso: None,
        };
        let json = serde_json::to_value(&line).unwrap();
        assert_eq!(json["type"], "document");
        assert_eq!(json["memory_type"], "document");
    }
}
