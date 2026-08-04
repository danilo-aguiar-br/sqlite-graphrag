//! `--format` value enums shared by the subcommand argument structs.

/// Output format variants accepted by `--format` CLI flags.
#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// JSON variant.
    #[default]
    Json,
    /// Text variant.
    Text,
    /// Markdown variant.
    Markdown,
}

/// Restricted JSON-only format for commands that always emit JSON.
#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
pub enum JsonOutputFormat {
    /// JSON variant.
    #[default]
    Json,
}
