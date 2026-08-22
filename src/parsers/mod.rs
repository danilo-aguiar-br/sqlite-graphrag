//! Input format parsers (timestamp, range validators).

use chrono::DateTime;
use unicode_normalization::UnicodeNormalization;

/// Accepts a Unix epoch (integer >= 0) or RFC 3339 timestamp and returns the Unix epoch.
pub fn parse_expected_updated_at(s: &str) -> Result<i64, String> {
    if let Ok(secs) = s.parse::<i64>() {
        if secs >= 0 {
            return Ok(secs);
        }
    }
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp())
        .map_err(|e| {
            format!(
                "value must be a Unix epoch (integer >= 0) or RFC 3339 (e.g. 2026-04-19T12:00:00Z): {e}"
            )
        })
}

/// Shared range check behind every numeric read-path argument.
///
/// Until v1.2.7 this logic existed once, for `-k`, and the other twelve numeric
/// arguments had no validator at all. `related --limit` was the sharp end of
/// that: it drove `Vec::with_capacity(limit)` before any data could bound it,
/// so an absurd value aborted the process on allocation instead of returning
/// the exit code this crate reserves for memory pressure. Every ceiling now
/// lives in `crate::constants` and every public parser below is one line, so a
/// new bounded argument costs a wrapper rather than a copy of this function.
fn parse_usize_in_range(s: &str, lo: usize, hi: usize) -> Result<usize, String> {
    let value: usize = s
        .parse()
        .map_err(|_| format!("'{s}' is not a valid non-negative integer"))?;
    if !(lo..=hi).contains(&value) {
        // The argument is not named here on purpose: Clap already prefixes the
        // message with `invalid value '...' for '--limit <LIMIT>'`, so naming a
        // field would contradict that prefix wherever a parser is shared.
        return Err(format!(
            "must be between {lo} and {hi} (inclusive); got {value}"
        ));
    }
    Ok(value)
}

/// Validates `-k`/`--k` on every retrieval command.
///
/// The upper bound matches the historical `sqlite-vec` knn limit; values above
/// it used to surface a leaky engine error such as `k value in knn query too
/// large, provided 10000 and the limit is 4096`. Validating at parse time turns
/// the failure into a clean Clap error before any database work.
pub fn parse_k_range(s: &str) -> Result<usize, String> {
    parse_usize_in_range(s, 1, crate::constants::K_QUERY_RANGE_MAX)
}

/// Validates `--limit` on the commands that page over stored rows.
///
/// A looser ceiling than [`parse_k_range`] because `export --limit` ships a
/// default of 100_000. These values reach SQLite as a `LIMIT` clause, where the
/// row count already bounds the work, so the check rejects absurd input rather
/// than guarding memory.
pub fn parse_list_limit_range(s: &str) -> Result<usize, String> {
    parse_usize_in_range(s, 1, crate::constants::K_LIST_LIMIT_MAX)
}

/// Validates `--max-hops` and `--depth` where the argument is a `usize`.
pub fn parse_hops_range_usize(s: &str) -> Result<usize, String> {
    parse_usize_in_range(s, 1, crate::constants::K_MAX_HOPS_CEILING as usize)
}

/// Validates `--max-hops` and `--depth` where the argument is a `u32`.
pub fn parse_hops_range_u32(s: &str) -> Result<u32, String> {
    parse_usize_in_range(s, 1, crate::constants::K_MAX_HOPS_CEILING as usize).map(|v| v as u32)
}

/// Validates `enrich --quality-sample`.
///
/// Zero is ADMITTED and is not a degenerate case: `status.rs` treats
/// `sample_n == 0` as "skip the quality sample entirely", so the lower bound of
/// the shared [`parse_k_range`] would reject a documented, meaningful value.
///
/// The upper bound guards memory rather than the engine.
/// `quality_sample::sample_entity_description_quality` sizes a `Vec<f64>` from
/// this number before a single row is read, so an absurd value aborted the
/// process on allocation instead of returning the exit code this crate reserves
/// for memory pressure — the same failure `related --limit` had under
/// GAP-SG-213, in a field that gate could not see because it is declared as
/// `Option<usize>` rather than `usize`.
pub fn parse_quality_sample_range(s: &str) -> Result<usize, String> {
    parse_usize_in_range(s, 0, crate::constants::K_QUERY_RANGE_MAX)
}

/// Validates `deep-research --max-sub-queries`.
///
/// This ceiling guards spend rather than memory: every sub-query is a separate
/// REST round trip, so an unbounded value bills an unbounded fan-out.
pub fn parse_sub_queries_range(s: &str) -> Result<usize, String> {
    parse_usize_in_range(s, 1, crate::constants::K_MAX_SUB_QUERIES_CEILING)
}

