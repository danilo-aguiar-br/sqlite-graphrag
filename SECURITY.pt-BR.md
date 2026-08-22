Leia este documento em [inglês (EN)](SECURITY.md).


# Política de Segurança


## Versões Suportadas
- A tabela abaixo lista quais versões do sqlite-graphrag recebem correções de segurança atualmente
- Usuários em linhas descontinuadas são FORTEMENTE encorajados a atualizar para uma versão suportada
- Atualizar cedo reduz janela de exposição e alinha com a política de divulgação coordenada

| Versão  | Status        | Correções de Segurança     |
| ------- | ------------- | -------------------------- |
| 1.2.x   | Suportada     | Sim, recebe correções (linha atual; última v1.2.7 / crate 1.2.7) |
| 1.1.x   | Suportada     | Sim, recebe correções críticas; upgrade para 1.2.x recomendado |
| 1.0.x   | Suportada     | Sim, recebe correções críticas; upgrade para 1.2.x recomendado |
| 0.x     | Sem suporte   | Sem correções fornecidas   |

### Notas de segurança relevantes da v1.2.7
- **A proveniência do banco passou a ser publicada.** Todo verbo com efeito colateral emite `db_path_source` e `db_path_resolved` no envelope, então o chamador enxerga em qual banco a escrita realmente caiu
- Omitir `--db` NÃO usa o diretório atual: a chamada cai no banco XDG em silêncio e sai com exit 0. Os campos de proveniência tornam visível essa escrita no banco errado, antes silenciosa

### Notas de segurança relevantes da v1.2.6
- **Uma chave inexistente em `--select` ou `--filter` deixou de ser silenciada.** Antes o pedido degradava para um conjunto vazio com exit 0; agora é recusado com exit 2
- Um `--filter` passou a avaliar o conjunto inteiro em vez de apenas a página atual
- Um knob declarado sem efeito deixou de devolver exit 0
- Consequência de segurança: o chamador não pode mais confundir um pedido malformado com um resultado legítimo vazio

### Notas de segurança relevantes da v1.2.2
- **Envelope de falha nunca é filtrado.** A superfície de saída agent-native (`--filter`, `--select`, `--max-items`, …) remodela apenas linhas de resultado; um envelope com `error: true` ou `ok: false` chega ao chamador literalmente. O chamador nunca pode ser induzido a ler uma falha como conjunto de resultados vazio e bem-sucedido
- **Truncagem nunca é silenciosa.** `--truncate-content` e `--max-output-bytes` registram o que removeram em `agent_surface` e levantam a flag `truncated` de topo; `--max-output-bytes` descarta elementos do fim em vez de fatiar o texto JSON, então um envelope limitado ainda faz parse
- **`--no-input` recusa stdin de forma declarativa.** Todo leitor de stdin falha de antemão com exit 65 em vez de bloquear, então uma invocação desassistida não trava esperando entrada que nunca chegará. Precedência: flag > XDG `cli.no_input` > `false`
- Um `--filter` malformado sai com exit 2 em vez de devolver zero linhas, então um typo nunca é confundido com resultado vazio legítimo
- Schema permanece em **v16** (sem migração do DB principal); a superfície é aditiva e opt-in

### Notas de segurança relevantes da v1.2.1
- **Isolamento de claim por namespace** na fila sidecar do enrich: `dequeue_next_pending` / `count_eligible_pending` / resume exigem `operation` **e** `namespace`, de modo que um drain em um namespace não pode reivindicar ou processar trabalho pending de outro (reduz risco de processamento cross-namespace e efeito de circuit-breaker)
- Schema permanece em **v16** (sem migração do DB principal); mudanças são só de comportamento da fila sidecar

### Notas de segurança relevantes da v1.2.0
- Sem product env (`SQLITE_GRAPHRAG_*`) no hot path; config de runtime é flag > XDG `config set` > default
- `DEFAULT_EMBEDDING_DIM=1024` (sobrescrita via `--embedding-dim` / XDG `embedding.dim`; DBs existentes mantêm `schema_meta.dim`)
- GAP-SG-139: superfícies host aceitam `--db` como no-op documentado (`config`, `slots`, `cache`, `completions`); superfícies de grafo inalteradas
- Gate de qualidade offline: `scripts/e2e_offline_v120.sh`



