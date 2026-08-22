# HOW TO USE sqlite-graphrag (v1.2.8 — agent-native output surface, enrich CAPA seal, dim 1024, E2E, schema v17)

> Ship persistent memory to any AI agent with one local binary, a
> single SQLite file, and the LLM CLI you already trust.

- Versão em português: [HOW_TO_USE.pt-BR.md](HOW_TO_USE.pt-BR.md)
- Voltar ao [README.md](../README.md) para referência de comandos

## Configuration (XDG — v1.2.5)

- The complete registry of all 70 XDG keys, with value kind and default, lives in [AGENTS.md — Complete XDG key registry](AGENTS.md#required--complete-xdg-key-registry-all-70-keys-v125). The same 70 keys are reproduced below, in the section this guide owns, so you configure the binary without leaving the page.
- Runtime knobs resolve as **CLI flag > XDG `config set` > named default**
- Product env `SQLITE_GRAPHRAG_*` is **not** read at runtime (forbidden for product configuration)
- Secrets: `config add-key --provider openrouter` (stdin) or `--openrouter-api-key` per call
- Inspect: `config path`, `config list`, `config list --effective`, `config doctor`
- OpenRouter URLs: `config set network.openrouter.chat_url …` / `network.openrouter.embeddings_url …`
- Fail-fast offline recall: `config set llm.probe_timeout_ms 3000` and/or `--llm-backend none`
- Soft-delete cleanup: `purge --now --yes` for immediate hard-delete; default retention is 90 days and `--yes` alone does **not** wipe recent soft-deletes
- Allowed OS env only: locale (`LANG`/`LC_*`), `PATH`, `HOME`/`USERPROFILE`, XDG base dirs, `NO_COLOR`, plus subprocess OAuth whitelist (`ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, …)
- Offline gate: `bash scripts/e2e_offline_v120.sh` (historical wrapper `e2e_offline_v118.sh` superseded). Pin library consumers to `=1.2.2`. Schema stays at **v16** (no migrate if already on v16). **DEFAULT_EMBEDDING_DIM=1024**. Enrich CAPA seal (namespace claim, until-empty op+ns, force-redescribe reopen, re-embed LENGTH / entity: enqueue) — see section below.

```bash
sqlite-graphrag config set network.openrouter.embeddings_url "https://openrouter.ai/api/v1/embeddings"
sqlite-graphrag config list --effective --json
sqlite-graphrag purge --now --yes --json   # after forget, when you want immediate hard-delete
```

## What Changed in v1.2.1 — Enrich Queue CAPA Seal (No Migration)

- Crate **1.2.1**; schema **unchanged** at **v16**. No main-DB migration — **sidecar queue behaviour only**. Pin library consumers to `=1.2.1`.
- **Namespace isolation on claim** — `dequeue_next_pending` requires `operation` **and** `namespace`. Enrich in `ai-sdd` no longer processes `global` / empty-ns rows.
- **`--until-empty` counts only this op+namespace** — `count_eligible_pending` (not all pending across operations). Alien ReEmbed zombies no longer keep EntityDescriptions spinning until max-runtime with `completed=0`.
- **`--force-redescribe` reopens `skipped`/`done`** — `reopen_force_redescribe_candidates` once per process before first enqueue; never reopens `dead` (use `--requeue-dead`).
- **Re-embed zombie reconciliation** — `reconcile_satisfied_reembed_pending` marks pending ReEmbed `done` when live BLOB already matches active dim (`LENGTH(embedding) = dim*4`).
- **Re-embed eligibility uses BLOB length** — eligible when no vector with `LENGTH(embedding) = target_dim * 4`. CORRUPT rows (`dim=1024`, BLOB still 384) re-embed again.
- **Enqueue validates re-embed keys** — `entity:{name}` strips prefix for entity lookup; bare names still work; missing entities rejected. Chunk keys validate `chunk_id` exists in a non-deleted memory of the target namespace.
- **CAPA-D low-quality markers** — compound "configuration file" phrases only (no bare `%configuration file%` FP).
- Regressions: `enqueue_candidate_accepts_entity_prefixed_reembed_key`, `dequeue_next_pending_isolates_by_namespace`; queue suite **38** OK.
- Offline gate unchanged: `scripts/e2e_offline_v120.sh` **20/20**. Residual: live LQ description backfill remains operator campaign (`--force-redescribe` + `--until-empty`).
- See [MIGRATION.md](MIGRATION.md) and [CHANGELOG.md](../CHANGELOG.md) `[1.2.1]`.

### Recipes — enrich CAPA (v1.2.1)

```bash
DB="${DB:-$HOME/.local/share/sqlite-graphrag/memory.db}"
MODEL="${MODEL:-deepseek/deepseek-v4-flash:nitro}"

# Status (no LLM) — scoped by operation + namespace
sqlite-graphrag enrich --db "$DB" --status --operation re-embed --namespace global -q

# Re-embed entities after dim migrate / CORRUPT BLOB (LENGTH eligibility)
sqlite-graphrag enrich --db "$DB" --operation re-embed --target entities \
  --mode openrouter --openrouter-model "$MODEL" \
  --until-empty --namespace global -q --wait-lock 60

# Force-redescribe with reopen of skipped/done (never dead)
sqlite-graphrag enrich --db "$DB" --operation entity-descriptions \
  --mode openrouter --openrouter-model "$MODEL" \
  --force-redescribe --until-empty --namespace global -q

# Skipped-sink recovery (no raw SQL)
sqlite-graphrag enrich --db "$DB" --list-skipped --operation entity-descriptions --namespace global -q
sqlite-graphrag enrich --db "$DB" --requeue-skipped --operation entity-descriptions --namespace global -q
```

## What Changed in v1.2.0 — dim 1024 + XDG Config + residual seal (No Migration)

- Crate **1.2.0**; schema **unchanged** at **v16**. No main-DB migration. Pin library consumers to `=1.2.0`.
- **DEFAULT_EMBEDDING_DIM=1024** (flag `--embedding-dim` / XDG `embedding.dim` override; existing DBs keep `schema_meta.dim` until re-embed).
- Help scrub: no product env tables, no Box about on ingest/enrich. Precedence **CLI flag > XDG `config set` > default**.
- OpenRouter URLs wired from XDG; Auto query embed fail-fast (`llm.probe_timeout_ms` default 3000 ms + probe).
- EntityType fold: `"module"` → Concept; `related_to` → `related`.
- `remember-batch` requires non-empty `description` on create.
- Aliases: `pending-embeddings status`, `cache stats`; `purge --now`; `config list --effective`.
- Enrich queue multi-namespace (`namespace` column + unique key); **`--list-skipped` / `--requeue-skipped`** recover preservation/skipped debt without raw SQL.
- **GAP-SG-139**: host/XDG leaves (`config`, `slots`, `cache`, `completions`) accept `--db` as documented **no-op**.
- QISO: enrich queue claim is scoped by `operation` (memory-bindings cannot claim entity/pair rows).
- entity-descriptions: multi-domain neutral prompt, corpus grounding, `--force-redescribe` for low-quality rewrite.
- Status honesty: `enrich --status --force-redescribe` reports `scan_backlog_low_quality`, `quality_pct`, `state=blocked_dead` when applicable.
- Names: `--entity-names` / `--memory-names` (alias `--names` with per-operation semantics).
- remember hot-set: envelope fields `entities_created` / `enrich_recommended`; flag `--enqueue-enrich`.
- deep-research short `-o` alias of `--output`; atomic write + ack `{written,bytes,blake3}`.
- memory-entities forward JSON includes `entities[].description`.
- entity-connect remains fully implemented (persists relationships); large-DB: `--anchor-memory`, adaptive limits, yield, `budget_exhausted` / `preempted_for_gate`.
- Offline gate: `scripts/e2e_offline_v120.sh` (**20/20**); historical wrapper `e2e_offline_v118.sh` superseded. Hermetic `IsolatedEnv` / `xdg_isolation_guard` tests (no product env in tests/benches).
- Recommended order after write: entity-descriptions (hot) then entity-connect (cold).
- Residual honest: further monólito SRP optional; live LQ backfill is operator campaign (`--force-redescribe` + LLM).
- See [MIGRATION.md](MIGRATION.md) and [CHANGELOG.md](../CHANGELOG.md) `[1.2.0]`.

### Recipes — enrich quality and hot-set (v1.2.0)

```bash
# After curated remember: parse enrich_recommended, then priority ED
sqlite-graphrag remember --name demo --type note --description "d" --body "ICMS fiscal note" \
  --graph-stdin --enqueue-enrich --json <<'EOF'
{"entities":[{"name":"icms-p05","entity_type":"concept"}],"relationships":[]}
EOF
# envelope may include entities_created[] and enrich_recommended:["entity-descriptions"]

# Audit entity descriptions for one memory
sqlite-graphrag memory-entities --name demo --json | jaq '.entities[] | {name, description}'

# Priority pass on named entities (not memory names)
sqlite-graphrag enrich --operation entity-descriptions \
  --entity-names icms-p05 --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json

# Rewrite low-quality descriptions already filled
sqlite-graphrag enrich --operation entity-descriptions --force-redescribe \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --limit 20 --json
sqlite-graphrag enrich --operation entity-descriptions --status --force-redescribe --json

# deep-research short -o (same as --output)
sqlite-graphrag deep-research "auth decisions" -o /tmp/dr.json --quiet --json

# memory-bindings uses memory names
sqlite-graphrag enrich --operation memory-bindings --memory-names demo --dry-run --json
```


## What Changed in v1.1.06 — Entity-Connect Scan O(k) (No Migration)

- Official release name **v1.1.06**; `Cargo.toml` carries `1.1.6`. Schema **unchanged** at **v16**.
- Closes GAP-ENTITY-CONNECT-SCAN-CARTESIAN (P0 hang on large `global`).
- Pair candidates: **co-occurrence** in `memory_entities` + **hub × degree-0 island** fill.
- Queue keys `pair:{id1}:{id2}` with `item_type=entity_pair`; drain by primary key (no re-scan per item).
- First scan covered by `--max-runtime` / soft 120s via `InterruptHandle` → Timeout exit **1** (not singleton exit **75**).
- NDJSON: `scan_start` (before SQL) with `operation`, `entities_in_namespace`, `backlog_degree0_proxy`; `scan_meta` with `pairs_enqueued_this_scan` — do not equate the dual backlog fields.
- `cross-domain-bridges` uses the **same** fully-implemented O(k) path + `entity_connect_seen`; **GAP-002** convergence preserved.
- Suite: `tests/v1106_entity_connect_scan_regression.rs`. ADR-0066.
- Pin library consumers to `=1.1.6`.

### Recipe — Safe entity-connect dry-run on a large namespace

```bash
sqlite-graphrag enrich --operation entity-connect --dry-run --json --limit 50 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro
# Expect: validate → scan_start → scan → scan_meta (ms–s, not minutes of 100% CPU)
# scan_start.backlog_degree0_proxy ≠ scan_meta.pairs_enqueued_this_scan (dual backlog)
```

### Recipe — Converge with wall-clock ceiling on the first scan

```bash
# --max-runtime covers the FIRST scan (InterruptHandle). Timeout → exit 1, not 75.
sqlite-graphrag enrich --operation entity-connect --until-empty --max-runtime 600 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json
# cross-domain-bridges uses the same O(k) path + entity_connect_seen
```

## What Changed in v1.1.05 — Five Deep-Research Incident Bugs (No Migration)

- The official release name is **v1.1.05**; `Cargo.toml` carries `1.1.5` because SemVer rejects a leading zero in the patch segment. Schema is UNCHANGED at **v16** (from v1.1.04) — upgrading does NOT require `migrate`. Binary ~19 MiB. Library consumers pin `=1.1.5`.
- Just `cargo install sqlite-graphrag --locked --force`.
- Bug 1: single-token `deep-research` expands to multi-aspect sub-queries (`source: "aspect"`, EN/PT facets); manual via `--sub-query-strategy manual --sub-queries-file`.
- Bug 2: `deep-research --output PATH` atomwrite (tempfile → fsync → rename) + short stdout ack with `blake3`; global `--quiet`/`-q`.
- Bug 3: `graph traverse --fuzzy` for short nicknames; NotFound suggests canonical names without `--fuzzy`.
- Bug 4: `merge-entities` rejects self-ref (`--into-id` in `--ids`) before any DB work.
- Bug 5: `link --from-id`/`--to-id`; pure digit names rejected.
- Regression suite: `tests/v1105_incident_bugs_regression.rs`.
- See [ADR-0065](decisions/adr-0065-v1-1-05-incident-bugs.md).

### Recipe — Single-token deep-research with atomic `--output`

```bash
# Single subject token fans out to aspect sub-queries; full envelope lands on disk
sqlite-graphrag --quiet deep-research "alice" --max-sub-queries 7 --k 20 \
  --output /tmp/alice-research.json --json
# stdout is a short ack: {written, bytes, blake3, sub_queries_total, unique_memories_found, elapsed_ms}
# full envelope: jaq . /tmp/alice-research.json
```

### Recipe — Fuzzy graph traverse for short names

```bash
# Suggestions only (exact miss → exit 4 with ranked candidates)
sqlite-graphrag graph traverse --from alice --depth 2 --json
# Auto-resolve a clear winner
sqlite-graphrag graph traverse --from alice --fuzzy --depth 2 --json
```

### Recipe — Link by entity ID (never pure digits as names)

```bash
sqlite-graphrag link --from-id 42 --to-id 77 --relation related --json
# Pure digits as --from/--to names are rejected (no ghost entities)
```

### Recipe — Merge self-ref is rejected early

```bash
# Exit non-zero BEFORE opening/writing the DB when target is also a source
sqlite-graphrag merge-entities --ids 1,2,3 --into-id 3 --json
```

## Custom Providers (v1.0.83+)
- sqlite-graphrag supports Anthropic-compatible providers (Minimax/api.minimax.io, OpenRouter, AWS Bedrock, corporate gateways) by preserving the following env vars when spawning `claude -p` or `codex exec`
- Preserved vars: `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`, `OTEL_EXPORTER_OTLP_ENDPOINT`
- The OAuth-only mandate remains active: `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` still abort the spawn with exit 1
- The four OAuth-only guards at `claude_runner.rs:273`, `codex_spawn.rs:259`, `ingest_claude.rs:282`, `extract/llm_embedding.rs:237-253` are unchanged; only the env-clear whitelist was extended
- Shared helper `src/spawn/env_whitelist.rs` exposes `apply_env_whitelist(cmd, strict)`; the three spawners delegate instead of inlining the array
- HISTORICAL (v1.0.83): compliance environments that required strict env-clear (PCI-DSS, SOC2, HIPAA) passed --strict-env-clear, REMOVED in v1.2.0 with the subprocess spawners and rejected by clap with exit 2 in v1.2.8; the HISTORICAL `SQLITE_GRAPHRAG_STRICT_ENV_CLEAR` env var is not read at runtime either; strict mode preserved only `PATH`
- No new telemetry: the fix is silent. No `tracing::info!` macro logs which provider is in use. The no-leak audit test `audit_no_token_leak_in_subprocess_stderr` in `tests/claude_runner_env.rs` enforces that the literal token value NEVER appears in stdout or stderr even with `RUST_LOG=trace`
- See `docs/decisions/adr-0041-preserve-custom-provider-env.md` and `docs/COOKBOOK.md#how-to-use-custom-anthropic-compatible-providers-v1083` for the full recipe
- Resolves GAP-058 partially: custom-provider env vars route around OAuth quota contention; `recall`/`hybrid-search` stay deterministic under official OAuth fatigue

## What Changed in v1.1.04 — Nested-Runtime Fix + entity-connect Convergence

- See [docs/MIGRATION.md](MIGRATION.md) for the V016 upgrade path from v1.1.03. Schema advances v15→v16. Pin `=1.1.4` only if you must stay on that release.

## What Changed in v1.1.02 — GLiNER Removal, TooManyTokens Typed, Re-Embed Regression, Entity Orphan Prune (ADR-0062)
- The official release name is **v1.1.02**; `Cargo.toml` carries `1.1.2` because SemVer rejects a leading zero in the patch segment. Schema is UNCHANGED at v15 — upgrading does NOT require `migrate`. Binary ~19 MiB. Library consumers pin `=1.1.2`. User-Agent is `sqlite-graphrag/1.1.2`.
- **Gap 1 (BREAKING)**: --gliner-variant and the `GlinerVariant` enum are REMOVED from the parser — clap rejects --gliner-variant with exit 2 (precedent: --max-entity-degree of v1.0.99); `--mode gliner` is REMOVED too (the `IngestMode` enum now has only `none`); `SQLITE_GRAPHRAG_GLINER_MODEL`/`SQLITE_GRAPHRAG_GLINER_THRESHOLD` env vars are silently ignored.
- **Gap 2**: `AppError::TooManyTokens{tokens,limit}` is a new typed exit 6 variant (joins `BodyTooLarge`/`TooManyChunks`); the JSON envelope reports `{tokens,limit}` so callers can tell bytes vs chunks vs tokens apart.
- **Gap 3**: the `strip_prefix("entity:")` dispatch in `call_reembed` is covered by regression test `tests/reembed_entities_integration.rs` — entity embeddings backfill from 0→N and the coverage query hits zero missing.
- **New flag**: `enrich --prune-dead-entity-orphans` (mutually exclusive with `--prune-dead-orphans`) deletes entity-keyed dead-letter rows from `.enrich-queue.sqlite`; unit test `prune_dead_entity_orphans_removes_only_entity_dead_rows` + integration test `tests/prune_dead_entity_orphans_integration.rs`.
- 4 pre-existing rustdoc warnings resolved (backticks in HTML blocks, cfg(test) intra-doc links).

## What Changed in v1.1.01 — Entity/Chunk Embedding Backfill, Targeted Re-Embed, graph recompute-degree
- The official release name is **v1.1.01**; `Cargo.toml` carries `1.1.1` because SemVer rejects a leading zero in the patch segment. Schema is UNCHANGED at v15 — upgrading does NOT require `migrate`. Binary ~19 MiB. Library consumers pin `=1.1.1`.
- **P1**: entity embedding now routes through the OpenRouter REST path even with `--llm-backend none`; an empty-vector guard on the upserts prevents zero-byte embedding blobs.
- **P2**: `enrich --operation re-embed --target memories|entities|chunks|all` selects which embedding table to backfill; `--status` reports the per-target `scan_backlog`.
- **P3**: new command `graph recompute-degree` recomputes every entity degree in a single transaction; supports `--dry-run`; the envelope reports `{total, updated, zeroed, unchanged}`. Use it to fix historically accumulated degree drift.
- **P4**: `reclassify-relation --literal-from` matches the stored relation verbatim (bypasses clap normalisation); mutually exclusive with `--from-relation`. The tool for migrating legacy underscore edges such as `applies_to`.
- **P5**: `merge-entities --ids <a,b> --into-id <N>` and `rename-entity --id <N>` address entities by numeric id instead of name.
- **P6**: `health --json` gains `vec_memories_missing` / `vec_entities_missing` / `vec_chunks_missing` plus `vec_*_coverage_pct`; `embedding status --json` gains `memories_missing` / `entities_missing` / `chunks_missing` under `coverage`.
- **P7**: `EntityType` error messages now list the 13 canonical entity types.
- **P10**: the re-embed predicate also covers divergent-dimension and empty-blob rows, not only missing rows.
- **P11**: `AppError::BodyTooLarge` / `AppError::TooManyChunks` are typed variants; exit 6 is preserved and the JSON envelope message is now specific.
- **P12**: `ingest --name-prefix <PREFIX>` prefixes the generated memory names (local staging path only).

```bash
# Backfill missing entity/chunk embeddings (v1.1.01)
sqlite-graphrag enrich --operation re-embed --target entities \
  --mode openrouter --openrouter-model MODEL --json
sqlite-graphrag enrich --operation re-embed --target chunks \
  --mode openrouter --openrouter-model MODEL --json

# Recompute all entity degrees in one transaction
sqlite-graphrag graph recompute-degree --dry-run --json
sqlite-graphrag graph recompute-degree --json

# Audit embedding coverage
sqlite-graphrag health --json | jaq '{memories: .vec_memories_missing, entities: .vec_entities_missing, chunks: .vec_chunks_missing}'
```

## What Changed in v1.0.99 — Degree-Cap Removal + Doc/Convergence Fixes (GAP-SG-67/68/69, ADR-0059)
- **GAP-SG-67 (BREAKING)**: the --max-entity-degree flag is REMOVED from `remember` and `link`; passing it now fails with clap exit 2, and the old --max-entity-degree 0 mitigation is obsolete. The destructive global degree-cap pruning (`graph::enforce_degree_cap`) is deleted, so a write is 100% additive — it never prunes/deletes edges nor emits a degree warning, and the total `relationships` count never decreases on a normal write. Trade-off: hub degree grows unbounded; future normalisation is an explicit MAINTENANCE command only.
- **GAP-SG-68**: `graph entities --sort-by degree` is documented correctly — it sorts ascending by default; use `--order desc` for most-connected-first. Doc-only fix, no behaviour change.
- **GAP-SG-69**: `enrich --operation body-enrich ... --until-empty` now converges; vetoed `status='skipped'` short bodies are no longer re-enqueued on rescan, and the `.enrich-queue.sqlite` sidecar is kept while `skipped` verdicts remain (empirically 55→3).
- No migration; schema stays v15. See ADR-0059 and MIGRATION.md.

## What Changed in v1.0.96 — Enrich Dead-Letter + OpenRouter REST Fan-Out (GAP-ENRICH-BACKLOG-CONVERGE, GAP-OPENROUTER-REST-CONCURRENCY, ADR-0055)
- **GAP-ENRICH-BACKLOG-CONVERGE**: the enrich queue gains a terminal `dead` status plus `error_class` and `next_retry_at` columns (idempotent `ALTER TABLE` + `idx_enrich_queue_eligible`). Transient outcomes (rate-limit/timeout/5xx) reschedule with exponential backoff; a HardFailure goes terminal at once; an item turns `dead` after `--max-attempts` Transient retries. The dequeue honours `next_retry_at` and excludes `dead`, so the live set strictly decreases and the backlog always converges.
- `--until-empty` runs an internal scan→drain loop until no eligible items remain or `--max-runtime` (default 3600s) expires — it replaces the external bash retry loop. `--max-attempts <N>` (default 8, range 1..=20) is the Transient retry budget before `dead`.
- `--status` prints a read-only JSON queue report (`unbound_backlog`, per-operation `scan_backlog`, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`). It NEVER calls the LLM and NEVER acquires the singleton — safe to poll while a drain runs. `scan_backlog` (GAP-SG-77, v1.1.0) is the real per-operation database backlog a scan would enqueue — it kills the false `pending=0` for `entity-descriptions`/`body-enrich`/`re-embed`, and `state` derives `pending-scan` from it.
- **GAP-OPENROUTER-REST-CONCURRENCY**: `--rest-concurrency <N>` (default 8, clamp 1..=16) caps a bounded `JoinSet` REST fan-out for `--mode openrouter` (distinct from `--llm-parallelism`). Embedding batches 32 passages with per-chunk order preserved; the SQLite write stays serialized via WAL + atomic claim (single-writer intact).
- No migration; schema stays v15. nextest: 1086 passed, 0 failed, 6 skipped. See ADR-0055.

```bash
# Drain the enrich backlog until it converges (no external loop)
printf "%s" "sk-or-v1-your-key-here" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY is not read at runtime (G-T-XDG-04)
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "deepseek/deepseek-v4-flash:nitro" \
  --until-empty --rest-concurrency 8 --json

# Inspect the queue without running the LLM (no singleton, no tokens)
sqlite-graphrag enrich --status \
  --mode openrouter --openrouter-model "deepseek/deepseek-v4-flash:nitro" --json
```


## What Changed in v1.0.95 — OpenRouter Enrich JUDGE (GAP-OR-ENRICH, ADR-0054)
- **GAP-OR-ENRICH**: `enrich --mode openrouter` routes the JUDGE step to OpenRouter's `/chat/completions` REST endpoint. No local CLI subprocess is spawned. The SCAN→JUDGE→PERSIST pipeline is unchanged; only the JUDGE transport changes.
- The only enrich mode is `openrouter`.
- `--openrouter-model` is **REQUIRED** with `--mode openrouter` (NO default). Omitting it → exit 1 BEFORE any network call.
- `--openrouter-api-key` reads from XDG via `config add-key` (OPENROUTER_API_KEY is not read at runtime) or `config add-key --provider openrouter`. `--openrouter-timeout` defaults to 300s. `--openrouter-base-url` is optional.
- The request uses `response_format` `json_schema` with `strict: true` and `provider.require_parameters: true`. `reasoning.enabled: false` with a reasoning-mandatory fallback (one retry omitting `reasoning`). `usage.cost` is read from the response (`usage: {include: true}` is deprecated).
- 13/13 real models pass. Trade-off: OAuth zero-token (local CLI modes) vs tokens billed to the XDG-stored OpenRouter key (OPENROUTER_API_KEY is not read at runtime) (OpenRouter mode). No migration; schema stays v15. See ADR-0054.

```bash
# Enrich JUDGE via OpenRouter REST (no subprocess)
printf "%s" "sk-or-v1-your-key-here" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY is not read at runtime (G-T-XDG-04)
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" --json
```


## What Changed in v1.0.94 — Four-Gap Remediation (ADR-0053)
- **GAP-OR-ENTITY-EMBED**: Entity embedding in `remember`/`remember-batch`/`ingest` now honours `--embedding-backend openrouter`, routing via OpenRouter REST. `remember` with new entities drops from ~119s to ~0.9s.
- **GAP-EMBED-DIM-64**: `DEFAULT_EMBEDDING_DIM` raised from 64 to **384** (`constants.rs:29`). New databases default to dim 384. Legacy databases at dim 64 are preserved via `schema_meta.dim` — no forced re-embed.
- **GAP-EMBED-TIMEOUT-300**: `DEFAULT_EMBED_TIMEOUT_SECS` raised from 120 to **300** (`llm_embedding.rs:43`).
- **GAP-HEADLESS-DEFAULT**: `enrich --mode` is now **REQUIRED** (`default_value = "claude-code"` removed in `enrich.rs:379`). Omitting `--mode` → clap exit 2. Add `--mode codex` / `--mode claude-code` / `--mode opencode` to all `enrich --operation` invocations.

**Breaking change**: `enrich --operation <op>` now requires `--mode <value>`. See the [MIGRATION guide](MIGRATION.md) for the canonical pairing table.

## What Changed in v1.0.93 — OpenRouter Embedding Backend (GAP-OR-INGEST)
- New global flags: `--embedding-backend auto|openrouter|llm`, `--embedding-model MODEL`, `--openrouter-api-key KEY`
- OpenRouter REST API embedding replaces subprocess LLM for vector generation (~200ms vs 15s per call)
- `EmbeddingBackendChoice` propagated to ALL 13 embedding paths: `remember`, `remember-batch`, `ingest`, `recall`, `edit`, `restore`, `hybrid-search`, `deep-research`, `enrich`, `init`, `rename-entity`, `ingest` (claude mode), `remember` (chunk embedding)
- New `--enrich-after` flag for ingest triggers `enrich --operation memory-bindings` after embedding
- The user MUST specify `--embedding-model` when using `--embedding-backend openrouter` — NO default model
- Set API key via `config add-key --provider openrouter` (OPENROUTER_API_KEY is not read at runtime) or flag `--openrouter-api-key`
- 10 models verified E2E: Qwen 4B/8B, NVIDIA Nemotron (free), OpenAI small/large, Perplexity, Mistral, BAAI bge-m3, Google Gemini 001/002
- All models produce 384-dim vectors via MRL — zero schema change, zero migration
- **GAP-OR-PROPAGATION** (v1.0.93): 5 additional embedding paths fixed — `enrich --operation re-embed`, `init` (dimension probe), `rename-entity`, `ingest --mode claude-code` (4 call sites), and `remember` (chunk parallel embedding) now all honour `--embedding-backend openrouter`
- **BUG-OR-EXIT-CODE** (v1.0.93): OpenRouter config errors (missing API key, missing model, invalid key) now return exit code 78 (`EX_CONFIG`) instead of exit 1
```bash
# Setup
printf "%s" "sk-or-v1-your-key-here" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY is not read at runtime (G-T-XDG-04)

# Remember with OpenRouter
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  remember --name my-note --type note \
  --description "fast embedding" --body "content" --json

# Ingest with OpenRouter + auto-enrich
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  ingest ./docs --pattern "*.md" --recursive \
  --enrich-after --llm-backend openrouter --json
```


## What Changed in v1.0.90, v1.0.91

### v1.0.91 — CWD Isolation, Degree Fix, 6-Gap Doc Remediation

- **GAP-SPAWN-001**: `apply_cwd_isolation()` added in `src/spawn/mod.rs` — sets `current_dir(temp_dir)` and `CLAUDE_CONFIG_DIR=temp_dir` on ALL 10 LLM subprocess spawn sites. Eliminates `.mcp.json` walk-up interference. The manual workaround `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1 CLAUDE_CONFIG_DIR=/tmp/graphrag-empty-config` is NO LONGER NEEDED (HISTORICAL: that product env var is not read at runtime since v1.2.0)
- **GAP-SPAWN-002**: `cleanup_spawn_dir()` added in `src/main.rs` — removes spawn directory at process exit via non-recursive `remove_dir()`
- **BUG-14**: Test `opencode_adapter_build_args` fixed — asserted `"headless"` but adapter returns `"run"` since v1.0.90 refactor
- **BUG-15**: 7 JSON schemas updated from `backend_invoked: enum ["claude", "codex", "none"]` to `["claude", "codex", "opencode", "none", "auto"]`. Affected: `embedding-status`, `enrich-summary`, `hybrid-search`, `recall`, `remember`, `ingest-summary`, `edit`
- **BUG-16**: `deep-research.schema.json` gained `vec_degraded: boolean` in `ResearchStats` (was missing, violated `additionalProperties: false`)
- **BUG-17 (HIGH)**: `entities.degree` inflation fixed — `remember` and `ingest` now use `recalculate_degree()` after relationship insertion instead of `increment_degree()` per entity. `graph stats`, `graph entities`, and the `entities` table are now consistent

### v1.0.90 — OpenCode Backend Integration (ADR-0051)

- Third LLM backend: `--llm-backend opencode` spawns OpenCode CLI headless via `opencode run --format json --dangerously-skip-permissions`
- New flags, all REMOVED in v1.2.0 with the subprocess spawners: --opencode-binary, --opencode-model, --opencode-timeout; HISTORICAL env vars, not read at runtime since v1.2.0: `SQLITE_GRAPHRAG_OPENCODE_BINARY`, `SQLITE_GRAPHRAG_OPENCODE_MODEL`, `SQLITE_GRAPHRAG_OPENCODE_EMBED_MODEL`, `SQLITE_GRAPHRAG_OPENCODE_TIMEOUT`
- Default model: `opencode/big-pickle`; free models: `opencode/deepseek-v4-flash-free`, `opencode/mimo-v2.5-free`, `opencode/nemotron-3-ultra-free`, `opencode/north-mini-code-free`
- Fallback chain: `--llm-backend codex,claude,opencode,none` tries each backend in order
- `--mode opencode` for `ingest` and `enrich` entity extraction pipelines
- NDJSON output from opencode has 3 event types: `step_start`, `text`, `step_finish`
- 24 bugs/gaps remediated; full skill audit with ADR-0051

## What Changed in v1.0.86, v1.0.87, v1.0.88, v1.0.89 (ADR-0045, ADR-0046, ADR-0047, ADR-0048, ADR-0049)

Since v1.0.85.2, four releases introduced the LLM-heavy surface, the pre-flight validation layer, three hotfixes and the schema-as-derived-artifact contract.

### v1.0.86 — LLM-Heavy Surface and Host-Wide Slot Semaphore

- Five new subcommands expose the LLM subprocess pipeline: `pending list`, `pending show`, `pending cleanup`, `embedding status`, `embedding list`, `embedding abandon`, `pending-embeddings list`, `pending-embeddings process`, `slots status`, `slots release` — `pending-embeddings process` and the whole `pending` family never survived: the live family is `pending-embeddings list|status|abandon`
- `pending` (V014 — `pending_memories` table) provides a 3-stage checkpoint for the `remember` pipeline. The checkpointer survives a crash; on restart, `pending list` inspects the queue and `pending show <id>` reads one entry
- `embedding status --status pending|in_progress|done|abandoned` exposes the retry-fallback pipeline
- `slots status` reports `max_concurrency`, `acquired`, `waiting`, `held_by_pid[]`; `slots release --slot-id N --yes` reaps orphan slots
- New global flags: `--max-concurrency <N>`, `--wait-lock <SECONDS>`, `--llm-parallelism <N>` (default 4, clamp [1, 32]), `--ingest-parallelism <N>`, --graceful-shutdown-secs <N> (REMOVED in v1.2.0), `--skip-embedding-on-failure` (only valid with `--llm-backend …,none`)
- Lock contention handled by `fs4 = 0.9` with `fcntl(F_SETLK)` on Unix and `LockFileEx` on Windows (ADR-0039)

### v1.0.87 — Pre-Flight Validation Layer (ADR-0045, GAP-META-005)

- New module `src/spawn/preflight.rs` (≥200 lines, 7 guards, 15 unit tests) gates every LLM subprocess spawn BEFORE the fork
- New `AppError::PreFlightFailed(PreFlightError)` variant with `exit_code() == 16` and `is_permanent() == true`
- New exit code 16 (`EX_CONFIG`) for pre-flight failures. Not documented in any pre-existing exit code table
- The 7 guards in order: `check_argv_size` (argv would exceed ARG_MAX minus 4 KB), `check_binary_exists` (claude/codex reachable in PATH), `check_mcp_config_inline` (replaces literal `--mcp-config "{}"` with tempfile holding `{"mcpServers":{}}`), `check_mcp_config_path` (validates JSON contents), `check_walkup_mcp_json` (rejects invalid `.mcp.json` in workspace ancestor chain), `check_output_buffer` (raises parser buffer above 64 KB), `check_claude_config_dir` (avoids user-level MCP bleed-through)
- Bypass in emergencies (HISTORICAL: not read at runtime since v1.2.0): `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1` disabled all 7 guards. Bypassing reverts to direct `Command::spawn()` and inherits all 5 BUG classes from GAP-META-005
- The 4 spawners (`claude_runner`, `codex_spawn`, `ingest_claude`, `extract/llm_embedding`) share this single module

### v1.0.88 — Hotfixes BUG-11/12/13 (ADR-0046, ADR-0047)

- **BUG-11 (CRITICAL)** fixed: pre-flight failure in `extract/llm_embedding.rs:563-565` now propagates to `remember` via `embed_via_backend_strict` instead of silent persistence with `backend_invoked: "none"`
- **BUG-12 (MEDIUM)** fixed: OAuth-only enforcement now emits 1 stderr line (was 2) — duplicate `eprintln!` removed
- **BUG-13 (MEDIUM)** fixed: `link --create-missing` now respects entity-name validation; previously rejected ALL_CAPS abbreviations were accepted via CLI
- 11 new regression tests: `tests/bug11_preflight_regression.rs` (2), `oauth_stderr_emits_single_line_v1088` (1), `tests/entity_validation_integration.rs` (8)
- Test rename `embed_with_fallback_succeeds_via_none_when_chain_exhausts` → `embed_with_fallback_chain_of_only_none_aborts_without_skip_on_failure_v1088` documents the corrected contract

### v1.0.89 — Schema Drift, Flag Parity, Description Heuristic (ADR-0048, ADR-0049)

- **GAP-E2E-007 (P1)**: `health.schema.json` regenerated via `schemars` derive macro. 17 new fields added; `additionalProperties: true` (Must-Ignore policy per RFC 7493 I-JSON). New bin: `cargo run --bin dump-schema` regenerates 70+ schemas
- **GAP-E2E-008 (P3)**: `embedding status/list/abandon`, `pending list/show` now accept `--db <PATH>`. `clap::Arg::global = true` was REJECTED (invasive, pollutes help). 5 new tests in `tests/cli_db_flag_parity_regression.rs`
- **GAP-E2E-009 (P3)**: `migrate --dry-run --json` now reports pending migrations without applying. 1 new test in `tests/migrate_dry_run_regression.rs`
- **GAP-E2E-010 (P3)**: `codex-models --json` accepted as no-op; `pending list --db <PATH>` parity. Both with `#[arg(long, hide = true)]`. 1 new test in `tests/codex_models_json_regression.rs`
- **GAP-E2E-011 (P2)**: `ingest --auto-describe` (default true) extracts description from first meaningful body line (>20 chars, not a header). `extract_heuristic_description(body, path_hint)` falls back to file stem. `--no-auto-describe` opt-out. 5 new tests in `tests/ingest_auto_describe_regression.rs`
- **GAP-E2E-002 (P3)**: `health --namespace <NS> --json` filters counts to a single namespace. 1 new test in `tests/health_namespace_regression.rs`
- **GAP-E2E-001 (P2)**: Binary size 14.6 MiB documented in `Cargo.toml:6` (was 6 MB since v1.0.76). 1 new test in `tests/binary_size_documented_regression.rs`
- Total: 1059 tests passing. Binary 15.3 MB ELF stripped
## What v1.0.82 Changed (Five Gaps, Two Migrations, Four Subcommands)

v1.0.82 is a **patch** bump that DOES carry two additive database migrations (`V014__pending_memories`, `V015__pending_embeddings`). The schema version advances from 13 to 15. Library consumers must pin to `=1.0.82` per the stability policy (ADR-0032). The 5 gaps closed: GAP-001 three-stage `remember` checkpoint queue (ADR-0036), GAP-002 shutdown JSON envelope at exit code 19 (ADR-0037), GAP-003 `--llm-backend` user-choice flag (ADR-0038), GAP-004 host-wide LLM slot semaphore via `fs4` (ADR-0039), GAP-005 stderr-capture fallback chain that mitigates the codex OAuth 401 incident of 2026-06-14 (ADR-0040).

- **GAP-001 (ADR-0036)**: `pending_memories` table (V014) buffers the body, entities and relationships separately; SIGTERM during stage 2 or 3 leaves the row in `queued` for reprocessing. Inspect with `sqlite-graphrag embedding list --status pending --json`; the `pending` family itself was REMOVED in v1.2.8.
- **GAP-002 (ADR-0037)**: `SHUTDOWN_EXIT_CODE = 19` constant in `src/constants.rs`; any LLM-spawning command that receives SIGTERM/SIGINT/SIGHUP emits a deterministic JSON envelope on stdout. Envelope fields: `error`, `code`, `signal`, `graceful`, `message`. Schema: `docs/schemas/shutdown-envelope.schema.json`.
- **GAP-003 (ADR-0038)**: `--llm-backend <codex|claude|none,codex,...>` global flag; first non-error backend wins. `--llm-backend codex,claude,none` paired with `--skip-embedding-on-failure` allows null embedding when both backends fail.
- **GAP-004 (ADR-0039)**: Host-wide LLM slot semaphore via `fs4 = "0.9"` with `sync` feature (NOT `fs2`); `fcntl(F_SETLK)` on Linux/macOS, `LockFileEx` on Windows. Default `min(ncpus, oauth_tier_max)`. Inspect with `sqlite-graphrag slots status --json`; reap with `sqlite-graphrag slots release --slot-id <N> --yes`.
- **GAP-005 (ADR-0040)**: `pending_embeddings` table (V015) holds rows that failed every backend; the stderr-capture chain detects `refresh_token_reused` (2026-06-14 codex incident) and routes to the next backend. Inspect with `sqlite-graphrag embedding status|list --json`; retry through `sqlite-graphrag enrich --operation re-embed --json`; the documented `pending-embeddings process` never shipped.
## What Changed in v1.0.85, v1.0.85.1, v1.0.85.2 (ADR-0043, ADR-0044)

Since v1.0.84 (GAP-002 Claude backend split, ADR-0042), three further releases tightened the embedder:

### v1.0.85 — Five-Gap Remediation (ADR-0043)
- `FallbackReason` enum extended from 3 to 7 variants: `embedding_failed | slot_exhausted | oauth_quota | backend_mismatch | dim_zero | cancelled | timeout`
- `reason_code` discriminator in `recall` and `hybrid-search` envelopes distinguishes quota vs mismatch vs timeout
- `try_embed_query_with_deterministic_fallback` retries on `OAuthQuota` and applies 750ms ceiling on `SlotExhausted` before falling back to FTS5
- 12-14 `anthropic-ratelimit-*-remaining` headers captured in `LlmEmbedding::invoke_claude` (G45-CR5); `0` aborts embed and triggers codex fallback
- `dim 64` lock (Matryoshka Representation Learning, arXiv 2205.13147) reduces OAuth token spend by 6x (G56)
- 5 regression tests in `tests/embedder.rs`: `slot_exhaustion_returns_typed_error`, `oauth_quota_fallback_deterministic`, `anthropic_ratelimit_headers_captured`, `read_notfound_preserves_identifier`, `embedding_dim_reduces_token_cost`

### v1.0.85.1 — `recall`/`hybrid-search` `--llm-backend none` Graceful Fallback (GAP-004 hotfix)
- `--llm-backend none` now returns exit 0 with `vec_degraded: true` + `source: "fts_fallback"` + `vec_degraded_reason: "dim_zero"`
- Failsafe of v1.0.80 restored for the `--llm-backend none` case
- Intermediate arm `Ok((v, _backend)) if v.is_empty() => Err(FallbackReason::DimZero)` in `try_embed_query_with_choice`

### v1.0.85.2 — `embed_via_backend` Resolved Kind, --dry-run-backend Standalone (BUG-001/002/003, ADR-0044)
- --dry-run-backend (REMOVED in v1.2.0) worked standalone (no subcommand required) thanks to `pub command: Option<Commands>` in `src/cli.rs:248`
- `embed_via_backend` returns `Result<(Vec<f32>, LlmBackendKind), AppError>` propagating `resolved_kind`
- 7 envelopes now report `backend_invoked: "claude" | "codex" | "none"` consistently
- `setup_mock_path()` in `tests/embedder.rs:37-77` aligned to emit JSON (not JSONL)

### v1.0.84 — Claude Backend Split (ADR-0042, GAP-002)
- `--llm-backend claude` now forces `claude -p` invocation, no silent codex fallback
- `LlmEmbeddingBuilder` in `src/extract/llm_embedding.rs` with `with_claude_builder`, `with_codex_builder`, `override_binary`, `override_model`
- `embed_via_claude_local` in `src/embedder.rs:190+` is the real split entry point
- `apply_env_whitelist_for_claude` in `src/spawn/env_whitelist.rs` (shared by `invoke_claude` and `embed_via_claude_local`)
- 5 regression tests in `tests/embedder.rs`: `embed_via_backend_claude_does_not_invoke_codex`, `embed_via_backend_codex_does_not_invoke_claude`, `embed_via_backend_none_returns_empty_vector`, `cli_dry_run_backend_prints_resolved_path`, `claude_invocation_uses_isolated_config_dir`

### Migration Procedure (Operators on v1.0.80 / v1.0.81)
```bash
# 1. Backup before upgrade (recommended)
sqlite-graphrag backup --output /var/backups/graphrag-pre-v1-0-82.sqlite --json

# 2. Install v1.0.82
cargo install sqlite-graphrag --version 1.0.82 --force
sqlite-graphrag --version   # should report 1.0.82

# 3. Migrations V014 and V015 run automatically on first init/migrate
sqlite-graphrag migrate --json

# 4. codex login is MANDATORY after upgrade (OAuth 401 mitigation)
codex login

# 5. Smoke test the new subcommands
# the pending family was REMOVED in v1.2.8 -- use the two queue commands below
sqlite-graphrag slots status --json
sqlite-graphrag embedding status --json
sqlite-graphrag pending-embeddings list --json
```

See [MIGRATION.md](MIGRATION.md) for the full 6-step procedure including rollback.


## What v1.0.80 Changed (G45, G53, G55 S2, G56, G58, ADR-0033, ADR-0034)

v1.0.80 is a **patch** bump with NO database migration. The schema
is still v13, the G43 dim-adoption already runs in every
`open_rw` and `open_ro`, and the changes are all additive at
the binary and database level. Library consumers must pin to
`=1.0.80` because the lib API is unstable within v1.x.y
(ADR-0032).

- **G45 cross-process embedding singleton**: `acquire_embedding_singleton(namespace, db_path, wait_seconds, force)` serialises LLM embedding calls per `(namespace, db)` pair across concurrent CLI invocations. A second CLI trying to embed against the same database receives `AppError::EmbeddingSingletonLocked { namespace }` (exit 75, retryable). Pass --wait-embed-singleton <SECONDS> (REMOVED in v1.2.0 with the embedding singleton) to poll until the lock drops; distinct databases or namespaces acquire independent locks. Operationally prevents the "two remember invocations, two LLM subprocesses, two parallel batches" pathology that v1.0.79's in-process cache could not address.
- **G53 stability policy and `semver-checks` CI gate**: the public contract is the CLI; the library API is unstable in v1.x.y. New CI job `semver-checks` runs `cargo semver-checks check-baseline --baseline-version 1.0.79` in informational mode (becomes blocking in v1.0.81 once the 9 outstanding MAJOR violations are resolved). README and CHANGELOG carry the `Stability Policy` section. Pin to `=1.0.80` for lib consumers; use `^1.0` to stay on the CLI-stable track.
- **G55 S2 structural `MemoryNotFound`**: the legacy `NotFound(String)` path that masked which lookup target failed is replaced by `AppError::MemoryNotFound { name, namespace }` and `AppError::MemoryNotFoundById { id }` inside `read` and `hybrid-search`. The identifier is now part of the variant, eliminating the `not found: unknown` class of bugs. pt-BR messages carry the name and namespace explicitly.
- **G56 entity-embed in-process cache**: `embed_entity_texts_cached` sits in front of `embed_passages_parallel_local` for entity-name batches. Cache key is `blake3(model || "\0" || text)`. High hit rate in `ingest` (canonical entities re-embedded across many memories), modest in `remember` and `remember-batch`. `remember.rs`, `ingest.rs` and `remember_batch.rs` all route entity embeds through the cache; chunk embeds continue through the raw path. Stats are emitted via `tracing::debug!` (hit / miss / request counts).
- **G58 FTS5 fallback for `recall` and `hybrid-search`**: `recall --fallback-fts-only` and `hybrid-search --fallback-fts-only` route the query through FTS5 BM25 when the LLM subprocess fails (rate limit, OAuth contention, divergent dim). New envelope fields `vec_degraded` (bool), `vec_error` (string) and `warning` (string) are populated symmetrically across both commands. The `recall` and `hybrid-search` tests gained coverage for the FTS5-only path; 1 test is `#[ignore]` because the G58 S1 stub requires `PATH` without `codex` or `claude` to exercise `EmbeddingFailed`.
- **G53-WINDOWS-INFRA (ADR-0033)**: the `clippy` and `test` jobs of the windows-2025 matrix gained 2 new steps each (gated `if: matrix.os == 'windows-2025'`, no-op on ubuntu/macos): a pre-warm that downloads the rustup toolchain into the runner cache before the build, and a verify step that re-checks `rustup show active-toolchain` after install. The 2 historical infra failure modes (rustup download with transient network errors and `E0463 can't find crate for core` when the target stdlib is missing) are now recoverable on the first re-run instead of accumulating as red CI. Local cross-compile validation: `cargo check --target x86_64-pc-windows-msvc --lib --all-features` reproduces and `E0463` is fixed by `rustup target add x86_64-pc-windows-msvc --toolchain 1.88`; the build then reaches the `cc-rs: failed to find tool "lib.exe"` frontier, which is the expected host-Linux cross-compile limit.
- **SHUTDOWN resilience (ADR-0034)**: `src/signals.rs` is wrapped in a panic-catching boundary; even when the parent's stderr is a closed pipe (the orphaned-process scenario that the G42/C2 audit identified), the handler returns cleanly instead of `SIGABRT`-ing on `BrokenPipe`. The third consecutive Ctrl-C exits with code 130 and ZERO I/O, matching the contract documented in ADR-0034 and the recipe in `docs/HEADLESS_INVOCATION.md`. The 3-layer SHUTDOWN bypass recipe (`nohup` then `setsid` then `disown`) is the canonical reference for the agent harness when running long embedding jobs in background.

## What v1.0.79 Changed (G42 + G43)

The G42 work made the embedding pipeline fast, parallel and
batched; G43 made the dimensionality adoption universal:

- Default embedding dimensionality dropped from 384 to 64
  (HISTORICAL: `SQLITE_GRAPHRAG_EMBEDDING_DIM` is not read at runtime; use `--embedding-dim` or XDG `embedding.dim`, range
  [8, 4096]); pre-existing databases keep their recorded
  `schema_meta.dim` on every command (`open_rw`/`open_ro`
  adoption, G43).
- Embedding calls are batched (`{items:[{i,v}]}`; chunks at 8,
  entity names at 25 at dim 64; dim-adaptive — G44) and run in parallel under a bounded
  semaphore: `--llm-parallelism` on `remember` (default 4),
  `ingest` (default 2) and `edit` (default 4), clamp [1, 32].
- HISTORICAL: `SQLITE_GRAPHRAG_CLAUDE_EMBED_MODEL` is not read at runtime; it selected the claude
  embedding model. `SQLITE_GRAPHRAG_EMBED_TIMEOUT_SECS` is not read either;
  use `--openrouter-timeout` or XDG `embedding.timeout_secs` (default 300).
- `enrich --operation re-embed` and `edit --force-reembed` are
  the canonical re-embed paths.
- The remaining daemon code was deleted; the `embedding-legacy`
  and `ner-legacy` features were removed; `--enable-ner` is
  URL-regex only; the GLiNER-era flags were REMOVED in v1.1.02 (--gliner-variant is rejected by clap with exit 2, `--mode gliner` is rejected, and the `SQLITE_GRAPHRAG_GLINER_MODEL`/`SQLITE_GRAPHRAG_GLINER_THRESHOLD` env vars are silently ignored).


## What v1.0.76 Changed

The default build is now **LLM-only and one-shot**. There is no
local embedding model, no GLiNER NER, no ONNX runtime, no
`sqlite-vec` C extension. Every `remember` / `ingest` / `edit`
spawns a headless LLM subprocess (claude code or codex CLI) that
returns the embedding and (optionally) the extracted entities.

The CLI is one-shot: there is no daemon, no model to keep in
memory, no socket to clean up. The release binary is ~14.6 MiB (was
39 MB) and the cold start is 1-3 s (was 30 s with the ONNX model
load).


## Prerequisites

You need an **OpenRouter API key**. No CLI has to be installed and nothing
has to be on `PATH`: the headless `claude` / `codex` / `opencode` backends
were REMOVED in v1.2.0, and embedding is a plain REST call.

Store the key in the XDG config — never in a shell variable, never in argv:

```bash
echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin
sqlite-graphrag config doctor --json
```

`config doctor` reports whether the key resolves before you spend anything.
`sqlite-graphrag health --json` carries the same signal as the `embedding_key`
check (named `llm_cli` up to v1.2.4).


## Credentials

Keys live in `~/.config/sqlite-graphrag/config.toml` with mode `600`, are
zeroized on drop and are never logged. Precedence is the flag
`--openrouter-api-key` first, then the XDG store, then nothing.

FORBIDDEN as the configuration mechanism: any product environment variable
`SQLITE_GRAPHRAG_*`. It is not read on the hot path, so exporting one changes
nothing and hides the real setting.


## Install

```bash
cargo install sqlite-graphrag --version 1.2.5 --force
```

Verify:

```bash
sqlite-graphrag --version
# sqlite-graphrag 1.2.5
```

For the legacy fastembed pipeline (REMOVED in v1.0.79):

```bash
# REMOVED in v1.0.79: the embedding-legacy feature no longer exists.
# Versions 1.0.76-1.0.78 accepted it; pin one of those versions if you
# absolutely need the legacy fastembed pipeline (unsupported).
```


## Initialize a Database

```bash
sqlite-graphrag init --namespace my-project
```

The `init` command:

1. Creates `graphrag.sqlite` in the current directory.
2. Runs all migrations including V013 (drops vec tables, creates
   `memory_embeddings` / `entity_embeddings` / `chunk_embeddings`).
3. Spawns the LLM once to confirm the OAuth session is valid.
4. Reports `schema_version: 15` on success.

The first `init` is slow (1-3 s LLM round-trip). Subsequent
`init` calls are no-ops (the schema is already at the target
version).


## Persist Your First Memory

```bash
sqlite-graphrag remember \
    --name auth-decision-2026-06 \
    --type decision \
    --description "JWT token rotation strategy with 15-min expiry" \
    --body "We picked JWT with a 15-minute access token and a
    7-day refresh token. The refresh flow uses HttpOnly cookies.
    See https://auth0.com/docs/refresh-tokens for the spec." \
    --entities-file entities.json
```

Where `entities.json` is:

```json
[
  {"name": "JWT", "entity_type": "concept"},
  {"name": "Auth0", "entity_type": "tool"}
]
```

The `remember` command:

1. Calls the LLM to embed the body — batched and parallel since
   v1.0.79 (`--llm-parallelism`, default 4; 1-3 s per call).
2. Stores the memory in `memories` (FTS5 indexed).
3. Stores the embedding as a BLOB in `memory_embeddings`.
4. Links the entities via the `entities` table.
5. Returns JSON with `memory_id`, `version`, `elapsed_ms`.


## Search Memories

The two main search commands are:

```bash
# Exact-token + semantic search, fused via RRF
sqlite-graphrag hybrid-search "auth jwt design" --k 10 --json

# Semantic-only (no FTS5 component)
sqlite-graphrag recall "auth jwt design" --k 5 --no-graph --json
```

For the default namespace size (10k memories or fewer), the
cosine refinement over the embedding BLOB is fast enough
(single-digit ms). For larger namespaces, prefer
`hybrid-search` so FTS5 does the coarse filtering.


## Numeric Argument Ranges (v1.2.7)

Since v1.2.7, thirteen numeric arguments of the read surface carry a
clap range validator. The value is checked at parse time, so a bad
number is refused **before** the database is opened and before any
allocation is sized from it.

| Range | Arguments |
| --- | --- |
| `1..=4096` (top-k) | `recall -k`, `hybrid-search -k`, `related --limit`, `graph entities --limit`, `deep-research --k`, `deep-research --max-results` |
| `1..=1000000` (listing limit) | `export --limit`, `pending --limit`, `pending-embeddings --limit`, `embedding --limit` |
| `1..=64` (hops) | `related --max-hops` (alias `--hops`), `recall --max-hops`, `graph traverse --depth`, `deep-research --max-hops` |
| `1..=64` (sub-queries) | `deep-research --max-sub-queries` |

An out-of-range value exits with **code 2** and a clap range message on
stderr. There is no JSON envelope for this failure: the argument never
reaches the command, so nothing structured has been produced yet.
Treat exit 2 here as a caller bug, not as a retryable condition.

```bash
# Refused at parse time — exit 2, database untouched
sqlite-graphrag related --db ./graphrag.sqlite jwt --limit 999999999

# Accepted
sqlite-graphrag related --db ./graphrag.sqlite jwt --limit 50 --json
```

Before v1.2.7 an oversized `related --limit` was forwarded to
`Vec::with_capacity`, and the process aborted on the allocation with no
envelope at all. The range validator replaces that abort with a
deterministic parse-time refusal.

The ceilings are single-sourced in `src/constants/search.rs` as
`K_QUERY_RANGE_MAX`, `K_LIST_LIMIT_MAX`, `K_MAX_HOPS_CEILING` and
`K_MAX_SUB_QUERIES_CEILING`.


## Extract Entities via the LLM

The default `remember` does URL extraction only. For full NER
(entities + typed relationships), use the LLM backend:

```bash
sqlite-graphrag remember \
    --name design-review-q2 \
    --type note \
    --description "Q2 design review notes" \
    --body "$(cat design-review.md)"
# the extraction-backend selector was REMOVED in v1.2.0; extraction now follows
# the configured OpenRouter backend
```

The LLM returns structured JSON with entities and relationships
in the same prompt that produces the embedding. The total round-trip
is 3-8 s (longer than the embed-only path because the prompt
includes the schema and the response is larger).


## LLM Quality Tools (inherited from v1.0.69)
### `enrich` — LLM-Augmented Graph Quality
- The `enrich` subcommand runs LLM-curated graph-quality operations. Fully implemented (persist): `memory-bindings` (extract entities from orphan memories), `augment-bindings` (extra bindings on already-bound memories; requires `--names`/`--memory-names`/`--names-file`), `entity-descriptions` (fill NULL/empty **or** rewrite low-quality with `--force-redescribe`; multi-domain prompt + grounding; v1.1.8), `body-enrich` (expand short memory bodies), `re-embed` (vectors only), `entity-connect` (fully implemented — **persists** relationships; v1.1.04+ convergent via `entity_connect_seen`; **v1.1.06** O(k) co-occurrence + hub×island; **v1.1.8** adaptive budget/yield/`--anchor-memory`/`preempted_for_gate`), `cross-domain-bridges` (same fully-implemented path), `body-extract` with `--body-extract-graph-only` (graph only, no body rewrite) and without it (rewrites `memories.body`), `weight-calibrate` (persists `relationships.weight`), `relation-reclassify` (persists `relationships.relation` and weight), `entity-type-validate` (persists `entities.type`), `description-enrich` (persists `memories.description`), `domain-classify` (persists `memories.metadata`), and `deep-research-synth` (persists extracted entities and relationships).
- Name filters: `--entity-names` for entity-scoped ops (entity-descriptions); `--memory-names` for memory-scoped ops (memory-bindings, augment-bindings); `--names` remains a backward-compatible alias with per-operation semantics.
- The only scan-and-report operation that never writes is `graph-audit`: it surfaces a candidate list and leaves graph structure untouched.
- Treat every other operation as a paid write. A drain you authorise expecting a read-only report will mutate the database.
- `--mode openrouter` selects the JUDGE provider and is **REQUIRED** — there is NO default (the `claude-code` default was removed in v1.0.94). `claude-code`, `codex` and `opencode` are OAuth-only local CLIs; `openrouter` (v1.0.95) calls the `/chat/completions` REST endpoint with no subprocess.
- With `--mode openrouter` (v1.0.95): `--openrouter-model` is REQUIRED (NO default; omitting it → exit 1 before any network call). `--openrouter-api-key` reads from XDG via `config add-key` (OPENROUTER_API_KEY is not read at runtime) or `config add-key --provider openrouter`. `--openrouter-timeout` defaults to 300s. `--openrouter-base-url` is optional. Example: `enrich --operation memory-bindings --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" --json`.
- `--preflight-check` confirms the OpenRouter key resolves BEFORE scanning the candidate set. Default off to keep `--dry-run` and CI flows zero-cost.
- `--rate-limit-buffer <SECONDS>` defaults to 300. When the preflight probe detects that the OAuth rate-limit reset is less than the buffer away, it aborts with a suggestion to wait.
- `--names <a,b,c>` and `--names-file <PATH>` select a specific subset of memory names instead of scanning all candidates. `--names-file` accepts `#` comments and blank lines. Both flags combine as a union when both are set.
- `--preserve-threshold <FLOAT>` (default 0.7) controls the Jaccard trigram similarity gate for `body-enrich`. When the LLM rewrite scores below the threshold, the enriched body is REJECTED and emitted as `EnrichItemResult::PreservationFailed`. Protects against LLM invention.
- `--llm-parallelism <N>` spawns N parallel LLM worker threads (default 1, max 32). Codex tolerates up to 16 in production; Claude warns above 4 because of the OAuth-MCP fan-out. Since v1.0.79 the same flag also exists on `remember` (default 4), `ingest` (default 2) and `edit` (default 4) for the embedding fan-out.
- `--max-load-check` refuses to start when the 1-minute load average exceeds `2 × ncpus`. Set to false on contended CI runners.
- `--circuit-breaker-threshold <N>` (default 5) aborts the job after N consecutive `HardFailure` outcomes. Transient rate-limit and timeout errors do not count.
- `--dry-run` previews the candidate set without spawning any LLM. Output is NDJSON with one event per memory and a final summary.
- `--resume` continues a previously interrupted batch from the queue DB. `--retry-failed` retries only the failed items.
- `--until-empty` (v1.0.96) runs an internal scan→drain loop until the queue holds no eligible items or `--max-runtime <SECONDS>` (default 3600) expires — it replaces the external `while` retry loop. `--max-attempts <N>` (default 8, range 1..=20) is the Transient retry budget; an item turns terminal `dead` after that budget or on the first HardFailure (GAP-ENRICH-BACKLOG-CONVERGE, ADR-0055).
- `--status` (v1.0.96) prints a read-only JSON queue report (`unbound_backlog`, per-operation `scan_backlog`, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`). It never calls the LLM and never acquires the singleton, so it is safe to poll while a drain is running. `scan_backlog` (GAP-SG-77, v1.1.0) is the real per-operation database backlog a scan would enqueue — it kills the false `pending=0` for `entity-descriptions`/`body-enrich`/`re-embed`, and `state` derives `pending-scan` from it.
- `--list-dead` / `--requeue-dead` list terminal `dead` queue rows or move them `dead` → `pending` (no LLM, no singleton when used alone). Use after hard failures that exhausted `--max-attempts`.
- `--list-skipped` / `--requeue-skipped` list `skipped` / preservation-failed rows or move them `skipped` → `pending` (no LLM, no singleton when used alone). Recovers preservation/skipped debt without raw SQL on `.enrich-queue.sqlite`.
- `--rest-concurrency <N>` (v1.0.96, default 8, clamp 1..=16) caps the bounded `JoinSet` REST fan-out for `--mode openrouter`; it is distinct from `--llm-parallelism`. Embedding batches 32 passages with per-chunk order preserved while the SQLite write stays single-writer via WAL + atomic claim (GAP-OPENROUTER-REST-CONCURRENCY).
- `--prune-dead-orphans` (v1.0.97, GAP-SG-66, ADR-0058) is a read-only inspector (no LLM, no singleton, no `--operation`/`--mode`) that deletes ONLY enrich-queue rows with `status='dead'` and `item_type='memory'` whose `item_key` (the memory name) is absent from the main database; entity-keyed dead rows are untouched and only the `.enrich-queue.sqlite` sidecar is mutated. The JSON `DeadSummary` reports a `pruned` count. Use it to clear orphan dead-letter left when a memory is renamed or purged after it was enqueued — `--requeue-dead` would only re-fail those.
- `--prune-dead-entity-orphans` (v1.1.02, ADR-0062) is the entity-keyed counterpart: it deletes dead-letter rows with `item_type='entity'` from `.enrich-queue.sqlite`, and is mutually exclusive with `--prune-dead-orphans`. Run both in sequence for a full orphan sweep after an upgrade that renamed/merged/purged entities.

- `--reset-stale-claims` (v1.1.03, enrich) manually resets every processing claim older than the stale threshold back to `pending`. Use it after a hard crash that bypassed the startup auto-reset.
- `--stale-claim-secs <N>` (v1.1.03, enrich) overrides the staleness threshold used by both the startup auto-reset and `--reset-stale-claims`.
- `--literal-to <RELATION>` (v1.1.03, `reclassify-relation`) is the verbatim TARGET counterpart to `--literal-from`; together they migrate stored underscore literals (`applies_to`) to canonical hyphen form (`applies-to`) without clap normalization on the target side.
- `--cross-namespace` (v1.1.03, `merge-entities`) is an opt-in flag that lets `--ids`/`--into-id` resolve entities across ALL namespaces; default is same-namespace-only (safe) so a stray id cannot silently merge foreign data.
- `split-body` (v1.1.03, new subcommand) divides oversized memory bodies into daughters `{name}-part-{i}`, marks the original with metadata `superseded_by_split: true`, and creates canonical `replaces` relations from each daughter to the original. Use `split-body --name <N>` for one memory or `split-body --batch --threshold 25000` for every oversized body; daughters are NOT embedded inline — run `enrich --operation re-embed --target memories` afterward.
- `--target <memories|entities|chunks|all>` (v1.1.01) selects which embedding table `re-embed` backfills; only valid with `--operation re-embed` (fails loud otherwise). `--status` reports the per-target `scan_backlog`.
### `vec` — Vector Index Maintenance (G39)
- `vec orphan-list --json` lists memory embedding rows whose `memory_id` no longer exists in the `memories` table. Each row reports the `vector_hash` (BLAKE3 of the embedding blob) for traceability.
- `vec purge-orphan --yes --dry-run --json` previews the deletion count without removing anything.
- `vec purge-orphan --yes --json` purges the THREE vec tables (`vec_memories`, `vec_entities`, `vec_chunks`) in a single implicit transaction. The response reports `deleted`, `deleted_entities`, `deleted_chunks`, and `elapsed_ms`.
- `vec stats --json` exposes `vec_memories_rows`, `vec_entities_rows`, `vec_chunks_rows`, `orphans`, and the last vacuum timestamp. Use it to audit vector-table health after bulk `forget` cycles.
- The `forget` subcommand now calls `memories::delete_vec` BEFORE the soft-delete, preventing new orphans in the steady state.
### `optimize` and `backup` Hardening (G36 + G38)
- `optimize` pre-checks FTS5 health via `check_fts_functional` before rebuilding, and the skip is OPT-IN: pass `--fts-skip-when-functional` to leave a healthy index alone (saves ~10 minutes on a 4.3 GB database). Without the flag the rebuild always runs. The negated spelling --no-fts-skip-when-functional does not exist in v1.2.8 and clap rejects it with exit 2.
- `optimize --fts-dry-run --json` exits 1 if the FTS5 index needs a rebuild, 0 otherwise. CI-friendly.
- `optimize --fts-progress <N>` (default 30) emits a progress line every N seconds during the rebuild. Set to 0 to disable.
- `optimize --yes` skips the confirmation prompt. Required for non-interactive CI.
- `backup` defaults to `run_to_completion(1000, Duration::from_millis(5), None)` (was 100/50ms). For a 4.3 GB database this is a 25x speedup (~21s vs ~9 min).
- `backup --backup-step-size <PAGES>` and `--backup-step-sleep-ms <MS>` tune the page-copy granularity. `--backup-no-sleep` removes the inter-step sleep entirely for maximum throughput. `--backup-progress <PAGES>` (default 100) emits a progress line every N pages.
### `migrate` Subcommand Family (v1.0.76, updated v1.0.77 and v1.0.78)
- `migrate --rehash --json` rewrites recorded migration checksums to match the current file content. Idempotent. Required for v1.0.74 → v1.0.76 upgrades where the V002 migration was intentionally emptied to a no-op.
- `migrate --to-llm-only --drop-vec-tables --json` is the one-shot upgrade for v1.0.74 / v1.0.75 databases. Combines `--rehash` with the V013 vec-table drop. The `--drop-vec-tables` flag is REQUIRED as an explicit safety guard. The BLOB-backed `memory_embeddings` / `entity_embeddings` / `chunk_embeddings` tables remain and are the source of truth going forward; embeddings are recomputed lazily on the next `remember` / `edit` / `ingest`.
- v1.0.77 fix (G40): JSON response for both commands now includes `null_rows_fixed` (integer) and `vec_tables_removed_via_writable_schema` (integer). Databases with `applied_on = NULL` rows are auto-sanitized before the migration runner executes.
- v1.0.78 fix (G41): JSON response for both commands now includes `v013_tables_created` (boolean). Databases where V013 was registered in `refinery_schema_history` but the BLOB-backed embedding tables were never created are auto-repaired. Any CRUD command also triggers this repair unconditionally via `ensure_db_ready`.


## Migration from v1.0.74 / v1.0.75

See [MIGRATION.md](MIGRATION.md) for the full step-by-step. The
short version:

1. Install v1.0.76 (LLM-only).
2. Run `sqlite-graphrag init` — migration V013 runs automatically.
3. Old vec tables are dropped; new `memory_embeddings` is empty.
4. Memories are re-embedded lazily on the next `edit` / `ingest`.

For a large corpus, use the canonical one-shot re-embed loop
(G42/S9, v1.0.79) — each invocation processes a small batch and exits:

```bash
sqlite-graphrag enrich --operation re-embed --limit 5 --resume --mode openrouter --openrouter-model MODEL --json
```

Note: the old `edit --description "<same>"` recipe never re-embedded
anything (description-only edits are a no-op for embeddings); use
`edit --force-reembed` for a single memory.


## Running the Test Suite

This project ships no CI. `cargo test` is the release gate, and it runs on
your machine.

No LLM CLI has to be on `PATH`. Through v1.1.x the suite needed `claude` or
`codex` installed, because embedding went through a subprocess. v1.2.0 removed
those backends entirely: the only backends left are `openrouter` (REST) and
`none`, and every test that needs an embedding either uses `none` or a local
fixture. A CLI on `PATH` changes nothing.

```bash
# The release gate. Run it WITHOUT --no-fail-fast at least once: the first
# binary that exits abnormally stops every binary after it, and that cascade is
# how v1.2.4 shipped with 61 of 87 suites never launched (GAP-SG-189).
cargo test --all-features

# Full assertion count once the gate is green.
cargo test --all-features --no-fail-fast

# Doctests run last, so the cascade above hides them completely.
cargo test --doc

cargo clippy --all-targets --all-features -- -D warnings
```

v1.2.5 measures 90 test binaries and 1915 assertions, zero failures.

Tests that spend real OpenRouter credits are marked `#[ignore]`, so the command
above never bills anything. Run them deliberately:

```bash
cargo test --test openrouter_chat_real -- --ignored --nocapture
cargo test --test openrouter_live_concurrency -- --ignored --nocapture
```

See [TESTING.md](TESTING.md) for the per-suite breakdown.


## Complete CLI command inventory (v1.2.5)

Top-level commands (from `sqlite-graphrag --help`) with a one-line purpose:

- `init` — create/open the SQLite DB, apply migrations, smoke-test the LLM path
- `remember` — write one memory with optional curated entity graph
- `remember-batch` — create many memories from NDJSON stdin (description required on create)
- `ingest` — bulk-ingest files under a directory as memories
- `recall` — semantic (KNN) memory search with optional graph hops
- `read` — fetch one memory by name or id
- `list` — paginate memories with filters
- `forget` — soft-delete a memory (history kept)
- `purge` — hard-delete soft-deleted memories past retention (`--now` for immediate)
- `rename` — rename a memory while keeping versions
- `split-body` — split an oversized memory body into daughter memories
- `edit` — edit body/description/type and optionally re-embed
- `history` — list versions of a memory
- `restore` — restore a memory to a previous version
- `hybrid-search` — FTS5 + vector fused via Reciprocal Rank Fusion
- `health` — integrity, FTS5, sqlite version, vector coverage, super-hubs
- `migrate` — apply pending schema migrations (or `--dry-run` / `--rehash`)
- `namespace-detect` — resolve namespace precedence for this invocation
- `optimize` — `PRAGMA optimize` and optional FTS5 rebuild
- `stats` — counts of memories, entities, relationships
- `sync-safe-copy` — checkpoint then copy a cloud-sync-safe snapshot
- `backup` — Online Backup API copy to a destination path
- `vacuum` — WAL checkpoint + reclaim disk space
- `link` — create an entity-to-entity relationship
- `unlink` — remove relationships or a memory–entity binding
- `deep-research` — multi-hop GraphRAG research via query decomposition
- `related` — list memories graph-connected from a seed memory
- `graph` — export graph snapshot (`json`/`dot`/`mermaid`) or run graph subcommands
- `export` — export memories as NDJSON
- `fts` — FTS5 index management family
- `vec` — vector table maintenance family
- `prune-relations` — bulk-delete all relationships of a given type
- `prune-ner` — remove NER bindings from `memory_entities`
- `slots` — host-wide LLM slot semaphore inspection/cleanup
- `pending` — three-stage `remember` checkpoint queue
- `embedding` — pending-embeddings queue health and list
- `pending-embeddings` — batch ops on the embedding retry queue
- `cleanup-orphans` — remove entities with no memories and no relationships, plus every row `PRAGMA foreign_key_check` reports across the eleven child tables (the repair the migration warning points at)
- `memory-entities` — list entities for a memory (or reverse via `--entity`)
- `cache` — XDG model-cache list/stats/clear
- `delete-entity` — delete an entity and cascade its edges
- `reclassify` — reclassify entity types (single or batch)
- `rename-entity` — rename an entity preserving edges and bindings
- `merge-entities` — merge source entities into a target
- `enrich` — LLM-augmented graph quality pipeline and queue inspectors
- `reclassify-relation` — bulk rename relationship types (literal or normalized)
- `normalize-entities` — normalize entity names to kebab-case with auto-merge
- `completions` — generate shell completions
- `config` — XDG operational config and API keys

### Nested families

- `config`
  - `add-key` — store an API key (stdin) for a provider
  - `list-keys` — list masked key fingerprints
  - `remove-key` — delete a stored key
  - `doctor` — diagnose key/config resolution layers
  - `path` — print resolved XDG config file path
  - `set` — persist an operational setting
  - `get` — read one setting
  - `list` — list stored settings (`--effective` includes defaults)
  - `unset` — remove a stored setting
- `graph`
  - `traverse` — BFS walk from an entity (`--fuzzy` for short nicknames)
  - `stats` — node/edge counts and degree distribution
  - `entities` — list entities with sort/filter
  - `recompute-degree` — rebuild cached `entities.degree` in one transaction
- `fts`
  - `rebuild` — rebuild FTS5 from scratch
  - `check` — integrity-check without modifying the index
  - `stats` — FTS5 row/shadow-page statistics
- `vec`
  - `orphan-list` — list orphan embedding rows
  - `purge-orphan` — delete orphan rows from vec tables
  - `stats` — vec table row counts and orphan stats
- `slots`
  - `status` — held slots, PIDs, wait metrics
  - `release` — force-release a slot by id (`--yes`)
  - `cleanup` — reap stale/orphan slot files
- `pending`
  - `list` — list checkpoint-queue rows
  - `show` — show one checkpoint entry by id
  - `cleanup` — remove terminal-state rows
- `embedding`
  - `status` — queue health + vector coverage
  - `list` — per-entry inspection
  - `abandon` — abandon matching pending embeddings
- `pending-embeddings`
  - `list` — list embedding-retry rows
  - `status` — alias of `embedding status`
  - `abandon` — abandon matching retry rows
- `cache`
  - `clear-models` — remove cached model files
  - `list` — list cache files and sizes
  - `stats` — alias of `list`
- `enrich` key inspectors (no LLM / no singleton when used alone)
  - `--status` — read-only queue + scan backlog report
  - `--list-dead` — list terminal `dead` rows
  - `--requeue-dead` — move `dead` → `pending`
  - `--list-skipped` — list `skipped` / preservation-failed rows
  - `--requeue-skipped` — move `skipped` → `pending`
  - `--prune-dead-orphans` — drop memory-keyed dead rows missing from main DB
  - `--prune-dead-entity-orphans` — drop entity-keyed dead rows from the sidecar
- `enrich` write flags relevant in **v1.2.1**
  - `--until-empty` — loop scan→drain until eligible queue empty or `--max-runtime`; counts **this op+namespace only**
  - `--force-redescribe` — reopen `skipped`/`done` once per process for entity-descriptions low-quality rewrite; never reopens `dead`
  - `--operation re-embed --target memories|entities|chunks|all` — BLOB-length eligibility + zombie reconcile
  - `--namespace` — claim, count, and resume are namespace-scoped
  - `--mode openrouter` / `--rest-concurrency` — REST judge/embed fan-out
- Global output flags added in **v1.2.2** (apply to every subcommand)
  - `--select <KEYS>` / `--fields` — keep only these keys per result element; dotted paths OK; missing key skipped, never `null`
  - `--filter <EXPR>` — `key=value`, `key!=value`, `key~substring`; `==` synonym of `=`; repeat to conjoin with AND; malformed exits 2
  - `--max-items <N>` — cap on emitted elements, applied after filtering; distinct from per-subcommand `--limit` and from `-k`
  - `--sort <KEY>` — ascending by dotted path; numbers numeric, rest text
  - `--dedupe-by <KEY>` — drop later elements repeating the value
  - `--count-only` — payload becomes `{"count": N}`
  - `--truncate-content <N>` — shorten strings past N characters, never bytes
  - `--max-output-bytes <N>` — cap the envelope by dropping trailing elements, never by slicing JSON
  - Failure envelopes (`error: true` / `ok: false`) and `$schema` documents are never reshaped; NDJSON streams bypass the surface
- Global input flag added in **v1.2.2**
  - `--no-input` — refuse stdin anywhere in the invocation; every stdin reader fails up front with exit 1; precedence flag > XDG `cli.no_input` > `false`
- `schema` — machine-readable catalog of all **76** JSON contracts
  - `schema` — NDJSON listing, one `{"id","invoke"}` per line; `invoke` is the ready-to-copy command
  - `schema --name <ID>` — emit that contract's JSON Schema document
  - Unknown `<ID>` exits **4**; `$schema` documents are exempt from the agent-native output surface, so any global flag can be chained safely

> **GAP-SG-139:** host/XDG leaves (`config`, `slots`, `cache`, `completions`) accept `--db` as a documented **no-op** so agents that append `--db` everywhere do not get clap exit 2.

> **Top-level inventory (51 + help):** init, remember, remember-batch, ingest, recall, read, list, forget, purge, rename, split-body, edit, history, restore, hybrid-search, health, migrate, namespace-detect, optimize, stats, sync-safe-copy, backup, vacuum, link, unlink, deep-research, related, graph, export, fts, vec, prune-relations, prune-ner, slots, pending, embedding, pending-embeddings, cleanup-orphans, memory-entities, cache, delete-entity, reclassify, rename-entity, merge-entities, enrich, reclassify-relation, normalize-entities, schema, completions, config, help.

## Complete XDG configuration reference (70 keys)
- Every key below is accepted by `sqlite-graphrag config set <KEY> <VALUE>` and resolves as CLI flag > XDG `config set` > built-in default.
- `(none)` marks a key with no built-in default, so an unset key leaves the subsystem on its own runtime heuristic, on host detection, or on a required CLI flag.
- A key outside this list is refused with exit 1, so a typo never turns into a silent no-op.
- `sqlite-graphrag config list --effective --json` prints the same inventory straight from the running binary.
- The reference is asserted against `src/config/registry.rs` by `tests/docs_xdg_coverage.rs`, so it cannot drift away from the parser.

### Agent-native output surface
- `agent_surface.max_items` — default `0` — standing ceiling for `--max-items`; `0` disables it, and since v1.2.5 it caps every array in the envelope, not only the primary one
- `agent_surface.max_output_bytes` — default `0` — standing ceiling for `--max-output-bytes`; `0` disables it, and the output stays parseable JSON while the stub reports the requested ceiling
- `agent_surface.truncate_content` — default `0` — standing ceiling for `--truncate-content`, the per-field character cap; `0` disables it

### Database and storage
- `db.path` — default `(none)` — default database file, overridden by `--db <PATH>` after the subcommand; with neither, the XDG data directory `~/.local/share/sqlite-graphrag/graphrag.sqlite`
- `db.busy_retries` — default `5` — retries on `SQLITE_BUSY` before exit 15
- `db.busy_base_delay_ms` — default `300` — base delay of the exponential backoff between busy retries
- `db.query_timeout_ms` — default `5000` — per-query wall-clock ceiling
- `cache.dir` — default `(none)` — cache root, falling back to the XDG cache directory

### Embedding
- `embedding.dim` — default `1024` — vector dimensionality; changing it on a populated database silently breaks cosine similarity, so migrate deliberately and never as a flag side effect
- `embedding.model` — default `(none)` — default embedding model, read since v1.2.5
- `embedding.backend` — default `(none)` — default embedding backend, `auto` or `openrouter`
- `llm.backend` — default `(none)` — default LLM backend for embedding, `open-router` or `none`
- `embedding.batch_size` — default `32` — passages per REST embedding request
- `embedding.timeout_secs` — default `300` — per-request embedding timeout
- `embedding.entity_cache_max_entries` — default `10000` — entity-embedding LRU capacity
- `embedding.entity_cache_ttl_secs` — default `3600` — lifetime of one entity-embedding cache entry

### LLM transport and host slots
- `llm.model` — default `(none)` — default text model for graph extraction
- `llm.fallback` — default `none` — backend fallback chain; only `openrouter` and `none` are valid since v1.2.0
- `llm.openrouter_timeout_secs` — default `600` — per-request OpenRouter chat timeout
- `llm.probe_timeout_ms` — default `800` — credential and backend probe timeout
- `llm.max_host_concurrency` — default `(none)` — host-wide ceiling on concurrent LLM work, auto-sized when unset
- `llm.slot_wait_secs` — default `300` — how long to wait for a host slot before giving up
- `llm.slot_no_wait` — default `false` — fail immediately instead of queueing for a slot
- `llm.worker_rss_mb` — default `350` — assumed RSS per worker, used to size concurrency against free memory
- `llm.skip_embedding_on_failure` — default `false` — persist the row without a vector when embedding fails, instead of failing the write

### Enrichment
- `enrich.scan_page_size` — default `512` — keyset page width of the streaming scanners, range 1..=4096
- `enrich.yield_every_n_items` — default `10` — cooperative yield interval during long drains
- `enrich.reembed_claim_batch` — default `32` — rows claimed per `re-embed` transaction
- `enrich.rate_limit_deadline_secs` — default `3600` — wall-clock ceiling while backing off a rate limit
- `enrich.circuit_breaker_reset_secs` — default `60` — cooldown before the breaker closes again
- `enrich.entity_connect.default_limit` — default `100` — candidate pairs per `entity-connect` scan
- `enrich.entity_connect.large_ns_limit` — default `25` — lower ceiling applied to large namespaces
- `enrich.entity_description.domain` — default `auto` — domain hint for generated entity descriptions
- `enrich.entity_description.grounding_threshold` — default `0.30` — minimum grounding score for a description to be kept
- `enrich.entity_description.corpus_top_k` — default `8` — memories sampled as evidence per entity
- `enrich.entity_description.min_corpus_chars` — default `40` — minimum evidence length before the LLM is called; below it the entity is skipped, never described
- `enrich.entity_description.neighbour_top_k` — default `12` — typed graph relations sampled as evidence per entity
- `enrich.entity_description.snippet_chars` — default `2000` — characters per evidence snippet
- `enrich.entity_description.quality_sample` — default `50` — sample size behind `quality_pct` in `enrich --status`
- `enrich.entity_type.allowed_types` — default `(none)` — comma-separated entity-type vocabulary `entity-type-validate` accepts; unset means the canonical set, and `--allowed-types` overrides it
- `enrich.entity_type.on_unknown_type` — default `keep` — what `entity-type-validate` does with a label outside that vocabulary: `keep` stores it as written, `fallback` stores the nearest accepted label and preserves the raw one in the description, `strict` refuses with exit 1; `--on-unknown-type` overrides it
- `enrich.entity_type_validate.corpus_top_k` — default `8` — linked memory bodies shown to `entity-type-validate` as evidence
- `enrich.entity_type_validate.min_corpus_chars` — default `40` — below this the entity has no evidence and the operation abstains without spending a token
- `enrich.entity_type_validate.neighbour_top_k` — default `12` — typed graph relations shown to `entity-type-validate` as evidence
- `enrich.entity_type_validate.snippet_chars` — default `2000` — characters per evidence snippet for `entity-type-validate`

### Search
- `search.hybrid.max_graph_results` — default `50` — graph-match ceiling for `hybrid-search --with-graph`; `0` removes the cap and reopens the unbounded pre-v1.2.2 envelope

### Ingest and write limits
- `ingest.low_memory` — default `false` — trade throughput for a smaller resident set during ingest
- `limits.max_entities_per_memory` — default `50` — entities accepted per write
- `limits.max_relations_per_memory` — default `50` — relationships accepted per write

### Network
- `network.openrouter.chat_url` — default `https://openrouter.ai/api/v1/chat/completions` — OpenRouter chat completions endpoint
- `network.openrouter.embeddings_url` — default `https://openrouter.ai/api/v1/embeddings` — OpenRouter embeddings endpoint
- `network.chat_url` — default `(none)` — alias of `network.openrouter.chat_url`
- `network.embed_url` — default `(none)` — alias of `network.openrouter.embeddings_url`

### Concurrency and process control
- `parallelism.max_total_workers` — default `64` — absolute ceiling on worker tasks
- `parallelism.rayon_threads` — default `(none)` — Rayon pool size, auto-sized when unset
- `parallelism.embed_runtime_threads` — default `(none)` — Tokio worker threads for the embedding runtime, auto-sized when unset
- `system.max_load_per_ncpu` — default `2.0` — load-average ceiling per CPU before new work is throttled
- `cli.max_instances` — default `(none)` — concurrent process ceiling for this CLI, auto-sized when unset
- `retry.disable` — default `false` — disable the built-in retry policy
- `shutdown.ignore` — default `false` — ignore the graceful-shutdown signal path

### CLI behaviour, logging and locale
- `cli.no_input` — default `false` — standing `--no-input`, so every stdin reader refuses up front with exit 1 even when a pipe is attached
- `cli.stdin_timeout_secs` — default `60` — how long a stdin reader waits for input
- `namespace.default` — default `global` — namespace used when `--namespace` is absent
- `display.tz` — default `UTC` — IANA zone for the `*_iso` JSON fields
- `i18n.lang` — default `en` — UI language on stderr; JSON payloads stay in English
- `log.level` — default `warn` — local tracing level on stderr
- `log.format` — default `pretty` — `pretty` or `json`
- `log.to_file` — default `false` — mirror local tracing to a file
- `log.rotation` — default `daily` — rotation policy when `log.to_file` is on
- `log.retention_days` — default `7` — how long rotated logs are kept


## Configuration and maintenance subcommands
- These are the second-level verbs the inventory above names but never shows you how to type.
- Every invocation below was confirmed against `--help` before it was written down, so you can copy it as is.

### Read and remove one operational setting
```bash
sqlite-graphrag config get search.hybrid.max_graph_results --json
sqlite-graphrag config unset search.hybrid.max_graph_results
```

### Inspect and revoke stored API keys
```bash
sqlite-graphrag config list-keys --json
sqlite-graphrag config remove-key <FINGERPRINT>
```

### Keep the FTS5 index honest
```bash
sqlite-graphrag fts stats --db ./graphrag.sqlite --json
sqlite-graphrag fts check --db ./graphrag.sqlite
sqlite-graphrag fts rebuild --db ./graphrag.sqlite
```

### Audit the entity-type vocabulary the database really holds
```bash
sqlite-graphrag graph entity-types --db ./graphrag.sqlite
sqlite-graphrag graph entity-types --db ./graphrag.sqlite --format text
```

### Retire entries stuck in the embedding retry queue
```bash
sqlite-graphrag pending-embeddings abandon --status pending --yes --db ./graphrag.sqlite
```

### Reclaim host slots and disk from the model cache
```bash
sqlite-graphrag slots cleanup --stale-after 3600 --yes
sqlite-graphrag cache list --json
sqlite-graphrag cache clear-models --yes
```

## See Also

- [COOKBOOK.md](COOKBOOK.md) for common recipes
- [MIGRATION.md](MIGRATION.md) for v1.0.74 → v1.0.76 upgrade
- [CROSS_PLATFORM.md](CROSS_PLATFORM.md) for Windows / macOS
- [AGENTS.md](AGENTS.md) for agent integration
- [HEADLESS_INVOCATION.md](HEADLESS_INVOCATION.md) for OAuth-safe Claude/Codex/OpenCode headless invocation
- [decisions/](decisions/) for the 45 ADRs
