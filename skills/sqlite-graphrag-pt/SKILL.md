---
name: sqlite-graphrag-pt
description: Esta skill DEVE ativar para toda operação da CLI sqlite-graphrag e DEVE ser usada sempre que um agente armazena, recupera, busca, enriquece, liga, mescla ou mantém memória GraphRAG de longo prazo num grafo SQLite local. Ela ensina os 51 comandos, as famílias graph config fts vec slots pending embedding cache schema completions, as 63 chaves de configuração XDG, os 74 contratos JSON Schema, as flags agent-native select filter max-items sort dedupe-by count-only truncate-content max-output-bytes, os catálogos de modelos de embedding e de texto da OpenRouter, o armazenamento da chave por config add-key from-stdin, a separação obrigatória entre escrita e enrich, o paralelismo de embedding com llm-parallelism, o fan-out de enrich com rest-concurrency, a ramificação por exit code e a remediação de falhas. Palavras-chave sqlite-graphrag GraphRAG memória embedding openrouter remember remember-batch ingest edit restore recall hybrid-search deep-research enrich re-embed entity-connect force-redescribe link merge-entities purge XDG
---

## Quando Esta Skill Ativa
- DEVE ATIVAR para remember, salvar, recall, recuperar, buscar, persistir entre sessões
- DEVE ATIVAR para GraphRAG, grafo de conhecimento, ligação de entidades, memória por namespace
- DEVE ATIVAR quando sqlite-graphrag, embedding, FTS5, hybrid-search, OpenRouter ou entity-connect for mencionado
- DEVE ATIVAR para enrich, re-embed, entity-connect, link, unlink, merge-entities, rename-entity
- DEVE ATIVAR para deep-research, ingest, config, chaves XDG, contratos de schema, manutenção de grafo
- DEVE ATIVAR para pending, slots, backlog de embedding, fts, vec, vacuum, purge, backup
- NUNCA ATIVE para dados efêmeros, I/O simples de arquivo ou tarefas sem componente de memória
- SEMPRE carregue esta skill ANTES de inventar arquivos de memória ad-hoc, servidores MCP de memória ou diários Markdown


## Modelo Mental Central
- INVOQUE o binário como subprocesso one-shot; NÃO existe daemon, NÃO existe ONNX, NÃO existe cache de modelo
- SAIBA que o embedding é HTTP em processo; NÃO existe backend de embedding por subprocesso
- SAIBA que há SOMENTE DOIS seletores; NUNCA invente um terceiro
- SELETOR 1 é `--embedding-backend`, que aceita EXATAMENTE `auto` ou `openrouter`
- SELETOR 2 é `--llm-backend`, que aceita EXATAMENTE `openrouter` ou `none`
- SAIBA que `auto` degrada para NENHUM EMBEDDING sem chave alcançável, gravando memória sem vetor com exit 0
- SEMPRE passe `--embedding-backend openrouter` em toda escrita; NUNCA confie no `auto`
- SAIBA que a extração é `enrich --mode openrouter`, o ÚNICO modo aceito
- SAIBA que os backends headless por subprocesso foram REMOVIDOS; `--mode codex`, `--mode claude-code` e `--mode opencode` saem com exit 2
- ESCREVER e ENRIQUECER são processos SEPARADOS; a escrita produz vetores, o enrich SEPARADO muta o grafo
- NUNCA encadeie escrita e enrich com `&&`; SEMPRE aguarde exit 0 da escrita e só então execute o enrich como processo DISTINTO
- SAIBA que `ingest --enrich-after` é o ÚNICO encadeamento sancionado em processo, rodando memory-bindings depois que todos os arquivos entram
- SEMPRE passe `--json`; SEMPRE faça parse com `jaq` NUNCA `jq`; SEMPRE capture o stdout PRIMEIRO e só depois faça parse
- SEMPRE leia o exit code ANTES de fazer parse do stdout; NUNCA encadeie a CLI direto no `jaq`
- SAIBA que vetores vazios NUNCA são persistidos; FAÇA parse de `backend_invoked` para confirmar que o transporte rodou
- SAIBA que a fusão é FTS5 BM25 mais cosine KNN sobre BLOB via RRF em `memory_embeddings`, `entity_embeddings` e `chunk_embeddings`
- NUNCA exponha o binário como MCP ou HTTP; NUNCA escreva o `.sqlite` com outra ferramenta


## Contrato — Invocação e Parse
- `--db <PATH>` DEVE vir DEPOIS do verbo — `sqlite-graphrag remember --db ./g.sqlite --name x ...`
- ANTES do verbo o `--db` é REJEITADO com exit 2; omiti-lo mira o banco XDG em silêncio
- Superfícies de grafo EXIGEM `--db`; as folhas host `config`, `slots`, `cache` e `completions` o aceitam como no-op
- SEMPRE passe `--quiet` em pipelines headless; NUNCA misture stderr no JSON com `&>` ou `2>&1`
- A PRECEDÊNCIA é SEMPRE flag CLI, depois XDG `config set` ou `config add-key`, depois o default compilado
- PROIBIDAS as variáveis de ambiente de produto `SQLITE_GRAPHRAG_*`; o binário NÃO as lê no hot path
- SAIBA que a dimensão padrão de embedding é 1024 e que um banco existente mantém a dim gravada em `schema_meta`
- NUNCA passe `--embedding-dim` por hábito; ela SOBRESCREVE a dim gravada e um valor divergente mata a busca cosine
- USE `--embedding-dim` SOMENTE para migração deliberada de corpus, seguida de `enrich --operation re-embed`


