//! SQLite connection setup with PRAGMAs and 0600 permissions.
//!
//! v1.0.76: opens (or creates) the database file. The `sqlite-vec` extension
//! was REMOVED; vector similarity is now computed in pure Rust over the
//! `memory_embeddings(memory_id, embedding BLOB, source)` table. WAL/journal
//! PRAGMAs and 0600 file permissions on Unix are unchanged.

use crate::errors::AppError;
use crate::paths::AppPaths;
use crate::pragmas::{apply_connection_pragmas, apply_init_pragmas, ensure_wal_mode};
use crate::storage::foreign_keys::{
    assert_migration_orphaned_nothing, foreign_key_violation_counts,
    warn_about_pre_existing_violations,
};
use rusqlite::Connection;
use std::path::Path;

/// v1.0.76: no-op stub. Kept for source compatibility with callers that
/// still call `register_vec_extension()` during auto-init. The actual
/// extension registration is gone; the function is now a marker that
/// the LLM-only build does not need any vector extension.
pub fn register_vec_extension() {}

/// Open rw.
pub fn open_rw(path: &Path) -> Result<Connection, AppError> {
    let conn = Connection::open(path)?;
    apply_connection_pragmas(&conn)?;
    apply_secure_permissions(path);
    adopt_embedding_dim(&conn);
    Ok(conn)
}

/// G42/S1 follow-up (G43): adopts the dimensionality recorded in
/// `schema_meta.dim` for this process, so EVERY command that opens the
/// database — not only the `ensure_db_ready` auto-init path — produces
/// and queries vectors of the database dimensionality. Pre-G43 the
/// adoption only ran in `ensure_db_ready`, which `remember` / `edit` /
/// `recall` / `hybrid-search` never call; those commands silently used
/// the compiled default (64) against pre-v1.0.79 384-dim databases,
/// writing mixed-dim embeddings that cosine-score 0.0 against each
/// other.
///
/// Read-only and best-effort by design: a virgin database without
/// `schema_meta` is a no-op (the table is created and persisted later
/// by `ensure_schema` / `ensure_db_ready`). A CLI flag or XDG override
/// always wins and is handled inside `constants::embedding_dim`.
fn adopt_embedding_dim(conn: &Connection) {
    if crate::constants::embedding_dim_from_runtime().is_some() {
        return;
    }
    if let Ok(value) = conn.query_row(
        "SELECT value FROM schema_meta WHERE key = 'dim'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        if let Ok(dim) = value.parse::<usize>() {
            crate::constants::set_active_embedding_dim(dim);
        }
    }
}

/// Runs the pending refinery migrations with foreign key enforcement disabled,
/// then restores it and verifies that nothing was orphaned.
///
/// GAP-SG-277 follow-up: `PRAGMA foreign_keys` is a documented no-op while a
/// transaction is pending, and refinery opens one transaction per migration
/// (`refinery-core::drivers::rusqlite`). Every `PRAGMA foreign_keys = OFF`
/// written at the top of a migration file — V006, V008, V009, V010, V013 — has
/// therefore never taken effect: the connection arrives with enforcement ON
/// from [`apply_connection_pragmas`] and keeps it for the whole run.
///
/// That matters because `DROP TABLE` under enforcement performs an implicit
/// `DELETE FROM` before dropping, which fires the `ON DELETE CASCADE` of every
/// child table. `entities` has four such children (`relationships`,
/// `memory_entities`, `entity_embeddings`, `entity_connect_seen`), so the
/// rebuild-and-rename pattern those migrations use would silently empty the
/// whole graph on a populated database. Fresh databases never showed it
/// because the cascade has nothing to delete when the tables are still empty.
///
/// Toggling the pragma here — outside refinery's transaction — is what the
/// SQLite "making other kinds of table schema changes" procedure prescribes as
/// its very first step, before the transaction is opened.
///
/// # Errors
/// Returns `Err` when the pragma cannot be toggled, when a migration fails, or
/// when the migration itself leaves rows orphaned that were not orphaned before.
///
/// Pre-existing violations do NOT fail the run. `PRAGMA foreign_key_check`
/// inspects the whole database, not the slice a migration touched, so a single
/// dangling row inherited from an older schema — written back when enforcement
/// was effectively off — used to abort every migration on that file. Since
/// `ensure_db_ready` migrates on open, and nearly every subcommand calls it,
/// that turned one legacy row into a database no command could open: even
/// `cleanup-orphans`, the repair path, failed before reaching its first delete.
/// The assertion exists to prove that THIS migration broke nothing, and a row
/// that was already broken proves nothing about it.
pub(crate) fn run_migrations_with_foreign_keys_off(
    conn: &mut Connection,
    failure_label: &str,
) -> Result<(), AppError> {
    // Baseline first: what is already broken is not this migration's doing.
    let before = foreign_key_violation_counts(conn)?;

    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

    let migrated = crate::migrations::runner()
        .set_abort_divergent(false)
        .run(conn)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("{failure_label}: {e}")));

    // Restore enforcement before propagating, so a failed migration never
    // hands back a connection that silently accepts orphan rows.
    let restored = conn.execute_batch("PRAGMA foreign_keys = ON;");

    migrated?;
    restored?;

    let after = foreign_key_violation_counts(conn)?;
    assert_migration_orphaned_nothing(&before, &after)?;
    warn_about_pre_existing_violations(&after);
    Ok(())
}

