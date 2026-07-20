---
name: sqlite-graphrag
description: This skill MUST activate for every sqlite-graphrag CLI operation covering GraphRAG memory, hybrid-search, recall, deep-research -o, remember enqueue-enrich entities_created enrich_recommended, remember-batch, ingest, edit, restore, enrich force-redescribe entity-names memory-names quality_pct blocked_dead budget_exhausted preempted_for_gate, entity-connect scale, memory-entities description, forget, purge, link, unlink, merge-entities, rename-entity, reclassify, graph traverse, OpenRouter embed and text models, XDG keys, headless codex claude opencode, write-then-enrich formulas, parallel embedding, exit codes, concurrency, FTS5 BLOB fusion, canonical types relations, namespace isolation, OAuth-only. This skill MUST be used whenever the agent stores, retrieves, searches, enriches, links, merges, or maintains long-term GraphRAG memory. Keywords sqlite-graphrag GraphRAG memory embedding openrouter codex claude opencode remember recall hybrid-search ingest enrich entity-connect deep-research force-redescribe enqueue-enrich config XDG
---

## When This Skill Activates
- MUST ACTIVATE when the user asks to remember, save, recall, retrieve, search, or persist anything across sessions
- MUST ACTIVATE for long-term context, knowledge graph, GraphRAG, RAG, entity linking, memory management, and namespace-scoped knowledge
- MUST ACTIVATE when sqlite-graphrag, embedding, FTS5, hybrid-search, OpenRouter, codex, claude, opencode, entity-connect, or LLM memory is mentioned
- MUST ACTIVATE for enrich, re-embed, link, unlink, merge-entities, rename-entity, deep-research, ingest, config API keys, and graph maintenance
- NEVER ACTIVATE for one-off ephemeral data, simple file I/O, or tasks unrelated to persistent context
- ALWAYS load this skill before inventing ad-hoc memory files, MCP memory servers, or hand-written Markdown journals


## Core Mental Model
- KNOW THREE independent selectors; NEVER conflate them
- SELECTOR 1 — `--embedding-backend` HOW vectors are produced — `openrouter` (REST), `llm` (subprocess), or `auto`
- SELECTOR 2 — `--llm-backend` WHICH subprocess embeds when backend is `llm` — `codex`, `claude`, `opencode`, or `none`
- SELECTOR 3 — extraction via `enrich --mode` — `codex`, `claude-code`, `opencode`, or `openrouter` (REST `/chat/completions`); `--extraction-backend` is the related global selector
- WRITE and ENRICH are ALWAYS separate; write produces embeddings; SEPARATE `enrich` extracts or mutates the graph
- NEVER chain write and enrich with `&&`; ALWAYS wait for write exit 0, then run enrich as a DISTINCT process
- On EVERY write (`remember`, `remember-batch`, `ingest`, `edit`, `restore`) MUST pass `--llm-backend none` with `--embedding-backend openrouter` so embeddings STILL run via OpenRouter REST without an LLM subprocess timeout
- ALWAYS pass `--json`; ALWAYS parse with `jaq` NEVER `jq`; ALWAYS capture stdout FIRST then parse; NEVER pipe CLI output directly into `jaq` (NDJSON masks failures as null)
- KNOW empty vectors are NEVER persisted; PARSE `backend_invoked` to confirm the backend; RUN `enrich` only after write exit 0
- ALWAYS keep `--embedding-dim 384` identical on ALL write and read embed paths; mismatch collides with the stored index and fails knn with exit 11


## LLM Prompt Instruction Rules
- "remember this" → `remember --force-merge` with `--graph-stdin` curated entities and canonical relations, then SEPARATE `enrich`
- "what do you know about X" → `hybrid-search "X" --k 10 --json` FIRST, then `read --name <name> --json`
- "how is X related to Y" → `graph traverse --from X --depth 2 --json` or `related X --hops 2 --json`; on miss MUST RETRY with `--fuzzy` or pick exit 4 NotFound suggestions
- "deep research on X" → `deep-research "X" --k 20 --max-hops 3 --json`; single-token subjects fan out to aspect sub-queries; large envelopes MUST use `--output PATH` and `--quiet`
- "connect isolated entities" → `enrich --operation entity-connect` with mandatory `--mode` + model, then monitor `--status` until backlog drains
- BEFORE any create → `hybrid-search "<name>" --k 5 --json`; if duplicate MUST USE `--force-merge`
- AFTER create/update → capture-parse `read --name <name> --json` for `{name, description, body_length}`
- AFTER every turn → persist new findings or DECLARE "No new findings to persist"
- On non-zero exit → parse `jaq '{code, message, error_class}'` and REPORT remediation
- ALWAYS use canonical relations — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- ALWAYS map non-canonical first — `adds`/`creates`→`causes`, `implements`→`supports`, `blocks`→`contradicts`, `tested-by`→`related`, `part-of`→`applies-to`
- ALWAYS kebab-case ASCII lowercase entity names; LIMIT entities to domain concepts; REJECT generics, pronouns, UUIDs, timestamps
- NEVER use MCP Serena, `.md` memory files, or MEMORY.md; NEVER start a daemon; NEVER pass `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` to subprocess backends
- MUST use `remember --force-merge` for idempotent updates; MUST use `--graph-stdin` or `--graph-file` when a curated graph is available


