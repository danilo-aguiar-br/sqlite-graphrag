---
name: sqlite-graphrag
description: This skill MUST activate for every sqlite-graphrag CLI operation covering GraphRAG memory hybrid-search recall deep-research -o remember enqueue-enrich entities_created enrich_recommended remember-batch ingest edit restore enrich force-redescribe re-embed entity-connect memory-entities merge-entities link purge config XDG OpenRouter codex claude opencode namespace isolation claim until-empty resume retry-failed debug-schema. This skill MUST be used whenever the agent stores retrieves searches enriches links merges or maintains long-term GraphRAG memory. Keywords sqlite-graphrag GraphRAG memory embedding openrouter codex claude opencode remember hybrid-search enrich force-redescribe re-embed config XDG pending embedding slots fts vec
---

## When This Skill Activates
- MUST ACTIVATE for remember/save/recall/retrieve/search/persist across sessions; GraphRAG, knowledge graph, entity linking, namespace-scoped memory; when sqlite-graphrag, embedding, FTS5, hybrid-search, OpenRouter, codex, claude, opencode, entity-connect, or LLM memory is mentioned; for enrich, re-embed, link, unlink, merge-entities, rename-entity, deep-research, ingest, config, graph maintenance, pending, slots, vacuum, purge
- NEVER ACTIVATE for ephemeral data, simple file I/O, or non-memory tasks
- ALWAYS load this skill BEFORE inventing ad-hoc memory files, MCP memory servers, or Markdown journals

## Core Mental Model
- KNOW THREE independent selectors; NEVER conflate them
- SELECTOR 1 — `--embedding-backend` HOW vectors are produced — `openrouter` (REST), `llm` (subprocess), or `auto`
- SELECTOR 2 — `--llm-backend` WHICH subprocess embeds when backend is `llm` — `codex`, `claude`, `opencode`, or `none`
- SELECTOR 3 — extraction via `enrich --mode` — `codex`, `claude-code`, `opencode`, or `openrouter` (REST chat completions); `--extraction-backend` is the related global selector
- WRITE and ENRICH are ALWAYS separate processes; write produces embeddings; SEPARATE `enrich` extracts or mutates the graph
- NEVER chain write and enrich with `&&`; ALWAYS wait for write exit 0, then run enrich as a DISTINCT process
- On EVERY OpenRouter write (`remember`, `remember-batch`, `ingest`, `edit`, `restore`, `split-body`) MUST PASS `--llm-backend none` + `--embedding-backend openrouter` + `--embedding-model <MODEL>` + `--embedding-dim 1024`
- ALWAYS pass `--json`; ALWAYS parse with `jaq` NEVER `jq`; ALWAYS capture stdout FIRST then parse; NEVER pipe CLI output directly into `jaq` (NDJSON masks failures as null)
- KNOW empty vectors are NEVER persisted; PARSE `backend_invoked`; RUN `enrich` only after write exit 0
- ALWAYS keep `--embedding-dim 1024` identical on ALL write and read embed paths; mismatch → knn exit 11
- DEFAULT dim is 1024; precedence ALWAYS flag > XDG `config set` > default; FORBIDDEN product env `SQLITE_GRAPHRAG_*` on the hot path

## Prompt Instruction Rules
- "remember this" → `remember --force-merge` with `--graph-stdin` curated entities and canonical relations, then SEPARATE `enrich`
- "what do you know about X" → `hybrid-search "X" --k 10 --json` FIRST, then `read --name <name> --json`
- "how is X related to Y" → `graph traverse --from X --depth 2 --json` or `related X --hops 2 --json`; on miss MUST RETRY with `--fuzzy` or pick exit 4 NotFound suggestions
- "deep research on X" → `deep-research "X" --k 20 --max-hops 3 --json`; large envelopes MUST use `--output PATH` or `-o PATH` and `--quiet`
- "connect isolated entities" → `enrich --operation entity-connect` with mandatory `--mode` + model, then monitor `--status`
- BEFORE create → `hybrid-search "<name>" --k 5 --json`; if duplicate MUST USE `--force-merge`
- AFTER create/update → capture-parse `read --name <name> --json` for `{name, description, body_length}`; AFTER every turn → persist findings or DECLARE "No new findings to persist"
- On non-zero exit → parse `jaq '{code, message, error_class}'` and REPORT remediation
- ALWAYS use canonical relations — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- ALWAYS map non-canonical — `adds`/`creates`→`causes`, `implements`→`supports`, `blocks`→`contradicts`, `tested-by`→`related`, `part-of`→`applies-to`
- ALWAYS kebab-case ASCII lowercase entity names; LIMIT to domain concepts; REJECT generics, pronouns, UUIDs, timestamps
- NEVER use MCP Serena, `.md` memory files, or MEMORY.md; NEVER start a daemon; NEVER pass `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` to subprocess backends
- MUST use `remember --force-merge` for idempotent updates; MUST use `--graph-stdin` or `--graph-file` when a curated graph is available

