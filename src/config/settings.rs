//! Read and write of operational settings (`config get/set/unset/list`).
//!
//! The precedence itself (flag > XDG > default) is applied by callers through
//! [`crate::runtime_config`]; this module only owns the XDG layer.

use super::registry::{is_known_setting, nearest_setting_key, LEGACY_SETTING_KEYS, SETTING_KEYS};
use super::store::{load_config, save_config};
use crate::errors::AppError;
use crate::i18n::validation;

/// Read an operational setting from XDG config (flag > XDG > default is
/// applied by callers). Returns `None` when unset.
pub fn get_setting(key: &str) -> Result<Option<String>, AppError> {
    let cfg = load_config()?;
    if let Some(v) = cfg.settings.get(key) {
        return Ok(Some(v.clone()));
    }
    // GAP-SG-122: when the canonical key is missing, fall back to any retired
    // alias that still maps onto it (e.g. paths.cache → cache.dir).
    for (legacy, replacement) in LEGACY_SETTING_KEYS {
        if *replacement == key {
            if let Some(v) = cfg.settings.get(*legacy) {
                return Ok(Some(v.clone()));
            }
        }
    }
    Ok(None)
}

/// Persist an operational setting in XDG config.toml (G-T-XDG-01).
pub fn set_setting(key: &str, value: &str) -> Result<(), AppError> {
    if key.trim().is_empty() {
        return Err(AppError::Validation("config key must be non-empty".into()));
    }
    // GAP-SG-80: an unvalidated insert persisted typos and obsolete keys while
    // reporting success, so the operator saw the value in `config show` and
    // assumed it took effect. Reject early instead of storing a silent no-op.
    if !is_known_setting(key) {
        if let Some(replacement) = LEGACY_SETTING_KEYS
            .iter()
            .find(|(legacy, _)| *legacy == key)
            .map(|(_, replacement)| *replacement)
        {
            return Err(AppError::Validation(validation::config_key_retired(
                key,
                replacement,
            )));
        }
        return Err(AppError::Validation(validation::config_key_unknown(
            key,
            nearest_setting_key(key),
        )));
    }
    // GAP-SG-201: the key was validated and the VALUE never was, so
    // `embedding.dim nao-numero`, `log.format xml` and `llm.slot_no_wait talvez`
    // all persisted with exit 0 and only misbehaved on a later invocation, far
    // from the command that caused them. `display.tz 0` was the extreme case:
    // it bricked every subsequent run, including the `config unset` that would
    // have undone it (GAP-SG-200).
    //
    // Rejecting here puts the error where the operator can act on it. Keys of
    // kind `Text` have no checkable domain and skip this by construction.
    if let Some(entry) = SETTING_KEYS.iter().find(|entry| entry.key == key) {
        if let Some(expectation) = entry.kind.expectation() {
            if !entry.kind.accepts(value) {
                return Err(AppError::Validation(validation::config_value_invalid(
                    key,
                    value,
                    &expectation,
                )));
            }
        }
    }
    let mut cfg = load_config()?;
    cfg.settings.insert(key.to_string(), value.to_string());
    save_config(&cfg)
}

/// Remove an operational setting from XDG config.toml.
pub fn unset_setting(key: &str) -> Result<bool, AppError> {
    let mut cfg = load_config()?;
    let removed = cfg.settings.remove(key).is_some();
    if removed {
        save_config(&cfg)?;
    }
    Ok(removed)
}

/// List all operational settings (no secrets).
pub fn list_settings() -> Result<std::collections::BTreeMap<String, String>, AppError> {
    let cfg = load_config()?;
    Ok(cfg.settings)
}
