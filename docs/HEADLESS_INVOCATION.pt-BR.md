## Preservação de Env de Custom Provider em Invocação Headless (v1.0.83+)
- O pipeline de invocação headless (`claude_runner`, `codex_spawn`, `ingest_claude`) agora preserva seis env vars de custom-provider ao spawnar subprocessos: `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`, `OTEL_EXPORTER_OTLP_ENDPOINT`
- Os três spawners delegam para `apply_env_whitelist(cmd, strict)` de `src/spawn/env_whitelist.rs` em vez de inlinear o array de whitelist. Isso elimina o drift entre os três blocos duplicados de `env_clear` + re-injeção
- O guard OAuth-only em `claude_runner.rs:273`, `codex_spawn.rs:259`, `ingest_claude.rs:282`, `extract/llm_embedding.rs:237-253` permanece inalterado; `ANTHROPIC_API_KEY` e `OPENAI_API_KEY` ainda abortam com `AppError::Validation` (exit 1) e a nova mensagem de erro referencia `ANTHROPIC_AUTH_TOKEN` e `~/.codex/auth.json` como resoluções legítimas
- Nova flag global `--strict-env-clear` / `SQLITE_GRAPHRAG_STRICT_ENV_CLEAR=1` ativa modo estrito que preserva apenas `PATH`. Use em ambientes compliance (PCI-DSS, SOC2, HIPAA) onde encaminhamento de credenciais via env vars é proibido por política
- As 7 flags de endurecimento para `claude -p` (`--strict-mcp-config --mcp-config '{}' --settings '{"hooks":{}}' --dangerously-skip-permissions --output-schema` mais model e prompt) e o conjunto canônico para `codex exec` permanecem inalterados. A mudança no whitelist de env é puramente aditiva no passo de whitelist entre `env_clear()` e a construção das flags canônicas
- Sem telemetria nova: o fix é silencioso. O teste de auditoria no-leak `audit_no_token_leak_in_subprocess_stderr` em `tests/claude_runner_env.rs` garante que o valor literal do token NUNCA aparece em stdout ou stderr mesmo com `RUST_LOG=trace`
- Veja `docs/decisions/adr-0041-preserve-custom-provider-env.pt-BR.md` para a justificativa arquitetural completa
# Invocação Headless — Claude Code, Codex, OpenCode sem MCP e sem Hooks (v1.0.89 — Camada Pre-flight + Hotfixes)

> Como invocar LLMs headless neste projeto sem herdar MCPs ou hooks do ambiente, mantendo o login OAuth de assinatura.

- Versão em inglês deste guia vive em [HEADLESS_INVOCATION.md](HEADLESS_INVOCATION.md)
- Voltar ao [README.md](../README.md) para referência de comandos


## Resumo

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

- Schema permanece **v16** (sem migração main-DB). Gate offline ainda `scripts/e2e_offline_v120.sh` **20/20**. Pin `=1.2.1`.

## Atualização v1.2.0 — XDG, dim 1024, list-skipped, GAP-SG-139, hot-set headless

- Config de knobs de produto: **flag CLI > XDG `config set` > default**. Variáveis de ambiente de produto `SQLITE_GRAPHRAG_*` **não** são lidas no hot path. Harnesses DEVEM usar XDG isolado + flags — nunca exportar product env como contrato de config.
- **DEFAULT_EMBEDDING_DIM=1024** (override via `--embedding-dim` / XDG `embedding.dim`; bancos existentes mantêm `schema_meta.dim` até re-embed).
- Recuperar dívida da fila `skipped` / `preservation_failed` sem SQL cru:
  ```bash
  sqlite-graphrag enrich --list-skipped --json
  sqlite-graphrag enrich --requeue-skipped --json
  ```
- **GAP-SG-139:** folhas host/XDG (`config`, `slots`, `cache`, `codex-models`, `completions`) aceitam `--db` como **no-op** documentado — agentes headless podem anexar `--db` em todo spawn sem clap exit 2.
- Após `remember` curado, PARSEIE `entities_created` / `enrich_recommended` e/ou PASSE `--enqueue-enrich` para entity-descriptions prioritário antes de drains longos de entity-connect.
- Polle qualidade sem LLM: `enrich --operation entity-descriptions --status --force-redescribe --json` (`scan_backlog_low_quality`, `quality_pct`, `state` incluindo `blocked_dead`).
- Filtros de nome: `--entity-names` para entity-descriptions; `--memory-names` para memory-bindings.
- Audite bindings: `memory-entities --name <mem> --json` inclui `entities[].description`.
- entity-connect é totalmente implementado (persiste relações). Em DBs grandes espere `budget_exhausted` / `preempted_for_gate`; prefira ED quente → EC frio.
- Gate offline de produto: `bash scripts/e2e_offline_v120.sh` espera **20/20 PASS** (canônico; wrapper histórico `e2e_offline_v118.sh` / 16/16 supersedido).
- **Nota CURRENT sobre texto histórico abaixo:** seções que ensinam product env como config descrevem comportamento pré-v1.2.0 — a v1.2.0 **não** lê product env no hot path (somente XDG + flags). A whitelist OAuth/custom-provider de env para subprocessos LLM permanece válida e **não** é config de knobs de produto.
- Segredos: prefira `config add-key --provider openrouter` (stdin) ou `--openrouter-api-key`; `OPENROUTER_API_KEY` ainda pode resolver como entrada opcional de chave.

