## Custom Provider Env Preservation in Headless Invocation (v1.0.83+)
- The headless invocation pipeline (`claude_runner`, `codex_spawn`, `ingest_claude`) now preserves six custom-provider env vars when spawning subprocesses: `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`, `OTEL_EXPORTER_OTLP_ENDPOINT`
- The three spawners delegate to `apply_env_whitelist(cmd, strict)` from `src/spawn/env_whitelist.rs` instead of inlining the whitelist array. This eliminates drift between the three duplicated `env_clear` + re-injection blocks
- The OAuth-only guard at `claude_runner.rs:273`, `codex_spawn.rs:259`, `ingest_claude.rs:282`, `extract/llm_embedding.rs:237-253` is unchanged; `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` still abort with `AppError::Validation` (exit 1) and the new error message references `ANTHROPIC_AUTH_TOKEN` and `~/.codex/auth.json` as legitimate resolutions
- New global flag `--strict-env-clear` / `SQLITE_GRAPHRAG_STRICT_ENV_CLEAR=1` enables strict mode that preserves only `PATH`. Use in compliance environments (PCI-DSS, SOC2, HIPAA) where credential forwarding via env vars is forbidden by policy
- The 7 hardening flags for `claude -p` (`--strict-mcp-config --mcp-config '{}' --settings '{"hooks":{}}' --dangerously-skip-permissions --output-schema` plus model and prompt) and the canonical set for `codex exec` remain unchanged. The env whitelist change is purely additive in the whitelist step between `env_clear()` and the canonical flag construction
- No new telemetry: the fix is silent. The no-leak audit test `audit_no_token_leak_in_subprocess_stderr` in `tests/claude_runner_env.rs` enforces that the literal token value NEVER appears in stdout or stderr even with `RUST_LOG=trace`
- See `docs/decisions/adr-0041-preserve-custom-provider-env.md` for the full architectural rationale
# Headless Invocation — Claude Code, Codex, OpenCode without MCP and without Hooks

> How to invoke headless LLMs in this project without inheriting MCPs or hooks from the environment, while keeping the subscription OAuth login.

- Portuguese version of this guide lives in [HEADLESS_INVOCATION.pt-BR.md](HEADLESS_INVOCATION.pt-BR.md)
- Back to [README.md](../README.md) for the command reference


## Summary

- Claude Code OAuth without MCP uses `--strict-mcp-config --mcp-config '{}'`
- Codex OAuth without MCP uses `codex exec -c mcp_servers='{}'`
- OpenCode OAuth without MCP uses `OPENCODE_CONFIG_CONTENT` with `enabled` false per server
- The most important finding: on Claude, the `--bare` flag cuts MCPs but DISABLES OAuth. `--bare` then requires an API key, which is forbidden here. That is why `--bare` is NEVER used when login is subscription-based


## OAuth-Safe Command Table

| CLI | OAuth-safe headless command | Keeps OAuth | Cuts MCP | Cuts Hooks |
| --- | --- | --- | --- | --- |
| Claude Code | `claude -p "TASK" --strict-mcp-config --mcp-config '{}' ...` | yes | yes | yes |
| Codex CLI | `codex exec -c mcp_servers='{}' ...` | yes | yes | N/A |
| OpenCode | `OPENCODE_CONFIG_CONTENT='{...enabled:false...}' opencode run ...` | yes | yes | N/A |


## Claude Code Headless OAuth without MCP and without Hooks

### What To Do

Run `claude -p` with the MCP config locked down and empty, and the hooks config zeroed out.

### Why

- `-p` enables one-shot headless mode
- `--strict-mcp-config` tells it to ignore ALL MCP config from the environment
- `--mcp-config '{}'` provides an empty server list
- `--settings '{"hooks":{}}'` disables hooks for that specific call
- The combination guarantees zero MCPs and zero hooks running, while keeping the subscription login (OAuth Pro or Max)

### v1.0.79 Update — The Real Isolation Is an Empty `CLAUDE_CONFIG_DIR`

- Issue #10787 of `anthropics/claude-code` documents that `--strict-mcp-config` and `--mcp-config` are silently IGNORED by upstream
- The only mechanism upstream honours is `CLAUDE_CONFIG_DIR` pointing to an empty directory
- Since v1.0.79 (G42/S6), the CLI embedding pipeline uses an empty `CLAUDE_CONFIG_DIR` BY DEFAULT: it honours `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR`, otherwise a managed directory `~/.local/state/sqlite-graphrag/claude-empty-config` (mode 0700, copies `.credentials.json` when present)
- A populated `~/.claude` used to cost ~223k cache-creation tokens per call (~40-50s); the empty config dir brings it down to ~10-15s
- The flags below are still passed as defence in depth, but do NOT rely on them for isolation

