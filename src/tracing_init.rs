//! Centralized tracing subscriber initialization.
//!
//! Configures the global subscriber with JSON or pretty format,
//! installs the panic hook and the log-to-tracing bridge.
//!
//! GAP-SG-99 / GAP-SG-108: when XDG `log.to_file` is truthy, also write a
//! rolling file under the cache dir via `tracing-appender`. The returned
//! [`WorkerGuard`](tracing_appender::non_blocking::WorkerGuard) MUST be held
//! until process exit so buffers flush.

use std::sync::Mutex;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Process-wide guard so the non-blocking appender flushes on drop.
///
/// Held in a static so the signal handler (and early `process::exit` paths)
/// can still flush without access to `main` stack frames. Prefer
/// [`flush_tracing`] over bare `process::exit` without a prior flush
/// (tracing-appender WorkerGuard docs: abrupt exit may lose buffered lines).
static FILE_GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);

/// Initializes the global tracing subscriber, panic hook, and log bridge.
///
/// Must be called exactly once, before any tracing events are emitted.
/// After this call, panics on any thread produce `tracing::error!` events,
/// and `log` crate events from dependencies (refinery, ureq, ort) are
/// forwarded to the tracing subscriber.
///
/// Stderr is always the agent-facing diagnostic stream. File logging is
/// optional via XDG `log.to_file` (GAP-SG-108).
pub fn init_tracing(log_level: &str, log_format: &str) {
    // TR02: the log→tracing bridge is activated automatically by
    // tracing-subscriber's built-in `tracing-log` feature (default).
    // Calling LogTracer::init() separately would conflict with the
    // global logger that tracing-subscriber installs via .init().
    let use_ansi = crate::terminal::should_use_ansi();
    let to_file = crate::config::get_setting("log.to_file")
        .ok()
        .flatten()
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);

    let file_writer = if to_file {
        let dir = crate::paths::cache_dir()
            .unwrap_or_else(|_| std::env::temp_dir().join("sqlite-graphrag"));
        let log_dir = dir.join("logs");
        let _ = std::fs::create_dir_all(&log_dir);
        let rotation = crate::config::get_setting("log.rotation")
            .ok()
            .flatten()
            .unwrap_or_else(|| "daily".into());
        let appender = match rotation.as_str() {
            "hourly" => tracing_appender::rolling::hourly(&log_dir, "sqlite-graphrag.log"),
            "never" => tracing_appender::rolling::never(&log_dir, "sqlite-graphrag.log"),
            _ => tracing_appender::rolling::daily(&log_dir, "sqlite-graphrag.log"),
        };
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);
        if let Ok(mut slot) = FILE_GUARD.lock() {
            *slot = Some(guard);
        }
        Some(non_blocking)
    } else {
        None
    };

    // Always write diagnostics to stderr (agent-native). Optionally tee to file.
    if let Some(nb) = file_writer {
        use tracing_subscriber::fmt::writer::MakeWriterExt;
        let writer = std::io::stderr.and(nb);
        if log_format == "json" {
            tracing_subscriber::fmt()
                .json()
                .with_ansi(false)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_env_filter(EnvFilter::new(log_level))
                .with_writer(writer)
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_ansi(use_ansi)
                .with_env_filter(EnvFilter::new(log_level))
                .with_writer(writer)
                .init();
        }
    } else if log_format == "json" {
        tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_env_filter(EnvFilter::new(log_level))
            .with_writer(std::io::stderr)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_ansi(use_ansi)
            .with_env_filter(EnvFilter::new(log_level))
            .with_writer(std::io::stderr)
            .init();
    }

    // TR05: confirm effective filter after init
    tracing::debug!(
        target: "logging",
        filter = %log_level,
        format = %log_format,
        ansi = use_ansi,
        to_file,
        "tracing subscriber initialized (local only; no remote telemetry)"
    );

    // TR01 (v1.0.80, A1/G2): panic hook emits a structured tracing::error!
    // and DELIBERATELY DOES NOT call the previous hook. The default Rust
    // panic hook prints the same payload + location to stderr; combined
    // with the tracing event below, that produces a double-trace (one
    // structured event in JSON or pretty, one unstructured dump). We
    // prefer the structured single-trace: the tracing event carries the
    // same payload and location fields and is captured by the global
    // subscriber. Test runs still fail on panic because Rust aborts the
    // process regardless of which hook is installed.
    std::panic::set_hook(Box::new(|info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("<non-string panic>");
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        tracing::error!(
            target: "panic",
            message = %payload,
            location = location.as_deref().unwrap_or("unknown"),
            "thread panicked"
        );
    }));
}

/// Drop the file-appender worker so buffered events flush (GAP-SG-99).
///
/// Safe to call multiple times. Invoked from the second-signal path before
/// `process::exit` and at the end of a normal `main` return when possible.
pub fn flush_tracing() {
    if let Ok(mut slot) = FILE_GUARD.lock() {
        *slot = None;
    }
}
