//! Operations that reshape the entity GRAPH (GAP-SG-146).
//!
//! Edge weights, relation labels, new edges between isolated entities, entity
//! type validation, domain classification and graph audit. They all read and
//! write `entities`/`relationships` rather than a memory body.

use super::*;
use crate::errors::AppError;
use rusqlite::Connection;
use std::path::Path;
pub(crate) fn call_weight_calibrate(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    _binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let rel_id: i64 = item_key.parse().map_err(|_| {
        AppError::Validation(crate::i18n::validation::invalid_relationship_id(item_key))
    })?;
    // GAP-SG-279 (class): the two entity DESCRIPTIONS ride along on the join
    // that already runs, at no extra query. Without them the model was asked to
    // weigh an edge between `rd_gs` and `v017` knowing only how those two
    // strings are spelled — the same defect `entity-type-validate` carried, in
    // an operation that also writes its answer straight to the column.
    let (source_name, source_desc, target_name, target_desc, relation, current_weight): (
        String,
        Option<String>,
        String,
        Option<String>,
        String,
        f64,
    ) = conn
        .query_row(
            "SELECT e1.name, e1.description, e2.name, e2.description, r.relation, r.weight \
             FROM relationships r \
             JOIN entities e1 ON e1.id = r.source_id \
             JOIN entities e2 ON e2.id = r.target_id \
             WHERE r.id = ?1",
            rusqlite::params![rel_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .map_err(|_| {
            AppError::NotFound(crate::i18n::validation::relationship_id_not_found(rel_id))
        })?;

    let input_text = format!(
        "{}Relation: {relation}\nCurrent weight: {current_weight}",
        super::prompts::edge_endpoints_section(
            &source_name,
            source_desc.as_deref(),
            &target_name,
            target_desc.as_deref(),
        )
    );
    let (value, cost, is_oauth) = match mode {
        EnrichMode::OpenRouter => call_openrouter(
            WEIGHT_CALIBRATE_PROMPT,
            WEIGHT_CALIBRATE_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };

    let calibrated = value
        .get("calibrated_weight")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| {
            AppError::Validation(crate::i18n::validation::llm_missing_calibrated_weight())
        })?;

    conn.execute(
        "UPDATE relationships SET weight = ?1 WHERE id = ?2",
        rusqlite::params![calibrated, rel_id],
    )?;

    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: None,
        entities: 0,
        rels: 1,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// G27: Reclassify a generic relationship type via LLM.
pub(crate) fn call_relation_reclassify(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    _binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let rel_id: i64 = item_key.parse().map_err(|_| {
        AppError::Validation(crate::i18n::validation::invalid_relationship_id(item_key))
    })?;
    // GAP-SG-279 (class): same join, same cost, two descriptions more. Choosing
    // between `uses` and `depends-on` for a pair of entities the model has
    // never been told anything about is a coin toss that lands in a column.
    let (source_name, source_desc, target_name, target_desc, current_relation): (
        String,
        Option<String>,
        String,
        Option<String>,
        String,
    ) = conn
        .query_row(
            "SELECT e1.name, e1.description, e2.name, e2.description, r.relation \
             FROM relationships r \
             JOIN entities e1 ON e1.id = r.source_id \
             JOIN entities e2 ON e2.id = r.target_id \
             WHERE r.id = ?1",
            rusqlite::params![rel_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|_| {
            AppError::NotFound(crate::i18n::validation::relationship_id_not_found(rel_id))
        })?;

    let input_text = format!(
        "{}Current relation: {current_relation}",
        super::prompts::edge_endpoints_section(
            &source_name,
            source_desc.as_deref(),
            &target_name,
            target_desc.as_deref(),
        )
    );
    let (value, cost, is_oauth) = match mode {
        EnrichMode::OpenRouter => call_openrouter(
            RELATION_RECLASSIFY_PROMPT,
            RELATION_RECLASSIFY_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };

    let new_relation = value
        .get("relation")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Validation(crate::i18n::validation::llm_missing_relation()))?;
    let new_strength = value
        .get("strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);

    conn.execute(
        "UPDATE relationships SET relation = ?1, weight = ?2 WHERE id = ?3",
        rusqlite::params![new_relation, new_strength, rel_id],
    )?;

    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: None,
        entities: 0,
        rels: 1,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// G27 P2: Connect isolated entities via LLM-suggested relationship.
///
/// v1.1.06 (GAP-ENTITY-CONNECT-SCAN-CARTESIAN): `item_key` is
/// `pair:{id1}:{id2}` from the O(k) scan. Resolve both entities by primary key
/// — **never** re-run `scan_isolated_entity_pairs` (the old path re-executed
/// the cartesian SQL on every drain item and re-hung large namespaces).
pub(crate) fn call_entity_connect(
    conn: &Connection,
    namespace: &str,
    item_key: &str,
    _binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (e1_id, e2_id) = match super::scan::parse_pair_key(item_key) {
        Some(ids) => ids,
        None => {
            return Ok(EnrichItemResult::Skipped {
                cost: 0.0,
                reason: format!(
                    "legacy or invalid entity-connect key '{item_key}' \
                     (expected pair:id1:id2); re-scan to enqueue stable pair keys"
                ),
            });
        }
    };

    let load = |id: i64| -> Result<Option<(i64, String)>, AppError> {
        match conn.query_row(
            "SELECT id, name FROM entities WHERE id = ?1 AND namespace = ?2",
            rusqlite::params![id, namespace],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ) {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::Database(e)),
        }
    };
    let (e1_id, e1_name) = match load(e1_id)? {
        Some(v) => v,
        None => {
            return Ok(EnrichItemResult::Skipped {
                cost: 0.0,
                reason: format!("entity id {e1_id} missing in namespace '{namespace}'"),
            });
        }
    };
    let (e2_id, e2_name) = match load(e2_id)? {
        Some(v) => v,
        None => {
            return Ok(EnrichItemResult::Skipped {
                cost: 0.0,
                reason: format!("entity id {e2_id} missing in namespace '{namespace}'"),
            });
        }
    };

    // Skip if already evaluated or already related (queue may be stale).
    let already_seen: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM entity_connect_seen \
         WHERE source_id = ?1 AND target_id = ?2)",
        rusqlite::params![e1_id, e2_id],
        |r| r.get(0),
    )?;
    if already_seen {
        return Ok(EnrichItemResult::Skipped {
            cost: 0.0,
            reason: crate::i18n::validation::pair_already_seen(),
        });
    }
    let already_related: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM relationships r WHERE \
           (r.source_id = ?1 AND r.target_id = ?2) OR \
           (r.source_id = ?2 AND r.target_id = ?1))",
        rusqlite::params![e1_id, e2_id],
        |r| r.get(0),
    )?;
    if already_related {
        return Ok(EnrichItemResult::Skipped {
            cost: 0.0,
            reason: crate::i18n::validation::pair_already_related(),
        });
    }

    let input_text = format!("Entity A: {e1_name}\nEntity B: {e2_name}");
    let (value, cost, is_oauth) = match mode {
        EnrichMode::OpenRouter => call_openrouter(
            ENTITY_CONNECT_PROMPT,
            ENTITY_CONNECT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let relation = value
        .get("relation")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    if relation == "none" {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO entity_connect_seen (source_id, target_id, namespace, verdict, relation) \
             VALUES (?1, ?2, ?3, 'none', NULL)",
            rusqlite::params![e1_id, e2_id, namespace],
        );
        return Ok(EnrichItemResult::Skipped {
            cost: 0.0,
            reason: crate::i18n::validation::llm_found_no_relationship(),
        });
    }
    let strength = value
        .get("strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    conn.execute(
        "INSERT OR IGNORE INTO relationships (namespace, source_id, target_id, relation, weight) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![namespace, e1_id, e2_id, relation, strength],
    )?;
    let _ = conn.execute(
        "INSERT OR REPLACE INTO entity_connect_seen (source_id, target_id, namespace, verdict, relation) \
         VALUES (?1, ?2, ?3, 'related', ?4)",
        rusqlite::params![e1_id, e2_id, namespace, relation],
    );
    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: None,
        entities: 0,
        rels: 1,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// G27 P2: Validate entity type assignment via LLM.
///
/// The `UPDATE entities SET type` below is the only write in the crate that
/// does not go through `upsert_entity`, so it never saw the shape normalisation
/// every other path applies. While the vocabulary was closed the omission was
/// invisible — the SQL `CHECK` refused anything unknown — but V017 removed that
/// CHECK, which left this the one route by which a raw model string could reach
/// the column: `"Issue Tracker"`, a label with a trailing newline, or a
/// paragraph of prose would all have landed verbatim and split the entity from
/// every row spelled the normal way. The label therefore passes through
/// [`normalize_entity_type`] here, which enforces shape and never membership.
///
/// A label that fails normalisation keeps the entity's CURRENT type rather than
/// failing the item: this operation exists to improve a type, and turning one
/// unusable suggestion into a failed item would have the queue retry it and
/// eventually mark it dead, trading a harmless no-op for a permanent failure.
pub(crate) fn call_entity_type_validate(
    conn: &Connection,
    namespace: &str,
    item_key: &str,
    _binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (ent_id, ent_name, ent_type, ent_description): (i64, String, String, Option<String>) = conn
        .query_row(
            "SELECT id, name, type, description FROM entities \
             WHERE namespace = ?1 AND name = ?2",
            rusqlite::params![namespace, item_key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::EntityNotYetMaterialized {
                name: item_key.to_string(),
                namespace: namespace.to_string(),
            },
            other => AppError::Database(other),
        })?;

    // GAP-SG-279: gather the evidence BEFORE deciding whether to spend a token.
    //
    // `load_entity_evidence` is the same single source of truth the description
    // path and the `--status` sampler already read from, so the three agree on
    // what "what we know about this entity" means. Reusing it also means the
    // four tuning keys added for this operation behave exactly like the four
    // the description path has had all along.
    let evidence =
        super::descriptions::load_entity_evidence_tuned(conn, ent_id, ENTITY_TYPE_VALIDATE_TUNING)?;
    let description = ent_description.as_deref().map(str::trim).unwrap_or("");
    let evidence_chars = evidence.trim().chars().count();

    let min_corpus_chars = crate::runtime_config::resolve_usize(
        None,
        "enrich.entity_type_validate.min_corpus_chars",
        crate::preservation::DEFAULT_GROUNDING_MIN_CORPUS_CHARS,
    );
    if should_abstain_from_type_judgement(description, &evidence, min_corpus_chars) {
        return Ok(EnrichItemResult::Skipped {
            cost: 0.0,
            reason: crate::i18n::validation::entity_type_no_evidence(
                evidence_chars,
                min_corpus_chars,
            ),
        });
    }

    let input_text = super::prompts::entity_type_validate_user_text(
        &ent_name,
        &ent_type,
        ent_description.as_deref(),
        &evidence,
    );
    let (value, cost, is_oauth) = match mode {
        EnrichMode::OpenRouter => call_openrouter(
            ENTITY_TYPE_VALIDATE_PROMPT,
            ENTITY_TYPE_VALIDATE_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };

    // The schema carries an abstention channel now; honour it before anything
    // else. A model that read the evidence and declined is doing exactly what
    // it was asked, so this is `Skipped` — billed, because the completion was
    // produced and charged — and never an error.
    if let Some(false) = value.get("sufficient_evidence").and_then(|v| v.as_bool()) {
        return Ok(EnrichItemResult::Skipped {
            cost,
            reason: crate::i18n::validation::entity_type_model_abstained(evidence_chars),
        });
    }

    let was_correct = value
        .get("was_correct")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    if was_correct {
        return Ok(EnrichItemResult::Skipped {
            cost,
            reason: crate::i18n::validation::entity_type_confirmed(&ent_type),
        });
    }

    // An explicit null is the abstention path, not a malformed reply: the
    // schema admits it precisely so the model has somewhere to put "I cannot
    // tell" other than a plausible-looking label.
    let suggested = match value.get("validated_type") {
        Some(v) if v.is_null() => {
            return Ok(EnrichItemResult::Skipped {
                cost,
                reason: crate::i18n::validation::entity_type_model_abstained(evidence_chars),
            })
        }
        Some(v) => v.as_str().unwrap_or_default().to_string(),
        None => String::new(),
    };

    let normalized = match crate::entity_type::normalize_entity_type(&suggested) {
        Ok(normalized) => normalized,
        Err(e) => {
            // A label that fails normalisation keeps the entity's CURRENT type
            // rather than failing the item: this operation exists to improve a
            // type, and turning one unusable suggestion into a failed item
            // would have the queue retry it and eventually mark it dead,
            // trading a harmless no-op for a permanent failure.
            tracing::warn!(
                target: "enrich",
                entity = %ent_name,
                suggested_type = %suggested,
                current_type = %ent_type,
                error = %e,
                "suggested entity type is unusable; keeping the current type"
            );
            return Ok(EnrichItemResult::Skipped {
                cost,
                reason: crate::i18n::validation::entity_type_suggestion_unusable(&suggested),
            });
        }
    };

    // A model may answer `was_correct: false` and then hand back the label the
    // row already holds. Writing it would burn an UPDATE and report a change
    // that never happened.
    if normalized == ent_type {
        return Ok(EnrichItemResult::Skipped {
            cost,
            reason: crate::i18n::validation::entity_type_confirmed(&ent_type),
        });
    }

    if !crate::entity_type::is_canonical_entity_type(&normalized) {
        tracing::warn!(
            target: "enrich",
            entity = %ent_name,
            entity_type = %normalized,
            "validated entity type is outside the canonical vocabulary"
        );
    }

    // GAP-SG-283: the vocabulary policy runs HERE, between the model's verdict
    // and the column. Anywhere later would be a policy that reports on a write
    // it did not gate, which is what `--strict-entity-types` exists not to be.
    let signals = crate::commands::enrich::events::count_type_signals(description, &evidence);
    let outcome = crate::commands::enrich::events::apply_entity_type_policy(&normalized);
    let written = match &outcome {
        crate::commands::enrich::events::PolicyOutcome::Accept(label) => label.clone(),
        crate::commands::enrich::events::PolicyOutcome::Fallback { applied, raw } => {
            // The inverse of this rewrite is declared, not implied: the raw
            // label travels into the description, so `enrich --operation
            // entity-type-validate --allowed-types <raw>` (or a manual `edit`)
            // can restore it without a database backup.
            let note = crate::commands::enrich::events::raw_label_note(raw, applied);
            conn.execute(
                "UPDATE entities SET description = \
                 TRIM(COALESCE(description, '') || char(10) || ?1) WHERE id = ?2",
                rusqlite::params![note, ent_id],
            )?;
            applied.clone()
        }
        crate::commands::enrich::events::PolicyOutcome::Refuse(message) => {
            crate::commands::enrich::events::emit_policy_event(
                &ent_name,
                &normalized,
                None,
                signals,
                evidence_chars,
            );
            return Err(AppError::Validation(message.clone()));
        }
    };
    crate::commands::enrich::events::emit_policy_event(
        &ent_name,
        &normalized,
        Some(&written),
        signals,
        evidence_chars,
    );

    conn.execute(
        "UPDATE entities SET type = ?1, updated_at = unixepoch() WHERE id = ?2",
        rusqlite::params![written, ent_id],
    )?;

    Ok(EnrichItemResult::Retyped {
        entity_id: ent_id,
        previous_type: ent_type,
        validated_type: written,
        evidence_chars,
        cost,
        is_oauth,
    })
}

/// Whether an entity carries too little to judge its type from (GAP-SG-279).
///
/// Absence of evidence is a reason to ABSTAIN, not a licence to guess. This
/// runs BEFORE the request for the same reason the description path's gate
/// does: an item refused here costs nothing, while the previous behaviour paid
/// for a completion whose only possible content was a guess from the spelling
/// of a name — and then wrote that guess to the type column.
///
/// A description alone is thin but it is genuine evidence about the subject, so
/// it passes on its own. Only an entity with neither a description nor enough
/// linked corpus is refused, because for that entity every possible answer is
/// derived from the name.
///
/// Extracted as a free function so the decision can be tested without a network
/// call. The judgement of when NOT to spend money is not something to leave
/// exercised only by a live drain.
fn should_abstain_from_type_judgement(
    description: &str,
    evidence: &str,
    min_corpus_chars: usize,
) -> bool {
    description.trim().is_empty()
        && !crate::preservation::corpus_is_sufficient(evidence, min_corpus_chars)
}

/// Tuning keys and compiled defaults for the evidence `entity-type-validate`
/// gathers (GAP-SG-279).
///
/// Named separately from the description path's so the two operations can be
/// tuned apart. They start at the same values because the evidence they need is
/// the same evidence; what differs is that one writes a sentence and the other
/// writes a label, and an operator may well want to pay for more context before
/// rewriting ten thousand labels than before writing one description.
const ENTITY_TYPE_VALIDATE_TUNING: super::descriptions::EvidenceTuning =
    super::descriptions::EvidenceTuning {
        corpus_top_k_key: "enrich.entity_type_validate.corpus_top_k",
        snippet_chars_key: "enrich.entity_type_validate.snippet_chars",
        neighbour_top_k_key: "enrich.entity_type_validate.neighbour_top_k",
    };

/// G27 P2: Classify memory into domain category via LLM.
pub(crate) fn call_domain_classify(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    _binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (mem_id, body, desc): (i64, String, String) = conn
        .query_row(
            "SELECT id, body, description FROM memories WHERE name = ?1 AND deleted_at IS NULL",
            rusqlite::params![item_key],
            |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
        )
        .map_err(|_| {
            AppError::NotFound(crate::i18n::validation::memory_named_not_found(item_key))
        })?;
    let snippet: String = body
        .chars()
        .take(crate::constants::ENRICH_BODY_PREVIEW_CHARS)
        .collect();
    let input_text = format!("Memory: {item_key}\nDescription: {desc}\nBody preview: {snippet}");
    let (value, cost, is_oauth) = match mode {
        EnrichMode::OpenRouter => call_openrouter(
            DOMAIN_CLASSIFY_PROMPT,
            DOMAIN_CLASSIFY_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let domain = value
        .get("domain")
        .and_then(|v| v.as_str())
        .unwrap_or("uncategorized");
    let metadata = format!(r#"{{"domain":"{}"}}"#, domain.replace('"', "\\\""));
    conn.execute(
        "UPDATE memories SET metadata = ?1 WHERE id = ?2",
        rusqlite::params![metadata, mem_id],
    )?;
    Ok(EnrichItemResult::Done {
        memory_id: Some(mem_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

/// G27 P2: Audit memory graph quality via LLM.
pub(crate) fn call_graph_audit(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    _binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (mem_id, body, desc): (i64, String, String) = conn
        .query_row(
            "SELECT id, body, description FROM memories WHERE name = ?1 AND deleted_at IS NULL",
            rusqlite::params![item_key],
            |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
        )
        .map_err(|_| {
            AppError::NotFound(crate::i18n::validation::memory_named_not_found(item_key))
        })?;
    let snippet: String = body
        .chars()
        .take(crate::constants::ENRICH_BODY_PREVIEW_CHARS)
        .collect();
    let ent_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memory_entities WHERE memory_id = ?1",
            rusqlite::params![mem_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let input_text = format!("Memory: {item_key}\nDescription: {desc}\nEntity bindings: {ent_count}\nBody preview: {snippet}");
    let (value, cost, is_oauth) = match mode {
        EnrichMode::OpenRouter => call_openrouter(
            GRAPH_AUDIT_PROMPT,
            GRAPH_AUDIT_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let issues = value
        .get("issues")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    Ok(EnrichItemResult::Done {
        memory_id: Some(mem_id),
        entity_id: None,
        entities: 0,
        rels: issues,
        chars_before: None,
        chars_after: None,
        cost,
        is_oauth,
    })
}

#[cfg(test)]
mod entity_type_evidence_tests {
    use super::should_abstain_from_type_judgement;

    /// The shape GAP-SG-279 was opened for: nothing but a name.
    ///
    /// Before the fix this entity produced a paid completion whose only
    /// possible basis was how `rd_gs` is spelled, and the answer landed in
    /// `UPDATE entities SET type`.
    #[test]
    fn an_entity_with_neither_description_nor_corpus_is_refused() {
        assert!(should_abstain_from_type_judgement("", "", 40));
        assert!(should_abstain_from_type_judgement("   ", "  \n ", 40));
    }

    /// A description alone is thin, but it is a statement ABOUT the subject
    /// rather than a reading of its name, so it is enough to proceed.
    #[test]
    fn a_description_alone_is_enough_to_proceed() {
        assert!(!should_abstain_from_type_judgement(
            "prefix of the generated folders",
            "",
            40
        ));
    }

    /// Corpus alone is likewise enough: the entity appears in text someone
    /// wrote, which is the evidence the operation was always supposed to read.
    #[test]
    fn corpus_alone_is_enough_to_proceed() {
        let corpus = "Linked memory bodies:\nrd_gs marks the tree generated by the exporter";
        assert!(!should_abstain_from_type_judgement("", corpus, 40));
    }

    /// The threshold has to bite, or the gate is decoration.
    ///
    /// A corpus of two words is not evidence just because it is non-empty; the
    /// operator sets where that line falls through
    /// `enrich.entity_type_validate.min_corpus_chars`.
    #[test]
    fn a_corpus_below_the_threshold_does_not_count_as_evidence() {
        assert!(should_abstain_from_type_judgement("", "two words", 40));
        assert!(!should_abstain_from_type_judgement("", "two words", 4));
    }
}