## Inventário completo de comandos CLI para agentes headless (v1.2.1)

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
- `codex-models` — lista de modelos OAuth ChatGPT Pro (`--db` no-op, GAP-SG-139)
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

> **Top-level (50 + help):** init, remember, remember-batch, ingest, recall, read, list, forget, purge, rename, split-body, edit, history, restore, hybrid-search, health, migrate, namespace-detect, optimize, stats, sync-safe-copy, backup, vacuum, link, unlink, deep-research, related, graph, export, fts, vec, codex-models, prune-relations, prune-ner, slots, pending, embedding, pending-embeddings, cleanup-orphans, memory-entities, cache, delete-entity, reclassify, rename-entity, merge-entities, enrich, reclassify-relation, normalize-entities, completions, config, help.

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


- Claude Code OAuth sem MCP usa `--strict-mcp-config --mcp-config '{}'`
- Codex OAuth sem MCP usa `codex exec -c mcp_servers='{}'`
- OpenCode OAuth sem MCP usa `OPENCODE_CONFIG_CONTENT` com `enabled` falso por servidor
- A descoberta mais importante: no Claude, a flag `--bare` corta os MCP mas DESLIGA o OAuth. `--bare` passa a exigir chave de API, que aqui é proibida. Por isso NÃO se usa `--bare` quando o login é por assinatura


## Tabela de Comandos OAuth-Safe

| CLI | Comando headless OAuth-safe | Mantém OAuth | Corta MCP | Corta Hooks |
| --- | --- | --- | --- | --- |
| Claude Code | `claude -p "TAREFA" --strict-mcp-config --mcp-config '{}' ...` | sim | sim | sim |
| Codex CLI | `codex exec -c mcp_servers='{}' ...` | sim | sim | N/A |
| OpenCode | `OPENCODE_CONFIG_CONTENT='{...enabled:false...}' opencode run ...` | sim | sim | N/A |


## Claude Code Headless OAuth sem MCP e sem Hooks

### O Que Fazer

Rodar `claude -p` com a config de MCP travada e vazia, e a config de hooks zerada.

### Por Que Fazer

- O `-p` ativa o modo headless de uma tacada só
- O `--strict-mcp-config` manda ignorar TODA config de MCP do ambiente
- O `--mcp-config '{}'` entrega uma lista vazia de servidores
- O `--settings '{"hooks":{}}'` desliga os hooks naquela chamada específica
- A combinação garante zero MCP e zero hooks no ar, mantendo o login por assinatura (OAuth Pro ou Max)

### Atualização v1.0.79 — O Isolamento Real É `CLAUDE_CONFIG_DIR` Vazio

- A issue #10787 de `anthropics/claude-code` documenta que `--strict-mcp-config` e `--mcp-config` são silenciosamente IGNORADAS pelo upstream
- O único mecanismo que o upstream honra é `CLAUDE_CONFIG_DIR` apontando para um diretório vazio
- Desde a v1.0.79 (G42/S6), o pipeline de embedding da CLI usa `CLAUDE_CONFIG_DIR` vazio POR PADRÃO: honra `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR`, senão um diretório gerenciado `~/.local/state/sqlite-graphrag/claude-empty-config` (modo 0700, copia `.credentials.json` quando presente)
- Um `~/.claude` populado custava ~223k tokens de cache-creation por chamada (~40-50s); o config dir vazio derruba para ~10-15s
- As flags abaixo continuam sendo passadas por defesa em profundidade, mas NÃO confie nelas como isolamento

### Atualização v1.0.91 — Isolamento Automático de CWD via `apply_cwd_isolation()`

- Desde a v1.0.91, TODOS os 10 sites de spawn de subprocessos LLM chamam `apply_cwd_isolation()` de `src/spawn/mod.rs`
- Esta função define `current_dir(temp_dir)` — o CWD do subprocesso é um diretório limpo `/tmp/sqlite-graphrag-spawn-{PID}/` SEM `.mcp.json` em nenhum ancestral
- Também define `CLAUDE_CONFIG_DIR=temp_dir` — isolando o subprocesso da configuração MCP user-level de `~/.claude/`
- O workaround manual `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1 CLAUDE_CONFIG_DIR=/tmp/graphrag-empty-config` NÃO É MAIS NECESSÁRIO para operação normal (mantido como override de emergência)
- Diretórios de spawn são limpos automaticamente ao final do processo via `cleanup_spawn_dir()` em `src/main.rs` (GAP-SPAWN-002) — `remove_dir()` não-recursivo, seguro para diretórios não-vazios
- A env var `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` da v1.0.79 e o diretório gerenciado `~/.local/state/sqlite-graphrag/claude-empty-config` permanecem funcionais mas são suplantados pelo isolamento automático de CWD

