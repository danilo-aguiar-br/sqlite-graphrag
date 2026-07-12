---
name: sqlite-graphrag
description: Esta skill DEVE ativar para toda operação da CLI sqlite-graphrag cobrindo memória GraphRAG persistente, hybrid-search, recall, deep-research, remember, remember-batch, ingest, edit, restore, enrich incluindo entity-connect, forget, purge, link, unlink, merge-entities, rename-entity, reclassify, graph traverse, modelos de embedding e texto OpenRouter, config de API key, backends headless codex claude opencode, fórmulas write-then-enrich, embedding paralelo, códigos de saída, concorrência, env vars, fusão FTS5 mais cosine BLOB, tipos e relações canônicas, isolamento de namespace e regras OAuth-only. Esta skill DEVE ser usada sempre que o agente armazena, recupera, busca, enriquece, liga, mescla ou mantém memória GraphRAG de longo prazo. Palavras-chave sqlite-graphrag GraphRAG memory embedding openrouter codex claude opencode remember recall hybrid-search ingest enrich entity-connect deep-research config
---

## Quando Esta Skill Ativa
- DEVE ATIVAR quando o usuário pede para lembrar, salvar, recordar, recuperar, buscar ou persistir algo entre sessões
- DEVE ATIVAR para contexto de longo prazo, grafo de conhecimento, GraphRAG, RAG, ligação de entidades, gestão de memória e conhecimento por namespace
- DEVE ATIVAR quando sqlite-graphrag, embedding, FTS5, hybrid-search, OpenRouter, codex, claude, opencode, entity-connect ou memória LLM for mencionado
- DEVE ATIVAR para enrich, re-embed, link, unlink, merge-entities, rename-entity, deep-research, ingest, config de API keys e manutenção de grafo
- NUNCA ATIVE para dados efêmeros pontuais, I/O simples de arquivo ou tarefas sem relação a contexto persistente
- SEMPRE carregue esta skill antes de inventar arquivos de memória ad-hoc, servidores MCP de memória ou diários Markdown manuais


## Modelo Mental Central
- SAIBA TRÊS seletores independentes; NUNCA os confunda
- SELETOR 1 — `--embedding-backend` COMO os vetores são produzidos — `openrouter` (REST), `llm` (subprocesso) ou `auto`
- SELETOR 2 — `--llm-backend` QUAL subprocesso embeda quando backend é `llm` — `codex`, `claude`, `opencode` ou `none`
- SELETOR 3 — extração via `enrich --mode` — `codex`, `claude-code`, `opencode` ou `openrouter` (REST `/chat/completions`); `--extraction-backend` é o seletor global relacionado
- ESCREVER e ENRIQUECER são SEMPRE operações separadas; a escrita produz embeddings; o `enrich` SEPARADO extrai ou muta o grafo
- NUNCA encadeie escrita e enrich com `&&`; SEMPRE aguarde exit 0 da escrita e só então execute enrich como processo DISTINTO
- Em TODA escrita (`remember`, `remember-batch`, `ingest`, `edit`, `restore`) DEVE passar `--llm-backend none` com `--embedding-backend openrouter` para embeddings via OpenRouter REST sem timeout de subprocesso LLM
- SEMPRE passe `--json`; SEMPRE faça parse com `jaq` NUNCA `jq`; SEMPRE capture stdout PRIMEIRO e só depois parse; NUNCA encaneie a saída da CLI direto em `jaq` (NDJSON mascara falhas como null)
- SAIBA que vetores vazios NUNCA são persistidos; FAÇA parse de `backend_invoked` para confirmar o backend; EXECUTE `enrich` somente após exit 0 da escrita
- SEMPRE mantenha `--embedding-dim 384` idêntico em TODOS os caminhos de embed de escrita e leitura; dimensão divergente colide com o índice e falha knn com exit 11


## Regras de Instrução de Prompt LLM
- "lembre isso" → `remember --force-merge` com `--graph-stdin` de entidades e relações canônicas, depois `enrich` SEPARADO
- "o que você sabe sobre X" → `hybrid-search "X" --k 10 --json` PRIMEIRO, depois `read --name <name> --json`
- "como X se relaciona com Y" → `graph traverse --from X --depth 2 --json` ou `related X --hops 2 --json`; em miss DEVE RETENTAR com `--fuzzy` ou usar sugestões do exit 4 NotFound
- "pesquisa profunda sobre X" → `deep-research "X" --k 20 --max-hops 3 --json`; assuntos de um token expandem em subconsultas de aspecto; envelopes grandes DEVEM usar `--output PATH` e `--quiet`
- "conecte entidades isoladas" → `enrich --operation entity-connect` com `--mode` + modelo obrigatórios, depois monitore `--status` até o backlog esvaziar
- ANTES de qualquer criação → `hybrid-search "<name>" --k 5 --json`; se houver duplicata DEVE USAR `--force-merge`
- DEPOIS de criar/atualizar → capture e faça parse de `read --name <name> --json` para `{name, description, body_length}`
- DEPOIS de cada turno → persista achados novos ou DECLARE "No new findings to persist"
- Em exit diferente de zero → faça parse com `jaq '{code, message, error_class}'` e REPORTE a remediação
- SEMPRE use relações canônicas — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- SEMPRE mapeie não-canônicas primeiro — `adds`/`creates`→`causes`, `implements`→`supports`, `blocks`→`contradicts`, `tested-by`→`related`, `part-of`→`applies-to`
- SEMPRE nomes de entidade em kebab-case ASCII minúsculo; LIMITE entidades a conceitos de domínio; REJEITE genéricos, pronomes, UUIDs, timestamps
- NUNCA use MCP Serena, arquivos `.md` de memória ou MEMORY.md; NUNCA inicie daemon; NUNCA passe `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` a backends subprocesso
- DEVE usar `remember --force-merge` para updates idempotentes; DEVE usar `--graph-stdin` ou `--graph-file` quando houver grafo curado


