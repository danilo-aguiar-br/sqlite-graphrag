//! GAP-SG-142: agent-native reshaping of the JSON envelope.
//!
//! Every subcommand used to hand its whole envelope back to the caller, so an
//! agent had to keep a `jaq` filter in its prompt just to read one field. This
//! module gives the CLI the projection / filter / sort / dedup / limit /
//! truncation surface the sibling tools already expose, applied at a **single**
//! point: [`crate::output`] serializes the response, hands the resulting
//! [`serde_json::Value`] to `apply`, and writes what comes back.
//!
//! Working on the serialized value rather than on each command's response
//! struct is what keeps this DRY — one implementation covers the whole CLI and
//! no subcommand needs to know the surface exists.
//!
//! # Invariants
//!
//! * **Failures always reach the caller.** An envelope carrying `error: true`
//!   or `ok: false` is emitted verbatim; `--filter` shapes result rows, never
//!   the error contract.
//! * **JSON Schema documents are never shaped.** `--print-schema` output is
//!   recognised by its `$schema` member and passes through untouched.
//! * **Truncation is never silent.** Anything that removes data records it
//!   under the `agent_surface` member and raises a top-level `truncated` flag.
//! * **Derived arrays never survive a reshape.** Members that merely restate
//!   the reshaped array (`memories`, `entities`, `direct_matches`,
//!   `graph_matches`, `related_memories`) are dropped and listed under
//!   `aliases_removed`. Without a knob the surface is inert, so the envelope
//!   stays byte-for-byte identical to the pre-v1.2.2 output and the v1.0.66
//!   alias contract is untouched.
//! * **NDJSON streams bypass the surface.** Line-oriented emitters keep one
//!   record per line; reshaping them would change the stream contract.
//!
//! # Scope of each knob (GAP-SG-191)
//!
//! An envelope may carry more than one array, and the three ceilings do NOT all
//! reach the same members. The split is deliberate, and it follows from what
//! each knob removes:
//!
//! | knob | reaches | why |
//! | --- | --- | --- |
//! | `--max-output-bytes` | every array | it removes whole elements to hit a byte budget the caller set for the envelope as a whole |
//! | `--max-items` | every array | same: it removes whole elements, so a secondary member simply gets the same cap |
//! | `--select`, `--filter`, `--sort`, `--dedupe-by` | primary array only | they act on the *fields* or the *ordering* of elements |
//!
//! A secondary array is a different collection, not a restatement of the primary
//! one: `graph` pairs `nodes` with `edges`. Projecting `id` over `edges` would
//! rewrite every element to `{}` and erase `source`/`target` — the projection
//! would destroy the collection rather than narrow it. Filtering and sorting
//! fail the same way, on keys that member never had.
//!
//! Until v1.2.4 `--max-items` also stopped at the primary array, so
//! `graph --select id --max-items 2` answered with two nodes and all 59 066
//! edges: 4.55 MB for a request that asked for two items. Members shortened by
//! the cap are listed under `agent_surface.secondary_capped`.
//!
//! Precedence for every numeric knob is the crate-wide one: CLI flag > XDG
//! `config set` > named constant. No product environment variable is read.

pub mod budget;
pub mod filter;
pub mod gate;
pub mod shape;
pub mod stream;
pub mod target;
pub mod universe;
pub mod vocabulary;

#[cfg(test)]
mod tests;

use crate::errors::AppError;
use filter::FilterExpr;
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

