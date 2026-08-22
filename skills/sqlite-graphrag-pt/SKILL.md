---
name: sqlite-graphrag-pt
description: Esta skill DEVE ativar para toda operação da CLI sqlite-graphrag e DEVE ser usada sempre que um agente armazena, recupera, busca, enriquece, liga, mescla ou mantém memória GraphRAG num grafo SQLite local. Ela ensina os 50 comandos, as famílias graph config fts vec slots embedding cache schema completions, as 70 chaves de configuração XDG, os 76 contratos JSON Schema, as flags agent-native select filter max-items sort dedupe-by count-only truncate-content max-output-bytes, os catálogos vivos de modelos de embedding e de texto da OpenRouter, o armazenamento da chave por config add-key from-stdin, a separação obrigatória entre escrita e enrich, o paralelismo com llm-parallelism e rest-concurrency, o teto conjunto de workers, a orquestração desta CLI por codex claude-code e opencode em modo headless e a ramificação por exit code. Palavras-chave sqlite-graphrag GraphRAG memória embedding openrouter remember remember-batch ingest edit restore recall hybrid-search deep-research enrich re-embed entity-connect merge-entities XDG headless
---

## Quando Esta Skill Ativa
- DEVE ATIVAR para remember, recall, buscar, salvar, persistir memória entre sessões
- DEVE ATIVAR para GraphRAG, grafo de conhecimento, entidades, relações, namespace
- DEVE ATIVAR quando sqlite-graphrag, embedding, FTS5, hybrid-search, deep-research ou OpenRouter for mencionado
- DEVE ATIVAR para enrich, re-embed, entity-connect, link, merge-entities, ingest, config, chaves XDG
- DEVE ATIVAR para orquestrar esta CLI por codex, claude code ou opencode em modo headless
- NUNCA ATIVE para dados efêmeros ou I/O de arquivo sem componente de memória
- SEMPRE carregue esta skill ANTES de inventar arquivo de memória ad-hoc, MCP de memória ou diário Markdown


## Modelo Mental Central
- INVOQUE o binário como subprocesso one-shot; NÃO existe daemon, ONNX nem cache de modelo
- SAIBA que o embedding é HTTP em processo; NÃO existe backend de embedding por subprocesso
- SAIBA que há SOMENTE DOIS seletores e NUNCA invente um terceiro
- `--embedding-backend` aceita EXATAMENTE `auto` ou `openrouter`
- `--llm-backend` aceita EXATAMENTE `openrouter` ou `none`
- SAIBA que `auto` degrada para NENHUM EMBEDDING sem chave alcançável, gravando memória sem vetor com exit 0
- SEMPRE passe `--embedding-backend openrouter` em toda escrita; NUNCA confie no `auto`
- SAIBA que `enrich --mode` aceita UM ÚNICO valor, `openrouter`, que é REST puro
- SAIBA que NENHUM modo spawna CLI local; `--mode codex`, `claude-code` e `opencode` saem com exit 2
- ESCREVER e ENRIQUECER são processos SEPARADOS; a escrita produz vetores, o enrich muta o grafo
- NUNCA encadeie escrita e enrich com `&&`; AGUARDE exit 0 e execute o enrich como processo DISTINTO
- SEMPRE passe `--json`; SEMPRE faça parse com `jaq` NUNCA `jq`; SEMPRE capture o stdout antes do parse
- SEMPRE leia o exit code ANTES de parsear; NUNCA encadeie a CLI direto no `jaq`
- SAIBA que vetor vazio NUNCA é persistido e que `backend_invoked` tem semântica DIFERENTE por verbo
- SAIBA que em `remember` ele só é populado com corpo de UM chunk, e multi-chunk devolve `null` COM embedding OK
- SAIBA que em `edit` ele é populado sempre que o re-embed roda, e é `null` só se o corpo não mudou
- PROVE o embedding com `embedding status --json` exigindo `coverage.memories_missing` igual a zero
- NUNCA exponha o binário como MCP ou HTTP; NUNCA escreva o `.sqlite` com outra ferramenta


## Contrato — Invocação, Alvo e Parse
- `--db <PATH>` DEVE vir DEPOIS do verbo — `sqlite-graphrag remember --db ./g.sqlite --name x ...`
- ANTES do verbo o `--db` é REJEITADO com exit 2; omiti-lo mira o banco XDG em SILÊNCIO
- Superfícies de grafo EXIGEM `--db`; `config`, `slots`, `cache` e `completions` o aceitam como no-op
- SEMPRE passe `--quiet` em pipeline headless; NUNCA misture stderr no JSON com `&>` ou `2>&1`
- A PRECEDÊNCIA é flag CLI, depois XDG `config set`, depois o default compilado
- PROIBIDAS as variáveis de ambiente `SQLITE_GRAPHRAG_*`; o binário NÃO as lê no hot path
- SAIBA que a dimensão padrão é 1024 e que um banco existente guarda a dim em `schema_meta`
- NUNCA passe `--embedding-dim` por hábito; dim divergente mata a busca cosine em SILÊNCIO
- USE `--embedding-dim` SOMENTE em migração deliberada, seguida de `enrich --operation re-embed`
- LEIA `agent_surface.db_path_source` como `argv`, `xdg` ou `default`, e `db_path_resolved` como o caminho aberto
- SAIBA que só `argv` é designação explícita; `xdg` e `default` são autoridade ambiente
- PASSE `--use-active` para aceitar o padrão compilado de propósito; o envelope registra `db_path_dispensation`
- LEIA `discarded_flags` no envelope de falha para saber quais das SUAS flags não puderam ser aplicadas


