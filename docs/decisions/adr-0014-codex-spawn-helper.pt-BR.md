# ADR-0014 — Helper Unificado `codex_spawn` (v1.0.69)

- **Status.** Aceito.
- **Data.** 2026-06-05.
- **Decisores.** Danilo Aguiar (operador), Claude Code (consultor).
- **Supersede.** Nenhum.
- **Gaps relacionados.** G31 (flags ausentes em `enrich`), G32 (parser JSONL ingênuo), G33 (validação de modelo ausente).

## Contexto

`enrich --mode codex` e `ingest --mode codex` haviam divergido em três dimensões independentes:

1. **Flags de spawn.** `ingest_codex.rs:320-329` passava sete flags de hardening (`--json --output-schema --ephemeral --skip-git-repo-check --sandbox read-only --ignore-user-config --ignore-rules`). `enrich.rs:2773-2780` passava apenas três. O operador mantinha um wrapper externo em `~/.local/bin/codex-clean` para injetar as flags faltantes.
2. **Parser JSONL.** `ingest_codex.rs:430-540` implementava um `parse_codex_output` linha a linha adequado. `enrich.rs:2846-2850` usava `serde_json::from_str` sobre o stdout cru, que sempre falhava com `trailing characters at line 2 column 1`.
3. **Validação de modelo.** Nenhum dos call-sites validava `--codex-model` contra a whitelist OAuth do ChatGPT Pro; a rejeição vinha do próprio Codex depois de um turno OAuth desperdiçado.

Um script wrapper resolvia o problema imediato, mas multiplicava a superfície de configuração e escondia a correção real do codebase.

## Decisão

1. Extrair o pipeline de spawn para `src/commands/codex_spawn.rs` com `pub struct CodexSpawnArgs { binary, schema_path, model, timeout, sandbox_mode }` e `pub fn build_codex_command(args) -> Command`. A função SEMPRE passa as sete flags canônicas mais `-c mcp_servers='{}'` (hardening OAuth-only de gaps.md:234) e `--ask-for-approval never`.
2. Extrair o parser JSONL para `pub fn parse_codex_jsonl(stdout: &str) -> Result<(ExtractionResult, Usage), AppError>`. Ambos os call-sites consomem o mesmo parser.
3. Adicionar `validate_codex_model(model)`, `list_codex_models()` e `suggest_codex_model(query)` contra a whitelist OAuth do ChatGPT Pro (`codex-auto-review`, `gpt-5.3-codex-spark`, `gpt-5.4`, `gpt-5.4-mini`, `gpt-5.5`). A validação roda ANTES do subprocesso ser criado.
4. Mover o caminho do JSON de schema de `std::env::temp_dir()` para `paths::AppPaths::cache_dir().join("schemas")` para que sobreviva a reboots e viva num diretório confiável.
5. Expor a lista de modelos via um novo subcomando de topo `codex-models --json` para que operadores possam inspecionar sem criar um processo Codex.

## Consequências

- O wrapper externo `~/.local/bin/codex-clean` torna-se legado. Operadores podem `rm` depois de atualizar.
- Ambos os call-sites têm defaults IDÊNTICOS; hardening futuro chega num único lugar.
- 11 testes unitários cobrem casos de borda do parser (JSONL multi-linha, linhas malformadas, detecção de rate limit), validação de modelo (válido, inválido, vazio, alias customizado, match fuzzy) e presença das flags do comando.
- O caminho do schema agora é persistente no diretório de cache; depurar fica mais fácil porque o arquivo sobrevive entre execuções.

## Alternativas Consideradas

- Corrigir a divergência no lugar sem extrair um helper. REJEITADO. A duplicação reemergiria na próxima vez que um dos lados fosse atualizado.
- Usar uma biblioteca JSON-LD para o parsing JSONL. REJEITADO. `codex exec --json` emite JSON delimitado por newline, não JSON-LD.
- Tornar a lista de modelos uma consulta dinâmica contra a CLI do Codex. REJEITADO. A CLI não expõe um comando `list-models`; a whitelist estática espelha o conjunto aceito pelo provedor OAuth.

## Referências

- `src/commands/codex_spawn.rs` (~700 linhas, 11 testes).
- `src/commands/enrich.rs:3191-3207` (call_site usa o helper).
- `src/commands/ingest_codex.rs:265-340` (call_site usa o helper).
- `src/cli.rs:360` (novo subcomando `codex-models`).
- `src/main.rs:319-329` (dispatch).
- gaps.md G31+G32+G33 linhas 1444-1716.
