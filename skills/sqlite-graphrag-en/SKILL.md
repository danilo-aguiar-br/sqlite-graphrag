---
name: sqlite-graphrag-en
description: This skill MUST activate for every sqlite-graphrag CLI operation and MUST be used whenever an agent stores, recalls, searches, enriches, links, merges or maintains long-term GraphRAG memory in a local SQLite graph. It teaches the 50 commands, the graph config fts vec slots embedding cache schema completions families, the 70 XDG configuration keys, the 76 JSON Schema contracts, the agent-native flags select filter max-items sort dedupe-by count-only truncate-content max-output-bytes, the live OpenRouter embedding and text model catalogues, key storage via config add-key from-stdin, the mandatory split between write and enrich, parallelism via llm-parallelism and rest-concurrency, the joint worker ceiling, orchestrating this CLI from codex claude-code and opencode in headless mode, exit-code branching and failure remediation. Keywords sqlite-graphrag GraphRAG memory embedding openrouter remember remember-batch ingest edit restore recall hybrid-search deep-research enrich re-embed entity-connect link merge-entities purge XDG headless
---

## When This Skill Activates
- MUST ACTIVATE for remember, recall, search, save, persist memory across sessions
- MUST ACTIVATE for GraphRAG, knowledge graph, entities, relations, namespace
- MUST ACTIVATE when sqlite-graphrag, embedding, FTS5, hybrid-search, deep-research or OpenRouter is mentioned
- MUST ACTIVATE for enrich, re-embed, entity-connect, link, merge-entities, ingest, config, XDG keys
- MUST ACTIVATE to orchestrate this CLI from codex, claude code or opencode in headless mode
- NEVER ACTIVATE for ephemeral data or file I/O with no memory component
- ALWAYS load this skill BEFORE inventing an ad-hoc memory file, a memory MCP or a Markdown journal


## Core Mental Model
- RUN the binary as a one-shot subprocess; there is NO daemon, NO ONNX and NO model cache
- KNOW that embedding is in-process HTTP; there is NO subprocess embedding backend
- KNOW that there are ONLY TWO selectors and NEVER invent a third
- `--embedding-backend` accepts EXACTLY `auto` or `openrouter`
- `--llm-backend` accepts EXACTLY `openrouter` or `none`
- KNOW that `auto` degrades to NO EMBEDDING when no key is reachable, writing a vectorless memory with exit 0
- ALWAYS pass `--embedding-backend openrouter` on every write; NEVER trust `auto`
- KNOW that `enrich --mode` accepts ONE SINGLE value, `openrouter`, which is pure REST
- KNOW that NO mode spawns a local CLI; `--mode codex`, `claude-code` and `opencode` exit 2
- WRITING and ENRICHING are SEPARATE processes; the write produces vectors, the enrich mutates the graph
- NEVER chain write and enrich with `&&`; AWAIT exit 0 and run the enrich as a DISTINCT process
- ALWAYS pass `--json`; ALWAYS parse with `jaq` NEVER `jq`; ALWAYS capture stdout before parsing
- ALWAYS READ the exit code BEFORE parsing; NEVER pipe the CLI straight into `jaq`
- KNOW that an empty vector is NEVER persisted and that `backend_invoked` has DIFFERENT semantics per verb
- KNOW that in `remember` it is populated only for a ONE-chunk body, and multi-chunk returns `null` WITH the embedding successful
- KNOW that in `edit` it is populated whenever the re-embed runs, and stays `null` only for an edit that never touches the body
- PROVE the embedding with `embedding status --json` requiring `coverage.memories_missing` equal to zero
- NEVER expose the binary as MCP or HTTP; NEVER write the `.sqlite` with another tool


## Contract — Invocation, Target and Parse
- `--db <PATH>` MUST come AFTER the verb — `sqlite-graphrag remember --db ./g.sqlite --name x ...`
- Before the verb `--db` is REJECTED with exit 2; omitting it targets the XDG database SILENTLY
- Graph surfaces REQUIRE `--db`; `config`, `slots`, `cache` and `completions` accept it as a no-op
- ALWAYS pass `--quiet` in a headless pipeline; NEVER mix stderr into the JSON with `&>` or `2>&1`
- PRECEDENCE is CLI flag, then XDG `config set`, then the compiled default
- The `SQLITE_GRAPHRAG_*` environment variables are FORBIDDEN; the binary does NOT read them on the hot path
- KNOW that the default dimension is 1024 and that an existing database keeps its dim in `schema_meta`
- NEVER pass `--embedding-dim` out of habit; a divergent dim kills cosine search SILENTLY
- USE `--embedding-dim` ONLY in a deliberate migration, followed by `enrich --operation re-embed`
- READ `agent_surface.db_path_source` as `argv`, `xdg` or `default`, and `db_path_resolved` as the opened path
- KNOW that only `argv` is explicit designation; `xdg` and `default` are ambient authority
- PASS `--use-active` to accept the compiled default on purpose; the envelope records `db_path_dispensation`
- READ `discarded_flags` in a failure envelope to learn which of YOUR flags could not be applied


