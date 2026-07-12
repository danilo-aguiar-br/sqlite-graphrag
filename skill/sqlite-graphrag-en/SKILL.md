---
name: sqlite-graphrag
description: This skill MUST activate for every sqlite-graphrag CLI operation covering persistent memory, GraphRAG knowledge graph, entity linking, hybrid-search, recall, deep-research, remember, remember-batch, ingest, edit, restore, enrich, forget, purge, link, unlink, merge-entities, rename-entity, reclassify, graph traverse, OpenRouter embedding model prices, API key management, headless codex claude opencode backends, OpenRouter text models for enrich, write-then-enrich formulas, parallel embedding, exit codes, concurrency, env vars, FTS5 plus BLOB cosine fusion, canonical types and relations, namespace isolation, and OAuth-only rules. This skill MUST be used whenever the agent stores, retrieves, searches, enriches, links, merges, or maintains long-term GraphRAG memory. Keywords sqlite-graphrag GraphRAG memory embedding openrouter codex claude opencode remember recall hybrid-search ingest enrich deep-research forget purge link rename-entity merge-entities
---

## When This Skill Activates
- MUST ACTIVATE when the user asks to remember, save, recall, retrieve, search, or persist anything across sessions
- MUST ACTIVATE for long-term context, knowledge graph, GraphRAG, RAG, entity linking, memory management, and namespace-scoped knowledge
- MUST ACTIVATE when sqlite-graphrag, embedding, FTS5, hybrid-search, OpenRouter, codex, claude, opencode, or LLM memory is mentioned
- MUST ACTIVATE for enrich, re-embed, link, unlink, merge-entities, rename-entity, deep-research, ingest, and graph maintenance
- NEVER ACTIVATE for one-off ephemeral data, simple file I/O, or tasks unrelated to persistent context
- ALWAYS load this skill before inventing ad-hoc memory files, MCP memory servers, or hand-written Markdown journals


## Core Mental Model
- KNOW THREE independent selectors; NEVER conflate them
- SELECTOR 1 — `--embedding-backend` HOW vectors are produced — `openrouter` (REST), `llm` (subprocess), or `auto`
- SELECTOR 2 — `--llm-backend` WHICH subprocess embeds when backend is `llm` — `codex`, `claude`, `opencode`, or `none`
- SELECTOR 3 — extraction via `enrich --mode` — `codex`, `claude-code`, `opencode`, or `openrouter` (REST `/chat/completions`); `--extraction-backend` is the related global selector
- WRITE and ENRICH are ALWAYS separate; write produces embeddings; SEPARATE `enrich` extracts the graph
- NEVER chain write and enrich with `&&`; ALWAYS wait for write exit 0, then run enrich as a DISTINCT process
- On EVERY write (`remember`, `remember-batch`, `ingest`, `edit`, `restore`) MUST pass `--llm-backend none` with `--embedding-backend openrouter` so embeddings STILL run via OpenRouter REST without an LLM subprocess timeout
- ALWAYS pass `--json`; ALWAYS parse with `jaq` NEVER `jq`; ALWAYS capture stdout FIRST then parse; NEVER pipe CLI output directly into `jaq` (NDJSON masks failures as null)
- KNOW empty vectors are NEVER persisted; PARSE `backend_invoked` to confirm the backend; RUN `enrich` only after write exit 0


## LLM Prompt Instruction Rules
- "remember this" → `remember --force-merge` with `--graph-stdin` curated entities and canonical relations, then SEPARATE `enrich`
- "what do you know about X" → `hybrid-search "X" --k 10 --json` FIRST, then `read --name <name> --json`
- "how is X related to Y" → `graph traverse --from X --depth 2 --json` or `related X --hops 2 --json`; on miss RETRY `--fuzzy` or pick exit 4 NotFound suggestions
- "deep research on X" → `deep-research "X" --k 20 --max-hops 3 --json`; single-token subjects fan out to aspect sub-queries; large envelopes MUST use `--output PATH` and `--quiet`
- BEFORE any create → `hybrid-search "<name>" --k 5 --json`; if duplicate USE `--force-merge`
- AFTER create/update → capture-parse `read --name <name> --json` for `{name, description, body_length}`
- AFTER every turn → persist new findings or DECLARE "No new findings to persist"
- On non-zero exit → parse `jaq '{code, message, error_class}'` and REPORT remediation
- ALWAYS use canonical relations — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- ALWAYS map non-canonical first — `adds`/`creates`→`causes`, `implements`→`supports`, `blocks`→`contradicts`, `tested-by`→`related`, `part-of`→`applies-to`
- ALWAYS kebab-case ASCII lowercase entity names; LIMIT entities to domain concepts; REJECT generics, pronouns, UUIDs, timestamps
- NEVER use MCP Serena, `.md` memory files, or MEMORY.md; NEVER start a daemon; NEVER pass `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` to subprocess backends
- PREFER `remember --force-merge` over `edit`; PREFER `--graph-stdin` over inline extraction


## Architecture and Principles
- INVOKE as subprocess; stdout = JSON/NDJSON; stderr = logs; CHECK exit code BEFORE parsing
- KNOW NO daemon, NO ONNX, NO model cache; cosine is pure Rust over BLOB `memory_embeddings` / `entity_embeddings` / `chunk_embeddings`
- KNOW `init` or `migrate` applies live schema; READ `schema_version` from `health --json`
- ENFORCE OAUTH-ONLY for codex/claude — spawn ABORTS exit 1 if `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` is set; `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` are PRESERVED
- KNOW subprocess CWD is ISOLATED; 7 preflight guards run before every LLM fork; exit 16 = preflight failure; `claude -p` inherits CWD `.mcp.json` — ISOLATE config for `claude-code`
- SET `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1` ONLY in emergencies; namespace via `--namespace` or env (default `global`)
- NEVER expose as MCP/HTTP; NEVER write `.sqlite` from another tool; FUSION is FTS5 BM25 plus BLOB cosine KNN via RRF


