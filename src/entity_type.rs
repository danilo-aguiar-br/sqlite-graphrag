//! Canonical entity type taxonomy used across extraction, storage and CLI.
//!
//! `EntityType` is the single source of truth for the 13 graph entity kinds.
//! It derives `clap::ValueEnum` so CLI flags can use it directly, and derives
//! `serde::{Serialize, Deserialize}` with `rename_all = "lowercase"` so JSON
//! round-trips remain backward-compatible with the pre-enum string format.

use crate::errors::AppError;

/// The 13 canonical graph entity classifications.
///
/// Values are serialized as lowercase strings (`"person"`, `"organization"`,
/// etc.) matching the pre-enum wire format and the SQLite `type` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[clap(rename_all = "snake_case")]
pub enum EntityType {
    /// Concept variant.
    Concept,
    /// Date variant.
    Date,
    /// Dashboard variant.
    Dashboard,
    /// Decision variant.
    Decision,
    /// File variant.
    File,
    /// Incident variant.
    Incident,
    /// Issue tracker variant.
    IssueTracker,
    /// Location variant.
    Location,
    /// Memory variant.
    Memory,
    /// Organization variant.
    Organization,
    /// Person variant.
    Person,
    /// Project variant.
    Project,
    /// Tool variant.
    Tool,
}

impl EntityType {
    /// Returns the canonical lowercase string representation stored in SQLite.
    pub fn as_str(self) -> &'static str {
        match self {
            EntityType::Concept => "concept",
            EntityType::Date => "date",
            EntityType::Dashboard => "dashboard",
            EntityType::Decision => "decision",
            EntityType::File => "file",
            EntityType::Incident => "incident",
            EntityType::IssueTracker => "issue_tracker",
            EntityType::Location => "location",
            EntityType::Memory => "memory",
            EntityType::Organization => "organization",
            EntityType::Person => "person",
            EntityType::Project => "project",
            EntityType::Tool => "tool",
        }
    }

    /// Maps an arbitrary type label to the closest canonical [`EntityType`],
    /// never failing (GAP-SG-47).
    ///
    /// LLM extraction routinely emits type labels outside the 13 canonical
    /// kinds (`platform`, `language`, `feature`, `framework`, ...). The old
    /// parse path discarded those entities with a `WARN`, silently losing
    /// legitimate graph nodes. This function PRESERVES them by folding each
    /// label onto the nearest canonical kind. Anything it cannot place falls
    /// back to [`EntityType::Concept`], the most general kind — so a label is
    /// never dropped.
    ///
    /// Matching is case-insensitive and treats hyphens as underscores, so
    /// `"Issue-Tracker"` resolves to [`EntityType::IssueTracker`].
    /// Exact match against the 13 canonical kinds (case/hyphen-insensitive).
    fn parse_exact(key: &str) -> Option<EntityType> {
        match key {
            "concept" => Some(EntityType::Concept),
            "date" => Some(EntityType::Date),
            "dashboard" => Some(EntityType::Dashboard),
            "decision" => Some(EntityType::Decision),
            "file" => Some(EntityType::File),
            "incident" => Some(EntityType::Incident),
            "issue_tracker" => Some(EntityType::IssueTracker),
            "location" => Some(EntityType::Location),
            "memory" => Some(EntityType::Memory),
            "organization" => Some(EntityType::Organization),
            "person" => Some(EntityType::Person),
            "project" => Some(EntityType::Project),
            "tool" => Some(EntityType::Tool),
            _ => None,
        }
    }

    /// Map to canonical.
    pub fn map_to_canonical(s: &str) -> EntityType {
        let key = s.trim().to_lowercase().replace('-', "_");
        // Exact canonical (and case/hyphen-insensitive) match first.
        if let Some(et) = Self::parse_exact(&key) {
            return et;
        }
        match key.as_str() {
            // Concept-like: abstractions, technologies, capabilities, topics.
            "platform" | "language" | "feature" | "framework" | "library" | "technology"
            | "software" | "service" | "product" | "system" | "api" | "component" | "module"
            | "package" | "dependency" | "protocol" | "standard" | "format" | "algorithm"
            | "pattern" | "method" | "function" | "class" | "interface" | "command" | "flag"
            | "option" | "config" | "setting" | "version" | "release" | "model" | "metric"
            | "topic" | "skill" | "reference" | "note" | "feedback" | "url" | "link"
            | "keyword" | "tag" | "category" => EntityType::Concept,
            // File-like: documents, paths, code artifacts.
            "document" | "doc" | "artifact" | "directory" | "folder" | "path" | "repository"
            | "repo" | "codebase" | "script" => EntityType::File,
            // Person-like roles.
            "user" | "author" | "developer" | "maintainer" | "contributor" | "agent" | "owner"
            | "assignee" => EntityType::Person,
            // Organization-like collectives.
            "company" | "org" | "vendor" | "group" | "team" | "department" | "institution" => {
                EntityType::Organization
            }
            // Incident-like failures.
            "bug" | "error" | "failure" | "outage" | "vulnerability" | "cve" | "regression"
            | "defect" => EntityType::Incident,
            // Decision-like records.
            "adr" | "choice" | "policy" | "ruling" => EntityType::Decision,
            // Date-like temporals.
            "time" | "datetime" | "timestamp" | "day" | "month" | "year" | "deadline"
            | "milestone" => EntityType::Date,
            // Location-like places.
            "city" | "country" | "region" | "place" | "address" | "site" => EntityType::Location,
            // Issue-tracker-like.
            "ticket" | "issue" | "jira" | "github_issue" | "pr" | "pull_request" => {
                EntityType::IssueTracker
            }
            // Dashboard-like.
            "panel" | "board" | "view" | "report" | "chart" => EntityType::Dashboard,
            // Anything else: the most general canonical kind, never dropped.
            _ => EntityType::Concept,
        }
    }
}

