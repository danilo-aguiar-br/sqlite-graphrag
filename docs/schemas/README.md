# Machine-Readable JSON Schemas


## English
### Purpose
- Each file in this directory is a JSON Schema Draft 2020-12 document
- Output schemas describe the exact stdout contract of every `sqlite-graphrag` subcommand
- Input schemas describe the accepted JSON payloads for file-driven graph ingestion
- Agents and parsers MUST validate responses against these schemas before processing
- Most schemas use `"additionalProperties": false` — unexpected keys are contract violations
- `health.schema.json` (v1.0.89, GAP-E2E-007, ADR-0048) uses `"additionalProperties": true` (Must-Ignore policy per RFC 7493 I-JSON and `rules_rust_json_e_ndjson.md:33`) — unknown keys are accepted to enable schema evolution
- The 17 new fields added in v1.0.89: `vec_memories_missing`, `vec_memories_orphaned`, `sqlite_version`, `mentions_ratio`, `mentions_warning`, `top_relation`, `top_relation_ratio`, `applies_to_ratio`, `relation_concentration_warning`, `super_hub_count`, `super_hub_warning`, `top_hub_entity`, `top_hub_degree`, `hub_warning`, `non_normalized_count`, `normalization_warning`, `fts_query_ok`
- New exit code 16 (`EX_CONFIG`) emitted by `AppError::PreFlightFailed` is documented in v1.0.87 (ADR-0045, GAP-META-005) — see `error-envelope.schema.json` for the structured `PreFlightError` variant details
### Schema Files
| Subcommand | Schema file |
|---|---|
| `init` | `init.schema.json` |
| `remember` (updated v1.0.84, ADR-0042) | `remember.schema.json` |
| `recall` (updated v1.0.84, ADR-0042 / v1.0.85, ADR-0043 enum 7 variants) | `recall.schema.json` |
| `read` | `read.schema.json` |
| `list` | `list.schema.json` |
| `forget` | `forget.schema.json` |
| `purge` | `purge.schema.json` |
| `rename` | `rename.schema.json` |
| `edit` (updated v1.0.84, ADR-0042) | `edit.schema.json` |
| `history` | `history.schema.json` |
| `restore` | `restore.schema.json` |
| `hybrid-search` (updated v1.0.84, ADR-0042 / v1.0.85, ADR-0043 enum 7 variants) | `hybrid-search.schema.json` |
| `deep-research` | `deep-research.schema.json` |
| `deep-research --output` (v1.1.05) | `deep-research-output-ack.schema.json` |
| `health` | `health.schema.json` |
| `migrate` | `migrate.schema.json` |
| `migrate --rehash` (v1.0.76, updated v1.0.77, v1.0.78) | `migrate-rehash.schema.json` |
| `migrate --to-llm-only` (v1.0.76, updated v1.0.77, v1.0.78) | `migrate-to-llm-only.schema.json` |
| `namespace-detect` | `namespace-detect.schema.json` |
| `optimize` | `optimize.schema.json` |
| `stats` | `stats.schema.json` |
| `sync-safe-copy` | `sync-safe-copy.schema.json` |
| `vacuum` | `vacuum.schema.json` |
| `link` | `link.schema.json` |
| `unlink` | `unlink.schema.json` |
| `related` | `related.schema.json` |
| `graph` | `graph.schema.json` |
| `graph traverse` | `graph-traverse.schema.json` |
| `graph stats` | `graph-stats.schema.json` |
| `graph entities` | `graph-entities.schema.json` |
| `graph recompute-degree` (v1.1.01, P3) | `graph-recompute-degree.schema.json` |
| `cleanup-orphans` | `cleanup-orphans.schema.json` |
| `prune-relations` | `prune-relations.schema.json` |
| `reclassify-relation` | `reclassify-relation.schema.json` |
| `split-body` (v1.1.03, GAP-V8) | `split-body.schema.json` |
| `entity_connect_seen` (v1.1.04, GAP-002) | implicit via migration V016 — records `(source_id, target_id, namespace, verdict, relation, evaluated_at)` |
| `normalize-entities` | `normalize-entities.schema.json` |
| `enrich` (phase event) | `enrich-phase.schema.json` |
| `enrich` (per-item event) | `enrich-item-event.schema.json` |
| `enrich` (summary, updated v1.0.84, ADR-0042) | `enrich-summary.schema.json` |
| `enrich --status` (v1.0.96, GAP-ENRICH-BACKLOG-CONVERGE) | `enrich-status.schema.json` |
| `ingest` (per-file event) | `ingest-file-event.schema.json` |
| `ingest` (summary, updated v1.0.84, ADR-0042) | `ingest-summary.schema.json` |
| `ingest --mode claude-code` (phase event) | `ingest-claude-phase.schema.json` |
| `ingest --mode claude-code` (per-file event) | `ingest-claude-file-event.schema.json` |
| `ingest --mode claude-code` (summary) | `ingest-claude-summary.schema.json` |
| `debug-schema` | `debug-schema.schema.json` |
| `fts rebuild` | `fts-rebuild.schema.json` |
| `fts check` | `fts-check.schema.json` |
| `fts stats` | `fts-stats.schema.json` |
| `backup` | `backup.schema.json` |
| `delete-entity` | `delete-entity.schema.json` |
| `reclassify` | `reclassify.schema.json` |
| `merge-entities` | `merge-entities.schema.json` |
| `rename-entity` | `rename-entity.schema.json` |
| `memory-entities` (forward: `--name`) | `memory-entities.schema.json` |
| `memory-entities` (reverse: `--entity`) | `memory-entities-reverse.schema.json` |
| `prune-ner` | `prune-ner.schema.json` |
| `remember-batch` (per-item event) | `remember-batch.schema.json` |
| `remember-batch` (summary) | `remember-batch-summary.schema.json` |
| `export` (per-memory line) | `export-memory-line.schema.json` |
| `export` (summary) | `export-summary.schema.json` |
| `vec orphan-list` (v1.0.69) | `vec-orphan-list.schema.json` |
| `vec purge-orphan` (v1.0.69) | `vec-purge-orphan.schema.json` |
| `vec stats` (v1.0.69) | `vec-stats.schema.json` |
| `codex-models` (v1.0.69) | `codex-models.schema.json` |
| `slots status` (v1.0.82, GAP-004) | `slots-status.schema.json` |
| `pending list` (v1.0.82, GAP-001) | `pending-list.schema.json` |
| `embedding status` (v1.0.82, GAP-005, updated v1.0.84, ADR-0042) | `embedding-status.schema.json` |
| `embedding list` (v1.0.82, GAP-005) | `embedding-list.schema.json` |
| shutdown envelope (v1.0.82, GAP-002) | `shutdown-envelope.schema.json` |
| error envelope (all commands) | `error-envelope.schema.json` |
### Commands Without JSON Schemas
- `completions` emits shell completion scripts (Bash, Zsh, Fish, PowerShell, Elvish) as plain text — no JSON schema applies
- `daemon` was removed in v1.0.76 (remaining code deleted in v1.0.79) — no JSON schema applies (historical)
### Ingest Mode Schema Selection
- `--mode none` uses `ingest-file-event.schema.json` and `ingest-summary.schema.json`; `--mode gliner` was REMOVED in v1.1.02 (the `IngestMode` enum now exposes only `none`, `claude-code`, `codex`, `opencode` — clap rejects `gliner` with exit 2)
- `--mode claude-code` uses `ingest-claude-phase.schema.json`, `ingest-claude-file-event.schema.json`, and `ingest-claude-summary.schema.json`
- Claude-code mode emits additional phase events (validate, scan) before per-file events
- Per-file events in claude-code mode include `entities`, `rels`, and `cost_usd` fields not present in normal ingest
- `--mode codex` (added in v1.0.62) reuses the same NDJSON schema format as `--mode claude-code` — no separate codex schemas needed
- Codex mode emits the same PhaseEvent, FileEvent, and Summary shapes; agents validating claude-code output can reuse those schemas unchanged

