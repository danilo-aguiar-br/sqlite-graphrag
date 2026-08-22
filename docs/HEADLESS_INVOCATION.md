# Headless Invocation — OpenRouter REST without MCP and without Hooks

> How to drive this project headlessly. The local subprocess backends were
> removed, so there is no CLI to isolate: every LLM call is an OpenRouter REST
> request, and MCP servers and hooks are structurally out of reach.

- Portuguese version of this guide lives in [HEADLESS_INVOCATION.pt-BR.md](HEADLESS_INVOCATION.pt-BR.md)
- Back to [README.md](../README.md) for the command reference


## Summary

- The only LLM transport is the OpenRouter REST API (`reqwest` + `rustls-tls`); no subprocess is ever spawned
- `--llm-backend` accepts `openrouter` (default) and `none`; `--llm-fallback` defaults to `none`
- `--embedding-backend openrouter --embedding-model MODEL` routes embedding through `POST /api/v1/embeddings`
- `enrich --mode openrouter --openrouter-model MODEL` routes the JUDGE turn through `POST /api/v1/chat/completions`
- Because nothing is spawned, there is no MCP config to strip, no hooks to zero out, and no CWD to isolate


## v1.2.1 Update — Enrich CAPA for headless agents (sidecar only)

CAPA themes (enrich queue seal; schema **v16**, no main-DB migration):

1. **`dequeue_next_pending`** — the claim filters by `operation` **and** `namespace` (draining `ai-sdd` MUST NOT process rows from `global` / the empty ns; the same holds for `--resume` / `--retry-failed`).
2. **`count_eligible_pending` for `--until-empty`** — counts only pending rows of this **op+ns** (alien ops / ReEmbed zombies elsewhere no longer keep EntityDescriptions looping with `completed=0`).
3. **`reopen_force_redescribe_candidates`** — `--force-redescribe` reopens `skipped`/`done` **once per process** before the first enqueue (so `INSERT OR IGNORE` is not a silent no-op); it **never** reopens `dead` (use `--requeue-dead`).
4. **`reconcile_satisfied_reembed_pending`** — marks a pending ReEmbed as `done` when a live vector with `LENGTH(embedding) = dim*4` already exists, clearing zombies without API calls.
5. **Re-embed eligibility by BLOB LENGTH** — the predicates use `LENGTH(embedding) = dim*4`, **not** the `dim` column alone (CORRUPT / META_AHEAD rows carrying dim=1024 with a 384-d BLOB become eligible again).
6. **`entity:` prefix stripped on enqueue** — entity lookup uses the bare name while the queue key stays `entity:…` (bare names still work; a missing entity is rejected).
7. **Chunk enqueue validates the namespace** — `chunk_id` must exist in a non-deleted memory of the target namespace (rejects invalid / cross-ns keys before dead-letter and circuit-breaker churn).
8. **CAPA-D** — compound "configuration file" markers only (e.g. `is a configuration file`); no bare false positive on legitimate domain prose.

Queue regressions: `enqueue_candidate_accepts_entity_prefixed_reembed_key`, `dequeue_next_pending_isolates_by_namespace`. Queue unit suite: **38** OK (`cargo test --lib commands::enrich::queue` or `cargo test --lib commands::enrich`).

Ready-made formulas for agents:

```bash
DB="${DB:-$HOME/.local/share/sqlite-graphrag/memory.db}"
MODEL="${MODEL:-deepseek/deepseek-v4-flash:nitro}"
NS="${NS:-global}"

sqlite-graphrag enrich --db "$DB" --status --operation re-embed --namespace "$NS" -q
sqlite-graphrag enrich --db "$DB" --operation re-embed --target entities \
  --mode openrouter --openrouter-model "$MODEL" \
  --until-empty --namespace "$NS" -q --wait-lock 60
sqlite-graphrag enrich --db "$DB" --operation entity-descriptions \
  --mode openrouter --openrouter-model "$MODEL" \
  --force-redescribe --until-empty --namespace "$NS" -q
sqlite-graphrag enrich --db "$DB" --list-skipped --operation entity-descriptions --namespace "$NS" -q
```

- Schema stays **v16** (no main-DB migration). The offline gate is still `scripts/e2e_offline_v120.sh` **20/20**. Pin `=1.2.2`.


