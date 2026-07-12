# ADR-0066: v1.1.06 — Scan O(k) do entity-connect (GAP-ENTITY-CONNECT-SCAN-CARTESIAN)

- Status: Aceito
- Data: 2026-07-12
- Release: v1.1.06 (crate `1.1.6`)
- Supersede: nenhum
- Supersedido por: nenhum
- Relacionado: ADR-0064 (GAP-002 `entity_connect_seen` / convergência), ADR-0060 (convergência do backlog do enrich), ADR-0055 (`--until-empty` / `--max-runtime`)


## Contexto

A v1.1.04 fechou o GAP-002 para o `entity-connect` **convergir** via `entity_connect_seen` (V016). Isso **não** corrigiu o custo do primeiro (e de cada) scan de pares quando quase nenhum par havia sido visto.

Em namespaces `global` grandes (~96 209 entidades no incidente) `scan_isolated_entity_pairs` usava:

```sql
FROM entities e1, entities e2
… ORDER BY (SELECT COUNT(*) FROM memory_entities …) DESC
LIMIT 50
```

O SQLite materializava candidatos para um sort global (`USE TEMP B-TREE FOR ORDER BY`). O processo ficava em ~100% CPU com I/O próximo de zero, nunca emitia `phase: "scan"`, segurava o singleton do enrich e cascateava **exit 75** para outras ops. O `--max-runtime` só era checado depois do primeiro scan dentro de `--until-empty`. Além disso, `call_entity_connect` reexecutava o mesmo scan com `LIMIT 1` em cada item do drain, e a fila guardava só `e1.name` (pares ambíguos, `item_type=memory` incorreto).


## Decisão

1. **Substituir a geração cartesiana** por candidatos locais de evidência:
   - Primário: pares por coocorrência via self-join de `memory_entities` em `memory_id`.
   - Fill: hubs de maior grau × ilhas grau-0 com vínculos NER.
2. **Chaves de fila** `pair:{id1}:{id2}` (`id1 < id2`), `item_type = entity_pair`.
3. **Drain** resolve entidades por chave primária; nunca re-chama o scan de pares.
4. **Deadline** antes do primeiro scan: teto soft de 120s para ops de pares ∩ `--max-runtime`; watchdog `InterruptHandle`; `SQLITE_INTERRUPT` → `AppError::Timeout` (exit **1**).
5. **NDJSON** `phase: "scan_start"` antes do SQL; sem re-scan idêntico na 1ª iteração do `--until-empty`.
6. **Sem migração de schema** (V016 / `CURRENT_SCHEMA_VERSION` **16** inalterado). Semântica do GAP-002 preservada.


## Consequências

### Positivas

- Scans grandes de entity-connect no `global` terminam em tempo limitado.
- O singleton não fica preso por minutos de CPU pura sem progresso.
- Custo do drain é O(1) por item; workers paralelos não re-travam o DB.
- Operadores veem progresso `scan_start` / `scan` para hooks e agentes.

### Negativas / residuais

- `cross-domain-bridges` ainda compartilha o mesmo path de scan seguro (sem modelo semântico cross-domain separado).
- Linhas legadas da fila com nomes bare de entidade são skipped (re-scan enfileira chaves `pair:`).
- Coocorrência ainda usa GROUP BY no conjunto de co-pares (centenas de milhares em DBs densos); LIMIT + interrupt limitam o custo.
- O singleton ainda é adquirido antes do scan de pares (limitado pelo soft 120s / `--max-runtime`); não é retido para CPU cartesiana ilimitada.

### Follow-ups de auditoria fechados na mesma release

- `scan_start.operation` usa o nome CLI real em kebab-case (`entity-connect` **ou** `cross-domain-bridges`).
- Campos dual backlog: `backlog_degree0_proxy` vs `pairs_enqueued_this_scan` (também `entities_in_namespace` em `scan_start`).
- Testes unitários: nomes CLI das ops, mapeamento `SQLITE_INTERRUPT`, Timeout com deadline passado, `InterruptHandle` ao vivo.


## Validação

- Testes unitários em `scan.rs` / `queue.rs` (chaves de par, limit, exclusão de seen, item_type).
- `tests/v1106_entity_connect_scan_regression.rs` (fases de dry-run da CLI + chaves `pair:`).
- Smoke: `timeout 30 … enrich --operation entity-connect --dry-run` no `graphrag.sqlite` do projeto.
