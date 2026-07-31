---
name: sqlite-graphrag
description: Esta skill DEVE ativar para toda operação da CLI sqlite-graphrag cobrindo memória GraphRAG hybrid-search recall deep-research -o remember enqueue-enrich entities_created enrich_recommended remember-batch ingest edit restore enrich force-redescribe re-embed entity-connect memory-entities merge-entities link purge config XDG OpenRouter codex claude opencode isolamento de namespace claim until-empty resume retry-failed debug-schema. Esta skill DEVE ser usada sempre que o agente armazena recupera busca enriquece liga mescla ou mantém memória GraphRAG de longo prazo. Palavras-chave sqlite-graphrag GraphRAG memory embedding openrouter codex claude opencode remember hybrid-search enrich force-redescribe re-embed config XDG pending embedding slots fts vec
---

## Quando Esta Skill Ativa
- DEVE ATIVAR para remember/salvar/recall/recuperar/buscar/persistir entre sessões; GraphRAG, grafo de conhecimento, ligação de entidades, memória por namespace; quando sqlite-graphrag, embedding, FTS5, hybrid-search, OpenRouter, codex, claude, opencode, entity-connect ou memória LLM for mencionado; para enrich, re-embed, link, unlink, merge-entities, rename-entity, deep-research, ingest, config, debug-schema, manutenção de grafo
- NUNCA ATIVE para dados efêmeros, I/O simples de arquivo ou tarefas sem memória; SEMPRE carregue esta skill antes de inventar arquivos de memória ad-hoc, servidores MCP de memória ou diários Markdown

## Modelo Mental Central
- SAIBA TRÊS seletores independentes; NUNCA os confunda
- SELETOR 1 — `--embedding-backend` COMO os vetores são produzidos — `openrouter` (REST), `llm` (subprocesso) ou `auto`
- SELETOR 2 — `--llm-backend` QUAL subprocesso embeda quando backend é `llm` — `codex`, `claude`, `opencode` ou `none`
- SELETOR 3 — extração via `enrich --mode` — `codex`, `claude-code`, `opencode` ou `openrouter` (REST chat completions); `--extraction-backend` é o seletor global relacionado
- ESCREVER e ENRIQUECER são SEMPRE processos separados; a escrita produz embeddings; o `enrich` SEPARADO extrai ou muta o grafo; NUNCA encadeie escrita e enrich com `&&`; SEMPRE aguarde exit 0 da escrita e só então execute enrich como processo DISTINTO
- Em TODA escrita OpenRouter (`remember`, `remember-batch`, `ingest`, `edit`, `restore`) DEVE passar `--llm-backend none` + `--embedding-backend openrouter` + `--embedding-model <MODEL>` + `--embedding-dim 1024` para embeddings via OpenRouter REST sem timeout de subprocesso LLM
- SEMPRE passe `--json`; SEMPRE faça parse com `jaq` NUNCA `jq`; SEMPRE capture stdout PRIMEIRO e só depois parse; NUNCA encadeie a saída da CLI direto em `jaq` (NDJSON mascara falhas como null)
- SAIBA que vetores vazios NUNCA são persistidos; FAÇA parse de `backend_invoked`; EXECUTE `enrich` somente após exit 0 da escrita
- SEMPRE mantenha `--embedding-dim 1024` idêntico em TODOS os caminhos de embed de escrita e leitura; dimensão divergente falha knn com exit 11
- Dimensão PADRÃO é **1024**; precedência SEMPRE flag > XDG `config set` > default; PROIBIDO product env `SQLITE_GRAPHRAG_*` no hot path

## Regras de Instrução de Prompt
- "lembre isso" → `remember --force-merge` com `--graph-stdin` de entidades e relações canônicas, depois `enrich` SEPARADO
- "o que você sabe sobre X" → `hybrid-search "X" --k 10 --json` PRIMEIRO, depois `read --name <name> --json`
- "como X se relaciona com Y" → `graph traverse --from X --depth 2 --json` ou `related X --hops 2 --json`; em miss DEVE RETENTAR com `--fuzzy` ou usar sugestões do exit 4 NotFound
- "pesquisa profunda sobre X" → `deep-research "X" --k 20 --max-hops 3 --json`; envelopes grandes DEVEM usar `--output PATH` ou `-o PATH` e `--quiet`
- "conecte entidades isoladas" → `enrich --operation entity-connect` com `--mode` + modelo obrigatórios, depois monitore `--status`
- ANTES de qualquer criação → `hybrid-search "<name>" --k 5 --json`; se houver duplicata DEVE USAR `--force-merge`
- DEPOIS de criar/atualizar → parse `read --name <name> --json` para `{name, description, body_length}`; DEPOIS de cada turno → persista achados ou DECLARE "No new findings to persist"
- Em exit ≠0 → parse `jaq '{code, message, error_class}'` e REPORTE remediação
- SEMPRE relações canônicas — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- SEMPRE mapeie não-canônicas — `adds`/`creates`→`causes`, `implements`→`supports`, `blocks`→`contradicts`, `tested-by`→`related`, `part-of`→`applies-to`
- SEMPRE nomes kebab-case ASCII minúsculo; LIMITE a conceitos de domínio; REJEITE genéricos, pronomes, UUIDs, timestamps
- NUNCA MCP Serena, `.md` de memória ou MEMORY.md; NUNCA daemon; NUNCA `ANTHROPIC_API_KEY`/`OPENAI_API_KEY` a backends subprocesso
- DEVE `remember --force-merge` para updates idempotentes; DEVE `--graph-stdin` ou `--graph-file` com grafo curado