## Códigos de Saída
- 0 sucesso
- 1 validação, timeout, rate limit, erro de provedor, binário não encontrado e recusa do `--no-input`
- 2 argumento inválido, flag desconhecida, flag na posição errada ou valor de enum não aceito
- 3 conflito de lock otimista — RECARREGUE e RETENTE
- 4 não encontrado — LEIA as sugestões ranqueadas do envelope
- 5 erro de namespace
- 6 payload grande demais, chunks demais, tokens demais — DIVIDA o corpo
- 9 duplicata — RETENTE com `--force-merge`
- 10 erro de banco — EXECUTE `vacuum` e depois `health`
- 11 falha de embedding — VERIFIQUE backend, chave e dimensão
- 12 falha da extensão vetorial
- 13 batch parcial — REPROCESSE SOMENTE as linhas falhas, NUNCA o lote inteiro
- 14 erro de I/O
- 15 banco ocupado — AMPLIE o `--wait-lock`
- 19 desligamento por sinal, com o nome do sinal no envelope — o RETRY é OBRIGATÓRIO
- 20 erro interno ou de JSON
- 75 slot de concorrência ou job singleton ocupado — NUNCA retente de imediato
- 77 memória disponível insuficiente
- 78 falha de configuração, tipicamente chave ou modelo OpenRouter ausente
- 141 stdout fechado pelo consumidor; idêntico em Linux, macOS e Windows
- NUNCA ignore um exit não zero; NUNCA confunda o timeout de exit 1 com o exit 75


## Flags Globais Contra Flags Por Subcomando
- SAIBA que essa distinção decide a POSIÇÃO; uma flag por subcomando colocada antes do verbo sai com exit 2
- GLOBAIS, escritas antes do verbo — `--max-concurrency`, `--wait-lock`, `--fail-on-degraded`, `--lang`, `--tz`
- GLOBAIS — `-v`/`-vv`/`-vvv`, `-q`/`--quiet`, `--embedding-dim`, `--embedding-backend`, `--embedding-model`
- GLOBAIS — `--llm-backend`, `--llm-model`, `--llm-fallback`, `--llm-max-host-concurrency`
- GLOBAIS — `--llm-slot-wait-secs`, `--llm-slot-no-wait`, `--skip-embedding-on-failure`
- GLOBAIS — `--openrouter-timeout`, `--openrouter-api-key`, `--no-input` e as oito flags agent-native
- POR SUBCOMANDO, escritas depois do verbo — `--db`, `--namespace`, `--json`, `--format`, `--limit`
- POR SUBCOMANDO — `--llm-parallelism`, `--openrouter-model`, `--openrouter-base-url`, `--operation`, `--mode`
- POR SUBCOMANDO — `--wait-job-singleton`, `--force-job-singleton`, `--low-memory`, `--print-schema`
- `--fail-on-degraded` FAZ uma leitura degradada sair com exit não zero em vez de devolver resultado só-FTS com exit 0
- SEMPRE passe `--fail-on-degraded` em `recall`, `hybrid-search` e `deep-research` dentro de pipelines de agente
- SAIBA que uma degradação PEDIDA pelo chamador com `--fallback-fts-only` é deliberada e NUNCA falha
- `--openrouter-timeout <SEGUNDOS>` vincula também o cliente de EMBEDDING, não só o de chat; XDG `llm.openrouter_timeout_secs` padrão 600
- `--no-input` RECUSA stdin de forma declarativa; `--body-stdin`, `--graph-stdin` e `remember-batch` falham DE ANTEMÃO com exit 1
- DESLIGUE o opt-in XDG do `--no-input` REMOVENDO a chave `cli.no_input`, NUNCA com `--no-input=false`