/// Resolved output-shaping request for the current process.
#[derive(Debug, Clone, Default)]
pub struct AgentSurface {
    /// Subcommand that emitted the envelope, as
    /// [`crate::cli::Commands::agent_surface_slug`] reports it.
    ///
    /// CONTEXT, never a knob: it tells alias suppression which subcommand's
    /// contract applies, and therefore takes no part in [`Self::is_noop`]. A
    /// surface carrying only a command name still changes nothing.
    pub command: Option<String>,
    /// Whether the subcommand can change durable state, as
    /// [`crate::cli::Commands::mutates`] reports it.
    ///
    /// CONTEXT, never a knob, so it takes no part in [`Self::is_noop`]. The gate
    /// reads it to stay silent after a write: refusing at output time would
    /// report failure for an operation that already succeeded.
    pub mutates: bool,
    /// Escape hatch for `--allow-unknown-keys`: tolerate a key nothing carries.
    ///
    /// CONTEXT, never a knob. On its own it changes no envelope; it only widens
    /// what the gate accepts.
    pub allow_unknown_keys: bool,
    /// What `--filter-scope` declared the predicate may observe.
    ///
    /// CONTEXT, never a knob: on its own it changes no envelope, it only tells
    /// the gate which reading the caller meant.
    pub filter_scope: Option<universe::FilterScope>,
    /// Whether `--use-active` accepted an ambient target on purpose.
    ///
    /// CONTEXT, never a knob, so it takes no part in [`Self::is_noop`]. It
    /// dispenses a mutating verb from naming its target in the argv, and
    /// [`target`] records that it was used.
    pub use_active: bool,
    /// Keys kept by `--select` / `--fields`, in the requested order.
    pub select: Vec<String>,
    /// Predicates from `--filter`, conjoined with AND.
    pub filters: Vec<FilterExpr>,
    /// Sort key from `--sort`.
    pub sort: Option<String>,
    /// Dedup key from `--dedupe-by`.
    pub dedupe_by: Option<String>,
    /// Cap on emitted result elements (`--max-items`); `0` disables it.
    pub max_items: usize,
    /// Replace the payload with a count (`--count-only`).
    pub count_only: bool,
    /// Whether the subcommand emits one self-contained record per line.
    ///
    /// GAP-SG-209: [`crate::cli::Commands::streams`] reports it, and [`gate`]
    /// refuses the knobs that would otherwise be applied once per record.
    pub streamed: bool,
    /// Whether the subcommand actually persists, so its envelope is a receipt.
    ///
    /// CONTEXT, never a knob. GAP-SG-206: [`crate::cli::Commands::persists`]
    /// reports it, and `--count-only` is suppressed rather than honoured when it
    /// is set. DISTINCT from [`Self::mutates`], which answers `true` for
    /// `config list-keys` and every other command that merely failed to make the
    /// read-only list.
    pub writes_receipt: bool,
    /// Cap on string length in characters (`--truncate-content`); `0` disables it.
    pub truncate_content: usize,
    /// Cap on the serialized envelope in bytes (`--max-output-bytes`); `0` disables it.
    pub max_output_bytes: usize,
}

impl AgentSurface {
    /// `true` when no knob is set and `apply` must be a no-op.
    pub fn is_noop(&self) -> bool {
        self.select.is_empty()
            && self.filters.is_empty()
            && self.sort.is_none()
            && self.dedupe_by.is_none()
            && self.max_items == 0
            && !self.count_only
            && self.truncate_content == 0
            && self.max_output_bytes == 0
    }
}

static SURFACE: OnceLock<AgentSurface> = OnceLock::new();

/// Installs the process-wide surface. Idempotent, first call wins.
pub fn init(surface: AgentSurface) {
    let _ = SURFACE.set(surface);
}

/// Borrows the installed surface, or an inert one when `init` never ran.
pub fn get() -> &'static AgentSurface {
    static INERT: OnceLock<AgentSurface> = OnceLock::new();
    SURFACE
        .get()
        .unwrap_or_else(|| INERT.get_or_init(AgentSurface::default))
}

/// `true` when the installed surface would change an envelope.
///
/// Callers use it to skip the extra `Value` round-trip on the hot path.
pub fn active() -> bool {
    !get().is_noop()
}

/// Applies the installed surface to `value`.
///
/// # Errors
/// Propagates the refusal raised by `apply` when the request is incoherent.
pub fn apply_global(value: Value) -> Result<Value, AppError> {
    apply(get(), value)
}

/// Member holding the record of what the surface did.
const META_KEY: &str = "agent_surface";

/// Member raised whenever data was removed.
const TRUNCATED_KEY: &str = "truncated";

/// Applies `surface` to `value`, honouring the invariants documented above.
///
/// # Errors
/// Returns [`AppError::Usage`] when the request cannot be honoured as asked —
/// see [`gate`] for the three shapes that earns.
pub fn apply(surface: &AgentSurface, value: Value) -> Result<Value, AppError> {
    // The ONE place that reads both process-wide cells. Everything downstream
    // takes them as arguments, which is what makes the whole surface testable.
    let ceiling = universe::get();
    apply_with_premises(surface, value, target::record(surface, ceiling), ceiling)
}