### Error Envelope Changes in v1.0.68 (G28-B)
- The `error-envelope.schema.json` `message` field for `code: 75` now has two distinct templates, both routed to the same exit code
- Template A (new since v1.0.68, G28-B): `job <job_type> for namespace '<namespace>' is already running (exit 75); wait for it to finish or pass --wait-job-singleton <SECONDS>` — emitted by `enrich`, `ingest --mode claude-code`, and `ingest --mode codex` when a concurrent invocation holds the singleton
- Template B (legacy): `all <max> concurrency slots occupied after waiting <waited_secs>s (exit 75); use --max-concurrency or wait for other invocations to finish` — emitted by the counting semaphore for any other command
- Agents can disambiguate the two with a regex on `message`: matches `^job ` for Template A and `^all ` for Template B
- The schema itself remains `additionalProperties: false` because variant-specific fields are intentionally NOT serialised to JSON; structured access to `job_type` and `namespace` requires agents to parse the quoted strings inside `message`

### Schema Changes in v1.0.84 (ADR-0042 / GAP-002)
- Seven response schemas gained an OPTIONAL `backend_invoked: enum [claude, codex, opencode, openrouter, none, auto]` field that reports which LLM backend the live embedding path actually invoked (opencode added in v1.0.90)
- Affected envelopes: `embedding-status`, `remember`, `edit`, `recall`, `hybrid-search`, `ingest-summary`, `enrich-summary`
- The field is omitted (not `null`) when no backend was invoked, keeping happy-path envelopes clean
- Agents SHOULD treat `backend_invoked` as the ground truth for which CLI binary ran during the call
### Update (v1.0.85 / ADR-0043)
- Two response schemas gained `vec_degraded_reason` with the seven-variant enum `embedding_failed | slot_exhausted | oauth_quota | backend_mismatch | dim_zero | cancelled | timeout` plus explicit `null` for happy-path. Callers can switch on this discriminator instead of regex against `vec_error` strings.
- Two response schemas also gained `vec_degraded_reason: enum [embedding_failed, cancelled, timeout, null]` for callers that need to distinguish OAuth quota exhaustion from cancellation from timeout
- Affected envelopes: `recall`, `hybrid-search`
- The field is omitted when live embedding succeeded, and explicitly `null` when no degradation path was triggered
- All seven updated schemas keep `"additionalProperties": false`; the new fields are additive and `null`/`omitted` are distinct contract states
- See `docs/decisions/adr-0042-claude-backend-split.md` (EN) and `.pt-BR.md` for the full rationale
### Schema Changes in v1.0.85 (ADR-0043 / five-gap remediation)
- `recall` and `hybrid-search` response schemas extended `vec_degraded_reason` enum from 3 to 7 variants: `embedding_failed | slot_exhausted | oauth_quota | backend_mismatch | dim_zero | cancelled | timeout`
- `slot_exhausted` (GAP-003) discriminates LLM subprocess semaphore contention from quota exhaustion; callers can retry with `SQLITE_GRAPHRAG_LLM_SLOT_WAIT_SECS` override
- `oauth_quota` (G58, G45-CR5) discriminates Anthropic usage limit exhaustion from structural embedding errors; triggers deterministic codex <-> claude backend swap before falling back to FTS5
- `backend_mismatch` discriminates requested vs resolved backend divergence (e.g. `--llm-backend claude` resolved to codex via PATH-probe)
- `dim_zero` discriminates an embedding that returned a zero-dimension vector (structural bug indicator distinct from quota or contention)
- The expanded enum is backwards compatible: existing callers that switch on `embedding_failed | cancelled | timeout` continue to work; new variants are additive
- Default embedding `dim` is 64 (MRL, arXiv 2205.13147) since v1.0.79; v1.0.85 confirms and locks the constant at `src/constants.rs:22 DEFAULT_EMBEDDING_DIM = 64` (G56 docs)
- `anthropic-ratelimit-*-remaining` headers are now first-class signal in `LlmEmbedding::invoke_claude` (G45-CR5); a zero value aborts the spawn with `AppError::Embedding` mapped to `FallbackReason::OAuthQuota`
- `read` `AppError::MemoryNotFound` / `MemoryNotFoundById` Display is bilingue via `pt::memory_not_found` / `pt::memory_not_found_by_id` (G55 docs, preserved from v1.0.80)
- All schemas keep `"additionalProperties": false`; the seven-variant enum is the canonical discriminator for live-embedding degradation
- See `docs/decisions/adr-0043-five-gap-remediation.md` (EN) and `.pt-BR.md` for the full rationale
### Mudancas de Schema em v1.0.85 (ADR-0043 / remediacao dos cinco gaps)
- Schemas de resposta `recall` e `hybrid-search` estenderam o enum `vec_degraded_reason` de 3 para 7 variantes: `embedding_failed | slot_exhausted | oauth_quota | backend_mismatch | dim_zero | cancelled | timeout`
- `slot_exhausted` (GAP-003) discrimina contencao do semaforo de subprocessos LLM de exaustao de cota; chamadores podem re-tentar com override `SQLITE_GRAPHRAG_LLM_SLOT_WAIT_SECS`
- `oauth_quota` (G58, G45-CR5) discrimina exaustao de cota Anthropic de erros estruturais de embedding; dispara troca deterministica codex <-> claude antes de cair em FTS5-puro
- `backend_mismatch` discrimina divergencia entre backend solicitado e resolvido (ex. `--llm-backend claude` resolvido para codex via PATH-probe)
- `dim_zero` discrimina embedding que retornou vetor de dimensao zero (indicador de bug estrutural distinto de cota ou contencao)
- O enum expandido e retrocompativel: chamadores existentes que chaveiam em `embedding_failed | cancelled | timeout` continuam funcionando; variantes novas sao aditivas
- `dim` default de embedding e 64 (MRL, arXiv 2205.13147) desde v1.0.79; v1.0.85 confirma e tranca a constante em `src/constants.rs:22 DEFAULT_EMBEDDING_DIM = 64` (G56 docs)
- Headers `anthropic-ratelimit-*-remaining` agora sao sinal de primeira classe em `LlmEmbedding::invoke_claude` (G45-CR5); valor zero aborta o spawn com `AppError::Embedding` mapeado para `FallbackReason::OAuthQuota`
- `read` `AppError::MemoryNotFound` / `MemoryNotFoundById` Display e bilingue via `pt::memory_not_found` / `pt::memory_not_found_by_id` (G55 docs, preservado desde v1.0.80)
- Todos os schemas mantem `"additionalProperties": false`; o enum de sete variantes e o discriminador canonico para degradacao de embedding live
- Veja `docs/decisions/adr-0043-five-gap-remediation.md` (EN) e `.pt-BR.md` para a justificativa completa
### Input Payload Schemas
- `entities-input.schema.json` validates the JSON array accepted by `remember --entities-file`
- `relationships-input.schema.json` validates the JSON array accepted by `remember --relationships-file`
### Usage
- Inspect a `recall` response shape quickly: `sqlite-graphrag recall "query" | jaq '.'`
- Validate with a real JSON Schema validator: `jsonschema --instance <(sqlite-graphrag stats) docs/schemas/stats.schema.json`
- The `debug-schema` subcommand is hidden and intended for diagnostic tooling only — the binary exposes it with a double-underscore prefix (`debug-schema`) while the schema file uses the kebab-case name `debug-schema.schema.json` following the directory convention