## Superfície de Saída Agent-Native
- PREFIRA estas OITO flags globais a pipar o payload inteiro no `jaq`; o corte acontece ANTES da serialização
- `--select <CHAVES>` mantém só estas chaves separadas por vírgula em cada elemento; caminhos com ponto funcionam; `--fields` é a mesma flag
- SAIBA que chave ausente é PULADA, nunca emitida como `null`; envelope sem array de resultados é projetado ele mesmo
- `--filter <EXPR>` aceita `chave=valor`, `chave!=valor`, `chave~substring`; `==` é sinônimo de `=`
- REPITA `--filter` para conjugar com AND; expressão malformada sai com exit 2, então um typo NUNCA vira conjunto vazio
- `--max-items N` limita elementos EMITIDOS em TODO array do envelope e reporta `agent_surface.secondary_capped`
- SAIBA que `--max-items` é DISTINTA de `--limit` e de `-k`, que limitam a CONSULTA e não a saída
- `--sort <CHAVE>` ordena de forma ascendente por caminho com ponto; números comparam numericamente; elementos sem a chave ficam no FIM
- `--dedupe-by <CHAVE>` descarta elementos posteriores que repetem o valor; elementos sem a chave são SEMPRE mantidos
- `--count-only` substitui o payload por `{"count": N}`, contado DEPOIS de filter, dedupe e max-items
- `--truncate-content N` encurta strings acima de N CARACTERES, nunca bytes; uma sequência UTF-8 NUNCA é partida
- `--max-output-bytes N` limita o envelope DESCARTANDO elementos do fim, NUNCA fatiando o texto JSON
- A ORDEM é FIXA — filter, sort, dedupe, max-items, select, count-only, truncate-content, max-output-bytes
- NUNCA assuma que `--filter` esconde uma falha; envelope com `error: true` ou `ok: false` SEMPRE chega ao chamador
- SEMPRE faça parse de `agent_surface` quando um knob estiver ativo — `input_count`, `output_count`, `content_truncated`, `output_truncated`, `dropped`
- SAIBA que a truncagem levanta a flag `truncated` de topo e NUNCA é silenciosa
- SAIBA que o array de resultados é localizado por `results`, `items`, `entities`, `memories`, `hits`, `rows`, `matches`, `data`, nesta ordem
- SAIBA que documentos `$schema` passam intactos e que streams NDJSON contornam a superfície por completo


## Catálogo Completo de Comandos
- TOP-LEVEL, 51 verbos — `init` `remember` `remember-batch` `ingest` `recall` `read` `list` `forget` `purge` `rename` `split-body` `edit` `history` `restore` `hybrid-search` `health` `migrate` `namespace-detect` `optimize` `stats` `sync-safe-copy` `backup` `vacuum` `link` `unlink` `deep-research` `related` `graph` `export` `fts` `vec` `prune-relations` `prune-ner` `slots` `pending` `embedding` `pending-embeddings` `cleanup-orphans` `memory-entities` `cache` `delete-entity` `reclassify` `rename-entity` `merge-entities` `enrich` `reclassify-relation` `normalize-entities` `schema` `completions` `config` `help`
- SAIBA que `debug-schema` ainda funciona porém está OCULTO do `--help`; USE-O para inspecionar o schema vivo do banco
- Família `graph` — `graph traverse` `graph stats` `graph entities` `graph recompute-degree`
- Família `config` — `config add-key` `config list-keys` `config remove-key` `config doctor` `config path` `config set` `config get` `config list` `config unset`
- Família `fts` — `fts rebuild` `fts check` `fts stats`
- Família `vec` — `vec orphan-list` `vec purge-orphan` `vec stats`
- Família `slots` — `slots status` `slots release` `slots cleanup`
- Família `pending` — `pending list` `pending show` `pending cleanup`
- Família `embedding` — `embedding status` `embedding list` `embedding abandon`
- Família `pending-embeddings` — `list` `status` `abandon`, aliases de `embedding`
- Família `cache` — `cache clear-models` `cache list` `cache stats`
- `completions` — `bash|zsh|fish|elvish|powershell`
- `schema` emite 74 linhas NDJSON de `{"id","invoke"}`; `schema --name <ID>` emite aquele JSON Schema; ID desconhecido sai com exit 4
- SAIBA que documentos `$schema` são ISENTOS da superfície agent-native, então qualquer flag global encadeia com segurança


## Configuração da Chave OpenRouter
- ADICIONE a chave por stdin — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- VERIFIQUE com `sqlite-graphrag config list-keys --json` e depois `sqlite-graphrag config doctor --json`
- REMOVA com `sqlite-graphrag config remove-key <fingerprint> --json`; LOCALIZE com `sqlite-graphrag config path`
- DEFINA os endpoints quando houver proxy — `config set network.openrouter.chat_url https://openrouter.ai/api/v1/chat/completions`
- DEFINA o endpoint de embedding — `config set network.openrouter.embeddings_url https://openrouter.ai/api/v1/embeddings`
- SAIBA que as chaves ficam em `~/.config/sqlite-graphrag/config.toml` com modo 600, zeroizadas no drop e NUNCA logadas
- NUNCA passe `--openrouter-api-key` no histórico de shell em produção; SEMPRE prefira `config add-key --from-stdin`
- SEMPRE rode `config doctor` depois de adicionar a chave e antes de qualquer chamada paga; chave ou modelo ausente sai com exit 78


## Modelos de Embedding da OpenRouter
- PASSE `--embedding-model <MODELO>` sempre que usar `--embedding-backend openrouter`; NÃO há default e a omissão sai com exit 78
- INSPECIONE o catálogo vivo com a chave armazenada; os preços abaixo são indicativos em USD por milhão de tokens
- `nvidia/llama-nemotron-embed-vl-1b-v2:free` GRATUITO, com limite próximo de 20 requisições por minuto
- `qwen/qwen3-embedding-4b` 0,05
- `qwen/qwen3-embedding-8b` 0,05, a escolha operacional PADRÃO
- `openai/text-embedding-3-small` 0,05
- `perplexity/pplx-embed-v1-0.6b` 0,05
- `baai/bge-m3` 0,05
- `mistralai/mistral-embed-2312` 0,10
- `google/gemini-embedding-2` 0,12
- `openai/text-embedding-3-large` 0,13
- `google/gemini-embedding-005` 0,15
- SAIBA que a truncagem Matryoshka acontece NO SERVIDOR até a dimensão ativa
- SAIBA que openrouter propaga a TODO caminho de embed — `remember` `remember-batch` `ingest` `edit` `restore` `split-body` `recall` `hybrid-search` `deep-research` `rename-entity` `init` `enrich`


