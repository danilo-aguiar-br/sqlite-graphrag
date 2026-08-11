# ADR-008: Singleton de Job por Namespace para Comandos Pesados Movidos a LLM

## Status
- Aceito (2026-06-03, v1.0.68)

## Contexto
- Os comandos `enrich`, `ingest --mode claude-code` e `ingest --mode codex` cada um spawnam um subprocesso `claude -p` (ou `codex exec`) por item processado.
- O desenho anterior compartilhava um semáforo contador de 4 slots (`MAX_CONCURRENT_CLI_INSTANCES = 4` em `src/constants.rs:341`) entre todos os comandos da CLI, o que significa que duas invocações paralelas de `enrich` no mesmo banco conseguiriam ambas adquirir slots.
- Combinado com `--llm-parallelism` (padrão 1, máximo 32) e os típicos 8-10 servidores MCP configurados por usuário, uma única invocação de `enrich` podia spawnar 16-20 processos filhos; quatro invocações paralelas × 2 workers × 10 servidores MCP = ~160-192 processos, saturando um host de 10 CPUs até load average 276 (incidente real em 2026-06-03).
- A infraestrutura existente `try_acquire_slot` / `try_lock_exclusive` sobre arquivos `cli-slot-{N}.lock` já estava em produção e testada em campo; estendê-la para um tipo de lock diferente foi direto.

## Decisão
### Arquitetura
- Introduzir o enum `JobType` em `src/lock.rs:43` com três variantes: `Enrich`, `IngestClaudeCode`, `IngestCodex`.  Comandos leves (`recall`, `stats`, `read`, `list`) intencionalmente NÃO têm variantes — continuam usando o semáforo contador existente.
- A nova função `acquire_job_singleton(job_type, namespace, wait_seconds)` adquire um arquivo `job-singleton-{tag}-{namespace}.lock` (NÃO um dos 4 slots contadores).  O lock é por `(job_type, namespace)`, então dois namespaces podem rodar jobs independentes.
- O `File` retornado DEVE ser mantido vivo durante toda a duração do comando; descartá-lo libera o singleton para a próxima invocação.
- Quando o singleton está retido por outra invocação, retornar `AppError::JobSingletonLocked { job_type, namespace }` (exit 75, classificado como retryable) imediatamente, OU fazer polling a cada `JOB_SINGLETON_POLL_INTERVAL_MS` (1000ms) até o deadline de espera expirar.

### Integração dos Chamadores
- `enrich::run` (`src/commands/enrich.rs:986`) adquire `JobType::Enrich` imediatamente após a resolução do namespace.
- `ingest_claude::run_claude_ingest` (`src/commands/ingest_claude.rs:580`) adquire `JobType::IngestClaudeCode`.
- `ingest_codex::run_codex_ingest` (`src/commands/ingest_codex.rs:621`) adquire `JobType::IngestCodex`.
- As três aquisições são a PRIMEIRA operação após a resolução do namespace, então o singleton é retido antes de qualquer I/O caro (carga de modelo, varreduras do banco de fila).

### Schema de Erro
- Nova variante `AppError::JobSingletonLocked { job_type, namespace }` em `src/errors.rs:127`.
- Mapeada para o exit code 75 (`CLI_LOCK_EXIT_CODE`) — o mesmo código usado pela variante `AllSlotsFull` do semáforo contador existente, então código de tratamento de erro que já trata 75 como caso especial continua funcionando.
- Classificada como retryable em `is_retryable()`.
- Localizada em `src/i18n.rs` com `pt::job_singleton_locked(job_type, namespace)`.

### Saneamento do Namespace
- O caminho do arquivo de lock usa um slug kebab-case do namespace (`a-z`, `0-9`, `-`, `_`); qualquer outro caractere é substituído por `-` e o resultado é convertido para minúsculas.  Namespaces vazios usam `default` como padrão.
- Isso evita injeção de caminho por um namespace contendo `/` ou `..`.

## Consequências
- Duas invocações paralelas de `enrich` no mesmo namespace agora falham rápido com exit 75 em vez de empilhar.
- Um `enrich` de longa duração (ex.: 2.321 entidades × 12,5s = 8 horas serial) não pode ser duplicado acidentalmente por um operador que reexecuta o comando.
- O CI não precisa impor comportamento de instância única — o binário faz isso em runtime.
- Operadores que querem paralelizar entre bancos diferentes (ou namespaces diferentes do mesmo banco) ainda podem fazê-lo via a flag `--namespace`.
- O singleton é por `job_type`, então `enrich` e `ingest --mode claude-code` podem rodar em paralelo contra o mesmo banco sem interferir (árvores de processo diferentes, orçamentos de custo diferentes).

## Alternativas Consideradas
- **Limitar `--llm-parallelism` a 1 por padrão** — considerado, mas não endereça o problema entre invocações e desaceleraria silenciosamente operadores que querem usar o paralelismo.
- **Lock global de processo** — bloquearia TODOS os comandos, não só os pesados, quebrando o semáforo existente da CLI.
- **Lock de escrita do SQLite em nível de banco** — bloquearia `remember` e outros comandos de escrita também; o singleton é mais direcionado.
- **Reusar o semáforo contador com peso de custo maior** — seria confuso; usuários teriam que saber que "1 enrich = 4 slots" sem sinal óbvio.

## Referências
- Relatório de gap: `gaps.md#G28`
- Implementação: `src/lock.rs:43` (JobType), `src/lock.rs:204` (acquire_job_singleton), `src/commands/enrich.rs:986`, `src/commands/ingest_claude.rs:580`, `src/commands/ingest_codex.rs:621`
- Cobertura de testes: 3 testes unitários em `src/lock.rs::tests` (saneamento de caminho, bloqueio da segunda invocação, isolamento por namespace)
- Documentação: `docs/AGENTS.md#new-in-v1.0.68`, `docs/HOW_TO_USE.md#capping-process-proliferation`, `docs/COOKBOOK.md#how-to-cap-process-proliferation`
