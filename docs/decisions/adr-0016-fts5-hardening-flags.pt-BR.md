# ADR-0016 — Flags de Hardening do FTS5 (v1.0.69)

- **Status.** Aceito.
- **Data.** 2026-06-05.
- **Decisores.** Alice Martins (operador), Claude Code (consultor).
- **Supersede.** Nenhum.
- **Gaps relacionados.** G36 (`optimize` rebuilda FTS5 sem checar, sem progresso, sem dry-run).

## Contexto

`optimize` reconstruía o índice FTS5 incondicionalmente com `INSERT INTO fts_memories(fts_memories) VALUES('rebuild')`. Num banco de 4,3 GB o rebuild leva ~10 minutos de tempo de relógio mesmo quando o índice já está saudável. Operadores não tinham como saber se um rebuild era necessário, não tinham indicador de progresso durante o rebuild e não tinham modo dry-run para validação em CI.

O comando de rebuild do FTS5 é síncrono e não chama o progress handler do SQLite, então a única observabilidade disponível é um poll em background da contagem de linhas de `fts_memories`.

## Decisão

1. Pré-checar o FTS5 antes de reconstruir. `check_fts_functional` (já `pub` em `src/commands/fts.rs`) informa se o índice está saudável. O comportamento padrão é pular o rebuild quando o índice passa no integrity-check.
2. Adicionar `--no-fts-skip-when-functional` para forçar um rebuild mesmo quando o índice está saudável.
3. Adicionar `--fts-dry-run`. Quando definida, `optimize` roda `check_fts_functional` + `fts stats` e emite um `OptimizeResponse` com `status: "rebuild_recommended"` ou `"ok"`, saindo então com código 1 se um rebuild for recomendado.
4. Adicionar `--fts-progress <SECONDS>`. Quando definida com um inteiro positivo, uma thread em background abre uma conexão read-only SEPARADA (porque `rusqlite::Connection` não é `Send`) e emite uma linha `tracing::info!` com a contagem atual de linhas de `fts_memories` a cada N segundos. Default 30, 0 desabilita.
5. Adicionar `--yes` para pular qualquer prompt interativo futuro (atualmente reservada para compatibilidade adiante — nenhum prompt interativo existe ainda).
6. O `OptimizeResponse` expõe `fts_rebuilt`, `fts_skipped_functional`, `fts_unhealthy` e `fts_rows_indexed` (contagem de linhas observada) para observabilidade.

## Consequências

- Um índice FTS5 saudável não é mais reconstruído a cada chamada de `optimize`. A espera de 10 minutos vira um skip de 0,5 segundo.
- Operadores podem validar o estado do FTS5 em CI com `--fts-dry-run` e código de saída 1 como sinal não zero.
- Rebuilds longos emitem ao menos uma linha de progresso por intervalo de `--fts-progress`, então o operador consegue ver o trabalho de relógio acontecendo.
- 2 novos testes cobrem o dry-run e os campos da resposta; os testes existentes de `fts::check_fts_functional` permanecem inalterados.

## Alternativas Consideradas

- Usar `sqlite3_progress_handler` para progresso in-line. REJEITADO. O comando de rebuild do FTS5 não invoca o progress handler (confirmado por pesquisa com duckduckgo-search-cli e pela documentação do SQLite).
- Pular o rebuild quando o timestamp for recente. REJEITADO. A consulta `fts check` é a resposta autoritativa; uma heurística de timestamp estaria errada em casos de borda.
- Usar `fts5_test()` em vez de `fts check`. REJEITADO. `fts check` é um wrapper de nível mais alto que reporta um resultado estruturado; `fts5_test()` é um hook de C-API de nível mais baixo.

## Referências

- `src/commands/optimize.rs:36-67` (novas definições de flag).
- `src/commands/optimize.rs:105-110` (branch `--fts-dry-run`).
- `src/commands/optimize.rs:154-170` (thread em background de `--fts-progress` com `open_ro`).
- `src/commands/fts.rs:245-265` (`check_fts_functional`).
- gaps.md G36 linhas 1914-2010.
