# Test Plan


- Read the Portuguese version at [TEST_PLAN.pt-BR.md](TEST_PLAN.pt-BR.md)
- Companion guide: [TESTING.md](TESTING.md) documents infrastructure details per layer
- Created during the 2026-06-11 post-publication audit of v1.0.79 (gaps G46-G54)


## Objectives and Scope
### Why This Plan Exists
- G43 proved that suites outside the CI default path hide breakage for entire release cycles
- G50 proved that doctests run ONLY in CI, so a broken rustdoc example shipped in 10 releases
- The published crates.io artifact was never exercised directly before this plan existed
- This plan makes every layer explicit: what runs, when, with which command, and what passing means
### Scope
- Covers the sqlite-graphrag crate: lib unit tests, CLI integration, contracts, concurrency, benchmarks, post-publication audit
- Excludes manual exploratory testing and downstream consumer projects


## v1.2.0 regression gate (XDG + dim 1024 + offline E2E)

- Command: `bash scripts/e2e_offline_v120.sh` (expects **20/20 PASS** — 15 `check()` + 5 manual PASS; binary 1.2.0+; historical wrapper `e2e_offline_v118.sh` superseded)
- Companion unit/integration:
  - Help contract `tests/help_no_product_env` — help must not advertise product `SQLITE_GRAPHRAG_*` env as config
  - **GAP-SG-139** regression `tests/cli_db_noop_host_surfaces_regression.rs` — host/XDG leaves accept `--db` as documented no-op (`src/cli_db_noop.rs`: config×9, slots×3, cache×3, codex-models, completions)
- Scope: XDG-only harness; config set/list/effective; purge --now dry-run; EntityType fold; remember-batch description; pending-embeddings status; cache stats; help scrub (no Box about); **`list-skipped` / `requeue-skipped`** offline flags + help presence; effective **DEFAULT_EMBEDDING_DIM=1024** (`embedding.dim`)
- Contract smoke (manual or suite):
  - `deep-research "q" -o /tmp/dr.json --quiet --json` materializes file + ack blake3
  - `memory-entities --name … --json` includes `entities[].description`
  - `remember … --enqueue-enrich --json` may emit `entities_created` / `enrich_recommended`
  - `enrich --operation entity-descriptions --status --force-redescribe --json` exposes quality fields
  - `enrich --operation entity-descriptions --list-skipped --json` / `--requeue-skipped` recover skipped sink without raw SQL
  - entity-connect is fully implemented (persists); not documented as scan-only
  - host leaf no-op: `config doctor --db /tmp/x.sqlite` accepts `--db` without opening that path
- Pass criterion: offline harness **20/20**; zero help product-env advertisements; GAP-SG-139 regression green
- Companion docs: [TESTING.md](TESTING.md), [MIGRATION.md](MIGRATION.md), [CHANGELOG.md](../CHANGELOG.md) `[1.2.0]`


## v1.1.06 regression gate (entity-connect O(k) scan)
- Command: `/usr/bin/timeout 300 cargo test --test v1106_entity_connect_scan_regression`
- Scope: CLI-boundary + unit coverage for GAP-ENTITY-CONNECT-SCAN-CARTESIAN closed in v1.1.06
- **No schema migration** for this release: `CURRENT_SCHEMA_VERSION` stays **16**
- O(k) pair scan: co-occurrence + hub×island (never cartesian `entities × entities` with global ORDER BY)
- Queue keys `pair:{id1}:{id2}`, `item_type=entity_pair`; drain by PK without re-scan
- First-scan deadline: InterruptHandle / `--max-runtime` / soft 120s → Timeout exit 1 (not 75)
- NDJSON: `scan_start` before SQL (`operation`, `entities_in_namespace`, `backlog_degree0_proxy`); `scan_meta` (`pairs_enqueued_this_scan`)
- `cross-domain-bridges` shares the same scan path; GAP-002 `entity_connect_seen` preserved
- Pass criterion: ZERO failures
- Companion docs: [TESTING.md](TESTING.md), [ADR-0066](decisions/adr-0066-v1-1-06-entity-connect-scan.md), suite file [`tests/v1106_entity_connect_scan_regression.rs`](../tests/v1106_entity_connect_scan_regression.rs)
- Also run unit tests in enrich: `/usr/bin/timeout 300 cargo test --lib commands::enrich`


