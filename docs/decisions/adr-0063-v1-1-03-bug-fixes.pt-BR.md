# ADR-0063: v1.1.03 — Seis Bugs + split-body (Onda de Correção de Bugs)

- **Status**: Aceito
- **Data**: 2026-07-07
- **Release**: v1.1.03 (crate `1.1.3`)
- **Substitui**: nenhum
- **Substituído por**: nenhum
- **Relacionado**: ADR-0061 (roadmap de 12 prioridades da v1.1.01), ADR-0062 (fechamento de gaps da v1.1.02), ADR-0058 (`--prune-dead-orphans`), ADR-0059 (remoção do `--max-entity-degree`)

## Contexto

Após a execução de remediação do GraphRAG em 2026-07-07, seis bugs que bloqueavam operadores e um gap cosmético (V8) foram catalogados em `gaps.md` contra o binário v1.1.2. Todos os sete foram implementados e validados na onda v1.1.03:

1. **Bug 1 — scan-enqueue do enrich parecia um deadlock.** Com aproximadamente 44k candidatos de entidade, a fase de scan emitia `{"phase":"scan","items_total":44163}` e nunca avançava para o drain. O congelamento aparente tinha raiz em claims `processing` obsoletos deixados por um `kill -9` anterior (Bug 4): o enqueue por item adquiria o lock de escrita WAL sob contenção e o loop não limitado esfomeava o scan. O caminho de scan-enqueue também era linha-a-linha, então grandes volumes de trabalho amplificavam a starvation do lock.

2. **Bug 2 — `reclassify-relation` não conseguia migrar arestas legadas com underscore.** A fronteira do clap normalizava TANTO `--literal-from` QUANTO `--to-relation` para kebab-case antes da guarda from==to, então `--literal-from applies_to --to-relation applies-to` colapsava em `applies-to == applies-to` e levantava exit 1. 61 357 arestas legadas (39 362 `applies_to`, 17 956 `depends_on`, 4 036 `tracked_in`) estavam inalcançáveis por qualquer caminho da CLI — bloqueando o gate final V5.

3. **Bug 3 — `merge-entities` não conseguia cruzar namespaces.** O caminho `--ids`/`--into-id` resolvia cada ID contra um único namespace resolvido, então uma entidade duplicada vivendo em `ai-sdd` não conseguia ser mergeada na sua gêmea `global`. 15 duplicatas cross-namespace bloqueavam o gate V6; deletar o lado `ai-sdd` é proibido pela regra do operador.

4. **Bug 4 — `kill -9` deixava três camadas de lock obsoleto.** O job do enrich mantém (1) o singleton file-lock, (2) o lock de escrita do SQLite via um descritor de arquivo aberto, e (3) claims `queue_processing` + `state:draining` no sidecar. `--force-job-singleton` limpava apenas a camada 1; as camadas 2 e 3 persistiam e produziam exit 75 ("job in progress") na próxima invocação e o estado falso-`draining` observado no congelamento de scan do Bug 1.

5. **Bug 5 — `queue_pending` era interpretado erroneamente como uma fila física.** Operadores liam `queue_pending: 47300` como 47 300 itens sentados numa fila física, mas o valor é uma CONTAGEM CALCULADA sobre o sidecar (contagens de status), não um conjunto de linhas para drenar. Isso produzia falsos alarmes ("deadlock!") em estados de cooldown (`eligible_now == 0`, itens estacionados em `next_retry_at`) e obscurecia o sinal real (`scan_backlog`).

6. **Bug 6 — `enrich re-embed --target chunks` pulava chunks órfãos.** O scanner de chunks usava `JOIN memories m ON m.id = c.memory_id WHERE m.deleted_at IS NULL`. Mães soft-deleted mantêm seus chunks (CASCADE só dispara em HARD delete), então esses chunks eram invisíveis ao scanner E ao `count_operation_backlog`, produzindo um falso `scan_backlog: 0` enquanto `health` reportava `vec_chunks_missing > 0` — uma dissonância que o operador não conseguia fechar.

