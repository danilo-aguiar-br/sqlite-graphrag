---
name: sqlite-graphrag-en
description: This skill MUST activate for every sqlite-graphrag CLI operation and MUST be used whenever an agent stores, retrieves, searches, enriches, links, merges, migrates or maintains long-term GraphRAG memory inside a local SQLite graph. It teaches the 51 top-level commands, the families graph config fts vec slots pending embedding cache schema completions, the 63 XDG configuration keys, the 74 JSON schema contracts, the agent-native output flags select filter max-items sort dedupe-by count-only truncate-content max-output-bytes, the OpenRouter embedding and text model catalogues with prices, API key storage through config add-key from-stdin, the mandatory separation of write from enrich, embedding fan-out with llm-parallelism, enrich fan-out with rest-concurrency, exit-code branching and failure remediation. Keywords sqlite-graphrag GraphRAG memory embedding openrouter remember remember-batch ingest edit restore recall hybrid-search deep-research enrich re-embed entity-connect force-redescribe link merge-entities purge XDG
---

## When This Skill Activates
- MUST ACTIVATE for remember, save, recall, retrieve, search, persist across sessions
- MUST ACTIVATE for GraphRAG, knowledge graph, entity linking, namespace-scoped memory
- MUST ACTIVATE when sqlite-graphrag, embedding, FTS5, hybrid-search, OpenRouter or entity-connect is mentioned
- MUST ACTIVATE for enrich, re-embed, entity-connect, link, unlink, merge-entities, rename-entity
- MUST ACTIVATE for deep-research, ingest, config, XDG keys, schema contracts, graph maintenance
- MUST ACTIVATE for pending, slots, embedding backlog, fts, vec, vacuum, purge, backup
- NEVER ACTIVATE for ephemeral data, simple file I/O or tasks with no memory component
- ALWAYS load this skill BEFORE inventing ad-hoc memory files, MCP memory servers or Markdown journals


## Core Mental Model
- INVOKE the binary as a one-shot subprocess; there is NO daemon, NO ONNX, NO model cache
- KNOW embedding is HTTP in-process; there is NO subprocess embedding backend
- KNOW TWO selectors only; NEVER invent a third
- SELECTOR 1 is `--embedding-backend`, accepting EXACTLY `auto` or `openrouter`
- SELECTOR 2 is `--llm-backend`, accepting EXACTLY `openrouter` or `none`
- KNOW `auto` degrades to NO EMBEDDING when no key is reachable, writing a vectorless memory with exit 0
- ALWAYS pass `--embedding-backend openrouter` on every write; NEVER rely on `auto`
- KNOW extraction is `enrich --mode openrouter`, the ONLY accepted mode
- KNOW the headless subprocess backends were REMOVED; `--mode codex`, `--mode claude-code` and `--mode opencode` exit 2
- WRITE and ENRICH are SEPARATE processes; write produces vectors, SEPARATE enrich mutates the graph
- NEVER chain write and enrich with `&&`; ALWAYS wait for write exit 0, then run enrich as a DISTINCT process
- KNOW `ingest --enrich-after` is the ONE sanctioned in-process chain, running memory-bindings after all files land
- ALWAYS pass `--json`; ALWAYS parse with `jaq` NEVER `jq`; ALWAYS capture stdout FIRST, then parse
- ALWAYS read the exit code BEFORE parsing stdout; NEVER pipe the CLI straight into `jaq`
- KNOW empty vectors are NEVER persisted; PARSE `backend_invoked` to confirm the transport ran
- KNOW fusion is FTS5 BM25 plus BLOB cosine KNN via RRF over `memory_embeddings`, `entity_embeddings`, `chunk_embeddings`
- NEVER expose the binary as MCP or HTTP; NEVER write the `.sqlite` with another tool


## Contract — Invocation and Parsing
- `--db <PATH>` MUST come AFTER the verb — `sqlite-graphrag remember --db ./g.sqlite --name x ...`
- BEFORE the verb `--db` is REJECTED with exit 2; omitting it silently targets the XDG database
- Graph surfaces REQUIRE `--db`; host leaves `config`, `slots`, `cache`, `completions` accept it as a no-op
- ALWAYS pass `--quiet` in headless pipelines; NEVER merge stderr into JSON with `&>` or `2>&1`
- PRECEDENCE is ALWAYS CLI flag, then XDG `config set` or `config add-key`, then compiled default
- FORBIDDEN product environment variables `SQLITE_GRAPHRAG_*`; the binary does NOT read them on the hot path
- KNOW the default embedding dimensionality is 1024 and an existing database keeps its recorded `schema_meta` dim
- NEVER pass `--embedding-dim` casually; it OVERRIDES the recorded dim and a divergent value kills cosine search
- USE `--embedding-dim` ONLY for a deliberate corpus migration, followed by `enrich --operation re-embed`