## Architecture and Principles
- INVOKE as subprocess; stdout = JSON/NDJSON; stderr = logs; CHECK exit code BEFORE parsing
- KNOW NO daemon, NO ONNX, NO model cache; cosine is pure Rust over BLOB `memory_embeddings` / `entity_embeddings` / `chunk_embeddings`
- KNOW `init` or `migrate` applies live schema; READ `schema_version` from `health --json`
- ENFORCE OAUTH-ONLY for codex/claude — spawn ABORTS exit 1 if `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` is set; `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` are PRESERVED
- KNOW subprocess CWD is ISOLATED; 7 preflight guards run before every LLM fork; exit 16 = preflight failure; `claude -p` inherits CWD `.mcp.json` — MUST ISOLATE config for `claude-code` or MUST use codex instead
- SET emergency preflight skip ONLY via `sqlite-graphrag config set spawn.skip_preflight=1` (EMERGENCIES ONLY); namespace via `--namespace` or XDG `config set` (default `global`)
- FORBIDDEN product env `SQLITE_GRAPHRAG_*` — NOT read on the hot path; ALWAYS use CLI flags and XDG `config set`
- NEVER expose as MCP/HTTP; NEVER write `.sqlite` from another tool; FUSION is FTS5 BM25 plus BLOB cosine KNN via RRF


## OpenRouter Embedding Models and Prices
- PASS `--embedding-model <MODEL>` when `--embedding-backend openrouter`; there is NO default model, so omission triggers exit 78
- KNOW prices below are indicative USD per one million tokens; ALWAYS confirm live cost via provider pricing and per-call `usage.cost` when available
- USE `nvidia/llama-nemotron-embed-vl-1b-v2:free` for FREE zero-cost embedding
- USE `qwen/qwen3-embedding-4b` at 0.05 USD per million tokens
- USE `qwen/qwen3-embedding-8b` at 0.05 USD per million tokens — DEFAULT operational embedding model when the user does not specify one
- USE `baai/bge-m3` at about 0.05 USD per million tokens
- USE `openai/text-embedding-3-small` at 0.05 USD per million tokens
- USE `perplexity/pplx-embed-v1-0.6b` at 0.05 USD per million tokens
- USE `mistralai/mistral-embed-2312` at 0.10 USD per million tokens
- USE `google/gemini-embedding-2` at about 0.12 USD per million tokens
- USE `openai/text-embedding-3-large` at 0.13 USD per million tokens
- USE `google/gemini-embedding-005` at about 0.15 USD per million tokens
- KNOW MRL truncates server-side to `--embedding-dim`; higher native dims stay cheap when truncated to 384
- VERIFY live models only after a key is stored — use the key from `config add-key`, never product env; invalid model → exit 78
- CONFIRM keys with `sqlite-graphrag config doctor --json`
- VERIFY OpenRouter catalog live AFTER key storage — `sqlite-graphrag config doctor --json` then list models via OpenRouter REST with the stored key (NEVER product env); ALWAYS match ids from the embedding table above before paid calls
- KNOW openrouter propagates to ALL embed paths — `remember` `remember-batch` `ingest` `recall` `edit` `restore` `hybrid-search` `deep-research` `enrich` `init` `rename-entity`


## OpenRouter API Key Management
- REQUIRED — ADD a key via stdin — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- LIST stored keys — `sqlite-graphrag config list-keys --json`
- REMOVE a key by fingerprint — `sqlite-graphrag config remove-key <fingerprint> --json`
- RUN the diagnostic doctor — `sqlite-graphrag config doctor --json`
- INSPECT the config path — `sqlite-graphrag config path`
- KNOW keys live in XDG config `~/.config/sqlite-graphrag/config.toml` with `chmod 600` and are zeroized on drop, NEVER logged
- REQUIRED key precedence — CLI flag `--openrouter-api-key` > XDG `config add-key` store > none; product env is NOT primary and NOT read on the hot path
- FORBIDDEN: rely on `OPENROUTER_API_KEY` or any `SQLITE_GRAPHRAG_*` product env as the primary config mechanism
- NEVER pass the API key as a CLI argument in production shell history; ALWAYS prefer `config add-key --from-stdin`
- ALWAYS run `config doctor` after adding a key before any paid embedding or enrich call


## Headless LLM Backends
- ALWAYS pass the model flag explicitly on every headless invocation; NEVER rely on silent defaults alone
- CODEX DEFAULT model is `gpt-5.5`; MUST set embedding path with `--llm-backend codex --llm-model <MODEL>` and extraction with `enrich --mode codex --codex-model <MODEL>`; refresh OAuth with `codex login`; codex is OAUTH-ONLY — NEVER pass `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`
- CLAUDE DEFAULT model is `claude-sonnet-4-6`; MUST set embedding path with `--llm-backend claude --llm-model <MODEL>` and extraction with `enrich --mode claude-code --claude-model <MODEL>`; claude is OAUTH-ONLY — NEVER pass `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`
- OPENCODE DEFAULT model is `opencode/big-pickle`; MUST set embedding path with `--llm-backend opencode --llm-model <MODEL>` and extraction with `enrich --mode opencode --opencode-model <MODEL>` via its own auth (NOT OAuth)
- OPENROUTER extraction — MUST use `enrich --mode openrouter --openrouter-model <id>`; `--openrouter-model` is MANDATORY (no default; missing value exits 1 before any network call); key comes from `config add-key` or `--openrouter-api-key`
- KNOW opencode catalog is EXTERNAL/dynamic; `--opencode-model` is UNVALIDATED; PASS live OpenCode Zen ids; CONSULT `opencode.ai/zen`
- OVERRIDE binaries with `--codex-binary`, `--claude-binary`, `--opencode-binary`; TUNE timeouts with `--codex-timeout`, `--claude-timeout`, `--opencode-timeout`
- VALIDATE codex models with `--codex-model-validate` and `--codex-model-fallback <MODEL>`; LIST with `sqlite-graphrag codex-models --json` (CODEX models only, NOT OpenRouter)
- SWAP backend on rate limit with `enrich --fallback-mode codex` or global `--llm-fallback codex,claude,none`
- KNOW `--mode openrouter` is pure REST `/chat/completions` — NO local CLI required; bills the stored OpenRouter key (read `usage.cost`); codex/claude-code/opencode are zero-token OAuth/own-auth paths