## Exit Codes
- 0 success; 5 namespace error; 14 I/O error; 20 internal or JSON error
- 1 validation, timeout, rate limit, provider error or `--no-input` refusal
- 2 invalid argument, unknown flag, flag in the WRONG POSITION or unaccepted enum
- 3 optimistic lock conflict — RELOAD and RETRY
- 4 not found — READ the ranked suggestions in the envelope instead of guessing
- 6 payload, chunks or tokens in excess — SPLIT the body
- 9 duplicate — RETRY with `--force-merge`; 10 database error — RUN `vacuum` then `health`
- 11 embedding failure — CHECK backend, key, model and dimension; 12 vector extension failure
- 13 partial batch — REPROCESS ONLY the failed lines, NEVER the whole batch
- 15 database busy — RAISE `--wait-lock`; 77 out of memory; 141 stdout closed
- 19 signal shutdown, with the signal name in the envelope — the RETRY is MANDATORY
- 75 slot or singleton job busy — NEVER retry immediately
- 78 configuration, typically a missing or misspelled OpenRouter key or model
- NEVER ignore a non-zero exit; NEVER confuse the exit 1 timeout with exit 75
- ON non-zero exit — PARSE `jaq -c '{code, message, error_class}'` and REPORT the remediation


## Global Flags Versus Per-Subcommand Flags
- KNOW that this distinction decides POSITION; a per-subcommand flag before the verb exits 2
- GLOBAL, before the verb — `--max-concurrency`, `--wait-lock`, `--fail-on-degraded`, `--lang`, `--tz`, `--no-input`
- GLOBAL — `-v`/`-vv`/`-vvv`, `-q`/`--quiet`, `--embedding-dim`, `--embedding-backend`, `--embedding-model`
- GLOBAL — `--llm-backend`, `--llm-model`, `--llm-fallback`, `--llm-max-host-concurrency`, `--skip-embedding-on-failure`
- GLOBAL — `--llm-slot-wait-secs`, `--llm-slot-no-wait`, `--openrouter-timeout`, `--openrouter-api-key`, plus the eight agent-native
- PER-SUBCOMMAND, after the verb — `--db`, `--namespace`, `--json`, `--format`, `--limit`, `--low-memory`
- PER-SUBCOMMAND — `--llm-parallelism`, `--openrouter-model`, `--openrouter-base-url`, `--operation`, `--mode`
- PER-SUBCOMMAND — `--wait-job-singleton`, `--force-job-singleton`, `--print-schema`
- `--fail-on-degraded` makes a degraded read exit non-zero instead of returning an FTS-only result with exit 0
- ALWAYS pass `--fail-on-degraded` in agent `recall`, `hybrid-search` and `deep-research`
- KNOW that degradation REQUESTED with `--fallback-fts-only` is deliberate and NEVER fails
- `--openrouter-timeout <SECONDS>` also binds the EMBEDDING client; XDG `llm.openrouter_timeout_secs` defaults to 600
- `--no-input` REFUSES stdin; `--body-stdin`, `--graph-stdin` and `remember-batch` fail UP FRONT with exit 1
- TURN OFF the XDG opt-in of `--no-input` by REMOVING `cli.no_input`, NEVER with `--no-input=false`


## Agent-Native Output Surface
- PREFER these EIGHT global flags to piping the whole payload into `jaq`; the cut happens BEFORE serialization
- `--select <KEYS>` keeps only those keys; dotted paths work; `--fields` is the same flag
- `--filter <EXPR>` accepts `key=value`, `key!=value`, `key~substring`; `==` is a synonym of `=`
- PASS `--filter-scope page|universe` when filtering a PAGINATED command; without it a predicate over a truncated page is REFUSED with exit 2
- `--max-items N` limits EMITTED elements in every array and reports `agent_surface.secondary_capped`
- KNOW that `--max-items` is DISTINCT from `--limit` and from `-k`, which limit the QUERY and not the output
- `--sort <KEY>` sorts ascending; numbers compare numerically; a missing key goes to the END
- `--dedupe-by <KEY>` drops later repeats; elements without the key are ALWAYS kept
- `--count-only` returns `{"count": N}`, counted AFTER filter, dedupe and max-items
- `--truncate-content N` cuts strings by CHARACTER, never by byte, and NEVER splits UTF-8
- `--max-output-bytes N` limits the envelope by DROPPING trailing elements, NEVER by slicing the JSON
- The ORDER is FIXED — filter, sort, dedupe, max-items, select, count-only, truncate-content, max-output-bytes
- PARSE `agent_surface` when a knob is active — `input_count`, `output_count`, `content_truncated`, `output_truncated`, `dropped`
- KNOW that the array is located by `results`, `items`, `entities`, `memories`, `hits`, `rows`, `matches`, `data`, in that order


