//! Entity type vocabulary: open by default, with a canonical set as guidance.
//!
//! Until v1.2.8 this module owned a closed `EntityType` enum of thirteen kinds
//! and folded every other label onto the nearest one, terminating at `concept`.
//! The fold was lossy in the strict sense: the string the caller wrote was
//! consumed inside `Deserialize` and never existed as a value again, so no
//! layer above could report it, store it, or decide policy about it. On the
//! database of this workspace that put 69% of all entities in a single bucket,
//! which makes filtering by `concept` indistinguishable from not filtering.
//!
//! v1.2.8 opens the vocabulary instead of widening it, mirroring what
//! `V010__open_relation_vocabulary.sql` did for relations back in v1.0.49: the
//! SQL `CHECK` is gone (V017), the label travels as a plain `String`, and
//! [`crate::entity_type::CANONICAL_ENTITY_TYPES`] is advice rather than a gate.
//! The path is fully qualified on purpose: a `//!` header is concatenated with
//! the `///` written above `pub mod` in `lib.rs` and RESOLVES in that outer
//! scope, so a bare name here fails to resolve and `cargo doc` exits 101.
//! Because nothing is
//! folded any more, the caller's label survives by construction — it needs no
//! second column to be preserved, only the absence of something destroying it.
//!
//! What remains enforced is *shape*, never membership. See
//! [`crate::entity_type::normalize_entity_type`].

use crate::constants::MAX_ENTITY_TYPE_LEN;
use crate::errors::AppError;
use crate::i18n::validation;

/// The thirteen kinds that were canonical while the vocabulary was closed.
///
/// They stay as the recommended vocabulary — surfaced in help text, offered as
/// completion candidates, and enforced under `--strict-entity-types` — but they
/// no longer bound what can be stored. Deliberately mirrors
/// [`crate::parsers::CANONICAL_RELATIONS`], which has played exactly this role
/// for the relation vocabulary since v1.0.49.
///
/// Kept sorted so emitted diagnostics are stable.
pub const CANONICAL_ENTITY_TYPES: &[&str] = &[
    "concept",
    "dashboard",
    "date",
    "decision",
    "file",
    "incident",
    "issue_tracker",
    "location",
    "memory",
    "organization",
    "person",
    "project",
    "tool",
];

/// Kind assigned when a caller supplies no type at all.
///
/// This is the one place `concept` still wins by default. It is a default, not
/// a destination: a label that simply differs from the canonical set is now
/// stored as written, and only an *absent* label lands here.
pub const DEFAULT_ENTITY_TYPE: &str = "concept";

/// Reports whether `s` is one of [`CANONICAL_ENTITY_TYPES`].
///
/// Compares the already-normalised form, so callers should pass the output of
/// [`normalize_entity_type`]. Mirrors `parsers::is_canonical_relation`.
#[must_use]
pub fn is_canonical_entity_type(s: &str) -> bool {
    CANONICAL_ENTITY_TYPES.contains(&s)
}

/// Normalises an entity type label's *shape*, never its meaning.
///
/// Applies exactly three transformations — trim, lowercase, and hyphen to
/// underscore — so `"Issue-Tracker"` and `"issue_tracker"` remain the same
/// row rather than two, and so does `"Crate"` versus `"crate"`. It performs no
/// mapping whatsoever: an unrecognised label comes back as itself, which is
/// the whole point of the change.
///
/// Rejection is limited to labels that could not be a word in any vocabulary:
/// empty, digits only, containing a line break, or longer than
/// [`MAX_ENTITY_TYPE_LEN`]. Membership is never a reason to reject here;
/// that decision belongs to `--strict-entity-types`, one layer up, where the
/// caller has asked for it.
///
/// # Errors
/// Returns [`AppError::Validation`] when the label is blank, digits only,
/// contains a line break, or exceeds [`MAX_ENTITY_TYPE_LEN`] characters.
pub fn normalize_entity_type(s: &str) -> Result<String, AppError> {
    let normalized = s.trim().to_lowercase().replace('-', "_");

    if normalized.is_empty() {
        return Err(AppError::Validation(validation::entity_type_blank()));
    }
    if normalized.contains('\n') || normalized.contains('\r') {
        return Err(AppError::Validation(validation::entity_type_has_newline(
            &normalized,
        )));
    }
    if normalized.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::Validation(validation::entity_type_digits_only(
            &normalized,
        )));
    }
    if normalized.chars().count() > MAX_ENTITY_TYPE_LEN {
        return Err(AppError::Validation(validation::entity_type_too_long(
            &normalized,
            MAX_ENTITY_TYPE_LEN,
        )));
    }

    Ok(normalized)
}