### Schema Evolution in v1.0.86 → v1.0.89 (ADR-0045, ADR-0046, ADR-0047, ADR-0048, ADR-0049)
- v1.0.86 added 6 schemas for new LLM-pipeline subcommands: `slots-status.schema.json`, `pending-list.schema.json`, `embedding-status.schema.json` (updated v1.0.84 ADR-0042), `embedding-list.schema.json`, `shutdown-envelope.schema.json` (exit 19 envelope). `pending-embeddings process` reuses `pending-list.schema.json`
- v1.0.87 added `AppError::PreFlightFailed` (exit 16 `EX_CONFIG`) documented in `error-envelope.schema.json` with 8 variants: `ArgvExceedsArgMax`, `BinaryNotFound`, `McpConfigInlineJsonRejected`, `McpConfigPathMissing`, `McpConfigPathInvalidJson`, `WalkUpMcpJsonInvalid`, `OutputBufferTooSmall`, `ClaudeConfigDirNotEmpty`
- v1.0.88 fixed: `oauth_stderr_emits_single_line_v1088` regression test validates exit-19 envelope now emits 1 stderr line (was 2). All other schemas unchanged
- v1.0.89 (GAP-E2E-007) regenerated `health.schema.json` via `schemars 0.8` derive macro. Switched from `additionalProperties: false` to `true` (Must-Ignore). 17 new fields added. New `src/bin/dump_schema.rs` regenerates the schema idempotently via `schema_for!()` + BTreeMap ordering + recursive `apply_must_ignore` policy enforcement
- v1.0.89 (GAP-E2E-008, GAP-E2E-010) added `--db <PATH>` flag parity on 5 subcommands: `embedding-status`, `embedding-list`, `pending-list`, `codex-models`. No schema changes (the flag affects input parsing, not output envelope)
- v1.0.89 (GAP-E2E-009) added `--dry-run` and `--confirm` flags to `migrate`. New `migrate-dry-run.schema.json` describes the structured dry-run report (pending_migrations[], pending_count, checksum_mismatches[], status)
- v1.0.89 (GAP-E2E-011) added `--auto-describe` (default true) to `ingest`. No schema changes; affects how `description` field is populated in `ingest-file-event.schema.json` and `ingest-summary.schema.json` envelopes

