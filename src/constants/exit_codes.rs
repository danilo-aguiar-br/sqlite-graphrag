//! Process exit codes with a documented meaning.
//!
//! Split out of the former single-file `constants.rs` in v1.2.5;
//! every item is re-exported by the parent module, so `crate::constants::X`
//! resolves exactly as before.

/// Exit code for partial batch failure (PRD line 1822). Conflicts with DbBusy in v1.x;
/// in v2.0.0 DbBusy migrates to 15 and this code takes 13 per PRD.
pub const BATCH_PARTIAL_FAILURE_EXIT_CODE: i32 = 13;

/// Exit code for DbBusy in v2.0.0 (migrated from 13 to free 13 for batch failure).
pub const DB_BUSY_EXIT_CODE: i32 = 15;

/// Process exit code returned when the lock is busy and no wait was requested (EX_TEMPFAIL).
pub const CLI_LOCK_EXIT_CODE: i32 = 75;

/// Process exit code returned when stdout is a closed pipe.
///
/// `141` is `128 + SIGPIPE(13)`, the Unix convention a shell reports when a
/// consumer such as `head` or `jaq` exits early. Windows has no SIGPIPE, so the
/// same number is produced there by classifying the stdout write error as
/// [`std::io::ErrorKind::BrokenPipe`] — the exit-code contract is identical on
/// Linux, macOS and Windows.
pub const BROKEN_PIPE_EXIT_CODE: u8 = 141;

/// Process exit code returned when available memory is below [`crate::constants::MIN_AVAILABLE_MEMORY_MB`].
///
/// Value `77` is `EX_NOPERM` in glibc sysexits, reused here to indicate
/// "insufficient system resource to proceed".
pub const LOW_MEMORY_EXIT_CODE: i32 = 77;

/// Process exit code returned when a duplicate memory or entity is detected (exit 9).
///
/// Moved from `2` to `9` in v1.0.52 to free exit code `2` for future use and align
/// with the PRD exit code contract. Shell callers and LLM agents must use `9` from
/// this version onwards.
pub const DUPLICATE_EXIT_CODE: i32 = 9;

/// Process exit code returned when the argv is a valid parse but an invalid
/// combination (`EX_USAGE` in spirit, `2` by this project's contract).
///
/// GAP-SG-201 / GAP-SG-202 / GAP-SG-203 / GAP-SG-204: the agent-native surface
/// can only discover some usage errors once it sees the envelope — a `--filter`
/// key that exists nowhere in the result elements, or a predicate evaluated over
/// a page the query already truncated. Refusing those needs an error that maps
/// to the code clap already returns for a bad command line, so an agent branches
/// on one number for "you asked for something impossible" regardless of whether
/// the parser or the surface caught it.
///
/// `2` rather than `EX_USAGE` (64): [`DUPLICATE_EXIT_CODE`] above was moved off
/// `2` in v1.0.52 precisely to free it, `src/main.rs` already returns it for a
/// rejected flag, and introducing 64 now would give this binary two codes for
/// one meaning — which the exit-code rules forbid.
pub const USAGE_EXIT_CODE: i32 = 2;

/// Process exit code returned when shutdown is requested via SIGINT/SIGTERM/SIGHUP
/// (v1.0.82, GAP-002 final).
///
/// The shell sees this code INSTEAD of the legacy `128 + signal` (130/143/129) so
/// that LLM agents and orchestrators can branch on a single deterministic value
/// when the operation was cancelled by the user. The signal name is preserved in
/// the JSON envelope emitted before exit (`{"code":19,"signal":"SIGINT",...}`).
pub const SHUTDOWN_EXIT_CODE: i32 = 19;