## v1.1.05 Nota de Integridade — Escrita Atômica de Envelopes e Grafo (`--output`, merge, link)
- A v1.1.05 introduz `deep-research --output PATH` que grava o envelope JSON completo via algoritmo atomwrite (tempfile → fsync → rename) e emite um ack curto no stdout com checksum `blake3`
- O contrato de I/O permanece: JSON no stdout, logs no stderr. Nunca redirecione ambos para o mesmo arquivo com `&>` — isso contamina o parse JSON
- A flag global `--quiet`/`-q` suprime tracing não-erro no stderr, reduzindo ruído em pipelines agent/CI sem alterar o envelope
- Prefira `link --from-id`/`--to-id` quando a identidade for um ID numérico de entidade; nomes puramente numéricos são rejeitados por `validate_entity_name` (v1.1.05) para que `--create-missing` não crie entidades fantasma
- Merges auto-referenciais em `merge-entities` (target ID/nome também listado como source) são rejeitados ANTES de qualquer trabalho no DB (v1.1.05), protegendo a integridade do grafo contra word-splitting de shell
- Helpers compartilhados em `src/atomic_io.rs` (`write_atomic`, `write_json_atomic`) são unit-tested; a suite `tests/v1105_incident_bugs_regression.rs` cobre o caminho CLI
- Integridade do arquivo de saída: confira o checksum `blake3` do ack do stdout contra o conteúdo gravado se o pipeline exigir verificação ponta a ponta

## Reportando uma Vulnerabilidade
- OBRIGATÓRIO reportar questões de segurança via GitHub Security Advisories no repositório público `sqlite-graphrag` como canal privado preferencial
- Use o email daniloaguiarbr@proton.me apenas como fallback quando o reporte privado do GitHub estiver indisponível
- JAMAIS abra issue pública, pull request ou discussão no GitHub para relatos de segurança
- Inclua reprodução mínima, versões afetadas e comportamento esperado versus observado
- Inclua detalhes do ambiente como sistema operacional, arquitetura e versão do rustc
- Inclua estimativa de severidade CVSS 3.1 quando possível para acelerar triagem


## SLA de Resposta
- A triagem de cada advisory tem início comprometido em até 72 horas úteis após envio
- Email de reconhecimento inicial será enviado dentro dessa mesma janela de 72 horas
- Você receberá um identificador de caso e contato do mantenedor designado
- Atualizações de progresso são compartilhadas no mínimo a cada 7 dias até resolução ou divulgação


## SLA de Correção por Severidade CVSS
- Severidade crítica (CVSS 9.0 a 10.0) recebe patch em até 7 dias corridos após triagem validada
- Severidade alta (CVSS 7.0 a 8.9) recebe patch em até 14 dias corridos após triagem validada
- Severidade média (CVSS 4.0 a 6.9) recebe patch em até 30 dias corridos após triagem validada
- Severidade baixa (CVSS 0.1 a 3.9) recebe patch em até 90 dias corridos após triagem validada
- Correções liberadas seguem imediatamente com entrada no CHANGELOG e GitHub Security Advisory quando a linha afetada ainda estiver suportada


## Política de Divulgação
- Seguimos divulgação coordenada com janela padrão de 90 dias de embargo a partir do relato inicial
- O embargo pode ser encurtado quando correção é liberada antes de 90 dias
- O embargo pode ser estendido quando correção demanda mais tempo e o autor do relato concorda
- Divulgação pública inclui identificador CVE quando o impacto justificar
- Divulgação pública inclui o GitHub Security Advisory com versões afetadas e versão corrigida
- Crédito é atribuído ao autor do relato exceto se anonimato for explicitamente solicitado


## Política de Atualização de Segurança
- Patches para versões suportadas são entregues como nova release patch no crates.io e GitHub Releases
- Toda release é validada com o pipeline completo de 10 comandos descrito em CONTRIBUTING
- As verificações de advisory e de licença são executadas LOCALMENTE pelo operador com `cargo audit` e `cargo deny check advisories licenses bans sources`; o repositório não tem workflows de CI, e `cargo test` é o único gate automático
- Supply chain é protegida via pinagem `constant_time_eq = "=0.4.2"` para proteger MSRV 1.88
- Drift de MSRV de dependência transitiva é monitorado proativamente conforme política do PRD

