---
name: sqlite-graphrag
description: This skill MUST activate for every sqlite-graphrag CLI operation covering GraphRAG memory, hybrid-search, recall, deep-research -o, remember enqueue-enrich entities_created enrich_recommended, remember-batch, ingest, edit, restore, enrich force-redescribe entity-names memory-names quality_pct blocked_dead budget_exhausted preempted_for_gate, entity-connect scale, memory-entities description, forget, purge, link, unlink, merge-entities, rename-entity, reclassify, graph traverse, OpenRouter embed and text models, XDG keys, headless codex claude opencode, write-then-enrich formulas, parallel embedding, exit codes, concurrency, FTS5 BLOB fusion, canonical types relations, namespace isolation, OAuth-only. This skill MUST be used whenever the agent stores, retrieves, searches, enriches, links, merges, or maintains long-term GraphRAG memory. Keywords sqlite-graphrag GraphRAG memory embedding openrouter codex claude opencode remember recall hybrid-search ingest enrich entity-connect deep-research force-redescribe enqueue-enrich config XDG
---

## When This Skill Activates
- MUST ACTIVATE for remember/save/recall/retrieve/search/persist across sessions; for GraphRAG, RAG, knowledge graph, entity linking, namespace-scoped memory; when sqlite-graphrag, embedding, FTS5, hybrid-search, OpenRouter, codex, claude, opencode, entity-connect, or LLM memory is mentioned; for enrich, re-embed, link, unlink, merge-entities, rename-entity, deep-research, ingest, config keys, graph maintenance
- NEVER ACTIVATE for ephemeral data, simple file I/O, or non-memory tasks; ALWAYS load this skill before inventing ad-hoc memory files, MCP memory servers, or Markdown journals


## Core Mental Model
- KNOW THREE independent selectors; NEVER conflate them
- SELECTOR 1 — `--embedding-backend` HOW vectors are produced — `openrouter` (REST), `llm` (subprocess), or `auto`
- SELECTOR 2 — `--llm-backend` WHICH subprocess embeds when backend is `llm` — `codex`, `claude`, `opencode`, or `none`
- SELECTOR 3 — extraction via `enrich --mode` — `codex`, `claude-code`, `opencode`, or `openrouter` (REST `/chat/completions`); `--extraction-backend` is the related global selector
- WRITE and ENRICH are ALWAYS separate processes; write produces embeddings; SEPARATE `enrich` extracts or mutates the graph; NEVER chain write and enrich with `&&`; ALWAYS wait for write exit 0, then run enrich as a DISTINCT process
- On EVERY OpenRouter write (`remember`, `remember-batch`, `ingest`, `edit`, `restore`) MUST PASS `--llm-backend none` + `--embedding-backend openrouter` + `--embedding-model <MODEL>` + `--embedding-dim 1024` so embeddings STILL run via OpenRouter REST without an LLM subprocess timeout
- ALWAYS pass `--json`; ALWAYS parse with `jaq` NEVER `jq`; ALWAYS capture stdout FIRST then parse; NEVER pipe CLI output directly into `jaq` (NDJSON masks failures as null)
- KNOW empty vectors are NEVER persisted; PARSE `backend_invoked`; RUN `enrich` only after write exit 0
- ALWAYS keep `--embedding-dim 1024` identical on ALL write and read embed paths; mismatch → knn exit 11


## Prompt Instruction Rules
- "remember this" → `remember --force-merge` with `--graph-stdin` curated entities and canonical relations, then SEPARATE `enrich`
- "what do you know about X" → `hybrid-search "X" --k 10 --json` FIRST, then `read --name <name> --json`
- "how is X related to Y" → `graph traverse --from X --depth 2 --json` or `related X --hops 2 --json`; on miss MUST RETRY with `--fuzzy` or pick exit 4 NotFound suggestions
- "deep research on X" → `deep-research "X" --k 20 --max-hops 3 --json`; large envelopes MUST use `--output PATH` and `--quiet`
- "connect isolated entities" → `enrich --operation entity-connect` with mandatory `--mode` + model, then monitor `--status`
- BEFORE create → `hybrid-search "<name>" --k 5 --json`; if duplicate MUST USE `--force-merge`
- AFTER create/update → capture-parse `read --name <name> --json` for `{name, description, body_length}`; AFTER every turn → persist findings or DECLARE "No new findings to persist"
- On non-zero exit → parse `jaq '{code, message, error_class}'` and REPORT remediation
- ALWAYS use canonical relations — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- ALWAYS map non-canonical — `adds`/`creates`→`causes`, `implements`→`supports`, `blocks`→`contradicts`, `tested-by`→`related`, `part-of`→`applies-to`
- ALWAYS kebab-case ASCII lowercase entity names; LIMIT to domain concepts; REJECT generics, pronouns, UUIDs, timestamps
- NEVER use MCP Serena, `.md` memory files, or MEMORY.md; NEVER start a daemon; NEVER pass `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` to subprocess backends
- MUST use `remember --force-merge` for idempotent updates; MUST use `--graph-stdin` or `--graph-file` when a curated graph is available


