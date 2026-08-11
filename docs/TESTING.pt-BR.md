# TESTES — Adições à Suite de Testes da v1.0.83 (ADR-0041)
- `claude_subprocess_inherits_custom_anthropic_provider_env` — documenta a decisão de design de que o caminho de integração equivalente é coberto pela variante codex abaixo (a instalação real de `claude` em CI colide com o truque de prefixar PATH com o mock); veja ADR-0041 §Verification
- `claude_subprocess_rejects_prohibited_anthropic_api_key` — confirma que o guard OAuth-only ainda aborta o spawn com exit não-zero quando `ANTHROPIC_API_KEY` está setada; o script mock pode ou não rodar dependendo se o guard dispara primeiro
- `codex_subprocess_inherits_openai_base_url` — verifica que a env var `OPENAI_BASE_URL` propaga do pai para o subprocesso codex, o caminho canônico de teste de integração cross-process
- `strict_env_clear_drops_custom_provider_credentials` — confirma que `--strict-env-clear` / `SQLITE_GRAPHRAG_STRICT_ENV_CLEAR=1` descarta `ANTHROPIC_AUTH_TOKEN` do env do subprocesso, preservando apenas `PATH`
- `audit_no_token_leak_in_subprocess_stderr` — varre o stdout e stderr capturados do subprocesso com `RUST_LOG=trace` e afirma que o valor literal do token NUNCA aparece em nenhum dos dois streams; esta é a auditoria que previne regressões futuras onde um macro `tracing` possa imprimir o token bruto
- Mais 3 testes unitários helper em `src/spawn/env_whitelist.rs::tests` cobrindo a API Rust diretamente: `whitelist_includes_custom_provider_vars`, `whitelist_excludes_api_key_vars`, `strict_mode_drops_credentials`
- Todos os testes carregam `#[serial_test::serial(env)]` para serializar mutações de env no runner de testes paralelo
- Contagem total de testes: 818 (de 812 na v1.0.82; os 6 novos testes estão divididos entre 3 testes unitários em `env_whitelist.rs` e 3 testes de integração em `claude_runner_env.rs` mais os 2 testes estilo auditoria)
- Testes OAuth-only pré-existentes em `claude_runner.rs:574-666` e `codex_spawn.rs:684-758` permanecem verdes; a extensão do env whitelist NÃO enfraquece o guard
# Guia de Testes — v1.0.89 Suite de Testes Preflight + BUG-11/12/13 + Schema Drift (ADR-0045, ADR-0046, ADR-0047, ADR-0048, ADR-0049)


- Leia a versão em inglês em [TESTING.md](TESTING.md)
- Plano de testes formal com camadas, gatilhos e gates de release: [TEST_PLAN.pt-BR.md](TEST_PLAN.pt-BR.md)


## v1.2.7 — Suítes do Binário Instalado Viram Opt-In, e os Envs de Harness Ganham Nome

As três suítes `installed_binary_smoke_*` validam `~/.cargo/bin/sqlite-graphrag`, e não o binário que o `cargo test` acabou de compilar. Isso é deliberado: elas existem para pegar um workspace verde escondido atrás de uma instalação velha. Até a v1.2.7, porém, a checagem estava montada de um jeito que nenhum código correto conseguia satisfazer. O `Cargo.toml` sobe de versão antes do gate rodar, o gate exige que o instalado seja igual ao workspace, e instalar só é seguro depois que o gate passa. Toda release, portanto, começava com 26 testes vermelhos que nada diziam sobre o código.

Elas agora carregam `#[ignore]`, então o libtest as reporta no sumário como ignoradas, em vez de falhar e em vez de fingir. A ordem de execução é:

1. `cargo test --lib --all-features --locked` e `cargo test --tests --all-features --locked`, aceitando as 26 como ignoradas.
2. `cargo install --path . --locked --force`, que substitui o binário que o operador está usando agora — nunca um ato neutro no meio de uma sessão.
3. `cargo test --test installed_binary_smoke_crud --test installed_binary_smoke_graph --test installed_binary_smoke_maintenance -- --ignored`.

O passo 3 é onde `assert_expected_installed_version` justifica seu lugar. Ali as versões precisam bater, então divergência é defeito real e não artefato de ordenação.

A mesma release removeu um modo de falha silencioso. `skip_if_not_installed` chamava `std::process::exit(0)` quando não havia binário instalado. O libtest captura a saída de cada teste, e um `process::exit` nu nunca faz o flush dessa captura, então a mensagem explicativa não chegava a ninguém enquanto a suíte reportava sucesso sem rodar uma única asserção — exatamente o padrão que `src/commands/optimize.rs` já proíbe em `src/` sob o GAP-SG-125. Pedir a suíte do binário instalado sem binário instalado agora entra em pânico.

Três variáveis de ambiente pertencem ao harness de teste e não são configuração de produto. O produto não lê nenhuma delas (G-T-XDG-04):

- `SGR_TEST_EXPECT_INSTALLED_VERSION` sobrescreve a versão que as suítes smoke esperam, no lugar de `CARGO_PKG_VERSION`.
- `SGR_TEST_ALLOW_INSTALLED_VERSION_MISMATCH=1` rebaixa a asserção de versão para aviso, para auditoria deliberada de uma instalação antiga.
- `SGR_TEST_BENCH_OPT_IN=1` é exigida por `benches/retrieval_bench.rs`, que entra em pânico sem ela.

A mesma release moveu para `tests/` duas regras que nenhum gate vigiava, onde o `cargo test` as alcança:

- `tests/fmt_gate.rs::the_workspace_is_rustfmt_clean` roda `cargo fmt --all --check` de dentro da suíte. Este repositório proíbe CI, como `tests/no_ci_workflows_gate.rs` declara em texto literal, então o `cargo test` é o único gate automático e toda outra guarda vive em `tests/`. Uma regra que exige ferramenta diferente do rustc ficava, portanto, fora de qualquer gate, imposta só por um script pré-publicação e só quando alguém lembrava de invocá-lo. A execução pré-publicação da v1.2.7 encontrou deriva em cinco pontos, todos escritos na mesma sessão que acabara de declarar a release pronta — a assinatura de uma regra que nada verifica. O `cargo fmt` não lê nem escreve em `target/`, então não toma o build lock e não custa contenção com uma compilação já em curso (GAP-SG-211).
- `tests/numeric_bounds_gate.rs::every_bounded_numeric_argument_declares_a_value_parser` varre `src/commands/` e exige `value_parser` em todo argumento numérico do clap cujo nome limita um conjunto de resultados, uma página ou uma travessia: `limit`, `k`, `top_k`, `depth`, `max_hops`, `max_results`, `max_sub_queries`. A superfície de concorrência já tinha faixa declarada e a de memória não. O `related --limit` alocava `Vec::with_capacity(limit)` antes que qualquer linha pudesse limitá-lo, então um valor absurdo abortava o processo na alocação em vez de devolver o exit code que esta crate reserva para pressão de memória. O gate lê o texto-fonte em vez da `Command` construída porque o clap apaga a faixa dentro de um `ValueParser` opaco, de modo que a árvore parseada não consegue responder se um argumento é limitado (GAP-SG-213).

## v1.2.2 — Superfície de Saída Agent-Native e Type-Check Multiplataforma

### Testando a superfície de saída

