//! Stateless reshaping primitives applied to the result array of an envelope.
//!
//! Every function here takes and returns plain `serde_json` data so a single
//! call site in [`crate::output`] can serve every subcommand. None of them
//! knows what a memory, an entity or a hit is.

use super::filter::{lookup, scalar_text, FilterExpr};
use serde_json::{Map, Value};

/// Keeps only the elements accepted by every predicate.
pub fn filter(items: Vec<Value>, filters: &[FilterExpr]) -> Vec<Value> {
    if filters.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|item| super::filter::matches_all(filters, item))
        .collect()
}

/// Sorts elements ascending by the scalar found at the dotted `key`.
///
/// Numeric values compare numerically, everything else compares as text.
/// Elements without the key keep their relative order at the end of the list,
/// so a partially populated payload never loses rows to sorting.
pub fn sort(mut items: Vec<Value>, key: &str) -> Vec<Value> {
    let path: Vec<String> = key.split('.').map(str::to_string).collect();
    items.sort_by(|a, b| {
        let left = lookup(a, &path);
        let right = lookup(b, &path);
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
pub fn dedupe(items: Vec<Value>, key: &str) -> Vec<Value> {
    let path: Vec<String> = key.split('.').map(str::to_string).collect();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match lookup(&item, &path).and_then(scalar_text) {
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
pub fn project(items: Vec<Value>, keys: &[String]) -> Vec<Value> {
    if keys.is_empty() {
        return items;
    }
    items
        .into_iter()
        .map(|item| project_one(item, keys))
        .collect()
}

/// Projects a single value; see [`project`].
pub fn project_one(item: Value, keys: &[String]) -> Value {
    if keys.is_empty() {
        return item;
    }
    let Value::Object(_) = &item else {
        return item;
    };
    let mut out = Map::new();
    for key in keys {
        let path: Vec<String> = key.split('.').map(str::to_string).collect();
        if let Some(found) = lookup(&item, &path) {
            out.insert(key.clone(), found.clone());
        }
    }
    Value::Object(out)
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
