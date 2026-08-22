//! Entity type vocabulary policy for `entity-type-validate` (GAP-SG-283).
//!
//! `remember` has `--strict-entity-types` and `link` has `--strict-relations`,
//! so the two hand-written write channels let a project declare the vocabulary
//! it accepts. `enrich` had neither, and it is the channel that creates entity
//! labels in VOLUME: `entity-type-validate` took whatever string the model
//! returned and wrote it straight into `UPDATE entities SET type`, with V017
//! having removed the SQL `CHECK` that used to be the only thing in the way.
//!
//! This module is the missing brake. It resolves the accepted vocabulary and
//! the policy for a label outside it, applies that policy to ONE label, and
//! reports what it did on the NDJSON stream so an operator can measure the run
//! without opening the database.
//!
//! The default is `keep`, which is byte-for-byte the v1.2.8 behaviour: a caller
//! who passes no flag gets exactly the run they got before, warning included.
//!
//! It lives under `events` because what it publishes is contract — the policy
//! actually applied, and how many signals fed the decision — and because the
//! `call_*` helpers in `extraction_graph.rs` are reached through a dispatcher
//! that threads no `EnrichArgs`. Widening those signatures would edit
//! `drain_serial.rs` and `drain_parallel.rs`, so the resolved policy is
//! installed once per process from the same place the drain fan-out is sized.

use super::super::args::EnrichArgs;
use crate::entity_type::{normalize_entity_type, CANONICAL_ENTITY_TYPES, DEFAULT_ENTITY_TYPE};
use serde::Serialize;
use std::sync::OnceLock;

/// XDG key carrying the accepted entity type vocabulary, comma separated.
const ALLOWED_TYPES_KEY: &str = "enrich.entity_type.allowed_types";

/// XDG key carrying the policy for a label outside that vocabulary.
const ON_UNKNOWN_TYPE_KEY: &str = "enrich.entity_type.on_unknown_type";

/// Minimum Jaro-Winkler similarity for `fallback` to accept a nearest match.
///
/// Below it the suggestion resembles nothing in the vocabulary, and picking the
/// least-bad of a set of unrelated words would be a guess dressed as a mapping.
/// The run lands on [`DEFAULT_ENTITY_TYPE`] instead, which is what an absent
/// label has always resolved to.
const FALLBACK_MIN_SIMILARITY: f64 = 0.6;

/// Marker that carries the raw label into the entity description under
/// `fallback`, so the rewrite has a declared inverse.
const RAW_LABEL_MARKER: &str = "[entity-type-fallback]";

/// Evidence block headers emitted by the description path's evidence loader.
///
/// Counted OUT of the signal total: they label the evidence, they are not
/// evidence.
const EVIDENCE_HEADERS: [&str; 2] = ["Linked memory bodies:", "Typed relations in the graph:"];

/// What to do with a validated label outside the accepted vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum UnknownTypePolicy {
    /// Store the label as written — the v1.2.8 behaviour, and the default.
    Keep,
    /// Store the nearest accepted label, preserving the raw one in the
    /// entity description.
    Fallback,
    /// Refuse the item with a validation error (exit 1), mirroring
    /// `remember --strict-entity-types`.
    Strict,
}

/// Accepted spellings of [`UnknownTypePolicy`] on the command line.
///
/// Shared with the clap `value_parser` in `args.rs` so the flag and this parser
/// can never accept different sets.
pub(crate) const UNKNOWN_TYPE_POLICIES: [&str; 3] = ["keep", "fallback", "strict"];

impl UnknownTypePolicy {
    /// Parses a policy name, returning `None` for anything unrecognised.
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "keep" => Some(Self::Keep),
            "fallback" => Some(Self::Fallback),
            "strict" => Some(Self::Strict),
            _ => None,
        }
    }

    /// Wire name, identical to the flag value that selects it.
    fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Fallback => "fallback",
            Self::Strict => "strict",
        }
    }
}

/// Which precedence layer supplied a resolved value.
///
/// Published so a caller can tell a policy the operator CHOSE from one the
/// product supplied by omission, without reading the host configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PolicySource {
    /// A CLI flag on this invocation.
    Flag,
    /// An XDG key stored for this host.
    Xdg,
    /// The compiled default.
    Default,
}

/// The vocabulary and policy in force for this process.
#[derive(Debug, Clone)]
pub(crate) struct EntityTypePolicy {
    /// Accepted labels, already shape-normalised and sorted.
    allowed: Vec<String>,
    /// Where [`Self::allowed`] came from.
    allowed_source: PolicySource,
    /// What to do with a label outside [`Self::allowed`].
    policy: UnknownTypePolicy,
    /// Where [`Self::policy`] came from.
    policy_source: PolicySource,
}

