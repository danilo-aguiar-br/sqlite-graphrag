# ADR-0017 — Tratamento de Órfãos em vec (v1.0.69)

- **Status.** Aceito.
- **Data.** 2026-06-05.
- **Decisores.** Alice Martins (operador), Claude Code (consultor).
- **Supersede.** Nenhum.
- **Gaps relacionados.** G39 (vec_memories_orphaned sem diagnóstico ou purga).

## Contexto

`health` reporta `vec_memories_orphaned: N` (linhas em `vec_memories` cujo `memory_id` não existe mais em `memories`), mas não oferece caminho de remediação. Os órfãos se acumulam ao longo de operações de `forget` (soft-delete) e `purge` (hard-delete) porque nenhuma delas remove a linha correspondente em `vec_memories`. O custo de 1 KB por vetor é pequeno por memória, mas ilimitado no agregado.

## Decisão

1. Adicionar uma nova família de subcomandos `vec` em `src/commands/vec.rs`:
   - `vec orphan-list --json` — lista cada órfão com `memory_id` e `vector_hash` (BLAKE3 do blob de embedding).
   - `vec purge-orphan --yes --dry-run` — deleta órfãos de `vec_memories`, `vec_entities` e `vec_chunks` numa única transação. A flag `--yes` é obrigatória para prevenir perda acidental; `--dry-run` prevê a contagem.
   - `vec stats --json` — reporta as contagens `vec_memories_rows`, `vec_entities_rows`, `vec_chunks_rows` e `orphaned`.
2. Adicionar um hook em `src/commands/forget.rs:88-99` que chama `memories::delete_vec(memory_id)` ANTES do soft-delete. Isso previne a formação de novos órfãos em regime permanente.
3. Adicionar um hook paralelo em `purge.rs` para o hard-delete.
4. O comando `vec purge-orphan` purga TRÊS tabelas: `vec_memories`, `vec_entities` e `vec_chunks`. A resposta reporta `deleted`, `deleted_entities` e `deleted_chunks`.

## Consequências

- O aviso do `health` torna-se acionável: operadores rodam `vec purge-orphan --yes` para zerar a métrica.
- Nenhum órfão novo se forma em regime permanente porque `forget` e `purge` agora removem os vetores.
- 3 testes unitários cobrem `vec_table_exists` (renomeado para evitar shadowing), o conjunto de campos de `vec stats` e o escopo transacional de `vec purge-orphan`.
- Operadores sem órfãos podem rodar `vec stats --json` como checagem de rotina; o campo `orphaned` deve ser 0.

## Alternativas Consideradas

- Remover `vec_memories_orphaned` do `health` em vez de adicionar uma correção. REJEITADO. A métrica é útil para pegar bugs em `forget`/`purge`; a correção é prevenir órfãos, não escondê-los.
- Rodar `vec purge-orphan` automaticamente no `optimize`. REJEITADO. `optimize` é majoritariamente de leitura; acoplá-lo a uma operação destrutiva surpreende operadores.
- Usar constraints `FOREIGN KEY` no SQLite para impor integridade referencial. REJEITADO. A imposição de FK no SQLite é opt-in e exigiria uma migração de schema que toca toda tabela `vec_*`.

## Referências

- `src/commands/vec.rs` (~430 linhas, 3 testes).
- `src/commands/forget.rs:88-99` (hook `delete_vec`).
- `src/commands/mod.rs:51` (`pub mod vec`).
- gaps.md G39 linhas 2179-2275.
