---
name: sqlite-graphrag
description: Esta skill DEVE ativar para toda operação da CLI sqlite-graphrag cobrindo memória GraphRAG, hybrid-search, recall, deep-research -o, remember enqueue-enrich entities_created enrich_recommended, remember-batch, ingest, edit, restore, enrich force-redescribe entity-names memory-names quality_pct blocked_dead budget_exhausted preempted_for_gate, entity-connect escala, memory-entities description, forget, purge, link, unlink, merge-entities, rename-entity, reclassify, graph traverse, modelos embed e texto OpenRouter, XDG e chaves, headless codex claude opencode, fórmulas write-then-enrich, embedding paralelo, exit codes, concorrência, fusão FTS5 BLOB, tipos relações canônicas, namespace, OAuth-only. Esta skill DEVE ser usada sempre que o agente armazena, recupera, busca, enriquece, liga, mescla ou mantém memória GraphRAG de longo prazo. Palavras-chave sqlite-graphrag GraphRAG memory embedding openrouter codex claude opencode remember recall hybrid-search ingest enrich entity-connect deep-research force-redescribe enqueue-enrich config XDG
---

# sqlite-graphrag

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
- Em TODA escrita (`remember`, `remember-batch`, `ingest`, `edit`, `restore`) DEVE passar `--llm-backend none` + `--embedding-backend openrouter` + `--embedding-model` + `--embedding-dim 1024` para embeddings via OpenRouter REST sem timeout de subprocesso LLM
- SEMPRE passe `--json`; SEMPRE faça parse com `jaq` NUNCA `jq`; SEMPRE capture stdout PRIMEIRO e só depois parse; NUNCA encadeie a saída da CLI direto em `jaq` (NDJSON mascara falhas como null)
- SAIBA que vetores vazios NUNCA são persistidos; FAÇA parse de `backend_invoked`; EXECUTE `enrich` somente após exit 0 da escrita
- SEMPRE mantenha `--embedding-dim 1024` idêntico em TODOS os caminhos de embed de escrita e leitura; dimensão divergente colide com o índice e falha knn com exit 11

