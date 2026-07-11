# ADR-0065: v1.1.05 — Cinco Bugs Operacionais do Incidente de Deep-Research sobre "danilo"

- Status: Accepted
- Data: 2026-07-11
- Release: v1.1.05 (crate `1.1.5`)
- Substitui: nenhum
- Substituído por: nenhum
- Relacionado: ADR-0063 (onda de correções v1.1.03), ADR-0064 (nested-runtime do deep-research + entity-connect v1.1.04), ADR-0044 (padrão anterior de hotfix multi-bug)


## Contexto

Em 2026-07-08 um operador executou pesquisa multi-hop profunda contra um grafo de produção grande (`graphrag.sqlite`, binário v1.1.4) sobre o sujeito `"danilo"`. Cinco bugs da CLI bloquearam a investigação; um sexto erro do lado do shell amplificou um deles. Estão catalogados em `gaps.md` ("Bugs do GraphRAG — Relato de Deep Research sobre danilo") e fechados na v1.1.05. Nenhuma migração de schema SQLite é necessária (`CURRENT_SCHEMA_VERSION` permanece em 16 desde V016 / ADR-0064).

| # | Sintoma | Causa raiz (resumo) |
|---|---------|---------------------|
| Bug 1 | `deep-research "danilo"` produzia uma única busca híbrida em vez de fan-out multi-aspecto | A heurística `decompose_query` era puramente sintática; tokens únicos nunca se dividiam |
| Bug 2 | `jaq`/`jq` falhava ao parsear a captura completa de stdout | Truncamento do envelope / contaminação de stderr sob redirecionamentos do shell (`&>`), não `serde_json` inválido |
| Bug 3 | `graph traverse --from danilo` retornava vazio / NotFound opaco | Match apenas de nome exato; apelidos curtos nunca resolviam para nomes canônicos kebab |
| Bug 4 | `merge-entities` aceitava merges auto-referenciais sob argv malformado | A guarda existia mais fundo no caminho; word-splitting do zsh ainda podia colocar o alvo em `--ids` antes de o trabalho de DB ser evitado cedo o bastante |
| Bug 5 | `link --from 89975 --create-missing` criava entidade fantasma chamada `"89975"` | Strings numéricas tratadas como nomes; sem flags de link por ID |
| Erro de Shell 1 | word-splitting do zsh corrompia comandos de merge multi-arg | Higiene de shell (arrays); mitigado na CLI pelo Bug 4 |

A v1.1.04 tornou `deep-research` *executável* de novo (panic de Tokio aninhado, ADR-0064 GAP-001), mas não corrigiu o caminho de qualidade de token único que tornava inútil a pesquisa sobre um sujeito-nome-de-pessoa.


## Decisão

Aplicar cinco correções cirúrgicas de CLI/UX (mais I/O atômico compartilhado) sem avançar o schema do banco.

### D1 — Bug 1: fan-out de aspectos para token único

- Renomear o caminho de planejamento para `decompose_query_with_sources(query, max) -> Vec<(String, &'static str)>`.
- Manter os ramos sintáticos existentes (frases relacionais, `;`, `and`/`e`/vírgulas, pares multi-palavra).
- Quando **nenhum** ramo dispara e a query é um **token único**, expandir em:

  1. o token original (`source: "original"`), depois
  2. facetas `"{token} {aspect}"` (`source: "aspect"`) de `SINGLE_TOKEN_ASPECTS` (facetas EN/PT: patrimônio/stack/tecnologia/stakeholders/pessoas/projeto/decisão/relacionamento/contexto/architecture/history), limitadas por `--max-sub-queries` (padrão 7).

- Queries multi-palavra inseparáveis ainda retornam uma única sub-query `original` (sem ruído falso de aspectos).
- Override manual permanece de primeira classe: `--sub-query-strategy manual --sub-queries-file PATH` rotula linhas como `source: "manual"`.

### D2 — Bug 2: `--output` atômico + `--quiet` global + contrato documentado

- Novo `deep-research --output PATH` grava o envelope completo via `atomic_io::write_json_atomic` (tempfile → fsync → rename).
- Quando `--output` está setado, stdout emite um ack curto `{ written, bytes, blake3, sub_queries_total, unique_memories_found, elapsed_ms }` em vez do envelope multi-MB.
- `--quiet` / `-q` global suprime tracing que não seja erro para stderr não contaminar capturas.
- O help longo documenta o contrato: stdout = só JSON; stderr = logs; nunca `&>` o mesmo arquivo.

### D3 — Bug 3: resolução fuzzy de entidade em `graph traverse`

- Adicionar `entity_name_similarity`, `suggest_entity_names` e `resolve_entity_fuzzy` (Jaro-Winkler rapidfuzz + heurísticas de prefixo kebab / primeiro token).
- Match exato ainda vence.
- Sem `--fuzzy`: NotFound (exit 4) inclui sugestões ranqueadas de nomes canônicos.
- Com `--fuzzy`: um vencedor único claro é auto-resolvido; um aviso em stderr registra a substituição.

### D4 — Bug 4: guarda pré-DB de merge auto-referencial