## OpenRouter Text Models for Enrich
- PASS `--openrouter-model <MODEL>` from this table on `--mode openrouter`; prices are indicative input/output USD per one million tokens — ALWAYS confirm live via `usage.cost`
- KNOW these models serve ONLY entity extraction and enrichment, NEVER embedding; the embedding table is separate
- MUST use `openai/gpt-oss-120b` at 0.059/0.18 USD, 131k context, 36 tps as DEFAULT judge when the user does not specify a text model
- USE `openai/gpt-oss-120b:nitro` at 0.15/0.60 USD, 131k context, 300 tps for maximum throughput
- USE `deepseek/deepseek-v4-flash` at 0.09/0.18 USD, 1M context, 20 tps
- USE `deepseek/deepseek-v4-flash:nitro` at 0.14/0.28 USD, 1M context, 109 tps
- USE `deepseek/deepseek-v4-pro` at 1.30/2.60 USD, 1M context, 26 tps
- USE `google/gemini-3.1-flash-lite` at 0.95/3.00 USD, 1M context, 100 tps
- USE `minimax/minimax-m3` at 0.30/1.20 USD, 1M context, 42 tps
- USE `minimax/minimax-m2.7` at 0.25/1.00 USD, 205k context, 43 tps
- USE `minimax/minimax-m2.7:nitro` at 0.30/1.20 USD, 205k context, 146 tps
- USE `xiaomi/mimo-v2.5` at 0.10/0.28 USD, 1M context, 17 tps
- USE `xiaomi/mimo-v2.5-pro` at 0.43/0.87 USD, 1M context, 29 tps
- USE `z-ai/glm-5.2` and `z-ai/glm-5.2:nitro` whose price varies by provider; ALWAYS CONFIRM real cost via `usage.cost`
- KNOW `:nitro` variants route to the fastest provider at a higher price
- VERIFY a model honours strict `json_schema` BEFORE production; a model without Structured Outputs support fails with an explicit OpenRouter error


## Global Flags Reference
- `--db <PATH>` — override database; PLACE AFTER the subcommand; persistent default via `config set db.default_path <PATH>` (NOT product env)
- `--namespace <ns>` — scope operations; `--json` — structured JSON (ALWAYS pass); `--lang en|pt`; `--tz <TIMEZONE>`
- `--embedding-backend auto|openrouter|llm`; `--embedding-model <MODEL>` (MANDATORY with openrouter); `--embedding-dim N` default 384 MRL range [8, 4096]
- `--openrouter-api-key <KEY>` — FORBIDDEN in production shell history; prefer `config add-key --from-stdin`
- `--llm-backend codex|claude|opencode|none|auto`; `--llm-model <MODEL>`; `--llm-fallback <chain>`
- `--extraction-backend` related global; `--openrouter-model <MODEL>` MANDATORY for `--mode openrouter`; `--openrouter-base-url`; `--openrouter-timeout` default 600
- `--llm-parallelism N` — embedding fan-out default 4 clamp [1, 32]; governs subprocess AND OpenRouter REST JoinSet concurrency
- `--rest-concurrency N` — openrouter enrich fan-out clamp [1, 16] default 8; DISTINCT from `--llm-parallelism`
- `--max-concurrency N` clamp [1, 2×nCPUs]; `--llm-max-host-concurrency N`; `--llm-slot-wait-secs N` / `--llm-slot-no-wait`
- `--wait-lock SECS`; `--low-memory`; `--strict-env-clear`; `--graceful-shutdown-secs N`; `--skip-embedding-on-failure`
- `--codex-binary`, `--claude-binary`, `--opencode-binary`; `-v`/`-vv`/`-vvv`; `--quiet`/`-q` MANDATORY in headless pipelines; NEVER mix stderr into JSON with `&>`


## CRUD Write Operations
- INVOKE `remember --name <kebab> --type <kind> --description <text>` with exactly one body source — `--body` or `--body-file` or `--body-stdin` or `--graph-stdin`
- INVOKE `remember --graph-stdin` for `{body, entities, relationships}` in one JSON document; or `--graph-file` combined with `--body-file`
- PASS entities as `[{name, entity_type}]` kebab-case ASCII; PASS relationships as `[{source, target, relation, strength}]` strength in [0.0, 1.0]
- PASS `--strict-name` to REJECT non-kebab names; PASS `--force-merge` for idempotent updates; PASS `--replace-graph` with `--force-merge` for full graph replace
- PASS `--dry-run` to validate without persisting
- PASS `--enqueue-enrich` on `remember` ONLY when the operator wants hot-set entity-descriptions enqueued AFTER write; default is OFF
- PARSE remember JSON for `entities_created[]` and `enrich_recommended[]` (typically `entity-descriptions` when entities were created); NEVER ignore these fields
- WHEN `enrich_recommended` is non-empty MUST run SEPARATE `enrich --operation entity-descriptions` (or enable `--enqueue-enrich` for priority queue) AFTER write exit 0
- VALID memory `--type` values — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- INVOKE `remember-batch` for 10+ memories via NDJSON stdin; PASS `--transaction` for all-or-nothing
- REQUIRED on create — every NDJSON line MUST include non-empty `description` (and `type`); empty description on create is REJECTED (parity with `remember`)
- INVOKE `ingest <DIR> --recursive --pattern "*.md" --mode none` for body-only import, then enrich SEPARATELY
- KNOW `ingest --mode` accepts `none` (default body-only), `claude-code`, `codex`, `opencode`; non-none runs inline LLM extraction DURING ingest and needs NO separate enrich for those bindings
- USE `--resume`; `--retry-failed`; `--auto-describe`; `--name-prefix <prefix>`; `--force-merge` on ingest to UPDATE duplicates (dedup by `body_hash`)
- KNOW `ingest` auto-splits oversized bodies into chunks
- INVOKE `split-body --name <N>` for ONE memory over 25000 chars; PASS `--batch --threshold 25000` for all oversize; DAUGHTERS ARE NOT EMBEDDED INLINE — step1 openrouter embed + `--llm-backend none split-body`; step2 SEPARATE `enrich --operation re-embed --target memories`
- RESPECT 512000 bytes and 512 chunks per body; NEVER mix body sources; NEVER `fd | xargs remember` — USE `ingest`
- NEVER pass non-`none` `--llm-backend` on write when embedding via OpenRouter; ALWAYS pass `--llm-backend none`


