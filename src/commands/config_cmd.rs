use crate::cli_db_noop::DB_NOOP_HELP;
use crate::config::{self, compute_fingerprint, mask_key, ApiKeyEntry};
use crate::errors::AppError;
use clap::{Args, Subcommand};
use serde_json::json;
use std::io::{self, Read};

/// Config args.
#[derive(Debug, Args)]
pub struct ConfigArgs {
    /// Action.
    #[command(subcommand)]
    pub action: ConfigAction,
}

/// Config action.
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Add an API key for a provider (reads from stdin to avoid shell history).
    AddKey {
        /// Provider name.
        #[arg(long)]
        provider: String,
        /// From stdin.
        #[arg(long, default_value_t = true)]
        from_stdin: bool,
        /// GAP-SG-34: no-op; JSON is always emitted on stdout.
        #[arg(long, hide = true)]
        json: bool,
        /// GAP-SG-139: accepted as a no-op for agent uniformity (XDG keys; no graph I/O).
        #[arg(long, value_name = "PATH", help = DB_NOOP_HELP)]
        db: Option<String>,
    },
    /// List stored API keys (masked) with fingerprints.
    ListKeys {
        /// GAP-SG-34: no-op; JSON is always emitted on stdout.
        #[arg(long, hide = true)]
        json: bool,
        /// GAP-SG-139: accepted as a no-op for agent uniformity (XDG keys; no graph I/O).
        #[arg(long, value_name = "PATH", help = DB_NOOP_HELP)]
        db: Option<String>,
    },
    /// Remove an API key by its fingerprint.
    RemoveKey {
        /// Fingerprint.
        fingerprint: String,
        /// GAP-SG-34: no-op; JSON is always emitted on stdout.
        #[arg(long, hide = true)]
        json: bool,
        /// GAP-SG-139: accepted as a no-op for agent uniformity (XDG keys; no graph I/O).
        #[arg(long, value_name = "PATH", help = DB_NOOP_HELP)]
        db: Option<String>,
    },
    /// Diagnose key resolution layers (flag/cli and XDG config; product env deprecated).
    Doctor {
        /// GAP-SG-34: no-op; JSON is always emitted on stdout.
        #[arg(long, hide = true)]
        json: bool,
        /// GAP-SG-139: accepted as a no-op for agent uniformity (XDG keys; no graph I/O).
        #[arg(long, value_name = "PATH", help = DB_NOOP_HELP)]
        db: Option<String>,
    },
    /// Print the resolved XDG config file path.
    Path {
        /// GAP-SG-34: no-op; JSON is always emitted on stdout.
        #[arg(long, hide = true)]
        json: bool,
        /// GAP-SG-139: accepted as a no-op for agent uniformity (XDG keys; no graph I/O).
        #[arg(long, value_name = "PATH", help = DB_NOOP_HELP)]
        db: Option<String>,
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
        /// Emit machine-readable JSON on stdout.
        #[arg(long, hide = true)]
        json: bool,
        /// GAP-SG-139: accepted as a no-op for agent uniformity (XDG settings; no graph I/O).
        #[arg(long, value_name = "PATH", help = DB_NOOP_HELP)]
        db: Option<String>,
    },
    /// Get an operational setting from XDG config.
    Get {
        /// Key.
        key: String,
        /// Emit machine-readable JSON on stdout.
        #[arg(long, hide = true)]
        json: bool,
        /// GAP-SG-139: accepted as a no-op for agent uniformity (XDG settings; no graph I/O).
        #[arg(long, value_name = "PATH", help = DB_NOOP_HELP)]
        db: Option<String>,
    },
    /// List all operational settings (no secrets).
    List {
        /// Include well-known defaults even when not stored in XDG.
        #[arg(long, default_value_t = false)]
        effective: bool,
        /// Emit machine-readable JSON on stdout.
        #[arg(long, hide = true)]
        json: bool,
        /// Emit the JSON Schema for `config list` stdout and exit 0
        /// without reading settings (agent-native R-AN-01).
        #[arg(
            long,
            default_value_t = false,
            help = "Print JSON Schema for config list output and exit"
        )]
        print_schema: bool,
        /// GAP-SG-139: accepted as a no-op for agent uniformity (XDG settings; no graph I/O).
        #[arg(long, value_name = "PATH", help = DB_NOOP_HELP)]
        db: Option<String>,
    },
    /// Unset an operational setting.
    Unset {
        /// Key.
        key: String,
        /// Emit machine-readable JSON on stdout.
        #[arg(long, hide = true)]
        json: bool,
        /// GAP-SG-139: accepted as a no-op for agent uniformity (XDG settings; no graph I/O).
        #[arg(long, value_name = "PATH", help = DB_NOOP_HELP)]
        db: Option<String>,
    },
}