### v1.0.91 Update — Automatic CWD Isolation via `apply_cwd_isolation()`

- Since v1.0.91, ALL 10 LLM subprocess spawn sites call `apply_cwd_isolation()` from `src/spawn/mod.rs`
- This function sets `current_dir(temp_dir)` — the subprocess CWD is a clean `/tmp/sqlite-graphrag-spawn-{PID}/` directory with NO `.mcp.json` in any ancestor
- It also sets `CLAUDE_CONFIG_DIR=temp_dir` — isolating the subprocess from user-level `~/.claude/` MCP configuration
- The manual workaround `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1 CLAUDE_CONFIG_DIR=/tmp/graphrag-empty-config` is NO LONGER NEEDED for normal operation (retained as emergency override)
- Spawn directories are cleaned up automatically at process exit via `cleanup_spawn_dir()` in `src/main.rs` (GAP-SPAWN-002) — non-recursive `remove_dir()`, safe for non-empty directories
- The v1.0.79 `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` env var and the managed `~/.local/state/sqlite-graphrag/claude-empty-config` directory remain functional but are superseded by the automatic CWD isolation

### v1.0.93 Update — OpenRouter Embedding Backend Bypasses Subprocess

- Since v1.0.93, embedding can use the OpenRouter REST API instead of spawning a headless LLM subprocess
- Use `--embedding-backend openrouter --embedding-model MODEL` to route embedding through `POST /api/v1/embeddings`
- This eliminates subprocess cold-start (~200ms API call vs 15-20s subprocess spawn per embedding)
- The OpenRouter path uses `reqwest+rustls-tls` directly — no `claude -p`, no `codex exec`, no CWD isolation needed
- OAuth-only enforcement does NOT apply to OpenRouter — it uses its own `OPENROUTER_API_KEY` / `--openrouter-api-key`
- Headless subprocess spawning remains unchanged for LLM-based embedding (`--embedding-backend llm`) and for `enrich` operations run with `--mode claude-code|codex|opencode`
- Since v1.0.95 (ADR-0054), `enrich --mode openrouter` also bypasses the subprocess: the JUDGE step runs via OpenRouter's `/chat/completions` REST endpoint (`reqwest+rustls-tls`), with no `claude -p` / `codex exec` / `opencode run` spawn and no CWD isolation. The SCAN→JUDGE→PERSIST pipeline is unchanged; only the JUDGE transport differs.
- The `--enrich-after` flag on `ingest` still spawns a headless subprocess for the enrich phase when the enrich mode is a local CLI; with `--mode openrouter` the enrich phase stays subprocess-free
- See ADR-0052 (OpenRouter embedding) and ADR-0054 (OpenRouter enrich JUDGE) for the full architectural rationale

### v1.0.95 Update — OpenRouter Enrich JUDGE Bypasses Subprocess

- `enrich --mode openrouter` routes the JUDGE step to `POST /api/v1/chat/completions` — no local CLI subprocess
- `--openrouter-model` is REQUIRED with `--mode openrouter` (NO default; omitting it → exit 1 before any network call)
- `--openrouter-api-key` reads from env `OPENROUTER_API_KEY` or `config add-key --provider openrouter`; `--openrouter-timeout` defaults to 300s; `--openrouter-base-url` is optional
- Request uses `response_format` `json_schema` with `strict: true` and `provider.require_parameters: true`; `reasoning.enabled: false` with a one-shot reasoning-mandatory fallback; `usage.cost` is read from the response
- Trade-off: OAuth zero-token (local CLI modes) vs tokens billed to `OPENROUTER_API_KEY` (OpenRouter mode); schema advances v15 → v16 in v1.1.04 (migration V016 required); v15 in earlier releases

```bash
# Headless enrich JUDGE via OpenRouter REST (no subprocess, no CWD isolation)
export OPENROUTER_API_KEY="sk-or-v1-your-key-here"
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" --json
```

### v1.0.96 Update — Backlog Convergence and Read-Only Queue Status (ADR-0055)