## Regras de Instrução de Prompt
- "lembre isso" → `remember --force-merge` com `--graph-stdin` de entidades e relações canônicas, depois `enrich` SEPARADO
- "o que você sabe sobre X" → `hybrid-search "X" --k 10 --json` PRIMEIRO, depois `read --name <name> --json`
- "como X se relaciona com Y" → `graph traverse --from X --depth 2 --json` ou `related X --hops 2 --json`; em miss DEVE RETENTAR com `--fuzzy` ou usar sugestões do exit 4 NotFound
- "pesquisa profunda sobre X" → `deep-research "X" --k 20 --max-hops 3 --json`; assuntos de um token expandem em subconsultas de aspecto; envelopes grandes DEVEM usar `--output PATH` e `--quiet`
- "conecte entidades isoladas" → `enrich --operation entity-connect` com `--mode` + modelo obrigatórios, depois monitore `--status` até o backlog esvaziar
- ANTES de qualquer criação → `hybrid-search "<name>" --k 5 --json`; se houver duplicata DEVE USAR `--force-merge`
- DEPOIS de criar/atualizar → parse `read --name <name> --json` para `{name, description, body_length}`; DEPOIS de cada turno → persista achados ou DECLARE "No new findings to persist"
- Em exit ≠0 → parse `jaq '{code, message, error_class}'` e REPORTE remediação
- SEMPRE relações canônicas — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`; mapeie não-canônicas — `adds`/`creates`→`causes`, `implements`→`supports`, `blocks`→`contradicts`, `tested-by`→`related`, `part-of`→`applies-to`
- SEMPRE nomes kebab-case ASCII minúsculo; LIMITE a conceitos de domínio; REJEITE genéricos, pronomes, UUIDs, timestamps
- NUNCA MCP Serena, `.md` de memória ou MEMORY.md; NUNCA daemon; NUNCA `ANTHROPIC_API_KEY`/`OPENAI_API_KEY` a backends subprocesso
- DEVE `remember --force-merge` para updates idempotentes; DEVE `--graph-stdin` ou `--graph-file` com grafo curado

## Arquitetura
- INVOQUE como subprocesso; stdout = JSON/NDJSON; stderr = logs; VERIFIQUE o exit code ANTES do parse
- SAIBA NÃO há daemon, NÃO há ONNX, NÃO há cache de modelo; cosine é Rust puro sobre BLOB `memory_embeddings` / `entity_embeddings` / `chunk_embeddings`
- SAIBA que `init` ou `migrate` aplica o schema vivo; LEIA `schema_version` em `health --json`
- IMPONHA OAUTH-ONLY para codex/claude — o spawn ABORTA com exit 1 se `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` estiverem definidos; `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` são PRESERVADOS
- SAIBA que o CWD do subprocesso é ISOLADO; 7 guards de preflight rodam antes de cada fork LLM; exit 16 = falha de preflight; `claude -p` herda `.mcp.json` do CWD — DEVE ISOLAR config para `claude-code` ou DEVE usar codex
- DEFINA skip de preflight de emergência SOMENTE via `sqlite-graphrag config set spawn.skip_preflight=1` (SOMENTE EMERGÊNCIAS); namespace via `--namespace` ou XDG `config set` (padrão `global`)
- PROIBIDO product env `SQLITE_GRAPHRAG_*` — NÃO é lida no hot path; SEMPRE use flags CLI e XDG `config set`
- NUNCA exponha como MCP/HTTP; NUNCA escreva `.sqlite` com outra ferramenta; FUSÃO é FTS5 BM25 mais cosine KNN BLOB via RRF
- SAIBA folhas host `config`, `slots`, `cache`, `codex-models`, `completions` ACEITAM `--db` como no-op (não abrem o grafo); superfícies de grafo USAM `--db` de verdade para resolver o storage

## Modelos de Embedding OpenRouter
- PASSE `--embedding-model <MODEL>` quando `--embedding-backend openrouter`; NÃO há modelo padrão, omissão dispara exit 78
- SAIBA preços indicativos em USD por milhão de tokens; SEMPRE confirme custo ao vivo via pricing do provedor e `usage.cost` quando disponível
- USE `nvidia/llama-nemotron-embed-vl-1b-v2:free` GRATUITO (zero custo)
- USE `qwen/qwen3-embedding-4b` a 0,05; `qwen/qwen3-embedding-8b` a 0,05 — PADRÃO operacional quando o usuário não especificar outro
- USE `baai/bge-m3` ~0,05; `openai/text-embedding-3-small` 0,05; `perplexity/pplx-embed-v1-0.6b` 0,05
- USE `mistralai/mistral-embed-2312` 0,10; `google/gemini-embedding-2` ~0,12; `openai/text-embedding-3-large` 0,13; `google/gemini-embedding-005` ~0,15
- SAIBA que MRL trunca no servidor para `--embedding-dim`; dimensões nativas maiores ficam baratas truncadas em 1024
- SAIBA openrouter propaga a TODOS os caminhos de embed — `remember` `remember-batch` `ingest` `recall` `edit` `restore` `hybrid-search` `deep-research` `enrich` `init` `rename-entity`

## Chave API e Verificação de Catálogo
- OBRIGATÓRIO ADICIONE chave via stdin — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- LISTE `config list-keys --json`; REMOVA `config remove-key <fingerprint> --json`; DOCTOR `config doctor --json`; PATH `config path`
- SAIBA chaves em XDG `~/.config/sqlite-graphrag/config.toml` `chmod 600`, zeroizadas no drop, NUNCA logadas
- OBRIGATÓRIO precedência — flag `--openrouter-api-key` > XDG `config add-key` > nenhuma; product env NÃO é primária nem lida no hot path
- PROIBIDO `OPENROUTER_API_KEY` ou `SQLITE_GRAPHRAG_*` como config principal; NUNCA key no histórico de shell; SEMPRE `config add-key --from-stdin`
- SEMPRE `config doctor` após adicionar chave antes de ops pagas; VERIFIQUE catálogo ao vivo com chave armazenada (NUNCA product env); confira ids da tabela de embedding; modelo inválido → exit 78

## Backends LLM Headless
- SEMPRE passe a flag de modelo explicitamente em toda invocação headless; NUNCA confie só em defaults silenciosos
- DEFAULT CODEX `gpt-5.5`; DEVE definir embedding com `--llm-backend codex --llm-model <MODEL>` e extração com `enrich --mode codex --codex-model <MODEL>`; renove OAuth com `codex login`; codex é OAUTH-ONLY — NUNCA passe `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY`
- DEFAULT CLAUDE `claude-sonnet-4-6`; DEVE definir embedding com `--llm-backend claude --llm-model <MODEL>` e extração com `enrich --mode claude-code --claude-model <MODEL>`; claude é OAUTH-ONLY — NUNCA passe `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY`
- DEFAULT OPENCODE `opencode/big-pickle`; DEVE definir embedding com `--llm-backend opencode --llm-model <MODEL>` e extração com `enrich --mode opencode --opencode-model <MODEL>` via auth própria (NÃO OAuth)
- EXTRAÇÃO OPENROUTER — DEVE usar `enrich --mode openrouter --openrouter-model <id>`; `--openrouter-model` é OBRIGATÓRIO (sem default; ausência exit 1 antes de rede); chave de `config add-key` ou `--openrouter-api-key`
- SAIBA catálogo opencode EXTERNO/dinâmico; `--opencode-model` NÃO é validado; PASSE ids vivos do OpenCode Zen; CONSULTE `opencode.ai/zen`
- SOBRESCREVA binários com `--codex-binary`, `--claude-binary`, `--opencode-binary`; AJUSTE timeouts com `--codex-timeout`, `--claude-timeout`, `--opencode-timeout`
- VALIDE codex com `--codex-model-validate` e `--codex-model-fallback <MODEL>`; LISTE com `sqlite-graphrag codex-models --json` (CODEX apenas, NÃO OpenRouter)
- TROQUE backend em rate limit com `enrich --fallback-mode codex` ou global `--llm-fallback codex,claude,none`
- SAIBA `--mode openrouter` é REST puro `/chat/completions` — NÃO exige CLI local; fatura a chave OpenRouter (leia `usage.cost`); codex/claude-code/opencode são caminhos zero-token OAuth/auth própria

## Modelos de Texto OpenRouter
- PASSE `--openrouter-model <MODEL>` desta lista em `--mode openrouter`; preços indicativos input/output USD por milhão de tokens — SEMPRE confirme via `usage.cost`
- SAIBA estes modelos servem SOMENTE extração e enrich, NUNCA embedding
- DEVE usar `openai/gpt-oss-120b` (0,059/0,18, 131k, 36 tps) como judge PADRÃO quando o usuário não especificar modelo de texto
- USE `openai/gpt-oss-120b:nitro` (0,15/0,60, 131k, 300 tps) para throughput máximo
- USE `deepseek/deepseek-v4-flash` (0,09/0,18,1M,20tps); `deepseek/deepseek-v4-flash:nitro` (0,14/0,28,1M,109tps); `deepseek/deepseek-v4-pro` (1,30/2,60,1M,26tps); `google/gemini-3.1-flash-lite` (0,95/3,00,1M,100tps)
- USE `minimax/minimax-m3` (0,30/1,20,1M,42tps); `minimax/minimax-m2.7` (0,25/1,00,205k,43tps); `minimax/minimax-m2.7:nitro` (0,30/1,20,205k,146tps); `xiaomi/mimo-v2.5` (0,10/0,28,1M,17tps); `xiaomi/mimo-v2.5-pro` (0,43/0,87,1M,29tps); `z-ai/glm-5.2` e `z-ai/glm-5.2:nitro` (preço varia; CONFIRME via `usage.cost`)
- SAIBA `:nitro` = provedor mais rápido a preço maior; VERIFIQUE `json_schema` estrito ANTES de produção; sem Structured Outputs falha com erro OpenRouter

## Flags Globais e Inventário CLI
- `--db <PATH>` — sobrescreve o banco nas superfícies de grafo; COLOQUE DEPOIS do subcomando; default via `config set db.path <PATH>` (NÃO product env); em `config`/`slots`/`cache`/`codex-models`/`completions` `--db` é no-op aceito
- `--namespace <ns>`; `--json` (SEMPRE passe); `--lang en|pt`; `--tz <TIMEZONE>`
- `--embedding-backend auto|openrouter|llm`; `--embedding-model <MODEL>` (OBRIGATÓRIO com openrouter); `--embedding-dim N` padrão 1024 MRL [8, 4096]
- `--openrouter-api-key <KEY>` — PROIBIDO no histórico de shell em produção; prefira `config add-key --from-stdin`
- `--llm-backend codex|claude|opencode|none|auto`; `--llm-model <MODEL>`; `--llm-fallback <chain>`
- `--extraction-backend`; `--openrouter-model <MODEL>` OBRIGATÓRIO em `--mode openrouter`; `--openrouter-base-url`; `--openrouter-timeout` padrão 600
- `--llm-parallelism N` fan-out embed padrão 4 clamp [1, 32] (subprocesso E JoinSet REST OpenRouter)
- `--rest-concurrency N` fan-out enrich openrouter clamp [1, 16] padrão 8; DISTINTO de `--llm-parallelism`
- `--max-concurrency N` clamp [1, 2×nCPUs]; `--llm-max-host-concurrency N`; `--llm-slot-wait-secs N` / `--llm-slot-no-wait`
- `--wait-lock SECS`; `--low-memory`; `--strict-env-clear`; `--graceful-shutdown-secs N`; `--skip-embedding-on-failure`
- `--codex-binary`, `--claude-binary`, `--opencode-binary`; `-v`/`-vv`/`-vvv`; `--quiet`/`-q` OBRIGATÓRIO em pipelines headless; NUNCA misture stderr no JSON com `&>`
- INVENTÁRIO top-level — `init` `remember` `remember-batch` `ingest` `recall` `read` `list` `forget` `purge` `rename` `split-body` `edit` `history` `restore` `hybrid-search` `health` `migrate` `namespace-detect` `optimize` `stats` `sync-safe-copy` `backup` `vacuum` `link` `unlink` `deep-research` `related` `graph` `export` `fts` `vec` `codex-models` `prune-relations` `prune-ner` `slots` `pending` `embedding` `pending-embeddings` `cleanup-orphans` `memory-entities` `cache` `delete-entity` `reclassify` `rename-entity` `merge-entities` `enrich` `reclassify-relation` `normalize-entities` `completions` `config` `help`

## CRUD Escrita
- INVOQUE `remember --name <kebab> --type <kind> --description <text>` com exatamente uma fonte — `--body`|`--body-file`|`--body-stdin`|`--graph-stdin`
- INVOQUE `remember --graph-stdin` para `{body, entities, relationships}`; ou `--graph-file` com `--body-file`; entidades `[{name, entity_type}]` kebab-case; relações `[{source, target, relation, strength}]` strength [0.0,1.0]
- OBRIGATÓRIO allowlist graph-stdin — só `name`, `entity_type` (alias `type`), `description` opcional; PROIBIDO `observations`/`aliases`/extras → exit 1
- PASSE `--strict-name`; `--force-merge` idempotente; `--replace-graph`+`--force-merge` replace total; `--dry-run` valida sem persistir; `--enqueue-enrich` SOMENTE se operador quiser entity-descriptions prioritárias (padrão OFF)
- FAÇA parse de `entities_created[]` e `enrich_recommended[]`; NUNCA ignore; se `enrich_recommended` não vazio DEVE rodar SEPARADO `enrich --operation entity-descriptions` APÓS exit 0
- VALORES `--type` — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- INVOQUE `remember-batch` 10+ via NDJSON stdin; PASSE `--transaction`; cada linha DEVE `description` não vazia e `type` na criação
- INVOQUE `ingest <DIR> --recursive --pattern "*.md" --mode none` body-only depois enrich SEPARADO; `ingest --mode` `none`|`claude-code`|`codex`|`opencode` (non-none extrai inline sem enrich separado para esses bindings)
- USE `--resume`; `--retry-failed`; `--auto-describe`; `--name-prefix`; `--force-merge` no ingest (dedup `body_hash`); auto-divide corpos oversized
- INVOQUE `split-body --name <N>` acima de 25000 chars; `--batch --threshold 25000` para todos; FILHAS NÃO EMBEDADAS INLINE — passo1 openrouter + `--llm-backend none split-body`; passo2 SEPARADO `enrich --operation re-embed --target memories`
- RESPEITE 512000 bytes e 512 chunks; NUNCA misture fontes de corpo; NUNCA `fd | xargs remember` — USE `ingest`; SEMPRE `--llm-backend none` na escrita OpenRouter

## CRUD Leitura Atualização Exclusão
- INVOQUE `read --name <kebab> --json`; PASSE `--with-graph`; USE `--format raw` para corpo puro
- INVOQUE `list --type <kind> --limit N --offset N --json`; `history --name <n> --diff --json`
- INVOQUE `edit --name <n> --body-file <path>` ou `--description`/`--memory-type`; USE `--force-reembed`; USE `--expected-updated-at <ts>` (exit 3 = conflito — recarregue e retente)
- INVOQUE `rename --name <old> --new-name <new>`; `restore --name <n> --version <N>` (escrita — OpenRouter embed + `--llm-backend none`, depois enrich SEPARADO)
- INVOQUE `forget --name <n>`; hard-delete `purge --yes --dry-run` depois remova `--dry-run`
- OBRIGATÓRIO — `purge --yes` sozinho mantém retenção 90 dias; NÃO apaga imediatamente
- OBRIGATÓRIO purge imediato — `purge --yes --now` (alias `--retention-days 0`) ou `purge --yes --retention-days 0`
- SEMPRE dry-run primeiro `purge --now --dry-run --json`; depois `cleanup-orphans --yes` e `vacuum --json`
- NUNCA pule optimistic locking em pipelines concorrentes; NUNCA delete via shell `sqlite3`

## Grafo de Entidades
- INVOQUE `link --from <a> --to <b> --relation <type> --create-missing --weight <float>`; DEVE usar `link --from-id <N> --to-id <M>` quando IDs forem conhecidos; NUNCA dígitos puros como nomes `--from`/`--to`
- INVOQUE `unlink --from <a> --to <b> --relation <type>` ou `--entity <name> --all`; `unlink --memory <name> --entity <name>` para binding memória-entidade
- INVOQUE `graph entities --json` via `.entities[]` (NÃO `.items[]`); ORDENE `--sort-by name|degree|created-at`; PAGINE `--limit`/`--offset`
- INVOQUE `graph stats --json`; `graph traverse --from <root> --depth <N> --json`; EXPORTE `--format json|dot|mermaid --output <path>`
- DEVE passar `--fuzzy` em traverse com nome curto ambíguo; SEM `--fuzzy`, exit 4 inclui sugestões — SEMPRE use-as
- INVOQUE `rename-entity --name <old> --new-name <new>` ou `--id <N> --new-name <new>`
- INVOQUE `delete-entity --name <n> --cascade`; `merge-entities --names "a,b,c" --into <target>` ou `--ids 12,17 --into-id 3`
- NUNCA coloque `--into-id` dentro de `--ids` nem `--into` dentro de `--names`; merges auto-referenciais REJEITADOS ANTES do DB
- SEMPRE USE arrays de shell para listas dinâmicas de merge; PASSE `--cross-namespace` só quando intencional
- INVOQUE `reclassify --name <n> --new-type <kind>` ou `--from-type <old> --to-type <new> --batch`; `reclassify-relation --from-relation <old> --to-relation <new> --batch` com `--literal-from`/`--literal-to`
- INVOQUE `prune-relations --relation mentions --dry-run` depois `--yes`; `normalize-entities --yes`; `prune-ner --entity <n>` ou `--all --yes`; `graph recompute-degree --json` após delete/merge/prune (grau NÃO auto)
- INVOQUE `memory-entities --name <memory>` ou `--entity <name>`; parse `entities[].{name, description, entity_type}` — `description` OBRIGATÓRIA no envelope (vazia se ausente); SEMPRE apresente quando existir
- TIPOS canônicos — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`; fold via `map_to_canonical` (`module`→`concept`); graph-stdin ACEITA dobrados
- VALIDE nomes — mín 2 chars, sem newlines, sem ALL_CAPS ≤4, REJEITE dígitos puros; NUNCA `mentions` como relação padrão; escritas ADITIVAS sem teto de grau; NORMALIZE só via prune/merge/normalize

