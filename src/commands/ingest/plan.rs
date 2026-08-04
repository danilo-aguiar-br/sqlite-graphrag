//! Stage 1 of the ingest pipeline: turn a file list into a name plan.
//!
//! Every memory name is resolved here, before any parallelism starts, so the
//! Phase A workers observe one immutable name assignment (the v1.0.31 A10
//! contract). Uniqueness is decided against a single growing set on one
//! thread; doing it inside the workers would make the outcome depend on
//! completion order.
//!
//! The plan splits into two parallel vectors on purpose. [`SlotMeta`] holds
//! the reporting metadata and stays on the main thread; [`ProcessItem`] holds
//! only what the rayon producer needs (`PathBuf` + `String`) and is therefore
//! cheap to move across the thread boundary.

use super::args::IngestArgs;
use super::scan_fs::{derive_kebab_name, unique_name, validate_name_prefix};
use crate::constants::DERIVED_NAME_MAX_LEN;
use crate::errors::AppError;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Per-slot reporting metadata retained on the main thread for NDJSON output.
///
/// One entry per input file, in filesystem-sorted order. A slot is either
/// dropped before staging ([`SlotMeta::Skip`]) or paired with a
/// [`ProcessItem`] ([`SlotMeta::Process`]).
pub(super) enum SlotMeta {
    /// The file never reaches Phase A: its name could not be derived or was
    /// rejected as a duplicate.
    Skip {
        /// Source path, lossily rendered for the event payload.
        file_str: String,
        /// Derived name as far as it got; empty when derivation failed.
        derived_base: String,
        /// Whether the derived name hit the length budget.
        name_truncated: bool,
        /// Pre-truncation name, when truncation happened.
        original_name: Option<String>,
        /// Raw file stem, when it differs from the derived name.
        original_filename: Option<String>,
        /// Why the slot was skipped.
        reason: String,
    },
    /// The file is staged and persisted; carries its final name.
    Process {
        /// Source path, lossily rendered for the event payload.
        file_str: String,
        /// Final, unique memory name.
        derived_name: String,
        /// Whether the derived name hit the length budget.
        name_truncated: bool,
        /// Pre-truncation name, when truncation happened.
        original_name: Option<String>,
        /// Raw file stem, when it differs from the derived name.
        original_filename: Option<String>,
    },
}

/// The `Send` half of a slot: everything the rayon producer needs to stage one
/// file, and nothing else.
pub(super) struct ProcessItem {
    /// Index into the slot vector, so Phase B can find the matching
    /// [`SlotMeta`] when results arrive out of order.
    pub(super) idx: usize,
    /// Source path to read.
    pub(super) path: PathBuf,
    /// Source path, lossily rendered for progress events.
    pub(super) file_str: String,
    /// Final memory name assigned to this file.
    pub(super) derived_name: String,
}

/// Resolved name assignment for one ingest run.
pub(super) struct IngestPlan {
    /// One entry per input file, in filesystem-sorted order.
    pub(super) slots_meta: Vec<SlotMeta>,
    /// The subset that actually reaches Phase A.
    pub(super) process_items: Vec<ProcessItem>,
}

/// Reserves `capacity` entries, reporting a shortfall as a domain error.
///
/// A plain `Vec::with_capacity` would abort the process on a corpus large
/// enough to exhaust the allocator; `try_reserve` turns the same condition
/// into an exit code the caller can act on.
fn reserve<T>(capacity: usize, what: &str) -> Result<Vec<T>, AppError> {
    let mut v: Vec<T> = Vec::new();
    v.try_reserve(capacity).map_err(|_| {
        AppError::LimitExceeded(crate::i18n::errors_ops::allocation_would_exceed_memory(
            capacity, what,
        ))
    })?;
    Ok(v)
}