## OpenRouter Embedding Models and Prices
- PASS `--embedding-model <MODEL>` when `--embedding-backend openrouter`; there is NO default model, so omission triggers exit 78
- KNOW prices below are USD per one million tokens; CHOOSE the model by cost and quality for the task
- USE `nvidia/llama-nemotron-embed-vl-1b-v2:free` for FREE zero-cost embedding (no token charge)
- USE `qwen/qwen3-embedding-4b` at 0.05 USD per million tokens (4B, cheap and capable)
- USE `qwen/qwen3-embedding-8b` at 0.05 USD per million tokens (RECOMMENDED default, higher Qwen quality)
- USE `baai/bge-m3` at about 0.05 USD per million tokens
- USE `openai/text-embedding-3-small` at 0.05 USD per million tokens
- USE `perplexity/pplx-embed-v1-0.6b` at 0.05 USD per million tokens
- USE `mistralai/mistral-embed-2312` at 0.10 USD per million tokens
- USE `google/gemini-embedding-2` at about 0.12 USD per million tokens
- USE `openai/text-embedding-3-large` at 0.13 USD per million tokens
- USE `google/gemini-embedding-005` at about 0.15 USD per million tokens
- KEEP `--embedding-dim 384` consistent across ALL writes and reads; a mismatched dimension collides with the stored index and fails knn with exit 11
- KNOW MRL truncates server-side to `--embedding-dim`; higher native dims stay cheap when truncated to 384
- VERIFY live models — `curl -sS https://openrouter.ai/api/v1/models -H "Authorization: Bearer $OPENROUTER_API_KEY" | jaq -c '.data[] | select(.id | test("embed";"i")) | {id, pricing}'`; DO NOT trust stale ids alone
- CONFIRM keys with `sqlite-graphrag config doctor --json`; invalid model → exit 78; MONITOR `usage.cost` per call
- KNOW openrouter propagates to ALL embed paths — `remember` `remember-batch` `ingest` `recall` `edit` `restore` `hybrid-search` `deep-research` `enrich` `init` `rename-entity`


## OpenRouter API Key Management
- ADD a key via stdin — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- LIST stored keys — `sqlite-graphrag config list-keys --json`
- REMOVE a key by fingerprint — `sqlite-graphrag config remove-key <fingerprint> --json`
- RUN the diagnostic doctor — `sqlite-graphrag config doctor --json`
- INSPECT the config path — `sqlite-graphrag config path`
- KNOW keys live in XDG config `~/.config/sqlite-graphrag/config.toml` with `chmod 600` and are zeroized on drop, NEVER logged
- KNOW precedence — `OPENROUTER_API_KEY` env > `config.toml` > CLI flag `--openrouter-api-key`
- NEVER pass the API key as a CLI argument in production; PREFER stdin or env var to avoid shell-history exposure
- ALWAYS run `config doctor` after adding a key before any paid embedding or enrich call


## Headless LLM Backends
- CHOOSE codex with `--llm-backend codex --llm-model gpt-5.4-mini` for embedding paths and `enrich --mode codex --codex-model gpt-5.4-mini` for extraction; refresh OAuth with `codex login`; codex is OAUTH-ONLY — NEVER pass `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`
- CHOOSE claude with `--llm-backend claude --llm-model claude-sonnet-4-6` for embedding paths and `enrich --mode claude-code --claude-model claude-sonnet-4-6` for extraction via the OAuth zero-token path; claude is OAUTH-ONLY — NEVER pass `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`
- CHOOSE opencode with `--llm-backend opencode --llm-model opencode/big-pickle` for embedding paths and `enrich --mode opencode --opencode-model opencode/big-pickle` for extraction via its own auth (NOT OAuth)
- CHOOSE openrouter for extraction ONLY with `enrich --mode openrouter --openrouter-model <id>`; `--openrouter-model` is MANDATORY (no default; missing value exits 1 before any network call); key comes from `OPENROUTER_API_KEY`
- KNOW DEFAULT models — codex `gpt-5.5`, claude `claude-sonnet-4-6`, opencode `opencode/big-pickle`
- KNOW opencode catalog is EXTERNAL/dynamic; `--opencode-model` is UNVALIDATED; PASS live OpenCode Zen ids (default `opencode/big-pickle`); CONSULT `opencode.ai/zen`
- OVERRIDE binaries with `--codex-binary`, `--claude-binary`, `--opencode-binary`; TUNE ingest timeouts with `--codex-timeout`, `--claude-timeout`, `--opencode-timeout`
- VALIDATE codex models with `--codex-model-validate` and `--codex-model-fallback <MODEL>`; LIST with `sqlite-graphrag codex-models --json` (CODEX models only, NOT OpenRouter)
- SWAP backend on rate limit with `enrich --fallback-mode codex` or global `--llm-fallback codex,claude,none`
- KNOW `claude-code` spawns `claude -p` inheriting CWD `.mcp.json` (may fail); PREFER codex or isolate config dir
- KNOW `--mode openrouter` is pure REST `/chat/completions` — NO local CLI required; bills `OPENROUTER_API_KEY` (read `usage.cost`); codex/claude-code/opencode are zero-token OAuth/own-auth paths