## Modelos de Texto da OpenRouter
- SAIBA que modelos de texto servem SOMENTE extração e enriquecimento, NUNCA embedding
- PASSE `--openrouter-model <MODELO>` depois do verbo `enrich`; é OBRIGATÓRIO e a omissão falha antes de qualquer chamada de rede
- `deepseek/deepseek-v4-flash`
- `deepseek/deepseek-v4-flash:nitro`, a escolha operacional PADRÃO para vazão
- `deepseek/deepseek-v4-pro`
- `google/gemini-3.1-flash-lite`
- `minimax/minimax-m3`
- `minimax/minimax-m2.7`
- `minimax/minimax-m2.7:nitro`
- `openai/gpt-oss-120b`
- `openai/gpt-oss-120b:nitro`
- `xiaomi/mimo-v2.5`
- `xiaomi/mimo-v2.5-pro`
- `z-ai/glm-5.2`
- `z-ai/glm-5.2:nitro`
- SAIBA que `:nitro` seleciona o provedor mais rápido a preço maior
- VERIFIQUE o suporte a `json_schema` estrito ANTES de produção; sem Structured Outputs a OpenRouter devolve erro explícito
- CONFIRME o gasto real fazendo parse de `usage.cost` no envelope do enrich


## Registro de Configuração XDG
- LEIA o registry vivo com `sqlite-graphrag config doctor --json | jaq -r '.knobs[].key'`; ele tem 63 chaves
- DEFINA qualquer chave com `config set <chave> <valor>`; LEIA com `config get <chave>`; LIMPE com `config unset <chave>`
- LISTE os valores gravados com `config list --json` e os resolvidos com `config list --effective --json`
- Superfície agent — `agent_surface.max_items` 0, `agent_surface.max_output_bytes` 0, `agent_surface.truncate_content` 0
- Cache e CLI — `cache.dir`, `cli.max_instances`, `cli.no_input` false, `cli.stdin_timeout_secs` 60
- Banco — `db.busy_base_delay_ms` 300, `db.busy_retries` 5, `db.path`, `db.query_timeout_ms` 5000
- Exibição e locale — `display.tz` UTC, `i18n.lang` en
- Embedding — `embedding.backend`, `embedding.model`, `embedding.dim` 1024, `embedding.batch_size` 32
- Cache de embedding — `embedding.entity_cache_max_entries` 10000, `embedding.entity_cache_ttl_secs` 3600, `embedding.timeout_secs` 300
- Ritmo do enrich — `enrich.circuit_breaker_reset_secs` 60, `enrich.rate_limit_deadline_secs` 3600, `enrich.yield_every_n_items` 10
- Lotes do enrich — `enrich.reembed_claim_batch` 32, `enrich.scan_page_size` 512
- Entity connect — `enrich.entity_connect.default_limit` 100, `enrich.entity_connect.large_ns_limit` 25
- Descrições de entidade — `enrich.entity_description.corpus_top_k` 5, `enrich.entity_description.domain` auto, `enrich.entity_description.grounding_threshold` 0.12
- Descrições de entidade — `enrich.entity_description.min_corpus_chars` 40, `enrich.entity_description.quality_sample` 50, `enrich.entity_description.snippet_chars` 400
- Ingest e limites — `ingest.low_memory` false, `limits.max_entities_per_memory` 50, `limits.max_relations_per_memory` 50
- Transporte LLM — `llm.backend`, `llm.model`, `llm.fallback` none, `llm.openrouter_timeout_secs` 600, `llm.probe_timeout_ms` 800
- Slots LLM — `llm.max_host_concurrency`, `llm.slot_wait_secs` 300, `llm.slot_no_wait` false, `llm.worker_rss_mb` 350, `llm.skip_embedding_on_failure` false
- Log — `log.format` pretty, `log.level` warn, `log.retention_days` 7, `log.rotation` daily, `log.to_file` false
- Namespace — `namespace.default` global
- Rede — `network.chat_url`, `network.embed_url`, `network.openrouter.chat_url`, `network.openrouter.embeddings_url`
- Paralelismo — `parallelism.embed_runtime_threads`, `parallelism.max_total_workers` 64, `parallelism.rayon_threads`
- Busca — `search.hybrid.max_graph_results` 50
- Runtime — `retry.disable` false, `shutdown.ignore` false, `system.max_load_per_ncpu` 2.0
- NUNCA declare chave fora deste registry; chave desconhecida sai com exit 1 e uma sugestão de correção