/// The body of `apply`, with the resolved target supplied rather than read.
///
/// The target lives in a process-wide `OnceLock`, which is correct for a
/// one-shot binary and wrong for a test binary: `paths` and `agent_surface` unit
/// tests share one process, so whether a target exists would depend on which
/// test ran first. Taking it as an argument makes every test state its own
/// premise, and keeps the production call site the only place that reads global
/// state.
///
/// # Errors
/// Returns [`AppError::Usage`] when the request cannot be honoured as asked.
pub fn apply_with_target(
    surface: &AgentSurface,
    value: Value,
    target: Option<Map<String, Value>>,
) -> Result<Value, AppError> {
    apply_with_premises(surface, value, target, universe::get())
}

/// The body of `apply`, with BOTH ambient facts supplied rather than read.
///
/// GAP-SG-201 shipped a refusal that no test could reach, and this signature is
/// why it could not. The query ceiling lives in a second process-wide `OnceLock`,
/// and `OnceLock` offers a `static` no reset at all — `take` and every `get_mut`
/// require `&mut self` — so two tests in one binary could never state different
/// ceilings. Nothing in the family had a test, the compiler was the only reader
/// left, and `dead_code` cannot see a `pub` item in a lib crate. A guard was
/// therefore written, translated, reviewed and never called.
///
/// [`apply_with_target`] keeps its own shape because the seventeen tests that
/// only care about the target should not have to state a ceiling they have no
/// opinion about.
///
/// # Errors
/// Returns [`AppError::Usage`] when the request cannot be honoured as asked.
pub fn apply_with_premises(
    surface: &AgentSurface,
    mut value: Value,
    target: Option<Map<String, Value>>,
    ceiling: Option<&universe::QueryCeiling>,
) -> Result<Value, AppError> {
    // Checked FIRST, and separately from the no-op case below: a schema document
    // is a contract rather than a result, and a failure envelope carries its own
    // target record from `crate::output::error_envelope`, so neither is
    // annotated here.
    if is_passthrough(&value) {
        return Ok(value);
    }
    if surface.is_noop() {
        // No knob is set, so nothing is reshaped — and yet the resolved target
        // is still reported. Until v1.2.6 this branch returned unconditionally,
        // which is precisely how a universal contract ended up visible only to
        // callers who had already set an unrelated flag. See [`target`].
        if let Some(meta) = target {
            attach_meta(&mut value, &meta, false);
        }
        return Ok(value);
    }

    let array_key = locate_result_array(&value);
    let aliases_removed = suppress_alias_arrays(surface, &mut value, array_key.as_deref());
    let items = take_items(&mut value, array_key.as_deref());

    // Resolution runs on the lifted elements and on what is LEFT of the
    // envelope, which is exactly the split GAP-SG-203 turns on: a key found only
    // on the remainder is a key the predicate would never have reached.
    const NO_ELEMENTS: &[Value] = &[];
    let findings = gate::evaluate(
        surface,
        &vocabulary::Scope::new(items.as_deref().unwrap_or(NO_ELEMENTS), &value)
            .with_command(surface.command.as_deref()),
        array_key.as_deref(),
        items.is_some(),
        ceiling,
    )?;

    let (payload, mut meta) = match items {
        Some(items) => shape_items(surface, value, array_key.as_deref(), items, ceiling),
        None => shape_scalar_envelope(surface, value, ceiling),
    };

    // GAP-SG-205: merged at the ONE point both shaping paths converge on, so the
    // shaped and the inert envelopes can never report different things about the
    // same process.
    if let Some(record) = target {
        meta.extend(record);
    }

    if !aliases_removed.is_empty() {
        meta.insert("aliases_removed".into(), json!(aliases_removed));
    }
    if findings.is_partial() {
        // A projection that dropped part of what was asked for says so, so a
        // caller never reads a missing field as a missing value.
        meta.insert("unresolved_keys".into(), json!(findings.unresolved_keys));
        meta.insert("resolved_keys".into(), json!(findings.resolved_keys));
        meta.insert("key_resolution".into(), json!("partial"));
        if !findings.key_suggestions.is_empty() {
            meta.insert("key_suggestions".into(), json!(findings.key_suggestions));
        }
        if findings.vocabulary_partial {
            meta.insert("vocabulary_partial".into(), Value::Bool(true));
        }
    }
    // Which member the reshaping actually acted on, and whether the CLI named it
    // or the surface guessed. A caller that asked about a top-level key and got a
    // narrowed array deserves to see that its request was redirected.
    if let Some(key) = array_key.as_deref() {
        let source = if is_declared_result_array(key) {
            ARRAY_SOURCE_DECLARED
        } else {
            ARRAY_SOURCE_FALLBACK
        };
        meta.insert("result_array_source".into(), json!(source));
    }

    Ok(finalize(surface, payload, array_key.as_deref(), meta))
}

