//! JSON schemas and LLM prompt constants for enrich operations.
//! Extracted from mod.rs (Wave C1) so extraction helpers can share them.

// ---------------------------------------------------------------------------
// JSON schema used for memory-bindings and body-enrich extraction
// ---------------------------------------------------------------------------

/// Extraction contract for `memory-bindings` and `body-extract`.
///
/// `entity_type` is a free-form string. It carried an `enum` of ten labels
/// while the vocabulary was closed, and that enum was never the closed
/// vocabulary: it omitted `memory`, `dashboard` and `issue_tracker`, so the
/// model was forbidden three kinds the database accepted. V017 opened the
/// column, which leaves the enum both incomplete and wrong in direction — it
/// would now be the last gate forcing the model to fold its own answer, on the
/// one path where the fold is invisible to the caller. [`BINDINGS_PROMPT`]
/// keeps steering the model toward the usual kinds, as guidance rather than as
/// a refusal.
///
/// The relation enum stays: `parsers::CANONICAL_RELATIONS` is a closed
/// vocabulary and `relation_vocabulary_contract` below pins it.
pub(crate) const BINDINGS_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "entities": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": { "type": "string" },
          "entity_type": { "type": "string" }
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

/// Entity-description contract WITH an abstention channel.
///
/// The previous shape required a non-nullable `description`, and the request
/// is sent with `strict: true` (`chat_api/client.rs`). Together they made
/// abstention structurally impossible: a model that knows nothing about a
/// bare personal name had no valid output other than to invent one. Measured
/// against `deepseek/deepseek-v4-flash`, the old shape yields vacuous filler
/// ("X is a person whose details are not specified") that the Rust
/// post-filter does not recognise, so it is persisted and freezes the entity
/// out of every later correction path.
///
/// `description` is nullable and `sufficient_evidence` is mandatory, so
/// "I cannot ground this" is a first-class, machine-readable answer. Both
/// keys stay in `required` and `additionalProperties` stays false because
/// OpenAI-style strict mode demands it.
pub(crate) const ENTITY_DESCRIPTION_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "sufficient_evidence": { "type": "boolean" },
    "description": { "type": ["string", "null"] }
  },
  "required": ["sufficient_evidence", "description"],
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
/// Prompt for `entity-type-validate`.
///
/// It used to list ten of the thirteen canonical kinds, so the model was told
/// `memory`, `dashboard` and `issue_tracker` were invalid and would helpfully
/// "correct" entities that already held them — on the one operation whose whole
/// purpose is rewriting the type column.
///
/// The wording now presents the canonical set as PREFERRED rather than
/// exhaustive, because V017 opened the column. A model that finds none of them
/// apt should answer with the domain's own word instead of folding onto the
/// nearest kind; the fold is exactly what made `concept` hold 69% of the graph.
///
/// The list stays a literal because a `const` cannot format and the constant is
/// re-exported by name from `extraction.rs`. Drift is not left to review:
/// `entity_type_vocabulary_contract` below fails the build if this text and
/// [`crate::entity_type::CANONICAL_ENTITY_TYPES`] ever disagree.
///
/// GAP-SG-279 fixed the other half of the pair. The vocabulary was open and the
/// wording was right, but the model received two lines — the entity's name and
/// the label being disputed — and nothing else. Judging a type from a name is
/// judging `rd_gs` or `v017` by how it looks, and the answer came back wearing
/// the confidence of an audit before landing in `UPDATE entities SET type`.
/// The grounding rules below say what the evidence rules of the description
/// prompt already said: a name is not evidence, and abstaining beats guessing.
pub(crate) const ENTITY_TYPE_VALIDATE_PROMPT: &str = "You are a knowledge graph quality auditor. Verify whether this entity's type is correct.\n\n\
Preferred entity types, used whenever one of them fits: concept, dashboard, date, decision, file, incident, issue_tracker, location, memory, organization, person, project, tool.\n\
They are preferred, not exhaustive. If none of them describes the entity, answer with a specific lowercase term from the entity's own domain rather than forcing the nearest preferred type.\n\n\
Type format: lowercase, words joined by underscores, never digits only, never longer than one short word or two.\n\n\
Grounding rules:\n\
- Judge the type ONLY from the evidence in the user message: the entity's description, the bodies of the memories it is linked to, and its typed neighbours in the graph.\n\
- The entity's NAME is not evidence. Never infer a kind from how a name is spelled, abbreviated or capitalised.\n\
- If the evidence does not say what the entity is, set `sufficient_evidence` to false and `validated_type` to null. Abstaining is the correct answer, never a failure.\n\
- Never fall back to `concept` because nothing else fits; that fold is what the open vocabulary exists to undo.\n\n\
If the current type is correct, keep it and set `was_correct` to true. If wrong, answer with the correct type.\n\
Respond with the validated type and the reasoning that supports it from the evidence.";

