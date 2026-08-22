//! Name normalisation and boundary validation for `remember`.
//!
//! Auto-normalises `--name` to kebab-case (P2-H) and rejects everything the
//! canonical form forbids, so the persistence path only ever sees a valid slug.

use super::args::RememberArgs;
use crate::constants::{MAX_MEMORY_DESCRIPTION_LEN, MAX_MEMORY_NAME_LEN};
use crate::errors::AppError;

/// The name as the operator typed it, next to the canonical form.
pub(super) struct ResolvedName {
    /// `--name` exactly as received, echoed in the JSON response.
    pub(super) original: String,
    /// Canonical kebab-case form actually persisted.
    pub(super) normalized: String,
    /// True when normalisation changed the name.
    pub(super) was_normalized: bool,
}

/// Normalises and validates `--name` (and the `--description` length).
pub(super) fn resolve(args: &RememberArgs) -> Result<ResolvedName, AppError> {
    // GAP-SG-216: the positional form and `--name` are alternatives, and clap
    // already refuses both at once via `conflicts_with`. What clap cannot say is
    // that NEITHER was given, because neither is individually required — so the
    // refusal lands here, on the message the eight sibling commands already use.
    let original_name = args
        .name_positional
        .clone()
        .or_else(|| args.name.clone())
        .ok_or_else(|| {
            AppError::Validation(crate::i18n::validation::name_required_positional_or_flag())
        })?;

    // Auto-normalize to kebab-case before validation (P2-H).
    // v1.0.20: also trims hyphens at the boundary (including trailing) to avoid rejection
    // after truncation by a long filename ending in a hyphen.
    let normalized_name = {
        let lower = original_name.to_lowercase().replace(['_', ' '], "-");
        let trimmed = lower.trim_matches('-').to_string();
        if trimmed != original_name {
            tracing::warn!(target: "remember",
                original = %original_name,
                normalized = %trimmed,
                "name auto-normalized to kebab-case"
            );
        }
        trimmed
    };
    let name_was_normalized = normalized_name != original_name;

    // GAP-SG-37: when --strict-name is set, refuse to silently rewrite the name.
    // The operator gets the canonical form so they can re-submit it explicitly.
    if args.strict_name && name_was_normalized {
        return Err(AppError::Validation(
            crate::i18n::validation::strict_name_not_canonical(&original_name, &normalized_name),
        ));
    }

    if normalized_name.is_empty() {
        return Err(AppError::Validation(
            crate::i18n::validation::name_empty_after_normalization(),
        ));
    }
    if normalized_name.len() > MAX_MEMORY_NAME_LEN {
        return Err(AppError::LimitExceeded(
            crate::i18n::validation::name_length(MAX_MEMORY_NAME_LEN),
        ));
    }

    if normalized_name.starts_with("__") {
        return Err(AppError::Validation(
            crate::i18n::validation::reserved_name(),
        ));
    }

    {
        let slug_re = crate::constants::name_slug_regex();
        if !slug_re.is_match(&normalized_name) {
            return Err(AppError::Validation(crate::i18n::validation::name_kebab(
                &normalized_name,
            )));
        }
    }

    if let Some(ref desc) = args.description {
        if desc.len() > MAX_MEMORY_DESCRIPTION_LEN {
            return Err(AppError::Validation(
                crate::i18n::validation::description_exceeds(MAX_MEMORY_DESCRIPTION_LEN),
            ));
        }
    }

    Ok(ResolvedName {
        original: original_name,
        normalized: normalized_name,
        was_normalized: name_was_normalized,
    })
}