/// Drops the derived arrays that merely restate the member being reshaped.
///
/// The surface shapes exactly one array per envelope. Keeping a clone of it
/// under another name would hand the caller an unfiltered, unsorted,
/// unprojected copy of the very rows it asked to narrow, and would blow the
/// byte ceiling for a payload that is redundant by construction. Mappings come
/// from [`crate::constants::AGENT_SURFACE_ALIAS_ARRAYS`].
///
/// A member is derived only for the subcommand that declared it so, so both the
/// subcommand and the canonical member must match. `results` is a concatenation
/// in `recall` and a clone in `related`, while in `hybrid-search` it is disjoint
/// from `graph_matches` — suppressing there deleted required data. An unknown or
/// absent subcommand suppresses nothing.
///
/// Returns the removed member names in declaration order, so `apply` can
/// record them; an empty vector means nothing was dropped. Only members that
/// are actually arrays are removed, so an envelope that reuses one of these
/// names for a scalar keeps it, and a declared derived member the envelope
/// never carried is a silent no-op that is never reported as removed.
fn suppress_alias_arrays(
    surface: &AgentSurface,
    value: &mut Value,
    array_key: Option<&str>,
) -> Vec<String> {
    let Some(canonical) = array_key else {
        return Vec::new();
    };
    let Some(command) = surface.command.as_deref() else {
        return Vec::new();
    };
    let Some((_, _, aliases)) = crate::constants::AGENT_SURFACE_ALIAS_ARRAYS
        .iter()
        .find(|(cmd, key, _)| *cmd == command && *key == canonical)
    else {
        return Vec::new();
    };
    let Some(map) = value.as_object_mut() else {
        return Vec::new();
    };
    let mut removed = Vec::new();
    for alias in *aliases {
        if map.get(*alias).is_some_and(Value::is_array) {
            map.remove(*alias);
            removed.push((*alias).to_string());
        }
    }
    removed
}

/// Envelopes that must never be reshaped.
fn is_passthrough(value: &Value) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    // A JSON Schema document is a contract, not a result set.
    if map.contains_key("$schema") {
        return true;
    }
    // Failure envelopes reach the caller intact, always.
    if map.get("error") == Some(&Value::Bool(true)) {
        return true;
    }
    map.get("ok") == Some(&Value::Bool(false))
}

/// Finds the member holding the primary result array.
///
/// Well-known names from [`crate::constants::AGENT_SURFACE_RESULT_KEYS`] are
/// tried in order; otherwise the first member that is an array wins. Returns
/// `None` when `value` is itself an array or carries no array at all.
fn locate_result_array(value: &Value) -> Option<String> {
    let map = value.as_object()?;
    for candidate in crate::constants::AGENT_SURFACE_RESULT_KEYS {
        if map.get(*candidate).is_some_and(Value::is_array) {
            return Some((*candidate).to_string());
        }
    }
    map.iter()
        .find(|(_, v)| v.is_array())
        .map(|(k, _)| k.clone())
}

/// Whether `key` is a member the CLI declared as a result set.
///
/// The distinction is load-bearing, not cosmetic. [`locate_result_array`] falls
/// back to "the first member that is an array" when no known name matches, which
/// is a guess: `stats` carries `namespaces`, so the fallback elected it and
/// `--select total_memories` — a documented, top-level key — became unresolvable.
/// An array chosen by heuristic is not a declared collection, and the gate and
/// the record both have to say so. One implementation, two readers.
fn is_declared_result_array(key: &str) -> bool {
    crate::constants::AGENT_SURFACE_RESULT_KEYS.contains(&key)
}

/// Wire spelling for an array the CLI named as its result set.
const ARRAY_SOURCE_DECLARED: &str = "declared";

/// Wire spelling for an array the surface elected for want of a declared one.
const ARRAY_SOURCE_FALLBACK: &str = "fallback";

