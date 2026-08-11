//! Filter expression grammar for `--filter`.
//!
//! Three operators are accepted, deliberately kept small so an agent can emit
//! them without a parser of its own:
//!
//! | Form              | Meaning                                            |
//! |-------------------|----------------------------------------------------|
//! | `key=value`       | scalar at `key` equals `value` (case sensitive)    |
//! | `key!=value`      | scalar at `key` differs from `value`               |
//! | `key~substring`   | scalar at `key` contains `substring` (case folded) |
//!
//! `==` is accepted as a synonym of `=` for parity with sibling CLIs. `key`
//! may be a dotted path (`stats.total`) to reach a nested scalar. Repeating
//! `--filter` conjoins the predicates with a logical AND.
//!
//! Parsing is fail-fast: a malformed expression is rejected before any work is
//! done, so a typo can never be mistaken for "no rows matched".

use serde_json::Value;

/// Comparison performed by a single [`FilterExpr`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    /// `key=value` / `key==value`.
    Equals,
    /// `key!=value`.
    NotEquals,
    /// `key~substring`, case-insensitive substring test.
    Contains,
}

/// One parsed `--filter` predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterExpr {
    /// Dotted path segments addressing a scalar inside each element.
    path: Vec<String>,
    /// Comparison to perform.
    op: FilterOp,
    /// Right-hand side, compared against the scalar rendered as text.
    value: String,
}

/// Separator candidates ordered so the longest token wins at a given offset.
const SEPARATORS: &[(&str, FilterOp)] = &[
    ("!=", FilterOp::NotEquals),
    ("==", FilterOp::Equals),
    ("~", FilterOp::Contains),
    ("=", FilterOp::Equals),
];

impl FilterExpr {
    /// Parses one `--filter` expression.
    ///
    /// # Errors
    /// Returns a localized message when no operator is present or when the
    /// key side is empty.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut best: Option<(usize, usize, FilterOp)> = None;
        for (token, op) in SEPARATORS {
            if let Some(idx) = raw.find(token) {
                let better = match best {
                    None => true,
                    // Earliest position wins; at the same position the longer
                    // token wins so `!=` is never read as `=`.
                    Some((cur_idx, cur_len, _)) => {
                        idx < cur_idx || (idx == cur_idx && token.len() > cur_len)
                    }
                };
                if better {
                    best = Some((idx, token.len(), *op));
                }
            }
        }
        let Some((idx, len, op)) = best else {
            return Err(crate::i18n::validation::agent_surface_filter_invalid(raw));
        };
        let key = raw[..idx].trim();
        if key.is_empty() {
            return Err(crate::i18n::validation::agent_surface_filter_empty_key(raw));
        }
        let value = &raw[idx + len..];
        Ok(Self {
            path: key.split('.').map(str::to_string).collect(),
            op,
            value: value.to_string(),
        })
    }

    /// The dotted path this predicate addresses, as the caller wrote it.
    ///
    /// The gate needs the key to resolve it against the envelope vocabulary and
    /// to name it in a refusal; rebuilding the string from [`Self::path`] at
    /// every call site would duplicate the join.
    pub fn key(&self) -> String {
        self.path.join(".")
    }

    /// The parsed path segments, for callers that resolve rather than compare.
    pub fn path(&self) -> &[String] {
        &self.path
    }

    /// Evaluates the predicate against one element.
    ///
    /// A missing or non-scalar path never satisfies [`FilterOp::Equals`] or
    /// [`FilterOp::Contains`]; it *does* satisfy [`FilterOp::NotEquals`],
    /// which reads as "this element does not carry that value".
    pub fn matches(&self, element: &Value) -> bool {
        let scalar = lookup(element, &self.path).and_then(scalar_text);
        match (self.op, scalar) {
            (FilterOp::Equals, Some(text)) => text == self.value,
            (FilterOp::Equals, None) => false,
            (FilterOp::NotEquals, Some(text)) => text != self.value,
            (FilterOp::NotEquals, None) => true,
            (FilterOp::Contains, Some(text)) => {
                text.to_lowercase().contains(&self.value.to_lowercase())
            }
            (FilterOp::Contains, None) => false,
        }
    }
}

/// Walks a dotted path inside `value`.
pub fn lookup<'a>(value: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.as_object()?.get(segment.as_str())?;
    }
    Some(cursor)
}

/// Walks a dotted path given as one unsplit string.
///
/// The sibling of [`lookup`], for callers that hold the key as the caller wrote
/// it rather than pre-split. [`FilterExpr`] splits once at parse time because it
/// then walks the path for every element; the gate resolves a key against many
/// elements too, but it holds the key as text and splitting it per call would
/// allocate a `Vec<String>` the walk never needs.
pub fn resolve<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let mut cursor = value;
    for segment in key.split('.') {
        cursor = cursor.as_object()?.get(segment)?;
    }
    Some(cursor)
}

/// Renders a JSON scalar as the text used for comparison and dedup keys.
///
/// Containers return `None`: comparing an array to a string would only produce
/// surprising matches.
pub fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some(String::new()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

/// Returns `true` when every predicate accepts `element`.
pub fn matches_all(filters: &[FilterExpr], element: &Value) -> bool {
    filters.iter().all(|f| f.matches(element))
}