## Códigos de Saída
- 0 sucesso; 5 erro de namespace; 14 erro de I/O; 20 erro interno ou de JSON
- 1 validação, timeout, rate limit, erro de provedor ou recusa do `--no-input`
- 2 argumento inválido, flag desconhecida, flag na POSIÇÃO errada ou enum não aceito
- 3 conflito de lock otimista — RECARREGUE e RETENTE
- 4 não encontrado — LEIA as sugestões ranqueadas do envelope em vez de adivinhar
- 6 payload, chunks ou tokens em excesso — DIVIDA o corpo
- 9 duplicata — RETENTE com `--force-merge`; 10 erro de banco — RODE `vacuum` e depois `health`
- 11 falha de embedding — VERIFIQUE backend, chave, modelo e dimensão; 12 falha da extensão vetorial
- 13 batch parcial — REPROCESSE SOMENTE as linhas falhas, NUNCA o lote inteiro
- 15 banco ocupado — AMPLIE o `--wait-lock`; 77 memória insuficiente; 141 stdout fechado
- 19 desligamento por sinal, com o nome do sinal no envelope — o RETRY é OBRIGATÓRIO
- 75 slot ou job singleton ocupado — NUNCA retente de imediato
- 78 configuração, tipicamente chave ou modelo OpenRouter ausente ou grafado errado
- NUNCA ignore exit não zero; NUNCA confunda o timeout de exit 1 com o exit 75
- EM exit não zero — PARSEIE `jaq -c '{code, message, error_class}'` e REPORTE a remediação


## Flags Globais Contra Flags Por Subcomando
- SAIBA que essa distinção decide a POSIÇÃO; flag por subcomando antes do verbo sai com exit 2
- GLOBAIS, antes do verbo — `--max-concurrency`, `--wait-lock`, `--fail-on-degraded`, `--lang`, `--tz`, `--no-input`
- GLOBAIS — `-v`/`-vv`/`-vvv`, `-q`/`--quiet`, `--embedding-dim`, `--embedding-backend`, `--embedding-model`
- GLOBAIS — `--llm-backend`, `--llm-model`, `--llm-fallback`, `--llm-max-host-concurrency`, `--skip-embedding-on-failure`
- GLOBAIS — `--llm-slot-wait-secs`, `--llm-slot-no-wait`, `--openrouter-timeout`, `--openrouter-api-key`, e as oito agent-native
- POR SUBCOMANDO, depois do verbo — `--db`, `--namespace`, `--json`, `--format`, `--limit`, `--low-memory`
- POR SUBCOMANDO — `--llm-parallelism`, `--openrouter-model`, `--openrouter-base-url`, `--operation`, `--mode`
- POR SUBCOMANDO — `--wait-job-singleton`, `--force-job-singleton`, `--print-schema`
- `--fail-on-degraded` faz leitura degradada sair com exit não zero em vez de resultado só-FTS com exit 0
- SEMPRE passe `--fail-on-degraded` em `recall`, `hybrid-search` e `deep-research` de agente
- SAIBA que degradação PEDIDA com `--fallback-fts-only` é deliberada e NUNCA falha
- `--openrouter-timeout <SEGUNDOS>` vincula também o cliente de EMBEDDING; XDG `llm.openrouter_timeout_secs` padrão 600
- `--no-input` RECUSA stdin; `--body-stdin`, `--graph-stdin` e `remember-batch` falham DE ANTEMÃO com exit 1
- DESLIGUE o opt-in XDG do `--no-input` REMOVENDO `cli.no_input`, NUNCA com `--no-input=false`


## Superfície de Saída Agent-Native
- PREFIRA estas OITO flags globais a pipar o payload inteiro no `jaq`; o corte acontece ANTES da serialização
- `--select <CHAVES>` mantém só essas chaves; caminhos com ponto funcionam; `--fields` é a mesma flag
- `--filter <EXPR>` aceita `chave=valor`, `chave!=valor`, `chave~substring`; `==` é sinônimo de `=`
- PASSE `--filter-scope page|universe` ao filtrar comando PAGINADO; sem ela o predicado sobre página truncada é RECUSADO com exit 2
- `--max-items N` limita elementos EMITIDOS em todo array e reporta `agent_surface.secondary_capped`
- SAIBA que `--max-items` é DISTINTA de `--limit` e de `-k`, que limitam a CONSULTA e não a saída
- `--sort <CHAVE>` ordena ascendente; números comparam numericamente; sem a chave vai para o FIM
- `--dedupe-by <CHAVE>` descarta repetições posteriores; elementos sem a chave são SEMPRE mantidos
- `--count-only` devolve `{"count": N}`, contado DEPOIS de filter, dedupe e max-items
- `--truncate-content N` corta strings por CARACTERE, nunca por byte, e NUNCA parte UTF-8
- `--max-output-bytes N` limita o envelope DESCARTANDO elementos do fim, NUNCA fatiando o JSON
- A ORDEM é FIXA — filter, sort, dedupe, max-items, select, count-only, truncate-content, max-output-bytes
- PARSEIE `agent_surface` quando um knob estiver ativo — `input_count`, `output_count`, `content_truncated`, `output_truncated`, `dropped`
- SAIBA que o array é localizado por `results`, `items`, `entities`, `memories`, `hits`, `rows`, `matches`, `data`, nessa ordem