## Full Command Catalogue
- TOP-LEVEL, 50 verbs — `init` `remember` `remember-batch` `ingest` `recall` `read` `list` `forget` `purge` `rename` `split-body` `edit` `history` `restore` `hybrid-search` `health` `migrate` `namespace-detect` `optimize` `stats` `sync-safe-copy` `backup` `vacuum` `link` `unlink` `deep-research` `related` `graph` `export` `fts` `vec` `prune-relations` `prune-ner` `slots` `embedding` `pending-embeddings` `cleanup-orphans` `memory-entities` `cache` `delete-entity` `reclassify` `rename-entity` `merge-entities` `enrich` `reclassify-relation` `normalize-entities` `schema` `completions` `config` `help`
- KNOW that `debug-schema` works but is HIDDEN from `--help`; USE IT for the live database schema
- `graph` family — `traverse` `stats` `entities` `recompute-degree`
- `config` family — `add-key` `list-keys` `remove-key` `doctor` `path` `set` `get` `list` `unset`
- `fts` family — `rebuild` `check` `stats`; `vec` family — `orphan-list` `purge-orphan` `stats`
- `slots` family — `status` `release` `cleanup`; `cache` family — `clear-models` `list` `stats`
- `embedding` family — `status` `list` `abandon`, with `pending-embeddings` as an alias family
- `completions` — `bash|zsh|fish|elvish|powershell`
- `schema` emits 76 NDJSON lines `{"id","invoke"}`; `schema --name <ID>` emits the JSON Schema; an unknown ID exits 4


## OpenRouter Key and Model Catalogue
- ADD the key via stdin — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- CHECK with `config list-keys --json`, which returns `provider`, `fingerprint`, `masked_value` and `added_at`
- DIAGNOSE the layers with `config doctor --json` BEFORE any paid call
- REMOVE with `config remove-key <fingerprint> --json`; LOCATE the file with `config path --json`
- NEVER pass `--openrouter-api-key` into shell history; ALWAYS prefer `config add-key --from-stdin`
- KNOW that this CLI has NO catalogue verb; the live list comes from the OpenRouter API over HTTP
- LIST embeddings — `curl -s https://openrouter.ai/api/v1/embeddings/models | jaq -r '.data[].id' | sort`
- READ live pricing — `curl -s https://openrouter.ai/api/v1/embeddings/models | jaq -r '.data[]|"\(.id) \(.pricing.prompt)"'`
- FILTER structured outputs — `curl -s https://openrouter.ai/api/v1/models | jaq -r '.data[]|select(.supported_parameters|index("structured_outputs"))|.id'`
- CENTRAL TRAP — `:nitro` NEVER appears as an id in the catalogue, because it is routing applied at runtime
- NEVER validate a `:nitro` model against the catalogue; the validation REJECTS a model the API accepts
- CONFIRM the model by PROOF and never by catalogue — write a throwaway memory and check the vector coverage


## OpenRouter Embedding Models
- PASS `--embedding-model <MODEL>` whenever you use `--embedding-backend openrouter`; there is NO default and omitting it exits 78
- USE `qwen/qwen3-embedding-8b` as the DEFAULT operational choice
- USE `nvidia/llama-nemotron-embed-vl-1b-v2:free` on the FREE path, respecting the per-minute limit
- OTHER valid ids — `qwen/qwen3-embedding-4b`, `openai/text-embedding-3-small`, `openai/text-embedding-3-large`
- OTHER valid ids — `perplexity/pplx-embed-v1-0.6b`, `baai/bge-m3`, `mistralai/mistral-embed-2312`
- OTHER valid ids — `google/gemini-embedding-2`, `google/gemini-embedding-001`, `voyageai/voyage-4`
- NEVER write `google/gemini-embedding-005`; that id does NOT exist and the call exits 78
- NEVER hardcode pricing in this skill or in a prompt; CONSULT `pricing.prompt`, because pricing changes without notice
- KNOW that the model propagates to EVERY embed path — `remember` `remember-batch` `ingest` `edit` `restore` `split-body` `recall` `hybrid-search` `deep-research` `rename-entity` `init` `enrich`
- NEVER switch models on an already embedded corpus without running `enrich --operation re-embed --target all` right after


## OpenRouter Text Models
- KNOW that a text model serves ONLY extraction and enrichment, NEVER embedding
- PASS `--openrouter-model <MODEL>` AFTER the `enrich` verb; omitting it fails before the network
- USE `deepseek/deepseek-v4-flash:nitro` as the DEFAULT operational choice for throughput
- OTHER ids — `deepseek/deepseek-v4-flash`, `deepseek/deepseek-v4-pro`, `google/gemini-3.1-flash-lite`
- OTHER ids — `minimax/minimax-m3`, `minimax/minimax-m2.7`, `minimax/minimax-m2.7:nitro`
- OTHER ids — `openai/gpt-oss-120b`, `openai/gpt-oss-120b:nitro`, `xiaomi/mimo-v2.5`, `xiaomi/mimo-v2.5-pro`
- OTHER ids — `z-ai/glm-5.2`, `z-ai/glm-5.2:nitro`
- KNOW that `:nitro` picks the fastest provider at a higher price and is NOT listed in the catalogue
- REQUIRE `structured_outputs` support; without it OpenRouter returns an explicit error on extraction
- CONFIRM the real spend by parsing `usage.cost` in the enrich envelope


