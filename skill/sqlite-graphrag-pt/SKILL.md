---
name: sqlite-graphrag
description: Esta skill DEVE ativar para toda operação da CLI sqlite-graphrag cobrindo memória persistente, grafo de conhecimento GraphRAG, ligação de entidades, hybrid-search, recall, deep-research, remember, remember-batch, ingest, edit, restore, enrich, forget, purge, link, unlink, merge-entities, rename-entity, reclassify, graph traverse, preços de modelos de embedding OpenRouter, gestão de chave de API, backends headless codex claude opencode, modelos de texto OpenRouter para enrich, fórmulas write-then-enrich, embedding paralelo, códigos de saída, concorrência, env vars, fusão FTS5 mais cosine BLOB, tipos e relações canônicas, isolamento de namespace e regras OAuth-only. Esta skill DEVE ser usada sempre que o agente armazena, recupera, busca, enriquece, liga, mescla ou mantém memória GraphRAG de longo prazo. Palavras-chave sqlite-graphrag GraphRAG memory embedding openrouter codex claude opencode remember recall hybrid-search ingest enrich deep-research forget purge link rename-entity merge-entities
---


## Quando Esta Skill Ativa
- DEVE ATIVAR quando o usuário pede para lembrar, salvar, recordar, recuperar, buscar ou persistir algo entre sessões
- DEVE ATIVAR para contexto de longo prazo, grafo de conhecimento, GraphRAG, RAG, ligação de entidades, gestão de memória e conhecimento por namespace
- DEVE ATIVAR quando sqlite-graphrag, embedding, FTS5, hybrid-search, OpenRouter, codex, claude, opencode ou memória LLM for mencionado
- DEVE ATIVAR para enrich, re-embed, link, unlink, merge-entities, rename-entity, deep-research, ingest e manutenção de grafo
- NUNCA ATIVE para dados efêmeros pontuais, I/O simples de arquivo ou tarefas sem relação a contexto persistente
- SEMPRE carregue esta skill antes de inventar arquivos de memória ad-hoc, servidores MCP de memória ou diários Markdown manuais


## Modelo Mental Central
- SAIBA TRÊS seletores independentes; NUNCA os confunda
- SELETOR 1 — `--embedding-backend` COMO os vetores são produzidos — `openrouter` (REST), `llm` (subprocesso) ou `auto`
- SELETOR 2 — `--llm-backend` QUAL subprocesso embeda quando backend é `llm` — `codex`, `claude`, `opencode` ou `none`
- SELETOR 3 — extração via `enrich --mode` — `codex`, `claude-code`, `opencode` ou `openrouter` (REST `/chat/completions`); `--extraction-backend` é o seletor global relacionado
- ESCRITA e ENRICH são SEMPRE separadas; escrita produz embeddings; `enrich` SEPARADO extrai o grafo
- NUNCA encadeie escrita e enrich com `&&`; SEMPRE aguarde exit 0 da escrita, depois rode enrich como processo DISTINTO
- Em TODA escrita (`remember`, `remember-batch`, `ingest`, `edit`, `restore`) DEVE passar `--llm-backend none` com `--embedding-backend openrouter` para embeddings AINDA rodarem via REST OpenRouter sem timeout de subprocesso LLM
- SEMPRE passe `--json`; SEMPRE parseie com `jaq` NUNCA `jq`; SEMPRE capture stdout PRIMEIRO depois parseie; NUNCA pipe saída da CLI direto em `jaq` (NDJSON mascara falhas como null)
- SAIBA que vetores vazios NUNCA são persistidos; PARSEIE `backend_invoked` para confirmar o backend; RODE `enrich` só após exit 0 da escrita


## Regras de Instrução para LLMs
- "lembre disso" → `remember --force-merge` com `--graph-stdin` de entidades curadas e relações canônicas, depois `enrich` SEPARADO
- "o que você sabe sobre X" → `hybrid-search "X" --k 10 --json` PRIMEIRO, depois `read --name <name> --json`
- "como X se relaciona com Y" → `graph traverse --from X --depth 2 --json` ou `related X --hops 2 --json`; em miss RETENTE `--fuzzy` ou escolha sugestões NotFound exit 4
- "pesquise profundamente sobre X" → `deep-research "X" --k 20 --max-hops 3 --json`; token único faz fan-out para sub-queries de aspecto; envelopes grandes DEVE usar `--output PATH` e `--quiet`
- ANTES de qualquer create → `hybrid-search "<name>" --k 5 --json`; se duplicata USE `--force-merge`
- APÓS create/update → capture-parse `read --name <name> --json` para `{name, description, body_length}`
- APÓS cada turno → persista achados novos ou DECLARE "Nenhum achado novo para persistir"
- Em exit não-zero → parseie `jaq '{code, message, error_class}'` e REPORTE remediação
- SEMPRE use relações canônicas — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- SEMPRE mapeie não-canônicas primeiro — `adds`/`creates`→`causes`, `implements`→`supports`, `blocks`→`contradicts`, `tested-by`→`related`, `part-of`→`applies-to`
- SEMPRE kebab-case ASCII lowercase em nomes de entidade; LIMITE a conceitos de domínio; REJEITE genéricos, pronomes, UUIDs, timestamps
- NUNCA use MCP Serena, arquivos `.md` de memória ou MEMORY.md; NUNCA inicie daemon; NUNCA passe `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` a backends de subprocesso
- PREFIRA `remember --force-merge` sobre `edit`; PREFIRA `--graph-stdin` sobre extração inline


