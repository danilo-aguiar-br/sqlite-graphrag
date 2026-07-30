//! Shared LLM CLI spawn backend surface (GAP-SG-126).
//!
//! Thin trait covering the common binary-resolution and child-spawn entry
//! points used by `claude_runner` and `codex_spawn`. Full command construction,
//! OAuth guards, and output parsing stay in the per-backend command modules;
//! this module only extracts the duplicated adapter surface so call sites can
//! depend on a single trait instead of ad-hoc `which` + `Command::new` pairs.
//!
//! Prefer compile-green, minimal surface area over a full spawn redesign.

use crate::errors::AppError;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

/// Common surface for LLM CLI spawn adapters (`claude`, `codex`, …).
///
/// Implementations resolve the backend binary and spawn a prepared
/// [`Command`]. Argument construction and stdout parsing remain outside
/// this trait (see [`super::VersionAdapter`] for version/flag adaptation).
pub trait LlmSpawnBackend: Send + Sync {
    /// Logical backend name (e.g. `"claude"`, `"codex"`).
    fn name(&self) -> &'static str;

    /// Default binary basename looked up on `PATH` when no override is set.
    fn default_binary_name(&self) -> &'static str;

    /// Resolve the executable path, honouring an optional absolute/relative override.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::BinaryNotFound`] when neither the override nor the
    /// default basename can be located via `which`.
    fn resolve_binary(&self, override_path: Option<&Path>) -> Result<PathBuf, AppError> {
        let candidate = match override_path {
            Some(p) => p.to_path_buf(),
            None => PathBuf::from(self.default_binary_name()),
        };
        which::which(&candidate).map_err(|_| AppError::BinaryNotFound {
            name: candidate.display().to_string(),
        })
    }

    /// Spawn a prepared command under the shared process policy.
    ///
    /// Default implementation uses plain [`Command::spawn`]. Call sites that
    /// need `setsid` / `RLIMIT_AS` should continue to use
    /// `commands::claude_runner::spawn_with_memory_limit` until that helper is
    /// relocated into this module (follow-up).
    fn spawn_child(&self, cmd: &mut Command) -> std::io::Result<Child> {
        cmd.spawn()
    }
}

/// Claude Code (`claude`) spawn backend stub.
///
/// Wires binary resolution for the Claude CLI. Command construction lives in
/// [`crate::commands::claude_runner`].
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeSpawnBackend;

impl LlmSpawnBackend for ClaudeSpawnBackend {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn default_binary_name(&self) -> &'static str {
        "claude"
    }
}

/// Codex CLI (`codex`) spawn backend stub.
///
/// Wires binary resolution for the Codex CLI. Command construction lives in
/// [`crate::commands::codex_spawn`].
#[derive(Debug, Default, Clone, Copy)]
pub struct CodexSpawnBackend;

impl LlmSpawnBackend for CodexSpawnBackend {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn default_binary_name(&self) -> &'static str {
        "codex"
    }
}

/// OpenCode headless (`opencode`) spawn backend stub.
///
/// Wires binary resolution for the OpenCode CLI. Command construction lives in
/// [`crate::commands::opencode_runner`].
#[derive(Debug, Default, Clone, Copy)]
pub struct OpencodeSpawnBackend;

impl LlmSpawnBackend for OpencodeSpawnBackend {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn default_binary_name(&self) -> &'static str {
        "opencode"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_backend_names() {
        let b = ClaudeSpawnBackend;
        assert_eq!(b.name(), "claude");
        assert_eq!(b.default_binary_name(), "claude");
    }

    #[test]
    fn codex_backend_names() {
        let b = CodexSpawnBackend;
        assert_eq!(b.name(), "codex");
        assert_eq!(b.default_binary_name(), "codex");
    }

    #[test]
    fn opencode_backend_names() {
        let b = OpencodeSpawnBackend;
        assert_eq!(b.name(), "opencode");
        assert_eq!(b.default_binary_name(), "opencode");
    }

    #[test]
    fn resolve_binary_missing_returns_binary_not_found() {
        let b = ClaudeSpawnBackend;
        let err = b
            .resolve_binary(Some(Path::new(
                "/nonexistent/sqlite-graphrag-no-such-llm-binary",
            )))
            .unwrap_err();
        match err {
            AppError::BinaryNotFound { name } => {
                assert!(name.contains("sqlite-graphrag-no-such-llm-binary"));
            }
            other => panic!("expected BinaryNotFound, got {other:?}"),
        }
    }
}