## Exit Codes
- 0 success
- 1 validation, timeout, rate limit, provider error, binary not found, and `--no-input` refusal
- 2 invalid arguments, unknown flag, flag in the wrong position, or unaccepted enum value
- 3 optimistic lock conflict — RELOAD and RETRY
- 4 not found — READ the ranked suggestions in the envelope
- 5 namespace error
- 6 payload too large, too many chunks, too many tokens — SPLIT the body
- 9 duplicate — RETRY with `--force-merge`
- 10 database error — RUN `vacuum` then `health`
- 11 embedding failure — CHECK backend, key and dimensionality
- 12 vector extension failure
- 13 partial batch — REPROCESS ONLY the failed lines, NEVER the whole batch
- 14 I/O error
- 15 database busy — WIDEN `--wait-lock`
- 19 shutdown by signal, with the signal name in the envelope — RETRY is MANDATORY
- 20 internal or JSON error
- 75 concurrency slot or job singleton busy — NEVER retry immediately
- 77 insufficient available memory
- 78 configuration failure, typically a missing OpenRouter key or model
- 141 stdout closed by the consumer; identical on Linux, macOS and Windows
- NEVER ignore a non-zero exit; NEVER confuse exit 1 timeout with exit 75


## Global Flags Versus Per-Subcommand Flags
- KNOW the distinction decides POSITION; a per-subcommand flag placed before the verb exits 2
- GLOBAL, written before the verb — `--max-concurrency`, `--wait-lock`, `--fail-on-degraded`, `--lang`, `--tz`
- GLOBAL — `-v`/`-vv`/`-vvv`, `-q`/`--quiet`, `--embedding-dim`, `--embedding-backend`, `--embedding-model`
- GLOBAL — `--llm-backend`, `--llm-model`, `--llm-fallback`, `--llm-max-host-concurrency`
- GLOBAL — `--llm-slot-wait-secs`, `--llm-slot-no-wait`, `--skip-embedding-on-failure`
- GLOBAL — `--openrouter-timeout`, `--openrouter-api-key`, `--no-input`, and the eight agent-native flags
- PER-SUBCOMMAND, written after the verb — `--db`, `--namespace`, `--json`, `--format`, `--limit`
- PER-SUBCOMMAND — `--llm-parallelism`, `--openrouter-model`, `--openrouter-base-url`, `--operation`, `--mode`
- PER-SUBCOMMAND — `--wait-job-singleton`, `--force-job-singleton`, `--low-memory`, `--print-schema`
- `--fail-on-degraded` MAKES a degraded read exit non-zero instead of silently returning FTS-only results with exit 0
- ALWAYS pass `--fail-on-degraded` on `recall`, `hybrid-search` and `deep-research` in agent pipelines
- KNOW a degradation the caller ASKED for with `--fallback-fts-only` is deliberate and NEVER fails
- `--openrouter-timeout <SECONDS>` binds the EMBEDDING client too, not only chat; XDG `llm.openrouter_timeout_secs` default 600
- `--no-input` REFUSES stdin declaratively; `--body-stdin`, `--graph-stdin` and `remember-batch` fail UP FRONT with exit 1
- TURN the `--no-input` XDG opt-in OFF by UNSETTING `cli.no_input`, NEVER by `--no-input=false`


## Agent-Native Output Surface
- PREFER these EIGHT global flags over piping a whole payload into `jaq`; the cut happens BEFORE serialization
- `--select <KEYS>` keeps only these comma-separated keys per element; dotted paths work; `--fields` is the same flag
- KNOW a missing key is SKIPPED, never emitted as `null`; an envelope without a result array is projected itself
- `--filter <EXPR>` accepts `key=value`, `key!=value`, `key~substring`; `==` is a synonym of `=`
- REPEAT `--filter` to conjoin with AND; a malformed expression exits 2 so a typo is NEVER an empty result set
- `--max-items N` caps EMITTED elements across EVERY array in the envelope, and reports `agent_surface.secondary_capped`
- KNOW `--max-items` is DISTINCT from `--limit` and `-k`, which bound the QUERY, not the output
- `--sort <KEY>` sorts ascending by dotted path; numbers compare numerically; elements lacking the key stay at the END
- `--dedupe-by <KEY>` drops later elements repeating the value; elements lacking the key are ALWAYS kept
- `--count-only` replaces the payload with `{"count": N}`, counted AFTER filter, dedupe and max-items
- `--truncate-content N` shortens strings past N CHARACTERS never bytes; a UTF-8 sequence is NEVER split
- `--max-output-bytes N` caps the envelope by DROPPING trailing elements, NEVER by slicing JSON text
- ORDER is FIXED — filter, sort, dedupe, max-items, select, count-only, truncate-content, max-output-bytes
- NEVER assume `--filter` hides a failure; an envelope with `error: true` or `ok: false` ALWAYS reaches the caller
- ALWAYS parse `agent_surface` when a knob is set — `input_count`, `output_count`, `content_truncated`, `output_truncated`, `dropped`
- KNOW truncation raises the top-level `truncated` flag and is NEVER silent
- KNOW the result array is located by `results`, `items`, `entities`, `memories`, `hits`, `rows`, `matches`, `data`, in that order
- KNOW `$schema` documents pass through untouched and NDJSON streams bypass the surface entirely