### Schema Changes in v1.0.93 (ADR-0052 / OpenRouter Embedding Backend)
- Seven response schemas updated `backend_invoked` enum to include `openrouter` as a sixth variant: `claude | codex | opencode | openrouter | none | auto`
- `openrouter` is emitted when embedding was computed via the OpenRouter REST API (`--embedding-backend openrouter`) instead of a headless LLM subprocess
- Affected envelopes: `embedding-status`, `remember`, `edit`, `recall`, `hybrid-search`, `ingest-summary`, `enrich-summary`
- No new schema files were added — the OpenRouter backend uses the same output envelope structure as existing backends
- `ingest-summary.schema.json` now reflects the `--enrich-after` flag behavior: when active, the summary includes the enrich phase results inline

### Schema Changes in v1.0.95 (ADR-0054 / OpenRouter Chat Enrich)
- `enrich` gains a fourth extraction mode `openrouter` (`--mode openrouter`) that routes the JUDGE turn to the OpenRouter `/chat/completions` REST endpoint instead of a headless `claude`/`codex`/`opencode` subprocess
- NO new schema files were added — `enrich-phase.schema.json`, `enrich-item-event.schema.json`, and `enrich-summary.schema.json` are unchanged; the SCAN→JUDGE→PERSIST envelopes keep the same shape regardless of JUDGE transport
- The optional `backend_invoked` enum already covers `openrouter` (added v1.0.93 for embedding); the same variant now also describes an enrich JUDGE served via OpenRouter chat
- Structured Outputs (`response_format` `json_schema` `strict: true`) make the JUDGE output conform to the same entity/relationship structs the subprocess backends emit — no schema divergence