/// Removes the result array from `value` so it can be reshaped in place.
fn take_items(value: &mut Value, array_key: Option<&str>) -> Option<Vec<Value>> {
    match array_key {
        Some(key) => match value.as_object_mut()?.get_mut(key)? {
            Value::Array(items) => Some(std::mem::take(items)),
            _ => None,
        },
        None => match value {
            Value::Array(items) => Some(std::mem::take(items)),
            _ => None,
        },
    }
}

/// Runs the array pipeline and puts the result back into the envelope.
fn shape_items(
    surface: &AgentSurface,
    mut envelope: Value,
    array_key: Option<&str>,
    items: Vec<Value>,
    ceiling: Option<&universe::QueryCeiling>,
) -> (Value, Map<String, Value>) {
    let input_count = items.len();
    // GAP-SG-274: the same slug the gate resolved keys under, so a key admitted
    // through a mode-scoped synonym is read back under that very synonym.
    let command = surface.command.as_deref();
    let mut items = shape::filter(items, &surface.filters, command);
    if let Some(key) = &surface.sort {
        items = shape::sort(items, key, command);
    }
    if let Some(key) = &surface.dedupe_by {
        items = shape::dedupe(items, key, command);
    }
    // Measured BEFORE the output ceiling, because that is the only point where
    // "how many rows satisfied the predicate" still has an answer.
    let matched_count = items.len();
    items = shape::limit(items, surface.max_items);
    items = shape::project(items, &surface.select, command);
    let output_count = items.len();

    // GAP-SG-206. A write receipt reaches HERE, not only the scalar branch:
    // `remember` carries `entities_created`, and no member of that envelope is a
    // declared result key, so the surface elects the first array by fallback and
    // counts it. Scoping the guard to the scalar branch therefore protected
    // nothing — the receipt this exists to save took this path all along.
    if surface.count_only && !surface.writes_receipt {
        let mut meta = base_meta(surface, input_count, output_count, ceiling);
        meta.insert("count_only".into(), Value::Bool(true));
        meta.insert(
            "count_scope".into(),
            json!(universe::count_scope(output_count, matched_count, ceiling)),
        );
        return (json!({ "count": output_count }), meta);
    }

    // Applied while the primary member still holds the emptied array left by
    // `take_items`, so the loop below cannot reach it: its length is zero and
    // it is neither truncated nor reported.
    let secondary_capped = cap_secondary_arrays(&mut envelope, surface.max_items);

    match array_key {
        Some(key) => {
            if let Some(map) = envelope.as_object_mut() {
                map.insert(key.to_string(), Value::Array(items));
            }
        }
        None => envelope = Value::Array(items),
    }
    let mut meta = base_meta(surface, input_count, output_count, ceiling);
    if surface.count_only {
        // Reached only through the receipt branch above. Named so a caller that
        // asked for a count and received an envelope learns which of the two
        // happened instead of reading the full payload as a bug.
        meta.insert("count_only_suppressed".into(), Value::Bool(true));
    }
    if !secondary_capped.is_empty() {
        meta.insert("secondary_capped".into(), json!(secondary_capped));
    }
    (envelope, meta)
}

/// Applies `--max-items` to every array member other than the primary one.
///
/// GAP-SG-191: the cap used to bind the primary array alone, so
/// `graph --select id --max-items 2` answered with two nodes and all 59 066
/// edges — 4.55 MiB for a request that asked for two items. `--max-output-bytes`
/// already reached these members; `--max-items` did not, and nothing documented
/// the asymmetry.
///
/// `--select` deliberately does NOT follow: a secondary array holds a different
/// collection, not a restatement of the primary one, so projecting `id` over
/// `edges` would rewrite every element to `{}` and erase `source`/`target`
/// instead of shrinking them. Capping is safe because it removes whole
/// elements, never fields inside one.
///
/// Returns the member names that were actually shortened, in ascending key
/// order.
///
/// NOT insertion order, and the difference is observable: `serde_json` is built
/// here without the `preserve_order` feature, so a [`Map`] is a `BTreeMap` and
/// iteration follows the key's `Ord`. Saying "envelope order" invited a caller
/// to read a position that never encoded anything. Ascending order is also what
/// makes this list identical on Linux, macOS and Windows.
fn cap_secondary_arrays(envelope: &mut Value, max_items: usize) -> Vec<String> {
    if max_items == 0 {
        return Vec::new();
    }
    let Some(map) = envelope.as_object_mut() else {
        return Vec::new();
    };
    let mut capped = Vec::new();
    for (key, value) in map.iter_mut() {
        if let Value::Array(items) = value {
            if items.len() > max_items {
                items.truncate(max_items);
                capped.push(key.clone());
            }
        }
    }
    capped
}