7. **V8 — corpos superdimensionados (>25k chars) nunca eram divididos.** 777 corpos monstro permaneciam não-divididos. O impacto era cosmético (chunks eram embedados individualmente e permaneciam pesquisáveis), mas o gate final V8 não conseguia fechar sem um comando de divisão em nível de CLI.

## Decisão

### D1 — Bug 1: scan-enqueue transacional em lote

- O caminho de scan-enqueue escreve linhas candidatas numa única transação em lote em vez de inserções linha-a-linha sob o lock de escrita WAL.
- Isso remove a starvation de lock que, sob a contenção de claims obsoletos do Bug 4, se apresentava como uma fase de scan congelada para 44k entidades.
- A correção é puramente no loop de enqueue; os predicados de scan e a fase de drain permanecem inalterados.

**Link causal**: os claims `processing` obsoletos do Bug 4 são o gatilho; esta correção remove o amplificador (churn de lock por linha). O gatilho restante é tratado por D4.

### D2 — Bug 2: `--literal-to` para escrita verbatim do alvo

- Nova flag `reclassify-relation --literal-to <RELATION>` escreve o valor alvo VERBATIM (sem normalização kebab), complementando o `--literal-from` existente (match verbatim da fonte).
- A guarda from==to agora compara o literal cru do `--literal-from` contra o literal cru do `--literal-to`, então `--literal-from applies_to --literal-to applies-to` é a migração canônica de uma aresta legada com underscore para sua forma canônica com hífen.
- O runbook de migração é um comando por tipo de relação legada:

  ```
  reclassify-relation --literal-from applies_to --literal-to applies-to --batch --dry-run
  reclassify-relation --literal-from applies_to --literal-to applies-to --batch
  # repetir para depends_on e tracked_in
  ```

**Link causal**: isso desbloqueia V5 (61 357 arestas com underscore tornam-se alcançáveis para migração canônica).

### D3 — Bug 3: `merge-entities --cross-namespace` (opt-in)

- Nova flag `merge-entities --cross-namespace` opta pela resolução de ID cross-namespace.
- O comportamento padrão (sem flag) é inalterado: cada ID deve pertencer ao namespace resolvido — seguro por padrão, sem contaminação cross silenciosa.
- Com a flag, `--ids`/`--into-id` resolvem cada ID através de TODOS os namespaces; o merge mantém o namespace da entidade `--into-id` e re-pointa as arestas da entidade fonte.

**Link causal**: isso desbloqueia V6 (15 duplicatas cross-namespace podem ser mergeadas em `global`).

### D4 — Bug 4: coluna `claimed_at` + reset-no-startup + cleanup de SIGTERM + `--reset-stale-claims`

- A fila sidecar ganha uma coluna `claimed_at` (ALTER idempotente) para que um claim `processing` carregue um timestamp.
- Na inicialização do enrich, claims `processing` mais antigos que um threshold são resetados para `pending` automaticamente (sem intervenção manual para um reinício limpo após crash).
- Um handler de SIGTERM agora realiza cleanup gracioso (libera claims, checkpointa estado) antes do processo sair com exit 19 — então um término normal nunca deixa claims obsoletos.
- Nova flag `enrich --reset-stale-claims` realiza um reset manual de claims `processing` mais antigos que o threshold, para o operador que precisa limpar claims de um término não-gracioso sem um reinício completo.

**Links causais**: essa é a correção raiz para a contenção exit-15 e o gatilho do Bug 1. Com D4 em vigor, o batching de D1 roda numa fila cujos claims não estão obsoletos.

### D5 — Bug 5: esclarecer a semântica do campo `enrich --status`

