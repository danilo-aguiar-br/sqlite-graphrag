//! GAP-SG-202 / GAP-SG-203: resolving the caller's keys against the envelope
//! BEFORE any predicate runs.
//!
//! Until v1.2.6 a key the envelope never carried was indistinguishable from a
//! key whose value happened to be absent. `--select body_length read` answered
//! with an envelope missing the field, `--filter chave_errada=x list` answered
//! `count: 0` over 1892 memories, and both exited `0`. The caller read its own
//! typo as "the data is not there".
//!
//! The cure is to ask, once, where each requested key actually lives:
//!
//! * [`KeyOrigin::Element`] — the predicate has something to work on.
//! * [`KeyOrigin::EnvelopeOnly`] — the key names a member of the envelope, not a
//!   field of the elements. `--filter integrity_ok=false health` is this case:
//!   `integrity_ok` is a top-level scalar, the predicate was redirected onto the
//!   `checks` array, and all eight checks were deleted while `integrity_ok: true`
//!   survived in the payload contradicting the very predicate.
//! * [`KeyOrigin::Absent`] — the key exists nowhere the surface can see.
//!
//! # Cost
//!
//! Resolution scans EVERY element and allocates nothing: [`filter::resolve`] is a
//! pointer walk over borrowed data. Sampling here would be the wrong economy —
//! a key present only in an unsampled element would be reported absent, and the
//! gate would refuse a legitimate request. Sampling belongs to the suggestion
//! path alone, which runs only after a key has already failed.
//!
//! Serial by decision, not by omission: the parallelism rules forbid paying
//! coordination overhead for work smaller than it, and this is a handful of
//! pointer walks per requested key.

use super::filter;
use crate::constants::{
    agent_surface_field_synonym_groups, K_VOCABULARY_MAX_KEYS, K_VOCABULARY_MAX_SUGGESTIONS,
    K_VOCABULARY_SAMPLE_ELEMENTS, VOCABULARY_SUGGESTION_MIN_SIMILARITY,
};
use serde_json::Value;
use std::collections::BTreeSet;

/// GAP-SG-230: every spelling that names the same field as `key`, `key` first.
///
/// The synonym applies to the LAST segment of a dotted path and the prefix is
/// carried over unchanged, so `graph_context.entity_type` yields
/// `graph_context.type` and never a bare `type`. A synonym is a fact about a
/// FIELD NAME; where that field sits is the caller's statement about the payload
/// and is not ours to rewrite.
///
/// The caller's own spelling is always first, so a payload that carries BOTH
/// spellings — which nothing emits today, and which a future struct could —
/// resolves to what was asked for rather than to whichever the table lists first.
///
/// Allocates one small `Vec` per REQUESTED key, never per element. That is the
/// distinction the module docs draw about cost: [`Scope::classify`] still walks
/// every element with borrowed data, and the vector here is built once before
/// that walk starts, so a 107 135-element scan pays for it exactly once.
///
/// GAP-SG-274: `command` is the subcommand slug and it selects which groups of
/// the table apply, so `kind` names the entity type under `graph` and stays the
/// line discriminator under `graph-ndjson`. A spelling listed by two applicable
/// groups is emitted once.
fn spellings(key: &str, command: Option<&str>) -> Vec<String> {
    let (prefix, leaf) = match key.rfind('.') {
        Some(idx) => (&key[..=idx], &key[idx + 1..]),
        None => ("", key),
    };
    let mut out = vec![key.to_string()];
    for group in agent_surface_field_synonym_groups(command) {
        if !group.contains(&leaf) {
            continue;
        }
        for spelling in group {
            let candidate = format!("{prefix}{spelling}");
            if *spelling != leaf && !out.contains(&candidate) {
                out.push(candidate);
            }
        }
    }
    out
}

/// Where a requested key was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOrigin {
    /// Present in at least one result element.
    Element,
    /// Present on the envelope but in none of the elements.
    EnvelopeOnly,
    /// Present nowhere the surface observed.
    Absent,
}

/// The vocabulary one request is resolved against.
///
/// Borrows both halves; nothing here outlives the call that builds it.
pub struct Scope<'a> {
    /// Result elements, already lifted out of the envelope.
    elements: &'a [Value],
    /// What remains of the envelope once the result array was lifted out.
    envelope: &'a Value,
    /// GAP-SG-274: subcommand slug scoping the field-synonym table, if known.
    command: Option<&'a str>,
}