## Full Command Catalog
- TOP-LEVEL, 51 verbs — `init` `remember` `remember-batch` `ingest` `recall` `read` `list` `forget` `purge` `rename` `split-body` `edit` `history` `restore` `hybrid-search` `health` `migrate` `namespace-detect` `optimize` `stats` `sync-safe-copy` `backup` `vacuum` `link` `unlink` `deep-research` `related` `graph` `export` `fts` `vec` `prune-relations` `prune-ner` `slots` `pending` `embedding` `pending-embeddings` `cleanup-orphans` `memory-entities` `cache` `delete-entity` `reclassify` `rename-entity` `merge-entities` `enrich` `reclassify-relation` `normalize-entities` `schema` `completions` `config` `help`
- KNOW `debug-schema` still works but is HIDDEN from `--help`; USE it to dump the live database schema
- `graph` family — `graph traverse` `graph stats` `graph entities` `graph recompute-degree`
- `config` family — `config add-key` `config list-keys` `config remove-key` `config doctor` `config path` `config set` `config get` `config list` `config unset`
- `fts` family — `fts rebuild` `fts check` `fts stats`
- `vec` family — `vec orphan-list` `vec purge-orphan` `vec stats`
- `slots` family — `slots status` `slots release` `slots cleanup`
- `pending` family — `pending list` `pending show` `pending cleanup`
- `embedding` family — `embedding status` `embedding list` `embedding abandon`
- `pending-embeddings` family — `list` `status` `abandon`, aliases of `embedding`
- `cache` family — `cache clear-models` `cache list` `cache stats`
- `completions` — `bash|zsh|fish|elvish|powershell`
- `schema` emits 74 NDJSON lines of `{"id","invoke"}`; `schema --name <ID>` emits that JSON Schema; unknown ID exits 4
- KNOW `$schema` documents are EXEMPT from the agent-native surface, so any global flag chains safely


## OpenRouter Key Setup
- ADD the key through stdin — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- VERIFY with `sqlite-graphrag config list-keys --json` then `sqlite-graphrag config doctor --json`
- REMOVE with `sqlite-graphrag config remove-key <fingerprint> --json`; LOCATE with `sqlite-graphrag config path`
- SET the endpoints when a proxy is required — `config set network.openrouter.chat_url https://openrouter.ai/api/v1/chat/completions`
- SET the embedding endpoint — `config set network.openrouter.embeddings_url https://openrouter.ai/api/v1/embeddings`
- KNOW keys live in `~/.config/sqlite-graphrag/config.toml` with mode 600, zeroized on drop and NEVER logged
- NEVER pass `--openrouter-api-key` in production shell history; ALWAYS prefer `config add-key --from-stdin`
- ALWAYS run `config doctor` after adding a key and before any paid call; a missing key or model exits 78


## OpenRouter Embedding Models
- PASS `--embedding-model <MODEL>` whenever `--embedding-backend openrouter`; there is NO default and omission exits 78
- INSPECT the live catalogue with the stored key; prices below are indicative USD per million tokens
- `nvidia/llama-nemotron-embed-vl-1b-v2:free` FREE, rate limited near 20 requests per minute
- `qwen/qwen3-embedding-4b` 0.05
- `qwen/qwen3-embedding-8b` 0.05, the DEFAULT operational choice
- `openai/text-embedding-3-small` 0.05
- `perplexity/pplx-embed-v1-0.6b` 0.05
- `baai/bge-m3` 0.05
- `mistralai/mistral-embed-2312` 0.10
- `google/gemini-embedding-2` 0.12
- `openai/text-embedding-3-large` 0.13
- `google/gemini-embedding-005` 0.15
- KNOW Matryoshka truncation happens SERVER-SIDE to the active dimensionality
- KNOW openrouter propagates to EVERY embed path — `remember` `remember-batch` `ingest` `edit` `restore` `split-body` `recall` `hybrid-search` `deep-research` `rename-entity` `init` `enrich`