/// Copies the database aside before migrations touch an existing file.
///
/// There is no down migration in this project and refinery only moves forward,
/// so a migration that goes wrong has exactly one remedy: an earlier copy of the
/// file. Until v1.2.8 none was taken, while `ensure_db_ready` would happily
/// auto-migrate from inside a plain `recall`.
///
/// Uses the SQLite Online Backup API rather than a filesystem copy, because the
/// database runs in WAL mode: copying the `.sqlite` alone would silently omit
/// whatever still lives in the `-wal` sidecar.
///
/// A failure here ABORTS the migration. Migrating without the one available
/// remedy is the situation this function exists to prevent, so falling through
/// on error would defeat it.
///
/// # Errors
/// Returns `Err` when the destination cannot be created or the copy fails.
fn back_up_before_migrating(
    conn: &Connection,
    db_path: &Path,
    applied_schema_version: i64,
) -> Result<(), AppError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut destination = db_path.as_os_str().to_os_string();
    destination.push(format!(".bak.pre-schema-{applied_schema_version}.{stamp}"));
    let destination = std::path::PathBuf::from(destination);

    /// Pages copied per `sqlite3_backup_step`, matching the default the
    /// `backup` subcommand exposes as `--backup-step-size`.
    const STEP_PAGES: std::os::raw::c_int = 1_000;

    let mut target = Connection::open(&destination)?;
    {
        let backup = rusqlite::backup::Backup::new(conn, &mut target)?;
        backup.run_to_completion(
            STEP_PAGES,
            std::time::Duration::from_millis(crate::constants::BACKUP_BUSY_RETRY_DELAY_MS),
            None,
        )?;
    }
    apply_secure_permissions(&destination);

    tracing::warn!(target: "storage",
        backup = %destination.display(),
        "database copied aside before auto-migration"
    );
    Ok(())
}

/// Ensure schema.
pub fn ensure_schema(conn: &mut Connection) -> Result<(), AppError> {
    run_migrations_with_foreign_keys_off(conn, "migration failed")?;
    conn.execute_batch(&format!(
        "PRAGMA user_version = {};",
        crate::constants::SCHEMA_USER_VERSION
    ))?;
    Ok(())
}