## CRUD Read Update Delete
- INVOKE `read --name <kebab> --json`; PASS `--with-graph`; USE `--format raw` for pure body text
- INVOKE `list --type <kind> --limit N --offset N --json`; `history --name <n> --diff --json`
- INVOKE `edit --name <n> --body-file <path>` or `--description` / `--memory-type`; USE `--force-reembed`; USE `--expected-updated-at <ts>` (exit 3 = conflict — reload and retry)
- INVOKE `rename --name <old> --new-name <new>`; `restore --name <n> --version <N>` (write path — OpenRouter embed + `--llm-backend none`, then SEPARATE enrich)
- INVOKE `forget --name <n>`; for hard-delete: `purge --yes --dry-run` then drop `--dry-run`
- REQUIRED — `purge --yes` alone keeps the default 90-day retention; it does NOT wipe immediately
- REQUIRED for immediate soft-delete purge — `purge --yes --now` (alias of `--retention-days 0`) or `purge --yes --retention-days 0`
- ALWAYS dry-run first: `purge --now --dry-run --json`; then `cleanup-orphans --yes` then `vacuum --json`
- NEVER skip optimistic locking in concurrent pipelines; NEVER delete via the `sqlite3` shell


## Entity Graph Operations
- INVOKE `link --from <a> --to <b> --relation <type> --create-missing --weight <float>`; MUST use `link --from-id <N> --to-id <M>` when IDs are known
- NEVER pass pure digits as `--from`/`--to` names — digit-only names are REJECTED
- INVOKE `unlink --from <a> --to <b> --relation <type>` or `--entity <name> --all`; `unlink --memory <name> --entity <name>` for a single memory-entity binding
- INVOKE `graph entities --json` via `.entities[]` (NOT `.items[]`); ORDER `--sort-by name|degree|created-at`; PAGINATE `--limit`/`--offset`
- INVOKE `graph stats --json`; `graph traverse --from <root> --depth <N> --json`; EXPORT `--format json|dot|mermaid --output <path>`
- MUST pass `--fuzzy` on ambiguous short-name traverse; WITHOUT `--fuzzy`, exit 4 includes ranked suggestions — ALWAYS use them
- INVOKE `rename-entity --name <old> --new-name <new>` or `--id <N> --new-name <new>`
- INVOKE `delete-entity --name <n> --cascade`; `merge-entities --names "a,b,c" --into <target>` or `--ids 12,17 --into-id 3`
- NEVER put `--into-id` inside `--ids` or `--into` inside `--names`; self-referential merges are REJECTED BEFORE any DB work
- ALWAYS USE shell arrays for dynamic merge lists — quote the joined string; PASS `--cross-namespace` only when intentional
- INVOKE `reclassify --name <n> --new-type <kind>` or `--from-type <old> --to-type <new> --batch`
- INVOKE `reclassify-relation --from-relation <old> --to-relation <new> --batch`; PASS `--literal-from`/`--literal-to` for verbatim match
- INVOKE `prune-relations --relation mentions --dry-run` then drop `--dry-run` with `--yes`; `normalize-entities --yes`; `prune-ner --entity <n>` or `--all --yes`
- INVOKE `memory-entities --name <memory>` or `--entity <name>`; PARSE forward JSON `entities[].{name, description, entity_type}` — `description` is REQUIRED in the envelope (empty string when unset)
- PARSE reverse lookup `--entity <name>` the same way for bound memories; ALWAYS surface `description` to the user when present
- INVOKE `graph recompute-degree --json` after delete/merge/prune (degree is NOT auto-recomputed)
- CANONICAL entity types — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- REQUIRED EntityType fold — aliases map via `map_to_canonical`; `module` → `concept`; graph-stdin ACCEPTS folded types (NEVER hard-rejects aliases)
- VALIDATE entity names — min 2 chars, no newlines, no short ALL_CAPS ≤4 chars, REJECT pure digits; NEVER use `mentions` as default relation
- KNOW graph writes are ADDITIVE with NO degree cap; NORMALIZE only via prune/merge/normalize — NEVER during write