## OpenRouter Text Models
- KNOW text models serve extraction and enrichment ONLY, NEVER embedding
- PASS `--openrouter-model <MODEL>` after the `enrich` verb; it is MANDATORY and omission fails before any network call
- `deepseek/deepseek-v4-flash`
- `deepseek/deepseek-v4-flash:nitro`, the DEFAULT operational choice for throughput
- `deepseek/deepseek-v4-pro`
- `google/gemini-3.1-flash-lite`
- `minimax/minimax-m3`
- `minimax/minimax-m2.7`
- `minimax/minimax-m2.7:nitro`
- `openai/gpt-oss-120b`
- `openai/gpt-oss-120b:nitro`
- `xiaomi/mimo-v2.5`
- `xiaomi/mimo-v2.5-pro`
- `z-ai/glm-5.2`
- `z-ai/glm-5.2:nitro`
- KNOW `:nitro` selects the fastest provider at a higher price
- VERIFY strict `json_schema` support BEFORE production; without Structured Outputs OpenRouter returns an explicit error
- CONFIRM real spend by parsing `usage.cost` in the enrich envelope


## XDG Configuration Registry
- READ the live registry with `sqlite-graphrag config doctor --json | jaq -r '.knobs[].key'`; it holds 63 keys
- SET any key with `config set <key> <value>`; READ with `config get <key>`; CLEAR with `config unset <key>`
- LIST stored values with `config list --json` and resolved values with `config list --effective --json`
- Agent surface — `agent_surface.max_items` 0, `agent_surface.max_output_bytes` 0, `agent_surface.truncate_content` 0
- Cache and CLI — `cache.dir`, `cli.max_instances`, `cli.no_input` false, `cli.stdin_timeout_secs` 60
- Database — `db.busy_base_delay_ms` 300, `db.busy_retries` 5, `db.path`, `db.query_timeout_ms` 5000
- Display and locale — `display.tz` UTC, `i18n.lang` en
- Embedding — `embedding.backend`, `embedding.model`, `embedding.dim` 1024, `embedding.batch_size` 32
- Embedding cache — `embedding.entity_cache_max_entries` 10000, `embedding.entity_cache_ttl_secs` 3600, `embedding.timeout_secs` 300
- Enrich pacing — `enrich.circuit_breaker_reset_secs` 60, `enrich.rate_limit_deadline_secs` 3600, `enrich.yield_every_n_items` 10
- Enrich batching — `enrich.reembed_claim_batch` 32, `enrich.scan_page_size` 512
- Entity connect — `enrich.entity_connect.default_limit` 100, `enrich.entity_connect.large_ns_limit` 25
- Entity descriptions — `enrich.entity_description.corpus_top_k` 5, `enrich.entity_description.domain` auto, `enrich.entity_description.grounding_threshold` 0.12
- Entity descriptions — `enrich.entity_description.min_corpus_chars` 40, `enrich.entity_description.quality_sample` 50, `enrich.entity_description.snippet_chars` 400
- Ingest and limits — `ingest.low_memory` false, `limits.max_entities_per_memory` 50, `limits.max_relations_per_memory` 50
- LLM transport — `llm.backend`, `llm.model`, `llm.fallback` none, `llm.openrouter_timeout_secs` 600, `llm.probe_timeout_ms` 800
- LLM slots — `llm.max_host_concurrency`, `llm.slot_wait_secs` 300, `llm.slot_no_wait` false, `llm.worker_rss_mb` 350, `llm.skip_embedding_on_failure` false
- Logging — `log.format` pretty, `log.level` warn, `log.retention_days` 7, `log.rotation` daily, `log.to_file` false
- Namespace — `namespace.default` global
- Network — `network.chat_url`, `network.embed_url`, `network.openrouter.chat_url`, `network.openrouter.embeddings_url`
- Parallelism — `parallelism.embed_runtime_threads`, `parallelism.max_total_workers` 64, `parallelism.rayon_threads`
- Search — `search.hybrid.max_graph_results` 50
- Runtime — `retry.disable` false, `shutdown.ignore` false, `system.max_load_per_ncpu` 2.0
- NEVER declare a key outside this registry; an unknown key exits 1 with a did-you-mean suggestion