## Catálogo Completo de Comandos
- TOP-LEVEL, 50 verbos — `init` `remember` `remember-batch` `ingest` `recall` `read` `list` `forget` `purge` `rename` `split-body` `edit` `history` `restore` `hybrid-search` `health` `migrate` `namespace-detect` `optimize` `stats` `sync-safe-copy` `backup` `vacuum` `link` `unlink` `deep-research` `related` `graph` `export` `fts` `vec` `prune-relations` `prune-ner` `slots` `embedding` `pending-embeddings` `cleanup-orphans` `memory-entities` `cache` `delete-entity` `reclassify` `rename-entity` `merge-entities` `enrich` `reclassify-relation` `normalize-entities` `schema` `completions` `config` `help`
- SAIBA que `debug-schema` funciona porém está OCULTO do `--help`; USE-O para o schema vivo do banco
- Família `graph` — `traverse` `stats` `entities` `recompute-degree`
- Família `config` — `add-key` `list-keys` `remove-key` `doctor` `path` `set` `get` `list` `unset`
- Família `fts` — `rebuild` `check` `stats`; família `vec` — `orphan-list` `purge-orphan` `stats`
- Família `slots` — `status` `release` `cleanup`; família `cache` — `clear-models` `list` `stats`
- Família `embedding` — `status` `list` `abandon`, com `pending-embeddings` como família alias
- `completions` — `bash|zsh|fish|elvish|powershell`
- `schema` emite 76 linhas NDJSON `{"id","invoke"}`; `schema --name <ID>` emite o JSON Schema; ID desconhecido sai com exit 4


## Chave e Catálogo de Modelos da OpenRouter
- ADICIONE a chave por stdin — `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`
- VERIFIQUE com `config list-keys --json`, que devolve `provider`, `fingerprint`, `masked_value` e `added_at`
- DIAGNOSTIQUE as camadas com `config doctor --json` ANTES de qualquer chamada paga
- REMOVA com `config remove-key <fingerprint> --json`; LOCALIZE o arquivo com `config path --json`
- NUNCA passe `--openrouter-api-key` no histórico de shell; SEMPRE prefira `config add-key --from-stdin`
- SAIBA que esta CLI NÃO tem verbo de catálogo; a lista viva vem da API da OpenRouter por HTTP
- LISTE embeddings — `curl -s https://openrouter.ai/api/v1/embeddings/models | jaq -r '.data[].id' | sort`
- LEIA preço vivo — `curl -s https://openrouter.ai/api/v1/embeddings/models | jaq -r '.data[]|"\(.id) \(.pricing.prompt)"'`
- FILTRE structured outputs — `curl -s https://openrouter.ai/api/v1/models | jaq -r '.data[]|select(.supported_parameters|index("structured_outputs"))|.id'`
- ARMADILHA CENTRAL — `:nitro` NUNCA aparece como id no catálogo, porque é roteamento aplicado em runtime
- NUNCA valide um modelo `:nitro` contra o catálogo; a validação REJEITA modelo que a API aceita
- CONFIRME o modelo pela PROVA e nunca pelo catálogo — escreva memória descartável e confira a cobertura de vetor


## Modelos de Embedding da OpenRouter
- PASSE `--embedding-model <MODELO>` sempre que usar `--embedding-backend openrouter`; NÃO há default e a omissão sai com exit 78
- USE `qwen/qwen3-embedding-8b` como escolha operacional PADRÃO
- USE `nvidia/llama-nemotron-embed-vl-1b-v2:free` no caminho GRATUITO, respeitando o limite por minuto
- OUTROS ids válidos — `qwen/qwen3-embedding-4b`, `openai/text-embedding-3-small`, `openai/text-embedding-3-large`
- OUTROS ids válidos — `perplexity/pplx-embed-v1-0.6b`, `baai/bge-m3`, `mistralai/mistral-embed-2312`
- OUTROS ids válidos — `google/gemini-embedding-2`, `google/gemini-embedding-001`, `voyageai/voyage-4`
- NUNCA escreva `google/gemini-embedding-005`; esse id NÃO existe e a chamada sai com exit 78
- NUNCA crave preço nesta skill nem em prompt; CONSULTE `pricing.prompt`, porque preço muda sem aviso
- SAIBA que o modelo propaga a TODO caminho de embed — `remember` `remember-batch` `ingest` `edit` `restore` `split-body` `recall` `hybrid-search` `deep-research` `rename-entity` `init` `enrich`
- NUNCA troque de modelo num corpus já embedado sem rodar `enrich --operation re-embed --target all` em seguida


## Modelos de Texto da OpenRouter
- SAIBA que modelo de texto serve SOMENTE extração e enriquecimento, NUNCA embedding
- PASSE `--openrouter-model <MODELO>` DEPOIS do verbo `enrich`; a omissão falha antes da rede
- USE `deepseek/deepseek-v4-flash:nitro` como escolha operacional PADRÃO para vazão
- OUTROS ids — `deepseek/deepseek-v4-flash`, `deepseek/deepseek-v4-pro`, `google/gemini-3.1-flash-lite`
- OUTROS ids — `minimax/minimax-m3`, `minimax/minimax-m2.7`, `minimax/minimax-m2.7:nitro`
- OUTROS ids — `openai/gpt-oss-120b`, `openai/gpt-oss-120b:nitro`, `xiaomi/mimo-v2.5`, `xiaomi/mimo-v2.5-pro`
- OUTROS ids — `z-ai/glm-5.2`, `z-ai/glm-5.2:nitro`
- SAIBA que `:nitro` escolhe o provedor mais rápido a preço maior e NÃO é listado no catálogo
- EXIJA suporte a `structured_outputs`; sem ele a OpenRouter devolve erro explícito na extração
- CONFIRME o gasto real parseando `usage.cost` no envelope do enrich


