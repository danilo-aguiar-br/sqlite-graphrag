# sqlite-graphrag

[![Crates.io](https://img.shields.io/crates/v/sqlite-graphrag.svg)](https://crates.io/crates/sqlite-graphrag)
[![Docs.rs](https://docs.rs/sqlite-graphrag/badge.svg)](https://docs.rs/sqlite-graphrag)
[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](LICENSE)
[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)

> Persistent memory for AI agents in a single Rust binary with built-in GraphRAG.
> **Current release: v1.1.8 — E2E seal (help scrub / no product env, OpenRouter URLs via XDG, Auto query fail-fast, EntityType fold, remember-batch description parity, pending-embeddings status, cache stats, purge --now, config list --effective) plus enrich quality/latency/contract. No main-DB schema migration (stays at v16). Manual releases only (no GitHub Actions). crates.io owner `danilo-aguiar-br`.**

- Read this document in [Portuguese (pt-BR)](README.pt-BR.md).

- Portuguese version available at [README.pt-BR.md](README.pt-BR.md)
- Public package and repository are live on GitHub and crates.io
- Install the latest published release with `cargo install sqlite-graphrag --locked`
- Upgrade an existing install with `cargo install sqlite-graphrag --locked --force`
- Verify the active binary with `sqlite-graphrag --version`
- See [CHANGELOG.md](CHANGELOG.md) for the full release history
- Release-grade validation includes the `slow-tests` contract suites documented in `docs/TESTING.md`
- Build directly from the local checkout with `cargo install --path .`
- **Upgrading to v1.1.8?** No database migration required if you are already on schema **v16** (from v1.1.04+) — just `cargo install sqlite-graphrag --locked --force` (or `cargo install --path . --locked --force` from this checkout). Crate pin `=1.1.8`. Schema stays at v16 (no main-DB migration). E2E seal: help scrub (no product env, no Box about on ingest/enrich); OpenRouter URLs from XDG `network.openrouter.chat_url` / `network.openrouter.embeddings_url` (aliases `network.chat_url`, `network.embed_url`); query embed fail-fast (`llm.query_embed_timeout_secs` default 3s + credential probe); EntityType fold `map_to_canonical` (`module` → Concept); `remember-batch` requires non-empty `description` on create (parity with `remember`); `pending-embeddings status` (alias of `embedding status`); `cache stats` (alias of `cache list`); `purge --now` (immediate purge of soft-deletes; `--yes` alone does **not** wipe recent soft-deletes; default retention 90d); `config set|get|list|unset` with `config list --effective`; `related_to` → `related` canonical; telemetry lib alias removed; offline harness [`scripts/e2e_offline_v118.sh`](scripts/e2e_offline_v118.sh) **16/16**. Residual honest: monólitos >800 LOC partial; live LQ backfill is an operator campaign. Full notes: [CHANGELOG.md](CHANGELOG.md) `[1.1.8]` and [docs/MIGRATION.md](docs/MIGRATION.md).

- **Upgrading from v1.0.74 / v1.0.75?** See [docs/MIGRATION.md](docs/MIGRATION.md) for the v1.0.76 / v1.0.77 / v1.0.78 / v1.0.79 migration procedure
- **Upgrading from v1.0.79 to v1.0.80?** No database migration required; just `cargo install sqlite-graphrag --locked --force`. v1.0.80 adds the CI `semver-checks` job (informational), the Windows pre-warm steps (ADR-0033), and the panic-free third-signal exit (ADR-0034). Library consumers must pin to `=1.0.80`; see the `Stability Policy` below.
- **Upgrading from v1.0.80 / v1.0.81 to v1.0.82?** Two new migrations run automatically on first `init`/`migrate`: `V014__pending_memories` (pending `remember` checkpoint queue) and `V015__pending_embeddings` (pending embedding retry queue). After upgrading, run `codex login` once to refresh the OAuth refresh token — the 2026-06-14 incident showed that `codex exec` returning HTTP 401 `refresh_token_reused` is now caught by the new fallback chain (ADR-0040) and routed to the next backend in `--llm-backend codex,claude`. See [docs/MIGRATION.md](docs/MIGRATION.md) for the full 6-step procedure including rollback.
- **Upgrading from v1.0.82 / v1.0.83 to v1.0.85?** No database migration required; just `cargo install sqlite-graphrag --locked --force`. v1.0.84 (ADR-0042, GAP-002) added the real Claude backend split via `LlmEmbeddingBuilder` so `--llm-backend claude` invokes `claude` and never `codex`, the `backend_invoked` field in 7 JSON envelopes, the `vec_degraded_reason` field in `hybrid-search` and `recall`, the global `--dry-run-backend` flag for CI pre-flight, and `apply_env_whitelist_for_claude` for hardened providers. v1.0.85 (ADR-0043) extended `FallbackReason` from 3 to 7 variants with a `reason_code` discriminator (catches quota exhaustion, slot exhaustion, backend mismatch, dim zero, cancellation, timeout), `try_embed_query_with_deterministic_fallback` retries the alternative backend on `OAuthQuota` and sleeps 750ms on `SlotExhausted`, and `LlmEmbedding::invoke_claude` now captures 12-14 `anthropic-ratelimit-*-remaining` headers BEFORE checking the subprocess exit (G45-CR5). v1.0.85.1 (hotfix) restored the FTS5 failsafe for --llm-backend none (GAP-004, ADR-0043 hotfix). v1.0.85.2 (hotfix) made --dry-run-backend work standalone (BUG-001, ADR-0044), propagated resolved_kind from embed_via_backend so backend_invoked is populated in all 7 envelopes (BUG-002), and aligned the test mock JSON shape (BUG-003). Library consumers must pin to `=1.0.85.2`; see the `Stability Policy` below.
- **Upgrading from v1.0.91 / v1.0.92 to v1.0.94?** No database migration required; just `cargo install sqlite-graphrag --locked --force`. v1.0.94 adds the OpenRouter embedding backend (`--embedding-backend openrouter`), propagates `EmbeddingBackendChoice` to all 13 embedding paths (GAP-OR-PROPAGATION), fixes exit code 78 for OpenRouter config errors (BUG-OR-EXIT-CODE), and validates 10 embedding models E2E. Library consumers must pin to `=1.0.94`.
- **Upgrading to v1.1.06?** No database migration required; the schema stays at v16 from v1.1.04 — just `cargo install sqlite-graphrag --locked --force` (or `cargo install --path .` from this checkout). Closes GAP-ENTITY-CONNECT-SCAN-CARTESIAN: pair candidates come from co-occurrence in `memory_entities` plus hub×island fill (never full `entities × entities` ORDER BY); queue keys are `pair:{id1}:{id2}` with `item_type=entity_pair`; drain resolves by primary key; `--max-runtime` / soft 120s covers the **first** scan via `InterruptHandle` (Timeout exit 1); NDJSON emits `scan_start` (with `operation`, `entities_in_namespace`, `backlog_degree0_proxy`) before SQL and `scan_meta` with `pairs_enqueued_this_scan`. Suite: `tests/v1106_entity_connect_scan_regression.rs`. ADR-0066. Crate pin `=1.1.6`.
- **Upgrading to v1.1.05?** No database migration required; the schema stays at v16 from v1.1.04 — just `cargo install sqlite-graphrag --locked --force`. Closes the five operator-blocking bugs from the 2026-07-08 deep-research incident report (see `gaps.md`): (1) `deep-research` single-token queries expand to multi-aspect sub-queries (`source: "aspect"`, EN/PT facets; manual via `--sub-query-strategy manual --sub-queries-file`); (2) `deep-research --output PATH` atomic write (tempfile same dir → fsync → rename) with short stdout ack `{written, bytes, blake3, ...}` plus global `--quiet`/`-q` (never mix stderr into JSON with `&>`); (3) `graph traverse --fuzzy` auto-resolves a clear short-name winner, and without `--fuzzy` NotFound suggests ranked canonical names (rapidfuzz Jaro-Winkler + prefix); (4) `merge-entities` rejects self-referential merges (`--into-id` in `--ids`, or `--into` in `--names`) BEFORE any DB work; (5) `link --from-id`/`--to-id` resolve by ID, and pure digit names are rejected by `validate_entity_name` so `--create-missing` cannot create ghost numeric entities. Integration suite: `tests/v1105_danilo_bugs_regression.rs`. The official release name is v1.1.05; the crate manifest carries `version = "1.1.5"`. Library consumers must pin to `=1.1.5`.
- **Upgrading to v1.1.04?** Database migration REQUIRED — `migrate --json` applies V016 (`entity_connect_seen` table). Just `cargo install sqlite-graphrag --locked --force`. Closes the two structural gaps tracked in `gaps.md`: (1) GAP-001 — `deep-research` no longer panics with "Cannot start a runtime from within a runtime"; the sync entry point now computes per-sub-query embeddings BEFORE building its dedicated Tokio runtime (`compute_sub_embeddings`), and the three OpenRouter embedding paths in `embedder.rs` adopt the canonical `Handle::try_current` + `block_in_place` reentry pattern; `ingest_opencode` is also guarded. (2) GAP-002 — `entity-connect` now converges: the new `entity_connect_seen` table (V016) records the LLM verdict per pair, the scanner excludes evaluated pairs, `count_operation_backlog` reports a real O(n) backlog, and `--until-empty` reaches `eligible_remaining == 0`. The `entity-connect` enrich operation is promoted from "scan-only" to "fully-implemented". The crate manifest carries `version = "1.1.4"`. Library consumers must pin to `=1.1.4`.
- **Upgrading to v1.1.03?** No database migration required; the schema stays at v15 (the enrich sidecar queue gains a `claimed_at` column via idempotent ALTER) — just `cargo install sqlite-graphrag --locked --force`. Closes the six operator-blocking bugs catalogued in `gaps.md` plus the V8 oversized-body gate. Bug fixes: (1) the enrich scan-enqueue path now batches candidate inserts in a single transaction instead of row-by-row under the WAL write lock; (2) `reclassify-relation` gains `--literal-to <RELATION>` so `--literal-from applies_to --literal-to applies-to --batch` migrates the 61 357 legacy underscore edges to canonical hyphen form; (3) `merge-entities` gains `--cross-namespace` (opt-in, default same-namespace) so `--ids`/`--into-id` resolve across all namespaces; (4) the enrich sidecar gains a `claimed_at` column plus `enrich --reset-stale-claims` and `enrich --stale-claim-secs <N>`, with stale `processing` claims reset on startup and a SIGTERM handler performing graceful cleanup before exit 19; (5) docs-only — the `enrich --status` help text clarifies `scan_backlog` vs `queue_pending` vs cooldown vs deadlock; (6) the `re-embed --target chunks` scanner switches to `LEFT JOIN memories` so chunks of soft-deleted mothers reach 100% coverage. New subcommand: `split-body` divides memories whose body exceeds 25 000 characters into daughter memories and creates `replaces` relations (daughters need a follow-up `enrich --operation re-embed --target memories`). New flags: `--literal-to`, `--cross-namespace`, `--reset-stale-claims`, `--stale-claim-secs`. The official release name is v1.1.03; the crate manifest carries `version = "1.1.3"` because the SemVer parser rejects a leading zero in the patch component. Library consumers must pin to `=1.1.3`.

- **Upgrading to v1.1.02?** No database migration required; the schema stays at v15 — just `cargo install sqlite-graphrag --locked --force` (the crate manifest carries `version = "1.1.2"` because the SemVer parser rejects a leading zero in the patch component). v1.1.02 closes the two residual gaps tracked after v1.1.01 plus regression coverage and a new prune flag: the deprecated `--gliner-variant` argument is dropped from `remember` and `ingest` (clap rejects it with exit 2, dead GLiNER plumbing deleted, tests/gliner_variant_removed_regression.rs); the embedding token ceiling raises the typed `AppError::TooManyTokens { tokens, limit }` enforced at the write boundary of `remember`/`remember-batch`/`edit` and inside the shared embedding client (exit 6 preserved); `tests/reembed_entities_integration.rs` guards the re-embed entity dispatch fix landed in v1.1.01; and `enrich --prune-dead-entity-orphans` prunes entity-keyed dead-letter rows from the queue sidecar (complementing the memory-scoped `--prune-dead-orphans`). Four pre-existing rustdoc warnings were also resolved. Library consumers must pin to `=1.1.2`.
- **Upgrading to v1.1.01?** No database migration required; the schema stays at v15 — just `cargo install sqlite-graphrag --locked --force` (the crate manifest carries `version = "1.1.2"` because SemVer rejects a leading zero in the patch component). v1.1.01 closes the 12-priority `gaps.md` roadmap: entity/chunk vectors are written and backfilled through the same OpenRouter REST path as memories, with an empty-vector guard on the vector upserts (P1); `enrich --operation re-embed --target memories|entities|chunks|all` backfills per table and also re-selects divergent-`dim` or empty-blob vectors (P2/P10); `graph recompute-degree` reconciles the cached `entities.degree` with `--dry-run` and the `{total, updated, zeroed, unchanged}` envelope (P3); `reclassify-relation --literal-from` matches the stored relation verbatim to migrate legacy hyphenated edges (P4); `merge-entities --ids/--into-id` and `rename-entity --id` disambiguate by ID within a namespace (P5); `health --json` and `embedding status --json` expose per-table vector coverage (`vec_*_missing`, `vec_*_coverage_pct`) (P6); `EntityType` fails early with a message listing the 13 valid values (P7); the exit-6 limit errors are the typed `AppError::BodyTooLarge`/`AppError::TooManyChunks` carrying bytes/chunks and the limit in the envelope (P11); and `ingest --name-prefix` prefixes every derived memory name (P12). Library consumers must pin to `=1.1.2`.
- **Upgrading to v1.1.0?** No database migration required; the schema stays at v15 (the enrich sidecar `.enrich-queue.sqlite` gains diagnostic columns via idempotent ALTER) — just `cargo install sqlite-graphrag --locked --force`. v1.1.0 resolves the enrichment dead-letter backlog at its root: truncated OpenRouter completions are detected (`finish_reason=length`) and retried with a grown `max_tokens` (GAP-SG-70/71), dead-letter rows carry `finish_reason`/`input_tokens`/`output_tokens` (GAP-SG-72, via `--list-dead --json`), retry-classification is fully typed with no message-substring matching (GAP-SG-73), the shared `openrouter_http` module de-duplicates the chat/embedding clients (GAP-SG-74), the HTTP User-Agent is `sqlite-graphrag/1.1.0` (GAP-SG-75), the dequeue is bounded under lock contention (exit 15 on sustained `SQLITE_BUSY`, GAP-SG-76), `enrich --status` reports a real per-operation `scan_backlog` that never diverges from a real scan (GAP-SG-77), and a not-yet-materialized entity is retried as `Transient` instead of dead-lettered on first miss (GAP-SG-78). Library consumers must pin to `=1.1.2`.
- **Upgrading to v1.0.99?** No database migration required; the schema stays at v15 — just `cargo install sqlite-graphrag --locked --force`. v1.0.99 removes the `--max-entity-degree` flag from `remember`/`link` (BREAKING — passing it now yields a clap exit 2; the obsolete `--max-entity-degree 0` mitigation is no longer needed since writes never prune edges); no schema migration. v1.0.97 hardens the enrich dead-letter queue with recovery and inspection flags (`--requeue-dead` moves terminal `dead` items back to `pending`, `--list-dead` lists them with `error_class`/`message`, `--ignore-backoff` bypasses the `next_retry_at` cooldown, `--prune-dead-orphans` deletes orphan dead-letter rows whose memory was renamed or purged after enqueue), lets `--status`/`--list-dead`/`--requeue-dead`/`--prune-dead-orphans` run without `--operation`/`--mode`, adds the `augment-bindings` operation (requires `--names`) and `body-extract --body-extract-graph-only`, raises the `--max-attempts` default to 8 and the `--openrouter-timeout` default to 600s. `remember` gains `--graph-file` (combinable with `--body-file`), `--strict-name` and `--replace-graph`; `ingest` gains `--force-merge` with `body_hash` dedup and native large-body auto-split; `read` gains `--format raw`; `unlink` gains `--memory <name> --entity <name>` for curated bindings. `embedding status` adds a `coverage` object and `stats --json` a top-level `total_memories`. `--db` belongs AFTER the subcommand; `SQLITE_GRAPHRAG_DB_PATH` is the canonical position-independent override (SG-32). Library consumers must pin to `=1.0.99`.
- **Upgrading from v1.0.94 to v1.0.95?** No database migration required; the schema stays at v15 — just `cargo install sqlite-graphrag --locked --force`. v1.0.95 adds `enrich --mode openrouter`, routing the extraction JUDGE through the OpenRouter REST `/chat/completions` endpoint so structured extraction (memory-bindings, entity-descriptions, body-enrich, etc.) no longer requires a local claude/codex/opencode CLI. New flags: `--openrouter-model` (required with `--mode openrouter`; no default — its absence exits 1 before any network call), `--openrouter-api-key` (env `OPENROUTER_API_KEY`), `--openrouter-timeout` (default 300s) and `--openrouter-base-url`. The SCAN→JUDGE→PERSIST pipeline is unchanged; only the JUDGE transport moves (ADR-0054). Library consumers must pin to `=1.0.95`.
- **Upgrading from v1.0.85 / v1.0.86 / v1.0.87 / v1.0.88 / v1.0.89 / v1.0.90 to v1.0.91?** No database migration required; just `cargo install sqlite-graphrag --locked --force`. v1.0.91 fixes GAP-SPAWN-001 (LLM subprocesses no longer inherit `.mcp.json` — embedding works zero-config in any project), BUG-17 (`entities.degree` inflation replaced by `recalculate_degree`), BUG-15 (7 schema enums), BUG-16 (`deep-research` schema), GAP-SPAWN-002 (orphan dir cleanup) and BUG-14 (test fix). Library consumers must pin to `=1.0.91`.

```bash
cargo install sqlite-graphrag --locked --force
sqlite-graphrag --version
```


## What is it?
### sqlite-graphrag delivers durable memory for AI agents
- Stores memories, entities and relationships inside a single SQLite file under 25 MB
- **Build (v1.0.94):** LLM-only and one-shot — embeddings are generated by spawning `claude -p`, `codex exec`, `opencode run` with OAuth, or via OpenRouter REST API (`--embedding-backend openrouter`); no local model, no daemon, no ONNX runtime, ~19 MiB binary. LLM subprocesses run in an isolated temp directory (GAP-SPAWN-001) so `.mcp.json` from the caller's project is never inherited. Since v1.0.95, `enrich --mode openrouter` can run the extraction JUDGE entirely through the OpenRouter REST chat API — no local claude/codex/opencode CLI required (ADR-0054)
- **Legacy build:** REMOVED in v1.0.79 — the `embedding-legacy` feature and the local fastembed/ONNX path no longer exist
- Combines FTS5 full-text search with pure-Rust cosine similarity into a hybrid Reciprocal Rank Fusion ranker
- Stores and traverses an explicit entity graph with typed edges for multi-hop recall across memories
- Preserves every edit through an immutable version history table for full audit
- Runs on Linux, macOS and Windows natively with zero external services required (default build needs `claude`, `codex` or `opencode` CLI on `PATH`)


## Why sqlite-graphrag?
### Differentiators against cloud RAG stacks
- **OAuth-only LLM flow** — no API keys ever in the environment; the spawn ABORTS if `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` is set (defence in depth since v1.0.69)
- **Custom Anthropic-compatible providers (v1.0.83+)** — preserves `ANTHROPIC_AUTH_TOKEN` and `ANTHROPIC_BASE_URL` so Claude Code can route to MiniMax, OpenRouter or corporate gateways without breaking the OAuth-only mandate. Set `SQLITE_GRAPHRAG_STRICT_ENV_CLEAR=1` (or `--strict-env-clear`) for compliance environments that forbid credential forwarding.
- **No recurring embedding fees** — embeddings come from your existing Claude Pro / Max or ChatGPT Pro subscription
- Single-file SQLite storage replaces Docker clusters of vector databases entirely
- Graph-native retrieval beats pure vector RAG on multi-hop questions by design
- Deterministic JSON output unlocks clean orchestration by LLM agents in pipelines
- Native cross-platform binary ships without Python, Node or Docker dependencies (default build needs only `claude`, `codex` or `opencode` CLI)


## Stability Policy (G53, v1.0.80)

- The **public contract is the CLI**. The `--json` envelopes documented in `docs/schemas/*.schema.json` and the environment variables listed in `llms.txt` and `llms-full.txt` are stable across all v1.x.y releases. Consumers who depend on the CLI alone are not affected by minor or patch bumps.
- The **library API is unstable** in v1.x.y. Re-exports, public struct fields and function signatures may change in any v1.x.y release without a major version bump.
- Breaking changes to the library API ship as a **minor** bump, never patch (e.g. 1.0.79 -> 1.1.0 for a removed re-export). Patch bumps (1.0.79 -> 1.0.80) are limited to additive, non-breaking changes.
- Consumers who depend on the library API must pin to an exact version (`sqlite-graphrag = "=1.0.80"`) and review CHANGELOG.md before bumping.
- This stance is recorded in `docs/decisions/adr-0032-g53-lib-api-policy.md`.

## Superpowers for AI Agents
### First-class CLI contract for orchestration
- Every subcommand accepts `--json` producing deterministic stdout payloads
- **v1.0.76 is one-shot by default** — no background process; each embedding call spawns a fresh `claude -p`, `codex exec` or `opencode run`
- Every write is idempotent through `--name` kebab-case uniqueness constraints
- Stdin is explicit: use `--body-stdin` for body text or `--graph-stdin` for one `{body?, entities, relationships}` object; raw entity and relationship arrays use `--entities-file` and `--relationships-file`
- `remember` accepts body payloads up to `512000` bytes and up to `512` chunks
- Relationship payloads use `strength` in `[0.0, 1.0]`, mapped to `weight` in outputs
- Stderr carries tracing output under `SQLITE_GRAPHRAG_LOG_LEVEL=debug` only; use global `--quiet`/`-q` (v1.1.05) to suppress non-error tracing in headless pipelines (never mix stderr into JSON with `&>`)
- `--help` is English-first by design; use `--lang` for human-facing runtime messages, not static clap help text
- Cross-platform behavior is identical across Linux, macOS and Windows hosts


## Graph Schema
### Entity types, relation labels and edge strength
- `entity_type` accepts exactly 13 values: `project`, `tool`, `person`, `file`, `concept`, `incident`, `decision`, `memory`, `dashboard`, `issue_tracker`, `organization`, `location`, `date`
- `relation` (CLI input) accepts any kebab-case or snake_case string. 12 canonical values are well-known: `applies-to`, `uses`, `depends-on`, `causes`, `fixes`, `contradicts`, `supports`, `follows`, `related`, `mentions`, `replaces`, `tracked-in`. Custom values (e.g., `implements`, `tested-by`, `blocks`) are accepted with a `tracing::warn!`. JSON output normalizes to underscores (e.g., `applies_to`).
- `strength` is a float in `[0.0, 1.0]` representing edge weight; mapped to `weight` in all read outputs
- Unlisted `entity_type` values are rejected at write time with exit code 1. Custom `relation` values are accepted since v1.0.49.
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
- **GraphRAG is enabled by default and runs automatically.** Every subcommand auto-initializes `graphrag.sqlite` in the current working directory if it does not exist. Entity/relationship extraction comes from the LLM backend (`--extraction-backend llm`, the default) or from curated graph input (`--graph-stdin`, `--entities-file`).

### Automatic extraction (`--enable-ner`)
- Pass `--enable-ner` to activate automatic extraction on `remember` and `ingest` (product env is not read at runtime; v1.1.8)
- Since v1.0.79 this runs URL-regex extraction ONLY — the local GLiNER zero-shot pipeline was removed together with the `ner-legacy` feature
- `--gliner-variant` was REMOVED in v1.1.02 (clap rejects it with exit 2, following the `--max-entity-degree` precedent of v1.0.99); the `SQLITE_GRAPHRAG_GLINER_MODEL` and `SQLITE_GRAPHRAG_GLINER_THRESHOLD` env vars were deleted from the code in v1.1.02 and are silently ignored if set
- Response field `extraction_method` reports `url-regex`, `regex-only`, or `none:extraction-failed`
- For high-quality entity/relationship extraction prefer `ingest --mode claude-code`/`--mode codex` (LLM-curated) or pass curated entities via `--graph-stdin`
- `--skip-extraction` is deprecated since v1.0.45 and has no effect

- **`sqlite-graphrag init` is OPTIONAL** but recommended on first use because it creates the database, applies migrations and validates that a `claude`, `codex` or `opencode` CLI is reachable on `PATH` (there is no model download since v1.0.76 — embeddings come from the LLM subprocess).
- **`graphrag.sqlite` is created in the current working directory by default** (override with `--db <path>` after the subcommand, or persist a default via `config set db.default_path <path>`)
- For the local checkout, `cargo install --path .` is enough
- Re-run `sqlite-graphrag --version` after any upgrade to confirm the active binary
- After the public release, prefer `--locked` to preserve the tested MSRV dependency graph


## Version Highlights
- **v1.1.06**: Closes GAP-ENTITY-CONNECT-SCAN-CARTESIAN (P0, no schema migration; schema stays at v16) — `entity-connect` scan no longer builds the O(n²) cartesian product of namespace entities with a global `ORDER BY` that hung large `global` graphs (~10⁵ entities, ~100% CPU, no `phase: scan`, singleton → exit 75 cascade). Candidates come from **co-occurrence** in `memory_entities` plus **hub × degree-0 island** fill; queue keys are `pair:{id1}:{id2}` with `item_type=entity_pair`; drain resolves by ID without re-scanning; `--max-runtime` / soft 120s ceiling covers the **first** scan via `InterruptHandle` (Timeout exit 1, not 75); NDJSON emits `scan_start` before SQL. Suite: `tests/v1106_entity_connect_scan_regression.rs`. Crate manifest `version = "1.1.6"`; official release name v1.1.06

- **v1.1.05**: Five operator-blocking bugs from the 2026-07-08 deep-research "danilo" incident closed (no schema migration; schema stays at v16) — Bug 1: `deep-research` single-token queries expand to multi-aspect sub-queries (`source: "aspect"`, EN/PT facets; manual via `--sub-query-strategy manual --sub-queries-file`); Bug 2: `deep-research --output PATH` atomic write (atomwrite) with stdout ack `{written, bytes, blake3, ...}` plus global `--quiet`/`-q` (stdout JSON / stderr logs — never `&>`); Bug 3: `graph traverse --fuzzy` auto-resolves a clear short-name winner; without `--fuzzy`, NotFound includes ranked Jaro-Winkler/prefix suggestions; Bug 4: `merge-entities` rejects self-ref if `--into-id` is in `--ids` (or names) BEFORE any DB work; Bug 5: `link --from-id`/`--to-id`; pure digit names rejected so `--create-missing` cannot create ghost numeric entities. Suite: `tests/v1105_danilo_bugs_regression.rs`. The crate manifest carries `version = "1.1.5"`; SemVer rejects a leading zero in the patch component, so the official release name is v1.1.05

- **v1.1.04**: Two structural gaps from `gaps.md` closed plus schema v16 — Gap 1 (GAP-001): `deep-research` no longer panics with "Cannot start a runtime from within a runtime"; the sync entry point computes per-sub-query embeddings BEFORE building its dedicated Tokio runtime (`compute_sub_embeddings`), and the three OpenRouter embedding paths in `embedder.rs` adopt the canonical `Handle::try_current` + `block_in_place` reentry pattern; `ingest_opencode` is also guarded. Gap 2 (GAP-002): `entity-connect` now converges via the new `entity_connect_seen` table (migration V016) recording the LLM verdict (`related`/`none`) per pair, the `scan_isolated_entity_pairs` scanner excluding evaluated pairs and prioritising hubs, `count_operation_backlog` reporting a real O(n) backlog proxy (degree-0 entities with NER bindings), and `call_entity_connect` persisting the verdict on both branches; `entity-connect` promoted from scan-only to fully-implemented. Migration V016 adds `entity_connect_seen(source_id, target_id, namespace, verdict, relation, evaluated_at)` with composite PK, dual ON DELETE CASCADE FK to `entities(id)`, verdict CHECK and namespace index; `CURRENT_SCHEMA_VERSION` 15→16. The crate manifest carries `version = "1.1.4"`; SemVer rejects a leading zero in the patch component, so the official release name is v1.1.04

- **v1.1.03**: Six operator-blocking bugs from `gaps.md` closed plus the V8 oversized-body gate — Bug 1: enrich scan-enqueue batches candidate inserts in a single transaction instead of row-by-row under the WAL write lock (eliminates the scan-phase deadlock under stale-claim contention); Bug 2: `reclassify-relation --literal-to <RELATION>` (verbatim target) complements `--literal-from`, so `--literal-from applies_to --literal-to applies_to --batch` migrates the 61 357 legacy underscore edges to canonical hyphen form; Bug 3: `merge-entities --cross-namespace` (opt-in, default same-namespace) lets `--ids`/`--into-id` resolve across all namespaces; Bug 4: enrich sidecar gains a `claimed_at` column, stale `processing` claims reset on startup, a SIGTERM handler does graceful cleanup before exit 19, plus `enrich --reset-stale-claims` and `--stale-claim-secs <N>`; Bug 5 (docs-only): `enrich --status` help clarifies `scan_backlog` (real pending work) vs `queue_pending` (computed count) vs cooldown vs deadlock; Bug 6: `re-embed --target chunks` and `count_operation_backlog` switch to `LEFT JOIN memories` so chunks of soft-deleted mothers reach a real 100% coverage. New `split-body` subcommand divides memories over 25 000 characters into daughters at chunk boundaries and creates `replaces` relations (daughters need a follow-up `enrich --operation re-embed --target memories`). The crate manifest carries `version = "1.1.3"`; SemVer rejects a leading zero in the patch component, so the official release name is v1.1.03. Sidecar queue gains a `claimed_at` column via idempotent ALTER; main schema stays at v15 (no migration)

- **v1.1.02**: Two residual gaps closed after v1.1.01 (Gap 1: `--gliner-variant` removed from `remember`/`ingest`, clap exit 2, dead GLiNER plumbing deleted, tests/gliner_variant_removed_regression.rs; Gap 2: typed `AppError::TooManyTokens { tokens, limit }` enforced at the write boundary of `remember`/`remember-batch`/`edit` and inside the shared embedding client, exit 6 preserved; Gap 3: tests/reembed_entities_integration.rs regression test for the re-embed entity dispatch); `enrich --prune-dead-entity-orphans` prunes entity-keyed dead-letter rows from the queue sidecar; 4 pre-existing rustdoc warnings resolved (backticks escaping HTML tags, broken cfg(test) intra-doc links). No schema migration (v15)
- **v1.1.01**: 12-priority `gaps.md` roadmap closed (P1..P12) — entity embedding routed through the OpenRouter REST API even under `--llm-backend none` (chain `[OpenRouter]`) with an empty-vector guard on the memory/entity/chunk vector upserts (P1); `enrich --operation re-embed --target memories|entities|chunks|all` per-table backfill with a per-target `scan_backlog` in `--status` (P2); new `graph recompute-degree` subcommand (single transaction, `--dry-run`, envelope `{total, updated, zeroed, unchanged}`) (P3); `reclassify-relation --literal-from` matches the stored relation verbatim (no clap normalization) to migrate legacy hyphenated edges (P4); `merge-entities --ids/--into-id` and `rename-entity --id` for namespace-scoped ID disambiguation (P5); `health --json` gains `vec_memories_missing`/`vec_entities_missing`/`vec_chunks_missing` and per-table `vec_*_coverage_pct`, `embedding status --json` gains per-table `*_missing` counters (P6); canonical vocabularies documented and `EntityType` gains a manual `Deserialize` that fails early listing the 13 valid values (P7); `re-embed` predicates also select divergent-`dim` or empty-blob vectors, not only missing ones (P10); typed `AppError::BodyTooLarge`/`AppError::TooManyChunks` carrying bytes/chunks and the limit in the envelope, exit 6 preserved (P11); `ingest --name-prefix` with name-cap validation (P12); HTTP User-Agent derived from CARGO_PKG_VERSION (`sqlite-graphrag/1.1.1`). No schema migration (v15)
- **v1.1.0**: Enrichment dead-letter backlog resolved at the root (GAP-SG-70..78) — truncated OpenRouter completions retried with a grown `max_tokens` (GAP-SG-70), adaptive `max_tokens` constants (GAP-SG-71), dead-letter diagnostic columns `finish_reason`/`input_tokens`/`output_tokens` via `--list-dead --json` (GAP-SG-72), fully-typed retry-classification (exhausted-internal-retry is `Transient`, GAP-SG-73), shared `openrouter_http` module (GAP-SG-74), HTTP User-Agent `sqlite-graphrag/1.1.0` (GAP-SG-75), bounded dequeue failing loud with exit 15 on sustained `SQLITE_BUSY` (GAP-SG-76), `enrich --status` reporting a real per-operation `scan_backlog` that kills the false `pending=0` for `entity-descriptions`/`body-enrich`/`re-embed` with `state` derived from it (GAP-SG-77), and a not-yet-materialized entity classified `Transient` via typed `AppError::EntityNotYetMaterialized` with a namespace-blind `entity-type-validate` lookup fix (GAP-SG-78). No schema migration (v15)
- **v1.0.99**: GAP-SG-67 — removed the destructive global degree-cap pruning and the `--max-entity-degree` flag from `remember`/`link` (BREAKING: clap exit 2 if passed; obsolete `--max-entity-degree 0` mitigation); writes are now purely additive (the total relationship count never decreases on a normal write); GAP-SG-68 — aligned the `graph entities --sort-by degree` doc to its ascending behaviour (`--order desc` for most-connected-first); GAP-SG-69 — `enrich --operation body-enrich --until-empty` now converges (scan skips bodies already vetoed by the preservation guard). No migration; schema stays at v15
- **v1.0.97**: Enrich dead-letter recovery, queue inspectors and write ergonomics — `enrich` adds `--requeue-dead` (terminal `dead` → `pending`), `--list-dead` (lists each dead item with `error_class`/`message`) and `--ignore-backoff` (dequeue ignoring `next_retry_at`) and `--prune-dead-orphans` (read-only inspector that deletes orphan `dead` memory rows whose `item_key` is absent from the main DB, mutating only the `.enrich-queue.sqlite` sidecar; GAP-SG-66, ADR-0058); `--status`, `--list-dead`, `--requeue-dead` and `--prune-dead-orphans` no longer require `--operation`/`--mode`; new operation `augment-bindings` (adds bindings to already-linked memories, requires `--names`/`--names-file`) and `body-extract --body-extract-graph-only` (read-only graph extraction without rewriting the body); `--max-attempts` default raised to 8; `--openrouter-timeout` default raised to 600s; the enrich queue stays in the sidecar `.enrich-queue.sqlite`; the per-namespace singleton remains, with `--rest-concurrency` (clamp 1..=16, default 8) as the throughput remedy (GAP-20). `remember` adds `--graph-file` (loads the graph from a file, combinable with `--body-file`), `--strict-name` (rejects non-kebab names instead of normalizing) and `--replace-graph` (with `--force-merge`, zeroes existing bindings before writing). `ingest` adds `--force-merge` (updates duplicates), dedups by `body_hash`, and natively auto-splits oversized bodies. `read --format raw` prints the pure body. `unlink --memory <name> --entity <name>` removes a single curated memory-entity binding. `embedding status` reports a `coverage` object of real vector counts; `stats --json` exposes a top-level `total_memories`. `--db <PATH>` is positional after the subcommand; `SQLITE_GRAPHRAG_DB_PATH` is the canonical position-independent override (SG-32). No schema migration (v15)
- **v1.0.96**: Enrich dead-letter + OpenRouter REST concurrency (GAP-ENRICH-BACKLOG-CONVERGE, GAP-OPENROUTER-REST-CONCURRENCY, ADR-0055) — the enrich queue (`.enrich-queue.sqlite`) gains a terminal `dead` status plus `error_class`/`next_retry_at` columns (idempotent `ALTER TABLE`) and an `idx_enrich_queue_eligible` index so the live backlog is strictly monotonically decreasing and converges; classification reuses `AttemptOutcome` + `compute_delay` from `src/retry.rs` (Transient rate-limit/timeout/5xx → exponential-backoff `next_retry_at`, HardFailure validation/parse → immediate terminal), an item turns `dead` after `--max-attempts` Transient retries (default 5, range 1..=20) or on the first HardFailure, and dequeue honours `next_retry_at` while excluding `dead`; new flags `--until-empty` (internal scan→drain loop replacing the external bash loop), `--max-runtime <SECONDS>` (wall-clock cap for `--until-empty`, default 3600), `--max-attempts <N>`, `--status` (read-only JSON counts — unbound_backlog, queue pending/done/failed/dead/skipped, eligible_now, waiting — no LLM call, no singleton) and `--rest-concurrency <N>` (REST fan-out for `--mode openrouter`, clamp 1..=16, default 8, distinct from `--llm-parallelism`); `embed_passages_parallel_with_embedding_choice` (`src/embedder.rs`) fans out OpenRouter REST calls per 32-chunk batch via a bounded `tokio::task::JoinSet` (in-flight clamp 1..16, Cloudflare-safe, no new dependency) with chunk-index order preserved, while SQLite writes stay serialized via WAL + atomic claim (single-writer intact); order-proof live test: diagonal cosine 0.9999, off-diagonal max 0.899, argmax 64/64; nextest 1086 passed, 0 failed, 6 skipped; no schema migration (v15)
- **v1.0.95**: OpenRouter chat enrichment (GAP-OR-ENRICH, ADR-0054) — `enrich --mode openrouter` routes the extraction JUDGE through the OpenRouter REST `/chat/completions` endpoint, so structured extraction (memory-bindings, entity-descriptions, body-enrich, etc.) no longer requires a local claude/codex/opencode CLI; new `src/chat_api.rs` (`OpenRouterChatClient`) mirrors `src/embedding_api.rs` retry/backoff policy (abort on 401/400/404, honour `retry-after` on 429, exponential backoff + jitter on 5xx, Authorization: Bearer only); new flags `--openrouter-model` (required, no default — absence exits 1 before any network call), `--openrouter-api-key` (env `OPENROUTER_API_KEY`), `--openrouter-timeout` (default 300s), `--openrouter-base-url`; Structured Outputs via `response_format` json_schema `strict:true` + `provider.require_parameters:true`; `reasoning.enabled:false` with a graceful reasoning-mandatory fallback (retries once omitting reasoning); 13/13 OpenRouter models verified (9 direct, 4 via fallback); `usage.cost` read from the response; `OPENROUTER_API_KEY` held in `secrecy`, zeroized on drop, never logged, never passed to a subprocess; SCAN→JUDGE→PERSIST pipeline unchanged; no schema migration (v15)
- **v1.0.94**: OpenRouter embedding backend (GAP-OR-INGEST) — `--embedding-backend auto|openrouter|llm` with `--embedding-model` for REST API embeddings (~200ms vs 15s subprocess LLM); `EmbeddingBackendChoice` propagated to ALL 13 embedding paths including enrich, init, rename-entity, ingest_claude and remember chunks (GAP-OR-PROPAGATION); exit code 78 for OpenRouter config errors (BUG-OR-EXIT-CODE); `--enrich-after` flag for ingest; 10 models verified E2E (Qwen, OpenAI, Google Gemini, NVIDIA, Mistral, BAAI, Perplexity); 5 BUG-OR fixes; 1059 tests, 0 failures
- `v1.0.92`: 8-gap documentation remediation, skill audit, CRUD expansion
- **v1.0.91**: Spawn CWD isolation (GAP-SPAWN-001) — LLM subprocesses run in an isolated temp directory so `.mcp.json` from the caller's project is never inherited; `entities.degree` inflation fix (BUG-17) via `recalculate_degree`; 7 JSON schema enum fixes (BUG-15); `deep-research` schema fix (BUG-16); orphan spawn dir cleanup (GAP-SPAWN-002); 877+ tests, 0 failures
- **v1.0.90**: OpenCode backend integration (GAP-OPENCODE-001/002) — third LLM backend alongside codex and claude; `--llm-backend opencode`, `--mode opencode` for ingest/enrich, `--opencode-binary/model/timeout` flags, env vars `SQLITE_GRAPHRAG_OPENCODE_*`; fallback chain extended to `codex → claude → opencode → none`; Windows compilation fix (BUG-WINDOWS-001); embedding timeout hardcode fix (BUG-TIMEOUT-HARDCODE-001); `list` pagination fix (BUG-LIST-TOTAL-COUNT-001); 24 total bug/gap fixes; 875+ tests, 0 failures
- **v1.0.85**: Five-gap remediation (ADR-0043) — `FallbackReason` extended from 3 to 7 variants (`EmbeddingFailed | SlotExhausted | OAuthQuota { backend } | BackendMismatch { requested, resolved } | DimZero | Cancelled | Timeout`) with a `reason_code` discriminator in `hybrid-search` and `recall` envelopes for granular diagnosis; `try_embed_query_with_deterministic_fallback` retries the alternative backend (codex ↔ claude) on `OAuthQuota` and sleeps 750ms on `SlotExhausted` before yielding to FTS5-pure; `LlmEmbedding::invoke_claude` captures 12-14 `anthropic-ratelimit-*-remaining` headers BEFORE checking the subprocess exit (G45-CR5 — quota exhaustion aborts the embed and triggers immediate fallback); `.github/workflows/embedder-ignore.yml` runs `#[ignore]` tests in a hermetic env (no API keys); 5 new regression tests in `tests/embedder.rs` covering GAP-003, G58, G45-CR5, G55, G56
- **v1.0.85.1 (2026-06-17, hotfix)**: recall --llm-backend none and hybrid-search --llm-backend none return exit 0 with envelope vec_degraded=true + source=fts_fallback + vec_degraded_reason=dim_zero (GAP-004, ADR-0043 hotfix).
- **v1.0.85.2 (2026-06-17, hotfix)**: --dry-run-backend works standalone without a subcommand (pub command: Option<Commands> at src/cli.rs:248); embed_via_backend returns Result<(Vec<f32>, LlmBackendKind), AppError> propagating resolved_kind (BUG-002); setup_mock_path() in tests/embedder.rs:37-77 aligned to JSON (BUG-003). 945 tests green.
- **v1.0.84**: GAP-002 real Claude backend split (ADR-0042) — `--llm-backend claude` no longer delegates to `codex` via `LlmEmbedding::detect_available`; new `embed_via_claude_local` entry point and `LlmEmbeddingBuilder` with `with_claude_builder`/`with_codex_builder`/`override_binary`/`override_model`; `backend_invoked` field in 7 JSON envelopes (`embedding status`, `remember`, `edit`, `ingest`, `recall`, `hybrid-search`, `enrich`); `vec_degraded_reason` field in `hybrid-search` and `recall`; global `--dry-run-backend` flag (ADR-0042 S6) resolves and prints backend without spawning subprocess; `apply_env_whitelist_for_claude` helper for hardened providers; `LlmBackendKind::as_str` and `FallbackReason::reason_code` for envelope canonical serialization; 5 new regression tests in `tests/embedder.rs`
- **v1.0.83**: Custom Anthropic-compatible providers (ADR-0041) — `claude_runner`, `codex_spawn` and `ingest_claude` preserve `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`, and `OTEL_EXPORTER_OTLP_ENDPOINT` in the subprocess environment; enables Anthropic-compatible providers (MiniMax/api.minimax.io, OpenRouter, corporate gateways) without breaking the OAuth-only mandate; new global `--strict-env-clear` flag (`SQLITE_GRAPHRAG_STRICT_ENV_CLEAR=1`) for compliance environments that forbid credential forwarding; new helper module `src/spawn/env_whitelist.rs` consolidating the duplicated whitelist logic across three spawners; 5 new integration tests in `tests/claude_runner_env.rs` covering custom-provider propagation, OAuth-only abort, codex base-url inheritance, strict-mode credential dropping, and audit-of-no-token-leak

- **v1.0.79**: G42 closed — the LLM embedding pipeline is no longer slow, serialized or fragile. **(S1)** configurable embedding dimensionality, default 64 (`--embedding-dim`, `SQLITE_GRAPHRAG_EMBEDDING_DIM`, range [8, 4096]; precedence flag > env > `schema_meta.dim` > 64; existing 384-dim databases keep working unchanged, ZERO schema change). **(S2)** batched LLM calls (`{items:[{i,v}]}` — chunks at 8, entity names at 25 at dim 64, dim-adaptive via clamp(base×64/dim, 1, base) since G44; 39 spawns collapse into 4-5). **(S3)** real bounded parallelism via `Semaphore` + `JoinSet` with the new `--llm-parallelism` flag on `remember` (default 4), `ingest` (default 2) and `edit`; results stream through a bounded mpsc channel. **(S4)** codex schema tempfiles are RAII `NamedTempFile`s; the reaper also removes stale `codex-home-{pid}` dirs. **(S5)** `SQLITE_GRAPHRAG_CLAUDE_EMBED_MODEL` env override. **(S6)** empty `CLAUDE_CONFIG_DIR` by default on the embedding path (~40-50s → ~10-15s per call). **(S7)** actionable codex headless error. **(S8)** panic-free signal handler (second signal exits 130 with ZERO I/O). **(S9)** canonical re-embed: `enrich --operation re-embed` plus `edit --force-reembed`. **(C5)** `validate_dim` errors on divergent vectors instead of silently normalising. Every LLM subprocess uses `kill_on_drop` plus `SQLITE_GRAPHRAG_EMBED_TIMEOUT_SECS` (default 300s). Also REMOVED: the daemon infrastructure and the legacy `embedding-legacy`/`ner-legacy`/`full` features with the fastembed/ort/ndarray/tokenizers/hf-hub optional dependencies — every build is LLM-only.
- **v1.0.78**: G41 fix — `migrate --rehash` no longer inserts phantom rows for unapplied migrations (V013 was being registered without executing its SQL)
- **v1.0.77**: G40 fix — the `run_rehash` INSERT now writes `applied_on` (RFC3339); a NULL there blocked every subsequent migration
- **v1.0.76**: **Breaking architectural change** — the default build becomes LLM-only and one-shot: no daemon, no ONNX runtime, no local model download; embeddings/NER delegate to `claude -p` or `codex exec` headless (OAuth). Migration V013 drops the `vec_*` virtual tables in favour of BLOB-backed embedding tables with pure-Rust cosine similarity. New `migrate --rehash` and `migrate --to-llm-only --drop-vec-tables` upgrade paths. 7 new ADRs (0019-0025) plus ADR-0026 documenting the V002 drift root cause
- **v1.0.75**: new `ExtractionBackend` trait (G21) behind the global `--extraction-backend llm|embedding|none|both` flag; LLM-backed extraction becomes the default
- **v1.0.74**: `--skip-extraction` no-op compatibility restored (v1.0.45 promise honored) — the hard validation error introduced in v1.0.67 reverted to `tracing::warn!`
- **v1.0.73**: CI fix — `clang`/`mold`/`lld` installed inside the `cross` container for `aarch64-unknown-linux-gnu` builds
- **v1.0.72**: CI fix — mold linker installed on `ubuntu-latest` runners (12+ jobs failed with `invalid linker name in argument`)
- **v1.0.71**: CI fix — `Swatinem/rust-cache` repinned from the non-existent `v2.8` ref to `v2.9.1` across 17 call-sites
- **v1.0.70**: i18n fix — manual POSIX locale precedence `LC_ALL > LC_MESSAGES > LANG` (the cached system locale ignored runtime env vars)
- **v1.0.69**: 12 gaps closed (G28-G39) with full OAuth-only enforcement. **(OAuth-only behaviour change)** `claude -p` and `codex exec` spawns now ABORT with `AppError::Validation` if `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` are set; the `--bare` flag is REMOVED from all executable code. Operators using API keys MUST migrate to OAuth. **(G28 CRITICAL)** 4 reinforcing fixes for process proliferation: 7 flags hardening in `claude_runner::build_claude_command` (always passes `--strict-mcp-config --mcp-config '{}' --settings '{"hooks":{}}' --dangerously-skip-permissions`), `SIGTERM` on timeout, new `src/reaper.rs` walking `/proc` at startup, and `src/system_load.rs` plus `CircuitBreaker` integration. **(G29)** `enrich --operation body-enrich` now succeeds 100% (was 100% CHECK constraint failure), with audit trail via `memory_versions`, type-safe `MemorySource` enum, Jaccard preservation gate (10 tests, default 0.7), and `blake3` idempotency skip. **(G30)** Singleton lock scoped per `(job_type, namespace, db_hash)` with new `--wait-job-singleton` and `--force-job-singleton` flags. **(G31+G32+G33)** New `src/commands/codex_spawn.rs` (~700 lines, 11 tests) unifies spawn pipeline, JSONL parser, and ChatGPT Pro OAuth model validation; `enrich --mode codex` and `ingest --mode codex` share the same canonical command (was divergent, motivated the `~/.local/bin/codex-clean` wrapper). **(G34)** Worker warning is conditional to mode (Claude > 4, Codex > 16). **(G35)** `--preflight-check`, `--fallback-mode`, `--rate-limit-buffer` prevent batch loss on Claude rate limit. **(G36)** `optimize` pre-checks FTS5 health before rebuilding, plus new `--fts-dry-run`, `--fts-progress`, `--yes`. **(G37)** `--names <NAME>` and `--names-file <PATH>` for selective enrichment. **(G38)** Backup defaults 25x faster (1000/5ms vs 100/50ms) with 4 new tuning flags. **(G39)** New `vec orphan-list`/`vec purge-orphan`/`vec stats` subcommand family plus `forget` hook to prevent new orphans. **+53 tests** (692 → 745). 7 new ADRs (`docs/decisions/adr-0011-0017-*.md`) document every architectural decision.
- **v1.0.68**: 2 CRITICAL fixes for Windows + process proliferation.  **(G29)** `cargo install` no Windows was breaking with `error[E0308]` in `src/terminal.rs:29` because `HANDLE` in `windows-sys >= 0.59` is `*mut c_void` (was `isize` in 0.48/0.52).  Replaced with the type-safe idiom `!handle.is_null() && handle != INVALID_HANDLE_VALUE`, pinned `windows-sys` to `=0.59.0` exact, and added CI job `windows-build-check` that runs `cargo check --target x86_64-pc-windows-msvc` on every push.  **(G28-B)** Added `lock::acquire_job_singleton` per `(job_type, namespace)` so two parallel `enrich`/`ingest --mode claude-code|codex` invocations against the same database now fail fast with the new exit-75 `AppError::JobSingletonLocked { job_type, namespace }` instead of stacking 4 × N workers × 10 MCP processes (root cause of the 2026-06-03 276-load-average incident).  **(G28-A)** `claude_runner::build_claude_command` now respects `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` — when set to an empty directory, the subprocess is spawned with `CLAUDE_CONFIG_DIR=<that dir>`, suppressing user-scoped MCP servers and their 8-10-process fan-out.  Deliberately avoids `--strict-mcp-config` / `--mcp-config '{}'` because [anthropics/claude-code#10787] documents that Claude Code CLI ignores both flags.  **(G28-D)** `retry::CircuitBreaker` helper plus a `tracing::warn!` when `--llm-parallelism > 4` (combine with `CLAUDE_CONFIG_DIR` override to keep subprocess fan-out manageable).  Also fixed 3 pre-existing test failures in `src/commands/{history,list,read}.rs` that were leaking the `SQLITE_GRAPHRAG_DISPLAY_TZ` env var between parallel tests.
- **v1.0.67**: 2 NEW commands: `remember-batch` (NDJSON batch memory creation with `--transaction`/`--force-merge`), `completions` (shell completions for Bash/Zsh/Fish/PowerShell/Elvish); `read --id` for direct memory_id lookup, `enrich --llm-parallelism` for parallel LLM workers, `health` super-hub detection (degree > 50), `edit` skip-embed optimization via body_hash comparison, `rename` ghost purge for soft-deleted name conflicts, flag validation in hybrid-search/recall/ingest, V012 relationship timestamps migration, 24 gap fixes total
- **v1.0.66**: 35 BUG/GAP fixes including 3 CRITICAL (reclassify-relation crash, evidence chain flooding, link weight), `edit --type` flag, `graph_context` in deep-research, LLM-friendly aliases for graph/list JSON, full doc audit
- **v1.0.65**: 3 NEW commands: `reclassify-relation` (bulk relationship type renames with UNIQUE collision handling), `normalize-entities` (normalize entity names to kebab-case with auto-merge), `enrich` (LLM-augmented graph quality: memory-bindings, entity-descriptions, body-enrich); CRITICAL deep-research fixes: per-sub-query embeddings (was sharing one), RRF fusion for KNN+FTS5 (was hardcoded 0.5), directed evidence chains (was flat global dump); new deep-research flags `--rrf-k`, `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`; entity name normalization on all write paths; `health` reports relation concentration; `--max-entity-degree` warning on link/remember
- **v1.0.64**: NEW `deep-research` command for parallel multi-hop GraphRAG research via query decomposition (up to 7 sub-queries) with bounded JoinSet + Semaphore fan-out and evidence chain assembly; ingest claude-code disables hooks via `--settings` for OAuth (was failing 65% of files), detects OAuth and omits misleading `cost_usd`, validates body size BEFORE LLM extraction (files >512 KB skipped); rename/rename-entity reject same-name with exit 1
- **v1.0.63**: restore preserves current name after rename (was reverting to version's original name), ingest claude-code/codex normalizes relation strings before DB insertion, edit re-generates vector embeddings when body changes, OAuth-first auth docs
- **v1.0.62**: 10 bug fixes for ingest --mode claude-code (G01 CRITICAL: recall now works), NEW --mode codex for OpenAI Codex CLI extraction, new flags --codex-binary/--codex-model/--codex-timeout
- **v1.0.61**: 15 bug fixes for ingest --mode claude-code (B00-B13), new --claude-timeout flag, wait-timeout subprocess management
- **v1.0.60**: NEW ingest --mode claude-code for LLM-curated extraction via Claude Code CLI, queue DB for resume/retry, 7 new ingest flags
- **v1.0.59**: rename-entity name validation, unlink schema fix, reclassify `description_updated` field, contract+schema tests for rename-entity, E2E entity validation tests, doc audit (6 files)
- **v1.0.58**: FTS5 sync fix (CRITICAL: remember --force-merge was silently corrupting FTS5 index), merge-entities UNIQUE fix for memory_entities, new `rename-entity` command, entity name validation, `memory-entities --entity` reverse lookup, `reclassify --description`, purge response `action` field, fts help EXAMPLES, health tracing
- **v1.0.57**: 16 fixes — merge-entities UNIQUE constraint, memory-entities column name, --clear-body validation, WAL checkpoint for fts rebuild/check, degree recalculation for delete-entity/merge-entities adjacents, atomic backup via tempfile-rename, 18 new contract+schema tests
- **v1.0.56**: 9 new commands (fts, backup, delete-entity, reclassify, merge-entities, memory-entities, prune-ner), 7 new flags, 19 new JSON fields, FTS5 graceful degradation, JSON error envelope
- **v1.0.55**: Full doc audit — export summary `total`→`exported`, list response fields corrected, `--tz` exit code 1→2, exit 2 added to exit code table, stats legacy aliases documented
- **v1.0.54**: WAL checkpoint for `prune-relations` (last missing command), `--graph-stdin` empty body validation, `memory_type` JSON field in `list`/`export`, `Vec::with_capacity` in 9 cold paths
- **v1.0.53**: WAL checkpoint TRUNCATE after every write command for Dropbox/cloud-sync safety, `export --json` contract fix, `Vec::with_capacity` in 12 hot paths
- **v1.0.52**: 12 gaps fixed, new `export` subcommand, exit code Duplicate 2→9 (breaking), `forget` not-found no JSON (breaking)
- **v1.0.51**: Namespace env var fix (8 commands), remember on soft-deleted fix, per-chunk RSS watchdog (`--max-rss-mb`), daemon test coverage
- **v1.0.50**: `prune-relations` subcommand, daemon auto-restart on version mismatch, V011 index, 37 doc gaps fixed
- **v1.0.49**: Extensible relation vocabulary, V010 migration, 15 doc updates
- **v1.0.48**: GLiNER NER functional, 5 bug fixes, full doc audit
- **v1.0.47**: Replace BERT NER with GLiNER zero-shot, 13 custom entity types, `--gliner-variant` flag
- **v1.0.35**: Flag aliases (`--from`/`--to`, `--old`/`--new`, `--limit` as alias of `--k`)


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
- Without `--db` (or a persisted `db.default_path` via `config set`), every CRUD command in that directory uses `./graphrag.sqlite`. Product env `SQLITE_GRAPHRAG_DB_PATH` is **not** read at runtime (v1.1.8)
### Remember a memory with an optional explicit entity graph
- By default, `remember` does NOT run automatic URL extraction (off by default)
- Pass `--enable-ner` to activate URL-regex extraction for that call (the GLiNER pipeline was removed in v1.0.79). Product env overrides are not read at runtime (v1.1.8)
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
- For LLM-curated entity/relationship extraction use `ingest --mode claude-code` or `ingest --mode codex`
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
- Set `OPENROUTER_API_KEY` env var or pass `--openrouter-api-key`
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
- All models produce 384-dimension vectors by default via MRL truncation — compatible with existing databases
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

# Or via env var (CLI flag takes precedence):
SQLITE_GRAPHRAG_LOW_MEMORY=1 sqlite-graphrag ingest ./docs --type document
```
### Bulk-ingest with LLM-curated entities via Claude Code (v1.0.61)
<!-- skip-test: requires Claude Code installed with Pro/Max subscription. -->
```bash
# Extract entities and relationships using locally installed Claude Code CLI
sqlite-graphrag ingest ./docs --mode claude-code --recursive --json

# Resume interrupted ingestion
sqlite-graphrag ingest ./docs --mode claude-code --resume --json

# Set budget limit
sqlite-graphrag ingest ./docs --mode claude-code --max-cost-usd 5.00 --json

# Extract entities and relationships using locally installed OpenAI Codex CLI
sqlite-graphrag ingest ./docs --mode codex --recursive --json
```
> **Authentication:** OAuth is the ONLY accepted credential flow. API keys are PROHIBITED.
> `--mode claude-code` reads OAuth from `~/.claude/.credentials.json` (Claude Pro/Max/Team).
> `--mode codex` reads device auth from `codex login` (OpenAI ChatGPT).
> Defining `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` in the environment ABORTS the spawn with `AppError::Validation` and exit code 1. The `--bare` flag (which would also demand an API key) is REMOVED from all executable code paths.
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
| `init` | `--namespace <ns>` | Initialize database, apply migrations and validate that a `claude`/`codex` CLI is reachable (no model download) |
| `health` | `--json` | Show database integrity, FTS5 functional check, sqlite version, super-hub detection (degree > 50); v1.1.01 adds `vec_memories_missing`/`vec_entities_missing`/`vec_chunks_missing` and per-table `vec_*_coverage_pct` |
| `stats` | `--json` | Count memories, entities and relationships; the JSON exposes a top-level `total_memories` |
| `migrate` | `--json` | Apply pending schema migrations via `refinery` |
| `vacuum` | `--json` | Checkpoint WAL and reclaim disk space |
| `optimize` | `--json`, `--skip-fts` | Run `PRAGMA optimize` and rebuild FTS5 index (skip with `--skip-fts`) |
| `backup` | `--output <path>` | Back up the database using the SQLite Online Backup API |
| `sync-safe-copy` | `--dest <path>` (alias `--output`) | Checkpoint then copy a sync-safe snapshot |
| `config` | `set`, `get`, `list` (`--effective`), `unset`, `path`, `doctor`, `add-key`, `list-keys`, `remove-key` | XDG operational config and API keys (v1.1.8); precedence flag > XDG `config set` > default; no product env |
### Memory content lifecycle
| Command | Arguments | Description |
| --- | --- | --- |
| `remember` | `--name`, `--type`, `--description`, `--body` (or `--body-file`/`--body-stdin`), `--entities-file`, `--relationships-file`, `--graph-stdin`, `--graph-file <path>`, `--llm-parallelism <N>` (default 4), `--enable-ner` (URL-regex only since v1.0.79), `--strict-name`, `--force-merge`, `--replace-graph`, `--clear-body`, `--dry-run` | Save a memory with optional entity graph; `--graph-file` loads the graph from a file (combinable with `--body-file`); `--strict-name` rejects non-kebab names instead of normalizing; `--replace-graph` (with `--force-merge`) zeroes existing bindings before writing; `--type`/`--description` optional with `--force-merge` (inherited from existing); `--dry-run` validates without persisting |
| `remember-batch` | `--transaction`, `--force-merge`, `--fail-fast`; NDJSON fields `name`/`type`/`description`/`body` (description **required** on create, v1.1.8 parity with `remember`) | Batch-create memories from NDJSON stdin; one invocation, one slot, one DB connection; empty description on create is rejected |
| `recall` | `<query>`, `-k`/`--k` (alias `--limit`), `--type`, `--max-hops`, `--max-distance`, `--all-namespaces`, `--no-graph` | Search memories semantically via KNN + graph traversal |
| `read` | `[name]` or `--name <name>`, `--id <N>`, `--with-graph`, `--format raw` | Fetch a memory by exact name or integer memory_id; `--with-graph` includes linked entities and relationships; `--format raw` prints the pure body with no JSON envelope |
| `list` | `--type`, `--limit`, `--offset`, `--include-deleted` | Paginate memories sorted by `updated_at`; default limit is all with `--json`, 50 for text; response includes `total_count`, `truncated`, `body_length` |
| `forget` | `[name]` or `--name <name>` | Soft-delete a memory preserving history |
| `rename` | `[old]`, or `--name`/`--old`/`--from <NAME>`, `--new-name`/`--new`/`--to <NAME>` | Rename a memory while keeping versions |
| `edit` | `[name]` or `--name`, `--body`, `--description`, `--type`, `--force-reembed`, `--llm-parallelism <N>` | Edit body, description or memory type creating new version; skips re-embedding when body content is unchanged; `--force-reembed` (v1.0.79) regenerates the embedding without changing the body |
| `history` | `[name]` or `--name <name>`, `--diff` | List all versions of a memory; `--diff` includes character-level change summary |
| `memory-entities` | `[name]` or `--name <name>`, `--entity <name>` | List entities linked to a memory, or memories linked to an entity (reverse lookup via `--entity`) |
| `restore` | `--name`, `--version` | Restore a memory to a previous version |
| `ingest` | `<DIR>`, `--type`, `--pattern <GLOB>` (default `*.md`), `--recursive`, `--mode` (`none`/`claude-code`/`codex`/`opencode`; `gliner` removed in v1.0.79), `--ingest-parallelism N`, `--llm-parallelism N` (default 2, embedding workers), `--low-memory`, `--enable-ner` (URL-regex only since v1.0.79), `--force-merge`, `--fail-fast`, `--dry-run`, `--claude-binary`, `--claude-model`, `--resume`, `--retry-failed`, `--max-cost-usd`, `--claude-timeout`, `--rate-limit-wait`, `--keep-queue`, `--queue-db`, `--name-prefix <PREFIX>` (v1.1.01) | Bulk-ingest every matching file as a separate memory (NDJSON output); `--force-merge` updates duplicate files instead of skipping them (dedup by `body_hash`); oversized bodies are auto-split natively into chunks; `--mode claude-code` uses locally installed Claude Code CLI for LLM-curated entity/relationship extraction; `--dry-run` previews name mapping without writing; `--claude-timeout` sets per-file subprocess timeout (default 300s); `--name-prefix` (v1.1.01) prepends a kebab-case prefix to every derived memory name (80-char name cap enforced) |
| `export` | `--namespace`, `--type`, `--include-deleted`, `--limit`, `--offset` | Export memories as NDJSON for backup or migration |
| `cache clear-models` / `cache list` / `cache stats` | `--yes` (clear) | Remove or list model files under the XDG cache directory; `cache stats` is a v1.1.8 alias of `list` (exit 0) |

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
| `prune-ner` | `--entity <name>` or `--all`, `--dry-run`, `--yes` | Remove NER bindings from memory_entities table |
| `fts rebuild` | `--json` | Rebuild the FTS5 full-text search index from scratch |
| `fts check` | `--json` | Run FTS5 integrity-check without modifying the index |
| `fts stats` | `--json` | Show FTS5 index statistics (row count, shadow pages) |
| `completions` | `bash`, `zsh`, `fish`, `powershell`, `elvish` | Generate shell completions for the specified shell |
| `enrich` | `--operation <op>` (memory-bindings, entity-descriptions, body-enrich, re-embed, augment-bindings, weight-calibrate, relation-reclassify, entity-connect, entity-type-validate, description-enrich, cross-domain-bridges, domain-classify, graph-audit, deep-research-synth, body-extract), `--target <memories\|entities\|chunks\|all>` (v1.1.01, `re-embed` only; default `memories`), `--mode <claude-code\|codex\|opencode\|openrouter>`, `--openrouter-model`, `--openrouter-api-key`, `--openrouter-timeout` (default 600s), `--openrouter-base-url`, `--until-empty`, `--max-runtime <SECONDS>`, `--max-attempts <N>` (default 8), `--status`, `--list-dead`, `--requeue-dead`, `--ignore-backoff`, `--prune-dead-orphans`, `--body-extract-graph-only`, `--rest-concurrency <N>`, `--llm-parallelism <N>`, `--preserve-threshold <FLOAT>`, `--preflight-check`, `--fallback-mode <mode>`, `--rate-limit-buffer <SECONDS>`, `--names <NAMES>`, `--names-file <PATH>`, `--max-load-check`, `--circuit-breaker-threshold <N>`, `--codex-model-validate`, `--codex-model-fallback <MODEL>`, `--resume`, `--retry-failed`, `--max-cost-usd <USD>`, `--claude-binary/--claude-model/--claude-timeout`, `--codex-binary/--codex-model/--codex-timeout`, `--db <DB>`, `--wait-job-singleton <SECONDS>`, `--force-job-singleton` | LLM-augmented graph quality pipeline (G29 + G35 + G37); OAuth-only via `--mode claude-code` (Anthropic) or `--mode codex` (ChatGPT Pro); `--mode openrouter` (v1.0.95) routes the JUDGE through the OpenRouter REST chat API using `OPENROUTER_API_KEY` and requires `--openrouter-model`; v1.0.96 added a dead-letter queue (terminal status `dead`, columns `error_class`/`next_retry_at`, sidecar `.enrich-queue.sqlite`) for guaranteed backlog convergence, plus `--until-empty` scan→drain looping and a read-only `--status` report; v1.0.97 adds dead-letter recovery (`--requeue-dead` returns `dead`→`pending`, `--list-dead` reports each with `error_class`/`message`, `--ignore-backoff` bypasses the `next_retry_at` cooldown), lets `--status`/`--list-dead`/`--requeue-dead`/`--prune-dead-orphans` run WITHOUT `--operation`/`--mode` (the latter prunes orphan `dead` memory rows whose `item_key` is gone from the main DB, mutating only the sidecar; GAP-SG-66, ADR-0058), adds the `augment-bindings` operation (requires `--names`) and `body-extract --body-extract-graph-only` (read-only graph extraction), and raises the `--max-attempts` default to 8 and `--openrouter-timeout` default to 600s |
| `vec orphan-list` | `--json` | List orphan memory embedding rows (G39) with `vector_hash` for traceability |
| `vec purge-orphan` | `--yes`, `--dry-run`, `--json` | Delete orphan memory embedding rows from `vec_memories`, `vec_entities`, `vec_chunks` (G39); `--yes` required as safety guard |
| `vec stats` | `--json` | Show statistics for `vec_memories`, `vec_entities`, `vec_chunks` tables (G39) |
| `codex-models` | `--json`, `--suggest <substring>` | List the ChatGPT Pro OAuth accepted-model whitelist (G33) or return the closest match via substring + Levenshtein |
| `remember-batch` | `--json`, `--transaction`, `--force-merge`, `--fail-fast`; NDJSON `description` required on create (v1.1.8) | Batch-create memories from NDJSON stdin (one invocation, one slot, one DB connection) |
| `namespace-detect` | `--json`, `--namespace <name>` | Resolve namespace precedence for the current invocation |
| `deep-research` | `<query>`, `--k`, `--max-sub-queries`, `--max-hops`, `--min-weight`, `--max-results`, `--with-bodies`, `--max-concurrency`, `--timeout`, `--rrf-k`, `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--output <PATH>` (v1.1.05 atomwrite), `--sub-query-strategy`, `--sub-queries-file` (v1.1.05), `--json` | Parallel multi-hop GraphRAG research via query decomposition; single-token queries expand to multi-aspect sub-queries (`source: "aspect"`, EN/PT facets; v1.1.05); `--output` writes the full envelope atomically and prints a short stdout ack; returns `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context?`, `stats` |

### v1.0.82 / v1.0.85 subcommands (no new subcommands added in v1.0.83/84/85; new fields and flags only)
| Command | Arguments | Description |
| --- | --- | --- |
| `pending` | `list`, `show <id>`, `cleanup`, `--filter-status queued\|processing\|done\|failed`, `--limit`, `--json` | Inspect and process the three-stage `remember` checkpoint queue (GAP-001, ADR-0036); `cleanup` removes terminal-state rows |
| `pending-embeddings` | `list`, `process`, `status` (v1.1.8 alias of `embedding status`), `--filter-status queued\|processing\|done\|failed\|skipped`, `--limit`, `--json` | Inspect and process the embedding retry queue (GAP-005, ADR-0040); `process` retries failed embeddings with the next backend in `--llm-backend`; `status` is the queue health alias |
| `slots` | `status`, `release --slot-id <N> --yes`, `--json` | Cross-process LLM slot semaphore inspection and cleanup (GAP-004, ADR-0039); `status` returns `max_concurrency`, `acquired`, `waiting`, `held_by_pid[]`, `p50_wait_ms`, `p99_wait_ms`; `release` reaps orphan slots from dead PIDs |
| `embedding` | `status`, `list`, `--filter-status queued\|processing\|done\|failed\|skipped`, `--limit`, `--json` | Health and per-entry inspection of the pending-embeddings queue (GAP-005); `status --json` reports a `coverage` object with the real vector counts per table; v1.1.01 adds per-table `*_missing` counters to `status --json` |

### v1.0.82 / v1.0.85 global flags
| Flag | Applies to | Description |
| --- | --- | --- |
| `--llm-backend <codex\|claude\|none,codex,...>` | `remember`, `edit`, `ingest`, `enrich` | Comma-separated backend chain tried in order; first non-error wins (ADR-0038, ADR-0040) |
| `--llm-fallback-mode <claude\|codex>` | `remember`, `edit`, `enrich` | Swap backend on rate-limit; requires `--llm-backend` chain with at least 2 entries |
| `--llm-max-host-concurrency <N>` | All LLM-spawning commands | Cap concurrent LLM subprocesses host-wide via `fs4` flock (ADR-0039); default derived from CPU and OAuth tier |
| `--llm-slot-wait-secs <N>` | All LLM-spawning commands | Seconds to wait for a free slot before failing (default 30s); pair with `--llm-slot-no-wait` for fail-fast |
| `--strict-env-clear` | `remember`, `edit`, `ingest`, `enrich`, `embedding`, `pending-embeddings` | Drop ALL credential env vars from the subprocess; preserve only `PATH` for binary resolution. Prefer the flag; optional XDG `spawn.strict_env_clear=1` via `config set` (ADR-0041; product env not read at runtime in v1.1.8) |
| `--dry-run-backend` | Top-level global flag | Resolve and print the resolved LLM backend (binary path, model, flavour, chain) WITHOUT spawning the subprocess. Prefer the flag; optional XDG `llm.dry_run_backend=1` via `config set` (ADR-0042 S6; product env not read at runtime in v1.1.8). Use for pre-flight audit; exit 0 indicates successful resolution |
| `--quiet` / `-q` | Top-level global flag (v1.1.05) | Suppress non-error tracing on stderr so stdout JSON stays clean for headless pipelines; pair with `deep-research --output PATH` for large envelopes. NEVER redirect stdout+stderr to the same file with `&>` |

### v1.0.82 / v1.0.85 exit codes
| Code | Meaning | Emitted by |
| --- | --- | --- |
| `19` | Shutdown signal received; partial work discarded; see `shutdown-envelope.schema.json` for stdout envelope | Any LLM-spawning command on SIGTERM/SIGINT/SIGHUP (ADR-0037) |

### `cache` subcommands
| Subcommand | Description |
| --- | --- |
| `list` | List cached model files with sizes and total disk usage |
| `stats` | Alias of `list` (v1.1.8 — agents often call `cache stats`) |
| `clear-models` | Remove cached embedding/NER model files (forces re-download on next `init`) |


## Configuration (XDG — v1.1.8)
### Precedence and storage (no product env)
- Runtime knobs resolve as **CLI flag > XDG `config set` > named default**
- **FORBIDDEN product env:** `SQLITE_GRAPHRAG_*` (and other product knobs formerly documented as env) are **not** read at runtime — flag > XDG `config set` > default only (G-T-XDG-04). Do not export product tables for configuration
- Persist settings with `sqlite-graphrag config set <KEY> <VALUE>`; inspect with `config get`, `config list`, `config list --effective`, `config unset`
- Secrets: `config add-key` (stdin) or per-invocation `--openrouter-api-key`; prefer XDG key store over shell history
- Database path: pass `--db <PATH>` after the subcommand, or persist via `config set db.default_path <path>`; default is `./graphrag.sqlite`. Product env `SQLITE_GRAPHRAG_DB_PATH` is **forbidden** / ignored at runtime
- OS env still allowed for locale (`LANG`/`LC_*`), `PATH`, `HOME`/`USERPROFILE`, XDG base dirs, `NO_COLOR`, and subprocess OAuth forwarding (`ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CODEX_ACCESS_TOKEN`) — never raw `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` (OAuth-only abort)
- Remote OTEL / product telemetry is forbidden; local tracing only (`-v` / `-q` / XDG `log.level`)

### Common `config set` keys
| Key | Purpose | Example |
| --- | --- | --- |
| `network.openrouter.chat_url` | OpenRouter chat completions URL (alias `network.chat_url`) | `https://openrouter.ai/api/v1/chat/completions` |
| `network.openrouter.embeddings_url` | OpenRouter embeddings URL (alias `network.embed_url`) | `https://openrouter.ai/api/v1/embeddings` |
| `llm.query_embed_timeout_secs` | Fail-fast budget for Auto query embedding before FTS fallback | `3` |
| `llm.probe_timeout_ms` | Credential/backend probe timeout | `800` |
| `embedding.dim` | Default embedding dimensionality | `384` |
| `log.level` | Local tracing level on stderr | `info` |
| `log.format` | `pretty` or `json` | `json` |
| `display.tz` | IANA zone for `*_iso` JSON fields | `America/Sao_Paulo` |
| `llm.fallback` | Backend fallback chain tokens | `codex,claude,none` |
| `enrich.entity_description.grounding_threshold` | ED grounding gate | `0.35` |
| `enrich.entity_description.domain` | Neutral multi-domain ED hint | `general` |

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
sqlite-graphrag config set llm.query_embed_timeout_secs 3
sqlite-graphrag config list --effective --json
sqlite-graphrag config doctor --json
# Immediate hard-delete of soft-deleted rows (default purge keeps 90-day retention)
sqlite-graphrag purge --now --yes --json
# UX aliases (v1.1.8)
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
| `78` | OpenRouter configuration error | `--embedding-backend openrouter` without `--embedding-model`, or invalid/missing `OPENROUTER_API_KEY` |


## Performance
### Measured on a 1000-memory database
- Embedding latency is dominated by the headless LLM round-trip (~1-3 s per batched call); pure reads (`read`, `list`, `graph`) stay in the low milliseconds
- Since v1.0.79 LLM calls are BATCHED (calibration bases of 8 chunks / 25 entity names at dim 64, dim-adaptive — G44) and PARALLEL (`--llm-parallelism`, bounded `Semaphore` + `JoinSet`), so a 39-item memory embeds in 4-5 calls instead of 39 serialized spawns
- `--embedding-dim 384` (the default since v1.0.94) matches the production corpus; under OpenRouter REST the MRL truncation is server-side at no token cost
- `init` performs no model download — it only creates the database and validates that a `claude`/`codex` CLI is reachable
- **Build (v1.0.79):** each embedding call spawns `claude -p` or `codex exec` — RSS is ~350 MB per LLM worker (the 1100 MB ONNX model load no longer exists in any build)


## Memory Requirements
### Sizing RAM for ingest and recall workloads
- The CLI itself is lightweight (~19 MiB binary); RAM is dominated by the LLM subprocesses at roughly 350 MB RSS per worker (`LLM_WORKER_RSS_MB`)
- Worker budget: effective parallelism is `min(--llm-parallelism, cpus, free_ram × 0.5 / 350 MB, 32)` — the concurrency gate adapts to available memory automatically
- Default parallelism increases RSS roughly linearly per worker (`--llm-parallelism 4` ≈ 4 × 350 MB of subprocess RSS on top of the CLI)
- Low-memory mode: pass `--low-memory` to force single-threaded ingest. Equivalent to `--ingest-parallelism 1` and overrides any explicit value, at the cost of 3-4x wall time. Product env is not read at runtime (v1.1.8).
- Container/cgroup users: budget `MemoryMax` for the CLI plus N × 350 MB LLM workers (the old 3 GB ONNX floor no longer exists)


## Storage Footprint
### Expected DB size relative to ingested content
> **Expected overhead: roughly 8× the total ingested body size** (e.g., 7.6 MB of text → ~62.9 MB DB).
> Overhead comes from float embeddings (default 64-dim since v1.0.79; pre-existing databases keep their recorded dimensionality, e.g. 384), FTS5 full-text index, and the entities/relationships graph.
> Run `sqlite-graphrag vacuum --json` after bulk `forget`+`purge` cycles to reclaim reclaimed space.


## Safe Parallel Invocation
### Counting semaphore with up to four simultaneous slots
- Each LLM embedding worker (`claude -p`/`codex exec` subprocess) consumes roughly 350 MB of RSS — the budget unit used by the concurrency gate since v1.0.79
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
- Default behavior always creates or opens `graphrag.sqlite` in the current working directory
- Database locked after crash requires `sqlite-graphrag vacuum` to checkpoint the WAL
- `init` is near-instant since v1.0.76 — there is no model download; if it fails, check that a `claude` or `codex` CLI is reachable on `PATH`
- Embedding calls failing with exit 11 usually mean the LLM CLI is missing, unauthenticated (OAuth required) or timing out — raise the embed timeout via CLI flag or `config set` (not product env; v1.1.8)
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
- 64 schemas cover `init`, `remember`, `remember-batch` (+ summary), `recall`, `hybrid-search`, `deep-research`, `list`, `read`, `forget`, `purge`, `rename`, `edit`, `history`, `restore`, `link`, `unlink`, `prune-relations`, `health`, `stats`, `migrate` (+ `migrate-rehash` + `migrate-to-llm-only`), `vacuum`, `optimize`, `cleanup-orphans`, `sync-safe-copy`, `backup`, `graph` (+ stats/traverse/entities), `related`, `namespace-detect`, `debug-schema`, `entities-input`, `relationships-input`, `ingest-file-event` (+ `ingest-summary`), `ingest-claude-phase` (+ file-event + summary), `export-memory-line` (+ summary), `enrich-phase` (+ item-event + summary), `fts rebuild` (+ `fts check` + `fts stats`), `vec orphan-list` (+ `vec purge-orphan` + `vec stats`), `codex-models`, `error-envelope`
- Treat these schemas as the agent contract; SKILL.md documents the same shapes in human-readable form
- Validate downstream consumers with any standard JSON Schema validator (e.g. `ajv`, `jsonschema`)


## Changelog
### Release history tracked separately
- Read the full release history in [CHANGELOG.md](CHANGELOG.md)


## Acknowledgments
### Built on top of excellent open source
- `fastembed` and `sqlite-vec` powered the local embedding pipeline up to v1.0.75 (removed since — embeddings now come from `claude`/`codex` subprocesses)
- `refinery` runs schema migrations with transactional safety guarantees
- `clap` powers the CLI argument parsing with derive macros
- `rusqlite` wraps SQLite with safe Rust bindings and bundled build


## License
### Dual license MIT OR Apache-2.0
- Licensed under either of Apache License 2.0 or MIT License at your option
- See `LICENSE-APACHE` and `LICENSE-MIT` in the repository root for full text