## v1.1.05 regression gate (danilo incident)
- Command: `/usr/bin/timeout 300 cargo test --test v1105_danilo_bugs_regression`
- Scope: CLI-boundary coverage for the five operator bugs closed in v1.1.05
- Bug 1: single-token `deep-research` emits `source: "aspect"` fan-out; optional manual path `--sub-query-strategy manual --sub-queries-file PATH` (operator/smoke, not a separate suite case)
- Bug 2: `deep-research --output` writes atomic JSON via **atomwrite** and returns a stdout ack (`written`, `bytes`, `blake3`, …); global **`--quiet`** pairs with the stderr contract
- Bug 3: `graph traverse` short name suggests or resolves with `--fuzzy`
- Bug 4: `merge-entities` rejects self-referential `--ids`/`--into-id` before DB work
- Bug 5: `link` rejects pure-numeric names and accepts `--from-id`/`--to-id`
- Pass criterion: ZERO failures (5 tests)
- Companion docs: [TESTING.md](TESTING.md), [ADR-0065](decisions/adr-0065-v1-1-05-danilo-bugs.md), suite file [`tests/v1105_danilo_bugs_regression.rs`](../tests/v1105_danilo_bugs_regression.rs)


## Test Layer Matrix
### Layer 1 — Unit Tests (per commit)
- Command: `/usr/bin/timeout 300 cargo nextest run --profile default`
- Scope: pure functions, parsing, validation, error variants inside `src/`
- Pass criterion: ZERO failures
- Note: tests reading the global embedding dim MUST be `#[serial_test::serial(env)]` (G50 cause E)
### Layer 2 — Integration Tests (per commit)
- Command: same nextest invocation; files live in `tests/`
- Prerequisite: `export PATH="$PWD/tests/mock-llm:$PATH"` (dim-aware mocks since G51)
- Pass criterion: ZERO failures
### Layer 3 — Doctests (per commit, MANDATORY locally)
- Command: `/usr/bin/timeout 300 cargo test --doc`
- nextest DOES NOT execute doctests; skipping this layer locally is how G50 cause A shipped broken for 10 releases
- Pass criterion: ZERO failures
### Layer 4 — Slow Contract Suites (per release)
- Command: `/usr/bin/timeout 1800 cargo nextest run --profile heavy --features slow-tests`
- Command: `/usr/bin/timeout 1200 cargo test --features slow-tests --test doc_contract_integration -- --nocapture`
- Command: `/usr/bin/timeout 1200 cargo test --features slow-tests --test prd_compliance -- --nocapture`
- Pass criterion: ZERO failures across ~1881 tests
### Layer 5 — Loom Concurrency (explicit opt-in only)
- Command: `/usr/bin/timeout 3900 bash scripts/test-loom.sh`
- THERMAL RISK: never run outside the dedicated script (2026-04-19 incident)
- Pass criterion: all gated models complete within preemption bounds
### Layer 6 — Benchmarks (per release, informative)
- Command: `/usr/bin/timeout 1800 cargo bench --bench regression_baseline -- --quick`
- Prerequisite: mock LLM on PATH (G50 cause C)
- Pass criterion: no regression above 10 percent versus stored baseline
### Layer 7 — Post-Publication Black-Box (per release, MANDATORY)
- Target: the binary installed from crates.io (`cargo install sqlite-graphrag`), never `target/`
- Setup: temp database via `SQLITE_GRAPHRAG_DB_PATH`, isolated namespace, dim-aware mocks on PATH
- Matrix: bootstrap (init/health/migrate/stats), CRUD lifecycle, search commands, graph commands, maintenance (fts/optimize/backup/vec/export), exit-code contracts (1, 2, 3, 4, 9), JSON contracts versus `docs/schemas/`
- Robustness: OAuth-only abort with `ANTHROPIC_API_KEY` set, SIGPIPE exit 141 on large output, invalid `--tz` exit 2, invalid `SQLITE_GRAPHRAG_EMBEDDING_DIM` warns (G49)
- Dimensionality: fresh database adopts 64; pre-seeded 384 database is adopted (G43) and batches shrink (G44)
- Tarball: download the `.crate`, verify no forbidden files (scripts/legacy, agent configs) and correct READMEs
- Pass criterion: every command matches its expected exit code and schema; this layer would have caught G46-G49 before users did
### Layer 8 — Real-LLM Smoke (per release, OAuth cost)
- Commands: one small create with curated graph, one `recall` round-trip, one `edit --force-reembed`
- Budget: 3 LLM calls, under 5 minutes total; expected create latency under 90 seconds (G42 criterion)
- Record the top-hit score for the retrieval-quality baseline (G54)
- Rate limits are recorded as evidence, never retried in a loop