## OpenRouter Text Models for Enrich
- PASS `--openrouter-model <MODEL>` from this table on `--mode openrouter`; prices are input/output USD per one million tokens
- KNOW these models serve ONLY entity extraction and enrichment, NEVER embedding; the embedding table is separate
- USE `openai/gpt-oss-120b` at 0.059/0.18 USD, 131k context, 36 tps (RECOMMENDED cheap judge default)
- USE `openai/gpt-oss-120b:nitro` at 0.15/0.60 USD, 131k context, 300 tps (FASTEST throughput)
- USE `deepseek/deepseek-v4-flash` at 0.09/0.18 USD, 1M context, 20 tps
- USE `deepseek/deepseek-v4-flash:nitro` at 0.14/0.28 USD, 1M context, 109 tps
- USE `deepseek/deepseek-v4-pro` at 1.30/2.60 USD, 1M context, 26 tps
- USE `google/gemini-3.1-flash-lite` at 0.95/3.00 USD, 1M context, 100 tps
- USE `minimax/minimax-m3` at 0.30/1.20 USD, 1M context, 42 tps
- USE `minimax/minimax-m2.7` at 0.25/1.00 USD, 205k context, 43 tps
- USE `minimax/minimax-m2.7:nitro` at 0.30/1.20 USD, 205k context, 146 tps
- USE `xiaomi/mimo-v2.5` at 0.10/0.28 USD, 1M context, 17 tps
- USE `xiaomi/mimo-v2.5-pro` at 0.43/0.87 USD, 1M context, 29 tps
- USE `z-ai/glm-5.2` and `z-ai/glm-5.2:nitro` whose price varies by provider; ALWAYS CONFIRM real cost via `usage.cost` in the response
- KNOW `:nitro` variants route to the fastest provider at a higher price
- VERIFY a model honours strict `json_schema` BEFORE production; a model without Structured Outputs support fails with an explicit OpenRouter error
- READ `usage.cost` from the chat response to account the real token cost per item


## Global Flags Reference
- `--db <PATH>` — override database location; PLACE it AFTER the subcommand (e.g. `remember --db <PATH>`); the position-independent override is env `SQLITE_GRAPHRAG_DB_PATH`
- `--namespace <ns>` — scope operations to a namespace
- `--json` — structured JSON output (ALWAYS pass)
- `--lang en|pt` — force stderr language
- `--tz <TIMEZONE>` — localize timestamps
- `--embedding-backend auto|openrouter|llm` — vector production selector
- `--embedding-model <MODEL>` — OpenRouter embedding model (MANDATORY with openrouter)
- `--embedding-dim N` — embedding dimensionality [8, 4096], default 384 MRL
- `--openrouter-api-key <KEY>` — OpenRouter API key (FORBIDDEN in production shell history)
- `--llm-backend codex|claude|opencode|none|auto` — subprocess embedding backend; comma-separated chain allowed
- `--llm-model <MODEL>` — model for the active LLM backend
- `--llm-fallback <chain>` — comma-separated fallback chain when the primary fails
- `--extraction-backend codex|claude-code|opencode|openrouter` — entity-extraction backend selector (openrouter is REST, not a subprocess)
- `--openrouter-model <MODEL>` — MANDATORY judge model for `--mode openrouter` (no default; absence exits 1 before any network call)
- `--openrouter-base-url <URL>` — optional OpenRouter endpoint override for chat enrich
- `--openrouter-timeout <SECS>` — chat enrich request timeout, default 600
- `--llm-parallelism N` — embedding fan-out, default 4, clamp [1, 32]; governs subprocess AND OpenRouter REST JoinSet concurrency; small single-batch inputs stay serial
- `--max-concurrency N` — heavy-invocation cap, clamp [1, 2×nCPUs]
- `--llm-max-host-concurrency N` — host-wide LLM slot cap; `--llm-slot-wait-secs N` / `--llm-slot-no-wait`
- `--wait-lock SECS` — widen lock acquisition; `--low-memory` unitary parallelism; `--strict-env-clear` PATH-only subprocess; `--graceful-shutdown-secs N` before SIGKILL
- `--skip-embedding-on-failure` — store without vector when chain ends in `none`
- `--codex-binary`, `--claude-binary`, `--opencode-binary` — binary path overrides
- `-v` / `-vv` / `-vvv` — info / debug / trace on stderr
- `--quiet` / `-q` — suppress non-error stderr tracing; MANDATORY in headless pipelines; NEVER mix stderr into JSON with `&>`