## Write Step 1 — Embedding Formulas
- DEFINE the write prefix W as `sqlite-graphrag --embedding-backend openrouter --embedding-model <EMB> --llm-backend none`
- USE `<EMB>` = `qwen/qwen3-embedding-8b` by default, or `nvidia/llama-nemotron-embed-vl-1b-v2:free` for the free path
- SCALE embedding with `--llm-parallelism N` written AFTER the verb, clamped to 1..32
- KNOW ONLY `remember`, `remember-batch`, `ingest`, `edit` and `enrich` declare it; on `restore` or `split-body` it exits 2
- KNOW the fan-out engages only above roughly 32 texts; a single item is serial by construction
- SCALE file-level ingestion separately with `--ingest-parallelism N`, which is DISTINCT from the embedding fan-out
- REMEMBER — `echo '{"body":"text","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | W remember --db ./g.sqlite --name <n> --type decision --description "desc" --graph-stdin --force-merge --llm-parallelism 16 --json`
- CHOOSE exactly ONE body source — `--body` inline, `--body-file`, `--body-stdin` or `--graph-stdin`; `--graph-file` COMBINES with any of the first three
- REMEMBER hot set — ADD `--enqueue-enrich` to queue entity-descriptions for the entities linked in this call
- REMEMBER extras — `--strict-name`, `--replace-graph` with `--force-merge`, `--dry-run`, `--enable-ner`, `--metadata`, `--metadata-file`, `--session-id`, `--expected-updated-at`, `--entities-file`, `--relationships-file`, `--clear-body`, `--max-rss-mb`
- REMEMBER-BATCH — `W remember-batch --db ./g.sqlite --transaction --llm-parallelism 16 --json` reading NDJSON on stdin
- KNOW every create line MUST carry a non-empty `description` and a `type`; ADD `--fail-fast` to stop at the first bad line
- INGEST — `W ingest --db ./g.sqlite ./docs --mode none --recursive --pattern "*.md" --type document --llm-parallelism 16 --json`
- KNOW `ingest --mode` accepts ONLY `none`; `--resume` and `--retry-failed` were REMOVED with the LLM-curated queue
- INGEST extras — `--ingest-parallelism N` default `max(1, cpus/2).min(4)`, `--low-memory`, `--max-files`, `--max-cost-usd`, `--auto-describe`, `--no-auto-describe`, `--name-prefix`, `--max-name-length`, `--force-merge` deduplicating by `body_hash`
- INGEST one-process chain — ADD `--enrich-after` to run memory-bindings once all files are ingested
- EDIT — `W edit --db ./g.sqlite --name <n> --body-file new.md --json`, or `--description`, `--memory-type`, `--force-reembed`
- EDIT under contention — PASS `--expected-updated-at <ts>`; exit 3 means RELOAD and RETRY
- RESTORE — `W restore --db ./g.sqlite --name <n> --version <N> --json`
- SPLIT-BODY — `W split-body --db ./g.sqlite --name <N> --json`, or `--batch --threshold 25000` for every oversized body
- KNOW split daughters are NOT embedded inline; they REQUIRE a separate `enrich --operation re-embed --target memories`
- RESPECT 512000 bytes and 512 chunks per body; NEVER mix body sources; NEVER `fd | xargs remember`, USE `ingest`
- VALID memory `--type` — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- REQUIRED graph-stdin entity allowlist — ONLY `name`, `entity_type` with `type` folded as an alias, and optional `description`
- FORBIDDEN in graph-stdin — `observations`, `aliases` and free-form extras, which exit 1
- PARSE every remember envelope for `entities_created[]` and `enrich_recommended[]`; NEVER ignore either


## Enrich Step 2 — Formulas
- RUN enrich as a DISTINCT process only after the write returned exit 0
- BIND — `sqlite-graphrag enrich --db ./g.sqlite --operation memory-bindings --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --rest-concurrency 16 --until-empty --max-runtime 3600 --max-attempts 8 --json`
- DESCRIBE — `sqlite-graphrag enrich --db ./g.sqlite --operation entity-descriptions --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --entity-names jwt,auth-svc --force-redescribe --rest-concurrency 16 --json`
- CONNECT dry run — `sqlite-graphrag enrich --db ./g.sqlite --operation entity-connect --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --dry-run --limit 50 --json`
- CONNECT drain — the same with `--until-empty --max-runtime 600 --rest-concurrency 16` instead of `--dry-run`
- CONNECT anchored — ADD `--anchor-memory <name>` or `--entity-names a,b` to scope the pair scan
- BRIDGE — the same formulas with `--operation cross-domain-bridges`
- RE-EMBED — `W enrich --db ./g.sqlite --operation re-embed --target all --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --until-empty --rest-concurrency 16 --json` then `health --json`
- STATUS without any LLM call — `sqlite-graphrag enrich --db ./g.sqlite --status --quality-sample 50 --json`
- RECOVER dead — `... --list-dead --json` then `... --requeue-dead --json`
- RECOVER skipped — `... --list-skipped --json` then `... --requeue-skipped --json`
- SCALE with `--rest-concurrency N` clamped 1..16 inside ONE process; paid models MUST use 4 to 16
- KNOW `--llm-parallelism` is IGNORED on enrich in openrouter mode; `--rest-concurrency` is the ONLY fan-out knob there
- NEVER launch N enrich processes against one database; the job singleton REJECTS the second with exit 75
- WAIT for a stale singleton with `--wait-job-singleton SECS` or override with `--force-job-singleton`


