//! Library-wide error type.
//!
//! `AppError` is the single error type returned by every public API in the
//! crate. Each variant maps to a deterministic exit code through
//! `AppError::exit_code`, which the binary propagates to the shell on
//! failure. See the README for the full exit code contract.

use crate::i18n::{current, Language};
use crate::spawn::preflight::PreFlightError;
use thiserror::Error;

/// Unified error type for all CLI and library operations.
///
/// Each variant corresponds to a distinct failure category. The
/// [`AppError::exit_code`] method converts a variant into a stable numeric
/// code so that shell callers and LLM agents can route on it.
///
/// # SemVer Policy
///
/// This enum is `#[non_exhaustive]`. New variants may be added in minor
/// releases without breaking downstream match arms (use a wildcard `_`).
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum AppError {
    /// Input failed schema, length or format validation. Maps to exit code `1`.
    ///
    /// This variant groups multiple validation failure causes. Callers that need
    /// programmatic retry decisions should use [`AppError::is_retryable`] instead
    /// of parsing the message string.
    #[error("validation error: {0}")]
    Validation(String),

    /// External binary required for operation was not found in PATH. Maps to exit code `1`.
    #[error("binary not found: {name} — ensure it is installed and in PATH")]
    BinaryNotFound {
        /// Name associated with this error.
        name: String,
    },

    /// Remote service signaled rate limiting; caller should retry with backoff. Maps to exit code `1`.
    #[error("rate limited: {detail}")]
    RateLimited {
        /// Human-readable detail message.
        detail: String,
    },

    /// Operation exceeded its time budget. Maps to exit code `1`.
    #[error("timeout after {duration_secs}s: {operation}")]
    Timeout {
        /// Operation.
        operation: String,
        /// Duration secs.
        duration_secs: u64,
    },

    /// A memory or entity with the same `(namespace, name)` already exists. Maps to exit code `9`.
    #[error("duplicate detected: {0}")]
    Duplicate(String),

    /// Optimistic update lost the race because `updated_at` changed. Maps to exit code `3`.
    #[error("conflict: {0}")]
    Conflict(String),

    /// The requested record does not exist or was soft-deleted. Maps to exit code `4`.
    #[error("not found: {0}")]
    NotFound(String),

    /// Memory lookup by `(namespace, name)` returned no row. Maps to exit code `4`.
    ///
    /// G55 S2 (v1.0.80): structural variant that carries the requested identifier
    /// and namespace, eliminating the "not found: unknown in namespace 'X'" class
    /// of bugs that masked which lookup target failed. The display format matches
    /// the legacy string-based `NotFound` so the i18n replace-chain and external
    /// scripts that pattern-match on `memory not found: name='N' in namespace 'NS'`
    /// keep working.
    #[error("memory not found: name='{name}' in namespace '{namespace}'")]
    MemoryNotFound {
        /// Name associated with this error.
        name: String,
        /// Namespace scope.
        namespace: String,
    },

    /// Memory lookup by integer `id` returned no row. Maps to exit code `4`.
    #[error("memory not found: id={id}")]
    MemoryNotFoundById {
        /// Numeric identifier.
        id: i64,
    },

    /// GAP-SG-78: an entity referenced by a queued enrich item does not yet
    /// exist in `entities`. Maps to exit code `4`.
    ///
    /// # Cause
    ///
    /// Distinct from the terminal [`Self::NotFound`] / [`Self::MemoryNotFound`]
    /// cases (a memory that was deleted or renamed, permanently gone). An
    /// entity can be referenced by a queue row BEFORE it is materialized: a
    /// later enrich pass creates the entity, so its absence now is TRANSITORY,
    /// not terminal. Collapsing both into a single `NotFound` string sent every
    /// such item to the dead-letter on the first failure.
    ///
    /// # When it occurs
    ///
    /// Raised by the entity call-sites of `enrich` — `entity-descriptions`
    /// (`call_entity_description`) and `entity-type-validate`
    /// (`call_entity_type_validate`) — when the `(namespace, name)` lookup
    /// returns no row. Classified as [`Self::is_retryable`] so the item is
    /// rescheduled until `--max-attempts` is exhausted.
    #[error("entity '{name}' not yet materialized in namespace '{namespace}'")]
    EntityNotYetMaterialized {
        /// Name associated with this error.
        name: String,
        /// Namespace scope.
        namespace: String,
    },

    /// Namespace could not be resolved from flag, environment or markers. Maps to exit code `5`.
    #[error("namespace not resolved: {0}")]
    NamespaceError(String),

    /// Payload exceeded one of the configured body, name or batch limits. Maps to exit code `6`.
    ///
    /// v1.1.1 (P11): kept for caps other than the body-bytes and chunk-count
    /// ceilings, which now have the typed [`Self::BodyTooLarge`] and
    /// [`Self::TooManyChunks`] variants so the operator can tell WHICH cap
    /// fired without parsing the message.
    #[error("limit exceeded: {0}")]
    LimitExceeded(String),

    /// Body payload exceeded [`crate::constants::MAX_MEMORY_BODY_LEN`] bytes.
    /// Maps to exit code `6` (same contract as [`Self::LimitExceeded`]).
    ///
    /// v1.1.1 (P11): the two independent write ceilings — body bytes and chunk
    /// count — used to collapse into the generic `LimitExceeded` string, so an
    /// operator hitting exit 6 could not tell WHICH cap fired. This variant
    /// carries the measured size and the cap, and the message names the
    /// constant, so both the stderr line and the JSON envelope identify the
    /// ceiling deterministically (never by substring matching).
    #[error(
        "limit exceeded: body is {bytes} bytes, above the {limit}-byte cap \
         (MAX_MEMORY_BODY_LEN); split the content into multiple memories"
    )]
    /// Body too large.
    BodyTooLarge {
        /// Observed size in bytes.
        bytes: u64,
        /// Configured limit.
        limit: u64,
    },

    /// Chunking produced more chunks than
    /// [`crate::constants::REMEMBER_MAX_SAFE_MULTI_CHUNKS`]. Maps to exit
    /// code `6` (same contract as [`Self::LimitExceeded`]).
    ///
    /// v1.1.1 (P11): counterpart of [`Self::BodyTooLarge`] for the chunk-count
    /// ceiling. Carries the measured chunk count and the cap so the operator
    /// can distinguish a chunk overflow from a byte overflow on exit 6.
    #[error(
        "limit exceeded: document produces {chunks} chunks, above the \
         {limit}-chunk cap (REMEMBER_MAX_SAFE_MULTI_CHUNKS); split the \
         document before writing"
    )]
    /// Too many chunks.
    TooManyChunks {
        /// Observed chunk count.
        chunks: usize,
        /// Configured limit.
        limit: usize,
    },

    /// Body exceeded [`crate::constants::EMBEDDING_REQUEST_MAX_TOKENS`] tokens
    /// (conservative cl100k proxy for the `qwen/qwen3-embedding-8b` window).
    /// Maps to exit code `6` (same contract as [`Self::LimitExceeded`]).
    ///
    /// v1.1.2 (Gap 2): third typed payload ceiling alongside
    /// [`Self::BodyTooLarge`] (bytes) and [`Self::TooManyChunks`] (chunks).
    /// The token cap used to surface as a generic `Validation` (exit 1) deep
    /// inside the REST embedding client; it now fires at the write-command
    /// boundary with the estimated token count and the cap, so the operator
    /// can tell WHICH ceiling fired without substring matching (GAP-SG-73).
    #[error(
        "limit exceeded: body is {tokens} tokens (estimated), above the \
         {limit}-token cap (EMBEDDING_REQUEST_MAX_TOKENS); split the content \
         into multiple memories"
    )]
    /// Too many tokens.
    TooManyTokens {
        /// Observed token count.
        tokens: u64,
        /// Configured limit.
        limit: u64,
    },

    /// Low-level SQLite error propagated from `rusqlite`. Maps to exit code `10`.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Embedding generation via `fastembed` failed or produced the wrong shape. Maps to exit code `11`.
    #[error("embedding error: {0}")]
    Embedding(String),

    /// The `sqlite-vec` extension could not load or register its virtual table. Maps to exit code `12`.
    #[error("sqlite-vec extension failed: {0}")]
    VecExtension(String),

    /// SQLite returned `SQLITE_BUSY` after exhausting retries. Maps to exit code `15` (was `13` before v2.0.0; relocated to free `13` for BatchPartialFailure per PRD).
    #[error("database busy: {0}")]
    DbBusy(String),

    /// Batch operation failed partially — N of M items failed. Maps to exit code `13` (PRD 1822).
    ///
    /// Reserved for use in `import`, `reindex` and batch stdin (BLOCK 3/4). Variant present
    /// since v2.0.0 even if call-sites do not yet exist — stable exit code mapping.
    #[error("batch partial failure: {failed} of {total} items failed")]
    BatchPartialFailure {
        /// Total items processed.
        total: usize,
        /// Number of failed items.
        failed: usize,
    },

    /// Filesystem I/O error while reading or writing the database or cache. Maps to exit code `14`.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Unexpected internal error surfaced through `anyhow`. Maps to exit code `20`.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),

    /// JSON serialization or deserialization failure. Maps to exit code `20`.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Another instance is already running and holds the advisory lock. Maps to exit code `75`.
    ///
    /// Use `--allow-parallel` to skip the lock or `--wait-lock SECONDS` to retry.
    #[error("lock busy: {0}")]
    LockBusy(String),

    /// All concurrency slots are occupied after the wait timeout. Maps to exit code `75`.
    ///
    /// Occurs when [`crate::constants::MAX_CONCURRENT_CLI_INSTANCES`] instances are already
    /// active and the wait limit [`crate::constants::CLI_LOCK_DEFAULT_WAIT_SECS`] is exhausted.
    #[error(
        "all {max} concurrency slots occupied after waiting {waited_secs}s (exit 75); \
         use --max-concurrency or wait for other invocations to finish"
    )]
    /// All slots full.
    AllSlotsFull {
        /// Maximum allowed value.
        max: usize,
        /// Seconds spent waiting.
        waited_secs: u64,
    },

    /// A heavy long-running job is already running for this job_type/namespace
    /// pair. Maps to exit code `75` (the same `EX_TEMPFAIL` code used by the
    /// CLI semaphore).
    ///
    /// G28-B (v1.0.68): ensures at most one `enrich`, `ingest --mode
    /// claude-code`, or `ingest --mode codex` runs at a time per namespace.
    /// Use `--wait-job-singleton <SECONDS>` (per-command) to poll until the
    /// other invocation finishes.
    #[error(
        "job {job_type} for namespace '{namespace}' is already running (exit 75); \
         wait for it to finish or pass --wait-job-singleton <SECONDS>"
    )]
    /// Job singleton locked.
    JobSingletonLocked {
        /// Job type identifier.
        job_type: String,
        /// Namespace scope.
        namespace: String,
    },

    /// G45: an LLM embedding operation is already running against the
    /// same `(namespace, db)` pair in another process. Exit code 75
    /// (retryable). The caller can pass `--wait-embed-singleton
    /// <SECONDS>` to poll until the lock drops.
    #[error(
        "embedding singleton for namespace '{namespace}' is already held (exit 75); \
         another CLI is calling the LLM on this database; pass --wait-embed-singleton <SECONDS> to wait"
    )]
    /// Embedding singleton locked.
    EmbeddingSingletonLocked {
        /// Namespace scope.
        namespace: String,
    },

    /// Available memory is below the minimum required to load the model. Maps to exit code `77`.
    ///
    /// Returned when `sysinfo` reports available memory below
    /// [`crate::constants::MIN_AVAILABLE_MEMORY_MB`] MiB before starting the ONNX model load.
    #[error(
        "available memory ({available_mb}MB) below required minimum ({required_mb}MB) \
         to load the model; abort other loads or use --skip-memory-guard (exit 77)"
    )]
    /// Low memory.
    LowMemory {
        /// Available memory in megabytes.
        available_mb: u64,
        /// Required memory in megabytes.
        required_mb: u64,
    },

    /// v1.0.82 (GAP-002 final): shutdown was requested via SIGINT, SIGTERM or
    /// SIGHUP before the current command completed. Maps to exit code
    /// [`crate::constants::SHUTDOWN_EXIT_CODE`] (19).
    ///
    /// The signal name is preserved in the `signal` field so the JSON
    /// envelope emitted before exit can route the operator to a
    /// deterministic branch. Distinct from the legacy `128 + signal`
    /// Unix convention (130/143/129) so LLM agents can match on a
    /// single code for "cancelled by user".
    #[error("shutdown signal received: {signal}")]
    Shutdown {
        /// Signal that triggered shutdown.
        signal: String,
    },

    /// v1.0.87 (GAP-META-005, ADR-0045): pre-flight validation gate
    /// rejected the spawn before fork. Maps to exit code `16`.
    ///
    /// The `source` field carries the structured [`PreFlightError`]
    /// variant so callers and operators can route on the specific
    /// failure class (BinaryNotFound, ArgvExceedsArgMax,
    /// McpConfigInlineJsonRejected, McpConfigPathMissing,
    /// McpConfigPathInvalidJson, WalkUpMcpJsonInvalid,
    /// OutputBufferTooSmall, ClaudeConfigDirNotEmpty) instead of
    /// parsing the legacy `detail: String` representation.
    ///
    /// This variant is **permanent** — retrying the same argv will fail
    /// identically. Operators must fix the underlying condition (install
    /// the binary, shorten the body, override `CLAUDE_CONFIG_DIR`,
    /// substitute the inline `--mcp-config '{}'` for a tempfile path,
    /// etc.) before retrying.
    #[error("preflight validation failed: {source}")]
    PreFlightFailed {
        /// Underlying preflight error.
        source: Box<PreFlightError>,
    },

    /// v1.0.97 (GAP-SG-01/03): the OpenRouter provider returned a structured
    /// error object (an `error` field carrying `code` and `message`), often
    /// inside an HTTP 200 body (e.g. token/context-length overflow). Maps to
    /// exit code `1`.
    ///
    /// Modelling the provider rejection as a typed variant — instead of the
    /// generic `Embedding`/`Validation` string — stops the optimistic success
    /// parse from masking the cause with a misleading missing-field error. The
    /// `code` and `message` carry the REAL provider diagnostics.
    ///
    /// This variant is **permanent**: a structured provider error in a success
    /// body is a content or configuration rejection that retrying the identical
    /// request will not fix. Genuine rate limiting surfaces as HTTP 429 and is
    /// retried inside the HTTP client (then exposed via `RateLimited` when
    /// attempts are exhausted), so it never reaches callers as `ProviderError`.
    #[error("provider error (code {code}): {message}")]
    ProviderError {
        /// Provider error code.
        code: String,
        /// Provider error message.
        message: String,
    },
}

