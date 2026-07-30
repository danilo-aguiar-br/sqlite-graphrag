//! Types and extraction schema for claude-code ingest.

use serde::{Deserialize, Serialize};

pub(crate) const MIN_CLAUDE_VERSION: &str = "2.1.0";


pub(crate) const EXTRACTION_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "name": { "type": "string" },
    "description": { "type": "string" },
    "entities": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": { "type": "string" },
          "entity_type": {
            "type": "string",
            "enum": ["project","tool","person","file","concept","incident","decision","organization","location","date"]
          }
        },
        "required": ["name", "entity_type"],
        "additionalProperties": false
      }
    },
    "relationships": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "source": { "type": "string" },
          "target": { "type": "string" },
          "relation": {
            "type": "string",
            "enum": ["applies-to","uses","depends-on","causes","fixes","contradicts","supports","follows","related","replaces","tracked-in"]
          },
          "strength": { "type": "number", "minimum": 0, "maximum": 1 }
        },
        "required": ["source","target","relation","strength"],
        "additionalProperties": false
      }
    }
  },
  "required": ["name","description","entities","relationships"],
  "additionalProperties": false
}"#;

pub(crate) const EXTRACTION_PROMPT: &str = "You are a knowledge graph entity extractor. Given a document, extract:\n\
1. A short kebab-case name (max 60 chars) capturing the document's main topic\n\
2. A one-sentence description (10-20 words) summarizing the key insight\n\
3. Domain-specific entities (concepts, tools, people, decisions, projects, files)\n\
4. Typed relationships between entities with strength scores\n\n\
Rules:\n\
- Entity names: lowercase kebab-case, 2+ chars, domain-specific only\n\
- NEVER extract generic terms, stop words, numbers, UUIDs, or single characters\n\
- Relationship types MUST be one of: applies-to, uses, depends-on, causes, fixes, contradicts, supports, follows, related, replaces, tracked-in\n\
- NEVER use 'mentions' as relationship type\n\
- Strength: 0.9 for hard dependencies, 0.7 for design relationships, 0.5 for contextual links, 0.3 for weak references\n\
- Prefer fewer high-quality entities over many low-quality ones\n\
- Description must answer: What is this about and WHY does it matter?";

#[derive(Debug, Deserialize)]
pub(crate) struct ClaudeOutputElement {
    pub(crate) r#type: Option<String>,
    #[serde(default)]
    pub(crate) is_error: bool,
    pub(crate) result: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) terminal_reason: Option<String>,
}

/// Extraction result.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractionResult {
    /// Name of this item.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Extracted entities.
    pub entities: Vec<ExtractedEntity>,
    /// Relationships.
    pub relationships: Vec<ExtractedRelationship>,
}

/// Extracted entity.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractedEntity {
    /// Name of this item.
    pub name: String,
    /// Entity type label.
    pub entity_type: String,
}

/// Extracted relationship.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractedRelationship {
    /// Source side of the relationship.
    pub source: String,
    /// Target side of the relationship.
    pub target: String,
    /// Relationship type.
    pub relation: String,
    /// Strength.
    pub strength: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct PhaseEvent<'a> {
    pub(crate) phase: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) claude_path: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dir: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) files_total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) files_new: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) files_existing: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct FileEvent<'a> {
    pub(crate) file: &'a str,
    pub(crate) name: &'a str,
    pub(crate) status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entities: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rels: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<&'a str>,
    pub(crate) index: usize,
    pub(crate) total: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct Summary {
    pub(crate) summary: bool,
    pub(crate) files_total: usize,
    pub(crate) completed: usize,
    pub(crate) failed: usize,
    pub(crate) skipped: usize,
    pub(crate) entities_total: usize,
    pub(crate) rels_total: usize,
    pub(crate) cost_usd: f64,
    pub(crate) elapsed_ms: u64,
}