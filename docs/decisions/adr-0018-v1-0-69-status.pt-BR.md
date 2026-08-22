# ADR-0018 — Status de Fechamento v1.0.69 (2026-06-05)
- HISTÓRICO: este ADR é um registro histórico e descreve o produto no estado em que ele estava na data da decisão.
- HISTÓRICO: ele cita o subcomando `codex-models` e a flag `--fallback-mode`, e o binário v1.2.8 não tem nenhum dos dois.
- HISTÓRICO: os dois foram removidos sem substituto, enquanto `enrich --preflight-check` e `enrich --rate-limit-buffer` continuam vivos.

- **Status.** Aceito.
- **Data.** 2026-06-05.
- **Decisores.** Alice Martins (operador), Claude Code (consultor).
- **Supersede.** Nenhum.
- **Relacionado.** ADR-0011, ADR-0012, ADR-0013, ADR-0014, ADR-0015, ADR-0016, ADR-0017.

## Contexto

A release v1.0.69 fecha 12 gaps documentados em `gaps.md` (G28 até G39). Cada gap tem seu próprio ADR (0011-0017 mais 0008-0010 herdados da v1.0.68). Este ADR é o sumário executivo que o operador lê PRIMEIRO para confirmar que a release está pronta para publicação.

## Decisão

### Matriz de fechamento de gaps

| Gap | Severidade | Decisão | ADR |
| --- | --- | --- | --- |
| G28 (CRÍTICO) | Proliferação de subprocessos | A: 7 flags de hardening + B: singleton por db_hash + C: SIGTERM no timeout + D: system_load + CircuitBreaker + reaper | ADR-0011 |
| G29 | CHECK constraint + auditoria + preservação | Enum MemorySource + trilha de auditoria + gate Jaccard + idempotência blake3 + scripts/legacy | ADR-0012, ADR-0015 |
| G30 | Singleton ignora `--db` | db_hash BLAKE3 + --wait-job-singleton + --force-job-singleton | ADR-0013 |
| G31 | 5 flags de hardening ausentes em `enrich --mode codex` | codex_spawn helper unificado | ADR-0014 |
| G32 | Parser JSON errado em `enrich --mode codex` | parse_codex_jsonl compartilhado | ADR-0014 |
| G33 | Sem validação de modelo contra a whitelist OAuth | validate_codex_model + subcomando codex-models | ADR-0014 |
| G34 | Aviso de worker ignora o mode | match args.mode | (inline em `enrich.rs:1502`) |
| G35 | Sem preflight ou fallback para rate limit | --preflight-check, --fallback-mode, --rate-limit-buffer | (inline em `enrich.rs:653-749`) |
| G36 | Rebuild incondicional do FTS5 | --fts-dry-run, --fts-progress, --yes | ADR-0016 |
| G37 | Sem --names / --names-file | delimitado por vírgula + baseado em arquivo | (inline em `enrich.rs`) |
| G38 | Passo de backup pequeno demais | defaults 1000/5ms + 4 novas flags | (inline em `backup.rs:20-22`) |
| G39 | vec_memories_orphaned sem remediação | vec orphan-list + purge-orphan + stats + hook no forget | ADR-0017 |

### Imposição OAuth-Only (MUDANÇA DE COMPORTAMENTO)

O ADR-0011 documenta a mudança mais consequente da v1.0.69: o spawn de `claude -p` e `codex exec` agora ABORTA quando `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` está definida no ambiente. A flag `--bare` (que exige uma API key e desabilita OAuth) foi REMOVIDA de todo código executável. A whitelist de variáveis exclui ambas as variáveis de API key como defesa em profundidade.

Operadores que usam API keys DEVEM migrar para OAuth. A mensagem de erro é acionável e aponta para o fluxo de login OAuth.

### Delta na contagem de testes

- v1.0.68: 692 testes.
- v1.0.69: 745 testes (+53).
- Adições notáveis: 11 em `codex_spawn`, 10 em `preservation`, 8 em `memory_source`, 5 em `vec`, 4 de conformidade OAuth-only, 4 de reaper, 5 de system_load, 6 de lock.

### Documentação

- 7 novos ADRs (0011-0017) documentam cada decisão arquitetural.
- `gaps.md` é a fonte canônica de verdade sobre o que estava errado; este ADR é a fonte canônica de verdade sobre o que foi corrigido.
- `CHANGELOG.md` (EN) e `CHANGELOG.pt-BR.md` (PT) listam cada mudança.
- `AGENTS.md` (EN) e `AGENTS.pt-BR.md` (PT) incluem a seção v1.0.69 E corrigem a linha obsoleta "API keys are optional" na Nota de Autenticação.

## Consequências

- A release v1.0.69 está feature-complete e segura para publicar.
- Operadores rodando v1.0.68 que dependiam de API keys verão um erro de `Validation` e um caminho claro de migração. A migração é o login OAuth, que a documentação da v1.0.68 já descrevia.
- A contagem de 745 testes é o piso para a v1.0.70; qualquer regressão que derrube os testes abaixo de 745 deve ser corrigida em hotfix.
- Os 7 ADRs e o `gaps.md` juntos formam a trilha de auditoria. Mantenedores futuros conseguem reconstruir cada decisão lendo-os em ordem.

## Referências

- `gaps.md` (2424 linhas, 12 gaps, histórico completo).
- Seções v1.0.69 de `CHANGELOG.md` e `CHANGELOG.pt-BR.md`.
- Seções v1.0.69 de `docs/AGENTS.md` e `docs/AGENTS.pt-BR.md`.
- `docs/decisions/adr-0008-0018*.md` (8 herdados + 7 novos ADRs).
- `src/commands/claude_runner.rs:574-666` (4 testes OAuth-only).
- `src/commands/codex_spawn.rs:684-758` (4 testes OAuth-only).
- `src/commands/optimize.rs:36-67` (3 novas flags de FTS5).
- `src/commands/vec.rs` (~430 linhas, 3 testes).
- `src/preservation.rs` (10 testes).
- `src/memory_source.rs` (8 testes).
- `src/reaper.rs` (4 testes).
- `src/system_load.rs` (5 testes).