/// Ensures the database file exists and the schema is at the current version.
///
/// Behavior:
/// - DB does not exist: creates the file, applies init PRAGMAs, runs all migrations,
///   sets `PRAGMA user_version`, and populates `schema_meta` with default values.
///   Emits `tracing::info!` on creation.
/// - DB exists with `user_version` below `SCHEMA_USER_VERSION`: runs the remaining
///   migrations and updates `user_version`. Emits `tracing::warn!` on auto-migration.
/// - DB exists with `user_version` equal to `SCHEMA_USER_VERSION`: no-op.
///
/// This helper unifies the auto-init contract across CRUD handlers so users can run
/// any subcommand on a fresh directory without invoking `init` first. Idempotent
/// and safe to call before every handler that needs a ready database.
pub fn ensure_db_ready(paths: &AppPaths) -> Result<(), AppError> {
    register_vec_extension();
    paths.ensure_dirs()?;

    let db_existed = paths.db.exists();

    if !db_existed {
        tracing::info!(target: "storage",
            path = %paths.db.display(),
            schema_version = crate::constants::CURRENT_SCHEMA_VERSION,
            "creating database (auto-init)"
        );
    }

    let mut conn = open_rw(&paths.db)?;

    if !db_existed {
        apply_init_pragmas(&conn)?;
    }

    let current_user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0);
    let target_user_version = crate::constants::SCHEMA_USER_VERSION;

    // v1.2.8: `user_version` alone cannot gate this. It is an IDENTITY marker —
    // the constant is 50 so external tools recognise a sqlite-graphrag file at a
    // glance, and its own doc-comment states that bumping migrations does not
    // change it. A value that never changes cannot signal "there is something
    // new to apply": every database that reached 50 would stay there forever,
    // and V017 would never reach an existing database. The binary would then
    // accept `crate` (it no longer checks membership) while the un-migrated
    // schema still carried V008's CHECK and refused the write, surfacing as a
    // raw SQLite constraint error about a guard the caller cannot see.
    //
    // Ask the migration history instead, which is the thing that actually knows.
    let applied_schema_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM refinery_schema_history",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let target_schema_version = i64::from(crate::constants::CURRENT_SCHEMA_VERSION);

    let needs_migration = current_user_version < target_user_version
        || applied_schema_version < target_schema_version;

    if needs_migration {
        if db_existed {
            tracing::warn!(target: "storage",
                from = current_user_version,
                to = target_user_version,
                schema_from = applied_schema_version,
                schema_to = target_schema_version,
                path = %paths.db.display(),
                "auto-migrating database schema"
            );
            back_up_before_migrating(&conn, &paths.db, applied_schema_version)?;
        }
        // GAP-SG-140: `V002__vec_tables.sql` was edited after it had already been
        // applied in the field, so every legacy database carries a divergent
        // checksum for it. refinery aborts on divergence by default, which blocks
        // ALL pending migrations (exit 20) on databases below schema 16. The
        // divergence is inert: `V013__drop_vec_use_blob_embeddings.sql` already
        // drops the tables V002 created, so the historical text no longer
        // describes any live object. Tolerate divergence and keep migrating.
        run_migrations_with_foreign_keys_off(&mut conn, "auto-migration failed")?;
        conn.execute_batch(&format!("PRAGMA user_version = {target_user_version};"))?;

        if !db_existed {
            insert_default_schema_meta(&conn)?;
        }

        // Defensive re-assertion: refinery's migration runner may open internal
        // handles that revert journal_mode to delete on some platforms. Re-apply
        // WAL after migrations to guarantee the documented contract holds for
        // every command that goes through the auto-init path.
        ensure_wal_mode(&conn)?;
    }

    // G41 repair: if V013 is in history but embedding tables are missing,
    // execute V013 SQL directly. Runs unconditionally because databases
    // corrupted by G41 already have user_version=50 and skip the block above.
    crate::commands::migrate::ensure_v013_tables_exist(&conn)?;

    // G42/S1 (v1.0.79): synchronise the active embedding dimensionality
    // with the database. Existing databases keep their recorded `dim`
    // (e.g. 384 from pre-v1.0.79); an explicit env/flag override is
    // persisted back so `health --json` reports the truth. This is an
    // UPDATE of an existing `schema_meta` key — ZERO schema change.
    sync_embedding_dim_meta(&conn)?;

    Ok(())
}

/// G42/S1: two-way sync between `schema_meta.dim` and the process-wide
/// active embedding dimensionality.
///
/// - CLI flag / XDG override set → persist it into `schema_meta.dim`;
/// - no override → adopt the database value via
///   [`crate::constants::set_active_embedding_dim`] so a 384-dim database
///   keeps producing and querying 384-dim vectors even after the compiled
///   default moved to 1024;
/// - key missing (legacy/corrupt meta) → write the resolved default.
fn sync_embedding_dim_meta(conn: &Connection) -> Result<(), AppError> {
    let db_dim: Option<usize> = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'dim'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse::<usize>().ok());

    if let Some(override_dim) = crate::constants::embedding_dim_from_runtime() {
        if db_dim != Some(override_dim) {
            conn.execute(
                "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('dim', ?1)",
                rusqlite::params![override_dim.to_string()],
            )?;
        }
        return Ok(());
    }

    match db_dim {
        Some(dim) => crate::constants::set_active_embedding_dim(dim),
        None => {
            conn.execute(
                "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('dim', ?1)",
                rusqlite::params![crate::constants::embedding_dim().to_string()],
            )?;
        }
    }
    Ok(())
}

fn insert_default_schema_meta(conn: &Connection) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![crate::constants::CURRENT_SCHEMA_VERSION.to_string()],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('model', ?1)",
        rusqlite::params![crate::constants::SQLITE_GRAPHRAG_VERSION],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('dim', ?1)",
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
    Ok(())
}