### Atualização v1.0.93 — Backend de Embedding OpenRouter Dispensa Subprocesso

- Desde a v1.0.93, embedding pode usar a API REST do OpenRouter em vez de spawnar um subprocesso LLM headless
- Use `--embedding-backend openrouter --embedding-model MODELO` para rotear embedding via `POST /api/v1/embeddings`
- Isso elimina o cold-start do subprocesso (~200ms de chamada API vs 15-20s de spawn de subprocesso por embedding)
- O caminho OpenRouter usa `reqwest+rustls-tls` diretamente — sem `claude -p`, sem `codex exec`, sem isolamento de CWD necessário
- O enforcement OAuth-only NÃO se aplica ao OpenRouter — ele usa sua própria `OPENROUTER_API_KEY` / `--openrouter-api-key`
- O spawn de subprocesso headless permanece inalterado para embedding baseado em LLM (`--embedding-backend llm`) e para operações de `enrich` rodadas com `--mode claude-code|codex|opencode`
- Desde a v1.0.95 (ADR-0054), `enrich --mode openrouter` também dispensa o subprocesso: a etapa JUDGE roda via endpoint REST `/chat/completions` do OpenRouter (`reqwest+rustls-tls`), sem spawn de `claude -p` / `codex exec` / `opencode run` e sem isolamento de CWD. O pipeline SCAN→JUDGE→PERSIST permanece inalterado; só o transporte do JUDGE muda.
- A flag `--enrich-after` no `ingest` ainda spawna um subprocesso headless para a fase de enrich quando o modo de enrich é uma CLI local; com `--mode openrouter` a fase de enrich permanece sem subprocesso
- Veja ADR-0052 (embedding OpenRouter) e ADR-0054 (JUDGE do enrich via OpenRouter) para a justificativa arquitetural completa

### Atualização v1.0.95 — JUDGE do Enrich via OpenRouter Dispensa Subprocesso

- `enrich --mode openrouter` roteia a etapa JUDGE para `POST /api/v1/chat/completions` — sem subprocesso de CLI local
- `--openrouter-model` é OBRIGATÓRIA com `--mode openrouter` (SEM default; omiti-la → exit 1 antes de qualquer chamada de rede)
- `--openrouter-api-key` lê da env `OPENROUTER_API_KEY` ou de `config add-key --provider openrouter`; `--openrouter-timeout` tem default de 300s; `--openrouter-base-url` é opcional
- A requisição usa `response_format` `json_schema` com `strict: true` e `provider.require_parameters: true`; `reasoning.enabled: false` com fallback reasoning-mandatory de uma retentativa; `usage.cost` é lido da resposta
- Trade-off: OAuth zero-token (modos CLI locais) vs tokens cobrados na `OPENROUTER_API_KEY` (modo OpenRouter); o caminho JUDGE do OpenRouter em si não exige migração, mas a v1.1.04 avança o schema do banco principal para v16 (a tabela `entity_connect_seen` da V016, exigida apenas quando você rodar depois `enrich --operation entity-connect`)

```bash
# JUDGE do enrich headless via REST OpenRouter (sem subprocesso, sem isolamento de CWD)
export OPENROUTER_API_KEY="sk-or-v1-sua-chave-aqui"
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" --json
```

### Atualização v1.0.96 — Convergência do Backlog e Status Read-Only da Fila (ADR-0055)