/// Handles envelopes with no result array: projection applies to the object.
///
/// # GAP-SG-206: a count must not eat a write receipt
///
/// [`gate`] deliberately refuses nothing after a write, because reporting failure
/// for a succeeded `remember` makes a retrying caller write twice. That fence
/// stops the surface from FAILING the command; nothing stopped it from emptying
/// the answer — `--count-only` replaced the envelope with a number, discarding
/// `memory_id`, `entities_created` and `enrich_recommended`, the receipt callers
/// are told to parse, for an operation that cannot be replayed.
///
/// The guard turns on [`AgentSurface::writes_receipt`] and NOT on
/// [`AgentSurface::mutates`]. The second reports `true` for `config list-keys`,
/// which writes nothing but merely failed to make the read-only list, and keying
/// the suppression there took a working answer away — the integration suite
/// caught it. The first is [`crate::cli::Commands::persists`], the same question
/// the write policy already asks about naming a target.
fn shape_scalar_envelope(
    surface: &AgentSurface,
    envelope: Value,
    ceiling: Option<&universe::QueryCeiling>,
) -> (Value, Map<String, Value>) {
    if surface.count_only && !surface.writes_receipt {
        let mut meta = base_meta(surface, 1, 1, ceiling);
        meta.insert("count_only".into(), Value::Bool(true));
        // An envelope with no result array is one thing, and no ceiling can make
        // it fewer, so the count always describes what matched.
        meta.insert("count_scope".into(), json!(universe::COUNT_SCOPE_SCALAR));
        return (json!({ "count": 1 }), meta);
    }
    let projected = shape::project_one(envelope, &surface.select, surface.command.as_deref());
    let mut meta = base_meta(surface, 1, 1, ceiling);
    if surface.count_only {
        meta.insert("count_only_suppressed".into(), Value::Bool(true));
    }
    (projected, meta)
}

/// Builds the `agent_surface` record shared by both shaping paths.
fn base_meta(
    surface: &AgentSurface,
    input: usize,
    output: usize,
    ceiling: Option<&universe::QueryCeiling>,
) -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert("input_count".into(), json!(input));
    meta.insert("output_count".into(), json!(output));
    if !surface.select.is_empty() {
        meta.insert("select".into(), json!(surface.select));
    }
    if !surface.filters.is_empty() {
        meta.insert("filters".into(), json!(surface.filters.len()));
    }
    if let Some(key) = &surface.sort {
        meta.insert("sort".into(), json!(key));
    }
    if let Some(key) = &surface.dedupe_by {
        meta.insert("dedupe_by".into(), json!(key));
    }
    if surface.max_items > 0 {
        meta.insert("max_items".into(), json!(surface.max_items));
    }
    // GAP-SG-205 is deliberately NOT here: the target is merged by
    // [`apply_with_target`], which is the one point the shaped and the inert
    // paths share. Attaching it inside this function is what made it invisible
    // whenever the surface was inert.
    universe::insert_query_ceiling(&mut meta, ceiling);
    meta
}