## Arquitetura e Princípios
- INVOQUE como subprocesso; stdout = JSON/NDJSON; stderr = logs; CHEQUE exit code ANTES de parsear
- SAIBA NÃO tem daemon, NÃO tem ONNX, NÃO tem cache de modelo; cosine é pure Rust sobre BLOB `memory_embeddings` / `entity_embeddings` / `chunk_embeddings`
- SAIBA `init` ou `migrate` aplica o schema vivo; LEIA `schema_version` de `health --json`
- ENFORCE OAUTH-ONLY para codex/claude — spawn ABORTA exit 1 se `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` estiver setada; `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL` são PRESERVADAS
- SAIBA CWD do subprocesso é ISOLADO; 7 guards preflight rodam antes de cada fork LLM; exit 16 = falha preflight; `claude -p` herda `.mcp.json` do CWD — ISOLE config para `claude-code`
- DEFINA `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1` APENAS em emergências; namespace via `--namespace` ou env (padrão `global`)
- NUNCA exponha como MCP/HTTP; NUNCA escreva `.sqlite` de outra ferramenta; FUSÃO é FTS5 BM25 mais cosine BLOB KNN via RRF


## Modelos de Embedding OpenRouter e Preços
- PASSE `--embedding-model <MODEL>` quando `--embedding-backend openrouter`; NÃO existe modelo padrão, então omissão dispara exit 78
- SAIBA preços abaixo são USD por um milhão de tokens; ESCOLHA por custo e qualidade
- USE `nvidia/llama-nemotron-embed-vl-1b-v2:free` para embedding FREE de custo zero (sem cobrança de tokens)
- USE `qwen/qwen3-embedding-4b` a 0.05 USD por milhão de tokens (4B, barato e capaz)
- USE `qwen/qwen3-embedding-8b` a 0.05 USD por milhão de tokens (padrão RECOMENDADO, maior qualidade Qwen)
- USE `baai/bge-m3` a cerca de 0.05 USD por milhão de tokens
- USE `openai/text-embedding-3-small` a 0.05 USD por milhão de tokens
- USE `perplexity/pplx-embed-v1-0.6b` a 0.05 USD por milhão de tokens
- USE `mistralai/mistral-embed-2312` a 0.10 USD por milhão de tokens
- USE `google/gemini-embedding-2` a cerca de 0.12 USD por milhão de tokens
- USE `openai/text-embedding-3-large` a 0.13 USD por milhão de tokens
- USE `google/gemini-embedding-005` a cerca de 0.15 USD por milhão de tokens
- MANTENHA `--embedding-dim 384` consistente em TODAS escritas e leituras; dimensão divergente colide com o índice e falha knn com exit 11
- SAIBA MRL trunca server-side para `--embedding-dim`; dims nativas maiores continuam baratas truncadas a 384
- VERIFIQUE modelos vivos — `curl -sS https://openrouter.ai/api/v1/models -H "Authorization: Bearer $OPENROUTER_API_KEY" | jaq -c '.data[] | select(.id | test("embed";"i")) | {id, pricing}'`; NÃO confie só em ids obsoletos
- CONFIRME chaves com `sqlite-graphrag config doctor --json`; modelo inválido → exit 78; MONITORE `usage.cost` por chamada
- SAIBA openrouter se propaga a TODOS os paths de embed — `remember` `remember-batch` `ingest` `recall` `edit` `restore` `hybrid-search` `deep-research` `enrich` `init` `rename-entity`


## Gestão de Chave de API OpenRouter
- ADICIONE chave via stdin — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- LISTE chaves — `sqlite-graphrag config list-keys --json`
- REMOVA por fingerprint — `sqlite-graphrag config remove-key <fingerprint> --json`
- RODE doctor — `sqlite-graphrag config doctor --json`
- INSPECIONE path — `sqlite-graphrag config path`
- SAIBA chaves vivem em XDG `~/.config/sqlite-graphrag/config.toml` com `chmod 600` e são zeroizadas no drop, JAMAIS logadas
- SAIBA precedência — env `OPENROUTER_API_KEY` > `config.toml` > flag CLI `--openrouter-api-key`
- NUNCA passe a chave como argumento CLI em produção; PREFIRA stdin ou env para evitar histórico do shell
- SEMPRE rode `config doctor` após adicionar chave antes de qualquer embedding ou enrich pago


## Backends LLM Headless
- ESCOLHA codex com `--llm-backend codex --llm-model gpt-5.4-mini` para paths de embedding e `enrich --mode codex --codex-model gpt-5.4-mini` para extração; refresque OAuth com `codex login`; codex é OAUTH-ONLY — NUNCA passe `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY`
- ESCOLHA claude com `--llm-backend claude --llm-model claude-sonnet-4-6` para paths de embedding e `enrich --mode claude-code --claude-model claude-sonnet-4-6` para extração via path OAuth zero-token; claude é OAUTH-ONLY — NUNCA passe `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY`
- ESCOLHA opencode com `--llm-backend opencode --llm-model opencode/big-pickle` para paths de embedding e `enrich --mode opencode --opencode-model opencode/big-pickle` para extração via auth próprio (NÃO OAuth)
- ESCOLHA openrouter SOMENTE para extração com `enrich --mode openrouter --openrouter-model <id>`; `--openrouter-model` é OBRIGATÓRIA (sem default; valor ausente sai exit 1 antes de rede); chave vem de `OPENROUTER_API_KEY`
- SAIBA modelos DEFAULT — codex `gpt-5.5`, claude `claude-sonnet-4-6`, opencode `opencode/big-pickle`
- SAIBA catálogo opencode é EXTERNO/dinâmico; `--opencode-model` é SEM VALIDAÇÃO; PASSE ids vivos OpenCode Zen (default `opencode/big-pickle`); CONSULTE `opencode.ai/zen`
- SOBRESCREVA binários com `--codex-binary`, `--claude-binary`, `--opencode-binary`; AJUSTE timeouts de ingest com `--codex-timeout`, `--claude-timeout`, `--opencode-timeout`
- VALIDE modelos codex com `--codex-model-validate` e `--codex-model-fallback <MODEL>`; LISTE com `sqlite-graphrag codex-models --json` (só modelos CODEX, NÃO OpenRouter)
- TROQUE backend em rate limit com `enrich --fallback-mode codex` ou global `--llm-fallback codex,claude,none`
- SAIBA `claude-code` spawna `claude -p` herdando `.mcp.json` do CWD (pode falhar); PREFIRA codex ou isole config
- SAIBA `--mode openrouter` é REST puro `/chat/completions` — NÃO exige CLI local; cobra `OPENROUTER_API_KEY` (leia `usage.cost`); codex/claude-code/opencode são paths OAuth/own-auth zero-token