## Contract
- `--db <PATH>` MUST come AFTER the verb always — `sqlite-graphrag remember --db ./g.sqlite ...`; BEFORE the verb is REJECTED; persistent default via `config set db.path <PATH>`
- Graph surfaces REQUIRE and USE `--db`; host/XDG leaves (`config`, `slots`, `cache`, `codex-models`, `completions`) accept `--db` as documented no-op
- ALWAYS `--json`; ALWAYS `--quiet`/`-q` in headless pipelines; NEVER mix stderr into JSON with `&>` or `2>&1`
- Key precedence REQUIRED — CLI flag > XDG `config set` / `config add-key` > default; FORBIDDEN product env as primary
- EXIT codes — 0 success; 1 validation OR Timeout (EC InterruptHandle — NOT 75); 2 args; 3 optimistic lock; 4 not found (suggestions without `--fuzzy`); 5 namespace; 6 payload too large (SPLIT body); 9 duplicate (`--force-merge`); 10 database (`vacuum`+`health`); 11 embedding (backend/dim/key); 13 partial batch (reprocess failed only); 14 I/O; 15 busy (widen `--wait-lock`); 16 preflight (fix MCP; NEVER transient); 19 SHUTDOWN (retry MANDATORY); 20 internal; 75 singleton locked (NEVER retry immediately); 77 RAM; 78 config (key/model missing)
- NEVER ignore non-zero; NEVER reprocess full batch after exit 13; NEVER confuse exit 1 Timeout with exit 75 or exit 9

## Architecture
- INVOKE as subprocess; stdout = JSON/NDJSON; stderr = logs; CHECK exit code BEFORE parsing; NO daemon, NO ONNX, NO model cache; cosine is pure Rust over BLOB `memory_embeddings` / `entity_embeddings` / `chunk_embeddings`; FUSION is FTS5 BM25 plus BLOB cosine KNN via RRF
- KNOW `init` or `migrate` applies live schema; READ `schema_version` from `health --json`
- ENFORCE OAUTH-ONLY for codex/claude — spawn ABORTS exit 1 if `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` is set; `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` are PRESERVED
- KNOW subprocess CWD is ISOLATED; 7 preflight guards before every LLM fork; exit 16 = preflight failure; `claude -p` inherits CWD `.mcp.json` — MUST ISOLATE config for `claude-code` or MUST use codex
- SET emergency preflight skip ONLY via `sqlite-graphrag config set spawn.skip_preflight=1` (EMERGENCIES ONLY); namespace via `--namespace` or XDG (default `global`)
- NEVER expose as MCP/HTTP; NEVER write `.sqlite` from another tool

## OpenRouter Models
- PASS `--embedding-model <MODEL>` when `--embedding-backend openrouter`; NO default model → exit 78 on omission; prices indicative USD per million tokens; ALWAYS confirm live via `usage.cost` when available
- EMBED catalog — `nvidia/llama-nemotron-embed-vl-1b-v2:free` FREE; `qwen/qwen3-embedding-4b` $0.05/M; `qwen/qwen3-embedding-8b` $0.05/M DEFAULT operational; `openai/text-embedding-3-small` $0.05/M; `perplexity/pplx-embed-v1-0.6b` $0.05/M; `baai/bge-m3` ~$0.05/M; `mistralai/mistral-embed-2312` $0.10/M; `google/gemini-embedding-2` ~$0.12/M; `openai/text-embedding-3-large` $0.13/M; `google/gemini-embedding-005` ~$0.15/M
- KNOW MRL truncates server-side to `--embedding-dim` (default 1024); dim mismatch → exit 11
- openrouter propagates to ALL embed paths — `remember` `remember-batch` `ingest` `recall` `edit` `restore` `hybrid-search` `deep-research` `enrich` `init` `rename-entity` `split-body`
- REQUIRED ADD key — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- LIST — `config list-keys --json`; REMOVE — `config remove-key <fingerprint> --json`; DOCTOR — `config doctor --json`; PATH — `config path`
- Keys live in XDG `~/.config/sqlite-graphrag/config.toml` with `chmod 600`, zeroized on drop, NEVER logged
- NEVER pass API key as CLI argument in production shell history; ALWAYS prefer `config add-key --from-stdin`; ALWAYS run `config doctor` after adding a key before paid calls
- Text models serve ONLY extraction/enrichment, NEVER embedding; MUST use `openai/gpt-oss-120b` as DEFAULT judge; `:nitro` = fastest provider at higher price
- TEXT catalog (all IDs REQUIRED literal) — `deepseek/deepseek-v4-flash`, `deepseek/deepseek-v4-flash:nitro`, `deepseek/deepseek-v4-pro`, `google/gemini-3.1-flash-lite`, `minimax/minimax-m3`, `minimax/minimax-m2.7`, `minimax/minimax-m2.7:nitro`, `openai/gpt-oss-120b`, `openai/gpt-oss-120b:nitro`, `xiaomi/mimo-v2.5`, `xiaomi/mimo-v2.5-pro`, `z-ai/glm-5.2`, `z-ai/glm-5.2:nitro`
- VERIFY strict `json_schema` BEFORE production; missing Structured Outputs → explicit OpenRouter error

