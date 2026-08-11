//! GAP-SG-201: what the QUERY already discarded, declared so the output surface
//! can stop describing a set it never saw.
//!
//! `--filter type=skill list` answers 39 over 1892 memories. The same request
//! with `--limit 50` answered `0`, with `exit 0`, because the predicate was
//! handed the fifty rows SQL returned rather than the corpus the caller asked
//! about. Both numbers are produced by the same code; only one of them is an
//! answer to the question.
//!
//! The surface cannot see this on its own. It receives a serialized envelope,
//! downstream of `LIMIT`, and an array of fifty is indistinguishable from a
//! corpus of fifty. So the command that applied the ceiling declares it here,
//! and the surface reads the declaration.
//!
//! # Why a process-wide cell rather than a field on every response
//!
//! This binary is one-shot: one process runs one subcommand and emits one
//! envelope. A cell is therefore not ambient state that could belong to someone
//! else — it is the single fact about the single query this process ran. The
//! same reasoning already governs [`super::AgentSurface`]. Threading a new field
//! through six response structs would also change six published schemas to carry
//! a fact none of them is about.
//!
//! # Pagination is not top-k
//!
//! [`CeilingKind`] is the distinction the refusal turns on, and it is not
//! cosmetic. `list --limit 50` pages a countable universe: 50 of 1892 is a
//! recorte, and a predicate over it answers the wrong question. `hybrid-search
//! -k 5` does not page anything — the five best matches ARE the result set the
//! caller asked for, and filtering them is a legitimate operation on a complete
//! answer. Refusing there would break every semantic search that carries a
//! filter while curing nothing.

use std::sync::OnceLock;

/// What the caller declares `--filter` may observe.
///
/// Absent, the surface refuses a predicate over a truncated page rather than
/// answering about a set it never saw. Present, the caller has said which
/// reading it meant, and the surface obeys.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterScope {
    /// Judge only the rows the query returned, and say so in the record.
    Page,
    /// Require the predicate to observe the whole universe.
    ///
    /// Identical to the default today; declaring it makes the requirement
    /// explicit in a script that must not silently start filtering a page if a
    /// limit is added later.
    Universe,
}

/// What kind of ceiling the query applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeilingKind {
    /// A page of an enumerable universe whose size the command can count.
    ///
    /// `list` and `graph entities`: both run a `COUNT` beside the query, so
    /// "did the ceiling actually cut anything" has a factual answer.
    Pagination,
    /// A bound on a ranked or traversed result set, with no universe to compare.
    ///
    /// `hybrid-search -k`, `recall -k`, `deep-research --max-results` and
    /// `related --limit`, which stops a breadth-first walk rather than paging a
    /// table. The ceiling defines the answer instead of truncating it.
    TopK,
}

impl CeilingKind {
    /// Wire spelling for the `agent_surface` record.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pagination => "pagination",
            Self::TopK => "top-k",
        }
    }
}

/// Where the ceiling's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeilingSource {
    /// The caller passed it.
    Flag,
    /// A named constant supplied it and the caller never asked for a cut.
    ///
    /// This is the case that made GAP-SG-201 fire without anyone doing anything
    /// wrong: `graph entities` caps at 50 by default, so `--filter` judged 50 of
    /// 15 615 entities on a command line that mentioned no limit at all.
    Default,
}

impl CeilingSource {
    /// Wire spelling for the `agent_surface` record.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Default => "default",
        }
    }
}

/// The ceiling one query applied, as the command that applied it saw it.
#[derive(Debug, Clone, Copy)]
pub struct QueryCeiling {
    /// Rows the query was allowed to return.
    pub applied: usize,
    /// Rows the query skipped before returning any.
    ///
    /// A non-zero offset means the page is a recorte even when `applied` alone
    /// would have covered the universe.
    pub offset: usize,
    /// Whether the caller chose the value or a constant did.
    pub source: CeilingSource,
    /// Whether the ceiling pages a universe or bounds a ranking.
    pub kind: CeilingKind,
    /// Size of the universe, when the command can count it.
    ///
    /// `None` is not "unbounded": it means the command has no universe to
    /// compare against, which is exactly why [`CeilingKind::TopK`] is never
    /// refused.
    pub universe_total: Option<usize>,
}

impl QueryCeiling {
    /// `true` when the ceiling actually kept rows out of the envelope.
    ///
    /// A `--limit` wider than the corpus cut nothing, so a predicate over the
    /// result observed the whole universe and there is nothing to refuse. This
    /// is what keeps `list --limit 100000 --filter …` working.
    pub fn truncated_the_universe(&self) -> bool {
        if self.offset > 0 {
            return true;
        }
        self.universe_total
            .is_some_and(|total| self.applied < total)
    }
}

static CEILING: OnceLock<QueryCeiling> = OnceLock::new();

/// Declares the ceiling this process's query applied. First call wins.
///
/// Called by the command at the point it resolves the effective limit, which is
/// also where it knows the source and, for a paginated command, the total.
pub fn record(ceiling: QueryCeiling) {
    let _ = CEILING.set(ceiling);
}

/// The declared ceiling, or `None` when the command declared none.
pub fn get() -> Option<&'static QueryCeiling> {
    CEILING.get()
}