## XDG Configuration Registry
- READ the live registry with `config doctor --json | jaq -r '.knobs[].key'`; it holds 70 keys
- DEFINE with `config set <key> <value>`; READ with `config get <key>`; CLEAR with `config unset <key>`
- LIST stored values with `config list --json` and resolved ones with `config list --effective --json`
- Agent surface — `agent_surface.max_items` 0, `agent_surface.max_output_bytes` 0, `agent_surface.truncate_content` 0
- Cache and CLI — `cache.dir`, `cli.max_instances`, `cli.no_input` false, `cli.stdin_timeout_secs` 60
- Database — `db.busy_base_delay_ms` 300, `db.busy_retries` 5, `db.path`, `db.query_timeout_ms` 5000
- Display — `display.tz` UTC, `i18n.lang` en; namespace — `namespace.default` global
- Embedding — `embedding.backend`, `embedding.model`, `embedding.dim` 1024, `embedding.batch_size` 32, `embedding.timeout_secs` 300
- Embedding cache — `embedding.entity_cache_max_entries` 10000, `embedding.entity_cache_ttl_secs` 3600
- Enrich pacing — `enrich.circuit_breaker_reset_secs` 60, `enrich.rate_limit_deadline_secs` 3600, `enrich.yield_every_n_items` 10
- Enrich batches — `enrich.reembed_claim_batch` 32, `enrich.scan_page_size` 512
- Entity connect — `enrich.entity_connect.default_limit` 100, `enrich.entity_connect.large_ns_limit` 25
- Descriptions — `enrich.entity_description.corpus_top_k` 8, `.domain` auto, `.grounding_threshold` 0.30, `.neighbour_top_k` 12
- Descriptions — `enrich.entity_description.min_corpus_chars` 40, `.quality_sample` 50, `.snippet_chars` 2000
- Type validation — `enrich.entity_type_validate.corpus_top_k` 8, `.min_corpus_chars` 40, `.neighbour_top_k` 12, `.snippet_chars` 2000
- PARSE `retyped` in the summary to learn how many labels CHANGED; confirmation and abstention land in `skipped` with their `reason`
- Ingest and limits — `ingest.low_memory` false, `limits.max_entities_per_memory` 50, `limits.max_relations_per_memory` 50
- LLM transport — `llm.backend`, `llm.model`, `llm.fallback` none, `llm.openrouter_timeout_secs` 600, `llm.probe_timeout_ms` 800
- LLM slots — `llm.max_host_concurrency`, `llm.slot_wait_secs` 300, `llm.slot_no_wait` false, `llm.worker_rss_mb` 350, `llm.skip_embedding_on_failure` false
- Log — `log.format` pretty, `log.level` warn, `log.retention_days` 7, `log.rotation` daily, `log.to_file` false
- Network — `network.chat_url`, `network.embed_url`, `network.openrouter.chat_url`, `network.openrouter.embeddings_url`
- Parallelism — `parallelism.embed_runtime_threads`, `parallelism.max_total_workers` 64, `parallelism.rayon_threads`
- Search and runtime — `search.hybrid.max_graph_results` 50, `retry.disable` false, `shutdown.ignore` false, `system.max_load_per_ncpu` 2.0
- NEVER declare a key outside this registry; an unknown key exits 1 with a suggestion


## Parallelism and Multiprocessing
- KNOW that there are THREE distinct knobs and that confusing them is the commonest cause of low throughput
- KNOB 1 is `--llm-parallelism N`, AFTER the verb, which opens the EMBEDDING fan-out, clamped 1..32
- KNOW that ONLY `remember`, `remember-batch`, `ingest`, `edit` and `enrich` declare it; on `restore` or `split-body` it exits 2
- KNOB 2 is `--rest-concurrency N`, AFTER `enrich`, clamped 1..16 with default 8
- KNOW that `--rest-concurrency` is the ONLY enrich fan-out knob in openrouter mode
- KNOW that `--llm-parallelism` is INERT in openrouter enrich and merely emits a warning
- KNOB 3 is `--ingest-parallelism N`, which parallelizes FILES in `ingest`, default `max(1, cpus/2).min(4)`
- KNOW that there is a JOINT CEILING invisible in the `--help` of any isolated flag
- COMPUTE the ceiling as `parallelism.max_total_workers` divided by the resolved `--max-concurrency`
- KNOW that `max_total_workers` is 64 by default, so `--max-concurrency 4` leaves 16 permits per process
- KNOW that asking for `--llm-parallelism 32` under `--max-concurrency 8` delivers 8, never 32, with no error at all
- LOWER `--max-concurrency` when you want a HIGH fan-out in one process, because the two share one budget
- NEVER launch N enrich processes against one database; the singleton job REJECTS the second with exit 75
- NEVER launch N `deep-research` processes; LET the verb parallelize its sub-queries internally
- PARALLELIZE safely only concurrent READS, with a low pool, that do not contend for the singleton
- PASS `--wait-lock <SECONDS>` ONE single time to await a slot; NEVER busy-loop over exit 75