## Arquitetura e Princípios
- INVOQUE como subprocesso; stdout = JSON/NDJSON; stderr = logs; VERIFIQUE o exit code ANTES do parse
- SAIBA NÃO há daemon, NÃO há ONNX, NÃO há cache de modelo; cosine é Rust puro sobre BLOB `memory_embeddings` / `entity_embeddings` / `chunk_embeddings`
- SAIBA que `init` ou `migrate` aplica o schema vivo; LEIA `schema_version` em `health --json`
- IMPONHA OAUTH-ONLY para codex/claude — o spawn ABORTA com exit 1 se `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` estiverem definidos; `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` são PRESERVADOS
- SAIBA que o CWD do subprocesso é ISOLADO; 7 guards de preflight rodam antes de cada fork LLM; exit 16 = falha de preflight; `claude -p` herda `.mcp.json` do CWD — DEVE ISOLAR config para `claude-code` ou DEVE usar codex
- DEFINA `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1` SOMENTE em emergências; namespace via `--namespace` ou env (padrão `global`)
- NUNCA exponha como MCP/HTTP; NUNCA escreva `.sqlite` com outra ferramenta; FUSÃO é FTS5 BM25 mais cosine KNN BLOB via RRF


## Modelos de Embedding OpenRouter e Preços
- PASSE `--embedding-model <MODEL>` quando `--embedding-backend openrouter`; NÃO há modelo padrão, omissão dispara exit 78
- SAIBA que os preços abaixo são indicativos em USD por milhão de tokens; SEMPRE confirme custo ao vivo via pricing do provedor e `usage.cost` por chamada quando disponível
- USE `nvidia/llama-nemotron-embed-vl-1b-v2:free` para embedding GRATUITO (zero custo)
- USE `qwen/qwen3-embedding-4b` a 0,05 USD por milhão de tokens
- USE `qwen/qwen3-embedding-8b` a 0,05 USD por milhão de tokens — modelo de embedding operacional PADRÃO quando o usuário não especificar outro
- USE `baai/bge-m3` a cerca de 0,05 USD por milhão de tokens
- USE `openai/text-embedding-3-small` a 0,05 USD por milhão de tokens
- USE `perplexity/pplx-embed-v1-0.6b` a 0,05 USD por milhão de tokens
- USE `mistralai/mistral-embed-2312` a 0,10 USD por milhão de tokens
- USE `google/gemini-embedding-2` a cerca de 0,12 USD por milhão de tokens
- USE `openai/text-embedding-3-large` a 0,13 USD por milhão de tokens
- USE `google/gemini-embedding-005` a cerca de 0,15 USD por milhão de tokens
- SAIBA que MRL trunca no servidor para `--embedding-dim`; dimensões nativas maiores ficam baratas truncadas em 384
- VERIFIQUE modelos ao vivo — `curl -sS https://openrouter.ai/api/v1/models -H "Authorization: Bearer $OPENROUTER_API_KEY" | jaq -c '.data[] | select(.id | test("embed";"i")) | {id, pricing}'`
- CONFIRME chaves com `sqlite-graphrag config doctor --json`; modelo inválido → exit 78
- SAIBA que openrouter propaga a TODOS os caminhos de embed — `remember` `remember-batch` `ingest` `recall` `edit` `restore` `hybrid-search` `deep-research` `enrich` `init` `rename-entity`


## Gestão de Chave de API OpenRouter
- ADICIONE chave via stdin — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- LISTE chaves armazenadas — `sqlite-graphrag config list-keys --json`
- REMOVA chave por fingerprint — `sqlite-graphrag config remove-key <fingerprint> --json`
- EXECUTE o diagnóstico doctor — `sqlite-graphrag config doctor --json`
- INSPECIONE o path de config — `sqlite-graphrag config path`
- SAIBA que as chaves vivem em XDG `~/.config/sqlite-graphrag/config.toml` com `chmod 600` e são zeroizadas no drop, NUNCA logadas
- SEMPRE resolva chaves por esta precedência — `OPENROUTER_API_KEY` env > `config.toml` > flag CLI `--openrouter-api-key`
- NUNCA passe a API key como argumento CLI em produção; SEMPRE use stdin ou env var para evitar exposição no histórico do shell
- SEMPRE execute `config doctor` após adicionar uma chave antes de qualquer embedding ou enrich pago