impl<'a> Scope<'a> {
    /// Builds a scope over the elements and the envelope that carried them.
    ///
    /// The synonym scope starts unset, which admits only the groups that hold
    /// for every command. That is the fail-safe half of the pair: a caller that
    /// never states which subcommand it is resolving for gets no mode-specific
    /// synonym, rather than the synonyms of some arbitrary mode.
    pub fn new(elements: &'a [Value], envelope: &'a Value) -> Self {
        Self {
            elements,
            envelope,
            command: None,
        }
    }

    /// GAP-SG-274: states which subcommand slug this scope resolves keys for.
    ///
    /// Consumed by the synonym table alone. `graph` declares `kind` a spelling
    /// of the entity type; `graph-ndjson` does not, because there `kind` is the
    /// line discriminator and answering `node` to `--select type` would be a
    /// wrong value rather than a missing one.
    #[must_use]
    pub fn with_command(mut self, command: Option<&'a str>) -> Self {
        self.command = command;
        self
    }

    /// `true` when there are no elements to resolve a key against.
    ///
    /// An empty result array carries no vocabulary, so it cannot tell a key that
    /// does not exist from a key that simply had no row to appear in.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Where `key` lives, if anywhere.
    ///
    /// An envelope with no elements can still answer [`KeyOrigin::EnvelopeOnly`],
    /// which is what makes the scalar-envelope refusal precise instead of a
    /// blanket "no array here".
    pub fn classify(&self, key: &str) -> KeyOrigin {
        // GAP-SG-230: every spelling of the field counts, not just the one the
        // caller typed. This is the single point the four shaping knobs share —
        // `--select` reaches it through `resolve_projection`, and `--filter`,
        // `--sort` and `--dedupe-by` reach it directly — so resolving the synonym
        // here is what stops three of the four from needing their own copy.
        let candidates = spellings(key, self.command);
        if self.elements.iter().any(|element| {
            candidates
                .iter()
                .any(|candidate| filter::resolve(element, candidate).is_some())
        }) {
            return KeyOrigin::Element;
        }
        if candidates
            .iter()
            .any(|candidate| filter::resolve(self.envelope, candidate).is_some())
        {
            return KeyOrigin::EnvelopeOnly;
        }
        KeyOrigin::Absent
    }

    /// GAP-SG-230: the spelling this payload actually uses for `key`.
    ///
    /// Returns `key` itself when the payload carries it, the synonym when the
    /// payload spells the same field differently, and `None` when no spelling in
    /// the group is present anywhere in scope.
    ///
    /// # Why this is a separate question from [`Self::classify`]
    ///
    /// `classify` answers "may this request proceed"; this answers "which name do
    /// I look the value up under". They only look like one question while both
    /// spellings are the same string. `graph entities` emits `entity_type` and
    /// `graph --format json` emits `type` for the very same column, so a caller
    /// that learned one name asks with it against both — and the four shaping
    /// knobs walk the path with the name the CALLER wrote, never with the verdict
    /// this scope reached. Answering only the first question would let a request
    /// pass the gate and then match nothing, which trades a refusal that names
    /// the fix for an empty set with `exit 0`.
    ///
    /// Elements are consulted before the envelope, mirroring `classify`, so the
    /// answer describes the place a predicate or a projection will actually look.
    pub fn effective_key(&self, key: &str) -> Option<String> {
        let candidates = spellings(key, self.command);
        for candidate in &candidates {
            if self
                .elements
                .iter()
                .any(|element| filter::resolve(element, candidate).is_some())
            {
                return Some(candidate.clone());
            }
        }
        candidates
            .into_iter()
            .find(|candidate| filter::resolve(self.envelope, candidate).is_some())
    }