## Write Step 1 — Embedding Formulas
- DEFINE prefix W as `sqlite-graphrag --embedding-backend openrouter --embedding-model <EMB> --openrouter-timeout 300 --llm-backend none`
- USE `<EMB>` equal to `qwen/qwen3-embedding-8b`, or `nvidia/llama-nemotron-embed-vl-1b-v2:free` on the free path
- REMEMBER — `echo '{"body":"text","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | W remember --db ./g.sqlite --name <n> --type decision --description "desc" --graph-stdin --force-merge --llm-parallelism 16 --json`
- PICK ONE body source — `--body`, `--body-file`, `--body-stdin` or `--graph-stdin`
- KNOW that `--graph-file` COMBINES with `--body`, `--body-file` or `--body-stdin`, and is the fourth graph source
- REMEMBER extras — `--enqueue-enrich`, `--strict-name`, `--replace-graph`, `--dry-run`, `--enable-ner`, `--metadata`, `--metadata-file`, `--session-id`, `--expected-updated-at`, `--entities-file`, `--relationships-file`, `--clear-body`, `--max-rss-mb`
- REMEMBER-BATCH — `W remember-batch --db ./g.sqlite --transaction --llm-parallelism 16 --json` reading NDJSON on stdin
- KNOW that every creation line REQUIRES a non-empty `description` and a `type`; `--fail-fast` stops at the first bad line
- INGEST — `W ingest --db ./g.sqlite ./docs --mode none --recursive --pattern "*.md" --type document --ingest-parallelism 4 --llm-parallelism 16 --json`
- KNOW that `ingest --mode` accepts ONLY `none`; `--resume` and `--retry-failed` were REMOVED
- INGEST extras — `--low-memory`, `--max-files`, `--max-cost-usd`, `--auto-describe`, `--no-auto-describe`, `--name-prefix`, `--max-name-length`, `--force-merge` deduplicating by `body_hash`, `--enrich-after`
- EDIT — `W edit --db ./g.sqlite --name <n> --body-file new.md --llm-parallelism 16 --json`, or `--description`, `--memory-type`, `--force-reembed`
- KNOW that `edit --body-file` does NOT require a graph source, unlike `remember --body`, which makes `edit` the right verb to append to an existing memory
- EDIT under concurrency — PASS `--expected-updated-at <ts>`; exit 3 means RELOAD and RETRY
- RESTORE — `W restore --db ./g.sqlite --name <n> --version <N> --json`, which RE-EMBEDS the restored body
- SPLIT-BODY — `W split-body --db ./g.sqlite --name <N> --json`, or `--batch --threshold 25000`
- KNOW that split children are NOT embedded inline; they REQUIRE a separate `enrich --operation re-embed --target memories`
- RESPECT 512000 bytes per body and 512 chunks; chunking engages above 8000 characters
- NEVER mix body sources; NEVER do `fd | xargs remember`, USE `ingest`
- `--type` accepts — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- In graph-stdin ONLY `name`, `entity_type` with `type` as an alias, and an optional `description`
- FORBIDDEN in graph-stdin — `observations`, `aliases` and free extras, which exit 1
- ADD `--strict-entity-types` to refuse a type outside the thirteen canonical ones, sibling of `--strict-name`
- PARSE every write envelope looking for `entities_created[]` and `enrich_recommended[]`


## Enrich Step 2 — Formulas
- RUN the enrich as a DISTINCT process only after the write returned exit 0
- DEFINE prefix E as `sqlite-graphrag enrich --db ./g.sqlite --mode openrouter --openrouter-model <TXT> --rest-concurrency 16`
- BIND — `E --operation memory-bindings --until-empty --max-runtime 3600 --max-attempts 8 --json`
- DESCRIBE — `E --operation entity-descriptions --entity-names jwt,auth-svc --force-redescribe --json`
- CONNECT dry run — `E --operation entity-connect --dry-run --limit 50 --json`
- CONNECT drain — the same, swapping `--dry-run` for `--until-empty --max-runtime 600`
- CONNECT anchored — ADD `--anchor-memory <name>` or `--entity-names a,b` to scope the scan
- RE-EMBED — `W enrich --db ./g.sqlite --operation re-embed --target all --mode openrouter --openrouter-model <TXT> --until-empty --rest-concurrency 16 --json` and then `health --json`
- STATUS with no LLM call — `sqlite-graphrag enrich --db ./g.sqlite --status --operation <OP> --quality-sample 50 --json`
- ALWAYS pass `--operation` with `--status`; WITHOUT it the inspector falls back to `memory-bindings` and reports `empty` while another queue is full
- RECOVER — `--list-dead` then `--requeue-dead`; `--list-skipped` then `--requeue-skipped`
- AWAIT a stuck singleton with `--wait-job-singleton SECS` or override it with `--force-job-singleton`