## Architecture
- INVOKE as subprocess; stdout = JSON/NDJSON; stderr = logs; CHECK exit code BEFORE parsing; NO daemon, NO ONNX, NO model cache; cosine is pure Rust over BLOB `memory_embeddings` / `entity_embeddings` / `chunk_embeddings`; FUSION is FTS5 BM25 plus BLOB cosine KNN via RRF
- KNOW `init` or `migrate` applies live schema; READ `schema_version` from `health --json`
- ENFORCE OAUTH-ONLY for codex/claude — spawn ABORTS exit 1 if `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` is set; `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` are PRESERVED
- KNOW subprocess CWD is ISOLATED; 7 preflight guards before every LLM fork; exit 16 = preflight failure; `claude -p` inherits CWD `.mcp.json` — MUST ISOLATE config for `claude-code` or MUST use codex
- SET emergency preflight skip ONLY via `sqlite-graphrag config set spawn.skip_preflight=1` (EMERGENCIES ONLY); namespace via `--namespace` or XDG `config set` (default `global`)
- FORBIDDEN product env `SQLITE_GRAPHRAG_*` — NOT read on the hot path; ALWAYS use CLI flags and XDG `config set` only; key precedence REQUIRED — CLI flag `--openrouter-api-key` > XDG `config add-key` > none
- NEVER expose as MCP/HTTP; NEVER write `.sqlite` from another tool
- Host/XDG leaves accept `--db` as documented no-op — config×9, slots×3, cache×3, `codex-models`, `completions`; graph surfaces STILL REQUIRE and USE `--db`


## OpenRouter Embed Models
- PASS `--embedding-model <MODEL>` when `--embedding-backend openrouter`; NO default model → exit 78 on omission; prices indicative USD per million tokens; ALWAYS confirm live via `usage.cost` when available
- Catalog — `nvidia/llama-nemotron-embed-vl-1b-v2:free` FREE; `qwen/qwen3-embedding-4b` $0.05/M; `qwen/qwen3-embedding-8b` $0.05/M DEFAULT operational; `openai/text-embedding-3-small` $0.05/M; `perplexity/pplx-embed-v1-0.6b` $0.05/M; `baai/bge-m3` ~$0.05/M; `mistralai/mistral-embed-2312` $0.10/M; `google/gemini-embedding-2` ~$0.12/M; `openai/text-embedding-3-large` $0.13/M; `google/gemini-embedding-005` ~$0.15/M
- KNOW MRL truncates server-side to `--embedding-dim` (default **1024**); dim mismatch → exit 11
- KNOW openrouter propagates to ALL embed paths — `remember` `remember-batch` `ingest` `recall` `edit` `restore` `hybrid-search` `deep-research` `enrich` `init` `rename-entity`


## OpenRouter Key and Catalog Verify
- REQUIRED ADD key — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- LIST — `sqlite-graphrag config list-keys --json`; REMOVE — `sqlite-graphrag config remove-key <fingerprint> --json`; DOCTOR — `sqlite-graphrag config doctor --json`; PATH — `sqlite-graphrag config path`
- KNOW keys live in XDG `~/.config/sqlite-graphrag/config.toml` with `chmod 600`, zeroized on drop, NEVER logged
- VERIFY models via `config doctor` + stored key NEVER product env; invalid model → exit 78; VERIFY catalog live AFTER key storage via doctor then OpenRouter REST with the stored key; ALWAYS match embedding-table ids before paid calls
- NEVER pass API key as CLI argument in production shell history; ALWAYS prefer `config add-key --from-stdin`; ALWAYS run `config doctor` after adding a key before paid calls
- FORBIDDEN rely on `OPENROUTER_API_KEY` or any `SQLITE_GRAPHRAG_*` product env as primary config