## Enrich Pipeline Rules
- PASS `--operation` and `--mode` together for every LLM operation; omitting `--mode` exits 2
- EXEMPT read-only inspectors from `--mode` — `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` and `--dry-run`
- PERSISTING operations — `memory-bindings`, `augment-bindings`, `entity-descriptions`, `body-enrich`, `body-extract`, `re-embed`, `weight-calibrate`, `relation-reclassify`, `entity-connect`, `entity-type-validate`, `description-enrich`, `cross-domain-bridges`, `domain-classify`, `deep-research-synth`
- SCAN and REPORT only — `graph-audit`, which NEVER mutates structure
- KNOW `augment-bindings` REQUIRES `--memory-names`, `--names` or `--names-file`
- PREFER `--entity-names` for entity-keyed operations and `--memory-names` for memory-keyed ones; `--names` is a compatibility alias
- STOP when an empty match surfaces `matched=0` plus a `hint`; NEVER widen blindly
- PASS `--target memories|entities|chunks|all` on `re-embed` ONLY, defaulting to `memories`
- KNOW re-embed eligibility is BLOB length `LENGTH(embedding)=dim*4`, not the `dim` column alone
- KNOW claim, `--resume`, `--retry-failed` and `--until-empty` are scoped to this operation AND this namespace ONLY
- KNOW `--force-redescribe` reopens matching `skipped` and `done` rows once per process, and NEVER reopens `dead`
- KNOW low-quality markers are COMPOUND phrases only; a bare domain phrase alone MUST NOT trigger redescription
- READ `--status` for `scan_backlog`, `queue_pending`, `queue_dead`, `eligible_now`, `waiting`, `quality_pct` and `state`
- DISTINGUISH `scan_backlog`, the database candidates a fresh scan would enqueue, from `queue_pending`, the sidecar count
- KNOW `eligible_now == 0` with `queue_pending > 0` is COOLDOWN, not a stall
- KNOW `state` is `draining`, `cooldown`, `pending-scan` or `blocked_dead`; clear `blocked_dead` by requeue or prune FIRST
- RESET a stuck `draining` claim with `--reset-stale-claims` after a `kill -9`
- KNOW the queue is the sidecar `.enrich-queue.sqlite`, and truncated completions re-emit with a GROWN token budget
- KNOW entity-connect persists verdicts in `entity_connect_seen`, keyed `pair:{id1}:{id2}`, scanning co-occurrence in O(k) and NEVER a full Cartesian product
- PARSE `budget_exhausted` and `preempted_for_gate`; the first is a runtime budget end, the second a deliberate yield
- PASS `--preflight-check` to ping the provider before a paid drain, aborting early instead of burning turns on a closed rate-limit window
- PASS `--ignore-backoff` to process items still inside their `next_retry_at` cooldown, which `--status` reports under `waiting`
- TUNE body-enrich with `--min-output-chars` default 500, `--max-output-chars` default 2000 and `--prompt-template <PATH>`
- TUNE entity descriptions inline with `--entity-description-domain` and `--entity-description-grounding-threshold`
- ORDER long runs as memory-bindings, then entity-descriptions, then entity-connect, or PASS `--ops-gate` to enforce that order