### Schema Changes in v1.1.04 (ADR-0064)
- Migration V016 introduces the `entity_connect_seen` table recording the LLM verdict (`related`/`none`) per evaluated entity pair for convergent `entity-connect`
- `CURRENT_SCHEMA_VERSION` advances 15 to 16
- The `entity_connect` enrich operation is promoted from scan-only to fully-implemented
- No new output schema file: `entity_connect_seen` is an internal table (implicit schema via the V016 migration), not a subcommand stdout contract

### Schema Changes in v1.1.06 (ADR-0066)
- **No required database migration.** `CURRENT_SCHEMA_VERSION` stays at **16**. Operators do **not** need `migrate` for this release.
- Closes **GAP-ENTITY-CONNECT-SCAN-CARTESIAN**: `enrich --operation entity-connect` / `cross-domain-bridges` no longer use a cartesian pair scan. Queue `item_key` is `pair:{id1}:{id2}` with `item_type=entity_pair` (sidecar queue contract; not a main-DB migration). Drain resolves by entity primary key.
- NDJSON phases for operators/hooks: `scan_start` **before** SQL (`operation`, `entities_in_namespace`, `backlog_degree0_proxy`) and `scan_meta` (`pairs_enqueued_this_scan`, `scan_elapsed_ms`). Existing `enrich-phase.schema.json` / `enrich-item-event.schema.json` remain the phase envelopes; field additions are additive observational payloads on those phases.
- First-scan wall-clock uses `InterruptHandle` → Timeout exit **1** (not singleton **75**). No new stdout summary schema file.
- Regression suite: `tests/v1106_entity_connect_scan_regression.rs`. Decision: [ADR-0066](../decisions/adr-0066-v1-1-06-entity-connect-scan.md).

