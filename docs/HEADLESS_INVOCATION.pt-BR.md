# Invocação Headless — OpenRouter REST sem MCP e sem Hooks

> Como operar este projeto de forma headless. Os backends por subprocesso local
> foram removidos, então não há CLI a isolar: toda chamada de LLM é uma
> requisição REST ao OpenRouter, e servidores MCP e hooks ficam estruturalmente
> fora de alcance.

- Versão em inglês deste guia vive em [HEADLESS_INVOCATION.md](HEADLESS_INVOCATION.md)
- Voltar ao [README.md](../README.md) para referência de comandos


## Resumo

- O único transporte de LLM é a API REST do OpenRouter (`reqwest` + `rustls-tls`); nenhum subprocesso é spawnado
- `--llm-backend` aceita `openrouter` (padrão) e `none`; `--llm-fallback` tem padrão `none`
- `--embedding-backend openrouter --embedding-model MODELO` roteia o embedding por `POST /api/v1/embeddings`
- `enrich --mode openrouter --openrouter-model MODELO` roteia o turno JUDGE por `POST /api/v1/chat/completions`
- Como nada é spawnado, não há config MCP a remover, hooks a zerar nem CWD a isolar

## Atualização v1.2.1 — CAPA enrich para agentes headless (só sidecar)

Temas CAPA (selo da fila enrich; schema **v16**, sem migração main-DB):

1. **`dequeue_next_pending`** — claim filtra por `operation` **e** `namespace` (drenar `ai-sdd` NÃO DEVE processar linhas de `global` / ns vazio; o mesmo vale para `--resume` / `--retry-failed`).
2. **`count_eligible_pending` para `--until-empty`** — conta só pendentes desta **op+ns** (ops alienígenas / zumbis ReEmbed em outro lugar não mantêm EntityDescriptions em loop com `completed=0`).
3. **`reopen_force_redescribe_candidates`** — `--force-redescribe` reabre `skipped`/`done` **uma vez por processo** antes do primeiro enqueue (para `INSERT OR IGNORE` não ser no-op silencioso); **nunca** reabre `dead` (use `--requeue-dead`).
4. **`reconcile_satisfied_reembed_pending`** — marca ReEmbed pending como `done` quando já existe vetor vivo com `LENGTH(embedding) = dim*4`, limpando zumbis sem chamadas de API.
5. **Elegibilidade de re-embed por LENGTH do BLOB** — predicados usam `LENGTH(embedding) = dim*4`, **não** só a coluna `dim` (linhas CORRUPT / META_AHEAD com dim=1024 e BLOB 384-d re-embedam de novo).
6. **Strip do prefixo `entity:` no enqueue** — lookup de entidade usa o nome bare; a chave da fila permanece `entity:…` (nomes bare ainda funcionam; entidade ausente é rejeitada).
7. **Enqueue de chunk valida namespace** — `chunk_id` deve existir em memória não-deletada do namespace alvo (rejeita chaves inválidas / cross-ns antes de churn de dead-letter/circuit-breaker).
8. **CAPA-D** — apenas marcadores compostos de "configuration file" (ex.: `is a configuration file`); sem FP bare em prosa de domínio legítima.

Regressões da fila: `enqueue_candidate_accepts_entity_prefixed_reembed_key`, `dequeue_next_pending_isolates_by_namespace`. Suite unitária da fila: **38** OK (`cargo test --lib commands::enrich::queue` ou `cargo test --lib commands::enrich`).

Fórmulas prontas para agentes:

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

- Schema permanece **v16** (sem migração main-DB). Gate offline ainda `scripts/e2e_offline_v120.sh` **20/20**. Pin `=1.2.2`.

## Atualização v1.2.0 — XDG, dim 1024, list-skipped, GAP-SG-139, hot-set headless