## CRUD Write Operations
- INVOKE `remember --name <kebab> --type <kind> --description <text>` with exactly one body source — `--body <text>` or `--body-file <path>` or `--body-stdin` or `--graph-stdin`
- INVOKE `remember --graph-stdin` to attach `{body, entities, relationships}` in a single JSON document
- INVOKE `remember --graph-file <path>` to load the entity graph from a file; COMBINE with `--body-file <path>` to supply body and graph from separate files
- PASS entities as `[{name, entity_type}]` in kebab-case ASCII; PASS relationships as `[{source, target, relation, strength}]` where strength is in [0.0, 1.0]
- PASS `--strict-name` to REJECT a non-kebab-case name instead of auto-normalizing it
- PASS `--force-merge` for idempotent updates and soft-deleted restoration
- PASS `--replace-graph` together with `--force-merge` to ZERO existing entity/relationship bindings before writing the new graph (full replace, not merge)
- PASS `--dry-run` to validate inputs without persisting
- VALID memory `--type` values — `user`, `feedback`, `project`, `reference`, `decision`, `incident`, `skill`, `document`, `note`
- INVOKE `remember-batch` for 10 or more memories via NDJSON stdin; PASS `--transaction` for all-or-nothing
- INVOKE `ingest <DIR> --recursive --pattern "*.md" --mode none` to import a directory as body-only, then enrich SEPARATELY
- KNOW `ingest --mode` accepts `none` (default body-only), `claude-code`, `codex`, `opencode`; each non-none mode runs LLM-curated extraction inline DURING ingest and needs NO separate enrich for the bindings of that ingest
- USE `--resume` to continue from the queue after interruption; `--retry-failed` for failed items only; `--auto-describe` to synthesize descriptions
- PASS `--name-prefix <prefix>` on `ingest` to prefix derived file names; the prefix counts toward the name-length ceiling and applies ONLY to local directory ingestion
- PASS `--force-merge` on `ingest` to UPDATE duplicate files instead of skipping them; ingest dedups by `body_hash`, so an unchanged file is skipped even after a rename
- KNOW `ingest` natively auto-splits an oversized body into multiple chunks, so a file above the per-body limit is chunked, NOT rejected
- INVOKE `split-body --name <N>` to split ONE memory over 25000 characters into daughters at chunk boundaries; original is marked `SUPERCEDIDO` with `replaces` edges; PASS `--batch --threshold 25000` for all oversize bodies; DAUGHTERS ARE NOT EMBEDDED INLINE — step 1 `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-backend none split-body --name <n> --json`; step 2 SEPARATE `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 enrich --operation re-embed --target memories --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --json`
- RESPECT the 512000 bytes and 512 chunks limit per body
- NEVER mix `--body`, `--body-file`, `--body-stdin`, `--graph-stdin` in a single invocation
- NEVER use `fd | xargs remember`; INVOKE `ingest` instead
- NEVER pass `--llm-backend codex` (or any non-`none` LLM backend) on any write; ALWAYS pass `--llm-backend none` with OpenRouter embeddings


## CRUD Read Update Delete
- INVOKE `read --name <kebab> --json` for O(1) fetch; PASS `--with-graph` to include linked entities
- USE `read --name <n> --format raw` to print pure body text with NO JSON envelope when piping into another tool
- INVOKE `list --type <kind> --limit N --offset N --json` to filter and paginate
- INVOKE `history --name <n> --diff --json` for version history with character diff stats
- INVOKE `edit --name <n> --body-file <path>` to update the body, or `--description <text>` and `--memory-type <kind>` for metadata
- USE `--force-reembed` to regenerate the embedding without a body change
- USE `--expected-updated-at <ts>` for optimistic locking; TREAT exit 3 as a conflict — reload and retry
- INVOKE `rename --name <old> --new-name <new>` to rename a memory preserving history
- INVOKE `restore --name <n> --version <N>` to restore an old version (write path — still requires OpenRouter embed + `--llm-backend none`, then SEPARATE enrich)
- INVOKE `forget --name <n>` for a reversible soft-delete
- INVOKE `purge --retention-days <N> --yes --dry-run` to preview, then drop `--dry-run` for the hard delete
- INVOKE `cleanup-orphans --yes` after bulk forget, then `vacuum --json`
- NEVER skip optimistic locking in concurrent pipelines; NEVER delete manually via the `sqlite3` shell