/// Flexible boolean parser for Clap env var integration.
///
/// Accepts common truthy/falsy conventions used in shell environments:
/// truthy: `1`, `true`, `yes`, `on` (case-insensitive)
/// falsy: `0`, `false`, `no`, `off`, empty string (case-insensitive)
pub fn parse_bool_flexible(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" | "" => Ok(false),
        _ => Err(format!(
            "invalid boolean value '{s}': expected true/false/1/0/yes/no/on/off"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_unix_epoch() {
        assert_eq!(parse_expected_updated_at("1700000000").unwrap(), 1700000000);
    }

    #[test]
    fn accepts_zero() {
        assert_eq!(parse_expected_updated_at("0").unwrap(), 0);
    }

    #[test]
    fn accepts_rfc_3339_utc() {
        let result = parse_expected_updated_at("2020-01-01T00:00:00Z");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1577836800);
    }

    #[test]
    fn accepts_rfc_3339_with_offset() {
        let result = parse_expected_updated_at("2026-04-19T12:00:00+00:00");
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_invalid_string() {
        assert!(parse_expected_updated_at("bananas").is_err());
    }

    #[test]
    fn rejects_negative() {
        let err = parse_expected_updated_at("-1");
        assert!(err.is_err());
    }

    #[test]
    fn error_message_mentions_format() {
        let msg = parse_expected_updated_at("invalid").unwrap_err();
        assert!(msg.contains("RFC 3339") || msg.contains("Unix epoch"));
    }

    #[test]
    fn k_accepts_valid_range_endpoints() {
        assert_eq!(parse_k_range("1").unwrap(), 1);
        assert_eq!(parse_k_range("4096").unwrap(), 4096);
        assert_eq!(parse_k_range("10").unwrap(), 10);
    }

    #[test]
    fn k_rejects_zero() {
        let msg = parse_k_range("0").unwrap_err();
        assert!(msg.contains("between 1 and 4096"));
    }

    #[test]
    fn k_rejects_above_limit() {
        let msg = parse_k_range("10000").unwrap_err();
        assert!(msg.contains("between 1 and 4096"));
    }

    #[test]
    fn k_rejects_non_integer() {
        let msg = parse_k_range("abc").unwrap_err();
        assert!(msg.contains("not a valid"));
    }

    #[test]
    fn k_rejects_negative() {
        // usize parser fails on negatives before range check
        assert!(parse_k_range("-5").is_err());
    }

    #[test]
    fn bool_flexible_truthy() {
        for v in &["1", "true", "True", "TRUE", "yes", "Yes", "on", "ON"] {
            assert!(parse_bool_flexible(v).unwrap(), "should be true: {v}");
        }
    }

    #[test]
    fn bool_flexible_falsy() {
        for v in &["0", "false", "False", "FALSE", "no", "No", "off", "OFF", ""] {
            assert!(!parse_bool_flexible(v).unwrap(), "should be false: {v}");
        }
    }

    #[test]
    fn bool_flexible_rejects_invalid() {
        assert!(parse_bool_flexible("banana").is_err());
        assert!(parse_bool_flexible("2").is_err());
        assert!(parse_bool_flexible("nope").is_err());
    }
}

/// The 12 well-known relation types, in the ONE spelling this crate stores.
///
/// v1.2.8: kebab-case. The list used to be snake_case while the JSON Schema
/// handed to the extraction model (`enrich::schemas`) declared the same twelve
/// names in kebab-case, and `enrich::extraction_body` persisted the model's
/// answer verbatim. The result was a store split across two spellings of the
/// same relation — measured at 67 651 kebab edges against 3 578 snake ones over
/// three production databases, so the spelling this constant called canonical
/// was the one 5% of the data used.
///
/// The split was invisible because every read filter normalises before a
/// LITERAL `WHERE`: `related --relation applies-to` returned zero rows, exit 0,
/// on a hub that has `applies-to` edges. Instruments read the wrong scale for
/// the same reason — `health.applies_to_ratio` reported 0.0085% where the true
/// share is 17.8%, a factor of 2098.
///
/// Only the three multi-word relations can differ at all, and those are exactly
/// the ones carrying hierarchy: `applies-to`, `depends-on`, `tracked-in`.
///
/// Non-canonical relations are accepted but emit a `tracing::warn!`.
pub const CANONICAL_RELATIONS: &[&str] = &[
    "applies-to",
    "uses",
    "depends-on",
    "causes",
    "fixes",
    "contradicts",
    "supports",
    "follows",
    "related",
    "mentions",
    "replaces",
    "tracked-in",
];

/// The generic relation, named once so consumers stop repeating the literal.
///
/// `enrich::predicates`, `enrich::scan::relationships` and `health` each held
/// their own copy of this string, and each held the snake_case one. A literal
/// repeated in four places drifts in four places.
pub const GENERIC_RELATION: &str = "applies-to";

/// Returns `true` when the relation is one of the 12 canonical types.
pub fn is_canonical_relation(s: &str) -> bool {
    CANONICAL_RELATIONS.contains(&s)
}

/// Normalizes a relation string: lowercase + underscores to hyphens.
///
/// v1.2.8 reversed the direction along with [`CANONICAL_RELATIONS`]. Callers
/// keep passing either spelling; what changed is which one survives to SQL.
pub fn normalize_relation(s: &str) -> String {
    s.to_lowercase().replace('_', "-")
}

/// Normalizes an entity name to kebab-case ASCII.
///
/// Applies NFKD decomposition, filters to ASCII (transliterating by dropping
/// diacritical combining marks), lowercases, converts spaces and underscores
/// to hyphens, collapses consecutive hyphens, and trims leading/trailing hyphens.
///
/// # Examples
///
/// ```
/// use sqlite_graphrag::parsers::normalize_entity_name;
///
/// assert_eq!(normalize_entity_name("Alice Martins"), "alice-martins");
/// assert_eq!(normalize_entity_name("CANONICAL_RELATIONS"), "canonical-relations");
/// assert_eq!(normalize_entity_name("  hello  world  "), "hello-world");
/// assert_eq!(normalize_entity_name("alice-martins"), "alice-martins"); // idempotent
/// ```
pub fn normalize_entity_name(s: &str) -> String {
    // NFKD: decompose precomposed characters into base + combining marks.
    // Then keep only ASCII characters, effectively stripping diacritics.
    let ascii: String = s.nfkd().filter(|c| c.is_ascii()).collect();
    // Lowercase, then replace spaces and underscores with hyphens.
    let hyphenated: String = ascii
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse consecutive hyphens and trim from both ends.
    let mut result = String::with_capacity(hyphenated.len());
    let mut prev_was_hyphen = false;
    for ch in hyphenated.chars() {
        if ch == '-' {
            if !prev_was_hyphen {
                result.push('-');
            }
            prev_was_hyphen = true;
        } else {
            result.push(ch);
            prev_was_hyphen = false;
        }
    }
    result.trim_matches('-').to_string()
}

/// Validates that a NORMALIZED relation matches `^[a-z][a-z0-9-]*$`.
///
/// v1.2.8: hyphen replaced underscore here together with [`normalize_relation`].
/// The pair must agree, and before they did this function rejected the very
/// spelling the crate was persisting: `applies-to` failed validation while
/// 50 346 rows of it sat in one database, because the write path that produced
/// them called neither this nor the normaliser.
///
/// Takes the normalised form. Callers that hold raw input run it through
/// [`normalize_relation`] or [`parse_relation`] first.
pub fn validate_relation_format(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("relation must not be empty".to_string());
    }
    if !s.as_bytes()[0].is_ascii_lowercase() {
        return Err(format!(
            "relation must start with a lowercase letter, got '{s}'"
        ));
    }
    if !s
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(format!(
            "relation must contain only lowercase letters, digits and hyphens, got '{s}'"
        ));
    }
    Ok(())
}