## v1.0.76 Aplicação OAuth-Only de Credencial LLM — HISTÓRICA, o mecanismo protegido não existe mais
- SUPERFÍCIE VIGENTE, medida no binário 1.2.8 desta working tree: NADA em `src/` inicia subprocesso. Embedding e enriquecimento são chamadas HTTPS à OpenRouter, feitas no próprio processo e one-shot, selecionadas por `--embedding-backend openrouter|auto` e `--llm-backend openrouter|none`.
- CAMINHO VIGENTE DE CREDENCIAL: `config add-key --provider openrouter --from-stdin` guarda a chave em repouso sob XDG e a mantém fora do histórico do shell e da tabela de processos; `--openrouter-api-key <CHAVE>` sobrepõe esse armazenamento por uma invocação e FICA visível na tabela de processos, então prefira o armazenamento. `config list-keys` mostra fingerprints mascarados, `config remove-key` apaga uma chave e `config doctor` reporta qual camada resolveu. Variáveis de ambiente de produto não são lidas em runtime e não são canal de configuração.
- RISCO RESIDUAL VIGENTE, dito sem invenção: a chave fica num arquivo XDG protegido por permissão de sistema de arquivos e trafega ao provider por TLS. Não há guarda OAuth-only, não há whitelist de env-clear e não há fronteira de spawn neste build, porque não há processo filho — as proteções descritas no restante desta seção NÃO estão em vigor e não devem ser presumidas.
- TODO BULLET ABAIXO É HISTÓRICO. Ele registra a era de subprocesso, removida na v1.2.0, e é mantido como registro do que foi construído, nunca como garantia. Verificação: `ANTHROPIC_API_KEY` e `OPENAI_API_KEY` ocorrem zero vezes em `src/`, e `claude_runner.rs`, `codex_spawn.rs` e `ingest_claude.rs` não são mais arquivos deste repositório.
- HISTÓRICO: o build padrão era apenas LLM e one-shot, e cada chamada de embedding spawnava um subprocesso headless `claude code` ou `codex`.
- HISTÓRICO: o spawn ABORTAVA com `AppError::Validation` e código de saída 1 quando `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` eram detectadas no ambiente. Esse aborto não existe hoje, e nenhum código lê qualquer uma das duas variáveis.
- HISTÓRICO: o fluxo OAuth (assinatura Claude Pro/Max ou ChatGPT Pro) era o ÚNICO mecanismo de credencial aceito. Hoje o mecanismo aceito é uma chave de API da OpenRouter guardada em repouso sob XDG.
- HISTÓRICO: ambas as variáveis de chave de API estavam INTENCIONALMENTE AUSENTES da whitelist de env-clear em `claude_runner.rs`, `codex_spawn.rs` e `ingest_claude.rs`, como defesa em profundidade contra um refactor futuro mover a guarda. Essa defesa desapareceu junto com o risco que ela cobria: nenhum filho é iniciado, então nenhuma variável chega a um.
- HISTÓRICO: a flag `--bare` (que também exigiria uma chave de API) foi REMOVIDA de todo caminho executável na v1.0.69.
- HISTÓRICO: quatro testes `#[serial_test::serial(env)]` validavam o conjunto canônico de flags e o comportamento de aborto.
- JUSTIFICATIVA HISTÓRICA: `docs/decisions/adr-0011-oauth-only-enforcement.md` para o raciocínio completo e `docs/decisions/adr-0025-oauth-only-embedding.md` para a aplicação específica em embedding da v1.0.76.
- Migração: o operador que ainda dependa de `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` precisa migrar para uma chave da OpenRouter, porque desde a v1.2.0 nenhuma das duas é lida por nada.

