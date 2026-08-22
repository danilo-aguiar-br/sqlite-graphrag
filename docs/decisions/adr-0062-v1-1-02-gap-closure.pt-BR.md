# ADR-0062: v1.1.02 — Dois Gaps Residuais Fechados (Remoção GLiNER, TooManyTokens Tipado) + Prune de Órfãos de Entidade + Teste de Regressão Re-Embed
- HISTÓRICO: este ADR é um registro histórico e descreve o produto no estado em que ele estava na data da decisão.
- HISTÓRICO: `IngestMode` não expõe mais `claude-code`, `codex` nem `opencode`: `ingest --mode` aceita hoje apenas `none`.
- HISTÓRICO: `--embedding-backend llm` também é recusado; os valores vivos são `openrouter` e `none`.

- **Status**: Aceito
- **Data**: 2026-07-06
- **Release**: v1.1.02 (crate `1.1.2`)
- **Substitui**: nenhum
- **Substituído por**: nenhum
- **Relacionado**: ADR-0058 (`--prune-dead-orphans`, chave de memória), ADR-0059 (precedente de remoção do `--max-entity-degree`), ADR-0061 (fechamento do roadmap de 12 prioridades da v1.1.01)

## Contexto

Após a v1.1.01 fechar o roadmap de 12 prioridades do `gaps.md` (ADR-0061), uma auditoria focada do `gaps.md` revelou três gaps residuais não cobertos pelo escopo da v1.1.01, além de um drift documental surgido durante a auditoria de preparação da release v1.1.02:

1. **Gap 1 — `--gliner-variant` persistia como no-op obsoleto.** Desde a v1.0.79 o pipeline ONNX do GLiNER havia sido removido, mas a flag clap `--gliner-variant`, o enum `GlinerVariant` e as env vars `SQLITE_GRAPHRAG_GLINER_MODEL`/`SQLITE_GRAPHRAG_GLINER_THRESHOLD` permaneciam no parser como no-ops que emitiam `tracing::warn!`. A documentação em `docs/`, raiz `*.md`, `llms*.txt` e os arquivos SKILL ainda os descreviam como "aceitos por compatibilidade" — um sinal falso de que ainda faziam algo.

2. **Gap 2 — `TooManyTokens` não tinha envelope tipado.** O exit 6 já era tipado para `BodyTooLarge{bytes,limit}` e `TooManyChunks{chunks,limit}` (P11 da v1.1.01), mas um corpo que excedesse o teto de tokens do modelo de embedding (≈32k tokens para `qwen/qwen3-embedding-8b`) colapsava num erro genérico de payload sem os campos `{tokens,limit}` no envelope JSON.

3. **Gap 3 — dispatch de re-embed com chave de entidade estava silenciosamente quebrado.** O dispatch `strip_prefix("entity:")` em `call_reembed` (`src/commands/enrich/extraction.rs`) roteia uma chave de fila para `call_reembed_entity` ou para o caminho de memória. O ramo existia no trunk mas não tinha teste de regressão — um refactor futuro que dropasse o `strip_prefix` silenciosamente enviaria todo re-embed entity-keyed para o caminho de memória (`QueryReturnedNoRows` → `NotFound` → dead-letter).

4. **Sub-gap — dead-letter entity-keyed não tinha comando de prune.** O ADR-0058 adicionou `enrich --prune-dead-orphans` mas seu predicado filtra apenas `item_type='memory'`. Linhas dead com chave de entidade (da acumulação histórica de 14 680 linhas sob v1.1.1) não tinham caminho dedicado de limpeza; operadores precisavam editar o sidecar SQLite à mão.

5. **Drift documental.** Os sete arquivos narrativos da raiz e a árvore `docs/` (AGENTS, HOW_TO_USE, MIGRATION, COOKBOOK, HEADLESS_INVOCATION, DOCUMENTATION_FRAMEWORK, TEST_PLAN, TESTING, `schemas/README.md`) declaravam `Current release: v1.1.01`, pin `=1.1.1`, User-Agent `sqlite-graphrag/1.1.1`, e descreviam `--gliner-variant` como no-op. A face pública de publicação da release (crates.io, docs.rs) publicaria docs desatualizados.

## Decisão

### 1. Gap 1 — REMOVER `--gliner-variant`, `GlinerVariant` e as env vars GLINER inteiramente

- Deletar o campo clap `--gliner-variant` de `RememberArgs` e `IngestArgs`.
- Deletar o enum `GlinerVariant`.
- Deletar as leituras das env vars `SQLITE_GRAPHRAG_GLINER_MODEL` e `SQLITE_GRAPHRAG_GLINER_THRESHOLD`.
- Clap agora rejeita `--gliner-variant` com exit 2, seguindo o precedente `--max-entity-degree` estabelecido pelo ADR-0059 na v1.0.99.
- Remover também `--mode gliner` do enum `IngestMode` — ele agora expõe apenas `none`, `claude-code`, `codex`, `opencode`. (Antes `gliner` era uma variante obsoleta que caía para URL-regex; callers devem usar `--mode none` + `--enable-ner` para extração URL-regex.)

