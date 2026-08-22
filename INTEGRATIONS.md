# Integrations

> **v1.2.8 (current):** the dead `pending` family is removed — three verbs that could never return a row — taking the top-level catalogue to 50 verbs and the schema catalogue to 76 contracts. `hybrid-search` drops `fts_bm25`, a field the schema published and no code path ever filled. The orphan reaper runs on `sysinfo` instead of `/proc`, so macOS stops reporting zero orphans without having measured. A new gate refuses Portuguese identifiers and comments in `src/`, exempting only translated string literals under `src/i18n/`. Inherited from v1.2.8: `tests/rustdoc_link_gate.rs` closes the class of defect only `cargo doc` could see — a public doc comment linking a private item — statically, in milliseconds, with no build lock. `graph --format ndjson` now honours the agent-native surface instead of parsing its flags and discarding them: `--select` and `--truncate-content` apply per record, the whole-set knobs are refused before the first byte, and the summary line carries the `agent_surface` block. `--select`, `--filter`, `--sort` and `--dedupe-by` accept `entity_type` and `type` as one field, so a key learned on `graph entities` also resolves against the `graph --format json` snapshot; the wire is unchanged and the projection answers under the spelling you asked for. Inherited from v1.2.8: `remember` accepts the memory name positionally or via `--name`, never both, closing the last gap with `edit` / `read` / `forget` / `history`. A declared `entity_type` outside the canonical thirteen is ACCEPTED AND STORED AS WRITTEN: the fold onto a canonical kind is HISTORICAL, removed by the V017 migration that opened the vocabulary, and `normalize_entity_type` (`src/entity_type.rs`) now only trims, lowercases and turns `-` into `_`. Every non-canonical label is reported in `warnings` on `remember` AND `remember-batch`, and `--strict-entity-types` refuses the write instead — the sibling of `--strict-name`. `remember --dry-run` reports `entities_parsed`, `relationships_parsed` and the non-canonical labels it would store, instead of four members and `warnings: null`. Two input contracts are published: `schema --name graph-input` for the `--graph-stdin` / `--graph-file` wire shape, and `schema --name remember-dry-run` for an envelope that satisfied no schema at all. Inherited from v1.2.8: `--count-only` is REFUSED with exit `2` over a paginated command whose limit actually cut rows — `--filter-scope page` accepts the narrower reading and `agent_surface.count_scope` then reports `page` instead of `matched`. A top-k bound is never refused. The same knob, plus `--sort`, `--dedupe-by` and `--max-output-bytes`, is refused on `export` and `ingest`, which emit one record per line. After a write with no result array the knob is suppressed rather than honoured, and `count_only_suppressed` says so. `export` and `embedding list` now declare their query ceiling, so the surface stops reporting `query_limited: null` over a page. Inherited from v1.2.8: the canonical relation vocabulary is kebab-case (`applies-to`, `depends-on`, `tracked-in`) in ONE place — `parsers::CANONICAL_RELATIONS` — and `create_or_fetch_relationship` / `upsert_relationship` canonicalise at the persistence boundary, so no write path can store a divergent spelling. Relation filters in `related` match both spellings, reaching rows written by earlier binaries. `link` accepts `--strength` as an alias of `--weight`. `remember` reports in `warnings` every declared `entity_type` outside the canonical thirteen — until V017 that label was folded onto a canonical kind, and since V017 it is stored as written. Enrichment no longer overwrites an entity type that is not the generic `concept`. `related` is deterministic: its ordering is total, so identical invocations return identical results. Schema is **v17** — the V017 migration opened the entity-type vocabulary and `health --json` reports `schema_version: 17`; `DEFAULT_EMBEDDING_DIM=1024`.
> **v1.2.7:** ten global agent-native flags (`--select`/`--fields`, `--filter`, `--max-items`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`, `--filter-scope`, `--allow-unknown-keys`; failure envelopes are never filtered) plus `--no-input`. An unresolvable key or a predicate over a truncated page is refused with exit 2 instead of answering empty, and the failure envelope names the dropped arguments in `discarded_flags`. Every envelope of a process that resolved a database reports `db_path_source` and `db_path_resolved` inside the `agent_surface` block, with no flag required. A subcommand that changes durable state exits 2 without touching anything when NOTHING named its target — no `--db` and no `db.path` in the configuration. An XDG target is permitted, because `config set db.path` is a designation made once rather than per invocation; `--use-active` accepts the compiled default on purpose. Schema was **v16** at that release; V017 advanced it to **v17**, which is what a 1.2.8 binary reports; `DEFAULT_EMBEDDING_DIM=1024`. Per-release detail lives in [CHANGELOG.md](CHANGELOG.md).


> Read this document in [Portuguese (pt-BR)](INTEGRATIONS.pt-BR.md)


> 27 AI agents and 20+ platforms in a single CLI contract (21 catalogued + 6 community)

- Read the Portuguese version at [INTEGRATIONS.pt-BR.md](INTEGRATIONS.pt-BR.md)
- Every recipe below is ready to copy and costs nothing to run
- **v1.0.79: every build is LLM-only and one-shot.** Embedding generation delegates to a headless `claude code` or `codex` subprocess (OAuth). The daemon, the ONNX runtime and the `embedding-legacy` feature were fully removed; embeddings are batched, parallel (`--llm-parallelism`). **Current default dimensionality is 1024** since v1.2.0 (`--embedding-dim` / XDG `embedding.dim`, range [8, 4096]; historical G42/G44 defaults 64/384 retired for new DBs).


## CLI Flag Aliases (since v1.0.35)
- `recall` and `hybrid-search` accept `--limit` as an alias of `-k`/`--k`. Existing examples below use `--k` and remain valid.
- `rename` accepts `--from`/`--to` as aliases of `--name`/`--new-name` (legacy `--old`/`--new` also remain valid).
- All `schema_version` JSON fields (`init`, `stats`, `migrate`, `health`) are emitted as JSON numbers (was string in `init`/`stats`/`migrate` before v1.0.35).
- Auto-init via `remember`/`ingest`/etc. now activates `journal_mode = wal` correctly (regression fix).

## New Flags (since v1.0.45)
- NER entity extraction is **disabled by default**. Pass `--enable-ner` on `remember` or `ingest` to opt in; there is no XDG key and no environment override for it.
- `--skip-extraction` is deprecated and has no effect since v1.0.45 (NER is off by default); the flag is kept as a hidden no-op for backwards compatibility — remove it from scripts.
- `--graph-stdin` on `remember` reads a single JSON object from stdin containing `body`, `entities`, and `relationships`, making it the preferred way to supply curated graphs from an LLM.

## New Flags (since v1.0.47)
- The GLiNER zero-shot NER pipeline was REMOVED in v1.0.79 with the `ner-legacy` feature; `--enable-ner` now performs URL-regex extraction only.
- --gliner-variant was REMOVED in v1.1.02: clap REJECTS it with exit 2, so an invocation carrying it aborts before any work — it is NOT tolerated as a silent no-op. The product env vars `SQLITE_GRAPHRAG_GLINER_VARIANT` and `SQLITE_GRAPHRAG_GLINER_THRESHOLD` are historical as well: product env is not read at runtime and has no effect.
- For LLM-curated entity/relationship extraction run a SEPARATE `enrich --mode openrouter` pass after `ingest --mode none`.
- Entity types now include `organization`, `location`, `date` alongside `person`, `project`, `tool`, `file`, `concept`, `decision`, `incident`, `dashboard`, `issue_tracker`, `memory`.

## New Commands and Flags (since v1.2.5)

- Crate **`1.2.5`**; pin library consumers `=1.2.5`. Additive agent-native output surface plus `--no-input`; no envelope change when no flag is set. Main-DB schema stays at **v16** (no migrate; sidecar queue behaviour only). July 2026 enrich-queue **CAPA seal**.
- **Namespace claim isolation** — `dequeue_next_pending`, `count_eligible_pending`, `--resume` / `--retry-failed` filter by `operation` **and** `namespace`. An enrich drain for `ai-sdd` no longer claims or counts `global` / empty-ns rows (reduces cross-namespace HardFailure / circuit-breaker risk).
- **`--until-empty` counts only this op+namespace** — `count_eligible_pending` no longer sums *all* pending rows across operations (alien ReEmbed zombies no longer keep EntityDescriptions spinning with `completed=0`).
- **`--force-redescribe` reopens `skipped`/`done`** — `reopen_force_redescribe_candidates` runs once per process before first enqueue so `INSERT OR IGNORE` is not a silent no-op; never reopens `dead` (use `--requeue-dead`).
- **Re-embed zombie reconciliation** — `reconcile_satisfied_reembed_pending` marks pending ReEmbed rows `done` when a live vector already exists at the active dim (`LENGTH(embedding) = dim*4`), clearing zombies without API calls.
- **Re-embed eligibility uses BLOB length** — scan/predicates select CORRUPT / META_AHEAD rows (`dim=1024` with a 384-d BLOB still eligible when `LENGTH(embedding) ≠ target_dim * 4`).
- **Enqueue validates re-embed keys** — `entity:{name}` strips the prefix for entity lookup (queue key stays `entity:…`; bare names still work; missing entities rejected). Chunk keys validate `chunk_id` exists in a non-deleted memory of the target namespace.
- **CAPA-D low-quality markers** — bare `%configuration file%` removed; compound markers only (e.g. `is a configuration file`) so legitimate domain prose is not force-redescribe fodder.
- Regressions: `enqueue_candidate_accepts_entity_prefixed_reembed_key`, `dequeue_next_pending_isolates_by_namespace`; queue unit suite 38 tests OK.

## New Commands and Flags (since v1.2.0)

- Crate **`1.2.0`**; pin library consumers `=1.2.0`. Main-DB schema stays at **v16** (no migrate if already on v16; sidecar queue migrate only).
- **DEFAULT_EMBEDDING_DIM=1024** — flag `--embedding-dim` / XDG `embedding.dim` still override; existing DBs keep `schema_meta.dim` until re-embed.
- Config precedence: **CLI flag > XDG `config set` > named default**. Product env `SQLITE_GRAPHRAG_*` is **not** the config path (ignored at runtime).
- `enrich --list-skipped` / `enrich --requeue-skipped` — recover `preservation_failed` / skipped sink without raw SQL (G-PR-3/4).
- Enrich queue multi-namespace — column `namespace` + `UNIQUE(namespace, operation, item_key)`; scoped `DELETE`; SQL `status='pending'` fix.
- **GAP-SG-139** — host/XDG leaves (`config`×9, `slots`×3, `cache`×3, `completions`) accept `--db` as a documented **no-op** (`src/cli_db_noop.rs`); graph surfaces unchanged.
- UX aliases: `pending-embeddings status` (= `embedding status`), `cache stats` (= `cache list`); `purge --now` (alias of `--retention-days 0`); `config list --effective` (dim/log defaults from `constants`, no hard-coded `"384"`).
- Offline seal: `scripts/e2e_offline_v120.sh` (historical wrapper `e2e_offline_v118.sh` superseded; **20/20** on release binary 1.2.0).

## New Commands and Flags (since v1.1.06)

- Official release name **v1.1.06**; crate `version = "1.1.6"` — pin `=1.1.6`. No schema migration (v16). Closes **GAP-ENTITY-CONNECT-SCAN-CARTESIAN**: O(k) entity-connect scan (co-occurrence + hub×island), queue keys `pair:{id1}:{id2}`, `item_type=entity_pair`, first-scan deadline via `InterruptHandle` (Timeout exit 1 ≠ 75), NDJSON `scan_start` / `scan_meta` with real `operation` plus dual backlog fields `backlog_degree0_proxy` and `pairs_enqueued_this_scan`. ADR-0066; suite `tests/v1106_entity_connect_scan_regression.rs`.
- `enrich --operation entity-connect|cross-domain-bridges` is safe on large `global` namespaces (no cartesian hang); both share the fully-implemented `entity_connect_seen` path.

## New Commands and Flags (since v1.1.05)
- The official release name is v1.1.05; the crate manifest carries `version = "1.1.5"` because the SemVer parser rejects a leading zero in the patch component — pin with `=1.1.5`. No schema migration (schema stays at v16 from v1.1.04). Closes the five operator-blocking bugs from the 2026-07-08 single-subject deep-research incident (`gaps.md`): agent pipeline safety via `deep-research --output PATH` (atomic write + stdout ack with `blake3`), global `--quiet`/`-q`, single-token aspect fan-out for deep-research, `graph traverse --fuzzy` with NotFound name suggestions, `link --from-id`/`--to-id` (pure-numeric names rejected), and `merge-entities` self-ref rejection before any DB work. Never mix stderr into JSON with `&>`.
- `deep-research --output PATH` — atomwrite (tempfile same dir → fsync → rename); short stdout ack `{written, bytes, blake3, ...}`; use with `--quiet` in headless agent pipelines
- `deep-research --sub-query-strategy` / `--sub-queries-file` — manual strategy; single-token queries expand to multi-aspect sub-queries (`source: "aspect"`, EN/PT facets)
- Global `--quiet` / `-q` — suppress non-error tracing on stderr so stdout JSON stays parseable
- `graph traverse --fuzzy` — auto-resolve clear short-name winners; without `--fuzzy`, NotFound includes ranked Jaro-Winkler / prefix suggestions
- `link --from-id` / `--to-id` — resolve entities by ID; pure digit names rejected by `validate_entity_name` so `--create-missing` cannot create ghost numeric entities
- `merge-entities` — rejects self-referential merges (`--into-id` in `--ids`, or `--into` in `--names`) BEFORE any DB work

## New Commands and Flags (since v1.1.02)
- The official release name is v1.1.04; the crate manifest carries `version = "1.1.4"` because the SemVer parser rejects a leading zero in the patch component — pin with `=1.1.4`. The HTTP `User-Agent` is `sqlite-graphrag/1.1.4` (derived from `CARGO_PKG_VERSION`); the release binary is approximately 19 MiB. v1.1.04 closes the two structural gaps tracked in `gaps.md` after v1.1.03: GAP-001 (the `deep-research` nested-Tokio-runtime panic is fixed — the sync entry point computes per-sub-query embeddings before building its dedicated runtime, and the three OpenRouter embedding paths adopt the `Handle::try_current` + `block_in_place` reentry pattern) and GAP-002 (`entity-connect` now converges via the new `entity_connect_seen` table recording the LLM verdict per pair). Database migration REQUIRED: `migrate --json` applies V016 (`entity_connect_seen`); schema advances v15→v16
- The official release name is v1.1.02; the crate manifest carries `version = "1.1.2"` because the SemVer parser rejects a leading zero in the patch component — pin with `=1.1.2`. The HTTP `User-Agent` is `sqlite-graphrag/1.1.2` (derived from `CARGO_PKG_VERSION`); the release binary is approximately 19 MiB; the schema stays at version 15 with no migration. v1.1.02 closes the two residual gaps left after v1.1.01 (the deprecated --gliner-variant argument is dropped from `remember`/`ingest` with clap exit 2; the embedding token ceiling becomes the typed `AppError::TooManyTokens { tokens, limit }` enforced at the write boundary), ships a regression test for the re-embed entity dispatch (tests/reembed_entities_integration.rs), and adds `enrich --prune-dead-entity-orphans` to prune entity-keyed dead-letter rows from the queue sidecar
- Entity vectors are written through the OpenRouter REST path even under `--llm-backend none` (the entity-embedding chain resolves to `[OpenRouter]`, no subprocess), and an empty-embedding guard in `upsert_entity_vec`/`upsert_chunk_vec`/`memories::upsert_vec` keeps vector-less rows visible to the re-embed backfill instead of masking them behind an empty BLOB (P1)
- `enrich --operation re-embed --target memories|entities|chunks|all` — retroactive embedding backfill per vector table (default `memories`, fully retro-compatible); `enrich --status` reports the `scan_backlog` per target; the re-embed predicates also select rows whose stored `dim` diverges from the configured `--embedding-dim` or whose blob is empty (P2, P10)
- `graph recompute-degree` — reconciles the cached `entities.degree` with the real edge counts in one IMMEDIATE transaction, per namespace (or all), with `--dry-run` and the envelope `{namespace, dry_run, total, updated, zeroed, unchanged, elapsed_ms}` (P3)
- `reclassify-relation --literal-from <REL>` — matches the stored relation VERBATIM (no hyphen→underscore normalization at the clap boundary), making legacy hyphenated edges such as `applies-to` reachable; mutually exclusive with `--from-relation` (P4)
- `merge-entities --ids <a,b> --into-id <N>` and `rename-entity --id <N>` — ID-based, namespace-scoped selection for entity maintenance when duplicated names across namespaces block merges and renames (P5)
- `health --json` gains `vec_memories_missing`, `vec_entities_missing`, `vec_chunks_missing` and the per-table `vec_*_coverage_pct` fields; `embedding status --json` gains the per-table `*_missing` counters (P6)
- `EntityType` deserialization is a manual `Deserialize` with a rich boundary error listing the 13 valid values, surfaced as a Validation error (exit 1) with early validation of curated graph input (`--graph-stdin`, `--entities-file`) instead of a bare serde error (exit 20) (P7)
- The exit-6 limit errors are typed: `AppError::BodyTooLarge { bytes, limit }` and `AppError::TooManyChunks { chunks, limit }` replace the generic `LimitExceeded` message at every body-size call site — the exit CODE stays 6, only the envelope MESSAGE gains actionable context (P11)
- `ingest --name-prefix <PREFIX>` — kebab-case prefix applied to every derived memory name, with the derived-part budget shrunk so `prefix + derived` always respects the 80-char name cap (P12)

## New Commands and Flags (since v1.0.94)
- `--embedding-backend auto|openrouter|llm` — select embedding backend (global flag)
- `--embedding-model MODEL` — select embedding model for OpenRouter (global flag, REQUIRED with openrouter)
- `--openrouter-api-key KEY` — API key for OpenRouter (global flag)
- `--enrich-after` — run enrich after ingest completes (ingest flag)
- **GAP-OR-PROPAGATION**: All 13 embedding paths now honour `--embedding-backend` — including `enrich`, `init`, `rename-entity`, `ingest --mode claude-code`, `remember` (chunks)
- Exit code 78 (`EX_CONFIG`) for OpenRouter config errors (missing API key, missing model, invalid key)
- 10 models verified E2E with dim=64 MRL: `google/gemini-embedding-001` (0.892), `google/gemini-embedding-2` (0.868), `mistralai/mistral-embed-2312` (0.832), `qwen/qwen3-embedding-8b` (0.814), `qwen/qwen3-embedding-4b` (0.754), `openai/text-embedding-3-small` (0.668), `nvidia/llama-nemotron-embed-vl-1b-v2:free` (0.662), `baai/bge-m3` (0.537), `openai/text-embedding-3-large` (0.449), `perplexity/pplx-embed-v1-0.6b` (0.415)

## New Commands and Flags (since v1.0.95)
- `enrich --mode openrouter` — new opt-in mode routing the JUDGE step to OpenRouter `/chat/completions` REST (no local CLI); the four modes are now `claude-code`, `codex`, `opencode`, `openrouter` (GAP-OR-ENRICH, ADR-0054)
- `--openrouter-model MODEL` — REQUIRED with `--mode openrouter`; omitting it exits 1 before any network call
- `--openrouter-api-key KEY` — API key for the chat client (XDG via `config add-key` (OPENROUTER_API_KEY is not read at runtime)); reuses the embedding-backend key with the same `secrecy`/zeroize handling
- `--openrouter-timeout SECS` — chat request timeout (default 300s)
- `--openrouter-base-url URL` — optional override of the OpenRouter base URL
- New module `src/chat_api.rs` (`OpenRouterChatClient`) mirrors `src/embedding_api.rs`; SCAN→JUDGE→PERSIST unchanged, only the JUDGE transport differs; 13/13 real models verified; no migration, schema v15

## New Commands and Flags (since v1.0.97)
- `enrich --requeue-dead` — moves terminal `dead` items back to `pending` for another pass (no in-place queue reset); `enrich --list-dead` — read-only JSON listing of each dead item with its `error_class` and `message`; `enrich --ignore-backoff` — dequeues eligible items immediately, bypassing the `next_retry_at` cooldown; `enrich --prune-dead-orphans` — read-only inspector (no LLM, no singleton) that deletes `dead` memory-type queue entries whose `item_key` no longer exists in the main database, leaving entity rows untouched (GAP-SG-66, ADR-0058)
- `enrich --status`, `--list-dead`, `--requeue-dead` and `--prune-dead-orphans` now run WITHOUT `--operation`/`--mode` (previously `--mode` was mandatory) — ideal for hook/timer integration
- `enrich --operation augment-bindings` — adds bindings to memories that are ALREADY linked; REQUIRES `--names <a,b,c>` or `--names-file <path>`. `enrich --operation body-extract --body-extract-graph-only` — read-only graph extraction without rewriting the body
- `--max-attempts` default raised to 8 (range 1..=20); `--openrouter-timeout` default raised to 600s
- `remember --graph-file <path>` — loads the entity graph from a file (combinable with `--body-file`); `remember --strict-name` — rejects a non-kebab name instead of normalizing; `remember --replace-graph` (with `--force-merge`) zeroes existing bindings before writing
- `ingest --force-merge` — updates duplicate files instead of skipping (dedup by `body_hash`); oversized bodies auto-split natively into chunks
- `read --format raw` — prints the pure body with no JSON envelope; `unlink --memory <name> --entity <name>` — removes a single curated memory-to-entity binding
- `embedding status --json` adds a `coverage` object (real vector counts per table); `stats --json` adds a top-level `total_memories`
- `--db <PATH>` must be placed AFTER the subcommand; no position-independent override exists, so the canonical alternative is the XDG key `db.path` via `config set` (SG-32). The per-namespace enrich singleton is unchanged, with `--rest-concurrency` (clamp 1..=16, default 8) as the throughput remedy (GAP-20)

## New Commands and Flags (since v1.0.96)
- `enrich --until-empty` — internal scan→drain loop that runs until the queue holds no eligible items or `--max-runtime` expires; replaces the external bash retry loop (GAP-ENRICH-BACKLOG-CONVERGE, ADR-0055)
- `--max-runtime <SECONDS>` — wall-clock ceiling for `--until-empty` (default 3600)
- `--max-attempts <N>` — Transient retry budget before an item becomes terminal `dead` (default 5, range 1..=20)
- `--status` — read-only JSON report of the queue counts (`unbound_backlog`, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`) plus the per-operation `scan_backlog` (the real database candidates a scan would enqueue, sharing the scanners' WHERE predicates so it never diverges from a real scan; GAP-SG-77 kills the false `pending=0` for `entity-descriptions`/`body-enrich`/`re-embed`, and `state` derives `pending-scan` from it); does NOT call the LLM and does NOT acquire the singleton — its deterministic output is ideal for hook/timer integration
- `--rest-concurrency <N>` — bounded REST fan-out for `--mode openrouter` embedding batches; clamp 1..=16 (default 8), distinct from `--llm-parallelism`
- Dead-letter convergence: the `.enrich-queue.sqlite` queue gains `error_class` and `next_retry_at` columns (idempotent ALTER TABLE) plus a terminal `dead` status; Transient failures (rate-limit/timeout/5xx) reschedule with exponential backoff, HardFailures (validation/parse) go terminal immediately, and dequeue excludes `dead` so the live set strictly decreases

## New Commands and Flags (since v1.0.68)
### Process Lifecycle (G28)
- `enrich` acquires a per-namespace singleton before doing real work.  A second concurrent invocation against the same database fails fast with `AppError::JobSingletonLocked { job_type, namespace }` (exit 75) instead of stacking up subprocess trees.
- Historical, removed in v1.2.0 — the `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` env var (opt-in) once pointed at an existing empty directory, and the Claude Code subprocess was spawned with `CLAUDE_CONFIG_DIR=<that dir>`, suppressing user-scoped MCP servers and their 8-10-process fan-out.  That was the only mechanism upstream Claude Code actually honoured (see [anthropics/claude-code#10787]).  We deliberately did NOT pass `--strict-mcp-config` or `--mcp-config '{}'` because both are ignored.  The headless subprocess backends are gone, so the mechanism no longer exists.
- `retry::CircuitBreaker` (Rust crate API) — opt-in helper with `AttemptOutcome::{Success, Transient, HardFailure}`.  Rate-limited and timeout errors are explicitly excluded from the failure count.  Use in custom retry loops to cap persistent-failure iterations.
- `enrich` emits a `tracing::warn!` (visible with `-v`) when `--llm-parallelism > 4`; the `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` remedy it once named was removed in v1.2.0, so lower `--llm-parallelism` instead.
### Windows Build (G29)
- `cargo install sqlite-graphrag` on Windows now succeeds.  `HANDLE` type is treated type-safely via `!handle.is_null() && handle != INVALID_HANDLE_VALUE`.  `windows-sys` is pinned to `=0.59.0` exact in `Cargo.toml`.  New CI job `windows-build-check` runs `cargo check --target x86_64-pc-windows-msvc --lib --all-features` on every push and PR.

## New Commands and Flags (since v1.0.69)
### OAuth-Only Enforcement (G28-A, G31, Behaviour Change)
- `claude -p` and `codex exec` spawns now ABORT with `AppError::Validation` if `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` are present in the environment.  OAuth (Claude Pro/Max or ChatGPT Pro) is the ONLY accepted credential flow.  See `docs/decisions/adr-0011-oauth-only-enforcement.md` for the full rationale.
- The `--bare` flag (which demands an API key and disables OAuth) is REMOVED from every executable path.  Both API key env vars are also excluded from the `env_clear` whitelist as defence in depth.
### `enrich` — New Subcommand (G29 + G35 + G37)
- `enrich --operation <op> --mode openrouter --json` runs LLM-curated graph quality. At introduction (v1.0.69) three ops shipped first: `memory-bindings`, `entity-descriptions`, `body-enrich` (G29 source CHECK + `memory_versions` audit). **Current FULLY-IMPLEMENTED set** also includes `re-embed`, `augment-bindings`, `body-extract`, `entity-connect` (v1.1.04 `entity_connect_seen` + **v1.1.06** O(k) co-occurrence+hub×island, keys `pair:{id1}:{id2}` / `entity_pair`, drain by PK, first-scan InterruptHandle → Timeout exit **1** ≠ 75, NDJSON `scan_start`/`scan_meta`), and `cross-domain-bridges` (same O(k) path). See “New Commands and Flags (since v1.1.06)” above.
- `--preserve-threshold <FLOAT>` (default 0.7) controls the Jaccard trigram preservation gate from `src/preservation.rs` (10 tests).  Scores below the threshold are rejected and emitted as `EnrichItemResult::PreservationFailed`.
- `--preflight-check` and `--rate-limit-buffer <SECONDS>` (default 300) guard a long run: the preflight probe confirms the OpenRouter key resolves before scanning N candidates.
- `--names <a,b,c>` and `--names-file <PATH>` select a specific subset of memory names.  `--names-file` accepts `#` comments and blank lines.  Both flags combine as a union.
- `--llm-parallelism <N>` warning is conditional to the mode: Claude warns at 5 (OAuth-MCP fan-out), Codex warns at 17 (rate-limit risk), Codex 5..16 is silent (validated at 1161 items, 0 failures in production).
- `--max-load-check` refuses to start when load average > `2 × ncpus`.  `--circuit-breaker-threshold <N>` (default 5) aborts after N consecutive `HardFailure` outcomes.
### `vec` Subcommand Family (G39)
- `vec orphan-list --json` lists orphan memory embedding rows with `vector_hash` (BLAKE3 of the embedding blob).
- `vec purge-orphan --yes --dry-run --json` previews the deletion.  `vec purge-orphan --yes --json` purges the THREE vec tables (`vec_memories`, `vec_entities`, `vec_chunks`) in a single transaction.
- `vec stats --json` exposes `vec_memories_rows`, `vec_entities_rows`, `vec_chunks_rows`, `orphans`, and the last vacuum timestamp.
- `forget` now calls `memories::delete_vec` BEFORE the soft-delete, preventing new orphans in the steady state.

## New Commands and Flags (since v1.0.76)
### LLM-Only One-Shot Architecture (G21 + G22 + G23 + G24 + G25)
- The default build of v1.0.76 is LLM-Only and one-shot.  No daemon, no ONNX runtime, no `multilingual-e5-small` model download.  Embedding generation and NER delegate to a headless `claude code` or `codex` subprocess (OAuth, no MCP, no hooks).  Release binary is approximately 6 MB.
- The `embedding-legacy` feature was REMOVED in v1.0.79 (ahead of the v1.1.0 schedule).  The legacy fastembed + ort + tokenizers pipeline no longer exists; every build is LLM-only.
- See ADR-0019, ADR-0020, ADR-0021, ADR-0022, ADR-0023, ADR-0024, ADR-0025, ADR-0026 for the full architectural decisions.
### `migrate` Subcommand Family (v1.0.76)
- `migrate --rehash --json` rewrites recorded migration checksums to match the current file content.  Algorithm matches `refinery-core 0.9.1` (SipHasher13, same hashing order).  Required for v1.0.74 → v1.0.76 upgrades where V002 was intentionally emptied to a no-op.  Response schema: `migrate-rehash.schema.json`.
- `migrate --to-llm-only --drop-vec-tables --json` is the one-shot upgrade for v1.0.74 / v1.0.75 databases: rehash + V013 vec-table drop + vec-table state report.  The `--drop-vec-tables` flag is REQUIRED as a safety guard.  Response schema: `migrate-to-llm-only.schema.json`.
### BLOB-Backed Embedding Tables (G22)
- V013 migration drops the `vec_memories`, `vec_entities`, `vec_chunks` virtual tables and replaces them with regular BLOB-backed `memory_embeddings`, `entity_embeddings`, `chunk_embeddings` tables.  Cosine similarity is computed in pure Rust on demand in `src/similarity.rs` (ADR-0020, ADR-0022).
### Hybrid Search Refinement (G24)
- `hybrid-search` uses FTS5 for coarse filtering and refines the candidate set with a pure-Rust cosine over the BLOB embeddings.  FTS5 stays healthy because the rebuild is gated by `optimize --fts-skip-when-functional` (G36 from v1.0.69).
### Extraction Backend Selector
- HISTORICAL (subprocess era): the global flag --extraction-backend llm|embedding|none|both, default `llm`, selected the extraction backend — `llm` was the subprocess-backed path, `embedding` a permanent stub returning a migration error, `none` a no-op, `both` a parallel merge. The flag is GONE: a 1.2.8 binary answers `unexpected argument` with exit 2, and the subprocess extraction path it selected was removed in v1.2.0. Today extraction is chosen per command — `ingest --mode none` for body-only ingestion, then a SEPARATE `enrich --mode openrouter` pass for LLM-curated entities and relationships.
- `src/extract/` still exposes the `ExtractionBackend` trait with the four implementations, and none of them spawns a process any more. HISTORICAL: `src/spawn/` exposed the `VersionAdapter` trait with `CodexAdapter` (detected `codex 0.130.0` through `0.138+` and adapted flags — `codex 0.137.0` removed `--ask-for-approval` in favour of `-a never`), `ClaudeAdapter` (claude code 2.1.0+) and `OpencodeAdapter` (opencode headless). That directory does not exist in the 1.2.8 tree: the three headless backends were removed in v1.2.0.
### Daemon Removal (ADR-0021)
- The `daemon` subcommand was DEPRECATED in v1.0.76 and FULLY REMOVED in v1.0.79 (ahead of the v1.1.0 schedule).  The LLM subprocess is the "model loader"; the CLI is 100% one-shot with zero IPC.

## New Commands and Flags (v1.0.79 — G42 embedding pipeline)
- `--embedding-dim <N>` global flag sets the embedding dimensionality (default **1024**, range [8, 4096]); precedence: flag > XDG `embedding.dim` > the `dim` recorded in `schema_meta` > 1024; existing 384-dim databases keep working via recorded dim
- `--llm-parallelism <N>` is now available on `remember` (default 4), `ingest` (default 2) and `edit` — bounded fan-out via `Semaphore` + `JoinSet`, permits clamp [1, 32]
- `enrich --operation re-embed --limit N --resume` is the canonical one-shot re-embed path (e.g. after changing `--embedding-dim`)
- `edit --force-reembed` regenerates the embedding of one memory without changing its body
- Historical, removed in v1.2.0 — `SQLITE_GRAPHRAG_CLAUDE_EMBED_MODEL` overrode the claude embedding model (symmetric to the codex variable); the embedding deadline is now `--openrouter-timeout <SECONDS>` or the XDG key `embedding.timeout_secs` (default 300)
- LLM calls are batched (`{items:[{i,v}]}` schema — calibration bases of 8 chunks / 25 entity names at dim 64, dim-adaptive as clamp(base×64/dim, 1, base) since G44) and every subprocess uses `kill_on_drop` plus an explicit timeout

## New Commands and Flags (since v1.0.67)
- `remember-batch` batch-creates memories from NDJSON stdin in a single invocation; `--transaction` for atomicity, `--force-merge` for idempotent updates, `--fail-fast` to stop on first error
- `completions` generates shell completions for Bash, Zsh, Fish, PowerShell, and Elvish
- `read --id <N>` fetches a memory by integer `memory_id` directly (bypasses name resolution)
- `read --with-graph` includes linked entities and relationships in the JSON response
- `enrich --llm-parallelism <N>` spawns N parallel LLM worker threads (default 1, max 32)
- `health` detects super-hub entities (degree > 50) and reports `super_hub_count`, `top_hub_entity`, `top_hub_degree`
- `health` reports `non_normalized_count` and `normalization_warning` for entities not matching kebab-case
- `edit` skips re-embedding when body content is unchanged (body_hash comparison)
- `rename` purges ghost soft-deleted memories occupying the target name before UPDATE
- `hybrid-search` and `recall` reject `--max-hops` and `--min-weight` when graph traversal is disabled
- V012 migration adds `created_at`/`updated_at` timestamps to relationships table

## New Commands and Flags (since v1.0.66)
- `edit --type` changes memory type without re-creating the memory
- `deep-research` `graph_context` field in JSON response with entities and relationships from result memories
- `graph --format json` includes `entities` alias alongside `nodes` for LLM agent compatibility
- `list --json` includes `memories` alias alongside `items` for LLM agent compatibility
- `graph entities --json` includes `description` field per entity
- `health --json` includes `vec_memories_missing` and `vec_memories_orphaned` counts

## New Commands and Flags (since v1.0.65)
- `reclassify-relation --from-relation <old> --to-relation <new> --batch` renames relationship types in bulk; single mode via `--source`/`--target`; handles UNIQUE collisions via `UPDATE OR IGNORE` + `DELETE`; `--dry-run` previews; optional `--filter-source-type`/`--filter-target-type`
- `normalize-entities --yes` normalizes all entity names to lowercase kebab-case ASCII; auto-merges collisions; `--dry-run` previews
- `enrich --operation <op> --mode claude-code|codex|opencode|openrouter` LLM-augmented graph quality; current FULLY-IMPLEMENTED ops: `memory-bindings`, `entity-descriptions`, `body-enrich`, `re-embed`, `augment-bindings`, `body-extract`, `entity-connect` (v1.1.06 O(k) + pair keys), `cross-domain-bridges`; `--dry-run` previews without LLM; `--max-cost-usd`, `--resume`, `--retry-failed`, `--until-empty`, `--max-runtime`
- `deep-research` new flags: `--rrf-k` (default 60), `--graph-decay` (default 0.7), `--graph-min-score` (default 0.05)), `--max-neighbors-per-hop`
- --max-entity-degree flag REMOVED from `link` and `remember` in v1.0.99 — writes are now purely additive and NEVER prune, delete edges, or emit a degree warning (passing the flag now yields a clap exit 2)
- `health` reports `top_relation`, `top_relation_ratio`, `applies_to_ratio`, `relation_concentration_warning` when any relation exceeds 40%
- Entity names are normalized to lowercase kebab-case on every write path (remember, ingest, link, rename-entity)

## Daemon Behavior (HISTORICAL — daemon removed in v1.0.79)
- v1.0.50 through v1.0.78 only: the CLI auto-restarted the daemon on version mismatch.  Since v1.0.79 there is no daemon process at all

## New Commands and Flags (since v1.0.56)
- `fts rebuild` rebuilds the FTS5 full-text search index from scratch
- `fts check` runs FTS5 integrity-check without modifying the index
- `fts stats` shows FTS5 index statistics (row count, shadow pages, functional status)
- `backup --output <path>` creates a safe database copy via SQLite Online Backup API
- `delete-entity --name <entity> --cascade` deletes entity and cascades to all relationships and NER bindings
- `reclassify --name <entity> --entity-type <new>` changes entity type; `--from-type <old> --to-type <new> --batch` for bulk
- `merge-entities --names "a,b,c" --into <target>` merges source entities into target, moving all edges
- `rename-entity --name <old> --new-name <new>` renames a graph entity preserving all FK-based relationships and re-embeds for semantic search
- `memory-entities --name <memory>` lists entities linked to a specific memory
- `prune-ner --entity <name>` or `--all --yes` removes NER bindings from memory_entities table
- `cleanup-orphans --dry-run --json` audits entities with zero memories and zero relationships; `--yes` removes them
- `prune-relations --relation <type> --dry-run --json` previews bulk removal of all relationships of a given type; `--yes` executes
- `remember --dry-run` validates input and reports planned actions without persisting
- `remember --clear-body` explicitly clears body during `--force-merge` (empty body now preserves existing by default)
- `remember --type` and `--description` are now optional with `--force-merge` (inherited from existing memory)
- `list` default limit is all memories with `--json`, 50 for text; response includes `total_count`, `truncated`, `body_length`
- `history --diff` includes character-level change summary between consecutive versions
- `hybrid-search` graceful FTS5 degradation: `fts_degraded`, `fts_error`, `fts_auto_rebuilt` fields; auto-rebuilds on corruption
- `hybrid-search` adds `normalized_score` (0-1), `vec_distance`, `fts_bm25` raw scores
- `health` adds `fts_query_ok` (functional FTS5 MATCH test), `sqlite_version`
- `optimize --skip-fts` skips FTS5 rebuild; `fts_rebuilt` field in response
- `link --strict-relations` rejects non-canonical relation types; `warnings` field in response
- `unlink --relation` is now optional (removes all between pair); `--entity <name> --all` for bulk
- `graph entities --sort-by degree|name|created_at --order asc|desc`; `degree` field in response
- `ingest --max-name-length N` configures name truncation; `body_length` in NDJSON; auto-prefix `doc-` for numeric names
- daemon --ping added `model_name`, `model_variant` fields (HISTORICAL — the daemon was removed in v1.0.79)
- ALL error paths now emit JSON on stdout: `{"error": true, "code": N, "message": "..."}`
- FTS5 sync fixed in `edit`, `rename`, `restore` — edited memories are now immediately findable via full-text search


## Summary Table
### Catalog — Every Supported Integration
| Name | Type | Minimum Version | Example | Official Docs |
| --- | --- | --- | --- | --- |
| Claude Code | AI Agent | 1.0+ | `sqlite-graphrag recall "query" --json` | https://docs.anthropic.com/claude-code |
| Codex CLI | AI Agent | 0.5+ | `sqlite-graphrag remember --name X --type user --body "..."` | https://github.com/openai/codex |
| Gemini CLI | AI Agent | any recent | `sqlite-graphrag hybrid-search "query" --k 5 --json` | https://github.com/google-gemini/gemini-cli |
| Opencode | AI Agent | any recent | `sqlite-graphrag recall "auth flow" --json` | https://github.com/opencode-ai/opencode |
| OpenClaw | AI Agent | any recent | `sqlite-graphrag list --type user --json` | community project |
| Paperclip | AI Agent | any recent | `sqlite-graphrag read --name note --json` | community project |
| VS Code Copilot | AI Agent | 1.90+ | tasks.json | https://code.visualstudio.com/docs/copilot |
| Google Antigravity | AI Agent | any recent | `sqlite-graphrag hybrid-search "prompt" --json` | Google Antigravity docs |
| Windsurf | AI Agent | any recent | `sqlite-graphrag recall "refactor plan" --json` | https://windsurf.com/docs |
| Cursor | AI Agent | 0.40+ | `sqlite-graphrag remember --name cursor-ctx --type project --body "..."` | https://cursor.com/docs |
| Zed | AI Agent | any recent | `sqlite-graphrag recall "open tabs" --json` | https://zed.dev/docs |
| Aider | AI Agent | 0.60+ | `sqlite-graphrag recall "refactor" --k 5 --json` | https://aider.chat |
| Jules | AI Agent | preview | `sqlite-graphrag stats --json` | https://jules.google |
| Kilo Code | AI Agent | any recent | `sqlite-graphrag recall "tasks" --json` | community project |
| Roo Code | AI Agent | any recent | `sqlite-graphrag hybrid-search "repo ctx" --json` | community project |
| Cline | AI Agent | VS Code ext | `sqlite-graphrag list --limit 20 --json` | https://cline.bot |
| Continue | AI Agent | VS Code or JetBrains | `sqlite-graphrag recall "docstring" --json` | https://docs.continue.dev |
| Factory | AI Agent | any recent | `sqlite-graphrag recall "pr context" --json` | https://factory.ai |
| Augment Code | AI Agent | any recent | `sqlite-graphrag hybrid-search "review" --json` | https://docs.augmentcode.com |
| JetBrains AI Assistant | AI Agent | 2024.2+ | `sqlite-graphrag recall "stacktrace" --json` | https://www.jetbrains.com/ai |
| OpenRouter | AI Router | any | `sqlite-graphrag recall "rule" --json` | https://openrouter.ai/docs |
| POSIX Shells | Shell | any | `sqlite-graphrag recall "$query" --json` | https://www.gnu.org/software/bash |
| Nushell | Shell | 0.90+ | `^sqlite-graphrag recall "query" --k 5 --json \| from json \| get results` | https://www.nushell.sh/book |
| Local cron/systemd/launchd/Task Scheduler | Ops | any | local one-shot | (no cloud CI) |
| GitLab CI | CI/CD | any | `.gitlab-ci.yml` | https://docs.gitlab.com/ee/ci |
| CircleCI | CI/CD | any | `.circleci/config.yml` | https://circleci.com/docs |
| Jenkins | CI/CD | 2.400+ | Jenkinsfile | https://www.jenkins.io/doc |
| Docker and Podman Alpine | Container | any | Dockerfile | https://docs.docker.com |
| Kubernetes | Orchestrator | 1.25+ | Job or CronJob | https://kubernetes.io/docs |
| Scoop and Chocolatey | Package Manager | Windows | `scoop install sqlite-graphrag` (planned) | https://scoop.sh and https://chocolatey.org |
| Nix and Flakes | Package Manager | any | `nix run .#sqlite-graphrag` | https://nixos.org |


## Claude Code
### Anthropic Agent — Subprocess Integration
- Recipe ready to copy into `.claude/hooks/`, zero cloud cost, memory stays on your machine
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to persist context across Claude Code sessions without external memory services
- Use `sqlite-graphrag recall "$USER_PROMPT" --k 5 --json` in a pre-task hook to inject context
- Minimum version requires Claude Code 1.0 or later for stable `.claude/hooks/` directory support
- Official docs live at https://docs.anthropic.com/claude-code describing hook lifecycle events
- Golden tip is to capture exit code `75` as retry-later and keep the agent alive gracefully
- HISTORICAL (v1.0.61 until v1.2.0): `ingest --mode claude-code` used the Claude Code binary for LLM-curated entity/relationship extraction during bulk ingestion, spawning `claude -p` headless per file against a Pro/Max subscription. A 1.2.8 binary REFUSES it — `invalid value 'claude-code' for '--mode <MODE>'`, exit 2 — because `none` is the only accepted value.
- Today the same result is two steps with no subprocess: `ingest --mode none` for the body, then a SEPARATE `enrich --mode openrouter --openrouter-model <MODEL>` pass that reaches the provider over HTTP.
- HISTORICAL: --claude-timeout <S> (default 300s) bounded that subprocess and is rejected with exit 2 today. Nothing spawns, so nothing can hang: the only time budget is the global `--openrouter-timeout <SECONDS>`, applied per OpenRouter REST call.


## Codex CLI
### OpenAI Agent — AGENTS.md Driven Subprocess
- Recipe ready to paste into `AGENTS.md` at repo root, zero cloud cost to activate
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to expose the memory contract through the native `AGENTS.md` convention
- Use `sqlite-graphrag recall "<query>" --k 5 --json` documented inside `AGENTS.md` at repo root
- Minimum version requires Codex CLI 0.5 or later for deterministic AGENTS.md parsing rules
- Official docs live at https://github.com/openai/codex covering AGENTS.md discovery order
- Golden tip is to include a working invocation example under each listed command for Codex
- HISTORICAL (v1.0.62 until v1.2.0): `ingest --mode codex` used the Codex CLI binary for LLM-curated entity/relationship extraction during bulk ingestion, spawning `codex exec --json` headless per file against a ChatGPT OAuth session. A 1.2.8 binary REFUSES it — `none` is the only accepted value of `--mode`, exit 2 otherwise.
- Today the recipe is `ingest --mode none` followed by a SEPARATE `enrich --mode openrouter --openrouter-model <MODEL>` pass over HTTP; the `AGENTS.md` contract above is unaffected, because it only documents read verbs.
- HISTORICAL: --codex-timeout <S> (default 300s) bounded that subprocess and is rejected with exit 2 today. The only remaining time budget is the global `--openrouter-timeout <SECONDS>`.

> **Authentication (current):** there is no subprocess and no OAuth flow. Embedding and enrichment are HTTP calls to OpenRouter with a key stored at rest under XDG by `config add-key --provider openrouter --from-stdin`; `config doctor` shows which layer resolved it.
> **Historical:** until v1.2.0 OAuth was the only accepted flow and API keys were refused — `--mode claude-code` read `~/.claude/.credentials.json` (Claude Pro/Max/Team) and `--mode codex` read the device auth of `codex login` (OpenAI ChatGPT). Both modes are rejected by a 1.2.8 binary.
> Defining `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` in the environment ABORTS the spawn with `AppError::Validation` and exit code 1. The `--bare` flag (which would also demand an API key) is REMOVED from all executable code paths.
> See `docs/decisions/adr-0011-oauth-only-enforcement.md` for the full rationale.

## Gemini CLI
### Google Agent — Subprocess With JSON Contract
- Recipe ready to copy into your Gemini CLI config, zero cloud cost, runs fully local
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to inject memory into Gemini 2.5 Pro prompts during long coding sessions
- Use `sqlite-graphrag hybrid-search "query" --k 5 --json` for recall with mixed keyword intent
- Minimum version supports any recent Gemini CLI release with subprocess invocation enabled
- Official docs live at https://github.com/google-gemini/gemini-cli for tool integration patterns
- Golden tip is to pass the global `--lang pt` flag, or persist `config set i18n.lang pt`, when prompting Gemini in Portuguese contexts


## Opencode
### Community Agent — Subprocess Integration
- Recipe ready to copy into the Opencode plugin hook, zero cloud cost, runs as subprocess
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to persist multi-turn context in the open source Opencode orchestration loop
- Use `sqlite-graphrag recall "$query" --json` as part of the Opencode pre-generation pipeline
- Minimum version supports any recent Opencode release exposing a plugin subprocess hook
- Official project lives at https://github.com/opencode-ai/opencode with community issue tracker
- Golden tip is to set the namespace to the repo slug to avoid cross-project memory leakage


## OpenClaw
### Community Agent — Subprocess Driver
- Recipe ready to drop into OpenClaw startup, zero cloud cost, memory is fully local
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to inject persistent memory into OpenClaw agent loops without plugin rebuild
- Use `sqlite-graphrag list --type user --json` to fetch seed context at the start of a run
- Minimum version supports any recent OpenClaw release able to shell out to CLI binaries
- Official docs live inside the OpenClaw GitHub README explaining subprocess integration rules
- Golden tip is to run the binary inside the target project folder and keep the default `graphrag.sqlite`


## Paperclip
### Community Agent — Subprocess Client
- Recipe ready to paste into Paperclip hook config, zero cloud cost, all memory stays local
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to persist cross-session memory in the Paperclip autonomous developer agent
- Use `sqlite-graphrag read --name onboarding-note --json` to seed the session with prior notes
- Minimum version supports any recent Paperclip release that can spawn child subprocess calls
- Official docs live in the Paperclip community repository describing subprocess hook contracts
- Golden tip is to run `health --json` at startup and abort when integrity reports any damage


## VS Code Copilot
### Microsoft Agent — tasks.json Integration
- Recipe ready to paste into tasks.json, zero cloud cost, recall fires from inside the editor
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to surface relevant memory from a selection inside VS Code Copilot chat panels
- Use the example tasks.json entry that calls `sqlite-graphrag recall "$selection" --json`
- Minimum version requires VS Code 1.90 or later for the latest tasks.json variable substitutions
- Official docs live at https://code.visualstudio.com/docs/copilot covering chat tool registration
- Golden tip is to bind the task to `Cmd+Shift+M` for single-keystroke memory recall invocation


## Google Antigravity
### Google Agent — Runner Integration
- Recipe ready to register as an Antigravity runner, zero cloud cost, binary is self-contained
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to run sqlite-graphrag as a first-class runner inside Antigravity pipelines at scale
- Use `sqlite-graphrag hybrid-search "$PROMPT" --json --k 10` as the retrieval step in a runner
- Minimum version supports any recent Antigravity release that accepts arbitrary runner binaries
- Official docs live on the Google Antigravity product page describing runner configuration format
- Golden tip is to run `sync-safe-copy` before each pipeline to guard the shared memory artifact


## Windsurf
### Codeium Agent — Terminal Integration
- Recipe ready to paste into a Windsurf Run task binding, zero cloud cost to activate recall
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to expose memory recall to Windsurf assistant panels via terminal task invocation
- Use `sqlite-graphrag recall "$EDITOR_CONTEXT" --json` mapped to a Windsurf Run task binding
- Minimum version supports any recent Windsurf release with terminal task execution enabled
- Official docs live at https://windsurf.com/docs describing the terminal task binding syntax
- Golden tip is to persist results to `/tmp/ng.json` so Windsurf prompt templates can read them


## Cursor
### Cursor Agent — Terminal Integration
- Recipe ready to drop into `.cursorrules` or a terminal binding, zero cloud cost, memory is local
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to pair Cursor AI with a local memory backend that survives editor restarts
- Use `sqlite-graphrag remember --name cursor-ctx --type project --body "$SELECTION"` from a key binding
- Minimum version requires Cursor 0.40 or later for stable AI rules and terminal env override
- Official docs live at https://cursor.com/docs covering AI rules and terminal integration patterns
- Golden tip is to pass `--namespace ${workspaceFolderBasename}` per project workspace, or persist `config set namespace.default <NAME>`


## Zed
### Zed Industries Agent — Assistant Panel Integration
- Recipe ready to add as a Zed task profile, zero cloud cost, runs from the built-in terminal
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to wire memory recall into the Zed assistant panel without custom extensions
- Use `sqlite-graphrag recall "open tabs" --json --k 5` as a terminal command available to Zed
- Minimum version supports any recent Zed release with the assistant panel and terminal tasks
- Official docs live at https://zed.dev/docs describing assistant panel and terminal integration
- Golden tip is to define a Zed task profile sharing memory across multiple open workspaces


## Aider
### Open Source Agent — Shell Integration
- Recipe ready to paste into your shell alias before `aider`, zero cloud cost, zero config server
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to augment Aider pair programming with durable memory across git repositories
- Use `sqlite-graphrag recall "refactor target" --k 5 --json` invoked before each Aider prompt
- Minimum version requires Aider 0.60 or later for stable subprocess and hook invocation
- Official docs live at https://aider.chat describing configuration and custom shell commands
- Golden tip is to scope memory by repository via `--namespace $(basename $(pwd))` on every invocation


## Jules
### Google Labs Agent — CI Automation
- Recipe ready to add as a Jules CI step, zero cloud cost, binary installs in seconds via cargo
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to run memory maintenance inside Jules preview automation pipelines automatically
- Use `sqlite-graphrag stats --json` as a CI step to monitor memory growth week over week
- Minimum version is the current Jules preview release available via Google Labs early access
- Official docs live at https://jules.google explaining CI job configuration and authentication
- Golden tip is to fail the pipeline when `stats.memories` exceeds agreed thresholds for a project


## Kilo Code
### Community Agent — Subprocess Integration
- Recipe ready to paste into Kilo Code startup hook, zero cloud cost, memory is a local file
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to expose a persistent memory layer to the Kilo Code autonomous engineering agent
- Use `sqlite-graphrag recall "recent tasks" --json` at the start of every Kilo Code agent run
- Minimum version supports any recent Kilo Code release capable of spawning child processes
- Official docs live in the Kilo Code community repository describing the subprocess contract
- Golden tip is to log exit code `75` as retryable rather than fatal when orchestrator is busy


## Roo Code
### Community Agent — Subprocess Integration
- Recipe ready to wire into Roo Code hook lifecycle, zero cloud cost, all data is local SQLite
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to inject memory into Roo Code agent prompts for deeper repository understanding
- Use `sqlite-graphrag hybrid-search "repo context" --json` for recall across mixed query types
- Minimum version supports any recent Roo Code release with hook capabilities for subprocess
- Official docs live in the Roo Code community repository explaining hook lifecycle conventions
- Golden tip is to chain `related <name> --hops 2` after recall for multi-hop graph expansion


## Cline
### Community VS Code Extension — Terminal Integration
- Recipe ready to register as a Cline terminal tool, zero cloud cost, memory persists locally
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to give Cline persistent memory across VS Code sessions without cloud services
- Use `sqlite-graphrag list --limit 20 --json` as a seed step at Cline conversation startup
- Minimum version supports the current Cline VS Code extension release in the marketplace
- Official docs live at https://cline.bot covering terminal tool registration and usage patterns
- Golden tip is to bind the command to a Cline tool with descriptive name and usage explanation


## Continue
### Open Source Agent — IDE Terminal Integration
- Recipe ready to paste into Continue custom commands config, zero cloud cost, no server needed
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to surface sqlite-graphrag memory inside Continue chat panels in VS Code or JetBrains
- Use `sqlite-graphrag recall "docstring" --json` from a Continue custom command registration
- Minimum version supports any recent Continue extension release in VS Code or JetBrains stores
- Official docs live at https://docs.continue.dev describing custom commands and tool integration
- Golden tip is to document each command in the Continue config so the embedded LLM picks it up


## Factory
### Factory Agent — API Or Subprocess
- Recipe ready to add to the Factory droid tool config, zero cloud cost, binary is self-contained
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to integrate sqlite-graphrag with Factory autonomous development droids in production
- Use `sqlite-graphrag recall "pr context" --json` during the Factory droid plan preparation phase
- Minimum version supports any recent Factory release with subprocess or API tool integration
- Official docs live at https://factory.ai explaining droid tool configuration and plan execution
- Golden tip is to set a long `--wait-lock` for Factory droids running under heavy concurrency


## Augment Code
### Augment Agent — IDE Integration
- Recipe ready to wire into Augment IDE tool registration, zero cloud cost, runs as subprocess
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to feed Augment Code review agents with persistent cross-repository memory state
- Use `sqlite-graphrag hybrid-search "code review" --json` inside Augment IDE review preparation
- Minimum version supports any recent Augment Code release with terminal and subprocess hooks
- Official docs live at https://docs.augmentcode.com describing tool registration and agents
- Golden tip is to enable `--lang en` explicitly for consistent review language across teams


## JetBrains AI Assistant
### JetBrains Agent — IDE Integration
- Recipe ready to register as a JetBrains external tool, zero cloud cost, recall takes milliseconds
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to add sqlite-graphrag memory to JetBrains AI Assistant across IntelliJ PyCharm WebStorm
- Use `sqlite-graphrag recall "$SELECTION" --json` registered as a JetBrains external tool runner
- Minimum version requires JetBrains AI Assistant 2024.2 or later for modern tool registration
- Official docs live at https://www.jetbrains.com/ai explaining tool and external runner registration
- Golden tip is to bind the tool to a keyboard shortcut to invoke recall with one hand on keyboard


## OpenRouter
### Multi-LLM Router — Any Version Supported
- Recipe ready to add as a preamble to any OpenRouter pipeline, zero cloud cost, memory stays local
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to share a common memory backend across every OpenRouter-hosted LLM in a pipeline
- Use `sqlite-graphrag recall "routing rule" --json` as a preamble step before any routed request
- Minimum version supports any OpenRouter API release since memory remains local and independent
- Official docs live at https://openrouter.ai/docs explaining routing rules and API integration
- Golden tip is to reuse the same namespace across all routed models for consistent context


### OpenRouter Embedding Backend (v1.0.94)
- Since v1.0.94, sqlite-graphrag can use OpenRouter as a dedicated embedding backend via REST API
- Use `--embedding-backend openrouter --embedding-model MODEL` for ~200ms embedding instead of 15s subprocess
- 10 models verified: Qwen 4B/8B, NVIDIA Nemotron (free), OpenAI small/large, Perplexity, Mistral, BAAI, Google Gemini
- Set API key via `config add-key --provider openrouter` or `--openrouter-api-key` (OPENROUTER_API_KEY is not read at runtime)

```bash
printf "%s" "sk-or-v1-your-key-here" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY is not read at runtime (G-T-XDG-04)
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  remember --name test --type note --description "test" --body "content" --json
```

## Minimax (since v1.0.83 — ADR-0041)
### Anthropic-Compatible Provider — MiniMax/api.minimax.io
- Recipe ready to route Claude Code through any Anthropic-compatible endpoint without breaking the OAuth-only mandate
- While the OAuth-only guard still rejects `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` with exit 1 (defence in depth from v1.0.69), the new whitelist preserves `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CODEX_ACCESS_TOKEN`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`, and `OTEL_EXPORTER_OTLP_ENDPOINT`
- Purpose is to enable Anthropic-compatible providers (MiniMax/api.minimax.io, OpenRouter, AWS Bedrock custom routes, corporate gateways) without forcing operators to pay the official Anthropic API key path
- HISTORICAL RECIPE: the env vars below were exported before any `sqlite-graphrag` command that triggered embedding (`remember`, `edit`, `ingest --mode claude-code`). None of it applies to a 1.2.8 binary: there is no subprocess to inherit an env var, `ingest --mode claude-code` is rejected, and the maintenance section below states that product environment variables are ignored at runtime. The current path to a custom provider is `config add-key --provider openrouter --from-stdin` plus `--llm-backend openrouter` / `--embedding-backend openrouter`, both of which are CLI flags with XDG counterparts set through `config set`
- Minimum version requires `sqlite-graphrag` 1.0.83 or later; older releases will spawn the subprocess without the custom-provider env vars and the provider will return `401 Invalid authentication credentials`
- Official docs live at https://platform.minimax.io/document and `docs/decisions/adr-0041-preserve-custom-provider-env.md` explains the architectural rationale
- Golden tip is to verify the provider reachability with `curl -fsS "$ANTHROPIC_BASE_URL/v1/models" -H "Authorization: Bearer $ANTHROPIC_AUTH_TOKEN"` before running any `sqlite-graphrag` command

### Configuration Block
```bash
# Configure once per shell session before invoking sqlite-graphrag
export ANTHROPIC_AUTH_TOKEN="sk-cp-your-provider-token"
export ANTHROPIC_BASE_URL="https://api.minimax.io/anthropic"
# Optional: opt out of subprocess telemetry forwarding
export DISABLE_TELEMETRY="1"
# Optional: route OpenTelemetry to a local collector instead of provider default
export OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4317"
```

### Smoke Test
```bash
# 1. Verify the provider returns models for the configured token
curl -fsS "$ANTHROPIC_BASE_URL/v1/models" \
  -H "Authorization: Bearer $ANTHROPIC_AUTH_TOKEN" \
  | head -c 200 && echo

# 2. Persist a smoke-test memory through the custom provider
sqlite-graphrag remember \
  --name smoke-test-minimax-v183 \
  --type note \
  --description "validacao do provider customizado via v1.0.83" \
  --body "smoke test executado em $(date -u +%FT%TZ)" \
  --graph-stdin <<'EOF'
{
  "body": "smoke test executado em $(date -u +%FT%TZ)",
  "entities": [
    {"name": "minimax", "entity_type": "tool", "description": "Anthropic-compatible provider"}
  ],
  "relationships": []
}
EOF

# 3. Confirm the embedding landed in memory_embeddings (not NULL)
sqlite-graphrag read --name smoke-test-minimax-v183 --json | jaq '{name, memory_id, has_embedding: (.body | length > 0)}'

# 4. Run a recall to verify the embedding participates in vector search
sqlite-graphrag recall "validacao do provider customizado" --k 3 --json | jaq '.results[] | {name, score}'
```

### Troubleshooting 401 Invalid Authentication Credentials
- **Symptom**: `sqlite-graphrag remember` returns exit 11 with `claude exited with exit status: 1: stderr=` (or `codex` equivalent)
- **Cause**: the `ANTHROPIC_AUTH_TOKEN` or `ANTHROPIC_BASE_URL` env vars did NOT reach the subprocess (older sqlite-graphrag, or strict mode, or shell wrapping that strips env)
- **Resolution paths**:
  - Confirm `sqlite-graphrag --version` reports `1.0.83` or later
  - Confirm the env vars are exported in the SAME shell where the command runs (not a parent shell, not a `.envrc` consumed only by direnv)
  - Run with `env | rg "ANTHROPIC_(AUTH_TOKEN|BASE_URL)"` to confirm presence
  - HISTORICAL: --strict-env-clear was the compliance switch of the subprocess era and lived on the global surface until v1.2.2, as `src/cli/globals.rs` still records; a 1.2.8 binary answers unexpected argument '--strict-env-clear' found with exit 2. Nothing to strip today: the process forwards no credential to any child, because it starts none
  - Capture the exact error with `RUST_LOG=trace sqlite-graphrag remember ... 2> trace.log` and grep for `apply_env_whitelist`
- **Defense-in-depth confirmation**: the OAuth-only guard still rejects `ANTHROPIC_API_KEY` if accidentally set; verify with `export ANTHROPIC_API_KEY=sk-ant-test && sqlite-graphrag remember --name test --body x` returning exit 1
## POSIX Shells
### Bash Zsh Fish PowerShell — Any Version
- Recipe ready to paste into any shell alias or script, zero cloud cost, pipes work out of the box
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess by default with no daemon to manage (the daemon was removed in v1.0.79)
- Purpose is to compose sqlite-graphrag with classic Unix and Windows shell pipelines seamlessly
- Use `sqlite-graphrag recall "$query" --json | jaq '.hits[].name'` in any POSIX-compatible shell
- Minimum version supports any recent Bash Zsh Fish or PowerShell 7 release
- Official docs live at https://www.gnu.org/software/bash and respective shell project homepages
- Golden tip is to quote variables explicitly to avoid word splitting in queries with spaces


## Nushell
### Nushell — Structured Data Pipeline Integration
- Recipe ready to paste into a Nushell script, zero cloud cost, output becomes native Nu table
- While MCPs require a dedicated server, sqlite-graphrag runs as a subprocess via `^` sigil in Nu
- Purpose is to compose sqlite-graphrag output with Nushell structured data pipelines natively
- Use `^sqlite-graphrag recall "query" --k 5 --json | from json | get results` to query memory
- Minimum version supports Nushell 0.90 or later for stable external command and `from json` pipeline
- Official docs live at https://www.nushell.sh/book describing external commands and JSON parsing
- Golden tip is to pipe results into `select name score` to display a ranked memory table in Nu


## Maintenance And Graph Commands In Pipelines
### Composition Surface — jaq, NDJSON, Headless One-Shots
- Every command below is a one-shot subprocess with a JSON or NDJSON contract on stdout, so it composes with `jaq`, redirection, and any scheduler documented here
- Pass `--json` on the single-envelope commands; `export` and `schema` already stream NDJSON and need no flag
- Place `--db <PATH>` AFTER the subcommand; product environment variables `SQLITE_GRAPHRAG_*` are ignored at runtime, so use CLI flags plus `sqlite-graphrag config set <KEY> <VALUE>`
- Preview every mutating command with `--dry-run` first; `cleanup-orphans`, `prune-ner` and `prune-relations` additionally require `--yes` to commit
- `split-body` writes child memories without inline embeddings — chain a SEPARATE `enrich --operation re-embed --target memories` pass after it exits 0

| Command | Emits on stdout | Pipeline recipe |
| --- | --- | --- |
| `backup` | `{action, source, destination, size_bytes, pages_copied, elapsed_ms}` | `sqlite-graphrag backup --output snap.sqlite --json \| jaq '{destination, size_bytes}'` |
| `export` | NDJSON, one line per memory plus a trailing summary line | `sqlite-graphrag export --type decision --namespace my-project > backup.ndjson` |
| `schema` | NDJSON catalogue of `{id, invoke}`; `--name <ID>` emits one JSON Schema | `sqlite-graphrag schema \| jaq -r .id` |
| `related` | `{name, max_hops, results, related_memories}` | `sqlite-graphrag related onboarding --max-hops 3 --json \| jaq -r '.results[].name'` |
| `memory-entities` | `{memory_name, entities}` with `description` per entity | `sqlite-graphrag memory-entities --name my-memory --json \| jaq '.entities[] \| {name, description}'` |
| `namespace-detect` | `{namespace, source, cwd}` | `NS=$(sqlite-graphrag namespace-detect --json \| jaq -r .namespace)` |
| `cleanup-orphans` | `{orphan_count, deleted, dry_run, namespace}` | `sqlite-graphrag cleanup-orphans --dry-run --json \| jaq .orphan_count` |
| `delete-entity` | `{entity_name, relationships_removed, bindings_removed}` | `sqlite-graphrag delete-entity --name stale-tool --cascade --json \| jaq '{relationships_removed, bindings_removed}'` |
| `prune-ner` | `{entity, bindings_removed}` | `sqlite-graphrag prune-ner --all --dry-run --json \| jaq .bindings_removed` |
| `prune-relations` | `{relation, entities_affected, affected_entity_names}` | `sqlite-graphrag prune-relations --relation mentions --dry-run --json \| jaq .entities_affected` |
| `reclassify` | `{action, description_updated, namespace}` | `sqlite-graphrag reclassify --from-type concept --to-type tool --batch --json \| jaq .action` |
| `reclassify-relation` | `{from_relation, to_relation, merged_duplicates}` | `sqlite-graphrag reclassify-relation --literal-from applies-to --to-relation uses --batch --dry-run --json \| jaq '{from_relation, to_relation}'` |
| `split-body` | split report per oversized memory, previewable with `--dry-run` | `sqlite-graphrag split-body --batch --threshold 50000 --dry-run --json` |


## Local schedulers (no GitHub Actions / cloud CI)
### Linux systemd user / cron — macOS launchd — Windows Task Scheduler
- The product **forbids** GitHub Actions / CI workflows in-repo (manual releases only).
- Use **local** multi-platform schedulers for one-shot maintenance:
  - Linux: a systemd --user timer (--user is a systemd flag, not a sqlite-graphrag one) or `cron` running `sqlite-graphrag purge --days 30 --yes` and `vacuum`
  - macOS: `launchd` plist invoking the cargo-installed binary
  - Windows: Task Scheduler with the same one-shot binary
- While MCPs require a dedicated server, sqlite-graphrag installs via cargo and exits after each run
- Golden tip: archive `sync-safe-copy` output on local filesystem for rollback


## Docker and Podman Alpine
### Container — Any Recent Version
- Recipe ready to copy into a Dockerfile, zero cloud cost, final image fits under 25 MB Alpine
- While MCPs require a dedicated server, sqlite-graphrag is a single static binary with no runtime deps
- Purpose is to package sqlite-graphrag in minimal Alpine images for reproducible production deployments
- Use a multi-stage Dockerfile with a Rust builder stage and an Alpine runtime copying the binary
- Minimum version supports any Docker or Podman release compatible with multi-stage build syntax
- Official docs live at https://docs.docker.com covering multi-stage build and image minimization
- Golden tip is to mount the SQLite file as a named volume to persist memory across container restarts


## Kubernetes Jobs And CronJobs
### Kubernetes — 1.25+
- Recipe ready to copy into a CronJob manifest, zero cloud cost, runs inside your existing cluster
- While MCPs require a dedicated server, sqlite-graphrag runs as a one-shot Job with no sidecar needed
- Purpose is to run sqlite-graphrag maintenance as Kubernetes CronJobs inside managed production clusters
- Use a CronJob manifest referencing the Alpine image and invoking purge plus vacuum on schedule
- Minimum version requires Kubernetes 1.25 or later for stable CronJob and concurrency policy support
- Official docs live at https://kubernetes.io/docs describing Job CronJob and PersistentVolumeClaim
- Golden tip is to mount the DB from a PVC with access mode `ReadWriteOnce` for data safety


## Scoop And Chocolatey
### Package Manager — Windows
- Recipe ready to run once the manifest lands, zero cloud cost, installs the same binary as cargo
- While MCPs require a dedicated server, sqlite-graphrag is a single exe with no runtime dependency
- Purpose is to install sqlite-graphrag on Windows with Scoop or Chocolatey familiar to Windows developers
- Use `scoop install sqlite-graphrag` or `choco install sqlite-graphrag` once official manifests land
- Minimum version supports any Scoop 0.3 or Chocolatey 2.0 release with modern manifest features
- Official docs live at https://scoop.sh and https://chocolatey.org explaining manifest conventions
- Golden tip is to run the binary inside the target project folder so it creates `graphrag.sqlite` there


## Nix And Flakes
### Package Manager — Any Nix Version
- Recipe ready to add as a flake input, zero cloud cost, binary hash is pinned for reproducibility
- While MCPs require a dedicated server, sqlite-graphrag runs as a pure binary in any Nix dev shell
- Purpose is to install sqlite-graphrag in reproducible Nix environments including NixOS and dev shells
- Use `nix run github:danilo-aguiar-br/sqlite-graphrag#sqlite-graphrag` to execute without installation
- Minimum version requires Nix 2.4 or later with Flakes feature enabled in user configuration
- Official docs live at https://nixos.org describing Flakes enablement and usage from command line
- Golden tip is to pin the flake input hash so the binary stays reproducible across every rebuild
