//! Retrieval tuning and the agent-native surface vocabulary.
//!
//! Split out of the former single-file `constants.rs` in v1.2.5;
//! every item is re-exported by the parent module, so `crate::constants::X`
//! resolves exactly as before.

/// Jaccard threshold above which two memories are considered fuzzy duplicates.
pub const DEDUP_FUZZY_THRESHOLD: f64 = 0.8;

/// Cosine distance threshold below which two memories are semantic duplicates.
pub const DEDUP_SEMANTIC_THRESHOLD: f32 = 0.1;

/// Maximum number of hops allowed in graph traversals.
pub const MAX_GRAPH_HOPS: u32 = 2;

/// Minimum relationship weight required for traversal inclusion.
pub const MIN_RELATION_WEIGHT: f64 = 0.3;

/// Default traversal depth for `related` when `--hops` is omitted.
pub const DEFAULT_MAX_HOPS: u32 = 2;

/// Default minimum weight filter applied during graph traversal.
pub const DEFAULT_MIN_WEIGHT: f64 = 0.3;

/// Default weight assigned to newly created relationships.
pub const DEFAULT_RELATION_WEIGHT: f64 = 0.5;

/// Default `k` used by `recall` when the caller omits `--k`.
pub const DEFAULT_K_RECALL: usize = 10;

/// Default `k` for memory KNN searches when the caller omits `--k`.
pub const K_MEMORIES_DEFAULT: usize = 10;

/// Default `k` for entity KNN searches during graph expansion.
pub const K_ENTITIES_SEARCH: usize = 5;

/// Default `k` constant used by Reciprocal Rank Fusion in `hybrid-search`.
pub const RRF_K_DEFAULT: u32 = 60;

/// Maximum result count from the recursive graph CTE in `recall`.
pub const K_GRAPH_MATCHES_LIMIT: usize = 20;

/// Default `--limit` for `list` when omitted.
pub const K_LIST_DEFAULT_LIMIT: usize = 100;

/// Default `--limit` for `graph entities` when omitted.
pub const K_GRAPH_ENTITIES_DEFAULT_LIMIT: usize = 50;

/// Default `--limit` for `related` when omitted.
pub const K_RELATED_DEFAULT_LIMIT: usize = 10;

/// Default `--limit` for `history` when omitted.
pub const K_HISTORY_DEFAULT_LIMIT: usize = 20;

/// Maximum edges pulled when `deep-research` expands the graph around its hits.
pub const K_DEEP_RESEARCH_GRAPH_EDGES_LIMIT: usize = 50;

/// Default weight for the vector contribution in the `hybrid-search` RRF formula.
pub const WEIGHT_VEC_DEFAULT: f64 = 1.0;

/// Default weight for the BM25 text contribution in the `hybrid-search` RRF formula.
pub const WEIGHT_FTS_DEFAULT: f64 = 1.0;

/// GAP-SG-142: envelope members searched, in order, for the primary result
/// array reshaped by [`crate::agent_surface`].
///
/// The list is ordered from most to least specific so an envelope that carries
/// several arrays (for example `recall`, which exposes `direct_matches`,
/// `graph_matches` and the merged `results`) is reshaped on the member callers
/// actually consume. A payload matching none of these falls back to its first
/// array member.
///
/// `nodes` precedes `entities` because the `graph` envelope carries both and
/// `nodes` is the canonical one there; `entities` is its v1.0.66 alias and is
/// listed in [`AGENT_SURFACE_ALIAS_ARRAYS`]. Reshaping the alias while leaving
/// the canonical member untouched is precisely the failure that table closes.
pub const AGENT_SURFACE_RESULT_KEYS: &[&str] = &[
    "results", "items", "nodes", "entities", "memories", "hits", "rows", "matches", "data",
];