As oito flags globais (`--select`/`--fields`, `--filter`, `--max-items`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`) são aplicadas em um **único** ponto — `src/agent_surface/` remodela o envelope serializado, então uma implementação cobre a CLI inteira. Teste ali, não por subcomando.

- Suite unitária: `src/agent_surface/tests.rs`. Os módulos irmãos `filter.rs`, `shape.rs` e `budget.rs` NÃO têm `#[test]` próprio; toda asserção sobre eles vive em `tests.rs`.
- As invariantes que precisam permanecer verdes, porque cada uma é uma forma de mentir para o agente:
  - **Envelope de falha nunca é filtrado.** `error: true` / `ok: false` chega ao chamador literalmente. Se `--filter` pudesse suprimi-lo, um agente leria uma falha como conjunto de resultados vazio.
  - **Documento `$schema` passa intacto.** É contrato, não conjunto de resultados.
  - **Truncagem nunca é silenciosa.** Tudo que foi removido é registrado em `agent_surface` e levanta a flag `truncated` de topo.
  - **Streams NDJSON contornam a superfície.** Remodelá-los mudaria o contrato do stream.
  - **`--filter` malformado sai com exit 2** em vez de devolver zero linhas — um typo nunca pode parecer resultado vazio legítimo.
  - **Chave ausente é pulada, não emitida como `null`** — uma projeção jamais pode inventar campo.
  - **`--max-output-bytes` descarta elementos do fim, nunca fatia o texto JSON** — envelope fatiado não faria parse.
  - **Sem knob definido, o envelope é idêntico byte a byte à saída anterior à v1.2.2** — a superfície é opt-in.

```bash
cargo test --lib agent_surface
```

### Exemplos documentados são executáveis

`tests/readme_examples_executable.rs` faz `include_str!` dos dois READMEs e **executa** os blocos `bash` cercados contra o binário real, exigindo pelo menos **dez** blocos executáveis por README. Um bloco é pulado quando contém `|`, `>`, `<`, `$(`, crase, `&&`, `||` ou `;`; quando usa `--embedding-backend openrouter` (o job de CI é hermético, sem chave); ou quando a linha anterior à cerca de abertura carrega `<!-- skip-test -->` (a forma `<!-- skip-test: motivo -->` também é aceita). Um `TempDir` e um `init` são compartilhados por README.

Portanto, um **exemplo novo que DEVA rodar** precisa ser comando único, offline, sem pipe e sem redirecionamento. Um exemplo novo que não possa rodar precisa do marcador explícito — nunca o deixe sem marcação torcendo para dar certo.

```bash
cargo test --test readme_examples_executable
```

`docs/COOKBOOK.md` **não** tem validação de exemplos: as receitas em `tests/cookbook_recipes.rs` estão hardcoded lá, não são parseadas do documento. Editar uma receita do cookbook não muda o que aquele teste assere.

### Gate de release — por que `--no-fail-fast` não é opcional

```bash
cargo test --all-features --no-fail-fast
```

Sem `--no-fail-fast`, o cargo para no primeiro alvo vermelho. A execução então reporta um **prefixo da suíte** com output que parece completo — você lê uma cauda verde que nunca foi alcançada. `--no-fail-fast` roda todos os alvos e reporta todas as falhas, que é a única forma do gate significar o que promete.

### Type-check multiplataforma — o ponto cego do `#[cfg]`

Um `#[cfg]` mutuamente exclusivo cria **duas árvores de compilação**, e um gate local no Linux só enxerga uma delas. Foi exatamente assim que a CLI parou de compilar em Windows e macOS sem ninguém notar: todo teste local ficava verde, porque a árvore quebrada nunca era construída.

O alvo óbvio, `x86_64-pc-windows-msvc`, falha em `ring` por falta de `lib.exe`. Use o alvo GNU: com mingw ele compila a crate inteira, e é `cfg(windows)` do mesmo jeito:

<!-- skip-test: compilação cruzada, exige a toolchain mingw -->
```bash
env CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
    AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar \
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    cargo check --target x86_64-pc-windows-gnu --all-features
```

Pré-requisito: `rustup target add x86_64-pc-windows-gnu` e a toolchain mingw-w64. Veja também [CROSS_PLATFORM.pt-BR.md](CROSS_PLATFORM.pt-BR.md).

## v1.2.1 — Regressões CAPA da Fila Enrich

- **Escopo**: sem migração main-DB — schema permanece **v16**; CAPA é **somente comportamento da fila sidecar** (crate **1.2.1**).
- **Suite unitária da fila**: **38** testes OK (fila enrich / claim / enqueue).
- **Regressões adicionadas / obrigatoriamente verdes:**
  - `enqueue_candidate_accepts_entity_prefixed_reembed_key` (em `queue_tests_b.rs`) — aceita `entity:ownership` e bare `ownership`; rejeita entidade ausente.
  - `dequeue_next_pending_isolates_by_namespace` (em `queue_tests_a.rs`) — claim filtra `operation` **e** `namespace`.
- **Temas CAPA cobertos pela suite (comportamento, nem sempre um teste nomeado cada):**
  - `--until-empty` / `count_eligible_pending` conta **somente esta op+namespace**
  - `--force-redescribe` reabre `skipped`/`done` uma vez por processo; nunca `dead`
  - Reconciliação de zumbi re-embed quando `LENGTH(embedding) = dim*4` já satisfeito
  - Elegibilidade re-embed por comprimento do BLOB (CORRUPT / META_AHEAD re-elegível)
  - Enqueue de chunk valida chunk em memória não-deletada do namespace alvo
  - Marcadores CAPA-D compostos de "configuration file" apenas
- Gate offline do produto permanece `scripts/e2e_offline_v120.sh` **20/20** (sem harness offline novo para 1.2.1).
- Docs complementares: [TEST_PLAN.pt-BR.md](TEST_PLAN.pt-BR.md), [CHANGELOG.pt-BR.md](../CHANGELOG.pt-BR.md) `[1.2.1]`, [gaps.md](../gaps.md).

```bash
# Regressões focadas da fila (nomes registrados na suite lib)
cargo test --lib enqueue_candidate_accepts_entity_prefixed_reembed_key
cargo test --lib dequeue_next_pending_isolates_by_namespace
# Gate offline completo (inalterado)
cargo build --release && bash scripts/e2e_offline_v120.sh
```

## Gate E2E Offline — v1.2.0 (`scripts/e2e_offline_v120.sh`)
- **Gate offline obrigatório do produto na v1.2.0**: `scripts/e2e_offline_v120.sh` (espera **20/20 PASS** — 15 `check()` + 5 PASS manuais; wrapper histórico `e2e_offline_v118.sh` supersedido).
- Escopo: sem product env, sem chave OpenRouter, dirs XDG isolados sob `$TMPDIR`, só flags (`--db`), apenas local.
- Asserções incluem: `config set` / `config doctor`, `purge --now --dry-run`, contrato de help (`help_contract` — sem propaganda de product env / sem `Box` about do clippy), fold de EntityType, description em remember-batch, pending-embeddings status, cache stats, **help + flags offline de `list-skipped` / `requeue-skipped`** (`list_skipped`, `requeue_skipped`, `help_skipped_flags`), **`embedding.dim` efetivo 1024** (`effective_dim_1024`; **DEFAULT_EMBEDDING_DIM=1024**), e contratos relacionados da v1.2.0.
- Smoke de contrato complementar (veja [TEST_PLAN.pt-BR.md](TEST_PLAN.pt-BR.md)): materialização `deep-research -o`, `description` em `memory-entities`, `entities_created`/`enrich_recommended` no remember, campos de qualidade em `enrich --status --force-redescribe`, honestidade do help de entity-connect (totalmente implementado).
- Contratos unitários/integração companheiros:
  - `tests/help_no_product_env` — o help não deve anunciar product env `SQLITE_GRAPHRAG_*` como mecanismo de config.
  - `tests/cli_db_noop_host_surfaces_regression.rs` — **GAP-SG-139**: folhas host/XDG (`config`×9, `slots`×3, `cache`×3, `completions`) aceitam `--db` como no-op documentado (`src/cli_db_noop.rs`).
- **NÃO** é pipeline obrigatório de GitHub Actions do produto — rode localmente (ou cron/systemd/launchd no host). GitHub Actions é infraestrutura opcional do operador, não o gate de release desta CLI.
- Pré-requisito de build: binário release em `target/release/sqlite-graphrag` (o script compila se faltar).

```bash
cargo build --release
scripts/e2e_offline_v120.sh
# contratos de help + GAP-SG-139 opcionais:
cargo test --test help_no_product_env
cargo test --test cli_db_noop_host_surfaces_regression
```

## Infraestrutura de Testes — Matriz CI de Features (2 features desde a v1.0.79)
- Prefira `cargo test` / nextest **local**; **não** trate GitHub Actions como CI de produto obrigatório para esta CLI.
- Nota histórica: alguns workflows rodavam `clippy` e `test` em uma matriz de 2 features desde a v1.0.79: `default` e `llm-only` (`embedding-legacy` foi removida junto com a feature).
- Os jobs `default` e `llm-only` instalam uma CLI stub `mock-llm` no `PATH` para que os testes de round-trip de embedding rodem sem uma assinatura real de LLM.
- 26 arquivos de teste foram cabeados para consumir uma mock LLM CLI para que o CI nunca precisasse de credenciais vivas. Esse harness está INERTE desde a remoção dos backends por subprocesso; o caminho offline agora precisa de um stub OpenRouter.
- 107 de 115 testes previamente lentos foram corrigidos no commit `bd0a3f5` (a mock LLM desbloqueia testes que dependiam de um turno OAuth real).
- Execute a matriz multi-OS **localmente** (GitHub Actions não é obrigatório para o produto): `cargo test --lib` em Linux, macOS e Windows; agende com cron, systemd.timer ou launchd — nunca como pipeline obrigatório de GitHub Actions para a CLI.

### Contrato da Mock LLM CLI
- Os scripts mock em `tests/mock-llm/` foram preservados mas não são mais alcançáveis: nada spawna CLI local.
- Para requisições de embedding: devolvem vetores `f32` de 64 dimensões zerados (a dimensionalidade default ativa desde a v1.0.79, G42/S1).
- Os dois formatos de resposta são falados desde o fix do G43: single (`{"embedding":[...]}`) e batch (`{"items":[{"i":N,"v":[...]}]}` quando o prompt pede EXATAMENTE N itens, G42/S2).
- Testes de extração de entidades devem mockar em nível mais alto ou chamar a API da biblioteca; os scripts são dedicados ao caminho de embedding.
- Esses testes de integração ficam atrás do gate `--features slow-tests` e NÃO rodam na matriz default do CI.
- Operadores rodando testes localmente precisam prepender a mock ao `PATH`:
  ```bash
  export PATH="$PWD/target/debug:$PATH"
  cargo test --workspace
  ```

### Seleção de Testes por Feature Flag
- `cargo test --lib` — roda contra features padrão (mock LLM em CI, LLM real requerida localmente).
- `cargo test --lib --no-default-features --features llm-only` — mesmo comportamento que default, opt-in explícito.
- `cargo test --workspace --features slow-tests` — roda a suíte completa de contratos incluindo a matriz de integração de 832 testes.


## Adições de Testes v1.0.78 — Cobertura da Correção G41
### Delta de Contagem de Testes
- Linha de base v1.0.77: 723 testes de lib passando
- v1.0.78 final: 726 testes de lib passando (+3 novos unitários, +1 unitário atualizado)
### Testes Unitários em `src/commands/migrate.rs`
- `rehash_does_not_insert_missing_migrations` — verifica que `run_rehash` não insere mais linhas fantasma para migrações não aplicadas (ATUALIZADO de `rehash_insert_includes_applied_on`)
- `ensure_v013_tables_noop_when_no_history` — verifica no-op quando `refinery_schema_history` não existe
## v1.0.87 — Testes da Camada de Validação Pre-Flight (ADR-0045, GAP-META-005)

- `tests/bug11_preflight_regression.rs` (2 testes) — reprodutores para as 5 classes BUG que o GAP-META-005 endereça. Os 7 guards (`check_argv_size`, `check_binary_exists`, `check_mcp_config_inline`, `check_mcp_config_path`, `check_walkup_mcp_json`, `check_output_buffer`, `check_claude_config_dir`) têm 2 testes cada: caso positivo (passa) e caso negativo (retorna a variante específica `PreFlightError`)
- `src/spawn/preflight.rs` (15 testes unitários inline) — `check_argv_size_rejects_oversized_argv`, `check_argv_size_accepts_under_limit`, `check_binary_exists_returns_binary_not_found`, `check_mcp_config_inline_creates_tempfile_for_braces`, `check_mcp_config_inline_passes_when_already_tempfile`, `check_mcp_config_path_validates_json`, `check_mcp_config_path_rejects_missing_file`, `check_walkup_mcp_json_walks_to_root`, `check_walkup_mcp_json_accepts_absent`, `check_output_buffer_doubles_capacity_above_64k`, `check_output_buffer_passes_when_under_limit`, `check_claude_config_dir_rejects_non_empty`, `check_claude_config_dir_accepts_empty`, `is_skipped_returns_true_with_env_var`, `is_skipped_returns_false_without_env_var`
- Todos os 4 spawners (`claude_runner`, `codex_spawn`, `ingest_claude`, `extract/llm_embedding`) ganham cobertura `#[test]` do site de chamada pre-flight

## v1.0.88 — Testes dos Hotfixes BUG-11/12/13 (ADR-0046, ADR-0047)

- `tests/bug11_preflight_regression.rs::embed_via_backend_strict_returns_no_backends_error` — verifica que quando pre-flight falha em `extract/llm_embedding.rs:563-565`, `remember` propaga o erro via exit 11 em vez de persistir silenciosamente com `backend_invoked: "none"`
- `tests/bug11_preflight_regression.rs::remember_with_mcp_config_dir_in_legacy_path_aborts` — repro do BUG-11: `CLAUDE_CONFIG_DIR=/tmp/bad-config-with-mcp` causa exit 11 com envelope JSON de erro
- `tests/oauth_stderr_emits_single_line_v1088` — verifica fix do BUG-12: `ANTHROPIC_API_KEY=sk-test init` emite exatamente 1 linha stderr (eram 2)
- `tests/entity_validation_integration.rs` (8 testes) — verifica fix do BUG-13: `link --create-missing` agora respeita validação de nome de entidade. Caso de borda de 4 chars (`API` é rejeitado, `claude` é aceito)
- Renomeação de teste em `embedder.rs:1704` — `embed_with_fallback_succeeds_via_none_when_chain_exhausts` → `embed_with_fallback_chain_of_only_none_aborts_without_skip_on_failure_v1088` (agora documenta o contrato corrigido)

## v1.0.89 — Sete Testes de Regressão para os Dez GAPs (ADR-0048, ADR-0049)

- `tests/health_namespace_regression.rs::health_accepts_namespace_flag_v1089` — GAP-E2E-002. Verifica que `health --namespace prod --json` retorna 0 e filtra contagens para o namespace
- `tests/migrate_dry_run_regression.rs::dry_run_does_not_mutate_schema_history_v1089` — GAP-E2E-009. Verifica que `migrate --dry-run` sai com 0 e `refinery_schema_history` permanece inalterado
- `tests/codex_models_json_regression.rs` — REMOVIDO. O comando `codex-models` e seu teste de regressão saíram junto com os backends de subprocesso na v1.2.0; o comando agora responde exit 2. Listado aqui como histórico para que quem encontrar o nome numa nota de release antiga saiba por que o arquivo não existe
- `tests/cli_db_flag_parity_regression.rs` (5 testes) — GAP-E2E-008 + GAP-E2E-010b. Verifica que `embedding status`, `embedding list`, `embedding abandon`, `pending list`, `pending show` todos aceitam `--db <PATH>` sem erro de clap
- `tests/ingest_auto_describe_regression.rs` (5 testes) — GAP-E2E-011. Verifica `extract_heuristic_description(body, path_hint)`:
  - `auto_describe_uses_body_summary` — primeira linha significativa (>20 chars) vence
  - `auto_describe_falls_back_on_headers_only` — markdown apenas com headers cai para `"ingested document"` quando não há `path_hint`
  - `auto_describe_falls_back_to_stem_when_only_headers` — com `path_hint`, cai para o stem do arquivo (ex.: `headers-only`)
  - `auto_describe_truncates_long_line` — descrições truncadas a ≤100 chars
  - `auto_describe_ignores_short_and_blank_lines` — linhas curtas (<21 chars) e linhas em branco são puladas
- `tests/binary_size_documented_regression.rs::assert_documented_size_matches_real` — GAP-E2E-001. Verifica que a descrição em `Cargo.toml:6` confere com o tamanho real do binário dentro de ±5%
- `tests/health_schema_drift_regression.rs::assert_all_health_keys_in_schema` — GAP-E2E-007. Verifica que todos os 17 novos campos estão presentes no `health.schema.json` regenerado e que `additionalProperties: true` (política Must-Ignore por RFC 7493 I-JSON) é respeitada

## v1.0.85 — Suite de Testes de Cinco Gaps (ADR-0043)

Cinco novos testes de regressão em `tests/embedder.rs` cobrem o enum FallbackReason com 7 variantes:

- `slot_exhaustion_returns_typed_error` — GAP-003: `acquire_llm_slot_for_embedding` retorna `AppError::Embedding` com `reason_code: "slot_exhausted"` após teto de backoff de 750ms
- `oauth_quota_fallback_deterministic` — G58: `try_embed_query_with_deterministic_fallback` retenta em `OAuthQuota` e propaga `reason_code` para `vec_degraded_reason`
- `anthropic_ratelimit_headers_captured` — G45-CR5: `LlmEmbedding::invoke_claude` parseia 12-14 headers `anthropic-ratelimit-*-remaining` e aborta em `0`
- `read_notfound_preserves_identifier` — G55 docs: `read NotFound` retorna mensagem bilíngue com identificador (nome ou id) e namespace preservados
- `embedding_dim_reduces_token_cost` — G56: dim=64 produz ≤1/6 dos tokens OAuth consumidos por dim=384

Todos os cinco testes são gated por `#[serial_test::serial(env)]` para prevenir poluição de PATH entre runs concorrentes.

## v1.0.85.1 — Teste de Regressão GAP-004

`try_embed_query_with_none_returns_dim_zero_fallback` em `tests/embedder.rs`: `--llm-backend none` em `recall` e `hybrid-search` agora sai com exit 0 e `vec_degraded: true` + `source: "fts_fallback"` + `vec_degraded_reason: "dim_zero"`. Sem este teste, v1.0.85.0 quebrou o failsafe do v1.0.80 silenciosamente.

## v1.0.85.2 — Testes BUG-001/002/003 (ADR-0044)

- `cli_dry_run_backend_works_standalone` — `--dry-run-backend` sai com exit 0 sem subcommand, imprime `{action, backend, binary, model, flavour, chain, strict_env_clear}`
- `embed_via_backend_returns_resolved_kind` — `embed_via_backend` retorna `Result<(Vec<f32>, LlmBackendKind), AppError>` propagando `resolved_kind`
- `setup_mock_path_emits_json` — `setup_mock_path()` em `tests/embedder.rs:37-77` alinhado para emitir JSON (não JSONL)

## v1.0.99 — Remoção do Degree-Cap + Correções de Doc/Convergência (ADR-0059, GAP-SG-67/68/69)

- **GAP-SG-67 (remoção de testes)**: `graph::enforce_degree_cap` e seus 5 testes unitários (mais o helper `setup_cap_db`) foram DELETADOS junto com a função. Não há novo teste de regressão dedicado — a garantia aditiva é estrutural: com o código de poda removido do caminho de escrita, um `remember`/`link` normal não pode mais deletar arestas, portanto o total de `relationships` não pode diminuir em uma escrita. `cargo clippy --all-targets -- -D warnings` confirma que não há referência pendente a `enforce_degree_cap`/`max_entity_degree`.
- **GAP-SG-68**: os 6 testes `build_order_by_*` em `src/commands/graph_export.rs` (`build_order_by_defaults_to_name_asc`, `build_order_by_name_desc`, `build_order_by_degree_desc`, `build_order_by_degree_asc`, `build_order_by_created_at_asc`, `build_order_by_created_at_desc`) permanecem verdes; eles fixam o padrão ascendente e o caminho `--order desc` que o doc-comment realinhado agora descreve.
- **GAP-SG-69**: `src/commands/enrich/queue.rs::tests::skipped_item_keys_excludes_only_skipped_for_operation` prova que o novo helper `skipped_item_keys` retorna apenas linhas com `status='skipped'` para a operação consultada, de modo que o rescan do body-enrich com `--until-empty` não re-enfileira corpos curtos vetados e o sidecar é retido enquanto houver veredictos `skipped` (empiricamente 55→3).
- Sem migração; schema permanece v15; `Cargo.toml` é 1.0.99. Os totais da suíte não foram re-aferidos nesta passagem de documentação — rode `cargo nextest -P ci` para a contagem ao vivo.


## v1.1.06 — Suite de Regressão do Scan O(k) do entity-connect

- Suite de integração [`tests/v1106_entity_connect_scan_regression.rs`](../tests/v1106_entity_connect_scan_regression.rs) (fronteira CLI via `assert_cmd`). ADR: [ADR-0066](decisions/adr-0066-v1-1-06-entity-connect-scan.pt-BR.md). Fecha **GAP-ENTITY-CONNECT-SCAN-CARTESIAN**.
- **Scan O(k)** — `scan_isolated_entity_pairs` gera pares por coocorrência em `memory_entities` mais preenchimento hub × ilha grau-0; nunca o produto cartesiano `entities × entities` com `ORDER BY` global que travava o `global` grande.
- **Tipagem da fila** — chaves `pair:{id1}:{id2}`, `item_type=entity_pair`; drain por chave primária sem re-scan por item.
- **Deadline no primeiro scan** — `InterruptHandle` + `--max-runtime` / teto soft 120s cobre o primeiro SQL; timeout de wall-clock → `AppError::Timeout` exit **1** (não exit 75 de singleton).
- **Backlog dual NDJSON** — `scan_start` antes do SQL (`operation`, `entities_in_namespace`, `backlog_degree0_proxy`); `scan_meta` com `pairs_enqueued_this_scan` / `scan_elapsed_ms`. Nome real de `operation` para entity-connect **e** cross-domain-bridges.
- **GAP-002 preservado** — convergência via `entity_connect_seen` (v1.1.04) permanece verde; `--until-empty` ainda converge.
- Cobertura unitária também em `src/commands/enrich/scan.rs`, `queue.rs` e testes de interrupt/deadline em `mod.rs`. Rode com `cargo test --test v1106_entity_connect_scan_regression` e `cargo test --lib commands::enrich`.
- Sem migração de schema (permanece v16). Nome oficial v1.1.06; crate `1.1.6`.

## v1.1.05 — Testes de Regressão dos Cinco Bugs "danilo"

- Suite de integração [`tests/v1105_danilo_bugs_regression.rs`](../tests/v1105_danilo_bugs_regression.rs) cobre os cinco bugs na fronteira da CLI. ADR: [ADR-0065](decisions/adr-0065-v1-1-05-danilo-bugs.pt-BR.md).
  - **Bug 1**: `deep-research "danilo"` emite mais de uma sub-query; a primeira é o token original e as demais têm `source: "aspect"`. Caminho manual opcional (operador): `--sub-query-strategy manual --sub-queries-file PATH`.
  - **Bug 2**: `--output PATH` grava o envelope completo via **atomwrite** (tempfile → fsync → rename); o stdout carrega o ack com `written`, `bytes`, `blake3`, `sub_queries_total`, `unique_memories_found`, `elapsed_ms`; **`--quiet`** / `-q` suprime tracing não-erro.
  - **Bug 3**: `graph traverse --from <nome-curto>` sem `--fuzzy` retorna NotFound (exit 4) com sugestões; com `--fuzzy`, resolve o vencedor canônico.
  - **Bug 4**: `merge-entities` com **self-ref** (`--ids` contendo `--into-id`, ou nomes) é rejeitado **pré-DB** com exit de validação.
  - **Bug 5**: `link --from-id`/`--to-id` aceitos pelo clap; nomes só de dígitos rejeitados sob `--create-missing`.
- Helpers cobertos por testes unitários: `src/atomic_io.rs` (`write_atomic`, `write_json_atomic`); `entities::resolve_entity_fuzzy`, `suggest_entity_names`, `entity_name_similarity` (Jaro-Winkler + prefixo kebab).
- Sem migração de schema (permanece v16). Nome oficial v1.1.05; crate `1.1.5`. Rode com `cargo test --test v1105_danilo_bugs_regression`.

## v1.1.04 — Testes de Fechamento de GAPs (ADR-0064)

- **GAP-001 (runtime tokio aninhado)**: teste de regressão cobre o panic "Cannot start a runtime from within a runtime" que o `deep-research` atingia quando seu entry point síncrono construía um runtime Tokio dedicado enquanto chamadas de embedding do OpenRouter já estavam em um runtime. O fix move a computação de embeddings por sub-query para o novo helper `compute_sub_embeddings` (executado antes da construção do runtime) e os três caminhos OpenRouter em `embedder.rs` (single, batch serial, fan-out JoinSet) adotam o padrão canônico de reentrada `Handle::try_current` + `block_in_place`. O `ingest_opencode` recebe o mesmo guard. `recall`/`hybrid-search` nunca foram afetados (nunca criam seu próprio runtime) e não precisam de cobertura nova.
- **GAP-002 (convergência do entity-connect)**: testes cobrem a nova tabela `entity_connect_seen` e a contabilidade de backlog. A migração V016 cria `entity_connect_seen(source_id, target_id, namespace, verdict, relation, evaluated_at)` com PK composta, FK dupla `ON DELETE CASCADE` para `entities(id)`, restrição `CHECK(verdict IN ('related','none'))` e índice de namespace. O `scan_isolated_entity_pairs` agora exclui pares já avaliados (LEFT ANTI JOIN contra `entity_connect_seen`) e prioriza entidades hub; o `count_operation_backlog` reporta um proxy real O(n) (entidades grau-0 com bindings NER) em vez de zero hard-coded; e o `call_entity_connect` persiste o veredito nos dois ramos (`related` e `none`). Como resultado, o `enrich --operation entity-connect --until-empty` atinge `eligible_remaining == 0` em vez de re-avaliar infinitamente os pares rejeitados.


## v1.1.03 — Testes dos Seis Bugs + V8 (ADR-0063)

- **Bug 6 (chunks órfãos)**: `scan_chunks_of_soft_deleted_memory_are_selected` e `count_backlog_includes_orphan_chunks` em `src/commands/enrich/scan.rs` provam que `LEFT JOIN memories` agora seleciona chunks de memórias soft-deletadas no re-embed
- **Bug 2 (--literal-to)**: `literal_to_writes_hyphenated_target`, `literal_from_applies_to_literal_to_applies_to_hyphen_migrates`, `literal_to_alone_keeps_verbatim` em `src/commands/reclassify_relation.rs` provam que `--literal-to` escreve o valor verbatim sem normalização do clap
- **Bug 3 (--cross-namespace)**: `cross_namespace_merges_source_from_other_namespace`, `cross_namespace_default_false_rejects_cross_id`, `cross_namespace_target_must_still_be_in_resolved_namespace` em `src/commands/merge_entities.rs` provam o opt-in cross-namespace e o padrão same-namespace seguro
- **Bug 4 (stale claims)**: `stale_processing_claim_is_reset_after_threshold`, `fresh_processing_claim_is_preserved`, `heartbeat_updates_claimed_at` em `src/commands/enrich/queue.rs` provam a coluna `claimed_at` + heartbeat + reset automático no startup
- **Bug 1 (enqueue batch)**: `enqueue_batch_is_atomic` em `src/commands/enrich/mod.rs` prova que o loop de enqueue é uma transação única (batch INSERT atômico)
- **V8 (split-body)**: `split_body_divides_long_memory_into_parts`, `split_body_marks_original_as_superseded`, `split_body_creates_replaces_relations`, `split_body_preserves_history` em `src/commands/split_body.rs` (novo módulo) provam a divisão + tag `SUPERCEDIDO` + relação `replaces` + preservação de histórico
- **Bug 5 (apenas docs)**: nenhuma mudança de teste — apenas o help text do `enrich --status` esclarecendo `scan_backlog` vs `queue_pending`
- Suite total: ~1070 testes de lib passando (`cargo test --lib`). Schema permanece v15. Sem nova telemetria.

## v1.1.02 — Testes de Fechamento de Gaps (ADR-0062)

- `commands::enrich::queue::tests::prune_dead_entity_orphans_removes_only_entity_dead_rows` — prova que o novo helper `prune_dead_entity_orphans` deleta apenas linhas dead com `item_type='entity'`, preservando linhas memory-dead e linhas vivas de entidade.
- `tests/prune_dead_entity_orphans_integration.rs` — exercício end-to-end da CLI `enrich --prune-dead-entity-orphans`; planta linhas entity-dead + memory-dead, afirma `pruned==1`, entity-dead removida, memory-dead preservada.
- `tests/reembed_entities_integration.rs` — regressão do Gap 3: `remember --graph-stdin` planta 2 entidades com embeddings vazios (`--llm-backend none`), `enrich --operation re-embed --target entities` faz backfill dos dois (0→2), segunda execução idempotente. Esqueleto espelha `tests/v1063_features.rs`.
- Contexto: o dispatch `strip_prefix("entity:")` em `call_reembed` estava silenciosamente quebrado desde que o caminho de re-embed com chave de entidade foi adicionado; este teste garante que ele continue roteando para `call_reembed_entity`.

## v1.0.97 — Testes da Auditoria Pós-Selagem (ADR-0056/0057/0058, GAP-SG-57..66)

- `commands::enrich::queue::tests::prune_dead_orphans_removes_only_orphan_memory_rows` — GAP-SG-66. Prova que `enrich --prune-dead-orphans` deleta só linhas `dead` órfãs de memória, mantém a linha de memória viva e nunca toca em linhas de entidade (retorna 1).
- `paths::tests` ×3 para `sidecar_path` — GAP-SG-64/65. Cobrem DB absoluto (sidecar derivado ao lado), nome relativo puro (fallback para CWD) e nome com diretório (sidecar nesse diretório).
- `tests/enrich_queue_db_isolation.rs` — GAP-SG-64. Planta uma fila ao lado de `db_a` e prova que `enrich --status` a lê de um CWD não relacionado.
- Cluster flaky `llm_slots::tests` endurecido (GAP-SG-63) — testes de slot sensíveis a contenção foram de ~8/10 falhas para 0/10 sob a suíte completa.
- Lint gate `#![cfg_attr(not(test), warn(clippy::unwrap_used, clippy::expect_used))]` em `src/lib.rs` (GAP-SG-58) — a contagem real de `unwrap`/`expect` de produção (~36, não os 423 que contavam `#[cfg(test)]`) foi convertida para `?`/`ok_or_else`/recuperação de poison; o gate revelou mais 5 em `config_cmd.rs`.
- Contagens registradas no selo: `cargo test --lib` 973/0, `cargo test` padrão 1164/0, `cargo test --features slow-tests` 1522/0/11 ignorados; após o trabalho pós-selagem `cargo install --path . --locked --force` realinhou o binário global e `installed_binary_smoke` roda 26/0 SEM bypass (GAP-SG-62 resolvido); `cargo fmt --check` 0 diffs; `cargo clippy --all-targets --features slow-tests -- -D warnings` 0 warnings.

## v1.0.96 — Testes de Dead-Letter do Enrich + Concorrência REST OpenRouter (ADR-0055)

- Testes unitários de dead-letter (`commands::enrich::tests`, 8 testes): rate-limit / timeout / db-busy classificam como `Transient`; validação / parse classificam como `HardFailure`; `open_queue_db` executa o `ALTER TABLE` de `error_class` + `next_retry_at` de forma idempotente; `record_item_failure` marca um HardFailure como `dead`, um Transient como `pending` com `next_retry_at` futuro (via `compute_delay`) e um Transient além de `--max-attempts` como `dead`; o dequeue pula linhas com retry futuro e exclui `dead`
- Teste de ordem do embedding (`embedder::tests::reassemble_ordered_restores_input_order`): conclusão fora de ordem do `JoinSet` é remontada pelo índice de chunk, restaurando a ordem de entrada
- Teste vivo de concorrência `tests/openrouter_live_concurrency.rs` (`#[ignore]`): rodar com `cargo test --test openrouter_live_concurrency -- --ignored --nocapture`; embeda 64 chunks de `docs/*.md` em k=1 vs k=8 e prova a ordem (cosseno diagonal 0.9999, off-diagonal max 0.899, argmax 64/64). Requer `OPENROUTER_API_KEY`
- Convergência E2E: ingest de 6 ADRs (`--mode none`) depois `enrich --until-empty --rest-concurrency 8` drena `unbound_backlog` 6 → 0; a segunda passada idempotente faz zero trabalho (~6 ms)
- Total da suite: 1086 passed, 0 failed, 6 skipped (nextest)

## Tamanho Atual da Suite de Testes

A v1.1.06 adiciona a suite `tests/v1106_entity_connect_scan_regression.rs` (GAP-ENTITY-CONNECT-SCAN-CARTESIAN / scan O(k), chaves pair, interrupt no primeiro scan, NDJSON de backlog dual) mais unitários de enrich. A v1.1.05 adiciona `tests/v1105_danilo_bugs_regression.rs` (cinco bugs do incidente deep-research "danilo"). ~1072+ testes de lib a partir de v1.1.04 (v1.1.02 + v1.1.03 + v1.1.04 adicionam chunks-soft-delete, literal-to, cross-namespace, stale-claims, heartbeat, enqueue-batch, split-body, prune-dead-entity-orphans, re-embed entidades, regressão de nested-runtime do deep-research, convergência do entity_connect_seen); `cargo nextest -P ci` a partir de v1.0.93; a v1.0.95 adiciona testes unitários wiremock de `chat_api` mais o teste real-LLM de 13 modelos em `tests/openrouter_chat_real.rs`; a v1.0.96 leva o total nextest a 1086 passed, 0 failed, 6 skipped, adicionando 8 testes unitários de dead-letter, o teste de ordem do embedder e o teste vivo de concorrência `#[ignore]`. Use `--test-threads=2` para desenvolvimento local; o profile `ci` em `.config/nextest.toml` controla paralelismo em CI.

## O Que Mudou nas versões v1.0.90, v1.0.91, v1.0.92, v1.0.93, v1.0.94, v1.0.95
- v1.0.90: testes do backend OpenCode (875 testes de lib)
- v1.0.91: testes de isolamento de CWD, testes de recálculo de grau (877 testes de lib + 21 doc tests + 38 testes de contrato de schema)
- v1.0.92: release apenas de documentação, sem novos testes
- v1.0.93: testes de embedding OpenRouter em `tests/openrouter_embedding.rs`; contagem de testes 986+ testes de lib
- v1.0.94: Remediação de quatro gaps — testes de regressão renomeados (`init_default_dim_is_384`, `embed_timeout_default_is_300`) e um teste de contrato afirmando que `enrich` sem `--mode` é rejeitado (clap exit 2); gate verde (cargo test exit 0)
- v1.0.95: testes de enrich via chat OpenRouter — testes unitários wiremock em `chat_api.rs` (montagem de request, parse duplo do content, usage.cost, retry/backoff 429/5xx/401, incompatibilidade por content vazio), um teste de contrato `validate_mode_flags` que rejeita flags cruzadas sob `--mode openrouter`, um teste de `--openrouter-model` obrigatório (exit 1) e o teste de integração real-LLM `tests/openrouter_chat_real.rs` (#[ignore]) exercitando 13 modelos de texto (13/13 compatíveis — 9 com `reasoning.enabled: false`, 4 via o fallback reasoning-mandatory); gate verde (cargo test exit 0)
- v1.0.96: dead-letter do enrich + concorrência REST OpenRouter — 8 testes unitários de dead-letter em `commands::enrich::tests` (classificação Transient/HardFailure, ALTER idempotente de `error_class`/`next_retry_at`, roteamento de `record_item_failure`, elegibilidade de dequeue) mais `embedder::tests::reassemble_ordered_restores_input_order` e o teste vivo `tests/openrouter_live_concurrency.rs` (#[ignore]); total nextest 1086 passed, 0 failed, 6 skipped
- Scripts Mock LLM em `tests/mock-llm/` agora cobrem backends `claude`, `codex`, `opencode`
- Embedding OpenRouter usa API ao vivo em testes E2E (não mockado) — requer `OPENROUTER_API_KEY`
- `ensure_v013_tables_noop_when_tables_exist` — verifica no-op quando `memory_embeddings` já existe
- `ensure_v013_tables_creates_when_phantom` — verifica reparo quando V013 está no histórico mas as tabelas não existem
### Justificativa de Cobertura
- G41 corrigiu um bug onde `run_rehash` registrava V013 como aplicada sem executar o SQL
- O teste atualizado valida que a remoção do branch `else` está correta
- Os 3 novos testes cobrem o helper `ensure_v013_tables_exist` para os 3 cenários (sem histórico, tabelas existem, phantom)
- Reparo automático em `ensure_db_ready` é coberto transitivamente via o helper ensure

- Reparo automático em `ensure_db_ready` é coberto transitivamente via o helper ensure


## Adições de Testes na v1.0.79 — G42-G52 e Remoção do Daemon
### Testes Adicionados por Gap
- `embedder::adaptive_batch_for_dim` fórmula: 6 testes cobrem a função `clamp(base×64/dim, 1, base)` nas dims 64, 128, 256, 384, 4096, mais casos degenerados (dim 0, base 0) e o wrapper env-dim end-to-end com `#[serial_test::serial(env)]`
- `connection.rs`: 4 testes para `adopt_embedding_dim()` cobrindo adoção rw/ro, precedência de env e bancos virgens
- `mock-llm`: extração de dim do prompt e do `--output-schema`; detecção de formato de batch
- `mocks_64_dim` e `mocks_64_dim_batch`: cobertura end-to-end para bancos 384 + mock
- `recall` e `hybrid-search`: fallback trigram, campo vec_degraded, caminho FTS5-only
- `vec stats`: `dim_breakdown_groups_rows_per_dim_and_table`
- 2 testes obsoletos de daemon viraram guardas de regressão da remoção do daemon
- 2 testes de `--autostart-daemon` atualizados para afirmar que a flag é rejeitada
- 1 teste atualizado `rehash_does_not_insert_missing_migrations` (substitui o teste que validava comportamento com bug)
- 9 testes `#[serial_test::serial(env)]` para adoção de dim em chunks/memories/entities
- 3 novos testes unitários para `ensure_v013_tables_exist` (noop, repair fantasma, sem histórico)
### Racional de Cobertura
- G42 fechou o pipeline de embedding LLM lento/serializado/frágil com 9 sub-soluções; testes cobrem a fórmula de batch, pico de paralelismo (AtomicUsize), panic-com-permit-RAII, cancelamento, falha por dim divergente
- G43 corrigiu a lacuna de cobertura de adoção de dim; testes agora cobrem todos os 4 caminhos de abertura de conexão
- G44 tornou o tamanho de batch adaptativo à dim; testes verificam a fórmula e o wrapper env-dim
- G50 corrigiu 6 causas de CI vermelho; testes cobrem o doctest, dim do mock, LLM do benchmark, política de linguagem, race de dim
- G51 tornou os mocks LLM cientes de multi-dim; testes cobrem a extração de dim e o formato de batch
- G52 corrigiu o contrato de schema do vec-stats; testes cobrem o dim breakdown
- G47 corrigiu flags de CLI documentadas mas faltantes; testes cobrem a resolução de alias
- G48 corrigiu o ponto cego do G20 em valores default; testes cobrem a checagem `is_some()`
- G49 corrigiu o descarte silencioso de dim inválida; testes cobrem a emissão de `tracing::warn!`


## Adições de Testes na v1.0.80 — G45, G53, G55 S2, G56, G58, ADR-0033, ADR-0034
### Testes Adicionados por Gap
- `lock::acquire_embedding_singleton`: 4 testes cobrem escopo namespace/db, polling de fs4 flock, quebra forçada de lock stale e `is_retryable() == true` para a nova variante `AppError::EmbeddingSingletonLocked`
- Job CI `semver-checks`: 1 teste em `tests/semver_checks_smoke.rs` valida que `cargo +stable semver-checks check-baseline --baseline-version 1.0.79` roda sem panic no `Cargo.toml` atual; o job é informativo em v1.0.80 e vira bloqueante em v1.0.81
- Steps CI `windows-2025`: 2 steps novos em cada um dos jobs `clippy` e `test` (gateados em `if: matrix.os == 'windows-2025'`) para pre-warm e verify; o YAML do workflow é o artefato de teste (sem teste Rust inline, validado rodando os jobs localmente)
- `signals::handler`: 1 novo teste `panic_free_third_signal_exits_130_with_zero_io` valida que mesmo com `SIGPIPE` no stderr (o cenário de processo órfão), o handler retorna limpo; o terceiro Ctrl-C consecutivo sai com código 130 e ZERO I/O
- `AppError::MemoryNotFound { name, namespace }` e `AppError::MemoryNotFoundById { id }`: 2 testes cobrem a variante estrutural; mensagens em pt-BR carregam nome e namespace
- `embed_entity_texts_cached`: 3 testes cobrem a chave de cache `blake3(model || \0 || text)`, o snapshot de stats e a taxa de hit
- `recall --fallback-fts-only` e `hybrid-search --fallback-fts-only`: 2 testes cobrem o caminho FTS5-only; 1 teste é `#[ignore]` porque o stub G58 S1 exige `PATH` sem `codex` ou `claude` para exercitar `EmbeddingFailed`
- As 7 novas conclusões de teste em v1.0.80 (4 do singleton G45 + 1 do semver-checks + 1 de signals + 1 de MemoryNotFound) elevam a suíte total para 1176 testes; 0 falhas
### Racional de Cobertura
- A política de estabilidade do ADR-0032 é aplicada por `cargo +stable semver-checks` no CI (informativo em v1.0.80); o teste smoke previne regressões no próprio harness smoke
- A resiliência de infra Windows do ADR-0033 é validada pelos novos steps de pre-warm e verify; validação local de cross-compile reproduz `E0463` e é corrigida via `rustup target add x86_64-pc-windows-msvc --toolchain 1.88`
- A resiliência de SHUTDOWN do ADR-0034 é validada pelo teste panic-free third-signal; o teste reproduz o cenário de processo órfão da auditoria G42/C2
- O singleton G45 previne a patologia de contenção LLM multi-sessão; testes cobrem o contrato `is_retryable`
- O `MemoryNotFound` estrutural do G55 S2 elimina a classe de bugs "not found: unknown"; testes cobrem a variante estrutural
- O cache de entity-embed do G56 reduz custo em entidades canônicas; testes cobrem a chave de cache e a taxa de hit
- O fallback FTS5 do G58 mantém o caminho de leitura vivo sob contenção OAuth; testes cobrem o caminho FTS5-only e o campo de envelope `vec_degraded`




## Adições de Testes v1.0.77 — Cobertura da Correção G40
### Delta de Contagem de Testes
- Linha de base v1.0.76: 719 testes de lib passando
- v1.0.77 final: 723 testes de lib passando (+4 unitários, +2 integração)
### Testes Unitários em `src/commands/migrate.rs`
- `sanitize_null_applied_on_fixes_null_rows` — verifica que linhas com `applied_on` NULL são corrigidas
- `sanitize_null_applied_on_noop_when_all_filled` — verifica no-op quando não há NULLs
- `rehash_insert_includes_applied_on` — verifica que INSERT agora inclui `applied_on` (renomeado para `rehash_does_not_insert_missing_migrations` na v1.0.78)
- `remove_vec_tables_noop_when_no_vec` — verifica no-op quando não há tabelas vec
### Testes de Integração em `tests/schema_migration_integration.rs`
- `migrate_rehash_fixes_null_applied_on` — rehash end-to-end com correção de NULL
- `migrate_to_llm_only_fixes_null_applied_on` — `--to-llm-only` end-to-end com correção de NULL
### Justificativa de Cobertura
- G40 corrigiu um bug onde `applied_on` ficava NULL após rehash
- Os 4 testes unitários cobrem cada caminho no módulo migrate
- Os 2 testes de integração validam o fluxo CLI end-to-end


## Por Que Categorizar os Testes
### O Incidente de Livelock Térmico — 2026-04-19
- Em 2026-04-19 às 11:37:40, o Intel i9-14900KF do desenvolvedor atingiu Tjmax 100°C
- A temperatura do VRM chegou a 99°C e o sistema exigiu reset forçado após 3 minutos e 11 segundos
- Causa raiz: `tests/loom_lock_slots.rs` executava sem gate `#[cfg(sqlite_graphrag_loom)]`
- O agendador do loom é intensivo por design — ele explora todas as permutações de threads
- Executar modelos loom sem isolamento causa runaway térmico em CPUs de alto núcleo
- Foi o terceiro incidente em sete dias causado pelo mesmo arquivo de testes sem proteção
- TODOS os testes loom DEVEM ter gate `#[cfg(sqlite_graphrag_loom)]` e ser serializados com `#[serial(loom_model)]`
- NUNCA execute testes loom dentro da invocação padrão `cargo nextest run`


## Categorias de Testes
### Testes Unitários — Inline com o Código-Fonte
- Localização: blocos `#[cfg(test)] mod tests` dentro de cada módulo em `src/`
- Executar com: `/usr/bin/timeout 300 cargo nextest run --profile default`
- Escopo: funções puras, variantes de erro, mascaramento, parsing, validação
- Isolamento: sem I/O, sem filesystem, sem chamadas HTTP
- Gate: sempre compilado, sempre executado no profile default
### Testes de Integração — Arquivos Separados
- Localização: diretório `tests/`
- Executar com: `/usr/bin/timeout 300 cargo nextest run --profile default`
- Escopo: subcomandos CLI, contratos de schema JSON, conformidade PRD, CRUD de storage
- Isolamento: `TempDir` por teste, `env_clear()`, wiremock para HTTP
- Gate: sempre compilado, sempre executado no profile default
### Testes de Concorrência Loom — Opt-in Explícito
- Localização: `tests/loom_lock_slots.rs`
- Executar com: `/usr/bin/timeout 3900 bash scripts/test-loom.sh` ou o job CI `loom`
- Escopo: teste de permutação do semáforo de lock slots
- Isolamento: NUNCA executar em paralelo com outros testes — um modelo por vez
- Gate: `#[cfg(sqlite_graphrag_loom)]` obrigatório em CADA função de teste e bloco de imports
- Risco térmico: testes loom sem proteção causaram travamento do sistema em 2026-04-19
### Testes End-to-End Lentos e Stress — Opt-in via Feature Flag
- Localização: arquivos em `tests/` protegidos por `#[cfg(feature = "slow-tests")]`
- Executar com: `/usr/bin/timeout 1800 cargo nextest run --profile heavy --features slow-tests`
- Escopo: suítes end-to-end longas, contratos, paridade i18n, roteamento de exit code, alta concorrência e loops de retry estendidos
- Gate: excluído dos profiles nextest `default` e `ci`
- Suítes críticas de release: `/usr/bin/timeout 1200 cargo test --features slow-tests --test doc_contract_integration -- --nocapture`
- Suítes críticas de release: `/usr/bin/timeout 1200 cargo test --features slow-tests --test prd_compliance -- --nocapture`
- O CI executa essas duas suítes em um job dedicado `slow-contracts` em `ubuntu-latest`
### Benchmarks — Criterion
- Localização: `benches/`
- Executar com: `/usr/bin/timeout 1800 cargo bench` ou `/usr/bin/timeout 1800 cargo criterion`
- Escopo: baselines de latência para remember, recall, hybrid-search, stats, graph
- Gate: nunca incluído em `cargo nextest run`
### Testes de Ingestão Claude Code
- Testes unitários em `src/commands/ingest_claude.rs` cobrem: parsing JSON, fallback de structured_output, tratamento de erros, detecção de rate limit, validação de entity_type, conformidade do schema
- 9 testes unitários protegem invariantes de parsing de extração sem requerer o binário Claude Code
- Testes de integração requerem Claude Code >= 2.1.0 instalado localmente — executar manualmente, não no CI
- Nomes de testes seguem convenções `test_parse_claude_output_*` e `test_extraction_schema_*`
### Testes de Ingestão Codex (v1.0.62)
- 7 testes unitários protegem o parser JSONL do Codex em `src/commands/ingest_codex.rs`
- Testes cobrem: extração válida, erros turn.failed, detecção de rate limit, validação de schema, descoberta de binário
- Parser valida o padrão "último agent_message vence" para múltiplos eventos item.completed
- Testes de integração requerem Codex CLI instalado; pulam graciosamente se indisponível
### Testes de Regressão v1.0.63
- 3 testes de integração em `tests/v1063_features.rs` protegem as correções da v1.0.63
- `restore_preserves_name_after_rename`: remember → edit → rename → restore; asserta que nome permanece renomeado
- `restore_does_not_crash_when_old_name_occupied`: remember A → rename para B → remember novo A → restore B; asserta exit 0 (era exit 10 UNIQUE crash antes da correção)
- `edit_reembeds_when_body_changes`: remember → edit body → recall novo conteúdo; asserta que recall encontra a memória editada com score preciso
### Testes de Regressão v1.0.64
- 14 testes unitários em `src/commands/deep_research.rs` protegem decomposição de query, concorrência bounded, dedup, montagem de cadeias de evidência e edge cases
- Testes unitários em `src/commands/ingest_claude.rs` cobrem parsing de terminal_reason, detecção OAuth via apiKeySource e pré-validação de tamanho do body
- Testes unitários em `src/commands/rename.rs` e `src/commands/rename_entity.rs` cobrem rejeição de mesmo nome com exit 1

### Testes de Regressão v1.0.68
#### Correção do Tipo HANDLE no Windows (G29)
- `tests/terminal_compile_windows.rs` é um novo teste de integração que roda em toda plataforma: confirma que `terminal::init_console` e `should_use_ansi` continuam chamáveis de fora do crate
- No Windows, o teste adicionalmente referencia a checagem type-safe `HANDLE.is_null() + INVALID_HANDLE_VALUE`; se o contrato de tipo regredir, `cargo check --target x86_64-pc-windows-msvc` no job de CI `windows-build-check` falha antes desse teste ser alcançado
- O novo job de CI é a checagem canônica de regressão; o teste de integração é a sonda local de pré-publish
#### Singleton de Jobs (G28-B)
- Três testes unitários em `src/lock.rs::tests`: `job_singleton_path_sanitises_namespace` (verifica slug em kebab-case a partir de input arbitrário), `job_singleton_blocks_second_invocation_same_namespace` (verifica `AppError::JobSingletonLocked` no segundo acquire), `job_singleton_allows_different_namespaces` (verifica isolamento por namespace)
- Rode via `cargo test --lib lock::tests` (sem `#[serial]` porque os IDs únicos por namespace em cada teste isolam-nos de interferência de estado compartilhado)
#### Circuit Breaker (G28-D)
- Três testes unitários em `src/retry.rs::circuit_breaker_tests`: `opens_after_threshold_consecutive_hard_failures`, `ignores_transient_errors`, `success_resets_consecutive_failures`.  Validam a classificação de AttemptOutcome que distingue `AppError::RateLimited` e `AppError::Timeout` (Transient) de `AppError::Validation` e `AppError::Conflict` (HardFailure)
#### Correções de Testes Pré-Existentes de Timezone
- Três falhas de teste pré-existentes foram corrigidas em `src/commands/{history,list,read}.rs`: os testes agora parseiam a string ISO via `chrono::DateTime::parse_from_rfc3339` e comparam `timestamp()` contra `DateTime::UNIX_EPOCH` em vez de afirmar o prefixo hardcoded `1970-01-01T00:00:00`.  Isso torna as asserções timezone-agnostic então a suite fica verde independentemente da env var `SQLITE_GRAPHRAG_DISPLAY_TZ`

### Testes de Novos Comandos v1.0.67
- Testes de `remember-batch` em `src/commands/remember_batch.rs`: testes de serialização para BatchItemEvent e BatchSummary
- Comando `completions`: testado via smoke test `cargo run -- completions bash`
- Integração `read --id`: testado via round-trip `read --id <memory_id> --json`
- Detecção de super-hub no `health`: testado com banco de produção (1059 memórias, 3 super-hubs detectados)
- `edit` skip-embed: testado via comparação body_hash (edição idempotente pula embedding)
- `rename` ghost purge: testado via workflow forget → rename
- Validação de flags: testado via `hybrid-search --max-hops 2` (sem `--with-graph`) esperando exit 1

### Testes dos Novos Comandos v1.0.65
#### Testes de Deep Research
- Testes unitários em `src/commands/deep_research.rs` cobrem divisão de decompose_query, passthrough de query única, semáforo de concorrência bounded, deduplicação de resultados, montagem de cadeias de evidência (filtro depth >= 2) e validação de query vazia
- Teste de contrato `contract_36_deep_research` em `tests/doc_contract_integration.rs`: insere duas memórias, executa `deep-research "auth and deploy" --max-sub-queries 2 --k 5`, verifica chaves obrigatórias (`query`, `sub_queries`, `results`, `evidence_chains`, `stats`) e valida enum `sub_queries[].source`
- Teste de schema `schema_36_deep_research` em `tests/schema_contract_strict.rs`: valida a resposta completa contra `docs/schemas/deep-research.schema.json` (Draft 2020-12, `additionalProperties: false`)
#### Testes de reclassify-relation
- 8 testes unitários em `src/commands/reclassify_relation.rs` cobrem serialização, action dry_run, contagem de merged_duplicates, caso sem matches e guarda de mesmo valor
- Teste de contrato `contract_37_reclassify_relation`: vincula duas entidades via `mentions`, executa `reclassify-relation --from-relation mentions --to-relation related --batch --dry-run`, verifica as 7 chaves obrigatórias e `action == "dry_run"`
- Teste de schema `schema_37_reclassify_relation`: valida contra `docs/schemas/reclassify-relation.schema.json`
#### Testes de normalize-entities
- 5 testes unitários em `src/commands/normalize_entities.rs` cobrem contagem em dry-run, renomeação in-place, merge por colisão, serialização e campo action em dry-run
- Teste de contrato `contract_38_normalize_entities`: insere uma memória, executa `normalize-entities --dry-run`, verifica 5 chaves obrigatórias e `action == "dry_run"`
- Teste de schema `schema_38_normalize_entities`: valida contra `docs/schemas/normalize-entities.schema.json`
#### Testes de enrich
- Teste de contrato `contract_39_enrich`: insere uma memória, executa `enrich --operation memory-bindings --dry-run`, parseia linhas NDJSON, verifica evento de fase validate, evento de fase scan, eventos de item preview (status=`preview`) e linha de summary com todas as chaves obrigatórias
- Teste de schema `schema_39_enrich`: valida cada tipo de linha NDJSON contra o schema correspondente (`enrich-phase.schema.json`, `enrich-item-event.schema.json`, `enrich-summary.schema.json`)
- Todos os testes de enrich usam `--dry-run` para evitar spawnar o binário LLM


## Como Executar
### Default — Desenvolvimento Local
- Executar todos os testes unitários e de integração: `/usr/bin/timeout 300 cargo nextest run --profile default`
- Executar com saída em caso de falha: `/usr/bin/timeout 300 cargo nextest run --profile default --no-capture`
- Executar um teste específico pelo nome: `/usr/bin/timeout 300 cargo nextest run --profile default fragmento_do_nome`
- Executar um arquivo específico: `/usr/bin/timeout 300 cargo nextest run --profile default -E 'test(schema_contract)'`
### CI — Paralelismo Controlado
- Executar todos os testes como o CI faria: `/usr/bin/timeout 600 cargo nextest run --profile ci`
- O profile `ci` define `test-threads = 2` e `RUST_TEST_THREADS=2`
- O profile `ci` habilita retentativas em testes instáveis
- O workflow também executa `doc_contract_integration` e `prd_compliance` separadamente com `--features slow-tests`
### Heavy — Testes de Stress e Lentos
- Executar testes de stress e lentos: `/usr/bin/timeout 1800 cargo nextest run --profile heavy --features slow-tests`
- O profile `heavy` define `test-threads = 1` para isolamento máximo
- NUNCA execute o profile `heavy` em máquina com throttling térmico ativo
- Para validação de release, prefira os comandos explícitos de contrato acima antes de uma rodada heavy mais ampla


## Auditoria Segura do Remember
### Reproduza o Comportamento da Binária Instalada com Limites de cgroup
- Use `/usr/bin/timeout 3900 bash scripts/audit-remember-safely.sh <diretorio-do-corpus>` para auditar o `remember` com segurança contra um corpus real
- O script usa por padrão o `sqlite-graphrag` instalado no `PATH`
- Sobrescreva a binária com `BIN=./target/debug/sqlite-graphrag` para comparar mudanças locais com a build publicada
- O script usa `systemd-run --user --scope -p MemoryMax=4G -p MemorySwapMax=0`
- O script inicializa um banco temporário isolado para cada execução
- A CLI é one-shot (sem daemon); cada chamada de embedding spawna e descarta o subprocesso LLM
- O script executa casos conhecidos de sucesso, limiar, falha e caso sintético


## Testes de Concorrência Loom
### Como o Loom Funciona
- O loom executa cada teste múltiplas vezes permutando os entrelaçamentos de threads
- Usa redução de estados para evitar explosão combinatória
- Cada modelo deve terminar dentro de um limite de preempção definido
- O uso de CPU é extremamente alto — um núcleo satura completamente por modelo
- NUNCA execute testes loom junto com outros testes no mesmo processo
### Executar Testes Loom Localmente
- Use o script canônico: `/usr/bin/timeout 3900 bash scripts/test-loom.sh`
- O script define `RUSTFLAGS="--cfg sqlite_graphrag_loom"` e `RUST_TEST_THREADS=1`
- O script define `LOOM_MAX_PREEMPTIONS=1` para iteração local limitada
- Execute somente no modo release: `--release` é obrigatório para velocidade aceitável
- Monitore a temperatura da CPU antes e durante a execução
### Executar Testes Loom Individualmente
- Compilar primeiro: `/usr/bin/timeout 600 env RUSTFLAGS="--cfg sqlite_graphrag_loom" cargo build --release --tests`
- Executar modelo único: `/usr/bin/timeout 3600 env RUSTFLAGS="--cfg sqlite_graphrag_loom" RUST_TEST_THREADS=1 cargo nextest run --release -E 'test(lock_slot)'`
- Limite menor para iteração local: `LOOM_MAX_PREEMPTIONS=1`
- Aumente os limites manualmente apenas em depurações focadas
### Checkpoint e Retomada
- Defina `LOOM_CHECKPOINT_FILE=/tmp/loom-checkpoint.json` para retomar execuções interrompidas
- O arquivo de checkpoint registra as permutações já exploradas
- Delete o arquivo de checkpoint para iniciar uma exploração nova


## Variáveis de Ambiente
### Variáveis do Loom — Definir Antes de Executar `scripts/test-loom.sh`
- `RUSTFLAGS="--cfg sqlite_graphrag_loom"` — habilita o gate local do projeto para loom, OBRIGATÓRIO para todos os testes loom
- `LOOM_MAX_PREEMPTIONS=1` — limita a profundidade de preempção por modelo (padrão local e CI: 1)
- `LOOM_MAX_BRANCHES=100` — limita o fator de ramificação por execução (padrão local e CI: 100)
- `LOOM_LOG=1` — habilita rastreamento detalhado de execução do loom no stderr
- `LOOM_CHECKPOINT_FILE=/tmp/loom.json` — caminho para arquivo de checkpoint para retomar execuções
- `RUST_TEST_THREADS=1` — OBRIGATÓRIO, proíbe execução paralela de modelos loom
### Variáveis do Cargo e Nextest
- `RUST_TEST_THREADS=N` — controla o paralelismo do nextest em nível de processo
- `CARGO_TERM_COLOR=always` — preserva cores nos logs do CI
- `NEXTEST_PROFILE=ci` — sobrescreve o profile ativo do nextest via ambiente
### Variáveis Específicas do sqlite-graphrag
### Notas específicas do sqlite-graphrag (v1.2.0)
- Product env `SQLITE_GRAPHRAG_*` **não** é o mecanismo de config em runtime — prefira flags `--db` e XDG isolado (`XDG_CONFIG_HOME` / `XDG_DATA_HOME` / …) nos harnesses, como em `scripts/e2e_offline_v120.sh`.
- Isolamento de teste: passe `--db /tmp/test/graphrag.sqlite` (ou paths únicos em temp) por teste; não confie em product env para o path do banco.
- Defaults de host para logs/timezone em shells de operador: XDG `config set log.format json`, `config set display.tz America/Sao_Paulo` (flags ainda vencem).
- `tests/help_no_product_env` protege o texto de help contra propaganda de product env.


## Profiles do CI
### Profile — default
- Ativa: sempre, a menos que seja sobrescrito
- `test-threads`: 2
- `RUST_TEST_THREADS`: não definido, herda o padrão do sistema
- Tentativas: 0
- Slow-timeout: período 60s, termina após 2 períodos (120s kill efetivo)
- Exclui: testes loom, feature slow-tests
### Profile — ci
- Ativa: `/usr/bin/timeout 600 cargo nextest run --profile ci`
- `test-threads`: 2
- `RUST_TEST_THREADS`: 2 (explícito, previne sobrecarga térmica em runners compartilhados)
- Tentativas: 2 para testes instáveis
- Slow-timeout: período 180s, termina após 3 períodos (540s kill efetivo)
- Exclui: testes loom, feature slow-tests
- Job dedicado `slow-contracts` cobre `doc_contract_integration` e `prd_compliance` com `/usr/bin/timeout 1200 cargo test --features slow-tests`
### Profile — heavy
- Ativa: `/usr/bin/timeout 1800 cargo nextest run --profile heavy --features slow-tests`
- `test-threads`: 1
- `RUST_TEST_THREADS`: 1
- Tentativas: 0
- Slow-timeout: período 900s, termina após 2 períodos (1800s kill efetivo)
- Inclui: testes com gate da feature slow-tests
- Exclui: testes loom (sempre separados)
### Job CI Loom — Etapa Separada no Workflow
- Ativa: job chamado `loom` em `ci.yml`
- Ambiente: `RUSTFLAGS="--cfg sqlite_graphrag_loom"`, `RUST_TEST_THREADS=1`, `LOOM_MAX_PREEMPTIONS=1`, `LOOM_MAX_BRANCHES=100`
- Executa: `/usr/bin/timeout 600 cargo test --test loom_lock_slots --release -- --test-threads=1`
- NUNCA deve ser mesclado com as execuções dos profiles default ou ci


## Solução de Problemas
### Throttling Térmico Durante os Testes
- Sintoma: a suíte de testes desacelera progressivamente, CPU reporta temperatura alta
- Causa: testes loom ou de stress rodando sem limites de thread adequados
- Correção: interrompa a execução imediatamente, deixe a CPU esfriar por 5 minutos
- Prevenção: NUNCA execute `cargo test` sem os profiles do nextest configurados
- Prevenção: SEMPRE use `scripts/test-loom.sh` para testes loom
### Travamento do Sistema Durante Testes Loom
- Sintoma: máquina fica sem resposta, exige reset forçado
- Causa: modelos loom executando em paralelo (RUST_TEST_THREADS > 1) em CPU de alto TDP
- Correção: reset forçado, depois defina `RUST_TEST_THREADS=1` antes de qualquer execução loom
- Caso histórico: 2026-04-19 11:37:40 — i9-14900KF travou por 3 minutos e 11 segundos
- Prevenção: atributo `#[serial(loom_model)]` DEVE estar presente em todo teste loom
### Teste Loom Não Termina
- Sintoma: modelo loom não termina após vários minutos
- Causa: `LOOM_MAX_PREEMPTIONS` não definido, exploração sem limite padrão
- Correção: defina `LOOM_MAX_PREEMPTIONS=1` para iteração local limitada
- Trade-off: valores menores perdem entrelaçamentos raros; aumente o limite apenas em depurações focadas
### Testes Instáveis no CI
- Sintoma: teste passa localmente mas falha de forma intermitente no CI
- Causa: ausência de `#[serial]` em testes que compartilham estado global ou variáveis de ambiente
- Correção: adicione `#[serial]` da crate `serial_test` nos testes afetados
- Diagnóstico: execute `/usr/bin/timeout 600 cargo nextest run --profile ci --retries 0` para ver todas as falhas


## Referências

## Inventário de Testes da v1.0.69
### Delta de Contagem de Testes
- Linha de base v1.0.68: 692 testes passando.
- v1.0.69 final: 745 testes passando (+53).
- 0 falhas, 3 ignorados (testes loom gateados por `#[cfg(sqlite_graphrag_loom)]`).
### Novos Testes por Módulo
- `src/commands/claude_runner.rs`: +4 testes de conformidade OAuth-only (`build_command_oauth_only_mandatory_flags`, `build_command_aborts_when_anthropic_api_key_set`, e mais 2) marcados `#[serial_test::serial(env)]` para serializar mutação de env.
- `src/commands/codex_spawn.rs`: +4 testes de conformidade OAuth-only paralelos ao claude, mais 11 testes para o helper de spawn em si (casos de borda do parser, validação de modelo, presença de flags de comando).
- `src/commands/ingest_claude.rs`: testes existentes atualizados para esperar o conjunto canônico de flags OAuth-only.
- `src/preservation.rs`: 10 testes para `jaccard_similarity` (condições de borda, trigramas, strings vazias, Unicode) e `PreservationVerdict` (variantes Preserved, Rejected, Unchanged).
- `src/memory_source.rs`: 8 testes para `as_str`, `TryFrom<&str>` (válido e inválido), `Display` e serialização.
- `src/reaper.rs`: 4 testes (`orphan_min_age_is_one_minute`, `orphan_targets_include_claude_and_codex`, `reaper_report_starts_zeroed`, `scan_completes_without_panic_on_linux`).
- `src/system_load.rs`: 5 testes para `load_average_one`, `ncpus` e `is_system_saturated`.
- `src/commands/vec.rs`: 3 testes para `vec orphan-list`, `vec purge-orphan` e `vec stats`.
- `src/commands/optimize.rs`: 1 novo teste para o conjunto de campos de `OptimizeResponse`; 2 testes existentes atualizados.
- `src/lock.rs`: 6 testes (sanitização de namespace, bloqueio de segunda invocação, isolamento por namespace, determinismo de db_hash, divergência de db_hash, flag force).
### Testes Serializados
- Todos os 8 testes OAuth-only são marcados `#[serial_test::serial(env)]` porque mutam o ambiente global via `unsafe { std::env::set_var(...) }` e `unsafe { std::env::remove_var(...) }`. Rodá-los em paralelo causaria race.
- A crate `serial_test` (já é dependência do projeto) fornece o atributo; os testes são auto-descobertos por `cargo nextest run` com semântica de execução serial.
### Tempo de Execução dos Testes
- Tempo total da suíte completa no host de referência: ~10 segundos para os 745 testes.
- O grupo OAuth-only adiciona ~0.04 segundos (mutação de env é rápida).
- Testes loom NÃO estão incluídos na contagem padrão; são gateados e devem ser rodados via `scripts/test-loom.sh`.
- Documentação da crate loom: `https://docs.rs/loom/latest/loom/`
- Repositório GitHub do loom: `https://github.com/tokio-rs/loom`
- Documentação do cargo-nextest: `https://nexte.st/`
- Referência de configuração do cargo-nextest: `https://nexte.st/docs/configuration/`
- Crate serial_test: `https://docs.rs/serial_test/latest/serial_test/`


## Notas da Suíte de Testes da v1.0.82
### Contagem de Testes e Flakes Conhecidos
- A v1.0.82 sai com 807 testes, 1 ignorado, 0 falhando (conforme o registro em gaps.md em 2026-06-15)
- Os quatro novos subcomandos (`pending`, `slots`, `embedding`, `pending-embeddings`) têm cada um 2-3 testes unitários e 1-2 testes de integração
- Os 5 novos ADRs (0036-0040) têm cada um um teste de regressão em `tests/` nomeado conforme o número do ADR
- Flake conhecido: `slot_enforces_max_concurrency` é sensível a temporização em runners lentos; ele é retentado automaticamente uma vez com backoff de 50ms antes de ser marcado como falho
- Flake conhecido: `pending-embeddings process reprocesses failed rows` exige sessão OAuth funcional; gateie-o em `tests/mock-llm/codex` estar no `PATH`
- A nova crate `fs4` (NÃO `fs2`) é exercitada em `src/llm_slots.rs::acquire_llm_slot`; o teste `llm_slots_acquire_release_cross_process` roda 2 processos filhos que disputam o mesmo slot
### Artefato do Plano de Testes
- Veja `docs/TEST_PLAN.pt-BR.md` para o plano consolidado de validação ponta a ponta em 10 fases, por versão
- O plano valida as migrações de schema V014 e V015, todas as 5 decisões de ADR, o novo exit code 19 e a mitigação do incidente de 401 OAuth do codex
- Rode da Fase 1 à Fase 10 do `docs/TEST_PLAN.pt-BR.md` em sequência com um banco novo a cada execução (o snapshot avulso `TEST_PLAN_v1.0.82.md` foi aposentado na v1.0.96)