## v1.0.83 Preservação de Credenciais de Provider Customizado (ADR-0041)
- HISTÓRICO: o build padrão PRESERVAVA sete variáveis de ambiente de provider customizado ao spawnar subprocessos `claude -p` ou `codex exec`, habilitando providers Anthropic-compatíveis (MiniMax/api.minimax.io, OpenRouter, AWS Bedrock, gateways corporativos)
- As variáveis preservadas são `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CODEX_ACCESS_TOKEN`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY` e `OTEL_EXPORTER_OTLP_ENDPOINT`
- HISTÓRICO: aquelas variáveis eram SEMANTICAMENTE DISTINTAS das rejeitadas pelo OAuth-only `ANTHROPIC_API_KEY` e `OPENAI_API_KEY`, e a guarda OAuth-only rejeitava as chaves de API com exit 1. Medido nesta árvore de trabalho: `claude_runner.rs`, `codex_spawn.rs` e `ingest_claude.rs` não são mais arquivos deste repositório, e guarda nenhuma roda
- HISTÓRICO: a whitelist vivia em um helper compartilhado `src/spawn/env_whitelist.rs` expondo `apply_env_whitelist(cmd, strict)` e `is_strict_env_clear()`, e os três spawners delegavam a ele. O diretório `src/spawn/` inteiro saiu junto com os backends de subprocesso
- HISTÓRICO: para ambientes de compliance que proíbem encaminhamento de credenciais via env vars (PCI-DSS, SOC2, HIPAA), operadores passavam --strict-env-clear, e o modo estrito preservava apenas `PATH` descartando as demais env vars. A flag viveu na superfície global até a v1.2.2, como `src/cli/globals.rs` ainda registra; um binário 1.2.8 responde unexpected argument '--strict-env-clear' found com exit 2. Nada encaminha credencial hoje, porque nada spawna. Product env `SQLITE_GRAPHRAG_*` também não é lida em runtime, logo não é o caminho de configuração
- HISTÓRICO: cinco testes de regressão `#[serial_test::serial(env)]` viviam em `tests/claude_runner_env.rs`, arquivo que este repositório não contém mais, cobrindo propagação de provider customizado, preservação do aborto OAuth-only, herança de base-URL pelo codex, descarte de credenciais no modo estrito e auditoria de no-leak que varre o stderr do subprocesso procurando o valor literal do token com `RUST_LOG=trace`
- Telemetria nenhuma é emitida em ponto algum deste produto, nem antes nem hoje. HISTÓRICO: a correção era silenciosa exceto quando a guarda OAuth-only disparava, emitindo um arg de marcador orientativo apontando para `ANTHROPIC_AUTH_TOKEN` ou `~/.codex/auth.json` como resoluções legítimas; essa guarda não existe mais
- Modelo de ameaça HISTÓRICO: valores de credencial para providers customizados fluíam do processo orquestrador para o subprocesso LLM pela fronteira de processo, o teste de auditoria de no-leak guardava contra uma macro `tracing` imprimir o token bruto no stderr, e operadores em hosts compartilhados eram orientados a preferir --strict-env-clear. Essa fronteira não existe na v1.2.8 e a flag é rejeitada com exit 2; a superfície de credencial hoje é uma única chave da OpenRouter lida do armazenamento XDG e enviada por TLS
- Veja `docs/decisions/adr-0041-preserve-custom-provider-env.md` (PT-BR) e `.md` (EN) para a decisão arquitetural completa e alternativas consideradas

## v1.0.87+ Camada de Validação Pre-flight (ADR-0045)
- Todo spawn de subprocesso LLM passa por src/spawn/preflight.rs (15 testes unitários, 7 guardas) ANTES do fork. Falhas retornam AppError::PreFlightFailed (exit code 16, EX_CONFIG).
- 7 guardas: check_argv_size, check_binary_exists, check_mcp_config_inline (substitui o literal --mcp-config '{}' por tempfile, corrige BUG-2), check_mcp_config_path, check_walkup_mcp_json (valida o walk-up de .mcp.json, corrige BUG-5), check_output_buffer (corrige BUG-4), check_claude_config_dir (evita vazamento de MCP).
- Bypass (somente emergências): `sqlite-graphrag config set spawn.skip_preflight 1` desabilita as 7 guardas. Opt-out de último recurso; o bypass reverte para Command::spawn() direto e herda as 5 classes de BUG. Product env não é o caminho suportado na v1.1.8.
- Hotfixes da v1.0.88: BUG-11 (falha de preflight em extract/llm_embedding.rs não propagava para remember; corrigido com embed_via_backend_strict), BUG-12 (OAuth-only emitia 2 linhas idênticas no stderr; corrigido com stderr de linha única), BUG-13 (link --create-missing burlava a validação de nome de entidade; corrigido validando ANTES de normalizar em entity_validation_integration.rs, 8 testes, fronteira de 4 caracteres).
- Veja docs/decisions/adr-0045-preflight-validation-layer.md e adr-0046-preflight-remediation.md para a decisão arquitetural completa.

