//! `ValueEnum` types shared across subcommand argument structs.

/// Graph export format.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum GraphExportFormat {
    /// JSON variant.
    Json,
    /// DOT variant.
    Dot,
    /// Mermaid variant.
    Mermaid,
    /// Stream one JSON object per entity, then one per edge, then a summary line.
    Ndjson,
}

/// Memory type.
#[derive(Copy, Clone, Debug, Default, clap::ValueEnum)]
pub enum MemoryType {
    /// User variant.
    User,
    /// Feedback variant.
    Feedback,
    /// Project variant.
    Project,
    /// Reference variant.
    Reference,
    /// Decision variant.
    Decision,
    /// Incident variant.
    Incident,
    /// Skill variant.
    Skill,
    /// Document variant.
    #[default]
    Document,
    /// Note variant.
    Note,
}

impl MemoryType {
    /// Return the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
            Self::Decision => "decision",
            Self::Incident => "incident",
            Self::Skill => "skill",
            Self::Document => "document",
            Self::Note => "note",
        }
    }
}
