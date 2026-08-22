//! Process entry point: signal handling, language/timezone init, dispatch.

// The process uses the system allocator.
//
// `mimalloc` was the `#[global_allocator]` until v1.2.2. It is a C library, and
// this project is meant to ship as a self-contained rust-native binary; an
// allocator is an optimisation, not a requirement, so it was the removable half
// of the C toolchain. Measured over the real binary on a 200-memory corpus, the
// swap is inside run-to-run noise for a one-shot CLI whose wall time is
// dominated by process spawn and SQLite I/O — the allocator never gets a
// long-lived heap to amortise against.
//
// It also carried `#[cfg(not(sqlite_graphrag_miri))]`, because Miri cannot model
// the foreign `mi_malloc_aligned`. That cfg existed for nothing else and is gone
// with it, so the Miri job no longer needs `RUSTFLAGS` to run the unsafe tests.

use clap::Parser;
use sqlite_graphrag::{
    cli::Cli,
    commands,
    constants::{
        CLI_LOCK_DEFAULT_WAIT_SECS, MAX_CONCURRENT_CLI_INSTANCES, MIN_AVAILABLE_MEMORY_MB,
    },
    lock::acquire_cli_slot,
    memory_guard::{available_memory_mb, calculate_safe_concurrency, check_available_memory},
    storage::connection::register_vec_extension,
};