## Entity Graph Operations
- INVOKE `link --from <a> --to <b> --relation <type> --create-missing --weight <float>` to create an edge by name
- PREFER `link --from-id <N> --to-id <M> --relation <type> --weight <float>` when you hold integer entity IDs
- NEVER pass pure digits as `--from` / `--to` entity names — digit-only names are REJECTED so `--create-missing` cannot create ghost numeric entities
- KNOW `--create-missing` applies to name-based link only; ID-based link REQUIRES pre-existing entities
- INVOKE `unlink --from <a> --to <b> --relation <type>` to remove one edge, or `--entity <name> --all` to drop all edges of an entity
- INVOKE `unlink --memory <name> --entity <name>` to remove a single curated memory-to-entity binding without touching entity-to-entity edges
- INVOKE `graph entities --json` via `.entities[]` (NOT `.items[]`); ORDER `--sort-by name|degree|created-at` with `--order asc|desc`; PAGINATE `--limit N --offset N`
- INVOKE `graph stats --json` for `node_count`, `edge_count`, `avg_degree`, `max_degree`
- INVOKE `graph traverse --from <root> --depth <N> --json`; EXPORT `--format json|dot|mermaid --output <path>`
- PASS `--fuzzy` on `graph traverse` to auto-resolve a clear short-name winner; WITHOUT `--fuzzy`, exit 4 NotFound includes ranked Jaro-Winkler and prefix suggestions — ALWAYS use them
- INVOKE `rename-entity --name <old> --new-name <new>` or `--id <N> --new-name <new>` to disambiguate across namespaces
- INVOKE `delete-entity --name <n> --cascade` to delete an entity and its edges
- INVOKE `merge-entities --names "a,b,c" --into <target>` or `merge-entities --ids 12,17 --into-id 3`; name and ID modes conflict; IDs are globally unique
- NEVER put `--into-id` inside `--ids` or `--into` inside `--names`; self-referential merges are REJECTED BEFORE any DB work (including `--cross-namespace`); this PROTECTS against zsh word-splitting injecting the survivor into the source list
- ALWAYS USE shell arrays for dynamic merge lists — `ids=(12 17); survivor=3; sqlite-graphrag merge-entities --ids "$(IFS=,; printf '%s' "${ids[*]}")" --into-id "$survivor" --json` — ALWAYS quote the joined string
- PASS `--cross-namespace` to resolve IDs across ALL namespaces (opt-in); survivor inherits `--into-id` namespace
- INVOKE `reclassify --name <n> --new-type <kind>` for one entity, or `--from-type <old> --to-type <new> --batch` for bulk type migration
- INVOKE `reclassify-relation --from-relation <old> --to-relation <new> --batch` for bulk relation migration; FILTER with `--filter-source-type` and `--filter-target-type`; PASS `--literal-from` / `--literal-to` for VERBATIM match/write without kebab normalization (mutually exclusive with `--from-relation`); CANONICAL underscore migration — `reclassify-relation --literal-from applies_to --literal-to applies-to --batch` after `--dry-run`; REPEAT for `depends_on` and `tracked_in`
- INVOKE `prune-relations --relation mentions --dry-run` to preview low-value edges, then drop `--dry-run` with `--yes`
- INVOKE `normalize-entities --yes` to normalize all names to kebab-case ASCII
- INVOKE `prune-ner --entity <n>` to remove NER bindings; `prune-ner --all --yes` for the whole namespace
- INVOKE `memory-entities --name <memory>` for forward lookup, or `--entity <name>` for reverse lookup
- KNOW graph writes are ADDITIVE with NO degree cap; NORMALIZE only via `prune-relations`, `merge-entities`, `normalize-entities` — NEVER during write
- INVOKE `graph recompute-degree --json` after `delete-entity`, `merge-entities`, or `prune-relations` (degree is NOT auto-recomputed); envelope `{total, updated, zeroed, unchanged}` with sum equal to `total`; PASS `--dry-run` to preview
- CANONICAL entity types — `project`, `tool`, `person`, `file`, `concept`, `incident`, `decision`, `memory`, `dashboard`, `issue_tracker`, `organization`, `location`, `date`
- KNOW invalid `entity_type` fails EARLY listing all 13 values before any DB write
- VALIDATE entity names — min 2 chars, no newlines, no short ALL_CAPS ≤4 chars, REJECT pure digits
- NEVER use `mentions` as a default relation


## GraphRAG Search Operations
- USE the canonical three-layer pattern — `hybrid-search` then `read --name` then `related` or `graph traverse`
- INVOKE `recall <query> --k N` for pure semantic KNN; PASS `--no-graph` to disable graph expansion, `--precise` for exact scoring, `--max-distance <f>`, `--max-graph-results N`, `--all-namespaces`
- INVOKE `hybrid-search <query> --k N` for FTS5 plus KNN fusion via RRF
- PASS `--rrf-k 60` for standard fusion; `--weight-vec 1.0 --weight-fts 1.0` for balanced fusion
- PASS `--fallback-fts-only` to skip live embedding and serve FTS5 BM25 only in offline mode
- USE `--with-graph --max-hops 2 --min-weight 0.3` for graph expansion; READ BOTH `results[]` AND `graph_matches[]`
- INVOKE `related <name> --hops N --relation <type>` for multi-hop traversal from a memory
- INVOKE `deep-research "<query>" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies` for parallel multi-hop research
- KNOW single-token queries fan out to multi-aspect sub-queries whose `source` field equals `aspect` (EN/PT facets); PARSE `sub_queries[].source` and EXPECT `aspect` for single-token subjects; for full manual control PASS `--sub-query-strategy manual --sub-queries-file PATH`
- WRITE large deep-research envelopes with `--output PATH` (atomwrite); PARSE the short stdout ack containing `written`, `bytes`, and `blake3` when `--output` is set; PASS global `--quiet` / `-q` in headless pipelines; NEVER use `&>` (it mixes stderr into the JSON stream)
- TUNE deep-research with `--graph-decay <f>`, `--graph-min-score <f>`, `--max-neighbors-per-hop N`, `--max-cost-usd <f>`, `--timeout <secs>`
- PARSE `recall` returns `results[].{name, snippet, distance, score, source}`
- PARSE `hybrid-search` returns `results[].{name, combined_score, vec_rank, fts_rank}`
- PARSE `deep-research` returns `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context`, `stats`
- NEVER confuse `distance` with `combined_score` in ranking; NEVER raise hops without inspecting `graph stats` first