    /// Key names close enough to `key` to be worth offering as a correction.
    ///
    /// Ordered by descending similarity and capped at
    /// [`K_VOCABULARY_MAX_SUGGESTIONS`]. An empty vector means nothing in the
    /// vocabulary resembled the request, which is itself informative: the caller
    /// is looking at the wrong command, not at a typo.
    ///
    /// GAP-SG-230: a DECLARED synonym that the payload really carries is placed
    /// first and bypasses the similarity floor entirely. Similarity is a proxy
    /// for "you mistyped this"; a synonym is not a typo, it is the same field
    /// under the name a sibling surface chose, and the table says so as a fact.
    /// Leaving it to Jaro-Winkler was measured to lose exactly the case this
    /// exists for: `entity_type` against `type` shares no prefix, so the metric
    /// scores it below [`VOCABULARY_SUGGESTION_MIN_SIMILARITY`] and the caller
    /// who asked with the sibling spelling was told nothing resembled its key —
    /// while the field sat right there under another name.
    pub fn suggestions(&self, key: &str) -> Vec<String> {
        let (vocabulary, _) = self.candidate_keys();
        if vocabulary.is_empty() {
            return Vec::new();
        }

        // Only spellings the payload actually carries are offered, so a synonym
        // group never advertises a name this envelope has no column for.
        let declared: Vec<String> = spellings(key, self.command)
            .into_iter()
            .skip(1)
            .filter(|candidate| {
                let leaf = candidate.rsplit('.').next().unwrap_or(candidate);
                vocabulary.contains(leaf)
            })
            .collect();

        // `BatchComparator` pre-processes the needle once and reuses it across
        // the whole vocabulary, which is exactly the one-against-many shape here.
        let comparator = rapidfuzz::distance::jaro_winkler::BatchComparator::new(key.chars());
        let mut ranked: Vec<(f64, &str)> = vocabulary
            .iter()
            .map(|candidate| {
                (
                    comparator.normalized_similarity(candidate.chars()),
                    *candidate,
                )
            })
            .filter(|(score, _)| *score >= VOCABULARY_SUGGESTION_MIN_SIMILARITY)
            .collect();

        // Descending by score, then by name so two equally close candidates come
        // out in the same order on every platform.
        ranked.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(b.1))
        });
        // The declared synonyms take their slots first, and the similarity
        // ranking fills whatever is left of the budget without restating them.
        let mut out = declared;
        for (_, name) in ranked {
            if out.len() >= K_VOCABULARY_MAX_SUGGESTIONS {
                break;
            }
            if !out.iter().any(|already| already.as_str() == name) {
                out.push(name.to_string());
            }
        }
        out.truncate(K_VOCABULARY_MAX_SUGGESTIONS);
        out
    }

    /// Distinct field names a correction could plausibly have meant.
    ///
    /// Reads the elements when there are elements and the envelope when there
    /// are none, because that is the same split [`resolve_projection`] uses to
    /// decide what a key must address. Without the fallback the most useful
    /// refusal in the catalogue — `--select body_length read`, the case
    /// GAP-SG-202 was written from — named no alternative at all, since a `read`
    /// envelope carries no array to sample.
    ///
    /// Borrowed, never cloned: the set holds `&str` into the payload, so a
    /// vocabulary of five hundred names costs five hundred pointers.
    ///
    /// [`resolve_projection`]: super::gate
    /// Whether the SUGGESTION vocabulary was built from less than everything.
    ///
    /// Reported as `vocabulary_partial` so a caller reading an empty or thin
    /// suggestion list can tell "nothing resembled your key" from "the sampler
    /// stopped before it got there". Only the suggestion path samples;
    /// [`Self::classify`] always scans every element, so a PARTIAL vocabulary
    /// never weakens a verdict — it only shortens the advice that follows one.
    pub fn vocabulary_is_partial(&self) -> bool {
        self.elements.len() > K_VOCABULARY_SAMPLE_ELEMENTS || self.candidate_keys().1
    }

    /// Returns the candidate names and whether a ceiling cut the collection short.
    fn candidate_keys(&self) -> (BTreeSet<&'a str>, bool) {
        let mut names = BTreeSet::new();
        if self.elements.is_empty() {
            let capped = self
                .envelope
                .as_object()
                .is_some_and(|map| Self::absorb(map.keys(), &mut names));
            return (names, capped);
        }
        for element in self.elements.iter().take(K_VOCABULARY_SAMPLE_ELEMENTS) {
            let Some(map) = element.as_object() else {
                continue;
            };
            if Self::absorb(map.keys(), &mut names) {
                return (names, true);
            }
        }
        (names, false)
    }

    /// Inserts names until the ceiling is reached; `true` means it was reached.
    ///
    /// The ceiling exists because the envelope is caller-influenced, and the
    /// memory rules forbid letting untrusted input size an allocation without a
    /// bound. Hitting it shortens the suggestion list and nothing else.
    fn absorb<I>(keys: I, names: &mut BTreeSet<&'a str>) -> bool
    where
        I: Iterator<Item = &'a String>,
    {
        for name in keys {
            if names.len() >= K_VOCABULARY_MAX_KEYS {
                return true;
            }
            names.insert(name.as_str());
        }
        false
    }
}