## Backends LLM Headless
- SEMPRE passe a flag de modelo explicitamente em toda invocação headless; NUNCA confie só em defaults silenciosos
- DEFAULT CODEX é `gpt-5.5`; DEVE definir caminho de embedding com `--llm-backend codex --llm-model <MODEL>` e extração com `enrich --mode codex --codex-model <MODEL>`; renove OAuth com `codex login`; codex é OAUTH-ONLY — NUNCA passe `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY`
- DEFAULT CLAUDE é `claude-sonnet-4-6`; DEVE definir embedding com `--llm-backend claude --llm-model <MODEL>` e extração com `enrich --mode claude-code --claude-model <MODEL>`; claude é OAUTH-ONLY — NUNCA passe `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY`
- DEFAULT OPENCODE é `opencode/big-pickle`; DEVE definir embedding com `--llm-backend opencode --llm-model <MODEL>` e extração com `enrich --mode opencode --opencode-model <MODEL>` via autenticação própria (NÃO OAuth)
- EXTRAÇÃO OPENROUTER — DEVE usar `enrich --mode openrouter --openrouter-model <id>`; `--openrouter-model` é OBRIGATÓRIO (sem default; ausência sai com exit 1 antes de qualquer rede); chave vem de `OPENROUTER_API_KEY` ou config
- SAIBA que o catálogo opencode é EXTERNO/dinâmico; `--opencode-model` NÃO é validado; PASSE ids vivos do OpenCode Zen; CONSULTE `opencode.ai/zen`
- SOBRESCREVA binários com `--codex-binary`, `--claude-binary`, `--opencode-binary`; AJUSTE timeouts com `--codex-timeout`, `--claude-timeout`, `--opencode-timeout`
- VALIDE modelos codex com `--codex-model-validate` e `--codex-model-fallback <MODEL>`; LISTE com `sqlite-graphrag codex-models --json` (modelos CODEX apenas, NÃO OpenRouter)
- TROQUE backend em rate limit com `enrich --fallback-mode codex` ou global `--llm-fallback codex,claude,none`
- SAIBA que `--mode openrouter` é REST puro `/chat/completions` — NÃO exige CLI local; fatura `OPENROUTER_API_KEY` (leia `usage.cost`); codex/claude-code/opencode são caminhos zero-token OAuth/auth própria


## Modelos de Texto OpenRouter para Enrich
- PASSE `--openrouter-model <MODEL>` desta tabela em `--mode openrouter`; preços são indicativos input/output USD por milhão de tokens — SEMPRE confirme ao vivo via `usage.cost`
- SAIBA que estes modelos servem SOMENTE extração e enrich, NUNCA embedding; a tabela de embedding é separada
- DEVE usar `openai/gpt-oss-120b` a 0,059/0,18 USD, contexto 131k, 36 tps como judge PADRÃO quando o usuário não especificar modelo de texto
- USE `openai/gpt-oss-120b:nitro` a 0,15/0,60 USD, contexto 131k, 300 tps para throughput máximo
- USE `deepseek/deepseek-v4-flash` a 0,09/0,18 USD, contexto 1M, 20 tps
- USE `deepseek/deepseek-v4-flash:nitro` a 0,14/0,28 USD, contexto 1M, 109 tps
- USE `deepseek/deepseek-v4-pro` a 1,30/2,60 USD, contexto 1M, 26 tps
- USE `google/gemini-3.1-flash-lite` a 0,95/3,00 USD, contexto 1M, 100 tps
- USE `minimax/minimax-m3` a 0,30/1,20 USD, contexto 1M, 42 tps
- USE `minimax/minimax-m2.7` a 0,25/1,00 USD, contexto 205k, 43 tps
- USE `minimax/minimax-m2.7:nitro` a 0,30/1,20 USD, contexto 205k, 146 tps
- USE `xiaomi/mimo-v2.5` a 0,10/0,28 USD, contexto 1M, 17 tps
- USE `xiaomi/mimo-v2.5-pro` a 0,43/0,87 USD, contexto 1M, 29 tps
- USE `z-ai/glm-5.2` e `z-ai/glm-5.2:nitro` cujo preço varia por provedor; SEMPRE CONFIRME o custo real via `usage.cost`
- SAIBA que variantes `:nitro` roteiam para o provedor mais rápido a preço maior
- VERIFIQUE se o modelo honra `json_schema` estrito ANTES de produção; modelo sem Structured Outputs falha com erro explícito da OpenRouter


## Referência de Flags Globais
- `--db <PATH>` — sobrescreve o banco; COLOQUE DEPOIS do subcomando; override por env é `SQLITE_GRAPHRAG_DB_PATH`
- `--namespace <ns>` — escopo; `--json` — JSON estruturado (SEMPRE passe); `--lang en|pt`; `--tz <TIMEZONE>`
- `--embedding-backend auto|openrouter|llm`; `--embedding-model <MODEL>` (OBRIGATÓRIO com openrouter); `--embedding-dim N` padrão 384 MRL faixa [8, 4096]
- `--openrouter-api-key <KEY>` — PROIBIDO no histórico de shell em produção
- `--llm-backend codex|claude|opencode|none|auto`; `--llm-model <MODEL>`; `--llm-fallback <chain>`
- `--extraction-backend` global relacionado; `--openrouter-model <MODEL>` OBRIGATÓRIO em `--mode openrouter`; `--openrouter-base-url`; `--openrouter-timeout` padrão 600
- `--llm-parallelism N` — fan-out de embedding padrão 4 clamp [1, 32]; governa subprocesso E concorrência JoinSet REST OpenRouter
- `--rest-concurrency N` — fan-out enrich openrouter clamp [1, 16] padrão 8; DISTINTO de `--llm-parallelism`
- `--max-concurrency N` clamp [1, 2×nCPUs]; `--llm-max-host-concurrency N`; `--llm-slot-wait-secs N` / `--llm-slot-no-wait`
- `--wait-lock SECS`; `--low-memory`; `--strict-env-clear`; `--graceful-shutdown-secs N`; `--skip-embedding-on-failure`
- `--codex-binary`, `--claude-binary`, `--opencode-binary`; `-v`/`-vv`/`-vvv`; `--quiet`/`-q` OBRIGATÓRIO em pipelines headless; NUNCA misture stderr no JSON com `&>`


