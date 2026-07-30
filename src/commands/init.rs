//! Handler for the `init` CLI subcommand.

use crate::errors::AppError;
use crate::output;
use crate::paths::AppPaths;
use crate::pragmas::{apply_init_pragmas, ensure_wal_mode};
use crate::storage::connection::open_rw;
use serde::Serialize;

/// Embedding model choices exposed through `--model`.
///
/// Legacy flag kept for CLI compatibility only: since v1.0.76 the build is
/// LLM-only and no local model is downloaded. The value is accepted and
/// ignored; `schema_meta.model` records the CLI version (G46).
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum EmbeddingModelChoice {
    /// Multilingual E 5 small variant.
    #[value(name = "multilingual-e5-small")]
    MultilingualE5Small,
}

#[derive(clap::Args)]
#[command(after_long_help = "EXAMPLES:\n  \
    # Initialize a new database in the current directory\n  \
    sqlite-graphrag init\n\n  \
    # Initialize with a specific namespace\n  \
    sqlite-graphrag init --namespace my-project\n\n  \
    # Initialize at a custom database path\n  \
    sqlite-graphrag init --db /path/to/graphrag.sqlite")]
/// Init args.
pub struct InitArgs {
    /// Path to graphrag.sqlite.
    ///
    /// Resolution precedence (highest to lowest): `--db` flag > XDG `db.path`
    /// > `graphrag.sqlite` under the XDG data directory. The current working
    /// > directory is NOT part of the cascade on a host with a home directory.
    #[arg(long)]
    pub db: Option<String>,
    /// Legacy embedding model identifier (accepted and ignored since the
    /// v1.0.76 LLM-only build; kept for CLI compatibility). Safe to omit.
    #[arg(long, value_enum)]
    pub model: Option<EmbeddingModelChoice>,
    /// Force re-initialization, overwriting any existing schema metadata.
    /// Use only when the schema is corrupted; loses configuration but preserves data.
    #[arg(long)]
    pub force: bool,
    /// Initial namespace to resolve. Aligned with bilingual docs that mention `init --namespace`.
    /// When provided, overrides XDG `namespace.default`; otherwise falls back to `global`.
    #[arg(long)]
    pub namespace: Option<String>,
    /// Emit machine-readable JSON on stdout.
    #[arg(long, hide = true, help = "No-op; JSON is always emitted on stdout")]
    pub json: bool,
}

#[derive(Serialize)]
struct InitResponse {
    db_path: String,
    /// Latest applied migration number from `refinery_schema_history`.
    /// Emitted as a JSON number for cross-command consistency with `health` and `stats` (since v1.0.35).
    schema_version: u32,
    model: String,
    dim: usize,
    /// Active namespace resolved during initialisation, aligned with the bilingual docs.
    namespace: String,
    status: String,
    /// Total execution time in milliseconds from handler start to serialisation.
    elapsed_ms: u64,
}

/// Run.
pub fn run(
    args: InitArgs,
    llm_backend: crate::cli::LlmBackendChoice,
    embedding_backend: crate::cli::EmbeddingBackendChoice,
) -> Result<(), AppError> {
    let start = std::time::Instant::now();
    let paths = AppPaths::resolve(args.db.as_deref())?;
    paths.ensure_dirs()?;

    let namespace = crate::namespace::resolve_namespace(args.namespace.as_deref())?;

    let mut conn = open_rw(&paths.db)?;

    apply_init_pragmas(&conn)?;

    crate::migrations::runner()
        .run(&mut conn)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("migration failed: {e}")))?;

    conn.execute_batch(&format!(
        "PRAGMA user_version = {};",
        crate::constants::SCHEMA_USER_VERSION
    ))?;

    // Defensive re-assertion: refinery may revert journal_mode during migrations.
    ensure_wal_mode(&conn)?;

    let schema_version = latest_schema_version(&conn)?;

    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![schema_version],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('model', ?1)",
        rusqlite::params![crate::constants::SQLITE_GRAPHRAG_VERSION],
    )?;
    // G43: pre-v1.0.79 this hardcoded '384' as a literal, bypassing the
    // active default (DEFAULT_EMBEDDING_DIM = 1024).
    // INSERT OR IGNORE preserves the recorded dim on re-init of an existing
    // database; the active dim (env > database > default) fills new ones.
    conn.execute(
        "INSERT OR IGNORE INTO schema_meta (key, value) VALUES ('dim', ?1)",
        rusqlite::params![crate::constants::embedding_dim().to_string()],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('created_at', CAST(unixepoch() AS TEXT))",
        [],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('sqlite-graphrag_version', ?1)",
        rusqlite::params![crate::constants::SQLITE_GRAPHRAG_VERSION],
    )?;
    // Persist the resolved namespace so downstream tools can inspect it without re-resolving.
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('namespace_initial', ?1)",
        rusqlite::params![namespace],
    )?;

    output::emit_progress_i18n(
        "Validating embedding backend...",
        "Validando backend de embedding...",
    );

    // GAP-INIT-EMBEDDING-001 FIX (v1.0.89): init must succeed without LLM.
    // Schema, tables and FTS5 are created above; the smoke test only validates
    // that the embedding subprocess is reachable. When it is not (OAuth expired,
    // CLI missing), init still succeeds with dim from the database or default.
    // ADR-0011: Validation errors (OAuth-only enforcement) are FATAL — propagate.
    // v1.0.89 (GAP-EMBED-PROPAGATION): honour --llm-backend via embed_passage_with_choice.
    let (dim, status) = match crate::embedder::embed_passage_with_embedding_choice(
        &paths.models,
        "smoke test",
        embedding_backend,
        llm_backend,
    ) {
        Ok((v, _backend)) => (v.len(), "ok"),
        Err(crate::errors::AppError::Validation(msg)) => {
            return Err(crate::errors::AppError::Validation(msg))
        }
        Err(e) => {
            tracing::warn!(target: "init", error = %e, "embedding smoke test failed; init continues without LLM validation");
            (crate::constants::embedding_dim(), "ok_no_embedding")
        }
    };

    output::emit_json(&InitResponse {
        db_path: paths.db.display().to_string(),
        schema_version,
        model: crate::constants::SQLITE_GRAPHRAG_VERSION.to_string(),
        dim,
        namespace,
        status: status.to_string(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    })?;

    Ok(())
}