/// v1.1.8: manual `Deserialize` folds aliases via [`EntityType::map_to_canonical`]
/// so EVERY JSON entry point (`--graph-stdin`, `--entities-file`, enrich)
/// accepts extraction labels like `module`/`platform` without dropping nodes.
/// Persistence always stores the 13 canonical kinds.
impl<'de> serde::Deserialize<'de> for EntityType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(EntityType::map_to_canonical(&s))
    }
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for EntityType {
    type Err = AppError;

    /// Folds non-canonical labels onto the nearest kind (never rejects).
    /// Canonical kinds round-trip exactly; aliases like `module` → Concept.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(EntityType::map_to_canonical(s))
    }
}

impl rusqlite::types::FromSql for EntityType {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = String::column_result(value)?;
        s.parse::<EntityType>().map_err(|e| {
            rusqlite::types::FromSqlError::Other(Box::new(std::io::Error::other(e.to_string())))
        })
    }
}

impl rusqlite::types::ToSql for EntityType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::from(self.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // v1.1.8: serde folds aliases via map_to_canonical (never rejects).
    #[test]
    fn deserialize_folds_aliases_and_unknown_to_canonical() {
        assert_eq!(
            serde_json::from_str::<EntityType>("\"reference\"").unwrap(),
            EntityType::Concept
        );
        assert_eq!(
            serde_json::from_str::<EntityType>("\"module\"").unwrap(),
            EntityType::Concept
        );
        assert_eq!(
            serde_json::from_str::<EntityType>("\"document\"").unwrap(),
            EntityType::File
        );
        assert_eq!(
            serde_json::from_str::<EntityType>("\"banana\"").unwrap(),
            EntityType::Concept
        );
    }

    #[test]
    fn deserialize_valid_and_case_insensitive_entity_type() {
        assert_eq!(
            serde_json::from_str::<EntityType>("\"issue_tracker\"").unwrap(),
            EntityType::IssueTracker
        );
        // FromStr lowercases, so serde now accepts mixed case like the CLI.
        assert_eq!(
            serde_json::from_str::<EntityType>("\"Tool\"").unwrap(),
            EntityType::Tool
        );
    }

    #[test]
    fn serialize_stays_snake_case() {
        assert_eq!(
            serde_json::to_string(&EntityType::IssueTracker).unwrap(),
            "\"issue_tracker\""
        );
    }

    #[test]
    fn from_str_lowercase_roundtrip() {
        assert_eq!("person".parse::<EntityType>().unwrap(), EntityType::Person);
        assert_eq!(
            "organization".parse::<EntityType>().unwrap(),
            EntityType::Organization
        );
        assert_eq!(
            "issue_tracker".parse::<EntityType>().unwrap(),
            EntityType::IssueTracker
        );
    }

    #[test]
    fn from_str_uppercase_is_case_insensitive() {
        assert_eq!("PERSON".parse::<EntityType>().unwrap(), EntityType::Person);
        assert_eq!(
            "Organization".parse::<EntityType>().unwrap(),
            EntityType::Organization
        );
    }

    #[test]
    fn from_str_unknown_folds_to_concept() {
        let result = "not-a-real-type".parse::<EntityType>();
        assert_eq!(result.unwrap(), EntityType::Concept);
    }

    #[test]
    fn as_str_returns_canonical_lowercase() {
        assert_eq!(EntityType::Person.as_str(), "person");
        assert_eq!(EntityType::IssueTracker.as_str(), "issue_tracker");
    }

    #[test]
    fn serde_json_serializes_as_lowercase_string() {
        let json = serde_json::to_string(&EntityType::Person).unwrap();
        assert_eq!(json, "\"person\"");
        let json = serde_json::to_string(&EntityType::IssueTracker).unwrap();
        assert_eq!(json, "\"issue_tracker\"");
    }

    #[test]
    fn serde_json_deserializes_from_lowercase_string() {
        let et: EntityType = serde_json::from_str("\"person\"").unwrap();
        assert_eq!(et, EntityType::Person);
    }

    #[test]
    fn map_to_canonical_preserves_canonical_types() {
        assert_eq!(EntityType::map_to_canonical("person"), EntityType::Person);
        assert_eq!(EntityType::map_to_canonical("concept"), EntityType::Concept);
        assert_eq!(
            EntityType::map_to_canonical("issue_tracker"),
            EntityType::IssueTracker
        );
        // Hyphen + case variants normalize to the canonical kind.
        assert_eq!(
            EntityType::map_to_canonical("Issue-Tracker"),
            EntityType::IssueTracker
        );
    }

    #[test]
    fn map_to_canonical_folds_non_canonical_instead_of_discarding() {
        // GAP-SG-47: platform/language/feature were previously DISCARDED.
        assert_eq!(
            EntityType::map_to_canonical("platform"),
            EntityType::Concept
        );
        assert_eq!(
            EntityType::map_to_canonical("language"),
            EntityType::Concept
        );
        assert_eq!(EntityType::map_to_canonical("feature"), EntityType::Concept);
        // Role/collective folds.
        assert_eq!(
            EntityType::map_to_canonical("developer"),
            EntityType::Person
        );
        assert_eq!(
            EntityType::map_to_canonical("company"),
            EntityType::Organization
        );
        assert_eq!(EntityType::map_to_canonical("document"), EntityType::File);
    }

    #[test]
    fn map_to_canonical_unknown_falls_back_to_concept_never_dropped() {
        assert_eq!(
            EntityType::map_to_canonical("totally-made-up-kind"),
            EntityType::Concept
        );
        assert_eq!(EntityType::map_to_canonical(""), EntityType::Concept);
    }
}
