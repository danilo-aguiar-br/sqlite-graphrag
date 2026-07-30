//! Shell completion script generation.

use clap::CommandFactory;
use clap_complete::{generate, Shell};

use crate::cli_db_noop::DbNoopArgs;

/// Completions args.
#[derive(clap::Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
    /// GAP-SG-139: accepted as a no-op for agent uniformity (no graph I/O).
    #[command(flatten)]
    pub db_noop: DbNoopArgs,
}

/// Run.
pub fn run(args: CompletionsArgs) -> Result<(), crate::errors::AppError> {
    args.db_noop.ignore();
    let mut cmd = crate::cli::Cli::command();
    let bin_name = cmd.get_name().to_string();
    generate(args.shell, &mut cmd, bin_name, &mut std::io::stdout());
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    #[test]
    fn completions_accepts_db_as_noop() {
        let cli = crate::cli::Cli::try_parse_from([
            "sqlite-graphrag",
            "completions",
            "bash",
            "--db",
            "/tmp/gap-sg-139-sentinel.sqlite",
        ])
        .expect("completions must accept --db as a no-op (GAP-SG-139)");

        match cli.command {
            Some(crate::cli::Commands::Completions(args)) => {
                assert_eq!(
                    args.db_noop.db.as_deref(),
                    Some("/tmp/gap-sg-139-sentinel.sqlite")
                );
            }
            other => panic!("expected Completions, got {other:?}"),
        }
    }
}