## v1.0.89 Remediação do Pipeline de Embedding e Correções de Segurança (ADR-0050)
- BUG-YES-FLAG-IGNORED: três comandos destrutivos (slots release, purge, cleanup-orphans) declaravam --yes mas executavam deleções sem ele. Todos agora abortam com AppError::Validation quando --yes está ausente, alinhando com os 5 outros comandos destrutivos que já aplicavam isso
- BUG-BOOLISH-ENV: quatro flags booleanas de CLI (--skip-embedding-on-failure, --strict-env-clear, --dry-run-backend, --llm-slot-no-wait) rejeitavam valores Unix padrão de env (1, yes, on) com exit 2. Corrigido via BoolishValueParser. A forma `SQLITE_GRAPHRAG_SKIP_EMBEDDING_ON_FAILURE` é histórica e foi removida; passe o valor pela flag na linha de comando
- BUG-STRICT-ENV-PROPAGATION (HISTÓRICO): a flag de CLI --strict-env-clear era silenciosamente ignorada porque main.rs não a propagava para a env var. Corrigido à época via set_var antes do dispatch do comando; a própria flag foi removida depois da v1.2.2 e um binário 1.2.8 a rejeita com exit 2
- GAP-FLAGS-MORTAS: 7 flags globais de LLM eram aceitas pelo clap mas silenciosamente ignoradas porque módulos internos liam env vars diretamente. Corrigido: main.rs agora faz a ponte das flags de CLI para env vars via set_var
- GAP-RECALL-001: deadlock de embedding causado por slots de subprocesso LLM obsoletos resolvido via drop(stdin) explícito, timeout reduzido (300s para 30s), reaper de slots obsoletos e limpeza de processos órfãos do sqlite-graphrag
- Veja docs/decisions/adr-0050-embedding-deadlock-remediation.md para a decisão arquitetural completa

## v1.0.93 Tratamento de Chave API OpenRouter (ADR-0052)
- v1.0.93 introduz `--embedding-backend openrouter` que usa uma chave de API real (NÃO OAuth) para chamadas REST diretas ao OpenRouter
- A chave é fornecida via flag `--openrouter-api-key` ou XDG `config add-key openrouter` (`OPENROUTER_API_KEY` não é lida em runtime; G-T-XDG-04)
- A chave é encapsulada em `secrecy::SecretString` e zeroizada no drop — JAMAIS mantida como String plana na memória após inicialização
- A chave JAMAIS é logada no stderr mesmo em nível `RUST_LOG=trace`
- A chave JAMAIS é persistida no `graphrag.sqlite` ou em qualquer arquivo de cache
- A chave JAMAIS é encaminhada para subprocessos LLM (claude, codex, opencode) — flui apenas para chamadas HTTPS `reqwest` para `api.openrouter.ai`
- HISTÓRICO: isto era SEMANTICAMENTE DISTINTO do enforço OAuth-only nos backends LLM, em que `ANTHROPIC_API_KEY` e `OPENAI_API_KEY` ABORTAVAM com exit 1. Nenhuma das duas é lida hoje
- Nota histórica: `OPENROUTER_API_KEY` nunca esteve na whitelist de env-clear; o produto nunca lê essa env agora — use flag ou `config add-key` apenas
- Operadores em hosts compartilhados DEVEM preferir a flag `--openrouter-api-key` ao invés da variável para minimizar janela de exposição
- Veja `docs/decisions/adr-0052-openrouter-embedding-backend.md` para a decisão arquitetural completa

## Hall da Fama
- Reconhecemos publicamente pesquisadores que reportam vulnerabilidades de forma responsável
- Esta seção está aberta a contribuições: seu nome será adicionado após divulgação coordenada
- Se preferir anonimato, respeitamos essa preferência sem exceção