## v1.2.0 Update — XDG, dim 1024, list-skipped, GAP-SG-139, headless hot-set

- Product-knob configuration: **CLI flag > XDG `config set` > default**. Product environment variables `SQLITE_GRAPHRAG_*` are **not** read on the hot path. Harnesses MUST use an isolated XDG root plus flags — never export product env as a configuration contract.
- **DEFAULT_EMBEDDING_DIM=1024** (override with `--embedding-dim` / XDG `embedding.dim`; existing databases keep `schema_meta.dim` until a re-embed).
- Recover `skipped` / `preservation_failed` queue debt without raw SQL:
  ```bash
  sqlite-graphrag enrich --list-skipped --json
  sqlite-graphrag enrich --requeue-skipped --json
  ```
- **GAP-SG-139:** host/XDG leaves (`config`, `slots`, `cache`, `completions`) accept `--db` as a documented **no-op** — headless agents can attach `--db` to every spawn without a clap exit 2.
- After a curated `remember`, PARSE `entities_created` / `enrich_recommended` and/or PASS `--enqueue-enrich` for priority entity-descriptions ahead of long entity-connect drains.
- Poll quality without an LLM: `enrich --operation entity-descriptions --status --force-redescribe --json` (`scan_backlog_low_quality`, `quality_pct`, `state` including `blocked_dead`).
- Name filters: `--entity-names` for entity-descriptions; `--memory-names` for memory-bindings.
- Audit bindings: `memory-entities --name <mem> --json` includes `entities[].description`.
- entity-connect is fully implemented (it persists relationships). On large databases expect `budget_exhausted` / `preempted_for_gate`; prefer hot ED → cold EC.
- Product offline gate: `bash scripts/e2e_offline_v120.sh` expects **20/20 PASS** (canonical; the historical `e2e_offline_v118.sh` / 16/16 wrapper is superseded).
- **CURRENT note about the historical text below:** sections teaching product env as configuration describe pre-v1.2.0 behaviour — v1.2.0 does **not** read product env on the hot path (XDG plus flags only). The OAuth / custom-provider env whitelist for LLM subprocesses remains valid and is **not** product-knob configuration.
- Secrets: prefer `config add-key --provider openrouter` (stdin) or `--openrouter-api-key`; `OPENROUTER_API_KEY` is not read at runtime.


## stdout/stderr contract and --quiet (v1.1.05) + `-o` alias (v1.1.8)

ADR: [ADR-0065](decisions/adr-0065-v1-1-05-incident-bugs.md). Regression suite: `tests/v1105_incident_bugs_regression.rs` (suite name **v1105**).

- Structured JSON ALWAYS on stdout; tracing logs ALWAYS on stderr
- Use `--quiet`/`-q` (global) to suppress non-error tracing — useful in headless pipelines that parse stdout with `jaq`
- For large `deep-research` envelopes, prefer `-o PATH` or `--output PATH` (atomwrite atomic write) instead of redirecting stdout into a file mixed with stderr. Stdout ack: `written`, `bytes`, `blake3`, `sub_queries_total`, `unique_memories_found`, `elapsed_ms`. Schema: `docs/schemas/deep-research-output-ack.schema.json`
- Single-token `deep-research` queries expand into sub-queries carrying `source: "aspect"` (multi-angle fan-out); manual strategy via `--sub-query-strategy manual --sub-queries-file`
- In headless scripts, use `graph traverse --fuzzy` when the canonical name is unknown; without an exact match, exit 4 carries suggestions
- Prefer `link --from-id`/`--to-id` in automation that only has IDs; NEVER pass bare digits to `--from`/`--to` with `--create-missing`
- `merge-entities` rejects self-references (`--into-id` also listed in `--ids`) before touching the database — useful under malformed zsh/bash loops
- Never use `sqlite-graphrag ... &> file` (it redirects stdout and stderr together and contaminates the JSON)