## GraphRAG Search Operations
- USE the canonical three-layer pattern — `hybrid-search` then `read --name` then `related` or `graph traverse`
- INVOKE `recall <query> --k N` for pure semantic KNN; PASS `--no-graph`, `--precise`, `--max-distance <f>`, `--max-graph-results N`, `--all-namespaces`
- INVOKE `hybrid-search <query> --k N` for FTS5 plus KNN fusion via RRF; PASS `--rrf-k 60`; `--weight-vec 1.0 --weight-fts 1.0`; `--fallback-fts-only` for offline FTS
- USE `--with-graph --max-hops 2 --min-weight 0.3`; READ BOTH `results[]` AND `graph_matches[]`
- INVOKE `related <name> --hops N --relation <type>`
- INVOKE `deep-research "<query>" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies`; single-token queries fan out to aspect sub-queries; for manual control PASS `--sub-query-strategy manual --sub-queries-file PATH`
- WRITE large deep-research envelopes with `--output PATH` or short `-o PATH` (atomwrite); PARSE short stdout ack `{written, bytes, blake3}`; PASS `--quiet`; NEVER use `&>`
- REQUIRED fail-fast — when `-o`/`--output` is set the file MUST exist with non-zero bytes after exit 0; NEVER claim success without the ack
- DEEP-RESEARCH formula with short flag — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 deep-research "question" --k 20 --max-hops 3 -o /tmp/dr.json --json`
- TUNE deep-research with `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--max-cost-usd`, `--timeout`
- PARSE `recall` → `results[].{name, snippet, distance, score, source}`; `hybrid-search` → `results[].{name, combined_score, vec_rank, fts_rank}`; `deep-research` → `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context`, `stats`
- NEVER confuse `distance` with `combined_score`; NEVER raise hops without inspecting `graph stats` first


## Enrich Operations
- INVOKE `enrich --operation <op> --mode <backend>` where BOTH flags are MANDATORY for any LLM operation; omitting `--mode` is rejected with exit 2 — EXCEPT read-only inspectors `--status`, `--list-dead`, `--requeue-dead`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` which do NOT require `--operation`/`--mode`; `--dry-run` also allows omitting `--mode`
- FULLY-IMPLEMENTED ops that PERSIST — `memory-bindings` (entities→unbound memories), `augment-bindings` (extra bindings on ALREADY-bound memories; REQUIRES `--names`/`--memory-names` or `--names-file`), `entity-descriptions`, `body-enrich`, `re-embed` (rebuild vectors only), `weight-calibrate`, `relation-reclassify`, `entity-connect`, `entity-type-validate`, `description-enrich`, `cross-domain-bridges`, `domain-classify`, `deep-research-synth` (when bindings persist), `body-extract` + `--body-extract-graph-only` (graph only, no body rewrite)
- SCAN/REPORT only — `graph-audit` (does NOT mutate graph structure)
- Valid `--mode` values — `codex`, `claude-code`, `opencode`, `openrouter`
- PASS `--codex-model`, `--claude-model`, `--opencode-model`, or `--openrouter-model` matching the chosen mode
- KNOW `--mode openrouter` requires `--openrouter-model`, key from XDG `config add-key` or `--openrouter-api-key`, REST `/chat/completions` with strict json_schema and `provider.require_parameters` true, billed via `usage.cost`
- PASS `--limit N --resume` for `re-embed`; `--retry-failed`; `--dry-run`; `--target memories|entities|chunks|all` on `re-embed` only (default `memories`)
- KNOW `re-embed` selects MISSING vectors, EMPTY blobs, or dimension DIVERGENT from configured `--embedding-dim`
- PASS `--min-output-chars N` for `body-enrich`; `--fallback-mode codex` on Claude rate limits
- REQUIRED name filters — prefer `--entity-names a,b` for entity-keyed ops (`entity-descriptions`, `entity-connect`, …) and `--memory-names a,b` for memory-keyed ops (`memory-bindings`, `body-enrich`, …); `--names` remains a BC alias with operation-dependent semantics
- REQUIRED empty-match — when a name filter matches zero candidates, PARSE JSON `matched=0` plus `hint` and STOP (NEVER pretend success)
- REQUIRED low-quality redescribe — PASS `--force-redescribe` on `entity-descriptions` to re-scan empty OR low-quality generic descriptions; default is write-once for non-empty descriptions
- ENTITY-DESCRIPTIONS force formula — `sqlite-graphrag enrich --operation entity-descriptions --mode openrouter --openrouter-model openai/gpt-oss-120b --force-redescribe --entity-names jwt,auth-svc --json`
- ENTITY-DESCRIPTIONS codex — `sqlite-graphrag enrich --operation entity-descriptions --mode codex --codex-model gpt-5.5 --force-redescribe --entity-names jwt --json`
- ENTITY-DESCRIPTIONS claude — `sqlite-graphrag enrich --operation entity-descriptions --mode claude-code --claude-model claude-sonnet-4-6 --force-redescribe --entity-names jwt --json`
- ENTITY-DESCRIPTIONS opencode — `sqlite-graphrag enrich --operation entity-descriptions --mode opencode --opencode-model opencode/big-pickle --force-redescribe --entity-names jwt --json`
- PASS `--quality-sample N` with `--status` to report `quality_pct` and `scan_backlog_low_grounding_est` (flag > XDG `enrich.entity_description.quality_sample` > default 50; `0` disables)
- KNOW queue isolation — each drain claims only rows for the selected `operation`; memory-only ops MUST NOT claim `pair:`/`entity:`/`chunk:` keys
- KNOW status `state` may be `draining` | `cooldown` | `pending-scan` | `blocked_dead`; `blocked_dead` means dead-letter debt blocks progress until `--list-dead` / `--requeue-dead` / prune
- NEVER run multiple `enrich` processes in parallel on the same database (per-namespace singleton); REST parallelism is ONLY `--rest-concurrency` inside ONE process
- PASS `--until-empty` to loop scan→drain until empty or `--max-runtime` (default 3600) expires; PASS `--max-attempts <N>` default 8 range 1..=20
- PASS `--status` for `scan_backlog`, `unbound_backlog`, queue counts, `eligible_now`, `waiting`, `quality_pct`, `state` — NO LLM, NO singleton
- KNOW `scan_backlog` is REAL DB backlog; DISTINCT from `unbound_backlog` and sidecar `queue_pending`
- PASS `--list-dead`; `--requeue-dead`; `--ignore-backoff`; `--prune-dead-orphans`; `--prune-dead-entity-orphans` (mutually exclusive); `--reset-stale-claims` after `kill -9`
- KNOW dead-letter has Transient backoff vs HardFailures; truncated OpenRouter completions (`finish_reason` = `length`) re-emit with GROWN `max_tokens` before JSON repair
- KNOW enrich queue is sidecar `.enrich-queue.sqlite` beside the main DB
- DISTINGUISH — `scan_backlog` = DB candidates a fresh scan WOULD enqueue; `queue_pending` = sidecar count; `eligible_now == 0` with `queue_pending > 0` is COOLDOWN; stuck `draining` → `--reset-stale-claims`; `state=blocked_dead` → requeue or prune dead letters FIRST
- STATUS formula — `sqlite-graphrag enrich --status --quality-sample 50 --json`
- UNTIL-EMPTY formula — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --max-attempts 8 --rest-concurrency 8 --json`
- MEMORY-BINDINGS by names — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --memory-names mem-a,mem-b --json`
- FULL BACKFILL re-embed — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 enrich --operation re-embed --target all --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --json` then `health --json`


## Entity-Connect Operational Rules
- KNOW `entity-connect` is FULLY-IMPLEMENTED and PERSISTS edges via `entity_connect_seen` with `related` or `none` verdicts; `cross-domain-bridges` uses the SAME scan and drain path
- KNOW the pair scan is O(k) over co-occurrence candidates plus hub × degree-0 island fill — SAFE on large `global` namespaces; NEVER materializes a full entity Cartesian product
- KNOW queue keys MUST be `pair:{id1}:{id2}` with `item_type=entity_pair`; drain resolves entities by primary key and MUST NOT re-run the full scan per item
- KNOW the first scan and rescans are covered by `--max-runtime` and a soft ~120s `InterruptHandle`; interrupt surfaces as Timeout exit **1** — NEVER treat that as exit 75 singleton lock
- KNOW NDJSON progress emits `scan_start` before SQL and `scan_meta` after with dual backlog fields `backlog_degree0_proxy` versus `pairs_enqueued_this_scan`
- PARSE summary JSON for `budget_exhausted` (true when wall-clock/runtime budget ends on large namespaces) and `preempted_for_gate` (true when EC yielded so memory-bindings/entity-descriptions could run first)
- PASS `--anchor-memory <name>` to limit the pair scan to the subgraph of entities linked to that memory
- PASS `--entity-names a,b` to restrict entity-connect candidates; empty match MUST surface `matched=0` + `hint`
- ALWAYS converge with `--until-empty` until backlog is empty or runtime expires; ALWAYS inspect `--status` between long runs
- ALWAYS dry-run first on production corpora
- ENTITY-CONNECT dry-run — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --dry-run --limit 50 --json`
- ENTITY-CONNECT until-empty openrouter — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 600 --rest-concurrency 8 --json`
- ENTITY-CONNECT codex — `sqlite-graphrag enrich --operation entity-connect --mode codex --codex-model gpt-5.5 --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT claude — `sqlite-graphrag enrich --operation entity-connect --mode claude-code --claude-model claude-sonnet-4-6 --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT opencode — `sqlite-graphrag enrich --operation entity-connect --mode opencode --opencode-model opencode/big-pickle --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT anchored — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --anchor-memory <mem> --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT bridges — same formulas with `--operation cross-domain-bridges`
- KNOW legacy queue rows without the `pair:` prefix are ignored by the drain path; requeue or rescan if an old sidecar still holds them
- KNOW priority order is memory-bindings then entity-descriptions BEFORE entity-connect; long EC drains MUST yield cooperatively