- Config de knobs de produto: **flag CLI > XDG `config set` > default**. Variáveis de ambiente de produto `SQLITE_GRAPHRAG_*` **não** são lidas no hot path. Harnesses DEVEM usar XDG isolado + flags — nunca exportar product env como contrato de config.
- **DEFAULT_EMBEDDING_DIM=1024** (override via `--embedding-dim` / XDG `embedding.dim`; bancos existentes mantêm `schema_meta.dim` até re-embed).
- Recuperar dívida da fila `skipped` / `preservation_failed` sem SQL cru:
  ```bash
  sqlite-graphrag enrich --list-skipped --json
  sqlite-graphrag enrich --requeue-skipped --json
  ```
- **GAP-SG-139:** folhas host/XDG (`config`, `slots`, `cache`, `completions`) aceitam `--db` como **no-op** documentado — agentes headless podem anexar `--db` em todo spawn sem clap exit 2.
- Após `remember` curado, PARSEIE `entities_created` / `enrich_recommended` e/ou PASSE `--enqueue-enrich` para entity-descriptions prioritário antes de drains longos de entity-connect.
- Polle qualidade sem LLM: `enrich --operation entity-descriptions --status --force-redescribe --json` (`scan_backlog_low_quality`, `quality_pct`, `state` incluindo `blocked_dead`).
- Filtros de nome: `--entity-names` para entity-descriptions; `--memory-names` para memory-bindings.
- Audite bindings: `memory-entities --name <mem> --json` inclui `entities[].description`.
- entity-connect é totalmente implementado (persiste relações). Em DBs grandes espere `budget_exhausted` / `preempted_for_gate`; prefira ED quente → EC frio.
- Gate offline de produto: `bash scripts/e2e_offline_v120.sh` espera **20/20 PASS** (canônico; wrapper histórico `e2e_offline_v118.sh` / 16/16 supersedido).
- **Nota CURRENT sobre texto histórico abaixo:** seções que ensinam product env como config descrevem comportamento pré-v1.2.0 — a v1.2.0 **não** lê product env no hot path (somente XDG + flags). A whitelist OAuth/custom-provider de env para subprocessos LLM permanece válida e **não** é config de knobs de produto.
- Segredos: prefira `config add-key --provider openrouter` (stdin) ou `--openrouter-api-key`; `OPENROUTER_API_KEY` não é lida em runtime; use apenas `config add-key` ou `--openrouter-api-key`.

## Inventário completo de comandos CLI para agentes headless (v1.2.5)

Orquestradores headless precisam conhecer a superfície completa de produto mesmo quando as receitas de spawn focam em `remember` / `enrich` / `deep-research`. Comandos de topo de produto (de `sqlite-graphrag --help`, excluindo o meta `help`):

- `init` — cria/abre DB + migrações + smoke-test LLM
- `remember` — grava uma memória (+ grafo opcional / `--enqueue-enrich`)
- `remember-batch` — criação em lote NDJSON (`description` obrigatória)
- `ingest` — ingere arquivos em massa como memórias
- `recall` — busca semântica KNN
- `read` / `list` / `forget` / `purge` / `rename` / `split-body` / `edit` / `history` / `restore` — CRUD e ciclo de vida de memórias
- `hybrid-search` — FTS5 + vetor RRF
- `health` / `migrate` / `namespace-detect` / `optimize` / `stats` — ops e diagnóstico
- `sync-safe-copy` / `backup` / `vacuum` — durabilidade e espaço
- `link` / `unlink` / `related` / `graph` / `export` — arestas e exportação
- `deep-research` — GraphRAG multi-hop (`-o` / `--output` atomwrite)
- `fts` / `vec` — famílias de manutenção de índice
- `prune-relations` / `prune-ner` / `cleanup-orphans` — higiene do grafo
- `slots` / `pending` / `embedding` / `pending-embeddings` — concorrência e filas
- `memory-entities` / `delete-entity` / `reclassify` / `rename-entity` / `merge-entities` / `reclassify-relation` / `normalize-entities` — admin de entidades
- `enrich` — qualidade do grafo via LLM + inspetores de fila (`--list-skipped` / `--requeue-skipped`)
- `cache` / `completions` / `config` — folhas host/XDG (`--db` no-op)

