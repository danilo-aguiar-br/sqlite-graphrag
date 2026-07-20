//! JSON schemas and LLM prompt constants for enrich operations.
//! Extracted from mod.rs (Wave C1) so extraction helpers can share them.

// ---------------------------------------------------------------------------
// JSON schema used for memory-bindings and body-enrich extraction
// ---------------------------------------------------------------------------

pub(crate) const BINDINGS_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
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
  "required": ["entities","relationships"],
  "additionalProperties": false
}"#;

pub(crate) const ENTITY_DESCRIPTION_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "description": { "type": "string" }
  },
  "required": ["description"],
  "additionalProperties": false
}"#;

pub(crate) const BODY_ENRICH_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "enriched_body": { "type": "string" }
  },
  "required": ["enriched_body"],
  "additionalProperties": false
}"#;

// G27 P1: weight-calibrate
pub(crate) const WEIGHT_CALIBRATE_PROMPT: &str = "You are a knowledge graph quality auditor. Evaluate whether this relationship weight is correctly calibrated.\n\n\
Scale:\n\
- 0.9 = vital hard dependency (A cannot function without B)\n\
- 0.7 = important design relationship (A strongly supports/enables B)\n\
- 0.5 = useful contextual link (A and B share relevant context)\n\
- 0.3 = weak reference (A mentions B without strong coupling)\n\n\
Respond with the calibrated weight and brief reasoning.";

pub(crate) const WEIGHT_CALIBRATE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "calibrated_weight": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
    "reasoning": { "type": "string" }
  },
  "required": ["calibrated_weight", "reasoning"],
  "additionalProperties": false
}"#;

// G27 P1: relation-reclassify
pub(crate) const RELATION_RECLASSIFY_PROMPT: &str = "You are a knowledge graph quality auditor. The relationship between these entities uses a generic type. Determine the REAL semantic relationship.\n\n\
Valid canonical relations (pick exactly one):\n\
- depends-on: A cannot function without B\n\
- uses: A utilizes B but could substitute it\n\
- supports: A reinforces or enables B\n\
- causes: A triggers or produces B\n\
- fixes: A resolves a problem in B\n\
- contradicts: A conflicts with or invalidates B\n\
- applies-to: A is relevant to or scoped within B\n\
- follows: A comes after B in sequence\n\
- replaces: A substitutes B\n\
- tracked-in: A is monitored in B\n\
- related: A and B share context (use sparingly)\n\n\
Respond with the correct relation, strength, and reasoning.";

pub(crate) const RELATION_RECLASSIFY_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "relation": { "type": "string" },
    "strength": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
    "reasoning": { "type": "string" }
  },
  "required": ["relation", "strength", "reasoning"],
  "additionalProperties": false
}"#;

// G27 P2: entity-connect — suggest relationships between isolated entities
pub(crate) const ENTITY_CONNECT_PROMPT: &str = "You are a knowledge graph quality auditor. Two entities exist in the same graph but have no relationship between them. Determine if a meaningful relationship exists.\n\n\
Valid canonical relations: depends-on, uses, supports, causes, fixes, contradicts, applies-to, follows, replaces, tracked-in, related.\n\n\
If NO meaningful relationship exists, set relation to \"none\".\n\
Respond with the relation (or \"none\"), strength, and reasoning.";

pub(crate) const ENTITY_CONNECT_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "relation": { "type": "string" },
    "strength": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
    "reasoning": { "type": "string" }
  },
  "required": ["relation", "strength", "reasoning"],
  "additionalProperties": false
}"#;

// G27 P2: entity-type-validate — verify entity type assignments
pub(crate) const ENTITY_TYPE_VALIDATE_PROMPT: &str = "You are a knowledge graph quality auditor. Verify whether this entity's type is correct.\n\n\
Valid entity types: project, tool, person, file, concept, incident, decision, organization, location, date.\n\n\
If the current type is correct, keep it. If wrong, suggest the correct type.\n\
Respond with the validated type and reasoning.";

pub(crate) const ENTITY_TYPE_VALIDATE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "validated_type": { "type": "string" },
    "was_correct": { "type": "boolean" },
    "reasoning": { "type": "string" }
  },
  "required": ["validated_type", "was_correct", "reasoning"],
  "additionalProperties": false
}"#;

// G27 P2: description-enrich — improve generic memory descriptions
pub(crate) const DESCRIPTION_ENRICH_PROMPT: &str = "You are a knowledge graph quality auditor. This memory has a generic or auto-generated description. Write a concise, semantic description (10-20 words) that captures WHAT this memory is about and WHY it matters.\n\n\
BAD: 'ingested from docs/auth.md'\n\
GOOD: 'JWT token rotation strategy with 15-min expiry and refresh flow'\n\n\
Respond with the improved description and reasoning.";