## Enrich Operations
- INVOKE `enrich --operation <op> --mode <backend>` where BOTH flags are MANDATORY for any LLM operation; omitting `--mode` is rejected with exit 2 — EXCEPT the read-only inspectors `--status`, `--list-dead`, `--requeue-dead`, `--prune-dead-orphans` (memory-keyed), and `--prune-dead-entity-orphans` (entity-keyed), which do NOT require `--operation` and `--mode`
- FULLY-IMPLEMENTED ops (persist) — `memory-bindings` (entities→unbound memories), `augment-bindings` (extra bindings on ALREADY-bound memories; requires `--names` or `--names-file`), `entity-descriptions`, `body-enrich`, `re-embed` (rebuild vectors only), `entity-connect` (PERSISTS edges via `entity_connect_seen` with `related`/`none` verdicts; v1.1.06: pair candidates from co-occurrence + hub×island — safe on large `global`; queue keys `pair:{id1}:{id2}`; converges with `--until-empty`), `cross-domain-bridges` (PERSISTS via same `entity_connect_seen` convergent path), `body-extract` + `--body-extract-graph-only` (graph only, no body rewrite)
- SCAN-ONLY ops (report only, no persist) — `weight-calibrate`, `relation-reclassify`, `entity-type-validate`, `description-enrich`, `domain-classify`, `graph-audit`, `deep-research-synth`
- Valid `--mode` values — `codex`, `claude-code`, `opencode`, `openrouter`
- USE `augment-bindings` to add MORE bindings to memories that are ALREADY linked; it REQUIRES `--names <a,b,c>` or `--names-file <path>` to scope the targets
- USE `body-extract --body-extract-graph-only` to extract the graph from a body without rewriting the body text
- PASS `--codex-model`, `--claude-model`, `--opencode-model`, or `--openrouter-model` to pick the extraction model matching the chosen mode
- KNOW `--mode openrouter` requires `--openrouter-model`, key from `OPENROUTER_API_KEY`, REST `/chat/completions` with strict json_schema and `provider.require_parameters` true, billed via `usage.cost`; other modes are OAuth/own-auth zero-token
- PASS `--limit N --resume` for `re-embed`; `--retry-failed`; `--dry-run` to preview
- PASS `--target memories|entities|chunks|all` on `re-embed` only (default `memories`)
- KNOW `re-embed` selects MISSING vectors, EMPTY blobs, or dimension DIVERGENT from configured `--embedding-dim`; `--status` sums `scan_backlog` over selected targets
- FULL BACKFILL — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 enrich --operation re-embed --target all --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --json` then `health --json`
- PASS `--min-output-chars N` for `body-enrich`; `--fallback-mode codex` on Claude rate limits
- NEVER run `enrich` in parallel on the same database (per-namespace singleton)
- PASS `--until-empty` to loop scan→drain until empty or `--max-runtime` (default 3600) expires
- PASS `--max-attempts <N>` (default 8, range 1..=20) before Transient items turn `dead`
- PASS `--status` for `scan_backlog`, `unbound_backlog`, queue counts, `eligible_now`, `waiting` — NO LLM, NO singleton, NO `--operation`/`--mode` required
- KNOW `scan_backlog` is REAL DB backlog; DISTINCT from `unbound_backlog` and sidecar `queue_pending`
- PASS `--rest-concurrency <N>` for openrouter fan-out (clamp 1..=16, default 8); DISTINCT from `--llm-parallelism`
- PASS `--list-dead` for terminal items (`error_class`, `message`, `finish_reason`, tokens); `--requeue-dead` returns them to `pending`; `--ignore-backoff` skips `next_retry_at`
- PASS `--prune-dead-orphans` to delete dead memory-keyed queue rows whose `item_key` is ABSENT from the main DB; entity-keyed rows UNTOUCHED; ONLY sidecar `.enrich-queue.sqlite` mutates — `sqlite-graphrag enrich --prune-dead-orphans --json`; USE BEFORE `--requeue-dead`
- PASS `--prune-dead-entity-orphans` for dead entity-keyed rows (mutually exclusive); FIRST try `enrich --operation re-embed --target entities --requeue-dead --ignore-backoff`; then `sqlite-graphrag enrich --prune-dead-entity-orphans --json`
- KNOW dead-letter has `error_class`, `next_retry_at`, terminal `dead` — Transient (rate-limit, timeout, 5xx, exhausted retry, not-yet-materialized entity) backs off up to `--max-attempts`; HardFailures (validation, parse) go terminal at once; dequeue skips `dead`
- KNOW truncated OpenRouter completions (`finish_reason` = `length`) re-emit with GROWN `max_tokens` before JSON repair — NOT dead-lettered on sight
- KNOW enrich queue is sidecar `.enrich-queue.sqlite` beside the main DB
- STATUS — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --status --json`
- DISTINGUISH — `scan_backlog` = DB candidates a fresh scan WOULD enqueue; `queue_pending` = sidecar computed count; `eligible_now == 0` with `queue_pending > 0` is COOLDOWN; `eligible_now > 0` stuck at state `draining` is deadlock — run `--reset-stale-claims`
- PASS `enrich --reset-stale-claims` for stale `processing` claims after `kill -9`; startup auto-resets; SIGTERM cleans up before exit 19
- UNTIL-EMPTY — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --max-attempts 8 --rest-concurrency 8 --json`


## Write Then Enrich — Two Separate Steps
- TREAT every write as STEP 1 (OpenRouter embed + `--llm-backend none`) then DISTINCT STEP 2 (`enrich`); NEVER chain with `&&`
- REMEMBER step 1 — `echo '{"body":"text","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 --llm-backend none remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --json`
- REMEMBER step 2 codex — `sqlite-graphrag --llm-backend codex --llm-model gpt-5.4-mini enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini --json`
- REMEMBER step 2 claude — `sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- REMEMBER step 2 opencode — `sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- REMEMBER step 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- REMEMBER-BATCH step 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-backend none remember-batch --transaction --json`
- REMEMBER-BATCH step 2 codex — `sqlite-graphrag --llm-backend codex --llm-model gpt-5.4-mini enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini --json`
- REMEMBER-BATCH step 2 claude — `sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- REMEMBER-BATCH step 2 opencode — `sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- REMEMBER-BATCH step 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- INGEST step 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 --llm-backend none ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- INGEST step 2 codex — `sqlite-graphrag --llm-backend codex --llm-model gpt-5.4-mini enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini --json`
- INGEST step 2 claude — `sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- INGEST step 2 opencode — `sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- INGEST step 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- EDIT step 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model perplexity/pplx-embed-v1-0.6b --embedding-dim 384 --llm-backend none edit --name <n> --body-file new.md --json`
- EDIT step 2 codex — `sqlite-graphrag --llm-backend codex --llm-model gpt-5.4-mini enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini --json`
- EDIT step 2 claude — `sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- EDIT step 2 opencode — `sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- EDIT step 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- RESTORE step 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 --llm-backend none restore --name <n> --version 2 --json`
- RESTORE step 2 codex — `sqlite-graphrag --llm-backend codex --llm-model gpt-5.4-mini enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini --json`
- RESTORE step 2 claude — `sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- RESTORE step 2 opencode — `sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- RESTORE step 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- KNOW key resolution prefers `OPENROUTER_API_KEY` env or `config.toml`; NEVER embed the raw key in production command lines


## Parallel Embedding and Enrich
- SCALE embed with `--llm-parallelism N` (JoinSet of N OpenRouter requests, order preserved); SCALE enrich with `--rest-concurrency N` + `--until-empty` (N chat calls; SQLite write stays serial via WAL claim)
- CLAMP `--llm-parallelism` 1..32 and `--rest-concurrency` 1..16; paid models prefer 4..16; `:free` caps ~20 req/min so USE low N; multiple keys do NOT add capacity
- REMEMBER parallel step 1 — `echo '{"body":"...","entities":[...],"relationships":[...]}' | sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --json`
- REMEMBER parallel step 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --rest-concurrency 8 --until-empty --max-runtime 3600 --max-attempts 8 --json`
- REMEMBER-BATCH parallel step 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 12 --llm-backend none remember-batch --transaction --json`
- REMEMBER-BATCH parallel step 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --rest-concurrency 12 --until-empty --max-runtime 3600 --json`
- INGEST parallel step 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 --llm-parallelism 6 --llm-backend none ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- INGEST parallel step 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b:nitro --rest-concurrency 12 --until-empty --max-runtime 7200 --max-attempts 8 --json`
- EDIT parallel step 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none edit --name <n> --body-file new.md --json`
- EDIT parallel step 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --rest-concurrency 8 --until-empty --json`
- RESTORE parallel step 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none restore --name <n> --version 2 --json`
- RESTORE parallel step 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --rest-concurrency 8 --until-empty --json`
- MONITOR with `--status` until `scan_backlog` `queue_pending` `eligible_now` are all 0; `queue_dead` is permanent data debt


## Read-Only OpenRouter Formulas
- INIT — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 init --namespace <ns>`
- RECALL with qwen-8b — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 recall "query" --k 10 --json`
- HYBRID-SEARCH with bge-m3 — `sqlite-graphrag --embedding-backend openrouter --embedding-model baai/bge-m3 --embedding-dim 384 hybrid-search "query" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- DEEP-RESEARCH with text-embedding-3-small — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model openai/text-embedding-3-small --embedding-dim 384 deep-research "question" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies --output /tmp/research.json --json`
- RENAME-ENTITY with perplexity — `sqlite-graphrag --embedding-backend openrouter --embedding-model perplexity/pplx-embed-v1-0.6b --embedding-dim 384 rename-entity --name <old> --new-name <new> --json`
- ENRICH re-embed with openrouter (MUTATION — backfills vectors) — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 enrich --operation re-embed --target all --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --json`
- HYBRID-SEARCH offline — `sqlite-graphrag hybrid-search "query" --k 10 --fallback-fts-only --json`
- TRAVERSE fuzzy — `sqlite-graphrag graph traverse --from <short-alias> --depth 2 --fuzzy --json`
- LINK by ID — `sqlite-graphrag link --from-id <N> --to-id <M> --relation uses --json`
- MERGE self-ref guard — NEVER run `merge-entities --ids 3,12 --into-id 3`; ALWAYS exclude the survivor from `--ids`
- VERIFY OpenRouter embedding catalog — `curl -sS https://openrouter.ai/api/v1/models -H "Authorization: Bearer $OPENROUTER_API_KEY" | jaq -r '.data[].id' | rg -i 'embed'`
- ALWAYS keep `--embedding-dim 384` identical to the write-time dimension; mismatch causes knn exit 11