## Contrato (db, flags, exits)
- `--db <PATH>` DEVE vir DEPOIS do verbo sempre — `sqlite-graphrag remember --db ./g.sqlite ...`; ANTES do verbo é REJEITADO; default persistente via `config set db.path <PATH>`
- Superfícies de grafo EXIGEM e USAM `--db`; folhas host/XDG (`config`, `slots`, `cache`, `codex-models`, `completions`) aceitam `--db` como no-op documentado
- SEMPRE `--json`; SEMPRE `--quiet`/`-q` em pipelines headless; NUNCA misture stderr no JSON com `&>` ou `2>&1`
- Precedência de chave OBRIGATÓRIA — flag CLI > XDG `config set` / `config add-key` > default; PROIBIDO product env como primária
- EXIT codes — 0 sucesso; 1 validação OU Timeout (EC InterruptHandle — NÃO 75); 2 args; 3 lock otimista; 4 not found (sugestões sem `--fuzzy`); 5 namespace; 6 payload grande (DIVIDA corpo); 9 duplicata (`--force-merge`); 10 banco (`vacuum`+`health`); 11 embedding (backend/dim/chave); 13 batch parcial (reprocesse só falhos); 14 I/O; 15 busy (amplie `--wait-lock`); 16 preflight (corrija MCP; NUNCA transitório); 19 SHUTDOWN (retry OBRIGATÓRIO); 20 interno; 75 singleton locked (NUNCA retente já); 77 RAM; 78 config (chave/modelo ausente)
- NUNCA ignore non-zero; NUNCA reprocessar batch inteiro após exit 13; NUNCA confunda exit 1 Timeout com exit 75 ou exit 9

## Arquitetura
- INVOQUE como subprocesso; stdout = JSON/NDJSON; stderr = logs; VERIFIQUE o exit code ANTES do parse; SEM daemon, SEM ONNX, SEM cache de modelo; cosine é Rust puro sobre BLOB `memory_embeddings` / `entity_embeddings` / `chunk_embeddings`; FUSÃO é FTS5 BM25 mais cosine KNN BLOB via RRF
- SAIBA que `init` ou `migrate` aplica o schema vivo; LEIA `schema_version` em `health --json`; INSPECCIONE schema com `debug-schema --json` quando necessário
- IMPONHA OAUTH-ONLY para codex/claude — o spawn ABORTA com exit 1 se `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` estiverem definidos; `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` são PRESERVADOS
- SAIBA que o CWD do subprocesso é ISOLADO; 7 guards de preflight antes de cada fork LLM; exit 16 = falha de preflight; `claude -p` herda `.mcp.json` do CWD — DEVE ISOLAR config para `claude-code` ou DEVE usar codex
- DEFINA skip de preflight de emergência SOMENTE via `sqlite-graphrag config set spawn.skip_preflight=1` (SOMENTE EMERGÊNCIAS); namespace via `--namespace` ou XDG (padrão `global`)
- NUNCA exponha como MCP/HTTP; NUNCA escreva `.sqlite` com outra ferramenta

## Modelos OpenRouter
- PASSE `--embedding-model <MODEL>` quando `--embedding-backend openrouter`; NÃO há modelo padrão → exit 78 na omissão; preços indicativos USD por milhão de tokens; SEMPRE confirme ao vivo via `usage.cost` quando disponível
- Catálogo embed — `nvidia/llama-nemotron-embed-vl-1b-v2:free` GRATUITO; `qwen/qwen3-embedding-4b` $0.05/M; `qwen/qwen3-embedding-8b` $0.05/M PADRÃO operacional; `openai/text-embedding-3-small` $0.05/M; `perplexity/pplx-embed-v1-0.6b` $0.05/M; `baai/bge-m3` ~$0.05/M; `mistralai/mistral-embed-2312` $0.10/M; `google/gemini-embedding-2` ~$0.12/M; `openai/text-embedding-3-large` $0.13/M; `google/gemini-embedding-005` ~$0.15/M
- SAIBA que MRL trunca no servidor para `--embedding-dim` (padrão **1024**); dim divergente → exit 11
- openrouter propaga a TODOS os caminhos de embed — `remember` `remember-batch` `ingest` `recall` `edit` `restore` `hybrid-search` `deep-research` `enrich` `init` `rename-entity`
- OBRIGATÓRIO ADICIONE chave — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- LISTE — `config list-keys --json`; REMOVA — `config remove-key <fingerprint> --json`; DOCTOR — `config doctor --json`; PATH — `config path`
- Chaves em XDG `~/.config/sqlite-graphrag/config.toml` com `chmod 600`, zeroizadas no drop, NUNCA logadas
- NUNCA passe API key como argumento CLI no histórico de shell em produção; SEMPRE prefira `config add-key --from-stdin`; SEMPRE rode `config doctor` após adicionar chave antes de ops pagas
- Modelos de texto servem SOMENTE extração/enrich, NUNCA embedding; DEVE usar `openai/gpt-oss-120b` como judge PADRÃO; `:nitro` = provedor mais rápido a preço maior
- Catálogo texto — `deepseek/deepseek-v4-flash` 0.09/0.18; `deepseek/deepseek-v4-flash:nitro` 0.14/0.28; `deepseek/deepseek-v4-pro` 1.30/2.60; `google/gemini-3.1-flash-lite` 0.95/3.00; `minimax/minimax-m3` 0.30/1.20; `minimax/minimax-m2.7` 0.25/1.00; `minimax/minimax-m2.7:nitro` 0.30/1.20; `openai/gpt-oss-120b` 0.059/0.18; `openai/gpt-oss-120b:nitro` 0.15/0.60; `xiaomi/mimo-v2.5` 0.10/0.28; `xiaomi/mimo-v2.5-pro` 0.43/0.87; `z-ai/glm-5.2` e `z-ai/glm-5.2:nitro` confirme via `usage.cost`
- VERIFIQUE `json_schema` estrito ANTES de produção; sem Structured Outputs → erro OpenRouter explícito