## Escrita Passo 1 — Fórmulas de Embedding
- DEFINA o prefixo de escrita W como `sqlite-graphrag --embedding-backend openrouter --embedding-model <EMB> --llm-backend none`
- USE `<EMB>` igual a `qwen/qwen3-embedding-8b` por padrão, ou `nvidia/llama-nemotron-embed-vl-1b-v2:free` no caminho gratuito
- ESCALE o embedding com `--llm-parallelism N` escrito DEPOIS do verbo, com clamp em 1..32
- SAIBA que SOMENTE `remember`, `remember-batch`, `ingest`, `edit` e `enrich` a declaram; em `restore` ou `split-body` ela sai com exit 2
- SAIBA que o fan-out só liga acima de cerca de 32 textos; um item único é serial por construção
- ESCALE a ingestão por arquivo à parte com `--ingest-parallelism N`, que é DISTINTA do fan-out de embedding
- REMEMBER — `echo '{"body":"texto","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | W remember --db ./g.sqlite --name <n> --type decision --description "desc" --graph-stdin --force-merge --llm-parallelism 16 --json`
- ESCOLHA exatamente UMA fonte de corpo — `--body` inline, `--body-file`, `--body-stdin` ou `--graph-stdin`; `--graph-file` COMBINA com qualquer uma das três primeiras
- REMEMBER hot set — ACRESCENTE `--enqueue-enrich` para enfileirar entity-descriptions das entidades ligadas nesta chamada
- REMEMBER extras — `--strict-name`, `--replace-graph` com `--force-merge`, `--dry-run`, `--enable-ner`, `--metadata`, `--metadata-file`, `--session-id`, `--expected-updated-at`, `--entities-file`, `--relationships-file`, `--clear-body`, `--max-rss-mb`
- REMEMBER-BATCH — `W remember-batch --db ./g.sqlite --transaction --llm-parallelism 16 --json` lendo NDJSON no stdin
- SAIBA que cada linha de criação DEVE ter `description` não vazia e `type`; ACRESCENTE `--fail-fast` para parar na primeira linha ruim
- INGEST — `W ingest --db ./g.sqlite ./docs --mode none --recursive --pattern "*.md" --type document --llm-parallelism 16 --json`
- SAIBA que `ingest --mode` aceita SOMENTE `none`; `--resume` e `--retry-failed` foram REMOVIDAS junto com a fila curada por LLM
- INGEST extras — `--ingest-parallelism N` padrão `max(1, cpus/2).min(4)`, `--low-memory`, `--max-files`, `--max-cost-usd`, `--auto-describe`, `--no-auto-describe`, `--name-prefix`, `--max-name-length`, `--force-merge` deduplicando por `body_hash`
- INGEST encadeado em um processo — ACRESCENTE `--enrich-after` para rodar memory-bindings depois que todos os arquivos entrarem
- EDIT — `W edit --db ./g.sqlite --name <n> --body-file novo.md --json`, ou `--description`, `--memory-type`, `--force-reembed`
- EDIT sob concorrência — PASSE `--expected-updated-at <ts>`; exit 3 significa RECARREGUE e RETENTE
- RESTORE — `W restore --db ./g.sqlite --name <n> --version <N> --json`
- SPLIT-BODY — `W split-body --db ./g.sqlite --name <N> --json`, ou `--batch --threshold 25000` para todo corpo oversized
- SAIBA que as filhas do split NÃO são embedadas inline; elas EXIGEM um `enrich --operation re-embed --target memories` separado
- RESPEITE 512000 bytes e 512 chunks por corpo; NUNCA misture fontes de corpo; NUNCA `fd | xargs remember`, USE `ingest`
- VALORES válidos de `--type` — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- OBRIGATÓRIA a allowlist de entidade no graph-stdin — SOMENTE `name`, `entity_type` com `type` dobrado como alias, e `description` opcional
- PROIBIDOS no graph-stdin — `observations`, `aliases` e extras livres, que saem com exit 1
- FAÇA parse de todo envelope de remember buscando `entities_created[]` e `enrich_recommended[]`; NUNCA ignore nenhum dos dois


## Enrich Passo 2 — Fórmulas
- EXECUTE o enrich como processo DISTINTO somente depois que a escrita devolveu exit 0
- LIGAR — `sqlite-graphrag enrich --db ./g.sqlite --operation memory-bindings --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --rest-concurrency 16 --until-empty --max-runtime 3600 --max-attempts 8 --json`
- DESCREVER — `sqlite-graphrag enrich --db ./g.sqlite --operation entity-descriptions --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --entity-names jwt,auth-svc --force-redescribe --rest-concurrency 16 --json`
- CONECTAR dry run — `sqlite-graphrag enrich --db ./g.sqlite --operation entity-connect --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --dry-run --limit 50 --json`
- CONECTAR drain — o mesmo com `--until-empty --max-runtime 600 --rest-concurrency 16` no lugar do `--dry-run`
- CONECTAR ancorado — ACRESCENTE `--anchor-memory <nome>` ou `--entity-names a,b` para escopar o scan de pares
- PONTES — as mesmas fórmulas com `--operation cross-domain-bridges`
- RE-EMBED — `W enrich --db ./g.sqlite --operation re-embed --target all --mode openrouter --openrouter-model deepseek/deepseek-v4-flash:nitro --until-empty --rest-concurrency 16 --json` e depois `health --json`
- STATUS sem nenhuma chamada de LLM — `sqlite-graphrag enrich --db ./g.sqlite --status --quality-sample 50 --json`
- RECUPERAR dead — `... --list-dead --json` e depois `... --requeue-dead --json`
- RECUPERAR skipped — `... --list-skipped --json` e depois `... --requeue-skipped --json`
- ESCALE com `--rest-concurrency N` com clamp em 1..16 dentro de UM processo; modelos pagos DEVEM usar de 4 a 16
- SAIBA que `--llm-parallelism` é IGNORADA no enrich em modo openrouter; ali `--rest-concurrency` é o ÚNICO knob de fan-out
- NUNCA lance N processos de enrich contra um banco; o job singleton REJEITA o segundo com exit 75
- AGUARDE um singleton preso com `--wait-job-singleton SEGS` ou sobrescreva com `--force-job-singleton`