## Modelos de Texto OpenRouter para Enrich
- PASSE `--openrouter-model <MODEL>` desta tabela no `--mode openrouter`; preços entrada/saída USD por um milhão de tokens
- SAIBA estes modelos servem APENAS extração e enrichment, NUNCA embedding; a tabela de embedding é separada
- USE `openai/gpt-oss-120b` a 0.059/0.18 USD, contexto 131k, 36 tps (judge barato RECOMENDADO)
- USE `openai/gpt-oss-120b:nitro` a 0.15/0.60 USD, contexto 131k, 300 tps (throughput MAIS RÁPIDO)
- USE `deepseek/deepseek-v4-flash` a 0.09/0.18 USD, contexto 1M, 20 tps
- USE `deepseek/deepseek-v4-flash:nitro` a 0.14/0.28 USD, contexto 1M, 109 tps
- USE `deepseek/deepseek-v4-pro` a 1.30/2.60 USD, contexto 1M, 26 tps
- USE `google/gemini-3.1-flash-lite` a 0.95/3.00 USD, contexto 1M, 100 tps
- USE `minimax/minimax-m3` a 0.30/1.20 USD, contexto 1M, 42 tps
- USE `minimax/minimax-m2.7` a 0.25/1.00 USD, contexto 205k, 43 tps
- USE `minimax/minimax-m2.7:nitro` a 0.30/1.20 USD, contexto 205k, 146 tps
- USE `xiaomi/mimo-v2.5` a 0.10/0.28 USD, contexto 1M, 17 tps
- USE `xiaomi/mimo-v2.5-pro` a 0.43/0.87 USD, contexto 1M, 29 tps
- USE `z-ai/glm-5.2` e `z-ai/glm-5.2:nitro` com preço variável por provider; SEMPRE CONFIRME custo real via `usage.cost`
- SAIBA variantes `:nitro` roteiam ao provider mais rápido a preço maior
- VERIFIQUE `json_schema` strict ANTES de produção; sem Structured Outputs falha com erro explícito OpenRouter
- LEIA `usage.cost` da resposta do chat para o custo real por item


## Referência de Flags Globais
- `--db <PATH>` — sobrescrever banco; COLOQUE DEPOIS do subcomando (ex `remember --db <PATH>`); override independente de posição é env `SQLITE_GRAPHRAG_DB_PATH`
- `--namespace <ns>` — escopar a um namespace
- `--json` — saída JSON estruturada (SEMPRE passe)
- `--lang en|pt` — forçar idioma do stderr
- `--tz <TIMEZONE>` — localizar timestamps
- `--embedding-backend auto|openrouter|llm` — seletor de produção de vetor
- `--embedding-model <MODEL>` — modelo OpenRouter (OBRIGATÓRIO com openrouter)
- `--embedding-dim N` — dimensionalidade [8, 4096], padrão 384 MRL
- `--openrouter-api-key <KEY>` — chave OpenRouter (PROIBIDO no histórico de shell em produção)
- `--llm-backend codex|claude|opencode|none|auto` — backend de embedding de subprocesso; cadeia por vírgula permitida
- `--llm-model <MODEL>` — modelo do backend LLM ativo
- `--llm-fallback <chain>` — cadeia de fallback por vírgula quando o primário falha
- `--extraction-backend codex|claude-code|opencode|openrouter` — seletor de extração (openrouter é REST, não subprocesso)
- `--openrouter-model <MODEL>` — judge OBRIGATÓRIO para `--mode openrouter` (sem default; ausência sai exit 1 antes de rede)
- `--openrouter-base-url <URL>` — override opcional do endpoint OpenRouter no chat enrich
- `--openrouter-timeout <SECS>` — timeout do chat enrich, padrão 600
- `--llm-parallelism N` — fan-out de embedding, padrão 4, clamp [1, 32]; governa subprocesso E JoinSet REST OpenRouter; single-batch pequeno fica serial
- `--max-concurrency N` — cap de invocações pesadas, clamp [1, 2×nCPUs]
- `--llm-max-host-concurrency N` — cap de slots LLM no host; `--llm-slot-wait-secs N` / `--llm-slot-no-wait`
- `--wait-lock SECS` — ampliar lock; `--low-memory` paralelismo unitário; `--strict-env-clear` PATH-only no subprocesso; `--graceful-shutdown-secs N` antes do SIGKILL
- `--skip-embedding-on-failure` — armazenar sem vetor quando a cadeia termina em `none`
- `--codex-binary`, `--claude-binary`, `--opencode-binary` — overrides de path
- `-v` / `-vv` / `-vvv` — info / debug / trace no stderr
- `--quiet` / `-q` — suprime tracing não-erro no stderr; OBRIGATÓRIO em pipelines headless; NUNCA misture stderr no JSON com `&>`