## Headless LLM Backends
- ALWAYS pass the model flag explicitly; NEVER rely on silent defaults alone
- CODEX — `enrich --mode codex --codex-model <MODEL>`; OAuth-only; default `gpt-5.5`; `codex login`; embedding path `--llm-backend codex --llm-model <MODEL>`; NEVER pass `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`
- CLAUDE — `enrich --mode claude-code --claude-model <MODEL>`; OAuth-only; default `claude-sonnet-4-6`; embedding path `--llm-backend claude --llm-model <MODEL>`; NEVER pass `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`
- OPENCODE — `enrich --mode opencode --opencode-model <MODEL>`; default `opencode/big-pickle`; embedding path `--llm-backend opencode --llm-model <MODEL>`; own auth (NOT OAuth); catalog EXTERNAL/dynamic; `--opencode-model` UNVALIDATED — PASS live OpenCode Zen ids; CONSULT `opencode.ai/zen`
- OPENROUTER extraction — MUST use `enrich --mode openrouter --openrouter-model <id>`; `--openrouter-model` is MANDATORY (no default; missing value exits 1 before network); key from `config add-key` or `--openrouter-api-key`
- OVERRIDE binaries `--codex-binary`, `--claude-binary`, `--opencode-binary`; TUNE timeouts `--codex-timeout`, `--claude-timeout`, `--opencode-timeout`
- VALIDATE codex models with `--codex-model-validate` and `--codex-model-fallback <MODEL>`; LIST with `sqlite-graphrag codex-models --json` (CODEX only, NOT OpenRouter)
- SWAP backend on rate limit with `enrich --fallback-mode codex` or global `--llm-fallback codex,claude,none`
- KNOW `--mode openrouter` is pure REST `/chat/completions` — NO local CLI; bills stored OpenRouter key (read `usage.cost`); codex/claude-code/opencode are zero-token OAuth/own-auth paths


## OpenRouter Text Models
- PASS `--openrouter-model <MODEL>` on `--mode openrouter`; text models serve ONLY extraction/enrichment, NEVER embedding; prices indicative input/output USD per million tokens — ALWAYS confirm via `usage.cost`; `:nitro` = fastest provider at higher price
- MUST use `openai/gpt-oss-120b` 0.059/0.18 as DEFAULT judge when unspecified
- ALL ids REQUIRED — `deepseek/deepseek-v4-flash` 0.09/0.18; `deepseek/deepseek-v4-flash:nitro` 0.14/0.28; `deepseek/deepseek-v4-pro` 1.30/2.60; `google/gemini-3.1-flash-lite` 0.95/3.00; `minimax/minimax-m3` 0.30/1.20; `minimax/minimax-m2.7` 0.25/1.00; `minimax/minimax-m2.7:nitro` 0.30/1.20; `openai/gpt-oss-120b` 0.059/0.18 default judge; `openai/gpt-oss-120b:nitro` 0.15/0.60 max throughput; `xiaomi/mimo-v2.5` 0.10/0.28; `xiaomi/mimo-v2.5-pro` 0.43/0.87; `z-ai/glm-5.2` and `z-ai/glm-5.2:nitro` price varies — CONFIRM via `usage.cost`
- VERIFY strict `json_schema` BEFORE production; missing Structured Outputs → explicit OpenRouter error


## Global Flags and CLI Inventory
- `--db <PATH>` — PLACE AFTER the subcommand; persistent default via `config set db.path <PATH>` (NOT product env); graph commands REQUIRE real `--db`; host/XDG leaves accept `--db` as documented no-op
- `--namespace <ns>`; `--json` ALWAYS; `--lang en|pt`; `--tz <TIMEZONE>`
- `--embedding-backend auto|openrouter|llm`; `--embedding-model <MODEL>` MANDATORY with openrouter; `--embedding-dim N` default 1024 MRL [8, 4096]
- `--openrouter-api-key <KEY>` FORBIDDEN in production shell history; prefer `config add-key --from-stdin`
- `--llm-backend codex|claude|opencode|none|auto`; `--llm-model <MODEL>`; `--llm-fallback <chain>`
- `--extraction-backend` related global; `--openrouter-model <MODEL>` MANDATORY for `--mode openrouter`; `--openrouter-base-url`; `--openrouter-timeout` default 600
- `--llm-parallelism N` embed fan-out default 4 clamp [1, 32]; `--rest-concurrency N` openrouter enrich fan-out clamp [1, 16] default 8; DISTINCT flags
- `--max-concurrency N` clamp [1, 2×nCPUs]; `--llm-max-host-concurrency N`; `--llm-slot-wait-secs N` / `--llm-slot-no-wait`; `--wait-lock SECS`; `--low-memory`; `--strict-env-clear`; `--graceful-shutdown-secs N`; `--skip-embedding-on-failure`
- `--codex-binary`, `--claude-binary`, `--opencode-binary`; `-v`/`-vv`/`-vvv`; `--quiet`/`-q` MANDATORY in headless pipelines; NEVER mix stderr into JSON with `&>`
- TOP-LEVEL CLI inventory — `init` `remember` `remember-batch` `ingest` `recall` `read` `list` `forget` `purge` `rename` `split-body` `edit` `history` `restore` `hybrid-search` `health` `migrate` `namespace-detect` `optimize` `stats` `sync-safe-copy` `backup` `vacuum` `link` `unlink` `deep-research` `related` `graph` `export` `fts` `vec` `codex-models` `prune-relations` `prune-ner` `slots` `pending` `embedding` `pending-embeddings` `cleanup-orphans` `memory-entities` `cache` `delete-entity` `reclassify` `rename-entity` `merge-entities` `enrich` `reclassify-relation` `normalize-entities` `completions` `config` `help`


