# ADR-007: Arquitetura da Política de Retry


## Status
- Aceito (2026-05-31)


## Contexto
- A CLI executa operações que podem falhar transitoriamente em três domínios
- Concorrência do SQLite (SQLITE_BUSY/SQLITE_LOCKED)
- Rate-limiting do LLM (subprocesso Claude Code / Codex retorna 429)
- Contenção de file-lock (semáforo de slots da CLI)
- Cada domínio tem características de latência distintas, exigindo políticas separadas
- A implementação anterior tinha lógica de retry duplicada em 4 arquivos sem centralização


## Decisão
### Infraestrutura
- Struct `RetryConfig` centralizada em `src/retry.rs` com construtores nomeados por domínio
- Fórmula de half-jitter: `delay = base/2 + fastrand::u64(0..base/2)` produzindo [base/2, base)
- Kill switch via variável de ambiente `SQLITE_GRAPHRAG_DISABLE_RETRY=1`
- Nenhum crate externo adotado

### Justificativa para Não Usar Crate Externo
- `backon` é apenas assíncrono (exige runtime tokio) — a CLI é síncrona
- O crate `backoff` adiciona dependências transitivas para 3 laços de retry simples
- A implementação total tem ~120 LOC com cobertura completa de testes
- Conforme rules §16 L778: "NUNCA reimplementar quando crate resolve" — exceção justificada: nenhum crate síncrono resolve sem overhead

### Políticas

| Domínio | Delay Base | Delay Máximo | Tentativas Máximas | Deadline | Jitter |
|--------|-----------|-----------|--------------|----------|--------|
| SQLite BUSY | 300ms | 4800ms | 5 | 30s | Half |
| Rate-limit de LLM | 60s | 900s | 20 | 3600s (1h) | Half |
| Cold-start | 2s | 4s | 2 | 30s | Nenhum |
| Poll de file-lock | 500ms | 2000ms | baseado em deadline | configurável | Progressivo |

### Observabilidade
- `tracing::debug` por tentativa com `attempt`, `delay_ms`, `error_kind`
- `tracing::error` na exaustão com tempo total decorrido
- Campos estruturados conforme §12 L619-654 de rules_rust_retry_com_backoff.md

### Classificação de Erros
- `is_retryable()` retorna true para: DbBusy, LockBusy, AllSlotsFull, LowMemory, RateLimited, Timeout
- `is_permanent()` retorna true para: Validation, BinaryNotFound, Duplicate, NotFound, NamespaceError, LimitExceeded, VecExtension
- Não classificados (Database, Io, Internal, Json): nem retryable nem permanentes — o chamador decide

### Kill Switch
- A variável de ambiente `SQLITE_GRAPHRAG_DISABLE_RETRY=1` desabilita todos os laços de retry imediatamente
- Verificada no topo de cada iteração do laço de retry
- Registra `tracing::warn` quando ativa para garantir visibilidade na telemetria
- Caso de uso: resposta emergencial a incidente para evitar tempestades de retry


## Consequências
- Todo comportamento de retry é documentado e auditável
- Mudanças de política exigem apenas modificar os construtores de `RetryConfig` em `src/retry.rs`
- O kill switch permite desabilitar instantaneamente durante incidentes sem reiniciar o processo
- Half-jitter evita thundering herd em cenários de workers paralelos
- O deadline total evita bloqueio indefinido em rate-limiting persistente
