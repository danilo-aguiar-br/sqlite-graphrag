# ADR-0062: v1.1.02 — Two Residual Gaps Closed (GLiNER Removal, TooManyTokens Typed) + Entity Orphan Prune + Re-Embed Regression Test

- **Status**: Accepted
- **Date**: 2026-07-06
- **Release**: v1.1.02 (crate `1.1.2`)
- **Supersedes**: none
- **Superseded by**: none
- **Related**: ADR-0058 (`--prune-dead-orphans`, memory-keyed), ADR-0059 (`--max-entity-degree` removal precedent), ADR-0061 (v1.1.01 twelve-priority roadmap closure)

## Context

After v1.1.01 closed the 12-priority `gaps.md` roadmap (ADR-0061), a focused audit of `gaps.md` revealed three residual gaps that were not covered by the v1.1.01 scope, plus a documentation drift surfaced during the v1.1.02 release-prep audit:

1. **Gap 1 — `--gliner-variant` lingered as a deprecated no-op.** Since v1.0.79 the GLiNER ONNX pipeline had been removed, but the `--gliner-variant` clap flag, the `GlinerVariant` enum, and the `SQLITE_GRAPHRAG_GLINER_MODEL`/`SQLITE_GRAPHRAG_GLINER_THRESHOLD` env vars remained in the parser as no-ops that emitted a `tracing::warn!`. Documentation across `docs/`, root `*.md`, `llms*.txt` and the SKILL files still described them as "accepted for compatibility" — a false signal that they still did something.

2. **Gap 2 — `TooManyTokens` had no typed envelope.** Exit 6 was already typed for `BodyTooLarge{bytes,limit}` and `TooManyChunks{chunks,limit}` (P11 of v1.1.01), but a body that exceeded the embedding model's token ceiling (≈32k tokens for `qwen/qwen3-embedding-8b`) collapsed into a generic payload error with no `{tokens,limit}` fields in the JSON envelope.

3. **Gap 3 — entity-keyed re-embed dispatch was silently broken.** The `strip_prefix("entity:")` dispatch in `call_reembed` (`src/commands/enrich/extraction.rs`) routes a queue key to either `call_reembed_entity` or the memory path. The branch existed in trunk but had no regression test — a future refactor that dropped the `strip_prefix` would silently send every entity-keyed re-embed to the memory path (`QueryReturnedNoRows` → `NotFound` → dead-letter).

4. **Sub-gap — entity-keyed dead-letter had no prune command.** ADR-0058 added `enrich --prune-dead-orphans` but its predicate filters `item_type='memory'` only. Entity-keyed dead rows (from the historical 14 680-row accumulation under v1.1.1) had no dedicated cleanup path; operators had to edit the sidecar SQLite by hand.

5. **Docs drift.** The seven narrative root files and the `docs/` tree (AGENTS, HOW_TO_USE, MIGRATION, COOKBOOK, HEADLESS_INVOCATION, DOCUMENTATION_FRAMEWORK, TEST_PLAN, TESTING, `schemas/README.md`) all declared `Current release: v1.1.01`, pin `=1.1.1`, User-Agent `sqlite-graphrag/1.1.1`, and described `--gliner-variant` as a no-op. The release-publication surface (crates.io, docs.rs) would publish stale docs.

## Decision

### 1. Gap 1 — REMOVE `--gliner-variant`, `GlinerVariant`, and the GLINER env vars entirely

- Delete the `--gliner-variant` clap field from `RememberArgs` and `IngestArgs`.
- Delete the `GlinerVariant` enum.
- Delete the `SQLITE_GRAPHRAG_GLINER_MODEL` and `SQLITE_GRAPHRAG_GLINER_THRESHOLD` env-var reads.
- Clap now rejects `--gliner-variant` with exit 2, following the `--max-entity-degree` precedent established by ADR-0059 in v1.0.99.
- Also remove `--mode gliner` from the `IngestMode` enum — it now exposes only `none`, `claude-code`, `codex`, `opencode`. (Previously `gliner` was a deprecated variant that fell through to URL-regex; callers should use `--mode none` + `--enable-ner` for URL-regex extraction.)