- No início de `merge-entities::run`, rejeitar quando `--into-id` ∈ `--ids` ou `--into` ∈ `--names` **antes** de qualquer abertura/resolução de DB.
- Manter a rechecagem de defesa em profundidade no momento da resolução.
- Fecha o caminho de amplificação por word-splitting do shell que poderia corromper o grafo sob `--cross-namespace`.

### D5 — Bug 5: link por ID + rejeitar nomes só de dígitos

- Novas flags mutuamente exclusivas: `--from-id` / `--to-id` ao lado de `--from` / `--to`.
- `validate_entity_name` rejeita nomes puramente de dígitos ASCII para que `--create-missing` não possa mintar entidades fantasmas com aparência de ID.
- A mensagem de erro orienta o operador a `--from-id`/`--to-id`.

### Infraestrutura compartilhada

- Novo módulo `src/atomic_io.rs` (`write_atomic`, `write_json_atomic`) reutilizado pelo Bug 2 e com testes unitários.
- Suite de integração `tests/v1105_danilo_bugs_regression.rs` cobre os cinco bugs na fronteira da CLI.


## Alternativas Consideradas

1. **Decomposição de query por LLM para tokens únicos (Bug 1)** — Rejeitada no caminho padrão: adiciona custo, latência e dependência OAuth a um comando local-first cujo `--mode` padrão permanece heurístico `none`. A estratégia manual já cobre listas de facetas de especialistas.
2. **Apenas documentar "cite seus redirecionamentos" para o Bug 2** — Rejeitada como única correção: envelopes multi-MB com `--with-bodies` ainda competem sob SIGTERM/buffers de pipe; atomwrite é o contrato durável para agentes.
3. **Auto-fuzzy sempre ligado sem flag (Bug 3)** — Rejeitado: resolução silenciosa pode atravessar a entidade errada em namespaces densos. O padrão permanece exact + sugestões; `--fuzzy` opt-in para recuperação interativa.
4. **Somente `value_parser` do clap para self-merge (Bug 4)** — Insuficiente sozinho: IDs chegam como `Vec` após o parse; a validação deve comparar conjuntos. A guarda pré-DB é a camada correta.
5. **Auto-detectar dígitos puros como IDs de entidade em `--from`/`--to` (Bug 5)** — Rejeitado: ambíguo (nomes reais poderiam ser numéricos em teoria) e surpreende scripts. `--from-id`/`--to-id` explícitos mais rejeição dura de *nomes* só de dígitos é mais seguro.
6. **Migração de schema / novas tabelas para qualquer um dos cinco** — Rejeitada: os cinco são preocupações de CLI/resolução/saída; o modelo de dados do grafo não muda.


## Consequências

### Positivas

- Deep-research de token único produz cobertura multi-aspecto (`source: "aspect"`) sem custo de LLM.
- Envelopes JSON grandes são seguros a crash via atomwrite; pipelines verificam `blake3` no ack.
- Apelidos curtos são recuperáveis via sugestões ou `--fuzzy`.
- Merges auto-referenciais falham alto antes de qualquer escrita; erros de shell não podem orphanar arestas.
- Uso indevido de ID numérico não cria entidades fantasma; link por ID é de primeira classe.
- Nenhum passo de migração para o operador (`migrate` não é obrigatório neste release).

### Negativas

- A lista de facetas de aspecto é uma heurística fixa EN/PT — imperfeita para domínios arbitrários; operadores que precisam de facetas específicas de domínio devem usar `--sub-query-strategy manual`.
- `--fuzzy` ainda pode escolher um near-match errado se duas entidades pontuarem de forma similar; operadores devem preferir nomes canônicos exatos em automação.
- `deep-research.schema.json` historicamente enumerava `sub_queries[].source` apenas como `original | decomposed`; o runtime agora também emite `aspect` e `manual` (nota em `docs/schemas/README.md`; regen opcional de schema é não-bloqueante para consumidores Must-Ignore-friendly, mas validadores estritos devem regenerar).

### Neutras

- O SemVer do crate é `1.1.5` enquanto a marca do release é **v1.1.05** (zero à esquerda rejeitado pelo SemVer do cargo).
- `CURRENT_SCHEMA_VERSION` permanece **16**.
- O Erro de Shell 1 continua sendo principalmente higiene do operador; a CLI apenas endurece o caminho de merge.


## Validação

- Unitário: `test_decompose_single_token_danilo_fans_out`, testes de atomic_io, guardas clap/ID para link e merge.
- Integração: `tests/v1105_danilo_bugs_regression.rs` (`bug1`…`bug5`).
- A tarefa de docs não reexecuta a suíte completa; as tarefas de implementação já fecharam os cinco bugs.


## Commits

- Implementação dos Bugs 1–5 + `atomic_io` + suite de regressão (tarefas de código).
- Este ADR (EN + PT-BR), `docs/decisions/INDEX.md` e a nota de schemas v1.1.05 fecham o lado de documentação do release.
- Rastreador primário: tabela de status em `gaps.md` (todos os cinco **FIXED** na v1.1.05).