/// Bridges the structured [`PreFlightError`] produced by the
/// pre-flight validation gate (v1.0.87, ADR-0045) into the unified
/// [`AppError`] envelope. Lets spawners use the `?` operator instead
/// of hand-rolling `AppError::PreFlightFailed { source: ... }` at every
/// call site, and keeps the variant alive as the canonical exit code 16
/// path rather than the dead code it was at v1.0.87.
impl From<PreFlightError> for AppError {
    fn from(source: PreFlightError) -> Self {
        AppError::PreFlightFailed {
            source: Box::new(source),
        }
    }
}

impl AppError {
    /// Returns the deterministic process exit code for this error variant.
    ///
    /// The codes follow the contract documented in the README: `1` for
    /// validation, `9` for duplicates (moved from `2` in v1.0.52), `3` for conflicts, `4` for missing
    /// records, `5` for namespace errors, `6` for limit violations, `10`–`14`
    /// for infrastructure failures, `13` for BatchPartialFailure (PRD 1822),
    /// `15` for DbBusy (migrated from `13` in v2.0.0), `20` for internal errors,
    /// `75` (EX_TEMPFAIL) when the advisory CLI lock is held or all concurrency
    /// slots are exhausted, and `77` when available memory is insufficient to
    /// load the embedding model.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqlite_graphrag::errors::AppError;
    ///
    /// assert_eq!(AppError::Validation("invalid field".into()).exit_code(), 1);
    /// assert_eq!(AppError::Duplicate("ns/mem".into()).exit_code(), 9);
    /// assert_eq!(AppError::Conflict("ts changed".into()).exit_code(), 3);
    /// assert_eq!(AppError::NotFound("id 42".into()).exit_code(), 4);
    /// assert_eq!(AppError::NamespaceError("no marker".into()).exit_code(), 5);
    /// assert_eq!(AppError::LimitExceeded("body too large".into()).exit_code(), 6);
    /// assert_eq!(AppError::Embedding("wrong dim".into()).exit_code(), 11);
    /// assert_eq!(AppError::DbBusy("retries exhausted".into()).exit_code(), 15);
    /// assert_eq!(AppError::LockBusy("another instance".into()).exit_code(), 75);
    /// ```
    #[inline]
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Validation(_) => 1,
            Self::BinaryNotFound { .. } => 1,
            Self::RateLimited { .. } => 1,
            Self::Timeout { .. } => 1,
            Self::Duplicate(_) => crate::constants::DUPLICATE_EXIT_CODE,
            Self::Conflict(_) => 3,
            Self::NotFound(_) => 4,
            Self::MemoryNotFound { .. } => 4,
            Self::MemoryNotFoundById { .. } => 4,
            Self::EntityNotYetMaterialized { .. } => 4,
            Self::NamespaceError(_) => 5,
            Self::LimitExceeded(_) => 6,
            Self::BodyTooLarge { .. } => 6,
            Self::TooManyChunks { .. } => 6,
            Self::TooManyTokens { .. } => 6,
            Self::Database(_) => 10,
            Self::Embedding(_) => 11,
            Self::VecExtension(_) => 12,
            Self::BatchPartialFailure { .. } => crate::constants::BATCH_PARTIAL_FAILURE_EXIT_CODE,
            Self::DbBusy(_) => crate::constants::DB_BUSY_EXIT_CODE,
            Self::Io(_) => 14,
            Self::Internal(_) => 20,
            Self::Json(_) => 20,
            Self::LockBusy(_) => crate::constants::CLI_LOCK_EXIT_CODE,
            Self::AllSlotsFull { .. } => crate::constants::CLI_LOCK_EXIT_CODE,
            Self::JobSingletonLocked { .. } => crate::constants::CLI_LOCK_EXIT_CODE,
            Self::EmbeddingSingletonLocked { .. } => crate::constants::CLI_LOCK_EXIT_CODE,
            Self::LowMemory { .. } => crate::constants::LOW_MEMORY_EXIT_CODE,
            Self::Shutdown { .. } => crate::constants::SHUTDOWN_EXIT_CODE,
            Self::PreFlightFailed { .. } => 16,
            Self::ProviderError { .. } => 1,
        }
    }

    /// Returns `true` when the error is transient and the operation may
    /// succeed on retry with backoff.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqlite_graphrag::errors::AppError;
    ///
    /// assert!(AppError::DbBusy("busy".into()).is_retryable());
    /// assert!(AppError::LockBusy("held".into()).is_retryable());
    /// assert!(!AppError::NotFound("x".into()).is_retryable());
    /// assert!(!AppError::Validation("bad".into()).is_retryable());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::DbBusy(_)
                | Self::LockBusy(_)
                | Self::AllSlotsFull { .. }
                | Self::JobSingletonLocked { .. }
                | Self::EmbeddingSingletonLocked { .. }
                | Self::LowMemory { .. }
                | Self::RateLimited { .. }
                | Self::Timeout { .. }
                | Self::EntityNotYetMaterialized { .. }
        )
    }

    /// Returns `true` when shutdown was requested by the user via signal.
    ///
    /// Distinct from `is_permanent` because shutdown is a USER intent, not
    /// a state to retry against. The operation should be retried with
    /// `--resume` (GAP-001) when the persisted staging row still exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqlite_graphrag::errors::AppError;
    ///
    /// assert!(AppError::Shutdown { signal: "SIGINT".into() }.is_shutdown());
    /// assert!(!AppError::Validation("x".into()).is_shutdown());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_shutdown(&self) -> bool {
        matches!(self, Self::Shutdown { .. })
    }

    /// Returns `true` when the error is permanent and must NOT be retried.
    ///
    /// Complement to [`Self::is_retryable`]. Errors not classified by either
    /// method (e.g. `Database`, `Io`, `Internal`) are ambiguous — the caller
    /// decides based on context.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqlite_graphrag::errors::AppError;
    ///
    /// assert!(AppError::Validation("bad".into()).is_permanent());
    /// assert!(!AppError::DbBusy("busy".into()).is_permanent());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            Self::Validation(_)
                | Self::BinaryNotFound { .. }
                | Self::Duplicate(_)
                | Self::NotFound(_)
                | Self::MemoryNotFound { .. }
                | Self::MemoryNotFoundById { .. }
                | Self::NamespaceError(_)
                | Self::LimitExceeded(_)
                | Self::BodyTooLarge { .. }
                | Self::TooManyChunks { .. }
                | Self::TooManyTokens { .. }
                | Self::VecExtension(_)
                | Self::PreFlightFailed { .. }
                | Self::ProviderError { .. }
        )
    }

    /// GAP-SG-39: returns an actionable remediation hint for the error, surfaced
    /// in the stdout error envelope as the `suggestion` field. The hint tells the
    /// operator HOW to recover instead of leaving an exit code without guidance —
    /// this is what makes a write rejection (e.g. a malformed name) observable and
    /// fixable. Returns `None` for variants whose own message is already
    /// self-remediating.
    #[must_use]
    pub fn suggestion(&self) -> Option<&'static str> {
        match self {
            Self::Validation(_) => Some(
                "review the input against the command's --help; names must be kebab-case (lowercase letters, digits, hyphens) and bodies non-empty",
            ),
            Self::Duplicate(_) => {
                Some("pass --force-merge to update the existing memory instead of failing")
            }
            Self::Conflict(_) => Some(
                "another writer changed the row; re-read with `read --name <n> --json` and retry with a fresh --expected-updated-at",
            ),
            Self::NotFound(_) | Self::MemoryNotFound { .. } | Self::MemoryNotFoundById { .. } => {
                Some("verify the name/id and namespace with `list --json` or `read --name <n> --json`")
            }
            Self::NamespaceError(_) => {
                // GAP-SG-103: product env is not read (G-T-XDG-04). Point operators
                // at the real channels: CLI flag and XDG `namespace.default`.
                Some("set --namespace or `config set namespace.default <name>`; inspect with `namespace-detect --json`")
            }
            Self::LimitExceeded(_) => {
                Some("split the input into smaller memories or raise the documented cap before retrying")
            }
            Self::BodyTooLarge { .. } => {
                Some("the body-bytes cap (MAX_MEMORY_BODY_LEN) fired; split the content into multiple memories or use --body-file")
            }
            Self::TooManyChunks { .. } => {
                Some("the chunk-count cap (REMEMBER_MAX_SAFE_MULTI_CHUNKS) fired; split the document into smaller memories before writing")
            }
            Self::TooManyTokens { .. } => {
                Some("the token cap (EMBEDDING_REQUEST_MAX_TOKENS) fired; split the content into multiple memories, keeping each under ~25000 tokens")
            }
            Self::Embedding(_) => Some(
                "verify the embedding backend and OPENROUTER_API_KEY; re-run `enrich --operation re-embed` once resolved",
            ),
            Self::Database(_) | Self::DbBusy(_) => {
                Some("run `health --json` then `vacuum --json`; widen --wait-lock if the database is busy")
            }
            Self::Io(_) => Some("check the path exists and is writable, then retry"),
            Self::RateLimited { .. } => {
                Some("wait for the reported retry-after window, then retry")
            }
            Self::LockBusy(_) | Self::AllSlotsFull { .. } | Self::JobSingletonLocked { .. } => {
                Some("wait for the other invocation to finish or pass --wait-lock / --wait-job-singleton")
            }
            _ => None,
        }
    }

    /// Returns the localized error message in the active language (`--lang` / XDG `i18n.lang`).
    ///
    /// In English the text is identical to the `Display` generated by thiserror.
    /// In Portuguese the prefixes and messages are translated to PT-BR.
    pub fn localized_message(&self) -> String {
        self.localized_message_for(current())
    }

    /// Returns the localized message for the explicitly provided language.
    /// Useful in tests that cannot depend on the global `OnceLock`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqlite_graphrag::errors::AppError;
    /// use sqlite_graphrag::i18n::Language;
    ///
    /// let err = AppError::NotFound("mem-xyz".into());
    ///
    /// let en = err.localized_message_for(Language::English);
    /// assert!(en.contains("not found"));
    ///
    /// let pt = err.localized_message_for(Language::Portuguese);
    /// assert!(pt.contains("n\u{e3}o encontrado"));
    /// ```
    pub fn localized_message_for(&self, lang: Language) -> String {
        match lang {
            Language::English => self.to_string(),
            Language::Portuguese => self.to_string_pt(),
        }
    }

    fn to_string_pt(&self) -> String {
        use crate::i18n::validation::app_error_pt as pt;
        match self {
            Self::Validation(msg) => pt::validation(msg),
            Self::BinaryNotFound { name } => pt::binary_not_found(name),
            Self::RateLimited { detail } => pt::rate_limited(detail),
            Self::Timeout {
                operation,
                duration_secs,
            } => pt::timeout(operation, *duration_secs),
            Self::Duplicate(msg) => pt::duplicate(msg),
            Self::Conflict(msg) => pt::conflict(msg),
            Self::NotFound(msg) => pt::not_found(msg),
            Self::MemoryNotFound { name, namespace } => pt::memory_not_found(name, namespace),
            Self::MemoryNotFoundById { id } => pt::memory_not_found_by_id(*id),
            Self::EntityNotYetMaterialized { name, namespace } => {
                pt::entity_not_yet_materialized(name, namespace)
            }
            Self::NamespaceError(msg) => pt::namespace_error(msg),
            Self::LimitExceeded(msg) => pt::limit_exceeded(msg),
            Self::BodyTooLarge { bytes, limit } => pt::body_too_large(*bytes, *limit),
            Self::TooManyChunks { chunks, limit } => pt::too_many_chunks(*chunks, *limit),
            Self::TooManyTokens { tokens, limit } => pt::too_many_tokens(*tokens, *limit),
            Self::Database(e) => pt::database(&e.to_string()),
            Self::Embedding(msg) => pt::embedding(msg),
            Self::VecExtension(msg) => pt::vec_extension(msg),
            Self::DbBusy(msg) => pt::db_busy(msg),
            Self::BatchPartialFailure { total, failed } => {
                pt::batch_partial_failure(*total, *failed)
            }
            Self::Io(e) => pt::io(&e.to_string()),
            Self::Internal(e) => pt::internal(&e.to_string()),
            Self::Json(e) => pt::json(&e.to_string()),
            Self::LockBusy(msg) => pt::lock_busy(msg),
            Self::AllSlotsFull { max, waited_secs } => pt::all_slots_full(*max, *waited_secs),
            Self::JobSingletonLocked {
                job_type,
                namespace,
            } => pt::job_singleton_locked(job_type, namespace),
            Self::EmbeddingSingletonLocked { namespace } => {
                pt::embedding_singleton_locked(namespace)
            }
            Self::LowMemory {
                available_mb,
                required_mb,
            } => pt::low_memory(*available_mb, *required_mb),
            Self::Shutdown { signal } => pt::shutdown(signal),
            Self::PreFlightFailed { source } => pt::preflight_failed(&source.to_string()),
            Self::ProviderError { code, message } => pt::provider_error(code, message),
        }
    }
}
#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