- `enrich --until-empty` replaces the external bash retry loop in headless invocation: a single process runs the internal scan→drain loop until the queue has no eligible items left or `--max-runtime <SECONDS>` (default 3600) expires. The dead-letter queue guarantees the live set strictly decreases — transient failures reschedule `next_retry_at` with backoff, an item turns `dead` after `--max-attempts` (default 8) transient retries or on the first hard failure, and `dead` rows are excluded from dequeue.
- `enrich --status --json` is the read-only probe for hooks and timers: it reports the queue counts (`unbound_backlog`, per-operation `scan_backlog`, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`) and does NOT call the LLM and does NOT acquire the per-namespace singleton. `scan_backlog` (GAP-SG-77, v1.1.0) is the real per-operation database backlog a scan would enqueue — it kills the false `pending=0` for `entity-descriptions`/`body-enrich`/`re-embed`, and `state` derives `pending-scan` from it. A cron or systemd timer can poll it without contending with a running `enrich`.
- `enrich --prune-dead-orphans --json` is a companion read-only inspector (no LLM, no singleton): it deletes dead-letter rows (`status='dead'`, `item_type='memory'`) whose memory name no longer exists in the main DB, mutating only the `.enrich-queue.sqlite` sidecar; entity-keyed dead rows are left untouched. Use it in headless maintenance scripts to clear orphan dead-letter accumulation from memories renamed or purged after they were enqueued (ADR-0058, GAP-SG-66, v1.0.97).
- `enrich --prune-dead-entity-orphans --json` (v1.1.02, ADR-0062) is the entity-keyed counterpart: it deletes dead-letter rows with `item_type='entity'`, and is mutually exclusive with `--prune-dead-orphans`. Run both in sequence for a full orphan sweep after an upgrade that renamed/merged/purged entities.
- `--rest-concurrency <N>` (clamp 1..=16, default 8) sets the in-flight REST fan-out for `--mode openrouter` embedding; raise it for OpenRouter throughput. It is distinct from `--llm-parallelism` (which caps local LLM subprocesses) and from `--max-attempts` (the retry budget).

```bash
# Headless backlog drain — no external while-loop, no subprocess for OpenRouter
export OPENROUTER_API_KEY="sk-or-v1-your-key-here"
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" \
  --until-empty --max-runtime 1800 --max-attempts 8 --rest-concurrency 8 --json

# Hook/timer probe — inspect the queue without spawning the LLM or taking the singleton
sqlite-graphrag enrich --status --json | jaq '{eligible_now, waiting, dead: .queue_dead}'
```

### Why NOT To Use `--bare`

- `--bare` also cuts MCP, hooks, skills, plugins and auto memory
- BUT `--bare` disables OAuth and the keychain (issue #39069 of `anthropics/claude-code`)
- With `--bare`, Claude requires `ANTHROPIC_API_KEY`, which is forbidden in this project
- To keep OAuth, the right path is `--strict-mcp-config`, never `--bare`

### How To Do It

```bash
claude -p "YOUR TASK HERE" \
  --strict-mcp-config \
  --mcp-config '{}' \
  --dangerously-skip-permissions \
  --settings '{"hooks":{}}' \
  --model claude-sonnet-4-6 \
  --max-turns 8 \
  --output-format json
```

### What Each Piece Does

- `--strict-mcp-config` ignores MCP from global and project settings
- `--mcp-config '{}'` provides the empty list that zeroes out servers
- `--dangerously-skip-permissions` avoids stalling on confirmation prompts (`bypassPermissions` mode)
- `--settings '{"hooks":{}}'` disables hooks for that specific call
- `--model claude-sonnet-4-6` picks the model without depending on an environment variable
- `--max-turns 8` caps agent turns as a safety net against infinite loops
- `--output-format json` delivers output that is easy to parse with `jaq`

### How To Guarantee OAuth

- Log in once with the Pro or Max account before automating (`claude auth login`)
- Do NOT set `ANTHROPIC_API_KEY` in the call environment
- Do NOT use `--bare`
- Without the variable and without `--bare`, Claude uses the logged-in OAuth session

### Known Bug Caveat

- Issue #14490 of `anthropics/claude-code` documents that `--strict-mcp-config` does NOT override the `disabledMcpServers` list stored in `~/.claude.json`
- For a clean environment, ensure `~/.claude.json` does not contain the server in `disabledMcpServers`, or use `--bare` only in a controlled environment with `ANTHROPIC_API_KEY` (a scenario explicitly FORBIDDEN in this project)
- The robust solution is to combine `--strict-mcp-config --mcp-config '{}'` and ensure the server is not in `disabledMcpServers` in `~/.claude.json`


## Codex CLI Headless OAuth without MCP

### What To Do

Run `codex exec` zeroing out the MCP server table from the config.

### Why

- `codex exec` is the non-interactive mode built for scripts
- It writes only the final message to stdout and progress to stderr
- The `-c mcp_servers='{}'` override replaces the entire table with an empty one
- That way no MCP server from `config.toml` comes up for that call

### How To Do It

```bash
codex exec \
  --model gpt-5.5 \
  -c mcp_servers='{}' \
  --sandbox workspace-write \
  --ask-for-approval never \
  "YOUR TASK HERE"
```

### More Aggressive Alternative

- Use `--ignore-user-config` to skip reading the user `config.toml` entirely
- That zeroes out MCP along with everything else in the config
- The OAuth login is stored in `auth.json`, which is a separate file
- That is why `--ignore-user-config` does NOT break the login

```bash
codex exec --model gpt-5.5 --ignore-user-config --sandbox workspace-write "YOUR TASK HERE"
```

### What Each Piece Does

- `-c mcp_servers='{}'` zeroes only the MCPs and preserves the model and the rest of the config
- `--ignore-user-config` is the full cut when you want a clean environment
- `--sandbox workspace-write` allows file editing without network access
- `--ask-for-approval never` runs without pausing for permission

### How To Guarantee OAuth

- Run `codex login` once for the browser flow with ChatGPT
- On a remote or browserless machine, use `codex login --device-auth`
- Do NOT set `OPENAI_API_KEY` in the call environment
- The login is stored in `~/.codex/auth.json` and `codex exec` reuses the session

### Old Bug Caveat

- Old Codex versions (0.33.0) installed via Homebrew did not read `[mcp_servers]` correctly
- Issue #3441 of the `openai/codex` repository confirms the fix landed in 0.34.0+
- Validate the version with `codex --version` before using the `-c mcp_servers='{}'` override


## OpenCode Headless without MCP

### The Honest Difference

- OpenCode does NOT have a single CLI flag to disable MCP
- Claude has `--strict-mcp-config` and Codex has `-c mcp_servers='{}'`
- OpenCode controls MCP only through the JSON config
- OpenCode configs are merged, not replaced, so each server must be disabled individually

### What To Do

- Discover the active server names with `opencode mcp list`
- Disable each one with `enabled: false` in the config

### Why

- `opencode run` is the headless mode that takes the prompt and returns the result
- Because the config is merged, deleting the key is not enough to remove the server
- Setting `enabled` false under the same name overrides and disables that MCP
- The runtime override via `OPENCODE_CONFIG_CONTENT` avoids touching project files

### How To Do It — Step 1 List Active Servers

```bash
opencode mcp list
```

### How To Do It — Step 2 Run Headless Disabling Each Server

```bash
OPENCODE_CONFIG_CONTENT='{"mcp":{"server-name-1":{"enabled":false},"server-name-2":{"enabled":false}}}' \
  opencode run --model anthropic/claude-sonnet-4-5 "YOUR TASK HERE"
```

### Permanent Alternative

- Edit `opencode.json` and mark each MCP with `enabled` false
- Worth it when you never want that server in automatic execution

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "server-name-1": { "enabled": false },
    "server-name-2": { "enabled": false }
  }
}
```

### What Each Piece Does

- `opencode mcp list` shows server names and connection status
- `OPENCODE_CONFIG_CONTENT` injects inline config with high precedence
- `enabled` false per server is what actually prevents the MCP from coming up
- `--model` picks the model in `provider/model` format

### How To Guarantee OAuth

- Run `opencode auth login` once and choose the provider
- The credential is stored in `auth.json` in the OpenCode data folder
- `opencode run` reuses that credential on subsequent calls


## OAuth Login per CLI

- Claude: session login via `claude auth login`. Do NOT use `--bare` to preserve OAuth
- Codex: `codex login` or `codex login --device-auth` (browserless)
- OpenCode: `opencode auth login`


## Headless Mode per CLI

- Claude: `claude -p`
- Codex: `codex exec`
- OpenCode: `opencode run`



### v1.1.06 Update — Headless entity-connect on Large Namespaces (ADR-0066)

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

### v1.2.0 Update — XDG Config, dim 1024, list-skipped, GAP-SG-139, Headless Hot-Set

- Config for product knobs is **CLI flag > XDG `config set` > default**. Product env `SQLITE_GRAPHRAG_*` is **not** read on the hot path. Harnesses MUST use isolated XDG (`XDG_CONFIG_HOME` / `XDG_DATA_HOME` / …) plus flags — never export product env as the config contract.
- **DEFAULT_EMBEDDING_DIM=1024** (override via `--embedding-dim` / XDG `embedding.dim`; existing DBs keep `schema_meta.dim` until re-embed).
- Recover skipped / `preservation_failed` queue debt without raw SQL:
  ```bash
  sqlite-graphrag enrich --list-skipped --json
  sqlite-graphrag enrich --requeue-skipped --json
  ```
- **GAP-SG-139:** host/XDG leaves (`config`, `slots`, `cache`, `codex-models`, `completions`) accept `--db` as a documented **no-op** — headless agents may append `--db` on every spawn without clap exit 2.
- After curated `remember`, PARSE `entities_created` / `enrich_recommended` and/or PASS `--enqueue-enrich` so entity-descriptions runs as a hot set before long entity-connect drains.
- Poll quality without LLM: `enrich --operation entity-descriptions --status --force-redescribe --json` (`scan_backlog_low_quality`, `quality_pct`, `state` including `blocked_dead`).
- Name filters: `--entity-names` for entity-descriptions; `--memory-names` for memory-bindings; do not assume `--names` always means memories.
- Audit bindings: `memory-entities --name <mem> --json` includes `entities[].description`.
- entity-connect is fully implemented (persists relationships). On large DBs expect adaptive budget fields (`budget_exhausted`, `preempted_for_gate`) and prefer ED hot → EC cold.
- Offline product gate: `bash scripts/e2e_offline_v120.sh` expects **20/20 PASS** (canonical; historical wrapper `e2e_offline_v118.sh` / 16/16 is superseded).
- **CURRENT note on historical text below:** sections that teach product env as config describe pre-v1.2.0 behaviour — v1.2.0 does **not** read product env on the hot path (XDG + flags only). The OAuth/custom-provider env whitelist for LLM subprocesses remains valid and is **not** product-knob config.
- Secrets: prefer `config add-key --provider openrouter` (stdin) or `--openrouter-api-key`; `OPENROUTER_API_KEY` may still resolve as optional key input.

## Complete CLI command inventory for headless agents (v1.2.0)

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
- `codex-models` — ChatGPT Pro OAuth model list (`--db` no-op, GAP-SG-139)
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
- `enrich` flags — `--status`, `--list-dead`, `--requeue-dead`, **`--list-skipped`**, **`--requeue-skipped`**, prune-orphan inspectors

> Full one-line inventory: [HOW_TO_USE.md](HOW_TO_USE.md#complete-cli-command-inventory-v120) and [COOKBOOK.md](COOKBOOK.md).

### v1.1.05 Update — Headless Pipeline Safety (`--quiet`, `deep-research --output` / `-o`)

Decision record: [ADR-0065](decisions/adr-0065-v1-1-05-danilo-bugs.md). Regression suite: `tests/v1105_danilo_bugs_regression.rs` (suite name **v1105**).

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
  deep-research "danilo" --max-sub-queries 7 --k 20 --with-bodies \
  -o "$OUTDIR/research.json" --json
# Parse ack from stdout; full envelope from the file
# Optional manual facets:
# printf '%s\n' 'danilo stack' 'danilo projetos' > "$OUTDIR/subs.txt"
# sqlite-graphrag --quiet deep-research "danilo" \
#   --sub-query-strategy manual --sub-queries-file "$OUTDIR/subs.txt" \
#   -o "$OUTDIR/research.json" --json
```

### v1.1.04 Update — Deep-Research Stability + entity-connect Convergence (ADR-0064)

- GAP-001: `deep-research` no longer panics with "Cannot start a runtime from within a runtime" when invoked headless (agent harnesses, CI runners, scheduled jobs). The sync entry point `deep_research::run` now computes per-sub-query embeddings BEFORE building its dedicated Tokio runtime via the new `compute_sub_embeddings` helper, and the three OpenRouter embedding paths in `embedder.rs` (single, serial batch, JoinSet fan-out) adopt the canonical `Handle::try_current` + `block_in_place` reentry pattern already used by the batch path. `ingest_opencode` is also guarded. For headless orchestrators this means long-running `deep-research --with-bodies` jobs that previously crashed mid-flight now complete reliably.
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
OPENROUTER_API_KEY="$KEY" sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" \
  --until-empty --max-runtime 1800 --json
```

## v1.0.80 Update — SHUTDOWN Resilience and the 3-Layer Bypass Recipe

v1.0.80 (ADR-0034) hardens the `src/signals.rs` handler so that the
orphaned-process scenario that the G42/C2 audit identified no longer
triggers a `SIGABRT` on `BrokenPipe`. The third consecutive Ctrl-C
exits with code 130 and **ZERO I/O**, matching the contract below.

For long embedding jobs that the agent harness (or any background
orchestrator) may kill via SIGINT, use the 3-layer bypass recipe.
All 3 layers are independent and the recipe composes additively:

```bash
# Layer 1 — PATH: route the LLM subprocess through the mock CLI in CI
export PATH="$PWD/tests/mock-llm:$PATH"

# Layer 2 — env: tell the embedder to ignore the SHUTDOWN check
export SQLITE_GRAPHRAG_IGNORE_SHUTDOWN=1

# Layer 3 — process group: detach the CLI from the harness's pgroup
setsid -w timeout 600 \
  sqlite-graphrag remember --graph-stdin < payload.json
```

- **Layer 1 (PATH)**: routes any spawned `claude -p` or `codex exec`
  through the deterministic mock-llm binary checked into
  `tests/mock-llm/`. The real LLM subprocess is bypassed; SIGINT
  cannot kill a subprocess that does not exist. This is the cheapest
  layer and is the right default for CI.
- **Layer 2 (env)**: makes the embedder's `if should_obey_shutdown()`
  short-circuit to `true`, so the `tokio::select!` cancellation arm
  is dropped and the batch runs to completion even if the
  cancellation token is already cancelled. Zero overhead in
  production because the env read is one `std::env::var` per
  `should_obey_shutdown()` call, not in a hot path.
- **Layer 3 (setsid)**: gives the CLI its own process group via
  `setsid -w`, so SIGINT from the parent harness does not propagate
  to the child. `timeout` adds a hard wall-clock cap (the Rust
  `timeout-cli` v0.1.0 binary, integer seconds only — `600` is 10
  minutes; do not pass `10m`).

The recipe is now the canonical reference for any agent harness
running long embedding jobs in background. The bypass is
explicitly opt-in: production code MUST NOT call
`try_reset_shutdown()`, and the env var MUST NOT be set in
production. Tests and audit invocations are the only valid
consumers.

If the run is interrupted between layers, the SQLite file remains
consistent (WAL, atomic commit, no partial writes), and `restore`
or `enrich --operation re-embed --resume` can pick up from the
last successful memory.

## Pre-flight Validation Layer (v1.0.87+ — ADR-0045)

From v1.0.87 onwards, every LLM subprocess spawn passes through a
mandatory pre-flight gate in `src/spawn/preflight.rs` (15 unit tests,
7 guards). The gate aborts the spawn BEFORE the fork when the
invocation would fail in runtime, returning
`AppError::PreFlightFailed` (exit code 16, `EX_CONFIG`).

### The 7 guards (in order)

1. `check_argv_size` — rejects invocations whose argv total would
   exceed `ARG_MAX` minus 4 KB safety margin
2. `check_binary_exists` — confirms `claude` or `codex` is reachable
   in `PATH` before invoking
3. `check_mcp_config_inline` — replaces literal `--mcp-config {}`
   with a tempfile holding `{"mcpServers":{}}` (fixes BUG-2)
4. `check_mcp_config_path` — validates the JSON contents of
   `--mcp-config <PATH>` if used
5. `check_walkup_mcp_json` — walks the workspace root looking for
   `.mcp.json` and validates the JSON
6. `check_output_buffer` — raises the parser buffer above 64 KB
   when expected output exceeds it (fixes BUG-4)
7. `check_claude_config_dir` — validates `CLAUDE_CONFIG_DIR` is empty
   or absent (avoids MCP bleed-through from user-level config)

### Bypassing pre-flight in emergencies

Set `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1` to disable all 7 guards. This
is a **last-resort opt-out** intended for production incident
mitigation; it is not a normal mode of operation. When pre-flight
is skipped, the spawner reverts to direct `Command::spawn()` and
inherits all 5 BUG classes from GAP-META-005.

### Related regressions (v1.0.88 hotfixes)

Three BUGs were discovered and fixed in v1.0.88 after the pre-flight
gate was introduced in v1.0.87:

- **BUG-11**: preflight failure in `extract/llm_embedding.rs` did not
  propagate to `remember`, which silently persisted the memory with
  `backend_invoked: "none"` and no embedding. Fixed in v1.0.88 with
  `embed_via_backend_strict` (2 tests in `bug11_preflight_regression.rs`).
- **BUG-12**: OAuth-only enforcement emitted 2 identical stderr lines.
  Fixed in v1.0.88 with single-line stderr (test:
  `oauth_stderr_emits_single_line_v1088`).
- **BUG-13**: `link --create-missing` bypassed entity-name validation
  by normalizing the name BEFORE the validator ran. Fixed in v1.0.88
  by validating BEFORE normalizing (8 tests in
  `entity_validation_integration.rs`).

## v1.0.93 Update — OpenRouter Embedding Backend
- New `--embedding-backend openrouter` flag enables REST API embedding without LLM subprocess
- Eliminates cold-start overhead: ~200ms per embedding vs 15s with subprocess
- Requires `OPENROUTER_API_KEY` env var or `--openrouter-api-key` flag
- Requires `--embedding-model MODEL` (no default — user must specify)
- Works with all 8 embedding commands in headless mode
- Example headless invocation:

```bash
OPENROUTER_API_KEY="$KEY" sqlite-graphrag \
  --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  ingest ./docs --pattern "*.md" --recursive --json
```

## v1.0.90 Update — OpenCode as Third LLM Backend (ADR-0051)

v1.0.90 adds OpenCode as the third LLM backend for embedding,
ingest, and enrich pipelines. The auto-detect priority is now
`codex > claude > opencode > none`. The fallback chain default
is `codex,claude,opencode,none`.

### sqlite-graphrag with opencode backend

```bash
# Force opencode backend with specific model
sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle \
  remember --name example --type note --body "text" --json

# Ingest with opencode extraction
sqlite-graphrag ingest ./docs --mode opencode --recursive --json

# Enrich with opencode
sqlite-graphrag enrich --operation memory-bindings --mode opencode --json
```

### New env vars for opencode

- `SQLITE_GRAPHRAG_OPENCODE_BINARY` — override the opencode binary path
- `SQLITE_GRAPHRAG_OPENCODE_MODEL` — select the opencode model for extraction
- `SQLITE_GRAPHRAG_OPENCODE_EMBED_MODEL` — select the opencode model for embedding
- Precedence: `OPENCODE_EMBED_MODEL > OPENCODE_MODEL > default opencode/big-pickle`
- Cross-contamination fix (BUG-AUDIT-001): opencode model resolution does NOT fall back to `SQLITE_GRAPHRAG_LLM_MODEL`

## v1.0.89 Update — LLM Flag Propagation and Model Selection (ADR-0050)

v1.0.89 fixes a critical class of dead-flag bugs: 7 global CLI flags
were accepted by clap but never propagated to the internal embedding
modules. All 7 now work via CLI or env var.

### New and fixed global flags

- `--llm-model <MODEL>` / `SQLITE_GRAPHRAG_LLM_MODEL` — select the
  embedding model. Defaults: `gpt-5.5` (codex), `claude-sonnet-4-6`
  (claude). Overrides per-backend vars
  `SQLITE_GRAPHRAG_CODEX_EMBED_MODEL` and
  `SQLITE_GRAPHRAG_CLAUDE_EMBED_MODEL`
- `--llm-backend <auto|codex|claude|none>` /
  `SQLITE_GRAPHRAG_LLM_BACKEND` — select which CLI spawns the
  embedding subprocess. `auto` (default) probes PATH: codex first,
  then claude
- `--codex-binary <PATH>` / `SQLITE_GRAPHRAG_CODEX_BINARY` — override
  the codex binary location (new in v1.0.89; `--claude-binary` existed
  since v1.0.82)
- `--llm-fallback <chain>` / `SQLITE_GRAPHRAG_LLM_FALLBACK` — fallback
  chain when primary backend fails (default: `codex,claude,none`)
- `--skip-embedding-on-failure` /
  `SQLITE_GRAPHRAG_SKIP_EMBEDDING_ON_FAILURE` — persist memory without
  embedding when LLM fails (exit 0 instead of exit 11)
- `--llm-max-host-concurrency <N>` /
  `SQLITE_GRAPHRAG_LLM_MAX_HOST_CONCURRENCY` — cap concurrent LLM
  subprocesses host-wide
- `--llm-slot-wait-secs <N>` / `SQLITE_GRAPHRAG_LLM_SLOT_WAIT_SECS` —
  seconds to wait for a free slot before failing
- `--llm-slot-no-wait` / `SQLITE_GRAPHRAG_LLM_SLOT_NO_WAIT` — fail
  immediately if no slot is available

### BoolishValueParser for boolean env vars

Boolean flags with `env = "SQLITE_GRAPHRAG_*"` now accept `1`, `yes`,
`on`, `true` (and `0`, `no`, `off`, `false`). Previously only
`true`/`false` were accepted, causing exit 2 for scripts that set
`SQLITE_GRAPHRAG_SKIP_EMBEDDING_ON_FAILURE=1`.

### Headless invocation with explicit model

```bash
# Claude with explicit model
claude -p "YOUR TASK" \
  --model claude-sonnet-4-6 \
  --strict-mcp-config --mcp-config '{}' \
  --dangerously-skip-permissions \
  --settings '{"hooks":{}}' \
  --output-format json

# Codex with explicit model
codex exec \
  --model gpt-5.5 \
  -c mcp_servers='{}' \
  --sandbox workspace-write \
  --ask-for-approval never \
  "YOUR TASK"
```

### sqlite-graphrag with backend and model override

```bash
# Force claude backend with specific model
sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 \
  remember --name example --type note --body "text" --json

# Force codex backend with specific model
sqlite-graphrag --llm-backend codex --llm-model gpt-5.5 \
  recall "query" --k 5 --json

# Skip embedding on failure (persist memory without vector)
sqlite-graphrag --skip-embedding-on-failure \
  remember --name resilient --type note --body "text" --json
```


## Validated External References

### Claude Code

- `code.claude.com/docs/en/headless` — headless mode and clear exit codes
- `amux.io/guides/claude-code-headless/` — complete headless self-hosting guide (2026)
- `github.com/anthropics/claude-code/issues/39069` — `--bare` mode skips OAuth/keychain, unusable for OAuth-only
- `computingforgeeks.com/claude-code-cheat-sheet/` — cheat sheet covering `--mcp-config` and `--strict-mcp-config`
- `github.com/anthropics/claude-code/issues/14490` — `--strict-mcp-config` does not override `disabledMcpServers`

### Codex CLI

- `developers.openai.com/codex/cli/reference` — canonical CLI options reference
- `deepwiki.com/openai/codex/6.1-mcp-server-configuration` — MCP server config in `config.toml`
- `ofox.ai/blog/codex-cli-config-toml-deep-dive/` — every `config.toml` setting explained
- `github.com/openai/codex/issues/3441` — bug where `[mcp_servers]` did not work in an old Codex version

### OpenCode

- `opencode.ai/docs/mcp-servers/` — MCP control via `enabled: false` per server
- `open-code.ai/en/docs/config` — `opencode.json` reference with providers, models, MCP
- `computingforgeeks.com/opencode-cli-cheat-sheet/` — cheat sheet with headless and MCP flags


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
# Pre-flight: validate both backends are reachable before launching
timeout 30 codex exec --help >/dev/null 2>&1 || { echo "codex missing"; exit 1; }
timeout 30 claude --help >/dev/null 2>&1 || { echo "claude missing"; exit 1; }

# Launch with the fallback chain
sqlite-graphrag remember --name foo --type note --body "..." \
  --llm-backend codex,claude --json

# If all backends fail, inspect the pending queue
sqlite-graphrag pending-embeddings list --filter-status failed --json
```
### Slot semaphore poll pattern (GAP-004, ADR-0039)
```bash
# Wait until a slot is free before launching a heavy batch
while [ "$(sqlite-graphrag slots status --json | jaq '.acquired')" -gt 0 ]; do
  sleep 5
done
sqlite-graphrag ingest ./big-corpus --recursive --json
```
### codex OAuth 401 mitigation pattern (ADR-0040, 2026-06-14 incident)
```bash
# Refresh the OAuth token at the start of any long batch
codex login

# Configure the fallback chain to handle refresh_token_reused 401
sqlite-graphrag remember --name auth-fix --type decision \
  --body "Refresh-token rotation policy" \
  --llm-backend codex,claude --json
```