## Melhores Práticas para Usuários
- SEMPRE instale releases publicadas com `cargo install sqlite-graphrag --locked`
- Use `cargo install --path .` apenas quando estiver testando intencionalmente um checkout local não publicado
- SEMPRE rotacione seus tokens de API do `crates.io` em intervalo regular
- SEMPRE mantenha sua toolchain rustc atualizada na última release estável compatível com MSRV 1.88
- SEMPRE revise entradas do CHANGELOG antes de atualizar entre versões majors
- JAMAIS commite segredos ou tokens no repositório ou em forks derivados
- JAMAIS desabilite o memory guard em produção via flags não documentadas
- JAMAIS eleve concorrência de comandos pesados cegamente em hosts com memória restrita; prefira execução serial em auditorias
- JAMAIS ignore warnings do `cargo audit` sem abrir um advisory de segurança rastreado
- Conselho HISTÓRICO, mantido como registro: JAMAIS defina `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` no ambiente, porque o spawn abortava com exit 1. Um binário 1.2.8 não lê nenhuma das duas e não spawna nada, então defini-las não muda nada — e removê-las não protege nada.
- Conselho HISTÓRICO: JAMAIS dependa do encaminhamento de `ANTHROPIC_AUTH_TOKEN` quando o host é compartilhado com processos não confiáveis, preferindo --strict-env-clear para que credenciais permanecessem apenas no processo pai. Tanto o encaminhamento quanto a flag acabaram; em host compartilhado a exposição a vigiar hoje é `--openrouter-api-key` na linha de comando, visível na tabela de processos — guarde a chave com `config add-key --provider openrouter --from-stdin`.
- JAMAIS faça commit de chaves OpenRouter (nem valores de `OPENROUTER_API_KEY`, que o produto ignora) no repositório ou em forks derivados
- SEMPRE use a flag `--openrouter-api-key` em vez da variável de ambiente em hosts compartilhados
- v1.1.06: o wall-clock do primeiro scan de `enrich --operation entity-connect` / `cross-domain-bridges` usa `InterruptHandle` e devolve Timeout exit **1** (não exit **75** de singleton). Orquestradores não devem tratar timeout de scan como contenção de lock. Veja ADR-0066 e `docs/HEADLESS_INVOCATION.pt-BR.md`.


## v1.0.94 Hardening do Modo Headless (ADR-0053)
- A v1.0.94 torna `enrich --mode` OBRIGATÓRIO (removido o default `claude-code`); omitir é rejeitado pelo clap com exit 2.
- Isso evita um spawn acidental de `claude -p` que herdaria o `.mcp.json` do projeto do chamador e executaria servidores MCP não confiáveis em contexto headless.
- Nenhum novo exit code e nenhuma nova variável de ambiente são introduzidos; a mudança é apenas uma superfície de default mais segura.
- Modos válidos são `claude-code`, `codex`, `opencode`; escolha o que casa com seu `--llm-backend`.


## v1.0.95 Tratamento de Chave de Chat OpenRouter (ADR-0054)
- A v1.0.95 adiciona `enrich --mode openrouter`, que roteia a etapa JUDGE ao `/chat/completions` do OpenRouter via HTTPS (`src/chat_api.rs`) em vez de spawnar uma CLI local.
- Ele reutiliza a MESMA credencial OpenRouter já documentada para o backend de embedding (flag / `config add-key`; `OPENROUTER_API_KEY` não é lida em runtime), com o MESMO tratamento: envolvida em `secrecy::SecretBox`, zeroizada no drop, JAMAIS logada, JAMAIS passada a qualquer subprocesso.
- A chave flui apenas para o cliente HTTPS `reqwest` que aponta para `openrouter.ai`; não está na whitelist de env-clear e permanece apenas no processo pai.
- Nenhuma nova superfície de credencial é introduzida além da já documentada para o backend de embedding OpenRouter.


## v1.0.96 Fan-out REST Bounded e Convergência por Dead-letter (ADR-0055)
- A v1.0.96 adiciona concorrência bounded ao path REST de embedding OpenRouter (`embed_passages_parallel_with_embedding_choice`); as requisições in-flight são clampadas em 1..=16 (padrão 8, faixa Cloudflare-safe) via `tokio::task::JoinSet`, SEM nova dependência.
- O fan-out apenas paraleliza leituras HTTPS de saída para `openrouter.ai`; NÃO amplia a superfície de credencial ou de rede, e o MESMO tratamento da chave OpenRouter (flag/`config add-key`, secrecy/zeroize, jamais logada, jamais passada a subprocesso; env de produto ignorada) permanece inalterado.
- Escritas SQLite permanecem serializadas via WAL mais claim atômico — o banco continua single-writer; a fila dead-letter (`error_class`, `next_retry_at`, terminal `dead`) apenas agenda retries e jamais contorna o lock de escrita.
- `enrich --status` é read-only: inspeciona a fila sem chamada de LLM e sem adquirir o singleton, portanto é seguro integrar a hooks e timers.
- Nenhum novo exit code e nenhuma nova superfície de credencial são introduzidos.