**Trade-off**: BREAKING for any script that still passes `--gliner-variant` or `--mode gliner`. The mitigation is mechanical: `rg -- "--gliner-variant|--mode gliner" ci/ Makefile scripts/` and delete the occurrences. The env vars are silently ignored (no error) so CI pipelines that set them keep running — they just have no effect.

### 2. Gap 2 — Add `AppError::TooManyTokens{tokens,limit}` typed variant

- New enum variant `AppError::TooManyTokens { tokens: usize, limit: usize }`.
- Exit 6 is preserved; the JSON envelope now serializes `{tokens, limit}` for this variant, alongside the existing `{bytes, limit}` (BodyTooLarge) and `{chunks, limit}` (TooManyChunks).
- The validation lives at the boundary: `remember.rs`, `edit.rs`, `remember_batch.rs` estimate the token count before embedding and short-circuit with the typed error rather than letting the provider reject the request opaquely.

### 3. Gap 3 — Add regression test `tests/reembed_entities_integration.rs`

- Idiomatic skeleton mirrors `tests/v1063_features.rs`: `assert_cmd::Command`, `serial_test::serial`, `tempfile::TempDir`, `#[path = "common/mod.rs"] mod common;`.
- **Arrange**: `init(&tmp)`; `remember --name m1 --type note --description d --body "..." --graph-stdin --llm-backend none` with a curated payload declaring 2 entities. Verify `entities_persisted == 2`; open the DB via `rusqlite::Connection::open` and assert `COUNT(*) FROM entities == 2` and `COUNT(*) FROM entity_embeddings == 0` (because `--llm-backend none` produces an empty vector and `upsert_entity_vec` skips the write).
- **Act**: `enrich --operation re-embed --target entities --mode claude-code --embedding-backend llm` — the mock `tests/mock-llm/claude` (injected via `common::prepend_path`) returns `{"embedding":[0.0; 64]}`. (`--mode openrouter` is avoided to keep CI hermetic — no API key, no network.)
- **Assert**: reopen the DB; `COUNT(*) FROM entity_embeddings == 2`; the canonical coverage query `SELECT COUNT(*) FROM entities e LEFT JOIN entity_embeddings ee ON ee.entity_id=e.id WHERE ee.entity_id IS NULL == 0`.
- **Idempotency**: a second enrich run leaves `COUNT(*) FROM entity_embeddings == 2` (scan only elects rows that still miss a vector; `upsert_entity_vec` does DELETE+INSERT by `entity_id`).

### 4. Sub-gap — New flag `enrich --prune-dead-entity-orphans`

- `EnrichArgs` gains `#[arg(long, conflicts_with = "prune_dead_orphans")] pub prune_dead_entity_orphans: bool`. `conflicts_with` is bidirectional in clap, so the symmetric declaration on `prune_dead_orphans` is implicit.
- The `required_unless_present_any` arrays at the enrich gate are extended with `"prune_dead_entity_orphans"` (the flag is valid without `--operation`/`--mode`, just like `--prune-dead-orphans`).
- New `queue::prune_dead_entity_orphans(queue_conn, operation)`: SQL `DELETE FROM queue WHERE status='dead' AND item_type='entity' AND (operation=?1 OR operation IS NULL)`. No read of the main DB (entity rows are entity-keyed; orphan detection against the `entities` table is out of scope — the flag is for dead rows, which by definition already failed terminally). `PRAGMA wal_checkpoint(TRUNCATE)` runs when `pruned > 0`.
- The handler emits the existing `DeadSummary` struct; the `action` field discriminates (`"prune-dead-entity-orphans"` vs `"prune-dead-orphans"`).
- `src/cli.rs` `tolerates_missing_embedding_key` is extended so the flag does not require an embedding API key.
- Unit test `prune_dead_entity_orphans_removes_only_entity_dead_rows`: plants 3 rows (`entity:foo` dead, `mem-dead` dead, `entity:bar` pending); asserts `pruned == 1`, the memory-dead row survives, the entity-pending row survives.
- Integration test `tests/prune_dead_entity_orphans_integration.rs`: plants entity-dead + memory-dead in the sidecar, runs `enrich --operation re-embed --prune-dead-entity-orphans --json`, asserts `json["action"]=="prune-dead-entity-orphans"`, `json["pruned"]==1`, reopens the sidecar confirming entity-dead gone and memory-dead preserved.