## Read and Search Formulas
- DEFINE the read prefix R as `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --fail-on-degraded`
- USE the three-layer pattern — `hybrid-search`, then `read --name`, then `related` or `graph traverse`
- HYBRID-SEARCH — `R hybrid-search --db ./g.sqlite "query" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- HYBRID tuning — `--weight-vec 1.0 --weight-fts 1.0`, `--type <kind>`, `--max-graph-results N`
- HYBRID offline — `sqlite-graphrag hybrid-search --db ./g.sqlite "query" --k 10 --fallback-fts-only --json`
- RECALL — `R recall --db ./g.sqlite "query" --k 10 --json`; ADD `--no-graph`, `--precise`, `--max-distance <f>`, `--all-namespaces`
- DEEP-RESEARCH — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --fail-on-degraded deep-research --db ./g.sqlite "question" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies -o /tmp/dr.json --json`
- DEEP-RESEARCH manual control — `--sub-query-strategy manual --sub-queries-file PATH`
- DEEP-RESEARCH tuning — `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--max-cost-usd`, `--timeout`
- WRITE large envelopes with `-o PATH`; PARSE the acknowledgement `{written, bytes, blake3}`; the file MUST exist non-empty after exit 0
- READ — `sqlite-graphrag read --db ./g.sqlite --name <kebab> --json`; ADD `--with-graph`; USE `--format raw` for the body alone
- LIST — `sqlite-graphrag list --db ./g.sqlite --type <kind> --limit N --offset N --json`; ADD `--include-deleted`
- HISTORY — `sqlite-graphrag history --db ./g.sqlite --name <n> --diff --json`
- RELATED — `sqlite-graphrag related --db ./g.sqlite <name> --hops 2 --relation uses --json`
- MEMORY-ENTITIES — `sqlite-graphrag memory-entities --db ./g.sqlite --name <memory> --json`, then parse `entities[].description`
- RENAME-ENTITY on the embed path — `R rename-entity --db ./g.sqlite --name <old> --new-name <new> --json`
- PARSE `recall` results as `{name, snippet, distance, score, source}`
- PARSE `hybrid-search` results as `{name, combined_score, vec_rank, fts_rank}` and read `graph_matches[]` as well
- PARSE `deep-research` as `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context` and `stats`
- NEVER confuse `distance` with `combined_score`; NEVER raise hops without reading `graph stats` first


## Entity Graph
- LINK — `sqlite-graphrag link --db ./g.sqlite --from <a> --to <b> --relation uses --weight 0.8 --create-missing --json`
- LINK by identifier — `link --from-id <N> --to-id <M> --relation uses --json`; NEVER pass pure digits as names
- LINK strictly — ADD `--strict-relations` to reject any relation outside the canonical set
- UNLINK — `unlink --from <a> --to <b> --relation <type>`, or `--entity <name> --all`, or `--memory <m> --entity <e>`
- TRAVERSE — `sqlite-graphrag graph traverse --db ./g.sqlite --from <root> --depth 2 --json`; ADD `--fuzzy` for short ambiguous names
- KNOW that without `--fuzzy` a miss exits 4 carrying ranked suggestions; ALWAYS use them instead of guessing
- LIST entities — `graph entities --db ./g.sqlite --json`, reading `.entities[]` and NEVER `.items[]`
- ORDER entities with `--sort-by name|degree|created-at` plus `--order asc|desc`, and paginate with `--limit` and `--offset`
- TYPE auto-created entities with `--entity-type` on `link --create-missing`, which otherwise defaults to `concept`
- FILTER the entity listing with `graph entities --entity-type person` against the 13 canonical types
- EXPORT — `graph --format json|dot|mermaid|ndjson --output <path>`; MEASURE with `graph stats --json`
- RECOMPUTE — `graph recompute-degree --json` after any delete, merge or prune, because degree is NOT automatic
- MERGE — `merge-entities --names "a,b,c" --into <target> --json`, or `--ids 12,17 --into-id 3`
- NEVER put `--into-id` inside `--ids`, nor `--into` inside `--names`; a self-referential merge is REJECTED before any database work
- PASS `--cross-namespace` on merge ONLY when crossing namespaces is intentional
- DELETE — `delete-entity --name <n> --cascade --json`; RENAME — `rename-entity --name <old> --new-name <new>` or `--id <N>`
- RECLASSIFY — `reclassify --name <n> --new-type <kind>`, or `--from-type <old> --to-type <new> --batch`
- RECLASSIFY relations — `reclassify-relation --from-relation <old> --to-relation <new> --batch`, with `--literal-from` and `--literal-to` for verbatim matching
- PRUNE — `prune-relations --relation mentions --dry-run` then repeat with `--yes`; `normalize-entities --yes`; `prune-ner --all --yes`
- CANONICAL relations — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- MAP non-canonical relations — `adds` and `creates` to `causes`, `implements` to `supports`, `blocks` to `contradicts`, `tested-by` to `related`, `part-of` to `applies-to`
- CANONICAL entity types — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- VALIDATE entity names as lowercase ASCII kebab-case, at least 2 characters, no newlines, no short all-caps, never pure digits
- NEVER use `mentions` as a default relation; graph writes are ADDITIVE with no degree ceiling