/// Builds the name plan for `files`.
///
/// # Errors
/// Returns [`AppError::Validation`] when `--name-prefix` cannot fit the name
/// budget, and [`AppError::LimitExceeded`] when the plan itself cannot be
/// allocated.
pub(super) fn build_plan(args: &IngestArgs, files: &[PathBuf]) -> Result<IngestPlan, AppError> {
    let files_cap = files.len();
    let mut slots_meta: Vec<SlotMeta> = reserve(
        files_cap,
        crate::i18n::errors_ops::alloc_label_slot_metadata(),
    )?;
    let mut process_items: Vec<ProcessItem> = reserve(
        files_cap,
        crate::i18n::errors_ops::alloc_label_process_items(),
    )?;
    let mut truncations: Vec<(String, String)> = reserve(
        files_cap,
        crate::i18n::errors_ops::alloc_label_truncation_entries(),
    )?;

    // v1.1.1 (P12): validate the prefix once and shrink the derived-name
    // budget so `prefix + derived` always fits MAX_MEMORY_NAME_LEN.
    let max_name_length = match args.name_prefix.as_deref() {
        Some(prefix) => validate_name_prefix(prefix, args.max_name_length)?,
        None => args.max_name_length,
    };

    let mut taken_names: BTreeSet<String> = BTreeSet::new();
    for path in files {
        plan_one(
            path,
            args,
            max_name_length,
            &mut taken_names,
            &mut slots_meta,
            &mut process_items,
            &mut truncations,
        );
    }

    if !truncations.is_empty() {
        tracing::info!(
            target: "ingest",
            count = truncations.len(),
            max_name_length = max_name_length,
            max_len = DERIVED_NAME_MAX_LEN,
            "derived names truncated; pass -vv (debug) for per-file detail"
        );
    }

    Ok(IngestPlan {
        slots_meta,
        process_items,
    })
}

/// Resolves one file into a slot, appending to the plan vectors in place.
fn plan_one(
    path: &Path,
    args: &IngestArgs,
    max_name_length: usize,
    taken_names: &mut BTreeSet<String>,
    slots_meta: &mut Vec<SlotMeta>,
    process_items: &mut Vec<ProcessItem>,
    truncations: &mut Vec<(String, String)>,
) {
    let file_str = path.to_string_lossy().into_owned();
    let (derived_base, name_truncated, original_name) = derive_kebab_name(path, max_name_length);
    let original_basename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    if name_truncated {
        if let Some(ref orig) = original_name {
            truncations.push((orig.clone(), derived_base.clone()));
        }
    }

    if derived_base.is_empty() {
        // original_filename: always include when it differs from the empty derived name
        let orig_filename = if original_basename.is_empty() {
            None
        } else {
            Some(original_basename.to_string())
        };
        slots_meta.push(SlotMeta::Skip {
            file_str,
            derived_base: String::new(),
            name_truncated: false,
            original_name: None,
            original_filename: orig_filename,
            reason: "could not derive a non-empty kebab-case name from filename".to_string(),
        });
        return;
    }

    // v1.1.1 (P12): prefix applied AFTER kebab normalization of the
    // basename; the shrunken budget above guarantees the final length
    // fits MAX_MEMORY_NAME_LEN.
    let derived_base = match args.name_prefix.as_deref() {
        Some(prefix) => format!("{prefix}{derived_base}"),
        None => derived_base,
    };

    match unique_name(&derived_base, taken_names) {
        Ok(derived_name) => {
            taken_names.insert(derived_name.clone());
            let idx = slots_meta.len();
            // original_filename: present only when the raw basename differs from the derived name
            let orig_filename = if original_basename == derived_name {
                None
            } else {
                Some(original_basename.to_string())
            };
            process_items.push(ProcessItem {
                idx,
                path: path.to_path_buf(),
                file_str: file_str.clone(),
                derived_name: derived_name.clone(),
            });
            slots_meta.push(SlotMeta::Process {
                file_str,
                derived_name,
                name_truncated,
                original_name,
                original_filename: orig_filename,
            });
        }
        Err(e) => {
            let orig_filename = if original_basename == derived_base {
                None
            } else {
                Some(original_basename.to_string())
            };
            slots_meta.push(SlotMeta::Skip {
                file_str,
                derived_base,
                name_truncated,
                original_name,
                original_filename: orig_filename,
                reason: e.to_string(),
            });
        }
    }
}
