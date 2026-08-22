//! Resolution of the `--names` / `--names-file` subset applied to every scan.

use super::super::args::EnrichArgs;
use crate::errors::AppError;
use std::path::Path;

/// Reads a list of memory names from a UTF-8 text file (G37).
///
/// Empty lines and lines beginning with `#` are skipped. Returns a
/// de-duplicated, order-preserving list of trimmed names.
pub(in crate::commands::enrich) fn read_names_file(path: &Path) -> Result<Vec<String>, AppError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        AppError::Validation(crate::i18n::validation::failed_to_read_names_file(
            &path.display().to_string(),
            &e,
        ))
    })?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
}

/// Resolves the union of `--names`, `--entity-names`, `--memory-names` and
/// `--names-file` (G37, GAP-CLI-NAMES-02).
///
/// `--entity-names` and `--memory-names` are documented in `args.rs` as
/// aliases of `--names`, and the alias was never wired: clap accepted both
/// flags and this function read neither, so a targeted
/// `--force-redescribe --entity-names x,y` silently degraded to an unfiltered
/// scan and reported `matched: 0`. Accepting a flag and ignoring it is worse
/// than rejecting it, because the caller has no way to tell.
pub(in crate::commands::enrich) fn resolve_name_filter(
    args: &EnrichArgs,
) -> Result<Vec<String>, AppError> {
    // Reserve the upper bound and dedupe through a set. The previous version
    // grew from `Vec::new()` and deduped with `out.contains(n)`, which is O(n²)
    // in a list `--names-file` sizes from a file: ten thousand lines cost fifty
    // million `String` comparisons to produce ten thousand entries.
    let upper = args.names.len()
        + args.entity_names.len()
        + args.memory_names.len()
        + usize::from(args.names_file.is_some());
    let mut combined: Vec<String> = Vec::with_capacity(upper);
    let mut seen: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(upper);
    let mut push_unique = |src: &[String], out: &mut Vec<String>| {
        for n in src {
            if seen.insert(n.clone()) {
                out.push(n.clone());
            }
        }
    };
    push_unique(&args.names, &mut combined);
    push_unique(&args.entity_names, &mut combined);
    push_unique(&args.memory_names, &mut combined);
    if let Some(p) = &args.names_file {
        let from_file = read_names_file(p)?;
        push_unique(&from_file, &mut combined);
    }
    Ok(combined)
}

/// Widens a name filter to also carry the kebab-ASCII form of each name.
///
/// Entity names are normalised on write by `parsers::normalize_entity_name`
/// (NFKD, ASCII, lowercase, spaces and underscores to hyphens), and the scan
/// predicates match `name IN (...)` against the raw strings the caller typed.
/// `--entity-names "Relatório Anual"` therefore matched nothing, because the row
/// is stored as `relatorio-anual` — the same class of defect as the relation
/// spelling: a normalisation applied at one boundary and not at the other.
///
/// Both forms are kept rather than substituted, because the same flag family
/// also carries MEMORY names, which do not follow that normalisation. Replacing
/// the raw form would fix entity lookups by breaking memory lookups.
///
/// Names already in canonical form contribute nothing, so a filter written the
/// normalised way is unchanged in length and cost.
pub(in crate::commands::enrich) fn widen_name_filter(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(names.len() * 2);
    let mut seen: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(names.len() * 2);
    for n in names {
        if seen.insert(n.clone()) {
            out.push(n.clone());
        }
    }
    for n in names {
        let normalized = crate::parsers::normalize_entity_name(n);
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
}

#[cfg(test)]
mod name_filter_widening {
    use super::widen_name_filter;

    /// A name typed the way a human types it must reach the stored row.
    ///
    /// Entities are stored kebab-ASCII, and the scan predicate compares
    /// `name IN (...)` against the raw strings. `--entity-names "Relatório Anual"`
    /// therefore matched nothing and reported `matched: 0`, which reads as
    /// "this entity has nothing to do" rather than "you cannot address it".
    #[test]
    fn adds_the_normalized_form() {
        let widened = widen_name_filter(&["Relatório Anual".to_string()]);
        assert!(widened.contains(&"Relatório Anual".to_string()));
        assert!(widened.contains(&"relatorio-anual".to_string()));
    }

    /// The raw form survives because the same flag family carries MEMORY names,
    /// which are not kebab-normalised. Replacing instead of widening would fix
    /// entity lookups by breaking memory lookups.
    #[test]
    fn keeps_the_raw_form_for_memory_names() {
        let widened = widen_name_filter(&["auditoria-hooks-grok-20260814".to_string()]);
        assert_eq!(widened.len(), 1, "already canonical: nothing to add");
        assert_eq!(widened[0], "auditoria-hooks-grok-20260814");
    }

    #[test]
    fn does_not_duplicate() {
        let widened = widen_name_filter(&["Ana".to_string(), "ana".to_string()]);
        let unique: std::collections::BTreeSet<&String> = widened.iter().collect();
        assert_eq!(unique.len(), widened.len(), "duplicates: {widened:?}");
    }
}
