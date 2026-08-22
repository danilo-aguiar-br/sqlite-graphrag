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

use super::universe::{self, CeilingKind, FilterScope, QueryCeiling};
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
/// Argv spelling of the knob that replaces the payload with a count.
const COUNT_ONLY_FLAG: &str = "--count-only";
/// Argv spelling of the envelope byte ceiling.
const MAX_OUTPUT_BYTES_FLAG: &str = "--max-output-bytes";
/// Argv spelling of the cap on emitted result elements.
const MAX_ITEMS_FLAG: &str = "--max-items";

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
    ceiling: Option<&universe::QueryCeiling>,
) -> Result<Findings, AppError> {
    let mut findings = Findings::default();

    // The fence. See the module docs for why this is unconditional.
    if surface.mutates {
        return Ok(findings);
    }

    // First, because "this is a stream" is a fact about the SHAPE of the output
    // and outranks every question about the set the query returned.
    refuse_whole_set_knobs_on_a_stream(surface)?;
    refuse_inert_knobs(surface, has_array)?;
    refuse_a_predicate_over_a_page(surface, ceiling)?;
    refuse_a_count_over_a_page(surface, ceiling)?;

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

/// GAP-SG-215: the same verdict, for a payload that is a STREAM of records.
///
/// [`evaluate`] answers about one complete envelope, and every question it asks
/// past the refusals — which member is the result array, is that array declared,
/// did a ceiling cut it — is a question about envelope structure a stream does
/// not have. Running it per line is precisely the defect this closes:
/// `--select name export` projected three records correctly and then failed on
/// the fourth line, the summary, because that line is a different shape and was
/// judged as though it were a record.
///
/// So a stream gets the two decisions that ARE about records, sharing the very
/// functions [`evaluate`] uses:
///
/// * the stream refusals, so an unusable knob fails before the first byte
/// * projection resolution against the record vocabulary, so `--select` is
///   answered ONCE for the whole stream instead of once per line
///
/// `scope` is built from a bounded prefix of the records rather than from all of
/// them. [`super::vocabulary::Scope::classify`] scans every element it is given,
/// and giving it every element of a 100 000-row export would mean holding the
/// whole corpus as `Value` — measured at ~24 KB per record, so ~2.4 GB. The
/// prefix is the honest bound, and `vocabulary_partial` on the trailer declares
/// that it was one. It is still strictly stronger than what it replaces, which
/// judged each line in isolation against no vocabulary at all.
///
/// # Errors
/// Returns [`AppError::Usage`] — exit `2` — before the caller has written
/// anything, which is the property the per-line path could not offer.
pub fn evaluate_stream(surface: &AgentSurface, scope: &Scope) -> Result<Findings, AppError> {
    let mut findings = Findings::default();

    // The fence, for the same reason as in `evaluate`: `ingest` streams and
    // writes, and refusing after a write is what makes a caller retry a
    // succeeded operation.
    if surface.mutates {
        return Ok(findings);
    }

    refuse_whole_set_knobs(surface)?;
    refuse_a_predicate_on_a_stream(surface)?;

    if surface.allow_unknown_keys || scope.is_empty() {
        return Ok(findings);
    }
    // `true` because a stream IS its records: there is no envelope for a key to
    // resolve against instead, so projection always targets the elements.
    resolve_projection(surface, scope, true, &mut findings)?;
    Ok(findings)
}

/// GAP-SG-201: the ceiling, when it hid rows the caller is about to report on.
///
/// Returns `None` — meaning "nothing to refuse" — in the three cases that are
/// not a hidden page:
///
/// * No ceiling was declared, so the command never paged anything.
/// * A [`CeilingKind::TopK`] bound, which IS the answer rather than a truncation
///   of one — see [`super::universe`] for why that distinction is load-bearing.
/// * A ceiling that cut nothing, because a `--limit` wider than the corpus left
///   the caller looking at the whole universe.
///
/// And in the one case where the caller already answered the question:
/// [`FilterScope::Page`] declares the narrower reading on purpose.
///
/// # Why this is a function and not two copies of four conditions
///
/// GAP-SG-201 shipped with the predicate refusal wired and the count refusal
/// written, translated and never called. Spelling the escapes out twice is how
/// the second copy would drift from the first the next time one is added — the
/// same "two hand-written lists with no contract between them" that produced the
/// split relation vocabulary. One implementation, both readers.
///
/// The ceiling arrives as an ARGUMENT rather than being read here. It lives in a
/// process-wide `OnceLock`, which is correct for a one-shot binary and wrong for
/// a test binary: `OnceLock` offers no reset for a `static` — `take` and every
/// `get_mut` need `&mut self` — so two tests in one process could never state
/// different ceilings, and no refusal in this family had a test at all. Taking
/// it as a parameter makes each test state its own premise. [`super::apply_with_target`]
/// already does exactly this with the resolved target, for the same reason.
fn a_truncated_page<'a>(
    surface: &AgentSurface,
    ceiling: Option<&'a QueryCeiling>,
) -> Option<&'a QueryCeiling> {
    let ceiling = ceiling?;
    if ceiling.kind != CeilingKind::Pagination || !ceiling.truncated_the_universe() {
        return None;
    }
    if surface.filter_scope == Some(FilterScope::Page) {
        return None;
    }
    Some(ceiling)
}