/// Normalises `s`, falling back to [`DEFAULT_ENTITY_TYPE`] when it is unusable.
///
/// For the read paths that materialise a label already stored in SQLite, where
/// refusing is not an option because the row exists either way. Write paths
/// must call [`normalize_entity_type`] and surface the error instead.
#[must_use]
pub fn normalize_entity_type_or_default(s: &str) -> String {
    normalize_entity_type(s).unwrap_or_else(|_| DEFAULT_ENTITY_TYPE.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_set_has_thirteen_sorted_members() {
        assert_eq!(CANONICAL_ENTITY_TYPES.len(), 13);
        let mut sorted = CANONICAL_ENTITY_TYPES.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            sorted.as_slice(),
            CANONICAL_ENTITY_TYPES,
            "kept sorted so diagnostics are stable"
        );
    }

    #[test]
    fn canonical_labels_are_recognised() {
        for kind in CANONICAL_ENTITY_TYPES {
            assert!(is_canonical_entity_type(kind), "{kind} must be canonical");
        }
    }

    #[test]
    fn shape_normalisation_is_case_and_hyphen_insensitive() {
        assert_eq!(
            normalize_entity_type("  Issue-Tracker ").unwrap(),
            "issue_tracker"
        );
        assert_eq!(normalize_entity_type("PERSON").unwrap(), "person");
    }

    /// The regression this whole change exists to prevent: a label outside the
    /// canonical set must come back as itself, not as `concept`.
    #[test]
    fn non_canonical_labels_survive_verbatim() {
        for label in ["crate", "gap", "flag", "migration", "schema", "framework"] {
            let normalized = normalize_entity_type(label).unwrap();
            assert_eq!(normalized, label, "{label} must not be folded");
            assert!(
                !is_canonical_entity_type(&normalized),
                "{label} is not canonical, but is still storable"
            );
        }
    }

    /// `framework` was on the deliberate fold list until v1.2.8. Pinned
    /// separately because reintroducing that map would pass every other test.
    #[test]
    fn previously_folded_labels_are_no_longer_folded() {
        for label in [
            "framework",
            "library",
            "method",
            "metric",
            "platform",
            "protocol",
        ] {
            assert_eq!(normalize_entity_type(label).unwrap(), label);
        }
    }

    #[test]
    fn blank_and_digit_only_labels_are_refused() {
        assert!(normalize_entity_type("").is_err());
        assert!(normalize_entity_type("   ").is_err());
        assert!(normalize_entity_type("42").is_err());
    }

    #[test]
    fn line_breaks_are_refused() {
        assert!(normalize_entity_type("person\nrole").is_err());
        assert!(normalize_entity_type("person\rrole").is_err());
    }

    #[test]
    fn overlong_labels_are_refused_by_characters_not_bytes() {
        let long = "a".repeat(MAX_ENTITY_TYPE_LEN + 1);
        assert!(normalize_entity_type(&long).is_err());

        let at_limit = "a".repeat(MAX_ENTITY_TYPE_LEN);
        assert!(normalize_entity_type(&at_limit).is_ok());

        // Multi-byte characters count once each, never by their UTF-8 width.
        let accented = "á".repeat(MAX_ENTITY_TYPE_LEN);
        assert!(normalize_entity_type(&accented).is_ok());
    }

    #[test]
    fn default_is_used_only_when_normalisation_fails() {
        assert_eq!(normalize_entity_type_or_default("crate"), "crate");
        assert_eq!(normalize_entity_type_or_default(""), DEFAULT_ENTITY_TYPE);
    }
}
