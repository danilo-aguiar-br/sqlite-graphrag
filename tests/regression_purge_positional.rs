//! Regression tests for GAP-SG-272: `purge` accepts the name positionally.
//!
//! Three cases are covered:
//! 1. The positional `NAME` alone is captured and `--name` stays `None`.
//! 2. The `--name` flag alone keeps working, preserving every existing caller.
//! 3. Supplying both spellings is refused by clap, which exits with code 2.
//!
//! Omitting both spellings must stay valid, because that is what selects the
//! bulk mode driven by `--retention-days`.

use clap::Parser;
use sqlite_graphrag::cli::{Cli, Commands};

#[test]
fn regression_purge_args_accepts_name_positional() {
    let cli = Cli::try_parse_from(["sqlite-graphrag", "purge", "old-memory"])
        .expect("positional NAME must parse");
    let Some(Commands::Purge(args)) = cli.command else {
        panic!("expected the Purge command");
    };
    assert_eq!(
        args.name_positional.as_deref(),
        Some("old-memory"),
        "PurgeArgs must capture the positional NAME"
    );
    assert!(
        args.name.is_none(),
        "PurgeArgs.name must be None when the positional is used"
    );
}

#[test]
fn regression_purge_args_accepts_flag_name() {
    let cli = Cli::try_parse_from(["sqlite-graphrag", "purge", "--name", "old-memory"])
        .expect("--name must parse");
    let Some(Commands::Purge(args)) = cli.command else {
        panic!("expected the Purge command");
    };
    assert_eq!(
        args.name.as_deref(),
        Some("old-memory"),
        "PurgeArgs must keep capturing --name for back-compat"
    );
    assert!(
        args.name_positional.is_none(),
        "PurgeArgs.name_positional must be None when --name is used"
    );
}

#[test]
fn regression_purge_args_rejects_both_spellings_with_exit_code_2() {
    // `Cli` does not implement `Debug`, so the Ok arm cannot be unwrapped here.
    let parsed = Cli::try_parse_from([
        "sqlite-graphrag",
        "purge",
        "old-memory",
        "--name",
        "old-memory",
    ]);
    let err = match parsed {
        Ok(_) => panic!("supplying both spellings must be refused"),
        Err(err) => err,
    };
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::ArgumentConflict,
        "the refusal must come from the parser conflict, not from the handler"
    );
    assert_eq!(
        err.exit_code(),
        2,
        "a bad command line must exit with code 2"
    );
}

#[test]
fn regression_purge_args_without_name_keeps_bulk_mode() {
    let cli = Cli::try_parse_from(["sqlite-graphrag", "purge", "--retention-days", "30"])
        .expect("bulk mode must parse without any name");
    let Some(Commands::Purge(args)) = cli.command else {
        panic!("expected the Purge command");
    };
    assert!(args.name_positional.is_none());
    assert!(args.name.is_none());
    assert_eq!(args.retention_days, 30);
}
