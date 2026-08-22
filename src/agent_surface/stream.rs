//! GAP-SG-215: the NDJSON stream contract, decided.
//!
//! [`super`] is defined over one complete envelope. `export` and `ingest` emit
//! something else — N self-contained records followed by a summary — and until
//! v1.2.8 they reached that envelope machinery once per LINE, through
//! `crate::output::emit_json_compact`. Three defects followed, all measured:
//!
//! * `--select name export --limit 3` emitted three correctly projected records
//!   and then `exit 2` on the fourth line. The summary carries `namespace`, not
//!   `name`, so the projection that resolved for every record failed on the one
//!   line that is not a record — after stdout had already been written to.
//! * `--select namespace export` did the mirror of that in SILENCE, `exit 0`:
//!   the key resolved, so the summary was projected down to `{"namespace":…}`
//!   and lost `summary: true`, the only end-of-stream signal a consumer has. A
//!   truncated export then looks exactly like a complete one.
//! * With NO knob at all, every line carried a 278-byte `agent_surface` record —
//!   measured over 200 lines — restating one fact about the PROCESS once per
//!   memory, absolute database path included. At the default `--limit 100000`
//!   that is ~27.8 MB, written into the file `docs/AGENTS.md` recommends
//!   creating with `export > backup.ndjson`.
//!
//! # The contract
//!
//! * **A record line carries the record and nothing else.** No `agent_surface`,
//!   no `truncated`. This is the invariant `super`'s module docs have declared
//!   since GAP-SG-142 — "NDJSON streams bypass the surface" — restored to being
//!   true. The NDJSON specification is explicit that the format carries no
//!   per-line header, metadata or schema; a stream is data, and the frame around
//!   it belongs somewhere else.
//! * **Only per-record knobs act.** `--select` and `--truncate-content` are
//!   stateless per record and mean the same thing whether a record arrives alone
//!   or in a stream. `crate::output::stream` named exactly that pair as the safe
//!   extension and asked for a contract decision before wiring it; this module is
//!   that decision. Everything else is refused by [`super::gate::evaluate_stream`]
//!   BEFORE the first byte, so a refusal never leaves a half-written stream.
//! * **The trailer is never shaped and carries the one record.** The summary
//!   line is already about the stream rather than about a memory, so the resolved
//!   target, the query ceiling and the projection findings ride there — once.
//!
//! The published schemas allow all three: `docs/schemas/export-memory-line` and
//! `export-summary` both declare `agent_surface` OPTIONAL, so dropping it from
//! the record and keeping it on the summary breaks no contract. What the old
//! behaviour did break was `export-summary`'s `required` list, every time a
//! projection deleted `summary`, `exported` or `elapsed_ms`.
//!
//! # Why the state is a cell and the decisions are not
//!
//! One process runs one subcommand and emits one stream, so a process-wide cell
//! is the single fact about that stream rather than ambient state — the same
//! reasoning [`super::universe`] documents. But GAP-SG-201 shipped a refusal no
//! test could reach precisely because the DECISION read the cell from inside
//! itself. So every function here that decides anything takes its premises as
//! arguments, and the cell is read at exactly one place: the emitters in
//! `crate::output::stream`.

use super::gate::{self, Findings};
use super::vocabulary::Scope;
use super::{shape, AgentSurface};
use crate::errors::AppError;
use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// Member marking an `agent_surface` record as describing a stream.
///
/// A consumer that reads the block off a summary line needs to know the counts
/// in it are about N lines rather than about the one it is holding.
const STREAM_KEY: &str = "stream";

/// Member counting the records `--truncate-content` actually shortened.
const RECORDS_TRUNCATED_KEY: &str = "records_truncated";

/// What one stream resolved before its first line, and what it did after.
///
/// Built once by [`open_with`] and read by every emission. The projection paths
/// are compiled HERE rather than per line for the same reason
/// [`shape::project`] compiles them once for a `Vec`: splitting a dotted key
/// inside the emission loop would allocate a `Vec<String>`, plus a `String` per
/// segment, for every record times every key. A stream has no `Vec` to hoist the
/// work out of, so the hoisting has to be the stream's own state.
#[derive(Debug)]
pub struct StreamState {
    /// What `--select` resolved against the record vocabulary, decided once.
    findings: Findings,
    /// `--select` keys pre-split into lookup paths.
    select_paths: Vec<Vec<String>>,
    /// How many records `--truncate-content` shortened.
    ///
    /// Atomic rather than a `Cell` because the emitters take `&'static
    /// StreamState` out of a `OnceLock`, which is `Sync` only if its contents
    /// are. Uninteresting cost: the counter is touched only on a record that was
    /// actually cut.
    shortened: AtomicUsize,
}

impl StreamState {
    /// The state of a stream that was never opened.
    ///
    /// Emitting through this shapes nothing and refuses nothing, which is the
    /// right failure mode for a stream whose command forgot to call [`open`]:
    /// records go out verbatim, which is the contract, and the trailer still
    /// carries the process record. `tests/stream_contract_gate.rs` is what makes
    /// forgetting visible rather than merely harmless.
    fn inert() -> &'static Self {
        static INERT: OnceLock<StreamState> = OnceLock::new();
        INERT.get_or_init(|| StreamState {
            findings: Findings::default(),
            select_paths: Vec::new(),
            shortened: AtomicUsize::new(0),
        })
    }
}

