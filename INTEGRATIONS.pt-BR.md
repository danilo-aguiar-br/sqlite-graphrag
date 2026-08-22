# Integrações

> Leia este documento em [inglês (EN)](INTEGRATIONS.md)


> 27 agentes de IA e 20+ plataformas em um único contrato de CLI (21 catalogados + 6 community)

- Leia a versão em inglês em [INTEGRATIONS.md](INTEGRATIONS.md)
- Cada receita abaixo está pronta para copiar e custa zero para executar
- **v1.2.8 (atual):** a família morta `pending` foi removida — três verbos que jamais devolviam linha — levando o catálogo de topo a 50 verbos e o de schemas a 76 contratos. O `hybrid-search` perde `fts_bm25`, campo que o schema publicava e nenhum caminho de código preenchia. O reaper de órfãos roda sobre `sysinfo` em vez de `/proc`, então o macOS para de reportar zero órfão sem ter medido. Um gate novo recusa identificador e comentário em português em `src/`, isentando somente literais de string traduzidos sob `src/i18n/`. Herdado da v1.2.8: o `tests/rustdoc_link_gate.rs` fecha estaticamente, em milissegundos e sem lock de build, a classe de defeito que só o `cargo doc` enxergava — doc comment público apontando item privado. O `graph --format ndjson` passa a honrar a superfície agent-native em vez de parsear as flags e descartá-las: `--select` e `--truncate-content` valem por registro, os knobs de conjunto completo são recusados antes do primeiro byte, e a linha de sumário carrega o bloco `agent_surface`. `--select`, `--filter`, `--sort` e `--dedupe-by` aceitam `entity_type` e `type` como um único campo, então a chave aprendida no `graph entities` também resolve contra o snapshot do `graph --format json`; o wire não muda e a projeção responde sob a grafia que você pediu. Herdado da v1.2.8: `remember` aceita o nome da memória posicionalmente ou por `--name`, nunca os dois, fechando a última divergência com `edit` / `read` / `forget` / `history`. Um `entity_type` declarado fora dos treze canônicos é ACEITO E ARMAZENADO COMO ESCRITO: a dobra sobre um tipo canônico é HISTÓRICA, removida pela migração V017 que abriu o vocabulário, e `normalize_entity_type` (`src/entity_type.rs`) hoje só faz trim, lowercase e troca `-` por `_`. Toda etiqueta não canônica é reportada em `warnings` no `remember` E no `remember-batch`, e `--strict-entity-types` recusa a escrita — o irmão de `--strict-name`. `remember --dry-run` reporta `entities_parsed`, `relationships_parsed` e as etiquetas não canônicas que armazenaria, em vez de quatro campos e `warnings: null`. Dois contratos de entrada passam a ser publicados: `schema --name graph-input` para a forma de fio de `--graph-stdin` / `--graph-file`, e `schema --name remember-dry-run` para um envelope que não satisfazia schema algum. Herdado da v1.2.8: `--count-only` é RECUSADO com exit `2` sobre comando paginado cujo limite de fato cortou linhas — `--filter-scope page` aceita a leitura mais estreita e `agent_surface.count_scope` passa a reportar `page` em vez de `matched`. Teto top-k nunca é recusado. O mesmo knob, mais `--sort`, `--dedupe-by` e `--max-output-bytes`, é recusado em `export` e `ingest`, que emitem um registro por linha. Depois de uma escrita sem array de resultado o knob é suprimido em vez de honrado, e `count_only_suppressed` reporta. `export` e `embedding list` passam a declarar o teto de consulta, então a superfície para de reportar `query_limited: null` sobre uma página. Herdado da v1.2.8: o vocabulário canônico de relações é kebab-case (`applies-to`, `depends-on`, `tracked-in`) em UM lugar — `parsers::CANONICAL_RELATIONS` — e `create_or_fetch_relationship` / `upsert_relationship` canonizam na fronteira de persistência, então nenhum caminho de escrita grava grafia divergente. Os filtros de relação do `related` casam as duas grafias, alcançando linhas escritas por binários anteriores. `link` aceita `--strength` como alias de `--weight`. `remember` reporta em `warnings` todo `entity_type` declarado fora dos treze canônicos — até a V017 essa etiqueta era dobrada num tipo canônico, e desde a V017 ela é armazenada como escrita. O enrich não sobrescreve mais tipo de entidade que não seja o genérico `concept`. `related` é determinístico: a ordenação é total, então invocações idênticas devolvem resultados idênticos. O schema é **v17** — a migração V017 abriu o vocabulário de entity_type e `health --json` reporta `schema_version: 17`; `DEFAULT_EMBEDDING_DIM=1024`.
- **v1.2.7:** dez flags globais agent-native (`--select`/`--fields`, `--filter`, `--max-items`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`, `--filter-scope`, `--allow-unknown-keys`; envelopes de falha nunca são filtrados) mais `--no-input`. Chave irresolvível ou predicado sobre página truncada são recusados com exit 2 em vez de responder vazio, e o envelope de falha nomeia os argumentos descartados em `discarded_flags`. Todo envelope de um processo que resolveu banco reporta `db_path_source` e `db_path_resolved` dentro do bloco `agent_surface`, sem exigir flag alguma. Um subcomando que altera estado durável sai com exit 2 sem tocar em nada quando NADA nomeou o alvo — nem `--db` nem `db.path` na configuração. Alvo XDG é permitido, porque `config set db.path` é designação feita uma vez em vez de a cada invocação; `--use-active` aceita o padrão compilado de propósito. O schema estava em **v16** naquela release; a V017 avançou para **v17**, que é o que um binário 1.2.8 reporta; `DEFAULT_EMBEDDING_DIM=1024`. O detalhe por release está em [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md).
- **v1.0.79: todo build é apenas LLM e one-shot.** A geração de embedding delega para um subprocesso headless `claude code` ou `codex` (OAuth). O daemon, o runtime ONNX e a feature `embedding-legacy` foram totalmente removidos; os embeddings são em lote, paralelos (`--llm-parallelism`) e com **1024** dimensões por padrão (`--embedding-dim`, faixa [8, 4096]; v1.2.0).


## Aliases de Flags CLI (desde v1.0.35)
- `recall` e `hybrid-search` aceitam `--limit` como alias de `-k`/`--k`. Os exemplos abaixo usam `--k` e continuam válidos.
- `rename` aceita `--from`/`--to` como aliases de `--name`/`--new-name` (aliases legados `--old`/`--new` continuam suportados).
- Todos os campos JSON `schema_version` (`init`, `stats`, `migrate`, `health`) são emitidos como números JSON (eram string em `init`/`stats`/`migrate` antes da v1.0.35).
- Auto-init via `remember`/`ingest`/etc. agora ativa `journal_mode = wal` corretamente (correção de regressão).

## Novas Flags (desde v1.0.45)
- A extração NER de entidades está **desativada por padrão**. Passe `--enable-ner` em `remember` ou `ingest` para ativar; não existe chave XDG nem override de ambiente para ela.
- `--skip-extraction` está obsoleto e não tem efeito desde v1.0.45 (NER está desativado por padrão); a flag é mantida como no-op oculto para compatibilidade — remova-a dos scripts.
- `--graph-stdin` em `remember` lê um único objeto JSON do stdin contendo `body`, `entities` e `relationships`, sendo a forma preferida de fornecer grafos curados por um LLM.

## Novas Flags (desde v1.0.47)
- O pipeline GLiNER zero-shot NER foi REMOVIDO na v1.0.79 com a feature `ner-legacy`; `--enable-ner` agora executa apenas extração de URL por regex.
- --gliner-variant foi REMOVIDA na v1.1.02: o clap a REJEITA com exit 2, então a invocação que a carrega aborta antes de qualquer trabalho — ela NÃO é tolerada como no-op silencioso. As env vars de produto `SQLITE_GRAPHRAG_GLINER_VARIANT` e `SQLITE_GRAPHRAG_GLINER_THRESHOLD` também são históricas: env de produto não é lida em runtime e não tem efeito.
- Para extração de entidades/relacionamentos curada por LLM rode um passo SEPARADO de `enrich --mode openrouter` após `ingest --mode none`.
- Os tipos de entidade agora incluem `organization`, `location`, `date` além de `person`, `project`, `tool`, `file`, `concept`, `decision`, `incident`, `dashboard`, `issue_tracker`, `memory`.

## Novos Comandos e Flags (desde v1.2.5)

- Crate **`1.2.5`**; pin de consumidores de biblioteca `=1.2.5`. Superfície de saída agent-native aditiva mais `--no-input`; sem mudança de envelope quando nenhuma flag é definida. Schema do DB principal permanece em **v16** (sem migrate; apenas comportamento da fila sidecar). Selo **CAPA** da fila enrich (julho 2026).
- **Isolamento de claim por namespace** — `dequeue_next_pending`, `count_eligible_pending`, `--resume` / `--retry-failed` filtram por `operation` **e** `namespace`. Um drain de enrich em `ai-sdd` não reivindica nem conta mais linhas `global` / ns vazio (reduz HardFailure / circuit-breaker cross-namespace).
- **`--until-empty` conta só esta op+namespace** — `count_eligible_pending` não soma mais *todas* as linhas pending entre operações (zumbis ReEmbed alienígenas não mantêm EntityDescriptions girando com `completed=0`).
- **`--force-redescribe` reabre `skipped`/`done`** — `reopen_force_redescribe_candidates` roda uma vez por processo antes do primeiro enqueue para que `INSERT OR IGNORE` não seja no-op silencioso; nunca reabre `dead` (use `--requeue-dead`).
- **Reconciliação de zumbis de re-embed** — `reconcile_satisfied_reembed_pending` marca linhas ReEmbed pending como `done` quando já existe vetor vivo na dim ativa (`LENGTH(embedding) = dim*4`), limpando zumbis sem chamadas de API.
- **Elegibilidade de re-embed usa comprimento do BLOB** — scan/predicados selecionam linhas CORRUPT / META_AHEAD (`dim=1024` com BLOB 384-d ainda elegível quando `LENGTH(embedding) ≠ target_dim * 4`).
- **Enqueue valida chaves de re-embed** — `entity:{name}` faz strip do prefixo na lookup da entidade (chave da fila permanece `entity:…`; nomes bare ainda funcionam; entidades ausentes rejeitadas). Chaves de chunk validam que `chunk_id` existe em memória não-deletada do namespace alvo.
- **CAPA-D marcadores de baixa qualidade** — bare `%configuration file%` removido; só marcadores compostos (ex.: `is a configuration file`) para que prosa legítima de domínio não vire fodder de force-redescribe.
- Regressões: `enqueue_candidate_accepts_entity_prefixed_reembed_key`, `dequeue_next_pending_isolates_by_namespace`; suíte unitária da fila 38 testes OK.

## Novos Comandos e Flags (desde v1.2.0)

- Crate **`1.2.0`**; pin de consumidores de biblioteca `=1.2.0`. Schema do DB principal permanece em **v16** (sem migrate se já em v16; migrate só da fila sidecar).
- **DEFAULT_EMBEDDING_DIM=1024** — flag `--embedding-dim` / XDG `embedding.dim` ainda sobrescrevem; DBs existentes mantêm `schema_meta.dim` até re-embed.
- Precedência de config: **flag CLI > XDG `config set` > default nomeado**. Product env `SQLITE_GRAPHRAG_*` **não é lida** em runtime e não é o caminho de config.
- `enrich --list-skipped` / `enrich --requeue-skipped` — recuperam sink `preservation_failed` / skipped sem SQL cru (G-PR-3/4).
- Fila enrich multi-namespace — coluna `namespace` + `UNIQUE(namespace, operation, item_key)`; `DELETE` escopado; correção SQL `status='pending'`.
- **GAP-SG-139** — folhas host/XDG (`config`×9, `slots`×3, `cache`×3, `completions`) aceitam `--db` como **no-op** documentado (`src/cli_db_noop.rs`); superfícies de grafo inalteradas.
- Aliases UX: `pending-embeddings status` (= `embedding status`), `cache stats` (= `cache list`); `purge --now` (alias de `--retention-days 0`); `config list --effective` (defaults de dim/log via `constants`, sem `"384"` hard-coded).
- Seal offline: `scripts/e2e_offline_v120.sh` (wrapper histórico `e2e_offline_v118.sh` supersedido; **20/20** no binário de release 1.2.0).

## Novos Comandos e Flags (desde v1.1.06)

- Nome oficial **v1.1.06**; crate `version = "1.1.6"` — pin `=1.1.6`. Sem migração (v16). Fecha **GAP-ENTITY-CONNECT-SCAN-CARTESIAN**: scan O(k) (coocorrência + hub×ilha), chaves `pair:{id1}:{id2}`, `item_type=entity_pair`, deadline no primeiro scan (`InterruptHandle`, Timeout exit 1 ≠ 75), NDJSON `scan_start`/`scan_meta` com `operation` real e backlog dual `backlog_degree0_proxy` + `pairs_enqueued_this_scan`. ADR-0066; suite `tests/v1106_entity_connect_scan_regression.rs`.
- `enrich --operation entity-connect|cross-domain-bridges` é seguro em namespaces `global` grandes (sem hang cartesiano); ambos compartilham o caminho fully-implemented de `entity_connect_seen`.

## Novos Comandos e Flags (desde v1.1.05)
- O nome oficial do release é v1.1.05; o manifesto do crate carrega `version = "1.1.5"` porque o parser SemVer rejeita zero à esquerda no componente patch — faça pin com `=1.1.5`. O `User-Agent` HTTP é `sqlite-graphrag/1.1.5`; o binário de release tem aproximadamente 19 MiB. **Sem migração de schema** (permanece em v16 após a V016 da v1.1.04). Fecha os cinco bugs operacionais do relato de incidente deep-research de sujeito único (`gaps.md`)
- Bug 1: `deep-research` com token único expande em sub-queries multi-aspecto (`source: "aspect"`, facetas EN/PT); estratégia manual via `--sub-query-strategy manual --sub-queries-file`
- Bug 2: `deep-research --output PATH` grava o envelope via algoritmo atomwrite (tempfile → fsync → rename) e emite ack curto no stdout com checksum `blake3`; flag global `--quiet`/`-q` suprime tracing não-erro; contrato stdout-JSON / stderr-logs (nunca `&>` no mesmo arquivo); helpers em `src/atomic_io.rs`
- Bug 3: `graph traverse --from <nome-curto>` — match exato prioritário; sem `--fuzzy`, NotFound (exit 4) inclui sugestões Jaro-Winkler/prefixo; com `--fuzzy`, auto-resolve vencedor claro com warning em stderr (`rapidfuzz`)
- Bug 4: `merge-entities` rejeita self-ref (`--ids` contendo `--into-id`, ou `--names` contendo `--into`) **antes** de qualquer trabalho no DB
- Bug 5: `link --from-id` / `--to-id` resolvem por ID; `validate_entity_name` rejeita nomes só de dígitos (impede entidades fantasma sob `--create-missing`)
- Suite de regressão: `tests/v1105_incident_bugs_regression.rs`

## Novos Comandos e Flags (desde v1.1.02)
- (histórico v1.1.04) O nome oficial do release é v1.1.04; o manifesto do crate carrega `version = "1.1.4"` porque o parser SemVer rejeita zero à esquerda no componente patch — faça pin com `=1.1.4`. O `User-Agent` HTTP é `sqlite-graphrag/1.1.4` (derivado de `CARGO_PKG_VERSION`); o binário de release tem aproximadamente 19 MiB. A v1.1.04 fecha os dois gaps estruturais rastreados em `gaps.md` após a v1.1.03: o GAP-001 (o panic de nested Tokio runtime no `deep-research` está corrigido — o entry point síncrono computa os embeddings por sub-query ANTES de construir seu runtime dedicado, e os três caminhos de embedding OpenRouter adotam o padrão de reentrada `Handle::try_current` + `block_in_place`) e o GAP-002 (o `entity-connect` agora converge via a nova tabela `entity_connect_seen` registrando o veredito do LLM por par). Migração de banco OBRIGATÓRIA: `migrate --json` aplica a V016 (`entity_connect_seen`); o schema avança v15→v16
- O nome oficial da release é v1.1.02; o manifesto do crate carrega `version = "1.1.2"` porque o parser SemVer rejeita zero à esquerda no componente patch — faça pin com `=1.1.2`. O `User-Agent` HTTP é `sqlite-graphrag/1.1.2` (derivado de `CARGO_PKG_VERSION`); o binário de release tem aproximadamente 19 MiB; o schema permanece na versão 15 sem migração. A v1.1.02 fecha os dois gaps residuais deixados após v1.1.01 (o argumento depreciado --gliner-variant é removido de `remember`/`ingest` com clap exit 2; o teto de tokens de embedding vira a variante tipada `AppError::TooManyTokens { tokens, limit }` enforced na borda de escrita), entrega um teste de regressão para o dispatch de re-embed de entidades (tests/reembed_entities_integration.rs), e adiciona `enrich --prune-dead-entity-orphans` para remover linhas dead-letter entity-keyed da fila sidecar
- Vetores de entidade são escritos pelo path REST OpenRouter mesmo sob `--llm-backend none` (a chain de embedding de entidade resolve para `[OpenRouter]`, sem subprocesso), e uma guarda de vetor vazio em `upsert_entity_vec`/`upsert_chunk_vec`/`memories::upsert_vec` mantém linhas sem vetor visíveis ao backfill do re-embed em vez de mascará-las atrás de um BLOB vazio (P1)
- `enrich --operation re-embed --target memories|entities|chunks|all` — backfill retroativo de embedding por tabela de vetor (padrão `memories`, totalmente retrocompatível); `enrich --status` reporta o `scan_backlog` por alvo; os predicados do re-embed também selecionam linhas cujo `dim` gravado diverge do `--embedding-dim` configurado ou cujo blob está vazio (P2, P10)
- `graph recompute-degree` — reconcilia o `entities.degree` cacheado com as contagens reais de arestas em uma única transação IMMEDIATE, por namespace (ou todos), com `--dry-run` e o envelope `{namespace, dry_run, total, updated, zeroed, unchanged, elapsed_ms}` (P3)
- `reclassify-relation --literal-from <REL>` — casa a relação armazenada VERBATIM (sem normalização hífen→underscore na borda do clap), tornando alcançáveis as arestas legadas com hífen como `applies-to`; mutuamente exclusiva com `--from-relation` (P4)
- `merge-entities --ids <a,b> --into-id <N>` e `rename-entity --id <N>` — seleção por ID escopada por namespace para manutenção de entidades quando nomes duplicados entre namespaces bloqueiam merges e renomeações (P5)
- `health --json` ganha `vec_memories_missing`, `vec_entities_missing`, `vec_chunks_missing` e os campos `vec_*_coverage_pct` por tabela; `embedding status --json` ganha os contadores `*_missing` por tabela (P6)
- A desserialização de `EntityType` é um `Deserialize` manual com erro rico de borda listando os 13 valores válidos, exposto como erro de Validação (exit 1) com validação precoce do input de grafo curado (`--graph-stdin`, `--entities-file`) em vez de um erro serde cru (exit 20) (P7)
- Os erros de limite do exit 6 são tipados: `AppError::BodyTooLarge { bytes, limit }` e `AppError::TooManyChunks { chunks, limit }` substituem a mensagem genérica `LimitExceeded` em todo call site de tamanho de corpo — o CÓDIGO de saída continua 6, apenas a MENSAGEM do envelope ganha contexto acionável (P11)
- `ingest --name-prefix <PREFIXO>` — prefixo kebab-case aplicado a todo nome de memória derivado, com o orçamento da parte derivada reduzido para que `prefixo + derivado` sempre respeite o teto de 80 caracteres do nome (P12)

## Novos Comandos e Flags (desde v1.0.94)
- `--embedding-backend auto|openrouter|llm` — seleciona o backend de embedding (flag global)
- `--embedding-model MODEL` — seleciona o modelo de embedding para OpenRouter (flag global, OBRIGATÓRIO com openrouter)
- `--openrouter-api-key KEY` — chave de API para OpenRouter (flag global)
- `--enrich-after` — executa enrich após a conclusão do ingest (flag do ingest)
- **GAP-OR-PROPAGATION**: Todos os 13 paths de embedding agora honram `--embedding-backend` — incluindo `enrich`, `init`, `rename-entity`, `ingest --mode claude-code`, `remember` (chunks)
- Exit code 78 (`EX_CONFIG`) para erros de configuração OpenRouter (chave API ausente, modelo ausente, chave inválida)
- 10 modelos verificados E2E com dim=64 MRL: `google/gemini-embedding-001` (0.892), `google/gemini-embedding-2` (0.868), `mistralai/mistral-embed-2312` (0.832), `qwen/qwen3-embedding-8b` (0.814), `qwen/qwen3-embedding-4b` (0.754), `openai/text-embedding-3-small` (0.668), `nvidia/llama-nemotron-embed-vl-1b-v2:free` (0.662), `baai/bge-m3` (0.537), `openai/text-embedding-3-large` (0.449), `perplexity/pplx-embed-v1-0.6b` (0.415)

## Novos Comandos e Flags (desde v1.0.95)
- `enrich --mode openrouter` — novo modo opt-in que roteia a etapa JUDGE ao REST `/chat/completions` do OpenRouter (sem CLI local); os quatro modos agora são `claude-code`, `codex`, `opencode`, `openrouter` (GAP-OR-ENRICH, ADR-0054)
- `--openrouter-model MODEL` — OBRIGATÓRIA com `--mode openrouter`; omiti-la sai com exit 1 antes de qualquer chamada de rede
- `--openrouter-api-key KEY` — chave de API do cliente de chat (XDG via `config add-key` (OPENROUTER_API_KEY is not read at runtime)); reutiliza a chave do backend de embedding com o mesmo tratamento `secrecy`/zeroize
- `--openrouter-timeout SECS` — timeout da requisição de chat (padrão 300s)
- `--openrouter-base-url URL` — override opcional da base URL do OpenRouter
- Novo módulo `src/chat_api.rs` (`OpenRouterChatClient`) espelha `src/embedding_api.rs`; SCAN→JUDGE→PERSIST inalterado, apenas o transporte do JUDGE muda; 13/13 modelos reais verificados; sem migração, schema v15

## Novos Comandos e Flags (desde v1.0.97)
- `enrich --requeue-dead` — move itens terminais `dead` de volta para `pending` para outra passada (sem reset in-place da fila); `enrich --list-dead` — listagem read-only JSON de cada item dead com seu `error_class` e `message`; `enrich --ignore-backoff` — desenfileira itens elegíveis de imediato, ignorando o cooldown `next_retry_at`; `enrich --prune-dead-orphans` — inspetor read-only (sem LLM, sem singleton) que deleta entradas `dead` do tipo memory da fila cujo `item_key` não existe mais no banco principal, deixando linhas de entidade intocadas (GAP-SG-66, ADR-0058)
- `enrich --status`, `--list-dead`, `--requeue-dead` e `--prune-dead-orphans` agora rodam SEM `--operation`/`--mode` (antes `--mode` era obrigatório) — ideal para integração com hooks/timers
- `enrich --operation augment-bindings` — adiciona vínculos a memórias que JÁ estão vinculadas; EXIGE `--names <a,b,c>` ou `--names-file <path>`. `enrich --operation body-extract --body-extract-graph-only` — extração de grafo read-only sem reescrever o corpo
- default de `--max-attempts` elevado para 8 (faixa 1..=20); default de `--openrouter-timeout` elevado para 600s
- `remember --graph-file <path>` — carrega o grafo de entidades de um arquivo (combinável com `--body-file`); `remember --strict-name` — rejeita nome não-kebab em vez de normalizar; `remember --replace-graph` (com `--force-merge`) zera os vínculos existentes antes de escrever
- `ingest --force-merge` — atualiza arquivos duplicados em vez de pular (dedup por `body_hash`); corpos grandes demais são divididos nativamente em chunks
- `read --format raw` — imprime o corpo puro sem envelope JSON; `unlink --memory <nome> --entity <nome>` — remove um único vínculo curado memória-entidade
- `embedding status --json` adiciona um objeto `coverage` (contagens reais de vetor por tabela); `stats --json` adiciona um `total_memories` no topo
- `--db <PATH>` deve vir DEPOIS do subcomando; não existe override independente de posição, então a alternativa canônica é a chave XDG `db.path` via `config set` (SG-32). O singleton por namespace do enrich permanece, com `--rest-concurrency` (clamp 1..=16, padrão 8) como remédio de vazão (GAP-20)

## Novos Comandos e Flags (desde v1.0.96)
- `enrich --until-empty` — loop interno scan→drain que roda até a fila não ter mais itens elegíveis ou `--max-runtime` expirar; substitui o loop de retry externo em bash (GAP-ENRICH-BACKLOG-CONVERGE, ADR-0055)
- `--max-runtime <SEGUNDOS>` — teto wall-clock para `--until-empty` (padrão 3600)
- `--max-attempts <N>` — orçamento de retries Transient antes de um item virar terminal `dead` (padrão 5, faixa 1..=20)
- `--status` — relatório read-only JSON das contagens da fila (`unbound_backlog`, `queue_pending/done/failed/dead/skipped`, `eligible_now`, `waiting`) mais o `scan_backlog` por operação (os candidatos reais do banco que um scan enfileiraria, compartilhando os predicados WHERE dos scanners de modo que nunca diverge de um scan real; o GAP-SG-77 elimina o falso `pending=0` para `entity-descriptions`/`body-enrich`/`re-embed`, e o `state` deriva `pending-scan` dele); NÃO chama o LLM e NÃO adquire o singleton — a saída determinística é ideal para integração com hooks/timers
- `--rest-concurrency <N>` — fan-out REST bounded para os lotes de embedding em `--mode openrouter`; clamp 1..=16 (padrão 8), distinta de `--llm-parallelism`
- Convergência por dead-letter: a fila `.enrich-queue.sqlite` ganha colunas `error_class` e `next_retry_at` (ALTER TABLE idempotente) mais o status terminal `dead`; falhas Transient (rate-limit/timeout/5xx) reagendam com backoff exponencial, HardFailures (validação/parse) vão a terminal imediatamente, e o dequeue exclui `dead` para o conjunto vivo decrescer estritamente

## Novos Comandos e Flags (desde v1.0.68)
### Ciclo de Vida de Processos (G28)
- `enrich` adquire um singleton por namespace antes de fazer trabalho real.  Uma segunda invocação concorrente no mesmo banco falha rápido com `AppError::JobSingletonLocked { job_type, namespace }` (exit 75) em vez de empilhar árvores de subprocessos.
- Histórico, removido na v1.2.0 — a env var `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` (opt-in) apontava para um diretório existente e vazio, e o subprocesso do Claude Code era iniciado com `CLAUDE_CONFIG_DIR=<esse dir>`, suprimindo servidores MCP do escopo user e a fan-out de 8-10 processos.  Aquele era o único mecanismo que o upstream do Claude Code realmente honrava (veja [anthropics/claude-code#10787]).  Deliberadamente NÃO passávamos `--strict-mcp-config` nem `--mcp-config '{}'` porque ambos são ignorados.  Os backends headless de subprocesso não existem mais, então o mecanismo acabou junto.
- `retry::CircuitBreaker` (API do crate Rust) — helper opt-in com `AttemptOutcome::{Success, Transient, HardFailure}`.  Erros rate-limited e timeout são explicitamente excluídos da contagem.  Use em loops de retry customizados para limitar iterações em falhas persistentes.
- `enrich` emite `tracing::warn!` (visível com `-v`) quando `--llm-parallelism > 4`; o remédio `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` que ele citava é histórico e foi removido na v1.2.0, então reduza `--llm-parallelism`.
### Build Windows (G29)
- `cargo install sqlite-graphrag` no Windows agora compila.  O tipo `HANDLE` é tratado de forma type-safe via `!handle.is_null() && handle != INVALID_HANDLE_VALUE`.  `windows-sys` está fixado em `=0.59.0` exato em `Cargo.toml`.  Novo job de CI `windows-build-check` roda `cargo check --target x86_64-pc-windows-msvc --lib --all-features` em todo push e PR.

## Novos Comandos e Flags (desde v1.0.69)
### Enforcement OAuth-Only (G28-A, G31, Mudança Comportamental)
- Os spawns de `claude -p` e `codex exec` agora ABORTAM com `AppError::Validation` se `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` estiverem presentes no ambiente.  OAuth (Claude Pro/Max ou ChatGPT Pro) é o ÚNICO fluxo de credencial aceito.  Veja `docs/decisions/adr-0011-oauth-only-enforcement.md` para a justificativa completa.
- A flag `--bare` (que exige uma chave de API e desabilita OAuth) foi REMOVIDA de todo caminho executável.  Ambas as variáveis de chave de API também são excluídas da whitelist de `env_clear` como defesa em profundidade.
### `enrich` — Novo Subcomando (G29 + G35 + G37)
- `enrich --operation <op> --mode openrouter --json` roda qualidade do grafo curada por LLM. Na introdução (v1.0.69) três ops chegaram primeiro: `memory-bindings`, `entity-descriptions`, `body-enrich` (G29 CHECK de `source` + auditoria `memory_versions`). **Conjunto FULLY-IMPLEMENTED atual** também inclui `re-embed`, `augment-bindings`, `body-extract`, `entity-connect` (v1.1.04 `entity_connect_seen` + **v1.1.06** O(k) coocorrência+hub×ilha, chaves `pair:{id1}:{id2}` / `entity_pair`, drain por PK, primeiro scan InterruptHandle → Timeout exit **1** ≠ 75, NDJSON `scan_start`/`scan_meta`) e `cross-domain-bridges` (mesmo path O(k)). Veja “Novos Comandos e Flags (desde v1.1.06)” acima.
- `--preserve-threshold <FLOAT>` (padrão 0.7) controla o portão de preservação trigrama Jaccard de `src/preservation.rs` (10 testes).  Scores abaixo do threshold são rejeitados e emitidos como `EnrichItemResult::PreservationFailed`.
- `--preflight-check` e `--rate-limit-buffer <SEGUNDOS>` (padrão 300) protegem um run longo: a sondagem de preflight confirma que a chave OpenRouter resolve antes de escanear N candidatos.
- `--names <a,b,c>` e `--names-file <CAMINHO>` selecionam um subconjunto específico de nomes de memória.  `--names-file` aceita comentários `#` e linhas em branco.  As duas flags se combinam como união.
- O aviso de `--llm-parallelism <N>` é condicional ao modo: Claude avisa em 5 (fan-out OAuth-MCP), Codex avisa em 17 (risco de rate limit), Codex 5..16 fica silencioso (validado em 1161 itens, 0 falhas em produção).
- `--max-load-check` recusa iniciar quando o load average > `2 × ncpus`.  `--circuit-breaker-threshold <N>` (padrão 5) aborta após N resultados `HardFailure` consecutivos.
### Família de Subcomandos `vec` (G39)
- `vec orphan-list --json` lista linhas de embedding de memória órfãs com `vector_hash` (BLAKE3 do blob de embedding).
- `vec purge-orphan --yes --dry-run --json` faz preview da deleção.  `vec purge-orphan --yes --json` purga as TRÊS vec tables (`vec_memories`, `vec_entities`, `vec_chunks`) em uma única transação.
- `vec stats --json` expõe `vec_memories_rows`, `vec_entities_rows`, `vec_chunks_rows`, `orphans` e o timestamp do último vacuum.
- `forget` agora chama `memories::delete_vec` ANTES do soft-delete, prevenindo novos órfãos em estado estável.

## Novos Comandos e Flags (desde v1.0.76)
### Arquitetura LLM-Only One-Shot (G21 + G22 + G23 + G24 + G25)
- O build padrão da v1.0.76 é LLM-Only e one-shot.  Sem daemon, sem runtime ONNX, sem download do modelo `multilingual-e5-small`.  A geração de embeddings e a NER delegam para um subprocesso headless `claude code` ou `codex` (OAuth, sem MCP, sem hooks).  O binário de release tem aproximadamente 6 MB.
- A feature `embedding-legacy` foi REMOVIDA na v1.0.79 (antecipando o cronograma da v1.1.0).  O pipeline legado fastembed + ort + tokenizers não existe mais; todo build é LLM-only.
- Veja ADR-0019, ADR-0020, ADR-0021, ADR-0022, ADR-0023, ADR-0024, ADR-0025, ADR-0026 para todas as decisões arquiteturais.
### Família de Subcomandos `migrate` (v1.0.76)
- `migrate --rehash --json` reescreve os checksums registrados de migração para casar com o conteúdo atual do arquivo.  Algoritmo casa com `refinery-core 0.9.1` (SipHasher13, mesma ordem de hashing).  Obrigatório para upgrades v1.0.74 → v1.0.76 onde V002 foi intencionalmente esvaziada para um no-op.  Schema de resposta: `migrate-rehash.schema.json`.
- `migrate --to-llm-only --drop-vec-tables --json` é o upgrade one-shot para bancos v1.0.74 / v1.0.75: rehash + descarte da V013 das vec tables + relatório de estado das vec tables.  A flag `--drop-vec-tables` é OBRIGATÓRIA como rede de segurança.  Schema de resposta: `migrate-to-llm-only.schema.json`.
### Tabelas de Embedding com Backing BLOB (G22)
- A migração V013 descarta as virtual tables `vec_memories`, `vec_entities` e `vec_chunks` e as substitui por tabelas regulares com backing BLOB `memory_embeddings`, `entity_embeddings` e `chunk_embeddings`.  A similaridade por cosseno é computada em Rust puro sob demanda em `src/similarity.rs` (ADR-0020, ADR-0022).
### Refinamento da Hybrid Search (G24)
- A `hybrid-search` usa FTS5 como filtro grosso e refina o conjunto de candidatos com cosseno em Rust puro sobre os embeddings BLOB.  O FTS5 permanece saudável porque o `optimize` pula a reconstrução quando o índice já está funcional, que é o PADRÃO desde o G36 da v1.0.69; `--no-fts-skip-when-functional` força a reconstrução.
### Seletor de Backend de Extração
- HISTÓRICO (era de subprocesso): a flag global --extraction-backend llm|embedding|none|both, padrão `llm`, selecionava o backend de extração — `llm` era o caminho por subprocesso, `embedding` um stub permanente que retornava erro de migração, `none` um no-op, `both` uma fusão em paralelo. A flag NÃO EXISTE MAIS: um binário 1.2.8 responde `unexpected argument` com exit 2, e o caminho de extração por subprocesso que ela selecionava foi removido na v1.2.0. Hoje a extração é escolhida por comando — `ingest --mode none` para ingestão só de corpo, depois um passo SEPARADO de `enrich --mode openrouter` para entidades e relações curadas por LLM.
- `src/extract/` continua expondo o trait `ExtractionBackend` com as quatro implementações, e nenhuma delas spawna processo. HISTÓRICO: `src/spawn/` expunha o trait `VersionAdapter` com `CodexAdapter` (detectava `codex 0.130.0` até `0.138+` e adaptava flags — `codex 0.137.0` removeu `--ask-for-approval` em favor de `-a never`), `ClaudeAdapter` (claude code 2.1.0+) e `OpencodeAdapter` (opencode headless). Esse diretório não existe na árvore 1.2.8: os três backends headless foram removidos na v1.2.0.
### Remoção do Daemon (ADR-0021)
- O subcomando `daemon` foi DEPRECIADO na v1.0.76 e TOTALMENTE REMOVIDO na v1.0.79 (antecipando o cronograma da v1.1.0).  O subprocesso LLM é o "model loader"; a CLI é 100% one-shot com zero IPC.

## Novos Comandos e Flags (v1.0.79 — pipeline de embedding G42)
- Flag global `--embedding-dim <N>` define a dimensionalidade do embedding (padrão **1024**, faixa [8, 4096]); precedência: flag > XDG `embedding.dim` > o `dim` gravado em `schema_meta` > 1024; bancos 384-dim existentes continuam via dim gravada
- `--llm-parallelism <N>` agora disponível em `remember` (padrão 4), `ingest` (padrão 2) e `edit` — fan-out limitado via `Semaphore` + `JoinSet`, permits com clamp [1, 32]
- `enrich --operation re-embed --limit N --resume` é o caminho canônico de re-embed one-shot (ex.: após mudar `--embedding-dim`)
- `edit --force-reembed` regenera o embedding de uma memória sem alterar o corpo
- Histórico, removido na v1.2.0 — `SQLITE_GRAPHRAG_CLAUDE_EMBED_MODEL` sobrescrevia o modelo de embedding do claude (simétrica à variável do codex); o prazo de embedding agora é `--openrouter-timeout <SEGUNDOS>` ou a chave XDG `embedding.timeout_secs` (padrão 300)
- Chamadas LLM são em lote (schema `{items:[{i,v}]}` — bases de calibração de 8 chunks / 25 nomes de entidade em dim 64, adaptativas por clamp(base×64/dim, 1, base) desde o G44) e todo subprocesso usa `kill_on_drop` mais timeout explícito

## Novos Comandos e Flags (desde v1.0.67)
- `remember-batch` cria memórias em lote via NDJSON no stdin em uma única invocação; `--transaction` para atomicidade, `--force-merge` para atualizações idempotentes, `--fail-fast` para parar no primeiro erro
- `completions` gera completions de shell para Bash, Zsh, Fish, PowerShell e Elvish
- `read --id <N>` busca memória por `memory_id` inteiro diretamente (sem resolução de nome)
- `read --with-graph` inclui entidades e relacionamentos vinculados na resposta JSON
- `enrich --llm-parallelism <N>` spawna N threads paralelas de LLM (padrão 1, máximo 32)
- `health` detecta entidades super-hub (grau > 50) e reporta `super_hub_count`, `top_hub_entity`, `top_hub_degree`
- `health` reporta `non_normalized_count` e `normalization_warning` para entidades fora do padrão kebab-case
- `edit` pula re-embedding quando conteúdo do body é inalterado (comparação body_hash)
- `rename` purga memórias ghost (soft-deleted) que ocupam o nome destino antes do UPDATE
- `hybrid-search` e `recall` rejeitam `--max-hops` e `--min-weight` quando travessia de grafo está desabilitada
- Migração V012 adiciona `created_at`/`updated_at` na tabela relationships

## Novos Comandos e Flags (desde v1.0.66)
- `edit --type` altera tipo de memória sem recriar
- `deep-research` campo `graph_context` na resposta JSON com entidades e relacionamentos das memórias encontradas
- `graph --format json` inclui alias `entities` junto com `nodes` para compatibilidade com agentes LLM
- `list --json` inclui alias `memories` junto com `items` para compatibilidade com agentes LLM
- `graph entities --json` inclui campo `description` por entidade
- `health --json` inclui contagens `vec_memories_missing` e `vec_memories_orphaned`

## Novos Comandos e Flags (desde v1.0.65)
- `reclassify-relation --from-relation <antigo> --to-relation <novo> --batch` renomeia tipos de relação em massa; modo individual via `--source`/`--target`; trata colisões UNIQUE via `UPDATE OR IGNORE` + `DELETE`; `--dry-run` faz preview; filtros opcionais `--filter-source-type`/`--filter-target-type`
- `normalize-entities --yes` normaliza todos os nomes de entidade para kebab-case ASCII minúsculo; mescla colisões automaticamente; `--dry-run` faz preview
- `enrich --operation <op> --mode claude-code|codex|opencode|openrouter` qualidade do grafo aumentada por LLM; ops FULLY-IMPLEMENTED atuais: `memory-bindings`, `entity-descriptions`, `body-enrich`, `re-embed`, `augment-bindings`, `body-extract`, `entity-connect` (v1.1.06 O(k) + pair keys), `cross-domain-bridges`; `--dry-run` faz preview sem LLM; `--max-cost-usd`, `--resume`, `--retry-failed`, `--until-empty`, `--max-runtime`
- `deep-research` novas flags: `--rrf-k` (padrão 60), `--graph-decay` (padrão 0.7), `--graph-min-score` (padrão 0.05), `--max-neighbors-per-hop`
- flag --max-entity-degree REMOVIDA de `link` e `remember` na v1.0.99 — a escrita agora é puramente aditiva e NUNCA poda, deleta arestas nem emite warn de grau (passar a flag agora resulta em clap exit 2)
- `health` reporta `top_relation`, `top_relation_ratio`, `applies_to_ratio`, `relation_concentration_warning` quando qualquer relação excede 40%
- Nomes de entidade normalizados para kebab-case em todo path de escrita (remember, ingest, link, rename-entity)

## Comportamento do Daemon (HISTÓRICO — daemon removido na v1.0.79)
- Apenas da v1.0.50 até a v1.0.78: a CLI reiniciava automaticamente o daemon em caso de incompatibilidade de versão.  Desde a v1.0.79 não existe processo daemon

## Novos Comandos e Flags (desde v1.0.56)
- `fts rebuild` reconstrói o índice FTS5 de busca textual do zero
- `fts check` executa integrity-check do FTS5 sem modificar o índice
- `fts stats` exibe estatísticas do índice FTS5 (contagem, páginas shadow, status funcional)
- `backup --output <caminho>` cria cópia segura do banco via SQLite Online Backup API
- `delete-entity --name <entidade> --cascade` remove entidade e cascateia para relacionamentos e bindings NER
- `reclassify --name <entidade> --entity-type <novo>` altera tipo; `--from-type <antigo> --to-type <novo> --batch` para massa
- `merge-entities --names "a,b,c" --into <destino>` funde entidades-fonte no destino, movendo todas as edges
- `rename-entity --name <antigo> --new-name <novo>` renomeia uma entidade do grafo preservando todos os relacionamentos baseados em FK e re-gera embedding para busca semântica
- `memory-entities --name <memória>` lista entidades vinculadas a uma memória específica
- `prune-ner --entity <nome>` ou `--all --yes` remove bindings NER da tabela memory_entities
- `cleanup-orphans --dry-run --json` audita entidades com zero memórias e zero relacionamentos; `--yes` remove
- `prune-relations --relation <tipo> --dry-run --json` visualiza remoção em massa de todos relacionamentos de um tipo; `--yes` executa
- `remember --dry-run` valida input e reporta ações planejadas sem persistir
- `remember --clear-body` limpa explicitamente o body durante `--force-merge` (body vazio agora preserva existente por padrão)
- `remember --type` e `--description` agora opcionais com `--force-merge` (herdados da memória existente)
- `list` limite padrão é todas as memórias com `--json`, 50 para texto; resposta inclui `total_count`, `truncated`, `body_length`
- `history --diff` inclui resumo de mudanças por caractere entre versões consecutivas
- `hybrid-search` degradação graciosa do FTS5: campos `fts_degraded`, `fts_error`, `fts_auto_rebuilt`; auto-rebuild em corrupção
- `hybrid-search` adiciona `normalized_score` (0-1), `vec_distance`, `fts_bm25` scores brutos
- `health` adiciona `fts_query_ok` (teste funcional FTS5 MATCH), `sqlite_version`
- `optimize --skip-fts` pula rebuild do FTS5; campo `fts_rebuilt` na resposta
- `link --strict-relations` rejeita tipos de relação não-canônicos; campo `warnings` na resposta
- `unlink --relation` agora opcional (remove todos entre o par); `--entity <nome> --all` para massa
- `graph entities --sort-by degree|name|created_at --order asc|desc`; campo `degree` na resposta
- `ingest --max-name-length N` configura truncagem; `body_length` no NDJSON; auto-prefixo `doc-` para nomes numéricos
- daemon --ping adicionava campos `model_name`, `model_variant` (HISTÓRICO — o daemon foi removido na v1.0.79)
- TODOS os caminhos de erro agora emitem JSON no stdout: `{"error": true, "code": N, "message": "..."}`
- Sync FTS5 corrigido em `edit`, `rename`, `restore` — memórias editadas agora imediatamente localizáveis via busca textual


## Tabela Resumo
### Catálogo — Toda Integração Suportada
| Nome | Tipo | Versão Mínima | Exemplo | Docs Oficiais |
| --- | --- | --- | --- | --- |
| Claude Code | Agente IA | 1.0+ | `sqlite-graphrag recall "query" --json` | https://docs.anthropic.com/claude-code |
| Codex CLI | Agente IA | 0.5+ | `sqlite-graphrag remember --name X --type user --body "..."` | https://github.com/openai/codex |
| Gemini CLI | Agente IA | recente | `sqlite-graphrag hybrid-search "query" --k 5 --json` | https://github.com/google-gemini/gemini-cli |
| Opencode | Agente IA | recente | `sqlite-graphrag recall "auth flow" --json` | https://github.com/opencode-ai/opencode |
| OpenClaw | Agente IA | recente | `sqlite-graphrag list --type user --json` | projeto comunitário |
| Paperclip | Agente IA | recente | `sqlite-graphrag read --name note --json` | projeto comunitário |
| VS Code Copilot | Agente IA | 1.90+ | tasks.json | https://code.visualstudio.com/docs/copilot |
| Google Antigravity | Agente IA | recente | `sqlite-graphrag hybrid-search "prompt" --json` | docs do Antigravity |
| Windsurf | Agente IA | recente | `sqlite-graphrag recall "plano refactor" --json` | https://windsurf.com/docs |
| Cursor | Agente IA | 0.40+ | `sqlite-graphrag remember --name cursor-ctx --type project --body "..."` | https://cursor.com/docs |
| Zed | Agente IA | recente | `sqlite-graphrag recall "abas abertas" --json` | https://zed.dev/docs |
| Aider | Agente IA | 0.60+ | `sqlite-graphrag recall "refactor" --k 5 --json` | https://aider.chat |
| Jules | Agente IA | preview | `sqlite-graphrag stats --json` | https://jules.google |
| Kilo Code | Agente IA | recente | `sqlite-graphrag recall "tarefas" --json` | projeto comunitário |
| Roo Code | Agente IA | recente | `sqlite-graphrag hybrid-search "contexto repo" --json` | projeto comunitário |
| Cline | Agente IA | extensão VS Code | `sqlite-graphrag list --limit 20 --json` | https://cline.bot |
| Continue | Agente IA | VS Code ou JetBrains | `sqlite-graphrag recall "docstring" --json` | https://docs.continue.dev |
| Factory | Agente IA | recente | `sqlite-graphrag recall "contexto pr" --json` | https://factory.ai |
| Augment Code | Agente IA | recente | `sqlite-graphrag hybrid-search "review" --json` | https://docs.augmentcode.com |
| JetBrains AI Assistant | Agente IA | 2024.2+ | `sqlite-graphrag recall "stacktrace" --json` | https://www.jetbrains.com/ai |
| OpenRouter | Roteador IA | qualquer | `sqlite-graphrag recall "regra" --json` | https://openrouter.ai/docs |
| Shells POSIX | Shell | qualquer | `sqlite-graphrag recall "$query" --json` | https://www.gnu.org/software/bash |
| Nushell | Shell | 0.90+ | `^sqlite-graphrag recall "query" --k 5 --json \| from json \| get results` | https://www.nushell.sh/book |
| Agendador local cron/systemd/launchd/Task Scheduler | Ops | qualquer | one-shot local | (sem CI cloud) |
| GitLab CI | CI/CD | qualquer | `.gitlab-ci.yml` | https://docs.gitlab.com/ee/ci |
| CircleCI | CI/CD | qualquer | `.circleci/config.yml` | https://circleci.com/docs |
| Jenkins | CI/CD | 2.400+ | Jenkinsfile | https://www.jenkins.io/doc |
| Docker e Podman Alpine | Container | qualquer | Dockerfile | https://docs.docker.com |
| Kubernetes | Orquestrador | 1.25+ | Job ou CronJob | https://kubernetes.io/docs |
| Scoop e Chocolatey | Gerenciador Pacote | Windows | `scoop install sqlite-graphrag` (planejado) | https://scoop.sh e https://chocolatey.org |
| Nix e Flakes | Gerenciador Pacote | qualquer | `nix run .#sqlite-graphrag` | https://nixos.org |


## Claude Code
### Agente Anthropic — Integração Subprocess
- Receita pronta para copiar em `.claude/hooks/`, zero custo, memória permanece na sua máquina
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é persistir contexto entre sessões do Claude Code sem serviços externos de memória
- Use `sqlite-graphrag recall "$USER_PROMPT" --k 5 --json` em um hook pre-task para injetar contexto
- Versão mínima exige Claude Code 1.0 ou posterior para suporte estável ao diretório `.claude/hooks/`
- Docs oficiais em https://docs.anthropic.com/claude-code descrevendo o ciclo de vida dos hooks
- Dica de ouro é capturar exit code `75` como retry-later mantendo o agente vivo graciosamente
- HISTÓRICO (v1.0.61 até a v1.2.0): `ingest --mode claude-code` usava o binário Claude Code para extração curada por LLM de entidades/relações durante ingestão em massa, spawnando `claude -p` headless por arquivo contra assinatura Pro/Max. Um binário 1.2.8 RECUSA — `invalid value 'claude-code' for '--mode <MODE>'`, exit 2 — porque `none` é o único valor aceito.
- Hoje o mesmo resultado sai em dois passos sem subprocesso: `ingest --mode none` para o corpo e depois um passo SEPARADO de `enrich --mode openrouter --openrouter-model <MODELO>` que alcança o provider por HTTP.
- HISTÓRICO: --claude-timeout <S> (padrão 300s) limitava aquele subprocesso e hoje é rejeitada com exit 2. Nada spawna, então nada trava: o único orçamento de tempo é a flag global `--openrouter-timeout <SEGUNDOS>`, aplicada por chamada REST à OpenRouter.


## Codex CLI
### Agente OpenAI — Subprocess Dirigido Por AGENTS.md
- Receita pronta para colar no `AGENTS.md` da raiz do repo, zero custo para ativar
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é expor o contrato de memória via convenção nativa do `AGENTS.md` da própria OpenAI
- Use `sqlite-graphrag recall "<query>" --k 5 --json` documentado dentro do `AGENTS.md` na raiz do repo
- Versão mínima exige Codex CLI 0.5 ou posterior para regras determinísticas de parsing do AGENTS.md
- Docs oficiais em https://github.com/openai/codex cobrindo a ordem de descoberta do AGENTS.md
- Dica de ouro é incluir um exemplo de invocação funcional sob cada comando listado para Codex
- HISTÓRICO (v1.0.62 até a v1.2.0): `ingest --mode codex` usava o binário Codex CLI para extração curada por LLM de entidades/relações durante ingestão em massa, spawnando `codex exec --json` headless por arquivo contra sessão ChatGPT OAuth. Um binário 1.2.8 RECUSA — `none` é o único valor aceito de `--mode`, exit 2 caso contrário.
- Hoje a receita é `ingest --mode none` seguido de um passo SEPARADO de `enrich --mode openrouter --openrouter-model <MODELO>` por HTTP; o contrato do `AGENTS.md` acima não muda, porque ele só documenta verbos de leitura.
- HISTÓRICO: --codex-timeout <S> (padrão 300s) limitava aquele subprocesso e hoje é rejeitada com exit 2. O único orçamento de tempo restante é a flag global `--openrouter-timeout <SEGUNDOS>`.

> **Autenticação (vigente):** não há subprocesso nem fluxo OAuth. Embedding e enriquecimento são chamadas HTTP à OpenRouter com chave guardada em repouso sob XDG por `config add-key --provider openrouter --from-stdin`; `config doctor` mostra qual camada resolveu.
> **Histórico:** até a v1.2.0 o OAuth era o único fluxo aceito e chaves de API eram recusadas — `--mode claude-code` lia `~/.claude/.credentials.json` (Claude Pro/Max/Team) e `--mode codex` lia a autenticação de dispositivo do `codex login` (OpenAI ChatGPT). Um binário 1.2.8 rejeita os dois modos.
> Definir `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` no ambiente ABORTA o spawn com `AppError::Validation` e código de saída 1. A flag `--bare` (que também exigiria uma chave de API) foi REMOVIDA de todo caminho executável.
> Veja `docs/decisions/adr-0011-oauth-only-enforcement.md` para a justificativa completa.

## Gemini CLI
### Agente Google — Subprocess Com Contrato JSON
- Receita pronta para copiar na config do Gemini CLI, zero custo, roda completamente local
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é injetar memória em prompts do Gemini 2.5 Pro durante sessões longas de código
- Use `sqlite-graphrag hybrid-search "query" --k 5 --json` para recall com intenção mista de keyword
- Versão mínima suporta qualquer release recente do Gemini CLI com invocação subprocess habilitada
- Docs oficiais em https://github.com/google-gemini/gemini-cli sobre padrões de integração de tool
- Dica de ouro é passar a flag global `--lang pt`, ou persistir `config set i18n.lang pt`, ao prompt-ar Gemini em contextos em português


## Opencode
### Agente Comunitário — Integração Subprocess
- Receita pronta para copiar no hook plugin do Opencode, zero custo, roda como subprocess
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é persistir contexto multi-turno no loop open source de orquestração do Opencode
- Use `sqlite-graphrag recall "$query" --json` como parte do pipeline pre-generation do Opencode
- Versão mínima suporta qualquer release recente do Opencode expondo hook subprocess via plugin
- Projeto oficial em https://github.com/opencode-ai/opencode com issue tracker comunitário
- Dica de ouro é definir o namespace pelo slug do repo para evitar vazamento entre projetos


## OpenClaw
### Agente Comunitário — Driver Subprocess
- Receita pronta para adicionar no startup do OpenClaw, zero custo, memória é totalmente local
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é injetar memória persistente em loops do agente OpenClaw sem rebuild de plugin
- Use `sqlite-graphrag list --type user --json` para buscar contexto inicial no começo de uma run
- Versão mínima suporta qualquer release recente do OpenClaw capaz de shell out para binários CLI
- Docs oficiais dentro do README GitHub do OpenClaw explicando regras de integração subprocess
- Dica de ouro é executar o binário dentro da pasta alvo e manter o default `graphrag.sqlite`


## Paperclip
### Agente Comunitário — Cliente Subprocess
- Receita pronta para colar na config de hook do Paperclip, zero custo, memória fica local
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é persistir memória cross-session no agente autônomo de desenvolvimento Paperclip
- Use `sqlite-graphrag read --name onboarding-note --json` para semear a sessão com notas prévias
- Versão mínima suporta qualquer release recente do Paperclip que possa spawnar subprocess filho
- Docs oficiais no repositório comunitário do Paperclip descrevendo o contrato de hook subprocess
- Dica de ouro é rodar `health --json` no startup e abortar quando integridade reporta dano algum


## VS Code Copilot
### Agente Microsoft — Integração tasks.json
- Receita pronta para colar no tasks.json, zero custo, recall dispara de dentro do editor
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é expor memória relevante de uma seleção dentro dos painéis de chat do VS Code Copilot
- Use a entrada de exemplo em tasks.json que chama `sqlite-graphrag recall "$selection" --json`
- Versão mínima exige VS Code 1.90 ou posterior para as substituições mais recentes de tasks.json
- Docs oficiais em https://code.visualstudio.com/docs/copilot cobrindo registro de tool no chat
- Dica de ouro é mapear a task em `Cmd+Shift+M` para invocação de recall com uma única tecla


## Google Antigravity
### Agente Google — Integração Runner
- Receita pronta para registrar como runner Antigravity, zero custo, binário é autocontido
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é rodar sqlite-graphrag como runner de primeira classe em pipelines Antigravity em escala
- Use `sqlite-graphrag hybrid-search "$PROMPT" --json --k 10` como passo de retrieval em um runner
- Versão mínima suporta qualquer release recente do Antigravity que aceite runners binários arbitrários
- Docs oficiais na página do produto Google Antigravity descrevendo formato de config de runner
- Dica de ouro é rodar `sync-safe-copy` antes de cada pipeline para proteger o artefato compartilhado


## Windsurf
### Agente Codeium — Integração Terminal
- Receita pronta para colar em um binding Run task do Windsurf, zero custo para ativar recall
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é expor recall de memória para painéis assistentes do Windsurf via invocação de terminal
- Use `sqlite-graphrag recall "$EDITOR_CONTEXT" --json` mapeado para um binding Run task no Windsurf
- Versão mínima suporta qualquer release recente do Windsurf com execução de task de terminal ativa
- Docs oficiais em https://windsurf.com/docs descrevendo a sintaxe de binding de task de terminal
- Dica de ouro é persistir resultados em `/tmp/ng.json` para templates de prompt Windsurf lerem


## Cursor
### Agente Cursor — Integração Terminal
- Receita pronta para adicionar em `.cursorrules` ou binding de terminal, zero custo, memória é local
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é parear Cursor AI com um backend de memória local que sobrevive restarts do editor
- Use `sqlite-graphrag remember --name cursor-ctx --type project --body "$SELECTION"` por atalho
- Versão mínima exige Cursor 0.40 ou posterior para regras AI estáveis e override de env de terminal
- Docs oficiais em https://cursor.com/docs cobrindo padrões de regras AI e integração de terminal
- Dica de ouro é passar `--namespace ${workspaceFolderBasename}` por workspace, ou persistir `config set namespace.default <NOME>`


## Zed
### Agente Zed Industries — Integração Assistant Panel
- Receita pronta para adicionar como task profile do Zed, zero custo, roda do terminal integrado
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é cablear recall de memória no painel assistente do Zed sem extensões customizadas
- Use `sqlite-graphrag recall "abas abertas" --json --k 5` como comando de terminal disponível ao Zed
- Versão mínima suporta qualquer release recente do Zed com painel assistente e tasks de terminal
- Docs oficiais em https://zed.dev/docs descrevendo painel assistente e integração de terminal
- Dica de ouro é definir um profile de task Zed compartilhando memória entre múltiplos workspaces


## Aider
### Agente Open Source — Integração Shell
- Receita pronta para colar no alias shell antes do `aider`, zero custo, zero servidor de config
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é aumentar pair programming do Aider com memória durável entre repositórios git
- Use `sqlite-graphrag recall "refactor target" --k 5 --json` invocado antes de cada prompt Aider
- Versão mínima exige Aider 0.60 ou posterior para invocação subprocess e hook estáveis e suportadas
- Docs oficiais em https://aider.chat descrevendo configuração e comandos shell customizados
- Dica de ouro é escopar memória por repositório via `--namespace $(basename $(pwd))` em cada invocação


## Jules
### Agente Google Labs — Automação CI
- Receita pronta para adicionar como passo CI do Jules, zero custo, binário instala em segundos
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é rodar manutenção de memória dentro dos pipelines de automação preview do Jules
- Use `sqlite-graphrag stats --json` como passo CI para monitorar crescimento de memória semanal
- Versão mínima é a release preview corrente do Jules disponível via early access do Google Labs
- Docs oficiais em https://jules.google explicando configuração de job CI e autenticação necessária
- Dica de ouro é falhar o pipeline quando `stats.memories` excede o limite combinado para um projeto


## Kilo Code
### Agente Comunitário — Integração Subprocess
- Receita pronta para colar no hook de startup do Kilo Code, zero custo, memória é arquivo local
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é expor camada de memória persistente ao agente autônomo de engenharia Kilo Code
- Use `sqlite-graphrag recall "tarefas recentes" --json` no começo de toda run do agente Kilo Code
- Versão mínima suporta qualquer release recente do Kilo Code capaz de spawnar processos filhos
- Docs oficiais no repositório comunitário do Kilo Code descrevendo o contrato de subprocess
- Dica de ouro é logar exit code `75` como retryable em vez de fatal quando orquestrador está ocupado


## Roo Code
### Agente Comunitário — Integração Subprocess
- Receita pronta para cablear no ciclo de hook do Roo Code, zero custo, dados em SQLite local
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é injetar memória em prompts do agente Roo Code para entendimento profundo do repo
- Use `sqlite-graphrag hybrid-search "contexto repo" --json` para recall entre tipos mistos de query
- Versão mínima suporta qualquer release recente do Roo Code com capacidade de hook subprocess
- Docs oficiais no repositório comunitário do Roo Code explicando convenções de ciclo de hook
- Dica de ouro é encadear `related <name> --hops 2` após recall para expansão multi-hop no grafo


## Cline
### Extensão Comunitária VS Code — Integração Terminal
- Receita pronta para registrar como tool de terminal do Cline, zero custo, memória persiste local
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é dar ao Cline memória persistente entre sessões VS Code sem serviços em cloud
- Use `sqlite-graphrag list --limit 20 --json` como passo inicial no startup da conversa do Cline
- Versão mínima suporta a release atual da extensão VS Code do Cline no marketplace
- Docs oficiais em https://cline.bot cobrindo registro de tool de terminal e padrões de uso
- Dica de ouro é mapear o comando como tool Cline com nome descritivo e explicação de uso


## Continue
### Agente Open Source — Integração Terminal IDE
- Receita pronta para colar nos custom commands do Continue, zero custo, sem servidor necessário
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é expor memória sqlite-graphrag nos painéis de chat Continue em VS Code ou JetBrains
- Use `sqlite-graphrag recall "docstring" --json` de um registro de custom command do Continue
- Versão mínima suporta qualquer release recente da extensão Continue em VS Code ou JetBrains
- Docs oficiais em https://docs.continue.dev descrevendo comandos customizados e integração de tool
- Dica de ouro é documentar cada comando no config do Continue para o LLM embutido detectar


## Factory
### Agente Factory — API Ou Subprocess
- Receita pronta para adicionar na config de tool do droid Factory, zero custo, binário autocontido
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é integrar sqlite-graphrag com droids autônomos de desenvolvimento Factory em produção
- Use `sqlite-graphrag recall "contexto pr" --json` durante preparação do plano do droid Factory
- Versão mínima suporta qualquer release recente do Factory com integração subprocess ou API
- Docs oficiais em https://factory.ai explicando configuração de tool do droid e execução do plano
- Dica de ouro é definir `--wait-lock` longo para droids Factory rodando sob concorrência pesada


## Augment Code
### Agente Augment — Integração IDE
- Receita pronta para cablear no registro de tool da IDE Augment, zero custo, roda como subprocess
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é alimentar agentes de review Augment Code com memória persistente entre repositórios
- Use `sqlite-graphrag hybrid-search "code review" --json` na preparação de review da IDE Augment
- Versão mínima suporta qualquer release recente do Augment Code com hooks de terminal e subprocess
- Docs oficiais em https://docs.augmentcode.com descrevendo registro de tool e agentes suportados
- Dica de ouro é ativar `--lang en` explicitamente para linguagem de review consistente entre times


## JetBrains AI Assistant
### Agente JetBrains — Integração IDE
- Receita pronta para registrar como external tool do JetBrains, zero custo, recall leva milissegundos
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é adicionar memória sqlite-graphrag ao JetBrains AI Assistant em IntelliJ PyCharm WebStorm
- Use `sqlite-graphrag recall "$SELECTION" --json` registrado como runner de external tool JetBrains
- Versão mínima exige JetBrains AI Assistant 2024.2 ou posterior para registro moderno de tool
- Docs oficiais em https://www.jetbrains.com/ai explicando registro de tool e external runner
- Dica de ouro é mapear o tool a um atalho de teclado para invocar recall com uma mão no teclado


## OpenRouter
### Roteador Multi-LLM — Qualquer Versão Suportada
- Receita pronta para adicionar como preâmbulo de qualquer pipeline OpenRouter, zero custo local
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é compartilhar backend comum de memória entre todo LLM hospedado via OpenRouter
- Use `sqlite-graphrag recall "regra roteamento" --json` como preâmbulo antes de request roteado
- Versão mínima suporta qualquer release da API OpenRouter já que memória fica local e independente
- Docs oficiais em https://openrouter.ai/docs explicando regras de roteamento e integração da API
- Dica de ouro é reusar o mesmo namespace entre todos os modelos roteados para contexto coeso


### Backend de Embedding OpenRouter (v1.0.94)
- Desde v1.0.94, sqlite-graphrag pode usar OpenRouter como backend dedicado de embedding via REST API
- Use `--embedding-backend openrouter --embedding-model MODEL` para embedding em ~200ms em vez de 15s via subprocesso
- 10 modelos verificados: Qwen 4B/8B, NVIDIA Nemotron (gratuito), OpenAI small/large, Perplexity, Mistral, BAAI, Google Gemini
- Defina a chave de API via `config add-key --provider openrouter` (OPENROUTER_API_KEY não é lida em runtime) ou flag `--openrouter-api-key`

```bash
printf "%s" "sk-or-v1-your-key-here" | sqlite-graphrag config add-key --provider openrouter --from-stdin
# OPENROUTER_API_KEY is not read at runtime (G-T-XDG-04)
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  remember --name teste --type note --description "teste" --body "conteúdo" --json
```

## Minimax (desde v1.0.83 — ADR-0041)
### Provider Anthropic-Compatível — MiniMax/api.minimax.io
- Receita pronta para rotear Claude Code através de qualquer endpoint Anthropic-compatível sem violar o mandato OAuth-only
- Embora a guarda OAuth-only continue rejeitando `ANTHROPIC_API_KEY` e `OPENAI_API_KEY` com exit 1 (defesa em profundidade desde v1.0.69), a nova whitelist preserva `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CODEX_ACCESS_TOKEN`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY` e `OTEL_EXPORTER_OTLP_ENDPOINT`
- Propósito é habilitar providers Anthropic-compatíveis (MiniMax/api.minimax.io, OpenRouter, rotas customizadas do AWS Bedrock, gateways corporativos) sem forçar operadores a pagar pela rota oficial de chave de API Anthropic
- RECEITA HISTÓRICA: as variáveis de ambiente abaixo eram exportadas antes de qualquer comando `sqlite-graphrag` que disparava embedding (`remember`, `edit`, `ingest --mode claude-code`). Nada disso vale para um binário 1.2.8: não há subprocesso para herdar env var, `ingest --mode claude-code` é rejeitado, e a seção de manutenção adiante declara que variáveis de ambiente de produto são ignoradas em runtime. O caminho vigente para provider customizado é `config add-key --provider openrouter --from-stdin` mais `--llm-backend openrouter` / `--embedding-backend openrouter`, que são flags de CLI com contrapartes XDG definidas por `config set`
- Versão mínima requer `sqlite-graphrag` 1.0.83 ou posterior; releases anteriores vão spawnar o subprocesso sem as vars do provider customizado e o provider retornará `401 Invalid authentication credentials`
- Documentação oficial em https://platform.minimax.io/document e `docs/decisions/adr-0041-preserve-custom-provider-env.md` explica a justificativa arquitetural
- Dica de ouro é verificar a alcançabilidade do provider com `curl -fsS "$ANTHROPIC_BASE_URL/v1/models" -H "Authorization: Bearer $ANTHROPIC_AUTH_TOKEN"` antes de rodar qualquer comando `sqlite-graphrag`

### Bloco de Configuração
```bash
# Configure uma vez por sessão de shell antes de invocar sqlite-graphrag
export ANTHROPIC_AUTH_TOKEN="sk-cp-seu-token-do-provider"
export ANTHROPIC_BASE_URL="https://api.minimax.io/anthropic"
# Opcional: opt-out de encaminhamento de telemetria do subprocesso
export DISABLE_TELEMETRY="1"
# Opcional: roteia OpenTelemetry para collector local em vez do padrão do provider
export OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4317"
```

### Smoke Test
```bash
# 1. Verifica que o provider retorna modelos para o token configurado
curl -fsS "$ANTHROPIC_BASE_URL/v1/models" \
  -H "Authorization: Bearer $ANTHROPIC_AUTH_TOKEN" \
  | head -c 200 && echo

# 2. Persiste uma memória de smoke test através do provider customizado
sqlite-graphrag remember \
  --name smoke-test-minimax-v183 \
  --type note \
  --description "validacao do provider customizado via v1.0.83" \
  --body "smoke test executado em $(date -u +%FT%TZ)" \
  --graph-stdin <<'EOF'
{
  "body": "smoke test executado em $(date -u +%FT%TZ)",
  "entities": [
    {"name": "minimax", "entity_type": "tool", "description": "Provider Anthropic-compatível"}
  ],
  "relationships": []
}
EOF

# 3. Confirma que o embedding aterrissou em memory_embeddings (não NULL)
sqlite-graphrag read --name smoke-test-minimax-v183 --json | jaq '{name, memory_id, has_embedding: (.body | length > 0)}'

# 4. Roda recall para verificar que o embedding participa da busca vetorial
sqlite-graphrag recall "validacao do provider customizado" --k 3 --json | jaq '.results[] | {name, score}'
```

### Troubleshooting 401 Invalid Authentication Credentials
- **Sintoma**: `sqlite-graphrag remember` retorna exit 11 com `claude exited with exit status: 1: stderr=` (ou equivalente `codex`)
- **Causa**: as env vars `ANTHROPIC_AUTH_TOKEN` ou `ANTHROPIC_BASE_URL` NÃO chegaram ao subprocesso (sqlite-graphrag antigo, modo estrito, ou wrapping de shell que remove env)
- **Caminhos de resolução**:
  - Confirme que `sqlite-graphrag --version` reporta `1.0.83` ou posterior
  - Confirme que as env vars estão exportadas no MESMO shell onde o comando roda (não shell pai, não `.envrc` consumido só pelo direnv)
  - Rode `env | rg "ANTHROPIC_(AUTH_TOKEN|BASE_URL)"` para confirmar presença
  - HISTÓRICO: --strict-env-clear era a chave de compliance da era de subprocesso e viveu na superfície global até a v1.2.2, como `src/cli/globals.rs` ainda registra; um binário 1.2.8 responde unexpected argument '--strict-env-clear' found com exit 2. Não há o que remover hoje: o processo não encaminha credencial a filho nenhum, porque não inicia nenhum
  - Capture o erro exato com `RUST_LOG=trace sqlite-graphrag remember ... 2> trace.log` e procure por `apply_env_whitelist`
- **Confirmação de defesa em profundidade**: a guarda OAuth-only ainda rejeita `ANTHROPIC_API_KEY` se acidentalmente setada; verifique com `export ANTHROPIC_API_KEY=sk-ant-test && sqlite-graphrag remember --name test --body x` retornando exit 1
## Shells POSIX
### Bash Zsh Fish PowerShell — Qualquer Versão
- Receita pronta para colar em alias ou script de shell, zero custo, pipes funcionam imediatamente
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess sem processo extra
- Propósito é compor sqlite-graphrag com pipelines clássicos Unix e Windows shell sem atrito
- Use `sqlite-graphrag recall "$query" --json | jaq '.hits[].name'` em qualquer shell POSIX
- Versão mínima suporta qualquer Bash Zsh Fish ou PowerShell 7 recente
- Docs oficiais em https://www.gnu.org/software/bash e homepages dos respectivos projetos shell
- Dica de ouro é colocar variáveis entre aspas para evitar word splitting em queries com espaços


## Nushell
### Nushell — Integração Pipeline de Dados Estruturados
- Receita pronta para colar em script Nushell, zero custo, saída vira tabela Nu nativa
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como subprocess via sigil `^` no Nu
- Propósito é compor saída do sqlite-graphrag com pipelines de dados estruturados do Nushell nativamente
- Use `^sqlite-graphrag recall "query" --k 5 --json | from json | get results` para consultar memória
- Versão mínima suporta Nushell 0.90 ou posterior para comando externo estável e pipeline `from json`
- Docs oficiais em https://www.nushell.sh/book descrevendo comandos externos e parsing de JSON
- Dica de ouro é encadear `| select name score` para exibir tabela de memória ranqueada no Nu


## Comandos de Manutenção e Grafo em Pipelines
### Superfície de Composição — jaq, NDJSON, One-Shots Headless
- Cada comando abaixo é um subprocesso one-shot com contrato JSON ou NDJSON no stdout, então compõe com `jaq`, redirecionamento e qualquer agendador documentado aqui
- Passe `--json` nos comandos de envelope único; `export` e `schema` já emitem NDJSON e dispensam a flag
- Coloque `--db <PATH>` DEPOIS do subcomando; variáveis de ambiente de produto `SQLITE_GRAPHRAG_*` não são lidas em runtime, então use flags CLI mais `sqlite-graphrag config set <KEY> <VALUE>`
- Pré-visualize todo comando mutador com `--dry-run` primeiro; `cleanup-orphans`, `prune-ner` e `prune-relations` exigem ainda `--yes` para efetivar
- `split-body` grava memórias filhas sem embeddings inline — encadeie uma passada SEPARADA de `enrich --operation re-embed --target memories` após o exit 0

| Comando | Emite no stdout | Receita de pipeline |
| --- | --- | --- |
| `backup` | `{action, source, destination, size_bytes, pages_copied, elapsed_ms}` | `sqlite-graphrag backup --output snap.sqlite --json \| jaq '{destination, size_bytes}'` |
| `export` | NDJSON, uma linha por memória mais uma linha final de sumário | `sqlite-graphrag export --type decision --namespace my-project > backup.ndjson` |
| `schema` | catálogo NDJSON de `{id, invoke}`; `--name <ID>` emite um JSON Schema | `sqlite-graphrag schema \| jaq -r .id` |
| `related` | `{name, max_hops, results, related_memories}` | `sqlite-graphrag related onboarding --max-hops 3 --json \| jaq -r '.results[].name'` |
| `memory-entities` | `{memory_name, entities}` com `description` por entidade | `sqlite-graphrag memory-entities --name my-memory --json \| jaq '.entities[] \| {name, description}'` |
| `namespace-detect` | `{namespace, source, cwd}` | `NS=$(sqlite-graphrag namespace-detect --json \| jaq -r .namespace)` |
| `cleanup-orphans` | `{orphan_count, deleted, dry_run, namespace}` | `sqlite-graphrag cleanup-orphans --dry-run --json \| jaq .orphan_count` |
| `delete-entity` | `{entity_name, relationships_removed, bindings_removed}` | `sqlite-graphrag delete-entity --name stale-tool --cascade --json \| jaq '{relationships_removed, bindings_removed}'` |
| `prune-ner` | `{entity, bindings_removed}` | `sqlite-graphrag prune-ner --all --dry-run --json \| jaq .bindings_removed` |
| `prune-relations` | `{relation, entities_affected, affected_entity_names}` | `sqlite-graphrag prune-relations --relation mentions --dry-run --json \| jaq .entities_affected` |
| `reclassify` | `{action, description_updated, namespace}` | `sqlite-graphrag reclassify --from-type concept --to-type tool --batch --json \| jaq .action` |
| `reclassify-relation` | `{from_relation, to_relation, merged_duplicates}` | `sqlite-graphrag reclassify-relation --literal-from applies-to --to-relation uses --batch --dry-run --json \| jaq '{from_relation, to_relation}'` |
| `split-body` | relatório de divisão por memória oversized, pré-visualizável com `--dry-run` | `sqlite-graphrag split-body --batch --threshold 50000 --dry-run --json` |


## Agendadores locais (sem GitHub Actions / CI cloud)
### Linux systemd user / cron — macOS launchd — Windows Task Scheduler
- O produto **proíbe** GitHub Actions / workflows CI no repositório (releases manuais).
- Use agendadores **locais** multiplataforma para manutenção one-shot:
  - Linux: timer do systemd em modo --user (a flag --user é do systemd, não do sqlite-graphrag) ou `cron` com `sqlite-graphrag purge --days 30 --yes` e `vacuum`
  - macOS: plist `launchd` invocando o binário instalado via cargo
  - Windows: Task Scheduler com o mesmo binário one-shot
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag instala via cargo e encerra após cada execução
- Dica de ouro: arquive a saída de `sync-safe-copy` no filesystem local para rollback


## Docker e Podman Alpine
### Container — Qualquer Versão Recente
- Receita pronta para copiar em Dockerfile, zero custo, imagem final cabe em menos de 25 MB Alpine
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag é binário estático sem dependência de runtime
- Propósito é empacotar sqlite-graphrag em imagens Alpine mínimas para deploys reproduzíveis em produção
- Use Dockerfile multi-stage com stage builder Rust e runtime Alpine copiando o binário único
- Versão mínima suporta qualquer Docker ou Podman com sintaxe multi-stage compatível ativada
- Docs oficiais em https://docs.docker.com cobrindo multi-stage build e minimização de imagem
- Dica de ouro é montar o arquivo SQLite como named volume para persistir memória entre restarts


## Kubernetes Jobs E CronJobs
### Kubernetes — 1.25+
- Receita pronta para copiar em manifesto CronJob, zero custo, roda no seu cluster existente
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como Job one-shot sem sidecar necessário
- Propósito é rodar manutenção sqlite-graphrag como Kubernetes CronJobs em clusters gerenciados
- Use manifesto CronJob referenciando a imagem Alpine e invocando purge mais vacuum agendados
- Versão mínima exige Kubernetes 1.25 ou posterior para Job CronJob e concurrency policy estáveis
- Docs oficiais em https://kubernetes.io/docs descrevendo Job CronJob e PersistentVolumeClaim
- Dica de ouro é montar o DB de um PVC com access mode `ReadWriteOnce` para segurança de dados


## Scoop E Chocolatey
### Gerenciador Pacote — Windows
- Receita pronta para executar assim que o manifesto entrar, zero custo, instala o mesmo binário do cargo
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag é único exe sem dependência de runtime
- Propósito é instalar sqlite-graphrag no Windows com Scoop ou Chocolatey familiares aos devs Windows
- Use `scoop install sqlite-graphrag` ou `choco install sqlite-graphrag` assim que manifestos oficiais saiam
- Versão mínima suporta Scoop 0.3 ou Chocolatey 2.0 com recursos modernos de manifesto ativos
- Docs oficiais em https://scoop.sh e https://chocolatey.org explicando convenções de manifesto
- Dica de ouro é executar o binário dentro da pasta do projeto para criar `graphrag.sqlite` ali


## Nix E Flakes
### Gerenciador Pacote — Qualquer Versão Nix
- Receita pronta para adicionar como flake input, zero custo, hash do binário fixado para reprodutibilidade
- Enquanto MCPs exigem servidor dedicado, sqlite-graphrag roda como binário puro em qualquer dev shell Nix
- Propósito é instalar sqlite-graphrag em ambientes Nix reproduzíveis incluindo NixOS e dev shells
- Use `nix run github:danilo-aguiar-br/sqlite-graphrag#sqlite-graphrag` para executar sem instalação prévia
- Versão mínima exige Nix 2.4 ou posterior com feature Flakes habilitada na config do usuário
- Docs oficiais em https://nixos.org descrevendo ativação de Flakes e uso via linha de comando
- Dica de ouro é fixar o hash de input do flake para o binário permanecer reproduzível em rebuilds