## Operações CRUD de Escrita
- INVOQUE `remember --name <kebab> --type <kind> --description <text>` com exatamente uma fonte de corpo — `--body` ou `--body-file` ou `--body-stdin` ou `--graph-stdin`
- INVOQUE `remember --graph-stdin` para `{body, entities, relationships}` em um JSON; ou `--graph-file` combinado com `--body-file`
- PASSE entidades como `[{name, entity_type}]` kebab-case ASCII; PASSE relacionamentos como `[{source, target, relation, strength}]` strength em [0.0, 1.0]
- PASSE `--strict-name` para REJEITAR nomes não-kebab; PASSE `--force-merge` para updates idempotentes; PASSE `--replace-graph` com `--force-merge` para replace total do grafo
- PASSE `--dry-run` para validar sem persistir
- VALORES válidos de `--type` de memória — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- INVOQUE `remember-batch` para 10+ memórias via NDJSON stdin; PASSE `--transaction` para all-or-nothing
- INVOQUE `ingest <DIR> --recursive --pattern "*.md" --mode none` para import body-only, depois enrich SEPARADO
- SAIBA que `ingest --mode` aceita `none` (padrão body-only), `claude-code`, `codex`, `opencode`; modos non-none rodam extração LLM inline DURANTE o ingest e NÃO precisam de enrich separado para esses bindings
- USE `--resume`; `--retry-failed`; `--auto-describe`; `--name-prefix <prefix>`; `--force-merge` no ingest para ATUALIZAR duplicatas (dedup por `body_hash`)
- SAIBA que `ingest` auto-divide corpos oversized em chunks
- INVOQUE `split-body --name <N>` para UMA memória acima de 25000 chars; PASSE `--batch --threshold 25000` para todos oversized; FILHAS NÃO SÃO EMBEDADAS INLINE — passo1 openrouter embed + `--llm-backend none split-body`; passo2 SEPARADO `enrich --operation re-embed --target memories`
- RESPEITE 512000 bytes e 512 chunks por corpo; NUNCA misture fontes de corpo; NUNCA `fd | xargs remember` — USE `ingest`
- NUNCA passe `--llm-backend` diferente de `none` na escrita quando o embed for OpenRouter; SEMPRE passe `--llm-backend none`


## CRUD Leitura Atualização Exclusão
- INVOQUE `read --name <kebab> --json`; PASSE `--with-graph`; USE `--format raw` para corpo puro
- INVOQUE `list --type <kind> --limit N --offset N --json`; `history --name <n> --diff --json`
- INVOQUE `edit --name <n> --body-file <path>` ou `--description` / `--memory-type`; USE `--force-reembed`; USE `--expected-updated-at <ts>` (exit 3 = conflito — recarregue e retente)
- INVOQUE `rename --name <old> --new-name <new>`; `restore --name <n> --version <N>` (caminho de escrita — OpenRouter embed + `--llm-backend none`, depois enrich SEPARADO)
- INVOQUE `forget --name <n>`; `purge --retention-days <N> --yes --dry-run` depois remova `--dry-run`; `cleanup-orphans --yes` depois `vacuum --json`
- NUNCA pule optimistic locking em pipelines concorrentes; NUNCA delete via shell `sqlite3`


## Operações de Grafo de Entidades
- INVOQUE `link --from <a> --to <b> --relation <type> --create-missing --weight <float>`; DEVE usar `link --from-id <N> --to-id <M>` quando IDs forem conhecidos
- NUNCA passe dígitos puros como nomes `--from`/`--to` — nomes só-numéricos são REJEITADOS
- INVOQUE `unlink --from <a> --to <b> --relation <type>` ou `--entity <name> --all`; `unlink --memory <name> --entity <name>` para um binding memória-entidade
- INVOQUE `graph entities --json` via `.entities[]` (NÃO `.items[]`); ORDENE `--sort-by name|degree|created-at`; PAGINE `--limit`/`--offset`
- INVOQUE `graph stats --json`; `graph traverse --from <root> --depth <N> --json`; EXPORTE `--format json|dot|mermaid --output <path>`
- DEVE passar `--fuzzy` em traverse com nome curto ambíguo; SEM `--fuzzy`, exit 4 inclui sugestões ranqueadas — SEMPRE use-as
- INVOQUE `rename-entity --name <old> --new-name <new>` ou `--id <N> --new-name <new>`
- INVOQUE `delete-entity --name <n> --cascade`; `merge-entities --names "a,b,c" --into <target>` ou `--ids 12,17 --into-id 3`
- NUNCA coloque `--into-id` dentro de `--ids` nem `--into` dentro de `--names`; merges auto-referenciais são REJEITADOS ANTES de qualquer trabalho no DB
- SEMPRE USE arrays de shell para listas dinâmicas de merge — quote a string unida; PASSE `--cross-namespace` só quando intencional
- INVOQUE `reclassify --name <n> --new-type <kind>` ou `--from-type <old> --to-type <new> --batch`
- INVOQUE `reclassify-relation --from-relation <old> --to-relation <new> --batch`; PASSE `--literal-from`/`--literal-to` para match literal
- INVOQUE `prune-relations --relation mentions --dry-run` depois remova `--dry-run` com `--yes`; `normalize-entities --yes`; `prune-ner --entity <n>` ou `--all --yes`
- INVOQUE `memory-entities --name <memory>` ou `--entity <name>`; `graph recompute-degree --json` após delete/merge/prune (grau NÃO é auto-recomputado)
- TIPOS canônicos de entidade — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- VALIDE nomes de entidade — mín 2 chars, sem newlines, sem ALL_CAPS curtos ≤4 chars, REJEITE dígitos puros; NUNCA use `mentions` como relação padrão
- SAIBA que escritas de grafo são ADITIVAS sem teto de grau; NORMALIZE só via prune/merge/normalize — NUNCA durante a escrita


