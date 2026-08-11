# ADR-010 — Isolamento de Servidores MCP via CLAUDE_CONFIG_DIR (G28-A)

- Status: Aceito
- Data: 2026-06-03
- Release Alvo: v1.0.68
- Relaciona-se a: G28 (Proliferação de Processos), issue anthropics/claude-code#10787

## Contexto

Um incidente de produção em 2026-06-03 revelou que uma invocação de `sqlite-graphrag enrich` contra um banco de 5 mil memórias spawnou 276 processos numa workstation Linux, com load average sustentado de 12,7. A análise de causa raiz rastreou o fan-out até dois eixos de multiplicação:

1. `--llm-parallelism 2` spawna 2 subprocessos `claude -p` concorrentes por invocação de `enrich`
2. Cada subprocesso `claude -p` inicia sua própria frota de servidores MCP (~8–10 servidores do `~/.claude.json` do usuário)
3. Mais 2 invocações irmãs de `enrich` rodando concorrentemente (totalizando 4 processos × 10 servidores ≈ 40 subprocessos MCP)

A correção esperada era passar `--mcp-config '{}'` ou `--strict-mcp-config` para suprimir a carga de servidores MCP de escopo de usuário. **Essa correção não funciona na prática.**

## Investigação

Uma busca direcionada no DuckDuckGo trouxe à tona [anthropics/claude-code#10787] com o título "[BUG] Claude CLI Ignores `--mcp-config` and `--strict-mcp-config` Flags". A leitura da thread da issue e da documentação do Claude Code confirmou:

- `--mcp-config <path>` é documentada, mas o Claude Code v2.x a ignora silenciosamente quando o caminho resolve para uma config vazia ou para uma config que omite a chave `mcpServers`
- `--strict-mcp-config` foi adicionada no Claude Code 2.0.0, mas a flag é parseada e imediatamente descartada pelo parser da CLI, sem efeito sobre quais servidores MCP são carregados
- O único mecanismo que suprime de forma confiável a frota MCP de escopo de usuário é a variável de ambiente `CLAUDE_CONFIG_DIR`, que aponta a CLI para uma raiz de config diferente

Esse achado invalidou o plano original de mitigação.

## Decisão

Adotar `CLAUDE_CONFIG_DIR` como o mecanismo canônico de isolamento de servidores MCP, exposto por uma nova variável de ambiente do `sqlite-graphrag` chamada `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR`.

Contrato de comportamento:

1. Quando `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` não está definida, `claude_runner::build_claude_command` continua usando o `CLAUDE_CONFIG_DIR` herdado do processo pai (comportamento atual, totalmente retrocompatível)
2. Quando `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` está definida com um caminho:
   - Se o caminho existe e é um diretório: `cmd.env("CLAUDE_CONFIG_DIR", <path>)` é adicionado ao subprocesso, mascarando os servidores MCP do usuário
   - Se o caminho está ausente ou não é um diretório: emitir um único `tracing::warn!` e seguir sem definir `CLAUDE_CONFIG_DIR` (degradado, mas sem falhar)
3. A CLI nunca cria o diretório automaticamente; o usuário DEVE pré-criar um diretório vazio para optar por participar
4. A CLI nunca apaga o diretório; o usuário é dono do ciclo de vida

Por que não as flags quebradas:

- `--mcp-config` e `--strict-mcp-config` são silenciosamente ignoradas pelo Claude Code v2.x conforme documentado na issue #10787
- O bug upstream está aberto desde 2026-04 e não mostra progresso rumo a uma correção
- O custo de fingir que essas flags funcionam é falha silenciosa: o usuário as habilita, não vê aviso algum e a proliferação continua

## Consequências

Positivas:

- Redução de fan-out funciona hoje: definir a variável de ambiente derruba a contagem de subprocessos de ~192 para ~8 por invocação de `enrich`
- Totalmente retrocompatível: usuários existentes sem a variável de ambiente não veem mudança
- Nenhuma dependência do ciclo de release do Claude Code: a variável de ambiente faz parte do Claude Code v1.x e permanece na v2.x
- Ponto único de controle: uma variável de ambiente suprime servidores MCP em todas as invocações de `claude -p` spawnadas pelo sqlite-graphrag

Negativas:

- Descobribilidade: a variável de ambiente é específica do `sqlite-graphrag`, não nativa do `claude`, então usuários lendo a documentação do Claude Code não a encontrarão
- Override por invocação: não há flag por chamada; a variável de ambiente é global para o processo pai
- Configuração manual: o usuário deve pré-criar o diretório vazio e definir a variável de ambiente no perfil do shell ou na unit do systemd

Mitigações:

- O `tracing::warn!` no `enrich` quando `--llm-parallelism > 4` recomenda a variável de ambiente em forma legível por humanos
- `docs/HOW_TO_USE.md` e `docs/COOKBOOK.md` incluem uma receita pronta para copiar e colar
- `skill/sqlite-graphrag-en/SKILL.md` e `docs/AGENTS.md` documentam a variável de ambiente na seção G28
- `INTEGRATIONS.md` e `llms.txt` descrevem o comportamento no changelog da v1.0.68

## Alternativas Consideradas

### Opção 1: Usar `--mcp-config '{}'`

Rejeitada: silenciosamente ignorada conforme a issue #10787.

### Opção 2: Usar `--strict-mcp-config`

Rejeitada: silenciosamente ignorada conforme a issue #10787.

### Opção 3: Definir a variável de ambiente `DISABLE_MCP=1`

Rejeitada: essa variável de ambiente não é honrada pelo Claude Code v2.x; o nome oficial é `CLAUDE_CONFIG_DIR`.

### Opção 4: Spawnar `claude -p` via um wrapper que filtra `~/.claude.json` antes do exec

Rejeitada: invasiva demais, exige chamar um binário customizado que o usuário precisa instalar e quebra o modelo determinístico de subprocessos do qual a suíte de testes de CI depende.

### Opção 5: Documentar a variável de ambiente e deixar o usuário defini-la manualmente

Aceita como o caminho mínimo viável; o aviso do `enrich` e a documentação tornam o fluxo do usuário descobrível.

## Notas de Implementação

- Código novo: `src/commands/claude_runner.rs:228–247` lê a variável de ambiente, valida o caminho e condicionalmente define `CLAUDE_CONFIG_DIR` no `Command`
- Nova constante: `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` registrada em `src/constants.rs` e em `src/i18n.rs` para a string de aviso em PT-BR
- Novo ponto de tracing: o `enrich` emite `tracing::warn!` em `src/commands/enrich.rs:1115–1124` quando `--llm-parallelism > 4`
- Sem novos testes unitários para a variável de ambiente: o caminho de código é direto e o teste de integração `cargo test --test enrich_warnings` (se adicionado depois) exigiria um binário Claude mockado
- Sem mudança de schema: a variável de ambiente é somente de entrada, não faz parte de nenhuma saída JSON

## Referências

- Issue no GitHub: anthropics/claude-code#10787 "Claude CLI Ignores `--mcp-config` and `--strict-mcp-config` Flags"
- Query de busca no DuckDuckGo: "claude code cli mcp strict mcp config empty flag"
- Fonte: `src/commands/claude_runner.rs:204–254` (`build_claude_command`)
- Fonte: `src/commands/enrich.rs:1108–1124` (aviso de paralelismo)
- Fonte: `src/i18n.rs` (string PT-BR do aviso)
- Documentação: receitas de `docs/HOW_TO_USE.md` e `docs/COOKBOOK.md` para a v1.0.68