## Write Then Enrich — Templates
- TREAT every write as STEP 1 then DISTINCT STEP 2; NEVER chain with `&&`
- DEFINE the OpenRouter embed prefix as — `sqlite-graphrag --embedding-backend openrouter --embedding-model <EMB> --embedding-dim 384 --llm-backend none`
- DEFAULT `<EMB>` = `qwen/qwen3-embedding-8b` unless the user picks another catalog id; FREE path uses `nvidia/llama-nemotron-embed-vl-1b-v2:free`
- STEP 1 formulas (ALWAYS exit 0 before STEP 2); ALWAYS parse `entities_created` and `enrich_recommended` on remember
- REMEMBER — `echo '{"body":"text","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | <PREFIX> remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --json`
- REMEMBER hot-set — same as REMEMBER plus `--enqueue-enrich` when the operator wants priority entity-descriptions queued immediately
- REMEMBER-BATCH — `<PREFIX> remember-batch --transaction --json` with NDJSON on stdin; PASS `--enqueue-enrich` for hot-set entity-descriptions after successful batch
- INGEST — `<PREFIX> ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- EDIT — `<PREFIX> edit --name <n> --body-file new.md --json`
- RESTORE — `<PREFIX> restore --name <n> --version 2 --json`
- STEP 2 templates (choose ONE backend per run; ALWAYS set model flags explicitly); APPLY after remember, remember-batch, ingest, edit, restore
- CODEX — `sqlite-graphrag enrich --operation memory-bindings --mode codex --codex-model gpt-5.5 --json`
- CLAUDE — `sqlite-graphrag enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- OPENCODE — `sqlite-graphrag enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- OPENROUTER text — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- REQUIRED full matrix — for EACH write path (remember, remember-batch, ingest, edit, restore) MUST run STEP 1 OpenRouter embed then ONE of the four STEP 2 backends above; NEVER skip model flags
- REMEMBER→CODEX full — STEP1 REMEMBER then `sqlite-graphrag enrich --operation memory-bindings --mode codex --codex-model gpt-5.5 --json` then if `enrich_recommended` contains entity-descriptions run `sqlite-graphrag enrich --operation entity-descriptions --mode codex --codex-model gpt-5.5 --entity-names <list> --json`
- REMEMBER→CLAUDE full — STEP1 REMEMBER then `sqlite-graphrag enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json` then entity-descriptions with `--mode claude-code --claude-model claude-sonnet-4-6`
- REMEMBER→OPENCODE full — STEP1 REMEMBER then `sqlite-graphrag enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json` then entity-descriptions with `--mode opencode --opencode-model opencode/big-pickle`
- REMEMBER→OPENROUTER full — STEP1 REMEMBER then `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json` then entity-descriptions with `--mode openrouter --openrouter-model openai/gpt-oss-120b --force-redescribe` when quality is low
- REMEMBER-BATCH / INGEST / EDIT / RESTORE → same four STEP 2 backends; ONLY the STEP 1 write command changes
- KNOW extraction STEP 2 does NOT require `--llm-backend` on the enrich line; pass embedding flags only when the operation needs vectors (`re-embed`) or when documenting host defaults
- KNOW key resolution is CLI flag `--openrouter-api-key` > XDG `config add-key`; NEVER embed the raw key in production command lines; FORBIDDEN product env as primary


## Parallel Embedding and Enrich
- SCALE embed with `--llm-parallelism N` on STEP 1 (JoinSet of N OpenRouter requests, order preserved)
- SCALE enrich with `--rest-concurrency N` + `--until-empty` on STEP 2 openrouter only (N chat calls; SQLite write stays serial via WAL claim)
- CLAMP `--llm-parallelism` 1..32 and `--rest-concurrency` 1..16; paid models MUST use 4..16; `:free` caps ~20 req/min so MUST use low N; multiple keys do NOT add capacity
- NEVER launch N enrich processes for parallelism; ONE process with `--rest-concurrency` is MANDATORY
- Headless codex/claude/opencode enrich does NOT use `--rest-concurrency` fan-out the same way; NEVER spawn multiple enrich processes to compensate
- REMEMBER parallel STEP 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --enqueue-enrich --json`
- Parallel STEP 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --rest-concurrency 8 --until-empty --max-runtime 3600 --max-attempts 8 --json`
- Parallel STEP 2 codex after OpenRouter embed — `sqlite-graphrag enrich --operation memory-bindings --mode codex --codex-model gpt-5.5 --until-empty --max-runtime 3600 --json`
- Parallel STEP 2 claude after OpenRouter embed — `sqlite-graphrag enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --until-empty --max-runtime 3600 --json`
- Parallel STEP 2 opencode after OpenRouter embed — `sqlite-graphrag enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --until-empty --max-runtime 3600 --json`
- REMEMBER-BATCH parallel STEP 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 12 --llm-backend none remember-batch --transaction --json`
- REMEMBER-BATCH parallel STEP 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --rest-concurrency 12 --until-empty --max-runtime 3600 --json`
- INGEST parallel STEP 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 --llm-parallelism 6 --llm-backend none ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- INGEST parallel STEP 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b:nitro --rest-concurrency 12 --until-empty --max-runtime 7200 --max-attempts 8 --json`
- EDIT parallel STEP 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none edit --name <n> --body-file new.md --json`
- EDIT parallel STEP 2 — same openrouter parallel STEP 2 as remember
- RESTORE parallel STEP 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none restore --name <n> --version 2 --json`
- RESTORE parallel STEP 2 — same openrouter parallel STEP 2 as remember
- MONITOR with `enrich --status --json` until `scan_backlog` `queue_pending` `eligible_now` are all 0; `queue_dead` is permanent data debt until requeue or prune