## Operações de Busca GraphRAG
- USE o padrão canônico de três camadas — `hybrid-search` depois `read --name` depois `related` ou `graph traverse`
- INVOQUE `recall <query> --k N` para KNN semântico puro; PASSE `--no-graph`, `--precise`, `--max-distance <f>`, `--max-graph-results N`, `--all-namespaces`
- INVOQUE `hybrid-search <query> --k N` para fusão FTS5 + KNN via RRF; PASSE `--rrf-k 60`; `--weight-vec 1.0 --weight-fts 1.0`; `--fallback-fts-only` para FTS offline
- USE `--with-graph --max-hops 2 --min-weight 0.3`; LEIA TANTO `results[]` QUANTO `graph_matches[]`
- INVOQUE `related <name> --hops N --relation <type>`
- INVOQUE `deep-research "<query>" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies`; consultas de um token expandem em subconsultas de aspecto; para controle manual PASSE `--sub-query-strategy manual --sub-queries-file PATH`
- ESCREVA envelopes grandes de deep-research com `--output PATH` (atomwrite); FAÇA parse do ack curto de stdout `{written, bytes, blake3}`; PASSE `--quiet`; NUNCA use `&>`
- AJUSTE deep-research com `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--max-cost-usd`, `--timeout`
- FAÇA parse de `recall` → `results[].{name, snippet, distance, score, source}`; `hybrid-search` → `results[].{name, combined_score, vec_rank, fts_rank}`; `deep-research` → `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context`, `stats`
- NUNCA confunda `distance` com `combined_score`; NUNCA aumente hops sem inspecionar `graph stats` primeiro