## Release Gates (run in order, stop on first failure)
### The 8 Mandatory Gates
- Gate 1: `cargo fmt --all --check`
- Gate 2: `/usr/bin/timeout 600 cargo clippy --all-targets --all-features -- -D warnings`
- Gate 3: layers 1-4 green, INCLUDING `cargo test --doc`
- Gate 4: `RUSTDOCFLAGS="-D warnings" /usr/bin/timeout 300 cargo doc --no-deps --all-features`
- Gate 5: `/usr/bin/timeout 120 cargo audit`
- Gate 6: `/usr/bin/timeout 180 cargo deny check advisories licenses bans sources`
- Gate 7: `/usr/bin/timeout 120 cargo publish --dry-run --allow-dirty` plus `cargo package --list` review
- Gate 8: GitHub Actions CI workflow GREEN on the release commit — publishing with a red CI is the root failure documented in G50
### Informative Gates (record, decide, do not skip silently)
- `cargo +stable semver-checks --baseline-version <previous>` — requires rustc >= 1.91; 9 major breaks shipped silently in v1.0.79 (G53)
- `cargo llvm-cov --lib --summary-only` — coverage target 80 percent for new code


## Triggers
### Per Commit
- Layers 1-3 plus Gates 1-2
### Per Release (before `cargo publish`)
- Layers 1-6 plus all 8 gates plus informative gates
### Post-Publication (after crates.io accepts the version)
- Layers 7-8 against the installed registry binary
- File new gaps in `gaps.md` using the G-number format for anything found


## Risks and Constraints
- Loom outside the script can thermally freeze high-core machines (hard reset on 2026-04-19)
- Real-LLM smoke depends on active OAuth; one call costs 10-90 seconds
- Background jobs longer than ~80 minutes can be killed by agent harnesses (G42/C1); keep test jobs short
- `cargo-nextest` and `cargo-llvm-cov` are NOT assumed installed; install via prebuilt binaries before Layer 1


## Latest Plans — v1.0.84 and v1.0.85

The Claude Backend Split test plan (ADR-0042) and the Five-Gap Remediation test plan (ADR-0043) are consolidated into this document; their standalone snapshot files were retired in v1.0.96.

## v1.0.99 Test Plan — Degree-Cap Removal + Doc/Convergence Fixes (ADR-0059, GAP-SG-67/68/69)

### Layer 1 (unit) changes
- GAP-SG-67: the 5 `enforce_degree_cap` unit tests and the `setup_cap_db` helper were removed with the function; no replacement regression test — the additive property is enforced by construction (no `DELETE FROM relationships` remains in the `remember`/`link` write path).
- GAP-SG-68: the 6 `build_order_by_*` tests pin the ascending default and the `--order desc` ordering the realigned doc-comment promises.
- GAP-SG-69: `skipped_item_keys_excludes_only_skipped_for_operation` pins that only `status='skipped'` rows for the operation are returned, so the body-enrich rescan converges.

### Manual / E2E validation
- GAP-SG-67: `remember`/`link` referencing a high-degree hub (degree > 50) — confirm via `graph stats` that the total relationship count does NOT decrease and the hub degree stays intact; passing `--max-entity-degree` must now fail with clap exit 2.
- GAP-SG-68: `graph entities --sort-by degree --json` returns ascending by default; `--order desc` returns most-connected-first.
- GAP-SG-69: run `enrich --operation body-enrich --mode openrouter ... --until-empty` against a DB with non-expandable short bodies — the backlog converges (empirically 55→3) and the `.enrich-queue.sqlite` sidecar is retained while `skipped` verdicts remain.

