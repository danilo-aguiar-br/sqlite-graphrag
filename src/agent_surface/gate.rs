//! The refusals. One place decides when a shaping request cannot be honoured.
//!
//! Until v1.2.6 every impossible request was answered with an empty set and
//! `exit 0`, which a caller cannot tell from "the data is not there". Four
//! measured shapes produced that:
//!
//! * GAP-SG-202 — a projection or predicate key that exists in no element.
//! * GAP-SG-203 — a predicate aimed at an envelope member, redirected onto an
//!   array the caller never named and deleting it whole.
//! * GAP-SG-204 — a knob declared against an envelope with no result array at
//!   all, so it could not possibly act.
//! * GAP-SG-201 — a predicate judging only the page the query returned.
//!
//! # Why refusals live here and not in the shaping primitives
//!
//! [`super::shape`] is stateless and knows nothing about commands, ceilings or
//! intent; keeping the verdict out of it is what lets one implementation serve
//! every subcommand. This module is the only one allowed to fail.
//!
//! # The fence
//!
//! [`super::apply`] runs at OUTPUT time, after the handler already did its work.
//! Refusing there for a command that changed durable state would report failure
//! for an operation that succeeded, and a caller that retries a succeeded
//! `remember` writes the memory twice. So a surface marked
//! [`super::AgentSurface::mutates`] is never refused — it is annotated instead.
//! That is a deliberate asymmetry: a diagnostic lost is cheaper than data
//! duplicated.

use super::universe::{self, CeilingKind, FilterScope};
use super::vocabulary::{KeyOrigin, Scope};
use super::AgentSurface;
use crate::errors::AppError;
use crate::i18n::validation as msg;

/// What resolution learned, for the `agent_surface` record.
#[derive(Debug, Default)]
pub struct Findings {
    /// Keys asked for by `--select` that the shaped payload can actually carry.
    pub resolved_keys: Vec<String>,
    /// Keys asked for by `--select` that nothing in scope carries.
    ///
    /// Non-empty here means the projection succeeded PARTIALLY. A caller that
    /// gets fewer fields than it asked for learns why from this list instead of
    /// concluding the records were incomplete.
    pub unresolved_keys: Vec<String>,
    /// Names the caller might have meant, for the keys that failed.
    ///
    /// Carried as DATA rather than folded into a sentence: a refusal message is
    /// localized, so parsing the suggestion out of it would mean parsing prose in
    /// one of two languages. `rules-rust-cli-com-clap-io-exitcodes-erros` asks a
    /// refusal to carry a corrective action, and the agent-native contract asks
    /// the action to be machine-readable.
    pub key_suggestions: Vec<String>,
    /// Whether the suggestion vocabulary was sampled rather than exhaustive.
    ///
    /// Never weakens a verdict — [`Scope::classify`] always scans every element.
    /// It qualifies the ADVICE: a short list under a partial vocabulary means
    /// "the sampler stopped", not "nothing resembles your key".
    pub vocabulary_partial: bool,
}

impl Findings {
    /// `true` when at least one requested key failed to resolve.
    pub fn is_partial(&self) -> bool {
        !self.unresolved_keys.is_empty()
    }
}

/// Label used when the result array is the envelope itself (a top-level array).
const ANONYMOUS_ARRAY: &str = "results";

/// Argv spellings reported under `discarded_flags`.
///
/// Named rather than inlined because they are a wire contract now: a consumer
/// matches on them, so a typo in one branch would be a silent contract break.
/// They are argv tokens, never prose, so they are NOT localized.
const FILTER_FLAG: &str = "--filter";
/// Argv spelling of the ordering knob.
const SORT_FLAG: &str = "--sort";
/// Argv spelling of the deduplication knob.
const DEDUPE_FLAG: &str = "--dedupe-by";
/// Argv spelling of the projection knob.
const SELECT_FLAG: &str = "--select";

/// Builds a refusal that names, as data, the flags it could not honour.
///
/// `rules-rust-cli-stdin-stdout-silent-discard` asks the error JSON to carry
/// `discarded_flags`. Every refusal below is by definition an argument the
/// caller typed and the binary did not apply, so each one fills it — reading
/// which of your own flags were dropped must never require parsing a sentence.
fn refuse(message: String, discarded_flags: Vec<String>) -> AppError {
    AppError::Usage {
        message,
        discarded_flags,
    }
}

/// Decides whether `surface` may be applied to the envelope described by `scope`.
///
/// # Errors
/// Returns [`AppError::Usage`] — exit `2` — when the request cannot be honoured
/// as written. Every message names the offending flag and a way forward.
pub fn evaluate(
    surface: &AgentSurface,
    scope: &Scope,
    array_key: Option<&str>,
    has_array: bool,
) -> Result<Findings, AppError> {
    let mut findings = Findings::default();

    // The fence. See the module docs for why this is unconditional.
    if surface.mutates {
        return Ok(findings);
    }

    refuse_inert_knobs(surface, has_array)?;
    refuse_a_predicate_over_a_page(surface)?;

    if surface.allow_unknown_keys {
        return Ok(findings);
    }

    // An EMPTY result array is absence of evidence, not evidence of absence.
    // `related --select name` over a seed with no neighbours found zero elements,
    // so every key was unresolvable and the refusal even suggested the key it had
    // just rejected. Refusing on an empty set inverts this gate into the very
    // false negative it exists to remove: the caller would read "your key is
    // wrong" where the truth is "there were no rows".
    //
    // A scalar envelope is excluded because it has no elements BY SHAPE and
    // resolves against the envelope itself, which is real evidence.
    if has_array && scope.is_empty() {
        return Ok(findings);
    }

    let array_name = array_key.unwrap_or(ANONYMOUS_ARRAY);
    for expr in &surface.filters {
        refuse_unusable_key(scope, FILTER_FLAG, &expr.key(), array_name)?;
    }
    if let Some(key) = &surface.sort {
        refuse_unusable_key(scope, SORT_FLAG, key, array_name)?;
    }
    if let Some(key) = &surface.dedupe_by {
        refuse_unusable_key(scope, DEDUPE_FLAG, key, array_name)?;
    }

    // An array elected by the FALLBACK is not a declared result set: `stats`
    // carries `namespaces`, which is not in `AGENT_SURFACE_RESULT_KEYS`, so the
    // surface picked it for want of anything better. Binding projection to it
    // refused the documented `stats --json --select total_memories`, whose key
    // is a top-level member — the same misdirection GAP-SG-203 describes for
    // predicates, reaching `--select`.
    let declared_array = array_key.is_some_and(super::is_declared_result_array);
    resolve_projection(surface, scope, has_array && declared_array, &mut findings)?;
    Ok(findings)
}