## Operações de Enrich
- INVOQUE `enrich --operation <op> --mode <backend>` onde AMBAS as flags são OBRIGATÓRIAS para qualquer operação LLM; omitir `--mode` é rejeitado com exit 2 — EXCETO inspetores read-only `--status`, `--list-dead`, `--requeue-dead`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` que NÃO exigem `--operation`/`--mode`
- Ops FULLY-IMPLEMENTED que PERSISTEM — `memory-bindings` (entidades→memórias unbound), `augment-bindings` (bindings extras em memórias JÁ ligadas; EXIGE `--names` ou `--names-file`), `entity-descriptions`, `body-enrich`, `re-embed` (só reconstrói vetores), `entity-connect`, `cross-domain-bridges`, `body-extract` + `--body-extract-graph-only` (só grafo, sem reescrever corpo)
- Ops SCAN-ONLY (só relatório, sem persistir) — `weight-calibrate`, `relation-reclassify`, `entity-type-validate`, `description-enrich`, `domain-classify`, `graph-audit`, `deep-research-synth`
- Valores válidos de `--mode` — `codex`, `claude-code`, `opencode`, `openrouter`
- PASSE `--codex-model`, `--claude-model`, `--opencode-model` ou `--openrouter-model` conforme o modo
- SAIBA que `--mode openrouter` exige `--openrouter-model`, chave de env/config, REST `/chat/completions` com json_schema estrito e `provider.require_parameters` true, faturado via `usage.cost`
- PASSE `--limit N --resume` para `re-embed`; `--retry-failed`; `--dry-run`; `--target memories|entities|chunks|all` só em `re-embed` (padrão `memories`)
- SAIBA que `re-embed` seleciona vetores AUSENTES, blobs VAZIOS ou dimensão DIVERGENTE de `--embedding-dim`
- PASSE `--min-output-chars N` para `body-enrich`; `--fallback-mode codex` em rate limits Claude
- NUNCA rode múltiplos processos `enrich` em paralelo no mesmo banco (singleton por namespace); paralelismo REST é SOMENTE `--rest-concurrency` DENTRO de UM processo
- PASSE `--until-empty` para loop scan→drain até esvaziar ou `--max-runtime` (padrão 3600) expirar; PASSE `--max-attempts <N>` padrão 8 faixa 1..=20
- PASSE `--status` para `scan_backlog`, `unbound_backlog`, contagens da fila, `eligible_now`, `waiting` — SEM LLM, SEM singleton
- SAIBA que `scan_backlog` é backlog REAL do DB; DISTINTO de `unbound_backlog` e do sidecar `queue_pending`
- PASSE `--list-dead`; `--requeue-dead`; `--ignore-backoff`; `--prune-dead-orphans`; `--prune-dead-entity-orphans` (mutuamente exclusivos); `--reset-stale-claims` após `kill -9`
- SAIBA que dead-letter tem backoff Transient vs HardFailures; completions OpenRouter truncadas (`finish_reason` = `length`) reemitem com `max_tokens` AUMENTADO antes do JSON repair
- SAIBA que a fila de enrich é sidecar `.enrich-queue.sqlite` ao lado do DB principal
- DISTINGA — `scan_backlog` = candidatos DB que um scan fresco ENFILEIRARIA; `queue_pending` = contagem sidecar; `eligible_now == 0` com `queue_pending > 0` é COOLDOWN; `draining` travado → `--reset-stale-claims`
- Fórmula STATUS — `sqlite-graphrag enrich --status --json`
- Fórmula UNTIL-EMPTY — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --max-attempts 8 --rest-concurrency 8 --json`
- BACKFILL completo re-embed — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 enrich --operation re-embed --target all --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --json` depois `health --json`


## Regras Operacionais de Entity-Connect
- SAIBA que `entity-connect` PERSISTE arestas via `entity_connect_seen` com vereditos `related` ou `none`; `cross-domain-bridges` usa o MESMO caminho de scan e drain
- SAIBA que o scan de pares é O(k) sobre candidatos de coocorrência mais preenchimento hub × ilha grau-0 — SEGURO em namespaces `global` grandes; NUNCA materializa produto cartesiano completo de entidades
- SAIBA que as chaves de fila DEVEM ser `pair:{id1}:{id2}` com `item_type=entity_pair`; o drain resolve entidades por chave primária e NÃO DEVE reexecutar o scan completo por item
- SAIBA que o primeiro scan e os rescans são cobertos por `--max-runtime` e um soft ~120s `InterruptHandle`; interrupt vira Timeout exit **1** — NUNCA trate isso como exit 75 de lock de singleton
- SAIBA que o progresso NDJSON emite `scan_start` antes do SQL e `scan_meta` depois com campos de backlog dual `backlog_degree0_proxy` versus `pairs_enqueued_this_scan`
- SEMPRE converja com `--until-empty` até o backlog esvaziar ou o runtime expirar; SEMPRE inspecione `--status` entre corridas longas
- SEMPRE faça dry-run primeiro em corpora de produção
- ENTITY-CONNECT dry-run — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --dry-run --limit 50 --json`
- ENTITY-CONNECT until-empty openrouter — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 600 --rest-concurrency 8 --json`
- ENTITY-CONNECT codex — `sqlite-graphrag enrich --operation entity-connect --mode codex --codex-model gpt-5.5 --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT claude — `sqlite-graphrag enrich --operation entity-connect --mode claude-code --claude-model claude-sonnet-4-6 --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT opencode — `sqlite-graphrag enrich --operation entity-connect --mode opencode --opencode-model opencode/big-pickle --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT bridges — mesmas fórmulas com `--operation cross-domain-bridges`
- SAIBA que linhas legadas da fila sem prefixo `pair:` são ignoradas pelo drain; reenfileire ou rescanne se um sidecar antigo ainda as contiver


## Escrever Depois Enrich — Templates
- TRATE toda escrita como PASSO 1 e depois PASSO 2 DISTINTO; NUNCA encadeie com `&&`
- DEFINA o prefixo de embed OpenRouter como — `sqlite-graphrag --embedding-backend openrouter --embedding-model <EMB> --embedding-dim 384 --llm-backend none`
- PADRÃO de `<EMB>` = `qwen/qwen3-embedding-8b` salvo se o usuário escolher outro id do catálogo; caminho GRATUITO usa `nvidia/llama-nemotron-embed-vl-1b-v2:free`
- Fórmulas do PASSO 1 (SEMPRE exit 0 antes do PASSO 2)
- REMEMBER — `echo '{"body":"text","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | <PREFIX> remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --json`
- REMEMBER-BATCH — `<PREFIX> remember-batch --transaction --json` com NDJSON no stdin
- INGEST — `<PREFIX> ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- EDIT — `<PREFIX> edit --name <n> --body-file new.md --json`
- RESTORE — `<PREFIX> restore --name <n> --version 2 --json`
- Templates do PASSO 2 (escolha UM backend por corrida; SEMPRE defina flags de modelo explicitamente)
- CODEX — `sqlite-graphrag enrich --operation memory-bindings --mode codex --codex-model gpt-5.5 --json`
- CLAUDE — `sqlite-graphrag enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- OPENCODE — `sqlite-graphrag enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- OPENROUTER texto — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- APLIQUE os mesmos templates do PASSO 2 após remember, remember-batch, ingest, edit e restore — o comando de escrita muda; o passo de enrich permanece o mesmo
- SAIBA que o PASSO 2 de extração NÃO exige `--llm-backend` na linha de enrich; passe flags de embedding só quando a operação precisar de vetores (`re-embed`) ou ao documentar defaults do host
- SAIBA que a resolução de chave prefere env `OPENROUTER_API_KEY` ou `config.toml`; NUNCA embuta a chave crua em linhas de comando de produção


## Embedding e Enrich Paralelos
- ESCALE o embed com `--llm-parallelism N` no PASSO 1 (JoinSet de N requests OpenRouter, ordem preservada)
- ESCALE o enrich com `--rest-concurrency N` + `--until-empty` no PASSO 2 openrouter apenas (N chat calls; escrita SQLite fica serial via claim WAL)
- CLAMPE `--llm-parallelism` 1..32 e `--rest-concurrency` 1..16; modelos pagos DEVEM usar 4..16; `:free` limita ~20 req/min então DEVE usar N baixo; múltiplas chaves NÃO aumentam capacidade
- NUNCA lance N processos enrich para paralelismo; UM processo com `--rest-concurrency` é OBRIGATÓRIO
- Enrich headless codex/claude/opencode NÃO usa fan-out `--rest-concurrency` da mesma forma; NUNCA multiplique processos enrich para compensar
- REMEMBER paralelo PASSO 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --json`
- Paralelo PASSO 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --rest-concurrency 8 --until-empty --max-runtime 3600 --max-attempts 8 --json`
- REMEMBER-BATCH paralelo PASSO 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 12 --llm-backend none remember-batch --transaction --json`
- REMEMBER-BATCH paralelo PASSO 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --rest-concurrency 12 --until-empty --max-runtime 3600 --json`
- INGEST paralelo PASSO 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 --llm-parallelism 6 --llm-backend none ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- INGEST paralelo PASSO 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b:nitro --rest-concurrency 12 --until-empty --max-runtime 7200 --max-attempts 8 --json`
- EDIT paralelo PASSO 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none edit --name <n> --body-file new.md --json`
- EDIT paralelo PASSO 2 — mesmo PASSO 2 openrouter paralelo do remember
- RESTORE paralelo PASSO 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none restore --name <n> --version 2 --json`
- RESTORE paralelo PASSO 2 — mesmo PASSO 2 openrouter paralelo do remember
- MONITORE com `enrich --status --json` até `scan_backlog` `queue_pending` `eligible_now` serem todos 0; `queue_dead` é dívida permanente até requeue ou prune