/// Maps an arbitrary relation label to its canonical form, never producing a
/// non-canonical value (GAP-SG-48).
///
/// Relation handling used to be inconsistent: non-canonical relations were
/// accepted raw (with only a `WARN`) while non-canonical entity types were
/// rejected outright. This unifies the policy — extraction never persists a
/// label outside the canonical vocabulary. Known aliases are rewritten via a
/// fixed table; values that are already canonical pass through unchanged;
/// anything else falls back to the generic `related`.
///
/// Alias table (mirrors the project's canonical relation map):
/// `adds`/`creates` → `causes`, `implements` → `supports`,
/// `blocks` → `contradicts`, `tested-by` → `related`, `part-of` → `applies-to`.
///
/// The arms are written in kebab-case because [`normalize_relation`] hands this
/// `match` kebab-case: an arm spelled `part_of` would be unreachable.
pub fn map_to_canonical_relation(s: &str) -> String {
    let normalized = normalize_relation(s);
    if is_canonical_relation(&normalized) {
        return normalized;
    }
    match normalized.as_str() {
        "adds" | "creates" => "causes",
        "implements" => "supports",
        "blocks" => "contradicts",
        "tested-by" | "related-to" => "related",
        "part-of" => "applies-to",
        // Any other non-canonical relation folds onto the generic canonical
        // kind rather than being persisted raw.
        _ => "related",
    }
    .to_string()
}