## Regras do Pipeline de Enrich
- PASSE `--operation` e `--mode` juntas em toda operação de LLM; omitir `--mode` sai com exit 2
- ISENTE de `--mode` os inspetores read-only — `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` e `--dry-run`
- Operações que PERSISTEM — `memory-bindings`, `augment-bindings`, `entity-descriptions`, `body-enrich`, `body-extract`, `re-embed`, `weight-calibrate`, `relation-reclassify`, `entity-connect`, `entity-type-validate`, `description-enrich`, `cross-domain-bridges`, `domain-classify`, `deep-research-synth`
- Operação de SCAN e REPORT apenas — `graph-audit`, que NUNCA muta estrutura
- SAIBA que `augment-bindings` EXIGE `--memory-names`, `--names` ou `--names-file`
- PREFIRA `--entity-names` para operações por entidade e `--memory-names` para operações por memória; `--names` é alias de compatibilidade
- PARE quando um empty match exibir `matched=0` mais um `hint`; NUNCA amplie às cegas
- PASSE `--target memories|entities|chunks|all` SOMENTE em `re-embed`, com padrão `memories`
- SAIBA que a elegibilidade do re-embed é o comprimento do BLOB `LENGTH(embedding)=dim*4`, e não a coluna `dim` sozinha
- SAIBA que claim, `--resume`, `--retry-failed` e `--until-empty` são escopados SOMENTE a esta operação E a este namespace
- SAIBA que `--force-redescribe` reabre linhas `skipped` e `done` uma vez por processo, e NUNCA reabre `dead`
- SAIBA que marcadores de baixa qualidade são SOMENTE frases compostas; uma frase de domínio sozinha NÃO DEVE acionar redescrição
- LEIA no `--status` os campos `scan_backlog`, `queue_pending`, `queue_dead`, `eligible_now`, `waiting`, `quality_pct` e `state`
- DISTINGA `scan_backlog`, os candidatos no banco que um scan fresco enfileiraria, de `queue_pending`, a contagem do sidecar
- SAIBA que `eligible_now == 0` com `queue_pending > 0` é COOLDOWN, e não travamento
- SAIBA que `state` é `draining`, `cooldown`, `pending-scan` ou `blocked_dead`; limpe `blocked_dead` com requeue ou prune PRIMEIRO
- RESETE um claim `draining` preso com `--reset-stale-claims` depois de um `kill -9`
- SAIBA que a fila é o sidecar `.enrich-queue.sqlite`, e que completions truncadas reemitem com orçamento de tokens AUMENTADO
- SAIBA que entity-connect persiste vereditos em `entity_connect_seen`, com chave `pair:{id1}:{id2}`, varrendo coocorrência em O(k) e NUNCA produto cartesiano completo
- FAÇA parse de `budget_exhausted` e `preempted_for_gate`; o primeiro é fim de orçamento de runtime, o segundo é cessão deliberada
- PASSE `--preflight-check` para pingar o provedor antes de um drain pago, abortando cedo em vez de queimar turnos numa janela de rate limit fechada
- PASSE `--ignore-backoff` para processar itens ainda dentro do cooldown `next_retry_at`, que o `--status` reporta sob `waiting`
- AJUSTE o body-enrich com `--min-output-chars` padrão 500, `--max-output-chars` padrão 2000 e `--prompt-template <PATH>`
- AJUSTE as descrições de entidade inline com `--entity-description-domain` e `--entity-description-grounding-threshold`
- ORDENE corridas longas como memory-bindings, depois entity-descriptions, depois entity-connect, ou PASSE `--ops-gate` para impor essa ordem