## CRUD Write
- INVOKE `remember --name <kebab> --type <kind> --description <text>` with exactly one body source — `--body` or `--body-file` or `--body-stdin` or `--graph-stdin`
- INVOKE `remember --graph-stdin` for `{body, entities, relationships}`; or `--graph-file` with `--body-file`
- PASS entities `[{name, entity_type}]` kebab-case ASCII; relationships `[{source, target, relation, strength}]` strength [0.0, 1.0]
- REQUIRED graph-stdin entity allowlist — ONLY keys `name`, `entity_type` (alias `type` folded), optional `description`; FORBIDDEN `observations`, `aliases`, free-form extras → exit 1; strip before send
- PASS `--strict-name`; `--force-merge` for idempotent updates; `--replace-graph` with `--force-merge`; `--dry-run` to validate without persisting
- PASS `--enqueue-enrich` on `remember` ONLY for hot-set entity-descriptions after write; default OFF
- PARSE remember JSON for `entities_created[]` and `enrich_recommended[]`; NEVER ignore; WHEN `enrich_recommended` non-empty MUST run SEPARATE `enrich --operation entity-descriptions` (or `--enqueue-enrich` for priority queue) AFTER write exit 0
- VALID memory `--type` — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- INVOKE `remember-batch` for 10+ memories via NDJSON stdin; PASS `--transaction`; every create line MUST include non-empty `description` and `type`
- INVOKE `ingest <DIR> --recursive --pattern "*.md" --mode none` for body-only import, then enrich SEPARATELY; `ingest --mode` accepts `none` (default), `claude-code`, `codex`, `opencode` (non-none runs inline LLM extraction, no separate enrich for those bindings)
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
- INVOKE `graph stats --json`; `graph traverse --from <root> --depth <N> --json`; EXPORT `--format json|dot|mermaid --output <path>`
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
- REQUIRED EntityType fold via `map_to_canonical`; `module` → `concept`; graph-stdin ACCEPTS folded types
- VALIDATE entity names — min 2 chars, no newlines, no short ALL_CAPS ≤4 chars, REJECT pure digits; NEVER use `mentions` as default relation; graph writes ADDITIVE with NO degree cap; NORMALIZE only via prune/merge/normalize


