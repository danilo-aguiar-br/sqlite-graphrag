//! Shared no-op `--db` flag for host/XDG CLI surfaces (GAP-SG-139).
//!
//! Graph-scoped subcommands resolve storage via `--db`. Host/XDG surfaces
//! (`config`, `slots`, `cache`, `completions`) never open the
//! graph database, but agents that append `--db` to every one-shot invocation
//! must not receive clap exit 2. This module provides a single help string and
//! an optional [`DbNoopArgs`](crate::cli_db_noop::DbNoopArgs) group for
//! `#[command(flatten)]` composition.

/// Help text for host/XDG `--db` no-op (English; agent-facing clap surface).
pub const DB_NOOP_HELP: &str = "Explicit database path. Accepted as a no-op for agent uniformity; host/XDG surfaces do not open the graph database";

/// Flattenable no-op `--db` for host/XDG argument structs (GAP-SG-139).
#[derive(Debug, Clone, Default, clap::Args)]
pub struct DbNoopArgs {
    /// Explicit database path. Accepted as a no-op for agent uniformity;
    /// host/XDG surfaces do not open the graph database.
    #[arg(long, value_name = "PATH", help = DB_NOOP_HELP)]
    pub db: Option<String>,
}

impl DbNoopArgs {
    /// Drop the value without using it (explicit no-op at the handler).
    #[inline]
    pub fn ignore(self) {
        let _ = self.db;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct Probe {
        #[command(flatten)]
        db_noop: DbNoopArgs,
    }

    #[test]
    fn db_noop_parses_long_flag() {
        let p = Probe::try_parse_from(["probe", "--db", "/tmp/sentinel.sqlite"])
            .expect("DbNoopArgs must accept --db");
        assert_eq!(p.db_noop.db.as_deref(), Some("/tmp/sentinel.sqlite"));
        p.db_noop.ignore();
    }

    #[test]
    fn db_noop_optional() {
        let p = Probe::try_parse_from(["probe"]).expect("DbNoopArgs must be optional");
        assert!(p.db_noop.db.is_none());
    }
}