```bash
# headless deep-research with atomic file output (recommended for agents)
OUTDIR=/tmp/graphrag-out
mkdir -p "$OUTDIR"
sqlite-graphrag --quiet \
  --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 \
  deep-research "alice" --max-sub-queries 7 --k 20 --with-bodies \
  -o "$OUTDIR/research.json" --json
# Parse the ack on stdout; the full envelope is in the file
# Optional manual facets:
# printf '%s\n' 'alice stack' 'alice projects' > "$OUTDIR/subs.txt"
# sqlite-graphrag --quiet deep-research "alice" \
#   --sub-query-strategy manual --sub-queries-file "$OUTDIR/subs.txt" \
#   --output "$OUTDIR/research.json" --json
```


## v1.0.93 Update — OpenRouter Embedding Backend

- Since v1.0.93, embedding can use the OpenRouter REST API instead of spawning a headless LLM subprocess
- Use `--embedding-backend openrouter --embedding-model MODEL` to route embedding through `POST /api/v1/embeddings`
- This eliminates subprocess cold-start (~200ms API call vs 15-20s subprocess spawn per embedding)
- The OpenRouter path uses `reqwest+rustls-tls` directly — nothing is spawned, so no CWD isolation is needed
- OAuth-only enforcement does NOT apply to OpenRouter — it uses XDG `config add-key` / `--openrouter-api-key` (OPENROUTER_API_KEY is not read at runtime)
- Since v1.0.95 (ADR-0054), `enrich --mode openrouter` runs the JUDGE step through OpenRouter's `/chat/completions` REST endpoint (`reqwest+rustls-tls`). The SCAN→JUDGE→PERSIST pipeline is unchanged; only the JUDGE transport differs.
- The `--enrich-after` flag on `ingest` still spawns a headless subprocess for the enrich phase when the enrich mode is a local CLI; with `--mode openrouter` the enrich phase stays subprocess-free
- See ADR-0052 (OpenRouter embedding) and ADR-0054 (OpenRouter enrich JUDGE) for the full architectural rationale

## v1.0.95 Update — OpenRouter Enrich JUDGE

- `enrich --mode openrouter` routes the JUDGE step to `POST /api/v1/chat/completions` — no local CLI subprocess
- `--openrouter-model` is REQUIRED with `--mode openrouter` (NO default; omitting it → exit 1 before any network call)
- `--openrouter-api-key` reads from XDG via `config add-key` (OPENROUTER_API_KEY is not read at runtime) or `config add-key --provider openrouter`; `--openrouter-timeout` defaults to 300s; `--openrouter-base-url` is optional
- Request uses `response_format` `json_schema` with `strict: true` and `provider.require_parameters: true`; `reasoning.enabled: false` with a one-shot reasoning-mandatory fallback; `usage.cost` is read from the response
- Trade-off: OAuth zero-token (local CLI modes) vs tokens billed to the XDG-stored OpenRouter key (OPENROUTER_API_KEY is not read at runtime) (OpenRouter mode); schema advances v15 → v16 in v1.1.04 (migration V016 required); v15 in earlier releases

```bash
# Headless enrich JUDGE via OpenRouter REST (no subprocess, no CWD isolation)
printf "%s" "sk-or-v1-your-key-here" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY is not read at runtime (G-T-XDG-04)
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" --json
```

## v1.0.96 Update — Backlog Convergence and Read-Only Queue Status (ADR-0055)