## Fórmulas OpenRouter Somente Leitura
- INIT — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 init --namespace <ns>`
- RECALL — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 recall "query" --k 10 --json`
- HYBRID-SEARCH — `sqlite-graphrag --embedding-backend openrouter --embedding-model baai/bge-m3 --embedding-dim 384 hybrid-search "query" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- DEEP-RESEARCH — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model openai/text-embedding-3-small --embedding-dim 384 deep-research "question" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies --output /tmp/research.json --json`
- RENAME-ENTITY — `sqlite-graphrag --embedding-backend openrouter --embedding-model perplexity/pplx-embed-v1-0.6b --embedding-dim 384 rename-entity --name <old> --new-name <new> --json`
- HYBRID-SEARCH offline — `sqlite-graphrag hybrid-search "query" --k 10 --fallback-fts-only --json`
- TRAVERSE fuzzy — `sqlite-graphrag graph traverse --from <short-alias> --depth 2 --fuzzy --json`
- LINK por ID — `sqlite-graphrag link --from-id <N> --to-id <M> --relation uses --json`
- GUARDA self-ref de MERGE — NUNCA execute `merge-entities --ids 3,12 --into-id 3`; SEMPRE exclua o survivor de `--ids`
- VERIFIQUE catálogo de embedding OpenRouter — `curl -sS https://openrouter.ai/api/v1/models -H "Authorization: Bearer $OPENROUTER_API_KEY" | jaq -r '.data[].id' | rg -i 'embed'`


## Diagnóstico e Manutenção
- INIT — `sqlite-graphrag init --namespace <ns>`
- HEALTH — `sqlite-graphrag health --json` para `{integrity_ok, schema_version, vec_*_missing, vec_*_coverage_pct}`; DISPARE `enrich --operation re-embed --target <target>` quando qualquer missing > 0
- MIGRATE — `migrate --dry-run --json` depois `migrate --json`; OPTIMIZE — `optimize --json`; VACUUM — `vacuum --json` após purge
- FTS — `fts check|stats|rebuild --json` quando `health.fts_degraded`; VEC — `vec orphan-list --json` depois `vec purge-orphan --yes`; `vec stats --json`
- EMBEDDING — `embedding --status --json`; `pending-embeddings --status --json` depois `pending-embeddings process --json`
- SLOTS — `slots status --json`; `slots release --slot-id <N> --yes`; PENDING — `pending list --filter-status queued --json`; `pending show <id>`; `pending cleanup --yes`
- EXPORT — `export --namespace <ns> --type <kind> --json`; STATS — `stats --json`
- BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECT — `namespace-detect --json`, `debug-schema --json`, `cache list --json`, `cache clear-models --yes`
- COMPLETIONS — `completions bash|zsh|fish|elvish|powershell`
- AGENDE semanalmente — `purge` → `cleanup-orphans` → `prune-relations --relation mentions` → `vacuum` → `optimize` → `sync-safe-copy`
- SE houver corrupção — `sqlite3 broken.sqlite ".recover" | sqlite3 repaired.sqlite`