/// Response contract for `entity-type-validate`.
///
/// GAP-SG-279 added `sufficient_evidence` and widened `validated_type` to admit
/// null, giving the model somewhere to put "I cannot tell" other than a guess.
///
/// Both changes obey the transport, not taste. OpenRouter sends this under
/// `strict: true` (see `chat_api/client.rs`), and strict mode requires EVERY
/// key in `properties` to appear in `required` — an "optional" field is a
/// rejected request, not a lenient one. The abstention channel therefore has to
/// be a required boolean plus a nullable value, which is the same shape
/// [`ENTITY_DESCRIPTION_SCHEMA`] settled on for the same reason.
pub(crate) const ENTITY_TYPE_VALIDATE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "sufficient_evidence": { "type": "boolean" },
    "validated_type": { "type": ["string", "null"] },
    "was_correct": { "type": "boolean" },
    "reasoning": { "type": "string" }
  },
  "required": ["sufficient_evidence", "validated_type", "was_correct", "reasoning"],
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

/// The canonical relations this module deliberately withholds from the model.
///
/// `mentions` is canonical and storable, and the extraction prompts still tell
/// the model never to emit it (see `BODY_EXTRACT_PROMPT`), because a generic
/// "A mentions B" edge is what the graph produces when the model has nothing
/// specific to say — it was the top relation at 24.6% before this exclusion.
/// The withholding is a decision, so it is named here rather than left as an
/// absence for a future reader to mistake for an oversight.
///
/// Read only by the contract test below: its job is to make the narrowing
/// checkable, not to drive runtime behaviour, so `dead_code` outside `cfg(test)`
/// is the accurate state and not an oversight.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const RELATIONS_WITHHELD_FROM_MODEL: &[&str] = &["mentions"];

#[cfg(test)]
mod entity_type_vocabulary_contract {
    /// The prompt's preferred list must be exactly the canonical set.
    ///
    /// This gate is why the list may stay a literal. The previous literal named
    /// ten of the thirteen kinds, and nothing compared the two: the model was
    /// instructed that `memory`, `dashboard` and `issue_tracker` were wrong, on
    /// the one operation that rewrites `entities.type`. A missing name is now a
    /// failing test rather than a silently degraded prompt.
    #[test]
    fn preferred_list_matches_the_canonical_set() {
        let line = super::ENTITY_TYPE_VALIDATE_PROMPT
            .lines()
            .find(|l| l.starts_with("Preferred entity types"))
            .expect("the prompt must name its preferred vocabulary on one line");
        let listed: std::collections::BTreeSet<&str> = line
            .split_once(':')
            .expect("preferred list must follow a colon")
            .1
            .trim_end_matches('.')
            .split(',')
            .map(str::trim)
            .collect();
        let canonical: std::collections::BTreeSet<&str> =
            crate::entity_type::CANONICAL_ENTITY_TYPES
                .iter()
                .copied()
                .collect();
        assert_eq!(
            listed, canonical,
            "ENTITY_TYPE_VALIDATE_PROMPT and CANONICAL_ENTITY_TYPES disagree; a kind \
             missing from the prompt is a kind the model is told to 'correct' away"
        );
    }

    /// The vocabulary must be offered as open, not as a closed enumeration.
    ///
    /// V017 removed the SQL `CHECK`; a prompt that still says "valid types are
    /// X" reinstates the closed vocabulary at the only layer the caller cannot
    /// inspect, and `entity-type-validate` would fold labels the database is
    /// perfectly willing to hold.
    #[test]
    fn prompt_presents_the_vocabulary_as_open() {
        let prompt = super::ENTITY_TYPE_VALIDATE_PROMPT;
        assert!(
            prompt.contains("not exhaustive"),
            "the prompt must state that the preferred list is not exhaustive"
        );
        assert!(
            !prompt.contains("Valid entity types"),
            "'Valid entity types' reads as a closed set; the column has been open since V017"
        );
    }