### Famílias aninhadas (resumo)

- `graph` — `traverse`, `stats`, `entities`, `recompute-degree`
- `embedding` — `status`, `list`, `abandon`
- `pending` — `list`, `show`, `cleanup`
- `pending-embeddings` — `list`, `status`, `abandon`
- `slots` — `status`, `release`, `cleanup`
- `cache` — `clear-models`, `list`, `stats`
- `config` — `add-key`, `list-keys`, `remove-key`, `doctor`, `path`, `set`, `get`, `list`, `unset`
- `fts` — `rebuild`, `check`, `stats`
- `vec` — `orphan-list`, `purge-orphan`, `stats`
- `enrich` inspetores: `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans`; **flags de escrita v1.2.1:** `--until-empty` (contagem op+ns), `--force-redescribe` (reabre skipped/done), `--operation re-embed --target …`, `--namespace`, `--mode openrouter`, `--rest-concurrency`
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

> **Top-level (50 + help):** init, remember, remember-batch, ingest, recall, read, list, forget, purge, rename, split-body, edit, history, restore, hybrid-search, health, migrate, namespace-detect, optimize, stats, sync-safe-copy, backup, vacuum, link, unlink, deep-research, related, graph, export, fts, vec, prune-relations, prune-ner, slots, pending, embedding, pending-embeddings, cleanup-orphans, memory-entities, cache, delete-entity, reclassify, rename-entity, merge-entities, enrich, reclassify-relation, normalize-entities, schema, completions, config, help.

> Inventário completo com propósitos em uma linha: [HOW_TO_USE.pt-BR.md](HOW_TO_USE.pt-BR.md) e [COOKBOOK.pt-BR.md](COOKBOOK.pt-BR.md).

## Contrato stdout/stderr e --quiet (v1.1.05) + alias `-o` (v1.1.8)

ADR: [ADR-0065](decisions/adr-0065-v1-1-05-danilo-bugs.pt-BR.md). Suite de regressão: `tests/v1105_danilo_bugs_regression.rs` (nome da suite **v1105**).

- JSON estruturado SEMPRE no stdout; logs de tracing SEMPRE no stderr
- Use `--quiet`/`-q` (global) para suprimir tracing não-erro — útil em pipelines headless que parseiam stdout com `jaq`
- Para envelopes grandes de `deep-research`, prefira `-o PATH` ou `--output PATH` (escrita atômica atomwrite) em vez de redirecionar stdout para arquivo misturado com stderr. Ack no stdout: `written`, `bytes`, `blake3`, `sub_queries_total`, `unique_memories_found`, `elapsed_ms`. Schema: `docs/schemas/deep-research-output-ack.schema.json`
- Queries de token único em `deep-research` expandem para sub-queries com `source: "aspect"` (fan-out multi-ângulo); estratégia manual via `--sub-query-strategy manual --sub-queries-file`
- Em scripts headless, use `graph traverse --fuzzy` quando o nome canônico for desconhecido; sem match exato, exit 4 inclui sugestões
- Prefira `link --from-id`/`--to-id` em automações que só têm IDs; NUNCA passe dígitos puros em `--from`/`--to` com `--create-missing`
- `merge-entities` rejeita self-ref (`--into-id` em `--ids`) antes do DB — útil sob loops zsh/bash malformados
- Nunca use `sqlite-graphrag ... &> arquivo` (redireciona stdout+stderr juntos e contamina o JSON)

