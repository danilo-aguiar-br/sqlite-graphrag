# ADR-0065: v1.1.05 — Five Operator Bugs from the "danilo" Deep-Research Incident

- Status: Accepted
- Date: 2026-07-11
- Release: v1.1.05 (crate `1.1.5`)
- Supersedes: none
- Superseded by: none
- Related: ADR-0063 (v1.1.03 bug-fix wave), ADR-0064 (v1.1.04 deep-research nested-runtime + entity-connect), ADR-0044 (prior multi-bug hotfix pattern)


## Context

On 2026-07-08 an operator ran deep multi-hop research against a large production graph (`graphrag.sqlite`, binary v1.1.4) on the subject `"danilo"`. Five CLI bugs blocked the investigation; a sixth shell-side error amplified one of them. They are catalogued in `gaps.md` ("Bugs do GraphRAG — Relato de Deep Research sobre danilo") and closed in v1.1.05. No SQLite schema migration is required (`CURRENT_SCHEMA_VERSION` stays at 16 from V016 / ADR-0064).

| # | Symptom | Root cause (summary) |
|---|---------|----------------------|
| Bug 1 | `deep-research "danilo"` produced a single hybrid search instead of multi-aspect fan-out | Heuristic `decompose_query` was purely syntactic; single tokens never split |
| Bug 2 | `jaq`/`jq` failed to parse the full stdout capture | Envelope truncation / stderr contamination under shell redirects (`&>`), not invalid `serde_json` |
| Bug 3 | `graph traverse --from danilo` returned empty / opaque NotFound | Exact-name match only; short nicknames never resolved to kebab canonical names |
| Bug 4 | `merge-entities` accepted self-referential merges under malformed argv | Guard existed deeper in the path; zsh word-splitting could still put target into `--ids` before DB work was skipped early enough |
| Bug 5 | `link --from 89975 --create-missing` created a ghost entity named `"89975"` | Numeric strings treated as names; no ID-based link flags |
| Shell Error 1 | zsh word-splitting mangled multi-arg merge commands | Shell hygiene (arrays); mitigated in CLI by Bug 4 |

v1.1.04 made `deep-research` *runnable* again (nested Tokio panic, ADR-0064 GAP-001) but did not fix the single-token quality path that made research on a person-name subject useless.


## Decision

Apply five surgical CLI/UX fixes (plus shared atomic I/O) without advancing the database schema.

### D1 — Bug 1: single-token aspect fan-out

- Rename the planning path to `decompose_query_with_sources(query, max) -> Vec<(String, &'static str)>`.
- Keep existing syntactic branches (relational phrases, `;`, `and`/`e`/commas, multi-word pairs).
- When **no** branch fires and the query is a **single token**, expand into:

  1. the original token (`source: "original"`), then
  2. `"{token} {aspect}"` facets (`source: "aspect"`) from `SINGLE_TOKEN_ASPECTS` (EN/PT facets: patrimônio/stack/tecnologia/stakeholders/pessoas/projeto/decisão/relacionamento/contexto/architecture/history), capped by `--max-sub-queries` (default 7).

- Multi-word unsplittable queries still return a single `original` sub-query (no false aspect noise).
- Manual override remains first-class: `--sub-query-strategy manual --sub-queries-file PATH` labels lines as `source: "manual"`.

### D2 — Bug 2: atomic `--output` + global `--quiet` + documented contract

- New `deep-research --output PATH` writes the full envelope via `atomic_io::write_json_atomic` (tempfile → fsync → rename).
- When `--output` is set, stdout emits a short ack `{ written, bytes, blake3, sub_queries_total, unique_memories_found, elapsed_ms }` instead of the multi-MB envelope.
- Global `--quiet` / `-q` suppresses non-error tracing so stderr does not pollute captures.
- Long help documents the contract: stdout = JSON only; stderr = logs; never `&>` the same file.

### D3 — Bug 3: fuzzy entity resolution for `graph traverse`

- Add `entity_name_similarity`, `suggest_entity_names`, and `resolve_entity_fuzzy` (rapidfuzz Jaro-Winkler + kebab prefix / first-token heuristics).
- Exact match still wins.
- Without `--fuzzy`: NotFound (exit 4) includes ranked canonical-name suggestions.
- With `--fuzzy`: a clear single winner is auto-resolved; a stderr warning records the substitution.