## Registro de Configuração XDG
- LEIA o registry vivo com `config doctor --json | jaq -r '.knobs[].key'`; ele tem 70 chaves
- DEFINA com `config set <chave> <valor>`; LEIA com `config get <chave>`; LIMPE com `config unset <chave>`
- LISTE gravados com `config list --json` e resolvidos com `config list --effective --json`
- Superfície agent — `agent_surface.max_items` 0, `agent_surface.max_output_bytes` 0, `agent_surface.truncate_content` 0
- Cache e CLI — `cache.dir`, `cli.max_instances`, `cli.no_input` false, `cli.stdin_timeout_secs` 60
- Banco — `db.busy_base_delay_ms` 300, `db.busy_retries` 5, `db.path`, `db.query_timeout_ms` 5000
- Exibição — `display.tz` UTC, `i18n.lang` en; namespace — `namespace.default` global
- Embedding — `embedding.backend`, `embedding.model`, `embedding.dim` 1024, `embedding.batch_size` 32, `embedding.timeout_secs` 300
- Cache de embedding — `embedding.entity_cache_max_entries` 10000, `embedding.entity_cache_ttl_secs` 3600
- Ritmo do enrich — `enrich.circuit_breaker_reset_secs` 60, `enrich.rate_limit_deadline_secs` 3600, `enrich.yield_every_n_items` 10
- Lotes do enrich — `enrich.reembed_claim_batch` 32, `enrich.scan_page_size` 512
- Entity connect — `enrich.entity_connect.default_limit` 100, `enrich.entity_connect.large_ns_limit` 25
- Descrições — `enrich.entity_description.corpus_top_k` 8, `.domain` auto, `.grounding_threshold` 0.30, `.neighbour_top_k` 12
- Descrições — `enrich.entity_description.min_corpus_chars` 40, `.quality_sample` 50, `.snippet_chars` 2000
- Validação de tipo — `enrich.entity_type_validate.corpus_top_k` 8, `.min_corpus_chars` 40, `.neighbour_top_k` 12, `.snippet_chars` 2000
- PARSEIE `retyped` no resumo para saber quantos rótulos MUDARAM; confirmação e abstenção caem em `skipped` com sua `reason`
- Ingest e limites — `ingest.low_memory` false, `limits.max_entities_per_memory` 50, `limits.max_relations_per_memory` 50
- Transporte LLM — `llm.backend`, `llm.model`, `llm.fallback` none, `llm.openrouter_timeout_secs` 600, `llm.probe_timeout_ms` 800
- Slots LLM — `llm.max_host_concurrency`, `llm.slot_wait_secs` 300, `llm.slot_no_wait` false, `llm.worker_rss_mb` 350, `llm.skip_embedding_on_failure` false
- Log — `log.format` pretty, `log.level` warn, `log.retention_days` 7, `log.rotation` daily, `log.to_file` false
- Rede — `network.chat_url`, `network.embed_url`, `network.openrouter.chat_url`, `network.openrouter.embeddings_url`
- Paralelismo — `parallelism.embed_runtime_threads`, `parallelism.max_total_workers` 64, `parallelism.rayon_threads`
- Busca e runtime — `search.hybrid.max_graph_results` 50, `retry.disable` false, `shutdown.ignore` false, `system.max_load_per_ncpu` 2.0
- NUNCA declare chave fora deste registry; chave desconhecida sai com exit 1 e uma sugestão


## Paralelismo e Multiprocessamento
- SAIBA que existem TRÊS knobs distintos e que confundi-los é a causa mais comum de vazão baixa
- KNOB 1 é `--llm-parallelism N`, DEPOIS do verbo, que abre o fan-out de EMBEDDING, com clamp 1..32
- SAIBA que SOMENTE `remember`, `remember-batch`, `ingest`, `edit` e `enrich` a declaram; em `restore` ou `split-body` sai com exit 2
- KNOB 2 é `--rest-concurrency N`, DEPOIS de `enrich`, com clamp 1..16 e default 8
- SAIBA que `--rest-concurrency` é o ÚNICO knob de fan-out do enrich em modo openrouter
- SAIBA que `--llm-parallelism` é INERTE no enrich openrouter e apenas emite um aviso
- KNOB 3 é `--ingest-parallelism N`, que paraleliza ARQUIVOS no `ingest`, default `max(1, cpus/2).min(4)`
- SAIBA que existe um TETO CONJUNTO invisível no `--help` de qualquer flag isolada
- CALCULE o teto como `parallelism.max_total_workers` dividido pela `--max-concurrency` resolvida
- SAIBA que `max_total_workers` vale 64 por padrão, então `--max-concurrency 4` deixa 16 permits por processo
- SAIBA que pedir `--llm-parallelism 32` sob `--max-concurrency 8` entrega 8, e nunca 32, sem erro algum
- REDUZA `--max-concurrency` quando quiser fan-out ALTO num processo, porque os dois disputam o mesmo orçamento
- NUNCA lance N processos de enrich contra um banco; o job singleton REJEITA o segundo com exit 75
- NUNCA lance N processos de `deep-research`; DEIXE o verbo paralelizar as subconsultas internamente
- PARALELIZE com segurança apenas LEITURAS concorrentes, com pool baixo, que não disputam o singleton
- PASSE `--wait-lock <SEGUNDOS>` uma ÚNICA vez para aguardar slot; NUNCA faça busy-loop sobre exit 75


