//! Auto-extracted tests (Wave C1).

    use super::args::{low_memory_setting_enabled, resolve_parallelism};
    use super::persist::persist_staged;
    use super::report::IngestDryRunBudget;
    use super::scan_fs::{
        collect_files, derive_kebab_name, matches_pattern, unique_name, validate_name_prefix,
        MAX_NAME_COLLISION_SUFFIX,
    };
    use super::stage::StagedFile;
    use super::validate::validate_mode_conditional_flags_ingest;
    use crate::chunking;
    use crate::constants::DERIVED_NAME_MAX_LEN;
    use crate::errors::AppError;
    use rusqlite::Connection;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    // v1.1.1 (P12): --name-prefix validation and budget arithmetic.
    #[test]
    fn validate_name_prefix_shrinks_budget_to_fit_name_cap() {
        // 80-char cap; a 10-char prefix leaves 70 for the derived part, but
        // the caller's budget (60) is smaller, so it wins.
        let budget = validate_name_prefix("projx-team", 60).unwrap();
        assert_eq!(budget, 60);
        // A long prefix shrinks the budget below the caller's 60.
        let long_prefix = "p".repeat(75);
        let budget = validate_name_prefix(&long_prefix, 60).unwrap();
        assert_eq!(budget, 5, "80-char cap minus 75-char prefix leaves 5");
    }

    #[test]
    fn validate_name_prefix_rejects_invalid_slugs() {
        for bad in ["", "-lead", "Upper", "has_underscore", "acentuação", "1x"] {
            let err = validate_name_prefix(bad, 60).unwrap_err();
            assert_eq!(err.exit_code(), 1, "prefix '{bad}' must be Validation");
        }
    }

    #[test]
    fn validate_name_prefix_too_long_is_limit_exceeded() {
        let huge = "p".repeat(crate::constants::MAX_MEMORY_NAME_LEN);
        let err = validate_name_prefix(&huge, 60).unwrap_err();
        assert_eq!(err.exit_code(), 6, "prefix >= name cap must be exit 6");
        assert!(
            err.to_string().contains("MAX_MEMORY_NAME_LEN"),
            "obtido: {err}"
        );
    }

    #[test]
    fn name_prefix_applies_after_kebab_normalization_and_fits_cap() {
        let prefix = "projx-";
        let budget = validate_name_prefix(prefix, 60).unwrap();
        let (base, _, _) = derive_kebab_name(&PathBuf::from("My File Name.md"), budget);
        let final_name = format!("{prefix}{base}");
        assert_eq!(final_name, "projx-my-file-name");
        assert!(final_name.len() <= crate::constants::MAX_MEMORY_NAME_LEN);
        assert!(crate::constants::name_slug_regex().is_match(&final_name));
    }

    /// GAP-SG-29: `ingest --mode none --resume` is rejected fail-fast by the
    /// mode-conditional validator, which `run()` invokes as its very first
    /// statement (before any DB/IO). clap 4.6 derive cannot express a
    /// value-conditional conflict (`--mode=none` vs `--resume`) without also
    /// breaking the valid `--mode claude-code --resume` combo, so the contract
    /// is enforced here instead of at the parser layer.
    #[test]
    fn ingest_mode_none_with_resume_is_rejected() {
        use crate::cli::{Cli, Commands};
        use clap::Parser;

        let none_resume = Cli::try_parse_from([
            "sqlite-graphrag",
            "ingest",
            "./docs",
            "--mode",
            "none",
            "--resume",
        ])
        .expect("parse succeeds; the conflict is value-conditional");
        let args = match none_resume.command {
            Some(Commands::Ingest(a)) => a,
            other => panic!("expected ingest, got {other:?}"),
        };
        assert!(
            validate_mode_conditional_flags_ingest(&args).is_err(),
            "--mode none + --resume must be rejected fail-fast"
        );

        // The valid LLM-mode combo is NOT rejected.
        let claude_resume = Cli::try_parse_from([
            "sqlite-graphrag",
            "ingest",
            "./docs",
            "--mode",
            "claude-code",
            "--resume",
        ])
        .expect("parse");
        let args = match claude_resume.command {
            Some(Commands::Ingest(a)) => a,
            other => panic!("expected ingest, got {other:?}"),
        };
        assert!(
            validate_mode_conditional_flags_ingest(&args).is_ok(),
            "--mode claude-code + --resume is valid and must pass"
        );
    }

    fn setup_ingest_conn() -> Connection {
        crate::storage::connection::register_vec_extension();
        let mut conn = Connection::open_in_memory().unwrap();
        crate::migrations::runner().run(&mut conn).unwrap();
        conn
    }

    fn make_staged(name: &str, body: &str) -> StagedFile {
        StagedFile {
            body: body.to_string(),
            body_hash: blake3::hash(body.as_bytes()).to_hex().to_string(),
            snippet: body.chars().take(200).collect(),
            name: name.to_string(),
            description: "desc".to_string(),
            embedding: None,
            chunk_embeddings: None,
            chunks_info: Vec::new(),
            entities: Vec::new(),
            relationships: Vec::new(),
            entity_embeddings: None,
            urls: Vec::new(),
            backend_invoked: None,
        }
    }

    // GAP-SG-54: re-ingesting the same name without --force-merge is a duplicate
    // (skipped); with --force-merge it updates in place.
    #[test]
    fn persist_staged_force_merge_updates_existing() {
        let mut conn = setup_ingest_conn();

        let first = persist_staged(
            &mut conn,
            "global",
            "document",
            make_staged("doc-a", "v1"),
            false,
        )
        .expect("create");
        assert_eq!(first.action, "created");

        // Same name, no force_merge → Duplicate (skip).
        let dup = persist_staged(
            &mut conn,
            "global",
            "document",
            make_staged("doc-a", "v2-changed"),
            false,
        );
        assert!(matches!(dup, Err(AppError::Duplicate(_))));

        // Same name, force_merge → updated, body refreshed.
        let upd = persist_staged(
            &mut conn,
            "global",
            "document",
            make_staged("doc-a", "v2-changed"),
            true,
        )
        .expect("update");
        assert_eq!(upd.action, "updated");
        assert_eq!(upd.memory_id, first.memory_id);
        let body: String = conn
            .query_row(
                "SELECT body FROM memories WHERE id = ?1",
                rusqlite::params![first.memory_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(body, "v2-changed");
    }

    // GAP-SG-55: identical body under a divergent name is deduped (skipped).
    #[test]
    fn persist_staged_dedupes_by_body_hash() {
        let mut conn = setup_ingest_conn();
        persist_staged(
            &mut conn,
            "global",
            "document",
            make_staged("parte-1", "identical content"),
            false,
        )
        .expect("create");

        // Divergent derived name, same content → skipped as duplicate.
        let res = persist_staged(
            &mut conn,
            "global",
            "document",
            make_staged("part-01", "identical content"),
            false,
        );
        match res {
            Err(AppError::Duplicate(msg)) => assert!(msg.contains("body_hash")),
            other => panic!("expected body_hash dedup duplicate, got {other:?}"),
        }
        // Only one memory persisted.
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    // GAP-SG-54: `ingest --force-merge` parses and sets the update flag.
    #[test]
    fn ingest_force_merge_flag_parses() {
        use crate::cli::{Cli, Commands};
        use clap::Parser;
        let cli = Cli::try_parse_from(["sqlite-graphrag", "ingest", "./docs", "--force-merge"])
            .expect("parse");
        match cli.command {
            Some(Commands::Ingest(a)) => assert!(a.force_merge),
            other => panic!("expected ingest, got {other:?}"),
        }
        // Default is off.
        let cli2 = Cli::try_parse_from(["sqlite-graphrag", "ingest", "./docs"]).expect("parse");
        match cli2.command {
            Some(Commands::Ingest(a)) => assert!(!a.force_merge),
            other => panic!("expected ingest, got {other:?}"),
        }
    }

    #[test]
    fn matches_pattern_suffix() {
        assert!(matches_pattern("foo.md", "*.md"));
        assert!(!matches_pattern("foo.txt", "*.md"));
        assert!(matches_pattern("foo.md", "*"));
    }

    #[test]
    fn matches_pattern_prefix() {
        assert!(matches_pattern("README.md", "README*"));
        assert!(!matches_pattern("CHANGELOG.md", "README*"));
    }

    #[test]
    fn matches_pattern_exact() {
        assert!(matches_pattern("README.md", "README.md"));
        assert!(!matches_pattern("readme.md", "README.md"));
    }

    #[test]
    fn derive_kebab_underscore_to_dash() {
        let p = PathBuf::from("/tmp/claude_code_headless.md");
        let (name, truncated, original) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert_eq!(name, "claude-code-headless");
        assert!(!truncated);
        assert!(original.is_none());
    }

    #[test]
    fn derive_kebab_uppercase_lowered() {
        let p = PathBuf::from("/tmp/README.md");
        let (name, truncated, original) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert_eq!(name, "readme");
        assert!(!truncated);
        assert!(original.is_none());
    }

    #[test]
    fn derive_kebab_strips_non_kebab_chars() {
        let p = PathBuf::from("/tmp/some@weird#name!.md");
        let (name, truncated, original) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert_eq!(name, "someweirdname");
        assert!(!truncated);
        assert!(original.is_none());
    }

    // Bug M-A3: NFD-based unicode normalization preserves base letters of
    // accented characters instead of dropping them entirely.
    #[test]
    fn derive_kebab_folds_accented_letters_to_ascii() {
        let p = PathBuf::from("/tmp/açaí.md");
        let (name, _, _) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert_eq!(name, "acai", "got '{name}'");
    }

    #[test]
    fn derive_kebab_handles_naive_with_diaeresis() {
        let p = PathBuf::from("/tmp/naïve-test.md");
        let (name, _, _) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert_eq!(name, "naive-test", "got '{name}'");
    }

    #[test]
    fn derive_kebab_drops_emoji_keeps_word() {
        let p = PathBuf::from("/tmp/🚀-rocket.md");
        let (name, _, _) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert_eq!(name, "rocket", "got '{name}'");
    }

    #[test]
    fn derive_kebab_mixed_unicode_emoji_keeps_letters() {
        let p = PathBuf::from("/tmp/açaí🦜.md");
        let (name, _, _) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert_eq!(name, "acai", "got '{name}'");
    }

    #[test]
    fn derive_kebab_pure_emoji_yields_empty() {
        let p = PathBuf::from("/tmp/🦜🚀🌟.md");
        let (name, _, _) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert!(name.is_empty(), "got '{name}'");
    }

    #[test]
    fn derive_kebab_collapses_consecutive_dashes() {
        let p = PathBuf::from("/tmp/a__b___c.md");
        let (name, truncated, original) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert_eq!(name, "a-b-c");
        assert!(!truncated);
        assert!(original.is_none());
    }

    #[test]
    fn derive_kebab_truncates_to_60_chars() {
        let p = PathBuf::from(format!("/tmp/{}.md", "a".repeat(80)));
        let (name, truncated, original) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert!(name.len() <= 60, "got len {}", name.len());
        assert!(truncated);
        assert!(original.is_some());
        assert!(original.unwrap().len() > 60);
    }

    #[test]
    fn collect_files_finds_md_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("a.md"), "x").unwrap();
        std::fs::write(tmp.path().join("b.md"), "y").unwrap();
        std::fs::write(tmp.path().join("c.txt"), "z").unwrap();
        let mut out = Vec::new();
        collect_files(tmp.path(), "*.md", false, &mut out).expect("collect");
        assert_eq!(out.len(), 2, "should find 2 .md files, got {out:?}");
    }

    #[test]
    fn collect_files_recursive_descends_subdirs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(tmp.path().join("a.md"), "x").unwrap();
        std::fs::write(sub.join("b.md"), "y").unwrap();
        let mut out = Vec::new();
        collect_files(tmp.path(), "*.md", true, &mut out).expect("collect");
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn collect_files_non_recursive_skips_subdirs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(tmp.path().join("a.md"), "x").unwrap();
        std::fs::write(sub.join("b.md"), "y").unwrap();
        let mut out = Vec::new();
        collect_files(tmp.path(), "*.md", false, &mut out).expect("collect");
        assert_eq!(out.len(), 1);
    }

    // ── v1.0.31 A10: name truncation warns and collisions are auto-resolved ──

    #[test]
    fn derive_kebab_long_basename_truncated_within_cap() {
        let p = PathBuf::from(format!("/tmp/{}.md", "a".repeat(120)));
        let (name, truncated, original) = derive_kebab_name(&p, DERIVED_NAME_MAX_LEN);
        assert!(
            name.len() <= DERIVED_NAME_MAX_LEN,
            "truncated name must respect cap; got {} chars",
            name.len()
        );
        assert!(!name.is_empty());
        assert!(truncated);
        assert!(original.is_some());
    }

    #[test]
    fn unique_name_returns_base_when_free() {
        let taken: BTreeSet<String> = BTreeSet::new();
        let resolved = unique_name("note", &taken).expect("must resolve");
        assert_eq!(resolved, "note");
    }

    #[test]
    fn unique_name_appends_first_free_suffix_on_collision() {
        let mut taken: BTreeSet<String> = BTreeSet::new();
        taken.insert("note".to_string());
        taken.insert("note-1".to_string());
        let resolved = unique_name("note", &taken).expect("must resolve");
        assert_eq!(resolved, "note-2");
    }

    #[test]
    fn unique_name_errors_after_collision_cap() {
        let mut taken: BTreeSet<String> = BTreeSet::new();
        taken.insert("note".to_string());
        for i in 1..=MAX_NAME_COLLISION_SUFFIX {
            taken.insert(format!("note-{i}"));
        }
        let err = unique_name("note", &taken).expect_err("must surface error");
        assert!(matches!(err, AppError::Validation(_)));
    }

    // ── v1.0.32 Onda 4B: in-process pipeline validation ──

    #[test]
    fn validate_relation_format_accepts_valid_relations() {
        use crate::parsers::{is_canonical_relation, validate_relation_format};
        assert!(validate_relation_format("applies_to").is_ok());
        assert!(validate_relation_format("depends_on").is_ok());
        assert!(validate_relation_format("implements").is_ok());
        assert!(validate_relation_format("").is_err());
        assert!(is_canonical_relation("applies_to"));
        assert!(!is_canonical_relation("implements"));
    }

    // ── `--low-memory` flag and the XDG setting `ingest.low_memory` ──
    //
    // GAP-SG-83/84: these cases used to be named after the retired product env
    // `SQLITE_GRAPHRAG_LOW_MEMORY`, which no reader consults since `G-T-XDG-04`.
    // The names now state what is actually asserted: the env path is INERT and
    // the XDG setting is the only channel.

    use serial_test::serial;

    /// Retired product env, kept here only so the inertness cases can set it.
    const RETIRED_LOW_MEMORY_ENV: &str = "SQLITE_GRAPHRAG_LOW_MEMORY";

    /// Sets (or clears) the retired env around a closure and restores it after,
    /// so a leaked value cannot make a sibling case pass for the wrong reason.
    fn with_retired_env<F: FnOnce()>(value: Option<&str>, f: F) {
        let prev = std::env::var(RETIRED_LOW_MEMORY_ENV).ok();
        match value {
            Some(v) => std::env::set_var(RETIRED_LOW_MEMORY_ENV, v),
            None => std::env::remove_var(RETIRED_LOW_MEMORY_ENV),
        }
        f();
        match prev {
            Some(p) => std::env::set_var(RETIRED_LOW_MEMORY_ENV, p),
            None => std::env::remove_var(RETIRED_LOW_MEMORY_ENV),
        }
    }

    #[test]
    #[serial]
    fn low_memory_setting_absent_returns_false() {
        with_retired_env(None, || assert!(!low_memory_setting_enabled()));
    }

    /// The decisive case: a truthy retired env must NOT enable low-memory.
    /// Asserting the negative is the whole point — a test that merely calls the
    /// function would keep passing if the env path were reintroduced.
    #[test]
    #[serial]
    fn truthy_retired_env_does_not_enable_low_memory() {
        for v in ["1", "true", "yes", "on"] {
            with_retired_env(Some(v), || {
                assert!(
                    !low_memory_setting_enabled(),
                    "retired env {RETIRED_LOW_MEMORY_ENV}={v:?} must stay inert"
                );
            });
        }
    }

    #[test]
    #[serial]
    fn falsy_and_unrecognized_retired_env_stay_inert() {
        for v in ["", "0", "false", "FALSE", "no", "off", "maybe"] {
            with_retired_env(Some(v), || {
                assert!(!low_memory_setting_enabled(), "value {v:?} must be falsy")
            });
        }
    }

    #[test]
    #[serial]
    fn resolve_parallelism_flag_forces_one_overriding_explicit_value() {
        with_retired_env(None, || {
            assert_eq!(resolve_parallelism(true, Some(4)), 1);
            assert_eq!(resolve_parallelism(true, Some(8)), 1);
            assert_eq!(resolve_parallelism(true, None), 1);
        });
    }

    #[test]
    #[serial]
    fn resolve_parallelism_ignores_retired_env() {
        // G-T-XDG-04: the retired product env must not shrink the pool.
        with_retired_env(Some("1"), || {
            assert_eq!(resolve_parallelism(false, Some(4)), 4);
        });
    }

    #[test]
    #[serial]
    fn resolve_parallelism_falsy_env_does_not_override() {
        with_retired_env(Some("0"), || {
            assert_eq!(resolve_parallelism(false, Some(4)), 4);
        });
    }

    #[test]
    #[serial]
    fn resolve_parallelism_explicit_value_when_low_memory_off() {
        with_retired_env(None, || {
            assert_eq!(resolve_parallelism(false, Some(3)), 3);
            assert_eq!(resolve_parallelism(false, Some(1)), 1);
        });
    }

    #[test]
    #[serial]
    fn resolve_parallelism_default_when_unset() {
        with_retired_env(None, || {
            let p = resolve_parallelism(false, None);
            assert!((1..=4).contains(&p), "default must be in [1, 4]; got {p}");
        });
    }

    #[test]
    fn ingest_args_parses_low_memory_flag_via_clap() {
        use clap::Parser;
        // Parse a synthetic Cli that contains the `ingest` subcommand. We rely
        // on the public `Cli` definition so the flag is wired end-to-end.
        let cli = crate::cli::Cli::try_parse_from([
            "sqlite-graphrag",
            "ingest",
            "/tmp/dummy",
            "--type",
            "document",
            "--low-memory",
        ])
        .expect("parse must succeed");
        match cli.command {
            Some(crate::cli::Commands::Ingest(args)) => {
                assert!(args.low_memory, "--low-memory must set field to true");
            }
            _ => panic!("expected Ingest subcommand"),
        }
    }

    #[test]
    fn ingest_args_low_memory_defaults_false() {
        use clap::Parser;
        let cli = crate::cli::Cli::try_parse_from([
            "sqlite-graphrag",
            "ingest",
            "/tmp/dummy",
            "--type",
            "document",
        ])
        .expect("parse must succeed");
        match cli.command {
            Some(crate::cli::Commands::Ingest(args)) => {
                assert!(!args.low_memory, "default must be false");
            }
            _ => panic!("expected Ingest subcommand"),
        }
    }

    // ── GAP-SG-06: --dry-run reports chunk and token counts ──

    #[test]
    fn dry_run_budget_event_serializes_chunk_and_token_counts() {
        let ev = IngestDryRunBudget {
            budget: true,
            file: "/tmp/doc.md",
            name: "doc",
            bytes: 1234,
            chunk_count: 3,
            token_count: 567,
            partition_count: 1,
            exceeds_limits: false,
        };
        let json = serde_json::to_string(&ev).expect("serialize budget event");
        assert!(json.contains("\"chunk_count\":3"), "got: {json}");
        assert!(json.contains("\"token_count\":567"), "got: {json}");
        assert!(json.contains("\"partition_count\":1"), "got: {json}");
        assert!(json.contains("\"exceeds_limits\":false"), "got: {json}");
    }

    #[test]
    fn assess_body_budget_feeds_dry_run_with_positive_counts() {
        // The dry-run path feeds chunking::assess_body_budget; a representative
        // body must report a positive chunk and token count.
        let body = "# Title\n\nsome representative body text for the budget.";
        let budget = chunking::assess_body_budget(body);
        assert!(budget.chunk_count >= 1);
        assert!(budget.approx_tokens >= 1);
        assert_eq!(budget.partition_count, 1);
    }