## GraphRAG Search
- USE three-layer pattern — `hybrid-search` then `read --name` then `related` or `graph traverse`
- INVOKE `recall <query> --k N` for pure semantic KNN; PASS `--no-graph`, `--precise`, `--max-distance <f>`, `--max-graph-results N`, `--all-namespaces`
- INVOKE `hybrid-search <query> --k N` for FTS5 plus KNN RRF; PASS `--rrf-k 60`; `--weight-vec 1.0 --weight-fts 1.0`; `--fallback-fts-only`; USE `--with-graph --max-hops 2 --min-weight 0.3`; READ BOTH `results[]` AND `graph_matches[]`
- INVOKE `related <name> --hops N --relation <type>`
- INVOKE `deep-research "<query>" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies`; single-token queries fan out; manual control PASS `--sub-query-strategy manual --sub-queries-file PATH`
- WRITE large envelopes with `--output PATH` or `-o PATH` (atomwrite); PARSE ack `{written, bytes, blake3}`; PASS `--quiet`; NEVER `&>`; when `-o`/`--output` set the file MUST exist non-zero after exit 0
- DEEP-RESEARCH formula — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 deep-research "question" --k 20 --max-hops 3 -o /tmp/dr.json --json`
- TUNE with `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--max-cost-usd`, `--timeout`
- PARSE `recall` → `results[].{name, snippet, distance, score, source}`; `hybrid-search` → `results[].{name, combined_score, vec_rank, fts_rank}`; `deep-research` → `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context`, `stats`
- NEVER confuse `distance` with `combined_score`; NEVER raise hops without inspecting `graph stats` first


## Enrich and Entity-Connect
- INVOKE `enrich --operation <op> --mode <backend>` — BOTH MANDATORY for LLM ops; omitting `--mode` → exit 2; EXCEPT read-only inspectors `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` and `--dry-run` (mode optional)
- PERSIST ops — `memory-bindings`, `augment-bindings` (REQUIRES `--names`/`--memory-names`/`--names-file`), `entity-descriptions`, `body-enrich`, `re-embed`, `weight-calibrate`, `relation-reclassify`, `entity-connect`, `entity-type-validate`, `description-enrich`, `cross-domain-bridges`, `domain-classify`, `deep-research-synth` (when bindings persist), `body-extract` + `--body-extract-graph-only`; SCAN/REPORT only — `graph-audit`
- Valid `--mode` — `codex`, `claude-code`, `opencode`, `openrouter`; PASS matching model flag; `--mode openrouter` requires `--openrouter-model`, key from XDG or `--openrouter-api-key`, REST `/chat/completions` with strict json_schema and `provider.require_parameters` true, billed via `usage.cost`
- PASS `--limit N --resume` for `re-embed`; `--retry-failed`; `--dry-run`; `--target memories|entities|chunks|all` on `re-embed` only (default `memories`); `re-embed` selects MISSING/EMPTY/dim-DIVERGENT vectors; PASS `--min-output-chars N` for `body-enrich`; `--fallback-mode codex` on Claude rate limits
- REQUIRED name filters — prefer `--entity-names a,b` for entity-keyed ops and `--memory-names a,b` for memory-keyed ops; `--names` is BC alias; empty match MUST surface `matched=0` + `hint` then STOP
- REQUIRED low-quality redescribe — PASS `--force-redescribe` on `entity-descriptions`; default write-once for non-empty descriptions
- ENTITY-DESCRIPTIONS formulas — openrouter `sqlite-graphrag enrich --operation entity-descriptions --mode openrouter --openrouter-model openai/gpt-oss-120b --force-redescribe --entity-names jwt,auth-svc --json`; codex `--mode codex --codex-model gpt-5.5`; claude `--mode claude-code --claude-model claude-sonnet-4-6`; opencode `--mode opencode --opencode-model opencode/big-pickle` (same op/flags otherwise)
- PASS `--quality-sample N` with `--status` for `quality_pct` and `scan_backlog_low_grounding_est` (flag > XDG `enrich.entity_description.quality_sample` > default 50; `0` disables)
- KNOW queue isolation — drain claims only selected `operation` rows; memory-only ops MUST NOT claim `pair:`/`entity:`/`chunk:` keys; status `state` = `draining`|`cooldown`|`pending-scan`|`blocked_dead`; `blocked_dead` → `--list-dead`/`--requeue-dead`/prune FIRST
- NEVER run multiple `enrich` processes on the same DB; REST parallelism is ONLY `--rest-concurrency` inside ONE process
- PASS `--until-empty` to loop scan→drain until empty or `--max-runtime` (default 3600); PASS `--max-attempts <N>` default 8 range 1..=20
- PASS `--status` for `scan_backlog`, `unbound_backlog`, queue counts, `eligible_now`, `waiting`, `quality_pct`, `state` — NO LLM, NO singleton
- DISTINGUISH — `scan_backlog` = DB candidates a fresh scan WOULD enqueue; `queue_pending` = sidecar count; `eligible_now == 0` with `queue_pending > 0` is COOLDOWN; stuck `draining` → `--reset-stale-claims`
- Ops list compressed — PASS `--list-dead`; `--requeue-dead`; `--list-skipped`; `--requeue-skipped` (recover skipped/`preservation_failed` without raw SQL); `--ignore-backoff`; `--prune-dead-orphans`; `--prune-dead-entity-orphans` (mutually exclusive); `--reset-stale-claims` after `kill -9`
- KNOW dead-letter Transient vs HardFailures; truncated OpenRouter completions (`finish_reason`=`length`) re-emit with GROWN `max_tokens` before JSON repair; queue is sidecar `.enrich-queue.sqlite`
- STATUS — `sqlite-graphrag enrich --status --quality-sample 50 --json`
- UNTIL-EMPTY — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --max-attempts 8 --rest-concurrency 8 --json`
- MEMORY-BINDINGS by names — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --memory-names mem-a,mem-b --json`
- FULL BACKFILL re-embed — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 enrich --operation re-embed --target all --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --json` then `health --json`
- ENTITY-CONNECT PERSISTS edges via `entity_connect_seen` with `related`|`none`; `cross-domain-bridges` uses SAME scan/drain; pair scan is O(k) co-occurrence + hub×degree-0 fill — NEVER full Cartesian; queue keys `pair:{id1}:{id2}` `item_type=entity_pair`
- First scan covered by `--max-runtime` and soft ~120s `InterruptHandle`; interrupt → Timeout exit **1** — NEVER exit 75
- PARSE `budget_exhausted` (runtime budget ends on large namespaces) and `preempted_for_gate` (EC yielded so memory-bindings/entity-descriptions run first)
- PASS `--anchor-memory <name>` and/or `--entity-names a,b`; empty match → `matched=0` + `hint`; ALWAYS `--until-empty` + inspect `--status`; ALWAYS dry-run first on production corpora
- EC dry-run — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --dry-run --limit 50 --json`
- EC until-empty openrouter — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 600 --rest-concurrency 8 --json`
- EC codex — `sqlite-graphrag enrich --operation entity-connect --mode codex --codex-model gpt-5.5 --until-empty --max-runtime 600 --json`
- EC claude — `sqlite-graphrag enrich --operation entity-connect --mode claude-code --claude-model claude-sonnet-4-6 --until-empty --max-runtime 600 --json`
- EC opencode — `sqlite-graphrag enrich --operation entity-connect --mode opencode --opencode-model opencode/big-pickle --until-empty --max-runtime 600 --json`
- EC anchored — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --anchor-memory <mem> --until-empty --max-runtime 600 --json`
- EC bridges — same formulas with `--operation cross-domain-bridges`
- Priority — memory-bindings then entity-descriptions BEFORE entity-connect; long EC drains MUST yield; legacy non-`pair:` queue rows ignored


## Write Then Enrich Templates
- TREAT every write as STEP 1 then DISTINCT STEP 2; NEVER chain with `&&`
- DEFINE PREFIX — `sqlite-graphrag --embedding-backend openrouter --embedding-model <EMB> --embedding-dim 1024 --llm-backend none`
- DEFAULT `<EMB>` = `qwen/qwen3-embedding-8b`; FREE path `nvidia/llama-nemotron-embed-vl-1b-v2:free`
- STEP 1 (ALWAYS exit 0 before STEP 2); ALWAYS parse `entities_created` and `enrich_recommended` on remember
- REMEMBER — `echo '{"body":"text","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | <PREFIX> remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --json`
- REMEMBER hot-set — same plus `--enqueue-enrich` for priority entity-descriptions
- REMEMBER-BATCH — `<PREFIX> remember-batch --transaction --json` with NDJSON stdin; PASS `--enqueue-enrich` after successful batch when hot-set
- INGEST — `<PREFIX> ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- EDIT — `<PREFIX> edit --name <n> --body-file new.md --json`
- RESTORE — `<PREFIX> restore --name <n> --version 2 --json`
- STEP 2 templates (ONE backend per run; ALWAYS set model flags); APPLY after remember, remember-batch, ingest, edit, restore
- CODEX — `sqlite-graphrag enrich --operation memory-bindings --mode codex --codex-model gpt-5.5 --json`
- CLAUDE — `sqlite-graphrag enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- OPENCODE — `sqlite-graphrag enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- OPENROUTER text — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- REQUIRED full matrix — EACH write path (remember, remember-batch, ingest, edit, restore) MUST run STEP 1 OpenRouter embed then ONE of the four STEP 2 backends; NEVER skip model flags
- REMEMBER→CODEX full — STEP1 REMEMBER then memory-bindings codex then if `enrich_recommended` has entity-descriptions run `sqlite-graphrag enrich --operation entity-descriptions --mode codex --codex-model gpt-5.5 --entity-names <list> --json`
- REMEMBER→CLAUDE full — STEP1 then memory-bindings + entity-descriptions with `--mode claude-code --claude-model claude-sonnet-4-6`
- REMEMBER→OPENCODE full — STEP1 then memory-bindings + entity-descriptions with `--mode opencode --opencode-model opencode/big-pickle`
- REMEMBER→OPENROUTER full — STEP1 then memory-bindings + entity-descriptions with `--mode openrouter --openrouter-model openai/gpt-oss-120b`; PASS `--force-redescribe` when quality is low
- REMEMBER-BATCH / INGEST / EDIT / RESTORE → same four STEP 2 backends; ONLY STEP 1 write command changes
- KNOW extraction STEP 2 does NOT require `--llm-backend` on enrich; pass embedding flags only for `re-embed` or host defaults; key resolution flag > XDG; FORBIDDEN product env as primary


## Parallel Embed and Enrich
- SCALE embed with `--llm-parallelism N` on STEP 1 (JoinSet of N OpenRouter requests, order preserved); SCALE openrouter enrich with `--rest-concurrency N` + `--until-empty` on STEP 2 (N chat calls; SQLite write stays serial via WAL claim)
- CLAMP `--llm-parallelism` 1..32 and `--rest-concurrency` 1..16; paid models MUST use 4..16; `:free` caps ~20 req/min so MUST use low N; multiple keys do NOT add capacity
- NEVER launch N enrich processes; ONE process with `--rest-concurrency` is MANDATORY; headless codex/claude/opencode do NOT use `--rest-concurrency` the same way; NEVER spawn multiple enrich processes to compensate
- REMEMBER parallel STEP 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 --llm-parallelism 8 --llm-backend none remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --enqueue-enrich --json`
- Parallel STEP 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --rest-concurrency 8 --until-empty --max-runtime 3600 --max-attempts 8 --json`
- Parallel STEP 2 codex — `sqlite-graphrag enrich --operation memory-bindings --mode codex --codex-model gpt-5.5 --until-empty --max-runtime 3600 --json`
- Parallel STEP 2 claude — `sqlite-graphrag enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --until-empty --max-runtime 3600 --json`
- Parallel STEP 2 opencode — `sqlite-graphrag enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --until-empty --max-runtime 3600 --json`
- REMEMBER-BATCH parallel STEP 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 --llm-parallelism 12 --llm-backend none remember-batch --transaction --json`
- REMEMBER-BATCH parallel STEP 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --rest-concurrency 12 --until-empty --max-runtime 3600 --json`
- INGEST parallel STEP 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 1024 --llm-parallelism 6 --llm-backend none ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- INGEST parallel STEP 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b:nitro --rest-concurrency 12 --until-empty --max-runtime 7200 --max-attempts 8 --json`
- EDIT parallel STEP 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 --llm-parallelism 8 --llm-backend none edit --name <n> --body-file new.md --json`
- EDIT parallel STEP 2 — same openrouter parallel STEP 2 as remember
- RESTORE parallel STEP 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 --llm-parallelism 8 --llm-backend none restore --name <n> --version 2 --json`
- RESTORE parallel STEP 2 — same openrouter parallel STEP 2 as remember
- MONITOR with `enrich --status --json` until `scan_backlog` `queue_pending` `eligible_now` are all 0; `queue_dead` is permanent data debt until requeue or prune