## Escrita Passo 1 — Fórmulas de Embedding
- DEFINA o prefixo W como `sqlite-graphrag --embedding-backend openrouter --embedding-model <EMB> --openrouter-timeout 300 --llm-backend none`
- USE `<EMB>` igual a `qwen/qwen3-embedding-8b`, ou `nvidia/llama-nemotron-embed-vl-1b-v2:free` no caminho gratuito
- REMEMBER — `echo '{"body":"texto","entities":[{"name":"jwt","entity_type":"concept"}],"relationships":[{"source":"jwt","target":"auth-svc","relation":"uses","strength":0.8}]}' | W remember --db ./g.sqlite --name <n> --type decision --description "desc" --graph-stdin --force-merge --llm-parallelism 16 --json`
- ESCOLHA UMA fonte de corpo — `--body`, `--body-file`, `--body-stdin` ou `--graph-stdin`
- SAIBA que `--graph-file` COMBINA com `--body`, `--body-file` ou `--body-stdin`, e é a quarta fonte de grafo
- REMEMBER extras — `--enqueue-enrich` `--strict-name` `--replace-graph` `--dry-run` `--enable-ner` `--metadata` `--metadata-file` `--session-id` `--expected-updated-at` `--entities-file` `--relationships-file` `--clear-body` `--max-rss-mb`
- REMEMBER-BATCH — `W remember-batch --db ./g.sqlite --transaction --llm-parallelism 16 --json` lendo NDJSON no stdin
- SAIBA que cada linha de criação EXIGE `description` não vazia e `type`; `--fail-fast` para na primeira linha ruim
- INGEST — `W ingest --db ./g.sqlite ./docs --mode none --recursive --pattern "*.md" --type document --ingest-parallelism 4 --llm-parallelism 16 --json`
- SAIBA que `ingest --mode` aceita SOMENTE `none`; `--resume` e `--retry-failed` foram REMOVIDAS
- INGEST extras — `--low-memory` `--max-files` `--max-cost-usd` `--auto-describe` `--no-auto-describe` `--name-prefix` `--max-name-length` `--enrich-after`, mais `--force-merge` deduplicando por `body_hash`
- EDIT — `W edit --db ./g.sqlite --name <n> --body-file novo.md --llm-parallelism 16 --json`, ou `--description`, `--memory-type`, `--force-reembed`
- SAIBA que `edit --body-file` NÃO exige fonte de grafo, ao contrário de `remember --body`, o que faz de `edit` o verbo certo para acrescentar a memória existente
- EDIT sob concorrência — PASSE `--expected-updated-at <ts>`; exit 3 significa RECARREGUE e RETENTE
- RESTORE — `W restore --db ./g.sqlite --name <n> --version <N> --json`, que RE-EMBEDA o corpo restaurado
- SPLIT-BODY — `W split-body --db ./g.sqlite --name <N> --json`, ou `--batch --threshold 25000`
- SAIBA que as filhas do split NÃO são embedadas inline; EXIGEM `enrich --operation re-embed --target memories` separado
- RESPEITE 512000 bytes por corpo e 512 chunks; o chunking liga acima de 8000 caracteres
- NUNCA misture fontes de corpo; NUNCA faça `fd | xargs remember`, USE `ingest`
- `--type` aceita — `user` `feedback` `project` `reference` `decision` `incident` `skill` `document` `note`
- No graph-stdin SOMENTE `name`, `entity_type` com `type` como alias, e `description` opcional
- PROIBIDOS no graph-stdin — `observations`, `aliases` e extras livres, que saem com exit 1
- ACRESCENTE `--strict-entity-types` para recusar tipo fora dos treze canônicos, irmã de `--strict-name`
- PARSEIE todo envelope de escrita buscando `entities_created[]` e `enrich_recommended[]`


## Enrich Passo 2 — Fórmulas
- EXECUTE o enrich como processo DISTINTO somente depois que a escrita devolveu exit 0
- DEFINA o prefixo E como `sqlite-graphrag enrich --db ./g.sqlite --mode openrouter --openrouter-model <TXT> --rest-concurrency 16`
- LIGAR — `E --operation memory-bindings --until-empty --max-runtime 3600 --max-attempts 8 --json`
- DESCREVER — `E --operation entity-descriptions --entity-names jwt,auth-svc --force-redescribe --json`
- CONECTAR dry run — `E --operation entity-connect --dry-run --limit 50 --json`
- CONECTAR drain — troque `--dry-run` por `--until-empty --max-runtime 600`
- CONECTAR ancorado — ACRESCENTE `--anchor-memory <nome>` ou `--entity-names a,b` para escopar o scan
- RE-EMBED — `W enrich --db ./g.sqlite --operation re-embed --target all --mode openrouter --openrouter-model <TXT> --until-empty --rest-concurrency 16 --json` e depois `health --json`
- STATUS sem chamada de LLM — `sqlite-graphrag enrich --db ./g.sqlite --status --operation <OP> --quality-sample 50 --json`
- SEMPRE passe `--operation` no `--status`; sem ela ele cai em `memory-bindings` e mostra `empty` com outra fila cheia
- RECUPERAR — `--list-dead` e depois `--requeue-dead`; `--list-skipped` e depois `--requeue-skipped`
- AGUARDE singleton preso com `--wait-job-singleton SEGS` ou sobrescreva com `--force-job-singleton`


