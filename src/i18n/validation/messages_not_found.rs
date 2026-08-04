//! GAP-SG-143: canonical English texts for every [`crate::errors::AppError::NotFound`]
//! payload the crate builds.
//!
//! # Why these are English-only
//!
//! Unlike the `AppError::Validation` catalogs such as [`super::messages_naming`], which
//! resolves [`crate::i18n::current`] at construction because
//! [`super::app_error_pt::validation`] only prefixes its argument, the
//! `NotFound` payload is translated at DISPLAY time by
//! [`super::app_error_pt::not_found`]. Resolving the locale here would break
//! that layer twice over: `localized_message_for(Language::English)` would
//! return Portuguese whenever the process locale is `pt-BR`, and the
//! replace-chain would run over text that no longer matches its English
//! patterns.
//!
//! So these functions own the ENGLISH wording, in one place, and
//! `app_error_pt::not_found` owns the Portuguese. The pairing is enforced by
//! `not_found_catalog_has_no_english_residue_in_pt` in `src/i18n/tests.rs`,
//! which pushes every builder below through the translation chain and fails on
//! any surviving English.
//!
//! Adding a builder here without extending the chain therefore fails the test
//! rather than shipping a bilingual hybrid — the exact regression GAP-SG-143
//! recorded.

// ── memories ──────────────────────────────────────────────────────────

/// A memory name did not resolve, in a context that has no namespace to report.
pub fn memory_named_not_found(name: &str) -> String {
    format!("memory '{name}' not found")
}

/// A memory name did not resolve inside a specific namespace.
pub fn memory_named_not_found_in_namespace(name: &str, namespace: &str) -> String {
    format!("memory '{name}' not found in namespace '{namespace}'")
}

/// A memory id exists but is owned by a different namespace than the one asked
/// for. Distinct from a plain miss: the row is there, the caller is not
/// entitled to it, and naming both namespaces is what makes that actionable.
pub fn memory_id_in_other_namespace(id: i64, actual: &str, requested: &str) -> String {
    format!("memory id {id} exists but belongs to namespace '{actual}', not '{requested}'")
}

// ── entities ──────────────────────────────────────────────────────────

/// An entity name did not resolve, in a context that has no namespace to report.
pub fn entity_named_not_found(name: &str) -> String {
    format!("entity '{name}' not found")
}

/// An entity name did not resolve inside a specific namespace.
pub fn entity_named_not_found_in_namespace(name: &str, namespace: &str) -> String {
    format!("entity '{name}' not found in namespace '{namespace}'")
}

/// An entity name did not resolve, but near-matches exist.
///
/// `suggestions` is rendered by the caller as a comma-separated list so this
/// function stays free of formatting policy.
pub fn entity_named_not_found_with_suggestions(
    name: &str,
    namespace: &str,
    suggestions: &str,
) -> String {
    format!(
        "entity '{name}' not found in namespace '{namespace}'. Did you mean: {suggestions}? \
         Re-run with --fuzzy to auto-resolve a clear match, or pass the canonical name."
    )
}

/// An entity id did not resolve inside a specific namespace.
pub fn entity_id_not_found_in_namespace(id: i64, namespace: &str) -> String {
    format!("entity id={id} not found in namespace '{namespace}'")
}

/// The SOURCE endpoint of an edge did not resolve.
///
/// Kept distinct from [`entity_named_not_found_in_namespace`] because an
/// operator fixing a relation needs to know WHICH endpoint is missing.
pub fn source_entity_not_found_in_namespace(name: &str, namespace: &str) -> String {
    format!("source entity '{name}' not found in namespace '{namespace}'")
}

/// The TARGET endpoint of an edge did not resolve.
pub fn target_entity_not_found_in_namespace(name: &str, namespace: &str) -> String {
    format!("target entity '{name}' not found in namespace '{namespace}'")
}

// ── graph edges and chunks ────────────────────────────────────────────

/// Both endpoints resolved but no edge carries `relation` between them.
pub fn edge_not_found_in_namespace(
    source: &str,
    relation: &str,
    target: &str,
    namespace: &str,
) -> String {
    format!("edge '{source}' --[{relation}]--> '{target}' not found in namespace '{namespace}'")
}

/// A relationship row id did not resolve.
pub fn relationship_id_not_found(id: i64) -> String {
    format!("relationship {id} not found")
}

/// A chunk id did not resolve inside a specific namespace.
pub fn chunk_id_not_found_in_namespace(id: i64, namespace: &str) -> String {
    format!("chunk {id} not found in namespace '{namespace}'")
}

// ── host-side records ─────────────────────────────────────────────────

/// A concurrency slot was asked to be released but holds no lock file.
pub fn slot_not_held(slot_id: u32, path: &str) -> String {
    format!("slot {slot_id} is not held (no file at {path})")
}

/// No stored API key matches the given fingerprint.
pub fn api_key_fingerprint_not_found(fingerprint: &str) -> String {
    format!("no key with fingerprint {fingerprint}")
}

/// A `pending_memories` row id did not resolve.
pub fn pending_id_not_found(pending_id: i64) -> String {
    format!("pending_id {pending_id} not found in pending_memories")
}

/// Every builder in this module, evaluated with placeholder arguments.
///
/// Exposed so `src/i18n/tests.rs` can assert the Portuguese chain covers the
/// whole catalog instead of whichever entries someone remembered to list.
/// A new builder that is not added here is caught by
/// `not_found_catalog_covers_every_builder`, which compares this length
/// against the count of `pub fn` in the module source.
#[cfg(test)]
pub(crate) fn catalog_samples() -> Vec<String> {
    vec![
        memory_named_not_found("alpha"),
        memory_named_not_found_in_namespace("alpha", "global"),
        memory_id_in_other_namespace(7, "other", "global"),
        entity_named_not_found("beta"),
        entity_named_not_found_in_namespace("beta", "global"),
        entity_named_not_found_with_suggestions("beta", "global", "beta-one, beta-two"),
        entity_id_not_found_in_namespace(9, "global"),
        source_entity_not_found_in_namespace("beta", "global"),
        target_entity_not_found_in_namespace("gamma", "global"),
        edge_not_found_in_namespace("beta", "uses", "gamma", "global"),
        relationship_id_not_found(42),
        chunk_id_not_found_in_namespace(3, "global"),
        slot_not_held(2, "/tmp/slot-2.lock"),
        api_key_fingerprint_not_found("ab12cd34"),
        pending_id_not_found(11),
    ]
}