## Enrich Pipeline Rules
- PASS `--operation` and `--mode` together in every LLM operation; omitting `--mode` exits 2
- EXEMPT from `--mode` are the read-only inspectors — `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` and `--dry-run`
- Operations that PERSIST — `memory-bindings`, `augment-bindings`, `entity-descriptions`, `body-enrich`, `body-extract`, `re-embed`, `weight-calibrate`, `relation-reclassify`, `entity-connect`, `entity-type-validate`, `description-enrich`, `cross-domain-bridges`, `domain-classify`, `deep-research-synth`
- SCAN and REPORT only operation — `graph-audit`, which NEVER mutates structure
- KNOW that `augment-bindings` REQUIRES `--memory-names`, `--names` or `--names-file`
- CONTROL `entity-type-validate` with `--allowed-types` and `--on-unknown-type keep|fallback|strict`, `keep` being the default
- PREFER `--entity-names` per entity and `--memory-names` per memory; `--names` is a compatibility alias
- STOP when an empty match shows `matched=0` plus a `hint`; NEVER widen blindly
- PASS `--target memories|entities|chunks|all` ONLY in `re-embed`, defaulting to `memories`
- KNOW that claim, `--resume`, `--retry-failed` and `--until-empty` are scoped to THIS operation and THIS namespace
- KNOW that `--force-redescribe` reopens `skipped` and `done` once per process, and NEVER reopens `dead`
- READ the `operation` field FIRST in `--status`, which declares which queue was measured, and only then `state` and the counts
- READ also `scan_backlog`, `queue_pending`, `queue_dead`, `eligible_now`, `waiting` and `quality_pct`
- KNOW that `eligible_now == 0` with `queue_pending > 0` is COOLDOWN, not a hang
- KNOW that `state` is `draining`, `cooldown`, `pending-scan` or `blocked_dead`; clear `blocked_dead` FIRST with requeue or prune
- PARSE `budget_exhausted`, which is end of budget, and `preempted_for_gate`, which is deliberate yielding
- PASS `--preflight-check` before a paid drain, to abort early in a closed rate-limit window
- PASS `--ignore-backoff` for items in `next_retry_at` cooldown, and `--reset-stale-claims` for a claim stuck after a forced kill
- TUNE body-enrich with `--min-output-chars` 500, `--max-output-chars` 2000 and `--prompt-template <PATH>`
- KNOW that the `body-enrich` preservation gate is `--preserve-threshold` 0.7 by trigram Jaccard, and that `--preserve-check` is INERT in the parser
- TUNE inline descriptions with `--entity-description-domain` and `--entity-description-grounding-threshold`
- ORDER long runs as memory-bindings, entity-descriptions, entity-connect, or PASS `--ops-gate`