/// GAP-SG-142: derived result arrays suppressed once the agent-native surface
/// reshapes their canonical source.
///
/// Each entry is `(subcommand, canonical member, members that merely restate
/// it)`: `list` clones `items` into `memories`, `graph export` clones `nodes`
/// into `entities`, `recall` publishes `results` as the concatenation of
/// `direct_matches` and `graph_matches`, and `related` clones `results` into
/// `related_memories`.
///
/// The subcommand is part of the key because "derived" is a property of one
/// command's envelope, not of a member name. `results` means a concatenation in
/// `recall` and a clone in `related`, and in `hybrid-search` it means neither:
/// there `graph_expansion` skips every id already present in `results`, so
/// `results` and `graph_matches` are DISJOINT and carry different types
/// (`HybridSearchItem` against `RecallItem`). Matching on the member name alone
/// deleted a set no other member restated — and one that
/// `docs/schemas/hybrid-search.schema.json` lists under `required`, so the
/// suppression produced an envelope invalid against this project's own schema.
/// `hybrid-search` is absent from this table by construction, which is what
/// keeps that from happening again.
///
/// Suppression only removes members that are actually present, so a declared
/// member the envelope never carried is a silent no-op and is never reported as
/// removed.
///
/// The surface reshapes exactly one array per envelope, so leaving a genuinely
/// derived member in place shipped the unfiltered, unsorted, unprojected copy
/// right next to the shaped one — the redundancy the projection exists to
/// remove, and a meta record (`sort`, `output_count`) that contradicted half the
/// payload. Those are therefore dropped whenever a knob is set.
///
/// Without any knob the surface is a no-op and nothing is removed, so the public
/// v1.0.66 alias contract stays intact byte for byte for every existing caller.
pub const AGENT_SURFACE_ALIAS_ARRAYS: &[(&str, &str, &[&str])] = &[
    ("list", "items", &["memories"]),
    ("graph", "nodes", &["entities"]),
    ("recall", "results", &["direct_matches", "graph_matches"]),
    ("related", "results", &["related_memories"]),
];

/// DEFAULT cap on `hybrid-search --with-graph` graph matches.
///
/// ACTIVE by default, unlike the `recall` flag of the same name, which defaults
/// to unbounded. `hybrid-search` had no cap at all: `graph_expansion` walks
/// outward from the fused results AND from the five entities nearest the query
/// embedding, then materialises every memory it reaches with a 300-character
/// snippet each. A `--k 3` query over a dense neighbourhood measured a 1 112 925
/// byte envelope — the caller asked for three results and got a megabyte.
///
/// A finite default is the only honest shape here: the flag caps a set the
/// caller never sized, so leaving it unbounded means the envelope is bounded by
/// the graph rather than by the request. 50 keeps a genuinely useful
/// neighbourhood while holding the envelope in the tens of kilobytes.
///
/// Read it through [`hybrid_search_max_graph_results`], never directly.
pub const DEFAULT_HYBRID_MAX_GRAPH_RESULTS: usize = 50;

/// Graph-match ceiling for `hybrid-search`: the `--max-graph-results` flag, then
/// XDG `search.hybrid.max_graph_results`, then
/// [`DEFAULT_HYBRID_MAX_GRAPH_RESULTS`].
///
/// `0` disables the cap at either layer, which is how a caller opts back into
/// the unbounded pre-v1.2.2 envelope. Returns `None` for that case so the
/// traversal loop can skip the check entirely.
pub fn hybrid_search_max_graph_results(flag: Option<usize>) -> Option<usize> {
    let resolved = flag
        .or_else(|| {
            crate::config::get_setting("search.hybrid.max_graph_results")
                .ok()
                .flatten()
                .and_then(|v| v.parse::<usize>().ok())
        })
        .unwrap_or(DEFAULT_HYBRID_MAX_GRAPH_RESULTS);
    (resolved > 0).then_some(resolved)
}

/// Default cap on emitted result elements (`--max-items`). `0` means no cap,
/// preserving the pre-GAP-SG-142 envelope byte for byte.
pub const DEFAULT_AGENT_SURFACE_MAX_ITEMS: usize = 0;

/// Default cap on string length in characters (`--truncate-content`).
/// `0` disables content truncation.
pub const DEFAULT_AGENT_SURFACE_TRUNCATE_CONTENT: usize = 0;

/// Default cap on the serialized envelope in bytes (`--max-output-bytes`).
/// `0` disables the ceiling.
pub const DEFAULT_AGENT_SURFACE_MAX_OUTPUT_BYTES: usize = 0;