### Schema Changes in v1.1.05 (ADR-0065)
- **No required database migration.** `CURRENT_SCHEMA_VERSION` stays at **16**. Operators do **not** need `migrate` for this release.
- `deep-research` — `sub_queries[].source` now also emits `aspect` (single-token facet fan-out) and `manual` (`--sub-query-strategy manual --sub-queries-file PATH`). `deep-research.schema.json` enum is updated in v1.1.05 to `original | decomposed | aspect | manual`. No new *required* output fields on the full envelope.
- `deep-research --output PATH` — when set, stdout is a short **ack** after atomwrite (tempfile same dir → fsync → rename); the full research envelope is on disk. Dedicated schema: `deep-research-output-ack.schema.json` (Bug 2 / ADR-0065). Required fields: `written` (string path), `bytes` (u64 file size), `blake3` (hex digest of written bytes), `sub_queries_total` (usize), `unique_memories_found` (usize), `elapsed_ms` (u64). `additionalProperties: false`. Regression suite: `tests/v1105_danilo_bugs_regression.rs`.
- `link --from-id` / `--to-id` — **CLI input flags only**. The `link.schema.json` **output** envelope is unchanged (`from`/`to` remain entity **names** after resolution). Digit-only strings are rejected as names (`validate_entity_name`); that is validation behaviour, not a new JSON property. Schema description notes the ID flags.
- `graph traverse --fuzzy` — resolution UX: with `--fuzzy`, the `from` field in `graph-traverse.schema.json` is the **resolved canonical name** (may differ from the CLI `--from` argument). NotFound name suggestions remain error UX only.
- Global `--quiet` / `-q` — affects stderr tracing volume only; no stdout schema impact.

### Schema Changes in v1.0.96 → v1.0.97 (ADR-0055, GAP-SG-15/16/41/43)
- v1.0.96 (ADR-0055): `enrich-status.schema.json` for the read-only `enrich --status` report (`unbound_backlog`, per-operation `scan_backlog` (GAP-SG-77, v1.1.0), queue `pending`/`done`/`failed`/`dead`/`skipped`, `eligible_now`, `waiting`); the `.enrich-queue.sqlite` sidecar gains the `error_class`/`next_retry_at` columns and the `dead` terminal status via an idempotent `ALTER TABLE`
- v1.0.97 (GAP-SG-15/16): `enrich-summary.schema.json` gains the `dead` and `waiting` count fields so the summary distinguishes terminal failures and cooldown from an empty backlog
- v1.0.97 (GAP-SG-43): `stats.schema.json` gains a top-level `total_memories` integer
- v1.0.97 (GAP-SG-41): `embedding-status.schema.json` gains a REQUIRED `coverage` object (`memories_total`/`memories_with_vec`/`entities_total`/`entities_with_vec`/`chunks_total`/`chunks_with_vec`) reporting real persisted-vector counts, distinct from the always-empty async `counts` queue; the live `embedding status` output always carries it
- NO main-database schema migration: the SQLite schema stays at v15 across both releases. The v1.0.97 queue-sidecar path change (ADR-0057) is a path-derivation fix, not a schema change

### Input Payload Schemas (Reference)
- `entities-input.schema.json` validates the JSON array accepted by `remember --entities-file`
- `relationships-input.schema.json` validates the JSON array accepted by `remember --relationships-file`

### Usage
- Inspect a `recall` response shape quickly: `sqlite-graphrag recall "query" | jaq '.'`
- Validate with a real JSON Schema validator: `jsonschema --instance <(sqlite-graphrag stats) docs/schemas/stats.schema.json`
- The `debug-schema` subcommand is hidden and intended for diagnostic tooling only — the binary exposes it with a double-underscore prefix (`debug-schema`) while the schema file uses the kebab-case name `debug-schema.schema.json` following the directory convention


