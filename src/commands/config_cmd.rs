use crate::config::{self, compute_fingerprint, mask_key, ApiKeyEntry};
use crate::errors::AppError;
use clap::{Args, Subcommand};
use serde_json::json;
use std::io::{self, Read};

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Add an API key for a provider (reads from stdin to avoid shell history).
    AddKey {
        #[arg(long)]
        provider: String,
        #[arg(long, default_value_t = true)]
        from_stdin: bool,
        /// GAP-SG-34: no-op; JSON is always emitted on stdout.
        #[arg(long, hide = true)]
        json: bool,
    },
    /// List stored API keys (masked) with fingerprints.
    ListKeys {
        /// GAP-SG-34: no-op; JSON is always emitted on stdout.
        #[arg(long, hide = true)]
        json: bool,
    },
    /// Remove an API key by its fingerprint.
    RemoveKey {
        fingerprint: String,
        /// GAP-SG-34: no-op; JSON is always emitted on stdout.
        #[arg(long, hide = true)]
        json: bool,
    },
    /// Diagnose key resolution layers (flag/cli and XDG config; product env deprecated).
    Doctor {
        /// GAP-SG-34: no-op; JSON is always emitted on stdout.
        #[arg(long, hide = true)]
        json: bool,
    },
    /// Print the resolved XDG config file path.
    Path {
        /// GAP-SG-34: no-op; JSON is always emitted on stdout.
        #[arg(long, hide = true)]
        json: bool,
    },
    /// Set an operational setting in XDG config (G-T-XDG-01).
    ///
    /// Known keys (non-exhaustive):
    /// `enrich.preserve_threshold`, `enrich.entity_description.domain`,
    /// `enrich.entity_description.grounding_threshold`,
    /// `enrich.entity_connect.default_limit`,
    /// `enrich.entity_connect.max_runtime_secs`,
    /// `network.openrouter.chat_url` (alias `network.chat_url`),
    /// `network.openrouter.embeddings_url` (alias `network.embed_url`),
    /// `log.level`, `log.format`, `display.tz`,
    /// `embedding.dim`, `llm.concurrency`.
    Set {
        /// Dotted key name, e.g. `enrich.preserve_threshold`.
        key: String,
        /// Value as string (parsed by consumers).
        value: String,
        #[arg(long, hide = true)]
        json: bool,
    },
    /// Get an operational setting from XDG config.
    Get {
        key: String,
        #[arg(long, hide = true)]
        json: bool,
    },
    /// List all operational settings (no secrets).
    List {
        /// Include well-known defaults even when not stored in XDG.
        #[arg(long, default_value_t = false)]
        effective: bool,
        #[arg(long, hide = true)]
        json: bool,
    },
    /// Unset an operational setting.
    Unset {
        key: String,
        #[arg(long, hide = true)]
        json: bool,
    },
}