## Operações CRUD de Escrita
- INVOQUE `remember --name <kebab> --type <kind> --description <text>` com exatamente uma fonte de corpo — `--body <text>` ou `--body-file <path>` ou `--body-stdin` ou `--graph-stdin`
- INVOQUE `remember --graph-stdin` para anexar `{body, entities, relationships}` em um único JSON
- INVOQUE `remember --graph-file <path>` para carregar grafo de arquivo; COMBINE com `--body-file <path>` para corpo e grafo separados
- PASSE entities como `[{name, entity_type}]` em kebab-case ASCII; PASSE relationships como `[{source, target, relation, strength}]` com strength em [0.0, 1.0]
- PASSE `--strict-name` para REJEITAR nome fora de kebab-case em vez de auto-normalizar
- PASSE `--force-merge` para updates idempotentes e restauração de soft-deleted
- PASSE `--replace-graph` junto de `--force-merge` para ZERAR vínculos existentes antes de escrever o novo grafo (replace total, não merge)
- PASSE `--dry-run` para validar sem persistir
- VALORES válidos de `--type` de memória — `user`, `feedback`, `project`, `reference`, `decision`, `incident`, `skill`, `document`, `note`
- INVOQUE `remember-batch` para 10+ memórias via NDJSON stdin; PASSE `--transaction` para all-or-nothing
- INVOQUE `ingest <DIR> --recursive --pattern "*.md" --mode none` para import body-only, depois enrich SEPARADO
- SAIBA `ingest --mode` aceita `none` (padrão body-only), `claude-code`, `codex`, `opencode`; modo não-none roda extração inline DURANTE ingest e NÃO precisa de enrich separado para os bindings daquela ingestão
- USE `--resume` após interrupção; `--retry-failed` só falhados; `--auto-describe` para sintetizar descrições
- PASSE `--name-prefix <prefixo>` no `ingest` para prefixar nomes derivados; conta no teto do nome e vale SÓ para ingestão local
- PASSE `--force-merge` no `ingest` para ATUALIZAR duplicatas em vez de pular; dedup por `body_hash`, arquivo inalterado é pulado mesmo após renomear
- SAIBA `ingest` divide nativamente corpo oversized em chunks, então arquivo acima do limite é chunkado, NÃO rejeitado
- INVOQUE `split-body --name <N>` para dividir UMA memória >25000 chars em filhas nos limites de chunk; original `SUPERCEDIDO` com arestas `replaces`; PASSE `--batch --threshold 25000` para todos oversized; FILHAS NÃO SÃO EMBEDADAS INLINE — etapa 1 `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-backend none split-body --name <n> --json`; etapa 2 SEPARADA `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 enrich --operation re-embed --target memories --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --json`
- RESPEITE 512000 bytes e 512 chunks por corpo
- NUNCA misture `--body`, `--body-file`, `--body-stdin`, `--graph-stdin` em uma invocação
- NUNCA use `fd | xargs remember`; INVOQUE `ingest`
- NUNCA passe `--llm-backend codex` (nem qualquer backend LLM diferente de `none`) em escrita; SEMPRE `--llm-backend none` com embeddings OpenRouter


## CRUD Leitura Atualização Deleção
- INVOQUE `read --name <kebab> --json` para fetch O(1); PASSE `--with-graph` para entidades vinculadas
- USE `read --name <n> --format raw` para corpo puro SEM envelope JSON ao pipar
- INVOQUE `list --type <kind> --limit N --offset N --json` para filtrar e paginar
- INVOQUE `history --name <n> --diff --json` para histórico com stats de diff
- INVOQUE `edit --name <n> --body-file <path>` para corpo, ou `--description <text>` e `--memory-type <kind>` para metadados
- USE `--force-reembed` para regenerar embedding sem mudar corpo
- USE `--expected-updated-at <ts>` para locking otimista; TRATE exit 3 como conflito — recarregue e retente
- INVOQUE `rename --name <old> --new-name <new>` preservando histórico
- INVOQUE `restore --name <n> --version <N>` (path de escrita — ainda exige embed OpenRouter + `--llm-backend none`, depois enrich SEPARADO)
- INVOQUE `forget --name <n>` para soft-delete reversível
- INVOQUE `purge --retention-days <N> --yes --dry-run` para preview, depois remova `--dry-run` para hard delete
- INVOQUE `cleanup-orphans --yes` após bulk forget, depois `vacuum --json`
- NUNCA pule locking otimista em pipelines concorrentes; NUNCA delete via shell `sqlite3`