## Fórmulas de Leitura e Busca
- DEFINA o prefixo de leitura R como `sqlite-graphrag --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --fail-on-degraded`
- USE o padrão de três camadas — `hybrid-search`, depois `read --name`, depois `related` ou `graph traverse`
- HYBRID-SEARCH — `R hybrid-search --db ./g.sqlite "consulta" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- Ajuste do HYBRID — `--weight-vec 1.0 --weight-fts 1.0`, `--type <kind>`, `--max-graph-results N`
- HYBRID offline — `sqlite-graphrag hybrid-search --db ./g.sqlite "consulta" --k 10 --fallback-fts-only --json`
- RECALL — `R recall --db ./g.sqlite "consulta" --k 10 --json`; ACRESCENTE `--no-graph`, `--precise`, `--max-distance <f>`, `--all-namespaces`
- DEEP-RESEARCH — `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model qwen/qwen3-embedding-8b --fail-on-degraded deep-research --db ./g.sqlite "pergunta" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies -o /tmp/dr.json --json`
- DEEP-RESEARCH com controle manual — `--sub-query-strategy manual --sub-queries-file PATH`
- Ajuste do DEEP-RESEARCH — `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--max-cost-usd`, `--timeout`
- ESCREVA envelopes grandes com `-o PATH`; FAÇA parse do ack `{written, bytes, blake3}`; o arquivo DEVE existir com bytes maiores que zero após exit 0
- READ — `sqlite-graphrag read --db ./g.sqlite --name <kebab> --json`; ACRESCENTE `--with-graph`; USE `--format raw` para o corpo puro
- LIST — `sqlite-graphrag list --db ./g.sqlite --type <kind> --limit N --offset N --json`; ACRESCENTE `--include-deleted`
- HISTORY — `sqlite-graphrag history --db ./g.sqlite --name <n> --diff --json`
- RELATED — `sqlite-graphrag related --db ./g.sqlite <nome> --hops 2 --relation uses --json`
- MEMORY-ENTITIES — `sqlite-graphrag memory-entities --db ./g.sqlite --name <memoria> --json`, e então parse de `entities[].description`
- RENAME-ENTITY no caminho de embed — `R rename-entity --db ./g.sqlite --name <old> --new-name <new> --json`
- FAÇA parse dos resultados de `recall` como `{name, snippet, distance, score, source}`
- FAÇA parse dos resultados de `hybrid-search` como `{name, combined_score, vec_rank, fts_rank}` e leia também `graph_matches[]`
- FAÇA parse de `deep-research` como `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context` e `stats`
- NUNCA confunda `distance` com `combined_score`; NUNCA aumente hops sem ler `graph stats` antes


## Grafo de Entidades
- LINK — `sqlite-graphrag link --db ./g.sqlite --from <a> --to <b> --relation uses --weight 0.8 --create-missing --json`
- LINK por identificador — `link --from-id <N> --to-id <M> --relation uses --json`; NUNCA passe dígitos puros como nomes
- LINK estrito — ACRESCENTE `--strict-relations` para rejeitar qualquer relação fora do conjunto canônico
- UNLINK — `unlink --from <a> --to <b> --relation <tipo>`, ou `--entity <nome> --all`, ou `--memory <m> --entity <e>`
- TRAVERSE — `sqlite-graphrag graph traverse --db ./g.sqlite --from <raiz> --depth 2 --json`; ACRESCENTE `--fuzzy` para nomes curtos ambíguos
- SAIBA que sem `--fuzzy` um miss sai com exit 4 carregando sugestões ranqueadas; SEMPRE use-as em vez de adivinhar
- LISTAR entidades — `graph entities --db ./g.sqlite --json`, lendo `.entities[]` e NUNCA `.items[]`
- ORDENE entidades com `--sort-by name|degree|created-at` mais `--order asc|desc`, e pagine com `--limit` e `--offset`
- TIPIFIQUE entidades auto-criadas com `--entity-type` no `link --create-missing`, que sem ela assume `concept`
- FILTRE a listagem de entidades com `graph entities --entity-type person` contra os 13 tipos canônicos
- EXPORTAR — `graph --format json|dot|mermaid|ndjson --output <path>`; MEÇA com `graph stats --json`
- RECOMPUTAR — `graph recompute-degree --json` após qualquer delete, merge ou prune, porque o grau NÃO é automático
- MERGE — `merge-entities --names "a,b,c" --into <alvo> --json`, ou `--ids 12,17 --into-id 3`
- NUNCA coloque `--into-id` dentro de `--ids`, nem `--into` dentro de `--names`; merge auto-referencial é REJEITADO antes de tocar o banco
- PASSE `--cross-namespace` no merge SOMENTE quando cruzar namespaces for intencional
- DELETE — `delete-entity --name <n> --cascade --json`; RENAME — `rename-entity --name <old> --new-name <new>` ou `--id <N>`
- RECLASSIFICAR — `reclassify --name <n> --new-type <kind>`, ou `--from-type <old> --to-type <new> --batch`
- RECLASSIFICAR relações — `reclassify-relation --from-relation <old> --to-relation <new> --batch`, com `--literal-from` e `--literal-to` para match literal
- PODAR — `prune-relations --relation mentions --dry-run` e repetir com `--yes`; `normalize-entities --yes`; `prune-ner --all --yes`
- RELAÇÕES canônicas — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- MAPEIE relações não canônicas — `adds` e `creates` para `causes`, `implements` para `supports`, `blocks` para `contradicts`, `tested-by` para `related`, `part-of` para `applies-to`
- TIPOS canônicos de entidade — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- VALIDE nomes de entidade como kebab-case ASCII minúsculo, com no mínimo 2 caracteres, sem newlines, sem all-caps curto e nunca só dígitos
- NUNCA use `mentions` como relação padrão; escritas de grafo são ADITIVAS e sem teto de grau


## Manutenção e Diagnóstico
- HEALTH — `sqlite-graphrag health --db ./g.sqlite --json` para `integrity_ok`, `schema_version`, `vec_*_missing`, `vec_*_coverage_pct` e `embedding_key`
- DISPARE `enrich --operation re-embed` sempre que algum `vec_*_missing` for maior que zero
- MIGRAR — `migrate --dry-run --json` e depois `migrate --json`; OTIMIZAR — `optimize --json`
- FTS — `fts check --json`, `fts stats --json`, `fts rebuild --json` quando o índice estiver degradado
- VEC — `vec orphan-list --json`, depois `vec purge-orphan --yes`, e `vec stats --json`
- Backlog de embedding — `embedding status --json`, `embedding list --json`, `embedding abandon`; `pending-embeddings` é a família alias
- SLOTS — `slots status --json`, `slots release --slot-id <N> --yes`, `slots cleanup --yes`
- PENDING — `pending list --json`, `pending show <id>`, `pending cleanup --yes`
- FORGET suave — `forget --name <n> --json`; PURGE definitivo — `purge --db ./g.sqlite --yes --now --dry-run --json` e depois repetir sem `--dry-run`
- SAIBA que `purge --yes` sozinho mantém a retenção de 90 dias; `--now` é o alias de `--retention-days 0`
- SIGA o purge com `cleanup-orphans --yes` e depois `vacuum --json`
- EXPORTAR — `export --namespace <ns> --type <kind> --json`; MEDIR — `stats --json`
- BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECIONAR — `namespace-detect --json`, `cache list --json`, `cache stats --json`, `cache clear-models --yes`
- INSTALAR completions — `completions bash|zsh|fish|elvish|powershell`
- AGENDE semanalmente — purge, depois `cleanup-orphans`, depois `prune-relations --relation mentions`, depois `vacuum`, depois `optimize`, depois `sync-safe-copy`
- RESPEITE o teto de concorrência de duas vezes o número de CPUs em `init`, `remember`, `ingest`, `recall` e `hybrid-search`


## Regras de Instrução de Prompt
- "lembre isso" — EXECUTE `remember --force-merge` com um `--graph-stdin` curado, e depois um enrich SEPARADO
- "o que você sabe sobre X" — EXECUTE `hybrid-search "X" --k 10 --json` PRIMEIRO, e depois `read --name <nome> --json`
- "como X se relaciona com Y" — EXECUTE `graph traverse --from X --depth 2 --json` ou `related X --hops 2 --json`
- "pesquisa profunda sobre X" — EXECUTE `deep-research "X" --k 20 --max-hops 3 -o PATH --json` com `--quiet`
- "conecte entidades isoladas" — EXECUTE `enrich --operation entity-connect` em dry primeiro, depois o drain, depois acompanhe o `--status`
- ANTES de qualquer criação — EXECUTE `hybrid-search "<nome>" --k 5 --json` e USE `--force-merge` em caso de duplicata
- DEPOIS de qualquer criação ou atualização — FAÇA parse de `read --name <nome> --json` buscando `{name, description, body_length}`
- DEPOIS de cada turno — PERSISTA os achados ou DECLARE que não há nada novo a persistir
- EM exit não zero — FAÇA parse de `jaq '{code, message, error_class}'` e REPORTE a remediação


## Antipadrões
- NUNCA encadeie escrita e enrich com `&&`; o único encadeamento sancionado é `ingest --enrich-after`
- NUNCA coloque `--db`, `--namespace`, `--json` ou `--llm-parallelism` antes do verbo
- NUNCA coloque `--fail-on-degraded`, `--embedding-backend` ou `--embedding-model` depois do verbo esperando escopo global em outros verbos
- NUNCA misture stderr no JSON com `&>` ou `2>&1`; SEMPRE passe `--quiet` e capture só o stdout
- NUNCA use `SQLITE_GRAPHRAG_*` como configuração; SEMPRE flag, depois XDG, depois default
- NUNCA chame a OpenRouter sem modelo e sem chave, o que sai com exit 78
- NUNCA passe `--embedding-dim` em um corpus já embedado em outra dimensão
- NUNCA omita `--embedding-backend openrouter` numa escrita, porque `auto` persiste memória sem vetor em silêncio
- NUNCA omita `--fail-on-degraded` numa leitura de agente, porque busca degradada devolve resultado por palavra-chave com exit 0
- NUNCA rode múltiplos processos de enrich em um banco; escale com `--rest-concurrency` dentro de UM processo
- NUNCA passe `--llm-parallelism` ao enrich em modo openrouter, onde ela é ignorada
- NUNCA peça `--mode codex`, `--mode claude-code` ou `--mode opencode`; esses backends não existem mais e saem com exit 2
- NUNCA use `ingest --resume` nem `ingest --retry-failed`; ambas foram removidas
- NUNCA ignore `entities_created` nem `enrich_recommended`; NUNCA ignore o exit 19, que obriga retry
- NUNCA reprocesse o lote inteiro após exit 13; reprocesse SOMENTE as linhas falhas
- NUNCA reabra linhas `dead` com `--force-redescribe`; USE `--requeue-dead`
- NUNCA assuma que `--until-empty` drena todas as operações; ela é escopada a esta operação e a este namespace
- NUNCA use servidores MCP de memória, MEMORY.md ou diários Markdown ad-hoc
- NUNCA abra o `.sqlite` com o shell `sqlite3` nem com editor