- O doc-comment e o texto de ajuda da flag `--status` agora documentam explicitamente:
  - `scan_backlog` = candidatos que um scan fresco SELECIONARIA (trabalho pendente REAL, mesmo predicado WHERE dos scanners).
  - `queue_pending` = uma CONTAGEM CALCULADA sobre o sidecar, NÃO uma fila física de linhas para processar — permanece não-zero após um drain limpo.
  - `eligible_now == 0` com `queue_pending > 0` é COOLDOWN (backoff de rate-limit), NÃO um deadlock.
  - `eligible_now > 0` travado contra `state: "draining"` É um deadlock — rode `--reset-stale-claims`.
- Sem mudança de comportamento; o relatório já carregava esses campos. Esta é uma clarificação documental para que operadores parem de interpretar cooldown erroneamente como deadlock.

### D6 — Bug 6: LEFT JOIN no scanner de re-embed de chunks

- `scan_chunks_missing_embeddings` e `count_operation_backlog` (o predicado compartilhado) trocam de `JOIN memories m` para `LEFT JOIN memories m` com o filtro de namespace relaxado para `(m.namespace = ?1 OR m.id IS NULL)`.
- Chunks cuja mãe foi soft-deleted agora são selecionados para re-embed, reconciliando `enrich --status` (`scan_backlog`) com `health` (`vec_chunks_missing`).
- A cobertura atinge um 100% real em vez de uma dissonante <100%.

**Link causal**: isso remove a dissonância health-vs-status e permite que `--until-empty` convirja sobre o backlog REAL.

### D7 — V8: subcomando `split-body`

- Novo subcomando `split-body --name <N>` divide uma memória cujo corpo excede 25 000 caracteres em memórias filhas nos limites de chunk.
- O modo padrão divide uma única memória nomeada; `--batch --threshold 25000` itera sobre cada memória acima do threshold.
- A memória original é marcada `SUPERCEDIDO` e relações `replaces` são criadas de cada filha para a original (então o histórico é preservado e o recall pode atravessar a linhagem).
- Filhas NÃO são embedadas inline pelo `split-body`; o operador DEVE rodar `enrich --operation re-embed --target memories` depois para backfillar os vetores filhos.

**Link causal**: isso fecha o gap cosmético V8 (777 corpos superdimensionados) sem bloquear a busca (chunks já estavam embedados).

## Consequências

- **Positivo**:
  - O scan de 44k entidades não se apresenta mais como deadlock; o enqueue em lote completa em segundos.
  - As 61 357 arestas legadas com underscore (V5) e as 15 duplicatas cross-namespace (V6) tornam-se alcançáveis via CLI.
  - Um SIGTERM normal não deixa mais claims obsoletos; um anormal é recuperável com `--reset-stale-claims` em vez de caçar PIDs.
  - `enrich --status` e `health` concordam sobre a cobertura de chunks; operadores param de perseguir um <100% fantasma.
  - Corpos superdimensionados V8 podem ser divididos, fechando o gate final.
- **Negativo**:
  - `merge-entities --cross-namespace` é uma flag de power-user: uso indevido pode mergear entidades que compartilham um nome através de namespaces não-relacionados. O padrão opt-in (somente mesmo namespace) é a mitigação.
  - `split-body` cria memórias filhas que exigem um `re-embed --target memories` subsequente; um operador que pule o re-embed deixa filhas sem vetores até a próxima varredura do enrich.
- **Neutro**:
  - Schema permanece em v15; a única mudança no sidecar é a coluna `claimed_at` idempotente na fila do enrich.

## Validação

- `cargo build --release` — zero erros.
- Todas as cinco tarefas de código (Bugs 1, 2, 3, 4, 6 + V8) validadas com seus testes dedicados antes desta tarefa de docs.
- Nenhum teste novo executado nesta tarefa de docs (testes já validados nas tarefas de implementação).

## Commits

- Commits de implementação dos Bugs 1–4 + 6 + V8 (ver as cinco tarefas de código).
- Este ADR + CHANGELOG + bump do Cargo.toml + gaps.md + alinhamento do SKILL fecham a release v1.1.03.