## Códigos de Saída e Estratégia de Retry
- EXIT 0 sucesso
- EXIT 1 validação **ou** Timeout de wall-clock (incluindo InterruptHandle do primeiro scan de entity-connect — NÃO é o mesmo que exit 75)
- EXIT 2 parse de argumentos; EXIT 3 optimistic lock (recarregue e retente)
- EXIT 4 not found (sem `--fuzzy`, inclui sugestões Jaro-Winkler/prefixo); EXIT 5 erro de namespace
- EXIT 6 payload grande demais — variantes reportam `bytes`/`limit`, `chunks`/`limit` ou `tokens`/`limit`; DIVIDA o corpo ou reduza tokens
- EXIT 9 duplicata — use `--force-merge`; EXIT 10 banco — rode `vacuum` mais `health`
- EXIT 11 falha de embedding — cheque backend, dim 384, OAuth/chave
- EXIT 13 batch parcial — reprocessar só falhos; EXIT 14 I/O; EXIT 15 busy — alargue `--wait-lock`
- EXIT 16 preflight — corrija config MCP; NUNCA trate como transitório
- EXIT 19 SHUTDOWN — retry OBRIGATÓRIO; trabalho parcial descartado; EXIT 20 interno
- EXIT 75 slots/singleton locked — respeite cooldown; NUNCA retente imediatamente
- EXIT 77 pressão de RAM; EXIT 78 config (chave OpenRouter ou modelo ausente)
- NUNCA ignore exit diferente de zero; NUNCA reprocessar batch inteiro após exit 13; NUNCA confunda exit 1 Timeout com exit 75 lock ou exit 9 duplicata


## Concorrência
- RESPEITE o teto duro `2 x nCPUs` para comandos pesados — `init`, `remember`, `ingest`, `recall`, `hybrid-search`
- DEFINA `--llm-parallelism N` padrão 4 em `remember` e `edit`, padrão 2 em `ingest`, clamp [1, 32]
- SAIBA JOB SINGLETON — `enrich` e `ingest --mode codex|claude-code` adquirem singleton por namespace
- USE `--wait-job-singleton SECS` ou `--force-job-singleton` para quebrar lock stale
- HABILITE `SQLITE_GRAPHRAG_LOW_MEMORY=1` para paralelismo unitário (mais lento)
- NUNCA rode `enrich` em paralelo contra o mesmo banco
- SAIBA que a concorrência REST de enrich é controlada por `--rest-concurrency`, NÃO por múltiplos processos enrich


## Variáveis de Ambiente
- `SQLITE_GRAPHRAG_DB_PATH` — override do path do banco
- `SQLITE_GRAPHRAG_NAMESPACE` — namespace persistente
- `SQLITE_GRAPHRAG_LLM_BACKEND` / `SQLITE_GRAPHRAG_LLM_MODEL` — backend e modelo LLM persistentes
- `SQLITE_GRAPHRAG_EMBEDDING_BACKEND` / `SQLITE_GRAPHRAG_EMBEDDING_MODEL` / `SQLITE_GRAPHRAG_EMBEDDING_DIM` — defaults do caminho de embed (dim padrão 384)
- `OPENROUTER_API_KEY` — chave OpenRouter, zeroizada no drop
- `SQLITE_GRAPHRAG_CODEX_BINARY` / `SQLITE_GRAPHRAG_CLAUDE_BINARY` / `SQLITE_GRAPHRAG_OPENCODE_BINARY` — overrides de binário
- `SQLITE_GRAPHRAG_OPENCODE_MODEL` / `SQLITE_GRAPHRAG_OPENCODE_TIMEOUT` — overrides opencode
- `SQLITE_GRAPHRAG_LOW_MEMORY` — paralelismo unitário; `SQLITE_GRAPHRAG_LOG_FORMAT` — `json` para agregadores
- `SQLITE_GRAPHRAG_SKIP_PREFLIGHT` — bypass de preflight, SOMENTE EMERGÊNCIAS
- NUNCA confie em `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` para codex/claude; são PROIBIDOS e abortam spawn com exit 1


## Regras Ativas
- SEMPRE `--json`; SEMPRE `jaq` após capture (NUNCA pipe NDJSON; NUNCA `jq`); SEMPRE faça parse de `backend_invoked`
- SEMPRE `--embedding-backend openrouter --embedding-model <MODEL> --embedding-dim 384` em ops de embed com chave via env/config
- SEMPRE `--llm-backend none` nas escritas; SEMPRE `enrich` SEPARADO com `--mode` + modelo; NUNCA encadeie com `&&`
- SEMPRE renove OAuth (`codex login` / claude OAuth) quando stale; SEMPRE mantenha dim 384 (mismatch → knn exit 11)
- SEMPRE arrays de shell + joins quotados para listas dinâmicas de `merge-entities`
- DEVE usar `--from-id`/`--to-id` para IDs numéricos de link; NUNCA dígitos puros como nomes `--from`/`--to`
- DEVE retentar com `--fuzzy` ou sugestões NotFound em traverse de nome curto; DEVE usar `--output PATH` + `--quiet` para deep-research grande; NUNCA `&>`
- NUNCA passe API keys a codex/claude (OAuth-only, exit 1); NUNCA `--llm-backend codex` em caminhos de escrita com embed OpenRouter
- NUNCA processos `enrich` paralelos no mesmo DB; NUNCA escreva `.sqlite` fora do binário
- NUNCA ignore exit 19 (retry OBRIGATÓRIO) ou exit 16 (corrija MCP); NUNCA openrouter sem modelo+chave (exit 78)
- NUNCA merge self-ref (`--into-id` dentro de `--ids` / `--into` dentro de `--names`); NUNCA memória MCP ou MEMORY.md
- TIPOS canônicos de memória — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- TIPOS canônicos de entidade — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- RELAÇÕES canônicas — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
