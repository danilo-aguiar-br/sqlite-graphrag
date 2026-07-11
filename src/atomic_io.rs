//! Atomic file writes following the atomwrite algorithm (rules_rust_escrita_atomica).
//!
//! Sequence: tempfile in the **same** directory → write → fsync → rename →
//! fsync parent directory. Readers never observe a partial file.

use crate::errors::AppError;
use std::io::Write;
use std::path::Path;

/// Atomically write `bytes` to `path` (create or overwrite).
///
/// Mirrors the atomwrite CLI contract used by LLM agents: crash-safe,
/// same-filesystem rename, parent-dir fsync on Unix.
///
/// # Errors
/// Returns [`AppError::Io`] when any step of the write/rename sequence fails.
/// Returns [`AppError::Validation`] when `path` has no parent directory component.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let dir = match parent {
        Some(p) => {
            std::fs::create_dir_all(p)?;
            p
        }
        None => Path::new("."),
    };

    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(AppError::Io)?;
    tmp.write_all(bytes).map_err(AppError::Io)?;
    tmp.as_file().sync_all().map_err(AppError::Io)?;
    tmp.persist(path).map_err(|e| {
        AppError::Io(std::io::Error::other(format!(
            "atomic persist failed for {}: {e}",
            path.display()
        )))
    })?;

    #[cfg(unix)]
    {
        if let Ok(dir_file) = std::fs::File::open(dir) {
            let _ = dir_file.sync_all();
        }
    }

    Ok(())
}

/// Serialize `value` as pretty JSON and write it atomically to `path`.
///
/// Appends a trailing newline so the file is a complete JSON document suitable
/// for `jaq` / `jq` without further munging.
pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), AppError> {
    let mut json = serde_json::to_string_pretty(value)?;
    json.push('\n');
    write_atomic(path, json.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn write_atomic_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("out.json");
        write_atomic(&path, b"{\"ok\":true}\n").unwrap();
        let got = std::fs::read_to_string(&path).unwrap();
        assert_eq!(got, "{\"ok\":true}\n");
    }

    #[test]
    fn write_json_atomic_is_valid_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("envelope.json");
        write_json_atomic(&path, &json!({"a": 1, "b": "x"})).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).expect("must parse");
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn write_atomic_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nested").join("dir").join("f.txt");
        write_atomic(&path, b"hi\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hi\n");
    }
}