/// GAP-SG-201: a predicate must not report on a set the query already cut.
///
/// `--sort` and `--select` are absent by design: reordering or projecting the
/// rows you received claims nothing about the rows you did not.
fn refuse_a_predicate_over_a_page(
    surface: &AgentSurface,
    ceiling: Option<&QueryCeiling>,
) -> Result<(), AppError> {
    if surface.filters.is_empty() {
        return Ok(());
    }
    let Some(ceiling) = a_truncated_page(surface, ceiling) else {
        return Ok(());
    };
    let total = ceiling.universe_total.unwrap_or(ceiling.applied);
    Err(refuse(
        msg::filter_scope_is_a_page(ceiling.applied, total, ceiling.source.as_str()),
        vec![FILTER_FLAG.to_string()],
    ))
}

/// GAP-SG-201: a bare count over a page reads as the inventory.
///
/// The sibling above turns on `--filter`, so `--count-only` ALONE slipped under
/// it: `--count-only graph entities` answered `50` over a universe of 107 111
/// with `exit 0`, on a command line that mentioned no limit at all, because that
/// subcommand caps at 50 by itself. A caller that reads `{"count": 50}` cannot
/// tell that from a corpus which really held fifty.
///
/// `--count-only` earns its own refusal rather than an extra clause on the
/// sibling because the two are independent knobs: a count is a claim about the
/// size of a set, which is exactly the claim a page cannot support, whether or
/// not a predicate was also given.
fn refuse_a_count_over_a_page(
    surface: &AgentSurface,
    ceiling: Option<&QueryCeiling>,
) -> Result<(), AppError> {
    if !surface.count_only {
        return Ok(());
    }
    let Some(ceiling) = a_truncated_page(surface, ceiling) else {
        return Ok(());
    };
    let total = ceiling.universe_total.unwrap_or(ceiling.applied);
    Err(refuse(
        msg::count_only_over_a_page(ceiling.applied, total),
        vec![COUNT_ONLY_FLAG.to_string()],
    ))
}

/// GAP-SG-209: a knob that needs the whole set was aimed at a stream.
///
/// `export` emits one self-contained record per line, and [`super::apply`] runs
/// once per emitted envelope. `--count-only export --limit 10` therefore
/// answered with ELEVEN `{"count":1}` lines — one per record plus the summary —
/// rather than one count of ten. The other three are the same mistake in a
/// quieter register: a byte budget spent per line is not a budget on the output,
/// and an ordering or a dedup that cannot see the next line is not one at all.
///
/// `--select` and `--truncate-content` are absent by design. Each acts WITHIN one
/// record and means exactly the same thing whether the record arrives alone or in
/// a stream, so refusing them would remove a working feature to cure nothing.
/// GAP-SG-215 makes that pair the WHOLE of what a stream accepts.
///
/// `--max-items` joined the list in v1.2.8. It was measured accepted and inert —
/// `--max-items 2 export --limit 5` answered with all five records and `exit 0` —
/// because it caps elements INSIDE an envelope and a record line carries no array
/// to cap. The corrective action is the query's own `--limit`, which the message
/// names.
///
/// Reached only for a read-only stream in practice: the `mutates` fence above
/// returns first for `ingest`, which is the other streaming subcommand, and
/// refusing after a write is the hazard that fence exists to prevent.
fn refuse_whole_set_knobs_on_a_stream(surface: &AgentSurface) -> Result<(), AppError> {
    if !surface.streamed {
        return Ok(());
    }
    refuse_whole_set_knobs(surface)?;
    refuse_a_predicate_on_a_stream(surface)
}

/// The body of the refusal above, with the "is this a stream" test already made.
///
/// Split out so [`evaluate_stream`] cannot forget it. That path is only ever
/// reached from a stream emitter, so re-testing `surface.streamed` there would
/// make the refusal depend on a flag being wired rather than on the caller being
/// a stream — the same "guard anchored to a proxy instead of the real property"
/// that GAP-SG-206 cost two attempts to unlearn.
fn refuse_whole_set_knobs(surface: &AgentSurface) -> Result<(), AppError> {
    let mut discarded = Vec::new();
    if surface.count_only {
        discarded.push(COUNT_ONLY_FLAG.to_string());
    }
    if surface.sort.is_some() {
        discarded.push(SORT_FLAG.to_string());
    }
    if surface.dedupe_by.is_some() {
        discarded.push(DEDUPE_FLAG.to_string());
    }
    if surface.max_output_bytes > 0 {
        discarded.push(MAX_OUTPUT_BYTES_FLAG.to_string());
    }
    if surface.max_items > 0 {
        discarded.push(MAX_ITEMS_FLAG.to_string());
    }
    if discarded.is_empty() {
        return Ok(());
    }
    Err(refuse(msg::knob_needs_a_whole_set(&discarded), discarded))
}

/// GAP-SG-215: `--filter` on a stream would desynchronise the trailer's tally.
///
/// Filtering per record is mechanically possible, which is exactly why this
/// needs its own refusal rather than a sixth entry in the list above: the reason
/// is not "it cannot act". It is that the COMMAND counts the records — `export`
/// reports `exported`, `ingest` reports `files_succeeded` — and those numbers are
/// computed before the surface ever sees a line. A predicate that silently
/// dropped records would leave the trailer claiming a count the stream never
/// emitted, which is a worse failure than refusing: the caller would have no way
/// to notice.
///
/// Until v1.2.8 this refused anyway, by accident, through
/// [`refuse_inert_knobs`] — a record line carries no result array, so the
/// predicate was rejected as having nothing to act on. Right verdict, wrong
/// reason, and a reason that would have stopped being true the moment a stream
/// emitted a record that happened to contain an array.
fn refuse_a_predicate_on_a_stream(surface: &AgentSurface) -> Result<(), AppError> {
    if surface.filters.is_empty() {
        return Ok(());
    }
    let discarded = vec![FILTER_FLAG.to_string()];
    Err(refuse(msg::filter_would_desync_a_tally(), discarded))
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
