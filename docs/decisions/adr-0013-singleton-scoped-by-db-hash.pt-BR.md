# ADR-0013 — Singleton Escopado por `db_hash` (v1.0.69)

- **Status.** Aceito.
- **Data.** 2026-06-05.
- **Decisores.** Danilo Aguiar (operador), Claude Code (consultor).
- **Supersede.** Nenhum.
- **Gaps relacionados.** G30 (singleton global ignorando `--db`), G09 (sinalização de wait).

## Contexto

`lock::acquire_job_singleton(JobType, namespace, wait_seconds)` gravava o arquivo de lock em `ProjectDirs::cache_dir()` — um caminho compartilhado por todo banco de dados que o usuário toca. Duas invocações concorrentes de `enrich` contra bancos DIFERENTES (`SQLITE_GRAPHRAG_DB_PATH=/tmp/a.sqlite` e `/tmp/b.sqlite`) colidiam, retornando `AppError::JobSingletonLocked` mesmo com os dois processos operando sobre recursos disjuntos. A mensagem de erro citava uma flag `--wait-job-singleton` que não existia na CLI, levando operadores a fazer `pkill` e remover o arquivo de lock na mão.

## Decisão

1. O caminho do arquivo de lock ganha um sufixo `db_hash`: `job-singleton-{tag}-{namespace_slug}-{db_hash}.lock`. O `db_hash` são os primeiros 12 caracteres hexadecimais de `blake3(canonicalize(db_path))`.
2. `db_path_hash` é `pub` para que chamadores possam calcular o hash sem adquirir o lock.
3. `acquire_job_singleton` ganha os parâmetros `db_path: &Path` e `force: bool`. `force: true` quebra um lock obsoleto de uma invocação que travou anteriormente.
4. A CLI expõe `--wait-job-singleton <SECONDS>` (aguardar o lock por polling) e `--force-job-singleton` (quebrar um lock obsoleto) em `enrich` e `ingest`. A mensagem de erro agora referencia a flag real.
5. `--wait-lock` (já presente) é mantida para `--max-concurrency` (slots de semáforo), distinta de `--wait-job-singleton` (espera do arquivo de lock). A tabela `after_long_help` lista ambas com descrições de uma linha.

## Consequências

- Duas invocações concorrentes de `enrich` contra bancos diferentes não colidem mais. O mesmo banco continua serializando.
- O `db_hash` é determinístico para um dado caminho canônico. Renomear um arquivo de banco invalida o lock automaticamente.
- 6 testes unitários cobrem sanitização de namespace, bloqueio da segunda invocação, isolamento por namespace, determinismo do db_hash, divergência do db_hash e comportamento da flag force.
- Operadores que se recuperam de uma invocação travada usam `--force-job-singleton` em vez de `flock -u` ou `pkill`.

## Alternativas Consideradas

- Usar um lock por PID em `XDG_RUNTIME_DIR`. REJEITADO. PIDs não são estáveis entre travamentos e não sobrevivem a reboots.
- Usar uma tabela SQLite para o lock. REJEITADO. O banco sendo travado é exatamente o recurso que não podemos acessar de forma confiável.
- Fazer o hash do caminho canônico com SHA-256 em vez de BLAKE3. REJEITADO. BLAKE3 já é dependência do projeto e é mais rápido.

## Referências

- `src/lock.rs:74-86` (legado `cache_dir()`).
- `src/lock.rs:92` (`db_path_hash`).
- `src/lock.rs:93-129` (`job_singleton_path`).
- `src/lock.rs:204-280` (`acquire_job_singleton`).
- `src/commands/enrich.rs:986`, `ingest_claude.rs:580`, `ingest_codex.rs:621` (call-sites passam `args.db`).
- `src/commands/ingest.rs:262-269` (flags `--wait-job-singleton` e `--force-job-singleton`).
- gaps.md G30 linhas 1325-1441.
