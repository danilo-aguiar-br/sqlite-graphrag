//! Codex CLI binary discovery and version validation.

use super::types::MIN_CODEX_VERSION;
use crate::errors::AppError;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 3. PATH search for `codex` (or `codex.exe` on Windows).
pub fn find_codex_binary(explicit: Option<&Path>) -> Result<PathBuf, AppError> {
    if let Some(p) = explicit {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        return Err(AppError::Validation(
            crate::i18n::validation::binary_not_found_at_path(
                "Codex CLI",
                &p.display().to_string(),
            ),
        ));
    }

    if let Some(env_path) = crate::runtime_config::codex_binary() {
        let p = PathBuf::from(&env_path);
        if p.exists() {
            return Ok(p);
        }
    }

    let name = if cfg!(windows) { "codex.exe" } else { "codex" };
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err(AppError::Validation(
        "Codex CLI binary not found in PATH. Install it from https://github.com/openai/codex or specify --codex-binary".to_string(),
    ))
}

/// Validates that the Codex CLI binary meets the minimum version requirement.
///
/// # Errors
///
/// Returns `AppError::Validation` when the binary cannot be executed or the
/// version is below `MIN_CODEX_VERSION`.
pub(crate) fn validate_codex_version(binary: &Path) -> Result<String, AppError> {
    let resolved = which::which(binary).map_err(|_| {
        AppError::Validation(crate::i18n::validation::executable_not_in_path(
            &binary.display().to_string(),
        ))
    })?;
    let output = Command::new(&resolved)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(AppError::Io)?;

    let raw = String::from_utf8(output.stdout)
        .map_err(|_| AppError::Validation(crate::i18n::validation::codex_version_not_utf8()))?;

    let version_str = raw.trim().to_string();

    // Codex CLI outputs: "codex-cli 0.133.0" or just "0.133.0"
    let numeric = version_str.split_whitespace().last().unwrap_or("").trim();

    fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
        let parts: Vec<&str> = s.splitn(3, '.').collect();
        if parts.len() < 2 {
            return None;
        }
        let major = parts[0].parse::<u64>().ok()?;
        let minor = parts[1].parse::<u64>().ok()?;
        let patch = parts
            .get(2)
            .and_then(|p| p.parse::<u64>().ok())
            .unwrap_or(0);
        Some((major, minor, patch))
    }

    if let (Some(actual), Some(min)) = (parse_semver(numeric), parse_semver(MIN_CODEX_VERSION)) {
        if actual < min {
            return Err(AppError::Validation(
                crate::i18n::validation::version_below_minimum(
                    "Codex CLI",
                    numeric,
                    MIN_CODEX_VERSION,
                ),
            ));
        }
    }

    Ok(version_str)
}
