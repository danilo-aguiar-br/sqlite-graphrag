//! Claude Code binary discovery and version validation.

use super::types::MIN_CLAUDE_VERSION;
use crate::errors::AppError;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Locates the Claude Code binary on the system.
pub fn find_claude_binary(explicit: Option<&Path>) -> Result<PathBuf, AppError> {
    if let Some(p) = explicit {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        return Err(AppError::Validation(
            crate::i18n::validation::binary_not_found_at_path(
                "Claude Code",
                &p.display().to_string(),
            ),
        ));
    }

    if let Some(env_path) = crate::runtime_config::claude_binary() {
        let p = PathBuf::from(&env_path);
        if p.exists() {
            return Ok(p);
        }
    }

    let name = if cfg!(windows) {
        "claude.exe"
    } else {
        "claude"
    };
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err(AppError::Validation(
        "Claude Code binary not found in PATH. Install it from https://docs.anthropic.com/claude-code or specify --claude-binary".to_string(),
    ))
}

/// Validates that the Claude Code binary meets the minimum version.
pub(crate) fn validate_claude_version(binary: &Path) -> Result<String, AppError> {
    let output = Command::new(binary)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(AppError::Io)?;

    if !output.status.success() {
        return Err(AppError::Validation(
            "failed to run 'claude --version'".to_string(),
        ));
    }

    let version_str = String::from_utf8(output.stdout)
        .map_err(|_| AppError::Validation(crate::i18n::validation::claude_version_not_utf8()))?;
    let version = version_str.trim().to_string();

    // Extract the numeric version part before first space or paren, e.g. "2.1.149 (Claude Code)" -> "2.1.149"
    let numeric = version.split([' ', '(']).next().unwrap_or("").trim();

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

    if let (Some(actual), Some(min)) = (parse_semver(numeric), parse_semver(MIN_CLAUDE_VERSION)) {
        if actual < min {
            return Err(AppError::Validation(
                crate::i18n::validation::version_below_minimum(
                    "Claude Code",
                    numeric,
                    MIN_CLAUDE_VERSION,
                ),
            ));
        }
    }

    Ok(version)
}