## Headless LLM Backends
- ALWAYS pass the model flag explicitly; NEVER rely on silent defaults alone
- CODEX — `enrich --mode codex --codex-model <MODEL>`; OAuth-only; default `gpt-5.5`; `codex login`; embedding path `--llm-backend codex --llm-model <MODEL>`
- CLAUDE — `enrich --mode claude-code --claude-model <MODEL>`; OAuth-only; default `claude-sonnet-4-6`; embedding path `--llm-backend claude --llm-model <MODEL>`
- OPENCODE — `enrich --mode opencode --opencode-model <MODEL>`; default `opencode/big-pickle`; embedding path `--llm-backend opencode --llm-model <MODEL>`; own auth (NOT OAuth); `--opencode-model` UNVALIDATED — PASS live OpenCode Zen ids
- OPENROUTER extraction — MUST use `enrich --mode openrouter --openrouter-model <id>`; `--openrouter-model` is MANDATORY (no default; missing value exits 1 before network)
- OVERRIDE binaries `--codex-binary`, `--claude-binary`, `--opencode-binary`; TUNE timeouts `--codex-timeout`, `--claude-timeout`, `--opencode-timeout`
- VALIDATE codex models with `--codex-model-validate` and `--codex-model-fallback <MODEL>`; LIST with `codex-models --json` (CODEX only)
- SWAP backend on rate limit with `enrich --fallback-mode codex` or global `--llm-fallback codex,claude,none`
- KNOW `--mode openrouter` is pure REST — NO local CLI; bills stored OpenRouter key

## Global Flags
- `--db <PATH>` AFTER verb; `--namespace <ns>`; `--json` ALWAYS; `--lang en|pt`; `--tz <TIMEZONE>`
- `--embedding-backend auto|openrouter|llm`; `--embedding-model <MODEL>` MANDATORY with openrouter; `--embedding-dim N` default 1024 MRL [8, 4096]
- `--openrouter-api-key <KEY>` FORBIDDEN in production shell history; prefer `config add-key --from-stdin`
- `--llm-backend codex|claude|opencode|none|auto`; `--llm-model <MODEL>`; `--llm-fallback <chain>`
- `--extraction-backend`; `--openrouter-model <MODEL>` MANDATORY for `--mode openrouter`; `--openrouter-base-url`; `--openrouter-timeout` default 600
- `--llm-parallelism N` embed fan-out default 4 clamp [1, 32]; `--rest-concurrency N` openrouter enrich fan-out clamp [1, 16] default 8; DISTINCT flags
- `--max-concurrency N` clamp [1, 2×nCPUs]; `--llm-max-host-concurrency N`; `--llm-slot-wait-secs N` / `--llm-slot-no-wait`; `--wait-lock SECS`; `--low-memory`; `--strict-env-clear`; `--graceful-shutdown-secs N`; `--skip-embedding-on-failure`
- `--codex-binary`, `--claude-binary`, `--opencode-binary`; `-v`/`-vv`/`-vvv`; `--quiet`/`-q` MANDATORY in headless pipelines

## FULL Command Catalog
- TOP-LEVEL — `init` `remember` `remember-batch` `ingest` `recall` `read` `list` `forget` `purge` `rename` `split-body` `edit` `history` `restore` `hybrid-search` `health` `migrate` `namespace-detect` `optimize` `stats` `sync-safe-copy` `backup` `vacuum` `link` `unlink` `deep-research` `related` `graph` `export` `fts` `vec` `codex-models` `prune-relations` `prune-ner` `slots` `pending` `embedding` `pending-embeddings` `cleanup-orphans` `memory-entities` `cache` `delete-entity` `reclassify` `rename-entity` `merge-entities` `enrich` `reclassify-relation` `normalize-entities` `completions` `config` `debug-schema` `help`
- `graph` family — `graph traverse` `graph stats` `graph entities` `graph recompute-degree` plus snapshot export flags `--format json|dot|mermaid|ndjson --output`
- `config` family — `config add-key` `config list-keys` `config remove-key` `config doctor` `config path` `config set` `config get` `config list` `config unset`
- `fts` family — `fts rebuild` `fts check` `fts stats`
- `vec` family — `vec orphan-list` `vec purge-orphan` `vec stats`
- `slots` family — `slots status` `slots release` `slots cleanup`
- `pending` family — `pending list` `pending show` `pending cleanup`
- `embedding` family — `embedding status` `embedding list` `embedding abandon`
- `pending-embeddings` family — `pending-embeddings list` `pending-embeddings status` `pending-embeddings abandon` (aliases of embedding)
- `cache` family — `cache clear-models` `cache list` `cache stats`
- `completions` — `completions bash|zsh|fish|elvish|powershell`
- `debug-schema` — dump live schema for diagnostics; `help` — full CLI help