- `enrich --until-empty` substitui o loop bash externo de retry na invocação headless: um único processo roda o loop interno scan→drain até a fila não ter mais itens elegíveis ou `--max-runtime <SECONDS>` (default 3600) expirar. A fila dead-letter garante que o conjunto vivo decresce estritamente — falhas transientes reagendam `next_retry_at` com backoff, um item vira `dead` após `--max-attempts` (padrão 8) retries transientes ou na primeira falha dura, e linhas `dead` são excluídas do dequeue.
- `enrich --status --json` é a sonda read-only para hooks e timers: reporta as contagens da fila (`unbound_backlog`, `scan_backlog` por operação, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`) e NÃO chama o LLM e NÃO adquire o singleton por namespace; o `scan_backlog` (GAP-SG-77, v1.1.0) é o backlog real do banco por operação que um scan enfileiraria — elimina o falso `pending=0` para `entity-descriptions`/`body-enrich`/`re-embed`, e o `state` deriva o `pending-scan` dele. Um timer cron ou systemd pode fazer poll sem disputar com um `enrich` em execução.
- `enrich --prune-dead-orphans --json` é um inspetor read-only complementar (sem LLM, sem singleton): deleta linhas dead-letter (`status='dead'`, `item_type='memory'`) cujo nome de memória não existe mais no banco principal, mutando apenas o sidecar `.enrich-queue.sqlite`; linhas dead de entidade não são tocadas. Use-o em scripts de manutenção headless para limpar acúmulo de dead-letter órfão de memórias renomeadas ou purgadas após o enqueue (ADR-0058, GAP-SG-66, v1.0.97).
- `enrich --prune-dead-entity-orphans --json` (v1.1.02, ADR-0062) é a contraparte para chaves de entidade: deleta linhas dead-letter com `item_type='entity'`, e é mutuamente exclusivo com `--prune-dead-orphans`. Rode ambos em sequência para uma varredura completa de órfãos após um upgrade que renomeou/fundiu/purgou entidades.
- `--rest-concurrency <N>` (clamp 1..=16, default 8) define o fan-out REST in-flight para embedding `--mode openrouter`; aumente-o para vazão OpenRouter. É distinta de `--llm-parallelism` (que limita subprocessos LLM locais) e de `--max-attempts` (o orçamento de retries).

```bash
# Drain headless do backlog — sem while-loop externo, sem subprocesso para OpenRouter
export OPENROUTER_API_KEY="sk-or-v1-sua-chave-aqui"
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" \
  --until-empty --max-runtime 1800 --max-attempts 8 --rest-concurrency 8 --json

# Sonda de hook/timer — inspecionar a fila sem spawnar o LLM nem adquirir o singleton
sqlite-graphrag enrich --status --json | jaq '{eligible_now, waiting, dead: .queue_dead}'
```

### Por Que NÃO Usar `--bare`

- O `--bare` também corta MCP, hooks, skills, plugins e auto memory
- MAS o `--bare` desativa o OAuth e o keychain (issue #39069 de `anthropics/claude-code`)
- Com `--bare`, o Claude exige `ANTHROPIC_API_KEY`, que é proibido neste projeto
- Para manter OAuth, o caminho certo é `--strict-mcp-config`, nunca `--bare`

### Como Fazer

```bash
claude -p "SUA TAREFA AQUI" \
  --strict-mcp-config \
  --mcp-config '{}' \
  --dangerously-skip-permissions \
  --settings '{"hooks":{}}' \
  --model claude-sonnet-4-6 \
  --max-turns 8 \
  --output-format json
```

### O Que Cada Pedaço Faz

- `--strict-mcp-config` ignora MCP de settings global e de projeto
- `--mcp-config '{}'` fornece a lista vazia que zera os servidores
- `--dangerously-skip-permissions` evita travar pedindo confirmação (modo `bypassPermissions`)
- `--settings '{"hooks":{}}'` desliga os hooks naquela chamada específica
- `--model claude-sonnet-4-6` escolhe o modelo sem depender de variável de ambiente
- `--max-turns 8` limita as voltas do agente como rede de segurança contra loop infinito
- `--output-format json` entrega saída fácil de parsear com `jaq`

### Como Garantir o OAuth

- Fazer login uma vez com a conta Pro ou Max antes de automatizar (`claude auth login`)
- NÃO definir `ANTHROPIC_API_KEY` no ambiente da chamada
- NÃO usar `--bare`
- Sem a variável e sem `--bare`, o Claude usa a sessão logada via OAuth

### Ressalva do Bug Conhecido

- Issue #14490 do `anthropics/claude-code` documenta que `--strict-mcp-config` NÃO sobrescreve a lista `disabledMcpServers` armazenada em `~/.claude.json`
- Para ambiente limpo, garantir que `~/.claude.json` não contém o servidor em `disabledMcpServers` ou usar `--bare` somente em ambiente controlado com `ANTHROPIC_API_KEY` (cenário explicitamente PROIBIDO neste projeto)
- A solução robusta é combinar `--strict-mcp-config --mcp-config '{}'` e garantir que o servidor não está em `disabledMcpServers` em `~/.claude.json`


## Codex CLI Headless OAuth sem MCP

### O Que Fazer

Rodar `codex exec` zerando a tabela de servidores MCP do config.

### Por Que Fazer

- O `codex exec` é o modo não interativo feito para scripts
- Ele escreve só a mensagem final no stdout e progresso no stderr
- O override `-c mcp_servers='{}'` substitui a tabela inteira por vazia
- Assim nenhum servidor MCP do `config.toml` sobe naquela chamada

### Como Fazer

```bash
codex exec \
  --model gpt-5.5 \
  -c mcp_servers='{}' \
  --sandbox workspace-write \
  --ask-for-approval never \
  "SUA TAREFA AQUI"
```

### Alternativa Mais Agressiva

- Usar `--ignore-user-config` para nem ler o `config.toml` do usuário
- Isso zera MCP junto com tudo mais que estiver no config
- O login OAuth fica salvo em `auth.json`, que é arquivo separado
- Por isso o `--ignore-user-config` NÃO derruba o login

```bash
codex exec --model gpt-5.5 --ignore-user-config --sandbox workspace-write "SUA TAREFA AQUI"
```

### O Que Cada Pedaço Faz

- `-c mcp_servers='{}'` zera só os MCP e preserva modelo e resto do config
- `--ignore-user-config` é o corte total quando você quer ambiente limpo
- `--sandbox workspace-write` libera edição de arquivos sem rede
- `--ask-for-approval never` roda sem pausar pedindo permissão

### Como Garantir o OAuth

- Rodar `codex login` uma vez para o fluxo do navegador com o ChatGPT
- Em máquina remota ou sem navegador, usar `codex login --device-auth`
- NÃO definir `OPENAI_API_KEY` no ambiente da chamada
- O login fica salvo em `~/.codex/auth.json` e o `codex exec` reaproveita a sessão

### Ressalva do Bug Antigo

- Versões antigas do Codex (0.33.0) instaladas via Homebrew não liam `[mcp_servers]` corretamente
- Issue #3441 do repositório `openai/codex` confirma que o fix está em 0.34.0+
- Validar versão com `codex --version` antes de usar o override `-c mcp_servers='{}'`


## OpenCode Headless sem MCP

### A Diferença Honesta

- O OpenCode NÃO tem uma flag única de CLI para desligar MCP
- O Claude tem `--strict-mcp-config` e o Codex tem `-c mcp_servers='{}'`
- O OpenCode controla MCP só pela config em JSON
- As configs do OpenCode são somadas, não trocadas, então é preciso desligar por servidor

### O Que Fazer

- Descobrir os nomes dos servidores ativos com `opencode mcp list`
- Desligar cada um com `enabled: false` no config

### Por Que Fazer

- O `opencode run` é o modo headless que recebe o prompt e devolve resultado
- Como a config é somada, apagar a chave não basta para remover o servidor
- Setar `enabled` falso com o mesmo nome sobrescreve e desliga aquele MCP
- O override de runtime via `OPENCODE_CONFIG_CONTENT` evita mexer nos arquivos do projeto

### Como Fazer — Passo 1 Listar Servidores Ativos

```bash
opencode mcp list
```

### Como Fazer — Passo 2 Rodar Headless Desligando Cada Servidor

```bash
OPENCODE_CONFIG_CONTENT='{"mcp":{"nome-do-server-1":{"enabled":false},"nome-do-server-2":{"enabled":false}}}' \
  opencode run --model anthropic/claude-sonnet-4-5 "SUA TAREFA AQUI"
```

### Alternativa Permanente

- Editar o `opencode.json` e marcar cada MCP com `enabled` falso
- Vale quando você nunca quer aquele servidor em execução automática

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "nome-do-server-1": { "enabled": false },
    "nome-do-server-2": { "enabled": false }
  }
}
```

### O Que Cada Pedaço Faz

- `opencode mcp list` mostra nomes e status de conexão dos servidores
- `OPENCODE_CONFIG_CONTENT` injeta config inline com alta precedência
- `enabled` falso por servidor é o que de fato impede a subida do MCP
- `--model` escolhe o modelo no formato `provedor/modelo`

### Como Garantir o OAuth

- Rodar `opencode auth login` uma vez e escolher o provedor
- A credencial fica salva em `auth.json` na pasta de dados do OpenCode
- O `opencode run` reaproveita essa credencial nas chamadas seguintes


## Login OAuth por CLI

- Claude: login na sessão via `claude auth login`. NÃO usar `--bare` para preservar OAuth
- Codex: `codex login` ou `codex login --device-auth` (sem navegador)
- OpenCode: `opencode auth login`


## Modo Headless por CLI

- Claude: `claude -p`
- Codex: `codex exec`
- OpenCode: `opencode run`



### Atualização v1.1.06 — entity-connect headless em namespaces grandes (ADR-0066)

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

- GAP-001: o `deep-research` não entra mais em panic com "Cannot start a runtime from within a runtime" quando invocado em modo headless (agent harnesses, runners de CI, jobs agendados). O entry point síncrono `deep_research::run` agora computa os embeddings por sub-query ANTES de construir seu runtime Tokio dedicado via o novo helper `compute_sub_embeddings`, e os três caminhos de embedding OpenRouter em `embedder.rs` (single, batch serial, fan-out JoinSet) adotam o padrão canônico de reentrada `Handle::try_current` + `block_in_place` já usado pelo path batch. O `ingest_opencode` também recebeu o guard. Para orquestradores headless isso significa que jobs `deep-research --with-bodies` de longa duração que antes crashavam no meio agora completam de forma confiável.
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
OPENROUTER_API_KEY="$KEY" sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model "qwen/qwen3-235b-a22b" \
  --until-empty --max-runtime 1800 --json
```

## Atualização v1.0.80 — Resiliência de SHUTDOWN e a Receita de Bypass em 3 Camadas

A v1.0.80 (ADR-0034) endurece o handler em `src/signals.rs` para que
o cenário de processo órfão que a auditoria G42/C2 identificou
não dispare mais `SIGABRT` em `BrokenPipe`. O terceiro Ctrl-C
consecutivo sai com código 130 e **ZERO I/O**, casando com o
contrato abaixo.

Para jobs longos de embedding que o harness do agente (ou qualquer
orquestrador em background) pode matar via SIGINT, use a receita
de bypass em 3 camadas. As 3 camadas são independentes e a receita
compõe aditivamente:

```bash
# Camada 1 — PATH: roteia o subprocesso LLM via o mock-llm no CI
export PATH="$PWD/tests/mock-llm:$PATH"

# Camada 2 — env: diz ao embedder para ignorar a checagem de SHUTDOWN
export SQLITE_GRAPHRAG_IGNORE_SHUTDOWN=1

# Camada 3 — grupo de processos: desanexa a CLI do pgroup do harness
setsid -w timeout 600 \
  sqlite-graphrag remember --graph-stdin < payload.json
```

- **Camada 1 (PATH)**: roteia qualquer `claude -p` ou `codex exec`
  spawned via a mock CLI determinística commitada em
  `tests/mock-llm/`. O subprocesso LLM real é desviado; SIGINT não
  consegue matar um subprocesso que não existe. É a camada mais
  barata e o default certo em CI.
- **Camada 2 (env)**: faz o `if should_obey_shutdown()` do embedder
  curto-circuitar para `true`, então o braço de cancelamento do
  `tokio::select!` é descartado e o batch roda até a conclusão
  mesmo se o cancellation token já estiver cancelled. Zero
  overhead em produção porque a leitura da env é um único
  `std::env::var` por chamada de `should_obey_shutdown()`, não
  em hot path.
- **Camada 3 (setsid)**: dá à CLI seu próprio grupo de processos via
  `setsid -w`, então SIGINT do harness pai não se propaga para o
  filho. `timeout` adiciona um teto rígido de wall-clock (binário
  Rust `timeout-cli` v0.1.0, somente inteiros em segundos —
  `600` é 10 minutos; não passe `10m`).

A receita é agora a referência canônica para qualquer harness de
agente rodando jobs longos de embedding em background. O bypass é
explicitamente opt-in: código de produção NUNCA deve chamar
`try_reset_shutdown()`, e a env var NUNCA deve ser setada em
produção. Tests e invocações de auditoria são os únicos
consumidores válidos.

Se a execução for interrompida entre as camadas, o arquivo SQLite
permanece consistente (WAL, commit atômico, sem escritas
parciais), e `restore` ou `enrich --operation re-embed --resume`
podem retomar a partir da última memória bem-sucedida.

## Camada de Validação Pre-Flight em Invocação Headless (v1.0.87, ADR-0045, GAP-META-005)
- O módulo `src/spawn/preflight.rs` (≥200 linhas, 7 guards, 15 testes unitários) porta todo spawn de subprocesso LLM ANTES do fork
- Os 7 guards em ordem: `check_argv_size`, `check_binary_exists`, `check_mcp_config_inline`, `check_mcp_config_path`, `check_walkup_mcp_json`, `check_output_buffer`, `check_claude_config_dir`
- Falhas retornam `AppError::PreFlightFailed(PreFlightError)` com `exit_code() == 16` (`EX_CONFIG`, `is_permanent() == true`)
- A variante `McpConfigInlineJsonRejected` (Bug 2 do GAP-META-005) é crítica em invocação headless: Claude Code 2.1.177 rejeita `--mcp-config '{}'` literal. O preflight substitui automaticamente por tempfile com `{"mcpServers":{}}`
- A variante `WalkUpMcpJsonInvalid` (Bug 5) detecta `.mcp.json` inválido em diretórios ancestrais do CWD — walk-up de até 16 níveis via `std::path::Path::ancestors()`
- A variante `ArgvExceedsArgMax` (Bug 3) protege contra `E2BIG` pós-fork para corpos de memória grandes. Threshold: `ARG_MAX - 4096` bytes (margem de 4 KB para env vars do kernel)
- A variante `BinaryNotFound` verifica que `claude` ou `codex` está em PATH antes do fork. Usa `which::which` em POSIX e `where` em Windows
- Bypass em emergências: `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1` desabilita todos os 7 guards. Bypass reverte para `Command::spawn()` direto e herda todas as 5 classes BUG do GAP-META-005
- O preflight compartilha o helper `apply_env_whitelist` (ADR-0041) — ordem de execução: env_clear primeiro, depois preflight
- Cada spawner adiciona uma única linha antes de `cmd.spawn()`: `preflight_check(&PreFlightArgs { ... }).map_err(|e| AppError::PreFlightFailed(e))?`
- Telemetria: `tracing::info!(event = "preflight_passed", spawner = %name, argv_bytes = total)` em sucesso; `tracing::warn!(event = "preflight_failed", spawner = %name, error = %e)` em falha
- Veja `docs/decisions/adr-0045-preflight-validation-layer.md` (en + pt-BR) para a justificativa arquitetural completa

## Hotfixes BUG-11/12/13 em Invocação Headless (v1.0.88, ADR-0046, ADR-0047)
- **BUG-11 (CRÍTICO)**: falha pre-flight em `extract/llm_embedding.rs:563-565` não propagava para `remember`, que silenciosamente persistia a memória com `backend_invoked: "none"` e zero chunks. Corrigido com `embed_via_backend_strict`. Repro: `CLAUDE_CONFIG_DIR=/tmp/bad-config-with-mcp remember --name X --type note --description x --body y` retorna exit 11 + envelope JSON de erro
- **BUG-12 (MÉDIO)**: enforço OAuth-only emitia 2 linhas stderr idênticas (uma de `tracing::error!`, uma de `eprintln!`). Corrigido removendo `eprintln!` duplicado em `src/output.rs`. Teste: `oauth_stderr_emits_single_line_v1088`. Repro: `ANTHROPIC_API_KEY=sk-test /path/bin/sqlite-graphrag init` agora emite 1 linha stderr (eram 2)
- **BUG-13 (MÉDIO)**: `link --create-missing` bypassava validação de nome de entidade. Corrigido validando ANTES de normalizar em `src/commands/link.rs`. 8 testes em `tests/entity_validation_integration.rs`
- Nova variante `AppError::PreFlightFailed(PreFlightError)` com `exit_code() == 16` e `is_permanent() == true`. Substitui os 3 spawners chamando `std::process::exit(16)` diretamente
- Veja `docs/decisions/adr-0046-preflight-remediation.md` e `adr-0047-stderr-deduplication.md` (en + pt-BR)

## Schema Drift e Flag Parity para Agentes Headless (v1.0.89, ADR-0048, ADR-0049)
- `health.schema.json` regenerado via `schemars 0.8` derive macro. 17 novos campos adicionados. `additionalProperties: true` (política Must-Ignore por RFC 7493 I-JSON)
- Agentes que validam resposta de `health --json` devem migrar de `additionalProperties: false` (strict) para Must-Ignore para receber benefícios de evolução de schema
- `--db <PATH>` agora aceito em `embedding status`, `embedding list`, `embedding abandon`, `pending list`, `pending show` — operadores headless podem apontar para múltiplos bancos sem flag global
- `codex-models --json` retorna envelope JSON `{"action":"codex_models","count":N,"default":"...","models":[...]}`
- `migrate --dry-run --json` reporta migrações pendentes sem aplicar. Adicionado `--confirm` para exigir confirmação literal antes de apply
- `ingest --auto-describe` (padrão true) extrai descrição da primeira linha significativa do corpo. Substitui a antiga `"ingested from <path>"` genérica
- `health --namespace <NS> --json` filtra contagens para um único namespace — útil em ambientes multi-tenant
- Binário medido em 15.323.128 bytes (14.6 MiB), dentro de 1 MiB do documentado em `Cargo.toml:6`. Drift viral "6 MB" eliminado
- 1877 testes passando (843 lib + 1013 integração + 21 doc)

## Atualização v1.0.93 — Backend de Embedding OpenRouter
- Nova flag `--embedding-backend openrouter` habilita embedding via REST API sem subprocesso LLM
- Elimina overhead de cold-start: ~200ms por embedding vs 15s com subprocesso
- Requer variável de ambiente `OPENROUTER_API_KEY` ou flag `--openrouter-api-key`
- Requer `--embedding-model MODEL` (sem padrão — o usuário deve especificar)
- Funciona com todos os 8 comandos de embedding no modo headless
- Exemplo de invocação headless:

```bash
OPENROUTER_API_KEY="$KEY" sqlite-graphrag \
  --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  ingest ./docs --pattern "*.md" --recursive --json
```

## Atualização v1.0.90 — OpenCode como Terceiro Backend LLM (ADR-0051)

A v1.0.90 adiciona o OpenCode como terceiro backend LLM para pipelines
de embedding, ingestão e enriquecimento. A prioridade de auto-detect
agora é `codex > claude > opencode > none`. A cadeia de fallback
padrão é `codex,claude,opencode,none`.

### sqlite-graphrag com backend opencode

```bash
# Forçar backend opencode com modelo específico
sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle \
  remember --name example --type note --body "text" --json

# Ingestão com extração opencode
sqlite-graphrag ingest ./docs --mode opencode --recursive --json

# Enriquecimento com opencode
sqlite-graphrag enrich --operation memory-bindings --mode opencode --json
```

### Novas variáveis de ambiente para opencode

- `SQLITE_GRAPHRAG_OPENCODE_BINARY` — sobrescreve o caminho do binário opencode
- `SQLITE_GRAPHRAG_OPENCODE_MODEL` — seleciona o modelo opencode para extração
- `SQLITE_GRAPHRAG_OPENCODE_EMBED_MODEL` — seleciona o modelo opencode para embedding
- Precedência: `OPENCODE_EMBED_MODEL > OPENCODE_MODEL > default opencode/big-pickle`
- Correção de contaminação cruzada (BUG-AUDIT-001): resolução de modelo opencode NÃO faz fallback para `SQLITE_GRAPHRAG_LLM_MODEL`

## Atualização v1.0.89 — Propagação de Flags LLM e Seleção de Modelo (ADR-0050)

A v1.0.89 corrige uma classe crítica de bugs de flag morta: 7 flags
globais de CLI eram aceitas pelo clap mas nunca propagadas para os
módulos internos de embedding. Todas as 7 agora funcionam via CLI ou
variável de ambiente.

### Flags globais novas e corrigidas

- `--llm-model <MODEL>` / `SQLITE_GRAPHRAG_LLM_MODEL` — seleciona o
  modelo de embedding. Padrões: `gpt-5.5` (codex), `claude-sonnet-4-6`
  (claude). Sobrescreve as variáveis por backend
  `SQLITE_GRAPHRAG_CODEX_EMBED_MODEL` e
  `SQLITE_GRAPHRAG_CLAUDE_EMBED_MODEL`
- `--llm-backend <auto|codex|claude|none>` /
  `SQLITE_GRAPHRAG_LLM_BACKEND` — seleciona qual CLI spawna o
  subprocesso de embedding. `auto` (padrão) sonda o PATH: codex
  primeiro, depois claude
- `--codex-binary <PATH>` / `SQLITE_GRAPHRAG_CODEX_BINARY` —
  sobrescreve a localização do binário codex (novo na v1.0.89;
  `--claude-binary` existe desde a v1.0.82)
- `--llm-fallback <chain>` / `SQLITE_GRAPHRAG_LLM_FALLBACK` — cadeia
  de fallback quando o backend primário falha (padrão:
  `codex,claude,none`)
- `--skip-embedding-on-failure` /
  `SQLITE_GRAPHRAG_SKIP_EMBEDDING_ON_FAILURE` — persiste a memória sem
  embedding quando o LLM falha (exit 0 em vez de exit 11)
- `--llm-max-host-concurrency <N>` /
  `SQLITE_GRAPHRAG_LLM_MAX_HOST_CONCURRENCY` — limita os subprocessos
  LLM concorrentes em todo o host
- `--llm-slot-wait-secs <N>` / `SQLITE_GRAPHRAG_LLM_SLOT_WAIT_SECS` —
  segundos para esperar por um slot livre antes de falhar
- `--llm-slot-no-wait` / `SQLITE_GRAPHRAG_LLM_SLOT_NO_WAIT` — falha
  imediatamente se nenhum slot estiver disponível

### BoolishValueParser para env vars booleanas

Flags booleanas com `env = "SQLITE_GRAPHRAG_*"` agora aceitam `1`,
`yes`, `on`, `true` (e `0`, `no`, `off`, `false`). Antes só
`true`/`false` eram aceitos, causando exit 2 para scripts que setavam
`SQLITE_GRAPHRAG_SKIP_EMBEDDING_ON_FAILURE=1`.

### Invocação headless com modelo explícito

```bash
# Claude com modelo explícito
claude -p "SUA TAREFA" \
  --model claude-sonnet-4-6 \
  --strict-mcp-config --mcp-config '{}' \
  --dangerously-skip-permissions \
  --settings '{"hooks":{}}' \
  --output-format json

# Codex com modelo explícito
codex exec \
  --model gpt-5.5 \
  -c mcp_servers='{}' \
  --sandbox workspace-write \
  --ask-for-approval never \
  "SUA TAREFA"
```

### sqlite-graphrag com override de backend e modelo

```bash
# Força o backend claude com modelo específico
sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 \
  remember --name example --type note --body "text" --json

# Força o backend codex com modelo específico
sqlite-graphrag --llm-backend codex --llm-model gpt-5.5 \
  recall "query" --k 5 --json

# Pula o embedding em caso de falha (persiste a memória sem vetor)
sqlite-graphrag --skip-embedding-on-failure \
  remember --name resilient --type note --body "text" --json
```


## Referências Externas Validadas

### Claude Code

- `code.claude.com/docs/en/headless` — modo headless e exit codes claros
- `amux.io/guides/claude-code-headless/` — guia completo de self-hosting headless (2026)
- `github.com/anthropics/claude-code/issues/39069` — `--bare` mode skips OAuth/keychain, unusable para OAuth-only
- `computingforgeeks.com/claude-code-cheat-sheet/` — cheat sheet com `--mcp-config` e `--strict-mcp-config`
- `github.com/anthropics/claude-code/issues/14490` — `--strict-mcp-config` não sobrescreve `disabledMcpServers`

### Codex CLI

- `developers.openai.com/codex/cli/reference` — referência canônica de CLI options
- `deepwiki.com/openai/codex/6.1-mcp-server-configuration` — MCP server config no `config.toml`
- `ofox.ai/blog/codex-cli-config-toml-deep-dive/` — cada setting do `config.toml` explicado
- `github.com/openai/codex/issues/3441` — bug de `[mcp_servers]` não funcionar em versão antiga do Codex

### OpenCode

- `opencode.ai/docs/mcp-servers/` — controle de MCP via `enabled: false` por servidor
- `open-code.ai/en/docs/config` — referência de `opencode.json` com providers, models, MCP
- `computingforgeeks.com/opencode-cli-cheat-sheet/` — cheat sheet com flags headless e MCP