## Busca GraphRAG
- USE padrão de três camadas — `hybrid-search` depois `read --name` depois `related` ou `graph traverse`
- INVOQUE `recall <query> --k N` KNN semântico puro; PASSE `--no-graph`, `--precise`, `--max-distance <f>`, `--max-graph-results N`, `--all-namespaces`
- INVOQUE `hybrid-search <query> --k N` fusão FTS5 + KNN via RRF; PASSE `--rrf-k 60`; `--weight-vec 1.0 --weight-fts 1.0`; `--fallback-fts-only` offline
- USE `--with-graph --max-hops 2 --min-weight 0.3`; LEIA `results[]` E `graph_matches[]`
- INVOQUE `related <name> --hops N --relation <type>`
- INVOQUE `deep-research "<query>" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies`; um token expande em subconsultas; controle manual `--sub-query-strategy manual --sub-queries-file PATH`
- ESCREVA envelopes grandes com `--output PATH` ou `-o PATH` (atomwrite); FAÇA parse do ack `{written, bytes, blake3}`; PASSE `--quiet`; NUNCA `&>`
- OBRIGATÓRIO fail-fast — com `-o`/`--output` o arquivo DEVE existir com bytes > 0 após exit 0; NUNCA declare sucesso sem o ack
- Fórmula DEEP-RESEARCH — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 deep-research "question" --k 20 --max-hops 3 -o /tmp/dr.json --json`
- AJUSTE com `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--max-cost-usd`, `--timeout`
- FAÇA parse `recall` → `results[].{name, snippet, distance, score, source}`; `hybrid-search` → `results[].{name, combined_score, vec_rank, fts_rank}`; `deep-research` → `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context`, `stats`
- NUNCA confunda `distance` com `combined_score`; NUNCA aumente hops sem `graph stats` primeiro

## Enrich e Entity-Connect
- INVOQUE `enrich --operation <op> --mode <backend>` — AMBAS OBRIGATÓRIAS para ops LLM; omitir `--mode` → exit 2; EXCETO inspetores read-only `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` e `--dry-run` (podem omitir `--mode`)
- Ops que PERSISTEM — `memory-bindings`, `augment-bindings` (EXIGE `--names`/`--memory-names`/`--names-file`), `entity-descriptions`, `body-enrich`, `re-embed`, `weight-calibrate`, `relation-reclassify`, `entity-connect`, `entity-type-validate`, `description-enrich`, `cross-domain-bridges`, `domain-classify`, `deep-research-synth`, `body-extract` + `--body-extract-graph-only`
- SCAN/REPORT — `graph-audit` (NÃO muta estrutura)
- `--mode` válidos — `codex`, `claude-code`, `opencode`, `openrouter`; PASSE `--codex-model`/`--claude-model`/`--opencode-model`/`--openrouter-model` conforme o modo
- SAIBA `--mode openrouter` exige `--openrouter-model`, chave XDG ou `--openrouter-api-key`, REST `/chat/completions` com json_schema estrito e `provider.require_parameters` true, faturado via `usage.cost`
- PASSE `--limit N --resume` em `re-embed`; `--retry-failed`; `--dry-run`; `--target memories|entities|chunks|all` só em `re-embed` (padrão `memories`); `re-embed` seleciona vetores AUSENTES, blobs VAZIOS ou dim DIVERGENTE
- PASSE `--min-output-chars N` em `body-enrich`; `--fallback-mode codex` em rate limits Claude
- OBRIGATÓRIO filtros — `--entity-names a,b` para ops por entidade; `--memory-names a,b` para ops por memória; `--names` alias BC; empty-match → parse `matched=0` + `hint` e PARE
- OBRIGATÓRIO --force-redescribe em `entity-descriptions` para re-scan de descriptions vazias OU genéricas de baixa qualidade; padrão write-once para não vazias
- ENTITY-DESCRIPTIONS openrouter — `sqlite-graphrag enrich --operation entity-descriptions --mode openrouter --openrouter-model openai/gpt-oss-120b --force-redescribe --entity-names jwt,auth-svc --json`
- ENTITY-DESCRIPTIONS codex — `sqlite-graphrag enrich --operation entity-descriptions --mode codex --codex-model gpt-5.5 --force-redescribe --entity-names jwt --json`
- ENTITY-DESCRIPTIONS claude — `sqlite-graphrag enrich --operation entity-descriptions --mode claude-code --claude-model claude-sonnet-4-6 --force-redescribe --entity-names jwt --json`
- ENTITY-DESCRIPTIONS opencode — `sqlite-graphrag enrich --operation entity-descriptions --mode opencode --opencode-model opencode/big-pickle --force-redescribe --entity-names jwt --json`
- PASSE `--quality-sample N` com `--status` para `quality_pct` e `scan_backlog_low_grounding_est` (flag > XDG `enrich.entity_description.quality_sample` > 50; `0` desliga)
- SAIBA isolamento de fila — drain só da `operation` selecionada; memory-only NÃO reivindica `pair:`/`entity:`/`chunk:`; `state` ACEITA `draining`|`cooldown`|`pending-scan`|`blocked_dead` (dívida até `--list-dead`/`--requeue-dead`/prune)
- NUNCA múltiplos `enrich` no mesmo banco (singleton por namespace); REST SOMENTE `--rest-concurrency` em UM processo; PASSE `--until-empty` até vazio ou `--max-runtime` 3600; `--max-attempts` padrão 8 (1..=20)
- PASSE `--status` para `scan_backlog`/`unbound_backlog`/fila/`eligible_now`/`waiting`/`quality_pct`/`state` — SEM LLM; `scan_backlog` = DB real ≠ sidecar `queue_pending`
- PASSE `--list-dead`; `--requeue-dead`; `--list-skipped`; `--requeue-skipped` (skipped/`preservation_failed` sem SQL); `--ignore-backoff`; `--prune-dead-orphans`; `--prune-dead-entity-orphans` (exclusivos); `--reset-stale-claims` após `kill -9`
- SAIBA dead-letter Transient vs HardFailures; truncados (`finish_reason`=`length`) reemitem com `max_tokens` AUMENTADO; fila sidecar `.enrich-queue.sqlite`; `eligible_now==0`+`queue_pending>0`=COOLDOWN; `draining` travado→`--reset-stale-claims`; `blocked_dead`→requeue/prune PRIMEIRO
- STATUS — `sqlite-graphrag enrich --status --quality-sample 50 --json`
- UNTIL-EMPTY — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --max-attempts 8 --rest-concurrency 8 --json`
- MEMORY-BINDINGS — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --memory-names mem-a,mem-b --json`
- BACKFILL re-embed — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 enrich --operation re-embed --target all --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --json` depois `health --json`
- SAIBA `entity-connect` PERSISTE via `entity_connect_seen` (`related`|`none`); `cross-domain-bridges` MESMO scan/drain; scan O(k) coocorrência+hub×ilha — SEGURO; NUNCA cartesiano; chaves `pair:{id1}:{id2}` `item_type=entity_pair`
- SAIBA scan sob `--max-runtime` e soft ~120s `InterruptHandle`; interrupt → Timeout exit 1 (NÃO exit 75); parse `budget_exhausted` e `preempted_for_gate`; PASSE `--anchor-memory` e `--entity-names`; empty → `matched=0`+`hint`
- SEMPRE `--until-empty`; SEMPRE `--status` entre corridas longas; SEMPRE dry-run primeiro em produção
- ENTITY-CONNECT dry-run — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --dry-run --limit 50 --json`
- ENTITY-CONNECT openrouter — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 600 --rest-concurrency 8 --json`
- ENTITY-CONNECT codex — `sqlite-graphrag enrich --operation entity-connect --mode codex --codex-model gpt-5.5 --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT claude — `sqlite-graphrag enrich --operation entity-connect --mode claude-code --claude-model claude-sonnet-4-6 --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT opencode — `sqlite-graphrag enrich --operation entity-connect --mode opencode --opencode-model opencode/big-pickle --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT ancorado — `sqlite-graphrag enrich --operation entity-connect --mode openrouter --openrouter-model openai/gpt-oss-120b --anchor-memory <mem> --until-empty --max-runtime 600 --json`
- ENTITY-CONNECT bridges — mesmas fórmulas com `--operation cross-domain-bridges`
- SAIBA linhas legadas sem `pair:` são ignoradas; prioridade memory-bindings → entity-descriptions → entity-connect; EC longo DEVE ceder cooperativamente

## Escrever Depois Enrich — Templates
- TRATE toda escrita como PASSO 1 e depois PASSO 2 DISTINTO; NUNCA encadeie com `&&`
- PREFIXO embed OpenRouter — `sqlite-graphrag --embedding-backend openrouter --embedding-model <EMB> --embedding-dim 1024 --llm-backend none`
- PADRÃO `<EMB>` = `qwen/qwen3-embedding-8b`; GRATUITO = `nvidia/llama-nemotron-embed-vl-1b-v2:free`
- PASSO 1 SEMPRE exit 0 antes do PASSO 2; SEMPRE parse `entities_created` e `enrich_recommended` no remember
- REMEMBER — `echo '{"body":"text","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | <PREFIX> remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --json`
- REMEMBER hot-set — REMEMBER + `--enqueue-enrich` para entity-descriptions prioritárias
- REMEMBER-BATCH — `<PREFIX> remember-batch --transaction --json` NDJSON stdin; PASSE `--enqueue-enrich` após batch OK
- INGEST — `<PREFIX> ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- EDIT — `<PREFIX> edit --name <n> --body-file new.md --json`
- RESTORE — `<PREFIX> restore --name <n> --version 2 --json`
- PASSO 2 — escolha UM backend; SEMPRE flags de modelo explícitas; APLIQUE após remember/remember-batch/ingest/edit/restore
- CODEX — `sqlite-graphrag enrich --operation memory-bindings --mode codex --codex-model gpt-5.5 --json`
- CLAUDE — `sqlite-graphrag enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- OPENCODE — `sqlite-graphrag enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- OPENROUTER — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- OBRIGATÓRIO matriz — CADA escrita (remember, remember-batch, ingest, edit, restore) DEVE PASSO 1 OpenRouter + UM dos quatro PASSO 2; NUNCA omita flags de modelo
- REMEMBER→CODEX — PASSO1 + memory-bindings codex/`gpt-5.5` + se recomendado `enrich --operation entity-descriptions --mode codex --codex-model gpt-5.5 --entity-names <list> --json`
- REMEMBER→CLAUDE — PASSO1 + memory-bindings claude-code/`claude-sonnet-4-6` + entity-descriptions `--mode claude-code --claude-model claude-sonnet-4-6`
- REMEMBER→OPENCODE — PASSO1 + memory-bindings opencode/`opencode/big-pickle` + entity-descriptions `--mode opencode --opencode-model opencode/big-pickle`
- REMEMBER→OPENROUTER — PASSO1 + memory-bindings openrouter/`openai/gpt-oss-120b` + entity-descriptions `--mode openrouter --openrouter-model openai/gpt-oss-120b --force-redescribe` se qualidade baixa
- REMEMBER-BATCH/INGEST/EDIT/RESTORE → mesmos quatro PASSO 2 (só PASSO 1 muda); PASSO 2 NÃO exige `--llm-backend`; embed flags só em `re-embed`; chave flag > XDG; NUNCA chave crua; PROIBIDO product env primária

## Embedding e Enrich Paralelos
- ESCALE embed com `--llm-parallelism N` no PASSO 1 (JoinSet N requests OpenRouter, ordem preservada)
- ESCALE enrich com `--rest-concurrency N` + `--until-empty` no PASSO 2 openrouter (N chat calls; SQLite serial via claim WAL)
- CLAMPE `--llm-parallelism` 1..32 e `--rest-concurrency` 1..16; pagos DEVEM 4..16; `:free` ~20 req/min → N baixo; múltiplas chaves NÃO aumentam capacidade
- NUNCA lance N processos enrich; UM processo com `--rest-concurrency` é OBRIGATÓRIO
- Headless codex/claude/opencode NÃO usam fan-out `--rest-concurrency` da mesma forma; NUNCA multiplique processos enrich
- REMEMBER PASSO 1 paralelo — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 --llm-parallelism 8 --llm-backend none remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --enqueue-enrich --json`
- PASSO 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --rest-concurrency 8 --until-empty --max-runtime 3600 --max-attempts 8 --json`
- PASSO 2 codex — `sqlite-graphrag enrich --operation memory-bindings --mode codex --codex-model gpt-5.5 --until-empty --max-runtime 3600 --json`
- PASSO 2 claude — `sqlite-graphrag enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --until-empty --max-runtime 3600 --json`
- PASSO 2 opencode — `sqlite-graphrag enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --until-empty --max-runtime 3600 --json`
- REMEMBER-BATCH PASSO 1 paralelo — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 --llm-parallelism 12 --llm-backend none remember-batch --transaction --json`
- REMEMBER-BATCH PASSO 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --rest-concurrency 12 --until-empty --max-runtime 3600 --json`
- INGEST PASSO 1 paralelo — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 1024 --llm-parallelism 6 --llm-backend none ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- INGEST PASSO 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b:nitro --rest-concurrency 12 --until-empty --max-runtime 7200 --max-attempts 8 --json`
- EDIT PASSO 1 paralelo — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 --llm-parallelism 8 --llm-backend none edit --name <n> --body-file new.md --json`
- EDIT PASSO 2 — mesmo PASSO 2 openrouter do remember
- RESTORE PASSO 1 paralelo — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 --llm-parallelism 8 --llm-backend none restore --name <n> --version 2 --json`
- RESTORE PASSO 2 — mesmo PASSO 2 openrouter do remember
- MONITORE `enrich --status --json` até `scan_backlog` `queue_pending` `eligible_now` = 0; `queue_dead` é dívida até requeue/prune

