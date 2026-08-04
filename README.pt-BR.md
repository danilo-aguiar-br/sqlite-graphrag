# sqlite-graphrag

[![Crates.io](https://img.shields.io/crates/v/sqlite-graphrag.svg)](https://crates.io/crates/sqlite-graphrag)
[![Docs.rs](https://docs.rs/sqlite-graphrag/badge.svg)](https://docs.rs/sqlite-graphrag)
[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](LICENSE)
[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)

> Memória persistente para agentes de IA em um único binário Rust com GraphRAG embutido.
> **Release atual: v1.2.5.** Contrato permanente, inalterado em toda a linha 1.2.x: schema **v16**, `DEFAULT_EMBEDDING_DIM=1024`, precedência de configuração **flag CLI > XDG `config set` > default** (env de produto `SQLITE_GRAPHRAG_*` **não** é lida no hot path), embedding e enrich **somente por OpenRouter REST**, releases manuais (sem GitHub Actions), owner no crates.io `danilo-aguiar-br`. O que cada release mudou está em [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md) — este banner não é uma segunda cópia dele.

- Leia este documento em [inglês (EN)](README.md).

- Versão em inglês disponível em [README.md](README.md)
- O pacote público e o repositório já estão disponíveis no GitHub e no crates.io
- Instale a última release publicada com `cargo install sqlite-graphrag --locked`
- Atualize uma instalação existente com `cargo install sqlite-graphrag --locked --force`
- Verifique o binário ativo com `sqlite-graphrag --version`
- Veja o histórico completo de releases em [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md)
- A validação de release inclui as suítes de contrato `slow-tests` documentadas em `docs/TESTING.pt-BR.md`
- Faça o build direto do checkout local com `cargo install --path .`
- **Atualizando para v1.2.0?** Nenhuma migração de banco; o schema permanece em **v16** — basta `cargo install sqlite-graphrag --locked --force` (ou `cargo install --path . --locked --force`). Crate `version = "1.2.0"`. **DEFAULT_EMBEDDING_DIM=1024** (bancos existentes mantêm `schema_meta.dim` até re-embed). Mapa legado XDG: `db.default_path` → `db.path`. Gate offline: `scripts/e2e_offline_v120.sh` (wrapper histórico `scripts/e2e_offline_v118.sh` supersedido por `e2e_offline_v120.sh`). Herda contrato XDG da v1.1.8: scrub de help; OpenRouter via XDG; fail-fast query Auto; EntityType fold; `remember-batch` description; `pending-embeddings status` + `cache stats`; `purge --now`; `config list --effective`. Residuais: monólitos >800 LOC; qualidade live LQ = operador. Consumidores de biblioteca fixam `=1.2.0`.
- **Atualizando para v1.2.2?** Nenhuma migração de banco — o schema principal permanece em **v16**. Basta `cargo install sqlite-graphrag --locked --force` (ou `cargo install --path . --locked --force`). Crate pin `=1.2.2`. **Somente aditivo:** as oito flags de saída agent-native (`--select`/`--fields`, `--filter`, `--max-items`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`) remodelam o envelope JSON de qualquer subcomando em um único ponto, então o agente deixa de pipar o payload inteiro no `jaq` para ler um campo; um envelope de falha (`error: true` / `ok: false`) **nunca** é filtrado e sempre chega ao chamador, documentos `$schema` passam intactos, streams NDJSON contornam a superfície, e a truncagem é registrada em `agent_surface` mais a flag `truncated` de topo. `--no-input` recusa stdin de forma declarativa: todo leitor de stdin falha de antemão com **exit 65** em vez de bloquear. Sem nenhuma flag definida, o envelope é idêntico byte a byte à saída da v1.2.1. Herda o selo CAPA da v1.2.1 e dim **1024** / XDG da v1.2.0. Notas: [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md) `[1.2.2]`.
- **Atualizando para v1.2.1?** Nenhuma migração de banco — schema principal permanece em **v16** (apenas comportamento da fila sidecar). Basta `cargo install sqlite-graphrag --locked --force` (ou `cargo install --path . --locked --force`). Crate pin `=1.2.1`. **Selo CAPA:** claim / contagem / `--resume` / `--retry-failed` exigem `operation` **e** `namespace` (um drain em `ai-sdd` não processa mais linhas `global` / ns vazio); `--until-empty` conta pendentes **só desta op+namespace**; `--force-redescribe` reabre `skipped`/`done` uma vez por processo via `reopen_force_redescribe_candidates` (nunca reabre `dead` — use `--requeue-dead`); elegibilidade de re-embed usa a verdade do BLOB `LENGTH(embedding) = dim*4` (linhas CORRUPT / META_AHEAD re-embedam de novo) e `reconcile_satisfied_reembed_pending` limpa zumbis quando o vetor vivo já bate a dim; enqueue faz strip de `entity:` na lookup (a chave da fila permanece `entity:…`) e valida que chunk keys existem em memória não-deletada do namespace alvo; CAPA-D usa só frases compostas de "configuration file" (sem FP do bare `%configuration file%`). Herda v1.2.0 dim **1024** / XDG / `--list-skipped`. Notas: [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md) `[1.2.1]`.
- **Atualizando de v1.0.74 / v1.0.75?** Veja [docs/MIGRATION.pt-BR.md](docs/MIGRATION.pt-BR.md) para o procedimento de migração da v1.0.76
- **Atualizando de v1.0.79 para v1.0.80?** Nenhuma migração de banco necessária; basta `cargo install sqlite-graphrag --locked --force`. A v1.0.80 adiciona o job de CI `semver-checks` (informativo), os steps de pre-warm do Windows (ADR-0033) e a saída sem panic no terceiro sinal (ADR-0034). Consumidores da biblioteca devem fixar em `=1.0.80`; veja a `Política de Estabilidade` abaixo. / v1.0.77 / v1.0.78 / v1.0.79
- **Atualizando de v1.0.80 / v1.0.81 para v1.0.82?** Duas novas migrations rodam automaticamente no primeiro `init`/`migrate`: `V014__pending_memories` (fila de checkpoint do `remember`) e `V015__pending_embeddings` (fila de retry de embedding). Após atualizar, rode `codex login` uma vez para refrescar o refresh token OAuth — o incidente de 2026-06-14 mostrou que `codex exec` retornando HTTP 401 `refresh_token_reused` agora é capturado pela nova cadeia de fallback (ADR-0040) e roteado para o próximo backend em `--llm-backend codex,claude`. Veja [docs/MIGRATION.pt-BR.md](docs/MIGRATION.pt-BR.md) para o procedimento completo em 6 passos incluindo rollback.
- **Atualizando de v1.0.91 / v1.0.92 para v1.0.94?** Nenhuma migração de banco necessária; basta `cargo install sqlite-graphrag --locked --force`. A v1.0.94 adiciona o backend de embedding OpenRouter (`--embedding-backend openrouter`), propaga `EmbeddingBackendChoice` para todos os 13 caminhos de embedding (GAP-OR-PROPAGATION), corrige exit code 78 para erros de configuração OpenRouter (BUG-OR-EXIT-CODE) e valida 10 modelos de embedding E2E. Consumidores da biblioteca devem fixar em `=1.0.94`.
- **Atualizando para v1.1.06?** Nenhuma migração de banco; schema permanece em v16 — `cargo install sqlite-graphrag --locked --force` (ou `cargo install --path .`). Fecha GAP-ENTITY-CONNECT-SCAN-CARTESIAN: candidatos por coocorrência + hub×ilha; chaves `pair:{id1}:{id2}` / `item_type=entity_pair`; drain por PK; `--max-runtime` / teto soft 120s no primeiro scan (`InterruptHandle`, Timeout exit 1); `scan_start` + `backlog_degree0_proxy` / `pairs_enqueued_this_scan`. Suite `tests/v1106_entity_connect_scan_regression.rs`. ADR-0066. Pin `=1.1.6`.
- **Atualizando para v1.1.05?** Nenhuma migração de banco necessária; o schema permanece em v16 (sem mudança desde a V016 da v1.1.04) — basta `cargo install sqlite-graphrag --locked --force`. Fecha os cinco bugs operacionais do relato de incidente deep-research "danilo" (`gaps.md`): (1) `deep-research` com token único gera sub-queries multi-aspecto (`source: "aspect"`, facetas EN/PT) em vez de uma única busca híbrida — estratégia manual via `--sub-query-strategy manual --sub-queries-file`; (2) envelopes JSON grandes: `deep-research --output PATH` grava via atomwrite e emite ack curto no stdout com checksum `blake3`; flag global `--quiet`/`-q` suprime tracing não-erro; nunca redirecione stdout+stderr no mesmo arquivo com `&>`; (3) `graph traverse --from <nome-curto>`: match exato prioritário; sem `--fuzzy`, NotFound (exit 4) inclui sugestões ranqueadas; com `--fuzzy`, vencedor claro é auto-resolvido com warning em stderr; (4) `merge-entities` rejeita merges auto-referenciais **antes** de qualquer trabalho no DB (defesa contra word-splitting do zsh sob `--cross-namespace`); (5) `link` ganha `--from-id`/`--to-id` e `validate_entity_name` rejeita nomes só de dígitos sob `--create-missing`. O manifesto do crate carrega `version = "1.1.5"`. Consumidores da biblioteca devem fixar em `=1.1.5`.
- **Atualizando para v1.1.04?** Migração de banco OBRIGATÓRIA — `migrate --json` aplica a V016 (tabela `entity_connect_seen`). Basta `cargo install sqlite-graphrag --locked --force`. Fecha os dois gaps estruturais rastreados em `gaps.md`: (1) GAP-001 — o `deep-research` não entra mais em panic com "Cannot start a runtime from within a runtime"; o entry point síncrono agora computa os embeddings por sub-query ANTES de construir seu runtime Tokio dedicado (`compute_sub_embeddings`), e os três caminhos de embedding OpenRouter em `embedder.rs` adotam o padrão canônico de reentrada `Handle::try_current` + `block_in_place`; o `ingest_opencode` também recebeu o guard. (2) GAP-002 — o `entity-connect` agora converge: a nova tabela `entity_connect_seen` (V016) registra o veredito do LLM por par, o scanner exclui pares já avaliados, o `count_operation_backlog` reporta um backlog real O(n), e o `--until-empty` atinge `eligible_remaining == 0`. A operação de enrich `entity-connect` é promovida de scan-only para fully-implemented. O manifesto do crate carrega `version = "1.1.4"`. Consumidores da biblioteca devem fixar em `=1.1.4`.
- **Atualizando para v1.1.03?** Nenhuma migração de banco necessária; o schema permanece em v15 (a fila sidecar do enrich ganha uma coluna `claimed_at` via ALTER idempotente) — basta `cargo install sqlite-graphrag --locked --force`. Fecha os seis bugs que bloqueavam operadores catalogados em `gaps.md` mais o portão V8 de corpo excessivamente grande. Correções de bugs: (1) o caminho de scan-enqueue do enrich agora insere candidatos em lote em uma única transação em vez de linha a linha sob o write lock do WAL; (2) `reclassify-relation` ganha `--literal-to <RELATION>` para que `--literal-from applies_to --literal-to applies_to --batch` migre as 61 357 arestas legadas com underscore para a forma canônica com hífen; (3) `merge-entities` ganha `--cross-namespace` (opt-in, default mesmo-namespace) para que `--ids`/`--into-id` resolvam através de todos os namespaces; (4) o sidecar do enrich ganha uma coluna `claimed_at` mais `enrich --reset-stale-claims` e `enrich --stale-claim-secs <N>`, com claims stale em `processing` resetadas no startup e um handler de SIGTERM fazendo cleanup graceful antes do exit 19; (5) apenas documentação — o texto de ajuda do `enrich --status` clarifica `scan_backlog` vs `queue_pending` vs cooldown vs deadlock; (6) o scanner de `re-embed --target chunks` troca para `LEFT JOIN memories` para que chunks de mães soft-deleted atinjam 100% de cobertura. Novo subcomando: `split-body` divide memórias cujo corpo excede 25 000 caracteres em memórias filhas e cria relações `replaces` (as filhas precisam de um `enrich --operation re-embed --target memories` depois). Novas flags: `--literal-to`, `--cross-namespace`, `--reset-stale-claims`, `--stale-claim-secs`. O nome oficial do release é v1.1.03; o manifesto do crate carrega `version = "1.1.3"` porque o parser SemVer rejeita zero à esquerda no componente patch. Consumidores da biblioteca devem fixar em `=1.1.3`.

- **Atualizando para v1.1.02?** Nenhuma migração de banco necessária; o schema permanece em v15 — basta `cargo install sqlite-graphrag --locked --force` (o manifesto do crate carrega `version = "1.1.2"` porque o parser SemVer rejeita zero à esquerda no componente patch). A v1.1.02 fecha os dois gaps residuais rastreados após v1.1.01 mais cobertura de regressão e uma nova flag de prune: o argumento depreciado `--gliner-variant` é removido de `remember` e `ingest` (clap o rejeita com exit 2, plumbing morto do GLiNER deletado, tests/gliner_variant_removed_regression.rs); o teto de tokens de embedding eleva a variante tipada `AppError::TooManyTokens { tokens, limit }` enforced na borda de escrita de `remember`/`remember-batch`/`edit` e dentro do cliente de embedding compartilhado (exit 6 preservado); `tests/reembed_entities_integration.rs` guarda a correção do dispatch de re-embed de entidades landingada em v1.1.01; e `enrich --prune-dead-entity-orphans` remove linhas dead-letter entity-keyed da fila sidecar (complementando o `--prune-dead-orphans` com escopo de memória). Quatro warnings pré-existentes do rustdoc também foram resolvidos. Consumidores da biblioteca devem fixar em `=1.1.2`.
- **Atualizando para v1.1.01?** Nenhuma migração de banco necessária; o schema permanece em v15 — basta `cargo install sqlite-graphrag --locked --force` (o manifesto do crate carrega `version = "1.1.2"` porque o SemVer rejeita zero à esquerda no componente patch). A v1.1.01 fecha o roteiro de 12 prioridades do `gaps.md`: vetores de entidade/chunk são escritos e preenchidos retroativamente pelo mesmo caminho REST OpenRouter das memórias, com guarda de vetor vazio nos upserts de vetor (P1); `enrich --operation re-embed --target memories|entities|chunks|all` faz backfill por tabela e também re-seleciona vetores com `dim` divergente ou blob vazio (P2/P10); `graph recompute-degree` reconcilia o `entities.degree` em cache com `--dry-run` e o envelope `{total, updated, zeroed, unchanged}` (P3); `reclassify-relation --literal-from` casa a relação armazenada verbatim para migrar arestas legadas com hífen (P4); `merge-entities --ids/--into-id` e `rename-entity --id` desambiguam por ID dentro de um namespace (P5); `health --json` e `embedding status --json` expõem cobertura de vetores por tabela (`vec_*_missing`, `vec_*_coverage_pct`) (P6); `EntityType` falha cedo com mensagem listando os 13 valores válidos (P7); os erros de limite exit 6 são as variantes tipadas `AppError::BodyTooLarge`/`AppError::TooManyChunks` carregando bytes/chunks e o limite no envelope (P11); e `ingest --name-prefix` prefixa cada nome de memória derivado (P12). Consumidores da biblioteca devem fixar em `=1.1.2`.
- **Atualizando para v1.1.0?** Nenhuma migração de banco necessária; o schema permanece em v15 (o sidecar do enrich `.enrich-queue.sqlite` ganha colunas de diagnóstico via ALTER idempotente) — basta `cargo install sqlite-graphrag --locked --force`. A v1.1.0 resolve o backlog dead-letter do enrichment na raiz: completions truncadas do OpenRouter são detectadas (`finish_reason=length`) e retentadas com `max_tokens` crescido (GAP-SG-70/71), linhas dead-letter carregam `finish_reason`/`input_tokens`/`output_tokens` (GAP-SG-72, via `--list-dead --json`), a classificação de retry é totalmente tipada sem substring de mensagem (GAP-SG-73), o módulo compartilhado `openrouter_http` deduplica os clientes de chat/embedding (GAP-SG-74), o User-Agent HTTP é `sqlite-graphrag/1.1.0` (GAP-SG-75), o dequeue é limitado sob contenção de lock (exit 15 em `SQLITE_BUSY` sustentado, GAP-SG-76), `enrich --status` reporta um `scan_backlog` real por operação que nunca diverge de um scan real (GAP-SG-77), e uma entidade ainda não materializada é retentada como `Transient` em vez de dead-letter no primeiro miss (GAP-SG-78). Consumidores da biblioteca devem fixar em `=1.1.2`.
- **Atualizando para v1.0.99?** Nenhuma migração de banco necessária; o schema permanece em v15 — basta `cargo install sqlite-graphrag --locked --force`. A v1.0.99 remove a flag `--max-entity-degree` de `remember`/`link` (BREAKING — passá-la agora dá clap exit 2; a mitigação obsoleta `--max-entity-degree 0` é desnecessária pois a escrita nunca poda arestas); sem migração de schema. A v1.0.97 fortalece a fila dead-letter do enrich com flags de recuperação e inspeção (`--requeue-dead` move itens terminais `dead` de volta para `pending`, `--list-dead` os lista com `error_class`/`message`, `--ignore-backoff` ignora o cooldown `next_retry_at`, `--prune-dead-orphans` remove linhas dead-letter órfãs cuja memória foi renomeada ou purgada após o enfileiramento), permite que `--status`/`--list-dead`/`--requeue-dead`/`--prune-dead-orphans` rodem sem `--operation`/`--mode`, adiciona a operação `augment-bindings` (exige `--names`) e `body-extract --body-extract-graph-only`, eleva o default de `--max-attempts` para 8 e o default de `--openrouter-timeout` para 600s. O `remember` ganha `--graph-file` (combinável com `--body-file`), `--strict-name` e `--replace-graph`; o `ingest` ganha `--force-merge` com dedup por `body_hash` e auto-split nativo de corpos grandes; o `read` ganha `--format raw`; o `unlink` ganha `--memory <nome> --entity <nome>` para vínculos curados. O `embedding status` adiciona um objeto `coverage` e o `stats --json` um `total_memories` no topo. O `--db` vem DEPOIS do subcomando. **Nota histórica:** `SQLITE_GRAPHRAG_DB_PATH` era o override independente de posição (SG-32) naquela era; a partir da v1.2.0 product env **não** é lida em runtime — use `--db` ou `config set db.path`. Consumidores da biblioteca devem fixar em `=1.0.99`.
- **Atualizando de v1.0.94 para v1.0.95?** Nenhuma migração de banco necessária; o schema permanece em v15 — basta `cargo install sqlite-graphrag --locked --force`. A v1.0.95 adiciona `enrich --mode openrouter`, roteando o JUDGE de extração pelo endpoint REST `/chat/completions` do OpenRouter para que a extração estruturada (memory-bindings, entity-descriptions, body-enrich, etc.) não exija mais uma CLI local claude/codex/opencode. Novas flags: `--openrouter-model` (obrigatória com `--mode openrouter`; sem default — sua ausência sai com exit 1 antes de qualquer chamada de rede), `--openrouter-api-key` (XDG via `config add-key` (OPENROUTER_API_KEY is not read at runtime)), `--openrouter-timeout` (padrão 300s) e `--openrouter-base-url`. O pipeline SCAN→JUDGE→PERSIST permanece inalterado; só o transporte do JUDGE muda (ADR-0054). Consumidores da biblioteca devem fixar em `=1.0.95`.
- **Atualizando de v1.0.85 / v1.0.86 / v1.0.87 / v1.0.88 / v1.0.89 / v1.0.90 para v1.0.91?** Nenhuma migração de banco necessária; basta `cargo install sqlite-graphrag --locked --force`. A v1.0.91 corrige GAP-SPAWN-001 (subprocessos LLM não herdam mais `.mcp.json` — embedding funciona zero-config em qualquer projeto), BUG-17 (inflação de `entities.degree` substituída por `recalculate_degree`), BUG-15 (7 enums de schema), BUG-16 (schema `deep-research`), GAP-SPAWN-002 (cleanup de diretórios órfãos) e BUG-14 (correção de teste). Consumidores da biblioteca devem fixar em `=1.0.91`.
- **Atualizando de v1.0.82 / v1.0.83 para v1.0.85?** Nenhuma migração de banco necessária; basta `cargo install sqlite-graphrag --locked --force`. A v1.0.84 (ADR-0042, GAP-002) adicionou o split real do backend Claude via `LlmEmbeddingBuilder` para que `--llm-backend claude` invoque `claude` e nunca `codex`, o campo `backend_invoked` em 7 envelopes JSON, o campo `vec_degraded_reason` em `hybrid-search` e `recall`, a flag global `--dry-run-backend` para auditoria pré-voo em CI, e `apply_env_whitelist_for_claude` para providers hardened. A v1.0.85 (ADR-0043) estendeu `FallbackReason` de 3 para 7 variantes com discriminador `reason_code` (captura exaustão de quota, exaustão de slot, mismatch de backend, dim zero, cancelamento, timeout), `try_embed_query_with_deterministic_fallback` re-tenta o backend alternativo em `OAuthQuota` e dorme 750ms em `SlotExhausted`, e `LlmEmbedding::invoke_claude` agora captura 12-14 headers `anthropic-ratelimit-*-remaining` ANTES de checar o exit do subprocesso (G45-CR5). Consumidores da biblioteca devem fixar em `=1.0.85`; veja a `Política de Estabilidade` abaixo.

```bash
cargo install sqlite-graphrag --locked --force
sqlite-graphrag --version
```


## O que é?
### sqlite-graphrag entrega memória durável para agentes de IA
- Armazena memórias, entidades e relacionamentos em um único arquivo SQLite abaixo de 25 MB
- **Build:** LLM-only e one-shot — os embeddings são gerados pela API REST do OpenRouter (`--embedding-backend openrouter`); sem modelo local, sem daemon, sem runtime ONNX, binário de ~19 MiB. O `enrich --mode openrouter` roda o JUDGE de extração pelo mesmo transporte REST (ADR-0054)
- **Build legado:** REMOVIDO na v1.0.79 — a feature `embedding-legacy` e o caminho local fastembed/ONNX não existem mais
- Combina busca full-text FTS5 com similaridade de cosseno em Rust puro em um ranqueador híbrido de Reciprocal Rank Fusion
- Armazena e atravessa um grafo explícito de entidades com arestas tipadas para recall multi-hop entre memórias
- Preserva cada edição através de uma tabela imutável de histórico de versões para auditoria completa
- Roda em Linux, macOS e Windows nativamente sem serviços externos (precisa apenas de uma chave de API OpenRouter)


## Por que sqlite-graphrag?
### Diferenciais contra stacks RAG em nuvem
- **Fluxo LLM OAuth-only** — sem chaves de API no ambiente; o spawn ABORTA se `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` estiverem definidas (defesa em profundidade desde v1.0.69)
- Armazenamento em arquivo SQLite único substitui clusters Docker de bancos vetoriais
- Recuperação com grafo supera RAG vetorial puro em perguntas multi-hop por design
- Saída JSON determinística habilita orquestração limpa por agentes de IA em pipelines
- Binário cross-platform nativo dispensa dependências Python, Node ou Docker


## Política de Estabilidade (G53, v1.0.80)

- O **contrato público é a CLI**. Os envelopes `--json` documentados em `docs/schemas/*.schema.json` e as variáveis de ambiente listadas em `llms.txt` e `llms-full.txt` permanecem estáveis em todas as versões v1.x.y. Consumidores que dependem apenas da CLI não são afetados por bumps minor ou patch.
- A **API da biblioteca é instável** em v1.x.y. Re-exports, campos públicos de struct e assinaturas de função podem mudar em qualquer release v1.x.y sem bump de major.
- Mudanças quebrantes na API da biblioteca saem como bump **minor**, nunca patch (ex.: 1.0.79 -> 1.1.0 para re-export removido). Bumps de patch (1.0.79 -> 1.0.80) são limitados a mudanças aditivas sem quebra.
- Consumidores que dependem da API da biblioteca devem fixar versão exata (`sqlite-graphrag = "=1.0.80"`) e revisar CHANGELOG.md antes de bumpar.
- Esta postura está registrada em `docs/decisions/adr-0032-g53-lib-api-policy.md`.

## Superpoderes para Agentes de IA
### Contrato de CLI de primeira classe para orquestração
- Todo subcomando aceita `--json` produzindo payloads determinísticos em stdout
- **One-shot por padrão** — sem processo em segundo plano; cada chamada de embedding é uma única requisição REST
- Toda escrita é idempotente via restrições de unicidade em `--name` kebab-case
- Stdin é explícito: use `--body-stdin` para texto ou `--graph-stdin` para um objeto `{body?, entities, relationships}`; arrays crus de entidades e relacionamentos usam `--entities-file` e `--relationships-file`
- `remember` aceita payloads de body até `512000` bytes e até `512` chunks
- Payloads de relacionamento usam `strength` em `[0.0, 1.0]`, mapeado para `weight` nas saídas
- Stderr carrega saída de tracing apenas sob `SQLITE_GRAPHRAG_LOG_LEVEL=debug`
- `--help` é inglês por padrão; use `--lang` para mensagens humanas de runtime, não para o help estático do clap
- Comportamento cross-platform é idêntico em hosts Linux, macOS e Windows


## Schema do Grafo
### Tipos de entidade, rótulos de relação e peso de aresta
- `entity_type` aceita exatamente 13 valores: `project`, `tool`, `person`, `file`, `concept`, `incident`, `decision`, `memory`, `dashboard`, `issue_tracker`, `organization`, `location`, `date`
- `relation` (entrada CLI) aceita qualquer string em kebab-case ou snake_case. 12 valores canônicos são bem conhecidos: `applies-to`, `uses`, `depends-on`, `causes`, `fixes`, `contradicts`, `supports`, `follows`, `related`, `mentions`, `replaces`, `tracked-in`. Valores customizados (ex.: `implements`, `tested-by`, `blocks`) são aceitos com um `tracing::warn!`. A saída JSON normaliza para underscores (ex.: `applies_to`).
- `strength` é um float em `[0.0, 1.0]` representando o peso da aresta; mapeado para `weight` em todos os outputs de leitura
- Valores de `entity_type` não listados são rejeitados na escrita com código de saída 1. Valores customizados de `relation` são aceitos desde v1.0.49.
- Use `sqlite-graphrag graph --format json` para inspecionar o grafo completo armazenado a qualquer momento


### 27 agentes de IA e IDEs suportados de imediato
| Agente | Fornecedor | Versão mínima | Padrão de integração |
| --- | --- | --- | --- |
| Claude Code | Anthropic | 1.0 | Subprocesso com stdout `--json` |
| Codex | OpenAI | 1.0 | Tool call envolvendo `cargo run -- recall` |
| Gemini CLI | Google | 1.0 | Function call retornando JSON |
| Opencode | Opencode | 1.0 | Shell tool com `hybrid-search --json` |
| OpenClaw | Comunidade | 0.1 | Subprocesso via pipe para filtros `jaq` |
| Paperclip | Comunidade | 0.1 | Invocação direta da CLI por mensagem |
| VS Code Copilot | Microsoft | 1.85 | Subprocesso de terminal via tasks |
| Google Antigravity | Google | 1.0 | Agent tool com JSON estruturado |
| Windsurf | Codeium | 1.0 | Registro de comando customizado |
| Cursor | Anysphere | 0.42 | Integração terminal ou wrapper MCP |
| Zed | Zed Industries | 0.160 | Extensão envolvendo subprocesso |
| Aider | Paul Gauthier | 0.60 | Hook de shell por turno |
| Jules | Google Labs | 1.0 | Integração de shell no workspace |
| Kilo Code | Comunidade | 1.0 | Invocação via subprocesso |
| Roo Code | Comunidade | 1.0 | Comando customizado via CLI |
| Cline | Saoud Rizwan | 3.0 | Ferramenta de terminal registrada manualmente |
| Continue | Continue Dev | 0.9 | Provedor de contexto via shell |
| Factory | Factory AI | 1.0 | Tool call com resposta JSON |
| Augment Code | Augment | 1.0 | Envolvimento de comando de terminal |
| JetBrains AI Assistant | JetBrains | 2024.3 | External tool por IDE |
| OpenRouter | OpenRouter | 1.0 | Roteamento de função via shell |
| Minimax | Minimax | 1.0 | Invocação via subprocesso |
| Z.ai | Z.ai | 1.0 | Invocação via subprocesso |
| Ollama | Ollama | 0.1 | Invocação via subprocesso |
| Hermes Agent | Comunidade | 1.0 | Invocação via subprocesso |
| LangChain | LangChain | 0.3 | Subprocesso via tool |
| LangGraph | LangChain | 0.2 | Subprocesso via nó |


## Início Rápido
### Instale e grave sua primeira memória em quatro comandos
```bash
cargo install sqlite-graphrag --locked --force
sqlite-graphrag init
sqlite-graphrag remember --name primeira-memoria --type user --description "primeira memória" --body "olá graphrag"
sqlite-graphrag recall "graphrag" --k 5 --json
```
> **Flags obrigatórias para `remember`:** `--name`, `--type`, `--description`. Body via `--body "texto"`, `--body-file <caminho>`, ou `--body-stdin` (pipe do stdin).
> **Limite do body: 500 KB (512000 bytes).** Entradas maiores são rejeitadas com código de saída 6 (`limit exceeded`); divida em múltiplas memórias ou reduza antes de enviar.
> **Usuários Windows (G29):** v1.0.68 é o primeiro release desde v1.0.65 que compila com sucesso via `cargo install` no Windows. Se você precisa ficar em v1.0.66 ou v1.0.67, veja [docs/CROSS_PLATFORM.pt-BR.md](./docs/CROSS_PLATFORM.pt-BR.md) para a solução manual.
- **GraphRAG está habilitado por padrão e roda automaticamente.** Cada subcomando auto-inicializa `graphrag.sqlite` no diretório de trabalho atual se ele não existir. A extração de entidades/relacionamentos vem do backend LLM (`--extraction-backend llm`, o padrão) ou de grafo curado (`--graph-stdin`, `--entities-file`).

### Extração automática (`--enable-ner`)
- Passe `--enable-ner` ou defina `SQLITE_GRAPHRAG_ENABLE_NER=1` para ativar extração automática em `remember` e `ingest`
- Desde a v1.0.79 isso executa APENAS extração de URL por regex — o pipeline local GLiNER zero-shot foi removido junto com a feature `ner-legacy`
- `--gliner-variant` foi REMOVIDO em v1.1.02 (clap o rejeita com exit 2, seguindo o precedente do `--max-entity-degree` da v1.0.99); as env vars `SQLITE_GRAPHRAG_GLINER_MODEL` e `SQLITE_GRAPHRAG_GLINER_THRESHOLD` foram deletadas do código em v1.1.02 e são silenciosamente ignoradas se definidas
- Campo `extraction_method` na resposta reporta `url-regex`, `regex-only` ou `none:extraction-failed`
- Para extração de alta qualidade passe entidades curadas via `--graph-stdin`, ou rode um passo SEPARADO de `enrich`
- `--skip-extraction` está obsoleto desde v1.0.45 e não tem efeito

- **`sqlite-graphrag init` é OPCIONAL** mas recomendado no primeiro uso porque cria o banco e aplica migrações (não há download de modelo — os embeddings vêm da API REST do OpenRouter)
- **`graphrag.sqlite` é criado no diretório de trabalho atual por padrão** (sobrescreva com `--db <caminho>` após o subcomando, ou persista via `config set db.path <caminho>`; product env `SQLITE_GRAPHRAG_DB_PATH` **não** é lida em runtime na v1.2.0)
- Para o checkout local, `cargo install --path .` é suficiente
- Reexecute `sqlite-graphrag --version` após qualquer upgrade para confirmar o binário ativo
- Depois da release pública, prefira `--locked` para preservar o grafo de dependências validado para o MSRV


## Destaques da Versão

O histórico por versão fica em [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md), a fonte única de verdade sobre o que cada versão mudou.

## Ciclo de Vida da Memória
### Sequência executável: init → remember → recall → forget → purge
```bash
# 1. Inicializar (uma vez por banco)
sqlite-graphrag init

# 2. Armazenar uma memória
sqlite-graphrag remember --name minha-nota --type user --description "demo" --body "primeira entrada"

# 3. Recuperar por similaridade semântica
sqlite-graphrag recall "primeira entrada" --k 5 --json

# 4. Exclusão suave (reversível)
sqlite-graphrag forget minha-nota

# 5. Remover permanentemente memórias soft-deleted com 0 dias de retenção
sqlite-graphrag purge --retention-days 0 --yes
```
> Todos os cinco comandos acima são seguros para executar em sequência em um banco recém-criado.


## Instalação
### Múltiplos canais de distribuição
- Instale a última release publicada com `cargo install sqlite-graphrag --locked`
- Atualize um binário publicado existente com `cargo install sqlite-graphrag --locked --force`
- Para fixar uma versão específica use `cargo install sqlite-graphrag --version <X.Y.Z> --locked`
- Instale a partir do checkout local com `cargo install --path .`
- Compile a partir do checkout local com `cargo build --release`


## Uso
### Inicialize o banco de dados
```bash
sqlite-graphrag init
sqlite-graphrag init --namespace projeto-foo
```
- Sem `--db` (ou um `db.path` persistido via `config set`), todo comando CRUD nessa pasta usa `./graphrag.sqlite`. Product env `SQLITE_GRAPHRAG_DB_PATH` **não** é lida em runtime (v1.2.0)
### Grave uma memória com grafo de entidades explícito opcional
- Por padrão, `remember` NÃO executa extração automática de URLs (desligada por padrão)
- Passe `--enable-ner` para ativar a extração de URL por regex nessa chamada (o pipeline GLiNER foi removido na v1.0.79). Product env não é lida em runtime na v1.2.0
```bash
sqlite-graphrag remember \
  --name testes-integracao-postgres \
  --type feedback \
  --description "prefira Postgres real a mocks SQLite" \
  --body "Testes de integração devem usar banco real."
```
- A resposta JSON de `remember` inclui `urls_persisted` (URLs roteadas para a tabela `memory_urls`) e `relationships_truncated` (bool, ativo quando relacionamentos foram truncados)
- URLs são armazenadas em `memory_urls` via schema V007 e nunca poluem o grafo de entidades
- Exemplo de saída JSON ilustrando entidades e relacionamentos extraídos (chaves em inglês por convenção):
```json
{
  "memory": {"id": 42, "name": "audit-note", "type": "project"},
  "extracted_entities": [
    {"name": "OpenAI", "kind": "organization", "saliency": 0.92},
    {"name": "Rust", "kind": "technology", "saliency": 0.85}
  ],
  "extracted_relationships": [
    {"source": "OpenAI", "target": "GPT-4", "relation": "develops"}
  ],
  "urls_persisted": [],
  "relationships_truncated": false
}
```
### Status da extração automática (GLiNER removido na v1.0.79)
- O pipeline local GLiNER zero-shot NER foi REMOVIDO na v1.0.79 com a feature `ner-legacy`; `--enable-ner` agora executa apenas extração de URL por regex
- Para extração de entidades/relacionamentos curada por LLM rode um passo SEPARADO de `enrich --mode openrouter` após `ingest --mode none`
- Para controle exato passe entidades curadas via `--graph-stdin`, `--entities-file` e `--relationships-file`
- O campo `extraction_method` na resposta JSON reporta qual caminho executou

```bash
sqlite-graphrag remember \
  --name notas-de-release-v1 \
  --type document \
  --description "notas de release para v1.0.0" \
  --enable-ner \
  --llm-parallelism 4 \
  --body-stdin < notas.md
```
### Backend de Embedding OpenRouter (v1.0.94)
- Use `--embedding-backend openrouter` com `--embedding-model` para embeddings rápidos via API REST (~200ms por chamada vs 15s subprocess)
- O usuário DEVE especificar `--embedding-model` — nenhum modelo padrão é hardcoded
- Defina `OPENROUTER_API_KEY` via `config add-key --provider openrouter` or `--openrouter-api-key` (OPENROUTER_API_KEY is not read at runtime)
```bash
# Remember com embedding OpenRouter
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  remember --name minha-nota --type note \
  --description "embedding rápido" --body "conteúdo aqui"

# Ingest com OpenRouter + auto-enrich
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "google/gemini-embedding-001" \
  ingest ./docs --pattern "*.md" --recursive --enrich-after --json

# Recall com embedding de query OpenRouter
sqlite-graphrag --embedding-backend openrouter \
  --embedding-model "qwen/qwen3-embedding-8b" \
  recall "busca semântica" --k 10 --json
```
- Modelos suportados: `qwen/qwen3-embedding-8b` (melhor qualidade), `nvidia/llama-nemotron-embed-vl-1b-v2:free` (custo zero), `google/gemini-embedding-001` (scores mais altos), `openai/text-embedding-3-large`, e mais 6
- Todos os modelos produzem vetores de 384 dimensões por padrão via truncamento MRL — compatível com bancos existentes
### Leia, esqueça, edite e renomeie usando argumento posicional
<!-- skip-test: forget soft-deleta a memória no meio do bloco, invalidando o edit/rename seguintes. O bloco ilustra o ciclo de vida; não é um script executável. -->
```bash
sqlite-graphrag read testes-integracao-postgres --json
sqlite-graphrag forget testes-integracao-postgres
sqlite-graphrag history testes-integracao-postgres --json
sqlite-graphrag edit testes-integracao-postgres --body "Corpo atualizado."
sqlite-graphrag rename testes-integracao-postgres --new testes-postgres
```
- Nome posicional é equivalente a `--name <nome>` para `read`, `forget`, `history`, `edit` e `rename`

### Busque memórias por similaridade semântica
```bash
sqlite-graphrag recall "testes integração postgres" --k 3 --json
```
### Busca híbrida combinando FTS5 e KNN vetorial
```bash
sqlite-graphrag hybrid-search "rollback migração postgres" --k 10 --json
```
### Pesquisa profunda com decomposição multi-hop paralela (v1.0.64)
```bash
sqlite-graphrag deep-research "decisões de arquitetura de autenticação e incidentes" --k 20 --json
```
- Decompõe a query em até 7 sub-queries, executa em paralelo via `JoinSet` + `Semaphore` bounded, mescla resultados com deduplicação cross-query e monta cadeias de evidência da travessia do grafo
- Defaults calibrados contra benchmarks NovelHopQA, StepChain, HopRAG: `--k 20`, `--max-sub-queries 7`, `--max-hops 3`
### Inspecione saúde e estatísticas do banco
```bash
sqlite-graphrag health --json
sqlite-graphrag stats --json
```
### Purgue memórias soft-deleted após período de retenção
```bash
sqlite-graphrag purge --retention-days 90 --dry-run --json
sqlite-graphrag purge --retention-days 90 --yes
```
> **Retenção padrão: 90 dias.** Para purgar TODAS as memórias esquecidas independentemente da idade, passe `--retention-days 0`.

### Ingestão em massa de arquivos Markdown em um diretório
<!-- skip-test: requer um diretório `./docs` com arquivos Markdown relativo ao cwd da invocação. -->
```bash
sqlite-graphrag ingest ./docs --type document --pattern '*.md' --recursive
```
### Ingestão em massa em modo de baixa memória (worker único)
<!-- skip-test: requer um diretório `./docs`; demonstra a flag --low-memory. -->
```bash
# Força ingest single-threaded para reduzir pressão de RSS (recomendado para
# ambientes com <4 GB de RAM e restrições de container/cgroup). Trade-off: 3-4x
# mais tempo de relógio.
sqlite-graphrag ingest ./docs --type document --pattern '*.md' --low-memory

# Ou via variável de ambiente (a flag CLI tem precedência):
SQLITE_GRAPHRAG_LOW_MEMORY=1 sqlite-graphrag ingest ./docs --type document
```
### Ingestão em massa e depois extração do grafo
```bash
# Passo 1 — corpos + embeddings (o único modo de ingest)
sqlite-graphrag ingest ./docs --mode none --recursive --json

# Passo 2 — extração do grafo, processo SEPARADO, só após o passo 1 sair com exit 0
sqlite-graphrag enrich --operation memory-bindings \
  --mode openrouter --openrouter-model MODELO --until-empty --json
```
> **Autenticação:** a chave de API do OpenRouter é a única credencial. Armazene uma vez com
> `echo "sk-or-v1-..." | sqlite-graphrag config add-key --provider openrouter --from-stdin`,
> ou passe `--openrouter-api-key`. Nunca coloque a chave no histórico do shell.
> `ingest` emite NDJSON no stdout: uma linha JSON por arquivo, seguida de uma linha de resumo.
> Valores de `status` por arquivo: `indexed` (criado), `skipped` (duplicata ou nome inválido), `failed` (erro).
> Duplicatas emitem `status: "skipped"` com `action: "duplicate"` e não contam como falhas.
> Passe `--dry-run` para pré-visualizar o mapeamento de nomes (basenames em kebab-case) sem escrever nada no banco.
> Schema: `docs/schemas/ingest-file-event.schema.json`, `docs/schemas/ingest-summary.schema.json`.

### Renomeie uma memória mantendo o histórico de versões
<!-- skip-test: nomes ilustrativos (`nome-antigo`, `nome-novo`) — a memória de origem não existe no banco isolado de teste. -->
```bash
sqlite-graphrag rename nome-antigo --new-name nome-novo --json
```
### Edite corpo ou descrição de uma memória (gera nova versão)
<!-- skip-test: depende da memória não ter sido soft-deleted por um bloco ilustrativo anterior. -->
```bash
sqlite-graphrag edit testes-integracao-postgres --body "Corpo atualizado."
sqlite-graphrag edit testes-integracao-postgres --description "Descrição atualizada."
```
### Restaure uma memória para uma versão anterior
<!-- skip-test: `restore --version 2` exige que a memória tenha pelo menos duas versões, o que não é o caso no banco isolado de exemplo. -->
```bash
sqlite-graphrag history testes-integracao-postgres --json
sqlite-graphrag restore --name testes-integracao-postgres --version 2 --json
```
### Aplique migrações de schema pendentes
```bash
sqlite-graphrag migrate --status --json
sqlite-graphrag migrate --json
```
### Resolva a precedência de namespace para a invocação atual
```bash
sqlite-graphrag namespace-detect --json
sqlite-graphrag namespace-detect --namespace projeto-foo --json
```
### Atualize as estatísticas do planejador de queries do SQLite
```bash
sqlite-graphrag optimize --json
```
### Recupere espaço em disco e faça checkpoint do WAL
```bash
sqlite-graphrag vacuum --json
```
### Crie um relacionamento tipado entre duas entidades
<!-- skip-test: requer que as entidades `OpenAI` e `GPT-4` já existam no namespace. -->
```bash
sqlite-graphrag link --from "OpenAI" --to "GPT-4" --relation uses --weight 0.8 --json
```
### Remova um relacionamento específico entre duas entidades
<!-- skip-test: requer o relacionamento criado pelo exemplo `link` anterior. -->
```bash
sqlite-graphrag unlink --from "OpenAI" --to "GPT-4" --relation uses --json
```
### Percorra memórias conectadas via grafo de entidades
```bash
sqlite-graphrag related primeira-memoria --max-hops 2 --limit 10 --json
```
> **Resultados vazios são normais** para memórias sem arestas no grafo ainda — extraia entidades primeiro via `remember` ou `ingest`. Arestas se formam quando ≥2 entidades co-ocorrem no mesmo corpo de memória.

### Exporte um snapshot do grafo em json, dot ou mermaid
<!-- skip-test: `--output graph.json` escreve um arquivo relativo ao cwd da invocação; polui o workspace de teste. Os demais subcomandos read-only do graph são exercitados pelos testes de integração do cookbook. -->
```bash
sqlite-graphrag graph --format json --output graph.json
sqlite-graphrag graph stats --json
sqlite-graphrag graph traverse --from "OpenAI" --depth 2 --json
sqlite-graphrag graph entities --entity-type organization --limit 50 --json
```
### Remova entidades órfãs sem memórias e sem relacionamentos
```bash
sqlite-graphrag cleanup-orphans --dry-run --json
sqlite-graphrag cleanup-orphans --yes --json
```
### Remoção em massa de relacionamentos por tipo
<!-- skip-test: requer que existam relacionamentos no namespace. -->
```bash
sqlite-graphrag prune-relations --relation mentions --dry-run --show-entities --json
sqlite-graphrag prune-relations --relation mentions --yes --json
```
### Limpe os modelos de embedding/NER em cache no diretório XDG
<!-- skip-test: apaga o cache de modelos de embedding; seguro em produção, mas no suite de integração obriga um re-download caro nos comandos seguintes. -->
```bash
sqlite-graphrag cache clear-models --yes
```
### Liste todas as versões de uma memória
<!-- skip-test: depende do estado do ciclo de vida estabelecido por blocos ilustrativos anteriores (também marcados `skip-test`). -->
```bash
sqlite-graphrag history testes-integracao-postgres --no-body --json
```


## Comandos
### Núcleo de ciclo de vida do banco
| Comando | Argumentos | Descrição |
| --- | --- | --- |
| `init` | `--namespace <ns>` | Inicializa o banco e aplica migrações (sem download de modelo, sem sondar binário) |
| `health` | `--json` | Exibe integridade, teste funcional FTS5, versão SQLite, detecção de super-hub (grau > 50); v1.1.01 adiciona `vec_memories_missing`/`vec_entities_missing`/`vec_chunks_missing` e `vec_*_coverage_pct` por tabela |
| `stats` | `--json` | Conta memórias, entidades e relacionamentos; o JSON expõe um `total_memories` no topo |
| `migrate` | `--json` | Aplica migrações pendentes via `refinery` |
| `vacuum` | `--json` | Faz checkpoint do WAL e libera espaço |
| `optimize` | `--json`, `--skip-fts` | Executa `PRAGMA optimize` e reconstrói índice FTS5 (pule com `--skip-fts`) |
| `backup` | `--output <caminho>` | Cria backup do banco via SQLite Online Backup API |
| `sync-safe-copy` | `--dest <caminho>` (alias `--output`) | Gera cópia segura para sincronização em nuvem |
| `config` | `set`, `get`, `list` (`--effective`), `unset`, `path`, `doctor`, `add-key`, `list-keys`, `remove-key` | Config operacional XDG e chaves de API (v1.2.0); precedência flag > XDG `config set` > default; sem product env |
### Ciclo de vida do conteúdo de memória
| Comando | Argumentos | Descrição |
| --- | --- | --- |
| `remember` | `--name`, `--type`, `--description`, `--body` (ou `--body-file`/`--body-stdin`), `--entities-file`, `--relationships-file`, `--graph-stdin`, `--graph-file <path>`, `--llm-parallelism <N>` (padrão 4), `--enable-ner` (apenas regex de URL desde v1.0.79), `--strict-name`, `--force-merge`, `--replace-graph`, `--clear-body`, `--dry-run`, `--enqueue-enrich` (hot-set v1.2.0) | Salva memória com grafo opcional; `--graph-file` carrega o grafo de um arquivo (combinável com `--body-file`); `--strict-name` rejeita nomes não-kebab em vez de normalizar; `--replace-graph` (com `--force-merge`) zera os vínculos existentes antes de escrever; `--type`/`--description` opcionais com `--force-merge` (herdados do existente); `--dry-run` valida sem persistir; `--enqueue-enrich` enfileira entity-descriptions e devolve `entities_created` / `enrich_recommended` |
| `remember-batch` | `--transaction`, `--force-merge`, `--fail-fast`, `--enqueue-enrich` | Criação em lote de memórias via NDJSON no stdin; **`description` obrigatória na criação** (v1.2.0); uma invocação, um slot, uma conexão DB |
| `recall` | `<query>`, `-k`/`--k` (alias `--limit` desde v1.0.35), `--type`, `--max-hops`, `--max-distance`, `--all-namespaces`, `--no-graph` | Busca memórias semanticamente via KNN + travessia do grafo |
| `read` | `[nome]` ou `--name <nome>`, `--id <N>`, `--with-graph`, `--format raw` | Recupera memória por nome kebab-case exato ou `memory_id` inteiro via `--id`; `--with-graph` inclui entidades e relacionamentos vinculados; `--format raw` imprime o corpo puro sem envelope JSON |
| `list` | `--type`, `--limit`, `--offset`, `--include-deleted` | Pagina memórias por `updated_at`; limite padrão é tudo com `--json`, 50 para texto; resposta inclui `total_count`, `truncated`, `body_length` |
| `forget` | `[nome]` ou `--name <nome>` | Remove memória logicamente preservando histórico |
| `rename` | `[antigo]`, ou `--name`/`--old`/`--from <NOME>` (desde v1.0.35), `--new-name`/`--new`/`--to <NOME>` (desde v1.0.35) | Renomeia memória mantendo versões |
| `edit` | `[nome]` ou `--name`, `--body`, `--description`, `--type`, `--force-reembed`, `--llm-parallelism <N>` | Edita corpo, descrição ou tipo gerando nova versão; pula re-embedding quando conteúdo do body é inalterado; `--force-reembed` (v1.0.79) regenera o embedding sem alterar o corpo |
| `history` | `[nome]` ou `--name <nome>`, `--diff` | Lista versões da memória; `--diff` inclui resumo de mudanças por caractere |
| `memory-entities` | `[nome]` ou `--name <nome>`, `--entity <nome>` | Lista entidades de uma memória, ou memórias vinculadas a uma entidade (busca reversa via `--entity`) |
| `restore` | `--name`, `--version` | Restaura memória para versão anterior |
| `ingest` | `<DIR>`, `--type`, `--pattern <GLOB>` (padrão `*.md`), `--recursive`, `--mode none` (único valor aceito; `claude-code`/`codex`/`opencode` removidos, `gliner` removido na v1.0.79), `--ingest-parallelism N`, `--llm-parallelism N` (padrão 2, workers de embedding), `--low-memory`, `--enable-ner` (apenas URL-regex desde a v1.0.79), `--force-merge`, `--fail-fast`, `--dry-run`, `--max-cost-usd`, `--enrich-after`, `--name-prefix <PREFIXO>` (v1.1.01) | Ingere em lote cada arquivo correspondente como memória separada (saída NDJSON); `--force-merge` atualiza arquivos duplicados em vez de pular (dedup por `body_hash`); corpos oversized são divididos nativamente em chunks; a extração é um passo SEPARADO de `enrich --mode openrouter`, não um modo do ingest; `--dry-run` faz preview do mapeamento de nomes sem escrever; `--name-prefix` (v1.1.01) prefixa cada nome derivado (teto de 80 chars) |
| `export` | `--namespace`, `--type`, `--include-deleted`, `--limit`, `--offset` | Exporta memórias como NDJSON para backup ou migração |
| `cache clear-models` / `list` / `stats` | `--yes` (clear) | Remove modelos legados do cache XDG; `list`/`stats` (v1.2.0) reportam tamanhos em disco |

> **Validação de nomes de memória.** Nomes devem corresponder a `[a-z0-9-]+` (kebab-case, somente ASCII).
> Unicode e maiúsculas são rejeitados com exit code 1. Nomes maiores que 60 caracteres
> emitidos por `ingest` são truncados; revise o log WARN para identificar nomes mutilados.
### Recuperação e grafo
| Comando | Argumentos | Descrição |
| --- | --- | --- |
| `hybrid-search` | `<query>`, `--k`, `--rrf-k`, `--with-graph`, `--max-hops`, `--min-weight`, `--weight-vec`, `--weight-fts` | FTS5 + vetor via RRF; degradação graciosa quando FTS5 corrompido (`fts_degraded`, auto-rebuild); `normalized_score` para comparabilidade |
| `deep-research` | `<query>`, `--k`, `--max-sub-queries`, `--max-hops`, `--with-bodies`, `--sub-query-strategy`, `--sub-queries-file`, `--output` (v1.1.05 atomwrite), `--json` | Pesquisa GraphRAG multi-hop; token único expande em sub-queries `source: "aspect"` (v1.1.05); `--output` grava envelope atômico + ack `blake3` no stdout |
| `namespace-detect` | `--namespace <nome>` | Resolve precedência de namespace para invocação |
| `link` | `--from`/`--to` ou `--from-id`/`--to-id` (v1.1.05), `--relation`, `--weight`, `--create-missing`, `--entity-type`, `--strict-relations` | Cria relacionamento; IDs numéricos via `--from-id`/`--to-id`; nomes só de dígitos são rejeitados; `--strict-relations` rejeita tipos não-canônicos |
| `unlink` | `--from`, `--to`, `--relation`, `--entity`, `--all`, `--memory <nome> --entity <nome>` | Remove relacionamentos; `--relation` agora opcional (remove todos entre o par); `--entity X --all` remove todas edges da entidade; `--memory <nome> --entity <nome>` remove um único vínculo curado memória-entidade sem tocar nas arestas entidade-entidade |
| `related` | `--name`, `--limit`, `--hops` | Percorre memórias conectadas pelo grafo a partir de uma memória base |
| `graph` | `--format`, `--output` | Exporta snapshot do grafo em `json`, `dot` ou `mermaid` |

> **Breaking change em v1.0.44.** O JSON de `graph entities` renomeou o array de nível superior
> de `items` para `entities`. Atualize filtros jaq/jq: `.items[]` vira `.entities[]`.
> O comando `list` continua usando `items`.

### Subcomandos do graph
| Subcomando | Descrição | Flags principais |
| --- | --- | --- |
| `graph traverse --from <ENTIDADE>` | Percorre o grafo de entidades a partir de um nó inicial usando BFS; v1.1.05: sugestões se NotFound; `--fuzzy` auto-resolve vencedor claro | `--depth` (padrão 2), `--namespace`, `--fuzzy` (v1.1.05) |
| `graph stats` | Imprime estatísticas do grafo (nós, arestas, distribuição de grau) | `--namespace` |
| `graph recompute-degree` | Reconcilia o `entities.degree` em cache com as contagens reais de arestas em uma única transação (v1.1.01); envelope `{total, updated, zeroed, unchanged}` | `--dry-run`, `--namespace` |
| `graph entities` | Lista entidades com grau e ordenação | `--limit` (padrão 50), `--entity-type`, `--namespace`, `--sort-by degree\|name\|created_at`, `--order asc\|desc` |

### Manutenção
| Comando | Argumentos | Descrição |
| --- | --- | --- |
| `purge` | `--retention-days <n>`, `--now` (v1.2.0, alias de `--retention-days 0`), `--dry-run`, `--yes` | Apaga permanentemente memórias soft-deleted; `--yes --now` limpa todas as soft-deleted independentemente da idade |
| `cleanup-orphans` | `--namespace`, `--dry-run`, `--yes` | Remove entidades sem memórias e sem relacionamentos |
| `prune-relations` | `--relation <tipo>`, `--namespace`, `--dry-run`, `--yes`, `--show-entities` | Remove em massa todos os relacionamentos de um tipo; `--show-entities` lista entidades afetadas |
| `delete-entity` | `--name <entidade>`, `--cascade` | Remove entidade e cascateia remoção de relacionamentos e bindings |
| `rename-entity` | `--name <entidade>` ou `--id <ID>` (v1.1.01), `--new-name <nome>` | Renomeia uma entidade preservando todos os relacionamentos e vínculos com memórias; re-gera vetor |
| `reclassify` | `--name <entidade> --new-type <tipo>`, `--description <texto>`, ou `--from-type <antigo> --to-type <novo> --batch` | Reclassifica tipos de entidade individual ou em massa; `--description` atualiza descrição no modo individual |
| `merge-entities` | `--names <a,b,c> --into <destino>`, ou `--ids <1,2,3> --into-id <ID>` (v1.1.01, escopo de namespace); `--cross-namespace` (v1.1.03); rejeita self-ref pré-DB (v1.1.05) | Funde entidades-fonte no destino, movendo todas as edges; self-ref rejeitado antes de qualquer trabalho no DB |
| `split-body` | `--name <N>` ou `--batch`, `--threshold` (padrão 25000), `--json` | Divide corpo sobredimensionado em filhas `{name}-part-{i}`; marca original `superseded_by_split`; cria relações `replaces`; filhas precisam de `enrich --operation re-embed --target memories` (v1.1.03) |
| `reclassify-relation` | `--from-relation` / `--to-relation`, ou `--literal-from` / `--literal-to`, `--batch`, `--json` | Renomeia tipos de relação em massa; `--literal-from`/`--literal-to` casam/escrevem verbatim (bypass da normalização do clap) para migrações underscore→hífen (v1.1.01/v1.1.03) |
| `normalize-entities` | `--namespace`, `--dry-run`, `--yes`, `--json` | Normaliza nomes de entidade para kebab-case e faz auto-merge de quase-duplicatas |
| `prune-ner` | `--entity <nome>` ou `--all`, `--dry-run`, `--yes` | Remove bindings NER da tabela memory_entities |
| `fts rebuild` | `--json` | Reconstrói o índice FTS5 de busca textual do zero |
| `fts check` | `--json` | Executa integrity-check do FTS5 sem modificar o índice |
| `fts stats` | `--json` | Exibe estatísticas do índice FTS5 (contagem, páginas shadow) |
| `completions` | `bash`, `zsh`, `fish`, `powershell`, `elvish` | Gera completions de shell para o shell especificado |
| `schema` | (nenhum), `--name <ID>` | Catálogo legível por máquina dos **75** contratos JSON (v1.2.2). `schema` puro emite NDJSON, um `{"id","invoke"}` por linha, onde `invoke` é o comando pronto para copiar; `--name <ID>` emite o documento JSON Schema daquele contrato. `<ID>` desconhecido sai com **exit 4**. Documentos `$schema` são isentos da superfície de saída agent-native, então qualquer flag global encadeia com segurança |
| `enrich` | `--operation <op>`, `--mode openrouter` (único valor aceito; resolvido por padrão quando omitido), `--status`, `--list-dead`, `--requeue-dead`, `--list-skipped`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans`, `--force-redescribe`, `--entity-names`, `--memory-names`, `--names`/`--names-file`, `--until-empty`, `--max-runtime`, `--max-attempts`, `--rest-concurrency`, `--resume`, `--retry-failed`, `--max-cost-usd`, `--preflight-check`, `--rate-limit-buffer`, `--reset-stale-claims`, `--openrouter-model` (obrigatória com openrouter), … | Pipeline de qualidade do grafo via LLM; fila enrich multi-namespace (v1.2.0); **v1.2.1 CAPA:** claim/contagem/resume isolados por `operation`+`namespace`; `--until-empty` conta só esta op+ns; `--force-redescribe` reabre `skipped`/`done` uma vez/processo (nunca `dead`); re-embed usa BLOB `LENGTH(embedding)=dim*4` + reconciliação de zumbis; enqueue faz strip de `entity:` e valida chunk no ns; CAPA-D só marcadores compostos de configuration file; inspetores de dead/skipped sem LLM; entity-descriptions com `--force-redescribe` / `--entity-names`; ops de memória com `--memory-names`; ver também `remember --enqueue-enrich` |
| `slots` | `status`, `release --slot-id <N> --yes`, `cleanup`, `--json` | Semáforo de slots LLM host-wide (GAP-004); `status` reporta `max_concurrency`/`acquired`/`waiting`/`held_by_pid[]`; `release` ceifa um slot; `cleanup` remove arquivos de slot stale/órfãos |
| `pending` | `list`, `show <id>`, `cleanup`, `--json` | Fila de checkpoint em 3 estágios do `remember` |
| `embedding` | `status`, `list`, `abandon`, `--json` | Saúde e inspeção da fila de embeddings; `status` reporta `coverage` e `*_missing` |
| `pending-embeddings` | `list`, `status`, `abandon` | Operações em lote na fila de retry; `status` é alias de `embedding status` (v1.2.0) |
| `vec orphan-list` / `purge-orphan` / `stats` | `--json`, `--yes` (purge) | Manutenção das tabelas vetoriais (órfãos e stats) |
| `deep-research` | `<query>`, `--output`/`-o`, `--quiet`, `--json`, … | Pesquisa GraphRAG multi-hop; token único expande em aspectos; `--output` atomwrite + ack `blake3` |

> **GAP-SG-139 (v1.2.0):** folhas host/XDG aceitam `--db` como **no-op** documentado para que agentes que anexam `--db` em toda invocação não recebam clap exit 2. Superfícies: `config`, `slots`, `cache`, `completions`. Comandos com escopo de grafo ainda resolvem storage via `--db` / `config set db.path`.

### Flags globais v1.0.82 / v1.0.85

| Flag | Aplica-se a | Descrição |
| --- | --- | --- |
| `--llm-backend <openrouter\|none>` | `remember`, `edit`, `ingest`, `enrich` | Transporte de embedding: `openrouter` (padrão) ou `none` (pula o embedding) |
| `--llm-fallback <cadeia>` | `remember`, `edit`, `ingest`, `enrich` | Cadeia ordenada de fallback quando o backend primário falha; padrão `none` |
| `--llm-max-host-concurrency <N>` | Todos os comandos que spawnam LLM | Limita subprocessos LLM concorrentes no host inteiro via flock `fs4` (ADR-0039); default derivado da CPU e do tier OAuth |
| `--llm-slot-wait-secs <N>` | Todos os comandos que spawnam LLM | Segundos de espera por um slot livre antes de falhar (default 30s); combine com `--llm-slot-no-wait` para fail-fast |
| `--quiet` / `-q` | Flag global de topo (v1.1.05) | Suprime tracing não-erro no stderr para que o JSON do stdout permaneça limpo em pipelines headless; combine com `deep-research --output PATH` para envelopes grandes. NUNCA redirecione stdout+stderr para o mesmo arquivo com `&>` |

### Flags globais v1.2.2 — superfície de saída agent-native (GAP-SG-142)

Oito flags de topo remodelam o envelope JSON em um único ponto, para que um agente pare de carregar um filtro `jaq` no prompt só para ler um campo. Elas valem para **todos** os subcomandos e compõem em ordem fixa: **filter → sort → dedupe → max-items → select → count-only → truncate-content → max-output-bytes**.

| Flag | Alias | Descrição |
| --- | --- | --- |
| `--select <CHAVES>` | `--fields` | Mantém apenas estas chaves separadas por vírgula em cada elemento de resultado. Aceita caminhos com ponto (`stats.total`). Chave ausente em um elemento é pulada, nunca emitida como `null` — a projeção jamais inventa campo. Envelope sem array de resultados é projetado ele mesmo |
| `--filter <EXPR>` | — | Mantém apenas elementos que satisfazem `EXPR`. Gramática: `chave=valor`, `chave!=valor`, `chave~substring` (contém, sem distinguir maiúsculas); `==` é sinônimo de `=`. Repita a flag para conjugar predicados com **AND**. Expressão malformada falha rápido com **exit 2**, para que um typo nunca seja confundido com conjunto de resultados vazio |
| `--max-items <N>` | — | Emite no máximo `N` elementos de resultado. **Distinta do `--limit` por subcomando e do `-k`**, que limitam a *consulta*; esta limita só o que chega ao stdout, e só *depois* do filtro |
| `--sort <CHAVE>` | — | Ordena os elementos em ordem ascendente por esta chave (caminho com ponto). Números comparam numericamente, o resto como texto. Elementos sem a chave mantêm a ordem relativa no fim da lista |
| `--dedupe-by <CHAVE>` | — | Descarta elementos posteriores que repetem o valor desta chave. Elementos sem a chave são sempre mantidos, já que nunca foram provados duplicados |
| `--count-only` | — | Substitui o payload por `{"count": N}`, onde `N` é o que sobreviveu a `--filter`, `--dedupe-by` e `--max-items` |
| `--truncate-content <N>` | — | Encurta toda string maior que `N`. Conta **caracteres, nunca bytes**, então uma sequência UTF-8 nunca é partida ao meio |
| `--max-output-bytes <N>` | — | Limita o envelope serializado a `N` bytes **descartando elementos de resultado do fim** até caber — nunca fatiando o texto JSON, que deixaria de fazer parse |

#### Garantias de contrato
- **Envelope de falha nunca é filtrado.** Um envelope com `error: true` ou `ok: false` chega ao chamador literalmente, independente do que `--filter` disser. `--filter` molda linhas de resultado; nunca molda o contrato de erro
- **Documentos JSON Schema passam intactos.** Um payload com `$schema` é contrato, não conjunto de resultados
- **Truncagem nunca é silenciosa.** Tudo que foi removido é registrado no membro `agent_surface` e levanta a flag `truncated` de topo
- **Streams NDJSON contornam a superfície** — emissores orientados a linha mantêm um registro por linha, porque remodelá-los mudaria o contrato do stream
- O array de resultados é localizado pelos nomes conhecidos `results`, `items`, `entities`, `memories`, `hits`, `rows`, `matches`, `data`, nesta ordem; caso contrário vence o primeiro membro que for array

#### O registro `agent_surface`
Presente sempre que um knob estiver ativo. Reporta `input_count` e `output_count` sempre, mais `select`, `filters`, `sort`, `dedupe_by`, `max_items` quando definidos, `count_only` sob `--count-only`, `content_truncated` + `truncate_content` quando uma string foi encurtada, e `output_truncated` + `dropped` + `max_output_bytes` quando o teto de bytes disparou.

#### Precedência
| Knob | Chave XDG | Default |
| --- | --- | --- |
| `--max-items` | `agent_surface.max_items` | `0` (sem teto) |
| `--truncate-content` | `agent_surface.truncate_content` | `0` (desligado) |
| `--max-output-bytes` | `agent_surface.max_output_bytes` | `0` (sem teto) |

Flag CLI > XDG `config set` > default nomeado, como em todo o resto. Nenhuma variável de ambiente de produto é lida. Sem nenhum knob definido, o envelope é idêntico byte a byte à saída anterior à v1.2.2.

#### Exemplos offline

```bash
sqlite-graphrag list --json --count-only
sqlite-graphrag stats --json --select total_memories
sqlite-graphrag graph entities --json --select name,entity_type --max-items 5
sqlite-graphrag health --json --truncate-content 200
sqlite-graphrag schema
sqlite-graphrag schema --name hybrid-search
```

### Flag global v1.2.2 — `--no-input`

| Flag | Aplica-se a | Descrição |
| --- | --- | --- |
| `--no-input` | Flag global de topo | Recusa ler stdin em qualquer ponto desta invocação |

A recusa é **declarativa, não emergente**. Sem a flag, um caminho de stdin só falha quando a leitura é tentada — imediatamente em TTY, depois do deadline nos demais casos. Com ela, `--body-stdin`, `--graph-stdin`, `remember-batch` e todo outro leitor de stdin falham de antemão com **exit 65**, mesmo com um pipe conectado que teria fornecido dados. É exatamente esse o objetivo: automação desassistida deve falhar rápido e alto, em vez de travar esperando um humano que não está lá.

Precedência: flag > XDG `cli.no_input` > `false`. Um host que optou pela flag via XDG a desliga **removendo a chave**, não com `--no-input=false` — essa grafia leria como "entrada é permitida aqui" enquanto a automação ao redor assume o contrário.

### Subcomandos de `cache`
| Subcomando | Descrição |
| --- | --- |
| `clear-models` | Remove os arquivos de modelo de embedding/NER em cache (força novo download no próximo `init`) |
| `list` / `stats` | Lista arquivos de modelo em cache com tamanhos e uso total; `stats` é alias de `list` (v1.2.0, GAP-E2E-09) |

### Fila de embeddings e status (v1.2.0)
| Comando | Argumentos | Descrição |
| --- | --- | --- |
| `pending-embeddings` | `list`, `status`, `abandon` | Inspeção da fila de retry de embedding; `status` (v1.2.0) é alias de `embedding status` |
| `embedding` | `status`, `list`, `abandon` | Saúde e inspeção por entrada da fila; `status --json` reporta `coverage` e contadores `*_missing` |


## Configuração (XDG — v1.2.5)

### Precedência (sem env de produto no hot path)

Runtime resolve knobs como **flag CLI > XDG `config set` > default** via `runtime_config` / `paths` / `resolve_api_key`. Bindings `clap env=` de produto foram removidos (G-T-XDG-04). Não use `SQLITE_GRAPHRAG_*` / `OPENROUTER_*` como contrato de configuração instalada — prefira flags e `config.toml` sob XDG.

| Camada | Uso |
| --- | --- |
| Flag CLI (`--db`, `--embedding-dim`, `--openrouter-api-key`, …) | Ganha sempre sobre XDG e defaults |
| XDG `config set` / `~/.config/sqlite-graphrag/config.toml` | Persistência operacional entre invocações |
| Default embutido | Valores seguros quando nada foi setado |

Chaves de API: **flag CLI > config XDG (`config add-key` / doctor) > env depreciada** (G-T-XDG-02/03). Env de SO legítima permanece só para locale/PATH/HOME/XDG/NO_COLOR.

### Comandos `config`

| Subcomando | Descrição |
| --- | --- |
| `config path` | Caminho resolvido do arquivo de config XDG |
| `config set <KEY> <VALUE>` | Grava setting operacional no XDG (sem segredos no help) |
| `config get <KEY>` | Lê um setting armazenado |
| `config list` | Lista settings armazenados (sem segredos) |
| `config list --effective` | Inclui defaults bem-conhecidos mesmo quando não gravados (v1.2.0) |
| `config unset <KEY>` | Remove um setting |
| `config doctor` | Diagnostica camadas de resolução de chave (flag/XDG; env de produto depreciada) |
| `config add-key` / `list-keys` / `remove-key` | Gerencia chaves de API (stdin; mascaradas) |

### Referência completa das chaves `config set` (63 chaves, v1.2.5)

Toda chave abaixo é aceita por `config set` e resolvida como **flag CLI > XDG `config set` > default**. `sqlite-graphrag config list --effective --json` imprime o mesmo inventário em tempo de execução; esta tabela é comparada com `src/config/registry.rs` pelo teste `tests/docs_xdg_coverage.rs`, então não diverge em silêncio.

`(nenhum)` significa que a chave não tem default embutido: quando não é setada, o subsistema recorre à própria heurística de runtime (auto-dimensionamento, detecção de host ou flag obrigatória).

Chave fora desta lista é rejeitada com exit 1. Até a v1.2.4 esta seção citava `enrich.preserve_threshold`, `enrich.entity_connect.max_runtime_secs` e `llm.concurrency`, que nunca existiram no registry.

#### Superfície de saída agent-native

| Chave | Default | Finalidade |
| --- | --- | --- |
| `agent_surface.max_items` | `0` | Teto permanente de `--max-items`. `0` desliga. Desde a v1.2.5 (GAP-SG-191) limita todo array do envelope, não só o primário |
| `agent_surface.max_output_bytes` | `0` | Teto permanente de `--max-output-bytes`. `0` desliga. A saída continua JSON parseável e o stub reporta o teto solicitado |
| `agent_surface.truncate_content` | `0` | Teto permanente de `--truncate-content` (corte por campo em caracteres). `0` desliga |

#### Banco e armazenamento

| Chave | Default | Finalidade |
| --- | --- | --- |
| `db.path` | `(nenhum)` | Banco padrão. Sobrescrito por `--db <PATH>` depois do subcomando. Sem nenhum dos dois, `./graphrag.sqlite` |
| `db.busy_retries` | `5` | Tentativas em `SQLITE_BUSY` antes do exit 15 |
| `db.busy_base_delay_ms` | `300` | Atraso base do backoff exponencial entre as tentativas |
| `db.query_timeout_ms` | `5000` | Teto de tempo por consulta |
| `cache.dir` | `(nenhum)` | Raiz do cache. Recai no diretório de cache XDG |

#### Embedding

| Chave | Default | Finalidade |
| --- | --- | --- |
| `embedding.dim` | `1024` | Dimensionalidade dos vetores. Alterar num banco populado quebra a similaridade de cosseno em silêncio — migre deliberadamente, nunca como efeito colateral de flag |
| `embedding.model` | `(nenhum)` | Modelo de embedding padrão. Lido desde a v1.2.5 (GAP-SG-192); antes a chave era documentada e ignorada |
| `embedding.backend` | `(nenhum)` | Backend de embedding padrão (`auto` ou `openrouter`). Registrada na v1.2.5 (GAP-SG-198); o `--help` de `--embedding-backend` a prometia desde a v1.0.93 enquanto `config set` respondia exit 1 |
| `llm.backend` | `(nenhum)` | Backend LLM de embedding padrão (`open-router` ou `none`). Registrada na v1.2.5 (GAP-SG-198), mesmo defeito de `embedding.backend` |
| `embedding.batch_size` | `32` | Passagens por requisição REST de embedding |
| `embedding.timeout_secs` | `300` | Timeout por requisição de embedding |
| `embedding.entity_cache_max_entries` | `10000` | Capacidade do LRU de embedding de entidades |
| `embedding.entity_cache_ttl_secs` | `3600` | Vida útil de cada entrada do cache de entidades |

#### Transporte LLM e slots de host

| Chave | Default | Finalidade |
| --- | --- | --- |
| `llm.model` | `(nenhum)` | Modelo de texto padrão para extração de grafo |
| `llm.fallback` | `none` | Cadeia de fallback de backend. Só `openrouter` e `none` são válidos desde a v1.2.0 |
| `llm.openrouter_timeout_secs` | `600` | Timeout por requisição de chat OpenRouter |
| `llm.probe_timeout_ms` | `800` | Timeout da sonda de credencial e de backend |
| `llm.max_host_concurrency` | `(nenhum)` | Teto de trabalho LLM concorrente no host. Auto-dimensionado quando ausente |
| `llm.slot_wait_secs` | `300` | Quanto esperar por um slot de host antes de desistir |
| `llm.slot_no_wait` | `false` | Falha imediatamente em vez de entrar na fila por um slot |
| `llm.worker_rss_mb` | `350` | RSS presumido por worker, usado para dimensionar concorrência contra a memória livre |
| `llm.skip_embedding_on_failure` | `false` | Persiste a linha sem vetor quando o embedding falha, em vez de falhar a escrita |

#### Enriquecimento

| Chave | Default | Finalidade |
| --- | --- | --- |
| `enrich.scan_page_size` | `512` | Largura da página keyset dos scanners de streaming (GAP-SG-185, faixa 1..=4096) |
| `enrich.yield_every_n_items` | `10` | Intervalo de yield cooperativo durante drains longos |
| `enrich.reembed_claim_batch` | `32` | Linhas reivindicadas por transação de `re-embed` |
| `enrich.rate_limit_deadline_secs` | `3600` | Teto de tempo enquanto recua diante de um rate limit |
| `enrich.circuit_breaker_reset_secs` | `60` | Cooldown antes de o breaker fechar de novo |
| `enrich.entity_connect.default_limit` | `100` | Pares candidatos por scan de `entity-connect` |
| `enrich.entity_connect.large_ns_limit` | `25` | Teto menor aplicado a namespaces grandes |
| `enrich.entity_description.domain` | `auto` | Dica de domínio para as descrições geradas |
| `enrich.entity_description.grounding_threshold` | `0.12` | Score mínimo de ancoragem para a descrição ser mantida |
| `enrich.entity_description.corpus_top_k` | `5` | Memórias amostradas como evidência por entidade |
| `enrich.entity_description.min_corpus_chars` | `40` | Tamanho mínimo de evidência antes de chamar o LLM |
| `enrich.entity_description.snippet_chars` | `400` | Caracteres por trecho de evidência |
| `enrich.entity_description.quality_sample` | `50` | Tamanho da amostra por trás de `quality_pct` no `enrich --status` |

#### Busca

| Chave | Default | Finalidade |
| --- | --- | --- |
| `search.hybrid.max_graph_results` | `50` | Teto de `graph_matches` no `hybrid-search --with-graph`. `0` remove o teto |

#### Ingest e limites de escrita

| Chave | Default | Finalidade |
| --- | --- | --- |
| `ingest.low_memory` | `false` | Troca throughput por menor conjunto residente durante o ingest |
| `limits.max_entities_per_memory` | `50` | Entidades aceitas por escrita |
| `limits.max_relations_per_memory` | `50` | Relações aceitas por escrita |

#### Rede

| Chave | Default | Finalidade |
| --- | --- | --- |
| `network.openrouter.chat_url` | `https://openrouter.ai/api/v1/chat/completions` | Endpoint de chat completions do OpenRouter |
| `network.openrouter.embeddings_url` | `https://openrouter.ai/api/v1/embeddings` | Endpoint de embeddings do OpenRouter |
| `network.chat_url` | `(nenhum)` | Alias de `network.openrouter.chat_url` |
| `network.embed_url` | `(nenhum)` | Alias de `network.openrouter.embeddings_url` |

#### Concorrência e controle de processo

| Chave | Default | Finalidade |
| --- | --- | --- |
| `parallelism.max_total_workers` | `64` | Teto absoluto de tarefas worker |
| `parallelism.rayon_threads` | `(nenhum)` | Tamanho do pool Rayon. Auto-dimensionado quando ausente |
| `parallelism.embed_runtime_threads` | `(nenhum)` | Threads Tokio do runtime de embedding. Auto-dimensionado quando ausente |
| `system.max_load_per_ncpu` | `2.0` | Teto de load average por CPU antes de estrangular trabalho novo |
| `cli.max_instances` | `(nenhum)` | Teto de processos concorrentes desta CLI. Auto-dimensionado quando ausente |
| `retry.disable` | `false` | Desliga a política de retry embutida |
| `shutdown.ignore` | `false` | Ignora o caminho de shutdown gracioso |

#### Comportamento da CLI, log e locale

| Chave | Default | Finalidade |
| --- | --- | --- |
| `cli.no_input` | `false` | `--no-input` permanente: leitores de stdin recusam de saída com **exit 1** (`AppError::Validation`) mesmo com pipe anexado |
| `cli.stdin_timeout_secs` | `60` | Quanto um leitor de stdin espera por entrada |
| `namespace.default` | `global` | Namespace usado quando `--namespace` está ausente |
| `display.tz` | `UTC` | Zona IANA dos campos JSON `*_iso` |
| `i18n.lang` | `en` | Idioma da UI no stderr. Os payloads JSON permanecem em inglês |
| `log.level` | `warn` | Nível de tracing local no stderr |
| `log.format` | `pretty` | `pretty` ou `json` |
| `log.to_file` | `false` | Espelha o tracing local em arquivo |
| `log.rotation` | `daily` | Política de rotação quando `log.to_file` está ligado |
| `log.retention_days` | `7` | Por quanto tempo os logs rotacionados são mantidos |

```bash
# Inspecionar defaults efetivos
sqlite-graphrag config list --effective --json

# URLs OpenRouter via XDG (sem hardcode no cliente)
sqlite-graphrag config set network.openrouter.embeddings_url "https://openrouter.ai/api/v1/embeddings"
sqlite-graphrag config set network.openrouter.chat_url "https://openrouter.ai/api/v1/chat/completions"
sqlite-graphrag config set search.hybrid.max_graph_results 50

# Caminho do TOML
sqlite-graphrag config path --json
```

### Observabilidade e manutenção (v1.2.0)

| Comando | Descrição |
| --- | --- |
| `pending-embeddings status` | Contagens de saúde da fila (alias de `embedding status`) |
| `pending-embeddings list` / `abandon` | Inspeção e abandono em lote |
| `cache stats` | Alias de `cache list` (tamanho em disco dos modelos legados) |
| `purge --now` | Equivale a `--retention-days 0` (todas as soft-deleted, qualquer idade); combine com `--yes` |

### O que não é mais contrato de produto

- Help scrub: sem menção a env de produto nem Box “about” no help.
- Alias `telemetry` removido; whitelist de spawn não encaminha OTEL remoto.
- `related_to` normaliza para `related`; EntityType `module` vira `Concept`.
- DB: `--db` ou XDG `db.path` — nunca env de produto como caminho canônico. A chave legada `db.default_path` não é alias: `config set db.default_path` falha com exit 1 e orienta a usar `db.path`.

## Padrões de Integração
### Compondo com pipelines e ferramentas Unix
```bash
sqlite-graphrag recall "testes auth" --k 5 --json | jaq -r '.results[].name'
```
### Alimente busca híbrida em endpoint sumarizador
```bash
sqlite-graphrag hybrid-search "migração postgres" --k 10 --json \
  | jaq -c '.results[] | {name, combined_score}' \
  | xh POST http://localhost:8080/summarize
```
### Backup com snapshot atômico e compressão
```bash
sqlite-graphrag sync-safe-copy --dest /tmp/ng.sqlite
ouch compress /tmp/ng.sqlite /tmp/ng-$(date +%Y%m%d).tar.zst
```
### Exemplo de subprocesso no Claude Code em Node
```javascript
const { spawn } = require('child_process');
const proc = spawn('sqlite-graphrag', ['recall', query, '--k', '5', '--json']);
```
### Build Docker Debian para pipelines de CI
```dockerfile
FROM rust:1.88-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo install --path .
```


## Códigos de Saída
### Status determinísticos para orquestração
| Código | Significado | Causa Possível |
| --- | --- | --- |
| `0` | Sucesso | Comando concluído e payload JSON impresso quando solicitado |
| `1` | Erro de validação ou falha em runtime | `--type` inválido, `--relation` malformado (vazio ou fora de snake_case), violação de kebab-case, erro genérico anyhow |
| `2` | Erro de uso da CLI | Flag inválida, argumento obrigatório ausente, timezone `--tz` inválido (Clap `FromStr` rejeita antes do código da aplicação) |
| `9` | Duplicata detectada | `--name` existente sem `--force-merge`; o `ingest` pula o arquivo e emite `status: "skipped"` com `action: "duplicate"` |
| `3` | Conflito durante atualização otimista | `edit` ou `restore` competiu com outro escritor |
| `4` | Memória ou entidade não encontrada | Alvo de `read`, `forget`, `edit`, `rename`, `restore` ou `graph traverse` ausente |
| `5` | Namespace não pôde ser resolvido | Sem `--namespace`, sem XDG `namespace.default`, sem padrão detectado |
| `6` | Payload excedeu limites configurados | `--name` maior que 80 bytes, body acima de `512000` bytes, mais de `512` chunks |
| `10` | Erro do banco SQLite | Arquivo corrompido, schema divergente, migração ausente |
| `11` | Geração de embedding falhou | Erro no subprocesso LLM ou falha ao carregar modelo |
| `12` | Extensão `sqlite-vec` falhou ao carregar | Extensão nativa ausente ou build do SQLite incompatível |
| `13` | Falha parcial em lote | `import`, `reindex` ou stdin batch com pelo menos um registro com falha |
| `14` | Erro de I/O do sistema de arquivos | Diretório de cache ou de banco sem permissão de escrita, diretório de destino `ingest` inexistente |
| `15` | Banco ocupado após tentativas | Contenção do WAL excedeu o orçamento de `with_busy_retry` |
| `20` | Erro interno ou de serialização JSON | Falha inesperada do serde ou violação de invariante |
| `75` | `EX_TEMPFAIL` lock timeout ou todos os slots ocupados | Cinco ou mais invocações concorrentes ou `flock` esperou mais de 300s |
| `77` | RAM disponível abaixo do mínimo | Menos de 2 GB de RAM livre detectados antes do load do modelo |
| `78` | Erro de configuração OpenRouter | `--embedding-backend openrouter` sem `--embedding-model`, ou chave OpenRouter inválida/ausente no XDG (OPENROUTER_API_KEY is not read at runtime) |


## Desempenho
### Medido em banco com 1000 memórias
- A latência de embedding é dominada pelo round-trip do LLM headless (~1-3 s por chamada em lote); leituras puras (`read`, `list`, `graph`) ficam em poucos milissegundos
- Desde a v1.0.79 as chamadas LLM são EM LOTE (bases de calibração de 8 chunks / 25 nomes de entidade em dim 64, adaptativas à dim — G44) e PARALELAS (`--llm-parallelism`, `Semaphore` + `JoinSet` limitados), então uma memória de 39 itens embeda em 4-5 chamadas em vez de 39 spawns serializados
- `--embedding-dim 1024` (o padrão desde a v1.2.0; era 384 de v1.0.94–v1.1.x) casa com modelos MRL modernos no OpenRouter; sob OpenRouter REST o truncamento MRL é no servidor a custo zero de token
- `init` não baixa modelo algum — apenas cria o banco e aplica migrações
- **Build:** cada chamada de embedding é uma requisição REST ao OpenRouter — RSS de ~350 MB por slot de worker (a carga de 1100 MB do modelo ONNX não existe mais em nenhum build)


## Requisitos de Memória
### Dimensionando RAM para cargas de ingest e recall
- A CLI em si é leve (binário de ~19 MiB); a RAM é dominada pelos subprocessos LLM com aproximadamente 350 MB de RSS por worker (`LLM_WORKER_RSS_MB`)
- Orçamento de workers: o paralelismo efetivo é `min(--llm-parallelism, cpus, ram_livre × 0.5 / 350 MB, 32)` — o portão de concorrência se adapta automaticamente à memória disponível
- O paralelismo padrão aumenta o RSS de forma quase linear por worker (`--llm-parallelism 4` ≈ 4 × 350 MB de RSS de subprocessos além da CLI)
- Modo de baixa memória: passe `--low-memory` (ou defina `SQLITE_GRAPHRAG_LOW_MEMORY=1`) para forçar ingest single-threaded. Equivale a `--ingest-parallelism 1` e sobrescreve qualquer valor explícito, ao custo de 3-4x mais tempo de relógio.
- Usuários de container/cgroup: orce `MemoryMax` para a CLI mais N × 350 MB de workers LLM (o antigo piso de 3 GB do ONNX não existe mais)


## Espaço em Disco
### Tamanho esperado do banco em relação ao conteúdo ingerido
> **Overhead esperado: aproximadamente 8× o tamanho total dos corpos ingeridos** (ex.: 7,6 MB de texto → ~62,9 MB de banco).
> O overhead vem dos embeddings float (**padrão 1024 dimensões desde a v1.2.0**; bancos pré-existentes mantêm a dimensionalidade gravada, ex.: 64/384), do índice FTS5 e do grafo de entidades/relacionamentos.
> Execute `sqlite-graphrag vacuum --json` após ciclos de `forget`+`purge` em massa para recuperar espaço.


## Invocação Paralela Segura
### Semáforo de contagem com até quatro slots simultâneos
- Cada worker LLM de embedding consome aproximadamente 350 MB de RSS — a unidade de orçamento do portão de concorrência desde a v1.0.79
- `MAX_CONCURRENT_CLI_INSTANCES` continua sendo o teto rígido de 4 subprocessos cooperantes
- Comandos pesados `init`, `remember`, `recall` e `hybrid-search` podem ser reduzidos dinamicamente para baixo desse teto quando a RAM disponível não sustenta o paralelismo com segurança
- Arquivos de lock em `~/.cache/sqlite-graphrag/cli-slot-{1..4}.lock` usando `flock`
- Uma quinta invocação aguarda até 300 segundos e então encerra com código 75
- Use `--max-concurrency N` para solicitar o limite de slots na invocação atual; comandos pesados ainda podem ser reduzidos automaticamente
- Memory guard aborta com saída 77 quando há menos de 2 GB de RAM disponível
- SIGINT e SIGTERM disparam shutdown graceful via atômica `shutdown_requested()`


## Solução de Problemas
### Segurança com cloud sync (Dropbox, iCloud, OneDrive)
- sqlite-graphrag usa modo WAL por padrão para escrita de alta concorrência
- Desde v1.0.54, todo comando de escrita executa `PRAGMA wal_checkpoint(TRUNCATE)` após commit (v1.0.53 cobriu 11 de 12; v1.0.54 adicionou o `prune-relations` faltante)
- Isso garante que o arquivo `.sqlite` esteja sempre autocontido quando ferramentas de cloud sync o leem
- Se ocorrer corrupção apesar do checkpoint, recupere com `sqlite3 corrompido.sqlite ".recover" | sqlite3 reparado.sqlite`

### Problemas comuns e correções
- O comportamento padrão sempre cria ou abre `graphrag.sqlite` no diretório atual
- Banco travado após crash exige `sqlite-graphrag vacuum` para fazer checkpoint do WAL
- `init` é quase instantâneo desde a v1.0.76 — não há download de modelo; se falhar, verifique o caminho do banco e as permissões
- Chamadas de embedding falhando com exit 11 normalmente indicam CLI LLM ausente, sem autenticação (OAuth obrigatório) ou timeout — aumente `SQLITE_GRAPHRAG_EMBED_TIMEOUT_SECS` (padrão 300) em links lentos
- A orientação sobre `ORT_DYLIB_PATH`/`libonnxruntime.so` é HISTÓRICA (≤ v1.0.75) — nenhum build carrega ONNX desde a v1.0.76
- Permissão negada no Linux indica falta de escrita no diretório de cache do usuário
- Detecção de namespace cai para `global` quando não há override explícito
- Invocações paralelas que excedem o limite seguro efetivo recebem saída 75 e DEVEM tentar com backoff; durante auditorias inicie comandos pesados com `--max-concurrency 1`


## Crates Rust Compatíveis
### Invoque sqlite-graphrag de qualquer framework Rust de IA via subprocesso
- Cada crate chama o binário via `std::process::Command` com a flag `--json`
- Nenhuma memória compartilhada ou FFI necessária: o contrato é JSON puro em stdout
- Fixe a versão do binário no `Cargo.toml` do workspace para builds reproduzíveis
- Todos os 18 crates abaixo funcionam identicamente em Linux, macOS Apple Silicon e Windows

### rig-core
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "project goals", "--k", "5", "--json"])
    .output().unwrap();
```

### swarms-rs
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["hybrid-search", "agent memory", "--k", "10", "--json"])
    .output().unwrap();
```

### autoagents
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["remember", "--name", "task-context", "--type", "project",
           "--description", "current sprint goal", "--body", "finish auth module"])
    .output().unwrap();
```

### graphbit
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "decision log", "--k", "3", "--json"])
    .output().unwrap();
```

### agentai
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["hybrid-search", "previous decisions", "--k", "5", "--json"])
    .output().unwrap();
```

### llm-agent-runtime
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "user preferences", "--k", "5", "--json"])
    .output().unwrap();
```

### anda
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["stats", "--json"])
    .output().unwrap();
```

### adk-rust
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "tool outputs", "--k", "5", "--json"])
    .output().unwrap();
```

### rs-graph-llm
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["hybrid-search", "graph relations", "--k", "10", "--json"])
    .output().unwrap();
```

### genai
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "model context", "--k", "5", "--json"])
    .output().unwrap();
```

### liter-llm
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["remember", "--name", "session-notes", "--type", "user",
           "--description", "resumo da sessão", "--body", "discutimos arquitetura"])
    .output().unwrap();
```

### llm-cascade
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "fallback context", "--k", "3", "--json"])
    .output().unwrap();
```

### async-openai
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "system prompt history", "--k", "5", "--json"])
    .output().unwrap();
```

### async-llm
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["hybrid-search", "chat context", "--k", "5", "--json"])
    .output().unwrap();
```

### anthropic-sdk
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "tool use patterns", "--k", "5", "--json"])
    .output().unwrap();
```

### ollama-rs
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "local model outputs", "--k", "5", "--json"])
    .output().unwrap();
```

### mistral-rs
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["hybrid-search", "inference context", "--k", "10", "--json"])
    .output().unwrap();
```

### llama-cpp-rs
```rust
use std::process::Command;
let out = Command::new("sqlite-graphrag")
    .args(["recall", "llama session context", "--k", "5", "--json"])
    .output().unwrap();
```


## Contribuindo
### Pull requests são bem-vindos
- Leia as diretrizes de contribuição em [CONTRIBUTING.md](CONTRIBUTING.md)
- Abra issues no repositório do GitHub para bugs ou pedidos de funcionalidade
- Siga o código de conduta descrito em [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)


## Segurança
### Política de divulgação responsável
- Reportes de segurança seguem a política descrita em [SECURITY.md](SECURITY.md)
- Contate o mantenedor em privado antes de divulgar vulnerabilidades publicamente


## JSON Schemas
### Contratos canônicos para cada resposta de subcomando
- JSON Schemas autoritativos para cada resposta `--json` ficam em [`docs/schemas/`](docs/schemas/) e são versionados junto com a crate
- 74 schemas cobrem `init`, `remember`, `remember-batch` (+ summary), `recall`, `hybrid-search`, `deep-research`, `list`, `read`, `forget`, `purge`, `rename`, `edit`, `history`, `restore`, `link`, `unlink`, `prune-relations`, `health`, `stats`, `migrate` (+ `migrate-rehash` + `migrate-to-llm-only`), `vacuum`, `optimize`, `cleanup-orphans`, `sync-safe-copy`, `backup`, `graph` (+ stats/traverse/entities), `related`, `namespace-detect`, `debug-schema`, `entities-input`, `relationships-input`, `ingest-file-event` (+ `ingest-summary`), `ingest-claude-phase` (+ file-event + summary), `export-memory-line` (+ summary), `enrich-phase` (+ item-event + summary), `fts rebuild` (+ `fts check` + `fts stats`), `vec orphan-list` (+ `vec purge-orphan` + `vec stats`), `error-envelope`
- Trate estes schemas como o contrato de agente; SKILL.md documenta as mesmas formas em formato humano
- Valide consumidores downstream com qualquer validador JSON Schema padrão (e.g. `ajv`, `jsonschema`)


## Histórico de Mudanças
### Histórico de releases mantido em arquivo separado
- Leia o histórico completo de releases em [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md)


## Agradecimentos
### Construído sobre excelente código aberto
- `fastembed` e `sqlite-vec` sustentaram o pipeline de embedding local até a v1.0.75 (removidos desde então — os embeddings agora vêm da API REST do OpenRouter)
- `refinery` executa migrações de schema com garantias transacionais
- `clap` potencializa o parsing de argumentos da CLI com macros derive
- `rusqlite` encapsula o SQLite com bindings Rust seguros e build embutido


## Licença
### Licença dual MIT OR Apache-2.0
- Licenciado sob Apache License 2.0 ou MIT License à sua escolha
- Veja `LICENSE-APACHE` e `LICENSE-MIT` na raiz do repositório para texto completo