### Gate
- No migration; schema stays v15; `Cargo.toml` is 1.0.99.

## v1.1.04 Test Plan — Deep-Research Nested-Runtime Fix + entity-connect Convergence + Migration V016 (ADR-0064)

- Official release name v1.1.04; `Cargo.toml` is `1.1.4`; binary ~19 MiB; User-Agent `sqlite-graphrag/1.1.4`. Database migration REQUIRED (V016).
- Schema advances v15 → v16 via migration V016 (`entity_connect_seen` table).

### GAP-001 — deep-research nested-Tokio-runtime panic

- Reproduction (pre-fix panicked): `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1 CLAUDE_CONFIG_DIR=/tmp/graphrag-empty-config sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 deep-research "<query>" --k 5 --max-hops 2 --json` must emit a structured JSON envelope (success OR structured error), NEVER a panic.
- Regression test `tests/deep_research_nested_runtime_regression.rs`: invokes `deep_research::run` within an active Tokio runtime; asserts `Ok`/structured `Err`, not panic.
- Validates `compute_sub_embeddings` helper (embeddings computed BEFORE T1 construction) and the `Handle::try_current()` + `block_in_place(|| handle.block_on(fut))` pattern in the three OpenRouter paths of `embedder.rs`.

### GAP-002 — entity-connect convergence

- `enrich --operation entity-connect --status --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json` must report `scan_backlog > 0` (was always 0 before v1.1.04).
- `enrich --operation entity-connect --until-empty --max-runtime 600 --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json` must converge: `eligible_remaining` decays to 0; pairs appear in `entity_connect_seen` with verdict `related` or `none`; re-scans skip evaluated pairs.
- `graph stats --json` shows new edges after convergence.
- Regression tests: `count_operation_backlog_entity_connect_counts_isolated` (degree-0 entity WITH NER binding counts; without binding does not); `scan_isolated_entity_pairs_excludes_seen` (pair in `entity_connect_seen` not returned).
- The `count_operation_backlog_advisory_ops_report_zero` test excludes `EntityConnect` (now has a real backlog predicate).
- **v1.1.06 (GAP-ENTITY-CONNECT-SCAN-CARTESIAN)**: pair scan uses co-occurrence + hub×island (never full `entities × entities` ORDER BY); queue keys are `pair:{id1}:{id2}`; dry-run emits `scan_start` then `scan` within a short wall-clock bound. Suite: `tests/v1106_entity_connect_scan_regression.rs` plus unit tests in `scan.rs` / `queue.rs`.

### Migration V016

- `migrate --dry-run --json` lists V016; `migrate --json` applies it; `health --json` reports `schema_version >= 16` and `integrity_ok: true`.
- `schema_version_matches_migrations_count` passes with V016 present and `CURRENT_SCHEMA_VERSION = 16`.

### Suite totals

- ~1072 lib tests passing (up from ~1070 in v1.1.03 — two new regression tests). `cargo nextest -P ci` for the live count.

## v1.1.03 Test Plan — Enrich Atomic Batch + Literal Relation Migration + Cross-Namespace Merge + Stale Claim Recovery + Chunk Orphan Re-Embed + split-body