## Read-Only OpenRouter Formulas
- INIT — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 init --namespace <ns>`
- RECALL — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 recall "query" --k 10 --json`
- HYBRID-SEARCH — `sqlite-graphrag --embedding-backend openrouter --embedding-model baai/bge-m3 --embedding-dim 384 hybrid-search "query" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- DEEP-RESEARCH — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model openai/text-embedding-3-small --embedding-dim 384 deep-research "question" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies -o /tmp/research.json --json`
- MEMORY-ENTITIES — `sqlite-graphrag memory-entities --name <memory> --json` then parse `entities[].description`
- RENAME-ENTITY — `sqlite-graphrag --embedding-backend openrouter --embedding-model perplexity/pplx-embed-v1-0.6b --embedding-dim 384 rename-entity --name <old> --new-name <new> --json`
- HYBRID-SEARCH offline — `sqlite-graphrag hybrid-search "query" --k 10 --fallback-fts-only --json`
- TRAVERSE fuzzy — `sqlite-graphrag graph traverse --from <short-alias> --depth 2 --fuzzy --json`
- LINK by ID — `sqlite-graphrag link --from-id <N> --to-id <M> --relation uses --json`
- MERGE self-ref guard — NEVER run `merge-entities --ids 3,12 --into-id 3`; ALWAYS exclude the survivor from `--ids`
- VERIFY OpenRouter embedding catalog only with a key from `config add-key` / doctor — NEVER depend on product env


## Diagnostics and Maintenance
- INIT — `sqlite-graphrag init --namespace <ns>`
- HEALTH — `sqlite-graphrag health --json` for `{integrity_ok, schema_version, vec_*_missing, vec_*_coverage_pct}`; TRIGGER `enrich --operation re-embed --target <target>` when any missing count > 0
- MIGRATE — `migrate --dry-run --json` then `migrate --json`; OPTIMIZE — `optimize --json`; VACUUM — `vacuum --json` after purge
- FTS — `fts check|stats|rebuild --json` when `health.fts_degraded`; VEC — `vec orphan-list --json` then `vec purge-orphan --yes`; `vec stats --json`
- EMBEDDING — `embedding --status --json`; REQUIRED alias works — `pending-embeddings --status --json` (queue health counts)
- RE-PROCESS pending vectors via `enrich --operation re-embed` (not a product-env path)
- SLOTS — `slots status --json`; `slots release --slot-id <N> --yes`; PENDING — `pending list --filter-status queued --json`; `pending show <id>`; `pending cleanup --yes`
- EXPORT — `export --namespace <ns> --type <kind> --json`; STATS — `stats --json`
- BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECT — `namespace-detect --json`, `debug-schema --json`, `cache list --json`, REQUIRED `cache stats --json` (alias of `list`), `cache clear-models --yes`
- COMPLETIONS — `completions bash|zsh|fish|elvish|powershell`
- SCHEDULE weekly — `purge --yes` (90d retention) or `purge --yes --now` when immediate → `cleanup-orphans` → `prune-relations --relation mentions` → `vacuum` → `optimize` → `sync-safe-copy`
- IF corruption — `sqlite3 broken.sqlite ".recover" | sqlite3 repaired.sqlite`