- `enrich --until-empty` replaces the external bash retry loop in headless invocation: a single process runs the internal scan→drain loop until the queue has no eligible items left or `--max-runtime <SECONDS>` (default 3600) expires. The dead-letter queue guarantees the live set strictly decreases — transient failures reschedule `next_retry_at` with backoff, an item turns `dead` after `--max-attempts` (default 8) transient retries or on the first hard failure, and `dead` rows are excluded from dequeue.
- `enrich --status --json` is the read-only probe for hooks and timers: it reports the queue counts (`unbound_backlog`, per-operation `scan_backlog`, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`) and does NOT call the LLM and does NOT acquire the per-namespace singleton. `scan_backlog` (GAP-SG-77, v1.1.0) is the real per-operation database backlog a scan would enqueue — it kills the false `pending=0` for `entity-descriptions`/`body-enrich`/`re-embed`, and `state` derives `pending-scan` from it. A cron or systemd timer can poll it without contending with a running `enrich`.
- `enrich --prune-dead-orphans --json` is a companion read-only inspector (no LLM, no singleton): it deletes dead-letter rows (`status='dead'`, `item_type='memory'`) whose memory name no longer exists in the main DB, mutating only the `.enrich-queue.sqlite` sidecar; entity-keyed dead rows are left untouched. Use it in headless maintenance scripts to clear orphan dead-letter accumulation from memories renamed or purged after they were enqueued (ADR-0058, GAP-SG-66, v1.0.97).
- `enrich --prune-dead-entity-orphans --json` (v1.1.02, ADR-0062) is the entity-keyed counterpart: it deletes dead-letter rows with `item_type='entity'`, and is mutually exclusive with `--prune-dead-orphans`. Run both in sequence for a full orphan sweep after an upgrade that renamed/merged/purged entities.
- `--rest-concurrency <N>` (clamp 1..=16, default 8) sets the in-flight REST fan-out for `--mode openrouter` embedding; raise it for OpenRouter throughput. It is distinct from `--llm-parallelism` (which caps local LLM subprocesses) and from `--max-attempts` (the retry budget).

```bash
# Headless backlog drain — no external while-loop, no subprocess for OpenRouter
printf "%s" "sk-or-v1-your-key-here" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY is not read at runtime (G-T-XDG-04)
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" \
  --until-empty --max-runtime 1800 --max-attempts 8 --rest-concurrency 8 --json

# Hook/timer probe — inspect the queue without spawning the LLM or taking the singleton
sqlite-graphrag enrich --status --json | jaq '{eligible_now, waiting, dead: .queue_dead}'
```

## v1.1.06 Update — Headless entity-connect on Large Namespaces (ADR-0066)

Decision record: [ADR-0066](decisions/adr-0066-v1-1-06-entity-connect-scan.md). Regression suite: `tests/v1106_entity_connect_scan_regression.rs` (suite name **v1106**).

- Closes **GAP-ENTITY-CONNECT-SCAN-CARTESIAN**: headless `enrich --operation entity-connect` (and `cross-domain-bridges`) on large `global` no longer hangs at 100% CPU before `phase: scan`. Pair scan is O(k) (co-occurrence + hub×island), not cartesian O(n²).
- Queue keys are `pair:{id1}:{id2}` with `item_type=entity_pair`; drain resolves by primary key (no re-scan per item). GAP-002 `entity_connect_seen` remains in force.
- **First-scan wall-clock** is covered by `--max-runtime` and a soft 120s ceiling via `InterruptHandle`. Timeout → `AppError::Timeout` exit **1**. Orchestrators MUST NOT treat scan timeout as exit **75** (job singleton / slot lock).
- NDJSON for hooks: expect `phase: "scan_start"` **before** SQL (`operation`, `entities_in_namespace`, `backlog_degree0_proxy`), then `scan` / `scan_meta` (`pairs_enqueued_this_scan`, `scan_elapsed_ms`). Do not equate the two backlog fields.
- Prefer dry-run smoke before long `--until-empty` jobs on dense graphs.
- No schema migration for v1.1.06 (schema stays v16). Pin `=1.1.6`.

```bash
# Headless dry-run must finish quickly and emit scan_start (no cartesian hang)
sqlite-graphrag enrich --operation entity-connect --dry-run --json --limit 50 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro

# Long converge: --max-runtime covers the FIRST scan too
sqlite-graphrag enrich --operation entity-connect --until-empty --max-runtime 600 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json
```

### v1.2.1 Update — Enrich CAPA for Headless Agents (Sidecar Only)

CAPA themes (enrich-queue seal; schema **v16**, no main-DB migration):

1. **`dequeue_next_pending`** — claim filters by `operation` **and** `namespace` (draining `ai-sdd` MUST NOT process `global` / empty-ns rows; same for `--resume` / `--retry-failed`).
2. **`count_eligible_pending` for `--until-empty`** — counts only this **op+ns** pending (alien ops / ReEmbed zombies elsewhere no longer keep EntityDescriptions spinning with `completed=0`).
3. **`reopen_force_redescribe_candidates`** — `--force-redescribe` reopens `skipped`/`done` **once per process** before first enqueue (so `INSERT OR IGNORE` is not a silent no-op); **never** reopens `dead` (use `--requeue-dead`).
4. **`reconcile_satisfied_reembed_pending`** — marks pending ReEmbed `done` when a live vector already matches `LENGTH(embedding) = dim*4`, clearing zombies without API calls.
5. **Re-embed eligibility by BLOB LENGTH** — predicates use `LENGTH(embedding) = dim*4`, **not** the `dim` column alone (CORRUPT / META_AHEAD rows with dim=1024 and a 384-d BLOB re-embed again).
6. **`entity:` prefix strip on enqueue lookup** — entity lookup uses the bare name; queue key stays `entity:…` (bare names still work; missing entity rejected).
7. **Chunk enqueue validates namespace** — `chunk_id` must exist on a non-deleted memory in the target namespace (rejects invalid / cross-ns keys before dead-letter/circuit-breaker churn).
8. **CAPA-D** — compound "configuration file" markers only (e.g. `is a configuration file`); no bare FP on legitimate domain prose.

Queue regressions: `enqueue_candidate_accepts_entity_prefixed_reembed_key`, `dequeue_next_pending_isolates_by_namespace`. Queue unit suite: **38** OK (`cargo test --lib commands::enrich::queue` or `cargo test --lib commands::enrich`).

Ready agent formulas:

```bash
DB="${DB:-$HOME/.local/share/sqlite-graphrag/memory.db}"
MODEL="${MODEL:-deepseek/deepseek-v4-flash:nitro}"
NS="${NS:-global}"

sqlite-graphrag enrich --db "$DB" --status --operation re-embed --namespace "$NS" -q
sqlite-graphrag enrich --db "$DB" --operation re-embed --target entities \
  --mode openrouter --openrouter-model "$MODEL" \
  --until-empty --namespace "$NS" -q --wait-lock 60
sqlite-graphrag enrich --db "$DB" --operation entity-descriptions \
  --mode openrouter --openrouter-model "$MODEL" \
  --force-redescribe --until-empty --namespace "$NS" -q
sqlite-graphrag enrich --db "$DB" --list-skipped --operation entity-descriptions --namespace "$NS" -q
```

- Schema stays **v16** (no main-DB migration). Offline gate still `scripts/e2e_offline_v120.sh` **20/20**. Pin `=1.2.2`.

### v1.2.0 Update — XDG Config, dim 1024, list-skipped, GAP-SG-139, Headless Hot-Set

- Config for product knobs is **CLI flag > XDG `config set` > default**. Product env `SQLITE_GRAPHRAG_*` is **not** read on the hot path. Harnesses MUST use isolated XDG (`XDG_CONFIG_HOME` / `XDG_DATA_HOME` / …) plus flags — never export product env as the config contract.
- **DEFAULT_EMBEDDING_DIM=1024** (override via `--embedding-dim` / XDG `embedding.dim`; existing DBs keep `schema_meta.dim` until re-embed).
- Recover skipped / `preservation_failed` queue debt without raw SQL:
  ```bash
  sqlite-graphrag enrich --list-skipped --json
  sqlite-graphrag enrich --requeue-skipped --json
  ```
- **GAP-SG-139:** host/XDG leaves (`config`, `slots`, `cache`, `completions`) accept `--db` as a documented **no-op** — headless agents may append `--db` on every spawn without clap exit 2.
- After curated `remember`, PARSE `entities_created` / `enrich_recommended` and/or PASS `--enqueue-enrich` so entity-descriptions runs as a hot set before long entity-connect drains.
- Poll quality without LLM: `enrich --operation entity-descriptions --status --force-redescribe --json` (`scan_backlog_low_quality`, `quality_pct`, `state` including `blocked_dead`).
- Name filters: `--entity-names` for entity-descriptions; `--memory-names` for memory-bindings; do not assume `--names` always means memories.
- Audit bindings: `memory-entities --name <mem> --json` includes `entities[].description`.
- entity-connect is fully implemented (persists relationships). On large DBs expect adaptive budget fields (`budget_exhausted`, `preempted_for_gate`) and prefer ED hot → EC cold.
- Offline product gate: `bash scripts/e2e_offline_v120.sh` expects **20/20 PASS** (canonical; historical wrapper `e2e_offline_v118.sh` / 16/16 is superseded).
- **CURRENT note on historical text below:** sections that teach product env as config describe pre-v1.2.0 behaviour — v1.2.0 does **not** read product env on the hot path (XDG + flags only). The OAuth/custom-provider env whitelist for LLM subprocesses remains valid and is **not** product-knob config.
- Secrets: prefer `config add-key --provider openrouter` (stdin) or `--openrouter-api-key`; `OPENROUTER_API_KEY` is not read at runtime; use `config add-key` or `--openrouter-api-key` only.

## Complete CLI command inventory for headless agents (v1.2.5)

Headless orchestrators must know the full product surface even when spawn recipes focus on `remember` / `enrich` / `deep-research`. Top-level product commands (from `sqlite-graphrag --help`, excluding meta `help`):

- `init` — create/open DB + migrations + LLM smoke-test
- `remember` — write one memory (+ optional graph / `--enqueue-enrich`)
- `remember-batch` — NDJSON batch create (description required)
- `ingest` — bulk-ingest files as memories
- `recall` — semantic KNN search
- `read` / `list` / `forget` / `purge` / `rename` / `split-body` / `edit` / `history` / `restore` — memory CRUD + lifecycle
- `hybrid-search` — FTS5 + vector RRF
- `health` / `migrate` / `namespace-detect` / `optimize` / `stats` — ops & diagnostics
- `sync-safe-copy` / `backup` / `vacuum` — durability & space
- `link` / `unlink` / `related` / `graph` / `export` — graph edges & export
- `deep-research` — multi-hop GraphRAG (`-o` / `--output` atomwrite)
- `fts` / `vec` — index maintenance families
- `prune-relations` / `prune-ner` / `cleanup-orphans` — graph hygiene
- `slots` / `pending` / `embedding` / `pending-embeddings` — concurrency & queues
- `memory-entities` / `delete-entity` / `reclassify` / `rename-entity` / `merge-entities` / `reclassify-relation` / `normalize-entities` — entity admin
- `enrich` — LLM graph quality + queue inspectors (`--list-skipped` / `--requeue-skipped`)
- `cache` / `completions` / `config` — host/XDG leaves (`--db` no-op)

### Nested families (brief)

- `graph` — `traverse`, `stats`, `entities`, `recompute-degree`
- `embedding` — `status`, `list`, `abandon`
- `pending` — `list`, `show`, `cleanup`
- `pending-embeddings` — `list`, `status`, `abandon`
- `slots` — `status`, `release`, `cleanup`
- `cache` — `clear-models`, `list`, `stats`
- `config` — `add-key`, `list-keys`, `remove-key`, `doctor`, `path`, `set`, `get`, `list`, `unset`
- `fts` — `rebuild`, `check`, `stats`
- `vec` — `orphan-list`, `purge-orphan`, `stats`
- `enrich` inspectors: `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans`; **v1.2.1 write flags:** `--until-empty` (op+ns count), `--force-redescribe` (reopen skipped/done), `--operation re-embed --target …`, `--namespace`, `--mode openrouter`, `--rest-concurrency`
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

> **Top-level (50 + help):** init, remember, remember-batch, ingest, recall, read, list, forget, purge, rename, split-body, edit, history, restore, hybrid-search, health, migrate, namespace-detect, optimize, stats, sync-safe-copy, backup, vacuum, link, unlink, deep-research, related, graph, export, fts, vec, prune-relations, prune-ner, slots, pending, embedding, pending-embeddings, cleanup-orphans, memory-entities, cache, delete-entity, reclassify, rename-entity, merge-entities, enrich, reclassify-relation, normalize-entities, schema, completions, config, help.

> Full inventory with one-line purposes: [HOW_TO_USE.md](HOW_TO_USE.md#complete-cli-command-inventory-v125) and [COOKBOOK.md](COOKBOOK.md).

### v1.1.05 Update — Headless Pipeline Safety (`--quiet`, `deep-research --output` / `-o`)

Decision record: [ADR-0065](decisions/adr-0065-v1-1-05-incident-bugs.md). Regression suite: `tests/v1105_incident_bugs_regression.rs` (suite name **v1105**).

- Global `--quiet` / `-q` suppresses non-error tracing on stderr so agent harnesses can parse stdout as pure JSON without log noise.
- `deep-research -o PATH` or `--output PATH` writes the full research envelope via atomwrite (tempfile in the same directory → fsync → rename) and prints only a short stdout ack: `written`, `bytes`, `blake3`, `sub_queries_total`, `unique_memories_found`, `elapsed_ms`. Prefer this for large `--with-bodies` jobs under agent orchestrators. Schema: `docs/schemas/deep-research-output-ack.schema.json`.
- Contract: **stdout = JSON** (envelope or ack), **stderr = logs**. NEVER redirect both to the same file with `&>` or `2>&1` into a JSON consumer.
- Single-token queries (e.g. a person name) expand to multi-aspect sub-queries (`source: "aspect"`) so headless research on a subject token is no longer a single hybrid hit. Manual override for orchestrators: `--sub-query-strategy manual --sub-queries-file PATH`.
- `graph traverse --fuzzy` is safe for headless nickname resolution; without `--fuzzy`, exit 4 NotFound includes ranked suggestions for the orchestrator to pick.
- `link --from-id`/`--to-id` and merge self-ref pre-DB rejection reduce silent graph corruption in scripted maintenance.

```bash
# Headless deep-research with atomic file output (recommended for agents)
OUTDIR=/tmp/graphrag-out
mkdir -p "$OUTDIR"
sqlite-graphrag --quiet \
  --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 \
  deep-research "alice" --max-sub-queries 7 --k 20 --with-bodies \
  -o "$OUTDIR/research.json" --json
# Parse ack from stdout; full envelope from the file
# Optional manual facets:
# printf '%s\n' 'alice stack' 'alice projetos' > "$OUTDIR/subs.txt"
# sqlite-graphrag --quiet deep-research "alice" \
#   --sub-query-strategy manual --sub-queries-file "$OUTDIR/subs.txt" \
#   -o "$OUTDIR/research.json" --json
```

### v1.1.04 Update — Deep-Research Stability + entity-connect Convergence (ADR-0064)

- GAP-001: `deep-research` no longer panics with "Cannot start a runtime from within a runtime" when invoked headless (agent harnesses, CI runners, scheduled jobs). The sync entry point `deep_research::run` now computes per-sub-query embeddings BEFORE building its dedicated Tokio runtime via the new `compute_sub_embeddings` helper, and the three OpenRouter embedding paths in `embedder.rs` (single, serial batch, JoinSet fan-out) adopt the canonical `Handle::try_current` + `block_in_place` reentry pattern already used by the batch path. For headless orchestrators this means long-running `deep-research --with-bodies` jobs that previously crashed mid-flight now complete reliably.
- GAP-002: `entity-connect` now converges in headless long-running loops. A new `entity_connect_seen` table (migration V016, main database schema v15 → v16) records the LLM verdict (`related`/`none`) for each evaluated pair; the `scan_isolated_entity_pairs` scanner excludes already-evaluated pairs and prioritises hub entities; and `call_entity_connect` persists the verdict on both branches. Combined with `--until-empty --max-runtime`, a headless `enrich --operation entity-connect` job now reaches `eligible_remaining == 0` instead of re-evaluating the same rejected pairs forever. Running `migrate --json` once on first open is REQUIRED before the first `entity-connect` invocation.


### v1.1.03 Update — Stale Claim Recovery in Headless Long-Running enrich

- Headless orchestrators (agent harnesses, CI runners, systemd timers) frequently send SIGINT, SIGTERM, and occasionally SIGKILL to long-running `enrich --until-empty` jobs
- SIGKILL is NOT capturable — the `.enrich-queue.sqlite` sidecar may be left with rows stuck in `status='processing'` under the dead PID
- Since v1.1.03 (ADR-0063, Bug 4), the queue sidecar gains a `claimed_at` INTEGER column and the enrich worker emits a per-item heartbeat (`UPDATE queue SET claimed_at = unixepoch() WHERE id = ?`)
- On EVERY enrich startup, the worker calls `reset_stale_processing_claims(conn, 1800)` — items with `status='processing' AND claimed_at < unixepoch() - 1800` are flipped back to `pending` and `claimed_at = NULL`
- The 1800-second (30-minute) threshold is the default; combined with the heartbeat it covers any job that stops making progress for half an hour
- For manual reset (e.g. after a known kill -9 incident), the new flag `enrich --reset-stale-claims --json` flushes stale claims without running the full scan→drain loop
- SIGTERM (capturable) is handled by the existing `signals::handler` graceful path; only SIGKILL relies on the timestamp-based recovery
- No new env var, no telemetry — the recovery is silent and idempotent

```bash
# Force-reset stale claims after a known kill -9 incident (no scan, no LLM)
sqlite-graphrag enrich --reset-stale-claims --json

# Normal headless enrich — stale claims auto-recovered at startup
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" \
  --until-empty --max-runtime 1800 --json
```

## v1.0.93 Update — OpenRouter Embedding Backend
- New `--embedding-backend openrouter` flag enables REST API embedding without LLM subprocess
- Eliminates cold-start overhead: ~200ms per embedding vs 15s with subprocess
Set API key via `config add-key --provider openrouter` or `--openrouter-api-key` (OPENROUTER_API_KEY is not read at runtime)
- Requires `--embedding-model MODEL` (no default — user must specify)
- Works with all 8 embedding commands in headless mode
- Example headless invocation:

```bash
sqlite-graphrag \
  --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  ingest ./docs --pattern "*.md" --recursive --json
```

## Global LLM flags for headless agents

- `--llm-backend <openrouter|none>` — selects the embedding transport. Default `openrouter`; `none` skips embedding
- `--llm-model <MODEL>` — model passed to the selected backend
- `--llm-fallback <chain>` — fallback chain when the primary backend fails. Default `none`
- `--skip-embedding-on-failure` — persist the memory without a vector (exit 0 instead of exit 11)
- `--llm-max-host-concurrency <N>` — cap concurrent LLM calls host-wide
- `--llm-slot-wait-secs <N>` — seconds to wait for a free slot before failing
- `--llm-slot-no-wait` — fail immediately when no slot is available

### sqlite-graphrag with backend and model override

```bash
# Force the OpenRouter backend with a specific embedding model
sqlite-graphrag --llm-backend openrouter --llm-model "qwen/qwen3-embedding-8b" \
  remember --name example --type note --body "text" --json

# Skip embedding entirely (no vector written)
sqlite-graphrag --llm-backend none \
  remember --name no-vector --type note --body "text" --json

# Skip embedding on failure (persist memory without vector)
sqlite-graphrag --skip-embedding-on-failure \
  remember --name resilient --type note --body "text" --json
```


## Headless Patterns Added in v1.0.82
### Shutdown envelope capture pattern (GAP-002, ADR-0037)
```bash
# Wrap a long-running sqlite-graphrag invocation in a signal handler
# that captures the shutdown JSON envelope on stdout at exit 19.
timeout 300 sqlite-graphrag remember --name big-corpus --type document \
  --body-file ./big.md --json 2>/tmp/err.log
EXIT=$?
if [ $EXIT -eq 19 ]; then
  # parse the envelope from the last line of stdout
  jaq -e '.error and .code == 19' /tmp/err.log
  jaq -r '.signal, .graceful' /tmp/err.log
fi
```
### Fallback chain wrap pattern (GAP-003 + GAP-005, ADR-0038 + ADR-0040)
```bash
# Pre-flight: confirm the OpenRouter key resolves before launching
sqlite-graphrag config doctor --json | jaq -e '.openrouter_key_present' >/dev/null \
  || { echo "OpenRouter key missing in XDG (config add-key); OPENROUTER_API_KEY is not read at runtime"; exit 1; }

# Launch with the explicit backend
sqlite-graphrag remember --name foo --type note --body "..." \
  --llm-backend openrouter --json

# If the backend fails, inspect the pending queue
sqlite-graphrag pending-embeddings list --status pending --json
```
### Slot semaphore poll pattern (GAP-004, ADR-0039)
```bash
# Wait until a slot is free before launching a heavy batch
while [ "$(sqlite-graphrag slots status --json | jaq '.acquired')" -gt 0 ]; do
  sleep 5
done
sqlite-graphrag ingest ./big-corpus --recursive --json
```