## Backends LLM Headless
- SEMPRE passe a flag de modelo explicitamente; NUNCA confie só em defaults silenciosos
- CODEX — `enrich --mode codex --codex-model <MODEL>`; OAuth-only; padrão `gpt-5.5`; `codex login`; embedding `--llm-backend codex --llm-model <MODEL>`
- CLAUDE — `enrich --mode claude-code --claude-model <MODEL>`; OAuth-only; padrão `claude-sonnet-4-6`; embedding `--llm-backend claude --llm-model <MODEL>`
- OPENCODE — `enrich --mode opencode --opencode-model <MODEL>`; padrão `opencode/big-pickle`; embedding `--llm-backend opencode --llm-model <MODEL>`; auth própria (NÃO OAuth); `--opencode-model` NÃO validado — PASSE ids vivos do OpenCode Zen
- EXTRAÇÃO OPENROUTER — DEVE usar `enrich --mode openrouter --openrouter-model <id>`; `--openrouter-model` é OBRIGATÓRIO (sem default; ausência exit 1 antes de rede)
- SOBRESCREVA binários `--codex-binary`, `--claude-binary`, `--opencode-binary`; AJUSTE timeouts `--codex-timeout`, `--claude-timeout`, `--opencode-timeout`
- VALIDE codex com `--codex-model-validate` e `--codex-model-fallback <MODEL>`; LISTE com `codex-models --json` (CODEX apenas)
- TROQUE backend em rate limit com `enrich --fallback-mode codex` ou global `--llm-fallback codex,claude,none`
- SAIBA `--mode openrouter` é REST puro — SEM CLI local; fatura a chave OpenRouter armazenada

## Flags Globais
- `--db <PATH>` DEPOIS do verbo; `--namespace <ns>`; `--json` SEMPRE; `--lang en|pt`; `--tz <TIMEZONE>`
- `--embedding-backend auto|openrouter|llm`; `--embedding-model <MODEL>` OBRIGATÓRIO com openrouter; `--embedding-dim N` padrão 1024 MRL [8, 4096]
- `--openrouter-api-key <KEY>` PROIBIDO no histórico de shell em produção; prefira `config add-key --from-stdin`
- `--llm-backend codex|claude|opencode|none|auto`; `--llm-model <MODEL>`; `--llm-fallback <chain>`
- `--extraction-backend`; `--openrouter-model <MODEL>` OBRIGATÓRIO em `--mode openrouter`; `--openrouter-base-url`; `--openrouter-timeout` padrão 600
- `--llm-parallelism N` fan-out embed padrão 4 clamp [1, 32]; `--rest-concurrency N` fan-out enrich openrouter clamp [1, 16] padrão 8; flags DISTINTAS
- `--max-concurrency N` clamp [1, 2×nCPUs]; `--llm-max-host-concurrency N`; `--llm-slot-wait-secs N` / `--llm-slot-no-wait`; `--wait-lock SECS`; `--low-memory`; `--strict-env-clear`; `--graceful-shutdown-secs N`; `--skip-embedding-on-failure`
- `--codex-binary`, `--claude-binary`, `--opencode-binary`; `-v`/`-vv`/`-vvv`; `--quiet`/`-q` OBRIGATÓRIO em pipelines headless

## Catálogo Completo de Comandos
- TOP-LEVEL — `init` `remember` `remember-batch` `ingest` `recall` `read` `list` `forget` `purge` `rename` `split-body` `edit` `history` `restore` `hybrid-search` `health` `migrate` `namespace-detect` `optimize` `stats` `sync-safe-copy` `backup` `vacuum` `link` `unlink` `deep-research` `related` `graph` `export` `fts` `vec` `codex-models` `prune-relations` `prune-ner` `slots` `pending` `embedding` `pending-embeddings` `cleanup-orphans` `memory-entities` `cache` `delete-entity` `reclassify` `rename-entity` `merge-entities` `enrich` `reclassify-relation` `normalize-entities` `completions` `config` `debug-schema` `help`
- Família `graph` — `graph traverse` `graph stats` `graph entities` `graph recompute-degree` mais export de snapshot `--format json|dot|mermaid|ndjson --output`
- Família `config` — `config add-key` `config list-keys` `config remove-key` `config doctor` `config path` `config set` `config get` `config list` `config unset`
- Família `fts` — `fts rebuild` `fts check` `fts stats`
- Família `vec` — `vec orphan-list` `vec purge-orphan` `vec stats`
- Família `slots` — `slots status` `slots release` `slots cleanup`
- Família `pending` — `pending list` `pending show` `pending cleanup`
- Família `embedding` — `embedding status` `embedding list` `embedding abandon`
- Família `pending-embeddings` — `pending-embeddings list` `pending-embeddings status` `pending-embeddings abandon` (aliases de embedding)
- Família `cache` — `cache clear-models` `cache list` `cache stats`
- `completions` — `completions bash|zsh|fish|elvish|powershell`
- `debug-schema` — inspeciona schema vivo; `help` — ajuda top-level e por verbo

