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
    /// Run `config doctor --json` for the full list of accepted keys with their
    /// defaults. That listing is derived from the registry, so it cannot drift
    /// from what the binary accepts — a static list here can, and did
    /// (GAP-SG-203). The sample below is kept short for that reason; the
    /// authoritative answer is always `config doctor`.
    ///
    /// The value is validated against the key's domain, so a bad timezone or a
    /// non-numeric size is rejected here rather than misbehaving later.
    ///
    /// Sample of accepted keys: `db.path`, `display.tz`, `embedding.dim`,
    /// `embedding.model`, `embedding.backend`, `llm.backend`, `log.level`,
    /// `log.format`, `namespace.default`,
    /// `network.openrouter.chat_url` (alias `network.chat_url`),
    /// `network.openrouter.embeddings_url` (alias `network.embed_url`).
    Set {
        /// Dotted key name, e.g. `display.tz`.
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
                // Declarative refusal (`--no-input`): never reach for the key.
                if crate::stdin_helper::no_input() {
                    return Err(AppError::Validation(
                        crate::i18n::validation::no_input_blocks_stdin(),
                    ));
                }
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
            crate::output::emit_json_compact(&output)?;
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
            crate::output::emit_json(&output)?;
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
                return Err(AppError::NotFound(
                    crate::i18n::validation::api_key_fingerprint_not_found(&fingerprint),
                ));
            }
            config::save_config(&cfg)?;
            let output = json!({
                "action": "key_removed",
                "fingerprint": fingerprint,
            });
            crate::output::emit_json_compact(&output)?;
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
            crate::output::emit_json(&output)?;
            Ok(())
        }
        ConfigAction::Path { json: _, db: _ } => {
            let path = config::config_file_path()?;
            let output = json!({
                "config_path": path.display().to_string(),
                "exists": path.exists(),
            });
            crate::output::emit_json_compact(&output)?;
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
            crate::output::emit_json_compact(&output)?;
            Ok(())
        }
        ConfigAction::Get {
            key,
            json: _,
            db: _,
        } => {
            let value = config::get_setting(&key)?;
            // GAP-SG-202: `found: false` alone conflated two states an operator
            // must tell apart — a real key that is simply unset, and a key that
            // does not exist at all. `config set` has always answered exit 1
            // with a did-you-mean for the second, so the two verbs disagreed
            // about the same string, and a typo read as "exists, empty".
            //
            // The exit code stays 0 on purpose: scripts probe presence with
            // this command, and turning a probe into a failure would break them
            // to say something the envelope can carry instead.
            let known = config::is_known_setting(&key);
            let output = json!({
                "key": key,
                "value": value,
                "found": value.is_some(),
                "known": known,
                "suggestion": if known { None } else { config::nearest_setting_key(&key) },
            });
            crate::output::emit_json_compact(&output)?;
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
                // GAP-SG-93 established that defaults must come from constants
                // rather than literals. GAP-SG-209 finishes the job: this used
                // to be a hand-maintained list of SEVEN pairs while
                // `SETTING_KEYS` declares a default for roughly fifty keys, so
                // `--effective` promised "well-known defaults" and delivered a
                // seventh of them. Worse, it was a SECOND copy — `display.tz`
                // was spelled `"UTC"` here and `Some("UTC")` in the registry,
                // free to drift apart in silence.
                //
                // The registry is the single source of truth for which keys
                // exist AND what they fall back to, so read it. A key whose
                // default is `None` is one the host derives at runtime (an XDG
                // directory, the CPU count, a probe); inventing a literal for
                // those would print a number the process never uses, which is
                // the drift this loop exists to prevent.
                for entry in config::SETTING_KEYS {
                    let Some(default) = entry.default else {
                        continue;
                    };
                    settings
                        .entry(entry.key.to_string())
                        .or_insert_with(|| default.to_string());
                }
            }
            let output = json!({
                "settings": settings,
                "effective": effective,
            });
            crate::output::emit_json(&output)?;
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
            crate::output::emit_json_compact(&output)?;
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