/// Applies 600 permissions (owner read/write only) to the SQLite file and its WAL/SHM
/// companion files on Unix to prevent leaking private memories in shared directories
/// (e.g. multi-user /tmp, Dropbox, NFS). On Windows, NTFS DACL default is private-to-user
/// so explicit permission setting is unnecessary; a debug log records the skip. Failures
/// are silent to avoid blocking the operation when the process does not own the file
/// (e.g. read-only mount).
#[allow(unused_variables)]
fn apply_secure_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let candidates = [
            path.to_path_buf(),
            path.with_extension(format!(
                "{}-wal",
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("sqlite")
            )),
            path.with_extension(format!(
                "{}-shm",
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("sqlite")
            )),
        ];
        for file in candidates.iter() {
            if file.exists() {
                if let Ok(meta) = std::fs::metadata(file) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o600);
                    let _ = std::fs::set_permissions(file, perms);
                }
            }
        }
    }
    #[cfg(windows)]
    {
        tracing::debug!(target: "storage",
            path = %path.display(),
            "skipping Unix mode 0o600 on Windows; NTFS DACL default is private-to-user"
        );
    }
}

/// Open ro.
pub fn open_ro(path: &Path) -> Result<Connection, AppError> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    // G43: read-only commands (`recall`, `hybrid-search`) embed the QUERY
    // text, so they must adopt the database dimensionality too.
    adopt_embedding_dim(&conn);
    Ok(conn)
}

#[cfg(test)]
mod migration_cascade_tests {
    use super::*;

    /// The regression that motivated `run_migrations_with_foreign_keys_off`.
    ///
    /// Measured on 2026-08-18 against a copy of this workspace's database:
    /// migrating 16 → 17 through a bare `runner().run(conn)` reported success,
    /// moved the schema to 17, and left `relationships` at ZERO rows, down from
    /// 213 029. `V017` rebuilds `entities`, and `DROP TABLE` under foreign key
    /// enforcement performs an implicit `DELETE FROM` that fires the children's
    /// `ON DELETE CASCADE`.
    ///
    /// Every pre-existing migration test bootstraps an EMPTY database, where a
    /// cascade has nothing to delete — which is exactly why nine migrations
    /// shipped this pattern unnoticed. This test therefore inserts rows FIRST
    /// and asserts they are still there afterwards. Without the guard it fails;
    /// with it, the edge survives.
    #[test]
    fn migrating_a_populated_database_preserves_the_edges() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let db_path = tmp.path().join("populated.sqlite");

        // Stop one migration short of V017, so the rebuild is still ahead.
        let mut conn = open_rw(&db_path).expect("open");
        crate::migrations::runner()
            .set_abort_divergent(false)
            .set_target(refinery::Target::Version(16))
            .run(&mut conn)
            .expect("migrate to 16");

        conn.execute_batch(
            "INSERT INTO entities (namespace, name, type) VALUES ('global', 'alpha', 'tool');
             INSERT INTO entities (namespace, name, type) VALUES ('global', 'beta', 'tool');
             INSERT INTO relationships (namespace, source_id, target_id, relation)
               VALUES ('global', 1, 2, 'uses');",
        )
        .expect("seed rows");