## Read Formulas
- INIT — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 1024 init --namespace <ns>`
- RECALL — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 recall "query" --k 10 --json`
- HYBRID-SEARCH — `sqlite-graphrag --embedding-backend openrouter --embedding-model baai/bge-m3 --embedding-dim 1024 hybrid-search "query" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- DEEP-RESEARCH — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model openai/text-embedding-3-small --embedding-dim 1024 deep-research "question" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies -o /tmp/research.json --json`
- MEMORY-ENTITIES — `sqlite-graphrag memory-entities --name <memory> --json` then parse `entities[].description`
- RENAME-ENTITY — `sqlite-graphrag --embedding-backend openrouter --embedding-model perplexity/pplx-embed-v1-0.6b --embedding-dim 1024 rename-entity --name <old> --new-name <new> --json`
- HYBRID-SEARCH offline — `sqlite-graphrag hybrid-search "query" --k 10 --fallback-fts-only --json`
- TRAVERSE fuzzy — `sqlite-graphrag graph traverse --from <short-alias> --depth 2 --fuzzy --json`
- LINK by ID — `sqlite-graphrag link --from-id <N> --to-id <M> --relation uses --json`
- MERGE self-ref guard — NEVER run `merge-entities --ids 3,12 --into-id 3`; ALWAYS exclude survivor from `--ids`
- VERIFY OpenRouter catalog only with key from `config add-key` / doctor — NEVER product env