## Português Brasileiro
### Propósito
- Cada arquivo neste diretório é um documento JSON Schema Draft 2020-12
- Schemas de saída descrevem o contrato exato de stdout de cada subcomando `sqlite-graphrag`
- Schemas de entrada descrevem os payloads JSON aceitos para ingestão de grafo orientada a arquivo
- Agentes e parsers DEVEM validar respostas contra estes schemas antes de processar
- A maioria dos schemas usa `"additionalProperties": false` — chaves inesperadas são violações de contrato
- `health.schema.json` (v1.0.89, GAP-E2E-007, ADR-0048) usa `"additionalProperties": true` (política Must-Ignore por RFC 7493 I-JSON e `rules_rust_json_e_ndjson.md:33`) — chaves desconhecidas são aceitas para permitir evolução do schema
- Os 17 novos campos adicionados em v1.0.89: `vec_memories_missing`, `vec_memories_orphaned`, `sqlite_version`, `mentions_ratio`, `mentions_warning`, `top_relation`, `top_relation_ratio`, `applies_to_ratio`, `relation_concentration_warning`, `super_hub_count`, `super_hub_warning`, `top_hub_entity`, `top_hub_degree`, `hub_warning`, `non_normalized_count`, `normalization_warning`, `fts_query_ok`
- Novo exit code 16 (`EX_CONFIG`) emitido por `AppError::PreFlightFailed` é documentado em v1.0.87 (ADR-0045, GAP-META-005) — veja `error-envelope.schema.json` para detalhes estruturados da variante `PreFlightError`
### Arquivos de Schema
| Subcomando | Arquivo de schema |
|---|---|
| `init` | `init.schema.json` |
| `remember` (atualizado v1.0.84, ADR-0042) | `remember.schema.json` |
| `recall` (atualizado v1.0.84, ADR-0042 / v1.0.85, ADR-0043 enum 7 variantes) | `recall.schema.json` |
| `read` | `read.schema.json` |
| `list` | `list.schema.json` |
| `forget` | `forget.schema.json` |
| `purge` | `purge.schema.json` |
| `rename` | `rename.schema.json` |
| `edit` (atualizado v1.0.84, ADR-0042) | `edit.schema.json` |
| `history` | `history.schema.json` |
| `restore` | `restore.schema.json` |
| `hybrid-search` (atualizado v1.0.84, ADR-0042 / v1.0.85, ADR-0043 enum 7 variantes) | `hybrid-search.schema.json` |
| `deep-research` | `deep-research.schema.json` |
| `deep-research --output` (v1.1.05) | `deep-research-output-ack.schema.json` |
| `health` | `health.schema.json` |
| `migrate` | `migrate.schema.json` |
| `migrate --rehash` (v1.0.76, atualizado v1.0.77, v1.0.78) | `migrate-rehash.schema.json` |
| `migrate --to-llm-only` (v1.0.76, atualizado v1.0.77, v1.0.78) | `migrate-to-llm-only.schema.json` |
| `namespace-detect` | `namespace-detect.schema.json` |
| `optimize` | `optimize.schema.json` |
| `stats` | `stats.schema.json` |
| `sync-safe-copy` | `sync-safe-copy.schema.json` |
| `vacuum` | `vacuum.schema.json` |
| `link` | `link.schema.json` |
| `unlink` | `unlink.schema.json` |
| `related` | `related.schema.json` |
| `graph` | `graph.schema.json` |
| `graph traverse` | `graph-traverse.schema.json` |
| `graph stats` | `graph-stats.schema.json` |
| `graph entities` | `graph-entities.schema.json` |
| `cleanup-orphans` | `cleanup-orphans.schema.json` |
| `prune-relations` | `prune-relations.schema.json` |
| `reclassify-relation` | `reclassify-relation.schema.json` |
| `normalize-entities` | `normalize-entities.schema.json` |
| `enrich` (evento de fase) | `enrich-phase.schema.json` |
| `enrich` (evento por item) | `enrich-item-event.schema.json` |
| `enrich` (sumário, atualizado v1.0.84, ADR-0042) | `enrich-summary.schema.json` |
| `enrich --status` (v1.0.96, GAP-ENRICH-BACKLOG-CONVERGE) | `enrich-status.schema.json` |
| `ingest` (evento por arquivo) | `ingest-file-event.schema.json` |
| `ingest` (sumário, atualizado v1.0.84, ADR-0042) | `ingest-summary.schema.json` |
| `ingest --mode claude-code` (evento de fase) | `ingest-claude-phase.schema.json` |
| `ingest --mode claude-code` (evento por arquivo) | `ingest-claude-file-event.schema.json` |
| `ingest --mode claude-code` (sumário) | `ingest-claude-summary.schema.json` |
| `debug-schema` | `debug-schema.schema.json` |
| `fts rebuild` | `fts-rebuild.schema.json` |
| `fts check` | `fts-check.schema.json` |
| `fts stats` | `fts-stats.schema.json` |
| `backup` | `backup.schema.json` |
| `delete-entity` | `delete-entity.schema.json` |
| `reclassify` | `reclassify.schema.json` |
| `merge-entities` | `merge-entities.schema.json` |
| `rename-entity` | `rename-entity.schema.json` |
| `memory-entities` (forward: `--name`) | `memory-entities.schema.json` |
| `memory-entities` (reverso: `--entity`) | `memory-entities-reverse.schema.json` |
| `prune-ner` | `prune-ner.schema.json` |
| `remember-batch` (evento por item) | `remember-batch.schema.json` |
| `remember-batch` (sumário) | `remember-batch-summary.schema.json` |
| `export` (linha por memória) | `export-memory-line.schema.json` |
| `export` (sumário) | `export-summary.schema.json` |
| `vec orphan-list` (v1.0.69) | `vec-orphan-list.schema.json` |
| `vec purge-orphan` (v1.0.69) | `vec-purge-orphan.schema.json` |
| `vec stats` (v1.0.69) | `vec-stats.schema.json` |
| `codex-models` (v1.0.69) | `codex-models.schema.json` |
| `slots status` (v1.0.82, GAP-004) | `slots-status.schema.json` |
| `pending list` (v1.0.82, GAP-001) | `pending-list.schema.json` |
| `embedding status` (v1.0.82, GAP-005, atualizado v1.0.84, ADR-0042) | `embedding-status.schema.json` |
| `embedding list` (v1.0.82, GAP-005) | `embedding-list.schema.json` |
| envelope de shutdown (v1.0.82, GAP-002) | `shutdown-envelope.schema.json` |
| envelope de erro (todos os comandos) | `error-envelope.schema.json` |

