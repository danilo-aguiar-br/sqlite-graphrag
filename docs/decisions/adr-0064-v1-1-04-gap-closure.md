# ADR-0064: v1.1.04 — Two Structural Gaps Closure (deep-research Nested-Runtime Panic, entity-connect Convergence)

- Status: Accepted
- Date: 2026-07-08
- Release: v1.1.04 (crate `1.1.4`)
- Supersedes: none
- Superseded by: none
- Related: ADR-0060 (enrichment backlog convergence), ADR-0062 (v1.1.02 gap closure), ADR-0063 (v1.1.03 bug fixes)


## Context

v1.1.03 closed six operator-blocking bugs but left two structural gaps tracked in `gaps.md`: GAP-001 and GAP-002.

- GAP-001: `deep-research` panics 100% reproductibly with "Cannot start a runtime from within a runtime" because its sync entry point creates a dedicated Tokio runtime T1 and then calls the embedder which creates/acquires T2 on the same thread.
- GAP-002: `entity-connect` never converges because `count_operation_backlog` returns a hard-coded zero for `EntityConnect`, `scan_isolated_entity_pairs` does an O(n²) CROSS JOIN without an evaluated-pair marker, and `--until-empty` re-evaluates rejected "none" pairs forever.
- Both gaps block core capabilities: GAP-001 makes `deep-research` entirely unusable.
- GAP-002 leaves ~11 079 degree-0 entities invisible to multi-hop traversal.
- GAP-002 also wastes LLM cost on infinite re-scans of pairs already evaluated as "none".


## Decision

- Apply Option A (surgical) AND Option B (defence in depth) for GAP-001: extract the per-sub-query embedding loop into a new sync helper `compute_sub_embeddings` that runs BEFORE `Builder::new_multi_thread` in `deep_research::run`.
- Propagate the canonical `Handle::try_current` + `block_in_place` reentry pattern (already canonical at `embedder.rs:1435` and `extract/llm_embedding.rs:629`) to the three OpenRouter embedding paths.
- The three affected paths are: single at ~1016, serial batch at ~1155, and JoinSet fan-out at ~1172.
- Also propagate the reentry pattern to `ingest_opencode`.
- For GAP-002, introduce a four-part fix:
- Part 1: migration V016 creating the `entity_connect_seen` table recording the LLM verdict per evaluated pair.
- Part 2: make `scan_isolated_entity_pairs` seen-aware via `LEFT JOIN entity_connect_seen` and prioritise hub entities.
- Part 3: replace the hard-coded `EntityConnect => 0` arm in `count_operation_backlog` with a real O(n) backlog proxy counting degree-0 entities that have NER bindings.
- Part 4: persist the verdict in `call_entity_connect` on both the `related` and `none` branches.


## Consequences

- `deep-research` works in 100% of invocations; the `--json` contract holds even on transient embedding failures.
- `entity-connect --until-empty` converges; each evaluated pair costs LLM exactly once.
- `enrich --status` reports a truthful backlog instead of a hard-coded zero.
- Multi-hop traversal can now reach the previously-isolated degree-0 entities.
- Schema advances v15 to v16; `migrate --json` is REQUIRED on upgrade (V016 is a numbered migration, not an idempotent ALTER).
- The `entity-connect` enrich operation is promoted from "scan-only" to "fully-implemented".
- Defence in depth: any future subcommand that creates its own runtime before calling the embedder will not re-trigger GAP-001 because the three embedding paths now use the reentry-safe pattern.
- The `entity_connect_seen` table schema is: `(source_id, target_id, namespace, verdict, relation, evaluated_at)`.
- The table carries a composite PK, dual FK ON DELETE CASCADE to `entities(id)`, a CHECK on `verdict`, and a namespace index.
- `CURRENT_SCHEMA_VERSION` advances from 15 to 16.


## Validation

- `cargo build --release` — zero errors.
- GAP-001 and GAP-002 implementation tasks validated with their dedicated tests prior to this docs task.
- No new tests run in this docs task (tests already validated in the implementation tasks).


## Commits

- GAP-001 (deep-research nested-runtime fix) and GAP-002 (entity-connect convergence) implementation commits (see the code tasks).
- This ADR + INDEX.md + gaps.md + schemas/README.md close the v1.1.04 release.
