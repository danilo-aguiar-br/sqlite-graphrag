# sqlite-graphrag

[![Crates.io](https://img.shields.io/crates/v/sqlite-graphrag.svg)](https://crates.io/crates/sqlite-graphrag)
[![Docs.rs](https://docs.rs/sqlite-graphrag/badge.svg)](https://docs.rs/sqlite-graphrag)
[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](LICENSE)
[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)

> Persistent memory for AI agents in a single Rust binary with built-in GraphRAG.
> **Current release: v1.2.8.** Standing contract across the 1.2.x line: schema **v17** (V017 opened the `entity_type` vocabulary; v1.2.7 and earlier shipped v16), `DEFAULT_EMBEDDING_DIM=1024`, configuration precedence **CLI flag > XDG `config set` > default** (product env `SQLITE_GRAPHRAG_*` is **not** read on the hot path), embedding and enrich over **OpenRouter REST only**, manual releases (no GitHub Actions), crates.io owner `danilo-aguiar-br`. What each release changed lives in [CHANGELOG.md](CHANGELOG.md) — this banner is not a second copy of it.

- Read this document in [Portuguese (pt-BR)](README.pt-BR.md).

- Portuguese version available at [README.pt-BR.md](README.pt-BR.md)
- Public package and repository are live on GitHub and crates.io
- Install the latest published release with `cargo install sqlite-graphrag --locked`
- Upgrade an existing install with `cargo install sqlite-graphrag --locked --force`
- Verify the active binary with `sqlite-graphrag --version`
- See [CHANGELOG.md](CHANGELOG.md) for the full release history
- Release-grade validation includes the `slow-tests` contract suites documented in `docs/TESTING.md`
- Build directly from the local checkout with `cargo install --path .`
- **Upgrading to v1.2.0?** No database migration required if you are already on schema **v16** (from v1.1.04+) — just `cargo install sqlite-graphrag --locked --force` (or `cargo install --path . --locked --force` from this checkout). Crate pin `=1.2.0`. Schema stays at v16 (no main-DB migration). **DEFAULT_EMBEDDING_DIM=1024** (existing DBs keep `schema_meta.dim` until re-embed). Legacy XDG map: `db.default_path` → `db.path`. E2E seal: offline gate [`scripts/e2e_offline_v120.sh`](scripts/e2e_offline_v120.sh) (historical wrapper `scripts/e2e_offline_v118.sh` superseded by `e2e_offline_v120.sh`). Inherits v1.1.8 XDG contract: help scrub (no product env); OpenRouter URLs from XDG `network.openrouter.*`; query embed fail-fast; EntityType fold; `remember-batch` description required; `pending-embeddings status` / `cache stats`; `purge --now`; `config list --effective`; `related_to` → `related`. Residual honest: monólitos >800 LOC partial; live LQ backfill is an operator campaign. Full notes: [CHANGELOG.md](CHANGELOG.md) `[1.2.0]` and [docs/MIGRATION.md](docs/MIGRATION.md).
- **Upgrading to v1.2.2?** No database migration — main schema stays at **v16**. Just `cargo install sqlite-graphrag --locked --force` (or `cargo install --path . --locked --force`). Crate pin `=1.2.2`. **Additive only:** the eight agent-native output flags (`--select`/`--fields`, `--filter`, `--max-items`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`) reshape any subcommand's JSON envelope at a single point, so an agent no longer pipes a whole payload into `jaq` to read one field; a failure envelope (`error: true` / `ok: false`) is **never** filtered and always reaches the caller, `$schema` documents pass through untouched, NDJSON streams bypass the surface, and truncation is recorded under `agent_surface` plus a top-level `truncated` flag. `--no-input` refuses stdin declaratively: every stdin reader fails up front with **exit 1** (`AppError::Validation`) instead of blocking. With no flag set the envelope is byte-for-byte identical to v1.2.1 output. Inherits the v1.2.1 CAPA seal and v1.2.0 dim **1024** / XDG. Full notes: [CHANGELOG.md](CHANGELOG.md) `[1.2.2]`.
- **Upgrading to v1.2.1?** No database migration — main schema stays at **v16** (sidecar queue behaviour only). Just `cargo install sqlite-graphrag --locked --force` (or `cargo install --path . --locked --force`). Crate pin `=1.2.1`. **CAPA seal:** claim / count / `--resume` / `--retry-failed` require `operation` **and** `namespace` (an `ai-sdd` drain no longer claims `global` / empty-ns rows); `--until-empty` counts pending for **this op+namespace only**; `--force-redescribe` reopens `skipped`/`done` once per process via `reopen_force_redescribe_candidates` (never reopens `dead` — use `--requeue-dead`); re-embed eligibility uses live BLOB truth `LENGTH(embedding) = dim*4` (CORRUPT / META_AHEAD rows re-embed again) and `reconcile_satisfied_reembed_pending` clears zombies when the live vector already matches; enqueue strips `entity:` for entity lookup (queue key stays `entity:…`) and validates chunk keys exist in a non-deleted memory of the target namespace; CAPA-D low-quality markers use compound "configuration file" phrases only (no bare `%configuration file%` FP). Inherits v1.2.0 dim **1024** / XDG / `--list-skipped`. Full notes: [CHANGELOG.md](CHANGELOG.md) `[1.2.1]`.

- **Upgrading from v1.0.74 / v1.0.75?** See [docs/MIGRATION.md](docs/MIGRATION.md) for the v1.0.76 / v1.0.77 / v1.0.78 / v1.0.79 migration procedure
- **Upgrading from v1.0.79 to v1.0.80?** No database migration required; just `cargo install sqlite-graphrag --locked --force`. v1.0.80 adds the CI `semver-checks` job (informational), the Windows pre-warm steps (ADR-0033), and the panic-free third-signal exit (ADR-0034). Library consumers must pin to `=1.0.80`; see the `Stability Policy` below.
- **Upgrading from v1.0.80 / v1.0.81 to v1.0.82?** Two new migrations run automatically on first `init`/`migrate`: `V014__pending_memories` (pending `remember` checkpoint queue) and `V015__pending_embeddings` (pending embedding retry queue). After upgrading, run `codex login` once to refresh the OAuth refresh token — the 2026-06-14 incident showed that `codex exec` returning HTTP 401 `refresh_token_reused` is now caught by the new fallback chain (ADR-0040) and routed to the next backend in `--llm-backend codex,claude`. See [docs/MIGRATION.md](docs/MIGRATION.md) for the full 6-step procedure including rollback.
- **Upgrading from v1.0.82 / v1.0.83 to v1.0.85?** No database migration required; just `cargo install sqlite-graphrag --locked --force`. v1.0.84 (ADR-0042, GAP-002) added the real Claude backend split via `LlmEmbeddingBuilder` so `--llm-backend claude` invokes `claude` and never `codex`, the `backend_invoked` field in 7 JSON envelopes, the `vec_degraded_reason` field in `hybrid-search` and `recall`, the global --dry-run-backend flag for CI pre-flight, and `apply_env_whitelist_for_claude` for hardened providers. v1.0.85 (ADR-0043) extended `FallbackReason` from 3 to 7 variants with a `reason_code` discriminator (catches quota exhaustion, slot exhaustion, backend mismatch, dim zero, cancellation, timeout), `try_embed_query_with_deterministic_fallback` retries the alternative backend on `OAuthQuota` and sleeps 750ms on `SlotExhausted`, and `LlmEmbedding::invoke_claude` now captures 12-14 `anthropic-ratelimit-*-remaining` headers BEFORE checking the subprocess exit (G45-CR5). v1.0.85.1 (hotfix) restored the FTS5 failsafe for --llm-backend none (GAP-004, ADR-0043 hotfix). v1.0.85.2 (hotfix) made --dry-run-backend work standalone (BUG-001, ADR-0044), propagated resolved_kind from embed_via_backend so backend_invoked is populated in all 7 envelopes (BUG-002), and aligned the test mock JSON shape (BUG-003). Library consumers must pin to `=1.0.85.2`; see the `Stability Policy` below.
- **Upgrading from v1.0.91 / v1.0.92 to v1.0.94?** No database migration required; just `cargo install sqlite-graphrag --locked --force`. v1.0.94 adds the OpenRouter embedding backend (`--embedding-backend openrouter`), propagates `EmbeddingBackendChoice` to all 13 embedding paths (GAP-OR-PROPAGATION), fixes exit code 78 for OpenRouter config errors (BUG-OR-EXIT-CODE), and validates 10 embedding models E2E. Library consumers must pin to `=1.0.94`.
- **Upgrading to v1.1.06?** No database migration required; the schema stays at v16 from v1.1.04 — just `cargo install sqlite-graphrag --locked --force` (or `cargo install --path .` from this checkout). Closes GAP-ENTITY-CONNECT-SCAN-CARTESIAN: pair candidates come from co-occurrence in `memory_entities` plus hub×island fill (never full `entities × entities` ORDER BY); queue keys are `pair:{id1}:{id2}` with `item_type=entity_pair`; drain resolves by primary key; `--max-runtime` / soft 120s covers the **first** scan via `InterruptHandle` (Timeout exit 1); NDJSON emits `scan_start` (with `operation`, `entities_in_namespace`, `backlog_degree0_proxy`) before SQL and `scan_meta` with `pairs_enqueued_this_scan`. Suite: `tests/v1106_entity_connect_scan_regression.rs`. ADR-0066. Crate pin `=1.1.6`.
- **Upgrading to v1.1.05?** No database migration required; the schema stays at v16 from v1.1.04 — just `cargo install sqlite-graphrag --locked --force`. Closes the five operator-blocking bugs from the 2026-07-08 deep-research incident report (see `gaps.md`): (1) `deep-research` single-token queries expand to multi-aspect sub-queries (`source: "aspect"`, EN/PT facets; manual via `--sub-query-strategy manual --sub-queries-file`); (2) `deep-research --output PATH` atomic write (tempfile same dir → fsync → rename) with short stdout ack `{written, bytes, blake3, ...}` plus global `--quiet`/`-q` (never mix stderr into JSON with `&>`); (3) `graph traverse --fuzzy` auto-resolves a clear short-name winner, and without `--fuzzy` NotFound suggests ranked canonical names (rapidfuzz Jaro-Winkler + prefix); (4) `merge-entities` rejects self-referential merges (`--into-id` in `--ids`, or `--into` in `--names`) BEFORE any DB work; (5) `link --from-id`/`--to-id` resolve by ID, and pure digit names are rejected by `validate_entity_name` so `--create-missing` cannot create ghost numeric entities. Integration suite: `tests/v1105_incident_bugs_regression.rs`. The official release name is v1.1.05; the crate manifest carries `version = "1.1.5"`. Library consumers must pin to `=1.1.5`.
- **Upgrading to v1.1.04?** Database migration REQUIRED — `migrate --json` applies V016 (`entity_connect_seen` table). Just `cargo install sqlite-graphrag --locked --force`. Closes the two structural gaps tracked in `gaps.md`: (1) GAP-001 — `deep-research` no longer panics with "Cannot start a runtime from within a runtime"; the sync entry point now computes per-sub-query embeddings BEFORE building its dedicated Tokio runtime (`compute_sub_embeddings`), and the three OpenRouter embedding paths in `embedder.rs` adopt the canonical `Handle::try_current` + `block_in_place` reentry pattern; `ingest_opencode` is also guarded. (2) GAP-002 — `entity-connect` now converges: the new `entity_connect_seen` table (V016) records the LLM verdict per pair, the scanner excludes evaluated pairs, `count_operation_backlog` reports a real O(n) backlog, and `--until-empty` reaches `eligible_remaining == 0`. The `entity-connect` enrich operation is promoted from "scan-only" to "fully-implemented". The crate manifest carries `version = "1.1.4"`. Library consumers must pin to `=1.1.4`.
- **Upgrading to v1.1.03?** No database migration required; the schema stays at v15 (the enrich sidecar queue gains a `claimed_at` column via idempotent ALTER) — just `cargo install sqlite-graphrag --locked --force`. Closes the six operator-blocking bugs catalogued in `gaps.md` plus the V8 oversized-body gate. Bug fixes: (1) the enrich scan-enqueue path now batches candidate inserts in a single transaction instead of row-by-row under the WAL write lock; (2) `reclassify-relation` gains `--literal-to <RELATION>` so `--literal-from applies_to --literal-to applies-to --batch` migrates the 61 357 legacy underscore edges to canonical hyphen form; (3) `merge-entities` gains `--cross-namespace` (opt-in, default same-namespace) so `--ids`/`--into-id` resolve across all namespaces; (4) the enrich sidecar gains a `claimed_at` column plus `enrich --reset-stale-claims` and `enrich --stale-claim-secs <N>`, with stale `processing` claims reset on startup and a SIGTERM handler performing graceful cleanup before exit 19; (5) docs-only — the `enrich --status` help text clarifies `scan_backlog` vs `queue_pending` vs cooldown vs deadlock; (6) the `re-embed --target chunks` scanner switches to `LEFT JOIN memories` so chunks of soft-deleted mothers reach 100% coverage. New subcommand: `split-body` divides memories whose body exceeds 25 000 characters into daughter memories and creates `replaces` relations (daughters need a follow-up `enrich --operation re-embed --target memories`). New flags: `--literal-to`, `--cross-namespace`, `--reset-stale-claims`, `--stale-claim-secs`. The official release name is v1.1.03; the crate manifest carries `version = "1.1.3"` because the SemVer parser rejects a leading zero in the patch component. Library consumers must pin to `=1.1.3`.

- **Upgrading to v1.1.02?** No database migration required; the schema stays at v15 — just `cargo install sqlite-graphrag --locked --force` (the crate manifest carries `version = "1.1.2"` because the SemVer parser rejects a leading zero in the patch component). v1.1.02 closes the two residual gaps tracked after v1.1.01 plus regression coverage and a new prune flag: the deprecated --gliner-variant argument is dropped from `remember` and `ingest` (clap rejects it with exit 2, dead GLiNER plumbing deleted, tests/gliner_variant_removed_regression.rs); the embedding token ceiling raises the typed `AppError::TooManyTokens { tokens, limit }` enforced at the write boundary of `remember`/`remember-batch`/`edit` and inside the shared embedding client (exit 6 preserved); `tests/reembed_entities_integration.rs` guards the re-embed entity dispatch fix landed in v1.1.01; and `enrich --prune-dead-entity-orphans` prunes entity-keyed dead-letter rows from the queue sidecar (complementing the memory-scoped `--prune-dead-orphans`). Four pre-existing rustdoc warnings were also resolved. Library consumers must pin to `=1.1.2`.
- **Upgrading to v1.1.01?** No database migration required; the schema stays at v15 — just `cargo install sqlite-graphrag --locked --force` (the crate manifest carries `version = "1.1.2"` because SemVer rejects a leading zero in the patch component). v1.1.01 closes the 12-priority `gaps.md` roadmap: entity/chunk vectors are written and backfilled through the same OpenRouter REST path as memories, with an empty-vector guard on the vector upserts (P1); `enrich --operation re-embed --target memories|entities|chunks|all` backfills per table and also re-selects divergent-`dim` or empty-blob vectors (P2/P10); `graph recompute-degree` reconciles the cached `entities.degree` with `--dry-run` and the `{total, updated, zeroed, unchanged}` envelope (P3); `reclassify-relation --literal-from` matches the stored relation verbatim to migrate legacy hyphenated edges (P4); `merge-entities --ids/--into-id` and `rename-entity --id` disambiguate by ID within a namespace (P5); `health --json` and `embedding status --json` expose per-table vector coverage (`vec_*_missing`, `vec_*_coverage_pct`) (P6); `EntityType` fails early with a message listing the 13 valid values (P7); the exit-6 limit errors are the typed `AppError::BodyTooLarge`/`AppError::TooManyChunks` carrying bytes/chunks and the limit in the envelope (P11); and `ingest --name-prefix` prefixes every derived memory name (P12). Library consumers must pin to `=1.1.2`.
- **Upgrading to v1.1.0?** No database migration required; the schema stays at v15 (the enrich sidecar `.enrich-queue.sqlite` gains diagnostic columns via idempotent ALTER) — just `cargo install sqlite-graphrag --locked --force`. v1.1.0 resolves the enrichment dead-letter backlog at its root: truncated OpenRouter completions are detected (`finish_reason=length`) and retried with a grown `max_tokens` (GAP-SG-70/71), dead-letter rows carry `finish_reason`/`input_tokens`/`output_tokens` (GAP-SG-72, via `--list-dead --json`), retry-classification is fully typed with no message-substring matching (GAP-SG-73), the shared `openrouter_http` module de-duplicates the chat/embedding clients (GAP-SG-74), the HTTP User-Agent is `sqlite-graphrag/1.1.0` (GAP-SG-75), the dequeue is bounded under lock contention (exit 15 on sustained `SQLITE_BUSY`, GAP-SG-76), `enrich --status` reports a real per-operation `scan_backlog` that never diverges from a real scan (GAP-SG-77), and a not-yet-materialized entity is retried as `Transient` instead of dead-lettered on first miss (GAP-SG-78). Library consumers must pin to `=1.1.2`.
- **Upgrading to v1.0.99?** No database migration required; the schema stays at v15 — just `cargo install sqlite-graphrag --locked --force`. v1.0.99 removes the --max-entity-degree flag from `remember`/`link` (BREAKING — passing it now yields a clap exit 2; the obsolete --max-entity-degree 0 mitigation is no longer needed since writes never prune edges); no schema migration. v1.0.97 hardens the enrich dead-letter queue with recovery and inspection flags (`--requeue-dead` moves terminal `dead` items back to `pending`, `--list-dead` lists them with `error_class`/`message`, `--ignore-backoff` bypasses the `next_retry_at` cooldown, `--prune-dead-orphans` deletes orphan dead-letter rows whose memory was renamed or purged after enqueue), lets `--status`/`--list-dead`/`--requeue-dead`/`--prune-dead-orphans` run without `--operation`/`--mode`, adds the `augment-bindings` operation (requires `--names`) and `body-extract --body-extract-graph-only`, raises the `--max-attempts` default to 8 and the `--openrouter-timeout` default to 600s. `remember` gains `--graph-file` (combinable with `--body-file`), `--strict-name` and `--replace-graph`; `ingest` gains `--force-merge` with `body_hash` dedup and native large-body auto-split; `read` gains `--format raw`; `unlink` gains `--memory <name> --entity <name>` for curated bindings. `embedding status` adds a `coverage` object and `stats --json` a top-level `total_memories`. `--db` belongs AFTER the subcommand. **Historical note:** `SQLITE_GRAPHRAG_DB_PATH` was the position-independent override (SG-32) in that era; as of v1.2.0 product env is **not** read at runtime — use `--db` or `config set db.path`. Library consumers must pin to `=1.0.99`.
- **Upgrading from v1.0.94 to v1.0.95?** No database migration required; the schema stays at v15 — just `cargo install sqlite-graphrag --locked --force`. v1.0.95 adds `enrich --mode openrouter`, routing the extraction JUDGE through the OpenRouter REST `/chat/completions` endpoint so structured extraction (memory-bindings, entity-descriptions, body-enrich, etc.) no longer requires a local claude/codex/opencode CLI. New flags: `--openrouter-model` (required with `--mode openrouter`; no default — its absence exits 1 before any network call), `--openrouter-api-key` (XDG via `config add-key` (OPENROUTER_API_KEY is not read at runtime)), `--openrouter-timeout` (default 300s) and `--openrouter-base-url`. The SCAN→JUDGE→PERSIST pipeline is unchanged; only the JUDGE transport moves (ADR-0054). Library consumers must pin to `=1.0.95`.
- **Upgrading from v1.0.85 / v1.0.86 / v1.0.87 / v1.0.88 / v1.0.89 / v1.0.90 to v1.0.91?** No database migration required; just `cargo install sqlite-graphrag --locked --force`. v1.0.91 fixes GAP-SPAWN-001 (LLM subprocesses no longer inherit `.mcp.json` — embedding works zero-config in any project), BUG-17 (`entities.degree` inflation replaced by `recalculate_degree`), BUG-15 (7 schema enums), BUG-16 (`deep-research` schema), GAP-SPAWN-002 (orphan dir cleanup) and BUG-14 (test fix). Library consumers must pin to `=1.0.91`.

```bash
cargo install sqlite-graphrag --locked --force
sqlite-graphrag --version
```


## What is it?
### sqlite-graphrag delivers durable memory for AI agents
- Stores memories, entities and relationships inside a single SQLite file under 25 MB
- **Build:** LLM-only and one-shot — embeddings are generated via the OpenRouter REST API (`--embedding-backend openrouter`); no local model, no daemon, no ONNX runtime, ~19 MiB binary. `enrich --mode openrouter` runs the extraction JUDGE through the same REST transport (ADR-0054)
- **Legacy build:** REMOVED in v1.0.79 — the `embedding-legacy` feature and the local fastembed/ONNX path no longer exist
- Combines FTS5 full-text search with pure-Rust cosine similarity into a hybrid Reciprocal Rank Fusion ranker
- Stores and traverses an explicit entity graph with typed edges for multi-hop recall across memories
- Preserves every edit through an immutable version history table for full audit
- Runs on Linux, macOS and Windows natively with zero external services required (needs only an OpenRouter API key)


## Why sqlite-graphrag?
### Differentiators against cloud RAG stacks
- **OAuth-only LLM flow** — no API keys ever in the environment; the spawn ABORTS if `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` is set (defence in depth since v1.0.69)
- Single-file SQLite storage replaces Docker clusters of vector databases entirely
- Graph-native retrieval beats pure vector RAG on multi-hop questions by design
- Deterministic JSON output unlocks clean orchestration by LLM agents in pipelines
- Native cross-platform binary ships without Python, Node or Docker dependencies


## Stability Policy (G53, v1.0.80)

- The **public contract is the CLI**. The `--json` envelopes documented in `docs/schemas/*.schema.json` are stable across all v1.x.y releases. Consumers who depend on the CLI alone are not affected by minor or patch bumps.
- **No environment variable is part of that contract.** The binary reads exactly three environment variables at runtime — `CLICOLOR_FORCE`, `NO_COLOR` and `XDG_RUNTIME_DIR` — and none of them is product configuration; product env is **not read**. Earlier revisions of this section promised stability for "the environment variables listed in `llms.txt` and `llms-full.txt`", which contradicted the forbidden-product-env rule stated below and in the banner above.
- The **library API is unstable** in v1.x.y. Re-exports, public struct fields and function signatures may change in any v1.x.y release without a major version bump.
- Breaking changes to the library API ship as a **minor** bump, never patch (e.g. 1.0.79 -> 1.1.0 for a removed re-export). Patch bumps (1.0.79 -> 1.0.80) are limited to additive, non-breaking changes.
- Consumers who depend on the library API must pin to an exact version (`sqlite-graphrag = "=1.0.80"`) and review CHANGELOG.md before bumping.
- This stance is recorded in `docs/decisions/adr-0032-g53-lib-api-policy.md`.

## Superpowers for AI Agents
### First-class CLI contract for orchestration
- Every subcommand accepts `--json` producing deterministic stdout payloads
- **One-shot by default** — no background process; each embedding call is a single REST request
- Every write is idempotent through `--name` kebab-case uniqueness constraints
- Stdin is explicit: use `--body-stdin` for body text or `--graph-stdin` for one `{body?, entities, relationships}` object; raw entity and relationship arrays use `--entities-file` and `--relationships-file`
- `remember` accepts body payloads up to `512000` bytes and up to `512` chunks
- Relationship payloads use `strength` in `[0.0, 1.0]`, mapped to `weight` in outputs
- Stderr carries tracing output only under `-v`/`-vv`/`-vvv` or the XDG key `log.level`; use global `--quiet`/`-q` (v1.1.05) to suppress non-error tracing in headless pipelines (never mix stderr into JSON with `&>`)
- `--help` is English-first by design; use `--lang` for human-facing runtime messages, not static clap help text
- Cross-platform behavior is identical across Linux, macOS and Windows hosts


## Graph Schema
### Entity types, relation labels and edge strength
- `entity_type` accepts ANY term since v1.2.8; the label you send is stored as written and never replaced by another one. 13 values are RECOMMENDED: `project`, `tool`, `person`, `file`, `concept`, `incident`, `decision`, `memory`, `dashboard`, `issue_tracker`, `organization`, `location`, `date`
- `relation` (CLI input) accepts any kebab-case or snake_case string. 12 canonical values are well-known: `applies-to`, `uses`, `depends-on`, `causes`, `fixes`, `contradicts`, `supports`, `follows`, `related`, `mentions`, `replaces`, `tracked-in`. Custom values (e.g., `implements`, `tested-by`, `blocks`) are accepted with a `tracing::warn!`. JSON output normalizes to underscores (e.g., `applies_to`).
- `strength` is a float in `[0.0, 1.0]` representing edge weight; mapped to `weight` in all read outputs
- An `entity_type` outside the 13 recommended values is accepted, stored as written, and reported in the response `warnings` array. Pass `--strict-entity-types` to restrict the write to the 13 and be refused with exit code 1 instead. Only shape is enforced: trimmed, lowercased, hyphens turned into underscores, and rejected when empty, digits only, containing a line break, or longer than 64 characters. Custom `relation` values are accepted since v1.0.49.
- Use `sqlite-graphrag graph --format json` to inspect the full stored graph at any time


### 27 AI agents and IDEs supported out of the box (21 catalogued + 6 community)
| Agent | Vendor | Minimum version | Integration pattern |
| --- | --- | --- | --- |
| Claude Code | Anthropic | 1.0 | Subprocess with `--json` stdout |
| Codex | OpenAI | 1.0 | Tool call wrapping `cargo run -- recall` |
| Gemini CLI | Google | 1.0 | Function call returning JSON |
| Opencode | Opencode | 1.0 | Shell tool with `hybrid-search --json` |
| OpenClaw | Community | 0.1 | Subprocess pipe into `jaq` filters |
| Paperclip | Community | 0.1 | Direct CLI invocation per message |
| VS Code Copilot | Microsoft | 1.85 | Terminal subprocess via tasks |
| Google Antigravity | Google | 1.0 | Agent tool with structured JSON |
| Windsurf | Codeium | 1.0 | Custom command registration |
| Cursor | Anysphere | 0.42 | Terminal integration or MCP wrapper |
| Zed | Zed Industries | 0.160 | Extension wrapping subprocess |
| Aider | Paul Gauthier | 0.60 | Shell command hook per turn |
| Jules | Google Labs | 1.0 | Workspace shell integration |
| Kilo Code | Community | 1.0 | Subprocess invocation |
| Roo Code | Community | 1.0 | Custom command via CLI |
| Cline | Saoud Rizwan | 3.0 | Terminal tool registered manually |
| Continue | Continue Dev | 0.9 | Context provider via shell |
| Factory | Factory AI | 1.0 | Tool call with JSON response |
| Augment Code | Augment | 1.0 | Terminal command wrapping |
| JetBrains AI Assistant | JetBrains | 2024.3 | External tool per IDE |
| OpenRouter | OpenRouter | 1.0 | Function routing through shell |
| Minimax | Minimax | 1.0 | Subprocess invocation |
| Z.ai | Z.ai | 1.0 | Subprocess invocation |
| Ollama | Ollama | 0.1 | Subprocess invocation |
| Hermes Agent | Community | 1.0 | Subprocess invocation |
| LangChain | LangChain | 0.3 | Subprocess via tool |
| LangGraph | LangChain | 0.2 | Subprocess via node |


## Quick Start
### Install and record your first memory in four commands
```bash
cargo install sqlite-graphrag --locked --force
sqlite-graphrag init
sqlite-graphrag remember --name onboarding-note --type user --description "first memory" --body "hello graphrag"
sqlite-graphrag recall "graphrag" --k 5 --json
```
> **Required flags for `remember`:** `--name`, `--type`, `--description`. Body via `--body "text"`, `--body-file <path>`, or `--body-stdin` (pipe from stdin).
> **Body limit: 500 KB (512000 bytes).** Larger inputs are rejected with exit code 6 (`limit exceeded`); split into multiple memories or trim before sending.
> **Windows users (G29):** v1.0.68 is the first release since v1.0.65 that successfully compiles via `cargo install` on Windows. If you must stay on v1.0.66 or v1.0.67, see [docs/CROSS_PLATFORM.md](./docs/CROSS_PLATFORM.md) for the manual workaround.
- **GraphRAG is enabled by default and runs automatically.** Every subcommand auto-initializes its `graphrag.sqlite` if it does not exist — at the path given by `--db`, else the persisted `db.path`, else the XDG **data** directory (`~/.local/share/sqlite-graphrag/graphrag.sqlite`), never the current working directory unless that data directory cannot be resolved. Entity/relationship extraction comes from the embedding/LLM transport selected by `--llm-backend` or from curated graph input (`--graph-stdin`, `--entities-file`). There is no --extraction-backend flag: passing it is refused by clap with **exit 2** (`unexpected argument`).

### Automatic extraction (`--enable-ner`)
- Pass `--enable-ner` to activate automatic extraction on `remember` and `ingest` (product env is not read at runtime; v1.2.0)
- Since v1.0.79 this runs URL-regex extraction ONLY — the local GLiNER zero-shot pipeline was removed together with the `ner-legacy` feature
- --gliner-variant was REMOVED in v1.1.02 (clap rejects it with exit 2, following the --max-entity-degree precedent of v1.0.99); the `SQLITE_GRAPHRAG_GLINER_MODEL` and `SQLITE_GRAPHRAG_GLINER_THRESHOLD` env vars were deleted from the code in v1.1.02 and are silently ignored if set
- Response field `extraction_method` reports `url-regex`, `regex-only`, or `none:extraction-failed`
- For high-quality entity/relationship extraction pass curated entities via `--graph-stdin`, or run a SEPARATE `enrich` pass
- `--skip-extraction` is deprecated since v1.0.45 and has no effect

- **`sqlite-graphrag init` is OPTIONAL** but recommended on first use because it creates the database and applies migrations (there is no model download — embeddings come from the OpenRouter REST API).
- **`graphrag.sqlite` is created in the XDG data directory by default** — `~/.local/share/sqlite-graphrag/graphrag.sqlite` (override with `--db <path>` after the subcommand, or persist a default via `config set db.path <path>`; the current working directory is used only when the data directory cannot be resolved)
- For the local checkout, `cargo install --path .` is enough
- Re-run `sqlite-graphrag --version` after any upgrade to confirm the active binary
- After the public release, prefer `--locked` to preserve the tested MSRV dependency graph


## Version Highlights

Per-release history lives in [CHANGELOG.md](CHANGELOG.md), the single source of truth for what each version changed.

## Memory Lifecycle
### Runnable sequence: init → remember → recall → forget → purge
```bash
# 1. Initialize (once per database)
sqlite-graphrag init

# 2. Store a memory
sqlite-graphrag remember --name my-note --type user --description "demo" --body "first entry"

# 3. Retrieve by semantic similarity
sqlite-graphrag recall "first entry" --k 5 --json

# 4. Soft-delete (reversible)
sqlite-graphrag forget my-note

# 5. Permanently remove soft-deleted memories older than 0 days
sqlite-graphrag purge --retention-days 0 --yes
```
> All five commands above are safe to run in sequence on a fresh database.


## Installation
### Minimum supported toolchain
- Rust 1.88 or newer (`rust-version = "1.88"` in `Cargo.toml`); older toolchains will fail with an MSRV error during `cargo install`.
### Multiple distribution channels
- Install the latest published release with `cargo install sqlite-graphrag --locked`
- Upgrade an existing published binary with `cargo install sqlite-graphrag --locked --force`
- Pin to a specific version with `cargo install sqlite-graphrag --version <X.Y.Z> --locked`
- Install from the local checkout with `cargo install --path .`
- Build from the local checkout with `cargo build --release`


## Usage
### Initialize the database
```bash
sqlite-graphrag init
sqlite-graphrag init --namespace project-foo
```
- Without `--db` (or a persisted `db.path` via `config set`), every CRUD command resolves the XDG data-directory database, **not** `./graphrag.sqlite`: the envelope reports `db_path_source: "default"` and `db_path_resolved` pointing at `~/.local/share/sqlite-graphrag/graphrag.sqlite`. Product env `SQLITE_GRAPHRAG_DB_PATH` is **not** read at runtime (v1.2.0)
### Remember a memory with an optional explicit entity graph
- By default, `remember` does NOT run automatic URL extraction (off by default)
- Pass `--enable-ner` to activate URL-regex extraction for that call (the GLiNER pipeline was removed in v1.0.79). Product env overrides are not read at runtime (v1.2.0)
```bash
sqlite-graphrag remember \
  --name integration-tests-postgres \
  --type feedback \
  --description "prefer real Postgres over SQLite mocks" \
  --body "Integration tests must hit a real database."
```
- `remember` JSON response includes `urls_persisted` (URLs routed to `memory_urls` table) and `relationships_truncated` (bool, set when relationships were capped)
- URLs are stored in `memory_urls` via schema V007 and never pollute the entity graph
- Sample JSON output illustrating extracted entities and relationships:
```json
{
  "memory": {"id": 42, "name": "audit-note", "type": "project"},
  "extracted_entities": [
    {"name": "OpenAI", "kind": "organization", "saliency": 0.92},
    {"name": "Rust", "kind": "technology", "saliency": 0.85}
  ],
  "extracted_relationships": [
    {"source": "OpenAI", "target": "GPT-4", "relation": "develops"}
  ],
  "urls_persisted": [],
  "relationships_truncated": false
}
```
### Automatic extraction status (GLiNER removed in v1.0.79)
- The local GLiNER zero-shot NER pipeline was REMOVED in v1.0.79 with the `ner-legacy` feature; `--enable-ner` now performs URL-regex extraction only
- For LLM-curated entity/relationship extraction run a SEPARATE `enrich --mode openrouter` pass after `ingest --mode none`
- For exact control pass curated entities via `--graph-stdin`, `--entities-file` and `--relationships-file`
- The `extraction_method` field in the JSON response reports which path ran

```bash
sqlite-graphrag remember \
  --name release-notes-v1 \
  --type document \
  --description "release notes for v1.0.0" \
  --enable-ner \
  --llm-parallelism 4 \
  --body-stdin < notes.md
```
### OpenRouter Embedding Backend (v1.0.94)
- Use `--embedding-backend openrouter` with `--embedding-model` for fast REST API embeddings (~200ms per call vs 15s subprocess)
- The user MUST specify `--embedding-model` — no default model is hardcoded
Use `config add-key --provider openrouter` ou `--openrouter-api-key` (OPENROUTER_API_KEY não é lida em runtime)
```bash
# Remember with OpenRouter embedding
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  remember --name my-note --type note \
  --description "fast embedding" --body "content here"

# Ingest with OpenRouter + auto-enrich
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "google/gemini-embedding-001" \
  ingest ./docs --pattern "*.md" --recursive --enrich-after --json

# Recall with OpenRouter query embedding
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  recall "search query" --k 10 --json
```
- Supported models: `qwen/qwen3-embedding-8b` (best quality), `nvidia/llama-nemotron-embed-vl-1b-v2:free` (zero cost), `google/gemini-embedding-001` (highest scores), `openai/text-embedding-3-large`, and 6 more
- Default embedding dimensionality is **1024** (`DEFAULT_EMBEDDING_DIM`); override with `--embedding-dim` or `config set embedding.dim`. Legacy databases keep their recorded `schema_meta.dim` until re-embed
### Read, forget, edit and rename using positional name argument
<!-- skip-test: forget soft-deletes the memory mid-block, which then invalidates the subsequent edit/rename. The block is a lifecycle illustration, not a runnable script. -->
```bash
sqlite-graphrag read integration-tests-postgres --json
sqlite-graphrag forget integration-tests-postgres
sqlite-graphrag history integration-tests-postgres --json
sqlite-graphrag edit integration-tests-postgres --body "Updated body text."
sqlite-graphrag rename integration-tests-postgres --new postgres-tests
```
- Positional name is equivalent to `--name <name>` for `read`, `forget`, `history`, `edit` and `rename`

### Recall memories by semantic similarity
```bash
sqlite-graphrag recall "postgres integration tests" --k 3 --json
```
### Hybrid search combining FTS5 and vector KNN
```bash
sqlite-graphrag hybrid-search "postgres migration rollback" --k 10 --json
```
### Deep research with parallel multi-hop query decomposition (v1.0.64)
```bash
sqlite-graphrag deep-research "auth architecture decisions and incidents" --k 20 --json
```
- Decomposes the query into up to 7 sub-queries, runs them in parallel via bounded `JoinSet` + `Semaphore`, merges results with cross-query deduplication, and assembles evidence chains from graph traversal
- Defaults calibrated against NovelHopQA, StepChain, HopRAG benchmarks: `--k 20`, `--max-sub-queries 7`, `--max-hops 3`
### Inspect database health and stats
```bash
sqlite-graphrag health --json
sqlite-graphrag stats --json
```
### Purge soft-deleted memories after retention period
```bash
sqlite-graphrag purge --retention-days 90 --dry-run --json
sqlite-graphrag purge --retention-days 90 --yes
```
> **Default retention: 90 days.** To purge ALL forgotten memories regardless of age, pass `--retention-days 0`.

### Bulk-ingest every Markdown file under a directory
<!-- skip-test: requires a `./docs` directory containing Markdown files relative to the invocation cwd. -->
```bash
sqlite-graphrag ingest ./docs --type document --pattern '*.md' --recursive
```
### Bulk-ingest with low-memory mode (single worker)
<!-- skip-test: requires a `./docs` directory; demonstrates the --low-memory flag. -->
```bash
# Force single-threaded ingest to reduce RSS pressure (recommended for <4 GB RAM
# environments and container/cgroup constraints). Trade-off: 3-4x longer wall time.
sqlite-graphrag ingest ./docs --type document --pattern '*.md' --low-memory

# Or persist it as an XDG key (the CLI flag still takes precedence):
sqlite-graphrag config set ingest.low_memory true
```
### Bulk-ingest, then extract the graph
```bash
# Step 1 — bodies + embeddings (the only ingest mode)
sqlite-graphrag ingest ./docs --mode none --recursive --json

# Step 2 — graph extraction, SEPARATE process, only after step 1 exits 0
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model MODEL --until-empty --json
```
> **Authentication:** the OpenRouter API key is the only credential. Store it once with
> `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`,
> or pass `--openrouter-api-key`. Never place the key in shell history.
> See `docs/decisions/adr-0011-oauth-only-enforcement.md` for the full rationale.
> `ingest` emits NDJSON on stdout: one JSON line per file, then a summary line.
> Per-file `status` values: `indexed` (created), `skipped` (duplicate or invalid name), `failed` (error).
> Duplicates emit `status: "skipped"` with `action: "duplicate"` and do not count as failures.
> Pass `--dry-run` to preview the name mapping (kebab-cased basenames) without writing anything to the database.
> Schema: `docs/schemas/ingest-file-event.schema.json`, `docs/schemas/ingest-summary.schema.json`.

### Rename a memory while keeping its version history
<!-- skip-test: illustrative names (`old-name`, `new-name`) — the source memory does not exist in this isolated test database. -->
```bash
sqlite-graphrag rename old-name --new-name new-name --json
```
### Edit a memory body or description (creates a new version)
<!-- skip-test: depends on the memory not having been soft-deleted by an earlier example block. -->
```bash
sqlite-graphrag edit integration-tests-postgres --body "Updated body."
sqlite-graphrag edit integration-tests-postgres --description "Updated description."
```
### Restore a memory to a previous version
<!-- skip-test: `restore --version 2` requires the memory to have at least two versions, which is not the case in the isolated example database. -->
```bash
sqlite-graphrag history integration-tests-postgres --json
sqlite-graphrag restore --name integration-tests-postgres --version 2 --json
```
### Apply pending schema migrations
```bash
sqlite-graphrag migrate --status --json
sqlite-graphrag migrate --json
```
### Resolve namespace precedence for the current invocation
```bash
sqlite-graphrag namespace-detect --json
sqlite-graphrag namespace-detect --namespace project-foo --json
```
### Refresh SQLite query planner statistics
```bash
sqlite-graphrag optimize --json
```
### Reclaim disk space and checkpoint the WAL
```bash
sqlite-graphrag vacuum --json
```
### Create a typed relationship between two entities
<!-- skip-test: requires the `OpenAI` and `GPT-4` entities to already exist in the namespace. -->
```bash
sqlite-graphrag link --from "OpenAI" --to "GPT-4" --relation uses --weight 0.8 --json
```
### Remove a specific relationship between two entities
<!-- skip-test: requires the relationship created by the preceding `link` example. -->
```bash
sqlite-graphrag unlink --from "OpenAI" --to "GPT-4" --relation uses --json
```
### Traverse memories connected via the entity graph
```bash
sqlite-graphrag related onboarding-note --max-hops 2 --limit 10 --json
```
> **Empty results are normal** for memories without graph edges yet — extract entities first via `remember` or `ingest`. Edges form when ≥2 entities co-occur in the same memory body.

### Export a graph snapshot in json, dot or mermaid
<!-- skip-test: `--output graph.json` writes a file relative to the invocation cwd; pollutes the test workspace. The remaining read-only graph subcommands are exercised by the cookbook integration tests. -->
```bash
sqlite-graphrag graph --format json --output graph.json
sqlite-graphrag graph stats --json
sqlite-graphrag graph traverse --from "OpenAI" --depth 2 --json
sqlite-graphrag graph entities --entity-type organization --limit 50 --json
```
### Remove orphan entities with no memories and no relationships
```bash
sqlite-graphrag cleanup-orphans --dry-run --json
sqlite-graphrag cleanup-orphans --yes --json
```
### Bulk-delete relationships by type
<!-- skip-test: requires relationships to exist in the namespace. -->
```bash
sqlite-graphrag prune-relations --relation mentions --dry-run --show-entities --json
sqlite-graphrag prune-relations --relation mentions --yes --json
```
### Clear cached embedding/NER models from the XDG cache
<!-- skip-test: deletes the embedding model cache; safe in production but slows the integration suite by forcing a re-download on later commands. -->
```bash
sqlite-graphrag cache clear-models --yes
```
### List every version of a memory
<!-- skip-test: depends on the lifecycle state established by earlier illustrative blocks (which are themselves marked `skip-test`). -->
```bash
sqlite-graphrag history integration-tests-postgres --no-body --json
```


## Commands
### Core database lifecycle
| Command | Arguments | Description |
| --- | --- | --- |
| `init` | `--namespace <ns>` | Initialize database and apply migrations (no model download, no binary probe) |
| `health` | `--json` | Show database integrity, FTS5 functional check, sqlite version, super-hub detection (degree > 50); v1.1.01 adds `vec_memories_missing`/`vec_entities_missing`/`vec_chunks_missing` and per-table `vec_*_coverage_pct` |
| `stats` | `--json` | Count memories, entities and relationships; the JSON exposes a top-level `total_memories` |
| `migrate` | `--json` | Apply pending schema migrations via `refinery` |
| `vacuum` | `--json` | Checkpoint WAL and reclaim disk space |
| `optimize` | `--json`, `--skip-fts` | Run `PRAGMA optimize` and rebuild FTS5 index (skip with `--skip-fts`) |
| `backup` | `--output <path>` | Back up the database using the SQLite Online Backup API |
| `sync-safe-copy` | `--dest <path>` (alias `--output`) | Checkpoint then copy a sync-safe snapshot |
| `config` | `set`, `get`, `list` (`--effective`), `unset`, `path`, `doctor`, `add-key`, `list-keys`, `remove-key` | XDG operational config and API keys (v1.2.0); precedence flag > XDG `config set` > default; no product env |
### Memory content lifecycle
| Command | Arguments | Description |
| --- | --- | --- |
| `remember` | `<NAME>` positional or `--name` (never both), `--type`, `--description`, `--body` (or `--body-file`/`--body-stdin`), `--entities-file`, `--relationships-file`, `--graph-stdin`, `--graph-file <path>`, `--llm-parallelism <N>` (default 4), `--enable-ner` (URL-regex only since v1.0.79), `--strict-name`, `--strict-entity-types`, `--force-merge`, `--replace-graph`, `--clear-body`, `--dry-run`, `--enqueue-enrich` (v1.2.0 hot-set) | Save a memory with optional entity graph; `--graph-file` loads the graph from a file (combinable with `--body-file`); `--strict-name` rejects non-kebab names instead of normalizing; `--replace-graph` (with `--force-merge`) zeroes existing bindings before writing; `--type`/`--description` optional with `--force-merge` (inherited from existing); `--dry-run` validates without persisting; `--enqueue-enrich` hot-enqueues entity-descriptions and returns `entities_created` / `enrich_recommended` |
| `remember-batch` | `--transaction`, `--force-merge`, `--fail-fast`; NDJSON fields `name`/`type`/`description`/`body` (description **required** on create, v1.2.0 parity with `remember`) | Batch-create memories from NDJSON stdin; one invocation, one slot, one DB connection; empty description on create is rejected |
| `recall` | `<query>`, `-k`/`--k` (alias `--limit`), `--type`, `--max-hops`, `--max-distance`, `--all-namespaces`, `--no-graph` | Search memories semantically via KNN + graph traversal |
| `read` | `[name]` or `--name <name>`, `--id <N>`, `--with-graph`, `--format raw` | Fetch a memory by exact name or integer memory_id; `--with-graph` includes linked entities and relationships; `--format raw` prints the pure body with no JSON envelope |
| `list` | `--type`, `--limit`, `--offset`, `--include-deleted` | Paginate memories sorted by `updated_at`; default limit is all with `--json`, 50 for text; response includes `total_count`, `truncated`, `body_length` |
| `forget` | `[name]` or `--name <name>` | Soft-delete a memory preserving history |
| `rename` | `[old]`, or `--name`/`--old`/`--from <NAME>`, `--new-name`/`--new`/`--to <NAME>` | Rename a memory while keeping versions |
| `edit` | `[name]` or `--name`, `--body`, `--description`, `--type`, `--force-reembed`, `--llm-parallelism <N>` | Edit body, description or memory type creating new version; skips re-embedding when body content is unchanged; `--force-reembed` (v1.0.79) regenerates the embedding without changing the body |
| `history` | `[name]` or `--name <name>`, `--diff` | List all versions of a memory; `--diff` includes character-level change summary |
| `memory-entities` | `[name]` or `--name <name>`, `--entity <name>` | List entities linked to a memory, or memories linked to an entity (reverse lookup via `--entity`) |
| `restore` | `--name`, `--version` | Restore a memory to a previous version |
| `ingest` | `<DIR>`, `--type`, `--pattern <GLOB>` (default `*.md`), `--recursive`, `--mode none` (only accepted value; `claude-code`/`codex`/`opencode` removed, `gliner` removed in v1.0.79), `--ingest-parallelism N`, `--llm-parallelism N` (default 2, embedding workers), `--low-memory`, `--enable-ner` (URL-regex only since v1.0.79), `--force-merge`, `--fail-fast`, `--dry-run`, `--max-cost-usd`, `--enrich-after`, `--name-prefix <PREFIX>` (v1.1.01) | Bulk-ingest every matching file as a separate memory (NDJSON output); `--force-merge` updates duplicate files instead of skipping them (dedup by `body_hash`); oversized bodies are auto-split natively into chunks; extraction is a SEPARATE `enrich --mode openrouter` pass, not an ingest mode; `--dry-run` previews name mapping without writing; `--name-prefix` (v1.1.01) prepends a kebab-case prefix to every derived memory name (80-char name cap enforced) |
| `export` | `--namespace`, `--type`, `--include-deleted`, `--limit`, `--offset` | Export memories as NDJSON for backup or migration. Stream contract (GAP-SG-215, v1.2.8): a record line carries the record and nothing else, the final summary line carries the single agent-surface record for the whole stream and is never reshaped; `--select` and `--truncate-content` act per record, while `--count-only`, `--sort`, `--dedupe-by`, `--max-items`, `--max-output-bytes` and `--filter` are refused with exit 2 before the first line |
| `cache clear-models` / `cache list` / `cache stats` | `--yes` (clear) | Remove or list model files under the XDG cache directory; `cache stats` is a v1.2.0 alias of `list` (exit 0) |

> **Memory name validation.** Names must match `[a-z0-9-]+` (kebab-case, ASCII only).
> Unicode and uppercase are rejected with exit code 1. Names longer than 60 chars
> emitted by `ingest` are truncated to fit; review the WARN log to spot mangled names.
### Retrieval and graph
| Command | Arguments | Description |
| --- | --- | --- |
| `hybrid-search` | `<query>`, `--k`, `--rrf-k`, `--with-graph`, `--max-hops`, `--min-weight`, `--weight-vec`, `--weight-fts` | FTS5 plus vector fused via Reciprocal Rank Fusion; graceful degradation when FTS5 is corrupted (`fts_degraded`, auto-rebuild); `normalized_score` for cross-method comparability |
| `namespace-detect` | `--namespace <name>` | Resolve namespace precedence for invocation |
| `link` | `--from`, `--to`, `--from-id`, `--to-id` (v1.1.05), `--relation`, `--weight`, `--create-missing`, `--entity-type`, `--strict-relations` | Create a relationship; `--from-id`/`--to-id` resolve by entity ID; pure-numeric names are rejected by `validate_entity_name` so `--create-missing` cannot create ghost numeric entities (v1.1.05); `--strict-relations` rejects non-canonical types; warnings in JSON for non-canonical |
| `unlink` | `--from`, `--to`, `--relation`, `--entity`, `--all`, `--memory <name> --entity <name>` | Remove relationships; `--relation` now optional (removes all between pair); `--entity X --all` removes all edges of entity; `--memory <name> --entity <name>` removes a single curated memory-to-entity binding without touching entity-to-entity edges |
| `related` | `--name`, `--limit`, `--hops` | Traverse graph-connected memories from a seed memory |
| `graph` | `--format`, `--output` | Export a graph snapshot in `json`, `dot` or `mermaid` |

> **Breaking change in v1.0.44.** `graph entities` JSON output renamed top-level array
> from `items` to `entities`. Update jaq/jq filters: `.items[]` becomes `.entities[]`.
> The `list` command still uses `items`.

### Graph subcommands
| Subcommand | Description | Key flags |
| --- | --- | --- |
| `graph traverse --from <ENTITY>` | Walk the entity graph from a starting node using BFS; without `--fuzzy`, NotFound suggests ranked canonical names (Jaro-Winkler + prefix); with `--fuzzy`, auto-resolves a clear single winner (v1.1.05) | `--depth` (default 2), `--namespace`, `--fuzzy` (v1.1.05) |
| `graph stats` | Print graph statistics (node count, edge count, degree distribution) | `--namespace` |
| `graph recompute-degree` | Reconcile the cached `entities.degree` with the real edge counts in a single transaction (v1.1.01); envelope `{total, updated, zeroed, unchanged}` | `--dry-run`, `--namespace` |
| `graph entities` | List entities with degree count and sorting | `--limit` (default 50), `--entity-type`, `--namespace`, `--sort-by degree\|name\|created_at`, `--order asc\|desc` |
| `graph entity-types` (v1.2.8) | Audit the entity-type vocabulary the database actually holds — each `type` with its `count` and a `canonical` flag, most frequent first | `--namespace`, `--format json\|text` |

### Maintenance
| Command | Arguments | Description |
| --- | --- | --- |
| `purge` | `--retention-days <n>` (default 90), `--now` (immediate; alias of `--retention-days 0`), `--dry-run`, `--yes` | Permanently delete soft-deleted memories past retention; `--yes` alone does **not** wipe recent soft-deletes — pair with `--now` or `--retention-days 0` |
| `cleanup-orphans` | `--namespace`, `--dry-run`, `--yes` | Remove entities that have no memories and no relationships |
| `prune-relations` | `--relation <type>`, `--namespace`, `--dry-run`, `--yes`, `--show-entities` | Bulk-delete all relationships of a given type; `--show-entities` lists affected entities in the dry-run preview |
| `delete-entity` | `--name <entity>`, `--cascade` | Delete an entity and cascade-remove all its relationships and bindings |
| `rename-entity` | `--name <entity>` or `--id <ID>` (v1.1.01), `--new-name <name>` | Rename an entity preserving all relationships and memory bindings; re-embeds vector |
| `reclassify` | `--name <entity> --new-type <type>`, `--description <text>`, or `--from-type <old> --to-type <new> --batch` | Reclassify entity types individually or in bulk; `--description` updates entity description in single mode |
| `merge-entities` | `--names <a,b,c> --into <target>`, or `--ids <1,2,3> --into-id <ID>` (v1.1.01, namespace-scoped); `--cross-namespace` (v1.1.03) | Merge source entities into target, moving all edges; rejects self-referential merges (`--into-id` in `--ids`, or `--into` in `--names`) BEFORE any DB work (v1.1.05) |
| `split-body` | `--name <N>` or `--batch`, `--threshold` (default 25000), `--json` | Split an oversized memory body into daughter memories `{name}-part-{i}`; marks original `superseded_by_split`; creates `replaces` relations; daughters need follow-up `enrich --operation re-embed --target memories` (v1.1.03) |
| `reclassify-relation` | `--from-relation` / `--to-relation`, or `--literal-from` / `--literal-to`, `--batch`, `--json` | Bulk rename relationship types; `--literal-from`/`--literal-to` match/write verbatim (bypass clap normalisation) for underscore→hyphen migrations (v1.1.01/v1.1.03) |
| `normalize-entities` | `--namespace`, `--dry-run`, `--yes`, `--json` | Normalize entity names to kebab-case and auto-merge near-duplicates |
| `prune-ner` | `--entity <name>` or `--all`, `--dry-run`, `--yes` | Remove NER bindings from memory_entities table |
| `fts rebuild` | `--json` | Rebuild the FTS5 full-text search index from scratch |
| `fts check` | `--json` | Run FTS5 integrity-check without modifying the index |
| `fts stats` | `--json` | Show FTS5 index statistics (row count, shadow pages) |
| `completions` | `bash`, `zsh`, `fish`, `powershell`, `elvish` | Generate shell completions for the specified shell |
| `schema` | (none), `--name <ID>` | Machine-readable catalog of all **76** JSON contracts (v1.2.2). Bare `schema` emits NDJSON, one `{"id","invoke"}` per line, where `invoke` is the ready-to-copy command; `--name <ID>` emits that contract's JSON Schema document. Unknown `<ID>` exits **4**. `$schema` documents are exempt from the agent-native output surface, so any global flag chains safely |
| `enrich` | `--operation <op>` (memory-bindings, entity-descriptions, body-enrich, re-embed, augment-bindings, weight-calibrate, relation-reclassify, entity-connect, entity-type-validate, description-enrich, cross-domain-bridges, domain-classify, graph-audit, deep-research-synth, body-extract), `--target <memories\|entities\|chunks\|all>` (v1.1.01, `re-embed` only; default `memories`), `--mode openrouter` (only accepted value; resolved by default when omitted), `--openrouter-model`, `--openrouter-api-key`, `--openrouter-timeout`, `--openrouter-base-url`, `--until-empty`, `--max-runtime <SECONDS>`, `--max-attempts <N>` (default 8), `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--ignore-backoff`, `--prune-dead-orphans`, `--prune-dead-entity-orphans`, `--force-redescribe`, `--entity-names`, `--memory-names`, `--names <NAMES>`, `--names-file <PATH>`, `--body-extract-graph-only`, `--rest-concurrency <N>`, `--llm-parallelism <N>`, `--preserve-threshold <FLOAT>`, `--preflight-check`, `--rate-limit-buffer <SECONDS>`, `--max-load-check`, `--circuit-breaker-threshold <N>`, `--reset-stale-claims`, `--stale-claim-secs <N>`, `--resume`, `--retry-failed`, `--max-cost-usd <USD>`, `--db <DB>`, `--wait-job-singleton <SECONDS>`, `--force-job-singleton` | LLM-augmented graph quality pipeline; multi-namespace enrich queue (`namespace` + unique key, v1.2.0); **v1.2.1 CAPA:** claim/count/resume isolate by `operation`+`namespace`; `--until-empty` counts only this op+ns; `--force-redescribe` reopens `skipped`/`done` once/process (never `dead`); re-embed uses BLOB `LENGTH(embedding)=dim*4` + zombie reconcile; enqueue strips `entity:` and validates chunk ns; CAPA-D compound configuration-file markers only; REST only via `--mode openrouter` (requires `--openrouter-model`); there is no local-CLI mode; dead-letter + skipped inspectors: `--status`, `--list-dead`/`--requeue-dead`, `--list-skipped`/`--requeue-skipped` (v1.2.0), `--prune-dead-orphans`, `--prune-dead-entity-orphans`; entity-descriptions: `--force-redescribe`, `--entity-names`; memory-scoped ops: `--memory-names` (alias `--names`); `--until-empty` scan→drain; see also `remember --enqueue-enrich` for hot-set enqueue |
| `vec orphan-list` | `--json` | List orphan memory embedding rows (G39) with `vector_hash` for traceability |
| `vec purge-orphan` | `--yes`, `--dry-run`, `--json` | Delete orphan memory embedding rows from `vec_memories`, `vec_entities`, `vec_chunks` (G39); `--yes` required as safety guard |
| `vec stats` | `--json` | Show statistics for `vec_memories`, `vec_entities`, `vec_chunks` tables (G39) |
| `remember-batch` | `--json`, `--transaction`, `--force-merge`, `--fail-fast`; NDJSON `description` required on create (v1.2.0) | Batch-create memories from NDJSON stdin (one invocation, one slot, one DB connection) |
| `namespace-detect` | `--json`, `--namespace <name>` | Resolve namespace precedence for the current invocation |
| `deep-research` | `<query>`, `--k`, `--max-sub-queries`, `--max-hops`, `--min-weight`, `--max-results`, `--with-bodies`, `--max-concurrency`, `--timeout`, `--rrf-k`, `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--output <PATH>` (v1.1.05 atomwrite), `--sub-query-strategy`, `--sub-queries-file` (v1.1.05), `--json` | Parallel multi-hop GraphRAG research via query decomposition; single-token queries expand to multi-aspect sub-queries (`source: "aspect"`, EN/PT facets; v1.1.05); `--output` writes the full envelope atomically and prints a short stdout ack; returns `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context?`, `stats` |

### v1.0.82 / v1.0.85 subcommands (no new subcommands added in v1.0.83/84/85; new fields and flags only)
| Command | Arguments | Description |
| --- | --- | --- |
| `pending-embeddings` | `list`, `process`, `status` (v1.2.0 alias of `embedding status`), `--status pending\|in_progress\|done\|abandoned`, `--limit`, `--json` | Inspect and process the embedding retry queue (GAP-005, ADR-0040); `process` retries failed embeddings with the next backend in `--llm-backend`; `status` is the queue health alias |
| `slots` | `status`, `release --slot-id <N> --yes`, `cleanup`, `--json` | Cross-process LLM slot semaphore inspection and cleanup (GAP-004, ADR-0039); `status` returns `max_concurrency`, `acquired`, `waiting`, `held_by_pid[]`, `p50_wait_ms`, `p99_wait_ms`; `release` reaps one slot by id; `cleanup` reaps stale/orphan slot files |
| `embedding` | `status`, `list`, `abandon`, `--status pending\|in_progress\|done\|abandoned`, `--limit`, `--json` | Health and per-entry inspection of the pending-embeddings queue (GAP-005); `status --json` reports a `coverage` object with the real vector counts per table; v1.1.01 adds per-table `*_missing` counters to `status --json` |

### v1.0.82 / v1.0.85 global flags
| Flag | Applies to | Description |
| --- | --- | --- |
| `--llm-backend <openrouter\|none>` | `remember`, `edit`, `ingest`, `enrich` | Embedding transport: `openrouter` (default) or `none` (skips embedding) |
| `--llm-fallback <chain>` | `remember`, `edit`, `ingest`, `enrich` | Ordered fallback chain when the primary backend fails; defaults to `none` |
| `--llm-max-host-concurrency <N>` | All embedding commands | Cap concurrent LLM calls host-wide via `fs4` flock (ADR-0039); default derived from CPU and available memory |
| `--llm-slot-wait-secs <N>` | All embedding commands | Seconds to wait for a free slot before failing (default 30s); pair with `--llm-slot-no-wait` for fail-fast |
| `--quiet` / `-q` | Top-level global flag (v1.1.05) | Suppress non-error tracing on stderr so stdout JSON stays clean for headless pipelines; pair with `deep-research --output PATH` for large envelopes. NEVER redirect stdout+stderr to the same file with `&>` |

> **Removed in v1.2.8:** the `pending` subcommand family (`list` / `show <id>` / `cleanup`) shipped in v1.0.82 for the three-stage `remember` checkpoint queue (GAP-001, ADR-0036) and no longer exists — the binary answers `unrecognized subcommand`. The embedding retry queue is unaffected and keeps its own surface under `pending-embeddings` and `embedding`.

> **GAP-SG-139 (v1.2.0):** host/XDG leaves accept `--db` as a documented **no-op** so agents that append `--db` to every invocation do not get clap exit 2. Surfaces: `config`, `slots`, `cache`, `completions`. Graph-scoped commands still resolve storage via `--db` / `config set db.path`.

### Other global flags (v1.2.8)

These top-level flags are neither embedding transport nor output shaping, and three of them change what the binary REFUSES. They apply to every subcommand.

| Flag | Description |
| --- | --- |
| `--use-active` | GAP-SG-207: accept the ambient database target for a verb that changes durable state. A mutating subcommand normally has to name its target with `--db` and is refused with **exit 2** otherwise; this is the explicit dispensation, and the envelope records that a human asked for it |
| `--fail-on-degraded` | Fail instead of degrading when the query embedding cannot be produced. Without it, `recall` and `hybrid-search` fall back to FTS5-only ranking, raise `vec_degraded` and still exit `0` — an agent parsing `.results` silently receives a keyword search where it asked for a hybrid one |
| `--wait-lock <SECONDS>` | Wait up to `SECONDS` for a free concurrency slot before giving up (**exit 75**), polling every 500 ms. Default 300s |
| `--llm-model <MODEL>` | v1.0.82 (GAP-003): model to invoke on the chosen backend. Prefer the flag; optional XDG `llm.model` |
| `--skip-embedding-on-failure` | v1.0.82 (GAP-005): persist with a NULL embedding when every backend in the chain fails. The memory stays in `pending_embeddings` for reprocessing. Prefer the flag; optional XDG `llm.skip_embedding_on_failure` |

### v1.2.2 global flags — agent-native output surface (GAP-SG-142)

Ten global flags make up the agent-native surface: the two guards of the `Surface guards` subsection decide when a reshaping request is refused, and the **eight** below reshape the JSON envelope at a single point, so an agent stops carrying a `jaq` filter in its prompt just to read one field. They apply to **every** subcommand and compose in a fixed order: **filter → sort → dedupe → max-items → select → count-only → truncate-content → max-output-bytes**.

| Flag | Alias | Description |
| --- | --- | --- |
| `--select <KEYS>` | `--fields` | Keep only these comma-separated keys in each result element. Accepts dotted paths (`stats.total`). A key missing from an element is skipped, never emitted as `null`, so a projection never invents a field. An envelope with no result array is projected itself |
| `--filter <EXPR>` | — | Keep only result elements satisfying `EXPR`. Grammar: `key=value`, `key!=value`, `key~substring` (case-insensitive containment); `==` is a synonym of `=`. Repeat the flag to conjoin predicates with **AND**. A malformed expression fails fast with **exit 2**, so a typo is never mistaken for an empty result set |
| `--max-items <N>` | — | Emit at most `N` result elements. **Distinct from the per-subcommand `--limit` and from `-k`**, which bound the *query*; this bounds only what reaches stdout, and only *after* filtering |
| `--sort <KEY>` | — | Sort result elements ascending by this key (dotted path). Numbers compare numerically, everything else as text. Elements lacking the key keep their relative order at the end of the list |
| `--dedupe-by <KEY>` | — | Drop later result elements repeating this key's value. Elements lacking the key are always kept, since they were never proven duplicate |
| `--count-only` | — | Replace the payload with `{"count": N}`, where `N` is what survived `--filter`, `--dedupe-by` and `--max-items` |
| `--truncate-content <N>` | — | Shorten every string longer than `N`. Counts **characters, never bytes**, so a UTF-8 sequence is never split |
| `--max-output-bytes <N>` | — | Cap the serialized envelope at `N` bytes by **dropping trailing result elements** until it fits — never by slicing the JSON text, which would not parse |

#### Surface guards (v1.2.6 / v1.2.8)
Two further global flags do not reshape anything; they widen what the reshaping flags are allowed to accept, and without them the refusal is the default.

| Flag | Description |
| --- | --- |
| `--filter-scope <SCOPE>` | GAP-SG-201: declare what `--filter` is allowed to observe. Omitted, a predicate over a page the query already truncated is refused with **exit 2**, because the answer would describe a set the predicate never saw. `page` accepts the narrower reading and records it in `count_scope`; `universe` states the requirement explicitly. A top-k bound is never refused: the k IS the answer |
| `--allow-unknown-keys` | GAP-SG-202: accept a `--select` / `--filter` / `--sort` / `--dedupe-by` key this envelope carries nowhere. Without it such a key is refused with **exit 2**, because an unresolvable key produces an empty answer indistinguishable from missing data — a typo would read as "the memory does not exist" |

#### Contract guarantees
- **Failure envelopes are never filtered.** An envelope carrying `error: true` or `ok: false` reaches the caller verbatim, whatever `--filter` says. `--filter` shapes result rows; it never shapes the error contract
- **JSON Schema documents pass through untouched.** A payload carrying `$schema` is a contract, not a result set
- **Truncation is never silent.** Anything removed is recorded under the `agent_surface` member and raises the top-level `truncated` flag
- **NDJSON streams bypass the surface** — line-oriented emitters keep one record per line, because reshaping them would change the stream contract
- The result array is located by the well-known names `results`, `items`, `entities`, `memories`, `hits`, `rows`, `matches`, `data`, in that order; otherwise the first array member wins

#### The `agent_surface` record
Present whenever a knob is active. Reports `input_count` and `output_count` always, plus `select`, `filters`, `sort`, `dedupe_by`, `max_items` when set, `count_only` under `--count-only`, `content_truncated` + `truncate_content` when a string was shortened, and `output_truncated` + `dropped` + `max_output_bytes` when the byte ceiling fired.

#### Precedence
| Knob | XDG key | Default |
| --- | --- | --- |
| `--max-items` | `agent_surface.max_items` | `0` (no cap) |
| `--truncate-content` | `agent_surface.truncate_content` | `0` (disabled) |
| `--max-output-bytes` | `agent_surface.max_output_bytes` | `0` (no ceiling) |

CLI flag > XDG `config set` > named default, as everywhere else. No product environment variable is read. With no knob set the envelope is byte-for-byte identical to pre-v1.2.2 output.

#### Offline examples

```bash
sqlite-graphrag list --json --count-only
sqlite-graphrag stats --json --select total_memories
sqlite-graphrag graph entities --json --select name,entity_type --max-items 5
sqlite-graphrag health --json --truncate-content 200
sqlite-graphrag schema
sqlite-graphrag schema --name hybrid-search
```

### v1.2.2 global flag — `--no-input`

| Flag | Applies to | Description |
| --- | --- | --- |
| `--no-input` | Top-level global flag | Refuse to read stdin anywhere in this invocation |

The refusal is **declarative, not emergent**. Without the flag, a stdin path only fails once the read is attempted — immediately on a TTY, after the deadline otherwise. With it, `--body-stdin`, `--graph-stdin`, `remember-batch` and every other stdin reader fail up front with **exit 1** (`AppError::Validation`), even when a pipe is attached and would have supplied data. That is the point: unattended automation should fail fast and loudly rather than hang waiting for a human who is not there.

Precedence: flag > XDG `cli.no_input` > `false`. A host that opted in through XDG turns it off by **unsetting the key**, not by `--no-input=false` — that spelling would read as "input is allowed here" while the surrounding automation assumes otherwise.

### v1.0.82 / v1.0.85 exit codes
| Code | Meaning | Emitted by |
| --- | --- | --- |
| `19` | Shutdown signal received; partial work discarded; see `shutdown-envelope.schema.json` for stdout envelope | Any LLM-spawning command on SIGTERM/SIGINT/SIGHUP (ADR-0037) |

### `cache` subcommands
| Subcommand | Description |
| --- | --- |
| `list` | List cached model files with sizes and total disk usage |
| `stats` | Alias of `list` (v1.2.0 — agents often call `cache stats`) |
| `clear-models` | Remove cached embedding/NER model files (forces re-download on next `init`) |


## Numeric Argument Ranges (v1.2.7)

Thirteen numeric arguments of the read surface are range-validated by clap at parse time. A value outside the range is refused with **exit 2** and a range message, before the database is touched.

| Range | Arguments |
| --- | --- |
| `1..=4096` (top-k) | `recall -k`, `hybrid-search -k`, `related --limit`, `graph entities --limit`, `deep-research --k`, `deep-research --max-results` |
| `1..=1000000` (listing limit) | `export --limit`, `pending-embeddings --limit`, `embedding --limit` |
| `1..=64` (hops) | `related --max-hops` (alias `--hops`), `recall --max-hops`, `graph traverse --depth`, `deep-research --max-hops` |
| `1..=64` (sub-queries) | `deep-research --max-sub-queries` |

The ceilings live in `src/constants/search.rs` as `K_QUERY_RANGE_MAX`, `K_LIST_LIMIT_MAX`, `K_MAX_HOPS_CEILING` and `K_MAX_SUB_QUERIES_CEILING`.


## Configuration (XDG — v1.2.5)
### Precedence and storage (no product env)
- Runtime knobs resolve as **CLI flag > XDG `config set` > named default**
- **FORBIDDEN product env:** `SQLITE_GRAPHRAG_*` (and other product knobs formerly documented as env) are **not** read at runtime — flag > XDG `config set` > default only (G-T-XDG-04). Do not export product tables for configuration
- Persist settings with `sqlite-graphrag config set <KEY> <VALUE>`; inspect with `config get`, `config list`, `config list --effective`, `config unset`
- Secrets: `config add-key` (stdin) or per-invocation `--openrouter-api-key`; prefer XDG key store over shell history
- Database path: pass `--db <PATH>` after the subcommand, or persist via `config set db.path <path>`; with neither, the default is the XDG data directory `~/.local/share/sqlite-graphrag/graphrag.sqlite`. Product env `SQLITE_GRAPHRAG_DB_PATH` is **forbidden** / ignored at runtime
- OS env still allowed for locale (`LANG`/`LC_*`), `PATH`, `HOME`/`USERPROFILE`, XDG base dirs and `NO_COLOR`. There is no subprocess, so no credential is ever forwarded to a child: the OpenRouter key is read from the XDG store or `--openrouter-api-key` and never leaves the process
- Remote OTEL / product telemetry is forbidden; local tracing only (`-v` / `-q` / XDG `log.level`)

### Complete `config set` key reference (70 keys, v1.2.8)
Every key below is accepted by `config set` and resolved as **CLI flag > XDG `config set` > default**. `sqlite-graphrag config list --effective --json` prints the same inventory at runtime; this table is asserted against `src/config/registry.rs` by `tests/docs_xdg_coverage.rs`, so it cannot silently drift.

`(none)` means the key has no built-in default: when it is unset, the subsystem falls back to its own runtime heuristic (auto-sizing, host detection or a required CLI flag).

A key outside this list is rejected with exit 1. Up to v1.2.4 this section cited `enrich.preserve_threshold`, `enrich.entity_connect.max_runtime_secs` and `llm.concurrency`, which never existed in the registry.

#### Agent-native output surface
| Key | Default | Purpose |
| --- | --- | --- |
| `agent_surface.max_items` | `0` | Standing ceiling for `--max-items`. `0` disables. Since v1.2.5 (GAP-SG-191) it caps every array in the envelope, not just the primary one |
| `agent_surface.max_output_bytes` | `0` | Standing ceiling for `--max-output-bytes`. `0` disables. Output stays parseable JSON; the stub reports the requested ceiling |
| `agent_surface.truncate_content` | `0` | Standing ceiling for `--truncate-content` (per-field character cap). `0` disables |

#### Database and storage
| Key | Default | Purpose |
| --- | --- | --- |
| `db.path` | `(none)` | Default database file. Overridden by `--db <PATH>` after the subcommand. Without either, the XDG data directory `~/.local/share/sqlite-graphrag/graphrag.sqlite` |
| `db.busy_retries` | `5` | Retries on `SQLITE_BUSY` before exit 15 |
| `db.busy_base_delay_ms` | `300` | Base delay of the exponential backoff between busy retries |
| `db.query_timeout_ms` | `5000` | Per-query wall-clock ceiling |
| `cache.dir` | `(none)` | Cache root. Defaults to the XDG cache directory |

#### Embedding
| Key | Default | Purpose |
| --- | --- | --- |
| `embedding.dim` | `1024` | Vector dimensionality. Changing it on a populated database silently breaks cosine similarity — migrate deliberately, never as a flag side effect |
| `embedding.model` | `(none)` | Default embedding model. Read since v1.2.5 (GAP-SG-192); before that the key was documented but ignored |
| `embedding.backend` | `(none)` | Default embedding backend (`auto` or `openrouter`). Registered in v1.2.5 (GAP-SG-198); `--embedding-backend --help` had promised it since v1.0.93 while `config set` answered exit 1 |
| `llm.backend` | `(none)` | Default LLM backend for embedding (`open-router` or `none`). Registered in v1.2.5 (GAP-SG-198), same defect as `embedding.backend` |
| `embedding.batch_size` | `32` | Passages per REST embedding request |
| `embedding.timeout_secs` | `300` | Per-request embedding timeout |
| `embedding.entity_cache_max_entries` | `10000` | Entity-embedding LRU capacity |
| `embedding.entity_cache_ttl_secs` | `3600` | Entity-embedding cache entry lifetime |

#### LLM transport and host slots
| Key | Default | Purpose |
| --- | --- | --- |
| `llm.model` | `(none)` | Default text model for graph extraction |
| `llm.fallback` | `none` | Backend fallback chain. Only `openrouter` and `none` are valid since v1.2.0 |
| `llm.openrouter_timeout_secs` | `600` | Per-request OpenRouter chat timeout |
| `llm.probe_timeout_ms` | `800` | Credential and backend probe timeout |
| `llm.max_host_concurrency` | `(none)` | Host-wide ceiling on concurrent LLM work. Auto-sized when unset |
| `llm.slot_wait_secs` | `300` | How long to wait for a host slot before giving up |
| `llm.slot_no_wait` | `false` | Fail immediately instead of queueing for a slot |
| `llm.worker_rss_mb` | `350` | Assumed RSS per worker, used to size concurrency against free memory |
| `llm.skip_embedding_on_failure` | `false` | Persist the row without a vector when embedding fails, instead of failing the write |

#### Enrichment
| Key | Default | Purpose |
| --- | --- | --- |
| `enrich.scan_page_size` | `512` | Keyset page width of the streaming scanners (GAP-SG-185, range 1..=4096) |
| `enrich.yield_every_n_items` | `10` | Cooperative yield interval during long drains |
| `enrich.reembed_claim_batch` | `32` | Rows claimed per `re-embed` transaction |
| `enrich.rate_limit_deadline_secs` | `3600` | Wall-clock ceiling while backing off a rate limit |
| `enrich.circuit_breaker_reset_secs` | `60` | Cooldown before the breaker closes again |
| `enrich.entity_connect.default_limit` | `100` | Candidate pairs per `entity-connect` scan |
| `enrich.entity_connect.large_ns_limit` | `25` | Lower ceiling applied to large namespaces |
| `enrich.entity_description.domain` | `auto` | Domain hint for generated entity descriptions |
| `enrich.entity_description.grounding_threshold` | `0.30` | Minimum grounding score for a description to be kept |
| `enrich.entity_description.corpus_top_k` | `8` | Memories sampled as evidence per entity |
| `enrich.entity_description.min_corpus_chars` | `40` | Minimum evidence length before the LLM is called; below it the entity is skipped, never described |
| `enrich.entity_description.neighbour_top_k` | `12` | Typed graph relations sampled as evidence per entity |
| `enrich.entity_description.snippet_chars` | `2000` | Characters per evidence snippet |
| `enrich.entity_description.quality_sample` | `50` | Sample size behind `quality_pct` in `enrich --status` |
| `enrich.entity_type.allowed_types` | `(none)` | Comma-separated entity type vocabulary `entity-type-validate` accepts. Unset means the canonical set. Overridden by `--allowed-types` |
| `enrich.entity_type.on_unknown_type` | `keep` | What `entity-type-validate` does with a label outside that vocabulary: `keep` stores it as written (v1.2.8 behaviour), `fallback` stores the nearest accepted label and preserves the raw one in the description, `strict` refuses with exit 1. Overridden by `--on-unknown-type` |
| `enrich.entity_type_validate.corpus_top_k` | `8` | Linked memory bodies shown to `entity-type-validate` as evidence |
| `enrich.entity_type_validate.min_corpus_chars` | `40` | Below this, the entity has no evidence and the operation abstains without spending a token |
| `enrich.entity_type_validate.neighbour_top_k` | `12` | Typed graph relations shown to `entity-type-validate` as evidence |
| `enrich.entity_type_validate.snippet_chars` | `2000` | Characters per evidence snippet for `entity-type-validate` |

#### Search
| Key | Default | Purpose |
| --- | --- | --- |
| `search.hybrid.max_graph_results` | `50` | Graph-match ceiling for `hybrid-search --with-graph`. `0` removes the cap |

#### Ingest and write limits
| Key | Default | Purpose |
| --- | --- | --- |
| `ingest.low_memory` | `false` | Trade throughput for a smaller resident set during ingest |
| `limits.max_entities_per_memory` | `50` | Entities accepted per write |
| `limits.max_relations_per_memory` | `50` | Relationships accepted per write |

#### Network
| Key | Default | Purpose |
| --- | --- | --- |
| `network.openrouter.chat_url` | `https://openrouter.ai/api/v1/chat/completions` | OpenRouter chat completions endpoint |
| `network.openrouter.embeddings_url` | `https://openrouter.ai/api/v1/embeddings` | OpenRouter embeddings endpoint |
| `network.chat_url` | `(none)` | Alias of `network.openrouter.chat_url` |
| `network.embed_url` | `(none)` | Alias of `network.openrouter.embeddings_url` |

#### Concurrency and process control
| Key | Default | Purpose |
| --- | --- | --- |
| `parallelism.max_total_workers` | `64` | Absolute ceiling on worker tasks |
| `parallelism.rayon_threads` | `(none)` | Rayon pool size. Auto-sized when unset |
| `parallelism.embed_runtime_threads` | `(none)` | Tokio worker threads for the embedding runtime. Auto-sized when unset |
| `system.max_load_per_ncpu` | `2.0` | Load-average ceiling per CPU before new work is throttled |
| `cli.max_instances` | `(none)` | Concurrent process ceiling for this CLI. Auto-sized when unset |
| `retry.disable` | `false` | Disable the built-in retry policy |
| `shutdown.ignore` | `false` | Ignore the graceful-shutdown signal path |

#### CLI behaviour, logging and locale
| Key | Default | Purpose |
| --- | --- | --- |
| `cli.no_input` | `false` | Standing `--no-input`: stdin readers refuse up front with **exit 1** (`AppError::Validation`) even when a pipe is attached |
| `cli.stdin_timeout_secs` | `60` | How long a stdin reader waits for input |
| `namespace.default` | `global` | Namespace used when `--namespace` is absent |
| `display.tz` | `UTC` | IANA zone for `*_iso` JSON fields |
| `i18n.lang` | `en` | UI language on stderr. JSON payloads stay in English |
| `log.level` | `warn` | Local tracing level on stderr |
| `log.format` | `pretty` | `pretty` or `json` |
| `log.to_file` | `false` | Mirror local tracing to a file |
| `log.rotation` | `daily` | Rotation policy when `log.to_file` is on |
| `log.retention_days` | `7` | How long rotated logs are kept |

### Config commands
| Command | Description |
| --- | --- |
| `config set <KEY> <VALUE>` | Persist operational setting in XDG config |
| `config get <KEY>` | Read one setting |
| `config list` | List stored settings (no secrets) |
| `config list --effective` | Include well-known defaults even when not stored |
| `config unset <KEY>` | Remove a stored setting |
| `config doctor` | Diagnose key resolution layers (flag / XDG) |
| `config path` | Print resolved XDG config file path |
| `config add-key` / `list-keys` / `remove-key` | Manage API keys (masked fingerprints) |

### Operator recipes
```bash
sqlite-graphrag config set network.openrouter.embeddings_url "https://openrouter.ai/api/v1/embeddings"
sqlite-graphrag config set search.hybrid.max_graph_results 50
sqlite-graphrag config list --effective --json
sqlite-graphrag config doctor --json
# Immediate hard-delete of soft-deleted rows (default purge keeps 90-day retention)
sqlite-graphrag purge --now --yes --json
# UX aliases (v1.2.0)
sqlite-graphrag pending-embeddings status --json
sqlite-graphrag cache stats --json
```


## Integration Patterns
### Compose with Unix pipelines and tools
```bash
sqlite-graphrag recall "auth tests" --k 5 --json | jaq -r '.results[].name'
```
### Feed hybrid search into a summarizer endpoint
```bash
sqlite-graphrag hybrid-search "postgres migration" --k 10 --json \
  | jaq -c '.results[] | {name, combined_score}' \
  | xh POST http://localhost:8080/summarize
```
### Backup with atomic snapshot and compression
```bash
sqlite-graphrag sync-safe-copy --dest /tmp/ng.sqlite
ouch compress /tmp/ng.sqlite /tmp/ng-$(date +%Y%m%d).tar.zst
```
### Claude Code subprocess example in Node
```javascript
const { spawn } = require('child_process');
const proc = spawn('sqlite-graphrag', ['recall', query, '--k', '5', '--json']);
```
### Docker Debian build for CI pipelines
```dockerfile
FROM rust:1.88-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo install --path .
```


## Exit Codes
### Deterministic status codes for orchestration
| Code | Meaning | Possible Cause |
| --- | --- | --- |
| `0` | Success | Command completed and JSON payload printed when requested |
| `1` | Validation error or runtime failure | Invalid `--type`, malformed `--relation` (empty or non-snake_case), kebab-case violation, generic anyhow error |
| `2` | CLI usage error | Invalid flag, missing required argument, invalid `--tz` timezone (Clap `FromStr` rejects before app code) |
| `9` | Duplicate detected | Existing `--name` without `--force-merge`; `ingest` skips the file and emits `status: "skipped"` with `action: "duplicate"` instead |
| `3` | Conflict during optimistic update | `edit` or `restore` raced against another writer |
| `4` | Memory or entity not found | `read`, `forget`, `edit`, `rename`, `restore` or `graph traverse` target missing |
| `5` | Namespace could not be resolved | No `--namespace` flag, no XDG `namespace.default`, no detected default |
| `6` | Payload exceeded configured limits | `--name` longer than 80 bytes, body over `512000` bytes, more than `512` chunks |
| `10` | SQLite database error | Corrupted file, schema mismatch, missing migration |
| `11` | Embedding generation failed | LLM subprocess error or model load failure |
| `12` | `sqlite-vec` extension failed to load | Missing native extension or unsupported SQLite build |
| `13` | Batch partial failure | `import`, `reindex` or stdin batch with at least one failing record |
| `14` | Filesystem I/O error | Cache or database directory not writable, nonexistent `ingest` target directory |
| `15` | Database busy after retries | WAL contention exceeded `with_busy_retry` budget |
| `20` | Internal or JSON serialization error | Unexpected serde failure or invariant violation |
| `75` | `EX_TEMPFAIL` lock timeout or all concurrency slots busy | Five-plus concurrent invocations or `flock` waited longer than 300s |
| `77` | Available RAM below minimum required | Less than 2 GB free RAM detected before model load |
| `78` | OpenRouter configuration error | `--embedding-backend openrouter` without `--embedding-model`, or invalid/missing OpenRouter key in XDG (`config add-key`; OPENROUTER_API_KEY is not read at runtime) |


## Performance
### Measured on a 1000-memory database
- Embedding latency is dominated by the headless LLM round-trip (~1-3 s per batched call); pure reads (`read`, `list`, `graph`) stay in the low milliseconds
- Since v1.0.79 LLM calls are BATCHED (calibration bases of 8 chunks / 25 entity names at dim 64, dim-adaptive — G44) and PARALLEL (`--llm-parallelism`, bounded `Semaphore` + `JoinSet`), so a 39-item memory embeds in 4-5 calls instead of 39 serialized spawns
- `--embedding-dim 1024` (the default since v1.2.0; was 384 from v1.0.94–v1.1.x) matches modern OpenRouter MRL models; under OpenRouter REST the MRL truncation is server-side at no token cost
- `init` performs no model download — it only creates the database and applies migrations
- **Build:** each embedding call is one OpenRouter REST request — RSS is ~350 MB per worker slot (the 1100 MB ONNX model load no longer exists in any build)


## Memory Requirements
### Sizing RAM for ingest and recall workloads
- The CLI itself is lightweight (~19 MiB binary); RAM is dominated by the LLM subprocesses at roughly 350 MB RSS per worker (`LLM_WORKER_RSS_MB`)
- Worker budget: effective parallelism is `min(--llm-parallelism, cpus, free_ram × 0.5 / 350 MB, 32)` — the concurrency gate adapts to available memory automatically
- Default parallelism increases RSS roughly linearly per worker (`--llm-parallelism 4` ≈ 4 × 350 MB of subprocess RSS on top of the CLI)
- Low-memory mode: pass `--low-memory` to force single-threaded ingest. Equivalent to `--ingest-parallelism 1` and overrides any explicit value, at the cost of 3-4x wall time. Product env is not read at runtime (v1.2.0).
- Container/cgroup users: budget `MemoryMax` for the CLI plus N × 350 MB LLM workers (the old 3 GB ONNX floor no longer exists)


## Storage Footprint
### Expected DB size relative to ingested content
> **Expected overhead: roughly 8× the total ingested body size** (e.g., 7.6 MB of text → ~62.9 MB DB).
> Overhead comes from float embeddings (**default 1024-dim since v1.2.0**; pre-existing databases keep their recorded dimensionality, e.g. 64/384), FTS5 full-text index, and the entities/relationships graph.
> Run `sqlite-graphrag vacuum --json` after bulk `forget`+`purge` cycles to reclaim reclaimed space.


## Safe Parallel Invocation
### Counting semaphore with up to four simultaneous slots
- Each LLM embedding worker consumes roughly 350 MB of RSS — the budget unit used by the concurrency gate since v1.0.79
- `MAX_CONCURRENT_CLI_INSTANCES` remains the hard ceiling at 4 cooperating subprocesses
- Heavy commands `init`, `remember`, `recall`, and `hybrid-search` are clamped lower dynamically when available RAM cannot sustain the requested parallelism safely
- Lock files live at `~/.cache/sqlite-graphrag/cli-slot-{1..4}.lock` using `flock`
- A fifth concurrent invocation waits up to 300 seconds then exits with code 75
- Use `--max-concurrency N` to request the slot limit for the current invocation; heavy commands may still be reduced automatically
- Memory guard aborts with exit 77 when less than 2 GB of RAM is available
- SIGINT and SIGTERM trigger graceful shutdown via `shutdown_requested()` atomic
- Exit code 130 when interrupted by SIGINT (Ctrl+C)
- Exit code 141 when SIGPIPE fires (stdout closed by downstream consumer in pipeline)
- Exit code 143 when terminated by SIGTERM
- Second signal forces immediate exit without waiting for current operation


## Troubleshooting FAQ
### Cloud sync safety (Dropbox, iCloud, OneDrive)
- sqlite-graphrag uses WAL mode by default for high-concurrency writes
- Since v1.0.54, every write command runs `PRAGMA wal_checkpoint(TRUNCATE)` after committing (v1.0.53 covered 11 of 12; v1.0.54 added the missing `prune-relations`)
- This ensures the `.sqlite` file is always self-contained when cloud sync tools read it
- If corruption occurs despite the checkpoint, recover with `sqlite3 broken.sqlite ".recover" | sqlite3 repaired.sqlite`

### Common issues and fixes
- Default behavior creates or opens `graphrag.sqlite` in the XDG data directory, not in the current working directory — pass `--db <PATH>` after the subcommand when the database must live beside the project
- Database locked after crash requires `sqlite-graphrag vacuum` to checkpoint the WAL
- `init` is near-instant since v1.0.76 — there is no model download; if it fails, check the database path and permissions
- Embedding calls failing with exit 11 usually mean the LLM CLI is missing, unauthenticated (OAuth required) or timing out — raise the embed timeout via CLI flag or `config set` (not product env; v1.2.0)
- `ORT_DYLIB_PATH`/`libonnxruntime.so` guidance is HISTORICAL (≤ v1.0.75) — no build loads ONNX since v1.0.76
- Permission denied on Linux means the cache directory lacks write access for your user
- Namespace detection falls back to `global` when no explicit override is present
- Parallel invocations that exceed the effective safe limit receive exit 75 and SHOULD retry with backoff; during audits start heavy commands with `--max-concurrency 1`


## Compatible Rust Crates
### Invoke sqlite-graphrag from any Rust AI framework via subprocess
- Each crate calls the binary through `std::process::Command` with `--json` flag
- No shared memory or FFI required: the contract is pure stdout JSON
- Pin the binary version in your `Cargo.toml` workspace for reproducible builds
- All 18 crates below work identically on Linux, Apple Silicon macOS and Windows

### rig-core
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "project goals", "--k", "5", "--json"])
    .output().unwrap();
```

### swarms-rs
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["hybrid-search", "agent memory", "--k", "10", "--json"])
    .output().unwrap();
```

### autoagents
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["remember", "--name", "task-context", "--type", "project",
           "--description", "current sprint goal", "--body", "finish auth module"])
    .output().unwrap();
```

### graphbit
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "decision log", "--k", "3", "--json"])
    .output().unwrap();
```

### agentai
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["hybrid-search", "previous decisions", "--k", "5", "--json"])
    .output().unwrap();
```

### llm-agent-runtime
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "user preferences", "--k", "5", "--json"])
    .output().unwrap();
```

### anda
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["stats", "--json"])
    .output().unwrap();
```

### adk-rust
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "tool outputs", "--k", "5", "--json"])
    .output().unwrap();
```

### rs-graph-llm
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["hybrid-search", "graph relations", "--k", "10", "--json"])
    .output().unwrap();
```

### genai
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "model context", "--k", "5", "--json"])
    .output().unwrap();
```

### liter-llm
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["remember", "--name", "session-notes", "--type", "user",
           "--description", "session recap", "--body", "discussed architecture"])
    .output().unwrap();
```

### llm-cascade
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "fallback context", "--k", "3", "--json"])
    .output().unwrap();
```

### async-openai
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "system prompt history", "--k", "5", "--json"])
    .output().unwrap();
```

### async-llm
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["hybrid-search", "chat context", "--k", "5", "--json"])
    .output().unwrap();
```

### anthropic-sdk
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "tool use patterns", "--k", "5", "--json"])
    .output().unwrap();
```

### ollama-rs
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "local model outputs", "--k", "5", "--json"])
    .output().unwrap();
```

### mistral-rs
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["hybrid-search", "inference context", "--k", "10", "--json"])
    .output().unwrap();
```

### llama-cpp-rs
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "llama session context", "--k", "5", "--json"])
    .output().unwrap();
```


## Contributing
### Pull requests are welcome
- Read the contribution guidelines in [CONTRIBUTING.md](CONTRIBUTING.md)
- Open issues at the GitHub repository for bugs or feature requests
- Follow the code of conduct described in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)


## Security
### Responsible disclosure policy
- Security reports follow the policy described in [SECURITY.md](SECURITY.md)
- Contact the maintainer privately before disclosing vulnerabilities publicly


## JSON Schemas
### Canonical contracts for every subcommand response
- Authoritative JSON Schemas for every `--json` response live under [`docs/schemas/`](docs/schemas/) and are versioned alongside the crate
- 76 schemas cover `init`, `remember`, `remember-batch` (+ summary), `recall`, `hybrid-search`, `deep-research`, `list`, `read`, `forget`, `purge`, `rename`, `edit`, `history`, `restore`, `link`, `unlink`, `prune-relations`, `health`, `stats`, `migrate` (+ `migrate-rehash` + `migrate-to-llm-only`), `vacuum`, `optimize`, `cleanup-orphans`, `sync-safe-copy`, `backup`, `graph` (+ stats/traverse/entities), `related`, `namespace-detect`, `debug-schema`, `entities-input`, `relationships-input`, `graph-input`, `remember-dry-run`, `ingest-file-event` (+ `ingest-summary`), `ingest-claude-phase` (+ file-event + summary), `export-memory-line` (+ summary), `enrich-phase` (+ item-event + summary), `fts rebuild` (+ `fts check` + `fts stats`), `vec orphan-list` (+ `vec purge-orphan` + `vec stats`), `error-envelope`
- Treat these schemas as the agent contract; SKILL.md documents the same shapes in human-readable form
- Validate downstream consumers with any standard JSON Schema validator (e.g. `ajv`, `jsonschema`)


## Changelog
### Release history tracked separately
- Read the full release history in [CHANGELOG.md](CHANGELOG.md)


## Acknowledgments
### Built on top of excellent open source
- `fastembed` and `sqlite-vec` powered the local embedding pipeline up to v1.0.75 (removed since — embeddings now come from the OpenRouter REST API)
- `refinery` runs schema migrations with transactional safety guarantees
- `clap` powers the CLI argument parsing with derive macros
- `rusqlite` wraps SQLite with safe Rust bindings and bundled build


## License
### Dual license MIT OR Apache-2.0
- Licensed under either of Apache License 2.0 or MIT License at your option
- See `LICENSE-APACHE` and `LICENSE-MIT` in the repository root for full text
