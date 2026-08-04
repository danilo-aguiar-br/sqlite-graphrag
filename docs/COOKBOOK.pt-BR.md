# Receitas do sqlite-graphrag (v1.2.5)

- Versão em inglês: [COOKBOOK.md](COOKBOOK.md)

> **Paridade incompleta, medida em 2026-08-04.** Este arquivo tem 82 seções de nível 2; [COOKBOOK.md](COOKBOOK.md) tem 165. Cerca de 83 receitas existem somente em inglês. As receitas de operação diária — escrita, busca híbrida, travessia de grafo, enrich, configuração XDG, exit codes, atualização entre versões — estão AQUI e são as mesmas do inglês. O que falta traduzir é majoritariamente receita histórica de versões 1.0.x. Enquanto a tradução não fecha, consulte o arquivo em inglês para qualquer receita que você não encontrar aqui: os comandos, flags e envelopes JSON são idênticos nos dois idiomas, porque nenhum deles é traduzido. Rastreado como resíduo declarado em [gaps.md](../gaps.md) sob GAP-SG-197.


## Receitas adicionadas na v1.2.2 (superfície de saída agent-native) e na v1.2.1 (selo CAPA enrich)

- Crate **1.2.5**, schema **v16** (sem migração main-DB). **DEFAULT_EMBEDDING_DIM=1024**. Precedência: **flag CLI > XDG `config set` > default**. Variáveis de ambiente de produto `SQLITE_GRAPHRAG_*` **não** são lidas no hot path. Gate offline: `scripts/e2e_offline_v120.sh` **20/20** (wrapper histórico `e2e_offline_v118.sh` supersedido). CAPA: claim por namespace, until-empty op+ns, reopen force-redescribe, re-embed LENGTH / entity: enqueue.

### Receita — Re-embed de entidades após migrate de dim (BLOB CORRUPT / META_AHEAD)

#### Problema
- Após subir a dim ativa (ex.: 384 → 1024), algumas linhas mantêm `dim=1024` na coluna com BLOB ainda em 384 floats — CORRUPT / META_AHEAD
- Antes da 1.2.1 a elegibilidade confiava só na coluna `dim` e deixava zumbis pending para sempre quando o vetor live já batia o comprimento alvo

#### Solução
```bash
DB="${DB:-$HOME/.local/share/sqlite-graphrag/memory.db}"
MODEL="${MODEL:-deepseek/deepseek-v4-flash:nitro}"

sqlite-graphrag enrich --db "$DB" --status --operation re-embed --namespace global -q
sqlite-graphrag enrich --db "$DB" --operation re-embed --target entities \
  --mode openrouter --openrouter-model "$MODEL" \
  --until-empty --namespace global -q --wait-lock 60
```

#### Explicação
- Elegibilidade 1.2.1 usa `LENGTH(embedding) = target_dim * 4`, então linhas CORRUPT re-embedam de novo
- `reconcile_satisfied_reembed_pending` marca pending `done` quando o vetor live já bate o comprimento da dim (sem API)
- Enqueue aceita chaves `entity:{name}` (prefixo removido no lookup; bare ok; missing rejeita)

### Receita — Force-redescribe com reopen de skipped/done

#### Problema
- `--force-redescribe` deixava linhas já `skipped`/`done` no sidecar intocadas, e `INSERT OR IGNORE` era no-op silencioso
- Operadores precisam de campanha segura que nunca reabra `dead` permanente

#### Solução
```bash
DB="${DB:-$HOME/.local/share/sqlite-graphrag/memory.db}"
MODEL="${MODEL:-deepseek/deepseek-v4-flash:nitro}"

sqlite-graphrag enrich --db "$DB" --operation entity-descriptions --status --force-redescribe --namespace global -q
sqlite-graphrag enrich --db "$DB" --operation entity-descriptions \
  --mode openrouter --openrouter-model "$MODEL" \
  --force-redescribe --until-empty --namespace global -q
# Nunca reabre dead — recupere explicitamente:
# sqlite-graphrag enrich --db "$DB" --requeue-dead --operation entity-descriptions --namespace global -q
```

#### Explicação
- `reopen_force_redescribe_candidates` roda **uma vez por processo** antes do primeiro enqueue
- CAPA-D: apenas marcadores compostos de "configuration file" (sem FP bare `%configuration file%`)
- Backfill live LQ permanece campanha do operador (custo LLM)

### Receita — Drain until-empty isolado por op+namespace

#### Problema
- Antes da 1.2.1, `--until-empty` contava *todo* pending entre operações, e zumbis ReEmbed mantinham EntityDescriptions girando até max-runtime com `completed=0`
- Claim sem filtro de namespace processava itens `global` ao drenar `ai-sdd`

#### Solução
```bash
DB="${DB:-$HOME/.local/share/sqlite-graphrag/memory.db}"
MODEL="${MODEL:-deepseek/deepseek-v4-flash:nitro}"
NS="${NS:-global}"

sqlite-graphrag enrich --db "$DB" --operation entity-descriptions \
  --mode openrouter --openrouter-model "$MODEL" \
  --until-empty --namespace "$NS" -q --wait-lock 60

sqlite-graphrag enrich --db "$DB" --operation re-embed --target all \
  --mode openrouter --openrouter-model "$MODEL" \
  --until-empty --namespace "$NS" -q --wait-lock 60
```

#### Explicação
- `count_eligible_pending` e `dequeue_next_pending` exigem `operation` **e** `namespace`
- Enqueue de chunk valida que `chunk_id` existe em memória não-deletada do namespace alvo

## Receitas da v1.2.0 (XDG + dim 1024)

- Crate **1.2.0** (histórico; selo CAPA 1.2.1 acima). **DEFAULT_EMBEDDING_DIM=1024**. Precedência: **flag CLI > XDG `config set` > default**. Product env `SQLITE_GRAPHRAG_*` **não** lida no hot path. Gate offline: `scripts/e2e_offline_v120.sh` **20/20**.

### Receita — Config XDG sem env de produto

#### Problema
- Scripts e agentes ainda tratam knobs de produto como variáveis de ambiente.
- Help e docs legados ensinam `SQLITE_GRAPHRAG_*` como contrato de runtime; a v1.2.0 **não** lê product env no hot path.

#### Solução
```bash
# Precedência: flag CLI > XDG config set > default
sqlite-graphrag config path --json
sqlite-graphrag config list --effective --json
sqlite-graphrag config set network.openrouter.embeddings_url "https://openrouter.ai/api/v1/embeddings"
sqlite-graphrag config set network.openrouter.chat_url "https://openrouter.ai/api/v1/chat/completions"
sqlite-graphrag config set llm.probe_timeout_ms 3000
sqlite-graphrag config doctor --json
# Chaves: prefira o keystore XDG (stdin) ou a flag por chamada
printf '%s' "$KEY" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY não é lida em runtime — pipe o segredo de arquivo ou secret store
# ou: --openrouter-api-key (evite histórico de shell quando possível)
```

#### Explicação
- **CURRENT:** apenas envs de SO/XDG e whitelist OAuth de subprocesso são relevantes; knobs de produto são **flags + XDG**.
- `OPENROUTER_API_KEY` não é lida em runtime; use `config add-key` / `--openrouter-api-key`.
- `config list --effective` inclui defaults bem-conhecidos mesmo quando não gravados.
- URLs OpenRouter vêm de `network.openrouter.*` (aliases `network.chat_url` / `network.embed_url`).
- Fail-fast de query Auto usa `llm.probe_timeout_ms` (padrão 3000 ms; wall-clock <10s no harness).

### Receita — Fail-fast de recall offline (llm-backend none / probe)

#### Problema
- Hosts offline ou somente-FTS travavam cerca de 20s em OAuth morto quando o modo Auto tentava embedar a query.

#### Solução
```bash
# Orçamento fail-fast de embed de query (padrão 3s) + probe
sqlite-graphrag config set llm.probe_timeout_ms 3000
sqlite-graphrag config set llm.probe_timeout_ms 800
# Caminho offline explícito
sqlite-graphrag recall "auth tests" --k 5 --llm-backend none --json
sqlite-graphrag hybrid-search "auth tests" --k 5 --llm-backend none --json
```

#### Explicação
- O probe de credencial mais `llm.probe_timeout_ms` abortam rapidamente a tentativa de embed do modo Auto, mantendo os caminhos FTS e híbrido interativos.

### Receita — pending-embeddings status e cache stats

```bash
sqlite-graphrag pending-embeddings status --json   # alias de embedding status
sqlite-graphrag cache stats --json                 # alias de cache list
```

### Receita — purge --now (todas as soft-deleted)

```bash
sqlite-graphrag purge --now --dry-run --json
sqlite-graphrag purge --yes --now --json   # destrutivo: equivalente a --retention-days 0
```

### Receita — remember-batch com description obrigatória

```bash
# description é obrigatória na criação (paridade com remember; GAP-E2E-05)
printf '{"name":"nota-a","type":"note","description":"primeira","body":"conteúdo a"}\n{"name":"nota-b","type":"note","description":"segunda","body":"conteúdo b"}\n' \
  | sqlite-graphrag remember-batch --transaction --json
```

### Receita — Mapear entity_type module → concept

#### Problema
- Payloads de grafo emitidos por LLM trazem rótulos livres como `"module"`, que antes falhavam no Deserialize.

#### Solução
```bash
# graph-stdin / entities-file: "module" é dobrado para Concept via map_to_canonical
printf '%s' '{"body":"x","entities":[{"name":"auth-module","entity_type":"module"}],"relationships":[]}' \
  | sqlite-graphrag remember --name fold-module --type note --description "demo de fold" --graph-stdin --json
sqlite-graphrag memory-entities --name fold-module --json | jaq '.entities[] | {name, entity_type, description}'
```

#### Explicação
- `EntityType::map_to_canonical` normaliza rótulos não canônicos (`module` → Concept) no parse/serde.

### Receita — Inspecionar config efetiva (config list --effective)

#### Problema
- Operadores precisam da visão resolvida flag > XDG > default sem adivinhar os defaults compilados.

#### Solução
```bash
sqlite-graphrag config path --json
sqlite-graphrag config list --json
sqlite-graphrag config list --effective --json
sqlite-graphrag config doctor --json
sqlite-graphrag config get network.openrouter.embeddings_url --json
sqlite-graphrag config unset llm.probe_timeout_ms --json
```

#### Explicação
- `--effective` preenche defaults bem-conhecidos (URLs do OpenRouter, probe/timeout, `embedding.dim`, `log.level`, `display.tz`, …) quando não estão gravados no XDG.

### Receita — entity-descriptions com force-redescribe e grounding

```bash
# Backlog de qualidade honesto (sem LLM)
sqlite-graphrag enrich --operation entity-descriptions --status --force-redescribe --json

sqlite-graphrag enrich --operation entity-descriptions \
  --force-redescribe \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro \
  --limit 20 --json
```

- Prompt multi-domínio neutro + gate de grounding contra o corpus (`preservation`).
- Claim da fila escopado por `operation` (QISO) — drains de ops diferentes não se envenenam.
- Poluição live é campanha do operador — o harness não limpa o grafo de produção sozinho.

### Receita — hot-set após remember (`--enqueue-enrich`)

```bash
sqlite-graphrag remember --name nota-fiscal --type note --description "ICMS" \
  --body "Nota sobre ICMS-P05 e NFC-e ordenada" \
  --enqueue-enrich --json \
  --graph-stdin <<'EOF'
{"entities":[{"name":"icms-p05","entity_type":"concept"}],"relationships":[]}
EOF
# Leia entities_created[] e enrich_recommended (em geral ["entity-descriptions"])

# ED por nomes de entidade; bindings por nomes de memória
sqlite-graphrag enrich --operation entity-descriptions \
  --entity-names icms-p05 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json
sqlite-graphrag enrich --operation memory-bindings --memory-names nota-fiscal --dry-run --json
```

- Ordem recomendada: escrita → entity-descriptions (quente) → entity-connect (frio).

### Receita — auditar description em memory-entities

```bash
sqlite-graphrag memory-entities --name nota-fiscal --json \
  | jaq '.entities[] | {name, entity_type, description}'
```

### Receita — deep-research com alias curto `-o`

```bash
sqlite-graphrag deep-research "decisões de autenticação" -o /tmp/dr.json --quiet --json
# stdout: ack curto com blake3; arquivo = envelope completo (atomwrite)
```

### Receita — Listar e reenfileirar itens skipped do enrich

#### Problema
- O gate de preservation (ou outros skips duros) deixa linhas no sidecar da fila como `skipped` / `preservation_failed`
- Agentes precisam recuperar sem SQL cru em `.enrich-queue.sqlite`

#### Solução
```bash
# Somente leitura: lista skipped / preservation-failed (sem LLM, sem singleton)
sqlite-graphrag enrich --list-skipped --json

# Move skipped → pending para o próximo drain retentar
sqlite-graphrag enrich --requeue-skipped --json

# Em seguida drene a operação que gerou os skips (exemplo: entity-descriptions)
sqlite-graphrag enrich --operation entity-descriptions \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro \
  --limit 50 --json
```

#### Explicação
- `--list-skipped` / `--requeue-skipped` são inspetores de fila (G-PR-3/4), irmãos de `--list-dead` / `--requeue-dead`
- Recuperam o sink skipped sem abrir o DB da fila à mão
- Combine com `--status` para confirmar que `queue_skipped` cai após o requeue

### Receita — Passar `--db` em toda invocação (GAP-SG-139)

#### Problema
- Harnesses de agentes anexam `--db <PATH>` em toda chamada por isolamento
- Folhas host/XDG (`config`, `slots`, `cache`, `completions`) historicamente rejeitavam `--db` desconhecido com clap exit 2

#### Solução
```bash
DB=/data/team.sqlite

# Superfícies de grafo abrem o DB normalmente
sqlite-graphrag health --db "$DB" --json
sqlite-graphrag namespace-detect --db "$DB" --json

# Folhas host/XDG aceitam --db como no-op (GAP-SG-139) — agentes podem sempre passar
sqlite-graphrag config list --effective --db "$DB" --json
sqlite-graphrag slots status --db "$DB" --json
sqlite-graphrag cache stats --db "$DB" --json
sqlite-graphrag completions bash --db "$DB" >/dev/null
```

#### Explicação
- **GAP-SG-139:** folhas host/XDG aceitam `--db` como **no-op** documentado (`src/cli_db_noop.rs`); não abrem o DB do grafo
- Superfícies de grafo ainda exigem caminho real via `--db` ou XDG `db.path`
- Prefira `--db` / `config set db.path` — product env `SQLITE_GRAPHRAG_DB_PATH` **não** é lida no hot path da v1.2.0

### Receita — Harness offline 20/20

```bash
# Exige binário 1.2.0+ (compila se faltar)
bash scripts/e2e_offline_v120.sh
# Esperado: SUMMARY pass=20 fail=0
# Wrapper histórico scripts/e2e_offline_v118.sh está supersedido — não trate 16/16 como corrente
```

- Harness isola XDG e usa apenas flags `--db` — sem product env, sem chave live obrigatória.
- Asserts incluem contrato de help, dim 1024, flags list-skipped e fail-fast de recall Auto.

## Inventário completo de comandos CLI (v1.2.5)

Comandos de topo de produto (de `sqlite-graphrag --help`, excluindo o meta `help`) com propósito em uma linha:

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

- `config` — `add-key`, `list-keys`, `remove-key`, `doctor`, `path`, `set`, `get`, `list`, `unset`
- `graph` — `traverse`, `stats`, `entities`, `recompute-degree`
- `fts` — `rebuild`, `check`, `stats`
- `vec` — `orphan-list`, `purge-orphan`, `stats`
- `slots` — `status`, `release`, `cleanup`
- `pending` — `list`, `show`, `cleanup`
- `embedding` — `status`, `list`, `abandon`
- `pending-embeddings` — `list`, `status`, `abandon`
- `cache` — `clear-models`, `list`, `stats`
- `enrich` inspetores — `--status`, `--list-dead`, `--requeue-dead`, **`--list-skipped`**, **`--requeue-skipped`**, `--prune-dead-orphans`, `--prune-dead-entity-orphans`
- Flags de escrita do `enrich` (CAPA v1.2.1) — `--until-empty` (conta **somente esta op+namespace**), `--force-redescribe` (reabre `skipped`/`done` uma vez por processo; nunca `dead`), `--operation re-embed --target memories|entities|chunks|all` (elegibilidade por comprimento do BLOB + reconciliação de zumbis), `--namespace` (claim/contagem/resume escopados)
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