/// Applies the content and byte ceilings, then attaches the record.
fn finalize(
    surface: &AgentSurface,
    mut payload: Value,
    array_key: Option<&str>,
    mut meta: Map<String, Value>,
) -> Value {
    let content_truncated = shape::truncate_strings(&mut payload, surface.truncate_content);
    if content_truncated {
        meta.insert("content_truncated".into(), Value::Bool(true));
        meta.insert("truncate_content".into(), json!(surface.truncate_content));
    }

    attach_meta(&mut payload, &meta, content_truncated);

    // Recording the ceiling's verdict makes the envelope grow, so the ceiling
    // has to be enforced against a budget that already accounts for the
    // record. Enforcing first and annotating afterwards would either exceed
    // the ceiling or force a second pass that collapses the envelope into the
    // stub purely because of its own annotation.
    let headroom = budget_headroom(surface, &payload, &meta);
    let effective_max = match surface.max_output_bytes {
        0 => 0,
        max => max.saturating_sub(headroom).max(1),
    };

    let outcome = budget::enforce(&mut payload, array_key, effective_max);
    if outcome.truncated && !outcome.stub {
        meta.insert("output_truncated".into(), Value::Bool(true));
        meta.insert("dropped".into(), json!(outcome.dropped));
        meta.insert("max_output_bytes".into(), json!(surface.max_output_bytes));
        // `output_count` was measured by the shaping stage, before the ceiling
        // existed. Left alone it reports the pre-budget length, so a caller
        // reading `output_count: 30` beside eleven elements concludes its own
        // parse lost nineteen. Re-measuring is safe for the reservation above:
        // the surviving length can only be smaller than the shaped one, so its
        // decimal form never grows and the headroom can never fall short.
        if let Some(surviving) = surviving_len(&payload, array_key) {
            meta.insert("output_count".into(), json!(surviving));
        }
        attach_meta(&mut payload, &meta, true);
    }
    if outcome.stub {
        // The stub is built inside `budget::enforce`, which only knows the
        // budget it was handed — `effective_max`, already reduced by the
        // headroom above. Reporting that number told a caller who asked for 400
        // that the ceiling was 340, a figure it never chose and cannot act on.
        // The non-stub branch above always reported the requested value; this
        // aligns the two.
        if let Some(map) = payload.as_object_mut() {
            map.insert("max_output_bytes".into(), json!(surface.max_output_bytes));
        }
    }
    payload
}

/// Length of the result array as it stands after the ceiling was enforced.
///
/// Returns `None` when the payload no longer carries an array, which is the
/// stub path; callers guard on that before asking.
fn surviving_len(payload: &Value, array_key: Option<&str>) -> Option<usize> {
    match array_key {
        Some(key) => payload.get(key)?.as_array().map(Vec::len),
        None => payload.as_array().map(Vec::len),
    }
}

/// Bytes the budget record will add to the envelope once the ceiling fires.
///
/// Measured rather than guessed: the members are inserted into a throwaway copy
/// of the record and the two serializations are compared. `dropped` is measured
/// at its widest possible value, so the reservation can never fall short.
fn budget_headroom(surface: &AgentSurface, payload: &Value, meta: &Map<String, Value>) -> usize {
    if surface.max_output_bytes == 0 {
        return 0;
    }
    let widest_dropped = meta
        .get("input_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut annotated = meta.clone();
    annotated.insert("output_truncated".into(), Value::Bool(true));
    annotated.insert("dropped".into(), json!(widest_dropped));
    annotated.insert("max_output_bytes".into(), json!(surface.max_output_bytes));

    let before = encoded_len(&Value::Object(meta.clone()));
    let after = encoded_len(&Value::Object(annotated));
    let mut extra = after.saturating_sub(before);

    if payload
        .as_object()
        .is_some_and(|map| !map.contains_key(TRUNCATED_KEY))
    {
        // `,"truncated":true`
        extra += TRUNCATED_KEY.len() + r#","":true"#.len();
    }
    extra
}

/// Compact serialized length, or `0` when the value cannot be serialized.
fn encoded_len(value: &Value) -> usize {
    serde_json::to_string(value).map_or(0, |s| s.len())
}

/// Writes the record into an object envelope, raising `truncated` when needed.
///
/// Array envelopes have nowhere to carry the record; the shaping still applied,
/// it is simply not annotated.
///
/// The flag is raised even over an existing `false`. Until v1.2.6 the write was
/// guarded by `!map.contains_key`, which sounds conservative and was not: `list`
/// serializes `truncated` unconditionally (`ListResponse` declares it as a plain
/// `bool`), so the member was ALWAYS present and the surface could never raise
/// it on the most used command in the binary. The module has always promised
/// that removing data is never silent; that promise was quietly false wherever
/// the command shipped its own flag.
///
/// Raising it is also monotonic — `true` is never written back to `false` — so
/// a command that already truncated its own rows keeps saying so.
fn attach_meta(payload: &mut Value, meta: &Map<String, Value>, truncated: bool) {
    let Some(map) = payload.as_object_mut() else {
        return;
    };
    map.insert(META_KEY.to_string(), Value::Object(meta.clone()));
    if truncated {
        map.insert(TRUNCATED_KEY.to_string(), Value::Bool(true));
    }
}