## CRUD Escrita / Leitura-Atualização-Exclusão
### Escrita
- INVOQUE `remember --name <kebab> --type <kind> --description <text>` com exatamente uma fonte de corpo — `--body` ou `--body-file` ou `--body-stdin` ou `--graph-stdin`
- INVOQUE `remember --graph-stdin` para `{body, entities, relationships}`; ou `--graph-file` com `--body-file`
- PASSE entidades `[{name, entity_type}]` kebab-case ASCII; relações `[{source, target, relation, strength}]` strength [0.0, 1.0]
- OBRIGATÓRIO allowlist graph-stdin — SÓ chaves `name`, `entity_type` (alias `type` dobrado), `description` opcional; PROIBIDO `observations`, `aliases`, extras livres → exit 1
- PASSE `--strict-name`; `--force-merge` para updates idempotentes; `--replace-graph` com `--force-merge`; `--dry-run` para validar sem persistir
- PASSE `--enqueue-enrich` em `remember` SOMENTE para entity-descriptions prioritárias após escrita; padrão OFF
- FAÇA parse do JSON de remember para `entities_created[]` e `enrich_recommended[]`; NUNCA ignore; QUANDO `enrich_recommended` não vazio DEVE rodar SEPARADO `enrich --operation entity-descriptions` APÓS exit 0 da escrita
- VALORES `--type` — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- INVOQUE `remember-batch` para 10+ memórias via NDJSON stdin; PASSE `--transaction`; cada linha de create DEVE incluir `description` não vazia e `type`
- INVOQUE `ingest <DIR> --recursive --pattern "*.md" --mode none` para import body-only, depois enrich SEPARADO; `ingest --mode` aceita `none` (padrão), `claude-code`, `codex`, `opencode`
- USE `--resume`; `--retry-failed`; `--auto-describe`; `--name-prefix <prefix>`; `--force-merge` (dedup por `body_hash`); ingest auto-divide corpos oversized
- INVOQUE `split-body --name <N>` para UMA memória acima de 25000 chars; PASSE `--batch --threshold 25000` para todas oversized; FILHAS NÃO EMBEDADAS INLINE — passo1 openrouter embed + `--llm-backend none` em `split-body`; passo2 SEPARADO `enrich --operation re-embed --target memories`
- RESPEITE 512000 bytes e 512 chunks por corpo; NUNCA misture fontes de corpo; NUNCA `fd | xargs remember` — USE `ingest`
- NUNCA passe `--llm-backend` diferente de `none` na escrita OpenRouter; SEMPRE passe `--llm-backend none`
### Leitura Atualização Exclusão
- INVOQUE `read --name <kebab> --json`; PASSE `--with-graph`; USE `--format raw` para corpo puro
- INVOQUE `list --type <kind> --limit N --offset N --json`; `history --name <n> --diff --json`
- INVOQUE `edit --name <n> --body-file <path>` ou `--description` / `--memory-type`; USE `--force-reembed`; USE `--expected-updated-at <ts>` (exit 3 = conflito — recarregue e retente)
- INVOQUE `rename --name <old> --new-name <new>`; `restore --name <n> --version <N>` (caminho de escrita — OpenRouter embed + `--llm-backend none`, depois enrich SEPARADO)
- INVOQUE `forget --name <n>`; hard-delete `purge --yes --dry-run` depois remova `--dry-run`
- OBRIGATÓRIO — `purge --yes` sozinho mantém retenção 90 dias; para wipe imediato `purge --yes --now` (alias `--retention-days 0`)
- SEMPRE dry-run primeiro `purge --now --dry-run --json`; depois `cleanup-orphans --yes` e `vacuum --json`
- NUNCA pule optimistic locking; NUNCA delete via shell `sqlite3`

## Grafo de Entidades
- INVOQUE `link --from <a> --to <b> --relation <type> --create-missing --weight <float>`; DEVE usar `link --from-id <N> --to-id <M>` quando IDs forem conhecidos; NUNCA dígitos puros como nomes `--from`/`--to`
- INVOQUE `unlink --from <a> --to <b> --relation <type>` ou `--entity <name> --all`; `unlink --memory <name> --entity <name>` para binding único
- INVOQUE `graph entities --json` via `.entities[]` (NÃO `.items[]`); ORDENE `--sort-by name|degree|created-at`; PAGINE `--limit`/`--offset`
- INVOQUE `graph stats --json`; `graph traverse --from <root> --depth <N> --json`; EXPORTE `--format json|dot|mermaid --output <path>`
- DEVE passar `--fuzzy` em traverse com nome curto ambíguo; SEM `--fuzzy`, exit 4 inclui sugestões — SEMPRE use-as
- INVOQUE `rename-entity --name <old> --new-name <new>` ou `--id <N> --new-name <new>`
- INVOQUE `delete-entity --name <n> --cascade`; `merge-entities --names "a,b,c" --into <target>` ou `--ids 12,17 --into-id 3`
- NUNCA coloque `--into-id` dentro de `--ids` nem `--into` dentro de `--names`; merges auto-referenciais REJEITADOS ANTES do DB; SEMPRE USE arrays de shell para listas dinâmicas; PASSE `--cross-namespace` só quando intencional
- INVOQUE `reclassify --name <n> --new-type <kind>` ou `--from-type <old> --to-type <new> --batch`
- INVOQUE `reclassify-relation --from-relation <old> --to-relation <new> --batch`; PASSE `--literal-from`/`--literal-to` para match literal
- INVOQUE `prune-relations --relation mentions --dry-run` depois remova `--dry-run` com `--yes`; `normalize-entities --yes`; `prune-ner --entity <n>` ou `--all --yes`
- INVOQUE `memory-entities --name <memory>` ou `--entity <name>`; FAÇA parse de `entities[].{name, description, entity_type}` — `description` OBRIGATÓRIA no envelope (string vazia se ausente); SEMPRE apresente `description` quando existir
- INVOQUE `graph recompute-degree --json` após delete/merge/prune (grau NÃO é auto-recomputado)
- TIPOS canônicos de entidade — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- VALIDE nomes — mín 2 chars, sem newlines, sem ALL_CAPS ≤4, REJEITE dígitos puros; NUNCA use `mentions` como relação padrão; escritas ADITIVAS sem teto de grau