## CRUD Write
- INVOKE `remember --name <kebab> --type <kind> --description <text>` with exactly one body source — `--body` or `--body-file` or `--body-stdin` or `--graph-stdin`
- INVOKE `remember --graph-stdin` for `{body, entities, relationships}`; or `--graph-file` with `--body-file`
- PASS entities `[{name, entity_type}]` kebab-case ASCII; relationships `[{source, target, relation, strength}]` strength [0.0, 1.0]
- REQUIRED graph-stdin entity allowlist — ONLY keys `name`, `entity_type` (alias `type` folded), optional `description`; FORBIDDEN `observations`, `aliases`, free-form extras → exit 1
- PASS `--strict-name`; `--force-merge` for idempotent updates; `--replace-graph` with `--force-merge`; `--dry-run` to validate without persisting
- PASS `--enqueue-enrich` on `remember` ONLY for hot-set entity-descriptions after write; default OFF
- PARSE remember JSON for `entities_created[]` and `enrich_recommended[]`; NEVER ignore; WHEN `enrich_recommended` non-empty MUST run SEPARATE `enrich --operation entity-descriptions` AFTER write exit 0
- VALID memory `--type` — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- INVOKE `remember-batch` for 10+ memories via NDJSON stdin; PASS `--transaction`; every create line MUST include non-empty `description` and `type`
- INVOKE `ingest <DIR> --recursive --pattern "*.md" --mode none` for body-only import, then enrich SEPARATELY; `ingest --mode` accepts `none` (default), `claude-code`, `codex`, `opencode`
- USE `--resume`; `--retry-failed`; `--auto-describe`; `--name-prefix <prefix>`; `--force-merge` (dedup by `body_hash`); ingest auto-splits oversized bodies
- INVOKE `split-body --name <N>` for ONE memory over 25000 chars; PASS `--batch --threshold 25000` for all oversize; DAUGHTERS NOT EMBEDDED INLINE — step1 openrouter embed + `--llm-backend none` on `split-body`; step2 SEPARATE `enrich --operation re-embed --target memories`
- RESPECT 512000 bytes and 512 chunks per body; NEVER mix body sources; NEVER `fd | xargs remember` — USE `ingest`
- NEVER pass non-`none` `--llm-backend` on OpenRouter write; ALWAYS pass `--llm-backend none`

## CRUD Read Update Delete
- INVOKE `read --name <kebab> --json`; PASS `--with-graph`; USE `--format raw` for pure body
- INVOKE `list --type <kind> --limit N --offset N --json`; `history --name <n> --diff --json`
- INVOKE `edit --name <n> --body-file <path>` or `--description` / `--memory-type`; USE `--force-reembed`; USE `--expected-updated-at <ts>` (exit 3 = conflict — reload and retry)
- INVOKE `rename --name <old> --new-name <new>`; `restore --name <n> --version <N>` (write path — OpenRouter embed + `--llm-backend none`, then SEPARATE enrich)
- INVOKE `forget --name <n>`; hard-delete `purge --yes --dry-run` then drop `--dry-run`
- REQUIRED — `purge --yes` alone keeps default 90-day retention; for immediate wipe `purge --yes --now` (alias `--retention-days 0`)
- ALWAYS dry-run first `purge --now --dry-run --json`; then `cleanup-orphans --yes` then `vacuum --json`
- NEVER skip optimistic locking; NEVER delete via the `sqlite3` shell

