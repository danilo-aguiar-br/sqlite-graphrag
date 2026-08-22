//! Stateless reshaping primitives applied to the result array of an envelope.
//!
//! Every function here takes and returns plain `serde_json` data so a single
//! call site in [`crate::output`] can serve every subcommand. None of them
//! knows what a memory, an entity or a hit is.
//!
//! GAP-SG-274: the one thing they do carry is the subcommand slug, threaded
//! through as `command` and used for nothing but scoping the field-synonym
//! table. It is not knowledge of the domain — it is the same answer the gate
//! used when it admitted the key, and passing it here is what keeps a key that
//! was accepted from silently matching nothing.

use super::filter::{lookup, scalar_text, FilterExpr};
use serde_json::{Map, Value};

/// Keeps only the elements accepted by every predicate.
pub fn filter(items: Vec<Value>, filters: &[FilterExpr], command: Option<&str>) -> Vec<Value> {
    if filters.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|item| super::filter::matches_all(filters, item, command))
        .collect()
}

/// Sorts elements ascending by the scalar found at the dotted `key`.
///
/// Numeric values compare numerically, everything else compares as text.
/// Elements without the key keep their relative order at the end of the list,
/// so a partially populated payload never loses rows to sorting.
pub fn sort(mut items: Vec<Value>, key: &str, command: Option<&str>) -> Vec<Value> {
    let path: Vec<String> = key.split('.').map(str::to_string).collect();
    items.sort_by(|a, b| {
        let left = lookup(a, &path, command);
        let right = lookup(b, &path, command);
        match (left, right) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(l), Some(r)) => compare(l, r),
        }
    });
    items
}

/// Total order over two JSON scalars used by [`sort`].
fn compare(left: &Value, right: &Value) -> std::cmp::Ordering {
    if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
        return l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal);
    }
    match (scalar_text(left), scalar_text(right)) {
        (Some(l), Some(r)) => l.cmp(&r),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Drops later elements whose scalar at `key` was already seen.
///
/// Elements lacking the key are always kept: dropping them would silently
/// collapse rows that were never proven duplicate.
pub fn dedupe(items: Vec<Value>, key: &str, command: Option<&str>) -> Vec<Value> {
    let path: Vec<String> = key.split('.').map(str::to_string).collect();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match lookup(&item, &path, command).and_then(scalar_text) {
            Some(text) => {
                if seen.insert(text) {
                    out.push(item);
                }
            }
            None => out.push(item),
        }
    }
    out
}

/// Truncates the list to at most `max` elements. `0` means "no cap".
pub fn limit(mut items: Vec<Value>, max: usize) -> Vec<Value> {
    if max > 0 && items.len() > max {
        items.truncate(max);
    }
    items
}

/// Rewrites each object element to carry only `keys`, in the requested order.
///
/// Keys absent from an element are skipped rather than emitted as `null`, so a
/// projection never invents fields. Non-object elements pass through unchanged.
/// The dotted paths are split ONCE here rather than per element.
///
/// Splitting inside the per-element loop allocated a `Vec<String>`, plus a
/// `String` per segment, for every element times every key — and discarded them
/// immediately. `graph entities --select name` over the measured corpus of
/// 107 135 entities did that 107 135 times to answer with one field. [`sort`] and
/// [`dedupe`] in this same file already hoist the split out of their loops, so
/// this is the file's own established shape rather than a new idea.
pub fn project(items: Vec<Value>, keys: &[String], command: Option<&str>) -> Vec<Value> {
    if keys.is_empty() {
        return items;
    }
    let paths = compile_paths(keys);
    items
        .into_iter()
        .map(|item| project_with(item, keys, &paths, command))
        .collect()
}

/// Splits every dotted key into its segments.
///
/// Visible to the whole surface because [`super::stream`] compiles ONCE when the
/// stream opens and reuses the result for every line. A stream has no `Vec` to
/// hoist the work out of, so the hoisting has to live in the caller.
pub(super) fn compile_paths(keys: &[String]) -> Vec<Vec<String>> {
    keys.iter()
        .map(|key| key.split('.').map(str::to_string).collect())
        .collect()
}

/// Projects one value against paths that were already compiled.
///
/// `keys` and `paths` are parallel by construction: both come from the same
/// slice, in the same order, and `keys` supplies the OUTPUT name while `paths`
/// supplies the lookup. They are not interchangeable — the output key keeps its
/// dotted spelling so `--select a.b` answers under `"a.b"` and not under `"b"`.
pub(super) fn project_with(
    item: Value,
    keys: &[String],
    paths: &[Vec<String>],
    command: Option<&str>,
) -> Value {
    let Value::Object(_) = &item else {
        return item;
    };
    let mut out = Map::new();
    for (key, path) in keys.iter().zip(paths) {
        if let Some(found) = lookup(&item, path, command) {
            out.insert(key.clone(), found.clone());
        }
    }
    Value::Object(out)
}

/// Projects a single value; see [`project`].
///
/// Compiles the paths for one value, which is the right trade for the scalar
/// envelope this serves: there is no loop to hoist the work out of.
pub fn project_one(item: Value, keys: &[String], command: Option<&str>) -> Value {
    if keys.is_empty() {
        return item;
    }
    project_with(item, keys, &compile_paths(keys), command)
}

/// Shortens every string longer than `max` characters, recursively.
///
/// Returns `true` when at least one string was cut, so the caller can flag the
/// envelope. Truncation counts characters, not bytes, and therefore never
/// splits a UTF-8 sequence.
pub fn truncate_strings(value: &mut Value, max: usize) -> bool {
    if max == 0 {
        return false;
    }
    match value {
        Value::String(s) => {
            if s.chars().count() > max {
                let cut = s
                    .char_indices()
                    .nth(max)
                    .map_or(s.len(), |(byte_idx, _)| byte_idx);
                s.truncate(cut);
                true
            } else {
                false
            }
        }
        Value::Array(items) => {
            let mut hit = false;
            for item in items {
                hit |= truncate_strings(item, max);
            }
            hit
        }
        Value::Object(map) => {
            let mut hit = false;
            for (_, item) in map.iter_mut() {
                hit |= truncate_strings(item, max);
            }
            hit
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}