## Busca GraphRAG
- USE padrão de três camadas — `hybrid-search` depois `read --name` depois `related` ou `graph traverse`
- INVOQUE `recall <query> --k N` para KNN semântico puro; PASSE `--no-graph`, `--precise`, `--max-distance <f>`, `--max-graph-results N`, `--all-namespaces`
- INVOQUE `hybrid-search <query> --k N` para FTS5 mais KNN RRF; PASSE `--rrf-k 60`; `--weight-vec 1.0 --weight-fts 1.0`; `--fallback-fts-only`; USE `--with-graph --max-hops 2 --min-weight 0.3`; LEIA `results[]` E `graph_matches[]`
- INVOQUE `related <name> --hops N --relation <type>`
- INVOQUE `deep-research "<query>" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies`; consultas de um token expandem; controle manual PASSE `--sub-query-strategy manual --sub-queries-file PATH`
- ESCREVA envelopes grandes com `--output PATH` ou `-o PATH` (atomwrite); FAÇA parse do ack `{written, bytes, blake3}`; PASSE `--quiet`; NUNCA `&>`; quando `-o`/`--output` o arquivo DEVE existir com bytes > 0 após exit 0
- AJUSTE com `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--max-cost-usd`, `--timeout`
- FAÇA parse `recall` → `results[].{name, snippet, distance, score, source}`; `hybrid-search` → `results[].{name, combined_score, vec_rank, fts_rank}`; `deep-research` → `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context`, `stats`
- NUNCA confunda `distance` com `combined_score`; NUNCA aumente hops sem `graph stats` primeiro