## Maintenance and Diagnostics
- HEALTH — `sqlite-graphrag health --db ./g.sqlite --json` for `integrity_ok`, `schema_version`, `vec_*_missing`, `vec_*_coverage_pct` and `embedding_key`
- TRIGGER `enrich --operation re-embed` whenever any `vec_*_missing` exceeds zero
- MIGRATE — `migrate --dry-run --json` then `migrate --json`; OPTIMIZE — `optimize --json`
- FTS — `fts check --json`, `fts stats --json`, `fts rebuild --json` when the index is degraded
- VEC — `vec orphan-list --json`, then `vec purge-orphan --yes`, and `vec stats --json`
- EMBEDDING backlog — `embedding status --json`, `embedding list --json`, `embedding abandon`; `pending-embeddings` is the alias family
- SLOTS — `slots status --json`, `slots release --slot-id <N> --yes`, `slots cleanup --yes`
- PENDING — `pending list --json`, `pending show <id>`, `pending cleanup --yes`
- FORGET soft — `forget --name <n> --json`; PURGE hard — `purge --db ./g.sqlite --yes --now --dry-run --json` then repeat without `--dry-run`
- KNOW `purge --yes` alone keeps the 90-day retention; `--now` is the alias of `--retention-days 0`
- FOLLOW purge with `cleanup-orphans --yes` and then `vacuum --json`
- EXPORT — `export --namespace <ns> --type <kind> --json`; MEASURE — `stats --json`
- BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECT — `namespace-detect --json`, `cache list --json`, `cache stats --json`, `cache clear-models --yes`
- INSTALL completions — `completions bash|zsh|fish|elvish|powershell`
- SCHEDULE weekly — purge, then `cleanup-orphans`, then `prune-relations --relation mentions`, then `vacuum`, then `optimize`, then `sync-safe-copy`
- RESPECT the concurrency ceiling of twice the CPU count on `init`, `remember`, `ingest`, `recall` and `hybrid-search`


## Prompt Instruction Rules
- "remember this" — RUN `remember --force-merge` with a curated `--graph-stdin`, then a SEPARATE enrich
- "what do you know about X" — RUN `hybrid-search "X" --k 10 --json` FIRST, then `read --name <name> --json`
- "how is X related to Y" — RUN `graph traverse --from X --depth 2 --json` or `related X --hops 2 --json`
- "deep research on X" — RUN `deep-research "X" --k 20 --max-hops 3 -o PATH --json` with `--quiet`
- "connect isolated entities" — RUN `enrich --operation entity-connect` dry first, then drain, then watch `--status`
- BEFORE any create — RUN `hybrid-search "<name>" --k 5 --json` and USE `--force-merge` on a duplicate
- AFTER any create or update — PARSE `read --name <name> --json` for `{name, description, body_length}`
- AFTER every turn — PERSIST the findings or DECLARE that there is nothing new to persist
- ON a non-zero exit — PARSE `jaq '{code, message, error_class}'` and REPORT the remediation


## Anti-Patterns
- NEVER chain write and enrich with `&&`; the only sanctioned chain is `ingest --enrich-after`
- NEVER put `--db`, `--namespace`, `--json` or `--llm-parallelism` before the verb
- NEVER put `--fail-on-degraded`, `--embedding-backend` or `--embedding-model` after the verb expecting global scope on other verbs
- NEVER merge stderr into JSON with `&>` or `2>&1`; ALWAYS pass `--quiet` and capture stdout alone
- NEVER use `SQLITE_GRAPHRAG_*` as configuration; ALWAYS flag, then XDG, then default
- NEVER call OpenRouter without both a model and a key, which exits 78
- NEVER pass `--embedding-dim` on a corpus already embedded at another dimensionality
- NEVER omit `--embedding-backend openrouter` on a write, because `auto` silently persists a vectorless memory
- NEVER omit `--fail-on-degraded` on an agent read, because a degraded search returns keyword hits with exit 0
- NEVER run multiple enrich processes on one database; scale with `--rest-concurrency` inside ONE process
- NEVER pass `--llm-parallelism` to enrich in openrouter mode, where it is ignored
- NEVER ask for `--mode codex`, `--mode claude-code` or `--mode opencode`; those backends are gone and exit 2
- NEVER use `ingest --resume` or `ingest --retry-failed`; both were removed
- NEVER ignore `entities_created` or `enrich_recommended`; NEVER ignore exit 19, which mandates a retry
- NEVER reprocess a whole batch after exit 13; reprocess ONLY the failed lines
- NEVER reopen `dead` rows with `--force-redescribe`; USE `--requeue-dead`
- NEVER assume `--until-empty` drains every operation; it is scoped to this operation and namespace
- NEVER use MCP memory servers, MEMORY.md or ad-hoc Markdown journals
- NEVER open the `.sqlite` with the `sqlite3` shell or any editor