fn main() -> std::process::ExitCode {
    // v1.0.80 (A1/G6): the explicit Write::flush calls below are NOT
    // redundant. `std::process::ExitCode` is a transparent wrapper around
    // a u8 returned from main; on process exit, the C runtime flushes its
    // OWN stdio buffers but does NOT know about Rust's internal
    // `BufWriter` wrapping stdout/stderr. Without the explicit flush, the
    // last partial line of JSON output (notably from
    // `output::emit_json_compact` and `emit_progress`) can be lost when
    // the process is killed by a signal or exits with an error code. This
    // is a deliberate defensive policy: flush every error-path AND the
    // success-path before returning.
    // v1.0.80 (A1/G1): the main thread is intentionally 100% synchronous.
    // The default LLM-only build (v1.0.76+) does not own a tokio runtime
    // here: every remember, ingest, and enrich drives the OpenRouter REST
    // API through a short-lived runtime and waits on its completion.
    // The per-call concurrency cap is enforced by the
    // acquire_cli_slot counting semaphore and the MAX_CONCURRENT_CLI_*
    // constants; cross-process sync happens via SQLite WAL and flock.
    // The pre-tokio design is a deliberate policy choice: no async
    // runtime context to cancel, no tokio::select! arms to skip, and no
    // JoinSet to drain on shutdown (see ADR-0034 for the SHUTDOWN global
    // and the audit-mode bypass). Touching this entry point requires
    // revisiting the per-subprocess cancellation policy, not just adding
    // a runtime.
    // Reset SIGPIPE to default so pipe consumers (head, jaq) cause clean exit 141.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    sqlite_graphrag::terminal::init_console();

    // G28: reap orphan LLM subprocesses from a previous crashed invocation
    // BEFORE doing any work. The scan is a no-op on non-Unix platforms.
    let _reaper_report = sqlite_graphrag::reaper::scan_and_kill_orphans();

    // v1.0.79: ONNX Runtime removed from the default LLM-only build. The
    // fastembed/ort/onnxruntime crates are no longer in the dependency tree;
    // embeddings and NER delegate to the OpenRouter REST API. The global
    // Rayon pool below stays
    // relevant for parallel similarity and batch ops on that path.
    //
    // GAP-SG-92: this used to write `RAYON_NUM_THREADS` into the process
    // environment inside an `unsafe` block, which made an env var the
    // configuration channel — the exact thing `G-T-XDG-04` retired — and
    // buried the cap as a bare literal. Building the pool explicitly keeps
    // the number inside the documented precedence and needs no mutation.
    //
    // `build_global` fails only if a pool already exists. At this point in
    // startup none does, and if one somehow did, its configuration is the one
    // that matters — so the error is logged, not propagated.
    let rayon_threads = sqlite_graphrag::runtime_config::rayon_threads(
        sqlite_graphrag::constants::DEFAULT_RAYON_THREADS,
    );
    if let Err(e) = rayon::ThreadPoolBuilder::new()
        .num_threads(rayon_threads)
        .build_global()
    {
        tracing::debug!(
            target: "startup",
            error = %e,
            threads = rayon_threads,
            "global rayon pool already initialized; keeping the existing one"
        );
    }

    // Pre-parse --verbose / -v / --quiet before tracing init so the flag
    // overrides the env var. We avoid full Cli::parse() here because it would
    // fail on missing required args when --help is requested.
    let quiet = std::env::args().any(|a| a == "--quiet" || a == "-q");
    let verbose_count: u8 = std::env::args()
        .skip(1)
        .map(|a| {
            if a == "--verbose" || a == "-v" {
                1u8
            } else if a.starts_with("-v") && a.chars().skip(1).all(|c| c == 'v') {
                (a.len() - 1).try_into().unwrap_or(u8::MAX)
            } else {
                0u8
            }
        })
        .sum();

    // v1.1.05 Bug 2: --quiet keeps stderr free of info/warn noise so pipelines
    // that mistakenly use 2>&1 are less likely to contaminate JSON; prefer
    // redirecting stdout alone (`> out.json`) or `--output path`.
    let log_level = if quiet {
        "error".to_string()
    } else if verbose_count > 0 {
        match verbose_count {
            1 => "info".to_string(),
            2 => "debug".to_string(),
            _ => "trace".to_string(),
        }
    } else if let Ok(Some(v)) = sqlite_graphrag::config::get_setting("log.level") {
        v
    } else {
        "warn".to_string()
    };
    let log_format = if let Ok(Some(v)) = sqlite_graphrag::config::get_setting("log.format") {
        v
    } else {
        "pretty".to_string()
    };

    sqlite_graphrag::tracing_init::init_tracing(&log_level, &log_format);

    register_vec_extension();

    // v1.0.80 (A1/G7): the deadlock-detection thread below is intentionally
    // process-scoped (it has no shutdown signal). It is a watchdog: it polls
    // every 10 seconds and reports any deadlocks it finds via tracing, then
    // sleeps again. When the process exits (via std::process::ExitCode
    // return or a signal), the kernel tears down all threads; there is no
    // leak because the thread is never joined or detached in the Rust
    // sense. The 10-second poll interval is a balance: short enough to
    // catch deadlocks in interactive tests, long enough to not pollute
    // tracing output during normal operation. The thread body is
    // panic-resistant: a panic inside the loop kills only this thread, and
    // since the main thread never joins it, the panic is silently dropped
    // (Rust's default panic-on-thread-death is bypassed for detached
    // threads). We accept this because the alternative — a panicking
    // deadlock check — would itself be a deadlock.

    #[cfg(feature = "deadlock-detection")]
    {
        std::thread::spawn(|| loop {
            std::thread::sleep(std::time::Duration::from_secs(
                sqlite_graphrag::constants::DEADLOCK_CHECK_INTERVAL_SECS,
            ));
            let deadlocks = parking_lot::deadlock::check_deadlock();
            if !deadlocks.is_empty() {
                tracing::error!(target: "deadlock_detection", count = deadlocks.len(), "deadlocks detected");
                for (i, threads) in deadlocks.iter().enumerate() {
                    for t in threads {
                        tracing::error!(
                            target: "deadlock_detection",
                            index = i,
                            thread_id = ?t.thread_id(),
                            backtrace = ?t.backtrace(),
                            "deadlock thread info"
                        );
                    }
                }
            }
        });
    }

    // Pre-parse --lang before Cli::parse() so the language is set even
    // when clap exits early via process::exit (--help, parse errors, etc.).
    // The subsequent call to init(cli.lang) will be silently ignored by the OnceLock.
    //
    // GAP-SG-98: --config-dir and --cache-dir MUST be captured in the same pass.
    // Without --lang, language resolution falls back to the XDG key `i18n.lang`,
    // which reads config.toml — so the config directory has to be known already.
    // Installing them later, in runtime_config::init, is too late: the language
    // OnceLock is already set from the wrong directory.
    {
        let args: Vec<String> = std::env::args().collect();
        let mut lang_override: Option<sqlite_graphrag::i18n::Language> = None;
        let mut paths = sqlite_graphrag::runtime_config::PathOverrides::default();
        let mut i = 1usize;
        while i < args.len() {
            let take_value = |flag: &str, i: &mut usize| -> Option<String> {
                if args[*i] == flag {
                    let v = args.get(*i + 1).cloned();
                    *i += 2;
                    return v;
                }
                if let Some(v) = args[*i].strip_prefix(&format!("{flag}=")) {
                    let v = v.to_string();
                    *i += 1;
                    return Some(v);
                }
                None
            };
            if let Some(v) = take_value("--lang", &mut i) {
                lang_override = sqlite_graphrag::i18n::Language::from_str_opt(&v);
            } else if let Some(v) = take_value("--config-dir", &mut i) {
                paths.config_dir = Some(v);
            } else if let Some(v) = take_value("--cache-dir", &mut i) {
                paths.cache_dir = Some(v);
            } else {
                i += 1;
            }
        }
        sqlite_graphrag::runtime_config::init_paths(paths);
        sqlite_graphrag::i18n::init(lang_override);
    }

    let cli = Cli::parse();

    // Initialize global language BEFORE any bilingual emit_progress.
    // This call is a no-op if the pre-parse above already initialized the OnceLock.
    sqlite_graphrag::i18n::init(cli.lang);

    // G-T-XDG-04: install CLI overrides into runtime_config (no product env).
    sqlite_graphrag::runtime_config::init(sqlite_graphrag::runtime_config::RuntimeOverrides {
        embedding_dim: cli.embedding_dim.and_then(|d| u32::try_from(d).ok()),
        llm_model: cli.llm_model.clone(),
        llm_fallback: cli.llm_fallback.clone(),
        skip_embedding_on_failure: cli.skip_embedding_on_failure,
        llm_max_host_concurrency: cli.llm_max_host_concurrency.map(|n| n as usize),
        llm_slot_wait_secs: cli.llm_slot_wait_secs,
        llm_slot_no_wait: cli.llm_slot_no_wait,
        // ORDERING IS LOAD-BEARING: this runs before the OpenRouter client is
        // built below, and that client is a `OnceLock` that FREEZES its timeout
        // on first construction. Installing the override later would leave the
        // client on the compiled default for the whole process.
        openrouter_timeout: cli.openrouter_timeout,
        log_level: None,
        log_format: None,
        lang: None, // set via i18n::init(cli.lang)
        display_tz: None,
        db_path: None,
    });

    // Initialize display timezone (flag --tz > XDG display.tz > UTC).
    if let Err(e) = sqlite_graphrag::tz::init(cli.tz) {
        sqlite_graphrag::output::emit_error(&e.localized_message());
        if let Some(code) = flush_std_streams() {
            return code;
        }
        return std::process::ExitCode::from(e.exit_code() as u8);
    }

    // Validate flags before any heavy initialization.
    if let Err(msg) = cli.validate_flags() {
        sqlite_graphrag::output::emit_error(&msg);
        if let Some(code) = flush_std_streams() {
            return code;
        }
        return std::process::ExitCode::from(2);
    }

    let embedding_heavy = cli.command.as_ref().is_some_and(|c| c.is_embedding_heavy());
    let measured_available_mb = if embedding_heavy {
        let available_mb = if cli.skip_memory_guard {
            available_memory_mb()
        } else {
            match check_available_memory(MIN_AVAILABLE_MEMORY_MB) {
                Ok(available_mb) => available_mb,
                Err(e) => {
                    sqlite_graphrag::output::emit_error(&e.localized_message());
                    if let Some(code) = flush_std_streams() {
                        return code;
                    }
                    return std::process::ExitCode::from(e.exit_code() as u8);
                }
            }
        };

        Some(available_mb)
    } else {
        None
    };

    // Resolve concurrency parameters with fallback to canonical constants.
    let requested_concurrency = cli.max_concurrency.unwrap_or(MAX_CONCURRENT_CLI_INSTANCES);
    let max_concurrency = if embedding_heavy {
        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        // SAFETY invariant: measured_available_mb is always Some when embedding_heavy is true,
        // because the block above (lines ~137-157) sets it to Some(available_mb) in that branch.
        // Using unwrap_or_else with exit instead of ? because main() returns ().
        let available_mb = match measured_available_mb {
            Some(mb) => mb,
            None => {
                sqlite_graphrag::output::emit_error_i18n(
                    "embedding-heavy command must measure available RAM",
                    &sqlite_graphrag::i18n::validation::runtime_pt::embedding_heavy_must_measure_ram(),
                );
                if let Some(code) = flush_std_streams() {
                    return code;
                }
                return std::process::ExitCode::from(20);
            }
        };
        // v1.0.79: every build is LLM-only; the per-worker budget is the
        // REST client footprint, not the old 1100 MB ONNX model load. The
        // budget is an ESTIMATE, never a measurement, so it is read through the
        // XDG-aware resolver rather than from the bare constant.
        let safe_concurrency = calculate_safe_concurrency(
            available_mb,
            cpu_count,
            sqlite_graphrag::constants::llm_worker_rss_mb(),
            MAX_CONCURRENT_CLI_INSTANCES,
        );
        let effective_concurrency = requested_concurrency.min(safe_concurrency);

        sqlite_graphrag::output::emit_progress_i18n(
            &format!(
                "Heavy command detected; available memory: {available_mb} MB; safe concurrency: {safe_concurrency}"
            ),
            &sqlite_graphrag::i18n::validation::runtime_pt::heavy_command_detected(
                available_mb,
                safe_concurrency,
            ),
        );

        if effective_concurrency < requested_concurrency {
            sqlite_graphrag::output::emit_progress_i18n(
                &format!(
                    "Reducing requested concurrency from {requested_concurrency} to {effective_concurrency} to avoid memory oversubscription"
                ),
                &sqlite_graphrag::i18n::validation::runtime_pt::reducing_concurrency(
                    requested_concurrency,
                    effective_concurrency,
                ),
            );
        }

        effective_concurrency
    } else {
        requested_concurrency.min(MAX_CONCURRENT_CLI_INSTANCES)
    };

    // Joint cap. `--max-concurrency` is validated against `2 × nCPUs` and
    // `--llm-parallelism` against 32, but nothing bounded their PRODUCT: on a
    // 16-core host the two ceilings together authorised 1024 in-flight workers.
    // Publishing the per-process fan-out share here is the only place that knows
    // the resolved concurrency; `embedder::batch` reads it back when it turns a
    // requested parallelism into actual permits.
    sqlite_graphrag::constants::set_joint_parallelism_ceiling(
        sqlite_graphrag::constants::joint_parallelism_ceiling_for(max_concurrency),
    );

    let wait_secs = cli.wait_lock.unwrap_or(CLI_LOCK_DEFAULT_WAIT_SECS);
    if wait_secs > 5 {
        tracing::info!(
            wait_secs,
            "cli slot acquire — using extended wait (cold-start headroom)"
        );
    }

    // Acquire a slot in the counting semaphore. The handle is kept alive until end of main
    // so the flock is released automatically when the file descriptor is closed.
    let _slot_guard = if cli.command.as_ref().is_some_and(|c| c.uses_cli_slot()) {
        Some(match acquire_cli_slot(max_concurrency, Some(wait_secs)) {
            Ok(pair) => pair,
            Err(e) => {
                sqlite_graphrag::output::emit_error(&e.localized_message());
                if let Some(code) = flush_std_streams() {
                    return code;
                }
                return std::process::ExitCode::from(e.exit_code() as u8);
            }
        })
    } else {
        None
    };

    sqlite_graphrag::signals::register_shutdown_handler();

    // v1.1.8 G-T-XDG-04: product env bridge removed; runtime_config holds CLI overrides.

    // The flag documented XDG `embedding.model` as its fallback from the day it
    // shipped, but nothing resolved the key, so `config set embedding.model`
    // was stored and then ignored. Resolving once, in function scope, keeps the
    // preflight below and the `init` handler on the same documented precedence.
    let embedding_model =
        sqlite_graphrag::runtime_config::embedding_model(cli.embedding_model.as_deref());

    // Same defect on the two backend selectors: both `--help` texts promised an
    // XDG fallback (`embedding.backend`, `llm.backend`) that no registry entry
    // declared, so the documented `config set` answered exit 1. Resolving here,
    // once, is what keeps every call site below on one precedence chain —
    // flag > XDG > compiled default — instead of reading a clap default that
    // would make the XDG layer unreachable.
    let embedding_backend =
        sqlite_graphrag::runtime_config::embedding_backend(cli.embedding_backend);
    let llm_backend = sqlite_graphrag::runtime_config::llm_backend(cli.llm_backend);

    // v1.0.93: initialise OpenRouter embedding client when configured.
    {
        use sqlite_graphrag::cli::EmbeddingBackendChoice;
        let wants_openrouter = matches!(
            embedding_backend,
            EmbeddingBackendChoice::Auto | EmbeddingBackendChoice::Openrouter
        );
        // RC-7 fix (v1.0.98): read-only / no-embedding subcommands (`init`, the
        // `enrich` queue inspectors) must not be hard-failed by the eager
        // OpenRouter key preflight. `init` self-degrades to `ok_no_embedding`
        // and the inspectors never embed, so a missing key is not fatal here.
        let tolerates_no_key = cli
            .command
            .as_ref()
            .is_some_and(|c| c.tolerates_missing_embedding_key());
        if wants_openrouter {
            // `tolerates_no_key` also guards the model preflight, not only the
            // key one below. Without it, `config set embedding.backend
            // openrouter` with no model stored is a one-way door: every later
            // invocation dies at this check, INCLUDING the `config unset` that
            // would undo it, so the operator has no CLI path back and has to
            // hand-edit the TOML. A knob whose wrong value disables the command
            // that fixes it is not a knob.
            if matches!(embedding_backend, EmbeddingBackendChoice::Openrouter)
                && embedding_model.is_none()
                && !tolerates_no_key
            {
                let msg = "--embedding-backend openrouter requires --embedding-model \
                           or XDG `config set embedding.model` \
                           (e.g. qwen/qwen3-embedding-8b)";
                sqlite_graphrag::output::emit_error_json(78, msg);
                sqlite_graphrag::output::emit_error(msg);
                if let Some(code) = flush_std_streams() {
                    return code;
                }
                return std::process::ExitCode::from(78_u8);
            }
            if let Some(model) = embedding_model.as_deref() {
                if let Some(resolved) = sqlite_graphrag::config::resolve_api_key(
                    "openrouter",
                    cli.openrouter_api_key.as_deref(),
                ) {
                    let dim = sqlite_graphrag::constants::embedding_dim();
                    if let Err(e) = sqlite_graphrag::embedder::get_openrouter_embedder(
                        resolved.value,
                        model,
                        dim,
                        // Global flag, read straight off `Cli`: the value no
                        // longer has to be dug out of one enum variant, which
                        // is what limited it to `enrich`.
                        sqlite_graphrag::runtime_config::openrouter_timeout_override(),
                    ) {
                        tracing::warn!(error = %e, "failed to initialise OpenRouter embedding client");
                        if matches!(embedding_backend, EmbeddingBackendChoice::Openrouter)
                            && !tolerates_no_key
                        {
                            sqlite_graphrag::output::emit_error_json(78, &e.to_string());
                            sqlite_graphrag::output::emit_error(&e.to_string());
                            if let Some(code) = flush_std_streams() {
                                return code;
                            }
                            return std::process::ExitCode::from(78_u8);
                        }
                    }
                } else if matches!(embedding_backend, EmbeddingBackendChoice::Openrouter)
                    && !tolerates_no_key
                {
                    let msg = "--embedding-backend openrouter needs a key: store one with `config add-key --provider openrouter --from-stdin` or pass --openrouter-api-key";
                    sqlite_graphrag::output::emit_error_json(78, msg);
                    sqlite_graphrag::output::emit_error(msg);
                    if let Some(code) = flush_std_streams() {
                        return code;
                    }
                    return std::process::ExitCode::from(78_u8);
                }
            }
        }
    }

    // GAP-SG-265: the two selectors are resolved together above and consumed
    // together by every write path, so the dispatch hands them over as one value.
    let backends = sqlite_graphrag::cli::BackendChoice::new(llm_backend, embedding_backend);
    let result = match cli.command {
        Some(cmd) => match cmd {
            sqlite_graphrag::cli::Commands::Init(args) => {
                commands::init::run(args, backends, embedding_model.as_deref())
            }
            sqlite_graphrag::cli::Commands::Remember(args) => {
                commands::remember::run(args, backends)
            }
            sqlite_graphrag::cli::Commands::RememberBatch(args) => {
                commands::remember_batch::run(args, backends)
            }
            sqlite_graphrag::cli::Commands::Ingest(args) => commands::ingest::run(*args, backends),
            sqlite_graphrag::cli::Commands::Recall(args) => {
                commands::recall::run(args, backends, cli.fail_on_degraded)
            }
            sqlite_graphrag::cli::Commands::Edit(args) => commands::edit::run(args, backends),
            sqlite_graphrag::cli::Commands::History(args) => commands::history::run(args),
            sqlite_graphrag::cli::Commands::Restore(args) => commands::restore::run(args, backends),
            sqlite_graphrag::cli::Commands::HybridSearch(args) => {
                commands::hybrid_search::run(args, backends, cli.fail_on_degraded)
            }
            sqlite_graphrag::cli::Commands::Read(args) => commands::read::run(args),
            sqlite_graphrag::cli::Commands::List(args) => commands::list::run(args),
            sqlite_graphrag::cli::Commands::Forget(args) => commands::forget::run(args),
            sqlite_graphrag::cli::Commands::Purge(args) => commands::purge::run(args),
            sqlite_graphrag::cli::Commands::Rename(args) => commands::rename::run(args),
            sqlite_graphrag::cli::Commands::SplitBody(args) => commands::split_body::run(args),
            sqlite_graphrag::cli::Commands::Health(args) => commands::health::run(args),
            sqlite_graphrag::cli::Commands::Migrate(args) => commands::migrate::run(args),
            sqlite_graphrag::cli::Commands::NamespaceDetect(args) => {
                commands::namespace_detect::run(args)
            }
            sqlite_graphrag::cli::Commands::Optimize(args) => commands::optimize::run(args),
            sqlite_graphrag::cli::Commands::Stats(args) => commands::stats::run(args),
            sqlite_graphrag::cli::Commands::SyncSafeCopy(args) => {
                commands::sync_safe_copy::run(args)
            }
            sqlite_graphrag::cli::Commands::Backup(args) => commands::backup::run(args),
            sqlite_graphrag::cli::Commands::Vacuum(args) => commands::vacuum::run(args),
            sqlite_graphrag::cli::Commands::Link(args) => commands::link::run(args),
            sqlite_graphrag::cli::Commands::Unlink(args) => commands::unlink::run(args),
            sqlite_graphrag::cli::Commands::DeepResearch(args) => {
                commands::deep_research::run(args, backends, cli.fail_on_degraded)
            }
            sqlite_graphrag::cli::Commands::Related(args) => commands::related::run(args),
            sqlite_graphrag::cli::Commands::Graph(args) => commands::graph_export::run(args),
            sqlite_graphrag::cli::Commands::Export(args) => commands::export::run(args),
            sqlite_graphrag::cli::Commands::Fts(args) => commands::fts::run(args),
            sqlite_graphrag::cli::Commands::Vec(args) => commands::vec::run(args),
            sqlite_graphrag::cli::Commands::PruneRelations(args) => {
                commands::prune_relations::run(args)
            }
            sqlite_graphrag::cli::Commands::PruneNer(args) => commands::prune_ner::run(args),
            sqlite_graphrag::cli::Commands::CleanupOrphans(args) => {
                commands::cleanup_orphans::run(args)
            }
            sqlite_graphrag::cli::Commands::MemoryEntities(args) => {
                commands::memory_entities::run(args)
            }
            sqlite_graphrag::cli::Commands::Cache(args) => commands::cache::run(args),
            sqlite_graphrag::cli::Commands::DeleteEntity(args) => {
                commands::delete_entity::run(args)
            }
            sqlite_graphrag::cli::Commands::Reclassify(args) => commands::reclassify::run(args),
            sqlite_graphrag::cli::Commands::RenameEntity(args) => {
                commands::rename_entity::run(args, backends)
            }
            sqlite_graphrag::cli::Commands::MergeEntities(args) => {
                commands::merge_entities::run(args)
            }
            sqlite_graphrag::cli::Commands::Enrich(args) => {
                commands::enrich::run(args.as_ref(), backends)
            }
            sqlite_graphrag::cli::Commands::ReclassifyRelation(args) => {
                commands::reclassify_relation::run(args)
            }
            sqlite_graphrag::cli::Commands::NormalizeEntities(args) => {
                commands::normalize_entities::run(args)
            }
            sqlite_graphrag::cli::Commands::Completions(args) => commands::completions::run(args),
            sqlite_graphrag::cli::Commands::Schema(args) => {
                sqlite_graphrag::print_schema::run(args)
            }
            sqlite_graphrag::cli::Commands::DebugSchema(args) => commands::debug_schema::run(args),
            sqlite_graphrag::cli::Commands::Slots(args) => commands::slots::run(args),
            sqlite_graphrag::cli::Commands::Embedding(args) => {
                commands::embedding::run(args, llm_backend)
            }
            sqlite_graphrag::cli::Commands::PendingEmbeddings(args) => {
                commands::pending_embeddings::run(args)
            }
            sqlite_graphrag::cli::Commands::Config(args) => commands::config_cmd::run(args),
        },
        None => Ok(()),
    };

    if let Err(e) = result {
        // GAP-SG-39: emit an actionable error envelope (cause + remediation) so a
        // non-zero exit from any write path is never silent on stdout. The retry
        // verdict travels with it, so an agent decides from the envelope rather
        // than from a table of exit codes it has to know by heart.
        sqlite_graphrag::output::emit_error_json_with_suggestion(
            e.exit_code(),
            &e.localized_message(),
            e.error_class(),
            e.is_retryable(),
            e.suggestion(),
            e.discarded_flags(),
        );
        sqlite_graphrag::output::emit_error(&e.localized_message());
        // A closed stdout pipe outranks the command's own code, but it must NOT
        // skip the teardown: returning early here would drop the last
        // diagnostics and leak the spawn directory.
        let broken_pipe = flush_std_streams();
        sqlite_graphrag::tracing_init::flush_tracing();
        cleanup_spawn_dir();
        return broken_pipe.unwrap_or_else(|| std::process::ExitCode::from(e.exit_code() as u8));
    }

    let broken_pipe = flush_std_streams();
    // GAP-SG-99/130: drop non-blocking file appender worker so buffers flush
    // before process exit (docsrs WorkerGuard contract).
    sqlite_graphrag::tracing_init::flush_tracing();

    // A consumer that closed the pipe early (`| head`, `| jaq`) outranks the
    // success code: on Unix the default SIGPIPE disposition would already have
    // killed the process, and this keeps Windows — which has no SIGPIPE — on the
    // same 141 contract.
    if let Some(code) = broken_pipe {
        cleanup_spawn_dir();
        return code;
    }

    if sqlite_graphrag::shutdown_requested() {
        cleanup_spawn_dir();
        return std::process::ExitCode::from(sqlite_graphrag::constants::SHUTDOWN_EXIT_CODE as u8);
    }

    cleanup_spawn_dir();
    std::process::ExitCode::SUCCESS
}