    /// The extraction schema must not re-close the vocabulary with an enum.
    ///
    /// A JSON Schema `enum` is a harder gate than prose: the request is sent
    /// with `strict: true`, so an enum leaves the model no way to answer with a
    /// label outside it, and the fold happens inside the provider where nothing
    /// on this side can report it.
    #[test]
    fn bindings_schema_leaves_entity_type_free_form() {
        let entity_type_line = super::BINDINGS_SCHEMA
            .lines()
            .find(|l| l.contains("\"entity_type\""))
            .expect("BINDINGS_SCHEMA must declare entity_type");
        assert!(
            !entity_type_line.contains("\"enum\""),
            "entity_type carries a JSON Schema enum; that re-closes a vocabulary V017 opened"
        );
    }
}

#[cfg(test)]
mod relation_vocabulary_contract {
    /// Every relation token in this file must be spelled the way the crate
    /// stores it, and must name a canonical relation.
    ///
    /// This is the gate that was missing. The relation vocabulary lived in two
    /// hand-written lists — `parsers::CANONICAL_RELATIONS` in snake_case and
    /// the prompts here in kebab-case — with nothing comparing them. The model
    /// obeyed the prompt, `enrich::extraction_body` persisted its answer
    /// verbatim, and 67 651 edges landed in a spelling every read filter
    /// normalised away. Nothing failed; the rows simply became unreachable.
    ///
    /// The check scans the SOURCE TEXT rather than a list, so it covers the
    /// prose bullets (`- depends-on: A cannot function without B`) as well as
    /// the JSON Schema enum. Generating the enum from the constant would have
    /// protected one of the eight sites and left the other seven free to drift,
    /// which is the same split policy that caused the defect.
    #[test]
    fn every_relation_token_in_prompts_is_canonical_and_kebab() {
        // Scan the PROMPTS only, not this module. A gate that reads its own
        // source finds the spelling it exists to forbid and accuses itself —
        // and the forbidden forms are derived from the canonical list rather
        // than written out, so they never appear as literals here either.
        let full = include_str!("schemas.rs");
        let source = full
            .split_once("mod relation_vocabulary_contract")
            .map_or(full, |(before, _)| before);

        // Only multi-word relations can differ; single-word names are identical
        // in both conventions.
        for rel in crate::parsers::CANONICAL_RELATIONS {
            if !rel.contains('-') {
                continue;
            }
            let wrong = rel.replace('-', "_");
            assert!(
                !source.contains(&wrong),
                "schemas.rs spells '{wrong}' in snake_case; the crate stores \
                 '{rel}' (see parsers::CANONICAL_RELATIONS). A prompt that \
                 teaches the model the wrong spelling writes rows no read \
                 filter can reach."
            );
            assert!(
                source.contains(rel),
                "schemas.rs never mentions the canonical relation '{rel}'; \
                 either the prompts stopped offering it or the spelling drifted"
            );
        }
    }

    /// The prompts may narrow the vocabulary, and must do so ON PURPOSE.
    #[test]
    fn withheld_relations_are_canonical_and_declared() {
        let canonical: std::collections::BTreeSet<&str> = crate::parsers::CANONICAL_RELATIONS
            .iter()
            .copied()
            .collect();
        for withheld in super::RELATIONS_WITHHELD_FROM_MODEL {
            assert!(
                canonical.contains(withheld),
                "'{withheld}' is withheld from the model but is not a canonical \
                 relation; withholding a name that does not exist hides a typo"
            );
        }
        // The JSON Schema enum offers the canonical set minus the withheld set.
        let enum_line = super::BINDINGS_SCHEMA
            .lines()
            .find(|l| l.contains("\"enum\"") && l.contains("applies-to"))
            .expect("BINDINGS_SCHEMA must declare the relation enum");
        for rel in crate::parsers::CANONICAL_RELATIONS {
            let withheld = super::RELATIONS_WITHHELD_FROM_MODEL.contains(rel);
            let offered = enum_line.contains(&format!("\"{rel}\""));
            assert_eq!(
                offered,
                !withheld,
                "relation '{rel}' is {} in the model enum but {} in \
                 RELATIONS_WITHHELD_FROM_MODEL — the two must agree, or the \
                 model is silently offered a vocabulary nobody declared",
                if offered { "present" } else { "absent" },
                if withheld { "withheld" } else { "not withheld" }
            );
        }
    }
}