> Detalhe aninhado completo: [HOW_TO_USE.pt-BR.md — Inventário completo de comandos CLI (v1.2.5)](HOW_TO_USE.pt-BR.md#inventário-completo-de-comandos-cli-v125). **GAP-SG-139:** folhas host/XDG aceitam `--db` como no-op. **DEFAULT_EMBEDDING_DIM=1024**. Gate offline: `scripts/e2e_offline_v120.sh` **20/20**. Regressões CAPA / suite da fila **38** OK — veja [gaps.md](../gaps.md).

---

## Como Usar Providers Anthropic-Compatíveis Customizados (v1.0.83+)

### Problema
- Você quer usar Minimax/api.minimax.io, OpenRouter, AWS Bedrock ou um gateway corporativo Anthropic-compatível
- O mandato OAuth-only da v1.0.69 rejeita `ANTHROPIC_API_KEY`/`OPENAI_API_KEY` do ambiente do spawn
- O whitelist env-clear da v1.0.76+ acidentalmente descartava `ANTHROPIC_AUTH_TOKEN` e `ANTHROPIC_BASE_URL` junto com as vars proibidas
- Você obtém `exit 11` com `401 Invalid authentication credentials` no stderr e linhas órfãs crescendo em `pending_embeddings`


### Solução
Sete as vars de ambiente do custom-provider no seu perfil de shell; o sqlite-graphrag as encaminha automaticamente:

```bash
# Para Minimax (cenário canônico deste fix)
export ANTHROPIC_AUTH_TOKEN="sk-cp-seu-token-minimax"
export ANTHROPIC_BASE_URL="https://api.minimax.io/anthropic"

# Para OpenRouter (usa endpoint Anthropic-compatível)
export ANTHROPIC_AUTH_TOKEN="sk-or-seu-token-openrouter"
export ANTHROPIC_BASE_URL="https://openrouter.ai/api/v1"

# Para AWS Bedrock (endpoint Anthropic-compatível)
export ANTHROPIC_AUTH_TOKEN="seu-token-bedrock"
export ANTHROPIC_BASE_URL="https://bedrock-runtime.us-east-1.amazonaws.com/anthropic"

# Para um gateway corporativo
export ANTHROPIC_AUTH_TOKEN="seu-token-corp"
export ANTHROPIC_BASE_URL="https://llm-gateway.corporate.internal/anthropic"

# Verificar com smoke test
sqlite-graphrag remember \
  --name v183-custom-provider \
  --type note \
  --description "smoke test custom provider" \
  --body "se você consegue ler isto, o custom provider está conectado corretamente"
sqlite-graphrag read --name v183-custom-provider --json | jaq '.body'
```


### Explicação
- v1.0.83 (ADR-0041) preserva seis vars de ambiente de custom-provider ao spawnar o subprocesso LLM: `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`, `OTEL_EXPORTER_OTLP_ENDPOINT`
- O guard OAuth-only em `claude_runner.rs:273`, `codex_spawn.rs:259`, `ingest_claude.rs:282` e `extract/llm_embedding.rs:237-253` é preservado; `ANTHROPIC_API_KEY` e `OPENAI_API_KEY` ainda abortam o spawn com `exit 1`
- O helper compartilhado `src/spawn/env_whitelist.rs` expõe `apply_env_whitelist(cmd, strict)` para que os três spawners deleguem em vez de inlinear o array
- Modo padrão é permissivo — nenhuma flag manual necessária para habilitar encaminhamento de env de custom-provider
- Resolve GAP-058 parcialmente roteando em torno de contenção de quota OAuth; `recall`/`hybrid-search` permanecem determinísticos quando quotas OAuth oficiais estão esgotadas


### Modo Compliance (env-clear estrito)
Para ambientes PCI-DSS, SOC2 ou HIPAA que proíbem encaminhamento de credenciais via env vars:

```bash
# Por invocação
sqlite-graphrag remember --name minha-memoria --body "x" --strict-env-clear

# Para toda a sessão
export SQLITE_GRAPHRAG_STRICT_ENV_CLEAR=1
sqlite-graphrag remember --name minha-memoria --body "x"
```

Em modo estrito, apenas `PATH` é preservado. As vars de custom-provider ficam no processo pai; o subprocesso roteia via subscription OAuth ou falha explicitamente.


### Armadilhas Comuns

| Sintoma | Causa Raiz | Correção |
| --- | --- | --- |
| `exit 11` com `401 Invalid authentication credentials` | v1.0.82 ou anterior; env_clear descartou o token | Atualizar para v1.0.83 (`cargo install sqlite-graphrag --version 1.0.83 --force`) |
| `exit 1` com `OAuth-only mandate violated` | `ANTHROPIC_API_KEY` está setada; guard rejeita | Unsetar `ANTHROPIC_API_KEY`; usar `ANTHROPIC_AUTH_TOKEN` em vez |
| Embedding sucede mas `recall` não retorna nada | Provider retornou dimensionalidade diferente da do banco | Rodar `sqlite-graphrag enrich --operation re-embed --target all --limit 100 --mode openrouter --openrouter-model MODELO` para refrescar embeddings na dim ativa do provider (`--target all` desde a v1.1.01; omita em binários mais antigos) |
| Token aparece em logs do stderr | (nunca deve acontecer; teste de auditoria enforça) | Reportar bug com captura do stderr; o teste no-leak `audit_no_token_leak_in_subprocess_stderr` enforça esse invariante |


### Verificação de que o Fix Funcionou

```bash
# 1. Confirmar versão
sqlite-graphrag --version   # deve reportar 1.0.83

# 2. Confirmar que env propaga para o subprocesso
export ANTHROPIC_AUTH_TOKEN="sk-cp-test-12345"
sqlite-graphrag remember --name v183-verify --body "x" 2> /tmp/stderr.log
grep -F "sk-cp-test-12345" /tmp/stderr.log   # não deve retornar nada

# 3. Confirmar que o abort OAuth-only ainda funciona
export ANTHROPIC_API_KEY="sk-ant-violation"
sqlite-graphrag remember --name v183-oauth-abort --body "x"
echo $?   # deve imprimir 1
unset ANTHROPIC_API_KEY
```

Veja `docs/decisions/adr-0041-preserve-custom-provider-env.pt-BR.md` para a decisão arquitetural completa.
# Livro de Receitas sqlite-graphrag


> 34 receitas de nível produção que poupam horas da sua equipe toda semana

- Leia a versão em inglês em [COOKBOOK.md](COOKBOOK.md)


## Aliases de Flags CLI (desde v1.0.35)
- `recall` e `hybrid-search` aceitam `--limit` como alias de `-k`/`--k`. As receitas abaixo usam `--k`; ambos funcionam.
- `rename` aceita `--from`/`--to` como aliases de `--name`/`--new-name`.
- Campos JSON `schema_version` (`init`, `stats`, `migrate`, `health`) são emitidos como números JSON desde v1.0.35.
- `rename` aceita argumentos posicionais: `rename <antigo> <novo>` (desde v1.0.44)
- `related` aceita argumento posicional de nome: `related <nome>` (desde v1.0.44)
- `graph entities` JSON response usa `entities` como chave de array top-level (renomeado de `items` em v1.0.44)
- `link --create-missing` auto-cria entidades inexistentes durante link (desde v1.0.44)
- `hybrid-search --with-graph` habilita travessia de grafo semeada dos top resultados RRF (desde v1.0.44)


## Nota de Latência — v1.0.76 Apenas LLM
- A CLI é 100% one-shot. Cada `remember`, `ingest`, `recall` ou `hybrid-search` emite uma requisição REST ao OpenRouter para geração de embedding
- Não há daemon, não há IPC, não há processo em segundo plano
- O custo de spawn de subprocesso é aproximadamente 1-3 segundos por chamada
- Pipelines em lote devem fazer batching no lado LLM (um prompt com N passagens) via `embed_passages_controlled` para amortizar o custo de spawn.
- Operadores com corpora muito grandes devem confiar no FTS5 (`hybrid-search --k 50`) para filtragem grossa antes de chegar ao refinamento por cosseno (veja ADR-0024).


## Referência de Valores Padrão
- `recall --k` padrão é 10 (não 5) — ajuste conforme o tradeoff precisão-revocação
- `list --limit` padrão é 50 — use `--limit 10000` para exportações completas antes de backup
- `hybrid-search --weight-vec` e `--weight-fts` ambos têm padrão 1.0
- `purge --retention-days` padrão é 90 — reduza para políticas de limpeza mais agressivas
- `ingest --max-files` padrão é 10000 — cap de segurança all-or-nothing, não janela deslizante
- `ingest --ingest-parallelism` padrão é `min(4, max(1, cpus/2))`
- `ingest --type` padrão é `document` quando omitido
- `link --weight` padrão é 0.5
- `graph traverse --depth` padrão é 2
- `hybrid-search --min-weight` padrão é 0.3 quando `--with-graph` está ativo


## Como Bootstrapar O Banco De Memória Em 60 Segundos
### Problem
- Seu laptop novo não tem banco de memória e seu agente perde contexto o tempo todo
- Cada onboarding queima 30 minutos com scripts frágeis e caça ao README


### Solution
```bash
cargo install --path .
sqlite-graphrag init --namespace global
sqlite-graphrag health --json
```


### Explanation
- Comando `init` cria o arquivo SQLite; sem download de modelo — o modelo remoto é o modelo
- Flag `--namespace global` fixa o escopo inicial para seus agentes concordarem no alvo
- Comando `health` valida a integridade com `PRAGMA integrity_check` devolvendo JSON
- Exit code `0` sinaliza que o banco está pronto para leitura e escrita por qualquer agente
- Poupa 30 minutos por laptop contra bootstrap Pinecone mais Docker mais Python


### Variants
- Compartilhe o DB com `--db /data/team.sqlite` em cada chamada, ou persista via `sqlite-graphrag config set db.path /data/team.sqlite` (XDG). (**HISTÓRICO:** product env `SQLITE_GRAPHRAG_DB_PATH` não é lida no hot path da v1.2.0.)
- Rode `sqlite-graphrag migrate --json` após bump de versão para aplicar upgrade de schema


### See Also
- Receita "Como Integrar sqlite-graphrag Com Loop Subprocess Do Claude Code"
- Receita "Como Agendar Purge E Vacuum Com Cron, systemd.timer Ou launchd"


## Como Usar OpenRouter Para Embedding Rápido (v1.0.93)
### Problem
- Embedding via subprocesso LLM leva 15-60 segundos por chamada por causa do cold-start do processo
- Ingest em massa de 100+ documentos leva horas com embedding via subprocesso
- Você quer embedding mais rápido sem alterar o schema do banco existente

### Solution
```bash
# Defina sua API key do OpenRouter
printf "%s" "sk-or-v1-sua-chave-aqui" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY não é lida em runtime (G-T-XDG-04)

# Remember com OpenRouter (melhor qualidade: Google Gemini 001)
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "google/gemini-embedding-001" \
  remember --name minha-nota --type note \
  --description "embedding rápido via OpenRouter" \
  --body "conteúdo a embedar" --json

# Ingest em massa com OpenRouter + auto-enrich
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  ingest ./docs --pattern "*.md" --recursive \
  --enrich-after --llm-backend openrouter --json

# Tier gratuito: use NVIDIA Nemotron (sem custo)
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "nvidia/llama-nemotron-embed-vl-1b-v2:free" \
  recall "query de busca" --k 10 --json
```

### Explanation
- `--embedding-backend openrouter` seleciona o caminho REST API (~200ms vs 15s subprocesso)
- `--embedding-model` é OBRIGATÓRIO — não há modelo padrão para OpenRouter
- Todos os 10 modelos verificados produzem vetores de 384 dimensões via MRL — zero mudança de schema
- Top modelos por recall score: Google Gemini 001 (0,892), Mistral (0,832), Qwen 8B (0,814)
- Opção gratuita: NVIDIA Nemotron produz qualidade razoável (0,662) sem custo
- `--enrich-after` no ingest dispara extração de entidades após o embedding completar
- Erros de configuração do OpenRouter retornam exit code 78 (`EX_CONFIG`)

### See Also
- Receita "Como Bootstrapar O Banco De Memória Em 60 Segundos"
- `docs/decisions/adr-0052-openrouter-embedding-backend.md`


## Arquitetura One-Shot (v1.0.76+)
### Status
- O subcomando `daemon` e TODA infraestrutura de daemon foram REMOVIDOS do codebase
- A CLI é 100% one-shot: cada `remember` / `ingest` / `recall` / `hybrid-search` emite uma requisição REST ao OpenRouter para geração de embedding
- Não há IPC, não há Unix socket, não há processo em segundo plano
- Veja ADR-0021 para a justificativa da deprecação e ADR-0019 para a arquitetura LLM-only


### See Also
- Receita "Como Bootstrapar O Banco De Memória Em 60 Segundos"
- Receita "Como Fazer Benchmark De hybrid-search Contra recall Vetorial Puro"


## Como Atualizar Para a v1.1.06 (Scan O(k) do entity-connect — Sem Migração)

- Nenhuma migração de banco; schema permanece em **v16**. Basta `cargo install sqlite-graphrag --locked --force` (nome oficial **v1.1.06**; versão no `Cargo.toml` é `1.1.6`; SemVer rejeita zero à esquerda no patch).
- Fecha **GAP-ENTITY-CONNECT-SCAN-CARTESIAN**: scan O(k) por coocorrência + hub×ilha; chaves `pair:{id1}:{id2}` / `item_type=entity_pair`; **drain por PK sem re-scan**; primeiro scan com `--max-runtime` / teto soft 120s via `InterruptHandle` → Timeout exit **1** (não 75); NDJSON `scan_start` / `scan_meta` com backlog dual; GAP-002 preservado; `cross-domain-bridges` no mesmo path.
- Consumidores de biblioteca pinam `=1.1.6`. ADR-0066. Suite: `tests/v1106_entity_connect_scan_regression.rs`.
- Veja a receita unificada "entity-connect Convergente (GAP-002, ADR-0064 + v1.1.06 scan O(k))" abaixo (mesma narrativa do COOKBOOK EN).
- Smoke: `enrich --operation entity-connect --dry-run --json --limit 50 --mode openrouter --openrouter-model <MODELO>` deve terminar em ms–s com `scan_start` depois `scan`.
- Se `--max-runtime` for omitido, o teto soft de 120s ainda cobre o primeiro scan.

## Como Atualizar Para a v1.1.05 (Cinco Bugs do Incidente deep-research "danilo" — Sem Migração)

- Nenhuma migração de banco; o schema permanece em v16 (V016 da v1.1.04). Basta `cargo install sqlite-graphrag --locked --force`.
- O nome de release é v1.1.05; a versão do `Cargo.toml` é `1.1.5` (SemVer rejeita zero à esquerda no componente patch); binário ~19 MiB; User-Agent `sqlite-graphrag/1.1.5`.
- ADR: [ADR-0065](decisions/adr-0065-v1-1-05-danilo-bugs.pt-BR.md). Suite de regressão: [`tests/v1105_danilo_bugs_regression.rs`](../tests/v1105_danilo_bugs_regression.rs).
- Bug 1: `deep-research` com token único gera sub-queries multi-aspecto (`source: "aspect"`); opcional `--sub-query-strategy manual --sub-queries-file`.
- Bug 2: `deep-research --output PATH` (atomwrite) + ack no stdout `{written, bytes, blake3, sub_queries_total, unique_memories_found, elapsed_ms}` + `--quiet` + contrato stdout-JSON / stderr-logs. Schema: [`deep-research-output-ack.schema.json`](schemas/deep-research-output-ack.schema.json).
- Bug 3: `graph traverse --fuzzy` + sugestões em NotFound (exit 4).
- Bug 4: `merge-entities` rejeita self-ref pré-DB.
- Bug 5: `link --from-id`/`--to-id`; nomes só dígitos rejeitados.
- Consumidores de biblioteca fixam `=1.1.5`.
- Veja a seção de receitas v1.1.05 abaixo.

## Como Atualizar Para a v1.1.04 (Correção de Runtime Aninhado do deep-research + Convergência do entity-connect + Schema v16)

- Migração de banco OBRIGATÓRIA: `migrate --json` aplica V016 (tabela `entity_connect_seen`). O schema avança de v15 para v16.
- O nome de release é v1.1.04; a versão do `Cargo.toml` é `1.1.4` (SemVer rejeita zero à esquerda no componente patch); binário ~19 MiB; User-Agent `sqlite-graphrag/1.1.4`.
- GAP-001 (pânico de runtime aninhado do deep-research) está corrigido — `deep-research` agora emite um envelope JSON estruturado em 100% das invocações.
- GAP-002 (não-convergência do entity-connect) está corrigido — `enrich --operation entity-connect --until-empty` agora converge.
- entity-connect é promovido de scan-only para fully-implemented.
- Basta `cargo install sqlite-graphrag --locked --force`, depois `sqlite-graphrag migrate --json`, depois `sqlite-graphrag health --json` (confirme `schema_version >= 16`).
- Consumidores de biblioteca fixam `=1.1.4`.
- Veja "Receitas adicionadas na v1.1.04" abaixo.

## Como Atualizar Para a v1.1.02 (Remoção do GLiNER + TooManyTokens Tipado + Prune de Órfãos de Entidade)
- Sem migração de banco; schema permanece em v15. Basta `cargo install sqlite-graphrag --locked --force` (nome de release v1.1.02; versão do `Cargo.toml` é `1.1.2`; binário ~19 MiB).
- BREAKING (Gap 1): `--gliner-variant` foi REMOVIDA de `remember`/`ingest` (clap rejeita com exit 2, seguindo o precedente `--max-entity-degree` da v1.0.99); `--mode gliner` também foi REMOVIDO (o enum `IngestMode` agora expõe apenas `none`); as env vars `SQLITE_GRAPHRAG_GLINER_MODEL`/`SQLITE_GRAPHRAG_GLINER_THRESHOLD` são silenciosamente ignoradas. Audite seus scripts (`rg -- "--gliner-variant|--mode gliner" seus-scripts/ ci/ Makefile .github/`) e apague todas as ocorrências.
- Gap 2: `AppError::TooManyTokens{tokens,limit}` é a nova variante tipada exit 6 (junta-se a `BodyTooLarge`/`TooManyChunks`); o envelope JSON informa `{tokens,limit}` para o caller distinguir bytes vs chunks vs tokens.
- Gap 3: o dispatch `strip_prefix("entity:")` em `call_reembed` é coberto pelo teste de regressão `tests/reembed_entities_integration.rs` — embeddings de entidades agora fazem backfill de forma confiável.
- Nova flag de manutenção `enrich --prune-dead-entity-orphans` (ADR-0062): deleta linhas `status='dead' AND item_type='entity'` do `.enrich-queue.sqlite`; mutuamente exclusiva com `--prune-dead-orphans`. Rode ambas em sequência para uma varredura completa de órfãos após renomeação/fusão/purga em massa de entidades.
- 4 warnings rustdoc pré-existentes resolvidos (backticks em blocos HTML, intra-doc links cfg(test)).
- Veja "Receitas adicionadas na v1.1.02" no fim deste cookbook.

## Como Atualizar Para a v1.1.01 (Backfill de Cobertura Vetorial + Manutenção do Grafo)
- Sem migração de banco; schema permanece em v15. Basta `cargo install sqlite-graphrag --locked --force` (nome de release v1.1.01; versão do `Cargo.toml` é `1.1.1`; binário ~19 MiB).
- `enrich --operation re-embed` ganha `--target memories|entities|chunks|all` (P2): embeddings de entidades e chunks agora podem receber backfill, e `enrich --status` reporta `scan_backlog` por alvo.
- Novo comando de manutenção `graph recompute-degree` (P3): recalcula `entities.degree` a partir das arestas reais em transação única; prévia com `--dry-run`; envelope `{total, updated, zeroed, unchanged}`.
- `reclassify-relation --literal-from` (P4): match verbatim do valor de relação gravado, contornando a normalização do clap — a receita canônica migra arestas legadas com underscore (ex.: `applies_to`) para a forma canônica com hífen.
- Manutenção de entidades por ID (P5): `merge-entities --ids "1,2,3" --into-id 4` e `rename-entity --id N --new-name x`.
- Diagnóstico de cobertura vetorial (P6): `health --json` agora emite `vec_memories_missing`/`vec_entities_missing`/`vec_chunks_missing` mais `vec_*_coverage_pct`; `embedding status --json` adiciona `memories_missing`/`entities_missing`/`chunks_missing` em `coverage`.
- `ingest --name-prefix <prefixo>` (P12): prefixa os nomes das memórias ingeridas para evitar colisões entre projetos que compartilham um namespace.
- Contexto: o embedding de entidade agora passa pelo REST do OpenRouter mesmo com `--llm-backend none` (P1); o exit 6 agora traz mensagem tipada indicando qual teto estourou — o corpo de 512000 bytes ou o limite de 512 chunks (P11).
- Veja "Receitas adicionadas na v1.1.01" no fim deste cookbook.

## Como Atualizar Para a v1.0.99 (Remoção do Degree-Cap — BREAKING)
- Sem migração de banco; schema permanece em v15. Basta `cargo install sqlite-graphrag --locked --force`.
- BREAKING: a flag `--max-entity-degree` foi REMOVIDA de `remember` e `link`. Passá-la falha com clap exit 2. Audite seus scripts (`rg -- "--max-entity-degree" seus-scripts/`) e delete todas as ocorrências, incluindo a mitigação `--max-entity-degree 0`, que é obsoleta.
- As escritas agora são 100% aditivas: `remember`/`link` nunca podam nem deletam arestas, portanto o total de relacionamentos nunca decresce numa escrita normal (GAP-SG-67, ADR-0059). Trade-off: o grau de hubs cresce sem limite; normalize depois apenas com um comando de MANUTENÇÃO explícito.
- `graph entities --sort-by degree` ordena de forma ascendente por padrão; adicione `--order desc` para mais-conectado-primeiro (GAP-SG-68).
- `enrich --operation body-enrich ... --until-empty` agora converge; corpos curtos vetados não são re-enfileirados (GAP-SG-69).

## Como Atualizar Para a v1.0.94 (Remediação de Quatro Gaps)
- Sem migração de banco; schema permanece em v15. Basta `cargo install sqlite-graphrag --locked --force`.
- QUEBRANTE: toda invocação de `enrich` agora exige `--mode`. O único valor aceito é `openrouter`. Atualize scripts para `enrich --operation memory-bindings --mode openrouter --openrouter-model MODELO`.
- A dimensão de embedding padrão é **1024** (v1.2.0). Bancos novos usam 1024; bancos legados em 64/384 mantêm a dim registrada. Re-embede na dim ativa com `sqlite-graphrag --embedding-backend openrouter --embedding-model MODELO enrich --operation re-embed --limit 100 --resume --mode openrouter --openrouter-model MODELO --json`.

## Como Atualizar De v1.0.74 Ou v1.0.75 Para v1.0.76 (Apenas LLM)
### Problema
- Você tem um banco v1.0.74 ou v1.0.75 com virtual tables vec0 (`vec_memories`, `vec_entities`, `vec_chunks`)
- O binário v1.0.76 removeu `sqlite-vec`, `fastembed`, `GLiNER` e o crate `tokenizers`
- Um `migrate` simples pode falhar com "applied migration V2 is different than filesystem one V2" (mismatch de checksum do refinery) porque V002 foi intencionalmente esvaziada para no-op na v1.0.76
- Um binário rotulado errado ainda pode reportar `1.0.76` enquanto embute o arquivo V002 antigo da v1.0.54
- Você quer que o upgrade seja one-shot, não uma sessão SQL manual

### Solução
```bash
# 1. Instale o novo binário
cargo install sqlite-graphrag --locked --force

# 2. Upgrade one-shot no seu banco existente
sqlite-graphrag migrate --to-llm-only --drop-vec-tables --db /caminho/para/graphrag.sqlite
```

### Explicação
- `--to-llm-only` executa três coisas em uma transação:
  - Detecta se as virtual tables `vec_memories`, `vec_entities`, `vec_chunks` existem em `sqlite_master` e reporta `vec_tables_were_present: true|false`
  - Reescreve os checksums de migração registrados para casar com o conteúdo atual do arquivo (cobre o mismatch V002; veja `--rehash` abaixo para a flag standalone)
  - Aplica a migração V013 que dropa as três vec tables e cria as tabelas BLOB-backed `memory_embeddings`, `entity_embeddings`, `chunk_embeddings`
- `--drop-vec-tables` é a guarda de segurança explícita; sem ela, `--to-llm-only` recusa rodar
- A CLI é `~14.6 MiB` (de 39 MB); sem download de modelo ONNX; sem instalação local do fastembed
- Novas chamadas de `remember`, `edit`, `ingest` re-embutem a memória afetada via subprocesso headless `claude code` ou `codex` (OAuth)
- Veja `docs/MIGRATION.pt-BR.md` para o caminho completo v1.0.74 → v1.0.76 → v1.1.0 e `docs/decisions/adr-0019-llm-only-one-shot.pt-BR.md` para a justificativa arquitetural

### Variantes
- Se você só precisa da reescrita de checksum sem aplicar migrações, use `sqlite-graphrag migrate --rehash`
- Se o mismatch persistir mesmo com `sqlite-graphrag --version` reportando `1.0.76`, reconstrua do checkout de fonte local e substitua o binário instalado antes de tocar em `refinery_schema_history`:

```bash
cargo build --release
cp target/release/sqlite-graphrag ~/.cargo/bin/sqlite-graphrag
```

- Veja `docs/decisions/adr-0026-v002-vec-tables-migration-drift.pt-BR.md` para a causa raiz completa e o rastro de validação
- A feature `embedding-legacy` foi removida na v1.0.79; para manter o pipeline fastembed da v1.0.74 é preciso fixar `--version 1.0.78` ou anterior (sem suporte)

### Correção v1.0.78: `run_rehash` Registrava V013 Sem Executar o SQL (G41)
- A v1.0.76 e v1.0.77 tinham um bug (G41) onde `migrate --rehash` inseria linhas fantasma para migrações não aplicadas em `refinery_schema_history`
- Isso fazia o runner do refinery pular V013 inteiramente — as tabelas BLOB-backed de embedding nunca eram criadas
- Sintomas: `no such table: memory_embeddings` (exit 10) em `recall`, `hybrid-search`, `remember`
- A v1.0.78 remove a inserção fantasma e adiciona reparo automático em todo comando CRUD

```bash
cargo install sqlite-graphrag --version 1.0.78 --force
# Qualquer comando repara automaticamente — não precisa de migrate explícito:
sqlite-graphrag recall "consulta teste" --json
```

- Veja ADR-0028 e `docs/MIGRATION.pt-BR.md` para os detalhes completos

### Correção v1.0.77: `migrate --rehash` Inseria `applied_on = NULL`
- A v1.0.76 tinha um bug (G40) onde `migrate --rehash` inseria linhas em `refinery_schema_history` sem o campo `applied_on`, deixando-o NULL
- O driver rusqlite do refinery-core 0.9.1 lê `applied_on` como `String` (NOT NULL), crashando com `InvalidColumnType(Null at index: 2)` na próxima migração
- A v1.0.77 detecta e corrige automaticamente essas linhas NULL antes de rodar o migration runner
- Se você foi afetado por esse bug na v1.0.76, atualize para v1.0.77 — nenhuma intervenção manual em SQL é necessária:

```bash
cargo install sqlite-graphrag --version 1.0.77 --force
sqlite-graphrag migrate
```

- Veja ADR-0027 e `docs/MIGRATION.pt-BR.md` para os detalhes completos


## Como Importar Em Massa Um Diretório De Base De Conhecimento
### Problem
- Seus 2000 arquivos Markdown ficam parados porque nenhum loader fala o schema sqlite-graphrag
- Entrada manual queima uma tarde inteira para cada cem arquivos de onboarding simples


### Solution
```bash
sqlite-graphrag ingest ./docs --recursive --pattern "*.md" --json \
  | jaq -c 'select(.status == "indexed") | .name'
```


### Explanation
- `ingest` substitui o loop `fd | xargs remember` por um único comando atômico com recursão e nomeação
- `--recursive` desce em subdiretórios; sem ele apenas o nível raiz é processado
- `--pattern "*.md"` filtra por extensão; padrão é `*.md` então a flag é mostrada para clareza
- Saída é NDJSON: uma linha JSON por arquivo com campo `status`, mais uma linha final de resumo com `summary: true`
- Nomes derivam dos basenames dos arquivos em kebab-case; nomes com mais de 60 caracteres são truncados com `truncated: true` no NDJSON
- Poupa 4 horas por mil arquivos contra scripts de importação artesanais ou loops `fd | xargs`


### Variants
- Extração automática desabilitada por padrão; use `--enable-ner` ou `SQLITE_GRAPHRAG_ENABLE_NER=1` para ativar — SOMENTE URL-regex desde a v1.0.79 (o pipeline GLiNER foi removido); a flag `--gliner-variant` foi REMOVIDA em v1.1.02 (clap rejeita com exit 2) e as env vars `SQLITE_GRAPHRAG_GLINER_MODEL`/`SQLITE_GRAPHRAG_GLINER_THRESHOLD` são silenciosamente ignoradas
- `--skip-extraction` está obsoleto desde v1.0.45 e não tem efeito; NER está desabilitado por padrão, use `--enable-ner` para ativar
- Campo de resposta `extraction_method` informa `url-regex` ou `none:extraction-failed`; os valores `gliner-<variant>+regex` e `regex-only` são HISTÓRICOS (≤ v1.0.75)
- Arquivos duplicados retornam `status: "skipped"` com `action: "duplicate"` em vez de `status: "failed"`
- Use `--fail-fast` para abortar no primeiro erro por arquivo em vez de continuar com report inline
- Use `--max-rss-mb <MiB>` para abortar embedding quando o RSS do processo exceder o limite (padrão 8192 MiB); útil em CI ou containers com memória restrita


### See Also
- Receita "Como Importar Corpora Grandes Em Hosts Com Memória Limitada"
- Receita "Como Exportar Memórias Para NDJSON Para Backup"


## Como Importar Um Diretório Tipado Com Progresso Em Streaming
### Problem
- Seu pipeline CI ingere 2000 documentos de decisão mas não tem visibilidade de progresso durante a execução
- A abordagem de resumo final esconde falhas por arquivo até o lote inteiro completar


### Solution
```bash
sqlite-graphrag ingest ./decisions --type decision --recursive --json \
  | while IFS= read -r line; do
      status=$(echo "$line" | jaq -r '.status // empty')
      if [ "$status" = "failed" ]; then
        echo "FAIL: $(echo "$line" | jaq -r '.file')" >&2
      elif [ "$status" = "skipped" ]; then
        echo "SKIP: $(echo "$line" | jaq -r '.file') (duplicate or invalid name)"
      fi
    done
```


### Explanation
- `--type decision` marca cada arquivo ingerido como memória do tipo `decision`; tipo padrão é `document`
- Saída NDJSON transmite uma linha por arquivo seguida de uma linha resumo com `summary: true`
- O loop `while read` processa cada linha ao chegar em vez de esperar o lote completo
- Filtre por `select(.status)` para ignorar a linha resumo que não tem campo `status`
- Valores válidos de `--type`: `user`, `feedback`, `project`, `reference`, `decision`, `incident`, `skill`, `document`, `note`
- Invoque `ingest` separadamente por tipo quando um diretório contém conteúdo misto


### Variants
- Agregue estatísticas finais: `| jaq -sc '[.[] | select(.status)] | group_by(.status) | map({status: .[0].status, count: length})'`
- Use `--pattern "memo-*"` para filtrar por prefixo de basename em vez de extensão


### See Also
- Receita "Como Importar Em Massa Um Diretório De Base De Conhecimento"
- Receita "Como Exportar Memórias Para NDJSON Para Backup"


## Como Combinar Busca Vetorial E FTS Com Pesos Ajustáveis
### Problem
- Recall vetorial puro perde matches exatos de token tipo `TODO-1234` em comentários de código
- FTS puro perde paráfrases que seus usuários digitaram em sinônimos e abreviações


### Solution
```bash
sqlite-graphrag hybrid-search "postgres migration deadlock" \
  --k 10 --rrf-k 60 --weight-vec 1.0 --weight-fts 1.0 --json
```


### Explanation
- `--rrf-k 60` é a constante de suavização Reciprocal Rank Fusion recomendada na literatura
- `--weight-vec 1.0` e `--weight-fts 1.0` são os padrões — ambas as fontes têm peso igual
- Ajuste os pesos apenas para tradeoffs explícitos entre semântica e precisão de tokens
- JSON emite `vec_rank` e `fts_rank` por resultado para agentes downstream auditarem a fusão
- Poupa 50 por cento dos tokens contra pedir a um LLM para re-rankear após vetor puro


### Variants
- Defina `--weight-vec 1.0 --weight-fts 0.0` para reproduzir um baseline `recall` puro em A/B
- Eleve `--k` para 50 antes de um re-ranker agent podar até os 5 hits finais
- Passe `--with-graph --max-hops 2` para semear travessia de grafo dos top resultados RRF; leia ambos `results[]` e `graph_matches[]` na saída (desde v1.0.44)


### See Also
- Receita "Como Debugar Queries Lentas Com Health E Stats"
- Receita "Como Expandir Hybrid Search Com Contexto De Grafo"


## Como Expandir Hybrid Search Com Contexto De Grafo
### Problem
- Seu hybrid search encontra as memórias seed certas mas perde conceitos relacionados conectados via grafo de entidades
- Rodar um comando `related` separado após cada hybrid search adiciona complexidade e latência ao pipeline


### Solution
```bash
sqlite-graphrag hybrid-search "authentication architecture" \
  --k 10 --with-graph --max-hops 2 --min-weight 0.3 --json \
  | jaq -r '(.results[], .graph_matches[]) | .name' | sort -u
```


### Explanation
- `--with-graph` habilita travessia de grafo de entidades semeada dos top resultados RRF (corrigido em v1.0.44)
- Matches de grafo aparecem em `graph_matches[]`, um array SEPARADO de `results[]`; leia AMBOS arrays
- `graph_matches[]` usa schema RecallItem: `name`, `distance`, `source` ("graph"), `graph_depth`
- `--min-weight 0.3` filtra arestas fracas do grafo para reduzir ruído de relações de baixa confiança
- `--max-hops 2` controla profundidade de travessia; aumente apenas após checar densidade via `graph stats`
- Elimina a necessidade de chamada separada de `related`, reduzindo etapas do pipeline de três para duas


### Variants
- Defina `--min-weight 0.0` para incluir todas as arestas independente do peso para máximo recall com mais ruído
- Extraia nomes de ambos arrays: `jaq -r '(.results[], .graph_matches[]) | .name' | sort -u > seeds.txt`


### See Also
- Receita "Como Combinar Busca Vetorial E FTS Com Pesos Ajustáveis"
- Receita "Como Explorar O Grafo De Entidades Com Stats, Entities E Traverse"


## Pesquisa Profunda Para Análise Multi-Hop Abrangente
### Problem
- Você precisa pesquisar um tópico complexo que abrange múltiplas memórias e conexões do grafo
- Rodar o pipeline manual de 3 camadas (hybrid-search, read, related) para cada sub-tópico é tedioso e lento


### Solution
```bash
sqlite-graphrag deep-research "decisões de arquitetura de autenticação e incidentes de segurança" --k 20 --max-hops 3 --json
```


### Explanation
- Decompõe a query em sub-queries ("decisões de arquitetura de autenticação", "incidentes de segurança", "autenticação segurança") e executa em paralelo com travessia do grafo
- Retorna resultados deduplicados de todas sub-queries mais cadeias de evidência mostrando caminhos entity-relation-entity
- Uma única invocação substitui o pipeline manual de 3 camadas de hybrid-search, read e related


### Variants
- Adicione `--with-bodies` para conteúdo completo das memórias na resposta
- Adicione `--max-concurrency 4` para limitar paralelismo em hosts com recursos limitados


### See Also
- Receita "Como Combinar Busca Vetorial E FTS Com Pesos Ajustáveis"
- Receita "Como Expandir Hybrid Search Com Contexto De Grafo"
- Receita "Como Percorrer O Grafo De Entidades Para Recall Multi-Hop"


## Como Percorrer O Grafo De Entidades Para Recall Multi-Hop
### Problem
- Sua query acerta uma memória mas perde notas conectadas que compartilham o mesmo grafo
- RAG vetorial puro pontua tokens similares e ignora relações tipadas que importam


### Solution
```bash
sqlite-graphrag related authentication-flow --hops 2 --json
```


### Explanation
- `related` percorre relacionamentos tipados do grafo entre entidades com contagem controlada
- `--hops 2` inclui memórias amigas-de-amigos conectadas via entidades compartilhadas
- Saída JSON reporta o caminho da travessia para o LLM raciocinar sobre cadeias de relação
- Argumento posicional de nome suportado desde v1.0.44: `related <nome>` é equivalente a `related --name <nome>`
- Poupa custo de re-embedding porque a expansão roda como grafo SQLite e não KNN
- Revela contexto que o RAG vetorial puro ignora com 80 por cento menos tokens


### Variants
- Use `graph --json` para dump completo quando um auditor humano quiser análise offline
- Encadeie `related` em `hybrid-search` filtrando candidatos ao conjunto percorrido


### See Also
- Receita "Como Combinar Busca Vetorial E FTS Com Pesos Ajustáveis"
- Receita "Como Orquestrar Recall Paralelo Entre Namespaces"


## Como Encadear Recuperação Profunda Em 3 Camadas
### Problema
- Seu agente dispara um único recall e perde tanto o body completo quanto os vizinhos transitivos do grafo
- Despejar todas as memórias em markdown queima 72x mais tokens de contexto do que uma cadeia de recuperação focada


### Solução
```bash
# Camada 1: hybrid-search encontra memórias seed via FTS5 + vetor RRF
SEED=$(sqlite-graphrag hybrid-search "arquitetura de autenticação" --k 3 --json \
  | jaq -r '.results[0].name')

# Camada 2: read expande o corpo completo do top seed
sqlite-graphrag read "$SEED" --json | jaq -r '.body'

# Camada 3: related descobre conhecimento transitivo via o grafo de entidades
sqlite-graphrag related "$SEED" --hops 2 --json \
  | jaq -r '.results[].name'
```


### Explicação
- Camada 1 (hybrid-search) encontra as memórias mais relevantes usando ranking combinado de texto e vetor
- Camada 2 (read) recupera o corpo completo do melhor resultado (hybrid-search retorna snippets truncados)
- Camada 3 (related) percorre o grafo de entidades para descobrir memórias conectadas invisíveis à busca vetorial
- Este padrão reduz tokens de contexto em até 72x versus dump de todas memórias em markdown
- Encadeie no prompt do LLM coletando o body da Camada 2 mais os nomes da Camada 3 para uma janela de contexto focada


### Variantes
- Troque `--k 3` por `--k 1` quando suas queries forem altamente específicas e você confiar no top hit
- Aumente `--hops` para 3 quando o grafo de entidades tiver conectividade esparsa entre tópicos


### Veja Também
- Receita "Como Combinar Busca Vetorial E FTS Com Pesos Ajustáveis"
- Receita "Como Percorrer O Grafo De Entidades Para Recall Multi-Hop"


## Como Usar Recuperação De Deriva De Schema (v1.0.89+, GAP-E2E-007, ADR-0048)

### Problem
- Um consumidor de `sqlite-graphrag health --json` parseia a resposta contra o schema publicado
- Após upgrade para v1.0.89, o validador do consumidor rejeita novos campos como `vec_memories_missing`, `sqlite_version`, `mentions_warning`, etc.
- Validação estrita (`additionalProperties: false`) falha nos 17 novos campos adicionados pelo derive `schemars 0.8`

### Solution
- `health.schema.json` usa `additionalProperties: true` (política Must-Ignore por RFC 7493 I-JSON e `rules_rust_json_e_ndjson.md:33`)
- Consumidores devem migrar para Must-Ignore para compatibilidade forward
- O novo `src/bin/dump_schema.rs` regenera o schema de forma idempotente via `schema_for!()` + ordenação BTreeMap + enforcement recursivo da política `apply_must_ignore`

### Recipe — Migrar consumidor Python para Must-Ignore

```python
import json
import jsonschema
from jsonschema import Draft202012Validator

# Pré-v1.0.89: validação estrita rejeitava chaves desconhecidas
with open("docs/schemas/health.schema.json") as f:
    schema = json.load(f)

# Antes: Draft202012Validator(schema) — falha em novos campos da v1.0.89
# Agora: ainda Draft202012Validator(schema), mas additionalProperties: true
#         significa que chaves desconhecidas são aceitas (Must-Ignore)

validator = Draft202012Validator(schema)
out = subprocess.check_output(["sqlite-graphrag", "health", "--json"])
errors = list(validator.iter_errors(json.loads(out)))
# Pré-v1.0.89 com schema estrito: errors continha "Additional properties are not allowed ('vec_memories_missing' was unexpected)"
# v1.0.89+ com Must-Ignore: zero erros para chaves desconhecidas; só violações de type/range permanecem
```

### Recipe — Migrar consumidor TypeScript para Must-Ignore

```typescript
import Ajv from "ajv";
import * as fs from "fs";

const schema = JSON.parse(fs.readFileSync("docs/schemas/health.schema.json", "utf-8"));
const ajv = new Ajv({ strict: false, allErrors: true });  // strict: false aceita additionalProperties
const validate = ajv.compile(schema);

const out = JSON.parse(execSync("sqlite-graphrag health --json").toString());
const valid = validate(out);
if (!valid) {
    console.error("Validation errors:", validate.errors);
}
```

### Recipe — Regenerar o schema a partir do fonte

```bash
# O schema agora é derivado de HealthResponse via macro derive schemars 0.8
# Regenere de forma idempotente:
cargo run --bin dump_schema -- --output docs/schemas/health.schema.json

# Valide que o schema regenerado bate com o que está no repo
diff <(cargo run --bin dump_schema) docs/schemas/health.schema.json
# Esperado: diff vazio (idempotente)
```

### Cross-references
- `docs/decisions/adr-0048-schema-as-derived-artifact.md` (en + pt-BR) — justificativa da política Must-Ignore
- `docs/HEADLESS_INVOCATION.md` — timeline de adoção do schemars
- `tests/health_schema_drift_regression.rs::assert_all_health_keys_in_schema` — teste de regressão

## Como Linkar Entidades Com Auto-Criação
### Problem
- Criar arestas de grafo requer que entidades existam antes, forçando um workflow tedioso de duas etapas
- Seu script de automação falha com exit code 4 cada vez que tenta linkar entidades não pré-registradas


### Solution
```bash
sqlite-graphrag link \
  --from auth-service --to postgres-db \
  --relation depends-on --weight 0.8 \
  --create-missing --entity-type tool
```


### Explanation
- `--create-missing` auto-cria entidades inexistentes com tipo padrão `concept` (desde v1.0.44)
- `--entity-type tool` sobrescreve o tipo padrão para todas entidades auto-criadas nesta invocação
- JSON response inclui `created_entities: ["auth-service", "postgres-db"]` quando entidades foram criadas
- `--weight` é opcional com padrão 0.5; valores devem estar no intervalo `[0.0, 1.0]`
- 12 tipos canônicos de relação: `applies-to`, `uses`, `depends-on`, `causes`, `fixes`, `contradicts`, `supports`, `follows`, `related`, `mentions`, `replaces`, `tracked-in`. Qualquer string customizada em kebab-case ou snake_case também é aceita desde v1.0.49 (ex.: `implements`, `tested-by`, `blocks`).
- Tipos válidos de entidade: `project`, `tool`, `person`, `file`, `concept`, `incident`, `decision`, `memory`, `dashboard`, `issue_tracker`, `organization`, `location`, `date`


### Variants
- Omita `--create-missing` quando entidades devem pré-existir; exit code 4 sinaliza entidade ausente
- Aceite `--source`/`--target` como aliases de `--from`/`--to` para scripts que usam terminologia source/target


### See Also
- Receita "Como Remover Uma Aresta Do Grafo Com Unlink"
- Receita "Como Explorar O Grafo De Entidades Com Stats, Entities E Traverse"


## Como Remover Uma Aresta Do Grafo Com Unlink
### Problem
- Uma aresta `depends-on` incorreta entre duas entidades polui travessias de grafo com caminhos irrelevantes
- A única opção de remoção que sua equipe conhece é deletar a memória inteira, destruindo corpo e histórico


### Solution
```bash
sqlite-graphrag unlink --from auth-service --to legacy-db --relation depends-on
```


### Explanation
- `--from` e `--to` são obrigatórios; `--relation` é opcional — omita para remover TODOS os relacionamentos entre o par
- `--source`/`--target` são aceitos como aliases de `--from`/`--to` para consistência com `link`
- A operação remove apenas a aresta de relacionamento; entidades e memórias permanecem intactas
- Exit code 4 sinaliza que a aresta especificada não existe no namespace atual
- Execute `cleanup-orphans` depois se as entidades desvinculadas não tiverem conexões restantes


### Variants
- Encadeie `graph entities --json | jaq '.entities[].name'` para descobrir nomes de entidades antes de desvincular
- Use `graph stats` antes e depois para verificar que a contagem de arestas diminuiu como esperado


### See Also
- Receita "Como Linkar Entidades Com Auto-Criação"
- Receita "Como Limpar Entidades Órfãs Após Deleção Em Massa"


## Como Limpar Entidades Órfãs Após Deleção Em Massa
### Problem
- Após esquecer 500 memórias, o grafo de entidades ainda contém centenas de nós órfãos sem arestas
- Travessia de grafo desperdiça ciclos visitando entidades sem saída que não referenciam nada


### Solution
```bash
sqlite-graphrag cleanup-orphans --dry-run --json
sqlite-graphrag cleanup-orphans --yes --json
```


### Explanation
- `--dry-run` audita contagem de órfãos sem modificar o banco; sempre execute isso primeiro
- `--yes` ignora o prompt de confirmação interativo para uso em pipelines automatizados
- Remove entidades que têm zero memórias vinculadas E zero arestas no grafo
- Agende periodicamente após operações em massa de `forget` ou `unlink`
- Não toca em memórias ou histórico de versões; apenas entidades de grafo são afetadas


### Variants
- Encadeie com `purge --retention-days 30 --yes` e `vacuum` em um cron semanal para higiene completa
- Inspecione candidatos primeiro com `graph entities --json | jaq '.entities[] | select(.degree == 0)'` se disponível


### See Also
- Receita "Como Agendar Purge E Vacuum Com Cron, systemd.timer Ou launchd"
- Receita "Como Remover Uma Aresta Do Grafo Com Unlink"


## Como Explorar O Grafo De Entidades Com Stats, Entities E Traverse
### Problem
- Seu grafo cresceu para milhares de entidades e você não tem visibilidade sobre sua densidade ou conectividade
- Planejar profundidade de travessia sem conhecer `avg_degree` desperdiça tempo em subgrafos vazios ou fan-outs sobrecarregados


### Solution
```bash
sqlite-graphrag graph stats --json | jaq '{node_count, edge_count, avg_degree}'
sqlite-graphrag graph entities --entity-type person --json | jaq '.entities[].name'
sqlite-graphrag graph traverse --from acme-corp --depth 3 --json
sqlite-graphrag graph --format mermaid --output graph.md
```


### Explanation
- `graph stats` reporta `node_count`, `edge_count`, `avg_degree` e `max_degree` para informar planejamento de travessia
- `graph entities` lista todas entidades; campo é `.entities[]` NÃO `.items[]` desde v1.0.44
- `graph traverse` parte de uma entidade tipada (não um nome de memória) e caminha até `--depth` hops
- Hops retornam `entity`, `relation`, `direction`, `weight` e `depth` por aresta visitada
- Formatos de exportação incluem `json`, `dot` (Graphviz) e `mermaid`; grave em arquivo via `--output <PATH>`
- Exit code 4 de `graph traverse` sinaliza entidade raiz inexistente


### Variants
- Filtre entidades por tipo: `--entity-type tool` mostra apenas nós do tipo tool
- Pagine listas grandes de entidades: `--limit 100 --offset 200` para datasets com milhares de entidades


### See Also
- Receita "Como Expandir Hybrid Search Com Contexto De Grafo"
- Receita "Como Debugar Queries Lentas Com Health E Stats"


## Como Integrar sqlite-graphrag Com Loop Subprocess Do Claude Code
### Problem
- Claude Code reinicia a cada sessão e esquece decisões feitas cinco minutos atrás
- Seu orquestrador não tem memória determinística entre iterações do agente


### Solution
```bash
# .claude/hooks/pre-task.sh
CONTEXT=$(sqlite-graphrag recall "$USER_PROMPT" --k 5 --json)
printf 'Relevant memories:\n%s\n' "$CONTEXT"

# .claude/hooks/post-task.sh
sqlite-graphrag remember \
  --name "session-$(date +%s)" \
  --type project \
  --description "decision log" \
  --body "$ASSISTANT_RESPONSE"
```


### Explanation
- Hook pre-task injeta memórias relevantes no prompt do agente antes de gerar resposta
- Hook post-task persiste a saída do agente no vector store para sessões futuras
- Scripts de hook rodam como subprocess respeitando exit codes e limites de slots
- Exit code `13` ou `75` dispara retry dentro do hook sem matar o agente
- Poupa 40 por cento dos tokens de contexto e mantém decisões entre restarts do Claude Code


### Variants
- Troque `recall` por `hybrid-search` quando seus prompts misturam palavras e conceitos
- Adicione `--namespace $CLAUDE_PROJECT` para isolar memória por projeto em hosts multi-repo


### See Also
- Receita "Como Integrar Com Codex CLI Via AGENTS.md"
- Receita "Como Configurar Painel Assistente Windsurf Ou Zed Com sqlite-graphrag"


## Como Integrar Com Codex CLI Via AGENTS.md
### Problem
- Codex lê `AGENTS.md` mas pula qualquer capacidade sem sintaxe exata de invocação listada
- Sua equipe de ops perde 10 minutos por sessão ensinando Codex o mesmo CLI de memória


### Solution
```md
<!-- AGENTS.md na raiz do repo -->
## Memory Layer
- Use `sqlite-graphrag recall "<query>" --k 5 --json` to fetch prior decisions
- Use `sqlite-graphrag remember --name "<kebab-name>" --type project --description "<sumário>" --body "<text>"` to persist output
- Prefer `hybrid-search` when the query mixes keywords and natural language
- Respect exit code 75 as retry-later rather than error
```


### Explanation
- AGENTS.md expõe o contrato CLI como parte do contexto do sistema Codex automaticamente
- Codex invoca comandos subprocess listados em AGENTS.md sem prompt adicional do operador
- Exit codes determinísticos permitem Codex reintentar em `75` sem intervenção humana
- Saída JSON integra com camada de parsing do Codex sem regex ou plugin customizado
- Poupa 10 minutos por sessão e sobrevive a upgrades do Codex sem quebrar o contrato


### Variants
- Adicione `SQLITE_GRAPHRAG_NAMESPACE=$REPO_NAME` no `.envrc` para Codex isolar memória por projeto
- Inclua um one-liner de exemplo sob cada comando para ancorar Codex em uso real


### See Also
- Receita "Como Integrar sqlite-graphrag Com Loop Subprocess Do Claude Code"
- Receita "Como Integrar Com Terminal Do Cursor Para Memória No Editor"


## Como Integrar Com Terminal Do Cursor Para Memória No Editor
### Problem
- Cursor perde contexto toda vez que você fecha o editor ou troca de branch localmente
- Sua sessão LLM pareada reinicia fria e repete as mesmas perguntas toda manhã


### Solution
```jsonc
// Snippet do settings.json do Cursor
{
  "terminal.integrated.env.osx": { "SQLITE_GRAPHRAG_NAMESPACE": "${workspaceFolderBasename}" },
  "cursor.ai.rules": "Before answering, run `sqlite-graphrag recall \"${selection}\" --k 5 --json` and use hits as context"
}
```


### Explanation
- Env var por workspace isola memória pelo nome da pasta do projeto sem config manual
- Regras AI do Cursor instruem o modelo embutido a chamar a CLI antes de responder prompts
- A CLI lê apenas o código selecionado então a latência fica abaixo de 50 ms em queries pequenas
- Exit code `0` com hits vazios mantém Cursor calado em vez de alucinar contexto
- Poupa 15 minutos por dia re-perguntando as mesmas coisas em sessões do Cursor


### Variants
- Troque `recall` por `hybrid-search` quando o código mistura docstring inglês e comentários português
- Adicione um hook `post-save` que chama `remember` com o diff como body para memória da sessão


### See Also
- Receita "Como Configurar Painel Assistente Windsurf Ou Zed Com sqlite-graphrag"
- Receita "Como Integrar Com Codex CLI Via AGENTS.md"


## Como Configurar Painel Assistente Windsurf Ou Zed Com sqlite-graphrag
### Problem
- Painéis assistentes do Windsurf e Zed saem sem backend de memória plugável por padrão
- Seu fluxo multi-IDE fragmenta memória entre silos Cursor Windsurf e Zed


### Solution
```bash
# Comando de terminal compartilhado que ambos IDEs podem rodar
sqlite-graphrag hybrid-search "$EDITOR_CONTEXT" --k 10 --json > /tmp/ng.json
```


### Explanation
- Windsurf e Zed chamam tarefas de terminal direto do painel assistente nativamente
- `/tmp/ng.json` atua como lingua franca consumida por ambos painéis para prompts
- Binário CLI único substitui três plugins dedicados evitando manutenção por IDE
- Exit code `0` com hits vazios é benigno então o painel degrada graciosamente
- Poupa horas por semana unificando memória entre editores sem rebuild de plugin


### Variants
- Mapeie o comando para um atalho tipo `Cmd+Shift+M` para invocação de recall com uma tecla
- Canalize a saída por `jaq` para transformar o payload no schema exato que cada IDE prefere


### See Also
- Receita "Como Integrar Com Terminal Do Cursor Para Memória No Editor"
- Receita "Como Orquestrar Recall Paralelo Entre Namespaces"


## Como Prevenir Corrupção Por Dropbox Ou iCloud Com sync-safe-copy
### Problem
- Seu arquivo SQLite mora no Dropbox e sincroniza no meio de uma escrita corrompendo o WAL
- Snapshots `cp` clássicos durante escrita produzem arquivos inválidos que não abrem depois


### Solution
```bash
sqlite-graphrag sync-safe-copy --dest ~/Dropbox/sqlite-graphrag/snapshot.sqlite
```


### Explanation
- O comando força um checkpoint WAL antes da cópia então o snapshot fica transacionalmente consistente
- Arquivo de saída recebe `chmod 600` em Unix para outros usuários não lerem memórias sensíveis
- Cópia roda atômica via `SQLite Online Backup API` eliminando risco de escrita parcial
- Exit code `0` garante que o snapshot abre limpo em qualquer máquina com o mesmo binário
- Poupa fins de semana de recovery quando o Dropbox corromperia o arquivo vivo


### Variants
- Agende de hora em hora via `launchd` no macOS ou `systemd --user` no Linux para backup contínuo
- Comprima com `ouch compress snapshot.sqlite snapshot.tar.zst` para upload cloud mais rápido


### See Also
- Receita "Como Agendar Purge E Vacuum Com Cron, systemd.timer Ou launchd"
- Receita "Como Versionar O Banco SQLite Com Git LFS"


## Como Agendar Purge E Vacuum Com Cron, systemd.timer Ou launchd
### Problem
- Memórias soft-deletadas empilham e incham o uso de disco após meses de uso pesado por agentes
- Seu arquivo SQLite estoura 10 GB porque `VACUUM` nunca roda na automação


### Solution
```yaml
# .github/workflows/ng-maintenance.yml
name: sqlite-graphrag maintenance
on:
  schedule: [{ cron: "0 3 * * 0" }]