/// Verdict for ONE validated label.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PolicyOutcome {
    /// Write the label as it stands.
    Accept(String),
    /// Write `applied` instead of `raw`, preserving `raw` in the description.
    Fallback {
        /// Accepted label to store.
        applied: String,
        /// Label the model actually returned.
        raw: String,
    },
    /// Refuse the item with this message.
    Refuse(String),
}

impl EntityTypePolicy {
    /// The compiled default: the canonical vocabulary, kept as written.
    fn compiled_default() -> Self {
        Self {
            allowed: CANONICAL_ENTITY_TYPES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            allowed_source: PolicySource::Default,
            policy: UnknownTypePolicy::Keep,
            policy_source: PolicySource::Default,
        }
    }

    /// Resolves flag > XDG > compiled default for both halves.
    fn resolve(flag_allowed: &[String], flag_policy: Option<&str>) -> Self {
        let mut resolved = Self::compiled_default();

        if let Some(list) = parse_vocabulary(&flag_allowed.join(",")) {
            resolved.allowed = list;
            resolved.allowed_source = PolicySource::Flag;
        } else {
            let stored = crate::runtime_config::resolve_string(None, ALLOWED_TYPES_KEY, "");
            if let Some(list) = parse_vocabulary(&stored) {
                resolved.allowed = list;
                resolved.allowed_source = PolicySource::Xdg;
            }
        }

        if let Some(policy) = flag_policy.and_then(UnknownTypePolicy::parse) {
            resolved.policy = policy;
            resolved.policy_source = PolicySource::Flag;
        } else {
            let stored = crate::runtime_config::resolve_string(None, ON_UNKNOWN_TYPE_KEY, "");
            if let Some(policy) = UnknownTypePolicy::parse(&stored) {
                resolved.policy = policy;
                resolved.policy_source = PolicySource::Xdg;
            }
        }

        resolved
    }

    /// Reports whether `label` is inside the accepted vocabulary.
    fn accepts(&self, label: &str) -> bool {
        self.allowed.iter().any(|a| a == label)
    }

    /// Nearest accepted label to `label`, or [`DEFAULT_ENTITY_TYPE`] when
    /// nothing is near enough to call a match.
    ///
    /// Reuses the `rapidfuzz` Jaro-Winkler scorer the setting registry already
    /// uses for did-you-mean, rather than adding a second similarity notion to
    /// the crate. Ties resolve to the FIRST candidate in the vocabulary, which
    /// is sorted, so the mapping is stable across runs.
    fn nearest(&self, label: &str) -> String {
        let mut best: Option<(&str, f64)> = None;
        for candidate in &self.allowed {
            let score = rapidfuzz::distance::jaro_winkler::normalized_similarity(
                label.chars(),
                candidate.chars(),
            );
            let better = match best {
                Some((_, current)) => score > current,
                None => true,
            };
            if better {
                best = Some((candidate.as_str(), score));
            }
        }
        match best {
            Some((candidate, score)) if score >= FALLBACK_MIN_SIMILARITY => candidate.to_string(),
            _ if self.accepts(DEFAULT_ENTITY_TYPE) => DEFAULT_ENTITY_TYPE.to_string(),
            _ => self
                .allowed
                .first()
                .cloned()
                .unwrap_or_else(|| DEFAULT_ENTITY_TYPE.to_string()),
        }
    }

    /// Applies the policy to ONE already shape-normalised label.
    fn apply(&self, label: &str) -> PolicyOutcome {
        if self.accepts(label) {
            return PolicyOutcome::Accept(label.to_string());
        }
        match self.policy {
            UnknownTypePolicy::Keep => PolicyOutcome::Accept(label.to_string()),
            UnknownTypePolicy::Fallback => PolicyOutcome::Fallback {
                applied: self.nearest(label),
                raw: label.to_string(),
            },
            UnknownTypePolicy::Strict => PolicyOutcome::Refuse(format!(
                "--on-unknown-type {} is in force and the validated entity type \
                 '{label}' is outside the accepted vocabulary: {}. Add the label with \
                 --allowed-types, or choose keep/fallback to let the run continue",
                UnknownTypePolicy::Strict.as_str(),
                self.allowed.join(", ")
            )),
        }
    }
}