## Regras do Pipeline Enrich
- INVOQUE `enrich --operation <op> --mode <backend>` — AMBAS OBRIGATÓRIAS para ops LLM; omitir `--mode` → exit 2; EXCETO inspetores read-only `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` e `--dry-run` (mode opcional)
- Ops que PERSISTEM — `memory-bindings`, `augment-bindings` (EXIGE `--names`/`--memory-names`/`--names-file`), `entity-descriptions`, `body-enrich`, `re-embed`, `weight-calibrate`, `relation-reclassify`, `entity-connect`, `entity-type-validate`, `description-enrich`, `cross-domain-bridges`, `domain-classify`, `deep-research-synth`, `body-extract` + `--body-extract-graph-only`; SCAN/REPORT — `graph-audit`
- `--mode` válidos — `codex`, `claude-code`, `opencode`, `openrouter`; PASSE flag de modelo correspondente; `--mode openrouter` exige `--openrouter-model`
- OBRIGATÓRIO filtros de nome — prefira `--entity-names a,b` para ops por entidade e `--memory-names a,b` para ops por memória; `--names` é alias BC; empty-match DEVE exibir `matched=0` + `hint` e PARAR
- Claim e until-empty são escopados a operation+namespace — `dequeue_next_pending`, `count_eligible_pending`, `--resume`/`--retry-failed` EXIGEM a `operation` selecionada E o `namespace` ativo; um drain em `ai-sdd` NÃO DEVE claimar nem contar linhas de `global`/ns vazio; `--until-empty` conta SOMENTE esta op+namespace (NUNCA todo pending de todas as ops)
- `--force-redescribe` em `entity-descriptions` reabre linhas `skipped`/`done` correspondentes para `pending` UMA VEZ por processo antes do primeiro enqueue para `INSERT OR IGNORE` não ser no-op silencioso; NUNCA reabre `dead` (use `--requeue-dead`); padrão write-once para descriptions não vazias
- Marcadores de baixa qualidade são SOMENTE COMPOSTOS (ex. `is a configuration file`, `is a software component`) — frases de domínio bare como `configuration file` sozinhas NÃO DEVEM acionar force-redescribe
- Elegibilidade de re-embed usa comprimento do BLOB `LENGTH(embedding)=dim*4`, NÃO só a coluna `dim` — linhas CORRUPT/META_AHEAD (dim=1024 com BLOB 384-d) permanecem elegíveis; `reconcile_satisfied_reembed_pending` marca pending ReEmbed como `done` quando já existe vetor vivo na dim ativa, limpando zumbis sem chamar a API
- Enqueue valida chaves de re-embed — `entity:{name}` remove o prefixo `entity:` no lookup; nomes bare continuam ok; entidades ausentes REJEITADAS; chaves de chunk validam se o `chunk_id` existe em memória não-deletada do namespace alvo
- PASSE `--target memories|entities|chunks|all` só em `re-embed` (padrão `memories`); PASSE `--limit N --resume`; `--retry-failed`; `--dry-run`
- PASSE `--quality-sample N` com `--status` para `quality_pct` e `scan_backlog_low_grounding_est` (flag > XDG `enrich.entity_description.quality_sample` > padrão 50; `0` desliga)
- Isolamento de fila — drain só da `operation` selecionada; ops memory-only NÃO DEVEM claimar chaves `pair:`/`entity:`/`chunk:`; `state` = `draining`|`cooldown`|`pending-scan`|`blocked_dead`; `blocked_dead` → `--list-dead`/`--requeue-dead`/prune PRIMEIRO
- NUNCA rode múltiplos `enrich` no mesmo DB; paralelismo REST é SOMENTE `--rest-concurrency` dentro de UM processo
- PASSE `--until-empty` para loop scan→drain até vazio ou `--max-runtime` (padrão 3600); PASSE `--max-attempts <N>` padrão 8 faixa 1..=20
- PASSE `--status` para `scan_backlog`, `unbound_backlog`, contagens de fila, `eligible_now`, `waiting`, `quality_pct`, `state` — SEM LLM, SEM singleton
- DISTINGA — `scan_backlog` = candidatos DB que um scan fresco ENFILEIRARIA; `queue_pending` = contagem do sidecar; `eligible_now == 0` com `queue_pending > 0` é COOLDOWN; `draining` travado → `--reset-stale-claims`
- Lista de ops compacta — PASSE `--list-dead`; `--requeue-dead`; `--list-skipped`; `--requeue-skipped` (recupera skipped/`preservation_failed` sem SQL cru); `--ignore-backoff`; `--prune-dead-orphans`; `--prune-dead-entity-orphans` (mutuamente exclusivos); `--reset-stale-claims` após `kill -9`
- SAIBA dead-letter Transient vs HardFailures; completions OpenRouter truncadas (`finish_reason`=`length`) reemitem com `max_tokens` AUMENTADO; fila é sidecar `.enrich-queue.sqlite`
- ENTITY-CONNECT PERSISTE arestas via `entity_connect_seen` com `related`|`none`; `cross-domain-bridges` usa o MESMO scan/drain; scan de pares é O(k) coocorrência + hub×grau-0 — NUNCA cartesiano completo; chaves `pair:{id1}:{id2}` `item_type=entity_pair`
- Primeiro scan coberto por `--max-runtime` e soft ~120s `InterruptHandle`; interrupt → Timeout exit **1** — NUNCA exit 75
- FAÇA parse de `budget_exhausted` (orçamento de runtime em namespaces grandes) e `preempted_for_gate` (EC cedeu para memory-bindings/entity-descriptions rodarem primeiro)
- PASSE `--anchor-memory <name>` e/ou `--entity-names a,b`; empty → `matched=0` + `hint`; SEMPRE `--until-empty` + inspecione `--status`; SEMPRE dry-run primeiro em corpora de produção
- Prioridade — memory-bindings depois entity-descriptions ANTES de entity-connect; drains longos de EC DEVEM ceder; linhas legadas sem `pair:` são ignoradas