/// Run.
pub fn run(args: ConfigArgs) -> Result<(), AppError> {
    match args.action {
        ConfigAction::AddKey {
            provider,
            from_stdin,
            json: _,
            db: _,
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
                return Err(AppError::Validation(
                    crate::i18n::validation::api_key_cannot_be_empty(),
                ));
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
        ConfigAction::ListKeys { json: _, db: _ } => {
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
            db: _,
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
        ConfigAction::Doctor { json: _, db: _ } => {
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
            // Operational knobs with source layer (flag|xdg|default|derived).
            // Product env is never a source.
            //
            // GAP-SG-85: this listing used to be a hand-written table of 14
            // entries next to a 44-key registry, and `db.path` — the key that
            // redirects the whole database — was one of the missing ones. The
            // list is now DERIVED from `config::SETTING_KEYS`, so a key cannot
            // exist without being discoverable here.
            let rt = crate::runtime_config::get();

            // Only these keys can be overridden by a CLI flag today. Mapping
            // them explicitly keeps the `flag` source honest instead of
            // claiming a flag layer that does not exist for the other keys.
            let flag_for = |key: &str| -> Option<&str> {
                match key {
                    "display.tz" => rt.display_tz.as_deref(),
                    "i18n.lang" => rt.lang.as_deref(),
                    "log.level" => rt.log_level.as_deref(),
                    "log.format" => rt.log_format.as_deref(),
                    "llm.claude_binary" => rt.claude_binary.as_deref(),
                    "llm.codex_binary" => rt.codex_binary.as_deref(),
                    "llm.opencode_binary" => rt.opencode_binary.as_deref(),
                    "llm.model" => rt.llm_model.as_deref(),
                    "llm.fallback" => rt.llm_fallback.as_deref(),
                    "db.path" => rt.db_path.as_deref(),
                    _ => None,
                }
            };

            let knobs: Vec<_> = config::SETTING_KEYS
                .iter()
                .map(|entry| {
                    let runtime_flag = flag_for(entry.key).filter(|v| !v.is_empty());
                    let xdg_value = config::get_setting(entry.key)
                        .ok()
                        .flatten()
                        .filter(|v| !v.is_empty());
                    let (source, value) = match (runtime_flag, xdg_value) {
                        (Some(v), _) => ("flag", Some(v.to_string())),
                        (None, Some(v)) => ("xdg", Some(v)),
                        // A key whose default is derived from the host has no
                        // literal to report; `derived` says so instead of
                        // printing an empty string that reads like "unset".
                        (None, None) => match entry.default {
                            Some(d) => ("default", Some(d.to_string())),
                            None => ("derived", None),
                        },
                    };
                    json!({ "key": entry.key, "value": value, "source": source })
                })
                .collect();
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
        ConfigAction::Path { json: _, db: _ } => {
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
            db: _,
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
        ConfigAction::Get {
            key,
            json: _,
            db: _,
        } => {
            let value = config::get_setting(&key)?;
            let output = json!({
                "key": key,
                "value": value,
                "found": value.is_some(),
            });
            println!("{}", serde_json::to_string(&output)?);
            Ok(())
        }
        ConfigAction::List {
            effective,
            json: _,
            print_schema,
            db: _,
        } => {
            if print_schema {
                return crate::print_schema::emit(crate::print_schema::SchemaId::ConfigList);
            }
            let mut settings = config::list_settings()?;
            if effective {
                // GAP-SG-93: defaults MUST come from constants / named defaults,
                // never hard-coded literals that can drift from DEFAULT_EMBEDDING_DIM.
                let dim_default = crate::constants::DEFAULT_EMBEDDING_DIM.to_string();
                let probe_default = crate::constants::DEFAULT_LLM_PROBE_TIMEOUT_MS.to_string();
                let defaults: &[(&str, &str)] = &[
                    (
                        "network.openrouter.chat_url",
                        crate::constants::DEFAULT_OPENROUTER_CHAT_URL,
                    ),
                    (
                        "network.openrouter.embeddings_url",
                        crate::constants::DEFAULT_OPENROUTER_EMBEDDINGS_URL,
                    ),
                    ("llm.probe_timeout_ms", probe_default.as_str()),
                    ("llm.fallback", "codex,claude,none"),
                    ("embedding.dim", dim_default.as_str()),
                    ("log.level", crate::constants::DEFAULT_LOG_LEVEL),
                    ("display.tz", "UTC"),
                ];
                for (k, v) in defaults {
                    settings
                        .entry(k.to_string())
                        .or_insert_with(|| v.to_string());
                }
            }
            let output = json!({
                "settings": settings,
                "effective": effective,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            Ok(())
        }
        ConfigAction::Unset {
            key,
            json: _,
            db: _,
        } => {
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

#[cfg(test)]
mod tests {
    use clap::Parser;

    #[test]
    fn config_doctor_accepts_db_as_noop() {
        let cli = crate::cli::Cli::try_parse_from([
            "sqlite-graphrag",
            "config",
            "doctor",
            "--db",
            "/tmp/gap-sg-139-sentinel.sqlite",
        ])
        .expect("config doctor must accept --db as a no-op (GAP-SG-139)");

        match cli.command {
            Some(crate::cli::Commands::Config(args)) => match args.action {
                super::ConfigAction::Doctor { db, .. } => {
                    assert_eq!(db.as_deref(), Some("/tmp/gap-sg-139-sentinel.sqlite"));
                }
                other => panic!("expected Doctor, got {other:?}"),
            },
            other => panic!("expected Config, got {other:?}"),
        }
    }

    #[test]
    fn config_list_accepts_db_as_noop() {
        let cli = crate::cli::Cli::try_parse_from([
            "sqlite-graphrag",
            "config",
            "list",
            "--db",
            "/tmp/gap-sg-139-sentinel.sqlite",
        ])
        .expect("config list must accept --db as a no-op (GAP-SG-139)");

        match cli.command {
            Some(crate::cli::Commands::Config(args)) => match args.action {
                super::ConfigAction::List { db, .. } => {
                    assert_eq!(db.as_deref(), Some("/tmp/gap-sg-139-sentinel.sqlite"));
                }
                other => panic!("expected List, got {other:?}"),
            },
            other => panic!("expected Config, got {other:?}"),
        }
    }

    #[test]
    fn config_add_key_accepts_db_as_noop() {
        let cli = crate::cli::Cli::try_parse_from([
            "sqlite-graphrag",
            "config",
            "add-key",
            "--provider",
            "openrouter",
            "--db",
            "/tmp/gap-sg-139-sentinel.sqlite",
        ])
        .expect("config add-key must accept --db as a no-op (GAP-SG-139)");

        match cli.command {
            Some(crate::cli::Commands::Config(args)) => match args.action {
                super::ConfigAction::AddKey { db, .. } => {
                    assert_eq!(db.as_deref(), Some("/tmp/gap-sg-139-sentinel.sqlite"));
                }
                other => panic!("expected AddKey, got {other:?}"),
            },
            other => panic!("expected Config, got {other:?}"),
        }
    }
}
