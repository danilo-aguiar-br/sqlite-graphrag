//! Extracted `fase_g_parsing_tests` (Wave C1).

    use super::Cli;
    use clap::Parser;

    /// GAP-SG-31(b): `enrich --status` parses without --operation/--mode.
    #[test]
    fn enrich_status_optional_operation_and_mode() {
        assert!(
            Cli::try_parse_from(["sqlite-graphrag", "enrich", "--status"]).is_ok(),
            "--status alone must not require --operation/--mode"
        );
        assert!(
            Cli::try_parse_from(["sqlite-graphrag", "enrich", "--list-dead"]).is_ok(),
            "--list-dead is read-only and must not require --operation/--mode"
        );
        // Write path still requires both: bare `enrich` is rejected.
        assert!(
            Cli::try_parse_from(["sqlite-graphrag", "enrich"]).is_err(),
            "bare enrich (no status/list-dead/requeue-dead) must require --operation/--mode"
        );
        // Full write invocation still parses.
        assert!(Cli::try_parse_from([
            "sqlite-graphrag",
            "enrich",
            "--operation",
            "memory-bindings",
            "--mode",
            "openrouter",
        ])
        .is_ok());
    }

    /// GAP-SG-34(c): `config doctor --json` parses (no-op flag accepted).
    #[test]
    fn config_doctor_accepts_json() {
        assert!(Cli::try_parse_from(["sqlite-graphrag", "config", "doctor", "--json"]).is_ok());
        assert!(Cli::try_parse_from(["sqlite-graphrag", "config", "list-keys", "--json"]).is_ok());
    }

    /// GAP-SG-33(d): a hyphen-led --description value is accepted, not parsed
    /// as a flag.
    #[test]
    fn remember_description_allows_leading_hyphen() {
        assert!(Cli::try_parse_from([
            "sqlite-graphrag",
            "remember",
            "--name",
            "mem",
            "--type",
            "note",
            "--description",
            "- bullet description",
        ])
        .is_ok());
    }

    /// GAP-SG-35(e): `remember-batch --llm-parallelism N` parses.
    #[test]
    fn remember_batch_accepts_llm_parallelism() {
        assert!(Cli::try_parse_from([
            "sqlite-graphrag",
            "remember-batch",
            "--llm-parallelism",
            "4"
        ])
        .is_ok());
    }

    /// GAP-SG-30: --graph-file combines with a body source but conflicts with
    /// the other graph-input flags.
    #[test]
    fn remember_graph_file_combines_with_body_but_conflicts_with_graph_stdin() {
        assert!(
            Cli::try_parse_from([
                "sqlite-graphrag",
                "remember",
                "--name",
                "mem",
                "--type",
                "note",
                "--body",
                "inline body",
                "--graph-file",
                "/tmp/graph.json",
            ])
            .is_ok(),
            "--body + --graph-file must coexist"
        );
        assert!(
            Cli::try_parse_from([
                "sqlite-graphrag",
                "remember",
                "--name",
                "mem",
                "--type",
                "note",
                "--graph-file",
                "/tmp/graph.json",
                "--graph-stdin",
            ])
            .is_err(),
            "--graph-file conflicts with --graph-stdin"
        );
    }