## Diagnostics Maintenance Exit Codes Concurrency XDG
- INIT — `sqlite-graphrag init --namespace <ns>`; HEALTH — `sqlite-graphrag health --json` for `{integrity_ok, schema_version, vec_*_missing, vec_*_coverage_pct}`; TRIGGER re-embed when missing > 0
- MIGRATE — `migrate --dry-run --json` then `migrate --json`; OPTIMIZE — `optimize --json`; VACUUM — `vacuum --json` after purge
- FTS — `fts check|stats|rebuild --json` when `health.fts_degraded`; VEC — `vec orphan-list --json` then `vec purge-orphan --yes`; `vec stats --json`
- EMBEDDING — `embedding --status --json`; alias `pending-embeddings --status --json`; re-process via `enrich --operation re-embed`
- SLOTS — `slots status --json`; `slots release --slot-id <N> --yes`; PENDING — `pending list --filter-status queued --json`; `pending show <id>`; `pending cleanup --yes`
- EXPORT — `export --namespace <ns> --type <kind> --json`; STATS — `stats --json`; BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECT — `namespace-detect --json`, `cache list --json`, `cache stats --json` (alias of `list`), `cache clear-models --yes`; COMPLETIONS — `completions bash|zsh|fish|elvish|powershell`
- SCHEDULE weekly — `purge --yes` (90d) or `purge --yes --now` → `cleanup-orphans` → `prune-relations --relation mentions` → `vacuum` → `optimize` → `sync-safe-copy`; IF corruption — `sqlite3 broken.sqlite ".recover" | sqlite3 repaired.sqlite`
- EXIT codes — 0 success; 1 validation OR Timeout (EC InterruptHandle — NOT 75); 2 args; 3 optimistic lock; 4 not found (suggestions without `--fuzzy`); 5 namespace; 6 payload too large (SPLIT body); 9 duplicate (`--force-merge`); 10 database (`vacuum`+`health`); 11 embedding (backend/dim/key); 13 partial batch (reprocess failed only); 14 I/O; 15 busy (widen `--wait-lock`); 16 preflight (fix MCP; NEVER transient); 19 SHUTDOWN (retry MANDATORY); 20 internal; 75 singleton locked (NEVER retry immediately); 77 RAM; 78 config (key/model missing)
- NEVER ignore non-zero; NEVER reprocess full batch after exit 13; NEVER confuse exit 1 Timeout with exit 75 or exit 9
- CONCURRENCY — hard ceiling `2 x nCPUs` for `init`/`remember`/`ingest`/`recall`/`hybrid-search`; `--llm-parallelism` default 4 on remember/edit, 2 on ingest, clamp [1, 32]; JOB SINGLETON on `enrich` and `ingest --mode codex|claude-code`; USE `--wait-job-singleton SECS` or `--force-job-singleton`; unitary via `--low-memory` or XDG (FORBIDDEN product env); NEVER parallel enrich on same DB; REST concurrency via `--rest-concurrency` only
- XDG precedence — CLI flag > XDG `config set` > default; FORBIDDEN `SQLITE_GRAPHRAG_*` hot path; FORBIDDEN product telemetry
- REQUIRED store keys — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- REQUIRED config ops — `config set <key> <value>`; `config get <key>`; `config list --json`; `config list --effective --json`; `config unset <key>`; `config path`; `config doctor --json`
- REQUIRED network URLs — `sqlite-graphrag config set network.openrouter.chat_url https://openrouter.ai/api/v1/chat/completions` and `sqlite-graphrag config set network.openrouter.embeddings_url https://openrouter.ai/api/v1/embeddings`; aliases `network.chat_url`, `network.embed_url`
- REQUIRED keys — `llm.query_embed_timeout_secs` default **3s**; `enrich.entity_description.quality_sample` default 50; common — `db.path`, `embedding.dim` (1024), `embedding.backend`, `embedding.model`, `llm.backend`, `llm.model`, `display.tz`, `i18n.lang`, `log.level`, `log.format`, `spawn.skip_preflight` (emergencies only), `enrich.yield_every_n_items`
- ALWAYS prefer one-shot flags for agents; XDG only for host defaults; NEVER rely on `ANTHROPIC_API_KEY`/`OPENAI_API_KEY` for codex/claude (FORBIDDEN, exit 1)


