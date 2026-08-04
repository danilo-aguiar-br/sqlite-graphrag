//! Canonical registry of operational setting keys.
//!
//! Owns the single source of truth for which dotted keys `config set` accepts,
//! which retired aliases still map onto a live key, and the drift test that
//! keeps the registry honest against the readers scattered across the crate.

use super::{SettingKey, ValueKind};

/// Canonical registry of operational setting keys accepted by `config set`.
///
/// Single source of truth shared by [`set_setting`] validation and the
/// `config doctor` knob listing. Keeping exactly one list prevents the
/// divergence class recorded as `GAP-SG-79`, where help text advertised
/// `db.default_path` while [`crate::paths::AppPaths::resolve`] has always
/// read `db.path`.
///
/// Every entry MUST have a matching [`get_setting`] reader somewhere in the
/// crate. Adding a key here without a reader recreates the silent no-op this
/// registry exists to prevent; `GAP-SG-90` adds the test that enforces it.
///
/// Literal defaults that mirror a constant are asserted against that constant
/// in this module's tests, so the two cannot drift apart in silence.
///
/// Kept sorted so the emitted diagnostics are stable across runs.
pub const SETTING_KEYS: &[SettingKey] = &[
    SettingKey {
        key: "agent_surface.max_items",
        default: Some("0"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "agent_surface.max_output_bytes",
        default: Some("0"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "agent_surface.truncate_content",
        default: Some("0"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "cache.dir",
        default: None,
        kind: ValueKind::Path,
    },
    SettingKey {
        key: "cli.max_instances",
        default: None,
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "cli.no_input",
        default: Some("false"),
        kind: ValueKind::Bool,
    },
    SettingKey {
        key: "cli.stdin_timeout_secs",
        default: Some("60"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "db.busy_base_delay_ms",
        default: Some("300"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "db.busy_retries",
        default: Some("5"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "db.path",
        default: None,
        kind: ValueKind::Path,
    },
    SettingKey {
        key: "db.query_timeout_ms",
        default: Some("5000"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "display.tz",
        default: Some("UTC"),
        kind: ValueKind::Tz,
    },
    SettingKey {
        key: "embedding.batch_size",
        default: Some("32"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "embedding.dim",
        default: Some("1024"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        // OpenRouter embedding model. `--embedding-model` promised this key as
        // its XDG fallback since v1.0.93 while nothing ever read it, so an
        // operator who ran `config set embedding.model` still got exit 78 for a
        // missing flag. Deliberately unset, because an absent value means
        // "operator passed nothing" — the state `--embedding-backend
        // openrouter` rejects.
        key: "embedding.model",
        default: None,
        kind: ValueKind::Text,
    },
    SettingKey {
        // Embedding backend selector. Same defect GAP-SG-192 fixed for
        // `embedding.model`, left behind on its sibling: `--embedding-backend`
        // has promised "optional XDG `config set embedding.backend`" in its
        // own `--help` while the key was absent from this registry, so the
        // documented command answered exit 1, `unknown config key`.
        // Unset by default so an omitted value stays distinguishable from an
        // explicit one — that is what lets flag > XDG > clap default resolve
        // instead of the XDG layer always winning with a value nobody chose.
        key: "embedding.backend",
        default: None,
        kind: ValueKind::OneOf(&["auto", "openrouter", "open-router"]),
    },
    SettingKey {
        // LLM backend for embedding. `--llm-backend` promises "optional XDG
        // `llm.backend` via `config set`" and the key was never registered.
        // Unset for the same reason as `embedding.backend` above: the clap
        // default (`open-router`) must keep winning when nobody set the key.
        key: "llm.backend",
        default: None,
        kind: ValueKind::OneOf(&["none", "openrouter", "open-router"]),
    },
    SettingKey {
        // Graph-match ceiling for `hybrid-search --with-graph`. `0` opts back
        // into the unbounded pre-v1.2.2 envelope, which reached 1.1 MB from a
        // `--k 3` query.
        key: "search.hybrid.max_graph_results",
        default: Some("50"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "embedding.entity_cache_max_entries",
        default: Some("10000"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "embedding.entity_cache_ttl_secs",
        default: Some("3600"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "embedding.timeout_secs",
        default: Some("300"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.circuit_breaker_reset_secs",
        default: Some("60"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.entity_connect.default_limit",
        default: Some("100"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.entity_connect.large_ns_limit",
        default: Some("25"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.entity_description.corpus_top_k",
        default: Some("5"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.entity_description.domain",
        default: Some("auto"),
        kind: ValueKind::Text,
    },
    SettingKey {
        key: "enrich.entity_description.grounding_threshold",
        default: Some("0.12"),
        kind: ValueKind::Float,
    },
    SettingKey {
        key: "enrich.entity_description.min_corpus_chars",
        default: Some("40"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.entity_description.quality_sample",
        default: Some("50"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.entity_description.snippet_chars",
        default: Some("400"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.rate_limit_deadline_secs",
        default: Some("3600"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.reembed_claim_batch",
        default: Some("32"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.scan_page_size",
        default: Some("512"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "enrich.yield_every_n_items",
        default: Some("10"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "i18n.lang",
        default: Some("en"),
        // Mirrors the clap aliases on `Language` in `src/i18n/mod.rs`.
        kind: ValueKind::OneOf(&[
            "en",
            "EN",
            "english",
            "pt",
            "PT",
            "pt-BR",
            "pt-br",
            "portugues",
            "portuguese",
        ]),
    },
    SettingKey {
        key: "ingest.low_memory",
        default: Some("false"),
        kind: ValueKind::Bool,
    },
    SettingKey {
        key: "limits.max_entities_per_memory",
        default: Some("50"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "limits.max_relations_per_memory",
        default: Some("50"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "llm.fallback",
        default: Some("none"),
        // v1.2.0 removed the subprocess backends; `none` is all that is left.
        kind: ValueKind::OneOf(&["none"]),
    },
    SettingKey {
        key: "llm.max_host_concurrency",
        default: None,
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "llm.model",
        default: None,
        kind: ValueKind::Text,
    },
    SettingKey {
        key: "llm.openrouter_timeout_secs",
        default: Some("600"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "llm.probe_timeout_ms",
        default: Some("800"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "llm.skip_embedding_on_failure",
        default: Some("false"),
        kind: ValueKind::Bool,
    },
    SettingKey {
        key: "llm.slot_no_wait",
        default: Some("false"),
        kind: ValueKind::Bool,
    },
    SettingKey {
        key: "llm.slot_wait_secs",
        default: Some("300"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "llm.worker_rss_mb",
        default: Some("350"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "log.format",
        default: Some("pretty"),
        kind: ValueKind::OneOf(&["pretty", "json"]),
    },
    SettingKey {
        key: "log.level",
        default: Some("warn"),
        kind: ValueKind::LogDirective,
    },
    SettingKey {
        key: "log.retention_days",
        default: Some("7"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "log.rotation",
        default: Some("daily"),
        kind: ValueKind::OneOf(&["daily", "hourly", "never"]),
    },
    SettingKey {
        key: "log.to_file",
        default: Some("false"),
        kind: ValueKind::Bool,
    },
    SettingKey {
        key: "namespace.default",
        default: Some("global"),
        kind: ValueKind::Text,
    },
    SettingKey {
        key: "network.chat_url",
        default: None,
        kind: ValueKind::Url,
    },
    SettingKey {
        key: "network.embed_url",
        default: None,
        kind: ValueKind::Url,
    },
    SettingKey {
        key: "network.openrouter.chat_url",
        default: Some(crate::constants::DEFAULT_OPENROUTER_CHAT_URL),
        kind: ValueKind::Url,
    },
    SettingKey {
        key: "network.openrouter.embeddings_url",
        default: Some(crate::constants::DEFAULT_OPENROUTER_EMBEDDINGS_URL),
        kind: ValueKind::Url,
    },
    SettingKey {
        key: "parallelism.embed_runtime_threads",
        default: None,
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "parallelism.max_total_workers",
        default: Some("64"),
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "parallelism.rayon_threads",
        default: None,
        kind: ValueKind::Unsigned,
    },
    SettingKey {
        key: "retry.disable",
        default: Some("false"),
        kind: ValueKind::Bool,
    },
    SettingKey {
        key: "shutdown.ignore",
        default: Some("false"),
        kind: ValueKind::Bool,
    },
    SettingKey {
        key: "system.max_load_per_ncpu",
        default: Some("2.0"),
        kind: ValueKind::Float,
    },
];

/// Iterates the registry key names in registration (sorted) order.
///
/// Callers that only need names use this instead of reaching into the struct,
/// so adding a field to [`SettingKey`] does not ripple through the crate.
pub fn setting_key_names() -> impl Iterator<Item = &'static str> {
    SETTING_KEYS.iter().map(|entry| entry.key)
}

/// Setting keys that were advertised historically but never had a reader.
///
/// Each tuple maps the obsolete key to its replacement. Present so
/// [`load_config`] can warn instead of ignoring the value in silence, which
/// is the failure mode `GAP-SG-79` documents.
///
/// These keys are rejected by [`set_setting`]; the warning covers configs
/// written before the validation existed.
// GAP-SG-79 / GAP-SG-122: legacy aliases map to the single canonical key.
// `db.default_path` never took effect (paths always read `db.path`).
// `paths.cache` was a parallel name for `cache.dir`.
pub(super) const LEGACY_SETTING_KEYS: &[(&str, &str)] =
    &[("db.default_path", "db.path"), ("paths.cache", "cache.dir")];

/// Minimum Jaro-Winkler similarity required to suggest a replacement key.
///
/// Below this a suggestion is more confusing than helpful, so the error lists
/// nothing rather than pointing at an unrelated key.
const SUGGESTION_THRESHOLD: f64 = 0.7;

/// Returns `true` when `key` belongs to [`SETTING_KEYS`].
pub fn is_known_setting(key: &str) -> bool {
    setting_key_names().any(|known| known == key)
}

/// Returns the closest known key to `key`, when one is similar enough.
///
/// Reuses `rapidfuzz` Jaro-Winkler, the same scorer already used for entity
/// name resolution in [`crate::storage::entities`], instead of introducing a
/// second similarity implementation.
pub fn nearest_setting_key(key: &str) -> Option<&'static str> {
    setting_key_names()
        .map(|candidate| {
            let score = rapidfuzz::distance::jaro_winkler::normalized_similarity(
                key.chars(),
                candidate.chars(),
            );
            (candidate, score)
        })
        .filter(|(_, score)| *score >= SUGGESTION_THRESHOLD)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(candidate, _)| candidate)
}
/// GAP-SG-90: every literal key read in production must appear in
/// [`SETTING_KEYS`]. These tests scan the crate source for the call shapes that
/// carry a literal key, and assert the source list itself stays complete.
#[cfg(test)]
mod setting_keys_drift_tests {
    use super::SETTING_KEYS;

    /// Every module holding a literal setting key, whether it reads it through
    /// `get_setting("…")` or through a `runtime_config::resolve_*` helper.
    ///
    /// A fixed list is used because a full-tree walk is flaky in package builds.
    /// Paths are relative to THIS file, so they climb out of `src/config/` with
    /// `../` (GAP-SG-146 moved the module into a directory).
    ///
    /// The `config` module itself is excluded: the prose right here mentions
    /// `get_setting("…")` and a placeholder would be scanned as a real key.
    /// `runtime_config.rs` is excluded because its own unit tests assert a
    /// deliberately unregistered key; its production literals are covered by
    /// those tests.
    const SCANNED_SOURCES: &[&str] = &[
        include_str!("../commands/enrich/extraction_descriptions.rs"),
        include_str!("../commands/enrich/prompts.rs"),
        include_str!("../commands/enrich/quality_sample.rs"),
        include_str!("../commands/enrich/scheduler.rs"),
        include_str!("../commands/enrich/status.rs"),
        include_str!("../commands/ingest/args.rs"),
        // v1.2.5 split the former single-file `constants.rs` into themed
        // submodules; every one is scanned so the coverage this guard provides
        // does not shrink with the refactor.
        include_str!("../constants/mod.rs"),
        include_str!("../constants/identity.rs"),
        include_str!("../constants/network.rs"),
        include_str!("../constants/exit_codes.rs"),
        include_str!("../constants/storage.rs"),
        include_str!("../constants/embedding.rs"),
        include_str!("../constants/enrich.rs"),
        include_str!("../constants/search.rs"),
        include_str!("../constants/runtime.rs"),
        include_str!("../constants/limits.rs"),
        include_str!("../embedder/getters.rs"),
        include_str!("../i18n/mod.rs"),
        include_str!("../lib.rs"),
        include_str!("../llm_slots.rs"),
        include_str!("../lock.rs"),
        include_str!("../main.rs"),
        include_str!("../namespace.rs"),
        include_str!("../paths.rs"),
        include_str!("../retry.rs"),
        include_str!("../system_load.rs"),
        include_str!("../tracing_init.rs"),
        include_str!("../tz.rs"),
    ];

    /// Call shapes that carry a literal setting key as an argument.
    ///
    /// `resolve_*` takes the key as its SECOND argument, after an explicit
    /// `None` override, so the marker includes that `None` to avoid matching a
    /// call that forwards a non-literal key.
    const KEY_CALL_MARKERS: &[&str] = &["get_setting(\""];

    /// `runtime_config` resolvers that take the key as their SECOND argument.
    ///
    /// The first argument is the precedence override (a CLI flag or `None`) and
    /// says nothing about whether the key exists, so it is skipped rather than
    /// matched. An earlier version required a literal `None` there to avoid
    /// matching calls that forward a non-literal key — that bought precision and
    /// paid with blindness on 22 call sites of the form
    /// `resolve_*(args.flag, "key", default)`, including `embedding.timeout_secs`.
    ///
    /// In a conformance guard, prefer a false positive to a false negative: a
    /// false positive shows up as a red test and someone looks, while a false
    /// negative is an orphaned key nobody sees.
    const KEY_RESOLVER_MARKERS: &[&str] = &[
        "resolve_u64(",
        "resolve_usize(",
        "resolve_bool(",
        "resolve_string(",
    ];

    /// Drops whole-line comments and all whitespace.
    ///
    /// Whole-line comments are removed because doc prose in this crate quotes
    /// `get_setting("…")` with a placeholder that is not a real key. Whitespace
    /// is removed because `resolve_*` calls are routinely wrapped by rustfmt
    /// across several lines, which would defeat a literal marker match.
    fn normalize(src: &str) -> String {
        src.lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .flat_map(|line| line.chars().filter(|c| !c.is_whitespace()))
            .collect()
    }

    /// Extracts every literal setting key read by `src`.
    ///
    /// Covers both call shapes: [`KEY_CALL_MARKERS`], where the key is the FIRST
    /// argument, and [`KEY_RESOLVER_MARKERS`], where it is the second.
    pub(super) fn scan_keys(src: &str) -> Vec<String> {
        let normalized = normalize(src);
        let mut found = Vec::new();
        for marker in KEY_CALL_MARKERS {
            for cap in normalized.split(marker).skip(1) {
                if let Some(end) = cap.find('"') {
                    if !cap[..end].is_empty() {
                        found.push(cap[..end].to_string());
                    }
                }
            }
        }
        for marker in KEY_RESOLVER_MARKERS {
            for cap in normalized.split(marker).skip(1) {
                // Skip the precedence argument and take the literal that follows
                // it. Bail out at the closing paren so a call with no literal
                // key cannot reach forward into an unrelated string.
                let Some(comma) = cap.find(',') else { continue };
                let close = cap.find(')').unwrap_or(usize::MAX);
                if comma > close {
                    continue;
                }
                let rest = &cap[comma + 1..];
                if !rest.starts_with('"') {
                    continue;
                }
                if let Some(end) = rest[1..].find('"') {
                    if end > 0 {
                        found.push(rest[1..1 + end].to_string());
                    }
                }
            }
        }
        found
    }

    #[test]
    fn setting_keys_covers_every_literal_key_in_src() {
        let registered: std::collections::HashSet<&str> =
            SETTING_KEYS.iter().map(|e| e.key).collect();
        let mut missing: Vec<String> = SCANNED_SOURCES
            .iter()
            .flat_map(|src| scan_keys(src))
            .filter(|key| !registered.contains(key.as_str()))
            .collect();
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "setting keys read at runtime but missing from SETTING_KEYS: {missing:?}"
        );
    }

    /// Modules whose literals are NOT production reads of a setting key.
    ///
    /// `config` holds the registry itself and quotes `get_setting("…")` in prose;
    /// `runtime_config` is the resolver, and its own unit tests assert a
    /// deliberately unregistered key.
    const COMPLETENESS_EXEMPT_DIRS: &[&str] = &["config"];
    const COMPLETENESS_EXEMPT_FILES: &[&str] = &["runtime_config.rs"];

    #[test]
    fn scanned_sources_lists_every_file_that_reads_a_setting_key() {
        // GAP-SG-90 follow-up. `SCANNED_SOURCES` has TWO failure modes and only
        // one is loud: a RENAMED file breaks the build through `include_str!`,
        // but a NEW file that reads a key and is never added stays invisible and
        // the guard silently stops covering it. This walk closes that half.
        //
        // It deliberately does NOT scan keys — `include_str!` still does the real
        // work. It only asserts the list is COMPLETE, so the objection against a
        // full-tree content walk does not apply.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        if !root.is_dir() {
            // Never fail on environment: a test that goes red because the source
            // tree is absent is worse than no test at all.
            eprintln!("skipping: {} is not a directory", root.display());
            return;
        }

        let mut rust_files = Vec::new();
        collect_rust_files(&root, &mut rust_files);
        assert!(
            !rust_files.is_empty(),
            "walk found zero .rs files under {} — the walk itself is broken, \
             which is exactly how this guard would go silently blind",
            root.display()
        );

        let listed: String = SCANNED_SOURCES.concat();
        let mut unlisted = Vec::new();
        for path in &rust_files {
            let Ok(rel) = path.strip_prefix(&root) else {
                continue;
            };
            let exempt_dir = rel.components().any(|c| {
                COMPLETENESS_EXEMPT_DIRS.contains(&c.as_os_str().to_string_lossy().as_ref())
            });
            let exempt_file = rel
                .file_name()
                .is_some_and(|f| COMPLETENESS_EXEMPT_FILES.contains(&f.to_string_lossy().as_ref()));
            if exempt_dir || exempt_file {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(path) else {
                continue;
            };
            if scan_keys(&body).is_empty() {
                continue;
            }
            // `SCANNED_SOURCES` holds file CONTENTS, not paths, so membership is
            // tested by content identity rather than by name.
            if !listed.contains(body.as_str()) {
                unlisted.push(rel.display().to_string());
            }
        }
        unlisted.sort();
        assert!(
            unlisted.is_empty(),
            "these files read a setting key but are absent from SCANNED_SOURCES: {unlisted:?}"
        );
    }

    /// Recursively collects `.rs` files under `dir`.
    fn collect_rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn scanner_sees_resolve_calls_wrapped_across_lines() {
        // Guards the blind spot this scanner was rewritten to close: keys that
        // only ever reach the config layer through a multi-line `resolve_*`.
        let src = r#"
            let v = crate::runtime_config::resolve_u64(
                None,
                "made.up.key",
                7,
            );
        "#;
        assert_eq!(scan_keys(src), vec!["made.up.key".to_string()]);
    }

    #[test]
    fn scanner_ignores_keys_quoted_inside_whole_line_comments() {
        // Guards the trap in this very module's prose.
        let src = "        /// scans for get_setting(\"placeholder\") call sites\n";
        assert!(scan_keys(src).is_empty());
    }
}