## Matriz de Fórmulas Write→Enrich
### Setup de chave OpenRouter
- OBRIGATÓRIO adicione chave — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- LISTE — `sqlite-graphrag config list-keys --json`; DOCTOR — `sqlite-graphrag config doctor --json`; PATH — `sqlite-graphrag config path`
- OBRIGATÓRIO URLs — `config set network.openrouter.chat_url https://openrouter.ai/api/v1/chat/completions` e `config set network.openrouter.embeddings_url https://openrouter.ai/api/v1/embeddings`
### Prefixo W e paralelismo
- DEFINA PREFIXO W — `sqlite-graphrag --embedding-backend openrouter --embedding-model <EMB> --embedding-dim 1024 --llm-backend none`
- PADRÃO `<EMB>` = `qwen/qwen3-embedding-8b`; caminho GRATUITO `nvidia/llama-nemotron-embed-vl-1b-v2:free`
- ESCALE embed no PASSO 1 com `--llm-parallelism N` (clamp 1..32); NUNCA confunda com `--rest-concurrency`
- ESCALE enrich openrouter no PASSO 2 com `--rest-concurrency N` em UM único processo (clamp 1..16; padrão 8); modelos pagos DEVEM usar 4..16; `:free` limita ~20 req/min → N baixo OBRIGATÓRIO
- NUNCA lance N processos enrich; UM processo com `--rest-concurrency` é OBRIGATÓRIO; NUNCA encadeie PASSO 1 e PASSO 2 com `&&`
### Enrich PASSO 2 por modo (reuse em todos os verbos)
- openrouter — `sqlite-graphrag enrich --db ./g.sqlite --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- codex — `sqlite-graphrag enrich --db ./g.sqlite --operation memory-bindings --mode codex --codex-model gpt-5.5 --json`
- claude-code — `sqlite-graphrag enrich --db ./g.sqlite --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- opencode — `sqlite-graphrag enrich --db ./g.sqlite --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- QUANDO `enrich_recommended` tiver entity-descriptions → rode `enrich --operation entity-descriptions --mode <backend> --entity-names <list> --json` APÓS memory-bindings; SEMPRE com flag de modelo explícita do modo
### remember — PASSO 1 + PASSO 2
- PASSO 1 openrouter — `echo '{"body":"text","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | <PREFIXO W> remember --db ./g.sqlite --name <n> --type decision --description "desc" --graph-stdin --force-merge --json`
- PASSO 1 hot-set — o mesmo mais `--enqueue-enrich`; PASSO 1 paralelo — adicione `--llm-parallelism N`
- PASSO 1 com embed via llm codex — `sqlite-graphrag --embedding-backend llm --llm-backend codex --llm-model gpt-5.5 --embedding-dim 1024 remember --db ./g.sqlite --name <n> --type decision --description "desc" --body "text" --force-merge --json`
- PASSO 1 com embed via llm claude — `... --embedding-backend llm --llm-backend claude --llm-model claude-sonnet-4-6 ...`
- PASSO 1 com embed via llm opencode — `... --embedding-backend llm --llm-backend opencode --llm-model opencode/big-pickle ...`
- PASSO 2 — rode UM dos quatro modos de enrich acima; NUNCA no mesmo processo da escrita
### remember-batch — PASSO 1 + PASSO 2
- PASSO 1 openrouter — `<PREFIXO W> remember-batch --db ./g.sqlite --transaction --json` com NDJSON stdin; cada create DEVE ter `description` e `type` não vazios
- PASSO 1 paralelo — adicione `--llm-parallelism N`; em exit 13 reprocesse SOMENTE falhos
- PASSO 1 codex/claude/opencode — troque para `--embedding-backend llm --llm-backend <codex|claude|opencode> --llm-model <MODEL>` sem `--llm-backend none`
- PASSO 2 — enrich nos 4 modos com flags de modelo explícitas; SEMPRE após exit 0 do batch
### ingest — PASSO 1 + PASSO 2
- PASSO 1 openrouter — `<PREFIXO W> ingest --db ./g.sqlite ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- PASSO 1 com retry — adicione `--retry-failed`; PASSO 1 paralelo — `--llm-parallelism N`
- PASSO 1 codex inline (job singleton) — `sqlite-graphrag --embedding-backend llm --llm-backend codex --llm-model gpt-5.5 ingest --db ./g.sqlite ./docs --mode codex --recursive --json` — ainda assim enrich SEPARADO se necessário
- PASSO 1 claude-code — `... --mode claude-code` com `--llm-backend claude --llm-model claude-sonnet-4-6`
- PASSO 1 opencode — `... --mode opencode` com `--llm-backend opencode --llm-model opencode/big-pickle`
- PASSO 2 preferido body-only — enrich openrouter/codex/claude-code/opencode com `--operation memory-bindings` depois `entity-descriptions`
### edit — PASSO 1 + PASSO 2
- PASSO 1 openrouter — `<PREFIXO W> edit --db ./g.sqlite --name <n> --body-file new.md --json`; force reembed — adicione `--force-reembed`
- PASSO 1 com lock — PASSE `--expected-updated-at <ts>`; exit 3 → recarregue e retente
- PASSO 1 codex/claude/opencode — `--embedding-backend llm --llm-backend <backend> --llm-model <MODEL>`
- PASSO 2 — enrich nos 4 modos; SEMPRE processo distinto
### restore — PASSO 1 + PASSO 2
- PASSO 1 openrouter — `<PREFIXO W> restore --db ./g.sqlite --name <n> --version 2 --json`
- PASSO 1 codex/claude/opencode — `--embedding-backend llm --llm-backend <backend> --llm-model <MODEL>`
- PASSO 2 — enrich nos 4 modos com modelo explícito; NUNCA `&&`
### Parallel enrich openrouter
- OBRIGATÓRIO um processo — `sqlite-graphrag enrich --db ./g.sqlite --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --rest-concurrency 8 --json`
- NUNCA N processos; REST fan-out SOMENTE via `--rest-concurrency N`

## Fórmulas de Leitura/Busca
- INIT — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 1024 init --db ./g.sqlite --namespace <ns>`
- HYBRID-SEARCH openrouter — `sqlite-graphrag --embedding-backend openrouter --embedding-model baai/bge-m3 --embedding-dim 1024 hybrid-search --db ./g.sqlite "query" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- HYBRID offline — `sqlite-graphrag hybrid-search --db ./g.sqlite "query" --k 10 --fallback-fts-only --json`
- RECALL openrouter — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 recall --db ./g.sqlite "query" --k 10 --json`
- DEEP-RESEARCH openrouter — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 deep-research --db ./g.sqlite "question" --k 20 --max-hops 3 -o /tmp/dr.json --json`
- RENAME-ENTITY openrouter — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 rename-entity --db ./g.sqlite --name <old> --new-name <new> --json`
- MEMORY-ENTITIES — `sqlite-graphrag memory-entities --db ./g.sqlite --name <memory> --json` e parse `entities[].description`
- TRAVERSE — `sqlite-graphrag graph traverse --db ./g.sqlite --from <root> --depth 2 --json`; fuzzy — adicione `--fuzzy`
- LINK — `sqlite-graphrag link --db ./g.sqlite --from <a> --to <b> --relation uses --json`; por ID — `link --from-id <N> --to-id <M> --relation uses --json`
- MERGE — `sqlite-graphrag merge-entities --db ./g.sqlite --names "a,b,c" --into <target> --json`; NUNCA self-ref (`--ids 3,12 --into-id 3` PROIBIDO)