## Active Rules
- ALWAYS `--json`; ALWAYS `jaq` after capture (NEVER pipe NDJSON; NEVER `jq`); ALWAYS parse `backend_invoked`
- ALWAYS OpenRouter embed flags + dim 1024 on embed ops; ALWAYS `--llm-backend none` on OpenRouter writes; ALWAYS SEPARATE `enrich` with `--mode`+model; NEVER `&&`
- ALWAYS parse `entities_created`/`enrich_recommended`; ALWAYS run entity-descriptions when recommended or `--enqueue-enrich` used
- ALWAYS prefer `--entity-names`/`--memory-names`; handle `matched=0`+`hint`; use `--force-redescribe` for low-quality; inspect `quality_pct`
- ALWAYS treat `blocked_dead` as hard debt; parse EC `budget_exhausted`/`preempted_for_gate`; parse `memory-entities` `description`; use `-o`/`--output`+`--quiet` for large deep-research
- ALWAYS refresh OAuth when stale; keep dim 1024; shell arrays for dynamic merges; `--from-id`/`--to-id` for numeric links; retry `--fuzzy` on short-name traverse
- NEVER API keys to codex/claude; NEVER multiple enrich processes; NEVER ignore exit 19/16; NEVER openrouter without model+key (exit 78); NEVER self-ref merge; NEVER MCP memory/MEMORY.md; NEVER document `SQLITE_GRAPHRAG_*` as config
- CANONICAL memory types — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- CANONICAL entity types — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- CANONICAL relations — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