/// Emits a `tracing::warn!` when the relation is not in [`CANONICAL_RELATIONS`].
pub fn warn_if_non_canonical(relation: &str) {
    if !is_canonical_relation(relation) {
        tracing::warn!(target: "parsers",
            relation,
            "non-canonical relation accepted; consider using a well-known value"
        );
    }
}

/// Clap `value_parser` for `--relation`: normalizes and validates format.
///
/// Accepts any kebab-case or snake_case string. Non-canonical values are
/// accepted at parse time; the warning is emitted at command execution.
pub fn parse_relation(s: &str) -> Result<String, String> {
    let normalized = normalize_relation(s);
    validate_relation_format(&normalized)?;
    Ok(normalized)
}

#[cfg(test)]
mod relation_tests {
    use super::*;

    #[test]
    fn canonical_relations_all_valid() {
        for r in CANONICAL_RELATIONS {
            assert!(
                validate_relation_format(r).is_ok(),
                "canonical relation '{r}' should be valid"
            );
        }
    }

    // v1.2.8: the expectations below changed direction because the CONTRACT
    // changed, by decision, not to make a red test green. The crate now stores
    // kebab-case, which is the spelling 95% of the existing rows already used
    // and the one every prompt and document already taught; snake_case was the
    // spelling only this constant believed in.
    #[test]
    fn normalize_converts_underscores_and_uppercase() {
        assert_eq!(normalize_relation("Depends_On"), "depends-on");
        assert_eq!(normalize_relation("TESTED_BY"), "tested-by");
        assert_eq!(normalize_relation("uses"), "uses");
    }

    #[test]
    fn validate_rejects_empty() {
        assert!(validate_relation_format("").is_err());
    }

    #[test]
    fn validate_rejects_digit_start() {
        assert!(validate_relation_format("123abc").is_err());
    }

    #[test]
    fn validate_rejects_spaces() {
        assert!(validate_relation_format("has spaces").is_err());
    }

    #[test]
    fn validate_accepts_custom_relations() {
        // Takes the NORMALISED form, so the multi-word cases arrive hyphenated.
        assert!(validate_relation_format("implements").is_ok());
        assert!(validate_relation_format("tested-by").is_ok());
        assert!(validate_relation_format("part-of").is_ok());
        assert!(validate_relation_format("blocks").is_ok());
    }

    /// The normaliser and the validator must agree, in both directions.
    ///
    /// They disagreed before v1.2.8, and that disagreement is the whole defect:
    /// the validator demanded underscores while the bulk write path stored
    /// hyphens, so the crate rejected as malformed the exact spelling it held
    /// 67 651 rows of. Nothing compared the two, so nothing said so.
    #[test]
    fn normaliser_output_always_passes_the_validator() {
        for raw in [
            "Applies-To",
            "applies_to",
            "DEPENDS_ON",
            "depends-on",
            "tracked_in",
            "uses",
            "tested-by",
            "part_of",
        ] {
            let normalized = normalize_relation(raw);
            assert!(
                validate_relation_format(&normalized).is_ok(),
                "normalize_relation({raw:?}) produced {normalized:?}, which the                  validator rejects — the two disagree about the stored form"
            );
        }
        for rel in CANONICAL_RELATIONS {
            assert_eq!(
                &normalize_relation(rel),
                rel,
                "normalize_relation is not idempotent on the canonical relation                  {rel:?}, so the constant names a form the crate never stores"
            );
            assert!(validate_relation_format(rel).is_ok());
        }
    }

    #[test]
    fn parse_relation_normalizes_and_validates() {
        assert_eq!(parse_relation("Tested_By").unwrap(), "tested-by");
        assert_eq!(parse_relation("Tested-By").unwrap(), "tested-by");
        assert_eq!(parse_relation("uses").unwrap(), "uses");
        assert!(parse_relation("").is_err());
    }

    #[test]
    fn is_canonical_detects_known() {
        assert!(is_canonical_relation("uses"));
        assert!(is_canonical_relation("applies-to"));
        // The snake spelling is no longer canonical, and saying so out loud is
        // the point: a database written by an older binary holds it, and only
        // the reader tolerates it — the writer never produces it again.
        assert!(!is_canonical_relation("applies_to"));
        assert!(!is_canonical_relation("implements"));
        assert!(!is_canonical_relation("blocks"));
    }