## Operações de Grafo de Entidades
- INVOQUE `link --from <a> --to <b> --relation <type> --create-missing --weight <float>` para aresta por nome
- PREFIRA `link --from-id <N> --to-id <M> --relation <type> --weight <float>` com IDs inteiros
- NUNCA passe dígitos puros como nomes `--from` / `--to` — nomes só de dígitos são REJEITADOS, então `--create-missing` NÃO cria fantasmas numéricos
- SAIBA `--create-missing` vale só para link por nome; link por ID EXIGE entidades pré-existentes
- INVOQUE `unlink --from <a> --to <b> --relation <type>` para uma aresta, ou `--entity <name> --all` para todas de uma entidade
- INVOQUE `unlink --memory <name> --entity <name>` para um vínculo memória-entidade sem tocar arestas entidade-entidade
- INVOQUE `graph entities --json` via `.entities[]` (NÃO `.items[]`); ORDENE `--sort-by name|degree|created-at` com `--order asc|desc`; PAGINE `--limit N --offset N`
- INVOQUE `graph stats --json` para `node_count`, `edge_count`, `avg_degree`, `max_degree`
- INVOQUE `graph traverse --from <root> --depth <N> --json`; EXPORTE `--format json|dot|mermaid --output <path>`
- PASSE `--fuzzy` em `graph traverse` para auto-resolver apelido curto com vencedor claro; SEM `--fuzzy`, exit 4 NotFound inclui sugestões Jaro-Winkler e prefixo — SEMPRE use-as
- INVOQUE `rename-entity --name <old> --new-name <new>` ou `--id <N> --new-name <new>` para desambiguar entre namespaces
- INVOQUE `delete-entity --name <n> --cascade` para entidade e arestas
- INVOQUE `merge-entities --names "a,b,c" --into <target>` ou `merge-entities --ids 12,17 --into-id 3`; modos nome e ID conflitam; IDs globalmente únicos
- NUNCA coloque `--into-id` dentro de `--ids` nem `--into` dentro de `--names`; merges auto-referenciais são REJEITADOS ANTES de qualquer trabalho no DB (inclusive `--cross-namespace`); PROTEGE contra word-splitting do zsh injetando o survivor na lista de origem
- SEMPRE USE arrays de shell para listas dinâmicas — `ids=(12 17); survivor=3; sqlite-graphrag merge-entities --ids "$(IFS=,; printf '%s' "${ids[*]}")" --into-id "$survivor" --json` — SEMPRE quote a string joinada
- PASSE `--cross-namespace` para resolver IDs em TODOS os namespaces (opt-in); survivor herda namespace do `--into-id`
- INVOQUE `reclassify --name <n> --new-type <kind>` ou `--from-type <old> --to-type <new> --batch`
- INVOQUE `reclassify-relation --from-relation <old> --to-relation <new> --batch`; FILTRE com `--filter-source-type` e `--filter-target-type`; PASSE `--literal-from` / `--literal-to` para match/write VERBATIM sem kebab (mutuamente exclusivo com `--from-relation`); migração CANÔNICA underscore — `reclassify-relation --literal-from applies_to --literal-to applies-to --batch` após `--dry-run`; REPITA para `depends_on` e `tracked_in`
- INVOQUE `prune-relations --relation mentions --dry-run` depois remova `--dry-run` com `--yes`
- INVOQUE `normalize-entities --yes`; `prune-ner --entity <n>` ou `prune-ner --all --yes`
- INVOQUE `memory-entities --name <memory>` forward ou `--entity <name>` reverse
- SAIBA escritas no grafo são ADITIVAS sem cap de grau; NORMALIZE só via `prune-relations`, `merge-entities`, `normalize-entities` — NUNCA durante escrita
- INVOQUE `graph recompute-degree --json` após `delete-entity`, `merge-entities` ou `prune-relations` (grau NÃO é auto-recalculado); envelope `{total, updated, zeroed, unchanged}` com soma = `total`; PASSE `--dry-run` para preview
- TIPOS canônicos de entidade — `project`, `tool`, `person`, `file`, `concept`, `incident`, `decision`, `memory`, `dashboard`, `issue_tracker`, `organization`, `location`, `date`
- SAIBA `entity_type` inválido falha CEDO listando os 13 valores antes de escrita no DB
- VALIDE nomes — min 2 chars, sem newlines, sem ALL_CAPS curto ≤4, REJEITE dígitos puros
- NUNCA use `mentions` como relação padrão


## Operações de Busca GraphRAG
- USE o padrão de três camadas — `hybrid-search` depois `read --name` depois `related` ou `graph traverse`
- INVOQUE `recall <query> --k N` para KNN semântico; PASSE `--no-graph`, `--precise`, `--max-distance <f>`, `--max-graph-results N`, `--all-namespaces`
- INVOQUE `hybrid-search <query> --k N` para fusão FTS5 + KNN via RRF
- PASSE `--rrf-k 60` padrão; `--weight-vec 1.0 --weight-fts 1.0` balanceado
- PASSE `--fallback-fts-only` para só FTS5 BM25 offline
- USE `--with-graph --max-hops 2 --min-weight 0.3`; LEIA `results[]` E `graph_matches[]`
- INVOQUE `related <name> --hops N --relation <type>` multi-hop a partir de memória
- INVOQUE `deep-research "<query>" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies` multi-hop paralelo
- SAIBA token único faz fan-out multi-aspecto cujo campo `source` é igual a `aspect` (facetas EN/PT); PARSEIE `sub_queries[].source` e ESPERE `aspect` em sujeitos de token único; controle manual com `--sub-query-strategy manual --sub-queries-file PATH`
- ESCREVA envelopes grandes com `--output PATH` (atomwrite); PARSEIE ack curto no stdout com `written`, `bytes` e `blake3`; PASSE `--quiet` / `-q` global headless; NUNCA use `&>` (mistura stderr no JSON)
- AJUSTE com `--graph-decay <f>`, `--graph-min-score <f>`, `--max-neighbors-per-hop N`, `--max-cost-usd <f>`, `--timeout <secs>`
- PARSEIE `recall` → `results[].{name, snippet, distance, score, source}`
- PARSEIE `hybrid-search` → `results[].{name, combined_score, vec_rank, fts_rank}`
- PARSEIE `deep-research` → `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context`, `stats`
- NUNCA confunda `distance` com `combined_score`; NUNCA aumente hops sem `graph stats`