**DRY/YAGNI notes**:
- The memory variant (`prune_dead_orphans`) has extra logic that checks the main DB for the memory's existence; the entity variant does NOT need that check (dead-letter rows are already terminal failures, not candidates for re-embedding). The two predicates are intentionally NOT unified.
- A `scope` field on `DeadSummary` would break the public `schemars::JsonSchema` contract; `action: &'static str` already discriminates.
- Chunk-keyed prune is YAGNI — there is no real accumulation and the re-embed path for chunks is idempotent.

### 5. Recovery strategy for the historical 14 680 entity-keyed dead rows

The new flag DELETES the queue rows but does NOT re-embed the entities. The correct operator sequence after upgrading is:

1. `enrich --operation re-embed --target entities --until-empty --max-runtime 600` — now that the dispatch is fixed (Gap 3), reprocessed items persist.
2. `enrich --operation re-embed --requeue-dead --ignore-backoff` — re-enqueue the 14 680 dead rows for one more attempt.
3. Only the TRUE orphans (entity-mother deleted from the main DB) will remain — those are the targets for `--prune-dead-entity-orphans`.

### 6. Docs alignment

The seven narrative root files (README.md, README.pt-BR.md, llms.txt, llms.pt-BR.txt, llms-full.txt, INTEGRATIONS.md, INTEGRATIONS.pt-BR.md) were aligned to v1.1.02 in commit `d24b4aa`. This ADR extends the same alignment to the `docs/` tree.

## Consequences

- **Positive**:
  - The parser no longer carries dead GLiNER weight; clap errors are loud and early (exit 2) instead of a silent `tracing::warn!`.
  - Exit 6 now has three typed variants; callers can branch on the JSON `error_class` to surface the right remediation (shrink body vs split chunks vs truncate tokens).
  - The entity-keyed re-embed dispatch is protected by a regression test; any future refactor that drops `strip_prefix` will turn the test red.
  - Operators have a dedicated, sidecar-only prune for entity-keyed dead rows — no hand-editing of `.enrich-queue.sqlite`.
- **Negative**:
  - BREAKING: scripts passing `--gliner-variant` or `--mode gliner` fail with exit 2. The fix is mechanical (delete the flag).
  - The `prune_dead_entity_orphans` predicate does not cross-check the main `entities` table; an operator who runs it on a database where the entity-mother still exists but the queue row is dead will lose the dead row without re-embedding. The mitigation is the documented recovery sequence (re-embed first, requeue-dead second, prune only what survives).
- **Neutral**:
  - Schema stays at v15 — no migration.
  - `IngestMode` enum surface shrinks; `gliner` is gone from `--mode` help.

## Validation

- `cargo check` exit 0.
- `cargo clippy --all-targets -- -D warnings` 0 warnings.
- `cargo fmt --check` 0 diffs.
- `cargo test --test reembed_entities_integration` green (entity_embeddings 0→2, idempotent).
- `cargo test --test prune_dead_entity_orphans_integration` green (`pruned==1`, entity-dead gone, memory-dead preserved).
- `cargo test --lib prune_dead_entity` green (3-row unit test).
- `cargo doc --no-deps` 0 warnings (4 pre-existing rustdoc warnings resolved).
- `enrich --help` displays `--prune-dead-entity-orphans`; `remember --gliner-variant small` exits 2.

## Commits

- `4570acd` — Gap 1 (remove `--gliner-variant`/`GlinerVariant`/GLINER env vars) + Gap 2 (`AppError::TooManyTokens` typed exit 6).
- `b73934b` — CHANGELOG entries.
- `b019531` — Gap 3 regression test + `--prune-dead-entity-orphans` flag + unit/integration tests.
- `a47b534` — 4 pre-existing rustdoc warnings resolved.
- `d24b4aa` — root docs alignment (README, llms\*.txt, INTEGRATIONS).
- This ADR + the `docs/` tree alignment close the release.