    #[test]
    fn map_to_canonical_relation_passes_through_canonical() {
        assert_eq!(map_to_canonical_relation("uses"), "uses");
        assert_eq!(map_to_canonical_relation("Applies-To"), "applies-to");
        assert_eq!(map_to_canonical_relation("DEPENDS_ON"), "depends-on");
        // Both spellings converge on the stored one, which is what lets the
        // persistence boundary canonicalise without rejecting any caller.
        assert_eq!(map_to_canonical_relation("applies_to"), "applies-to");
        assert_eq!(map_to_canonical_relation("tracked_in"), "tracked-in");
    }

    #[test]
    fn map_to_canonical_relation_rewrites_known_aliases() {
        // GAP-SG-48: part-of was previously accepted raw with only a WARN.
        assert_eq!(map_to_canonical_relation("part-of"), "applies-to");
        assert_eq!(map_to_canonical_relation("part_of"), "applies-to");
        assert_eq!(map_to_canonical_relation("implements"), "supports");
        assert_eq!(map_to_canonical_relation("blocks"), "contradicts");
        assert_eq!(map_to_canonical_relation("adds"), "causes");
        assert_eq!(map_to_canonical_relation("creates"), "causes");
        assert_eq!(map_to_canonical_relation("tested-by"), "related");
        assert_eq!(map_to_canonical_relation("related_to"), "related");
        assert_eq!(map_to_canonical_relation("related-to"), "related");
    }

    #[test]
    fn map_to_canonical_relation_unknown_folds_to_related() {
        assert_eq!(map_to_canonical_relation("some-weird-relation"), "related");
        // Output is always itself canonical.
        assert!(is_canonical_relation(&map_to_canonical_relation("xyz")));
    }
}

#[cfg(test)]
mod entity_name_tests {
    use super::*;

    #[test]
    fn strips_diacritics_from_accented_name() {
        assert_eq!(normalize_entity_name("Alice Martins"), "alice-martins");
    }

    #[test]
    fn strips_diacritics_unicode_accents() {
        // `é → e, ã → a, ç → c`
        assert_eq!(normalize_entity_name("São Paulo"), "sao-paulo");
        assert_eq!(normalize_entity_name("Ünit Tëst"), "unit-test");
    }

    #[test]
    fn converts_spaces_to_hyphens() {
        assert_eq!(normalize_entity_name("hello world"), "hello-world");
        assert_eq!(normalize_entity_name("  hello  world  "), "hello-world");
    }

    #[test]
    fn converts_underscores_to_hyphens() {
        assert_eq!(normalize_entity_name("hello_world"), "hello-world");
        assert_eq!(
            normalize_entity_name("CANONICAL_RELATIONS"),
            "canonical-relations"
        );
    }

    #[test]
    fn all_caps_becomes_lowercase_kebab() {
        assert_eq!(
            normalize_entity_name("CANONICAL_RELATIONS"),
            "canonical-relations"
        );
        assert_eq!(normalize_entity_name("MY_ENTITY_NAME"), "my-entity-name");
    }

    #[test]
    fn idempotent_on_already_normalized() {
        let name = "alice-martins";
        assert_eq!(normalize_entity_name(name), name);
        let name2 = "canonical-relations";
        assert_eq!(normalize_entity_name(name2), name2);
    }

    #[test]
    fn collapses_consecutive_hyphens() {
        assert_eq!(normalize_entity_name("foo--bar"), "foo-bar");
        assert_eq!(normalize_entity_name("foo - bar"), "foo-bar");
    }

    #[test]
    fn trims_leading_trailing_hyphens() {
        assert_eq!(normalize_entity_name("-foo-"), "foo");
        assert_eq!(normalize_entity_name("--hello--"), "hello");
    }

    #[test]
    fn empty_or_only_separators_returns_empty() {
        assert_eq!(normalize_entity_name(""), "");
        assert_eq!(normalize_entity_name("---"), "");
    }

    #[test]
    fn normalizes_dots_slashes_and_punctuation() {
        assert_eq!(normalize_entity_name("lei-14.478/2022"), "lei-14-478-2022");
        assert_eq!(normalize_entity_name("src/main.rs"), "src-main-rs");
        assert_eq!(normalize_entity_name("user@domain.com"), "user-domain-com");
        assert_eq!(normalize_entity_name("v1.0.66"), "v1-0-66");
        assert_eq!(normalize_entity_name("key:value"), "key-value");
    }
}