## Operações de Enrich
- INVOQUE `enrich --operation <op> --mode <backend>` com AMBAS flags OBRIGATÓRIAS para ops LLM; omitir `--mode` → exit 2 — EXCETO inspetores read-only `--status`, `--list-dead`, `--requeue-dead`, `--prune-dead-orphans` (memory-keyed) e `--prune-dead-entity-orphans` (entity-keyed), que NÃO exigem `--operation` e `--mode`
- Ops FULLY-IMPLEMENTED (persistem) — `memory-bindings` (entidades→memórias unbound), `augment-bindings` (vínculos extras em já vinculadas; exige `--names` ou `--names-file`), `entity-descriptions`, `body-enrich`, `re-embed` (só vetores), `entity-connect` (PERSISTE via `entity_connect_seen` com `related`/`none`; hub-first; `--until-empty`), `cross-domain-bridges` (mesmo path `entity_connect_seen`), `body-extract` + `--body-extract-graph-only` (só grafo)
- Ops SCAN-ONLY (só relatório) — `weight-calibrate`, `relation-reclassify`, `entity-type-validate`, `description-enrich`, `domain-classify`, `graph-audit`, `deep-research-synth`
- Valores de `--mode` — `codex`, `claude-code`, `opencode`, `openrouter`
- USE `augment-bindings` para MAIS vínculos em memórias JÁ vinculadas; EXIGE `--names` ou `--names-file`
- USE `body-extract --body-extract-graph-only` para grafo sem reescrever corpo
- PASSE `--codex-model`, `--claude-model`, `--opencode-model` ou `--openrouter-model` conforme o modo
- SAIBA `--mode openrouter` exige `--openrouter-model`, chave de `OPENROUTER_API_KEY`, REST `/chat/completions` com json_schema strict e `provider.require_parameters` true, cobrado via `usage.cost`; outros modos OAuth/own-auth zero-token
- PASSE `--limit N --resume` em `re-embed`; `--retry-failed`; `--dry-run`
- PASSE `--target memories|entities|chunks|all` só no `re-embed` (padrão `memories`)
- SAIBA `re-embed` seleciona vetores AUSENTES, blobs VAZIOS ou dim DIVERGENTE; `--status` soma `scan_backlog` nos alvos
- BACKFILL COMPLETO — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 enrich --operation re-embed --target all --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --json` depois `health --json`
- PASSE `--min-output-chars N` em `body-enrich`; `--fallback-mode codex` em rate limit Claude
- NUNCA rode `enrich` em paralelo no mesmo banco (singleton por namespace)
- PASSE `--until-empty` para loop scan→drain até vazio ou `--max-runtime` (padrão 3600)
- PASSE `--max-attempts <N>` (padrão 8, range 1..=20) antes de Transient virar `dead`
- PASSE `--status` para `scan_backlog`, `unbound_backlog`, contagens, `eligible_now`, `waiting` — SEM LLM, SEM singleton, SEM `--operation`/`--mode`
- SAIBA `scan_backlog` é backlog REAL de banco; DISTINTO de `unbound_backlog` e `queue_pending` do sidecar
- PASSE `--rest-concurrency <N>` para fan-out openrouter (clamp 1..=16, padrão 8); DISTINTO de `--llm-parallelism`
- PASSE `--list-dead` para terminais (`error_class`, `message`, `finish_reason`, tokens); `--requeue-dead` → `pending`; `--ignore-backoff` ignora `next_retry_at`
- PASSE `--prune-dead-orphans` para dead memory-keyed com `item_key` AUSENTE do DB principal; entity-keyed INTOCADAS; só sidecar `.enrich-queue.sqlite` muta — `sqlite-graphrag enrich --prune-dead-orphans --json`; USE ANTES de `--requeue-dead`
- PASSE `--prune-dead-entity-orphans` para dead entity-keyed (mutuamente exclusivo); PRIMEIRO `enrich --operation re-embed --target entities --requeue-dead --ignore-backoff`; depois `sqlite-graphrag enrich --prune-dead-entity-orphans --json`
- SAIBA dead-letter tem `error_class`, `next_retry_at`, status `dead` — Transient (rate-limit, timeout, 5xx, retry esgotado, entidade ainda não materializada) backoff até `--max-attempts`; HardFailures (validação, parse) terminal de imediato; dequeue pula `dead`
- SAIBA truncamento OpenRouter (`finish_reason` = `length`) re-emite com `max_tokens` MAIOR antes do reparo JSON — NÃO vira dead de imediato
- SAIBA fila vive no sidecar `.enrich-queue.sqlite` ao lado do DB principal
- STATUS — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --status --json`
- DISTINGA — `scan_backlog` = candidatos de banco que scan fresco ENFILEIRARIA; `queue_pending` = contagem do sidecar; `eligible_now == 0` com `queue_pending > 0` é COOLDOWN; `eligible_now > 0` preso em state `draining` é deadlock — rode `--reset-stale-claims`
- PASSE `enrich --reset-stale-claims` para claims `processing` stale após `kill -9`; startup auto-reseta; SIGTERM limpa antes do exit 19
- UNTIL-EMPTY — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --max-attempts 8 --rest-concurrency 8 --json`


## Escrita e Depois Enrich — Duas Etapas Separadas
- TRATE toda escrita como ETAPA 1 (embed OpenRouter + `--llm-backend none`) depois ETAPA 2 DISTINTA (`enrich`); NUNCA encadeie com `&&`
- REMEMBER etapa 1 — `echo '{"body":"text","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 --llm-backend none remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --json`
- REMEMBER etapa 2 codex — `sqlite-graphrag --llm-backend codex --llm-model gpt-5.4-mini enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini --json`
- REMEMBER etapa 2 claude — `sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- REMEMBER etapa 2 opencode — `sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- REMEMBER etapa 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- REMEMBER-BATCH etapa 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-backend none remember-batch --transaction --json`
- REMEMBER-BATCH etapa 2 codex — `sqlite-graphrag --llm-backend codex --llm-model gpt-5.4-mini enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini --json`
- REMEMBER-BATCH etapa 2 claude — `sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- REMEMBER-BATCH etapa 2 opencode — `sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- REMEMBER-BATCH etapa 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- INGEST etapa 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 --llm-backend none ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- INGEST etapa 2 codex — `sqlite-graphrag --llm-backend codex --llm-model gpt-5.4-mini enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini --json`
- INGEST etapa 2 claude — `sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- INGEST etapa 2 opencode — `sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- INGEST etapa 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- EDIT etapa 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model perplexity/pplx-embed-v1-0.6b --embedding-dim 384 --llm-backend none edit --name <n> --body-file new.md --json`
- EDIT etapa 2 codex — `sqlite-graphrag --llm-backend codex --llm-model gpt-5.4-mini enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini --json`
- EDIT etapa 2 claude — `sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- EDIT etapa 2 opencode — `sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- EDIT etapa 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- RESTORE etapa 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 --llm-backend none restore --name <n> --version 2 --json`
- RESTORE etapa 2 codex — `sqlite-graphrag --llm-backend codex --llm-model gpt-5.4-mini enrich --operation memory-bindings --mode codex --codex-model gpt-5.4-mini --json`
- RESTORE etapa 2 claude — `sqlite-graphrag --llm-backend claude --llm-model claude-sonnet-4-6 enrich --operation memory-bindings --mode claude-code --claude-model claude-sonnet-4-6 --json`
- RESTORE etapa 2 opencode — `sqlite-graphrag --llm-backend opencode --llm-model opencode/big-pickle enrich --operation memory-bindings --mode opencode --opencode-model opencode/big-pickle --json`
- RESTORE etapa 2 openrouter — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --json`
- SAIBA resolução de chave prefere env `OPENROUTER_API_KEY` ou `config.toml`; NUNCA embuta a chave crua em comandos de produção