## Entity Graph
- INVOKE `link --from <a> --to <b> --relation <type> --create-missing --weight <float>`; MUST use `link --from-id <N> --to-id <M>` when IDs known; NEVER pure digits as `--from`/`--to` names
- INVOKE `unlink --from <a> --to <b> --relation <type>` or `--entity <name> --all`; `unlink --memory <name> --entity <name>` for single binding
- INVOKE `graph entities --json` via `.entities[]` (NOT `.items[]`); ORDER `--sort-by name|degree|created-at`; PAGINATE `--limit`/`--offset`
- INVOKE `graph stats --json`; `graph traverse --from <root> --depth <N> --json`; EXPORT `--format json|dot|mermaid|ndjson --output <path>`
- MUST pass `--fuzzy` on ambiguous short-name traverse; WITHOUT `--fuzzy`, exit 4 includes ranked suggestions — ALWAYS use them
- INVOKE `rename-entity --name <old> --new-name <new>` or `--id <N> --new-name <new>`
- INVOKE `delete-entity --name <n> --cascade`; `merge-entities --names "a,b,c" --into <target>` or `--ids 12,17 --into-id 3`
- NEVER put `--into-id` inside `--ids` or `--into` inside `--names`; self-referential merges REJECTED BEFORE DB work; ALWAYS USE shell arrays for dynamic merge lists; PASS `--cross-namespace` only when intentional
- INVOKE `reclassify --name <n> --new-type <kind>` or `--from-type <old> --to-type <new> --batch`
- INVOKE `reclassify-relation --from-relation <old> --to-relation <new> --batch`; PASS `--literal-from`/`--literal-to` for verbatim match
- INVOKE `prune-relations --relation mentions --dry-run` then drop `--dry-run` with `--yes`; `normalize-entities --yes`; `prune-ner --entity <n>` or `--all --yes`
- INVOKE `memory-entities --name <memory>` or `--entity <name>`; PARSE `entities[].{name, description, entity_type}` — `description` REQUIRED in envelope (empty string when unset); ALWAYS surface `description` when present
- INVOKE `graph recompute-degree --json` after delete/merge/prune (degree NOT auto-recomputed)
- CANONICAL entity types — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- VALIDATE entity names — min 2 chars, no newlines, no short ALL_CAPS ≤4 chars, REJECT pure digits; NEVER use `mentions` as default relation; graph writes ADDITIVE with NO degree cap

## GraphRAG Search
- USE three-layer pattern — `hybrid-search` then `read --name` then `related` or `graph traverse`
- INVOKE `recall <query> --k N` for pure semantic KNN; PASS `--no-graph`, `--precise`, `--max-distance <f>`, `--max-graph-results N`, `--all-namespaces`
- INVOKE `hybrid-search <query> --k N` for FTS5 plus KNN RRF; PASS `--rrf-k 60`; `--weight-vec 1.0 --weight-fts 1.0`; `--fallback-fts-only`; USE `--with-graph --max-hops 2 --min-weight 0.3`; READ BOTH `results[]` AND `graph_matches[]`
- INVOKE `related <name> --hops N --relation <type>`
- INVOKE `deep-research "<query>" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies`; single-token queries fan out; manual control PASS `--sub-query-strategy manual --sub-queries-file PATH`
- WRITE large envelopes with `--output PATH` or `-o PATH` (atomwrite); PARSE ack `{written, bytes, blake3}`; PASS `--quiet`; NEVER `&>`; when `-o`/`--output` set the file MUST exist non-zero after exit 0
- TUNE with `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--max-cost-usd`, `--timeout`
- PARSE `recall` → `results[].{name, snippet, distance, score, source}`; `hybrid-search` → `results[].{name, combined_score, vec_rank, fts_rank}`; `deep-research` → `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context`, `stats`
- NEVER confuse `distance` with `combined_score`; NEVER raise hops without inspecting `graph stats` first