## Diagnostics and Maintenance
- INIT — `sqlite-graphrag init --namespace <ns>`
- HEALTH — `sqlite-graphrag health --json` for `{integrity_ok, schema_version, vec_*_missing, vec_*_coverage_pct}`; TRIGGER `enrich --operation re-embed --target <target>` when any missing count > 0
- MIGRATE — `migrate --dry-run --json` then `migrate --json`; OPTIMIZE — `optimize --json`; VACUUM — `vacuum --json` after purge
- FTS — `fts check|stats|rebuild --json` when `health.fts_degraded`; VEC — `vec orphan-list --json` then `vec purge-orphan --yes`; `vec stats --json`
- EMBEDDING — `embedding --status --json` with per-table `*_missing`; `pending-embeddings --status --json` then `pending-embeddings process --json`
- SLOTS — `slots status --json`; `slots release --slot-id <N> --yes`; PENDING — `pending list --filter-status queued --json`; `pending show <id>`; `pending cleanup --yes`
- EXPORT — `export --namespace <ns> --type <kind> --json`; STATS — `stats --json` includes `total_memories`
- BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECT — `namespace-detect --json`, `debug-schema --json`, `cache list --json`, `cache clear-models --yes`
- COMPLETIONS — `completions bash|zsh|fish|elvish|powershell`
- SCHEDULE weekly — `purge` → `cleanup-orphans` → `prune-relations --relation mentions` → `vacuum` → `optimize` → `sync-safe-copy`
- IF corruption — `sqlite3 broken.sqlite ".recover" | sqlite3 repaired.sqlite`