### Layer 1 (unit) additions
- `commands::enrich::queue::tests::enqueue_batch_is_atomic`: proves the batch enqueue path wraps every per-item insert in a single SQL transaction — an injected failure mid-batch rolls back ALL inserts, leaving the queue in its pre-batch state.
- `commands::reclassify_relation::tests::literal_to_writes_verbatim`: proves `--literal-to <RELATION>` stores the target VERBATIM (no clap normalization), symmetrical to `--literal-from`.
- `commands::reclassify_relation::tests::literal_from_applies_to_literal_to_applies_to_hyphen_migrates`: proves the canonical migration `--literal-from applies_to --literal-to applies-to` rewrites the stored literal to the hyphen form and reports the migrated count.
- `commands::merge_entities::tests::cross_namespace_merges_source_from_other_namespace`: proves `--cross-namespace` resolves `--ids`/`--into-id` across ALL namespaces and merges the foreign source into the target.
- `commands::merge_entities::tests::cross_namespace_default_false_rejects_cross_id`: proves that WITHOUT `--cross-namespace` a cross-namespace id is REJECTED (safe default) and no merge occurs.
- `commands::enrich::queue::tests::stale_processing_claim_is_reset_after_threshold`: proves a `processing` claim whose `claimed_at` is older than the stale threshold is reset to `pending` by the startup sweep.
- `commands::enrich::queue::tests::fresh_processing_claim_is_preserved`: proves a `processing` claim whose `claimed_at` is within the threshold is LEFT in `processing` (no false reset of live work).
- `commands::enrich::queue::tests::heartbeat_updates_claimed_at`: proves the background heartbeat advances `claimed_at` while an item is being processed, so a live worker is never classified as stale.
- `commands::enrich::tests::enrich_reset_stale_claims_manual_flag`: proves `enrich --reset-stale-claims` forces a reset of every stale claim immediately, independent of the startup sweep.
- `commands::enrich::scan::tests::scan_chunks_of_soft_deleted_memory_are_selected`: proves the chunk scan uses a `LEFT JOIN memories` so chunks whose parent memory was soft-deleted are STILL selected for re-embedding (previous behavior skipped them).
- `commands::enrich::scan::tests::count_backlog_includes_orphan_chunks`: proves `--status` `scan_backlog` for the chunk target includes orphan chunks in its count (no false `pending=0`).
- `commands::split_body::tests::split_body_divides_long_memory_into_parts`: proves `split-body --name <N>` divides an oversized body into daughters `{name}-part-{i}` at the configured threshold.
- `commands::split_body::tests::split_body_marks_original_as_superseded`: proves the original memory is marked with metadata `superseded_by_split: true` and is preserved in history.
- `commands::split_body::tests::split_body_creates_replaces_relations`: proves each daughter gets a canonical `replaces` relation pointing at the original so `related`/`graph traverse` still reach the superseded body.
- `commands::split_body::tests::split_body_preserves_history`: proves the split operation creates a history entry for the original (versioned, reversible via `restore`).

### Layer 2 (integration) additions
- `tests/split_body_integration.rs`: end-to-end `split-body --batch --threshold 25000` over a fixture corpus, followed by `enrich --operation re-embed --target memories`, asserting daughters become searchable via `recall` and the original still resolves via `replaces` edges.

### Gate
- No `migrate` on the main database; schema stays v15. The `.enrich-queue.sqlite` sidecar gains `claimed_at` via an idempotent `ALTER TABLE ADD COLUMN`. `Cargo.toml` is 1.1.3.


## v1.1.02 Test Plan — GLiNER Removal + TooManyTokens Typed + Re-Embed Regression + Entity Orphan Prune (ADR-0062)

### Layer 1 (unit) additions
- `commands::enrich::queue::tests::prune_dead_entity_orphans_removes_only_entity_dead_rows`: proves the new `prune_dead_entity_orphans` helper deletes only `status='dead' AND item_type='entity'` rows, leaving memory-keyed dead rows and live entity rows untouched.

### Layer 2 (integration) additions
- `tests/prune_dead_entity_orphans_integration.rs`: end-to-end CLI exercise of `enrich --prune-dead-entity-orphans --json`; plants both entity-dead and memory-dead rows in the sidecar, runs the flag, and asserts `pruned==1`, the entity-dead row gone, the memory-dead row preserved.
- `tests/reembed_entities_integration.rs`: regression for Gap 3 — `remember --graph-stdin` plants 2 entities with empty embeddings (`--llm-backend none`), then `enrich --operation re-embed --target entities` backfills both vectors (entity_embeddings 0→2); a second run is idempotent (no duplicate rows). Idiomatic skeleton mirrors `tests/v1063_features.rs` (`assert_cmd` + `serial_test` + `tempfile`).

### Regression rationale
- The `strip_prefix("entity:")` dispatch in `call_reembed` was silently broken for entity-keyed re-embed since the path was added; this regression test guarantees the dispatch keeps routing to `call_reembed_entity`.