jobs:
  maintenance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install --path .
      - run: sqlite-graphrag purge --retention-days 30 --yes
      - run: sqlite-graphrag vacuum --json
      - run: sqlite-graphrag optimize --json
```


### Explanation
- `purge --retention-days 30` apaga definitivamente linhas soft-deletadas mais antigas que a janela
- `vacuum` reclama páginas da freelist e faz checkpoint do WAL para o arquivo principal
- `optimize` refresca estatísticas do planner para recall mais rápido na próxima execução
- Cron semanal às 03:00 de domingo evita contenção com horário comercial de agentes
- Poupa 70 por cento do disco ao longo de 6 meses contra deploy sem manutenção


### Variants
- Rode `cron 0 3 * * *` todas as noites quando seu time escreve milhares de memórias por dia
- Substitua GitHub Actions por `systemd.timer` para ambientes air-gapped sem internet


### See Also
- Receita "Como Prevenir Corrupção Por Dropbox Ou iCloud Com sync-safe-copy"
- Receita "Como Debugar Queries Lentas Com Health E Stats"


## Como Exportar Memórias Para NDJSON Para Backup
### Problem
- Backups SQLite são opacos e exigem o binário instalado para qualquer auditoria de restore
- Compliance pede exports em texto puro para diff entre snapshots mensais


### Solution
```bash
sqlite-graphrag export > memories-$(date +%Y%m%d).ndjson
```


### Explanation
- `export` transmite todas as memórias como NDJSON com uma linha JSON por memória mais um resumo
- Filtre por tipo com `--type decision` ou por namespace com `--namespace my-project`
- Inclua memórias soft-deletadas com `--include-deleted` para trilha de auditoria completa
- Diff dois snapshots com `difft` para auditar o que mudou entre backups mensais limpo
- A partir da v1.0.53 cada escrita faz checkpoint do WAL para que ferramentas de sincronização na nuvem sempre vejam um arquivo consistente


### Variants
- Canalize por `ouch compress` para um arquivo `zst` antes de upload em buckets S3 ou GCS
- Loop em shell para paginar por namespaces se a instância hospeda memória multi-tenant


### See Also
- Receita "Como Versionar O Banco SQLite Com Git LFS"
- Receita "Como Agendar Purge E Vacuum Com Cron, systemd.timer Ou launchd"


## Como Versionar O Banco SQLite Com Git LFS
### Problem
- Seu arquivo SQLite de 500 MB quebra limites de push do GitHub e incha todos os clones
- Rebases de branch corrompem blobs binários quando o Git tenta merge com lógica textual


### Solution
```bash
git lfs install
git lfs track "*.sqlite"
echo "*.sqlite filter=lfs diff=lfs merge=lfs -text" >> .gitattributes
git add .gitattributes graphrag.sqlite
git commit -m "chore: track sqlite-graphrag db via LFS"
```


### Explanation
- Git LFS guarda arquivos SQLite em cache remoto então o repo Git fica abaixo de 100 MB
- Atributo `-text` impede o Git de tentar merge baseado em linha em conteúdo binário
- `sync-safe-copy` antes do commit garante que o arquivo está transacionalmente consistente
- Colegas clonam com `git lfs pull` baixando o DB só quando precisam de fato
- Poupa 90 por cento do tempo de clone para colegas que não precisam do banco local


### Variants
- Tag snapshots com `git tag db-2026-04-18` para fixar estado da memória em release
- Pule LFS e guarde saídas de sync-safe-copy em object storage com URL assinada


### See Also
- Receita "Como Exportar Memórias Para NDJSON Para Backup"
- Receita "Como Prevenir Corrupção Por Dropbox Ou iCloud Com sync-safe-copy"


## Como Orquestrar Recall Entre Namespaces Com Segurança
### Problem
- Seu agente multi-projeto precisa executar um recall por namespace no mesmo host
- Fan-out paralelo cego pode estourar RAM porque cada subprocesso de `recall` spawna um subprocesso LLM


### Solution
```bash
for ns in project-a project-b project-c project-d; do
  SQLITE_GRAPHRAG_NAMESPACE="$ns" \
    sqlite-graphrag --max-concurrency 1 recall "error rate" --k 5 --json