## Embedding e Enrich Paralelos
- ESCALE embed com `--llm-parallelism N` (JoinSet de N requests OpenRouter, ordem preservada); ESCALE enrich com `--rest-concurrency N` + `--until-empty` (N chats; escrita SQLite serial via claim WAL)
- CLAMP `--llm-parallelism` 1..32 e `--rest-concurrency` 1..16; modelos pagos preferem 4..16; `:free` ~20 req/min então USE N baixo; várias chaves NÃO somam capacidade
- REMEMBER paralelo etapa 1 — `echo '{"body":"...","entities":[...],"relationships":[...]}' | sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none remember --name <n> --type decision --description "desc" --graph-stdin --force-merge --json`
- REMEMBER paralelo etapa 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --rest-concurrency 8 --until-empty --max-runtime 3600 --max-attempts 8 --json`
- REMEMBER-BATCH paralelo etapa 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 12 --llm-backend none remember-batch --transaction --json`
- REMEMBER-BATCH paralelo etapa 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --rest-concurrency 12 --until-empty --max-runtime 3600 --json`
- INGEST paralelo etapa 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 --llm-parallelism 6 --llm-backend none ingest ./docs --mode none --recursive --pattern "*.md" --type document --resume --json`
- INGEST paralelo etapa 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b:nitro --rest-concurrency 12 --until-empty --max-runtime 7200 --max-attempts 8 --json`
- EDIT paralelo etapa 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none edit --name <n> --body-file new.md --json`
- EDIT paralelo etapa 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --rest-concurrency 8 --until-empty --json`
- RESTORE paralelo etapa 1 — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 --llm-parallelism 8 --llm-backend none restore --name <n> --version 2 --json`
- RESTORE paralelo etapa 2 — `sqlite-graphrag enrich --operation memory-bindings --mode openrouter --openrouter-model openai/gpt-oss-120b --rest-concurrency 8 --until-empty --json`
- MONITORE com `--status` até `scan_backlog` `queue_pending` `eligible_now` serem 0; `queue_dead` é dívida permanente de dados


## Fórmulas OpenRouter Somente-Leitura
- INIT — `sqlite-graphrag --embedding-backend openrouter --embedding-model nvidia/llama-nemotron-embed-vl-1b-v2:free --embedding-dim 384 init --namespace <ns>`
- RECALL com qwen-8b — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 recall "query" --k 10 --json`
- HYBRID-SEARCH com bge-m3 — `sqlite-graphrag --embedding-backend openrouter --embedding-model baai/bge-m3 --embedding-dim 384 hybrid-search "query" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- DEEP-RESEARCH com text-embedding-3-small — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model openai/text-embedding-3-small --embedding-dim 384 deep-research "question" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies --output /tmp/research.json --json`
- RENAME-ENTITY com perplexity — `sqlite-graphrag --embedding-backend openrouter --embedding-model perplexity/pplx-embed-v1-0.6b --embedding-dim 384 rename-entity --name <old> --new-name <new> --json`
- ENRICH re-embed com openrouter (MUTAÇÃO — backfill de vetores) — `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --embedding-dim 384 enrich --operation re-embed --target all --mode openrouter --openrouter-model openai/gpt-oss-120b --until-empty --max-runtime 3600 --json`
- HYBRID-SEARCH offline — `sqlite-graphrag hybrid-search "query" --k 10 --fallback-fts-only --json`
- TRAVERSE fuzzy — `sqlite-graphrag graph traverse --from <apelido-curto> --depth 2 --fuzzy --json`
- LINK por ID — `sqlite-graphrag link --from-id <N> --to-id <M> --relation uses --json`
- MERGE auto-ref — NUNCA rode `merge-entities --ids 3,12 --into-id 3`; SEMPRE exclua o survivor de `--ids`
- VERIFIQUE catálogo de embeddings OpenRouter — `curl -sS https://openrouter.ai/api/v1/models -H "Authorization: Bearer $OPENROUTER_API_KEY" | jaq -r '.data[].id' | rg -i 'embed'`
- SEMPRE mantenha `--embedding-dim 384` idêntico à escrita; mismatch causa knn exit 11


## Diagnóstico e Manutenção
- INIT — `sqlite-graphrag init --namespace <ns>`
- HEALTH — `sqlite-graphrag health --json` para `{integrity_ok, schema_version, vec_*_missing, vec_*_coverage_pct}`; DISPARE `enrich --operation re-embed --target <alvo>` se missing > 0
- MIGRATE — `migrate --dry-run --json` depois `migrate --json`; OPTIMIZE — `optimize --json`; VACUUM — `vacuum --json` após purge
- FTS — `fts check|stats|rebuild --json` se `health.fts_degraded`; VEC — `vec orphan-list --json` depois `vec purge-orphan --yes`; `vec stats --json`
- EMBEDDING — `embedding --status --json` com `*_missing` por tabela; `pending-embeddings --status --json` depois `pending-embeddings process --json`
- SLOTS — `slots status --json`; `slots release --slot-id <N> --yes`; PENDING — `pending list --filter-status queued --json`; `pending show <id>`; `pending cleanup --yes`
- EXPORT — `export --namespace <ns> --type <kind> --json`; STATS — `stats --json` inclui `total_memories`
- BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECT — `namespace-detect --json`, `debug-schema --json`, `cache list --json`, `cache clear-models --yes`
- COMPLETIONS — `completions bash|zsh|fish|elvish|powershell`
- AGENDE semanal — `purge` → `cleanup-orphans` → `prune-relations --relation mentions` → `vacuum` → `optimize` → `sync-safe-copy`
- SE corrupção — `sqlite3 broken.sqlite ".recover" | sqlite3 repaired.sqlite`


