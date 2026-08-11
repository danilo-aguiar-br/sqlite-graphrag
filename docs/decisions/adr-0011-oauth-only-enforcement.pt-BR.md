# ADR-0011 — Imposição de OAuth-Only (v1.0.69)

- **Status.** Aceito.
- **Data.** 2026-06-05.
- **Decisores.** Danilo Aguiar (operador), Claude Code (consultor).
- **Supersede.** Nenhum.
- **Gaps relacionados.** G28-A (proliferação de MCP), gaps.md linhas 41-49 (Regras Invioláveis de Invocação Headless).

## Contexto

O fluxo original de `enrich` e `ingest --mode {claude-code,codex}` passava meia dúzia de flags de hardening, mas permitia dois caminhos PROIBIDOS:

1. A função `claude_runner::build_claude_command` tinha um ramo `if ANTHROPIC_API_KEY.is_ok() { cmd.arg("--bare") }`. Conforme gaps.md:49, `--bare` é PROIBIDA porque desabilita OAuth e exige `ANTHROPIC_API_KEY` (justamente o que o projeto proíbe).
2. A whitelist de `codex_spawn::build_codex_command` incluía explicitamente `OPENAI_API_KEY`, passando qualquer chave de API do ambiente diretamente ao filho. Conforme gaps.md:48, `OPENAI_API_KEY` é PROIBIDA no ambiente de spawn de qualquer `codex exec`.

Os dois caminhos de código haviam divergido; `ingest_claude.rs` e `claude_runner.rs` mantinham arrays `ENV_WHITELIST` duplicados, e `ingest_claude.rs:325` tinha o mesmo ramo proibido `if ANTHROPIC_API_KEY { --bare }`.

Uma releitura de gaps.md linhas 41-49 (as quatro PROIBIÇÕES ABSOLUTAS sobre invocação headless de Claude/Codex) e o prompt explícito do operador "é proibido usar claude code headless com api" revelaram a inconsistência. Três call-sites tiveram que ser alinhados e o caminho de chave de API teve que virar erro duro.

## Decisão

1. O guard OAuth-only é obrigatório em TODO helper de spawn. O guard retorna um `AppError::Validation` e um comando `/usr/bin/false` carregando um marcador `--oauth-only-violation-*` quando a variável de ambiente proibida está presente.
2. `ANTHROPIC_API_KEY` e `OPENAI_API_KEY` estão INTENCIONALMENTE AUSENTES das whitelists de `env_clear`. Defesa em profundidade: mesmo que uma refatoração futura mova o guard, a variável nunca alcança o filho.
3. A flag `--bare` foi REMOVIDA de todo código executável. Ela aparece somente na documentação que explica por que é proibida.
4. Todo helper de spawn sempre passa o conjunto canônico de flags de hardening documentado em gaps.md:201-208 (claude) e 233-238 (codex).
5. Quatro testes novos (`#[serial_test::serial(env)]`) validam o conjunto canônico de flags e o comportamento de aborto.

## Consequências

- Operadores que usam chaves de API (uma minoria pequena) devem migrar para OAuth. A mensagem de erro é acionável e aponta para o fluxo de login OAuth.
- Os quatro testes rodam no grupo serial `env` para evitar corridas no ambiente global. Aumento total no tempo de teste: 0,04s.
- O marcador `--oauth-only-violation-{anthropic,openai}-api-key-set` torna as falhas de spawn autodocumentadas nos logs de CI.
- Os arrays `ENV_WHITELIST` estão agora em dois lugares (claude + codex). Uma refatoração futura deveria extrair `whitelist_env_clear` para um helper compartilhado. Registrado como follow-up.

## Decisões Relacionadas

- ADR-0041 — Custom Provider Credential Preservation (v1.0.83).
  Este follow-up do ADR-0011 foi registrado em 2026-06-17 e RESOLVE
  o follow-up de extração de helper via `src/spawn/env_whitelist.rs`.
  O helper compartilhado `apply_env_whitelist(cmd, strict)` unifica
  os três spawners duplicados (`claude_runner`, `codex_spawn`,
  `ingest_claude`) e estende a whitelist para preservar as
  variáveis de provedor customizado (`ANTHROPIC_AUTH_TOKEN`,
  `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`,
  `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`,
  `OTEL_EXPORTER_OTLP_ENDPOINT`) mantendo intacto o guard
  OAuth-only deste ADR-0011. Os dois ADRs compõem: este
  rejeita `ANTHROPIC_API_KEY`/`OPENAI_API_KEY`; o ADR-0041
  preserva os tokens OAuth e os overrides de base-URL usados por
  provedores compatíveis com a Anthropic (Minimax, OpenRouter, AWS
  Bedrock, gateways corporativos).
- ADR-0025 — OAuth-Only Embedding (v1.0.76). Reafirma este
  ADR-0011 na camada `extract/llm_embedding.rs`.

## Alternativas Consideradas

- Manter o caminho de chave de API com um aviso. REJEITADA. gaps.md:47,48,49 são PROIBIÇÕES ABSOLUTAS. Avisos não satisfazem proibições absolutas.
- Ler o token OAuth do ambiente via `OAUTH_TOKEN`. REJEITADA. O Claude Code lê o token OAuth de `~/.claude/.credentials.json` (ou do keychain do SO); o Codex lê de `~/.codex/auth.json`. O fluxo OAuth não passa tokens pelo ambiente.

## Referências

- `src/commands/claude_runner.rs:222-303` (comando canônico e guard OAuth-only).
- `src/commands/codex_spawn.rs:205-279` (comando canônico, flag `-c mcp_servers='{}'`, guard OAuth-only).
- `src/commands/ingest_claude.rs:255-340` (helper de extração alinhado com `claude_runner`).
- `src/commands/claude_runner.rs:574-666` (quatro testes `#[serial_test::serial(env)]`).
- `src/commands/codex_spawn.rs:684-758` (quatro testes `#[serial_test::serial(env)]`).
- gaps.md linhas 41-49 (Regras Invioláveis).
- gaps.md linhas 201-208 (comando canônico do claude).
- gaps.md linhas 233-238 (comando canônico do codex).