## Enrich Pipeline Rules
- INVOKE `enrich --operation <op> --mode <backend>` — BOTH MANDATORY for LLM ops; omitting `--mode` → exit 2; EXCEPT read-only inspectors `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` and `--dry-run` (mode optional)
- PERSIST ops — `memory-bindings`, `augment-bindings` (REQUIRES `--names`/`--memory-names`/`--names-file`), `entity-descriptions`, `body-enrich`, `re-embed`, `weight-calibrate`, `relation-reclassify`, `entity-connect`, `entity-type-validate`, `description-enrich`, `cross-domain-bridges`, `domain-classify`, `deep-research-synth`, `body-extract` + `--body-extract-graph-only`; SCAN/REPORT only — `graph-audit`
- Valid `--mode` — `codex`, `claude-code`, `opencode`, `openrouter`; PASS matching model flag; `--mode openrouter` requires `--openrouter-model`
- REQUIRED name filters — prefer `--entity-names a,b` for entity-keyed ops and `--memory-names a,b` for memory-keyed ops; `--names` is BC alias; empty match MUST surface `matched=0` + `hint` then STOP
- CLAIM / DEQUEUE / DRAIN SCOPE — `count_eligible_pending`, `dequeue_next_pending`, `--resume`, `--retry-failed`, and `--until-empty` are scoped to operation + namespace ONLY; a drain for `ai-sdd` MUST NOT claim or count `global`/empty-ns rows; `--until-empty` counts ONLY this op+namespace (NEVER all pending across ops)
- `--force-redescribe` on `entity-descriptions` reopens matching `skipped`/`done` queue rows to `pending` ONCE per process before first enqueue so `INSERT OR IGNORE` is not a silent no-op; NEVER reopens `dead` (use `--requeue-dead`); default write-once for non-empty descriptions
- Low-quality markers are COMPOUND only (e.g. `is a configuration file`, `is a software component`) — bare domain phrases like `configuration file` alone MUST NOT trigger force-redescribe fodder
- Re-embed eligibility uses BLOB length `LENGTH(embedding)=dim*4`, NOT the `dim` column alone — CORRUPT/META_AHEAD rows (dim=1024 with a 384-d BLOB) remain eligible; `reconcile_satisfied_reembed_pending` marks pending ReEmbed rows `done` when a live vector already exists at the active dim, clearing zombies without API calls
- Enqueue validates re-embed keys — `entity:{name}` strips the `entity:` prefix for entity lookup; bare names still work; missing entities REJECTED; chunk keys validate `chunk_id` exists in a non-deleted memory of the target namespace
- PASS `--target memories|entities|chunks|all` on `re-embed` only (default `memories`); PASS `--limit N --resume`; `--retry-failed`; `--dry-run`
- PASS `--quality-sample N` with `--status` for `quality_pct` and `scan_backlog_low_grounding_est` (flag > XDG `enrich.entity_description.quality_sample` > default 50; `0` disables)
- Queue isolation — drain claims only selected `operation` rows; memory-only ops MUST NOT claim `pair:`/`entity:`/`chunk:` keys; status `state` = `draining`|`cooldown`|`pending-scan`|`blocked_dead`; `blocked_dead` → `--list-dead`/`--requeue-dead`/prune FIRST
- NEVER run multiple `enrich` processes on the same DB; REST parallelism is ONLY `--rest-concurrency` inside ONE process
- PASS `--until-empty` to loop scan→drain until empty or `--max-runtime` (default 3600); PASS `--max-attempts <N>` default 8 range 1..=20
- PASS `--status` for `scan_backlog`, `unbound_backlog`, queue counts, `eligible_now`, `waiting`, `quality_pct`, `state` — NO LLM, NO singleton
- DISTINGUISH — `scan_backlog` = DB candidates a fresh scan WOULD enqueue; `queue_pending` = sidecar count; `eligible_now == 0` with `queue_pending > 0` is COOLDOWN; stuck `draining` → `--reset-stale-claims`
- Ops list compressed — PASS `--list-dead`; `--requeue-dead`; `--list-skipped`; `--requeue-skipped` (recover skipped/`preservation_failed` without raw SQL); `--ignore-backoff`; `--prune-dead-orphans`; `--prune-dead-entity-orphans` (mutually exclusive); `--reset-stale-claims` after `kill -9`
- KNOW dead-letter Transient vs HardFailures; truncated OpenRouter completions (`finish_reason`=`length`) re-emit with GROWN `max_tokens`; queue is sidecar `.enrich-queue.sqlite`
- ENTITY-CONNECT PERSISTS edges via `entity_connect_seen` with `related`|`none`; `cross-domain-bridges` uses SAME scan/drain; pair scan is O(k) co-occurrence + hub×degree-0 fill — NEVER full Cartesian; queue keys `pair:{id1}:{id2}` `item_type=entity_pair`
- First scan covered by `--max-runtime` and soft ~120s `InterruptHandle`; interrupt → Timeout exit 1 — NEVER exit 75
- PARSE `budget_exhausted` (runtime budget ends on large namespaces) and `preempted_for_gate` (EC yielded so memory-bindings/entity-descriptions run first)
- PASS `--anchor-memory <name>` and/or `--entity-names a,b`; empty match → `matched=0` + `hint`; ALWAYS `--until-empty` + inspect `--status`; ALWAYS dry-run first on production corpora
- Priority — memory-bindings then entity-descriptions BEFORE entity-connect; long EC drains MUST yield; legacy non-`pair:` queue rows ignored

## Write→Enrich Matrix Formulas
- KEY SETUP MUST store OpenRouter key via `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- KEY SETUP MUST verify with `config list-keys --json` and `config doctor --json`
- KEY SETUP MUST set network URLs when needed — `config set network.openrouter.chat_url https://openrouter.ai/api/v1/chat/completions` and `config set network.openrouter.embeddings_url https://openrouter.ai/api/v1/embeddings`
- KEY SETUP NEVER pass API key as CLI arg in production history
- DEFINE OPENROUTER WRITE PREFIX `W` = `sqlite-graphrag --embedding-backend openrouter --embedding-model <EMB> --embedding-dim 1024 --llm-backend none`
- DEFAULT `<EMB>` = `qwen/qwen3-embedding-8b`; FREE path `nvidia/llama-nemotron-embed-vl-1b-v2:free`
- PARALLEL embed MUST add `--llm-parallelism N` on STEP1 (clamp 1..32)
- TREAT every write as STEP1 then DISTINCT STEP2; NEVER chain with `&&`; ALWAYS parse `entities_created` and `enrich_recommended` on remember
- STEP2 MUST RUN exactly ONE mode after write exit 0; PARALLEL openrouter enrich MUST use ONE process with `--rest-concurrency N` (clamp 1..16) NEVER N enrich processes; paid models MUST use 4..16; `:free` caps ~20 req/min so MUST use low N