```bash
# deep-research headless com saída atômica em arquivo (recomendado para agentes)
OUTDIR=/tmp/graphrag-out
mkdir -p "$OUTDIR"
sqlite-graphrag --quiet \
  --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 \
  deep-research "danilo" --max-sub-queries 7 --k 20 --with-bodies \
  -o "$OUTDIR/research.json" --json
# Parseie o ack no stdout; envelope completo no arquivo
# Facetas manuais opcionais:
# printf '%s\n' 'danilo stack' 'danilo projetos' > "$OUTDIR/subs.txt"
# sqlite-graphrag --quiet deep-research "danilo" \
#   --sub-query-strategy manual --sub-queries-file "$OUTDIR/subs.txt" \
#   --output "$OUTDIR/research.json" --json
```


## Atualização v1.0.93 — Backend de Embedding OpenRouter

- Desde a v1.0.93, embedding pode usar a API REST do OpenRouter em vez de spawnar um subprocesso LLM headless
- Use `--embedding-backend openrouter --embedding-model MODELO` para rotear embedding via `POST /api/v1/embeddings`
- Isso elimina o cold-start do subprocesso (~200ms de chamada API vs 15-20s de spawn de subprocesso por embedding)
- O caminho OpenRouter usa `reqwest+rustls-tls` diretamente — nada é spawnado, então não há isolamento de CWD a fazer
- O enforcement OAuth-only NÃO se aplica ao OpenRouter — ele usa XDG `config add-key` / `--openrouter-api-key` (OPENROUTER_API_KEY não é lida em runtime)
- Desde a v1.0.95 (ADR-0054), `enrich --mode openrouter` roda a etapa JUDGE via endpoint REST `/chat/completions` do OpenRouter (`reqwest+rustls-tls`). O pipeline SCAN→JUDGE→PERSIST permanece inalterado; só o transporte do JUDGE muda.
- A flag `--enrich-after` no `ingest` ainda spawna um subprocesso headless para a fase de enrich quando o modo de enrich é uma CLI local; com `--mode openrouter` a fase de enrich permanece sem subprocesso
- Veja ADR-0052 (embedding OpenRouter) e ADR-0054 (JUDGE do enrich via OpenRouter) para a justificativa arquitetural completa

## Atualização v1.0.95 — JUDGE do Enrich via OpenRouter

- `enrich --mode openrouter` roteia a etapa JUDGE para `POST /api/v1/chat/completions` — sem subprocesso de CLI local
- `--openrouter-model` é OBRIGATÓRIA com `--mode openrouter` (SEM default; omiti-la → exit 1 antes de qualquer chamada de rede)
- `--openrouter-api-key` lê da XDG via `config add-key` (OPENROUTER_API_KEY is not read at runtime) ou de `config add-key --provider openrouter`; `--openrouter-timeout` tem default de 300s; `--openrouter-base-url` é opcional
- A requisição usa `response_format` `json_schema` com `strict: true` e `provider.require_parameters: true`; `reasoning.enabled: false` com fallback reasoning-mandatory de uma retentativa; `usage.cost` é lido da resposta
- Trade-off: OAuth zero-token (modos CLI locais) vs tokens cobrados na chave OpenRouter em XDG (OPENROUTER_API_KEY não é lida em runtime) (modo OpenRouter); o caminho JUDGE do OpenRouter em si não exige migração, mas a v1.1.04 avança o schema do banco principal para v16 (a tabela `entity_connect_seen` da V016, exigida apenas quando você rodar depois `enrich --operation entity-connect`)

```bash
# JUDGE do enrich headless via REST OpenRouter (sem subprocesso, sem isolamento de CWD)
printf "%s" "sk-or-v1-sua-chave-aqui" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY não é lida em runtime (G-T-XDG-04)
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" --json
```

## Atualização v1.0.96 — Convergência do Backlog e Status Read-Only da Fila (ADR-0055)