## Exit Codes and Retry Strategy
- EXIT 0 success; EXIT 1 validation; EXIT 2 argument parsing; EXIT 3 optimistic lock (reload and retry)
- EXIT 4 not found (without `--fuzzy`, includes Jaro-Winkler/prefix suggestions); EXIT 5 namespace error
- EXIT 6 payload too large — THREE variants report `bytes`/`limit`, `chunks`/`limit`, or `tokens`/`limit`; SPLIT body or reduce tokens
- EXIT 9 duplicate — use `--force-merge`; EXIT 10 database — run `vacuum` plus `health`
- EXIT 11 embedding failure — check backend, dim 384, OAuth/key
- EXIT 13 partial batch — reprocess failed only; EXIT 14 I/O; EXIT 15 busy — widen `--wait-lock`
- EXIT 16 preflight — fix MCP config; NEVER treat as transient
- EXIT 19 SHUTDOWN — retry MANDATORY; partial work discarded; EXIT 20 internal
- EXIT 75 slots/singleton locked — respect cooldown; NEVER retry immediately
- EXIT 77 RAM pressure; EXIT 78 config (OpenRouter key or model missing)
- NEVER ignore non-zero exit; NEVER reprocess full batch after exit 13; NEVER confuse exit 1 with exit 9


## Concurrency
- RESPECT the hard ceiling `2 x nCPUs` for heavy commands — `init`, `remember`, `ingest`, `recall`, `hybrid-search`
- SET `--llm-parallelism N` default 4 on `remember` and `edit`, default 2 on `ingest`, clamp [1, 32]
- KNOW JOB SINGLETON — `enrich` and `ingest --mode codex|claude-code` acquire a per-namespace singleton
- USE `--wait-job-singleton SECS` or `--force-job-singleton` to break a stale lock
- ENABLE `SQLITE_GRAPHRAG_LOW_MEMORY=1` for unitary parallelism (3 to 4 times slower)
- NEVER run `enrich` in parallel against the same database
- KNOW REST enrich concurrency is controlled by `--rest-concurrency`, NOT by launching multiple enrich processes


## Environment Variables
- `SQLITE_GRAPHRAG_DB_PATH` — database path override
- `SQLITE_GRAPHRAG_NAMESPACE` — persistent namespace
- `SQLITE_GRAPHRAG_LLM_BACKEND` — persistent LLM backend
- `SQLITE_GRAPHRAG_LLM_MODEL` — persistent LLM model
- `SQLITE_GRAPHRAG_EMBEDDING_BACKEND` — persistent embedding backend
- `SQLITE_GRAPHRAG_EMBEDDING_MODEL` — persistent OpenRouter embedding model
- `SQLITE_GRAPHRAG_EMBEDDING_DIM` — embedding dimension [8, 4096], default 384
- `OPENROUTER_API_KEY` — OpenRouter API key, zeroized on drop
- `SQLITE_GRAPHRAG_CODEX_BINARY`, `SQLITE_GRAPHRAG_CLAUDE_BINARY`, `SQLITE_GRAPHRAG_OPENCODE_BINARY` — binary path overrides
- `SQLITE_GRAPHRAG_OPENCODE_MODEL`, `SQLITE_GRAPHRAG_OPENCODE_TIMEOUT` — opencode overrides
- `SQLITE_GRAPHRAG_LOW_MEMORY` — enable unitary parallelism
- `SQLITE_GRAPHRAG_LOG_FORMAT` — `json` for log aggregators
- `SQLITE_GRAPHRAG_SKIP_PREFLIGHT` — bypass preflight, EMERGENCIES ONLY
- NEVER rely on `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` for codex/claude; they are FORBIDDEN and abort spawn with exit 1


## Active Rules
- ALWAYS `--json`; ALWAYS `jaq` after capture (NEVER pipe NDJSON; NEVER `jq`); ALWAYS parse `backend_invoked`
- ALWAYS `--embedding-backend openrouter --embedding-model <MODEL> --embedding-dim 384` on embed ops with key via env/config
- ALWAYS `--llm-backend none` on writes; ALWAYS SEPARATE `enrich` with `--mode` + model; NEVER chain with `&&`
- ALWAYS refresh OAuth (`codex login` / claude OAuth) when stale; ALWAYS keep dim 384 (mismatch → knn exit 11)
- ALWAYS shell arrays + quoted joins for dynamic `merge-entities` lists
- PREFER `--from-id`/`--to-id` for numeric link IDs; NEVER pure digits as `--from`/`--to` names
- PREFER `--fuzzy` or NotFound suggestions on short-name traverse; PREFER `--output PATH` + `--quiet` for large deep-research; NEVER `&>`
- NEVER pass API keys to codex/claude (OAuth-only, exit 1); NEVER `--llm-backend codex` on write paths
- NEVER parallel `enrich` on same DB; NEVER write `.sqlite` outside the binary
- NEVER ignore exit 19 (retry MANDATORY) or exit 16 (fix MCP); NEVER openrouter without model+key (exit 78)
- NEVER self-ref merge (`--into-id` inside `--ids` / `--into` inside `--names`); NEVER MCP memory or MEMORY.md
- CANONICAL memory types — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- CANONICAL entity types — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- CANONICAL relations — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