**Trade-off**: BREAKING para qualquer script que ainda passe `--gliner-variant` ou `--mode gliner`. A mitigação é mecânica: `rg -- "--gliner-variant|--mode gliner" ci/ Makefile scripts/` e deletar as ocorrências. As env vars são silenciosamente ignoradas (sem erro) então pipelines de CI que as definem continuam rodando — apenas não têm efeito.

### 2. Gap 2 — Adicionar variante tipada `AppError::TooManyTokens{tokens,limit}`

- Nova variante de enum `AppError::TooManyTokens { tokens: usize, limit: usize }`.
- Exit 6 é preservado; o envelope JSON agora serializa `{tokens, limit}` para esta variante, ao lado dos já existentes `{bytes, limit}` (BodyTooLarge) e `{chunks, limit}` (TooManyChunks).
- A validação vive na borda: `remember.rs`, `edit.rs`, `remember_batch.rs` estimam a contagem de tokens antes do embedding e curto-circuitam com o erro tipado em vez de deixar o provedor rejeitar a requisição de forma opaca.

### 3. Gap 3 — Adicionar teste de regressão `tests/reembed_entities_integration.rs`

- Esqueleto idiomático espelha `tests/v1063_features.rs`: `assert_cmd::Command`, `serial_test::serial`, `tempfile::TempDir`, `#[path = "common/mod.rs"] mod common;`.
- **Arrange**: `init(&tmp)`; `remember --name m1 --type note --description d --body "..." --graph-stdin --llm-backend none` com payload curado declarando 2 entidades. Verificar `entities_persisted == 2`; abrir o DB via `rusqlite::Connection::open` e asserir `COUNT(*) FROM entities == 2` e `COUNT(*) FROM entity_embeddings == 0` (porque `--llm-backend none` produz vetor vazio e `upsert_entity_vec` pula a escrita).
- **Act**: `enrich --operation re-embed --target entities --mode claude-code --embedding-backend llm` — o mock `tests/mock-llm/claude` (injetado via `common::prepend_path`) devolve `{"embedding":[0.0; 64]}`. (`--mode openrouter` é evitado para manter o CI hermético — sem API key, sem rede.)
- **Assert**: reabrir o DB; `COUNT(*) FROM entity_embeddings == 2`; a query canônica de cobertura `SELECT COUNT(*) FROM entities e LEFT JOIN entity_embeddings ee ON ee.entity_id=e.id WHERE ee.entity_id IS NULL == 0`.
- **Idempotência**: uma segunda execução do enrich deixa `COUNT(*) FROM entity_embeddings == 2` (scan só elege linhas que ainda carecem de vetor; `upsert_entity_vec` faz DELETE+INSERT por `entity_id`).

### 4. Sub-gap — Nova flag `enrich --prune-dead-entity-orphans`

- `EnrichArgs` ganha `#[arg(long, conflicts_with = "prune_dead_orphans")] pub prune_dead_entity_orphans: bool`. `conflicts_with` é bidirecional no clap, então a declaração simétrica em `prune_dead_orphans` é implícita.
- Os arrays `required_unless_present_any` no gate do enrich são estendidos com `"prune_dead_entity_orphans"` (a flag é válida sem `--operation`/`--mode`, assim como `--prune-dead-orphans`).
- Nova `queue::prune_dead_entity_orphans(queue_conn, operation)`: SQL `DELETE FROM queue WHERE status='dead' AND item_type='entity' AND (operation=?1 OR operation IS NULL)`. Sem leitura do banco principal (linhas entity são entity-keyed; detecção de órfão contra a tabela `entities` está fora do escopo — a flag é para linhas dead, que por definição já falharam terminalmente). `PRAGMA wal_checkpoint(TRUNCATE)` roda quando `pruned > 0`.
- O handler emite o struct `DeadSummary` existente; o campo `action` discrimina (`"prune-dead-entity-orphans"` vs `"prune-dead-orphans"`).
- `src/cli.rs` `tolerates_missing_embedding_key` é estendido para que a flag não exija API key de embedding.
- Teste unitário `prune_dead_entity_orphans_removes_only_entity_dead_rows`: planta 3 linhas (`entity:foo` dead, `mem-dead` dead, `entity:bar` pending); asserts `pruned == 1`, a linha memory-dead sobrevive, a linha entity-pending sobrevive.
- Teste de integração `tests/prune_dead_entity_orphans_integration.rs`: planta entity-dead + memory-dead no sidecar, roda `enrich --operation re-embed --prune-dead-entity-orphans --json`, asserts `json["action"]=="prune-dead-entity-orphans"`, `json["pruned"]==1`, reabre o sidecar confirmando entity-dead removida e memory-dead preservada.

