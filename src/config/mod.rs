//! XDG-based API key management for OpenRouter and other providers.
//!
//! Stores keys in `$XDG_CONFIG_HOME/sqlite-graphrag/config.toml` with
//! atomic write, symlink-attack defense and Unix permission hardening.

use secrecy::SecretBox;
use serde::{Deserialize, Serialize};

mod api_keys;
mod permissions;
mod registry;
mod settings;
mod store;

// GAP-SG-146: `config.rs` was the largest non-test file in the crate. It was
// split by responsibility, and every public item is re-exported here so no
// caller outside this module had to change. `SETTING_KEYS` in particular is
// `pub` and consumed from outside the crate.
pub use api_keys::{compute_fingerprint, mask_key, resolve_api_key};
pub use registry::{is_known_setting, nearest_setting_key, setting_key_names, SETTING_KEYS};
pub use settings::{get_setting, list_settings, set_setting, unset_setting};
pub use store::{config_file_path, load_config, save_config};

/// App config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Configuration schema version.
    pub schema_version: u32,
    /// Keys.
    #[serde(default)]
    pub keys: Vec<ApiKeyEntry>,
    /// Operational settings persisted via `config set/get` (G-T-XDG-01).
    /// Stringly-typed map keeps the schema open without migrations for every
    /// new key. Known keys are documented in `config set --help`.
    #[serde(default)]
    pub settings: std::collections::BTreeMap<String, String>,
}

/// API key entry.
#[derive(Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    /// Provider name.
    pub provider: String,
    /// Value.
    pub value: String,
    /// Added at.
    pub added_at: String,
    /// Fingerprint.
    pub fingerprint: String,
}

impl std::fmt::Debug for ApiKeyEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKeyEntry")
            .field("provider", &self.provider)
            .field("value", &mask_key(&self.value))
            .field("added_at", &self.added_at)
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            keys: vec![],
            settings: std::collections::BTreeMap::new(),
        }
    }
}

/// One entry of the canonical setting registry.
///
/// Carrying the default alongside the key is what lets `config doctor` derive
/// its whole listing from [`SETTING_KEYS`] instead of repeating a hand-written
/// table. `GAP-SG-85` was exactly that divergence: a 14-entry manual list next
/// to a 44-key registry, missing the one key that redirects the database.
pub struct SettingKey {
    /// Dotted key accepted by `config set`.
    pub key: &'static str,
    /// Literal default applied when neither a CLI flag nor the XDG config
    /// supplies a value.
    ///
    /// `None` marks a default that cannot be a static string because it is
    /// derived from the host at runtime — an XDG directory, the CPU count, or
    /// a probe. `config doctor` reports those as `derived` rather than
    /// inventing a number that would not match what the process actually uses.
    pub default: Option<&'static str>,
    /// Domain the stored value must belong to, enforced by [`set_setting`].
    ///
    /// `GAP-SG-201`: the registry validated the KEY and never the VALUE, so
    /// `config set embedding.dim nao-numero` reported success and the defect
    /// surfaced on the next invocation, far from its cause. `display.tz 0` was
    /// the worst case — it bricked the whole binary (`GAP-SG-200`).
    pub kind: ValueKind,
}

/// Domain a setting value must belong to.
///
/// Deliberately coarse. The point is to reject what can never work, not to
/// re-derive every consumer's parsing: a `u64` reader still clamps its own
/// range, and this only guarantees it receives digits at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// A boolean in any spelling the readers accept.
    ///
    /// Deliberately wider than `true|false`. The readers are not uniform —
    /// `tracing_init.rs` takes `1|true|yes|on`, `lib.rs` the same four,
    /// `retry.rs` only three — and the `--low-memory` help has always
    /// advertised `config set ingest.low_memory 1`. Validating against the
    /// narrowest spelling would reject a value the product documents and every
    /// reader honours, which is a regression dressed as a check.
    ///
    /// The job here is to reject what NO reader accepts (`talvez`, `maybe`),
    /// not to impose a house style on values that already work.
    Bool,
    /// A non-negative integer.
    Unsigned,
    /// A finite decimal number.
    Float,
    /// An IANA timezone name, e.g. `America/Sao_Paulo`.
    Tz,
    /// An absolute `http`/`https` URL.
    Url,
    /// A filesystem path. Only emptiness is rejected: existence is the
    /// caller's business, and a path that does not exist yet is legitimate.
    Path,
    /// Free text with no checkable domain.
    Text,
    /// A `tracing` filter directive, e.g. `warn` or `sqlite_graphrag=debug`.
    ///
    /// Not a closed set: the grammar accepts per-target directives, so listing
    /// the five level names would reject legitimate values. Validated by
    /// parsing with the same type `tracing_init` builds, because
    /// `EnvFilter::new` DISCARDS an unparseable directive instead of failing —
    /// which is how `log.level NIVEL_X` used to silence logging outright while
    /// `config set` reported success.
    LogDirective,
    /// One of a closed set of spellings.
    OneOf(&'static [&'static str]),
}

impl ValueKind {
    /// Returns a LOCALIZED description of the accepted domain, for the error
    /// message.
    ///
    /// Returns `None` when every string is acceptable, which is the signal to
    /// skip validation entirely rather than to accept with an empty reason.
    ///
    /// Localized rather than hard-coded English because the message that
    /// carries it is translated, and half-translating a sentence is worse than
    /// not translating it: the operator reads Portuguese prose that ends in an
    /// English clause.
    ///
    /// A closed set is NOT translated — those are literal spellings the
    /// operator must type, so `true|false` is the same string in every locale.
    pub fn expectation(&self) -> Option<String> {
        match self {
            ValueKind::Text => None,
            ValueKind::OneOf(options) => Some(options.join("|")),
            ValueKind::Bool => Some("true|false (also 1|0, yes|no, on|off)".to_string()),
            other => Some(crate::i18n::validation::config_value_expectation(*other)),
        }
    }

    /// `true` when `value` belongs to this domain.
    pub fn accepts(&self, value: &str) -> bool {
        let trimmed = value.trim();
        match self {
            ValueKind::Text => true,
            ValueKind::Bool => matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "true" | "false" | "1" | "0" | "yes" | "no" | "on" | "off"
            ),
            ValueKind::Unsigned => !trimmed.is_empty() && trimmed.parse::<u64>().is_ok(),
            ValueKind::Float => trimmed
                .parse::<f64>()
                .is_ok_and(|parsed| parsed.is_finite()),
            ValueKind::Tz => trimmed.parse::<chrono_tz::Tz>().is_ok(),
            // Scheme-only check on purpose. A full parse would drag a URL crate
            // into a path that just needs to reject `not-a-url` before the HTTP
            // client fails on it much later, with a much worse message.
            ValueKind::Url => {
                (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
                    && trimmed.len() > "https://".len()
            }
            ValueKind::Path => !trimmed.is_empty(),
            ValueKind::LogDirective => {
                !trimmed.is_empty() && tracing_subscriber::EnvFilter::try_new(trimmed).is_ok()
            }
            ValueKind::OneOf(options) => options.contains(&trimmed),
        }
    }
}

/// Resolved key.
pub struct ResolvedKey {
    /// Value.
    pub value: SecretBox<String>,
    /// Source side of the relationship.
    pub source: &'static str,
}