### Mudanças de Schema na v1.1.05 (ADR-0065)
- **Sem migração de banco obrigatória.** `CURRENT_SCHEMA_VERSION` permanece em **16**. Operadores **não** precisam de `migrate` nesta release.
- `deep-research` — `sub_queries[].source` também emite `aspect` (fan-out de facetas em token único) e `manual` (`--sub-query-strategy manual --sub-queries-file PATH`). O enum em `deep-research.schema.json` na v1.1.05 é `original | decomposed | aspect | manual`. Nenhum campo *obrigatório* novo no envelope completo.
- `deep-research --output PATH` — quando definido, o stdout é um **ack** curto após atomwrite (tempfile no mesmo diretório → fsync → rename); o envelope completo fica em disco. Schema dedicado: `deep-research-output-ack.schema.json` (Bug 2 / ADR-0065). Campos obrigatórios: `written` (caminho string), `bytes` (tamanho u64 do arquivo), `blake3` (digest hex dos bytes gravados), `sub_queries_total` (usize), `unique_memories_found` (usize), `elapsed_ms` (u64). `additionalProperties: false`. Suite de regressão: `tests/v1105_danilo_bugs_regression.rs`.
- `link --from-id` / `--to-id` — **apenas flags de entrada CLI**. O envelope de **saída** de `link.schema.json` permanece inalterado (`from`/`to` continuam sendo **nomes** de entidade após resolução). Strings só de dígitos são rejeitadas como nomes (`validate_entity_name`); isso é comportamento de validação, não uma nova propriedade JSON. A descrição do schema menciona as flags por ID.
- `graph traverse --fuzzy` — UX de resolução: com `--fuzzy`, o campo `from` em `graph-traverse.schema.json` é o **nome canônico resolvido** (pode diferir do argumento CLI `--from`). Sugestões de nome em NotFound permanecem apenas UX de erro.
- Global `--quiet` / `-q` — afeta apenas o volume de tracing em stderr; sem impacto no schema de stdout.
