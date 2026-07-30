//! Binary resolution for headless LLM embedding CLIs.

/// Follows symlinks and shell-script shim `exec` targets to find
/// the real ELF binary. Shim wrappers (like `~/.graphrag-shim/codex`)
/// can strip hardening flags; bypassing them is a security requirement.
pub fn resolve_real_binary(path: &std::path::Path) -> std::path::PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        if is_elf_binary(&canonical) {
            return canonical;
        }
        if let Some(exec_target) = extract_exec_target_from_shim(&canonical) {
            if exec_target.exists() && is_elf_binary(&exec_target) {
                return exec_target;
            }
        }
        return canonical;
    }
    path.to_path_buf()
}

fn is_elf_binary(path: &std::path::Path) -> bool {
    std::fs::read(path)
        .map(|bytes| bytes.len() >= 4 && bytes[..4] == [0x7f, b'E', b'L', b'F'])
        .unwrap_or(false)
}

fn extract_exec_target_from_shim(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let content = std::fs::read_to_string(path).ok()?;
    if !content.starts_with("#!") {
        return None;
    }
    for line in content.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with("exec ") {
            let after_exec = trimmed.strip_prefix("exec ")?;
            let binary = after_exec.split_whitespace().next()?;
            return Some(std::path::PathBuf::from(binary));
        }
    }
    None
}