## v1.0.97 Test Plan — Queue Sidecar from `--db` + Prune Dead-Letter Orphans (ADR-0056/0057/0058, GAP-SG-57..66)

### Layer 1 (unit) additions
- `paths::sidecar_path` (3 tests): an absolute `--db` derives the sidecar beside it; a bare filename (no parent) falls back to the CWD layout; a nested-directory `--db` derives the sidecar in that directory
- `prune_dead_orphans_removes_only_orphan_memory_rows`: only `status='dead'` rows with `item_type='memory'` whose `item_key` is absent from the main DB are deleted; entity-keyed dead rows and live-memory dead rows are untouched
- Production `unwrap`/`expect` audit (GAP-SG-57..60, ADR-0056) enforced by a Clippy lint gate (`-D warnings`); `parse_claude_output` de-duplication keeps the enrich and ingest_claude parsers behaviourally identical

### Layer 2 (integration) additions
- `tests/enrich_queue_db_isolation.rs`: enrich enqueues against `tmpA/db.sqlite`, then `enrich --status --db tmpA/db.sqlite` from a different CWD reports the backlog while `--db tmpB/db.sqlite` reports zero — proves the queue follows `--db`, not the CWD

### Flaky-test hardening
- GAP-SG-61 `concurrency_peak_never_exceeds_permits` and the GAP-SG-63 `llm_slots::tests` cluster were de-flaked (deterministic permit accounting); both green under the full suite

### Installed-binary smoke (GAP-SG-62)
- `cargo install --path . --locked --force` realigned `~/.cargo/bin/sqlite-graphrag` to 1.0.97; `installed_binary_smoke` now runs 26/0 WITHOUT the version-mismatch bypass

### Sealing totals
- `cargo test --lib` 973 passed / 0 failed; default `cargo test` 1164 / 0; `cargo test --features slow-tests` 1522 / 0 / 11 ignored; `cargo fmt --check` 0 diffs; `cargo clippy --all-targets --features slow-tests -- -D warnings` 0 warnings

## v1.0.96 Test Plan — Enrich Dead-Letter + OpenRouter REST Concurrency (ADR-0055, GAP-ENRICH-BACKLOG-CONVERGE, GAP-OPENROUTER-REST-CONCURRENCY)

### Layer 1 (unit) additions
- Outcome classification (`commands::enrich::tests`, 8 tests): rate-limit / timeout / db-busy map to `AttemptOutcome::Transient`; validation / parse map to `HardFailure`
- `open_queue_db`: idempotent `ALTER TABLE` adding the `error_class` and `next_retry_at` columns (a re-run is a no-op)
- `record_item_failure`: a HardFailure marks the item `dead` immediately; a Transient marks it `pending` with a future `next_retry_at` via `compute_delay`; a Transient past `--max-attempts` marks it `dead`
- Dequeue eligibility: rows with a future `next_retry_at` are skipped and `dead` rows are excluded, so the live set is strictly decreasing
- Embedding fan-out order (`embedder::tests::reassemble_ordered_restores_input_order`): out-of-order `JoinSet` completion is reassembled by chunk index, restoring input order

### Layer 2 (integration) additions
- Dead-letter convergence: ingest 6 ADRs with `--mode none`, then `enrich --until-empty --rest-concurrency 8` drains `unbound_backlog` 6 → 0
- Idempotent second pass: re-running `enrich --until-empty` does zero work (~6 ms) — no eligible items remain

### Layer 8 (real-LLM smoke) deltas
- `tests/openrouter_live_concurrency.rs` (`#[ignore]`, run with `cargo test --test openrouter_live_concurrency -- --ignored --nocapture`): embeds 64 chunks from `docs/*.md` at k=1 vs k=8
- Order proof: cosine diagonal 0.9999, off-diagonal max 0.899, argmax 64/64 — chunk order preserved despite out-of-order JoinSet completion
- Suite total: 1086 passed, 0 failed, 6 skipped via nextest

## v1.0.95 Test Plan — OpenRouter Chat Enrich (ADR-0054, GAP-OR-ENRICH)