/// Resolves a stream's request against its records, before anything is emitted.
///
/// `sample` is a bounded prefix of the records the command is about to write,
/// and `total` is how many there really are. See [`gate::evaluate_stream`] for
/// why it is a prefix and not the whole set.
///
/// `total` exists so the bound gets DECLARED. [`Scope::vocabulary_is_partial`]
/// compares the elements it was handed against its own sampling constant, and a
/// prefix of exactly that size compares equal — so a 100 000-record export judged
/// on 64 records would have reported a complete vocabulary. Passing the real
/// count is what turns "I judged a prefix" from an implementation detail into a
/// field on the trailer.
///
/// # Errors
/// Returns [`AppError::Usage`] — exit `2` — when a knob cannot act on a stream,
/// or when `--select` names nothing any record carries. Both happen with stdout
/// still untouched, which is the whole point of resolving up front.
pub fn open_with(
    surface: &AgentSurface,
    sample: &[Value],
    total: usize,
) -> Result<StreamState, AppError> {
    // A stream has no envelope for a key to resolve against instead of the
    // records, and `Scope` wants one, so it gets the empty value. That is not a
    // placeholder: it states, correctly, that nothing here is envelope-only.
    let no_envelope = Value::Null;
    let mut findings = gate::evaluate_stream(
        surface,
        &Scope::new(sample, &no_envelope).with_command(surface.command.as_deref()),
    )?;
    // Scoped to `--select`, because the field qualifies an ANSWER about field
    // names and there is no such answer without a projection. Raising it
    // unconditionally reported `vocabulary_partial: true` on a plain `export`,
    // where the sample is empty by design and nothing was ever judged — a true
    // statement about the sample, and a misleading one about the run.
    if !surface.select.is_empty() {
        findings.vocabulary_partial |= total > sample.len();
    }
    Ok(StreamState {
        select_paths: shape::compile_paths(&surface.select),
        findings,
        shortened: AtomicUsize::new(0),
    })
}

/// Applies the per-record knobs to one line. Never annotates it.
///
/// The absence of an `agent_surface` insertion here is the contract, not an
/// omission — see the module docs.
#[must_use]
pub fn shape_record_with(state: &StreamState, surface: &AgentSurface, value: Value) -> Value {
    let mut value = if surface.select.is_empty() {
        value
    } else {
        shape::project_with(
            value,
            &surface.select,
            &state.select_paths,
            surface.command.as_deref(),
        )
    };
    if shape::truncate_strings(&mut value, surface.truncate_content) {
        // Release pairs with the Acquire in `trailer_with`: the trailer is the
        // one reader, and it must see every increment that happened before it.
        state.shortened.fetch_add(1, Ordering::Release);
    }
    value
}

/// Annotates the trailer with the one record for the whole stream.
///
/// Deliberately does NOT project, filter or cap. The summary line is the stream
/// describing itself; a `--select` aimed at the records has no business either
/// failing on it or rewriting it, and both of those were measured defects.
#[must_use]
pub fn trailer_with(
    state: &StreamState,
    surface: &AgentSurface,
    target: Option<Map<String, Value>>,
    mut value: Value,
) -> Value {
    let mut meta = Map::new();
    meta.insert(STREAM_KEY.into(), Value::Bool(true));
    if !surface.select.is_empty() {
        meta.insert("select".into(), json!(surface.select));
    }
    let shortened = state.shortened.load(Ordering::Acquire);
    if shortened > 0 {
        meta.insert(RECORDS_TRUNCATED_KEY.into(), json!(shortened));
    }
    if state.findings.is_partial() {
        meta.insert(
            "unresolved_keys".into(),
            json!(state.findings.unresolved_keys),
        );
        meta.insert("resolved_keys".into(), json!(state.findings.resolved_keys));
        meta.insert("key_resolution".into(), json!("partial"));
        if !state.findings.key_suggestions.is_empty() {
            meta.insert(
                "key_suggestions".into(),
                json!(state.findings.key_suggestions),
            );
        }
    }
    // Reported whatever the verdict, because it qualifies the whole stream and
    // not just a partial projection: it says the vocabulary came from a prefix
    // of the records rather than from all of them.
    if state.findings.vocabulary_partial {
        meta.insert("vocabulary_partial".into(), Value::Bool(true));
    }
    if let Some(record) = target {
        meta.extend(record);
    }
    // `truncated` rides the trailer for the same reason the counts do. The
    // module has always promised that removing data is never silent, and with
    // record lines now unannotated the trailer is the only place left to keep
    // that promise.
    super::attach_meta(&mut value, &meta, shortened > 0);
    value
}

static STREAM: OnceLock<StreamState> = OnceLock::new();

/// Opens the process's stream. First call wins.
///
/// # Errors
/// Propagates the refusal from [`open_with`], so a streaming command can fail
/// before it writes its first record simply by using `?`.
pub fn open(surface: &AgentSurface, sample: &[Value], total: usize) -> Result<(), AppError> {
    let state = open_with(surface, sample, total)?;
    let _ = STREAM.set(state);
    Ok(())
}

/// How many records [`open`] needs to see to resolve a projection.
///
/// The surface's own sampling constant, reused rather than restated: judging a
/// stream's vocabulary and suggesting names for a failed key are the same
/// question about the same records, and two constants for one question is how
/// they drift.
pub const SAMPLE_RECORDS: usize = crate::constants::K_VOCABULARY_SAMPLE_ELEMENTS;

/// The open stream, or an inert one when the command never opened it.
pub fn get() -> &'static StreamState {
    STREAM.get().unwrap_or_else(StreamState::inert)
}