done
```


### Explanation
- O loop permanece serial de forma intencional porque `recall` é comando pesado de embedding
- `--max-concurrency 1` evita oversubscription local durante auditorias, CI e uso em desktop
- Env var `SQLITE_GRAPHRAG_NAMESPACE` escopa cada subprocesso ao seu próprio projeto limpo
- Um documento JSON por namespace ainda cai no stdout para um agregador downstream fundir ranks
- Esse padrão prioriza segurança do host e progresso determinístico em vez de redução agressiva de wall-clock


### Variants
- Reserve fan-out paralelo para comandos leves como `stats` ou `list`, não para `recall`
- Só aumente concorrência de comandos pesados depois de medir RSS, observar swap e confirmar que o host permanece estável


### See Also
- Receita "Como Combinar Busca Vetorial E FTS Com Pesos Ajustáveis"
- Receita "Como Fazer Benchmark De hybrid-search Contra recall Vetorial Puro"


## Como Tratar Exit Codes Em Pipelines Automatizados
### Problem
- Seu pipeline CI trata todo exit não-zero como fatal, matando operações retriáveis como exit 75 (slots esgotados)
- Debugar falhas de pipeline leva 30 minutos porque seu wrapper não distingue validação de conflitos de locking


### Solution
```bash
sqlite-graphrag remember --name "$NAME" --type project \
  --description "$DESC" --body-stdin < "$FILE"