/// GAP-SG-201: a predicate must not report on a set the query already cut.
///
/// Only [`CeilingKind::Pagination`] earns a refusal, and only when the ceiling
/// actually removed rows. A `--limit` wider than the corpus cut nothing, and a
/// top-k bound is the answer rather than a truncation of one — see
/// [`super::universe`] for why that distinction is load-bearing.
///
/// `--sort` and `--select` are absent by design: reordering or projecting the
/// rows you received claims nothing about the rows you did not.
fn refuse_a_predicate_over_a_page(surface: &AgentSurface) -> Result<(), AppError> {
    if surface.filters.is_empty() {
        return Ok(());
    }
    let Some(ceiling) = universe::get() else {
        return Ok(());
    };
    if ceiling.kind != CeilingKind::Pagination || !ceiling.truncated_the_universe() {
        return Ok(());
    }
    if surface.filter_scope == Some(FilterScope::Page) {
        return Ok(());
    }
    let total = ceiling.universe_total.unwrap_or(ceiling.applied);
    Err(refuse(
        msg::filter_scope_is_a_page(ceiling.applied, total, ceiling.source.as_str()),
        vec![FILTER_FLAG.to_string()],
    ))
}

/// GAP-SG-204: a knob with nothing to act on is an argument silently discarded.
fn refuse_inert_knobs(surface: &AgentSurface, has_array: bool) -> Result<(), AppError> {
    if has_array {
        return Ok(());
    }
    // `--select` is deliberately absent: on an envelope with no result array it
    // projects the envelope itself, which is a real effect.
    let mut discarded = Vec::new();
    if !surface.filters.is_empty() {
        discarded.push(FILTER_FLAG.to_string());
    }
    if surface.sort.is_some() {
        discarded.push(SORT_FLAG.to_string());
    }
    if surface.dedupe_by.is_some() {
        discarded.push(DEDUPE_FLAG.to_string());
    }
    if discarded.is_empty() {
        return Ok(());
    }
    Err(refuse(msg::knob_without_target(&discarded), discarded))
}

/// GAP-SG-202 and GAP-SG-203: a predicate key must address the elements it will
/// be evaluated against.
fn refuse_unusable_key(
    scope: &Scope,
    flag: &str,
    key: &str,
    array_name: &str,
) -> Result<(), AppError> {
    match scope.classify(key) {
        KeyOrigin::Element => Ok(()),
        KeyOrigin::EnvelopeOnly => Err(refuse(
            msg::key_is_envelope_only(flag, key, array_name),
            vec![flag.to_string()],
        )),
        KeyOrigin::Absent => Err(refuse(
            msg::key_absent(flag, key, &scope.suggestions(key)),
            vec![flag.to_string()],
        )),
    }
}

/// Classifies every `--select` key and refuses only when NONE of them resolve.
///
/// Partial success stays successful on purpose: an agent projecting six fields
/// across a heterogeneous result set still gets a useful answer when one field
/// is missing, and `unresolved_keys` tells it which. Refusing the whole request
/// there would trade a silent omission for a needless failure.
fn resolve_projection(
    surface: &AgentSurface,
    scope: &Scope,
    has_array: bool,
    findings: &mut Findings,
) -> Result<(), AppError> {
    if surface.select.is_empty() {
        return Ok(());
    }
    // Projection targets the elements when there are elements, and the envelope
    // otherwise, so what counts as resolvable follows the shape of the payload.
    let usable = if has_array {
        KeyOrigin::Element
    } else {
        KeyOrigin::EnvelopeOnly
    };
    for key in &surface.select {
        if scope.classify(key) == usable {
            findings.resolved_keys.push(key.clone());
        } else {
            findings.unresolved_keys.push(key.clone());
        }
    }
    if !findings.resolved_keys.is_empty() {
        // Partial success stays successful, so the advice has to travel WITH the
        // answer: this is the only path where a caller receives fewer fields than
        // it asked for and no error envelope explains why.
        if !findings.unresolved_keys.is_empty() {
            findings.vocabulary_partial = scope.vocabulary_is_partial();
            let mut seen = std::collections::BTreeSet::new();
            for key in &findings.unresolved_keys {
                for candidate in scope.suggestions(key) {
                    seen.insert(candidate);
                }
            }
            findings.key_suggestions = seen.into_iter().collect();
        }
        return Ok(());
    }
    let suggestions = surface
        .select
        .first()
        .map(|key| scope.suggestions(key))
        .unwrap_or_default();
    Err(refuse(
        msg::select_fully_unresolved(&surface.select, &suggestions),
        vec![SELECT_FLAG.to_string()],
    ))
}
