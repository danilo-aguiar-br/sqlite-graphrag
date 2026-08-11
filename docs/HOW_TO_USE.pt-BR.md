# COMO USAR sqlite-graphrag (v1.2.5 — superfície de saída agent-native, selo CAPA enrich, dim 1024, schema v16)

> Entregue memória persistente a qualquer agente de IA com um binário local, um único arquivo SQLite, e a CLI de LLM que você já confia.

- English version: [HOW_TO_USE.md](HOW_TO_USE.md)
- Voltar ao [README.pt-BR.md](../README.pt-BR.md) para referência de comandos

## Configuração (XDG — v1.2.5)

- O registro completo das 63 chaves XDG, com tipo de valor e default, está em [AGENTS.pt-BR.md — Registro completo de chaves XDG](AGENTS.pt-BR.md#obrigatório--registro-completo-de-chaves-xdg-as-63-chaves-v125). Ele fica em UM lugar de propósito: uma tabela copiada em cada guia vira vários lugares para divergir.
- Knobs de runtime resolvem como **flag CLI > XDG `config set` > default nomeado**
- Env de produto `SQLITE_GRAPHRAG_*` **não** é lida em runtime (proibida como configuração de produto)
- Segredos: `config add-key --provider openrouter` (stdin) ou `--openrouter-api-key` por chamada
- Inspecione: `config path`, `config list`, `config list --effective`, `config doctor`
- URLs OpenRouter: `config set network.openrouter.chat_url …` / `network.openrouter.embeddings_url …`
- Recall offline com fail-fast: `config set llm.probe_timeout_ms 3000` e/ou `--llm-backend none`
- Limpeza de soft-delete: `purge --now --yes` para hard-delete imediato; a retenção padrão é 90 dias e `--yes` sozinho **não** apaga soft-deletes recentes
- Env de SO permitida apenas: locale (`LANG`/`LC_*`), `PATH`, `HOME`/`USERPROFILE`, diretórios base XDG, `NO_COLOR`, mais a whitelist OAuth de subprocesso (`ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, …)
- Gate offline: `bash scripts/e2e_offline_v120.sh` (o wrapper histórico `e2e_offline_v118.sh` foi substituído). Pin de consumidores de biblioteca em `=1.2.2`. Schema permanece em **v16** (sem migrate se já estiver em v16). **DEFAULT_EMBEDDING_DIM=1024**. Selo CAPA do enrich (claim por namespace, until-empty op+ns, reopen do force-redescribe, LENGTH no re-embed / enqueue com `entity:`) — veja a seção abaixo.

```bash
sqlite-graphrag config set network.openrouter.embeddings_url "https://openrouter.ai/api/v1/embeddings"
sqlite-graphrag config list --effective --json
sqlite-graphrag purge --now --yes --json   # depois de forget, quando quiser hard-delete imediato
```

## O Que Mudou na v1.2.1 — Selo CAPA da Fila Enrich (Sem Migração)

- Crate **1.2.1**; schema **inalterado** em **v16**. Sem migração main-DB — **somente comportamento do sidecar**. Pin de consumidores de biblioteca em `=1.2.1`.
- **Isolamento de namespace no claim** — `dequeue_next_pending` exige `operation` **e** `namespace`. Enrich em `ai-sdd` não processa mais linhas de `global` / ns vazio.
- **`--until-empty` conta só esta op+namespace** — `count_eligible_pending` (não todo pending entre operações). Zumbis ReEmbed de outra op não mantêm EntityDescriptions girando até max-runtime com `completed=0`.
- **`--force-redescribe` reabre `skipped`/`done`** — `reopen_force_redescribe_candidates` uma vez por processo antes do primeiro enqueue; nunca reabre `dead` (use `--requeue-dead`).
- **Reconciliação de zumbi re-embed** — `reconcile_satisfied_reembed_pending` marca ReEmbed pending como `done` quando o BLOB live já corresponde à dim ativa (`LENGTH(embedding) = dim*4`).
- **Elegibilidade re-embed por comprimento do BLOB** — elegível quando não há vetor com `LENGTH(embedding) = target_dim * 4`. Linhas CORRUPT (`dim=1024`, BLOB ainda 384) re-embedam de novo.
- **Enqueue valida chaves re-embed** — `entity:{name}` remove o prefixo no lookup; nomes bare ainda funcionam; entidades ausentes rejeitadas. Chaves de chunk validam que `chunk_id` existe em memória não-deletada do namespace alvo.
- **Marcadores CAPA-D de baixa qualidade** — apenas frases compostas de "configuration file" (sem FP bare `%configuration file%`).
- Regressões: `enqueue_candidate_accepts_entity_prefixed_reembed_key`, `dequeue_next_pending_isolates_by_namespace`; suite da fila **38** OK.
- Gate offline inalterado: `scripts/e2e_offline_v120.sh` **20/20**. Residual: backfill live de descrições LQ permanece campanha do operador (`--force-redescribe` + `--until-empty`).
- Veja [MIGRATION.pt-BR.md](MIGRATION.pt-BR.md) e [CHANGELOG.pt-BR.md](../CHANGELOG.pt-BR.md) `[1.2.1]`.

### Receitas — CAPA do enrich (v1.2.1)

```bash
DB="${DB:-$HOME/.local/share/sqlite-graphrag/memory.db}"
MODEL="${MODEL:-deepseek/deepseek-v4-flash:nitro}"

# Status (sem LLM) — escopado por operação + namespace
sqlite-graphrag enrich --db "$DB" --status --operation re-embed --namespace global -q

# Re-embed de entidades após migrate de dim / BLOB CORRUPT (elegibilidade por LENGTH)
sqlite-graphrag enrich --db "$DB" --operation re-embed --target entities \
  --mode openrouter --openrouter-model "$MODEL" \
  --until-empty --namespace global -q --wait-lock 60

# Force-redescribe com reopen de skipped/done (nunca dead)
sqlite-graphrag enrich --db "$DB" --operation entity-descriptions \
  --mode openrouter --openrouter-model "$MODEL" \
  --force-redescribe --until-empty --namespace global -q

# Recuperação do sink skipped (sem SQL bruto)
sqlite-graphrag enrich --db "$DB" --list-skipped --operation entity-descriptions --namespace global -q
sqlite-graphrag enrich --db "$DB" --requeue-skipped --operation entity-descriptions --namespace global -q
```

## O Que Mudou na v1.2.0 — dim 1024 + XDG (Sem Migração)

- Crate **1.2.0**. Schema do banco principal **inalterado em v16** (sem migração main-DB).
- **DEFAULT_EMBEDDING_DIM=1024** (flag `--embedding-dim` / XDG `embedding.dim` ainda sobrescrevem; bancos existentes mantêm `schema_meta.dim` até re-embed).
- Fila enrich **multi-namespace** — coluna `namespace` + `UNIQUE(namespace, operation, item_key)`; `DELETE` escopado.
- `--list-skipped` / `--requeue-skipped` recuperam `preservation_failed` / sink `skipped` sem SQL cru.
- **GAP-SG-139** — folhas host/XDG (`config`, `slots`, `cache`, `completions`) aceitam `--db` como **no-op** documentado (não abrem o DB do grafo).
- Testes herméticos **IsolatedEnv** / `wire_assert_cmd` (ZERO env de produto operacional; `xdg_isolation_guard`).
- Gate offline E2E: `scripts/e2e_offline_v120.sh` (**20/20**); wrapper histórico `e2e_offline_v118.sh` supersedido.
- Config operacional: **flag CLI > XDG `config set` > default**. Help sem env de produto e sem Box about.
- OpenRouter: URLs via XDG `network.openrouter.*`. Fail-fast de query Auto: `llm.probe_timeout_ms` (3000 ms).
- EntityType `module`→Concept; `related_to`→`related`; alias de telemetry removido.
- `remember-batch` exige `description` na criação; `pending-embeddings status`; `cache stats`; `purge --now`; `config list --effective`.
- Claim da fila enrich escopado por `operation` (QISO).
- entity-descriptions: prompt multi-domínio neutro, grounding no corpus, `--force-redescribe` para reescrever descriptions de baixa qualidade.
- Status honesto: `enrich --status --force-redescribe` reporta `scan_backlog_low_quality`, `quality_pct`, `state=blocked_dead` quando aplicável.
- Nomes: `--entity-names` / `--memory-names` (alias `--names` com semântica por operação).
- Hot-set do remember: campos `entities_created` / `enrich_recommended`; flag `--enqueue-enrich`.
- deep-research: alias curto `-o` de `--output`; escrita atômica + ack `{written,bytes,blake3}`.
- memory-entities (forward): JSON inclui `entities[].description`.
- entity-connect permanece totalmente implementado (persiste relações); DB grande: `--anchor-memory`, limites adaptativos, yield, `budget_exhausted` / `preempted_for_gate`.
- Ordem recomendada após escrita: entity-descriptions (quente) depois entity-connect (frio).
- Residuais: monólitos >800 LOC; qualidade live LQ = operador (`--force-redescribe` + LLM).
- Pin da biblioteca: `=1.2.0`. Veja [MIGRATION.pt-BR.md](MIGRATION.pt-BR.md) e [CHANGELOG.md](../CHANGELOG.md) `[1.2.0]`.

### Receita — Config XDG efetiva

```bash
sqlite-graphrag config path --json
sqlite-graphrag config list --effective --json
sqlite-graphrag config set network.openrouter.chat_url "https://openrouter.ai/api/v1/chat/completions"
sqlite-graphrag config set network.openrouter.embeddings_url "https://openrouter.ai/api/v1/embeddings"
sqlite-graphrag config set llm.probe_timeout_ms 3000
```

### Receita — Status e purge da v1.2.0

```bash
sqlite-graphrag pending-embeddings status --json
sqlite-graphrag cache stats --json
sqlite-graphrag purge --now --dry-run --json   # preview; combine --yes para aplicar
```

### Receita — remember-batch com description obrigatória

```bash
printf '{"name":"nota-a","type":"note","description":"primeira","body":"conteúdo a"}\n' \
  | sqlite-graphrag remember-batch --json
# Sem description na criação → erro de validação (exit 1)
```

### Receita — qualidade do enrich e hot-set (v1.2.0)

```bash
# Após remember curado: leia enrich_recommended, depois ED prioritário
sqlite-graphrag remember --name demo --type note --description "d" --body "nota fiscal ICMS" \
  --graph-stdin --enqueue-enrich --json <<'EOF'
{"entities":[{"name":"icms-p05","entity_type":"concept"}],"relationships":[]}
EOF

# Audite descriptions das entidades da memória
sqlite-graphrag memory-entities --name demo --json | jaq '.entities[] | {name, description}'

# Pass de prioridade por nomes de entidade (não de memória)
sqlite-graphrag enrich --operation entity-descriptions \
  --entity-names icms-p05 --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json

# Reescreva descriptions de baixa qualidade já preenchidas
sqlite-graphrag enrich --operation entity-descriptions --force-redescribe \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --limit 20 --json
sqlite-graphrag enrich --operation entity-descriptions --status --force-redescribe --json

# deep-research com alias curto -o
sqlite-graphrag deep-research "decisões de autenticação" -o /tmp/dr.json --quiet --json

# memory-bindings usa nomes de memória
sqlite-graphrag enrich --operation memory-bindings --memory-names demo --dry-run --json
```

## O Que Mudou na v1.1.06 — Scan O(k) do entity-connect (Sem Migração)


- Nome oficial **v1.1.06**; manifesto `1.1.6`. Schema **inalterado** em **v16**.
- Fecha GAP-ENTITY-CONNECT-SCAN-CARTESIAN (hang P0 no `global` grande).
- Candidatos: **coocorrência** em `memory_entities` + **hub × ilha grau-0**.
- Chaves da fila `pair:{id1}:{id2}`; `item_type=entity_pair`; drain por chave primária (sem re-scan).
- Primeiro scan coberto por `--max-runtime` / teto soft 120s (`InterruptHandle` → Timeout exit **1**, não 75).
- NDJSON: `scan_start` (antes do SQL) com `operation`, `entities_in_namespace`, `backlog_degree0_proxy`; `scan_meta` com `pairs_enqueued_this_scan`.
- `cross-domain-bridges` usa o **mesmo** caminho fully-implemented + `entity_connect_seen`.
- Suite `tests/v1106_entity_connect_scan_regression.rs`. ADR-0066. Pin `=1.1.6`.

### Receita — Dry-run seguro em namespace grande

```bash
sqlite-graphrag enrich --operation entity-connect --dry-run --json --limit 50 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro
# Espere: validate → scan_start → scan → scan_meta (ms–s, não minutos a 100% CPU)
# scan_start.backlog_degree0_proxy ≠ scan_meta.pairs_enqueued_this_scan (backlog dual)
```

### Receita — Convergir com teto de wall-clock no primeiro scan

```bash
# --max-runtime cobre o PRIMEIRO scan (InterruptHandle). Timeout → exit 1, não 75.
sqlite-graphrag enrich --operation entity-connect --until-empty --max-runtime 600 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json
# cross-domain-bridges usa o mesmo path O(k) + entity_connect_seen (GAP-002 preservado)
```

## O Que Mudou na v1.1.05 — Cinco Bugs do Incidente deep-research "danilo"
- O nome oficial da release é **v1.1.05**; o `Cargo.toml` carrega `1.1.5` porque o SemVer rejeita zero à esquerda no segmento de patch. O schema permanece INALTERADO em v16 — o upgrade NÃO requer `migrate`. Binário ~19 MiB. Consumidores da biblioteca fixam `=1.1.5`. User-Agent `sqlite-graphrag/1.1.5`.
- **Bug 1**: `deep-research` com query de palavra única (ex.: `"danilo"`) expande em sub-queries multi-aspecto (`source: "aspect"`, facetas EN/PT). Estratégia manual: `--sub-query-strategy manual --sub-queries-file <PATH>`.
- **Bug 2**: `deep-research --output PATH` grava o envelope completo via atomwrite (tempfile → fsync → rename) e emite ack curto no stdout com checksum `blake3`. Flag global `--quiet`/`-q` suprime tracing não-erro. Contrato: JSON no stdout, logs no stderr — **nunca** `&>` no mesmo arquivo.
- **Bug 3**: `graph traverse --from <nome-curto>` — match exato prioritário; sem `--fuzzy`, NotFound (exit 4) inclui sugestões ranqueadas (Jaro-Winkler / prefixo); com `--fuzzy`, auto-resolve vencedor claro com warning em stderr.
- **Bug 4**: `merge-entities` rejeita self-ref (`--ids` contendo `--into-id`, ou `--names` contendo `--into`) **antes** de qualquer trabalho no DB.
- **Bug 5**: `link --from-id` / `--to-id` resolvem por ID; nomes só de dígitos são rejeitados por `validate_entity_name` (impede entidades fantasma sob `--create-missing`).
- Suite de regressão: `tests/v1105_danilo_bugs_regression.rs`.
- Consulte [ADR-0065](decisions/adr-0065-v1-1-05-danilo-bugs.pt-BR.md).

```bash
# Token único → fan-out aspect
sqlite-graphrag deep-research "danilo" --k 20 --max-sub-queries 7 --json

# Envelope atômico: ack curto no stdout; envelope completo no arquivo
sqlite-graphrag --quiet deep-research "auth" --output /tmp/dr.json --json
# stdout: {written, bytes, blake3, sub_queries_total, unique_memories_found, elapsed_ms}
# arquivo: jaq '.stats' /tmp/dr.json
jaq '.stats' /tmp/dr.json

# Traverse fuzzy
sqlite-graphrag graph traverse --from danilo --depth 2 --fuzzy --json

# Link por ID
sqlite-graphrag link --from-id 12 --to-id 34 --relation uses --json

# Merge self-ref rejeitado cedo (exit non-zero ANTES de abrir/escrever o DB)
sqlite-graphrag merge-entities --ids 1,2,3 --into-id 3 --json
```

## Custom Providers (v1.0.83+)
- O sqlite-graphrag suporta providers Anthropic-compatíveis (Minimax/api.minimax.io, OpenRouter, AWS Bedrock, gateways corporativos) preservando as seguintes env vars ao spawnar `claude -p` ou `codex exec`
- Vars preservadas: `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`, `OTEL_EXPORTER_OTLP_ENDPOINT`
- O mandato OAuth-only permanece ativo: `ANTHROPIC_API_KEY` e `OPENAI_API_KEY` ainda abortam o spawn com exit 1
- Os quatro guards OAuth-only em `claude_runner.rs:273`, `codex_spawn.rs:259`, `ingest_claude.rs:282`, `extract/llm_embedding.rs:237-253` não foram alterados; apenas o whitelist env-clear foi estendido
- Helper compartilhado `src/spawn/env_whitelist.rs` expõe `apply_env_whitelist(cmd, strict)`; os três spawners delegam em vez de inlinear o array
- Para ambientes compliance que exigem env_clear estrito (PCI-DSS, SOC2, HIPAA), setar `SQLITE_GRAPHRAG_STRICT_ENV_CLEAR=1` ou passar `--strict-env-clear`; modo estrito preserva apenas `PATH`
- Sem telemetria nova: o fix é silencioso. Nenhum macro `tracing::info!` registra qual provider está em uso. O teste de auditoria no-leak `audit_no_token_leak_in_subprocess_stderr` em `tests/claude_runner_env.rs` garante que o valor literal do token NUNCA aparece em stdout ou stderr mesmo com `RUST_LOG=trace`
- Veja `docs/decisions/adr-0041-preserve-custom-provider-env.pt-BR.md` e `docs/COOKBOOK.pt-BR.md#como-usar-providers-anthropic-compativeis-customizados-v1083` para a receita completa
- Resolve GAP-058 parcialmente: env vars de custom-provider roteiam em torno de contenção de quota OAuth; `recall`/`hybrid-search` permanecem determinísticos sob fadiga OAuth oficial

## O Que Mudou na v1.1.04 — Nested-Runtime Fix + entity-connect Convergence

- Veja [docs/MIGRATION.pt-BR.md](MIGRATION.pt-BR.md) para o caminho de upgrade V016 a partir da v1.1.03. O schema avança v15→v16. Fixe `=1.1.4` apenas se precisar permanecer nessa release.

## O Que Mudou na v1.1.02 — Remoção do GLiNER, TooManyTokens Tipado, Regressão Re-Embed, Prune de Órfãos de Entidade (ADR-0062)
- O nome oficial da release é **v1.1.02**; o `Cargo.toml` carrega `1.1.2` porque o SemVer rejeita zero à esquerda no segmento de patch. O schema permanece INALTERADO em v15 — o upgrade NÃO requer `migrate`. Binário ~19 MiB. Consumidores da biblioteca fixam `=1.1.2`. User-Agent `sqlite-graphrag/1.1.2`.
- **Gap 1 (BREAKING)**: `--gliner-variant` e o enum `GlinerVariant` foram REMOVIDOS do parser — clap rejeita `--gliner-variant` com exit 2 (precedente: `--max-entity-degree` da v1.0.99); `--mode gliner` também foi REMOVIDO (o enum `IngestMode` agora tem apenas `none`); as env vars `SQLITE_GRAPHRAG_GLINER_MODEL`/`SQLITE_GRAPHRAG_GLINER_THRESHOLD` são silenciosamente ignoradas.
- **Gap 2**: `AppError::TooManyTokens{tokens,limit}` é a nova variante tipada exit 6 (junta-se a `BodyTooLarge`/`TooManyChunks`); o envelope JSON informa `{tokens,limit}` para o caller distinguir bytes vs chunks vs tokens.
- **Gap 3**: o dispatch `strip_prefix("entity:")` em `call_reembed` é coberto pelo teste de regressão `tests/reembed_entities_integration.rs` — embeddings de entidades fazem backfill de 0→N e a query de cobertura atinge zero ausentes.
- **Nova flag**: `enrich --prune-dead-entity-orphans` (mutuamente exclusiva com `--prune-dead-orphans`) deleta linhas dead-letter com chave de entidade do `.enrich-queue.sqlite`; teste unitário `prune_dead_entity_orphans_removes_only_entity_dead_rows` + teste de integração `tests/prune_dead_entity_orphans_integration.rs`.
- 4 warnings rustdoc pré-existentes resolvidos (backticks em blocos HTML, intra-doc links cfg(test)).

## O Que Mudou na v1.1.01 — Backfill de Embedding de Entidade/Chunk, Re-Embed Direcionado, graph recompute-degree
- O nome oficial da release é **v1.1.01**; o `Cargo.toml` carrega `1.1.1` porque o SemVer rejeita zero à esquerda no segmento de patch. O schema permanece INALTERADO em v15 — o upgrade NÃO requer `migrate`. Binário ~19 MiB. Consumidores da biblioteca fixam `=1.1.1`.
- **P1**: o embedding de entidade agora roteia pelo caminho REST do OpenRouter mesmo com `--llm-backend none`; um guard de vetor vazio nos upserts previne blobs de embedding de zero bytes.
- **P2**: `enrich --operation re-embed --target memories|entities|chunks|all` seleciona qual tabela de embedding recebe o backfill; `--status` reporta o `scan_backlog` por alvo.
- **P3**: novo comando `graph recompute-degree` recomputa o grau de todas as entidades em uma transação única; suporta `--dry-run`; o envelope reporta `{total, updated, zeroed, unchanged}`. Use-o para corrigir drift de grau acumulado historicamente.
- **P4**: `reclassify-relation --literal-from` casa a relação armazenada de forma verbatim (bypassa a normalização do clap); mutuamente exclusiva com `--from-relation`. A ferramenta para migrar arestas legadas com underscore como `applies_to`.
- **P5**: `merge-entities --ids <a,b> --into-id <N>` e `rename-entity --id <N>` endereçam entidades por id numérico em vez de nome.
- **P6**: `health --json` ganha `vec_memories_missing` / `vec_entities_missing` / `vec_chunks_missing` mais `vec_*_coverage_pct`; `embedding status --json` ganha `memories_missing` / `entities_missing` / `chunks_missing` dentro de `coverage`.
- **P7**: mensagens de erro de `EntityType` agora listam os 13 tipos canônicos de entidade.
- **P10**: o predicado do re-embed também cobre linhas com dimensão divergente e blob vazio, não apenas linhas ausentes.
- **P11**: `AppError::BodyTooLarge` / `AppError::TooManyChunks` são variantes tipadas; o exit 6 é preservado e a mensagem do envelope JSON agora é específica.
- **P12**: `ingest --name-prefix <PREFIX>` prefixa os nomes de memória gerados (apenas no caminho de staging local).

```bash
# Backfill de embeddings ausentes de entidade/chunk (v1.1.01)
sqlite-graphrag enrich --operation re-embed --target entities \
  --mode openrouter --openrouter-model MODEL --json
sqlite-graphrag enrich --operation re-embed --target chunks \
  --mode openrouter --openrouter-model MODEL --json

# Recomputar o grau de todas as entidades em uma transação
sqlite-graphrag graph recompute-degree --dry-run --json
sqlite-graphrag graph recompute-degree --json

# Auditar a cobertura de embeddings
sqlite-graphrag health --json | jaq '{memories: .vec_memories_missing, entities: .vec_entities_missing, chunks: .vec_chunks_missing}'
```

## O Que Mudou na v1.0.99 — Remoção do Degree-Cap + Correções de Doc/Convergência (GAP-SG-67/68/69, ADR-0059)
- **GAP-SG-67 (BREAKING)**: a flag `--max-entity-degree` foi REMOVIDA de `remember` e `link`; passá-la agora falha com clap exit 2, e a mitigação `--max-entity-degree 0` é obsoleta. A poda global destrutiva de grau (`graph::enforce_degree_cap`) foi deletada, tornando a escrita 100% aditiva — nunca poda/deleta arestas nem emite aviso de grau, e o total de `relationships` nunca decresce numa escrita normal. Trade-off: o grau de hubs cresce sem limite; normalização futura é feita apenas via comando de MANUTENÇÃO explícito.
- **GAP-SG-68**: `graph entities --sort-by degree` está documentado corretamente — ordena de forma ascendente por padrão; use `--order desc` para mais-conectado-primeiro. Correção apenas de documentação, sem alteração de comportamento.
- **GAP-SG-69**: `enrich --operation body-enrich ... --until-empty` agora converge; corpos curtos vetados (`status='skipped'`) não são mais re-enfileirados no rescan, e o sidecar `.enrich-queue.sqlite` é mantido enquanto há verditos `skipped` (empiricamente 55→3).
- Sem migração; schema permanece em v15. Consulte ADR-0059 e MIGRATION.md.

## O Que Mudou na v1.0.96 — Dead-Letter no Enrich + Fan-Out REST OpenRouter (GAP-ENRICH-BACKLOG-CONVERGE, GAP-OPENROUTER-REST-CONCURRENCY, ADR-0055)
- **GAP-ENRICH-BACKLOG-CONVERGE**: a fila do enrich ganha o status terminal `dead` mais as colunas `error_class` e `next_retry_at` (`ALTER TABLE` idempotente + `idx_enrich_queue_eligible`). Resultados Transient (rate-limit/timeout/5xx) reagendam com backoff exponencial; um HardFailure vira terminal de imediato; um item vira `dead` após `--max-attempts` retries Transient. O dequeue respeita `next_retry_at` e exclui `dead`, então o conjunto vivo decresce estritamente e o backlog sempre converge.
- `--until-empty` roda um loop interno scan→drain até a fila não ter itens elegíveis ou `--max-runtime` (padrão 3600s) expirar — substitui o loop bash externo de retry. `--max-attempts <N>` (padrão 8, range 1..=20) é o orçamento de retries Transient antes de `dead`.
- `--status` imprime um relatório read-only JSON da fila (`unbound_backlog`, `scan_backlog` por operação, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`). NUNCA chama o LLM e NUNCA adquire o singleton — seguro para poll enquanto um drain roda; o `scan_backlog` (GAP-SG-77, v1.1.0) é o backlog real do banco por operação que um scan enfileiraria — elimina o falso `pending=0` para `entity-descriptions`/`body-enrich`/`re-embed`, e o `state` deriva o `pending-scan` dele.
- **GAP-OPENROUTER-REST-CONCURRENCY**: `--rest-concurrency <N>` (padrão 8, clamp 1..=16) limita um fan-out REST via `JoinSet` bounded para `--mode openrouter` (distinto de `--llm-parallelism`). O embedding processa lotes de 32 passagens com a ordem por chunk preservada; a escrita SQLite permanece serializada via WAL + claim atômico (single-writer intacto).
- Sem migração; schema permanece v15. nextest: 1086 passed, 0 failed, 6 skipped. Consulte ADR-0055.

```bash
# Drenar o backlog do enrich até convergir (sem loop externo)
printf "%s" "sk-or-v1-sua-chave-aqui" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY não é lida em runtime (G-T-XDG-04)
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "deepseek/deepseek-v4-flash:nitro" \
  --until-empty --rest-concurrency 8 --json

# Inspecionar a fila sem rodar o LLM (sem singleton, sem tokens)
sqlite-graphrag enrich --status \
  --mode openrouter --openrouter-model "deepseek/deepseek-v4-flash:nitro" --json
```


## O Que Mudou na v1.0.95 — JUDGE do Enrich via OpenRouter (GAP-OR-ENRICH, ADR-0054)
- **GAP-OR-ENRICH**: `enrich --mode openrouter` roteia a etapa JUDGE para o endpoint REST `/chat/completions` do OpenRouter. Nenhum subprocesso de CLI local é spawnado. O pipeline SCAN→JUDGE→PERSIST permanece inalterado; só o transporte do JUDGE muda.
- O único modo do enrich é `openrouter`.
- `--openrouter-model` é **OBRIGATÓRIA** com `--mode openrouter` (SEM default). Omiti-la → exit 1 ANTES de qualquer chamada de rede.
- `--openrouter-api-key` lê da XDG via `config add-key` (OPENROUTER_API_KEY is not read at runtime) ou de `config add-key --provider openrouter`. `--openrouter-timeout` tem default de 300s. `--openrouter-base-url` é opcional.
- A requisição usa `response_format` `json_schema` com `strict: true` e `provider.require_parameters: true`. `reasoning.enabled: false` com fallback reasoning-mandatory (uma retentativa omitindo `reasoning`). `usage.cost` é lido da resposta (`usage: {include: true}` está deprecado).
- 13/13 modelos reais passam. Trade-off: OAuth zero-token (modos CLI locais) vs tokens cobrados na chave OpenRouter em XDG (OPENROUTER_API_KEY não é lida em runtime) (modo OpenRouter). Sem migração; schema permanece v15. Consulte ADR-0054.

```bash
# JUDGE do enrich via REST OpenRouter (sem subprocesso)
printf "%s" "sk-or-v1-sua-chave-aqui" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY não é lida em runtime (G-T-XDG-04)
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" --json
```


## O Que Mudou na v1.0.94 — Remediação de Quatro Gaps (ADR-0053)
- **GAP-OR-ENTITY-EMBED**: O embedding de entidades em `remember`/`remember-batch`/`ingest` agora honra `--embedding-backend openrouter`, roteando via OpenRouter REST. `remember` com entidades novas cai de ~119s para ~0,9s.
- **GAP-EMBED-DIM-64**: `DEFAULT_EMBEDDING_DIM` elevado de 64 para **384** (`constants.rs:29`). Bancos novos usam dim 384 por padrão. Bancos legados em dim 64 são preservados via `schema_meta.dim` — sem re-embed forçado.
- **GAP-EMBED-TIMEOUT-300**: `DEFAULT_EMBED_TIMEOUT_SECS` elevado de 120 para **300** (`llm_embedding.rs:43`).
- **GAP-HEADLESS-DEFAULT**: `enrich --mode` agora é **OBRIGATÓRIO** (`default_value = "claude-code"` removido em `enrich.rs:379`). Omitir `--mode` → clap exit 2. Adicione `--mode codex` / `--mode claude-code` / `--mode opencode` a todas as invocações de `enrich --operation`.

**Mudança quebrante**: `enrich --operation <op>` agora requer `--mode <valor>`. Consulte o [guia de MIGRAÇÃO](MIGRATION.pt-BR.md) para a tabela de pareamento canônico.

## O Que Mudou na v1.0.93 — Backend de Embedding OpenRouter (GAP-OR-INGEST)
- Novos flags globais: `--embedding-backend auto|openrouter|llm`, `--embedding-model MODEL`, `--openrouter-api-key KEY`
- Embedding via API REST OpenRouter substitui subprocess LLM para geração de vetores (~200ms vs 15s por chamada)
- `EmbeddingBackendChoice` propagado para TODOS os 13 paths de embedding: `remember`, `remember-batch`, `ingest`, `recall`, `edit`, `restore`, `hybrid-search`, `deep-research`, `enrich`, `init`, `rename-entity`, `ingest` (modo claude), `remember` (embedding de chunks)
- Novo flag `--enrich-after` para ingest dispara `enrich --operation memory-bindings` após embedding
- O usuário DEVE especificar `--embedding-model` ao usar `--embedding-backend openrouter` — SEM modelo padrão
- Defina chave API via `config add-key --provider openrouter` (OPENROUTER_API_KEY is not read at runtime) ou flag `--openrouter-api-key`
- 10 modelos verificados E2E: Qwen 4B/8B, NVIDIA Nemotron (gratuito), OpenAI small/large, Perplexity, Mistral, BAAI bge-m3, Google Gemini 001/002
- Todos os modelos produzem vetores de 384 dims via MRL — zero mudança de schema, zero migração
- **GAP-OR-PROPAGATION** (v1.0.93): 5 paths de embedding adicionais corrigidos — `enrich --operation re-embed`, `init` (probe de dimensão), `rename-entity`, `ingest --mode claude-code` (4 call sites) e `remember` (embedding paralelo de chunks) agora honram `--embedding-backend openrouter`
- **BUG-OR-EXIT-CODE** (v1.0.93): Erros de configuração OpenRouter (chave ausente, modelo ausente, chave inválida) agora retornam exit code 78 (`EX_CONFIG`) em vez de exit 1
```bash
# Configuração
printf "%s" "sk-or-v1-sua-chave-aqui" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY não é lida em runtime (G-T-XDG-04)

# Remember com OpenRouter
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  remember --name minha-nota --type note \
  --description "embedding rápido" --body "conteúdo" --json

# Ingest com OpenRouter + auto-enrich
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  ingest ./docs --pattern "*.md" --recursive \
  --enrich-after --llm-backend openrouter --json
```


## O Que Mudou na v1.0.90, v1.0.91

### v1.0.91 — Isolamento de CWD, Correção de Degree, 6-Gap Doc Remediation

- **GAP-SPAWN-001**: `apply_cwd_isolation()` adicionado em `src/spawn/mod.rs` — define `current_dir(temp_dir)` e `CLAUDE_CONFIG_DIR=temp_dir` em TODOS os 10 sites de spawn de subprocessos LLM. Elimina interferência de walk-up de `.mcp.json`. O workaround manual `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1 CLAUDE_CONFIG_DIR=/tmp/graphrag-empty-config` NÃO É MAIS NECESSÁRIO
- **GAP-SPAWN-002**: `cleanup_spawn_dir()` adicionado em `src/main.rs` — remove diretório de spawn ao final do processo via `remove_dir()` não-recursivo
- **BUG-14**: Teste `opencode_adapter_build_args` corrigido — assertava `"headless"` mas adapter retorna `"run"` desde refatoração v1.0.90
- **BUG-15**: 7 JSON schemas atualizados de `backend_invoked: enum ["claude", "codex", "none"]` para `["claude", "codex", "opencode", "none", "auto"]`. Afetados: `embedding-status`, `enrich-summary`, `hybrid-search`, `recall`, `remember`, `ingest-summary`, `edit`
- **BUG-16**: `deep-research.schema.json` ganhou `vec_degraded: boolean` em `ResearchStats` (ausente, violava `additionalProperties: false`)
- **BUG-17 (ALTA)**: Inflação de `entities.degree` corrigida — `remember` e `ingest` agora usam `recalculate_degree()` após inserção de relações em vez de `increment_degree()` por entidade. `graph stats`, `graph entities` e tabela `entities` agora consistentes

### v1.0.90 — Integração Backend OpenCode (ADR-0051)

- Terceiro backend LLM: `--llm-backend opencode` spawna OpenCode CLI headless via `opencode run --format json --dangerously-skip-permissions`
- Novas flags: `--opencode-binary`, `--opencode-model`, `--opencode-timeout`; env vars `SQLITE_GRAPHRAG_OPENCODE_BINARY`, `SQLITE_GRAPHRAG_OPENCODE_MODEL`, `SQLITE_GRAPHRAG_OPENCODE_EMBED_MODEL`, `SQLITE_GRAPHRAG_OPENCODE_TIMEOUT`
- Modelo padrão: `opencode/big-pickle`; modelos gratuitos: `opencode/deepseek-v4-flash-free`, `opencode/mimo-v2.5-free`, `opencode/nemotron-3-ultra-free`, `opencode/north-mini-code-free`
- Cadeia de fallback: `--llm-backend codex,claude,opencode,none` tenta cada backend em ordem
- `--mode opencode` para pipelines de extração de entidades em `ingest` e `enrich`
- Saída NDJSON do opencode tem 3 tipos de evento: `step_start`, `text`, `step_finish`
- 24 bugs/gaps remediados; auditoria completa de skills com ADR-0051

## O Que Mudou na v1.0.86, v1.0.87, v1.0.88, v1.0.89 (ADR-0045, ADR-0046, ADR-0047, ADR-0048, ADR-0049)

Desde a v1.0.85.2, quatro releases introduziram a superfície LLM-heavy, a camada de validação pre-flight, três hotfixes e o contrato de schema como artefato derivado.

### v1.0.86 — Superfície LLM-Heavy e Semáforo de Slots Host-Wide

- Cinco novos subcomandos expõem o pipeline de subprocessos LLM: `pending list`, `pending show`, `pending cleanup`, `embedding status`, `embedding list`, `embedding abandon`, `pending-embeddings list`, `pending-embeddings process`, `slots status`, `slots release`
- `pending` (V014 — tabela `pending_memories`) fornece checkpoint de 3 estágios para o pipeline `remember`. O checkpointer sobrevive a crash; no restart, `pending list` inspeciona a fila e `pending show <id>` lê uma entrada
- `embedding status --status pending|in_progress|done|abandoned` expõe o pipeline retry-fallback
- `slots status` reporta `max_concurrency`, `acquired`, `waiting`, `held_by_pid[]`; `slots release --slot-id N --yes` ceifa slots órfãos
- Novas flags globais: `--max-concurrency <N>`, `--wait-lock <SECONDS>`, `--llm-parallelism <N>` (padrão 4, clamp [1, 32]), `--ingest-parallelism <N>`, `--graceful-shutdown-secs <N>`, `--skip-embedding-on-failure` (válido apenas com `--llm-backend …,none`)
- Contenção de lock via `fs4 = 0.9` com `fcntl(F_SETLK)` em Unix e `LockFileEx` em Windows (ADR-0039)

### v1.0.87 — Camada de Validação Pre-Flight (ADR-0045, GAP-META-005)

- Novo módulo `src/spawn/preflight.rs` (≥200 linhas, 7 guards, 15 testes unitários) porta todo spawn de subprocesso LLM ANTES do fork
- Nova variante `AppError::PreFlightFailed(PreFlightError)` com `exit_code() == 16` e `is_permanent() == true`
- Novo exit code 16 (`EX_CONFIG`) para falhas pre-flight. Não documentado em nenhuma tabela de exit code pré-existente
- Os 7 guards em ordem: `check_argv_size` (argv excederia ARG_MAX menos 4 KB), `check_binary_exists` (claude/codex alcançável em PATH), `check_mcp_config_inline` (substitui `--mcp-config "{}"` literal por tempfile com `{"mcpServers":{}}`), `check_mcp_config_path` (valida conteúdo JSON), `check_walkup_mcp_json` (rejeita `.mcp.json` inválido em cadeia ancestral do workspace), `check_output_buffer` (eleva buffer do parser acima de 64 KB), `check_claude_config_dir` (evita vazamento MCP user-level)
- Bypass em emergências: `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1` desabilita todos os 7 guards. Bypassing reverte para `Command::spawn()` direto e herda todas as 5 classes BUG do GAP-META-005
- Os 4 spawners (`claude_runner`, `codex_spawn`, `ingest_claude`, `extract/llm_embedding`) compartilham este módulo único

### v1.0.88 — Hotfixes BUG-11/12/13 (ADR-0046, ADR-0047)

- **BUG-11 (CRÍTICO)** corrigido: falha pre-flight em `extract/llm_embedding.rs:563-565` agora propaga para `remember` via `embed_via_backend_strict` em vez de persistência silenciosa com `backend_invoked: "none"`
- **BUG-12 (MÉDIO)** corrigido: enforço OAuth-only agora emite 1 linha stderr (eram 2) — `eprintln!` duplicado removido
- **BUG-13 (MÉDIO)** corrigido: `link --create-missing` agora respeita validação de nome de entidade; abreviações ALL_CAPS rejeitadas eram aceitas via CLI
- 11 novos regression tests: `tests/bug11_preflight_regression.rs` (2), `oauth_stderr_emits_single_line_v1088` (1), `tests/entity_validation_integration.rs` (8)
- Renomeação de teste `embed_with_fallback_succeeds_via_none_when_chain_exhausts` → `embed_with_fallback_chain_of_only_none_aborts_without_skip_on_failure_v1088` documenta o contrato corrigido

### v1.0.89 — Schema Drift, Flag Parity, Description Heuristic (ADR-0048, ADR-0049)

- **GAP-E2E-007 (P1)**: `health.schema.json` regenerado via `schemars` derive macro. 17 novos campos adicionados; `additionalProperties: true` (política Must-Ignore por RFC 7493 I-JSON). Novo binário: `cargo run --bin dump-schema` regenera 70+ schemas
- **GAP-E2E-008 (P3)**: `embedding status/list/abandon`, `pending list/show` agora aceitam `--db <PATH>`. `clap::Arg::global = true` foi REJEITADO (invasivo, polui help). 5 novos testes em `tests/cli_db_flag_parity_regression.rs`
- **GAP-E2E-009 (P3)**: `migrate --dry-run --json` agora reporta migrações pendentes sem aplicar. 1 novo teste em `tests/migrate_dry_run_regression.rs`
- **GAP-E2E-010 (P3)**: `codex-models --json` aceito como no-op; paridade de `pending list --db <PATH>`. Ambos com `#[arg(long, hide = true)]`. 1 novo teste em `tests/codex_models_json_regression.rs`
- **GAP-E2E-011 (P2)**: `ingest --auto-describe` (padrão true) extrai descrição da primeira linha significativa do corpo (>20 chars, não header). `extract_heuristic_description(body, path_hint)` cai para o stem do arquivo. Opt-out via `--no-auto-describe`. 5 novos testes em `tests/ingest_auto_describe_regression.rs`
- **GAP-E2E-002 (P3)**: `health --namespace <NS> --json` filtra contagens para um único namespace. 1 novo teste em `tests/health_namespace_regression.rs`
- **GAP-E2E-001 (P2)**: Tamanho do binário 14.6 MiB documentado em `Cargo.toml:6` (era 6 MB desde v1.0.76). 1 novo teste em `tests/binary_size_documented_regression.rs`
- Total: 1059 testes passando. Binário 15.3 MB ELF stripped
## O Que a v1.0.82 Mudou (Cinco Gaps, Duas Migrações, Quatro Subcomandos)

A v1.0.82 é um bump de **patch** que CARREGA duas migrações aditivas de banco (`V014__pending_memories`, `V015__pending_embeddings`). A versão de schema avança de 13 para 15. Consumidores de biblioteca devem pinar em `=1.0.82` conforme a política de estabilidade (ADR-0032). Os 5 gaps fechados: GAP-001 fila de checkpoint de três estágios no `remember` (ADR-0036), GAP-002 envelope JSON de shutdown no exit code 19 (ADR-0037), GAP-003 flag de escolha do usuário `--llm-backend` (ADR-0038), GAP-004 semáforo de slots LLM host-wide via `fs4` (ADR-0039), GAP-005 cadeia de fallback com captura de stderr que mitiga o incidente de OAuth 401 do codex em 2026-06-14 (ADR-0040).

- **GAP-001 (ADR-0036)**: a tabela `pending_memories` (V014) bufferiza corpo, entidades e relacionamentos separadamente; SIGTERM durante o estágio 2 ou 3 deixa a linha em `queued` para reprocessamento. Inspecione com `sqlite-graphrag pending list|show|cleanup --json`.
- **GAP-002 (ADR-0037)**: constante `SHUTDOWN_EXIT_CODE = 19` em `src/constants.rs`; qualquer comando que spawna LLM e recebe SIGTERM/SIGINT/SIGHUP emite um envelope JSON determinístico no stdout. Campos do envelope: `error`, `code`, `signal`, `graceful`, `message`. Schema: `docs/schemas/shutdown-envelope.schema.json`.
- **GAP-003 (ADR-0038)**: flag global `--llm-backend <codex|claude|none,codex,...>`; o primeiro backend sem erro vence. `--llm-backend codex,claude,none` combinado com `--skip-embedding-on-failure` permite embedding nulo quando ambos os backends falham.
- **GAP-004 (ADR-0039)**: semáforo de slots LLM host-wide via `fs4 = "0.9"` com feature `sync` (NÃO `fs2`); `fcntl(F_SETLK)` no Linux/macOS, `LockFileEx` no Windows. Default `min(ncpus, oauth_tier_max)`. Inspecione com `sqlite-graphrag slots status --json`; libere com `sqlite-graphrag slots release --slot-id <N> --yes`.
- **GAP-005 (ADR-0040)**: a tabela `pending_embeddings` (V015) guarda linhas que falharam em todos os backends; a cadeia de captura de stderr detecta `refresh_token_reused` (incidente do codex em 2026-06-14) e roteia para o próximo backend. Inspecione com `sqlite-graphrag embedding status|list --json`; retente com `sqlite-graphrag pending-embeddings process`.

## O Que Mudou na v1.0.80 (G45, G53, G55 S2, G56, G58, ADR-0033, ADR-0034)

A v1.0.80 é bump **patch** SEM migração de banco. O schema continua
v13, a adoção de dim do G43 já roda em todo `open_rw` e `open_ro`,
e as mudanças são todas aditivas no nível binário e de banco.
Consumidores da biblioteca devem fixar em `=1.0.80` porque a API
da lib é instável dentro de v1.x.y (ADR-0032).

- **G45 singleton de embedding cross-process**: `acquire_embedding_singleton(namespace, db_path, wait_seconds, force)` serializa chamadas de embedding LLM por par `(namespace, db)` entre invocações CLI concorrentes. Uma segunda CLI tentando embedar contra o mesmo banco recebe `AppError::EmbeddingSingletonLocked { namespace }` (exit 75, retentável). Passe `--wait-embed-singleton <SEGUNDOS>` para fazer poll até a soltura do lock; bancos ou namespaces distintos adquirem locks independentes. Operacionalmente previne a patologia de "duas invocações de remember, dois subprocessos LLM, dois batches paralelos" que o cache em processo da v1.0.79 não conseguia endereçar.
- **G53 política de estabilidade e gate de CI `semver-checks`**: o contrato público é a CLI; a API da biblioteca é instável em v1.x.y. Novo job de CI `semver-checks` roda `cargo semver-checks check-baseline --baseline-version 1.0.79` em modo informativo (vira bloqueante em v1.0.81 quando as 9 violações MAJOR pendentes forem resolvidas). README e CHANGELOG carregam a seção `Política de Estabilidade`. Fixe em `=1.0.80` para consumidores da lib; use `^1.0` para permanecer na trilha de estabilidade da CLI.
- **G55 S2 `MemoryNotFound` estrutural**: o caminho legado `NotFound(String)` que mascarava qual alvo de lookup falhou é substituído por `AppError::MemoryNotFound { name, namespace }` e `AppError::MemoryNotFoundById { id }` dentro de `read` e `hybrid-search`. O identificador agora é parte da variante, eliminando a classe de bugs `not found: unknown`. As mensagens em pt-BR carregam nome e namespace explicitamente.
## O Que Mudou em v1.0.85, v1.0.85.1, v1.0.85.2 (ADR-0043, ADR-0044)

Desde v1.0.84 (GAP-002 split do backend Claude, ADR-0042), três releases adicionais apertaram o embedder:

### v1.0.85 — Remediação de Cinco Gaps (ADR-0043)
- Enum `FallbackReason` estendido de 3 para 7 variantes: `embedding_failed | slot_exhausted | oauth_quota | backend_mismatch | dim_zero | cancelled | timeout`
- Discriminador `reason_code` nos envelopes `recall` e `hybrid-search` distingue quota vs mismatch vs timeout
- `try_embed_query_with_deterministic_fallback` retenta em `OAuthQuota` e aplica teto de 750ms em `SlotExhausted` antes de cair em FTS5
- 12-14 headers `anthropic-ratelimit-*-remaining` capturados em `LlmEmbedding::invoke_claude` (G45-CR5); `0` aborta embed e dispara fallback codex
- Lock de `dim 64` (Matryoshka Representation Learning, arXiv 2205.13147) reduz gasto de tokens OAuth em 6x (G56)
- 5 testes de regressão em `tests/embedder.rs`

### v1.0.85.1 — Fallback Gracioso `--llm-backend none` em `recall`/`hybrid-search` (hotfix GAP-004)
- `--llm-backend none` agora retorna exit 0 com `vec_degraded: true` + `source: "fts_fallback"` + `vec_degraded_reason: "dim_zero"`
- Failsafe do v1.0.80 restaurado para o caso `--llm-backend none`
- Braço intermediário `Ok((v, _backend)) if v.is_empty() => Err(FallbackReason::DimZero)` em `try_embed_query_with_choice`

### v1.0.85.2 — `embed_via_backend` Resolved Kind, `--dry-run-backend` Standalone (BUG-001/002/003, ADR-0044)
- `--dry-run-backend` funciona standalone (sem subcommand) graças a `pub command: Option<Commands>` em `src/cli.rs:248`
- `embed_via_backend` retorna `Result<(Vec<f32>, LlmBackendKind), AppError>` propagando `resolved_kind`
- 7 envelopes agora reportam `backend_invoked: "claude" | "codex" | "none"` consistentemente
- `setup_mock_path()` em `tests/embedder.rs:37-77` alinhado para emitir JSON (não JSONL)

### v1.0.84 — Split do Backend Claude (ADR-0042, GAP-002)
- `--llm-backend claude` agora força invocação de `claude -p`, sem fallback silencioso para codex
- `LlmEmbeddingBuilder` em `src/extract/llm_embedding.rs` com `with_claude_builder`, `with_codex_builder`, `override_binary`, `override_model`
- `embed_via_claude_local` em `src/embedder.rs:190+` é o entry point do split real
- `apply_env_whitelist_for_claude` em `src/spawn/env_whitelist.rs` (compartilhado por `invoke_claude` e `embed_via_claude_local`)
- 5 testes de regressão em `tests/embedder.rs`

- **G56 cache de entity-embed em processo**: `embed_entity_texts_cached` fica na frente de `embed_passages_parallel_local` para batches de nome de entidade. Chave do cache é `blake3(model || "\0" || text)`. Taxa de hit alta em `ingest` (entidades canônicas re-embedadas entre muitas memórias), modesta em `remember` e `remember-batch`. `remember.rs`, `ingest.rs` e `remember_batch.rs` roteiam embeddings de entidade pelo cache; embeddings de chunk continuam no caminho raw. Stats são emitidas via `tracing::debug!` (contagens hit / miss / request).
- **G58 fallback FTS5 para `recall` e `hybrid-search`**: `recall --fallback-fts-only` e `hybrid-search --fallback-fts-only` roteiam a query via FTS5 BM25 quando o subprocesso LLM falha (rate limit, contenção OAuth, dim divergente). Os novos campos do envelope `vec_degraded` (bool), `vec_error` (string) e `warning` (string) são preenchidos simetricamente em ambos os comandos. Os testes de `recall` e `hybrid-search` ganharam cobertura para o caminho FTS5-only; 1 teste é `#[ignore]` porque o stub G58 S1 exige `PATH` sem `codex` ou `claude` para exercitar `EmbeddingFailed`.
- **G53-WINDOWS-INFRA (ADR-0033)**: os jobs `clippy` e `test` da matrix windows-2025 ganharam 2 steps novos cada (gateados `if: matrix.os == 'windows-2025'`, no-op em ubuntu/macos): um pre-warm que baixa o toolchain rustup no cache do runner antes do build, e um verify step que re-checa `rustup show active-toolchain` após install. Os 2 modos históricos de falha de infra (download do rustup com erros transitórios de rede e `E0463 can't find crate for core` quando a stdlib do target está ausente) agora são recuperáveis na primeira re-run em vez de acumularem como CI vermelho. Validação local de cross-compile: `cargo check --target x86_64-pc-windows-msvc --lib --all-features` reproduzido e o `E0463` resolvido via `rustup target add x86_64-pc-windows-msvc --toolchain 1.88`; o build então atinge a fronteira `cc-rs: failed to find tool "lib.exe"`, que é o limite esperado de cross-compile MSVC a partir de host Linux.
- **Resiliência de SHUTDOWN (ADR-0034)**: `src/signals.rs` é envolvido em uma barreira de captura de panic; mesmo quando o stderr do pai é um pipe fechado (o cenário de processo órfão que a auditoria G42/C2 identificou), o handler retorna limpo em vez de `SIGABRT`-ar em `BrokenPipe`. O terceiro Ctrl-C consecutivo sai com código 130 e ZERO I/O, casando com o contrato documentado em ADR-0034 e a receita em `docs/HEADLESS_INVOCATION.md`. A receita de bypass SHUTDOWN em 3 camadas (`nohup` então `setsid` então `disown`) é a referência canônica para o harness do agente ao rodar jobs longos de embedding em background.

## O Que Mudou na v1.0.79 (G42 + G43)

O trabalho do G42 tornou o pipeline de embedding rápido, paralelo e em lote; o G43 tornou universal a adoção da dimensionalidade:

- A dimensionalidade default de embedding caiu de 384 para 64 (configurável via `SQLITE_GRAPHRAG_EMBEDDING_DIM`, faixa [8, 4096]); bancos pré-existentes mantêm a `schema_meta.dim` registrada em todo comando (adoção em `open_rw`/`open_ro`, G43).
- Chamadas de embedding são em lote (`{items:[{i,v}]}`; chunks em 8, nomes de entidade em 25 em dim 64; adaptativos à dim — G44) e rodam em paralelo sob semáforo bounded: `--llm-parallelism` em `remember` (default 4), `ingest` (default 2) e `edit` (default 4), clamp [1, 32].
- `SQLITE_GRAPHRAG_CLAUDE_EMBED_MODEL` seleciona o modelo de embedding do claude; `SQLITE_GRAPHRAG_EMBED_TIMEOUT_SECS` (default 300) limita cada chamada LLM.
- `enrich --operation re-embed` e `edit --force-reembed` são os caminhos canônicos de re-embedding.
- O código restante do daemon foi deletado; as features `embedding-legacy` e `ner-legacy` foram removidas; `--enable-ner` é somente URL-regex; as flags da era GLiNER foram REMOVIDAS em v1.1.02 (`--gliner-variant` rejeitada pelo clap com exit 2, `--mode gliner` rejeitada, e as env vars `SQLITE_GRAPHRAG_GLINER_MODEL`/`SQLITE_GRAPHRAG_GLINER_THRESHOLD` silenciosamente ignoradas).


## O Que Mudou na v1.0.76

O build padrão agora é **apenas LLM e one-shot**. Não há modelo local de embedding, não há NER GLiNER, não há runtime ONNX, não há extensão C do `sqlite-vec`. Cada `remember`, `ingest`, `edit` spawna um subprocesso headless de LLM (CLI do claude code ou codex) que devolve o embedding e, opcionalmente, as entidades extraídas.

A CLI é one-shot: não há daemon, não há modelo a manter em memória, não há socket a limpar. O binário de release tem ~14.6 MiB (era 39 MB) e o cold start é 1-3 s (era 30 s com a carga do modelo ONNX).


## Pré-Requisitos

Você precisa de uma **chave de API do OpenRouter**. Nenhuma CLI precisa estar
instalada e nada precisa estar no `PATH`: os backends headless `claude` /
`codex` / `opencode` foram REMOVIDOS na v1.2.0, e o embedding é uma chamada
REST comum.

Guarde a chave na config XDG — nunca em variável de shell, nunca em argv:

```bash
echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin
sqlite-graphrag config doctor --json
```

O `config doctor` informa se a chave resolve antes de você gastar qualquer
coisa. O `sqlite-graphrag health --json` traz o mesmo sinal no check
`embedding_key` (chamado `llm_cli` até a v1.2.4).


## Credenciais

As chaves ficam em `~/.config/sqlite-graphrag/config.toml` com modo `600`, são
zeroizadas no drop e nunca são logadas. A precedência é a flag
`--openrouter-api-key` primeiro, depois o armazenamento XDG, depois nada.

PROIBIDO como mecanismo de configuração: qualquer variável de ambiente de
produto `SQLITE_GRAPHRAG_*`. Ela não é lida no hot path, então exportá-la não
muda nada e ainda esconde a configuração real.


## Instalação

```bash
cargo install sqlite-graphrag --version 1.2.5 --force
```

Verifique:

```bash
sqlite-graphrag --version
# sqlite-graphrag 1.2.5
```


## Inicializar um Banco

```bash
sqlite-graphrag init --namespace meu-projeto
```

O comando `init`:

1. Cria `graphrag.sqlite` no diretório atual.
2. Roda todas as migrações incluindo V013 (dropa vec tables, cria `memory_embeddings`, `entity_embeddings`, `chunk_embeddings`).
3. Spawna a LLM uma vez para confirmar que a sessão OAuth é válida.
4. Reporta `schema_version: 15` no sucesso.

O primeiro `init` é lento (1-3 s de round-trip LLM). Chamadas subsequentes são no-ops (o schema já está na versão alvo).


## Persistir Sua Primeira Memória

```bash
sqlite-graphrag remember \
    --name decisao-auth-2026-06 \
    --type decision \
    --description "Estratégia de rotação de token JWT com expiração de 15 min" \
    --body "Escolhemos JWT com access token de 15 minutos e
    refresh token de 7 dias. O fluxo de refresh usa cookies HttpOnly.
    Veja https://auth0.com/docs/refresh-tokens para a especificação." \
    --entities-file entidades.json
```

Onde `entidades.json` é:

```json
[
  {"name": "JWT", "entity_type": "concept"},
  {"name": "Auth0", "entity_type": "tool"}
]
```

O comando `remember`:

1. Chama a LLM para embutir o corpo — em lote e em paralelo desde a v1.0.79 (`--llm-parallelism`, default 4; 1-3 s por chamada).
2. Armazena a memória em `memories` (indexada por FTS5).
3. Armazena o embedding como BLOB em `memory_embeddings`.
4. Liga as entidades via tabela `entities`.
5. Retorna JSON com `memory_id`, `version`, `elapsed_ms`.


## Buscar Memórias

Os dois comandos principais de busca são:

```bash
# Busca por token exato + semântica, fundida via RRF
sqlite-graphrag hybrid-search "design auth jwt" --k 10 --json

# Apenas semântica (sem componente FTS5)
sqlite-graphrag recall "design auth jwt" --k 5 --no-graph --json
```

Para o tamanho padrão de namespace (10k memórias ou menos), o refinamento por cosseno sobre o BLOB de embedding é rápido o suficiente (ms de dígito único). Para namespaces maiores, prefira `hybrid-search` para que o FTS5 faça a filtragem grossa.


## Faixas de Argumentos Numéricos (v1.2.7)

Desde a v1.2.7, treze argumentos numéricos da superfície de leitura
carregam um validador de faixa do clap. O valor é conferido em tempo de
parse, então um número inválido é recusado **antes** de o banco ser
aberto e antes de qualquer alocação ser dimensionada a partir dele.

| Faixa | Argumentos |
| --- | --- |
| `1..=4096` (top-k) | `recall -k`, `hybrid-search -k`, `related --limit`, `graph entities --limit`, `deep-research --k`, `deep-research --max-results` |
| `1..=1000000` (limite de listagem) | `export --limit`, `pending --limit`, `pending-embeddings --limit`, `embedding --limit` |
| `1..=64` (saltos) | `related --max-hops` (alias `--hops`), `recall --max-hops`, `graph traverse --depth`, `deep-research --max-hops` |
| `1..=64` (sub-consultas) | `deep-research --max-sub-queries` |

Um valor fora da faixa sai com **código 2** e mensagem de faixa do clap
no stderr. Não há envelope JSON para essa falha: o argumento nunca
chega ao comando, então nada estruturado foi produzido ainda. Trate o
exit 2 aqui como erro do chamador, não como condição retentável.

```bash
# Recusado em tempo de parse — exit 2, banco intocado
sqlite-graphrag related --db ./graphrag.sqlite jwt --limit 999999999

# Aceito
sqlite-graphrag related --db ./graphrag.sqlite jwt --limit 50 --json
```

Antes da v1.2.7 um `related --limit` gigante era repassado a
`Vec::with_capacity`, e o processo abortava na alocação sem envelope
algum. O validador de faixa substitui esse abort por uma recusão
determinística em tempo de parse.

Os tetos têm fonte única em `src/constants/search.rs` como
`K_QUERY_RANGE_MAX`, `K_LIST_LIMIT_MAX`, `K_MAX_HOPS_CEILING` e
`K_MAX_SUB_QUERIES_CEILING`.


## Extrair Entidades via LLM

O `remember` padrão faz apenas extração de URL. Para NER completo (entidades + relacionamentos tipados), use o backend LLM:

```bash
sqlite-graphrag remember \
    --name revisao-design-t2 \
    --type note \
    --description "Notas da revisão de design do T2" \
    --body "$(cat revisao-design.md)" \
    --extraction-backend llm
```

A LLM devolve JSON estruturado com entidades e relacionamentos no mesmo prompt que produz o embedding. O round-trip total é 3-8 s (mais longo que o caminho de só embedding porque o prompt inclui o schema e a resposta é maior).


## Ferramentas de Qualidade LLM (herdadas da v1.0.69)
### `enrich` — Qualidade do Grafo Aumentada por LLM
- O subcomando `enrich` executa operações de qualidade do grafo curadas por LLM. Totalmente implementadas (persistem): `memory-bindings` (extrai entidades de memórias órfãs), `augment-bindings` (vínculos extras em já vinculadas; exige `--names`/`--names-file`), `entity-descriptions` (preenche descrições NULL/vazias), `body-enrich` (expande corpos curtos), `re-embed` (só vetores), `entity-connect` (v1.1.04+ convergente via `entity_connect_seen`; **v1.1.06** scan O(k) coocorrência + hub×ilha, chaves `pair:{id1}:{id2}` / `item_type=entity_pair`, primeiro scan com InterruptHandle → Timeout exit 1), `cross-domain-bridges` (mesmo caminho fully-implemented de entity-connect / `entity_connect_seen`), e `body-extract` com `--body-extract-graph-only` (só grafo, sem reescrever o corpo).
- Operações restantes apenas de varredura exibem listas candidatas sem reescrever: `weight-calibrate`, `relation-reclassify`, `entity-type-validate`, `description-enrich`, `domain-classify`, `graph-audit`, `deep-research-synth` (e `body-extract` sem `--body-extract-graph-only` quando usado de forma consultiva).
- `--mode openrouter` seleciona o provedor do JUDGE e é **OBRIGATÓRIA** — NÃO há default (o default `claude-code` foi removido na v1.0.94). `claude-code`, `codex` e `opencode` são CLIs locais OAuth-only; `openrouter` (v1.0.95) chama o endpoint REST `/chat/completions` sem subprocesso.
- Com `--mode openrouter` (v1.0.95): `--openrouter-model` é OBRIGATÓRIA (SEM default; omiti-la → exit 1 antes de qualquer chamada de rede). `--openrouter-api-key` lê da XDG via `config add-key` (OPENROUTER_API_KEY is not read at runtime) ou de `config add-key --provider openrouter`. `--openrouter-timeout` tem default de 300s. `--openrouter-base-url` é opcional. Exemplo: `enrich --operation memory-bindings --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" --json`.
- `--preflight-check` confirma que a chave do OpenRouter resolve ANTES de varrer o conjunto candidato. Padrão desligado para manter `--dry-run` e fluxos automatizados com custo zero.
- `--rate-limit-buffer <SEGUNDOS>` padrão 300. Quando a sondagem detecta que o reset do rate limit OAuth está a menos do que o buffer de distância, aborta com sugestão para esperar.
- `--names <a,b,c>` e `--names-file <CAMINHO>` selecionam um subconjunto específico de nomes de memória em vez de varrer todos os candidatos. `--names-file` aceita comentários `#` e linhas em branco. As duas flags se combinam como união quando ambas estão setadas.
- `--preserve-threshold <FLOAT>` (padrão 0.7) controla o portão de similaridade trigrama Jaccard para `body-enrich`. Quando a reescrita do LLM pontua abaixo do threshold, o corpo enriquecido é REJEITADO e emitido como `EnrichItemResult::PreservationFailed`. Protege contra invenção do LLM.
- `--llm-parallelism <N>` spawna N threads de worker LLM em paralelo (padrão 1, máximo 32). Codex tolera até 16 em produção; Claude avisa acima de 4 por causa da fan-out OAuth-MCP. Desde a v1.0.79 a mesma flag também existe em `remember` (default 4), `ingest` (default 2) e `edit` (default 4) para o fan-out de embedding.
- `--max-load-check` recusa iniciar quando o load average de 1 minuto excede `2 × ncpus`. Defina como false em runners de CI disputados.
- `--circuit-breaker-threshold <N>` (padrão 5) aborta o job após N resultados `HardFailure` consecutivos. Erros transient de rate limit e timeout não contam.
- `--dry-run` faz preview do conjunto candidato sem spawnar nenhum LLM. A saída é NDJSON com um evento por memória e um resumo final.
- `--resume` continua um batch interrompido anteriormente a partir do queue DB. `--retry-failed` retenta apenas os itens que falharam.
- `--until-empty` (v1.0.96) roda um loop interno scan→drain até a fila não ter itens elegíveis ou `--max-runtime <SEGUNDOS>` (padrão 3600) expirar — substitui o loop externo `while` de retry. `--max-attempts <N>` (padrão 8, range 1..=20) é o orçamento de retries Transient; um item vira terminal `dead` após esse orçamento ou na primeira HardFailure (GAP-ENRICH-BACKLOG-CONVERGE, ADR-0055).
- `--status` (v1.0.96) imprime um relatório read-only JSON da fila (`unbound_backlog`, `scan_backlog` por operação, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`). Nunca chama o LLM e nunca adquire o singleton, então é seguro fazer poll enquanto um drain roda; o `scan_backlog` (GAP-SG-77, v1.1.0) é o backlog real do banco por operação que um scan enfileiraria — elimina o falso `pending=0` para `entity-descriptions`/`body-enrich`/`re-embed`, e o `state` deriva o `pending-scan` dele.
- `--list-dead` / `--requeue-dead` listam linhas terminais `dead` da fila ou as movem `dead` → `pending` (sem LLM, sem singleton quando usados sozinhos). Use após falhas duras que esgotaram `--max-attempts`.
- `--list-skipped` / `--requeue-skipped` listam linhas `skipped` / preservation-failed ou as movem `skipped` → `pending` (sem LLM, sem singleton quando usados sozinhos). Recuperam dívida de preservation/skipped sem SQL cru no `.enrich-queue.sqlite`.
- `--rest-concurrency <N>` (v1.0.96, padrão 8, clamp 1..=16) limita o fan-out REST via `JoinSet` bounded para `--mode openrouter`; é distinto de `--llm-parallelism`. O embedding processa lotes de 32 passagens com a ordem por chunk preservada enquanto a escrita SQLite permanece single-writer via WAL + claim atômico (GAP-OPENROUTER-REST-CONCURRENCY).
- `--prune-dead-orphans` (v1.0.97, GAP-SG-66, ADR-0058) é um inspetor read-only (sem LLM, sem singleton, sem `--operation`/`--mode`) que deleta SOMENTE linhas da fila de enrich com `status='dead'` e `item_type='memory'` cujo `item_key` (o nome da memória) sumiu do banco principal; linhas dead de entidade ficam intocadas e só o sidecar `.enrich-queue.sqlite` é mutado. A saída JSON `DeadSummary` reporta o campo `pruned`. Use para limpar dead-letter órfão deixado quando uma memória é renomeada ou purgada após o enfileiramento — `--requeue-dead` só as re-falha.
- `--prune-dead-entity-orphans` (v1.1.02, ADR-0062) é a contraparte para chaves de entidade: deleta linhas dead-letter com `item_type='entity'` do `.enrich-queue.sqlite`, e é mutuamente exclusiva com `--prune-dead-orphans`. Rode ambas em sequência para uma varredura completa de órfãos após um upgrade que renomeou/fundiu/purgou entidades.

- `--reset-stale-claims` (v1.1.03, enrich) reseta manualmente toda processing claim mais antiga que o limiar de stale de volta para `pending`. Use após um crash forte que passou pelo auto-reset do startup.
- `--stale-claim-secs <N>` (v1.1.03, enrich) sobrescreve o limiar de stale usado tanto pelo auto-reset do startup quanto por `--reset-stale-claims`.
- `--literal-to <RELATION>` (v1.1.03, `reclassify-relation`) é a contraparte verbatim de DESTINO para `--literal-from`; juntas migram literais underscore armazenados (`applies_to`) para a forma canônica com hífen (`applies-to`) sem normalização do clap no lado do destino.
- `--cross-namespace` (v1.1.03, `merge-entities`) é uma flag opt-in que permite a `--ids`/`--into-id` resolver entidades em TODOS os namespaces; o padrão é somente mesmo-namespace (seguro), então um id perdido não funde silenciosamente dados de outro namespace.
- `split-body` (v1.1.03, novo subcomando) divide corpos de memória sobredimensionados em filhas `{name}-part-{i}`, marca a original com metadata `superseded_by_split: true` e cria relações canônicas `replaces` de cada filha para a original. Use `split-body --name <N>` para uma memória ou `split-body --batch --threshold 25000` para todo corpo sobredimensionado; as filhas NÃO são embebedadas inline — rode `enrich --operation re-embed --target memories` depois.
- `--target <memories|entities|chunks|all>` (v1.1.01) seleciona qual tabela de embedding o `re-embed` cobre no backfill; válida apenas com `--operation re-embed` (falha alto caso contrário). `--status` reporta o `scan_backlog` por alvo.
### `vec` — Manutenção do Índice Vetorial (G39)
- `vec orphan-list --json` lista linhas de embedding de memória cujo `memory_id` não existe mais na tabela `memories`. Cada linha reporta o `vector_hash` (BLAKE3 do blob de embedding) para rastreabilidade.
- `vec purge-orphan --yes --dry-run --json` faz preview da contagem de deleção sem remover nada.
- `vec purge-orphan --yes --json` purga as TRÊS vec tables (`vec_memories`, `vec_entities`, `vec_chunks`) em uma única transação implícita. A resposta reporta `deleted`, `deleted_entities`, `deleted_chunks` e `elapsed_ms`.
- `vec stats --json` expõe `vec_memories_rows`, `vec_entities_rows`, `vec_chunks_rows`, `orphans` e o timestamp do último vacuum. Use para auditar a saúde das vec tables após ciclos de `forget` em massa.
- O subcomando `forget` agora chama `memories::delete_vec` ANTES do soft-delete, prevenindo novos órfãos em estado estável.
### Endurecimento de `optimize` e `backup` (G36 + G38)
- `optimize` agora faz pré-verificação da saúde do FTS5 via `check_fts_functional` ANTES de reconstruir. Um índice saudável não é mais reconstruído (economiza ~10 minutos em um banco de 4.3 GB). Force a reconstrução com `--no-fts-skip-when-functional`.
- `optimize --fts-dry-run --json` sai com código 1 se o índice FTS5 precisar de reconstrução, 0 caso contrário. Amigável para CI.
- `optimize --fts-progress <N>` (padrão 30) emite uma linha de progresso a cada N segundos durante a reconstrução. Defina como 0 para desabilitar.
- `optimize --yes` pula o prompt de confirmação. Obrigatório para CI não interativo.
- `backup` usa por padrão `run_to_completion(1000, Duration::from_millis(5), None)` (era 100/50ms). Para um banco de 4.3 GB isso é um speedup de 25x (~21s vs ~9 min).
- `backup --backup-step-size <PAGES>` e `--backup-step-sleep-ms <MS>` ajustam a granularidade de cópia de páginas. `--backup-no-sleep` remove o sleep entre steps totalmente para máximo throughput. `--backup-progress <PAGES>` (padrão 100) emite uma linha de progresso a cada N páginas.
### Família de Subcomandos `migrate` (v1.0.76, atualizado v1.0.77 e v1.0.78)
- `migrate --rehash --json` reescreve os checksums registrados de migração para casar com o conteúdo atual do arquivo. Idempotente. Obrigatório para upgrades v1.0.74 → v1.0.76 onde a migração V002 foi intencionalmente esvaziada para um no-op.
- `migrate --to-llm-only --drop-vec-tables --json` é o upgrade one-shot para bancos v1.0.74 / v1.0.75. Combina `--rehash` com o descarte da V013 das vec tables. A flag `--drop-vec-tables` é OBRIGATÓRIA como rede de segurança explícita. As tabelas com backing BLOB `memory_embeddings` / `entity_embeddings` / `chunk_embeddings` permanecem e são a fonte de verdade daqui em diante; embeddings são recomputados preguiçosamente no próximo `remember` / `edit` / `ingest`.
- Correção v1.0.77 (G40): a resposta JSON de ambos os comandos agora inclui `null_rows_fixed` (inteiro) e `vec_tables_removed_via_writable_schema` (inteiro). Bancos com linhas `applied_on = NULL` são sanitizados automaticamente antes do migration runner executar.
- Correção v1.0.78 (G41): a resposta JSON de ambos os comandos agora inclui `v013_tables_created` (boolean). Bancos onde V013 foi registrada em `refinery_schema_history` mas as tabelas BLOB-backed de embedding nunca foram criadas são reparados automaticamente. Qualquer comando CRUD também dispara esse reparo incondicionalmente via `ensure_db_ready`.


## Migração da v1.0.74 ou v1.0.75

Veja [MIGRATION.md](MIGRATION.md) para o passo a passo completo. A versão curta:

1. Instale a v1.0.76 (LLM-only).
2. Rode `sqlite-graphrag init` — a migração V013 roda automaticamente.
3. As vec tables antigas são dropadas; a nova `memory_embeddings` começa vazia.
4. As memórias são re-embutidas lazy no próximo `edit` ou `ingest`.

Para um corpus grande, use o loop one-shot canônico de re-embed (G42/S9, v1.0.79) — cada invocação processa um lote pequeno e encerra:

```bash
sqlite-graphrag enrich --operation re-embed --limit 5 --resume --mode openrouter --openrouter-model MODEL --json
```

Nota: a receita antiga `edit --description "<mesmo>"` nunca re-embedou nada (edições somente de descrição são no-op para embeddings); use `edit --force-reembed` para uma única memória.


## Rodando a Suíte de Testes

Este projeto não entrega CI. `cargo test` é o gate de release, e ele roda na
sua máquina.

Nenhuma CLI de LLM precisa estar no `PATH`. Até a v1.1.x a suíte exigia
`claude` ou `codex` instalado, porque o embedding passava por subprocesso. A
v1.2.0 removeu esses backends por completo: sobraram apenas `openrouter` (REST)
e `none`, e todo teste que precisa de embedding usa `none` ou uma fixture
local. Uma CLI no `PATH` não muda nada.

```bash
# O gate de release. Rode SEM --no-fail-fast pelo menos uma vez: o primeiro
# binário que sai anormalmente interrompe todos os seguintes, e essa cascata é
# como a v1.2.4 foi publicada com 61 de 87 suítes nunca lançadas (GAP-SG-189).
cargo test --all-features

# Contagem completa de asserções depois que o gate estiver verde.
cargo test --all-features --no-fail-fast

# Os doctests rodam por último, então a cascata acima os esconde por inteiro.
cargo test --doc

cargo clippy --all-targets --all-features -- -D warnings
```

Testes que gastam créditos reais de OpenRouter estão marcados com `#[ignore]`,
então o comando acima nunca cobra nada. Rode-os deliberadamente:

```bash
cargo test --test openrouter_chat_real -- --ignored --nocapture
cargo test --test openrouter_live_concurrency -- --ignored --nocapture
```

Veja [TESTING.pt-BR.md](TESTING.pt-BR.md) para o detalhamento por suíte.


## Inventário completo de comandos CLI (v1.2.5)

Comandos de topo (de `sqlite-graphrag --help`) com propósito em uma linha:

- `init` — cria/abre o DB SQLite, aplica migrações e faz smoke-test do caminho LLM
- `remember` — grava uma memória com grafo de entidades opcional
- `remember-batch` — cria várias memórias a partir de NDJSON no stdin (`description` obrigatória na criação)
- `ingest` — ingere em massa arquivos de um diretório como memórias
- `recall` — busca semântica (KNN) com hops de grafo opcionais
- `read` — lê uma memória por nome ou id
- `list` — pagina memórias com filtros
- `forget` — soft-delete de uma memória (histórico preservado)
- `purge` — hard-delete de soft-deletes após retenção (`--now` para imediato)
- `rename` — renomeia memória mantendo versões
- `split-body` — divide corpo sobredimensionado em memórias filhas
- `edit` — edita corpo/descrição/tipo e opcionalmente re-embute
- `history` — lista versões de uma memória
- `restore` — restaura uma memória para versão anterior
- `hybrid-search` — FTS5 + vetor fundidos via Reciprocal Rank Fusion
- `health` — integridade, FTS5, versão SQLite, cobertura de vetores, super-hubs
- `migrate` — aplica migrações pendentes (ou `--dry-run` / `--rehash`)
- `namespace-detect` — resolve a precedência de namespace desta invocação
- `optimize` — `PRAGMA optimize` e rebuild opcional do FTS5
- `stats` — contagens de memórias, entidades e relacionamentos
- `sync-safe-copy` — checkpoint e cópia segura para sync em nuvem
- `backup` — cópia via Online Backup API para um destino
- `vacuum` — checkpoint do WAL + reclamation de espaço
- `link` — cria relacionamento entidade–entidade
- `unlink` — remove relacionamentos ou um vínculo memória–entidade
- `deep-research` — pesquisa GraphRAG multi-hop via decomposição de query
- `related` — lista memórias conectadas pelo grafo a partir de uma semente
- `graph` — exporta snapshot do grafo (`json`/`dot`/`mermaid`) ou subcomandos
- `export` — exporta memórias como NDJSON
- `fts` — família de manutenção do índice FTS5
- `vec` — família de manutenção das tabelas vetoriais
- `prune-relations` — remove em massa relacionamentos de um tipo
- `prune-ner` — remove bindings NER de `memory_entities`
- `slots` — inspeção/limpeza do semáforo de slots LLM host-wide
- `pending` — fila de checkpoint em 3 estágios do `remember`
- `embedding` — saúde e listagem da fila de embeddings pendentes
- `pending-embeddings` — operações em lote na fila de retry de embedding
- `cleanup-orphans` — remove entidades sem memórias e sem relacionamentos
- `memory-entities` — lista entidades de uma memória (ou reverso via `--entity`)
- `cache` — list/stats/clear do cache de modelos XDG
- `delete-entity` — apaga entidade e cascateia arestas
- `reclassify` — reclassifica tipos de entidade (individual ou lote)
- `rename-entity` — renomeia entidade preservando arestas e vínculos
- `merge-entities` — funde entidades-fonte em um destino
- `enrich` — pipeline de qualidade do grafo via LLM e inspetores de fila
- `reclassify-relation` — renomeia tipos de relação em massa (literal ou normalizado)
- `normalize-entities` — normaliza nomes de entidade para kebab-case com auto-merge
- `completions` — gera completions de shell
- `config` — config operacional XDG e chaves de API

### Famílias aninhadas

- `config`
  - `add-key` — grava chave de API (stdin) de um provider
  - `list-keys` — lista fingerprints mascarados
  - `remove-key` — remove uma chave armazenada
  - `doctor` — diagnostica camadas de resolução de chave/config
  - `path` — imprime o caminho resolvido do config XDG
  - `set` — persiste um setting operacional
  - `get` — lê um setting
  - `list` — lista settings armazenados (`--effective` inclui defaults)
  - `unset` — remove um setting
- `graph`
  - `traverse` — caminhada BFS a partir de uma entidade (`--fuzzy` para apelidos curtos)
  - `stats` — contagens de nós/arestas e distribuição de grau
  - `entities` — lista entidades com ordenação/filtro
  - `recompute-degree` — reconstrói `entities.degree` em cache numa transação
- `fts`
  - `rebuild` — reconstrói o FTS5 do zero
  - `check` — integrity-check sem modificar o índice
  - `stats` — estatísticas de linhas/páginas shadow do FTS5
- `vec`
  - `orphan-list` — lista linhas de embedding órfãs
  - `purge-orphan` — apaga órfãos das tabelas vec
  - `stats` — contagens e órfãos das tabelas vec
- `slots`
  - `status` — slots retidos, PIDs e métricas de espera
  - `release` — força liberação de um slot por id (`--yes`)
  - `cleanup` — ceifa arquivos de slot stale/órfãos
- `pending`
  - `list` — lista linhas da fila de checkpoint
  - `show` — mostra uma entrada de checkpoint por id
  - `cleanup` — remove linhas em estado terminal
- `embedding`
  - `status` — saúde da fila + cobertura de vetores
  - `list` — inspeção por entrada
  - `abandon` — abandona embeddings pendentes que casam o filtro
- `pending-embeddings`
  - `list` — lista linhas da fila de retry de embedding
  - `status` — alias de `embedding status`
  - `abandon` — abandona linhas de retry que casam o filtro
- `cache`
  - `clear-models` — remove arquivos de modelo em cache
  - `list` — lista arquivos e tamanhos do cache
  - `stats` — alias de `list`
- `enrich` inspetores-chave (sem LLM / sem singleton quando usados sozinhos)
  - `--status` — relatório read-only da fila + scan backlog
  - `--list-dead` — lista linhas terminais `dead`
  - `--requeue-dead` — move `dead` → `pending`
  - `--list-skipped` — lista linhas `skipped` / preservation-failed
  - `--requeue-skipped` — move `skipped` → `pending`
  - `--prune-dead-orphans` — remove dead com chave de memória ausente do DB principal
  - `--prune-dead-entity-orphans` — remove dead com chave de entidade do sidecar
- Flags de escrita do `enrich` relevantes na **v1.2.1**
  - `--until-empty` — loop scan→drain até a fila elegível esvaziar ou `--max-runtime`; conta **somente esta op+namespace**
  - `--force-redescribe` — reabre `skipped`/`done` uma vez por processo para reescrever entity-descriptions de baixa qualidade; nunca reabre `dead`
  - `--operation re-embed --target memories|entities|chunks|all` — elegibilidade por comprimento do BLOB + reconciliação de zumbis
  - `--namespace` — claim, contagem e resume escopados por namespace
  - `--mode openrouter` / `--rest-concurrency` — fan-out REST de judge/embed
- Flags globais de saída adicionadas na **v1.2.2** (valem para todo subcomando)
  - `--select <CHAVES>` / `--fields` — mantém só estas chaves por elemento; caminhos com ponto OK; chave ausente é pulada, nunca `null`
  - `--filter <EXPR>` — `chave=valor`, `chave!=valor`, `chave~substring`; `==` sinônimo de `=`; repita para conjugar com AND; malformada sai com exit 2
  - `--max-items <N>` — teto de elementos emitidos, aplicado depois do filtro; distinta do `--limit` por subcomando e do `-k`
  - `--sort <CHAVE>` — ascendente por caminho com ponto; números numéricos, resto texto
  - `--dedupe-by <CHAVE>` — descarta elementos posteriores que repetem o valor
  - `--count-only` — o payload vira `{"count": N}`
  - `--truncate-content <N>` — encurta strings acima de N caracteres, nunca bytes
  - `--max-output-bytes <N>` — limita o envelope descartando elementos do fim, nunca fatiando o JSON
  - Envelopes de falha (`error: true` / `ok: false`) e documentos `$schema` nunca são remodelados; streams NDJSON contornam a superfície
- Flag global de entrada adicionada na **v1.2.2**
  - `--no-input` — recusa stdin em qualquer ponto da invocação; todo leitor de stdin falha de antemão com exit 1; precedência flag > XDG `cli.no_input` > `false`
- `schema` — catálogo legível por máquina dos **75** contratos JSON
  - `schema` — listagem NDJSON, um `{"id","invoke"}` por linha; `invoke` é o comando pronto para copiar
  - `schema --name <ID>` — emite o documento JSON Schema daquele contrato
  - `<ID>` desconhecido sai com **exit 4**; documentos `$schema` são isentos da superfície de saída agent-native, então qualquer flag global pode ser encadeada com segurança

> **GAP-SG-139:** folhas host/XDG (`config`, `slots`, `cache`, `completions`) aceitam `--db` como **no-op** documentado para que agentes que anexam `--db` em toda invocação não recebam clap exit 2.

> **Inventário top-level (51 + help):** init, remember, remember-batch, ingest, recall, read, list, forget, purge, rename, split-body, edit, history, restore, hybrid-search, health, migrate, namespace-detect, optimize, stats, sync-safe-copy, backup, vacuum, link, unlink, deep-research, related, graph, export, fts, vec, prune-relations, prune-ner, slots, pending, embedding, pending-embeddings, cleanup-orphans, memory-entities, cache, delete-entity, reclassify, rename-entity, merge-entities, enrich, reclassify-relation, normalize-entities, schema, completions, config, help.

## Veja Também

- [COOKBOOK.md](COOKBOOK.md) para receitas comuns
- [MIGRATION.md](MIGRATION.md) para upgrade v1.0.74 → v1.0.76
- [CROSS_PLATFORM.md](CROSS_PLATFORM.md) para Windows e macOS
- [AGENTS.md](AGENTS.md) para integração com agentes
- [HEADLESS_INVOCATION.md](HEADLESS_INVOCATION.md) para invocação headless OAuth-safe de Claude/Codex/OpenCode
- [decisions/](decisions/) para os 45 ADRs