## Regras do Pipeline de Enrich
- PASSE `--operation` e `--mode` juntas em toda operação de LLM; omitir `--mode` sai com exit 2
- ISENTOS de `--mode` os inspetores read-only — `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` e `--dry-run`
- Operações que PERSISTEM — `memory-bindings`, `augment-bindings`, `entity-descriptions`, `body-enrich`, `body-extract`, `re-embed`, `weight-calibrate`, `relation-reclassify`, `entity-connect`, `entity-type-validate`, `description-enrich`, `cross-domain-bridges`, `domain-classify`, `deep-research-synth`
- Operação de SCAN e REPORT apenas — `graph-audit`, que NUNCA muta estrutura
- SAIBA que `augment-bindings` EXIGE `--memory-names`, `--names` ou `--names-file`
- CONTROLE `entity-type-validate` com `--allowed-types` e `--on-unknown-type keep|fallback|strict`, sendo `keep` o padrão
- PREFIRA `--entity-names` por entidade e `--memory-names` por memória; `--names` é alias de compatibilidade
- PARE quando um empty match exibir `matched=0` mais um `hint`; NUNCA amplie às cegas
- PASSE `--target memories|entities|chunks|all` SOMENTE em `re-embed`, com padrão `memories`
- SAIBA que claim, `--resume`, `--retry-failed` e `--until-empty` são escopados a ESTA operação e a ESTE namespace
- SAIBA que `--force-redescribe` reabre `skipped` e `done` uma vez por processo, e NUNCA reabre `dead`
- LEIA no `--status` o campo `operation` PRIMEIRO, porque ele declara qual fila foi medida
- LEIA também `scan_backlog`, `queue_pending`, `queue_dead`, `eligible_now`, `waiting` e `quality_pct`
- SAIBA que `eligible_now == 0` com `queue_pending > 0` é COOLDOWN, e não travamento
- SAIBA que `state` é `draining`, `cooldown`, `pending-scan` ou `blocked_dead`; limpe `blocked_dead` PRIMEIRO com requeue ou prune
- PARSEIE `budget_exhausted`, que é fim de orçamento, e `preempted_for_gate`, que é cessão deliberada
- PASSE `--preflight-check` antes de drain pago, para abortar cedo em janela de rate limit fechada
- PASSE `--ignore-backoff` para itens em cooldown `next_retry_at`, e `--reset-stale-claims` para claim preso após kill forçado
- AJUSTE body-enrich com `--min-output-chars` 500, `--max-output-chars` 2000 e `--prompt-template <PATH>`
- SAIBA que o portão de preservação do `body-enrich` é `--preserve-threshold` 0.7 por Jaccard trigrama, e que `--preserve-check` é INERTE no parser
- AJUSTE descrições inline com `--entity-description-domain` e `--entity-description-grounding-threshold`
- ORDENE corridas longas como memory-bindings, entity-descriptions, entity-connect, ou PASSE `--ops-gate`


## Fórmulas de Leitura e Busca
- DEFINA o prefixo R como `sqlite-graphrag --quiet --embedding-backend openrouter --embedding-model <EMB> --openrouter-timeout 300 --fail-on-degraded`
- USE o padrão de três camadas — `hybrid-search`, depois `read --name`, depois `related` ou `graph traverse`
- HYBRID-SEARCH — `R hybrid-search --db ./g.sqlite "consulta" --k 10 --with-graph --max-hops 2 --min-weight 0.3 --rrf-k 60 --json`
- Ajuste — `--weight-vec 1.0 --weight-fts 1.0`, `--type <kind>`, `--max-graph-results N`
- HYBRID offline sem custo — `sqlite-graphrag hybrid-search --db ./g.sqlite "consulta" --k 10 --fallback-fts-only --json`
- RECALL — `R recall --db ./g.sqlite "consulta" --k 10 --json`; extras `--no-graph`, `--precise`, `--max-distance <f>`, `--all-namespaces`
- DEEP-RESEARCH — `R deep-research --db ./g.sqlite "pergunta" --k 20 --max-hops 3 --max-sub-queries 7 --max-results 50 --with-bodies -o /tmp/dr.json --json`
- Ajuste do DEEP-RESEARCH — `--graph-decay`, `--graph-min-score`, `--max-neighbors-per-hop`, `--max-cost-usd`, `--timeout`
- Controle manual — `--sub-query-strategy manual --sub-queries-file PATH`
- ESCREVA envelope grande com `-o PATH`; PARSEIE o ack `{written, bytes, blake3}` e confira bytes maiores que zero
- READ — `sqlite-graphrag read --db ./g.sqlite --name <kebab> --json`; extras `--with-graph` e `--format raw`
- SAIBA que `--no-body` omite o corpo da resposta e que `--show-entities` acrescenta as entidades ligadas
- LIST — `sqlite-graphrag list --db ./g.sqlite --type <kind> --limit N --offset N --include-deleted --json`
- HISTORY — `sqlite-graphrag history --db ./g.sqlite --name <n> --diff --json`
- RELATED — `sqlite-graphrag related --db ./g.sqlite <nome> --hops 2 --relation uses --json`
- MEMORY-ENTITIES — `sqlite-graphrag memory-entities --db ./g.sqlite --name <memoria> --json`, e parse de `entities[].description`
- RENAME-ENTITY no caminho de embed — `R rename-entity --db ./g.sqlite --name <old> --new-name <new> --json`, que RE-EMBEDA a entidade
- PARSEIE `recall` como `{name, snippet, distance, score, source}`, com `source` em `direct`, `graph` ou `fts_fallback`
- PARSEIE `hybrid-search` como `{name, combined_score, vec_rank, fts_rank}` mais `graph_matches[]`
- PARSEIE `deep-research` como `sub_queries[]`, `results[]`, `evidence_chains[]`, `graph_context` e `stats`
- LEIA `vec_degraded` e `vec_degraded_reason` em toda leitura; presentes, o resultado veio de BM25 lexical
- NUNCA confunda `distance` com `combined_score`; NUNCA aumente hops sem ler `graph stats` antes