### Layer 1 (unit) additions
- `ChatRequest` assembly (`src/chat_api.rs`, `OpenRouterChatClient`): wiremock tests verifying `response_format` `json_schema` with `strict:true`, `provider.require_parameters:true`, and `reasoning.enabled:false`
- Response parsing: extraction of `choices[].message.content` followed by a second JSON parse of the strict-schema payload
- `usage.cost` reading from the response body
- Retry: `429` with `retry-after` header, `5xx` exponential backoff, `401` permanent without retry
- `400`/`404` errors returned without retry
- Empty content / refusal response treated as incompatible model
- `validate_mode_flags`: rejects `claude`/`codex`/`opencode` flags under `--mode openrouter`
- `--openrouter-model` required: returns exit 1 before any network call when absent

### Layer 2 (integration) additions
- JUDGE dispatch to `call_openrouter` across all enrich operations (`memory-bindings`, `entity-descriptions`, `body-enrich`)
- API key validation via `resolve_api_key` without subprocess spawn

### Layer 8 (real-LLM smoke) deltas
- `tests/openrouter_chat_real.rs` (`#[ignore]`, runnable with `OPENROUTER_API_KEY`) iterating the 13 text models against the strict schema
- Compatibility matrix 13/13 (9 direct with `reasoning.enabled:false`, 4 via reasoning-mandatory fallback)

## v1.0.93 Test Plan — OpenRouter Embedding Backend (ADR-0052, GAP-OR-INGEST)

### Layer 1 (unit) additions
- `model_default_input_type()`: 10 tests covering per-model `input_type` selection (BUG-OR-1 fix — NVIDIA Nemotron returns `"passage"`, Mistral returns `None`, others return `"search_document"`)
- `model_supports_mrl()`: tests covering MRL detection for all 10 verified models including NVIDIA and BAAI (BUG-OR-2 fix)
- `validate_model_id()`: tests covering model ID validation against the 10 approved models and rejection of 5 non-existent IDs (BUG-OR-3, BUG-OR-4 fixes)
- `execute_with_retry()`: test covering HTTP 200 with malformed body retry (BUG-OR-5 fix — parse error on HTTP 200 treated as transient)

### Layer 2 (integration) additions
- `tests/openrouter_embedding.rs`: wiremock-based integration tests covering the full OpenRouter REST API embedding flow — request building, MRL truncation, `input_type` per model, batch chunking (MAX_BATCH_SIZE=32), error retry, and `secrecy::SecretString` API key handling
- `EmbeddingBackendChoice` propagation: tests verifying that `--embedding-backend openrouter` reaches all 8 commands (remember, remember-batch, ingest, recall, edit, restore, hybrid-search, deep-research)
- `--enrich-after` flag: tests verifying that `ingest --enrich-after` triggers sequential `enrich --operation memory-bindings` after embedding phase

### Layer 7 (post-publication) additions
- OpenRouter embedding round-trip: `remember` with `--embedding-backend openrouter --embedding-model "qwen/qwen3-embedding-8b"` followed by `recall` with same flags, verifying vector similarity
- Exit 78 on missing `--embedding-model` when `--embedding-backend openrouter` is specified

### Layer 8 (real-LLM smoke) deltas
- Optional: one OpenRouter embedding smoke test using a real `OPENROUTER_API_KEY` (opt-in via `SQLITE_GRAPHRAG_OPENROUTER_E2E=1`)
- Budget: 1 API call, under 5 seconds, expected embedding latency under 500ms

## Historical Plan — v1.0.80 Plan Deltas — G45, G53, G55 S2, G56, G58, ADR-0033, ADR-0034

The v1.0.80 release (patch bump, no schema migration) added the
following test deltas to the per-layer matrix above. Library
consumers are STRONGLY advised to pin to `=1.0.80` because the
lib API is unstable in v1.x.y (ADR-0032).

### Layer 1 (unit) additions

- `acquire_embedding_singleton` (G45): 5 tests covering same-db
  lock contention, distinct-db independence, `--wait-embed-singleton`
  polling, `force` flag, and PID-based stale-lock detection.
- `AppError::MemoryNotFound` and `AppError::MemoryNotFoundById`
  (G55 S2): 6 tests asserting the identifier is part of the
  variant, exit code is 4, and the pt-BR localized message
  carries name and namespace explicitly.