## Fórmulas de Leitura
- INIT — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 1024 init --namespace <ns>`
- RECALL — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 1024 recall "query" --k 10 --json`
- HYBRID-SEARCH — `sqlite-graphrag --embedding-backend openrouter --embedding-model baai/bge-m3 --embedding-dim 1024 hybrid-search "query" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- DEEP-RESEARCH — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model openai/text-embedding-3-small --embedding-dim 1024 deep-research "question" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies -o /tmp/research.json --json`
- MEMORY-ENTITIES — `sqlite-graphrag memory-entities --name <memory> --json` e parse `entities[].description`
- RENAME-ENTITY — `sqlite-graphrag --embedding-backend openrouter --embedding-model perplexity/pplx-embed-v1-0.6b --embedding-dim 1024 rename-entity --name <old> --new-name <new> --json`
- HYBRID offline — `sqlite-graphrag hybrid-search "query" --k 10 --fallback-fts-only --json`
- TRAVERSE fuzzy — `sqlite-graphrag graph traverse --from <short-alias> --depth 2 --fuzzy --json`
- LINK por ID — `sqlite-graphrag link --from-id <N> --to-id <M> --relation uses --json`
- GUARDA MERGE — NUNCA `merge-entities --ids 3,12 --into-id 3`; SEMPRE exclua o survivor de `--ids`
- VERIFIQUE catálogo só com chave de `config add-key`/doctor — NUNCA product env