## Grafo de Entidades
- LINK — `sqlite-graphrag link --db ./g.sqlite --from <a> --to <b> --relation uses --weight 0.8 --create-missing --entity-type concept --json`
- LINK por id — `link --from-id <N> --to-id <M> --relation uses --json`; NUNCA passe dígitos puros como nomes
- SAIBA que `--strength` é ALIAS de `--weight` no `link`, porque o schema de entrada chama a mesma propriedade de `strength`
- LINK estrito — ACRESCENTE `--strict-relations` para rejeitar relação fora do conjunto canônico
- UNLINK — `unlink --from <a> --to <b> --relation <tipo>`, ou `--entity <nome> --all`, ou `--memory <m> --entity <e>`
- TRAVERSE — `sqlite-graphrag graph traverse --db ./g.sqlite --from <raiz> --depth 2 --fuzzy --json`
- LISTAR entidades — `graph entities --db ./g.sqlite --json`, lendo `.entities[]` e NUNCA `.items[]`
- ORDENE com `--sort-by name|degree|created-at` mais `--order asc|desc`, e pagine com `--limit` e `--offset`
- FILTRE com `graph entities --entity-type person` contra os 13 tipos canônicos
- EXPORTAR — `graph --format json|dot|mermaid|ndjson --output <path>`; MEÇA com `graph stats --json`
- RECOMPUTAR — `graph recompute-degree --json` após delete, merge ou prune, porque o grau NÃO é automático
- MERGE — `merge-entities --names "a,b,c" --into <alvo> --json`, ou `--ids 12,17 --into-id 3`
- NUNCA coloque `--into-id` dentro de `--ids` nem `--into` dentro de `--names`; merge auto-referencial é REJEITADO
- PASSE `--cross-namespace` no merge SOMENTE quando cruzar namespaces for intencional
- DELETE — `delete-entity --name <n> --cascade --json`; RENAME de memória — `rename --name <old> --new-name <new> --json`
- RENAME de entidade por id — `rename-entity --id <N> --new-name <new> --json`
- RECLASSIFICAR — `reclassify --name <n> --new-type <kind>`, ou `--from-type <old> --to-type <new> --batch`
- RECLASSIFICAR relações — `reclassify-relation --from-relation <old> --to-relation <new> --batch --literal-from --literal-to`
- PODAR — `prune-relations --relation mentions --dry-run` e repetir com `--yes`; `normalize-entities --yes`; `prune-ner --all --yes`
- RELAÇÕES canônicas — `applies-to` `uses` `depends-on` `causes` `fixes` `contradicts` `supports` `follows` `related` `mentions` `replaces` `tracked-in`
- MAPEIE não canônicas — `adds` e `creates` para `causes`, `implements` para `supports`, `blocks` para `contradicts`, `tested-by` para `related`, `part-of` para `applies-to`
- TIPOS canônicos — `project` `tool` `person` `file` `concept` `incident` `decision` `memory` `dashboard` `issue_tracker` `organization` `location` `date`
- VALIDE nome de entidade como kebab-case ASCII minúsculo, mínimo 2 caracteres, sem newline, sem all-caps curto, nunca só dígitos
- NUNCA use `mentions` como relação padrão; escritas de grafo são ADITIVAS e sem teto de grau


## Manutenção e Diagnóstico
- HEALTH — `sqlite-graphrag health --db ./g.sqlite --json` para `integrity_ok`, `schema_version`, `vec_*_missing`, `vec_*_coverage_pct` e `embedding_key`
- DISPARE `enrich --operation re-embed` sempre que algum `vec_*_missing` for maior que zero
- MIGRAR — `migrate --dry-run --json` e depois `migrate --json`; OTIMIZAR — `optimize --json`
- FTS — `fts check --json`, `fts stats --json`, `fts rebuild --json` com o índice degradado
- VEC — `vec orphan-list --json`, depois `vec purge-orphan --yes`, e `vec stats --json`
- Backlog — `embedding status --json`, `embedding list --json`, `embedding abandon`
- SLOTS — `slots status --json`, `slots release --slot-id <N> --yes`, `slots cleanup --yes`
- FORGET suave — `forget --name <n> --json`; PURGE — `purge --db ./g.sqlite --yes --now --dry-run --json` e repetir sem `--dry-run`
- SAIBA que `purge --yes` sozinho mantém a retenção de 90 dias; `--now` é o alias de `--retention-days 0`
- SIGA o purge com `cleanup-orphans --yes` e depois `vacuum --json`
- EXPORTAR — `export --namespace <ns> --type <kind> --json`; MEDIR — `stats --json`
- BACKUP — `backup --output backup.sqlite --json`; SNAPSHOT — `sync-safe-copy --dest <path>`
- INSPECIONAR — `namespace-detect --json`, `cache list --json`, `cache stats --json`, `cache clear-models --yes`
- INSTALAR completions — `completions bash|zsh|fish|elvish|powershell`
- AGENDE semanalmente — purge, `cleanup-orphans`, `prune-relations --relation mentions`, `vacuum`, `optimize`, `sync-safe-copy`