**Notas DRY/YAGNI**:
- A variante de memória (`prune_dead_orphans`) tem lógica extra que checa o banco principal pela existência da memória; a variante de entidade NÃO precisa dessa checagem (linhas dead-letter já são falhas terminais, não candidatas a re-embedding). Os dois predicados são intencionalmente NÃO unificados.
- Um campo `scope` no `DeadSummary` quebraria o contrato público `schemars::JsonSchema`; `action: &'static str` já discrimina.
- Prune chunk-keyed é YAGNI — não há acúmulo real e o caminho de re-embed para chunks é idempotente.

### 5. Estratégia de recuperação para as 14 680 linhas dead entity-keyed históricas

A nova flag DELETA as linhas da fila mas NÃO re-embeda as entidades. A sequência correta de operador após o upgrade é:

1. `enrich --operation re-embed --target entities --until-empty --max-runtime 600` — agora que o dispatch está corrigido (Gap 3), itens reprocessados persistem.
2. `enrich --operation re-embed --requeue-dead --ignore-backoff` — re-enfileirar as 14 680 linhas dead para mais uma tentativa.
3. Apenas os ÓRFÃOS verdadeiros (entidade-mãe deletada do banco principal) restarão — esses são os alvos de `--prune-dead-entity-orphans`.

### 6. Alinhamento documental

Os sete arquivos narrativos da raiz (README.md, README.pt-BR.md, llms.txt, llms.pt-BR.txt, llms-full.txt, INTEGRATIONS.md, INTEGRATIONS.pt-BR.md) foram alinhados para v1.1.02 no commit `d24b4aa`. Este ADR estende o mesmo alinhamento à árvore `docs/`.

## Consequências

- **Positivo**:
  - O parser não carrega mais peso morto do GLiNER; erros do clap são altos e cedo (exit 2) em vez de um `tracing::warn!` silencioso.
  - Exit 6 agora tem três variantes tipadas; callers podem ramificar no `error_class` do JSON para surfacar a remediação certa (encolher corpo vs dividir chunks vs truncar tokens).
  - O dispatch de re-embed entity-keyed é protegido por um teste de regressão; qualquer refactor futuro que drop o `strip_prefix` tornará o teste vermelho.
  - Operadores têm um prune dedicado e sidecar-only para linhas dead entity-keyed — sem edição manual do `.enrich-queue.sqlite`.
- **Negativo**:
  - BREAKING: scripts que passam `--gliner-variant` ou `--mode gliner` falham com exit 2. A correção é mecânica (deletar a flag).
  - O predicado `prune_dead_entity_orphans` não faz cross-check com a tabela principal `entities`; um operador que o rode num banco onde a entidade-mãe ainda existe mas a linha da fila está dead perderá a linha dead sem re-embedding. A mitigação é a sequência de recuperação documentada (re-embed primeiro, requeue-dead depois, prune apenas o que sobreviver).
- **Neutro**:
  - Schema permanece em v15 — sem migração.
  - A superfície do enum `IngestMode` encolhe; `gliner` sumiu do help de `--mode`.

## Validação

- `cargo check` exit 0.
- `cargo clippy --all-targets -- -D warnings` 0 warnings.
- `cargo fmt --check` 0 diffs.
- `cargo test --test reembed_entities_integration` verde (entity_embeddings 0→2, idempotente).
- `cargo test --test prune_dead_entity_orphans_integration` verde (`pruned==1`, entity-dead removida, memory-dead preservada).
- `cargo test --lib prune_dead_entity` verde (teste unitário de 3 linhas).
- `cargo doc --no-deps` 0 warnings (4 warnings rustdoc pré-existentes resolvidos).
- `enrich --help` exibe `--prune-dead-entity-orphans`; `remember --gliner-variant small` exit 2.

## Commits

- `4570acd` — Gap 1 (remover `--gliner-variant`/`GlinerVariant`/env vars GLINER) + Gap 2 (`AppError::TooManyTokens` tipado exit 6).
- `b73934b` — entradas de CHANGELOG.
- `b019531` — teste de regressão Gap 3 + flag `--prune-dead-entity-orphans` + testes unitário/integração.
- `a47b534` — 4 warnings rustdoc pré-existentes resolvidos.
- `d24b4aa` — alinhamento de docs da raiz (README, llms\*.txt, INTEGRATIONS).
- Este ADR + o alinhamento da árvore `docs/` fecham a release.