### REMEMBER
- STEP1 MUST RUN `W remember --db ./g.sqlite --name <n> --type decision --description "..." --graph-stdin --force-merge --json` (or with `--llm-parallelism 8`); body via stdin graph JSON `{body, entities, relationships}`
- STEP1 hot-set — when hot-set entity-descriptions are REQUIRED MUST PASS `--enqueue-enrich`; default remains OFF
- STEP2 MUST RUN exactly ONE after exit 0
  - openrouter `enrich --db ./g.sqlite --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json` (+ optional `--rest-concurrency 8 --until-empty`)
  - codex `enrich --db ./g.sqlite --operation memory-bindings --mode codex --codex-model gpt-5.5 --json`
  - claude-code `enrich --db ./g.sqlite --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
  - opencode `enrich --db ./g.sqlite --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- WHEN `enrich_recommended` has entity-descriptions → run `enrich --operation entity-descriptions --mode <backend> --entity-names <list> --json` AFTER memory-bindings (same mode/model pattern as STEP2)

### REMEMBER-BATCH
- STEP1 MUST RUN `W remember-batch --db ./g.sqlite --transaction --json` with NDJSON stdin (or with `--llm-parallelism 8`)
- STEP2 MUST RUN exactly ONE after exit 0 — same four-mode matrix as REMEMBER STEP2 (`memory-bindings` + mode/model flags)

### INGEST
- STEP1 MUST RUN `W ingest --db ./g.sqlite ./docs --mode none --recursive --pattern "*.md" --type document --resume --json` (body-only; NEVER non-`none` ingest mode when OpenRouter write path is chosen)
- STEP2 MUST RUN exactly ONE after exit 0 — same four-mode matrix as REMEMBER STEP2

### EDIT
- STEP1 MUST RUN `W edit --db ./g.sqlite --name <n> --body-file new.md --json` (or `--description` / `--memory-type` / `--force-reembed`)
- STEP2 MUST RUN exactly ONE after exit 0 — same four-mode matrix as REMEMBER STEP2 when graph re-extraction is REQUIRED

### RESTORE
- STEP1 MUST RUN `W restore --db ./g.sqlite --name <n> --version <N> --json`
- STEP2 MUST RUN exactly ONE after exit 0 — same four-mode matrix as REMEMBER STEP2 when re-binding is REQUIRED

### SPLIT-BODY re-embed path
- STEP1 MUST RUN `W split-body --db ./g.sqlite --name <N> --json` (or `--batch --threshold 25000`)
- STEP2 MUST RUN `enrich --db ./g.sqlite --operation re-embed --target memories --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --json` (or codex/claude-code/opencode mode+model equivalents)