        let edges_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))
            .expect("count before");
        assert_eq!(edges_before, 1, "fixture must actually have an edge");

        // Enforcement is ON here, exactly as `open_rw` leaves it in production.
        let enforced: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .expect("read pragma");
        assert_eq!(enforced, 1, "the guard is only meaningful with FK enforced");

        run_migrations_with_foreign_keys_off(&mut conn, "test migration failed")
            .expect("guarded migration must succeed");

        let edges_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))
            .expect("count after");
        assert_eq!(
            edges_after, 1,
            "V017 rebuilt `entities` and the cascade emptied `relationships`"
        );

        let entities_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
            .expect("count entities");
        assert_eq!(entities_after, 2, "entities must survive the rebuild");

        // Enforcement restored, and no row left pointing at a missing parent.
        let restored: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .expect("read pragma");
        assert_eq!(restored, 1, "enforcement must be back on afterwards");
        assert!(foreign_key_violation_counts(&conn)
            .expect("read violations")
            .is_empty());
    }

    /// With the vocabulary open, the column must accept a label the old CHECK
    /// would have refused. Pinned here because it is the schema half of the
    /// change: `entity_type.rs` can stop folding, and the write still fails if
    /// V017 never reached the database.
    #[test]
    fn the_migrated_column_accepts_a_non_canonical_label() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let db_path = tmp.path().join("open-vocab.sqlite");
        let mut conn = open_rw(&db_path).expect("open");
        run_migrations_with_foreign_keys_off(&mut conn, "test migration failed").expect("migrate");

        conn.execute(
            "INSERT INTO entities (namespace, name, type) VALUES ('global', 'axum', ?1)",
            rusqlite::params!["crate"],
        )
        .expect("a label outside the canonical thirteen must be storable");

        let stored: String = conn
            .query_row("SELECT type FROM entities WHERE name = 'axum'", [], |r| {
                r.get(0)
            })
            .expect("read back");
        assert_eq!(stored, "crate", "the label must survive verbatim");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// G43 regression: `open_rw` must adopt `schema_meta.dim` so EVERY
    /// command (not only the `ensure_db_ready` auto-init path) produces
    /// vectors of the database dimensionality. Pre-G43, `remember` /
    /// `edit` / `recall` / `hybrid-search` used the compiled default
    /// against pre-v1.0.79 384-dim databases, silently writing
    /// mixed-dim embeddings that cosine-score 0.0 against each other.
    #[test]
    #[serial_test::serial(env)]
    fn open_rw_adopts_schema_meta_dim() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("g43.sqlite");
        {
            let conn = Connection::open(&db).expect("create seed db");
            conn.execute_batch(
                "CREATE TABLE schema_meta (key TEXT PRIMARY KEY, value TEXT);
                 INSERT INTO schema_meta VALUES ('dim', '128');",
            )
            .expect("seed schema_meta");
        }
        // GAP-SG-232: nothing to clear, because the product reads no variable
        // of its own. The dim comes from `--embedding-dim`, then the XDG key
        // `embedding.dim`, then `schema_meta`, then the compiled default.
        let _conn = open_rw(&db).expect("open_rw");
        let adopted = crate::constants::embedding_dim();
        // Restore the process-wide default before asserting so a failure
        // does not leak 128 into parallel tests.
        crate::constants::set_active_embedding_dim(crate::constants::DEFAULT_EMBEDDING_DIM);
        assert_eq!(adopted, 128, "open_rw must adopt the recorded db dim (G43)");
    }

    /// G43 regression: `open_ro` (used by `recall` / `hybrid-search` to
    /// embed the QUERY text) must adopt the database dim too.
    #[test]
    #[serial_test::serial(env)]
    fn open_ro_adopts_schema_meta_dim() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("g43-ro.sqlite");
        {
            let conn = Connection::open(&db).expect("create seed db");
            conn.execute_batch(
                "CREATE TABLE schema_meta (key TEXT PRIMARY KEY, value TEXT);
                 INSERT INTO schema_meta VALUES ('dim', '256');",
            )
            .expect("seed schema_meta");
        }
        let _conn = open_ro(&db).expect("open_ro");
        let adopted = crate::constants::embedding_dim();
        crate::constants::set_active_embedding_dim(crate::constants::DEFAULT_EMBEDDING_DIM);
        assert_eq!(adopted, 256, "open_ro must adopt the recorded db dim (G43)");
    }

    /// G43: the env override always wins over the recorded database dim
    /// (precedence contract of `constants::embedding_dim`).
    #[test]
    #[serial_test::serial(env)]
    fn env_override_wins_over_schema_meta_dim() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("g43-env.sqlite");
        {
            let conn = Connection::open(&db).expect("create seed db");
            conn.execute_batch(
                "CREATE TABLE schema_meta (key TEXT PRIMARY KEY, value TEXT);
                 INSERT INTO schema_meta VALUES ('dim', '128');",
            )
            .expect("seed schema_meta");
        }
        // G-T-XDG-04: product env is gone. Schema meta dim is adopted when no
        // process-wide runtime override was installed at bootstrap.
        let _conn = open_rw(&db).expect("open_rw");
        let adopted = crate::constants::embedding_dim();
        crate::constants::set_active_embedding_dim(crate::constants::DEFAULT_EMBEDDING_DIM);
        assert_eq!(
            adopted, 128,
            "schema_meta dim is adopted when no CLI/XDG override is active"
        );
    }

    /// G43: a virgin database without `schema_meta` must open cleanly
    /// (best-effort adoption is a no-op, never an error).
    #[test]
    #[serial_test::serial(env)]
    fn open_rw_on_virgin_db_is_a_noop() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("g43-virgin.sqlite");
        crate::constants::set_active_embedding_dim(crate::constants::DEFAULT_EMBEDDING_DIM);
        let _conn = open_rw(&db).expect("open_rw on virgin db must not fail");
        assert_eq!(
            crate::constants::embedding_dim(),
            crate::constants::DEFAULT_EMBEDDING_DIM,
            "virgin db must keep the compiled default (G43)"
        );
    }
}
