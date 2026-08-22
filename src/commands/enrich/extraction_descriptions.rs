//! Description generation and enrichment (GAP-SG-146).
//!
//! Everything that writes a `description`, for an entity or for a memory:
//! the grounding corpus loader, its tuning constants, the entity-description
//! operation, and the memory-description enrichment that used to sit in a
//! different size-sliced file.

use super::postprocess::persist_entity_description;
use super::*;
use crate::errors::AppError;
use crate::storage::memories;
use rusqlite::Connection;
use std::path::Path;
/// Default top-K linked memory bodies injected into the ED prompt
/// (GAP-CLI-ED-02). Overridable via XDG `enrich.entity_description.corpus_top_k`.
///
/// Raised 5 → 8 (G-PR-7). The old budget of 5 × 400 chars is roughly 500
/// tokens against the 1M-token context window of the configured chat model:
/// the corpus was starved for no reason the transport imposes.
pub(crate) const ENTITY_DESCRIPTION_CORPUS_TOP_K: usize = 8;
/// Per-body character budget for corpus snippets (GAP-CLI-ED-02).
/// Overridable via XDG `enrich.entity_description.snippet_chars`.
///
/// Raised 400 → 2000 (G-PR-7). 400 characters truncates mid-sentence, so the
/// grounding gate scored descriptions against a fragment of the evidence that
/// actually supports them.
///
/// MEMORY COST MEASURED, because a 5× corpus multiplied by a 16-way fan-out is
/// exactly the shape that turns a tuning change into an OOM. `/usr/bin/time -v`
/// over a 400-entity sample, which walks the same corpus and trigram path the
/// drain does: 143 MiB peak RSS with sampling off, 173 MiB with it on, i.e.
/// **~75 KiB per entity** for corpus, graph context and the trigram set
/// together. At the `--rest-concurrency` ceiling of 16 that is ~1.2 MiB of
/// concurrent working set against a 140 MiB baseline.
///
/// Applying the sizing rule `min(cpus, free_ram / ram_per_task)` on the
/// measuring host — 72 CPUs, 91 GiB free — yields 72, so RAM is nowhere near
/// binding: the clamp of 16 is imposed by the provider's rate limit, not by
/// memory. Re-measure before raising this again.
pub(crate) const ENTITY_DESCRIPTION_SNIPPET_CHARS: usize = 2000;

/// Loads top-K linked memory body snippets for grounding (GAP-CLI-ED-02).
///
/// Shared by entity-description generation and status quality sampling (DRY).
pub(crate) fn load_entity_corpus_snippets(
    conn: &Connection,
    entity_id: i64,
    top_k: usize,
    max_chars: usize,
) -> Result<String, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT COALESCE(m.body, '') AS body
         FROM memory_entities me
         JOIN memories m ON m.id = me.memory_id
         WHERE me.entity_id = ?1 AND m.deleted_at IS NULL
         ORDER BY COALESCE(m.updated_at, m.created_at) DESC, m.id DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![entity_id, top_k as i64], |r| {
        r.get::<_, String>(0)
    })?;
    let mut snippets = Vec::with_capacity(top_k);
    for row in rows {
        let body = row.map_err(AppError::Database)?;
        let trimmed = body.trim();
        if trimmed.is_empty() {
            continue;
        }
        let snippet: String = trimmed.chars().take(max_chars).collect();
        snippets.push(snippet);
    }
    Ok(snippets.join("\n---\n"))
}

/// Default number of typed neighbours injected into the ED prompt (G-PR-7).
/// Overridable via XDG `enrich.entity_description.neighbour_top_k`.
pub(crate) const ENTITY_DESCRIPTION_NEIGHBOUR_TOP_K: usize = 12;