pub fn run(args: ConfigArgs) -> Result<(), AppError> {
    match args.action {
        ConfigAction::AddKey {
            provider,
            from_stdin,
            json: _,
        } => {
            let key = if from_stdin {
                let mut buf = String::new();
                io::stdin().read_to_string(&mut buf).map_err(AppError::Io)?;
                buf.trim().to_string()
            } else {
                return Err(AppError::Validation(
                    "--from-stdin is required to avoid shell history exposure".into(),
                ));
            };
            if key.is_empty() {
                return Err(AppError::Validation("API key cannot be empty".into()));
            }
            let fingerprint = compute_fingerprint(&key);
            let entry = ApiKeyEntry {
                provider: provider.clone(),
                value: key,
                added_at: chrono::Utc::now().to_rfc3339(),
                fingerprint: fingerprint.clone(),
            };
            let mut cfg = config::load_config()?;
            cfg.keys.retain(|k| k.provider != provider);
            cfg.keys.push(entry);
            config::save_config(&cfg)?;
            let output = json!({
                "action": "key_added",
                "provider": provider,
                "fingerprint": fingerprint,
            });
            println!("{}", serde_json::to_string(&output)?);
            Ok(())
        }
        ConfigAction::ListKeys { json: _ } => {
            let cfg = config::load_config()?;
            let keys: Vec<_> = cfg
                .keys
                .iter()
                .map(|k| {
                    json!({
                        "provider": k.provider,
                        "fingerprint": k.fingerprint,
                        "masked_value": mask_key(&k.value),
                        "added_at": k.added_at,
                    })
                })
                .collect();
            let output = json!({ "keys": keys });
            println!("{}", serde_json::to_string_pretty(&output)?);
            Ok(())
        }
        ConfigAction::RemoveKey {
            fingerprint,
            json: _,
        } => {
            let mut cfg = config::load_config()?;
            let before = cfg.keys.len();
            cfg.keys.retain(|k| k.fingerprint != fingerprint);
            if cfg.keys.len() == before {
                return Err(AppError::NotFound(format!(
                    "no key with fingerprint {fingerprint}"
                )));
            }
            config::save_config(&cfg)?;
            let output = json!({
                "action": "key_removed",
                "fingerprint": fingerprint,
            });
            println!("{}", serde_json::to_string(&output)?);
            Ok(())
        }
        ConfigAction::Doctor { json: _ } => {
            let config_path = config::config_file_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "unavailable".to_string());
            let config_exists = std::path::Path::new(&config_path).exists();
            let providers = ["openrouter"];
            let mut results = vec![];
            for provider in &providers {
                let resolved = config::resolve_api_key(provider, None);
                results.push(json!({
                    "provider": provider,
                    "resolved": resolved.is_some(),
                    "source": resolved.as_ref().map(|r| r.source),
                    "masked_value": resolved.as_ref().map(|r| {
                        use secrecy::ExposeSecret;
                        mask_key(r.value.expose_secret())
                    }),
                }));
            }
            // Wave 4: operational knobs with source layer (flag|xdg|default).
            // Product env is never a source.
            let knob = |key: &str, default: &str, runtime_flag: Option<&str>| {
                let source = if runtime_flag.map(|s| !s.is_empty()).unwrap_or(false) {
                    "flag"
                } else if config::get_setting(key)
                    .ok()
                    .flatten()
                    .map(|v| !v.is_empty())
                    .unwrap_or(false)
                {
                    "xdg"
                } else {
                    "default"
                };
                let value = crate::runtime_config::resolve_string(runtime_flag, key, default);
                json!({ "key": key, "value": value, "source": source })
            };
            let rt = crate::runtime_config::get();
            let knobs = vec![
                knob(
                    "enrich.entity_description.quality_sample",
                    "50",
                    None,
                ),
                knob(
                    "enrich.entity_description.grounding_threshold",
                    "0.12",
                    None,
                ),
                knob("enrich.entity_connect.default_limit", "100", None),
                knob("enrich.entity_connect.large_ns_limit", "25", None),
                knob("enrich.yield_every_n_items", "10", None),
                knob("namespace.default", "global", None),
                knob("display.tz", "UTC", rt.display_tz.as_deref()),
                knob("i18n.lang", "en", rt.lang.as_deref()),
                knob("log.level", "warn", rt.log_level.as_deref()),
                knob(
                    "llm.claude_binary",
                    "",
                    rt.claude_binary.as_deref(),
                ),
                knob("llm.codex_binary", "", rt.codex_binary.as_deref()),
                knob(
                    "llm.opencode_binary",
                    "",
                    rt.opencode_binary.as_deref(),
                ),
                knob("paths.cache", "", None),
                knob("embedding.dim", "384", None),
            ];
            let output = json!({
                "config_path": config_path,
                "config_exists": config_exists,
                "providers": results,
                "knobs": knobs,
                "product_env_reads": false,
                "note": "Precedence: CLI flag > XDG config set > named default. No SQLITE_GRAPHRAG_* product env.",
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            Ok(())
        }
        ConfigAction::Path { json: _ } => {
            let path = config::config_file_path()?;
            let output = json!({
                "config_path": path.display().to_string(),
                "exists": path.exists(),
            });
            println!("{}", serde_json::to_string(&output)?);
            Ok(())
        }
        ConfigAction::Set {
            key,
            value,
            json: _,
        } => {
            config::set_setting(&key, &value)?;
            let output = json!({
                "action": "setting_set",
                "key": key,
                "value": value,
            });
            println!("{}", serde_json::to_string(&output)?);
            Ok(())
        }
        ConfigAction::Get { key, json: _ } => {
            let value = config::get_setting(&key)?;
            let output = json!({
                "key": key,
                "value": value,
                "found": value.is_some(),
            });
            println!("{}", serde_json::to_string(&output)?);
            Ok(())
        }
        ConfigAction::List { effective, json: _ } => {
            let mut settings = config::list_settings()?;
            if effective {
                let defaults: &[(&str, &str)] = &[
                    (
                        "network.openrouter.chat_url",
                        crate::constants::DEFAULT_OPENROUTER_CHAT_URL,
                    ),
                    (
                        "network.openrouter.embeddings_url",
                        crate::constants::DEFAULT_OPENROUTER_EMBEDDINGS_URL,
                    ),
                    ("llm.probe_timeout_ms", "800"),
                    ("llm.fallback", "codex,claude,none"),
                    ("embedding.dim", "384"),
                    ("log.level", "info"),
                    ("display.tz", "UTC"),
                ];
                for (k, v) in defaults {
                    settings.entry(k.to_string()).or_insert_with(|| v.to_string());
                }
            }
            let output = json!({
                "settings": settings,
                "effective": effective,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            Ok(())
        }
        ConfigAction::Unset { key, json: _ } => {
            let removed = config::unset_setting(&key)?;
            let output = json!({
                "action": "setting_unset",
                "key": key,
                "removed": removed,
            });
            println!("{}", serde_json::to_string(&output)?);
            Ok(())
        }
    }
}
