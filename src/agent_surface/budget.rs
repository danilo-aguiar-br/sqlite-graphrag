//! Hard ceiling on the serialized size of one envelope (`--max-output-bytes`).
//!
//! Agents pay for every byte they read back. The ceiling is enforced by
//! dropping trailing elements of the result array until the compact
//! serialization fits, never by cutting the JSON text — a byte-sliced envelope
//! would not parse.
//!
//! When even the envelope without any element exceeds the ceiling, the payload
//! is replaced by a small, always-parseable stub that says so.

use serde_json::{json, Value};

/// What the ceiling did to an envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BudgetOutcome {
    /// `true` when at least one element was dropped or the stub was emitted.
    pub truncated: bool,
    /// Number of result elements removed to fit the ceiling.
    pub dropped: usize,
    /// `true` when the envelope itself did not fit and was replaced.
    pub stub: bool,
}

/// Compact serialized length of `value`, or `usize::MAX` when it cannot be
/// serialized (treated as "does not fit" so the ceiling degrades safely).
fn encoded_len(value: &Value) -> usize {
    serde_json::to_string(value).map_or(usize::MAX, |s| s.len())
}

/// Borrows the result array addressed by `array_key`.
///
/// `None` selects `value` itself when it is an array.
fn array_mut<'a>(value: &'a mut Value, array_key: Option<&str>) -> Option<&'a mut Vec<Value>> {
    match array_key {
        None => value.as_array_mut(),
        Some(key) => value.as_object_mut()?.get_mut(key)?.as_array_mut(),
    }
}

/// Names every array member of `value` other than the one held by `array_key`.
///
/// GAP-SG-171: an envelope may carry arrays the surface does not reshape and
/// that are not aliases either — `graph` pairs `nodes` with `edges`, which is a
/// different collection, not a restatement of the same one. Emptying only the
/// canonical array then left `edges` alone, the envelope stayed over budget, and
/// the stub replaced everything. Trimming these before giving up keeps the
/// caller with data instead of a placeholder.
fn secondary_array_keys(value: &Value, array_key: Option<&str>) -> Vec<String> {
    let Some(map) = value.as_object() else {
        return Vec::new();
    };
    map.iter()
        .filter(|(k, v)| v.is_array() && Some(k.as_str()) != array_key)
        .map(|(k, _)| k.clone())
        .collect()
}

/// Lifts the result array out of `value`, leaving an empty array in its place.
///
/// Moving instead of copying is what lets [`enforce`] measure the envelope
/// without the elements while still owning them.
fn take_array(value: &mut Value, array_key: Option<&str>) -> Option<Vec<Value>> {
    array_mut(value, array_key).map(std::mem::take)
}

/// Puts `items` back where [`take_array`] found them.
///
/// A missing slot means the caller reshaped the envelope in between, in which
/// case the elements are dropped rather than reattached somewhere they do not
/// belong.
fn put_array(value: &mut Value, array_key: Option<&str>, items: Vec<Value>) {
    if let Some(slot) = array_mut(value, array_key) {
        *slot = items;
    }
}

/// Empties the secondary arrays of `value`, returning how many entries went.
fn drain_secondary_arrays(value: &mut Value, keys: &[String]) -> usize {
    let Some(map) = value.as_object_mut() else {
        return 0;
    };
    let mut dropped = 0usize;
    for key in keys {
        if let Some(Value::Array(items)) = map.get_mut(key) {
            dropped += items.len();
            items.clear();
        }
    }
    dropped
}

/// Shrinks `value` until its compact form is at most `max` bytes.
///
/// `max == 0` disables the ceiling. Returns what had to be sacrificed so the
/// caller can record it in the envelope; truncation is never silent.
pub fn enforce(value: &mut Value, array_key: Option<&str>, max: usize) -> BudgetOutcome {
    if max == 0 || encoded_len(value) <= max {
        return BudgetOutcome::default();
    }

    // Lifting the elements out first serves both steps below: the envelope left
    // behind IS the "primary array emptied" probe the secondary check needs, and
    // it is the skeleton the prefix scan measures against. Until v1.2.4 each of
    // those was a full `value.clone()`, and the search cloned once per
    // iteration — on a 7.6 MB envelope with 59 066 edges that was ~16 whole-tree
    // copies, over 100 MB of churn, to answer how many elements fit.
    let mut items = take_array(value, array_key).unwrap_or_default();
    let original_len = items.len();

    let secondary = secondary_array_keys(value, array_key);
    let mut dropped_secondary = 0usize;
    if !secondary.is_empty() && encoded_len(value) > max {
        dropped_secondary = drain_secondary_arrays(value, &secondary);
    }

    if original_len > 0 {
        // A compact JSON array is `[`, the elements, `,` between each, `]`. The
        // brackets are already inside `skeleton` because the emptied array
        // serialized as `[]`, so a prefix of k elements costs the sum of their
        // own lengths plus k-1 separators. Each element is serialized exactly
        // once and the running sum is monotonic, so the longest fitting prefix
        // falls out of a single forward scan — no probe, no clone.
        let skeleton = encoded_len(value);
        let mut used = 0usize;
        let mut best = 0usize;
        for (index, item) in items.iter().enumerate() {
            let separator = usize::from(index > 0);
            let cost = encoded_len(item).saturating_add(separator);
            if skeleton.saturating_add(used).saturating_add(cost) > max {
                break;
            }
            used = used.saturating_add(cost);
            best = index + 1;
        }
        if best > 0 {
            items.truncate(best);
            put_array(value, array_key, items);
            return BudgetOutcome {
                truncated: true,
                dropped: original_len - best + dropped_secondary,
                stub: false,
            };
        }
    }

    // Every array is empty by now. If the scaffolding alone fits, the caller
    // still gets a parseable envelope with its scalar fields intact, which is
    // strictly more than the stub would leave.
    if encoded_len(value) <= max {
        return BudgetOutcome {
            truncated: true,
            dropped: original_len + dropped_secondary,
            stub: false,
        };
    }

    *value = json!({
        "truncated": true,
        "truncated_reason": "max_output_bytes",
        "max_output_bytes": max,
    });
    BudgetOutcome {
        truncated: true,
        dropped: original_len + dropped_secondary,
        stub: true,
    }
}