/// Loads the entity's strongest typed relations as prompt evidence (G-PR-7).
///
/// Memory bodies alone answer "where is this name mentioned"; the edges answer
/// "what is this thing to the others", which for a person is the whole of the
/// useful description. The prompt used to ignore the graph entirely, so an
/// entity could be a partner in a company, a parent and a guarantor — all
/// stated as edges — and the model would still see nothing but prose that
/// happens to contain the name.
///
/// Direction is preserved in the rendering: `A --relation--> B` and
/// `B --relation--> A` are different facts and must not be flattened.
pub(crate) fn load_entity_graph_context(
    conn: &Connection,
    entity_id: i64,
    top_k: usize,
) -> Result<String, AppError> {
    let mut stmt = conn.prepare_cached(
        "SELECT r.relation, r.weight, e.name, r.source_id = ?1 AS outgoing
         FROM relationships r
         JOIN entities e
           ON e.id = CASE WHEN r.source_id = ?1 THEN r.target_id ELSE r.source_id END
         WHERE r.source_id = ?1 OR r.target_id = ?1
         ORDER BY r.weight DESC, e.name ASC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![entity_id, top_k as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, f64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, bool>(3)?,
        ))
    })?;
    let mut lines = Vec::with_capacity(top_k);
    for row in rows {
        let (relation, weight, other, outgoing) = row.map_err(AppError::Database)?;
        lines.push(if outgoing {
            format!("- this entity --{relation}--> {other} (weight {weight:.2})")
        } else {
            format!("- {other} --{relation}--> this entity (weight {weight:.2})")
        });
    }
    Ok(lines.join("\n"))
}

/// XDG keys that tune how much evidence one operation gathers (GAP-SG-279).
///
/// The evidence itself is assembled the same way for every caller — that is
/// the point of `load_entity_evidence` being a single source of truth — but
/// WHICH keys set the budget differs per operation. Entity descriptions and
/// entity-type validation both need the same shape of evidence while having
/// every reason to buy different amounts of it: one writes a sentence, the
/// other rewrites a label across ten thousand rows.
pub(crate) struct EvidenceTuning {
    /// Key holding how many linked memory bodies to read.
    pub(crate) corpus_top_k_key: &'static str,
    /// Key holding the per-body character budget.
    pub(crate) snippet_chars_key: &'static str,
    /// Key holding how many typed neighbours to render.
    pub(crate) neighbour_top_k_key: &'static str,
}

/// The keys `entity-descriptions` has always read.
pub(crate) const ENTITY_DESCRIPTION_TUNING: EvidenceTuning = EvidenceTuning {
    corpus_top_k_key: "enrich.entity_description.corpus_top_k",
    snippet_chars_key: "enrich.entity_description.snippet_chars",
    neighbour_top_k_key: "enrich.entity_description.neighbour_top_k",
};

/// The complete evidence an entity description may be grounded on (G-PR-7).
///
/// SINGLE source of truth, shared by the write path and the `--status`
/// sampler. They must agree: when the sampler measured only bodies while the
/// writer also saw edges, the reported quality described a corpus that never
/// existed. Whatever is shown to the model is exactly what the grounding gate
/// scores against, and exactly what sufficiency is judged on.
pub(crate) fn load_entity_evidence(conn: &Connection, entity_id: i64) -> Result<String, AppError> {
    load_entity_evidence_tuned(conn, entity_id, ENTITY_DESCRIPTION_TUNING)
}

/// Same assembly as [`load_entity_evidence`], with the budget read from the
/// caller's keys instead of the entity-description ones (GAP-SG-279).
///
/// The split is between WHAT the evidence is and HOW MUCH of it to buy. Only
/// the second half is per-operation, so only the second half is parameterised;
/// duplicating the assembly for a second operation would let the two drift
/// apart, which is exactly the failure the single source of truth exists to
/// prevent. The compiled defaults stay the entity-description ones, so a
/// caller whose keys are unset gathers what this path has always gathered.
pub(crate) fn load_entity_evidence_tuned(
    conn: &Connection,
    entity_id: i64,
    tuning: EvidenceTuning,
) -> Result<String, AppError> {
    let top_k = crate::runtime_config::resolve_usize(
        None,
        tuning.corpus_top_k_key,
        ENTITY_DESCRIPTION_CORPUS_TOP_K,
    );
    let snippet_chars = crate::runtime_config::resolve_usize(
        None,
        tuning.snippet_chars_key,
        ENTITY_DESCRIPTION_SNIPPET_CHARS,
    );
    let neighbour_top_k = crate::runtime_config::resolve_usize(
        None,
        tuning.neighbour_top_k_key,
        ENTITY_DESCRIPTION_NEIGHBOUR_TOP_K,
    );

    let bodies = load_entity_corpus_snippets(conn, entity_id, top_k, snippet_chars)?;
    let edges = load_entity_graph_context(conn, entity_id, neighbour_top_k)?;

    let mut parts = Vec::with_capacity(2);
    if !bodies.trim().is_empty() {
        parts.push(format!("Linked memory bodies:\n{bodies}"));
    }
    if !edges.trim().is_empty() {
        parts.push(format!("Typed relations in the graph:\n{edges}"));
    }
    Ok(parts.join("\n\n"))
}

