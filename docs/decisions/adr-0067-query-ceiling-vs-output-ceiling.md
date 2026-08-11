# ADR-0067: v1.2.6 — Query ceiling vs output ceiling, and exit 2 for an incoherent request

- Status: Accepted
- Date: 2026-08-10
- Release: v1.2.6 (crate `1.2.6`), amended in v1.2.7
- Supersedes: none
- Superseded by: none
- Related: ADR-0042 (`backend_invoked`), GAP-SG-201 through GAP-SG-207


## Context

The agent-native surface (`crate::agent_surface`) reshapes an envelope that has already been serialized. It therefore sits **downstream of the SQL `LIMIT`** and cannot see what the query removed before it ran.

Two independent ceilings existed with no declared precedence between them:

| ceiling | applied by | removes |
| --- | --- | --- |
| query ceiling | the subcommand's `--limit` / `-k` / `--max-results` | rows, before serialization |
| output ceiling | `--max-items`, `--max-output-bytes` | elements, after serialization |

Measured on a corpus of 1892 memories:

```
--filter type=skill --count-only list              → 39   (input_count 1892)
--filter type=skill --count-only list --limit 50   → 0    (input_count 50, exit 0)
```

Both numbers are produced by the same code. Only one answers the question the caller asked. An array of fifty is indistinguishable from a corpus of fifty, so the predicate silently changed meaning and reported success.

The same blindness produced four further shapes that all answered an impossible request with `exit 0`: a key no element carries, a predicate redirected onto an array the caller never named, a knob declared against an envelope with no result array, and a mutating verb whose target came from ambient configuration rather than from the argv.

`exit 0` on any of these is the failure mode that matters: an agent reads "empty" as "the data is not there", concludes the memory does not exist, and writes a duplicate.


## Decision

1. **The command declares its ceiling.** `crate::agent_surface::universe::record` is called at the line that resolves the effective limit, where the source and — for a paginated command — the universe total are both known.
2. **Pagination and top-k are distinct.** `list` and `graph entities` page a countable universe, so "did the ceiling cut anything" has a factual answer. `hybrid-search -k`, `recall -k`, `related --limit` and `deep-research --max-results` bound a ranking: the top-k **is** the answer, not a truncation of one.
3. **Refuse only under evidence.** A predicate is refused only when the ceiling is `Pagination` **and** it actually removed rows. A top-k is never refused; it is reported, so its narrowness stops being invisible.
4. **Instrument always, refuse narrowly.** Every read command reports `query_limit`, `query_limit_kind`, `query_limit_source` and `filter_scope`, whatever the verdict.
5. **Never refuse after a mutation.** The surface runs at output time, after the handler has done its work. `Commands::mutates` fences it: a write is annotated, never refused.
6. **An incoherent request exits 2.**
7. **The resolved target is always reported**, and a verb with a side effect is refused when NOTHING named it. Only `TargetSource::Default` earns the refusal: `db.path` is a first-class registry key, so an XDG target is a designation the operator made once rather than per invocation, and rejecting it would make the product's own configuration surface unusable.


## Why exit 2 and not EX_USAGE 64

`sysexits.h` defines `EX_USAGE` as `64` for incorrect command usage — wrong argument count, bad flag, bad syntax. That is a genuine match for the refusals above, which are precisely invalid combinations of arguments.

It was rejected anyway, for one reason: **this binary already fixed `2` as its incorrect-usage code**, in `src/main.rs`. `rules-rust-cli-com-clap-io-exitcodes-erros` forbids reusing one exit code for two semantics — and adopting `64` now would do exactly that in reverse, creating **two** codes meaning "you used the command wrong" inside the same binary. A consumer would have to know which subsystem raised the error to know which number to expect.

Internal coherence beats external convention when the two conflict and the convention was never adopted here to begin with. `2` is also what `clap` itself returns for a parse failure, so the CLI now answers with one number for one meaning across both layers.


## Consequences

### Positive

- An impossible request fails loudly instead of returning an empty set with `exit 0`.
- The caller learns what the query removed even when nothing is refused.
- `discarded_flags` names dropped arguments as data, so no consumer parses prose.
- The resolved database appears in every envelope, making a misdirected write detectable.

### Negative

- A script that filtered a page and accepted the answer now fails. `--filter-scope page` restores it by declaring the narrower intent.
- A mutating verb whose target nothing named now exits 2. `--use-active` restores the previous behaviour, explicitly, and `config set db.path` remains a fully supported designation.
- The envelope of a database-touching command is no longer byte-for-byte what it was before the surface existed: it carries `agent_surface.db_path_source`.

### Neutral

- No schema migration. No new dependency. The 66 schemas that close their root already declared `agent_surface`, so the target needed no root-level member.


## Amendment in v1.2.7

v1.2.6 attached the target record inside `base_meta`, which runs downstream of two short-circuits: `emit_json` skips the layer when no knob is set, and `apply` returns early when the surface is inert. The target therefore appeared **only for a caller that had already set an unrelated flag**, and was omitted on the default path — the path every agent uses.

A universal contract must not hang off an optional block. v1.2.7 moved the decision to enter the layer from "a knob is set" to "a knob is set **or** there is a target to report", and gave `apply` an inert path that annotates without reshaping.

This was the third time in one release that a contract rested on a proxy rather than on the fact it claimed to observe. The other two are recorded in GAP-SG-202 and GAP-SG-203.


## References

- `gaps.md` — GAP-SG-201 through GAP-SG-207
- `docs/schemas/agent-surface.schema.json` — the declared record
- `https://man.openbsd.org/sysexits` — `EX_USAGE`
- `https://www.man7.org/linux/man-pages/man3/sysexits.h.3head.html` — `EX_USAGE`