rc=$?
case $rc in
  0)  echo "Success" ;;
  2)  echo "Bad CLI argument (invalid flag, bad timezone): fix usage" ;;
  9)  echo "Duplicata ou soft-deleted: use --force-merge para restaurar e atualizar" ;;
  3)  echo "Conflict: re-read and retry" ;;
  6)  echo "Payload too large: split body" ;;
  15) echo "Busy: widen --wait-lock" ;;
  75) echo "Slots full: wait, do NOT raise concurrency" ;;
  77) echo "RAM pressure: free memory first" ;;
  *)  echo "Fatal: rc=$rc" >&2; exit 1 ;;
esac
```


### Explanation
- 18 exit codes de 0 a 77 seguindo convenções sysexits.h para roteamento de erros parseável por máquina
- Exit 3 significa conflito de locking otimista: recarregue a memória com `read --json` e tente novamente
- Exit 13 significa falha parcial em lote: reprocesse apenas os itens falhos, NÃO o lote inteiro
- Exit 75 e 77 sinalizam pressão de recursos: NUNCA aumente concorrência após receber esses códigos
- Exit 15 significa banco ocupado: amplie `--wait-lock <ms>` para esperar mais antes de falhar
- Tabela completa de códigos: 0=sucesso, 1=validação, 2=erro-argumento-Clap, 9=duplicata-ou-soft-deleted, 3=conflito, 4=não-encontrado, 5=namespace, 6=payload, 10=database, 11=embedding, 12=sqlite-vec (histórico — removido na v1.0.76), 13=parcial, 14=I/O, 15=ocupado, 16=preflight (EX_CONFIG), 19=shutdown (SHUTDOWN_EXIT_CODE), 20=interno, 75=slots, 77=RAM


### Variants
- Envolva o case em um loop de retry com backoff exponencial para códigos 3, 15, 75 e 77
- Logue `stderr` separadamente: `2>error.log` captura mensagens legíveis enquanto `stdout` captura JSON


### See Also
- Receita "Como Orquestrar Recall Entre Namespaces Com Segurança"
- Receita "Como Editar Uma Memória Com Locking Otimista"


## Como Debugar Queries Lentas Com Health E Stats
### Problem
- Seu recall que retornava em 8 ms agora leva 400 ms depois de meses de escrita
- Você não enxerga qual tabela inchou ou qual índice ficou stale ao longo do tempo


### Solution
```bash
sqlite-graphrag health --json | jaq '{integrity, wal_size_mb, journal_mode}'
sqlite-graphrag stats --json | jaq '{memories, entities, edges, avg_body_len}'
SQLITE_GRAPHRAG_LOG_LEVEL=debug sqlite-graphrag recall "slow query" --k 5 --json
sqlite-graphrag optimize --json
sqlite-graphrag debug-schema --json | jaq '{schema_version, objects: (.objects | length)}'
```


### Explanation
- `health` reporta `integrity`, tamanho WAL e `journal_mode` para detectar fragmentação rápido
- `stats` conta linhas revelando qual tabela cresceu desproporcionalmente desde a última auditoria
- `SQLITE_GRAPHRAG_LOG_LEVEL=debug` emite tempos por estágio SQLite em stderr para tracing
- Comparar `avg_body_len` atual ao baseline mostra se os bodies cresceram além dos defaults
- `optimize` atualiza estatísticas do query planner para que o próximo recall ou hybrid-search use índices atualizados
- `debug-schema` é um comando oculto que despeja versão do schema, contagem de objetos e histórico de migrações para troubleshooting de drift
- Poupa horas de tuning às cegas expondo o caminho lento exato em três comandos


### Variants
- Agende um painel que raspa `stats --json` toda hora e alerta em picos de crescimento
- Rode `optimize` seguido de `vacuum` quando o WAL passa de 100 MB para reclamar performance


### See Also
- Receita "Como Agendar Purge E Vacuum Com Cron, systemd.timer Ou launchd"
- Receita "Como Fazer Benchmark De hybrid-search Contra recall Vetorial Puro"


## Como Verificar Saúde Dos Embeddings (v1.0.76)
### Problem
- Você precisa confirmar que o pipeline de embedding LLM está funcionando após um upgrade
- Você quer verificar a integridade do banco antes de rodar um ingest em lote grande


### Solution
```bash
sqlite-graphrag health --json | jaq '{integrity_ok, fts_ok, fts_query_ok, vec_memories_ok}'
sqlite-graphrag stats --json | jaq '{memories, entities, relationships}'
```


### Explanation
- O subcomando `cache` foi removido na v1.0.76; todo embedding é gerenciado pelo subprocesso LLM
- Use `health --json` para verificar integridade do banco e status do índice FTS5
- `stats` fornece contagens globais para monitoramento de capacidade
- Nenhum arquivo de modelo local a gerenciar; o subprocesso LLM cuida do carregamento internamente


### See Also
- Receita "Como Debugar Queries Lentas Com Health E Stats"
- Receita "Como Agendar Purge E Vacuum Com Cron, systemd.timer Ou launchd"


## Como Fazer Benchmark De hybrid-search Contra recall Vetorial Puro
### Problem
- Você não tem dados para justificar habilitar hybrid search em produção contra vetor puro
- Seus stakeholders querem evidência numérica antes de aprovar o overhead de índice


### Solution
```bash
hyperfine --warmup 3 \
  'sqlite-graphrag recall "postgres migration" --k 10 --json > /dev/null' \
  'sqlite-graphrag hybrid-search "postgres migration" --k 10 --json > /dev/null'
```


### Explanation
- `hyperfine` mede ambos comandos com runs de warmup removendo ruído de cache frio
- Saída reporta latência média desvio padrão e speedup relativo em uma tabela limpa
- Resultados permitem comparar qualidade de recall contra latência em workload real
- Evidência numérica empodera conversas de tradeoff com stakeholders de produto e finanças
- Poupa semanas de debate ancorando a decisão em dados em vez de intuição


### Variants
- Troque a query única por 100 queries amostradas para computar p50 p95 p99 de latência
- Integre `hyperfine --export-json` em CI para detectar regressões entre pull requests


### See Also
- Receita "Como Combinar Busca Vetorial E FTS Com Pesos Ajustáveis"
- Receita "Como Orquestrar Recall Paralelo Entre Namespaces"


## Como Integrar Com rig-core Para Memória De Agente
### Problem
- Seu agente `rig-core` perde contexto entre invocações sem armazenamento persistente
- Reconstruir embeddings a cada execução desperdiça 50 minutos de compute e budget de API por semana

### Solution
```rust
use std::process::Command;
use serde_json::Value;

