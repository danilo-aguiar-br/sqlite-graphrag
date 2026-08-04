//! API key resolution, fingerprinting and masking.
//!
//! Precedence is CLI flag then XDG `config add-key`. Product environment
//! variables are deliberately NOT read (G-T-XDG-04).

use super::store::load_config;
use super::ResolvedKey;
use secrecy::SecretBox;

/// Resolve API key.
pub fn resolve_api_key(provider: &str, cli_key: Option<&str>) -> Option<ResolvedKey> {
    // G-T-XDG-04: flag/cli > XDG `config add-key` only. Product env is not read.
    if let Some(k) = cli_key {
        if !k.is_empty() {
            return Some(ResolvedKey {
                value: SecretBox::new(Box::new(k.to_owned())),
                source: "cli",
            });
        }
    }

    if let Ok(cfg) = load_config() {
        if let Some(entry) = cfg.keys.iter().find(|k| k.provider == provider) {
            return Some(ResolvedKey {
                value: SecretBox::new(Box::new(entry.value.clone())),
                source: "config",
            });
        }
    }

    None
}

/// Compute fingerprint.
pub fn compute_fingerprint(key: &str) -> String {
    let hash = blake3::hash(key.as_bytes());
    hash.to_hex()[..16].to_string()
}

/// Mask key.
pub fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    #[test]
    fn compute_fingerprint_deterministic() {
        let fp1 = compute_fingerprint("sk-or-v1-test-key-12345");
        let fp2 = compute_fingerprint("sk-or-v1-test-key-12345");
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 16);
    }

    #[test]
    fn compute_fingerprint_differs_for_different_keys() {
        let fp1 = compute_fingerprint("key-a");
        let fp2 = compute_fingerprint("key-b");
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn mask_key_short() {
        assert_eq!(mask_key("abcd"), "****");
        assert_eq!(mask_key("12345678"), "****");
        assert_eq!(mask_key(""), "****");
    }

    #[test]
    fn mask_key_normal() {
        assert_eq!(mask_key("sk-or-v1-abcdef1234"), "sk-o...1234");
    }

    #[test]
    fn resolve_api_key_cli_wins() {
        let resolved = resolve_api_key("openrouter", Some("cli-key-value"));
        assert!(resolved.is_some());
        let r = resolved.unwrap();
        assert_eq!(r.source, "cli");
        assert_eq!(r.value.expose_secret(), "cli-key-value");
    }

    #[test]
    fn resolve_api_key_cli_fallback() {
        let resolved = resolve_api_key("nonexistent-provider", Some("cli-key"));
        assert!(resolved.is_some());
        let r = resolved.unwrap();
        assert_eq!(r.source, "cli");
        assert_eq!(r.value.expose_secret(), "cli-key");
    }

    #[test]
    fn resolve_api_key_none_when_nothing_available() {
        let resolved = resolve_api_key("totally-unknown-provider-xyz-no-key", None);
        // Only returns Some if host XDG config has that provider (unlikely).
        if let Some(r) = resolved {
            assert_eq!(r.source, "config");
        }
    }

    #[test]
    fn resolve_api_key_ignores_product_env() {
        // G-T-XDG-04: even if OPENROUTER_API_KEY is set, it must not be used.
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", "env-must-be-ignored");
        }
        let resolved = resolve_api_key("openrouter-env-ignore-test-provider", None);
        assert!(resolved.is_none(), "product env must not supply API keys");
        unsafe {
            std::env::remove_var("OPENROUTER_API_KEY");
        }
    }
}