## Exit Codes and Retry Strategy
- EXIT 0 success
- EXIT 1 validation **or** wall-clock Timeout (including first-scan entity-connect InterruptHandle — NOT the same as exit 75)
- EXIT 2 argument parsing; EXIT 3 optimistic lock (reload and retry)
- EXIT 4 not found (without `--fuzzy`, includes Jaro-Winkler/prefix suggestions); EXIT 5 namespace error
- EXIT 6 payload too large — variants report `bytes`/`limit`, `chunks`/`limit`, or `tokens`/`limit`; SPLIT body or reduce tokens
- EXIT 9 duplicate — use `--force-merge`; EXIT 10 database — run `vacuum` plus `health`
- EXIT 11 embedding failure — check backend, dim 384, OAuth/key
- EXIT 13 partial batch — reprocess failed only; EXIT 14 I/O; EXIT 15 busy — widen `--wait-lock`
- EXIT 16 preflight — fix MCP config; NEVER treat as transient
- EXIT 19 SHUTDOWN — retry MANDATORY; partial work discarded; EXIT 20 internal
- EXIT 75 slots/singleton locked — respect cooldown; NEVER retry immediately
- EXIT 77 RAM pressure; EXIT 78 config (OpenRouter key or model missing)
- NEVER ignore non-zero exit; NEVER reprocess full batch after exit 13; NEVER confuse exit 1 Timeout with exit 75 lock or exit 9 duplicate


## Concurrency
- RESPECT the hard ceiling `2 x nCPUs` for heavy commands — `init`, `remember`, `ingest`, `recall`, `hybrid-search`
- SET `--llm-parallelism N` default 4 on `remember` and `edit`, default 2 on `ingest`, clamp [1, 32]
- KNOW JOB SINGLETON — `enrich` and `ingest --mode codex|claude-code` acquire a per-namespace singleton
- USE `--wait-job-singleton SECS` or `--force-job-singleton` to break a stale lock
- ENABLE unitary parallelism via flag `--low-memory` or XDG `config set` (slower); FORBIDDEN product env
- NEVER run `enrich` in parallel against the same database
- KNOW REST enrich concurrency is controlled by `--rest-concurrency`, NOT by launching multiple enrich processes


## XDG Config (REQUIRED)
- REQUIRED precedence for operational settings — CLI flag > XDG `config set` > named default
- FORBIDDEN — product env `SQLITE_GRAPHRAG_*` is NOT read on the hot path; do NOT document or export it as the config mechanism
- FORBIDDEN — no lib telemetry alias; no product telemetry
- REQUIRED store keys — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- REQUIRED set settings — `sqlite-graphrag config set <key> <value>`
- REQUIRED get/list — `config get <key>`; `config list --json`; `config list --effective --json` (includes defaults)
- REQUIRED unset — `config unset <key>`; path — `config path`; doctor — `config doctor --json`
- REQUIRED OpenRouter network URLs via config set — `sqlite-graphrag config set network.openrouter.chat_url https://openrouter.ai/api/v1/chat/completions` and `sqlite-graphrag config set network.openrouter.embeddings_url https://openrouter.ai/api/v1/embeddings`; aliases accepted — `network.chat_url`, `network.embed_url`
- REQUIRED query Auto fail-fast — `llm.query_embed_timeout_secs` default **3s** (wall-clock probe stays tight; do not raise casually)
- REQUIRED enrich quality sample default — `config set enrich.entity_description.quality_sample 50`
- REQUIRED common XDG keys — `db.default_path`, `embedding.dim`, `embedding.backend`, `embedding.model`, `llm.backend`, `llm.model`, `llm.query_embed_timeout_secs`, `display.tz`, `i18n.lang`, `log.level`, `log.format`, `spawn.skip_preflight` (emergencies only), `enrich.yield_every_n_items`
- ALWAYS prefer one-shot flags (`--db`, `--namespace`, `--embedding-backend`, …) for agents; use XDG only for host defaults
- NEVER rely on `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` for codex/claude; they are FORBIDDEN and abort spawn with exit 1


## Active Rules
- ALWAYS `--json`; ALWAYS `jaq` after capture (NEVER pipe NDJSON; NEVER `jq`); ALWAYS parse `backend_invoked`
- ALWAYS `--embedding-backend openrouter --embedding-model <MODEL> --embedding-dim 384` on embed ops with key via `config add-key` or `--openrouter-api-key`
- ALWAYS `--llm-backend none` on writes; ALWAYS SEPARATE `enrich` with `--mode` + model; NEVER chain with `&&`
- ALWAYS parse remember `entities_created` and `enrich_recommended`; ALWAYS run entity-descriptions when recommended or when `--enqueue-enrich` was used
- ALWAYS prefer `--entity-names` / `--memory-names` over ambiguous `--names`; ALWAYS handle empty-match `matched=0` + `hint`
- ALWAYS use `--force-redescribe` when re-annotating low-quality entity descriptions; ALWAYS inspect `quality_pct` via `--status --quality-sample`
- ALWAYS treat `state=blocked_dead` as hard debt until requeue/prune; ALWAYS parse EC `budget_exhausted` and `preempted_for_gate`
- ALWAYS parse `memory-entities` `entities[].description`; ALWAYS use `-o` or `--output` for large deep-research
- ALWAYS refresh OAuth (`codex login` / claude OAuth) when stale; ALWAYS keep dim 384 (mismatch → knn exit 11)
- ALWAYS shell arrays + quoted joins for dynamic `merge-entities` lists
- MUST use `--from-id`/`--to-id` for numeric link IDs; NEVER pure digits as `--from`/`--to` names
- MUST retry with `--fuzzy` or NotFound suggestions on short-name traverse; MUST use `--output PATH`/`-o PATH` + `--quiet` for large deep-research; NEVER `&>`
- NEVER pass API keys to codex/claude (OAuth-only, exit 1); NEVER `--llm-backend codex` on write paths with OpenRouter embed
- NEVER parallel `enrich` processes on same DB; NEVER write `.sqlite` outside the binary
- NEVER ignore exit 19 (retry MANDATORY) or exit 16 (fix MCP); NEVER openrouter without model+key (exit 78)
- NEVER self-ref merge (`--into-id` inside `--ids` / `--into` inside `--names`); NEVER MCP memory or MEMORY.md
- NEVER document product env `SQLITE_GRAPHRAG_*` as configuration; NEVER invent version-history prose inside this skill
- CANONICAL memory types — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- CANONICAL entity types — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- CANONICAL relations — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