### D4 — Bug 4: pre-DB self-referential merge guard

- At the start of `merge-entities::run`, reject when `--into-id` ∈ `--ids` or `--into` ∈ `--names` **before** any DB open / resolve work.
- Keep the existing resolve-time defence-in-depth re-check.
- Closes the shell-splitting amplification path that could otherwise corrupt the graph under `--cross-namespace`.

### D5 — Bug 5: ID-based link + reject digit-only names

- New mutually exclusive flags: `--from-id` / `--to-id` alongside `--from` / `--to`.
- `validate_entity_name` rejects pure-ASCII-digit names so `--create-missing` cannot mint ghost ID-looking entities.
- Error text steers operators to `--from-id`/`--to-id`.

### Shared infrastructure

- New module `src/atomic_io.rs` (`write_atomic`, `write_json_atomic`) reused by Bug 2 and unit-tested.
- Integration suite `tests/v1105_danilo_bugs_regression.rs` covers all five bugs at the CLI boundary.


## Alternatives Considered

1. **LLM-driven query decomposition for single tokens (Bug 1)** — Rejected for default path: adds cost, latency, and OAuth dependency to a local-first command whose default `--mode` remains heuristic `none`. Manual strategy already covers expert facet lists.
2. **Only document "quote your redirects" for Bug 2** — Rejected as sole fix: multi-MB `--with-bodies` envelopes still race under SIGTERM/pipe buffers; atomwrite is the durable contract for agents.
3. **Always-on auto-fuzzy without a flag (Bug 3)** — Rejected: silent resolution can traverse the wrong entity in dense namespaces. Default stays exact + suggestions; opt-in `--fuzzy` for interactive recovery.
4. **Clap `value_parser` only for self-merge (Bug 4)** — Insufficient alone: IDs arrive as `Vec` after parsing; validation must compare sets. Pre-DB guard is the right layer.
5. **Auto-detect pure digits as entity IDs in `--from`/`--to` (Bug 5)** — Rejected: ambiguous (real names can be numeric in theory) and surprises scripts. Explicit `--from-id`/`--to-id` plus hard reject of digit-only *names* is safer.
6. **Schema migration / new tables for any of the five** — Rejected: all five are CLI/resolution/output concerns; graph data model unchanged.


## Consequences

### Positive

- Single-token deep-research produces multi-aspect coverage (`source: "aspect"`) without LLM cost.
- Large JSON envelopes are crash-safe via atomwrite; pipelines verify `blake3` on the ack.
- Short nicknames are recoverable via suggestions or `--fuzzy`.
- Self-referential merges fail loud before any write; shell mistakes cannot orphan edges.
- Numeric ID misuse cannot create ghost entities; ID linking is first-class.
- No operator migration step (`migrate` not required for this release).

### Negative

- Aspect facet list is a fixed EN/PT heuristic — imperfect for arbitrary domains; operators who need domain-specific facets must use `--sub-query-strategy manual`.
- `--fuzzy` can still pick a wrong near-match if two entities score similarly; operators should prefer exact canonical names in automation.
- `deep-research.schema.json` historically enumerated `sub_queries[].source` as `original | decomposed` only; runtime now also emits `aspect` and `manual` (docs note in `docs/schemas/README.md`; optional schema regen is non-blocking for agents under Must-Ignore-friendly consumers, but strict validators should regen).

### Neutral

- Crate SemVer is `1.1.5` while the release brand is **v1.1.05** (leading zero rejected by cargo SemVer).
- `CURRENT_SCHEMA_VERSION` remains **16**.
- Shell Error 1 remains primarily an operator hygiene issue; CLI only hardens the merge path.


## Validation

- Unit: `test_decompose_single_token_danilo_fans_out`, atomic_io tests, clap/ID guards for link and merge.
- Integration: `tests/v1105_danilo_bugs_regression.rs` (`bug1`…`bug5`).
- Docs task does not re-run the full suite; implementation tasks already closed the five bugs.


## Commits

- Implementation of Bugs 1–5 + `atomic_io` + regression suite (code tasks).
- This ADR (EN + PT-BR), `docs/decisions/INDEX.md`, and the v1.1.05 schema docs note close the documentation side of the release.
- Primary tracker: `gaps.md` status table (all five **FIXED** in v1.1.05).