## Códigos de Saída e Estratégia de Retry
- EXIT 0 sucesso; EXIT 1 validação; EXIT 2 parsing de argumento; EXIT 3 lock otimista (recarregue e retente)
- EXIT 4 não encontrado (sem `--fuzzy`, inclui sugestões Jaro-Winkler/prefixo); EXIT 5 erro de namespace
- EXIT 6 payload grande demais — TRÊS variantes reportam `bytes`/`limit`, `chunks`/`limit` ou `tokens`/`limit`; DIVIDA corpo ou reduza tokens
- EXIT 9 duplicada — use `--force-merge`; EXIT 10 banco — rode `vacuum` mais `health`
- EXIT 11 falha de embedding — verifique backend, dim 384, OAuth/chave
- EXIT 13 batch parcial — reprocesse só falhados; EXIT 14 I/O; EXIT 15 ocupado — amplie `--wait-lock`
- EXIT 16 preflight — corrija config MCP; NUNCA trate como transitório
- EXIT 19 SHUTDOWN — retry OBRIGATÓRIO; trabalho parcial descartado; EXIT 20 interno
- EXIT 75 slots/singleton locked — respeite cooldown; NUNCA retente de imediato
- EXIT 77 pressão de RAM; EXIT 78 config (chave ou modelo OpenRouter ausente)
- NUNCA ignore exit não-zero; NUNCA reprocesse batch inteiro após exit 13; NUNCA confunda exit 1 com exit 9


## Concorrência
- RESPEITE teto rígido `2 x nCPUs` para comandos pesados — `init`, `remember`, `ingest`, `recall`, `hybrid-search`
- DEFINA `--llm-parallelism N` padrão 4 em `remember` e `edit`, padrão 2 em `ingest`, clamp [1, 32]
- SAIBA JOB SINGLETON — `enrich` e `ingest --mode codex|claude-code` adquirem singleton por namespace
- USE `--wait-job-singleton SECS` ou `--force-job-singleton` para quebrar lock stale
- HABILITE `SQLITE_GRAPHRAG_LOW_MEMORY=1` para paralelismo unitário (3 a 4 vezes mais lento)
- NUNCA rode `enrich` em paralelo no mesmo banco
- SAIBA concorrência REST do enrich é `--rest-concurrency`, NÃO múltiplos processos enrich


## Variáveis de Ambiente
- `SQLITE_GRAPHRAG_DB_PATH` — override do path do banco
- `SQLITE_GRAPHRAG_NAMESPACE` — namespace persistente
- `SQLITE_GRAPHRAG_LLM_BACKEND` — backend LLM persistente
- `SQLITE_GRAPHRAG_LLM_MODEL` — modelo LLM persistente
- `SQLITE_GRAPHRAG_EMBEDDING_BACKEND` — backend de embedding persistente
- `SQLITE_GRAPHRAG_EMBEDDING_MODEL` — modelo de embedding OpenRouter persistente
- `SQLITE_GRAPHRAG_EMBEDDING_DIM` — dimensão [8, 4096], padrão 384
- `OPENROUTER_API_KEY` — chave OpenRouter, zeroizada no drop
- `SQLITE_GRAPHRAG_CODEX_BINARY`, `SQLITE_GRAPHRAG_CLAUDE_BINARY`, `SQLITE_GRAPHRAG_OPENCODE_BINARY` — overrides de path
- `SQLITE_GRAPHRAG_OPENCODE_MODEL`, `SQLITE_GRAPHRAG_OPENCODE_TIMEOUT` — overrides opencode
- `SQLITE_GRAPHRAG_LOW_MEMORY` — paralelismo unitário
- `SQLITE_GRAPHRAG_LOG_FORMAT` — `json` para agregadores
- `SQLITE_GRAPHRAG_SKIP_PREFLIGHT` — bypass preflight, APENAS EMERGÊNCIAS
- NUNCA confie em `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` para codex/claude; são PROIBIDAS e abortam spawn com exit 1


## Regras Ativas
- SEMPRE `--json`; SEMPRE `jaq` após captura (NUNCA pipe NDJSON; NUNCA `jq`); SEMPRE parseie `backend_invoked`
- SEMPRE `--embedding-backend openrouter --embedding-model <MODEL> --embedding-dim 384` em ops de embed com chave via env/config
- SEMPRE `--llm-backend none` nas escritas; SEMPRE `enrich` SEPARADO com `--mode` + modelo; NUNCA encadeie com `&&`
- SEMPRE refresque OAuth (`codex login` / OAuth claude) quando stale; SEMPRE dim 384 (mismatch → knn exit 11)
- SEMPRE arrays de shell + joins quotados para listas dinâmicas de `merge-entities`
- PREFIRA `--from-id`/`--to-id` para IDs numéricos em link; NUNCA dígitos puros como nomes `--from`/`--to`
- PREFIRA `--fuzzy` ou sugestões NotFound em traverse de nome curto; PREFIRA `--output PATH` + `--quiet` para deep-research grande; NUNCA `&>`
- NUNCA passe chaves a codex/claude (OAuth-only, exit 1); NUNCA `--llm-backend codex` em paths de escrita
- NUNCA `enrich` paralelo no mesmo DB; NUNCA escreva `.sqlite` fora do binário
- NUNCA ignore exit 19 (retry OBRIGATÓRIO) ou exit 16 (corrija MCP); NUNCA openrouter sem model+chave (exit 78)
- NUNCA merge auto-ref (`--into-id` dentro de `--ids` / `--into` dentro de `--names`); NUNCA memória MCP ou MEMORY.md
- TIPOS canônicos de memória — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- TIPOS canônicos de entidade — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- RELAÇÕES canônicas — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