## Read/Search Formulas
- INIT — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 1024 init --db ./g.sqlite --namespace <ns>`
- HYBRID-SEARCH — `sqlite-graphrag --embedding-backend openrouter --embedding-model baai/bge-m3 --embedding-dim 1024 hybrid-search --db ./g.sqlite "query" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- HYBRID offline — `sqlite-graphrag hybrid-search --db ./g.sqlite "query" --k 10 --fallback-fts-only --json`
- RECALL — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 recall --db ./g.sqlite "query" --k 10 --json`
- DEEP-RESEARCH — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 deep-research --db ./g.sqlite "question" --k 20 --max-hops 3 -o /tmp/dr.json --json`
- RENAME-ENTITY (embed path) — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 rename-entity --db ./g.sqlite --name <old> --new-name <new> --json`
- MEMORY-ENTITIES — `sqlite-graphrag memory-entities --db ./g.sqlite --name <memory> --json` then parse `entities[].description`
- TRAVERSE — `sqlite-graphrag graph traverse --db ./g.sqlite --from <root> --depth 2 --json`; fuzzy — add `--fuzzy`
- LINK — `sqlite-graphrag link --db ./g.sqlite --from <a> --to <b> --relation uses --json`; by ID — `link --from-id <N> --to-id <M> --relation uses --json`
- MERGE — `sqlite-graphrag merge-entities --db ./g.sqlite --names "a,b,c" --into <target> --json`; NEVER self-ref (`--ids 3,12 --into-id 3` FORBIDDEN)

## Enrich/Maintenance Formulas
- STATUS — `sqlite-graphrag enrich --db ./g.sqlite --status --quality-sample 50 --json`
- UNTIL-EMPTY openrouter — `sqlite-graphrag enrich --db ./g.sqlite --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --max-attempts 8 --rest-concurrency 8 --json`
- FORCE-REDESCRIBE — `sqlite-graphrag enrich --db ./g.sqlite --operation entity-descriptions --mode openrouter --openrouter-model openai/gpt-oss-120b --force-redescribe --entity-names jwt,auth-svc --json`
- RE-EMBED entities — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 enrich --db ./g.sqlite --operation re-embed --target entities --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --json`
- RE-EMBED memories|chunks|all — same formula with `--target memories` or `--target chunks` or `--target all` then `health --json`
- LIST/REQUEUE skipped — `sqlite-graphrag enrich --db ./g.sqlite --operation entity-descriptions --list-skipped --json` then `... --requeue-skipped --json`
- LIST/REQUEUE dead — `... --list-dead --json` then `... --requeue-dead --json`
- EC until-empty — `sqlite-graphrag enrich --db ./g.sqlite --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 600 --rest-concurrency 8 --json`
- EC dry-run — same with `--dry-run --limit 50` instead of `--until-empty`
- HEALTH — `sqlite-graphrag health --db ./g.sqlite --json` for `{integrity_ok, schema_version, vec_*_missing, vec_*_coverage_pct}`; TRIGGER re-embed when missing > 0
- DEBUG-SCHEMA — `sqlite-graphrag debug-schema --db ./g.sqlite --json`
- CONFIG — `sqlite-graphrag config set <key> <value>`; `config get <key>`; `config list --json`; `config list --effective --json`; `config unset <key>`; `config path`; `config doctor --json`
- Common XDG keys — `db.path`, `embedding.dim` (1024), `embedding.backend`, `embedding.model`, `llm.backend`, `llm.model`, `llm.query_embed_timeout_secs` (default 3s), `display.tz`, `i18n.lang`, `log.level`, `log.format`, `spawn.skip_preflight` (emergencies only), `enrich.yield_every_n_items`, `enrich.entity_description.quality_sample`
- PURGE now — `sqlite-graphrag purge --db ./g.sqlite --yes --now --dry-run --json` then drop `--dry-run`; then `cleanup-orphans --yes` then `vacuum --json`
- MIGRATE — `migrate --dry-run --json` then `migrate --json`; OPTIMIZE — `optimize --json`; FTS — `fts check|stats|rebuild --json`; VEC — `vec orphan-list --json` then `vec purge-orphan --yes`; `vec stats --json`
- EMBEDDING — `embedding status --json`; alias `pending-embeddings status --json`; re-process via `enrich --operation re-embed`
- SLOTS — `slots status --json`; `slots release --slot-id <N> --yes`; `slots cleanup --yes`; PENDING — `pending list --json`; `pending show <id>`; `pending cleanup --yes`
- EXPORT — `export --namespace <ns> --type <kind> --json`; STATS — `stats --json`; BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECT — `namespace-detect --json`, `cache list --json`, `cache stats --json`, `cache clear-models --yes`; COMPLETIONS — `completions bash|zsh|fish|elvish|powershell`
- HELP — `sqlite-graphrag help`
- SCHEDULE weekly — `purge --yes` (90d) or `purge --yes --now` → `cleanup-orphans` → `prune-relations --relation mentions` → `vacuum` → `optimize` → `sync-safe-copy`
- CONCURRENCY — hard ceiling `2 x nCPUs` for `init`/`remember`/`ingest`/`recall`/`hybrid-search`; JOB SINGLETON on `enrich` and `ingest --mode codex|claude-code`; USE `--wait-job-singleton SECS` or `--force-job-singleton`; NEVER parallel enrich on same DB

## Anti-Patterns NEVER
- NEVER chain write and enrich with `&&`; ALWAYS wait exit 0 then separate enrich
- NEVER put `--db` before the verb; ALWAYS after
- NEVER mix stderr into JSON (`&>` / `2>&1`); ALWAYS `--quiet` + capture stdout only
- NEVER use product env `SQLITE_GRAPHRAG_*` as primary config; ALWAYS flag > XDG > default
- NEVER pass `ANTHROPIC_API_KEY`/`OPENAI_API_KEY` to codex/claude (OAuth-only exit 1)
- NEVER openrouter without model+key (exit 78); NEVER omit `--embedding-dim 1024` on embed paths
- NEVER run multiple enrich processes on same DB; REST via `--rest-concurrency` only
- NEVER ignore `entities_created`/`enrich_recommended`; NEVER ignore exit 19 (retry) or 16 (MCP)
- NEVER self-ref merge; NEVER pure-digit entity names as `--from`/`--to`; NEVER `mentions` as default relation
- NEVER use MCP memory / MEMORY.md / ad-hoc `.md` journals; NEVER write `.sqlite` outside the binary
- NEVER treat bare "configuration file" as low-quality redescribe trigger; ONLY compound markers
- NEVER assume `--until-empty` drains all operations; it is scoped to THIS op+namespace
- NEVER assume re-embed uses only the `dim` column; eligibility is `LENGTH(embedding)=dim*4`
- NEVER assume `entity:` keys fail lookup; prefix is stripped on enqueue
- NEVER reprocess full batch after exit 13; NEVER confuse exit 1 Timeout with exit 75
- NEVER reopen `dead` with `--force-redescribe`; use `--requeue-dead`
- CANONICAL memory types — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- CANONICAL entity types — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- CANONICAL relations — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