/// Splits a comma-separated vocabulary, dropping labels of unusable shape.
///
/// Returns `None` when the input declares nothing, so the caller can fall
/// through to the next precedence layer instead of installing an empty
/// vocabulary that would refuse every label.
fn parse_vocabulary(raw: &str) -> Option<Vec<String>> {
    let mut out: Vec<String> = raw
        .split(',')
        .filter_map(|part| normalize_entity_type(part).ok())
        .collect();
    if out.is_empty() {
        return None;
    }
    out.sort_unstable();
    out.dedup();
    Some(out)
}

/// Process-wide slot holding the policy resolved for this invocation.
static INSTALLED: OnceLock<EntityTypePolicy> = OnceLock::new();

/// Installs the policy resolved from `args`, once per process.
///
/// Called from [`super::resolve_drain_parallelism`], which runs exactly once
/// before the drain and is the last point on the path that still holds an
/// `EnrichArgs`.
pub(crate) fn install_from_args(args: &EnrichArgs) {
    let _ = INSTALLED.set(EntityTypePolicy::resolve(
        &args.allowed_types,
        args.on_unknown_type.as_deref(),
    ));
}

/// The policy in force, falling back to the compiled default when nothing was
/// installed — a read path must never be the thing that refuses a write.
fn effective() -> &'static EntityTypePolicy {
    INSTALLED.get_or_init(EntityTypePolicy::compiled_default)
}

/// Applies the policy in force to one shape-normalised label.
pub(crate) fn apply_entity_type_policy(label: &str) -> PolicyOutcome {
    effective().apply(label)
}

/// Sentence appended to an entity description so a `fallback` rewrite can be
/// undone without a backup.
pub(crate) fn raw_label_note(raw: &str, applied: &str) -> String {
    format!("{RAW_LABEL_MARKER} raw entity type '{raw}' rewritten to '{applied}'")
}

/// How many distinct signals fed a type decision (GAP-SG-279 leftover).
///
/// A description counts once, and every non-header line of gathered evidence
/// counts once. The number separates a verdict drawn from twenty linked bodies
/// from one drawn from a single edge, which `evidence_chars` alone cannot: one
/// long body and twenty short ones can carry the same character count.
pub(crate) fn count_type_signals(description: &str, evidence: &str) -> usize {
    let described = usize::from(!description.trim().is_empty());
    let lines = evidence
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !EVIDENCE_HEADERS.contains(line))
        .count();
    described + lines
}

/// One NDJSON record per label this policy ruled on.
#[derive(Debug, Serialize)]
pub(crate) struct EntityTypePolicyEvent<'a> {
    /// Event family discriminator, matching the other enrich phase records.
    pub(crate) phase: &'static str,
    /// Entity whose label was ruled on.
    pub(crate) item: &'a str,
    /// Policy actually applied on this invocation.
    pub(crate) policy: UnknownTypePolicy,
    /// Precedence layer that supplied the policy.
    pub(crate) policy_source: PolicySource,
    /// Precedence layer that supplied the accepted vocabulary.
    pub(crate) allowed_types_source: PolicySource,
    /// Size of the accepted vocabulary in force.
    pub(crate) allowed_types: usize,
    /// Label the model returned, after shape normalisation.
    pub(crate) raw_type: &'a str,
    /// Label the run will store, when it stores one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) applied_type: Option<&'a str>,
    /// Whether the label sits inside the accepted vocabulary.
    pub(crate) accepted: bool,
    /// Distinct signals that fed the decision.
    pub(crate) signals: usize,
    /// Characters of evidence the decision was made from.
    pub(crate) evidence_chars: usize,
}