## Diagnóstico Manutenção Exit Codes Concorrência XDG
- INIT `sqlite-graphrag init --namespace <ns>`; HEALTH `health --json` `{integrity_ok, schema_version, vec_*_missing, vec_*_coverage_pct}` — se missing>0 DISPARE `enrich --operation re-embed --target <target>`
- MIGRATE `migrate --dry-run --json` depois `migrate --json`; OPTIMIZE `optimize --json`; VACUUM `vacuum --json` após purge; FTS `fts check|stats|rebuild --json` se `fts_degraded`; VEC `vec orphan-list`/`purge-orphan`/`stats`
- EMBEDDING `embedding --status --json` (alias `pending-embeddings --status`); REPROCESE via `enrich --operation re-embed`; SLOTS `slots status`/`release --slot-id <N> --yes`; PENDING `pending list|show|cleanup`
- EXPORT `export --namespace <ns> --type <kind> --json`; STATS `stats --json`; BACKUP `backup --output backup.sqlite --json`; SNAPSHOT `sync-safe-copy --dest <path>`
- INSPECT `namespace-detect`, `cache list|stats`, `cache clear-models --yes`; COMPLETIONS `completions bash|zsh|fish|elvish|powershell`
- AGENDE semanal — `purge --yes` (90d) ou `purge --yes --now` → `cleanup-orphans` → `prune-relations --relation mentions` → `vacuum` → `optimize` → `sync-safe-copy`; corrupção `sqlite3 broken.sqlite ".recover" | sqlite3 repaired.sqlite`
- EXIT densos — 0 ok; 1 validação/Timeout wall-clock (EC InterruptHandle, NÃO 75); 2 args; 3 lock otimista (recarregue); 4 not found (+sugestões); 5 namespace; 6 payload grande (DIVIDA); 9 duplicata (`--force-merge`); 10 banco (`vacuum`+`health`); 11 embed (backend/dim1024/chave); 13 batch parcial (só falhos); 14 I/O; 15 busy (`--wait-lock`); 16 preflight MCP (NÃO transitório); 19 SHUTDOWN retry OBRIGATÓRIO; 20 interno; 75 slots/singleton (cooldown, NUNCA retente já); 77 RAM; 78 config (chave/modelo); NUNCA ignore non-zero; NUNCA reprocessar batch inteiro após 13; NUNCA confunda 1 com 75 ou 9
- RESPEITE teto `2 x nCPUs` em `init`/`remember`/`ingest`/`recall`/`hybrid-search`; `--llm-parallelism N` padrão 4 em `remember`/`edit`, 2 em `ingest`, clamp [1,32]
- SAIBA JOB SINGLETON — `enrich` e `ingest --mode codex|claude-code` por namespace; USE `--wait-job-singleton SECS` ou `--force-job-singleton` para lock stale
- HABILITE unitário via `--low-memory` ou XDG `config set`; PROIBIDO product env; NUNCA `enrich` paralelo no mesmo DB; REST via `--rest-concurrency` NÃO multi-processo
- OBRIGATÓRIO precedência — flag CLI > XDG `config set` > default; PROIBIDO `SQLITE_GRAPHRAG_*` no hot path; PROIBIDO telemetria de produto
- OBRIGATÓRIO chaves — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`; `config set|get|list|unset|path|doctor`; `config list --effective --json`
- OBRIGATÓRIO URLs — `config set network.openrouter.chat_url https://openrouter.ai/api/v1/chat/completions` e `config set network.openrouter.embeddings_url https://openrouter.ai/api/v1/embeddings`; aliases `network.chat_url`, `network.embed_url`
- OBRIGATÓRIO `llm.query_embed_timeout_secs` default 3s; `config set enrich.entity_description.quality_sample 50`
- OBRIGATÓRIO chaves XDG — `db.path`, `embedding.dim`, `embedding.backend`, `embedding.model`, `llm.backend`, `llm.model`, `llm.query_embed_timeout_secs`, `display.tz`, `i18n.lang`, `log.level`, `log.format`, `spawn.skip_preflight` (emergências), `enrich.yield_every_n_items`
- SEMPRE prefira flags one-shot (`--db`, `--namespace`, `--embedding-backend`, …) para agentes; XDG só defaults do host; NUNCA `ANTHROPIC_API_KEY`/`OPENAI_API_KEY` para codex/claude — PROIBIDOS, abortam spawn exit 1

