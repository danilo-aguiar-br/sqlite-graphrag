//! Extracted `json_only_format_tests` (Wave C1).

use super::Cli;
use clap::Parser;

#[test]
fn restore_accepts_only_format_json() {
    assert!(Cli::try_parse_from([
        "sqlite-graphrag",
        "restore",
        "--name",
        "mem",
        "--version",
        "1",
        "--format",
        "json",
    ])
    .is_ok());

    assert!(Cli::try_parse_from([
        "sqlite-graphrag",
        "restore",
        "--name",
        "mem",
        "--version",
        "1",
        "--format",
        "text",
    ])
    .is_err());
}

#[test]
fn hybrid_search_accepts_only_format_json() {
    assert!(Cli::try_parse_from([
        "sqlite-graphrag",
        "hybrid-search",
        "query",
        "--format",
        "json",
    ])
    .is_ok());

    assert!(Cli::try_parse_from([
        "sqlite-graphrag",
        "hybrid-search",
        "query",
        "--format",
        "markdown",
    ])
    .is_err());
}

#[test]
fn remember_recall_rename_vacuum_json_only() {
    assert!(Cli::try_parse_from([
        "sqlite-graphrag",
        "remember",
        "--name",
        "mem",
        "--type",
        "project",
        "--description",
        "desc",
        "--format",
        "json",
    ])
    .is_ok());
    assert!(Cli::try_parse_from([
        "sqlite-graphrag",
        "remember",
        "--name",
        "mem",
        "--type",
        "project",
        "--description",
        "desc",
        "--format",
        "text",
    ])
    .is_err());

    assert!(
        Cli::try_parse_from(["sqlite-graphrag", "recall", "query", "--format", "json",]).is_ok()
    );
    assert!(
        Cli::try_parse_from(["sqlite-graphrag", "recall", "query", "--format", "text",]).is_err()
    );

    assert!(Cli::try_parse_from([
        "sqlite-graphrag",
        "rename",
        "--name",
        "old",
        "--new-name",
        "new",
        "--format",
        "json",
    ])
    .is_ok());
    assert!(Cli::try_parse_from([
        "sqlite-graphrag",
        "rename",
        "--name",
        "old",
        "--new-name",
        "new",
        "--format",
        "markdown",
    ])
    .is_err());

    assert!(Cli::try_parse_from(["sqlite-graphrag", "vacuum", "--format", "json",]).is_ok());
    assert!(Cli::try_parse_from(["sqlite-graphrag", "vacuum", "--format", "text",]).is_err());
}