fn lembrar_contexto_agente(namespace: &str, conteudo: &str) -> anyhow::Result<()> {
    let name = format!(
        "rig-context-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    );
    let status = Command::new("sqlite-graphrag")
        .args([
            "remember",
            "--namespace", namespace,
            "--name", &name,
            "--type", "project",
            "--description", "contexto do agente rig-core",
            "--body", conteudo,
        ])
        .status()?;
    anyhow::ensure!(status.success(), "sqlite-graphrag remember falhou");
    Ok(())
}

fn recuperar_contexto_agente(namespace: &str, consulta: &str, k: u8) -> anyhow::Result<Vec<String>> {
    let output = Command::new("sqlite-graphrag")
        .args(["recall", "--namespace", namespace, "--k", &k.to_string(), "--json", consulta])
        .output()?;
    anyhow::ensure!(output.status.success(), "sqlite-graphrag recall falhou");
    let parsed: Value = serde_json::from_slice(&output.stdout)?;
    let itens = parsed["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v["snippet"].as_str().map(str::to_owned))
        .collect();
    Ok(itens)
}
```

### Explanation
- `Command::new("sqlite-graphrag")` executa o binário de 15 MB sem custo de FFI
- `--namespace` isola a memória do agente rig prevenindo contaminação entre agentes
- `--json` retorna saída estruturada que `serde_json` parseia sem regex frágil
- `anyhow::ensure!` converte falhas de exit-code em erros tipados que o agente trata
- Reduz 50 minutos de reconstrução de contexto por execução para uma chamada CLI de 5 milissegundos

### Variants
- Substitua `Command` por `tokio::process::Command` para pipelines async sem bloqueio
- Envolva as duas funções em um struct `RigMemoryAdapter` que implementa um trait `MemoryStore`

### See Also
- Receita "Como Inicializar Banco De Dados De Memória Em 60 Segundos"
- Receita "Como Executar Ollama Offline Com ollama-rs E Memória Persistente"


## Como Integrar Com swarms-rs Para Memória Multi-Agente
### Problem
- Seu swarm de agentes sobrescreve memórias uns dos outros ao compartilhar um namespace
- Depurar qual agente escreveu o quê leva horas de grep em arquivos de log não estruturados

### Solution
```rust
use std::process::Command;

fn swarm_lembrar(agent_id: &str, conteudo: &str) -> anyhow::Result<()> {
    let namespace = format!("swarm-{agent_id}");
    let name = format!(
        "swarm-note-{agent_id}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    );
    let status = Command::new("sqlite-graphrag")
        .args([
            "remember",
            "--namespace", &namespace,
            "--name", &name,
            "--type", "project",
            "--description", "nota do agente swarm",
            "--body", conteudo,
        ])
        .status()?;
    anyhow::ensure!(status.success(), "swarm remember falhou para agent {agent_id}");
    Ok(())
}

fn swarm_recuperar_todos(agent_ids: &[&str], consulta: &str) -> anyhow::Result<Vec<(String, String)>> {
    let mut resultados = Vec::new();
    for agent_id in agent_ids {
        let namespace = format!("swarm-{agent_id}");
        let output = Command::new("sqlite-graphrag")
            .args(["recall", "--namespace", &namespace, "--k", "5", "--json", consulta])
            .output()?;
        if output.status.success() {
            let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            if let Some(itens) = parsed["results"].as_array() {
                for item in itens {
                    if let Some(snippet) = item["snippet"].as_str() {
                        resultados.push((agent_id.to_string(), snippet.to_owned()));
                    }
                }
            }
        }
    }
    Ok(resultados)
}
```

### Explanation
- Namespace por agente `swarm-{agent_id}` isola memórias sem alterações de schema
- Um único arquivo SQLite hospeda todos os namespaces eliminando múltiplos bancos
- Iterar namespaces no coordenador coleta resultados ranqueados de cada membro do swarm
- Saída JSON estruturada com `serde_json` torna atribuição trivial versus logs de texto puro
- Reduz tempo de depuração multi-agente de horas para minutos tornando autoria explícita

### Variants
- Use `tokio::task::JoinSet` para recuperar todos os namespaces concorrentemente em swarms async
- Adicione um namespace `coordinator` onde o orquestrador grava decisões sintetizadas do swarm

### See Also
- Receita "Como Orquestrar Recall Paralelo Entre Namespaces"
- Receita "Como Integrar Com rig-core Para Memória De Agente"


## Como Usar genai Com sqlite-graphrag Para Memória Universal De LLM
### Problem
- Trocar provedores de LLM via `genai` reseta a memória do agente porque embeddings diferem por vendor
- Seu time perde 40 minutos por migração de provedor reconstruindo índices de busca semântica

### Solution
```rust
use std::process::Command;

async fn armazenar_turno_llm(
    namespace: &str,
    role: &str,
    conteudo: &str,
) -> anyhow::Result<()> {
    let entrada = format!("[{role}] {conteudo}");
    let name = format!(
        "llm-turn-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    );
    let status = Command::new("sqlite-graphrag")
        .args([
            "remember",
            "--namespace", namespace,
            "--name", &name,
            "--type", "project",
            "--description", "turno de conversa LLM",
            "--body", &entrada,
        ])
        .status()?;
    anyhow::ensure!(status.success(), "falhou ao persistir turno LLM");
    Ok(())
}

async fn recuperar_contexto_relevante(
    namespace: &str,
    consulta_usuario: &str,
    k: u8,
) -> anyhow::Result<String> {
    let output = Command::new("sqlite-graphrag")
        .args([
            "hybrid-search",
            "--namespace", namespace,
            "--k", &k.to_string(),
            "--json",
            consulta_usuario,
        ])
        .output()?;
    anyhow::ensure!(output.status.success(), "hybrid-search falhou");
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let contexto = parsed["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v["body"].as_str())
        .collect::<Vec<_>>()
        .join("\n---\n");
    Ok(contexto)
}
```

### Explanation
- sqlite-graphrag armazena embeddings via API REST do OpenRouter independente de outro provedor LLM
- Trocar de OpenAI para Mistral via `genai` não invalida entradas de memória existentes
- `hybrid-search` combina similaridade vetorial e FTS dando contexto mais rico que vetor puro
- Formatar turnos como `[role] conteudo` preserva estrutura de conversa no body da memória
- Elimina 40 minutos de reconstrução de índice por migração com uma camada agnóstica a provedor

### Variants
- Injete contexto recuperado como system message antes de cada request `genai::chat` automaticamente
- Armazene nome do modelo e temperatura junto ao body do turno para auditar qual modelo gerou cada resposta

### See Also
- Receita "Como Combinar Busca Vetorial E FTS Com Pesos Ajustáveis"
- Receita "Como Cascatear Com llm-cascade E Fallback De Memória"


## Como Cascatear Com llm-cascade E Fallback De Memória
### Problem
- Seu pipeline LLM em cascata perde tentativas anteriores quando um provedor falha e reexecuta
- Rederetear chamadas falhas sem contexto faz o modelo de fallback repetir erros custosos

### Solution
```rust
use std::process::Command;

fn persistir_tentativa_cascade(
    namespace: &str,
    provider: &str,
    prompt: &str,
    resultado: &str,
    sucesso: bool,
) -> anyhow::Result<()> {
    let rotulo = if sucesso { "SUCCESS" } else { "FAILURE" };
    let entrada = format!("[CASCADE:{rotulo}:{provider}] prompt={prompt} resultado={resultado}");
    let name = format!(
        "cascade-attempt-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    );
    let status = Command::new("sqlite-graphrag")
        .args([
            "remember",
            "--namespace", namespace,
            "--name", &name,
            "--type", "project",
            "--description", "log de tentativa llm-cascade",
            "--body", &entrada,
        ])
        .status()?;
    anyhow::ensure!(status.success(), "falhou ao persistir tentativa cascade");
    Ok(())
}

fn carregar_historico_cascade(namespace: &str, prompt: &str) -> anyhow::Result<String> {
    let output = Command::new("sqlite-graphrag")
        .args([
            "recall",
            "--namespace", namespace,
            "--k", "10",
            "--json",
            prompt,
        ])
        .output()?;
    anyhow::ensure!(output.status.success(), "recall falhou para histórico cascade");
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let historico = parsed["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v["snippet"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(historico)
}
```

### Explanation
- Rotular entradas com `CASCADE:SUCCESS:provider` permite ao fallback pular provedores já falhos
- Recuperar histórico antes de cada tentativa revela quais modelos já tentaram o mesmo prompt
- Um namespace por execução de pipeline garante isolamento sem gerenciar múltiplos bancos
- Rótulos estruturados parseiam com `str::contains` simples evitando overhead JSON na consulta
- Economiza falhas repetidas custosas dando ao fallback consciência plena do estado cascade anterior

### Variants
- Crie um struct `CascadeMemory` que chama `persistir` e `carregar` automaticamente em cada tentativa
- Filtre entradas `FAILURE` na seleção de fallback para pular provedores comprovadamente falhos

### See Also
- Receita "Como Usar genai Com sqlite-graphrag Para Memória Universal De LLM"
- Receita "Como Integrar Com rig-core Para Memória De Agente"


## Como Executar Ollama Offline Com ollama-rs E Memória Persistente
### Problem
- Seu agente `ollama-rs` offline perde todo o contexto de conversa quando o processo reinicia
- Ambientes air-gapped não podem usar vector stores em nuvem então cada sessão começa do zero

### Solution
```rust
use std::process::Command;

fn lembrar_offline(conteudo: &str) -> anyhow::Result<()> {
    let name = format!(
        "ollama-turn-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    );
    let status = Command::new("sqlite-graphrag")
        .args([
            "remember",
            "--namespace", "ollama-local",
            "--name", &name,
            "--type", "project",
            "--description", "contexto offline do ollama",
            "--body", conteudo,
        ])
        .status()?;
    anyhow::ensure!(status.success(), "lembrar offline falhou: exit code não zero");
    Ok(())
}

fn recuperar_offline(consulta: &str, k: u8) -> anyhow::Result<Vec<String>> {
    let output = Command::new("sqlite-graphrag")
        .args([
            "recall",
            "--namespace", "ollama-local",
            "--k", &k.to_string(),
            "--json",
            consulta,
        ])
        .output()?;
    anyhow::ensure!(output.status.success(), "recuperar offline falhou");
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let itens = parsed["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v["snippet"].as_str().map(str::to_owned))
        .collect();
    Ok(itens)
}

fn construir_prompt_com_contexto(consulta: &str, memorias: &[String]) -> String {
    let contexto = memorias.join("\n---\n");
    format!("Contexto relevante da memória:\n{contexto}\n\nConsulta do usuário: {consulta}")
}
```

### Explanation
- sqlite-graphrag delega embedding ao subprocesso LLM; a binária CLI tem ~14.6 MiB sem modelo local embutido
- A binária grava em um arquivo SQLite local que sobrevive a reinicializações do processo
- `--namespace ollama-local` mantém memórias offline isoladas de namespaces de agentes em rede
- `construir_prompt_com_contexto` injeta memórias recuperadas no prompt Ollama antes de cada inferência
- Entrega memória vetorial persistente em ambientes totalmente air-gapped sem dependências de nuvem

### Variants
- Encadeie `recuperar_offline` com `sqlite-graphrag link` para construir grafo de conhecimento das saídas Ollama
- Chame `sqlite-graphrag vacuum` periodicamente para recuperar espaço SQLite conforme o banco offline cresce

### See Also
- Receita "Como Inicializar Banco De Dados De Memória Em 60 Segundos"
- Receita "Como Integrar Com rig-core Para Memória De Agente"


## Como Exibir Timestamps no Fuso Horário Local
### Problem
- Saída JSON de todos os subcomandos inclui campos `*_iso` em UTC por padrão
- Agentes rodando em região específica querem timestamps localizados para log e exibição
- Pipelines que leem `created_at_iso` precisam de strings com offset para ordenação correta

### Solution
```bash
# Flag pontual: exibir timestamps no fuso horário de São Paulo
sqlite-graphrag read --name minha-nota --tz America/Sao_Paulo

# Variável de ambiente persistente: todos os comandos da sessão usam o fuso configurado
export SQLITE_GRAPHRAG_DISPLAY_TZ=America/Sao_Paulo
sqlite-graphrag list --json | jaq '.items[].updated_at_iso'   # ou .memories[] (alias v1.0.66)

# Pipeline CI: forçar UTC explicitamente para evitar surpresas de fuso do sistema
SQLITE_GRAPHRAG_DISPLAY_TZ=UTC sqlite-graphrag recall "notas de deploy" --json

# Extrair apenas a parte do offset para verificar que o fuso foi aplicado
sqlite-graphrag read --name plano-deploy --tz Europe/Berlin --json \
  | jaq -r '.created_at_iso' \
  | rg '\+\d{2}:\d{2}$'
```

### Explanation
- Flag `--tz <IANA>` sobrescreve todas as configurações e aplica o fuso IANA informado
- Variável `SQLITE_GRAPHRAG_DISPLAY_TZ` mantém a configuração entre invocações sem a flag
- Ambos caem para UTC quando ausentes, garantindo saída determinística retrocompatível
- Apenas campos string terminando em `_iso` são afetados; campos inteiros permanecem epoch Unix
- Nomes IANA inválidos causam exit 2 com mensagem de erro `Validation` no stderr
- Formato produzido: `2026-04-19T04:00:00-03:00` (offset explícito, sem sufixo `Z`)

### Variants
- Use `America/New_York` para Eastern Time (UTC-5/UTC-4 dependendo do horário de verão)
- Use `Asia/Tokyo` para Japan Standard Time (UTC+9, sem horário de verão)
- Use `Europe/Berlin` para Central European Time (UTC+1/UTC+2 dependendo do horário de verão)
- Use `UTC` para resetar ao padrão explicitamente em ambientes com variável de ambiente conflitante
- Use `--lang pt` para forçar mensagens stderr legíveis em português; stdout JSON permanece independente de idioma

### See Also
- Receita "Como Inicializar Banco De Dados De Memória Em 60 Segundos"
- Receita "Como Configurar Saída de Idioma Com a Flag --lang"


## Como Fazer Round-Trip De Forget E Restore Em Uma Memória
### Problema
- Você rodou `forget --name decisao-importante` e agora `recall` não retorna nada
- Ler SQL de `memory_versions` para recuperar a linha não faz parte do seu trabalho
- v1.0.21 deixava `history` rejeitando memórias esquecidas e `restore` exigindo `--version`


### Solução
```bash
sqlite-graphrag forget --name decisao-importante
sqlite-graphrag history --name decisao-importante --json | jaq '.deleted'
sqlite-graphrag restore --name decisao-importante
sqlite-graphrag recall "decisão" --json
```


### Explicação
- `history` em v1.0.22 retorna versões de memórias soft-deletadas com flag `deleted: true`
- `restore` sem `--version` escolhe automaticamente a última versão não-`restore`
- Juntos tornam `forget` reversível ponta-a-ponta sem inspecionar SQL
- `vec_memories` é re-embeddado no restore para que recall vetorial volte a encontrar a memória
- Round-trip é idempotente: esquecer uma memória já esquecida é um no-op


### Variantes
- Passe `--version N` explicitamente quando precisar voltar a uma edição específica
- Combine com `list --include-deleted --json | jaq '.items[] | select(.deleted)'` para auditar todas as esquecidas
- Pipe `history --json` para detectar estado esquecido programaticamente antes de restaurar


### Veja Também
- Receita "Como Agendar Purge E Vacuum Com Cron, systemd.timer Ou launchd"
- Receita "Como Exportar Memórias Para NDJSON Para Backup"


## Como Editar Uma Memória Com Locking Otimista
### Problem
- Dois agentes editando a mesma memória simultaneamente causa corrupção silenciosa de last-write-wins
- Sem detecção de conflito, seu pipeline sobrescreve mudanças de um colega sem aviso


### Solution
```bash
UPDATED=$(sqlite-graphrag read --name design-auth --json | jaq -r '.updated_at')
sqlite-graphrag edit --name design-auth \
  --body-file ./revised.md \
  --expected-updated-at "$UPDATED"
```


### Explanation
- Cada `edit` cria uma nova versão imutável preservando o histórico completo de edições anteriores
- `--expected-updated-at` habilita locking otimista; exit code 3 sinaliza modificação concorrente
- No exit code 3, releia a memória com `read --json` para obter o novo `updated_at` e tente novamente
- `--body-file` lê o novo corpo de um arquivo; alternativas são `--body` (inline) e `--body-stdin` (pipe)
- Altere apenas a descrição sem tocar o corpo: `edit --name <nome> --description "nova desc"`
- Altere o tipo da memória sem recriar: `edit --name <nome> --type decision` (pula re-embedding quando body não mudou)
- JSON response inclui `memory_id`, `name`, `action` ("updated"), `version` e `elapsed_ms`


### Variants
- Use `--body-stdin` para canalizar o corpo de outro comando: `cat revised.md | sqlite-graphrag edit --name design-auth --body-stdin`
- Omita `--expected-updated-at` quando escritas concorrentes são impossíveis (pipelines de agente único)


### See Also
- Receita "Como Fazer Round-Trip De Forget E Restore Em Uma Memória"
- Receita "Como Renomear Uma Memória Preservando Todo O Histórico"


## Como Renomear Uma Memória Preservando Todo O Histórico
### Problem
- Sua equipe renomeou o projeto de `auth-v1` para `authentication-flow` mas todos links do grafo ainda apontam para o nome antigo
- Delete-e-recrie manual perde histórico de versões e quebra auditorias de compliance


### Solution
```bash
sqlite-graphrag rename auth-v1 authentication-flow
sqlite-graphrag history --name authentication-flow --json | jaq '.versions | length'
```


### Explanation
- Argumentos posicionais `rename <antigo> <novo>` são suportados desde v1.0.44
- Todas versões e conexões de grafo transferem para o novo nome automaticamente
- `--from`/`--to` e `--name`/`--new-name` são aceitos como aliases de flag desde v1.0.35
- Exit code 4 sinaliza que a memória de origem não existe no namespace atual
- JSON response inclui `memory_id`, `name` (novo), `action` ("renamed"), `version` e `elapsed_ms`


### Variants
- Aplique locking otimista: `--expected-updated-at` previne renomear uma memória que mudou desde sua última leitura
- Verifique preservação do histórico: `history --name <novo> --json | jaq '.versions[].created_at_iso'`


### See Also
- Receita "Como Editar Uma Memória Com Locking Otimista"
- Receita "Como Fazer Round-Trip De Forget E Restore Em Uma Memória"


## Como Importar Corpora Grandes Em Hosts Com Memória Limitada
### Problem
- Seu pipeline de ingestão de 5000 arquivos pressiona um host com memória restrita porque cada worker de embedding LLM segura ~350 MB de RSS
- HISTÓRICO (≤ v1.0.75): o GLiNER NER rodava em cada corpo e seu modelo ONNX (1,1 GB fp32, 349 MB int8) estourava o orçamento de memória do CI; o pipeline foi removido na v1.0.79


### Solution
```bash
sqlite-graphrag ingest ./big-corpus --recursive \
  --low-memory --max-files 50000 --json \
  | jaq -c 'select(.summary) | {files_total, files_succeeded, elapsed_ms}'
```


### Explanation
- Extração automática (`--enable-ner`) é somente URL-regex desde a v1.0.79 e tem custo desprezível; o download do modelo GLiNER não existe mais
- Use `--llm-parallelism 1` para limitar os workers de embedding a um subprocesso (~350 MB de RSS) em hosts com memória restrita
- `--low-memory` força `--ingest-parallelism 1`, reduzindo RSS em aproximadamente 40 por cento para hosts restritos
- `--max-files 50000` eleva o cap de segurança do padrão 10000; a operação é rejeitada inteiramente se contagem de arquivos exceder o cap
- Dois eixos de paralelismo existem: `--max-concurrency` controla invocações CLI, `--ingest-parallelism` controla threads de extract+embed
- Trade-off é 3 a 4 vezes mais tempo de wall-clock para footprint de memória significativamente menor
- Linha resumo NDJSON reporta `files_total`, `files_succeeded`, `files_failed` e `elapsed_ms` para auditoria de pipeline
- Use `--max-rss-mb 2048` (ou similar) para abortar embedding quando o RSS do processo exceder o limite; fornece controle granular além de `--low-memory` em containers com caps de memória rígidos


### Variants
- Defina `SQLITE_GRAPHRAG_LOW_MEMORY=1` como env var persistente em vez de passar `--low-memory` por invocação
- Combine com chamadas separadas de `remember --entities-file` para grafos curados em documentos críticos


### See Also
- Receita "Como Importar Em Massa Um Diretório De Base De Conhecimento"
- Receita "Como Tratar Exit Codes Em Pipelines Automatizados"


## Como Remover Relacionamentos Em Massa Por Tipo
### Receita — Limpar arestas de baixo sinal geradas por NER
- Use `prune-relations` para remover tipos de relação de baixo valor em massa
- Visualize com `--dry-run` antes de confirmar
- Limpe entidades órfãs em seguida

#### Passo 1 — Auditar distribuição de relacionamentos
```bash
sqlite-graphrag graph stats --json | jaq '{nodes: .node_count, edges: .edge_count}'
```

#### Passo 2 — Dry-run do prune
```bash
sqlite-graphrag prune-relations --relation mentions --dry-run --json
```

#### Passo 3 — Executar o prune
```bash
sqlite-graphrag prune-relations --relation mentions --yes --json
```

#### Passo 4 — Limpar entidades órfãs
```bash
sqlite-graphrag cleanup-orphans --dry-run --json
sqlite-graphrag cleanup-orphans --yes --json
```

#### Passo 5 — Verificar saúde do grafo
```bash
sqlite-graphrag graph stats --json | jaq '{nodes: .node_count, edges: .edge_count}'
sqlite-graphrag health --json | jaq '.integrity_ok'
```


## Como Reparar Um Índice FTS5 Corrompido
### Receita: Reparar Índice FTS5 Corrompido
- Problema: `hybrid-search` retorna exit 10 ou resultados vazios mesmo havendo memórias
- Solução: `sqlite-graphrag fts rebuild --json`
- Explicação: Reconstrói a B-tree do FTS5 a partir da tabela de memórias sem perda de dados
- Verificação: `sqlite-graphrag fts check --json | jaq '.integrity_ok'`

#### Passo 1 — Verificar saúde do FTS5
```bash
sqlite-graphrag health --json | jaq '.fts_query_ok'
```

#### Passo 2 — Reconstruir o índice FTS5
```bash
sqlite-graphrag fts rebuild --json
```

#### Passo 3 — Confirmar integridade
```bash
sqlite-graphrag fts check --json | jaq '.integrity_ok'
```

#### Passo 4 — Inspecionar estatísticas
```bash
sqlite-graphrag fts stats --json
```


## Como Criar Um Backup Seguro do Banco de Dados
### Receita: Backup Seguro do Banco com WAL
- Problema: Precisa de backup consistente enquanto o banco está em uso
- Solução: `sqlite-graphrag backup --output /caminho/para/backup.sqlite --json`
- Explicação: Usa a API SQLite Online Backup, segura com WAL e leitores concorrentes

#### Passo 1 — Executar o backup
```bash
sqlite-graphrag backup --output backup.sqlite --json
```

#### Passo 2 — Verificar o backup
```bash
sqlite-graphrag health --db backup.sqlite --json | jaq '.integrity_ok'
```


## Como Validar Entrada do Remember Antes de Persistir
### Receita: Validar Entrada do Remember Antes de Persistir
- Problema: Quer visualizar o que o `remember` fará sem gravar no banco
- Solução: Passe `--dry-run` para inspecionar o parsing e a extração do grafo sem confirmar

#### Passo 1 — Dry-run de uma chamada remember
```bash
sqlite-graphrag remember --name test --type note --description "desc" --body "content" --dry-run --json
```

#### Passo 2 — Inspecionar a saída de preview
```bash
# Confirme entidades e relacionamentos parseados corretamente antes de persistir
sqlite-graphrag remember --name test --type note --description "desc" --body-file ./content.md --dry-run --json | jaq '{name, type, entity_count: (.entities | length)}'
```


## Como Limpar Entidades NER de Baixa Qualidade
### Receita: Limpar Entidades NER Ruidosas
- Problema: a auto-extração da era GLiNER (≤ v1.0.75) criou entidades demais com baixa qualidade ou espúrias
- Solução: `sqlite-graphrag prune-ner --entity noisy-entity --json` ou `--all --yes --json`
- Pós-limpeza: `sqlite-graphrag cleanup-orphans --yes --json`

#### Passo 1 — Auditar entidades criadas por NER
```bash
sqlite-graphrag graph entities --json | jaq -r '.entities[] | select(.source == "ner") | .name'
```

#### Passo 2 — Remover vínculos NER de uma entidade específica
```bash
sqlite-graphrag prune-ner --entity noisy-entity --json
```

#### Passo 3 — Remover todos os vínculos NER em massa (com confirmação)
```bash
sqlite-graphrag prune-ner --all --yes --json
```

#### Passo 4 — Limpar entidades órfãs resultantes
```bash
sqlite-graphrag cleanup-orphans --dry-run --json
sqlite-graphrag cleanup-orphans --yes --json
```

#### Passo 5 — Verificar saúde do grafo
```bash
sqlite-graphrag health --json | jaq '.integrity_ok'
sqlite-graphrag graph stats --json | jaq '{nodes: .node_count, edges: .edge_count}'
```


## Como Renomear Uma Entidade (v1.0.58)
### Problema
- Uma entidade tem nome errado ou não-canônico (ex.: "auth" deveria ser "authentication")
- Renomear manualmente exige criar nova entidade, migrar todas as arestas e deletar a antiga
### Solução
```bash
sqlite-graphrag rename-entity --name auth --new-name authentication --json
```
### Explicação
- Renomeia a entidade preservando todos os relacionamentos e vínculos com memórias (usam FK inteiro)
- Re-gera o vetor com o novo nome para precisão na busca semântica
- Retorna `{action: "renamed", old_name, new_name, entity_id}` em sucesso
- Falha com exit 4 se entidade não existe, exit 1 se novo nome já existe

## Como Listar Memórias Vinculadas a Uma Entidade (v1.0.58)
### Problema
- Antes de renomear ou deletar uma entidade, é preciso saber quais memórias a referenciam
- O comando `memory-entities` existente só funciona memória→entidades, não o inverso
### Solução
```bash
sqlite-graphrag memory-entities --entity authentication --json
```
### Explicação
- Flag `--entity` faz busca reversa: entidade→memórias via tabela junction `memory_entities`
- Retorna `{entity_name, memories: [{name, description, memory_type}], count}` em sucesso
- Retorna apenas memórias ativas (soft-deleted são excluídas)

## Como Atualizar Descrição de Uma Entidade (v1.0.58)
### Problema
- 88% das entidades têm descrição NULL ou vazia (vindas de extração automática)
- Não existia comando para atualizar descrições de entidades
### Solução
```bash
sqlite-graphrag reclassify --name rust-lang --description "The Rust programming language" --json
```
### Explicação
- Flag `--description` no `reclassify` atualiza a descrição da entidade no modo individual
- Pode ser combinada com `--new-type` para alterar tipo e descrição em uma operação
- Modo batch (`--batch`) ignora `--description`

## Como Deletar Uma Entidade e Seus Relacionamentos (v1.0.56)
### Problema
- Uma entidade foi criada por engano ou está obsoleta
- Removê-la manualmente exige deletar relacionamentos, bindings de memória e a linha da entidade
### Solução
```bash
sqlite-graphrag delete-entity --name conceito-obsoleto --cascade --json
```
### Explicação
- `--cascade` remove a entidade, todos seus relacionamentos e todos os bindings memory_entities em uma operação atômica
- Sem `--cascade` o comando recusa deletar entidade que ainda possui relacionamentos (exit 1)
- Execute `cleanup-orphans --dry-run --json` depois para auditar entidades recém-órfãs

## Como Mesclar Entidades Duplicadas (v1.0.56)
### Problema
- O mesmo conceito existe com múltiplos nomes (ex.: `jwt-auth` e `jwt-authentication`)
- Relacionamentos estão divididos entre duplicatas, enfraquecendo a travessia de grafo
### Solução
```bash
sqlite-graphrag merge-entities --names "jwt-authentication,jwt-tokens" --into jwt-auth --json
```
### Explicação
- Todos os relacionamentos das entidades de origem são redirecionados para a entidade alvo
- Relacionamentos duplicados após redirecionamento são removidos automaticamente (UPDATE OR IGNORE)
- Entidades de origem são deletadas após a mesclagem
- A entidade alvo deve existir previamente (exit 4 se não encontrada)
- Use `memory-entities --entity jwt-auth --json` depois para verificar os bindings consolidados

## Como Ingestar Documentos e Depois Extrair o Grafo
### Problema
- O `ingest` padrão cria memórias body-only com zero entidades e zero relacionamentos
- A NER da era GLiNER (≤ v1.0.75) produzia entidades ruidosas (genéricos ALL_CAPS, stop words) e relacionamentos `mentions` de baixa qualidade; limpar esse legado ainda é relevante em bancos antigos
- `remember --graph-stdin` manual por arquivo é lento para conjuntos grandes de documentos
### Solução
- Ingestão e extração são duas invocações SEPARADAS. `ingest --mode none` grava corpos e embeddings; um passo distinto de `enrich` extrai o grafo
- Nunca encadeie os dois com `&&` — aguarde exit 0 da escrita antes de iniciar o enrich
### Receita
```bash
# Passo 1 — corpos + embeddings
sqlite-graphrag ingest ./docs --mode none --recursive --pattern "*.md" \
  --type document --json

# Passo 2 — extração do grafo, processo SEPARADO, só após o passo 1 sair com exit 0
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model MODELO \
  --until-empty --max-runtime 3600 --rest-concurrency 8 --json
```
### Explicação
- `ingest --mode` aceita somente `none`; os modos curados por LLM foram removidos e o clap os rejeita com exit 2
- As flags de fila que pertenciam a esses modos (`--resume`, `--retry-failed`, `--keep-queue`) deixaram de existir
- O `enrich` mantém sua própria fila sidecar, então `--until-empty` converge sem loop externo de retry
- Monitore com `enrich --status --json` até `scan_backlog`, `queue_pending` e `eligible_now` chegarem a 0

## Como Reclassificar Tipos de Relacionamento em Massa (v1.0.65)

### Problema
- Seu grafo tem centenas de relacionamentos `mentions` ou `applies_to` que deveriam ser mais precisamente tipados
- Remover e recriar manualmente cada aresta é impraticável em escala

### Solução
```bash
sqlite-graphrag reclassify-relation --from-relation mentions --to-relation related --batch --dry-run --json
sqlite-graphrag reclassify-relation --from-relation mentions --to-relation related --batch --json
```

### Explicação
- `--dry-run` faz preview de quantas arestas mudariam sem modificar o banco
- `--batch` reclassifica todas as arestas do tipo especificado no namespace
- Trata colisões UNIQUE automaticamente: se (source, target, nova_relação) já existe, a aresta antiga é mesclada e deletada
- Use `--filter-source-type incident --filter-target-type tool` para restringir escopo a pares de tipos específicos

### Variantes
- Aresta individual: `sqlite-graphrag reclassify-relation --source entity-a --target entity-b --from-relation applies-to --to-relation uses --json`
- Batch direcionado: `sqlite-graphrag reclassify-relation --from-relation applies-to --to-relation depends-on --filter-source-type concept --batch --json`


## Como Normalizar Nomes de Entidade para Kebab-Case (v1.0.65)

### Problema
- Seu grafo tem entidades duplicadas como `Claude Code` e `claude-code` ou `CANONICAL_RELATIONS` e `canonical-relations`
- Travessia do grafo as trata como nós separados, dividindo relacionamentos e reduzindo recall

### Solução
```bash
sqlite-graphrag normalize-entities --dry-run --json
sqlite-graphrag normalize-entities --yes --json
```

### Explicação
- `--dry-run` mostra quais entidades seriam renomeadas ou mescladas sem tocar no banco
- Pipeline de normalização: decomposição NFKD, filtragem ASCII, minúsculas, espaços e underscores para hífens, colapso de hífens consecutivos
- Quando normalização cria colisão (ex.: `Claude Code` e `claude-code` existem), mescla automaticamente todos os relacionamentos no alvo e deleta a entidade de origem
- Após normalização, todos os nomes são kebab-case minúsculo idempotente

### Notas
- Nomes de entidade também são normalizados em todo path de escrita desde v1.0.65 (remember, ingest, link, rename-entity)
- Execute `cleanup-orphans --json` após normalização para remover entidades recém-órfãs


## Como Enriquecer Memórias Órfãs com Entidades Extraídas por LLM (v1.0.65)

### Problema
- A maioria das memórias no banco não tem vínculos com entidades (memórias órfãs invisíveis à travessia do grafo)
- Extração manual via `remember --graph-stdin` é tediosa para centenas de memórias

### Solução
```bash
sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model MODELO --limit 50 --dry-run --json
sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model MODELO --limit 50 --json
```

### Explicação
- Varre memórias sem vínculos com entidades, envia cada body ao LLM para extração estruturada e persiste entidades e relacionamentos extraídos via `--force-merge`
- `--dry-run` faz preview de quais memórias seriam enriquecidas sem spawnar o LLM (zero tokens)
- `--limit 50` limita o tamanho do lote para controle de custo
- `--max-cost-usd 5.00` limita gasto acumulado da API
- Saída é NDJSON: eventos de fase, eventos por item e linha de resumo

### Variantes
- Gerar descrições de entidade: `sqlite-graphrag enrich --operation entity-descriptions --mode openrouter --openrouter-model MODELO --limit 100 --json`
- Expandir corpos curtos: `sqlite-graphrag enrich --operation body-enrich --mode openrouter --openrouter-model MODELO --limit 20 --json`
- Retomar após interrupção: `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model MODELO --resume --json`
- Enriquecer via OpenRouter REST sem CLI local: `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model <model> --json` — `--openrouter-model` é obrigatório (sem default; ausência sai com exit 1 antes de qualquer chamada de rede) e a chave vem de `config add-key` (OPENROUTER_API_KEY não é lida em runtime); o JUDGE roda sobre `/chat/completions` com `json_schema` strict, e `--openrouter-timeout` tem padrão de 300s (`--openrouter-base-url` opcional)
- Drenar o backlog até convergir (v1.0.96, sem loop externo): `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --until-empty --rest-concurrency 8 --json` — `--until-empty` escaneia e drena até não restarem itens elegíveis ou `--max-runtime` (padrão 3600s) expirar; a fila dead-letter (`error_class`/`next_retry_at`, terminal `dead` após `--max-attempts`, padrão 8) garante que o conjunto vivo decresce estritamente
- Inspecionar a fila sem rodar o LLM (v1.0.96): `sqlite-graphrag enrich --status --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json` — contagens read-only (`unbound_backlog`, `scan_backlog` por operação, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`); nunca spawna o LLM e nunca adquire o singleton, então é seguro fazer poll durante o drain; o `scan_backlog` (GAP-SG-77, v1.1.0) é o backlog real do banco por operação que um scan enfileiraria — elimina o falso `pending=0` para `entity-descriptions`/`body-enrich`/`re-embed`, e o `state` deriva o `pending-scan` dele; desde a v1.1.01, o `re-embed` reporta `scan_backlog` por `--target` (memories/entities/chunks)
- Rodar com workers LLM em paralelo: `sqlite-graphrag enrich --operation entity-descriptions --mode openrouter --openrouter-model MODELO --llm-parallelism 4 --json`


## Como Limpar Linhas de Dead-Letter Órfãs (v1.0.97)

### Problema
- A fila do enrich acumula linhas `dead` de memórias que foram renomeadas ou purgadas após o enfileiramento
- `--requeue-dead` apenas re-falha essas linhas; não remove as verdadeiramente órfãs cuja chave de memória não existe mais no banco principal

### Solução
```bash
sqlite-graphrag enrich --prune-dead-orphans --json

# v1.1.02 — contraparte para chaves de entidade (rode AMBAS para varredura completa)
sqlite-graphrag enrich --prune-dead-entity-orphans --json
```

### Explicação
- Deleta apenas linhas `status='dead' AND item_type='memory'` cujo `item_key` (nome da memória) está ausente da tabela principal de memórias — linhas de entidade são intocadas
- Read-only em relação ao banco principal: somente o sidecar `.enrich-queue.sqlite` é mutado
- Roda sem `--operation` nem `--mode`; sem LLM, sem singleton adquirido (GAP-SG-66, ADR-0058)
- Verifique `.summary.pruned` na saída JSON para a contagem de linhas removidas


## Como Criar Memórias em Lote a Partir de NDJSON (v1.0.67)

### Problema
- Você tem uma lista de memórias para criar e chamar `remember` em loop é lento e não-atômico
- Uma escrita parcial deixa o banco em estado inconsistente quando o loop é interrompido

### Solução
```bash
# Criar 3 memórias em lote a partir de NDJSON
printf '{"name":"nota-a","type":"note","description":"primeira","body":"conteudo a"}\n{"name":"nota-b","type":"note","description":"segunda","body":"conteudo b"}\n{"name":"nota-c","type":"note","description":"terceira","body":"conteudo c"}' | sqlite-graphrag remember-batch --json

# Lote atômico com transação
cat memorias.ndjson | sqlite-graphrag remember-batch --transaction --force-merge --json
```

### Explicação
- `remember-batch` lê um objeto JSON por linha do stdin e insere cada um como memória
- `--transaction` envolve todos os inserts em uma única transação SQLite: todos confirmam ou todos revertem
- `--force-merge` faz upsert de memórias existentes em vez de falhar com exit 9 em duplicatas
- Cada linha de entrada suporta os mesmos campos do `remember`: `name`, `type`, `description`, `body`
- Saída é NDJSON: uma linha de resultado por memória de entrada mais uma linha resumo com `summary: true`
- Mais rápido que loop no shell porque todas as memórias compartilham uma invocação CLI e conexão DB

### Variantes
- Canalizar saída do `jaq` diretamente: `jaq -c '.[]' memorias.json | sqlite-graphrag remember-batch --json`
- Omitir `--transaction` quando sucesso parcial é aceitável e você quer detalhes de erro por item

### Veja Também
- Receita "Como Importar Em Massa Um Diretório De Base De Conhecimento"
- Receita "Como Tratar Exit Codes Em Pipelines Automatizados"


## Como Buscar Uma Memória Por ID (v1.0.67)

### Problema
- Você tem um `memory_id` inteiro da resposta de `list` ou `remember` e quer fetch O(1) direto
- Buscar pelo nome exige conhecer o slug kebab-case exato, que nem sempre está disponível

### Solução
```bash
# Busca direta pelo memory_id (do list ou resposta do remember)
sqlite-graphrag read --id 42 --json
```

### Explicação
- `--id` aceita o inteiro `memory_id` retornado por `list`, `remember` e `remember-batch`
- A busca é O(1) por chave primária — mais rápida que `--name` quando você já tem o ID
- `--id` e `--name` são mutuamente exclusivos; passe exatamente um por invocação
- Todas as outras flags de `read` (`--tz`, `--json`, `--lang`) funcionam normalmente com `--id`

### Variantes
- Extrair o ID da resposta do `remember`: `sqlite-graphrag remember ... --json | jaq '.memory_id'`
- Combinar com `list`: `sqlite-graphrag list --json | jaq '.items[0].memory_id'`

### Veja Também
- Receita "Como Encadear Recuperação Profunda Em 3 Camadas"
- Receita "Como Editar Uma Memória Com Locking Otimista"


## Como Instalar Completions de Shell

### Problema
- Você digita subcomandos do `sqlite-graphrag` de memória e perde flags que poderiam economizar tempo
- Tab-completion não está disponível após instalar o binário

### Solução
```bash
# Completions para Bash
sqlite-graphrag completions bash > ~/.bash_completion.d/sqlite-graphrag
# Completions para Zsh
sqlite-graphrag completions zsh > ~/.zfunc/_sqlite-graphrag
# Completions para Fish
sqlite-graphrag completions fish > ~/.config/fish/completions/sqlite-graphrag.fish
```

### Explicação
- O subcomando `completions` gera scripts de completion para bash, zsh e fish
- Bash: faça source do arquivo ou coloque-o em diretório que o bash carrega automaticamente no startup
- Zsh: adicione `fpath=(~/.zfunc $fpath)` e `autoload -U compinit && compinit` ao `~/.zshrc`
- Fish: o arquivo de completions é carregado automaticamente do diretório de completions do fish
- Completions cobrem todos os subcomandos, flags e valores de enum (ex.: `--type`, `--entity-type`, `--format`)

### Variantes
- PowerShell: `sqlite-graphrag completions powershell > sqlite-graphrag.ps1` e faça dot-source
- Elvish: `sqlite-graphrag completions elvish > ~/.config/elvish/lib/sqlite-graphrag.elv`

### Veja Também
- Receita "Como Bootstrapar O Banco De Memória Em 60 Segundos"
- Receita "Como Integrar sqlite-graphrag Com Loop Subprocess Do Claude Code"


## Como Sobreviver a Sinais de SHUTDOWN Durante Jobs Longos de Embedding (v1.0.80, ADR-0034)

### Problema

O harness do agente (e qualquer orquestrador em background)
envia SIGINT para a CLI quando seu orçamento de wall-clock de
80 minutos expira. A auditoria G42 identificou que o handler
anterior em `src/signals.rs` disparava `SIGABRT` em
`BrokenPipe` quando o stderr do pai era um pipe fechado — o
cenário de processo órfão. A v1.0.80 corrige o abort mas NÃO
torna os jobs de embedding mais rápidos; para jobs mais longos
que o orçamento do harness você ainda precisa de um bypass.

### Solução

A receita de bypass SHUTDOWN em 3 camadas é a resposta canônica.
As 3 camadas são independentes e a receita compõe aditivamente:

```bash
# Camada 1 — PATH: roteia o subprocesso LLM via o mock-llm
export PATH="$PWD/tests/mock-llm:$PATH"

# Camada 2 — env: diz ao embedder para ignorar a checagem de SHUTDOWN
export SQLITE_GRAPHRAG_IGNORE_SHUTDOWN=1

# Camada 3 — grupo de processos: desanexa a CLI do pgroup do harness
setsid -w timeout 600 \
  sqlite-graphrag remember --graph-stdin < payload.json
```

### Explicação

- **Camada 1 (PATH)** roteia qualquer `claude -p` ou `codex exec`
  spawned via a mock CLI determinística commitada em
  `tests/mock-llm/`. O subprocesso LLM real é desviado; SIGINT
  não consegue matar um subprocesso que não existe. É a camada
  mais barata e o default certo em CI.
- **Camada 2 (env)** faz o `if should_obey_shutdown()` do
  embedder curto-circuitar para `true`, então o braço de
  cancelamento do `tokio::select!` é descartado e o batch roda
  até a conclusão mesmo se o cancellation token já estiver
  cancelled. Zero overhead em produção porque a leitura da env
  é um único `std::env::var` por chamada de `should_obey_shutdown()`,
  não em hot path.
- **Camada 3 (setsid)** dá à CLI seu próprio grupo de processos
  via `setsid -w`, então SIGINT do harness pai não se propaga
  para o filho. `timeout` adiciona um teto rígido de wall-clock
  (binário Rust `timeout-cli` v0.1.0, somente inteiros em segundos
  — `600` é 10 minutos; não passe `10m`).

### Variantes

- Para runners de CI: pule a Camada 3 e confie nas Camadas 1+2;
  o mock-llm torna o caminho do subprocesso zero-custo, então
  `setsid` é desnecessário.
- Para daemons de produção: NÃO use esta receita. O bypass é
  opt-in; código de produção NUNCA deve chamar
  `try_reset_shutdown()`, e `SQLITE_GRAPHRAG_IGNORE_SHUTDOWN`
  NUNCA deve ser setada em produção. A receita é para tests e
  invocações de auditoria apenas.
- Para jobs interrompidos entre as camadas: o arquivo SQLite
  permanece consistente (WAL, commit atômico, sem escritas
  parciais), e `restore` ou `enrich --operation re-embed
  --resume` podem retomar a partir da última memória
  bem-sucedida.

### Veja Também

- `docs/HEADLESS_INVOCATION.pt-BR.md` → "Atualização v1.0.80 —
  Resiliência de SHUTDOWN e a Receita de Bypass em 3 Camadas"
- `docs/decisions/adr-0034-shutdown-resilience.pt-BR.md`
- `src/signals.rs` para a barreira de captura de panic na v1.0.80
- `src/embedder.rs:537` para o braço de cancelamento desviado

## Como Coordenar Invocações Concorrentes de `remember` (v1.0.80, G45)

### Problema

Dois teammates do agente chamando `remember` no mesmo banco ao
mesmo tempo costumavam spawnar 2 subprocessos LLM, 2 batches
paralelos e 2 requisições OAuth queimando quota. O cache em
processo da v1.0.79 não conseguia endereçar isso porque o cache
vive dentro do processo e os subprocessos vivem entre processos.

### Solução

A v1.0.80 introduz um singleton de embedding cross-process
(`acquire_embedding_singleton`) que serializa chamadas de
embedding LLM por par `(namespace, db)`:

```bash
# Comportamento default: a segunda CLI recebe
# AppError::EmbeddingSingletonLocked (exit 75, retentável)
# quando outra invocação já está embedando contra o mesmo
# par (namespace, db)

# Opt-in: faz poll até a soltura do lock
sqlite-graphrag remember --wait-embed-singleton 30 --graph-stdin < payload.json
```

### Explicação

- O singleton usa `fs4` flock, a mesma primitiva do
  `acquire_job_singleton` do G30. O arquivo de lock vive em
  `~/.local/share/sqlite-graphrag/embed-slots/` e é chaveado
  por `(namespace, db_hash)` para que bancos distintos e
  namespaces distintos adquiram locks independentes.
- A segunda CLI concorrente no mesmo par `(namespace, db)`
  recebe `AppError::EmbeddingSingletonLocked { namespace }`
  com exit code 75 e `is_retryable() == true`. A mensagem
  localizada em pt-BR nomeia o namespace explicitamente.
- `--wait-embed-singleton <SEGUNDOS>` faz poll do lock com o
  mesmo contrato de `--wait-job-singleton` (G30).
- Bancos ou namespaces distintos prosseguem em paralelo sem
  contenção; o singleton G45 só serializa trabalho de embedding
  para o MESMO par `(namespace, db)`.

### Variantes

- Para CI: pule o singleton usando o mock-llm no PATH (Camada 1
  da receita de bypass SHUTDOWN). O mock torna o embedding
  instantâneo, então o singleton só adiciona latência de poll
  sem benefício.
- Para pipelines de ingest de alto throughput: NÃO aumente
  `--llm-parallelism` acima de 4 no modo Claude sem definir
  `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` (G28-A). O
  singleton cross-process NÃO substitui a orientação de
  isolamento MCP do G28-A; ambos se aplicam.

### Veja Também

- `docs/AGENTS.pt-BR.md` → "OBRIGATÓRIO — G45 Singleton de
  Embedding Cross-Process"
- `docs/MIGRATION.pt-BR.md` → "G45 singleton de embedding
  cross-process"
- `src/lock.rs` para `acquire_embedding_singleton`
- `src/errors.rs` para `AppError::EmbeddingSingletonLocked`

## Como Buscar Uma Memória Por ID Sem a Máscara `unknown` do NotFound (v1.0.80, G55 S2)

### Problema

Na v1.0.79, `read --name <nome>` contra uma memória ausente
emitia `memory not found: unknown in namespace 'global'` — o
nome solicitado era descartado e substituído pelo literal
`unknown`. Com `--lang pt`, a mensagem virava um híbrido
meio-traduzido. Scripts que filtram stderr ou o campo `message`
do envelope JSON pelo nome solicitado NUNCA casavam.

### Solução

Na v1.0.80, o caminho legado `NotFound(String)` é substituído
por `AppError::MemoryNotFound { name, namespace }` e
`AppError::MemoryNotFoundById { id }`. O identificador é parte
da variante, então a mensagem sempre carrega o nome e o
namespace solicitados:

```bash
sqlite-graphrag read --name memoria-fantasma --json
# {"error":true,"code":4,"message":"memory 'memoria-fantasma' not found in namespace 'global'", ...}

sqlite-graphrag read --id 99999 --json
# {"error":true,"code":4,"message":"id=99999 not found in namespace 'global'", ...}

sqlite-graphrag --lang pt read --name memoria-fantasma --json
# {"error":true,"code":4,"message":"memória 'memoria-fantasma' não encontrada no namespace 'global'", ...}
```

### Explicação

- O compilador agora EXIGE o identificador ao construir a
  variante de erro, eliminando a classe inteira de bugs
  "esqueci de incluir o nome".
- A mensagem localizada em pt-BR carrega nome e namespace
  explicitamente. Zero fragmentos em inglês.
- O exit code permanece 4, então a lógica de retry existente
  e o roteamento por exit code continuam funcionando.

### Variantes

- Para scripts de auditoria que grep em stderr: a mensagem
  agora contém o nome solicitado verbatim, então `rg "<nome>"`
  no filtro de stderr funciona.
- Para o padrão `read` pós-`remember` que o harness do
  agente usa para verificar persistência: a mensagem agora
  confirma TANTO o nome consultado QUANTO o namespace,
  encerrando o loop de diagnóstico em uma única invocação.

### Veja Também

- `docs/AGENTS.pt-BR.md` → "OBRIGATÓRIO — G55 S2: `MemoryNotFound`
  Estrutural"
- `src/errors.rs` para `AppError::MemoryNotFound` e
  `AppError::MemoryNotFoundById`
- `src/commands/read.rs` para a nova construção do label

### Variantes
- Para containers com pouca RAM (≤ 4 GB): adicione `SQLITE_GRAPHRAG_LOW_MEMORY=1` e `--llm-parallelism 1`.
- Para runners de CI: defina a env var via YAML do workflow e passe `--max-rss-mb 2048` para `ingest --mode openrouter --openrouter-model MODELO` para abortar cedo em pressão de memória.

### Veja Também
- Receita "Como integrar sqlite-graphrag com o loop de subprocessos do Claude Code"
- docs/HOW_TO_USE.pt-BR.md → "Limitando proliferação de processos em execuções com Claude Code (G28, v1.0.68)"


## Receitas adicionadas na v1.0.82
### Receita — `remember` em Três Estágios Com Recuperação de Checkpoint (GAP-001, ADR-0036)
```bash
# Estágio 1: persiste a memória com entidades/relacionamentos na fila pending
sqlite-graphrag remember --name v1-0-82-release --type decision \
  --body "v1.0.82 entrega cinco gaps fechados; veja CHANGELOG.pt-BR.md" \
  --entities-file /tmp/ents.json --relationships-file /tmp/rels.json --json

# Estágio 2: SIGTERM durante subprocesso LLM de embed -> a linha fica queued
# Estágio 3: re-spawna manualmente o subprocesso de embed
sqlite-graphrag pending list --status pending --json | jaq '.pending[] | .id'

# Limpa linhas em estado terminal periodicamente
sqlite-graphrag pending cleanup --filter-status done --yes --json
```
### Receita — Mitigação do codex OAuth 401 (GAP-005, ADR-0040)
```bash
# Após atualizar para v1.0.82, refresque o token OAuth uma vez
codex login

# Configure a cadeia de fallback para que um 401 refresh_token_reused roteie para claude
sqlite-graphrag remember --name design-auth --type decision \
  --body "Padrão de mitigação OAuth 401 com cadeia de backend" \
  --llm-backend codex,claude --json

# Inspecione linhas que esgotaram todos os backends
sqlite-graphrag pending-embeddings list --filter-status failed --json
```
### Receita — Observabilidade de Slots Cross-Process (GAP-004, ADR-0039)
```bash
# Limita subprocessos LLM simultâneos host-wide
sqlite-graphrag remember --name minha-memoria --type note \
  --body "..." --llm-max-host-concurrency 4 --json

# Inspeciona uso de slots em outro terminal
sqlite-graphrag slots status --json | jaq '{acquired, waiting, p50_wait_ms}'

# Reapa slots órfãos de um PID morto
sqlite-graphrag slots release --slot-id 3 --yes --json
```
### Receita — Envelope de Shutdown Gracioso (GAP-002, ADR-0037)
```bash
# Dispara um shutdown no meio do embed (em outro terminal)
kill -SIGTERM $(pgrep -f "sqlite-graphrag remember")

# O processo sai com 19 e emite envelope JSON no stdout
sqlite-graphrag remember --name foo --type note --body "..." --json
# {"error":true,"code":19,"signal":"SIGTERM","graceful":true,"message":"..."}
```
### Receita — Cadeia de Backend Customizada (GAP-003, ADR-0038)
```bash
# Força claude apenas (sem fallback)
sqlite-graphrag remember --name a --type note --body "..." \
  --llm-backend claude --json

# Permite embedding NULL quando ambos falham
sqlite-graphrag remember --name b --type note --body "..." \
  --llm-backend codex,claude,none --skip-embedding-on-failure --json

# Health check após o lote
sqlite-graphrag health --json | jaq '.counts'
```

## Como Usar o OpenRouter Para Embedding Rápido (v1.0.93)

### Problema

Embedding via LLM subprocess (codex/claude/opencode) leva 15-60 segundos por
chamada devido ao overhead de cold-start. O ingest em lote de 100 arquivos
pode levar 25+ minutos. O modelo subprocess também é frágil sob SIGTERM e
exige sessões OAuth.

### Solução

Use `--embedding-backend openrouter` com um modelo de embedding dedicado para
chamar a API REST do OpenRouter diretamente (~200ms por chamada). Sem
subprocess, sem OAuth, sem cold-start. Armazene a chave com `config add-key` e passe
`--embedding-model`.

### Configuração

```bash
printf "%s" "sk-or-v1-sua-chave-aqui" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY não é lida em runtime (G-T-XDG-04)
```

### Remember com OpenRouter

```bash
sqlite-graphrag \
  --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-4b" \
  remember --name minha-decisao --type decision \
  --description "Escolha arquitetural" \
  --body "Escolhemos o OpenRouter para embeddings REST rápidos." \
  --force-merge --json
```

### Ingest com OpenRouter e Auto-Enrich

A flag `--enrich-after` dispara `enrich --operation memory-bindings`
automaticamente após o ingest — sem necessidade de segundo comando.

```bash
sqlite-graphrag \
  --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  ingest ./docs \
  --recursive --pattern "*.md" \
  --type document \
  --enrich-after \
  --json
```

### Recall com Embedding de Query via OpenRouter

O vetor de consulta também é produzido via OpenRouter — use o MESMO modelo
utilizado durante o ingest para manter consistência no espaço de embedding.

```bash
sqlite-graphrag \
  --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  recall "decisão arquitetural" --k 10 --json
```

### Guia de Seleção de Modelo

- Melhor qualidade: `google/gemini-embedding-001` (MTEB score ~0,93)
- Melhor custo-benefício: `qwen/qwen3-embedding-8b` (score ~0,68, $0,01/M tokens)
- Custo zero: `nvidia/llama-nemotron-embed-vl-1b-v2:free` (score ~0,42)
- Compatível OpenAI: `openai/text-embedding-3-large` (score ~0,72)
- Multilíngue: `mistral/mistral-embed` ou `baai/bge-m3`

Todos os modelos produzem vetores de 384 dimensões via truncação MRL. Zero
mudança de schema. Trocar de modelo no meio do projeto exige re-embedding das
memórias existentes.

### Variantes

- `--embedding-backend auto` — usa OpenRouter se a chave XDG estiver (OPENROUTER_API_KEY não é lida em runtime)
  definida, caso contrário volta à cadeia LLM subprocess.
- `--embedding-backend llm` — força LLM subprocess (codex/claude), ignora
  qualquer chave OpenRouter presente no ambiente.
- Passar `--openrouter-api-key` inline sobrescreve a chave no XDG (OPENROUTER_API_KEY não é lida em runtime).

### Veja Também

- `docs/HOW_TO_USE.pt-BR.md` → "O Que Mudou na v1.0.93 — Backend de Embedding
  OpenRouter (GAP-OR-INGEST)"
- Receita "Como ingerir um diretório de arquivos markdown no grafo"


## Receitas adicionadas na v1.1.05

Veja [ADR-0065](decisions/adr-0065-v1-1-05-danilo-bugs.pt-BR.md) e [`tests/v1105_danilo_bugs_regression.rs`](../tests/v1105_danilo_bugs_regression.rs).

### Receita — deep-research com token único (Bug 1)
```bash
# Token único agora gera sub-queries multi-aspecto (source: "aspect")
sqlite-graphrag deep-research "danilo" --k 20 --max-sub-queries 7 --json | jaq '.sub_queries | length'
# Estratégia manual permanece disponível
printf '%s\n' 'danilo patrimônio' 'danilo stack' 'danilo projetos' > /tmp/sq.txt
sqlite-graphrag deep-research "danilo" --sub-query-strategy manual --sub-queries-file /tmp/sq.txt --json
```

### Receita — envelope atômico com --output e --quiet (Bug 2)
```bash
# JSON completo no arquivo (atomwrite); ack curto no stdout; logs no stderr
sqlite-graphrag --quiet deep-research "auth architecture" --k 20 --output /tmp/dr.json --json
# Campos exatos do ack:
# { "written", "bytes", "blake3", "sub_queries_total", "unique_memories_found", "elapsed_ms" }
# Schema: docs/schemas/deep-research-output-ack.schema.json
jaq '.stats' /tmp/dr.json
# NUNCA redirecione stdout+stderr no mesmo arquivo: # errado: ... &> /tmp/all.log
```

### Receita — graph traverse com apelido curto e --fuzzy (Bug 3)
```bash
# Sem --fuzzy: NotFound (exit 4) inclui sugestões ranqueadas
sqlite-graphrag graph traverse --from danilo --depth 2 --json
# Com --fuzzy: auto-resolve vencedor claro (warning em stderr)
sqlite-graphrag graph traverse --from danilo --depth 2 --fuzzy --json
```

### Receita — merge sem self-ref e link por ID (Bugs 4 e 5)
```bash
# Bug 4: self-ref rejeitado antes de qualquer trabalho no DB
# (falha se 3 estiver em --ids)
# sqlite-graphrag merge-entities --ids 1,2,3 --into-id 3 --json

# Bug 5: link por ID numérico (não cria entidade fantasma "12")
sqlite-graphrag link --from-id 12 --to-id 34 --relation uses --json
```

## Receitas adicionadas na v1.1.04

### Receita — deep-research Agora Estável (GAP-001, ADR-0064)

```bash
# Antes da v1.1.04 isto entrava em pânico com "Cannot start a runtime from within a runtime"
sqlite-graphrag deep-research "decisões de arquitetura de autenticação e incidentes de segurança" --k 20 --max-hops 3 --json
```

- O pânico de runtime Tokio aninhado está corrigido: `deep_research::run` computa os embeddings das sub-queries antes de construir T1, e os três caminhos OpenRouter de `embedder.rs` usam `Handle::try_current()` + `block_in_place`.

### Receita — entity-connect Convergente (GAP-002, ADR-0064 + v1.1.06 scan O(k))

```bash
# 0. Dry-run primeiro no `global` grande (deve terminar em segundos, emitir scan_start depois scan)
sqlite-graphrag enrich --operation entity-connect --dry-run --json --limit 50 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro

# 1. Inspecione o backlog real (proxy grau-0; era sempre 0 antes da v1.1.04)
sqlite-graphrag enrich --operation entity-connect --status --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json

# 2. Rode até convergir (cada par avaliado UMA VEZ, veredito em entity_connect_seen)
#    --max-runtime também cobre o PRIMEIRO scan (v1.1.06 InterruptHandle → Timeout exit 1)
sqlite-graphrag enrich --operation entity-connect --until-empty --max-runtime 600 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json

# 3. Verifique se novas arestas apareceram
sqlite-graphrag graph stats --json
```

- entity-connect é fully-implemented (v1.1.04): `entity_connect_seen` (V016) registra vereditos do LLM para que re-scans pulem pares já avaliados (**GAP-002** preservado).
- **v1.1.06 (GAP-ENTITY-CONNECT-SCAN-CARTESIAN):** candidatos são pares por **coocorrência** em `memory_entities` mais preenchimento **hub × ilha grau-0** — nunca o cartesiano `entities × entities` com `ORDER BY` global que travava o `global` grande. Chaves da fila `pair:{id1}:{id2}` (`item_type=entity_pair`). **O drain resolve cada par enfileirado por chave primária** sem reexecutar o scan O(k)/cartesiano por item. NDJSON emite `scan_start` (com `operation`, `entities_in_namespace`, `backlog_degree0_proxy`) **antes** do SQL, depois `scan_meta` com `pairs_enqueued_this_scan` / `scan_elapsed_ms`. Teto soft de 120s no scan quando `--max-runtime` é omitido (Timeout exit **1**, não 75). `cross-domain-bridges` compartilha o mesmo path. ADR-0066; suite `tests/v1106_entity_connect_scan_regression.rs`.

### Receita — Aplicar Migração V016

```bash
sqlite-graphrag migrate --dry-run --json   # prévia da V016
sqlite-graphrag migrate --json              # aplicar
sqlite-graphrag health --json               # confirmar schema_version >= 16
```

## Receitas adicionadas na v1.1.03
### Receita — Migrar Relações Legadas Com Underscore Para A Forma Canônica Com Hífen (Bug 2)
Imports antigos armazenavam literais de relação com underscore (`applies_to`, `depends_on`, `tracked_in`) em vez da forma canônica com hífen. O parser normaliza na leitura, mas o literal ARMAZENADO continuava com underscore. Use a flag verbatim de destino `--literal-to` (simétrica a `--literal-from`) para que o destino seja escrito VERBATIM sem normalização do clap.
```bash
# Passo 1 — preview da contagem de candidatos para cada relação legada (dry-run)
sqlite-graphrag reclassify-relation --literal-from applies_to --literal-to applies-to --batch --dry-run --json
sqlite-graphrag reclassify-relation --literal-from depends_on --literal-to depends-on --batch --dry-run --json
sqlite-graphrag reclassify-relation --literal-from tracked_in --literal-to tracked-in --batch --dry-run --json

# Passo 2 — aplique (sem --dry-run) quando as contagens parecerem corretas
sqlite-graphrag reclassify-relation --literal-from applies_to --literal-to applies-to --batch --json
sqlite-graphrag reclassify-relation --literal-from depends_on --literal-to depends-on --batch --json
sqlite-graphrag reclassify-relation --literal-from tracked_in --literal-to tracked-in --batch --json
```
O envelope do `--dry-run` reporta a contagem exata de arestas legadas do SEU banco (cerca de 61357 em imports pré-era-canônica-com-hífen).

### Receita — Recuperar De Locks Stale Do Enrich Após `kill -9` (Bug 4)
Quando um worker do enrich morre de forma dura (OOM, `kill -9`, queda de energia) sua processing claim fica travada. O auto-reset do startup limpa claims mais antigas que o limiar de stale, mas se o binário já estiver rodando você pode forçar um reset sem reiniciar.
```bash
# Reseta toda processing claim stale de volta para pending agora
sqlite-graphrag enrich --reset-stale-claims --json

# Ou sobrescreva o limiar de stale (ex. 120 segundos) e resete
sqlite-graphrag enrich --stale-claim-secs 120 --reset-stale-claims --json
```
Um heartbeat em background atualiza o `claimed_at` enquanto um item está sendo processado, então um worker vivo nunca é falsamente classificado como stale.

### Receita — Dividir Corpos De Memória Sobredimensionados Para O Orçamento De Embedding (V8)
Um corpo de memória perto do teto de tokens do embedding eventualmente falha com exit 6 (`TooManyTokens`). O novo subcomando `split-body` divide corpos sobredimensionados em filhas `{name}-part-{i}`, marca a original com metadata `superseded_by_split: true` e cria relações canônicas `replaces` de cada filha para a original.
```bash
# Memória sobredimensionada única
sqlite-graphrag split-body --name log-import-enorme --json

# Toda memória acima de 25000 chars em um lote
sqlite-graphrag split-body --batch --threshold 25000 --json

# As filhas NÃO são embebedadas inline — re-embeba-as para que fiquem pesquisáveis
timeout 600 sqlite-graphrag \
  --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 \
  enrich --operation re-embed --target memories \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro \
  --until-empty --max-runtime 600 --json
```
`related log-import-enorme --hops 2` e `graph traverse --from log-import-enorme --depth 1` ainda alcançam o corpo sobrescrito via as arestas `replaces`.


## Receitas adicionadas na v1.1.02
### Receita — Auditar E Remover `--gliner-variant` / `--mode gliner` (Gap 1, BREAKING)
```bash
# Encontre todas as ocorrências na sua automação
rg -- "--gliner-variant|--mode gliner" seus-scripts/ ci/ Makefile .github/

# Após deletar os matches, verifique que o binário agora rejeita
sqlite-graphrag remember --name probe --type note --description d --body "x" \
  --gliner-variant small --json
# esperado: clap exit 2, erro "unexpected argument '--gliner-variant'"
```
- O enum `IngestMode` em v1.1.02 expõe apenas `none` — `gliner` sumiu
- As env vars `SQLITE_GRAPHRAG_GLINER_MODEL`/`SQLITE_GRAPHRAG_GLINER_THRESHOLD` são silenciosamente ignoradas (sem erro, sem warning)

### Receita — Varrer Órfãos Dead-Letter Tanto de Memória Quanto de Entidade
```bash
# 1. Inspecione o backlog dead-letter por item_type
sqlite-graphrag enrich --status --json | jaq '.queue_dead'

# 2. Descarte órfãos com chave de memória (memórias renomeadas/purgadas após enqueue)
sqlite-graphrag enrich --prune-dead-orphans --json

# 3. Descarte órfãos com chave de entidade (entidades renomeadas/fundidas/purgadas após enqueue) — v1.1.02
sqlite-graphrag enrich --prune-dead-entity-orphans --json
```
- As duas flags são mutuamente exclusivas — rode em invocações separadas
- Ambas são inspetores read-only (sem LLM, sem singleton); apenas o sidecar `.enrich-queue.sqlite` é mutado

## Receitas adicionadas na v1.1.01
### Receita — Backfill de Embeddings de Entidades e Chunks com `re-embed --target` (P2)
```bash
# Backfill apenas dos embeddings de entidades
sqlite-graphrag \
  --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  enrich --operation re-embed --target entities --limit 100 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json

# Backfill de tudo em uma passada (memórias + entidades + chunks)
sqlite-graphrag \
  --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  enrich --operation re-embed --target all --limit 100 \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json

# Inspecionar o backlog por alvo sem spawnar o LLM
sqlite-graphrag enrich --operation re-embed --status \
  --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --json
```
- `--target` tem padrão `memories`, preservando o comportamento pré-v1.1.01
- `--status` reporta `scan_backlog` por alvo, então você sabe quando cada tabela de embedding convergiu para backlog zero
- Combine com a receita de cobertura vetorial abaixo para decidir qual alvo ainda precisa de trabalho

### Receita — Recalcular Graus de Entidades Após Manutenção em Massa do Grafo (P3)
```bash
# Prévia do drift sem escrever
sqlite-graphrag graph recompute-degree --dry-run --json

# Aplicar — recalculado a partir das arestas reais em transação única
sqlite-graphrag graph recompute-degree --json
# {"total":812,"updated":37,"zeroed":4,"unchanged":771}
```
- `entities.degree` pode driftar após `prune-relations`, `merge-entities`, `delete-entity` ou `unlink` em massa
- O envelope reporta `total`, `updated`, `zeroed` (entidades que ficaram sem arestas) e `unchanged`
- Rode antes de confiar em rankings de `graph entities --sort-by degree`

### Receita — Migrar Relações Legadas com Underscore via `--literal-from` (P4)
```bash
# Receita canônica: applies_to (underscore legado) -> applies-to (hífen canônico)
sqlite-graphrag reclassify-relation \
  --literal-from "applies_to" --to-relation applies-to --batch --json
```
- `--literal-from` casa o valor de relação gravado de forma verbatim, contornando a normalização do clap que reescreveria `applies_to` antes do match
- Use para qualquer aresta legada cuja grafia gravada difira da forma canônica com hífen

### Receita — Mesclar e Renomear Entidades por ID (P5)
```bash
# Mesclar duplicatas por ID quando os nomes são ambíguos
sqlite-graphrag merge-entities --ids "1,2,3" --into-id 4 --json

# Renomear uma entidade por ID
sqlite-graphrag rename-entity --id 12 --new-name jwt-auth --json
```
- Os IDs vêm de `graph entities --json`; endereçar por ID desambigua entidades com nomes confusos ou fora do kebab-case
- As formas por nome (`--names`/`--into`, `--name`/`--new-name`) continuam funcionando sem mudança

### Receita — Diagnosticar Cobertura Vetorial com health e embedding status (P6)
```bash
sqlite-graphrag health --json | jaq '{vec_memories_missing, vec_entities_missing, vec_chunks_missing, vec_memories_coverage_pct, vec_entities_coverage_pct, vec_chunks_coverage_pct}'

sqlite-graphrag embedding status --json | jaq '.coverage'
```
- `health --json` expõe contadores `vec_*_missing` mais percentuais `vec_*_coverage_pct` por tabela de embedding
- `embedding status --json` reporta `memories_missing`/`entities_missing`/`chunks_missing` em `coverage`
- Qualquer contador `*_missing` não-zero aponta direto para a receita de `re-embed --target` acima

### Receita — Prefixar Nomes de Memórias Ingeridas por Projeto (P12)
```bash
sqlite-graphrag --llm-backend none ingest ./docs \
  --recursive --pattern "*.md" --name-prefix projeto-x- --json
```
- Cada nome de memória ingerida recebe o prefixo, então dois projetos que ingerem `README.md` no mesmo namespace deixam de colidir
- Combine com `--namespace` quando quiser isolamento em vez de mera desambiguação