## Fórmulas de Enrich/Manutenção
- STATUS — `sqlite-graphrag enrich --db ./g.sqlite --status --quality-sample 50 --json`
- UNTIL-EMPTY openrouter — `sqlite-graphrag enrich --db ./g.sqlite --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --max-attempts 8 --rest-concurrency 8 --json`
- FORCE-REDESCRIBE — `sqlite-graphrag enrich --db ./g.sqlite --operation entity-descriptions --mode openrouter --openrouter-model openai/gpt-oss-120b --force-redescribe --entity-names jwt,auth-svc --json`
- RE-EMBED entities — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 enrich --db ./g.sqlite --operation re-embed --target entities --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --json`
- RE-EMBED all — o mesmo com `--target all` depois `health --json`
- LIST/REQUEUE skipped — `sqlite-graphrag enrich --db ./g.sqlite --operation entity-descriptions --list-skipped --json` depois `... --requeue-skipped --json`
- LIST/REQUEUE dead — `... --list-dead --json` depois `... --requeue-dead --json`
- EC until-empty — `sqlite-graphrag enrich --db ./g.sqlite --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 600 --rest-concurrency 8 --json`
- EC dry-run — o mesmo com `--dry-run --limit 50` em vez de `--until-empty`
- HEALTH — `sqlite-graphrag health --db ./g.sqlite --json` para `{integrity_ok, schema_version, vec_*_missing, vec_*_coverage_pct}`; DISPARE re-embed quando missing > 0
- DEBUG-SCHEMA — `sqlite-graphrag debug-schema --db ./g.sqlite --json`
- CONFIG — `sqlite-graphrag config set <key> <value>`; `config get <key>`; `config list --json`; `config list --effective --json`; `config unset <key>`; `config path`; `config doctor --json`
- Chaves XDG comuns — `db.path`, `embedding.dim` (1024), `embedding.backend`, `embedding.model`, `llm.backend`, `llm.model`, `llm.query_embed_timeout_secs` (padrão 3s), `display.tz`, `i18n.lang`, `log.level`, `log.format`, `spawn.skip_preflight` (só emergências), `enrich.yield_every_n_items`, `enrich.entity_description.quality_sample`
- PURGE now — `sqlite-graphrag purge --db ./g.sqlite --yes --now --dry-run --json` depois remova `--dry-run`; depois `cleanup-orphans --yes` e `vacuum --json`
- MIGRATE — `migrate --dry-run --json` depois `migrate --json`; OPTIMIZE — `optimize --json`; FTS — `fts check|stats|rebuild --json`; VEC — `vec orphan-list --json` depois `vec purge-orphan --yes`; `vec stats --json`
- EMBEDDING — `embedding status --json`; alias `pending-embeddings status --json`; reprocessar via `enrich --operation re-embed`
- SLOTS — `slots status --json`; `slots release --slot-id <N> --yes`; `slots cleanup --json`; PENDING — `pending list --json`; `pending show <id>`; `pending cleanup --yes`
- EXPORT — `export --namespace <ns> --type <kind> --json`; STATS — `stats --json`; BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECT — `namespace-detect --json`, `cache list --json`, `cache stats --json`, `cache clear-models --yes`; COMPLETIONS — `completions bash|zsh|fish|elvish|powershell`
- AGENDE semanal — `purge --yes` (90d) ou `purge --yes --now` → `cleanup-orphans` → `prune-relations --relation mentions` → `vacuum` → `optimize` → `sync-safe-copy`
- CONCORRÊNCIA — teto rígido `2 x nCPUs` para `init`/`remember`/`ingest`/`recall`/`hybrid-search`; JOB SINGLETON em `enrich` e `ingest --mode codex|claude-code`; USE `--wait-job-singleton SECS` ou `--force-job-singleton`; NUNCA enrich paralelo no mesmo DB

## Anti-Padrões NUNCA
- NUNCA encadeie escrita e enrich com `&&`; SEMPRE aguarde exit 0 e depois enrich separado
- NUNCA coloque `--db` antes do verbo; SEMPRE depois
- NUNCA misture stderr no JSON (`&>` / `2>&1`); SEMPRE `--quiet` + capture só stdout
- NUNCA use product env `SQLITE_GRAPHRAG_*` como config primária; SEMPRE flag > XDG > default
- NUNCA passe `ANTHROPIC_API_KEY`/`OPENAI_API_KEY` a codex/claude (OAuth-only exit 1)
- NUNCA openrouter sem modelo+chave (exit 78); NUNCA omita `--embedding-dim 1024` nos caminhos de embed
- NUNCA rode múltiplos processos enrich no mesmo DB; REST só via `--rest-concurrency`
- NUNCA ignore `entities_created`/`enrich_recommended`; NUNCA ignore exit 19 (retry) ou 16 (MCP)
- NUNCA merge self-ref; NUNCA nomes de entidade só dígitos como `--from`/`--to`; NUNCA `mentions` como relação padrão
- NUNCA use memória MCP / MEMORY.md / diários `.md` ad-hoc; NUNCA escreva `.sqlite` fora do binário
- NUNCA trate "configuration file" bare como gatilho de redescribe de baixa qualidade; SÓ marcadores compostos
- NUNCA assuma que `--until-empty` drena todas as operações; é escopado a ESTA op+namespace
- NUNCA assuma que re-embed usa só a coluna `dim`; elegibilidade é `LENGTH(embedding)=dim*4`
- NUNCA assuma que chaves `entity:` falham no lookup; o prefixo é removido no enqueue
- NUNCA reprocessar batch inteiro após exit 13; NUNCA confunda exit 1 Timeout com exit 75
- TIPOS memória canônicos — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- TIPOS entidade canônicos — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- RELAÇÕES canônicas — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