- `enrich --until-empty` substitui o loop bash externo de retry na invocação headless: um único processo roda o loop interno scan→drain até a fila não ter mais itens elegíveis ou `--max-runtime <SECONDS>` (default 3600) expirar. A fila dead-letter garante que o conjunto vivo decresce estritamente — falhas transientes reagendam `next_retry_at` com backoff, um item vira `dead` após `--max-attempts` (padrão 8) retries transientes ou na primeira falha dura, e linhas `dead` são excluídas do dequeue.
- `enrich --status --json` é a sonda read-only para hooks e timers: reporta as contagens da fila (`unbound_backlog`, `scan_backlog` por operação, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`) e NÃO chama o LLM e NÃO adquire o singleton por namespace; o `scan_backlog` (GAP-SG-77, v1.1.0) é o backlog real do banco por operação que um scan enfileiraria — elimina o falso `pending=0` para `entity-descriptions`/`body-enrich`/`re-embed`, e o `state` deriva o `pending-scan` dele. Um timer cron ou systemd pode fazer poll sem disputar com um `enrich` em execução.
- `enrich --prune-dead-orphans --json` é um inspetor read-only complementar (sem LLM, sem singleton): deleta linhas dead-letter (`status='dead'`, `item_type='memory'`) cujo nome de memória não existe mais no banco principal, mutando apenas o sidecar `.enrich-queue.sqlite`; linhas dead de entidade não são tocadas. Use-o em scripts de manutenção headless para limpar acúmulo de dead-letter órfão de memórias renomeadas ou purgadas após o enqueue (ADR-0058, GAP-SG-66, v1.0.97).
- `enrich --prune-dead-entity-orphans --json` (v1.1.02, ADR-0062) é a contraparte para chaves de entidade: deleta linhas dead-letter com `item_type='entity'`, e é mutuamente exclusivo com `--prune-dead-orphans`. Rode ambos em sequência para uma varredura completa de órfãos após um upgrade que renomeou/fundiu/purgou entidades.
- `--rest-concurrency <N>` (clamp 1..=16, default 8) define o fan-out REST in-flight para embedding `--mode openrouter`; aumente-o para vazão OpenRouter. É distinta de `--llm-parallelism` (que limita subprocessos LLM locais) e de `--max-attempts` (o orçamento de retries).

```bash
# Drain headless do backlog — sem while-loop externo, sem subprocesso para OpenRouter
printf "%s" "sk-or-v1-sua-chave-aqui" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY não é lida em runtime (G-T-XDG-04)
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" \
  --until-empty --max-runtime 1800 --max-attempts 8 --rest-concurrency 8 --json

# Sonda de hook/timer — inspecionar a fila sem spawnar o LLM nem adquirir o singleton
sqlite-graphrag enrich --status --json | jaq '{eligible_now, waiting, dead: .queue_dead}'
```

## Atualização v1.1.06 — entity-connect headless em namespaces grandes (ADR-0066)

Registro de decisão: [ADR-0066](decisions/adr-0066-v1-1-06-entity-connect-scan.pt-BR.md). Suite de regressão: `tests/v1106_entity_connect_scan_regression.rs` (nome **v1106**).

- Fecha **GAP-ENTITY-CONNECT-SCAN-CARTESIAN**: `enrich --operation entity-connect` (e `cross-domain-bridges`) headless no `global` grande não trava mais a 100% de CPU antes de `phase: scan`. O scan de pares é O(k) (coocorrência + hub×ilha), não cartesiano O(n²).
- Chaves da fila `pair:{id1}:{id2}` com `item_type=entity_pair`; drain por chave primária (sem re-scan por item). GAP-002 `entity_connect_seen` permanece em vigor.
- **Wall-clock do primeiro scan** coberto por `--max-runtime` e teto soft de 120s via `InterruptHandle`. Timeout → `AppError::Timeout` exit **1**. Orquestradores NÃO DEVEM tratar timeout de scan como exit **75** (singleton/slot).
- NDJSON para hooks: espere `phase: "scan_start"` **antes** do SQL (`operation`, `entities_in_namespace`, `backlog_degree0_proxy`), depois `scan` / `scan_meta` (`pairs_enqueued_this_scan`, `scan_elapsed_ms`). Não equacione os dois campos de backlog.
- Prefira dry-run antes de jobs longos `--until-empty` em grafos densos.
- Sem migração de schema na v1.1.06 (permanece v16). Pin `=1.1.6`.

```bash
# Dry-run headless deve terminar rápido e emitir scan_start (sem hang cartesiano)
sqlite-graphrag enrich --operation entity-connect --dry-run --json --limit 50 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro

# Convergência longa: --max-runtime também cobre o PRIMEIRO scan
sqlite-graphrag enrich --operation entity-connect --until-empty --max-runtime 600 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json
```

### Atualização v1.1.04 — Estabilidade do deep-research + Convergência do entity-connect (ADR-0064)

- GAP-001: o `deep-research` não entra mais em panic com "Cannot start a runtime from within a runtime" quando invocado em modo headless (agent harnesses, runners de CI, jobs agendados). O entry point síncrono `deep_research::run` agora computa os embeddings por sub-query ANTES de construir seu runtime Tokio dedicado via o novo helper `compute_sub_embeddings`, e os três caminhos de embedding OpenRouter em `embedder.rs` (single, batch serial, fan-out JoinSet) adotam o padrão canônico de reentrada `Handle::try_current` + `block_in_place` já usado pelo path batch.  Para orquestradores headless isso significa que jobs `deep-research --with-bodies` de longa duração que antes crashavam no meio agora completam de forma confiável.
- GAP-002: o `entity-connect` agora converge em loops headless de longa duração. A nova tabela `entity_connect_seen` (migração V016, schema do banco principal v15 → v16) registra o veredito do LLM (`related`/`none`) por par avaliado; o scanner `scan_isolated_entity_pairs` exclui pares já avaliados e prioriza entidades hub; e o `call_entity_connect` persiste o veredito nos dois ramos. Combinado com `--until-empty --max-runtime`, um job headless `enrich --operation entity-connect` agora atinge `eligible_remaining == 0` em vez de re-avaliar infinitamente os mesmos pares rejeitados. Rodar `migrate --json` uma vez na primeira abertura é OBRIGATÓRIO antes da primeira invocação do `entity-connect`.


### Atualização v1.1.03 — Recuperação de Claims Stale no enrich Headless de Longa Duração

- Orquestradores headless (agent harnesses, runners de CI, timers do systemd) frequentemente enviam SIGINT, SIGTERM e ocasionalmente SIGKILL para jobs `enrich --until-empty` de longa duração
- SIGKILL NÃO é capturável — o sidecar `.enrich-queue.sqlite` pode ficar com linhas presas em `status='processing'` sob o PID morto
- Desde v1.1.03 (ADR-0063, Bug 4), o sidecar da fila ganha uma coluna `claimed_at` INTEGER e o worker do enrich emite um heartbeat por item (`UPDATE queue SET claimed_at = unixepoch() WHERE id = ?`)
- Em CADA startup do enrich, o worker chama `reset_stale_processing_claims(conn, 1800)` — itens com `status='processing' AND claimed_at < unixepoch() - 1800` são devolvidos para `pending` e `claimed_at = NULL`
- O threshold de 1800 segundos (30 minutos) é o padrão; combinado com o heartbeat ele cobre qualquer job que pare de progredir por meia hora
- Para reset manual (ex.: após um incidente kill -9 conhecido), a nova flag `enrich --reset-stale-claims --json` descarrega claims stale sem rodar o loop completo de scan→drain
- SIGTERM (capturável) é tratado pelo path gracioso existente do `signals::handler`; apenas SIGKILL depende da recuperação baseada em timestamp
- Sem nova variável de ambiente, sem telemetria — a recuperação é silenciosa e idempotente

```bash
# Forçar reset de claims stale após um incidente kill -9 conhecido (sem scan, sem LLM)
sqlite-graphrag enrich --reset-stale-claims --json

# enrich headless normal — claims stale são auto-recuperados no startup
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" \
  --until-empty --max-runtime 1800 --json
```

## Atualização v1.0.93 — Backend de Embedding OpenRouter
- Nova flag `--embedding-backend openrouter` habilita embedding via REST API sem subprocesso LLM
- Elimina overhead de cold-start: ~200ms por embedding vs 15s com subprocesso
- Requer `config add-key --provider openrouter` (OPENROUTER_API_KEY não é lida em runtime) ou flag `--openrouter-api-key`
- Requer `--embedding-model MODEL` (sem padrão — o usuário deve especificar)
- Funciona com todos os 8 comandos de embedding no modo headless
- Exemplo de invocação headless:

```bash
sqlite-graphrag \
  --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  ingest ./docs --pattern "*.md" --recursive --json
```

## Flags globais de LLM para agentes headless

- `--llm-backend <openrouter|none>` — seleciona o transporte de embedding. Padrão `openrouter`; `none` pula o embedding
- `--llm-model <MODELO>` — modelo passado ao backend selecionado
- `--llm-fallback <cadeia>` — cadeia de fallback quando o backend primário falha. Padrão `none`
- `--skip-embedding-on-failure` — persiste a memória sem vetor (exit 0 em vez de exit 11)
- `--llm-max-host-concurrency <N>` — limita as chamadas LLM concorrentes em todo o host
- `--llm-slot-wait-secs <N>` — segundos para esperar por um slot livre antes de falhar
- `--llm-slot-no-wait` — falha imediatamente se nenhum slot estiver disponível

### sqlite-graphrag com override de backend e modelo

```bash
# Força o backend OpenRouter com modelo de embedding específico
sqlite-graphrag --llm-backend openrouter --llm-model "qwen/qwen3-embedding-8b" \
  remember --name example --type note --body "text" --json

# Pula o embedding por completo (nenhum vetor gravado)
sqlite-graphrag --llm-backend none \
  remember --name sem-vetor --type note --body "text" --json

# Pula o embedding em caso de falha (persiste a memória sem vetor)
sqlite-graphrag --skip-embedding-on-failure \
  remember --name resilient --type note --body "text" --json
```


## Padrões Headless Adicionados na v1.0.82
### Padrão de captura do envelope de shutdown (GAP-002, ADR-0037)
```bash
# Envolva uma invocação longa de sqlite-graphrag num handler de sinal
# que captura o envelope JSON de shutdown no stdout no exit 19.
timeout 300 sqlite-graphrag remember --name big-corpus --type document \
  --body-file ./big.md --json 2>/tmp/err.log
EXIT=$?
if [ $EXIT -eq 19 ]; then
  # parseie o envelope na última linha do stdout
  jaq -e '.error and .code == 19' /tmp/err.log
  jaq -r '.signal, .graceful' /tmp/err.log
fi
```
### Padrão de wrap da cadeia de fallback (GAP-003 + GAP-005, ADR-0038 + ADR-0040)
```bash
# Pre-flight: confirme que a chave OpenRouter resolve antes de lançar
sqlite-graphrag config doctor --json | jaq -e '.openrouter_key_present' >/dev/null \
  || { echo "chave OpenRouter ausente no XDG (config add-key); OPENROUTER_API_KEY não é lida em runtime"; exit 1; }

# Lance com o backend explícito
sqlite-graphrag remember --name foo --type note --body "..." \
  --llm-backend openrouter --json

# Se o backend falhar, inspecione a fila pendente
sqlite-graphrag pending-embeddings list --filter-status failed --json
```
### Padrão de poll do semáforo de slots (GAP-004, ADR-0039)
```bash
# Aguarde um slot livre antes de lançar um lote pesado
while [ "$(sqlite-graphrag slots status --json | jaq '.acquired')" -gt 0 ]; do
  sleep 5
done
sqlite-graphrag ingest ./big-corpus --recursive --json
```
