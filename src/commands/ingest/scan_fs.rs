//! Filesystem scan and memory-name derivation for the `ingest` pipeline.
//!
//! Walks a directory tree (optionally recursive), filters basenames against a
//! simple glob pattern, and derives collision-safe kebab-case memory names from
//! file stems. It was shared by the local ingest path and the LLM-mode ingest
//! frontends (`ingest_claude`, `ingest_codex`, `ingest_opencode`) until v1.2.0
//! removed those; the local path is now the only caller.

use crate::errors::AppError;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Hard cap on the numeric suffix appended for collision resolution. If 1000
/// candidates collide we surface an error rather than loop forever.
pub(crate) const MAX_NAME_COLLISION_SUFFIX: usize = 1000;

/// Recursively (optional) collect files under `dir` whose basenames match `pattern`.
///
/// Pattern support is intentionally minimal: `*.ext` suffix, `prefix*`, or exact
/// equality. Results are appended to `out` in filesystem walk order (callers sort).
pub(crate) fn collect_files(
    dir: &Path,
    pattern: &str,
    recursive: bool,
    out: &mut Vec<PathBuf>,
) -> Result<(), AppError> {
    let entries = std::fs::read_dir(dir).map_err(AppError::Io)?;
    for entry in entries {
        let entry = entry.map_err(AppError::Io)?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(AppError::Io)?;
        if file_type.is_file() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if matches_pattern(&name_str, pattern) {
                out.push(path);
            }
        } else if file_type.is_dir() && recursive {
            collect_files(&path, pattern, recursive, out)?;
        }
    }
    Ok(())
}

/// Simple basename matcher for ingest `--pattern` (suffix/prefix/exact only).
pub(crate) fn matches_pattern(name: &str, pattern: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix('*') {
        name.ends_with(suffix)
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        name == pattern
    }
}

/// Validates `--name-prefix` and returns the effective budget for the DERIVED
/// part of the name, so `prefix + derived` never exceeds
/// [`crate::constants::MAX_MEMORY_NAME_LEN`].
///
/// The prefix is applied verbatim AFTER kebab normalization of the basename, so
/// it must itself be a valid slug head: starting with a lowercase letter and
/// containing only lowercase letters, digits and hyphens.
pub(crate) fn validate_name_prefix(
    prefix: &str,
    max_name_length: usize,
) -> Result<usize, AppError> {
    if prefix.is_empty() {
        return Err(AppError::Validation(
            "--name-prefix cannot be empty".to_string(),
        ));
    }
    let starts_lower = prefix
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase());
    let all_slug_chars = prefix
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !starts_lower || !all_slug_chars {
        return Err(AppError::Validation(
            crate::i18n::validation::name_prefix_kebab(prefix),
        ));
    }
    let cap = crate::constants::MAX_MEMORY_NAME_LEN;
    if prefix.len() >= cap {
        return Err(AppError::LimitExceeded(
            crate::i18n::errors_ops::name_prefix_exceeds_name_cap(prefix.len(), cap),
        ));
    }
    Ok(max_name_length.min(cap - prefix.len()))
}

/// Returns `(final_name, truncated, original_name)`.
///
/// `truncated` is true when the derived name exceeded `max_len`.
/// `original_name` holds the pre-truncation name only when `truncated=true`.
///
/// Non-ASCII characters are first decomposed via NFD and then stripped of
/// combining marks so accented letters fold to their base ASCII letter
/// (e.g. `acai` from accented input, `naive` from diaeresis). Characters with
/// no ASCII fallback (emoji, CJK ideographs, symbols) are dropped silently.
pub(crate) fn derive_kebab_name(path: &Path, max_len: usize) -> (String, bool, Option<String>) {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let lowered: String = stem
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .map(|c| {
            if c == '_' || c.is_whitespace() {
                '-'
            } else {
                c
            }
        })
        .map(|c| c.to_ascii_lowercase())
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
        .collect();
    let collapsed = collapse_dashes(&lowered);
    let trimmed_raw = collapsed.trim_matches('-').to_string();
    // Prefix names that start with a digit to keep them valid kebab-case identifiers.
    let trimmed = if trimmed_raw.starts_with(|c: char| c.is_ascii_digit()) {
        format!("doc-{trimmed_raw}")
    } else {
        trimmed_raw
    };
    if trimmed.len() > max_len {
        let truncated = trimmed[..max_len].trim_matches('-').to_string();
        // GAP-SG-38: warn (not debug) so the operator sees that a derived name
        // was cut at the cap and that any collision will be resolved with a
        // numeric disambiguation suffix. The pre-truncation form is also
        // surfaced per-file via `IngestFileEvent.original_name`.
        tracing::warn!(
            target: "ingest",
            original = %trimmed,
            truncated_to = %truncated,
            max_len = max_len,
            "derived memory name truncated to fit length cap; collisions will be resolved with numeric suffixes"
        );
        (truncated, true, Some(trimmed))
    } else {
        (trimmed, false, None)
    }
}

/// v1.0.31 A10: returns the first non-colliding kebab name by appending a
/// numeric suffix (`-1`, `-2`, …) when needed.
///
/// `taken` is the set of names already consumed in the current ingest run.
/// The caller is expected to insert the returned name into `taken` so the
/// next call observes the consumption. Cross-run collisions are intentionally
/// surfaced by the per-file persistence path as duplicates so re-ingestion
/// of identical corpora stays idempotent.
///
/// Returns `Err(AppError::Validation)` after `MAX_NAME_COLLISION_SUFFIX`
/// candidates collide, signalling a pathological corpus that should be
/// renamed manually.
pub(crate) fn unique_name(base: &str, taken: &BTreeSet<String>) -> Result<String, AppError> {
    if !taken.contains(base) {
        return Ok(base.to_string());
    }
    for suffix in 1..=MAX_NAME_COLLISION_SUFFIX {
        let candidate = format!("{base}-{suffix}");
        if !taken.contains(&candidate) {
            tracing::warn!(
                target: "ingest",
                base = %base,
                resolved = %candidate,
                suffix,
                "memory name collision resolved with numeric suffix"
            );
            return Ok(candidate);
        }
    }
    Err(AppError::Validation(
        crate::i18n::validation::too_many_name_collisions(base, MAX_NAME_COLLISION_SUFFIX),
    ))
}

/// Collapse consecutive dashes into a single dash.
pub(crate) fn collapse_dashes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c == '-' {
            if !prev_dash {
                out.push('-');
            }
            prev_dash = true;
        } else {
            out.push(c);
            prev_dash = false;
        }
    }
    out
}