/// Emits one policy record for `item` on the NDJSON stream.
pub(crate) fn emit_policy_event(
    item: &str,
    raw_type: &str,
    applied_type: Option<&str>,
    signals: usize,
    evidence_chars: usize,
) {
    let policy = effective();
    let accepted = policy.accepts(raw_type);
    crate::output::emit_json_line(&EntityTypePolicyEvent {
        phase: "entity-type-policy",
        item,
        policy: policy.policy,
        policy_source: policy.policy_source,
        allowed_types_source: policy.allowed_source,
        allowed_types: policy.allowed.len(),
        raw_type,
        applied_type,
        accepted,
        signals,
        evidence_chars,
    });
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    fn with(allowed: &[&str], policy: UnknownTypePolicy) -> EntityTypePolicy {
        EntityTypePolicy {
            allowed: allowed.iter().map(|s| (*s).to_string()).collect(),
            allowed_source: PolicySource::Flag,
            policy,
            policy_source: PolicySource::Flag,
        }
    }

    /// The compatibility guarantee this whole change is bounded by: with no
    /// flag and no key, an unknown label is stored exactly as v1.2.8 stored it.
    #[test]
    fn the_compiled_default_keeps_an_unknown_label() {
        let policy = EntityTypePolicy::compiled_default();
        assert_eq!(policy.policy, UnknownTypePolicy::Keep);
        assert_eq!(policy.policy_source, PolicySource::Default);
        assert_eq!(
            policy.apply("crate"),
            PolicyOutcome::Accept("crate".to_string())
        );
    }

    #[test]
    fn a_canonical_label_is_accepted_under_every_policy() {
        for chosen in [
            UnknownTypePolicy::Keep,
            UnknownTypePolicy::Fallback,
            UnknownTypePolicy::Strict,
        ] {
            let policy = with(CANONICAL_ENTITY_TYPES, chosen);
            assert_eq!(
                policy.apply("person"),
                PolicyOutcome::Accept("person".to_string())
            );
        }
    }

    #[test]
    fn strict_refuses_a_label_outside_the_vocabulary() {
        let policy = with(&["person", "project"], UnknownTypePolicy::Strict);
        let PolicyOutcome::Refuse(message) = policy.apply("crate") else {
            panic!("strict must refuse an unknown label");
        };
        assert!(message.contains("crate"), "message must name the label");
        assert!(
            message.contains("person, project"),
            "message must name the accepted vocabulary"
        );
    }

    #[test]
    fn fallback_maps_to_the_nearest_accepted_label_and_keeps_the_raw_one() {
        let policy = with(&["organization", "person"], UnknownTypePolicy::Fallback);
        assert_eq!(
            policy.apply("persona"),
            PolicyOutcome::Fallback {
                applied: "person".to_string(),
                raw: "persona".to_string(),
            }
        );
    }

    /// A label resembling nothing in the vocabulary must not be mapped onto the
    /// least-bad candidate: that is a guess, and a guess in a type column is
    /// what GAP-SG-283 exists to stop.
    #[test]
    fn fallback_lands_on_the_default_when_nothing_is_near() {
        let policy = with(CANONICAL_ENTITY_TYPES, UnknownTypePolicy::Fallback);
        assert_eq!(
            policy.apply("xyzzy"),
            PolicyOutcome::Fallback {
                applied: DEFAULT_ENTITY_TYPE.to_string(),
                raw: "xyzzy".to_string(),
            }
        );
    }

    #[test]
    fn a_vocabulary_is_normalised_sorted_and_deduplicated() {
        let parsed = parse_vocabulary(" Person , issue-tracker ,person, 42 ,, ")
            .expect("a non-empty vocabulary must parse");
        assert_eq!(parsed, vec!["issue_tracker", "person"]);
    }

    /// An empty declaration must fall THROUGH to the next precedence layer, or
    /// a stray `--allowed-types ""` would refuse every label in the graph.
    #[test]
    fn an_empty_declaration_does_not_install_an_empty_vocabulary() {
        assert!(parse_vocabulary("").is_none());
        assert!(parse_vocabulary(" , , ").is_none());
    }

    #[test]
    fn the_flag_wins_over_every_other_layer() {
        let resolved =
            EntityTypePolicy::resolve(&["crate".to_string(), "gap".to_string()], Some("strict"));
        assert_eq!(resolved.allowed, vec!["crate", "gap"]);
        assert_eq!(resolved.allowed_source, PolicySource::Flag);
        assert_eq!(resolved.policy, UnknownTypePolicy::Strict);
        assert_eq!(resolved.policy_source, PolicySource::Flag);
    }

    #[test]
    fn signals_count_the_description_and_every_evidence_line() {
        let evidence =
            "Linked memory bodies:\n- one\n- two\n\nTyped relations in the graph:\n- three";
        assert_eq!(count_type_signals("a description", evidence), 4);
        assert_eq!(count_type_signals("", evidence), 3);
        assert_eq!(count_type_signals("   ", "  \n "), 0);
    }

    #[test]
    fn the_raw_label_note_names_both_labels() {
        let note = raw_label_note("persona", "person");
        assert!(note.contains("persona"));
        assert!(note.contains("person"));
        assert!(note.contains(RAW_LABEL_MARKER));
    }

    #[test]
    fn every_advertised_policy_name_parses() {
        for name in UNKNOWN_TYPE_POLICIES {
            let parsed = UnknownTypePolicy::parse(name).expect("advertised name must parse");
            assert_eq!(parsed.as_str(), name);
        }
        assert!(UnknownTypePolicy::parse("maybe").is_none());
    }
}