fn latest_schema_version(conn: &rusqlite::Connection) -> Result<u32, AppError> {
    match conn.query_row(
        "SELECT version FROM refinery_schema_history ORDER BY version DESC LIMIT 1",
        [],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(version) => Ok(version.max(0) as u32),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(err) => Err(AppError::Database(err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_response_serializes_all_fields() {
        let resp = InitResponse {
            db_path: "/tmp/test.sqlite".to_string(),
            schema_version: 6,
            model: crate::constants::SQLITE_GRAPHRAG_VERSION.to_string(),
            dim: crate::constants::DEFAULT_EMBEDDING_DIM,
            namespace: "global".to_string(),
            status: "ok".to_string(),
            elapsed_ms: 100,
        };
        let json = serde_json::to_value(&resp).expect("serialization failed");
        assert_eq!(json["db_path"], "/tmp/test.sqlite");
        assert_eq!(json["schema_version"], 6);
        assert_eq!(json["model"], crate::constants::SQLITE_GRAPHRAG_VERSION);
        assert_eq!(json["dim"], crate::constants::DEFAULT_EMBEDDING_DIM);
        assert_eq!(json["namespace"], "global");
        assert_eq!(json["status"], "ok");
        assert!(json["elapsed_ms"].is_number());
    }

    #[test]
    fn latest_schema_version_returns_zero_for_empty_db() {
        let conn = rusqlite::Connection::open_in_memory().expect("failed to open in-memory db");
        conn.execute_batch("CREATE TABLE refinery_schema_history (version INTEGER NOT NULL);")
            .expect("failed to create table");

        let version = latest_schema_version(&conn).expect("latest_schema_version failed");
        assert_eq!(version, 0u32, "empty db must return schema_version 0");
    }

    #[test]
    fn latest_schema_version_returns_max_version() {
        let conn = rusqlite::Connection::open_in_memory().expect("failed to open in-memory db");
        conn.execute_batch(
            "CREATE TABLE refinery_schema_history (version INTEGER NOT NULL);
             INSERT INTO refinery_schema_history VALUES (1);
             INSERT INTO refinery_schema_history VALUES (3);
             INSERT INTO refinery_schema_history VALUES (2);",
        )
        .expect("failed to populate table");

        let version = latest_schema_version(&conn).expect("latest_schema_version failed");
        assert_eq!(version, 3u32, "must return the highest version present");
    }

    #[test]
    fn init_default_dim_matches_the_registered_setting_default() {
        // `init` stamps `schema_meta.dim` from `DEFAULT_EMBEDDING_DIM`, while
        // `config doctor` advertises the default of the `embedding.dim` key.
        // Asserting the two against each other — rather than against a literal
        // repeated here — means this test keeps its meaning after the next
        // change instead of becoming a third place to update.
        let registered = crate::config::SETTING_KEYS
            .iter()
            .find(|entry| entry.key == "embedding.dim")
            .and_then(|entry| entry.default)
            .expect("embedding.dim must be registered with a literal default");
        assert_eq!(
            registered.parse::<usize>().ok(),
            Some(crate::constants::DEFAULT_EMBEDDING_DIM),
            "config doctor would advertise {registered} while init stamps {}",
            crate::constants::DEFAULT_EMBEDDING_DIM
        );
    }

    #[test]
    fn init_default_dim_is_inside_the_accepted_range() {
        // A default outside the range would be rejected by the very resolver
        // that is supposed to fall back to it, leaving the dim unresolved.
        assert!(
            crate::constants::EMBEDDING_DIM_RANGE
                .contains(&crate::constants::DEFAULT_EMBEDDING_DIM),
            "default dim must sit inside EMBEDDING_DIM_RANGE"
        );
    }

    #[test]
    fn init_response_namespace_aligned_with_schema() {
        // Verify namespace field survives round-trip serialization with correct value.
        let resp = InitResponse {
            db_path: "/tmp/x.sqlite".to_string(),
            schema_version: 6,
            model: crate::constants::SQLITE_GRAPHRAG_VERSION.to_string(),
            dim: crate::constants::DEFAULT_EMBEDDING_DIM,
            namespace: "my-project".to_string(),
            status: "ok".to_string(),
            elapsed_ms: 0,
        };
        let json = serde_json::to_value(&resp).expect("serialization failed");
        assert_eq!(json["namespace"], "my-project");
    }
}