pub(crate) const DESCRIPTION_ENRICH_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "description": { "type": "string" },
    "reasoning": { "type": "string" }
  },
  "required": ["description", "reasoning"],
  "additionalProperties": false
}"#;

// G27 P2: domain-classify — classify memory into domain category
pub(crate) const DOMAIN_CLASSIFY_PROMPT: &str = "You are a knowledge graph quality auditor. Classify this memory into its primary domain category.\n\n\
Respond with the domain name (kebab-case, 2-4 words) and reasoning.";

pub(crate) const DOMAIN_CLASSIFY_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "domain": { "type": "string" },
    "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
    "reasoning": { "type": "string" }
  },
  "required": ["domain", "confidence", "reasoning"],
  "additionalProperties": false
}"#;

// G27 P2: graph-audit — audit graph for quality issues
pub(crate) const GRAPH_AUDIT_PROMPT: &str = "You are a knowledge graph quality auditor. Analyze this memory and its entity bindings for quality issues.\n\n\
Check for: missing entities, wrong entity types, redundant relationships, orphaned entities, generic descriptions, low-signal relationships.\n\n\
Respond with a list of issues found (or empty if none) and an overall quality score.";

pub(crate) const GRAPH_AUDIT_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "quality_score": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
    "issues": { "type": "array", "items": { "type": "object", "properties": { "kind": { "type": "string" }, "detail": { "type": "string" } }, "required": ["kind", "detail"] } },
    "reasoning": { "type": "string" }
  },
  "required": ["quality_score", "issues", "reasoning"],
  "additionalProperties": false
}"#;

// G27 P2: deep-research-synth — synthesize research findings into graph
pub(crate) const DEEP_RESEARCH_SYNTH_PROMPT: &str = "You are a knowledge graph synthesizer. Given this memory body, extract key findings and synthesize them into structured entities and relationships.\n\n\
Entity names: lowercase kebab-case, domain-specific.\n\
Relations: depends-on, uses, supports, causes, fixes, contradicts, applies-to, follows, related, replaces, tracked-in.\n\n\
Respond with extracted entities, relationships, and a synthesis summary.";

pub(crate) const DEEP_RESEARCH_SYNTH_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "entities": { "type": "array", "items": { "type": "object", "properties": { "name": { "type": "string" }, "entity_type": { "type": "string" } }, "required": ["name", "entity_type"] } },
    "relationships": { "type": "array", "items": { "type": "object", "properties": { "source": { "type": "string" }, "target": { "type": "string" }, "relation": { "type": "string" }, "strength": { "type": "number" } }, "required": ["source", "target", "relation", "strength"] } },
    "summary": { "type": "string" }
  },
  "required": ["entities", "relationships", "summary"],
  "additionalProperties": false
}"#;

// G27 P2: body-extract — extract structured content from unstructured text
pub(crate) const BODY_EXTRACT_PROMPT: &str = "You are a structured data extractor. Given this memory body (which may be unstructured text, raw notes, or a transcript), extract and restructure the content into a clean, well-organized markdown body.\n\n\
Preserve all factual content. Remove noise, fix formatting, add section headers where appropriate.\n\
Respond with the restructured body and a brief summary of changes.";

pub(crate) const BODY_EXTRACT_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "restructured_body": { "type": "string" },
    "changes_summary": { "type": "string" }
  },
  "required": ["restructured_body", "changes_summary"],
  "additionalProperties": false
}"#;

// ---------------------------------------------------------------------------
// Prompts
// ---------------------------------------------------------------------------

pub(crate) const BINDINGS_PROMPT: &str = "You are a knowledge graph entity extractor. Given a memory body, extract:\n\
1. Domain-specific entities (concepts, tools, people, decisions, projects, files)\n\
2. Typed relationships between entities with strength scores\n\n\
Rules:\n\
- Entity names: lowercase kebab-case, 2+ chars, domain-specific only\n\
- NEVER extract generic terms, stop words, numbers, UUIDs, or single characters\n\
- Relationship types MUST be one of: applies-to, uses, depends-on, causes, fixes, contradicts, supports, follows, related, replaces, tracked-in\n\
- NEVER use 'mentions' as relationship type\n\
- Strength: 0.9 for hard dependencies, 0.7 for design relationships, 0.5 for contextual links, 0.3 for weak references\n\
- Prefer fewer high-quality entities over many low-quality ones";

pub(crate) const BODY_ENRICH_PROMPT_PREFIX: &str = "You are a knowledge assistant. Given a short or sparse memory body, expand it into a richer, more complete and useful description. Preserve all existing facts. Add context, implications, and relationships that would be valuable for knowledge retrieval.\n\nConstraints:\n- Output only the enriched body text (no metadata, no headers)\n- Preserve the original meaning exactly\n- Target length is provided in the system context\n\nMemory body to enrich:\n\n";
