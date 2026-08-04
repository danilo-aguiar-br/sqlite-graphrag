//! Extracted `heavy_concurrency_tests` (Wave C1).

use super::*;
use clap::Parser;

#[test]
fn command_heavy_detects_init_and_embeddings() {
    let init = Cli::try_parse_from(["sqlite-graphrag", "init"]).expect("parse init");
    assert!(init
        .command
        .as_ref()
        .is_some_and(|c| c.is_embedding_heavy()));

    let remember = Cli::try_parse_from([
        "sqlite-graphrag",
        "remember",
        "--name",
        "test-memory",
        "--type",
        "project",
        "--description",
        "desc",
    ])
    .expect("parse remember");
    assert!(remember
        .command
        .as_ref()
        .is_some_and(|c| c.is_embedding_heavy()));

    let recall = Cli::try_parse_from(["sqlite-graphrag", "recall", "query"]).expect("parse recall");
    assert!(recall
        .command
        .as_ref()
        .is_some_and(|c| c.is_embedding_heavy()));

    let hybrid =
        Cli::try_parse_from(["sqlite-graphrag", "hybrid-search", "query"]).expect("parse hybrid");
    assert!(hybrid
        .command
        .as_ref()
        .is_some_and(|c| c.is_embedding_heavy()));
}

#[test]
fn command_light_does_not_mark_stats() {
    let stats = Cli::try_parse_from(["sqlite-graphrag", "stats"]).expect("parse stats");
    assert!(!stats
        .command
        .as_ref()
        .is_some_and(|c| c.is_embedding_heavy()));
}