## Read and Search Formulas
- DEFINE prefix R as `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model <EMB> --openrouter-timeout 300 --fail-on-degraded`
- USE the three-layer pattern — `hybrid-search`, then `read --name`, then `related` or `graph traverse`
- HYBRID-SEARCH — `R hybrid-search --db ./g.sqlite "query" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- Tuning — `--weight-vec 1.0 --weight-fts 1.0`, `--type <kind>`, `--max-graph-results N`
- HYBRID offline at no cost — `sqlite-graphrag hybrid-search --db ./g.sqlite "query" --k 10 --fallback-fts-only --json`
- RECALL — `R recall --db ./g.sqlite "query" --k 10 --json`; extras `--no-graph`, `--precise`, `--max-distance <f>`, `--all-namespaces`
- DEEP-RESEARCH — `R deep-research --db ./g.sqlite "question" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies -o /tmp/dr.json --json`
- DEEP-RESEARCH tuning — `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--max-cost-usd`, `--timeout`
- Manual control — `--sub-query-strategy manual --sub-queries-file PATH`
- WRITE a large envelope with `-o PATH`; PARSE the ack `{written, bytes, blake3}` and confirm bytes above zero
- READ — `sqlite-graphrag read --db ./g.sqlite --name <kebab> --json`; extras `--with-graph` and `--format raw`
- KNOW that `--no-body` omits the body from the response and that `--show-entities` adds the linked entities
- LIST — `sqlite-graphrag list --db ./g.sqlite --type <kind> --limit N --offset N --include-deleted --json`
- HISTORY — `sqlite-graphrag history --db ./g.sqlite --name <n> --diff --json`
- RELATED — `sqlite-graphrag related --db ./g.sqlite <name> --hops 2 --relation uses --json`
- MEMORY-ENTITIES — `sqlite-graphrag memory-entities --db ./g.sqlite --name <memory> --json`, and parse `entities[].description`
- RENAME-ENTITY on the embed path — `R rename-entity --db ./g.sqlite --name <old> --new-name <new> --json`, which RE-EMBEDS the entity
- PARSE `recall` as `{name, snippet, distance, score, source}`, with `source` in `direct`, `graph` or `fts_fallback`
- PARSE `hybrid-search` as `{name, combined_score, vec_rank, fts_rank}` plus `graph_matches[]`
- PARSE `deep-research` as `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context` and `stats`
- READ `vec_degraded` and `vec_degraded_reason` on every read; when present, the result came from lexical BM25
- NEVER confuse `distance` with `combined_score`; NEVER raise hops without reading `graph stats` first


## Entity Graph
- LINK — `sqlite-graphrag link --db ./g.sqlite --from <a> --to <b> --relation uses --weight 0.8 --create-missing --entity-type concept --json`
- LINK by id — `link --from-id <N> --to-id <M> --relation uses --json`; NEVER pass bare digits as names
- KNOW that `--strength` is an ALIAS of `--weight` in `link`, because the input schema calls the same property `strength`
- LINK strict — ADD `--strict-relations` to reject a relation outside the canonical set
- UNLINK — `unlink --from <a> --to <b> --relation <kind>`, or `--entity <name> --all`, or `--memory <m> --entity <e>`
- TRAVERSE — `sqlite-graphrag graph traverse --db ./g.sqlite --from <root> --depth 2 --fuzzy --json`
- LIST entities — `graph entities --db ./g.sqlite --json`, reading `.entities[]` and NEVER `.items[]`
- ORDER with `--sort-by name|degree|created-at` plus `--order asc|desc`, and paginate with `--limit` and `--offset`
- FILTER with `graph entities --entity-type person` against the 13 canonical types
- EXPORT — `graph --format json|dot|mermaid|ndjson --output <path>`; MEASURE with `graph stats --json`
- RECOMPUTE — `graph recompute-degree --json` after delete, merge or prune, because degree is NOT automatic
- MERGE — `merge-entities --names "a,b,c" --into <target> --json`, or `--ids 12,17 --into-id 3`
- NEVER put `--into-id` inside `--ids` nor `--into` inside `--names`; a self-referential merge is REJECTED
- PASS `--cross-namespace` on a merge ONLY when crossing namespaces is intentional
- DELETE — `delete-entity --name <n> --cascade --json`; memory RENAME — `rename --name <old> --new-name <new> --json`
- Entity RENAME by id — `rename-entity --id <N> --new-name <new> --json`
- RECLASSIFY — `reclassify --name <n> --new-type <kind>`, or `--from-type <old> --to-type <new> --batch`
- RECLASSIFY relations — `reclassify-relation --from-relation <old> --to-relation <new> --batch --literal-from --literal-to`
- PRUNE — `prune-relations --relation mentions --dry-run` and repeat with `--yes`; `normalize-entities --yes`; `prune-ner --all --yes`
- CANONICAL relations — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- MAP non-canonical ones — `adds` and `creates` to `causes`, `implements` to `supports`, `blocks` to `contradicts`, `tested-by` to `related`, `part-of` to `applies-to`
- CANONICAL types — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- VALIDATE an entity name as lowercase ASCII kebab-case, at least 2 characters, no newline, no short all-caps, never digits only
- NEVER use `mentions` as a default relation; graph writes are ADDITIVE and have no degree ceiling


## Maintenance and Diagnostics
- HEALTH — `sqlite-graphrag health --db ./g.sqlite --json` for `integrity_ok`, `schema_version`, `vec_*_missing`, `vec_*_coverage_pct` and `embedding_key`
- FIRE `enrich --operation re-embed` whenever any `vec_*_missing` is above zero
- MIGRATE — `migrate --dry-run --json` and then `migrate --json`; OPTIMIZE — `optimize --json`
- FTS — `fts check --json`, `fts stats --json`, `fts rebuild --json` with a degraded index
- VEC — `vec orphan-list --json`, then `vec purge-orphan --yes`, and `vec stats --json`
- Backlog — `embedding status --json`, `embedding list --json`, `embedding abandon`
- SLOTS — `slots status --json`, `slots release --slot-id <N> --yes`, `slots cleanup --yes`
- Soft FORGET — `forget --name <n> --json`; PURGE — `purge --db ./g.sqlite --yes --now --dry-run --json` and repeat without `--dry-run`
- KNOW that `purge --yes` alone keeps the 90-day retention; `--now` is the alias of `--retention-days 0`
- FOLLOW the purge with `cleanup-orphans --yes` and then `vacuum --json`
- EXPORT — `export --namespace <ns> --type <kind> --json`; MEASURE — `stats --json`
- BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECT — `namespace-detect --json`, `cache list --json`, `cache stats --json`, `cache clear-models --yes`
- INSTALL completions — `completions bash|zsh|fish|elvish|powershell`
- SCHEDULE weekly — purge, `cleanup-orphans`, `prune-relations --relation mentions`, `vacuum`, `optimize`, `sync-safe-copy`


## Headless Orchestration — Codex, Claude Code and OpenCode
- KNOW that the headless CLI is the CALLER and this binary is the CALLEE; NEVER confuse it with `enrich --mode`
- KNOW that embedding is ALWAYS OpenRouter and that the headless CLI NEVER produces a vector
- DEFINE C as `codex exec -m <MODEL> --json --skip-git-repo-check -C <DIR>`
- DEFINE K as `claude -p --model <MODEL> --output-format json --add-dir <DIR>`
- DEFINE O as `opencode run --model <MODEL> --format json --dir <DIR>`
- KNOW that `codex exec` accepts `-s <SANDBOX_MODE>`, `--approve-for-me`, `--dangerously-bypass-approvals-and-sandbox`, `--output-schema` and `-o <FILE>`
- KNOW that `claude -p` accepts `--permission-mode <MODE>`, `--dangerously-skip-permissions` and `--session-id <uuid>`
- KNOW that `opencode run` accepts `--agent`, `--continue`, `--session <id>`, and that `opencode models` lists the models
- STEP 1 embeds with prefix W; STEP 2 enriches with prefix E; NEVER merge the two into a single prompt
- ORDER the caller to RUN step 1, READ the exit and the vector coverage, and ONLY THEN run step 2
- REMEMBER via codex — `C "Run W remember --db ./g.sqlite --name n --type decision --description d --graph-stdin --llm-parallelism 16 --json; confirm exit 0; ONLY THEN run E --operation memory-bindings --until-empty --json"`
- REMEMBER via claude code — the SAME order swapping C for K
- REMEMBER via opencode — the SAME order swapping C for O
- REMEMBER-BATCH via any caller — swap step 1 for `W remember-batch --db ./g.sqlite --transaction --llm-parallelism 16 --json` reading NDJSON
- INGEST via any caller — swap step 1 for `W ingest --db ./g.sqlite ./docs --mode none --recursive --ingest-parallelism 4 --llm-parallelism 16 --json`
- EDIT via any caller — swap step 1 for `W edit --db ./g.sqlite --name n --body-file new.md --llm-parallelism 16 --json`
- RESTORE via any caller — swap step 1 for `W restore --db ./g.sqlite --name n --version N --json`
- With NO caller, straight to OpenRouter — RUN step 1 with W and step 2 with E, each as a DISTINCT process
- PARALLELIZE step 1 with `--llm-parallelism 16` and step 2 with `--rest-concurrency 16`, under the joint ceiling
- NEVER parallelize step 2 by launching processes; the singleton returns exit 75 to the second


## Prompt Instruction Rules
- "remember this" — RUN `remember --force-merge` with a curated `--graph-stdin`, then a SEPARATE enrich
- "append to memory X" — RUN `edit --name X --body-file <file>`, which requires NO graph source
- "what do you know about X" — RUN `hybrid-search "X" --k 10 --json` and then `read --name <name> --json`
- "how does X relate to Y" — RUN `graph traverse --from X --depth 2 --json` or `related X --hops 2 --json`
- "deep research on X" — RUN `deep-research "X" --k 20 --max-hops 3 -o PATH --json` with `--quiet`
- "connect isolated entities" — RUN `enrich --operation entity-connect` dry, then drain, then `--status`
- BEFORE creating — RUN `hybrid-search "<name>" --k 5 --json` and USE `--force-merge` on a duplicate
- AFTER creating or updating — PARSE `read --name <name> --json` looking for `{name, description, body_length}`


## Antipatterns
- NEVER chain write and enrich with `&&`; the only sanctioned chaining is `ingest --enrich-after`
- NEVER put `--db`, `--namespace`, `--json` or `--llm-parallelism` before the verb
- NEVER omit `--embedding-backend openrouter` on a write, because `auto` persists a vectorless memory silently
- NEVER omit `--fail-on-degraded` on an agent read, because a degraded one returns keyword hits with exit 0
- NEVER ask for `--mode codex`, `--mode claude-code` or `--mode opencode`; the only accepted value is `openrouter`
- NEVER treat `--help` text as proof of BEHAVIOR; it advertises `--preserve-check`, which no line reads, and prints `--no-fts-skip-when-functional`, which the parser refuses with exit 2
- NEVER run multiple enrich or deep-research processes on one database; scale INSIDE one process
- NEVER validate a `:nitro` model against the OpenRouter catalogue, which does not list it
- NEVER use `SQLITE_GRAPHRAG_*`, `ingest --resume` or `ingest --retry-failed`; all were removed
- NEVER prove embedding by `backend_invoked` alone; NEVER ignore `entities_created`, `enrich_recommended` or exit 19
- NEVER use a memory MCP, MEMORY.md or an ad-hoc Markdown journal
- NEVER open the `.sqlite` with the `sqlite3` shell or with an editor