## Regras Ativas
- SEMPRE `--json` + `jaq` após capture (NUNCA pipe NDJSON; NUNCA `jq`); SEMPRE parse `backend_invoked`
- SEMPRE `--embedding-backend openrouter --embedding-model <MODEL> --embedding-dim 1024` em embed com chave `config add-key` ou `--openrouter-api-key`
- SEMPRE `--llm-backend none` nas escritas; SEMPRE `enrich` SEPARADO com `--mode`+modelo; NUNCA `&&`; SEMPRE parse `entities_created`/`enrich_recommended` e rode entity-descriptions quando recomendado ou com `--enqueue-enrich`
- SEMPRE prefira `--entity-names`/`--memory-names`; empty-match `matched=0`+`hint`; SEMPRE `--force-redescribe` em baixa qualidade; SEMPRE `quality_pct` via `--status --quality-sample`
- SEMPRE trate `blocked_dead` até requeue/prune; SEMPRE parse EC `budget_exhausted`/`preempted_for_gate`; SEMPRE parse `memory-entities` `entities[].description`; SEMPRE `-o`/`--output` em deep-research grande
- SEMPRE renove OAuth (`codex login`/claude) se stale; SEMPRE dim 1024 (mismatch → exit 11); SEMPRE arrays de shell para `merge-entities`
- DEVE `--from-id`/`--to-id` para IDs de link; DEVE `--fuzzy`/sugestões em traverse curto; DEVE `-o`+`--quiet` em deep-research grande; NUNCA `&>`
- NUNCA API keys a codex/claude (OAuth-only exit 1); NUNCA `--llm-backend codex` em escrita OpenRouter; NUNCA `enrich` paralelo no mesmo DB; NUNCA `.sqlite` fora do binário
- NUNCA ignore exit 19 (retry OBRIGATÓRIO) ou 16 (MCP); NUNCA openrouter sem modelo+chave (78); NUNCA merge self-ref; NUNCA MCP/MEMORY.md; NUNCA documente `SQLITE_GRAPHRAG_*`; NUNCA prosa de histórico de versões
- TIPOS memória — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- TIPOS entidade — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- RELAÇÕES — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
