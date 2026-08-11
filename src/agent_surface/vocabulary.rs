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
    K_VOCABULARY_MAX_KEYS, K_VOCABULARY_MAX_SUGGESTIONS, K_VOCABULARY_SAMPLE_ELEMENTS,
    VOCABULARY_SUGGESTION_MIN_SIMILARITY,
};
use serde_json::Value;
use std::collections::BTreeSet;

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
}

impl<'a> Scope<'a> {
    /// Builds a scope over the elements and the envelope that carried them.
    pub fn new(elements: &'a [Value], envelope: &'a Value) -> Self {
        Self { elements, envelope }
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
        if self
            .elements
            .iter()
            .any(|element| filter::resolve(element, key).is_some())
        {
            return KeyOrigin::Element;
        }
        if filter::resolve(self.envelope, key).is_some() {
            return KeyOrigin::EnvelopeOnly;
        }
        KeyOrigin::Absent
    }

    /// Key names close enough to `key` to be worth offering as a correction.
    ///
    /// Ordered by descending similarity and capped at
    /// [`K_VOCABULARY_MAX_SUGGESTIONS`]. An empty vector means nothing in the
    /// vocabulary resembled the request, which is itself informative: the caller
    /// is looking at the wrong command, not at a typo.
    pub fn suggestions(&self, key: &str) -> Vec<String> {
        let (vocabulary, _) = self.candidate_keys();
        if vocabulary.is_empty() {
            return Vec::new();
        }

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
        ranked.truncate(K_VOCABULARY_MAX_SUGGESTIONS);
        ranked
            .into_iter()
            .map(|(_, name)| name.to_string())
            .collect()
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
