//! SQLite pragmas, busy-retry policy and schema versions.
//!
//! Split out of the former single-file `constants.rs` in v1.2.5;
//! every item is re-exported by the parent module, so `crate::constants::X`
//! resolves exactly as before.

/// Maximum attempts when a statement returns `SQLITE_BUSY`.
pub const MAX_SQLITE_BUSY_RETRIES: u32 = 5;

/// Base delay in milliseconds for the first SQLITE_BUSY retry.
///
/// Each subsequent attempt doubles the delay (exponential backoff):
/// 300 ms → 600 ms → 1200 ms → 2400 ms → 4800 ms (≈ 9.3 s total).
pub const SQLITE_BUSY_BASE_DELAY_MS: u64 = 300;

/// Ceiling on ONE busy-retry sleep, in milliseconds.
///
/// Doubling without a ceiling turns two configuration knobs into an unbounded
/// wait, and both are operator-settable: `db.busy_retries` and
/// `db.busy_base_delay_ms`. Measured on a workstation carrying `12` and `600`,
/// the twelfth attempt alone sleeps past twenty minutes and the full schedule
/// costs roughly half an hour — for a single contended statement. That is not a
/// tuning choice anyone made; it is what exponential growth does to a knob whose
/// range nobody bounded.
///
/// Five seconds matches [`BUSY_TIMEOUT_MILLIS`], so the longest a retry waits is
/// the same order as the lock timeout SQLite itself applies. Attempts are still
/// capped by `db.busy_retries`; only the growth of each sleep stops here.
pub const SQLITE_BUSY_MAX_DELAY_MS: u64 = 5_000;

/// Query timeout applied to statements in milliseconds.
pub const QUERY_TIMEOUT_MILLIS: u64 = 5_000;

/// `PRAGMA busy_timeout` value applied on every connection.
pub const BUSY_TIMEOUT_MILLIS: i32 = 5_000;

/// `PRAGMA cache_size` value in kibibytes (negative means KiB).
pub const CACHE_SIZE_KB: i32 = -64_000;

/// `PRAGMA mmap_size` value in bytes applied to each connection.
pub const MMAP_SIZE_BYTES: i64 = 268_435_456;

/// `PRAGMA wal_autocheckpoint` threshold in pages.
pub const WAL_AUTOCHECKPOINT_PAGES: i32 = 1_000;

/// Canonical value of `PRAGMA user_version` written after migrations.
///
/// **Why 50 instead of `CURRENT_SCHEMA_VERSION` (15)?**
/// `user_version` is a 32-bit integer that SQLite reserves for application use.
/// We deliberately set it to a project-specific marker (50 = decimal) so external
/// inspection tools (`sqlite3 db.sqlite "PRAGMA user_version"`, the `file` command,
/// SQLite browser GUIs) can distinguish a sqlite-graphrag database from a generic
/// SQLite file at a glance. The application-level schema version (15, matching
/// `CURRENT_SCHEMA_VERSION`) is stored in the `schema_meta` table and exposed via
/// `health --json`/`stats --json`. Bumping migrations does NOT change this constant.
/// Refinery uses its own `refinery_schema_history` table for migration bookkeeping.
pub const SCHEMA_USER_VERSION: i64 = 50;

/// Current schema version, equal to the highest migration number in `migrations/Vnnn__*.sql`.
///
/// Added in v1.0.27 as a runtime and test sanity check.
/// Must be bumped in sync with new Refinery migrations; the unit test
/// `schema_version_matches_migrations_count` validates this automatically.
pub const CURRENT_SCHEMA_VERSION: u32 = 16;

/// Pause, in milliseconds, between `sqlite3_backup_step` retries after a
/// transient `Busy`/`Locked`.
///
/// The backup loop is already bounded by the caller's own deadline; this value
/// only stops the retry from becoming a busy-spin. Coordination wait, so it
/// takes no XDG key.
pub const BACKUP_BUSY_RETRY_DELAY_MS: u64 = 50;

#[cfg(test)]
mod tests_schema_version {
    use super::CURRENT_SCHEMA_VERSION;

    #[test]
    fn schema_version_matches_migrations_count() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let migrations_dir = std::path::Path::new(manifest_dir).join("migrations");
        let count = std::fs::read_dir(&migrations_dir)
            .expect("migrations directory must exist")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with('V'))
            .count() as u32;
        assert_eq!(
            CURRENT_SCHEMA_VERSION, count,
            "CURRENT_SCHEMA_VERSION ({CURRENT_SCHEMA_VERSION}) must equal the number of V*.sql migrations ({count})"
        );
    }
}