/// Flushes stdout then stderr, reporting a CLOSED stdout pipe as exit 141.
///
/// Returns `Some(141)` when the stdout flush failed with
/// [`std::io::ErrorKind::BrokenPipe`], and `None` in every other case —
/// including any other I/O error, which is swallowed exactly as before because
/// a failing flush must never mask the real exit code.
///
/// # Why this exists on Windows
///
/// On Unix `main` resets `SIGPIPE` to its default disposition, so a consumer
/// that exits early (`| head`, `| jaq`) kills the process with the conventional
/// `128 + 13 = 141` before this code is reached. Windows has NO `SIGPIPE`: the
/// closed pipe surfaces only as a write error, and without this classification
/// the same pipeline would exit 0 there. One helper on every exit path keeps the
/// exit-code contract identical on Linux, macOS and Windows.
fn flush_std_streams() -> Option<std::process::ExitCode> {
    use std::io::Write;
    let stdout_broken = std::io::stdout()
        .flush()
        .is_err_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe);
    let _ = std::io::stderr().flush();
    if stdout_broken {
        return Some(std::process::ExitCode::from(
            sqlite_graphrag::constants::BROKEN_PIPE_EXIT_CODE,
        ));
    }
    None
}

fn cleanup_spawn_dir() {
    let dir = std::env::temp_dir().join(format!("sqlite-graphrag-spawn-{}", std::process::id()));
    let _ = std::fs::remove_dir(&dir);
}