- `embed_entity_texts_cached` (G56): 4 tests asserting cache
  hit on second call with same model+text, miss on different
  text, `EmbedCacheStats` accounting, and behaviour when the
  underlying embedder returns an error.
- `recall --fallback-fts-only` and `hybrid-search --fallback-fts-only`
  (G58): 3 tests covering the FTS5-only path, plus 1 `#[ignore]`
  test that exercises the `EmbeddingFailed` path (requires `PATH`
  without `codex` or `claude`).

### Layer 2 (integration) additions

- `tests/completions.rs`: 7 end-to-end tests for the `completions`
  subcommand (bash, zsh, fish, powershell, elvish, invalid shell
  exit code, non-empty output validation per shell).
- `tests/shutdown_bypass.rs`: 3 integration tests covering the
  3-layer SHUTDOWN bypass recipe (`PATH=tests/mock-llm:...` plus
  `SQLITE_GRAPHRAG_IGNORE_SHUTDOWN=1` plus `setsid -w timeout`).
- `tests/embedder_singleton.rs`: 2 integration tests covering
  the cross-process embedding singleton against a temp database
  (concurrent `remember` invocations on the same `(namespace, db)`
  pair serialize; distinct pairs proceed in parallel).

### Layer 3 (doctest) additions

- 4 new doctest examples for `acquire_embedding_singleton`,
  `embed_entity_texts_cached`, `MemoryNotFound` construction, and
  the 3-layer SHUTDOWN bypass recipe (verified via
  `cargo test --doc` on every commit).

### Layer 4 (slow contract) additions

- `tests/doc_contract_integration.rs`: 2 new contract tests
  validating that the `vec_degraded`, `vec_error` and `warning`
  envelope fields appear in `recall` and `hybrid-search` JSON
  responses when the LLM subprocess fails (G58).
- `tests/prd_compliance.rs`: 1 new PRD-compliance test validating
  that the 6 new public library symbols documented in
  CHANGELOG.md (G45 and G56) are all `pub` and have the documented
  signatures.

### Layer 7 (post-publication) additions

- The post-publication black-box matrix now includes 3 new
  exit-code contracts: `EmbeddingSingletonLocked` (exit 75,
  retryable), `MemoryNotFound` with identifier in the message
  (exit 4), and `vec_degraded: true` in `recall` (exit 0 with
  warning).

### Layer 8 (real-LLM smoke) deltas

- The top-hit score from the real-LLM `recall` round-trip is
  recorded as the new G54 retrieval-quality baseline (existing
  field in the smoke protocol; v1.0.80 just makes the recording
  mandatory).

### Gates — new additions

- Gate 2 (clippy) gains `--all-features` (was `--all-targets`
  only) and remains the blocking bar.
- Gate 8 (CI GREEN) now requires the new `semver-checks` job
  (informational mode in v1.0.80, will become blocking in
  v1.0.81). The duplicate `--manifest-path` bug from the
  v1.0.79-initial commit is fixed.
- The windows-2025 matrix jobs gained pre-warm and verify steps
  gated on `if: matrix.os == 'windows-2025'` (ADR-0033, G53-WINDOWS-INFRA).
  Local cross-compile validation: `cargo check --target
  x86_64-pc-windows-msvc --lib --all-features` reproduces and
  `E0463` is fixed by `rustup target add x86_64-pc-windows-msvc
  --toolchain 1.88`; the build then reaches the `cc-rs: failed to
  find tool "lib.exe"` frontier, which is the expected host-Linux
  cross-compile limit.

### Triggers update

- Per commit: Layers 1-3 plus Gates 1-2 (unchanged).
- Per release (before `cargo publish`): Layers 1-6 plus all 8 gates
  plus informative gates. The new `semver-checks` informative
  gate is now part of this trigger.
- Post-publication: Layers 7-8 against the installed registry
  binary (unchanged). The Layer 7 matrix now includes the 3 new
  v1.0.80 exit-code contracts above.

## Traceability
- Every failure found by this plan becomes a numbered gap in `gaps.md` with status, root cause, and cause-effect chain
- Gaps fixed must reference the regression test that protects the fix
- Audit of 2026-06-11: this plan's first execution produced G46-G54 and their fixes