## Orquestração Headless — Codex, Claude Code e OpenCode
- SAIBA que a CLI headless é o CHAMADOR e este binário é o CHAMADO; NUNCA confunda com `enrich --mode`
- SAIBA que o embedding é SEMPRE OpenRouter e que a CLI headless NUNCA gera vetor
- DEFINA C como `codex exec -m <MODELO> --json --skip-git-repo-check -C <DIR>`
- DEFINA K como `claude -p --model <MODELO> --output-format json --add-dir <DIR>`
- DEFINA O como `opencode run --model <MODELO> --format json --dir <DIR>`
- SAIBA que `codex exec` aceita `-s <SANDBOX_MODE>`, `--approve-for-me`, `--dangerously-bypass-approvals-and-sandbox`, `--output-schema` e `-o <FILE>`
- SAIBA que `claude -p` aceita `--permission-mode <MODO>`, `--dangerously-skip-permissions` e `--session-id <uuid>`
- SAIBA que `opencode run` aceita `--agent`, `--continue`, `--session <id>`, e que `opencode models` lista os modelos
- PASSO 1 embeda com o prefixo W; PASSO 2 enriquece com o prefixo E; NUNCA junte os dois num só prompt
- ORDENE ao invocador EXECUTAR o passo 1, LER o exit e a cobertura de vetor, e SÓ ENTÃO executar o passo 2
- REMEMBER por codex — `C "Rode W remember --db ./g.sqlite --name n --type decision --description d --graph-stdin --llm-parallelism 16 --json; confirme exit 0; SÓ ENTÃO rode E --operation memory-bindings --until-empty --json"`
- REMEMBER por claude code — a MESMA ordem trocando C por K
- REMEMBER por opencode — a MESMA ordem trocando C por O
- REMEMBER-BATCH por qualquer invocador — troque o passo 1 por `W remember-batch --db ./g.sqlite --transaction --llm-parallelism 16 --json` lendo NDJSON
- INGEST por qualquer invocador — troque o passo 1 por `W ingest --db ./g.sqlite ./docs --mode none --recursive --ingest-parallelism 4 --llm-parallelism 16 --json`
- EDIT por qualquer invocador — troque o passo 1 por `W edit --db ./g.sqlite --name n --body-file novo.md --llm-parallelism 16 --json`
- RESTORE por qualquer invocador — troque o passo 1 por `W restore --db ./g.sqlite --name n --version N --json`
- SEM invocador, direto na OpenRouter — RODE o passo 1 com W e o passo 2 com E, cada um como processo DISTINTO
- PARALELIZE o passo 1 com `--llm-parallelism 16` e o passo 2 com `--rest-concurrency 16`, sob o teto conjunto
- NUNCA paralelize o passo 2 lançando processos; o singleton devolve exit 75 ao segundo


## Regras de Instrução de Prompt
- "lembre isso" — EXECUTE `remember --force-merge` com `--graph-stdin` curado, e depois um enrich SEPARADO
- "acrescente à memória X" — EXECUTE `edit --name X --body-file <arquivo>`, que NÃO exige fonte de grafo
- "o que você sabe sobre X" — EXECUTE `hybrid-search "X" --k 10 --json` e depois `read --name <nome> --json`
- "como X se relaciona com Y" — EXECUTE `graph traverse --from X --depth 2 --json` ou `related X --hops 2 --json`
- "pesquisa profunda sobre X" — EXECUTE `deep-research "X" --k 20 --max-hops 3 -o PATH --json` com `--quiet`
- "conecte entidades isoladas" — EXECUTE `enrich --operation entity-connect` em dry, depois drain, depois `--status`
- ANTES de criar — EXECUTE `hybrid-search "<nome>" --k 5 --json` e USE `--force-merge` em caso de duplicata
- DEPOIS de criar ou atualizar — PARSEIE `read --name <nome> --json` buscando `{name, description, body_length}`


## Antipadrões
- NUNCA encadeie escrita e enrich com `&&`; o único encadeamento sancionado é `ingest --enrich-after`
- NUNCA coloque `--db`, `--namespace`, `--json` ou `--llm-parallelism` antes do verbo
- NUNCA omita `--embedding-backend openrouter` numa escrita, porque `auto` persiste memória sem vetor em silêncio
- NUNCA omita `--fail-on-degraded` numa leitura de agente, porque degradada devolve palavra-chave com exit 0
- NUNCA peça `--mode codex`, `--mode claude-code` ou `--mode opencode`; o único valor aceito é `openrouter`
- NUNCA trate o texto do `--help` como prova de COMPORTAMENTO; ele anuncia `--preserve-check`, que nenhuma linha lê, e imprime `--no-fts-skip-when-functional`, que o parser recusa com exit 2
- NUNCA rode múltiplos processos de enrich ou de deep-research num banco; escale DENTRO de um processo
- NUNCA valide um modelo `:nitro` contra o catálogo da OpenRouter, que não o lista
- NUNCA use `SQLITE_GRAPHRAG_*`, `ingest --resume` nem `ingest --retry-failed`; todos foram removidos
- NUNCA prove embedding por `backend_invoked` sozinho; NUNCA ignore `entities_created`, `enrich_recommended` nem o exit 19
- NUNCA use MCP de memória, MEMORY.md ou diário Markdown ad-hoc
- NUNCA abra o `.sqlite` com o shell `sqlite3` nem com editor