pub(crate) fn call_entity_description(
    conn: &Connection,
    namespace: &str,
    entity_name: &str,
    provider: ProviderCall<'_>,
    grounding_threshold: f64,
    domain_label: &str,
) -> Result<EnrichItemResult, AppError> {
    let (entity_id, entity_type, old_description): (i64, String, Option<String>) = conn
        .query_row(
            "SELECT id, type, description FROM entities WHERE namespace=?1 AND name=?2",
            rusqlite::params![namespace, entity_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::EntityNotYetMaterialized {
                name: entity_name.to_string(),
                namespace: namespace.to_string(),
            },
            other => AppError::Database(other),
        })?;
    // G-PR-5 / GAP-SG-96: chars_before is the pre-existing description length,
    // never a hard-coded zero on grounding failure.
    let chars_before_existing = old_description
        .as_deref()
        .map(|s| s.chars().count())
        .unwrap_or(0);

    let corpus = load_entity_evidence(conn, entity_id)?;

    // GAP-CLI-ED-03 / G-T-DRY-01 / G-PR-6: adaptive grounding. The value
    // arrives already resolved by `EnrichArgs::entity_description_grounding_threshold`
    // (flag > XDG > compiled default), so zero here means a deliberate zero.
    let threshold = grounding_threshold;
    let min_corpus_chars = crate::runtime_config::resolve_usize(
        None,
        "enrich.entity_description.min_corpus_chars",
        crate::preservation::DEFAULT_GROUNDING_MIN_CORPUS_CHARS,
    );

    // G-PR-7: absence of evidence is a reason to ABSTAIN, not a licence to
    // generate. This gate MUST run before the LLM call, for two reasons.
    //
    // First, `evaluate_grounding_adaptive` returns `Preserved { score: 1.0 }`
    // for an empty or sub-minimum corpus (preservation.rs), so after the call
    // the verdict can no longer tell "well grounded" from "no evidence at
    // all" — the entity with the LEAST support scores the HIGHEST.
    //
    // Second, an item skipped here costs zero tokens. The previous behaviour
    // paid for a completion whose only possible content was filler.
    let corpus_chars = corpus.trim().chars().count();
    if !crate::preservation::corpus_is_sufficient(&corpus, min_corpus_chars) {
        return Ok(EnrichItemResult::Skipped {
            cost: 0.0,
            reason: format!(
                "insufficient_grounding: linked corpus has {corpus_chars} chars, \
                 minimum is {min_corpus_chars} (consolidate the entity's variants \
                 or bind it to a memory before describing it)"
            ),
        });
    }

    let corpus_section = format!("Evidence (ground truth; use only these facts):\n{corpus}\n");

    let domain_section = super::prompts::entity_description_domain_section(domain_label);
    let user_text = super::prompts::entity_description_user_text(
        entity_name,
        &entity_type,
        &domain_section,
        &corpus_section,
    );

    let (value, mut cost, mut is_oauth) =
        invoke_entity_description_llm(provider, ENTITY_DESCRIPTION_SYSTEM_PROMPT, &user_text)?;

    // G-PR-7: the schema now carries an abstention channel. Honour it before
    // anything else — a model that reports insufficient evidence is doing
    // exactly what it was asked, so this is `Skipped`, never an error.
    if let Some(false) = value.get("sufficient_evidence").and_then(|v| v.as_bool()) {
        return Ok(EnrichItemResult::Skipped {
            cost,
            reason: format!(
                "insufficient_evidence: model declined to describe from {corpus_chars} chars \
                 of linked corpus"
            ),
        });
    }
    let mut description = match value.get("description") {
        // Explicit null is the abstention path, not a malformed reply.
        Some(v) if v.is_null() => {
            return Ok(EnrichItemResult::Skipped {
                cost,
                reason: crate::i18n::validation::description_returned_null(),
            })
        }
        Some(v) => v
            .as_str()
            .ok_or_else(|| {
                AppError::Validation(crate::i18n::validation::llm_missing_description_field())
            })?
            .to_string(),
        None => {
            return Err(AppError::Validation(
                crate::i18n::validation::llm_missing_description_field(),
            ))
        }
    };
    if description.trim().is_empty() {
        return Ok(EnrichItemResult::Skipped {
            cost,
            reason: crate::i18n::validation::description_returned_empty(),
        });
    }

    // G-PR-7: the "anti-jargon replace" escape hatch used to convert a
    // `Rejected` verdict into `Preserved` whenever the score cleared
    // `threshold * 0.25` — 0.03 under the default, i.e. 97% of the candidate's
    // trigrams absent from the evidence. It was removed: a replacement that
    // cannot be grounded is not an improvement over a bad description, it is
    // a second bad description with a fresh timestamp.
    let verdict = crate::preservation::PreservationVerdict::evaluate_grounding_adaptive(
        &description,
        &corpus,
        threshold,
        min_corpus_chars,
    );
    if !verdict.is_accepted() {
        let score = match verdict {
            crate::preservation::PreservationVerdict::Preserved { score, .. } => score,
            crate::preservation::PreservationVerdict::Rejected { score, .. } => score,
            crate::preservation::PreservationVerdict::Unchanged { .. } => 1.0,
        };
        return Ok(EnrichItemResult::PreservationFailed {
            score,
            threshold,
            chars_before: chars_before_existing,
            chars_after: description.chars().count(),
        });
    }

    // G-PR-2: post-filter quality — `done` requires !is_low_quality_description.
    if super::super::predicates::is_low_quality_description(&description) {
        let anti_jargon_user = format!(
            "{user_text}\n\nCRITICAL: your previous draft was rejected as generic filler. \
             Write a concrete domain description using only the evidence above, or set \
             `sufficient_evidence` to false if the evidence does not support one. \
             Forbidden: configuration file, software component, module that, system design, chatbot."
        );
        match invoke_entity_description_llm(
            provider,
            ENTITY_DESCRIPTION_SYSTEM_PROMPT,
            &anti_jargon_user,
        ) {
            Ok((value2, cost2, oauth2)) => {
                cost += cost2;
                is_oauth = is_oauth || oauth2;
                // A null description on the retry is abstention; leaving the
                // first draft in place lets the post-filter below reject it.
                if let Some(d2) = value2.get("description").and_then(|v| v.as_str()) {
                    let d2 = d2.to_string();
                    let v2 = crate::preservation::PreservationVerdict::evaluate_grounding_adaptive(
                        &d2,
                        &corpus,
                        threshold,
                        min_corpus_chars,
                    );
                    if v2.is_accepted()
                        && !super::super::predicates::is_low_quality_description(&d2)
                    {
                        description = d2;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "enrich",
                    error = %e,
                    "G-PR-2 anti-jargon retry failed; keeping first draft for quality gate"
                );
            }
        }
    }

    if super::super::predicates::is_low_quality_description(&description) {
        return Ok(EnrichItemResult::Skipped {
            cost,
            reason: format!(
                "quality_post_filter: description still matches low-quality predicate \
                 (orig={chars_before_existing} chars, candidate={} chars)",
                description.chars().count()
            ),
        });
    }

    persist_entity_description(conn, entity_id, &description)?;

    Ok(EnrichItemResult::Done {
        memory_id: None,
        entity_id: Some(entity_id),
        entities: 0,
        rels: 0,
        chars_before: Some(chars_before_existing),
        chars_after: Some(description.chars().count()),
        cost,
        is_oauth,
    })
}

/// Single LLM invocation for entity-description (DRY for G-PR-2 retry).
///
/// `system_prompt` carries policy; `user_text` carries the entity and its
/// evidence. Passing the whole thing as `system` with an empty `user_text`
/// makes `chat_api` emit a single-message request, which measurably degenerates.
fn invoke_entity_description_llm(
    provider: ProviderCall<'_>,
    system_prompt: &str,
    user_text: &str,
) -> Result<(serde_json::Value, f64, bool), AppError> {
    let ProviderCall {
        model,
        timeout,
        mode,
    } = provider;
    match mode {
        EnrichMode::OpenRouter => call_openrouter(
            system_prompt,
            ENTITY_DESCRIPTION_SCHEMA,
            user_text,
            model,
            timeout,
        ),
    }
}

/// G27 P2: Enrich generic memory description via LLM.
pub(crate) fn call_description_enrich(
    conn: &Connection,
    _namespace: &str,
    item_key: &str,
    _binary: &Path,
    model: Option<&str>,
    timeout: u64,
    mode: &EnrichMode,
) -> Result<EnrichItemResult, AppError> {
    let (mem_id, body, old_desc): (i64, String, String) = conn
        .query_row(
            "SELECT id, body, description FROM memories WHERE name = ?1 AND deleted_at IS NULL",
            rusqlite::params![item_key],
            |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
        )
        .map_err(|_| {
            AppError::NotFound(crate::i18n::validation::memory_named_not_found(item_key))
        })?;
    // The body is here so the model can RECOGNISE the memory it is describing,
    // not reason over it: the prompt already carries the name and the current
    // description. That is the preview role, and naming it keeps this budget
    // tied to the other preview sites instead of drifting as a local literal.
    let snippet: String = body
        .chars()
        .take(crate::constants::ENRICH_BODY_PREVIEW_CHARS)
        .collect();
    let input_text = format!(
        "Memory name: {item_key}\nCurrent description: {old_desc}\nBody preview: {snippet}"
    );
    let (value, cost, is_oauth) = match mode {
        EnrichMode::OpenRouter => call_openrouter(
            DESCRIPTION_ENRICH_PROMPT,
            DESCRIPTION_ENRICH_SCHEMA,
            &input_text,
            model,
            timeout,
        )?,
    };
    let new_desc = value
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or(&old_desc);
    let old_name: String = conn.query_row(
        "SELECT name FROM memories WHERE id = ?1",
        rusqlite::params![mem_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "UPDATE memories SET description = ?1 WHERE id = ?2",
        rusqlite::params![new_desc, mem_id],
    )?;
    memories::sync_fts_after_update(
        conn, mem_id, &old_name, &old_desc, &body, &old_name, new_desc, &body,
    )?;
    Ok(EnrichItemResult::Done {
        memory_id: Some(mem_id),
        entity_id: None,
        entities: 0,
        rels: 0,
        chars_before: Some(old_desc.len()),
        chars_after: Some(new_desc.len()),
        cost,
        is_oauth,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Minimal schema: only the three tables the evidence queries read.
    fn open_evidence_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE memories (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                body       TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
                deleted_at INTEGER
            );
            CREATE TABLE entities (
                id   INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL
            );
            CREATE TABLE memory_entities (
                memory_id INTEGER NOT NULL,
                entity_id INTEGER NOT NULL,
                PRIMARY KEY (memory_id, entity_id)
            );
            CREATE TABLE relationships (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                source_id INTEGER NOT NULL,
                target_id INTEGER NOT NULL,
                relation  TEXT NOT NULL,
                weight    REAL NOT NULL DEFAULT 0.5
            );
            INSERT INTO memories (id, body) VALUES (1, 'the subject signed the lease');
            INSERT INTO entities (id, name) VALUES (1, 'subject'), (2, 'landlord');
            INSERT INTO memory_entities (memory_id, entity_id) VALUES (1, 1);
            INSERT INTO relationships (source_id, target_id, relation, weight)
                VALUES (1, 2, 'depends-on', 0.9);",
        )
        .expect("fixture schema");
        conn
    }

    /// The tuned entry point was extracted FROM `load_entity_evidence`, so the
    /// only way the extraction can be wrong is by producing different evidence
    /// for the same entity under the keys that path has always read. Both
    /// halves of the evidence are present in the fixture — a linked body and a
    /// typed edge — because a regression that dropped one of them would still
    /// return a non-empty string and pass a mere emptiness check.
    #[test]
    fn tuned_evidence_matches_the_untuned_entry_point() {
        let conn = open_evidence_db();
        let untuned = load_entity_evidence(&conn, 1).expect("untuned evidence");
        let tuned = load_entity_evidence_tuned(&conn, 1, ENTITY_DESCRIPTION_TUNING)
            .expect("tuned evidence");
        assert_eq!(untuned, tuned);
        assert!(untuned.contains("Linked memory bodies:"));
        assert!(untuned.contains("Typed relations in the graph:"));
    }
}
