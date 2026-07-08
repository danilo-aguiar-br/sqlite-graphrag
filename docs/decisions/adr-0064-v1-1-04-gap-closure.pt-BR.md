# ADR-0064: v1.1.04 — Fechamento de Dois Gaps Estruturais (Panic de Nested-Runtime no deep-research, Convergência do entity-connect)

- Status: Accepted
- Data: 2026-07-08
- Release: v1.1.04 (crate `1.1.4`)
- Substitui: nenhum
- Substituído por: nenhum
- Relacionado: ADR-0060 (convergência do backlog de enrichment), ADR-0062 (fechamento de gaps v1.1.02), ADR-0063 (correções de bugs v1.1.03)


## Contexto

A v1.1.03 fechou seis bugs que bloqueavam operadores, mas deixou dois gaps estruturais registrados em `gaps.md`: GAP-001 e GAP-002.

- GAP-001: `deep-research` entra em panic 100% reproduzível com "Cannot start a runtime from within a runtime" porque seu entry point síncrono cria um runtime dedicado Tokio T1 e então chama o embedder que cria/adquire T2 na mesma thread.
- GAP-002: `entity-connect` nunca converge porque `count_operation_backlog` retorna zero fixo para `EntityConnect`, `scan_isolated_entity_pairs` faz um CROSS JOIN O(n²) sem marcador de par avaliado, e `--until-empty` reavalia para sempre os pares rejeitados como "none".
- Ambos os gaps bloqueiam capacidades centrais: GAP-001 torna `deep-research` inteiramente inutilizável.
- GAP-002 deixa ~11 079 entidades de grau 0 invisíveis à travessia multi-hop.
- GAP-002 também desperdiça custo de LLM em re-escaneamentos infinitos de pares já avaliados como "none".


## Decisão

- Aplicar Opção A (cirúrgica) E Opção B (defesa em profundidade) para GAP-001: extrair o loop de embedding por sub-query em um novo helper síncrono `compute_sub_embeddings` que executa ANTES de `Builder::new_multi_thread` em `deep_research::run`.
- Propagar o padrão canônico de reentrada `Handle::try_current` + `block_in_place` (já canônico em `embedder.rs:1435` e `extract/llm_embedding.rs:629`) para os três caminhos de embedding OpenRouter.
- Os três caminhos afetados são: single em ~1016, serial batch em ~1155, e JoinSet fan-out em ~1172.
- Propagar também o padrão de reentrada para `ingest_opencode`.
- Para GAP-002, introduzir uma correção de quatro partes:
- Parte 1: migração V016 criando a tabela `entity_connect_seen` que registra o veredito do LLM por par avaliado.
- Parte 2: tornar `scan_isolated_entity_pairs` ciente do seen via `LEFT JOIN entity_connect_seen` e priorizar entidades hub.
- Parte 3: substituir o braço `EntityConnect => 0` fixo em `count_operation_backlog` por um proxy real O(n) que conta entidades de grau 0 que possuem bindings NER.
- Parte 4: persistir o veredito em `call_entity_connect` nos dois ramos `related` e `none`.


## Consequências

- `deep-research` funciona em 100% das invocações; o contrato `--json` se mantém mesmo em falhas transitórias de embedding.
- `entity-connect --until-empty` converge; cada par avaliado custa LLM exatamente uma vez.
- `enrich --status` relata um backlog verdadeiro em vez de zero fixo.
- A travessia multi-hop agora alcança as entidades de grau 0 antes isoladas.
- O schema avança v15 para v16; `migrate --json` é OBRIGATÓRIO no upgrade (V016 é uma migração numerada, não um ALTER idempotente).
- A operação de enrich `entity-connect` é promovida de "scan-only" para "fully-implemented".
- Defesa em profundidade: qualquer subcomando futuro que crie seu próprio runtime antes de chamar o embedder não reativará GAP-001 porque os três caminhos de embedding agora usam o padrão seguro de reentrada.
- O schema da tabela `entity_connect_seen` é: `(source_id, target_id, namespace, verdict, relation, evaluated_at)`.
- A tabela carrega uma PK composta, FK dupla ON DELETE CASCADE para `entities(id)`, um CHECK em `verdict`, e um índice de namespace.
- `CURRENT_SCHEMA_VERSION` avança de 15 para 16.


## Validação

- `cargo build --release` — zero erros.
- As tarefas de implementação de GAP-001 e GAP-002 foram validadas com seus testes dedicados antes desta tarefa de docs.
- Nenhum teste novo foi executado nesta tarefa de docs (testes já validados nas tarefas de implementação).


## Commits

- Commits de implementação de GAP-001 (correção do nested-runtime do deep-research) e GAP-002 (convergência do entity-connect) (veja as tarefas de código).
- Este ADR + INDEX.md + gaps.md + schemas/README.md fecham o release v1.1.04.
