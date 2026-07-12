# ADR-0066: v1.1.06 — Scan O(k) do entity-connect (GAP-ENTITY-CONNECT-SCAN-CARTESIAN)

- Status: Aceito
- Data: 2026-07-12
- Release: v1.1.06 (crate `1.1.6`)
- Supersede: nenhum
- Supersedido por: nenhum
- Relacionado: ADR-0064 (GAP-002 `entity_connect_seen` / convergência), ADR-0060, ADR-0055


## Contexto

A v1.1.04 fechou o GAP-002 para o `entity-connect` **convergir** via `entity_connect_seen` (V016). Isso **não** corrigiu o custo do scan de pares quando quase nenhum par havia sido visto.

Em namespaces `global` grandes (~96 209 entidades no incidente) `scan_isolated_entity_pairs` usava produto cartesiano de entidades com `ORDER BY` global; o SQLite materializava candidatos (TEMP B-TREE), a CLI ficava em ~100% CPU sem emitir `phase: "scan"`, o singleton de enrich travava o namespace e outras ops recebiam **exit 75**. O `--max-runtime` não cobria o primeiro scan. O drain reexecutava o scan a cada item.


## Decisão

1. Gerar pares por **coocorrência** + fill **hub × ilha** (nunca cartesiano + ORDER BY global).
2. Chaves de fila `pair:{id1}:{id2}`, `item_type = entity_pair`.
3. Drain por PK; sem re-scan.
4. Deadline + `InterruptHandle` no primeiro scan → Timeout exit 1.
5. `scan_start` antes do SQL; sem double-scan na 1ª iteração do `--until-empty`.
6. Sem migração de schema. GAP-002 preservado.


## Consequências

- Scans grandes terminam em tempo limitado; singleton não fica preso; drain O(1).
- Residual: bridges usa o mesmo path; chaves legadas são skipped.


## Validação

Testes unitários + `tests/v1106_entity_connect_scan_regression.rs` + smoke dry-run.
