# ADR-0070: v1.2.8 — Operação de enrich que escreve no grafo precisa ver evidência, ou se abster

- Status: Aceito
- Data: 2026-08-18
- Release: v1.2.8 (crate `1.2.8`)
- Substitui: a convenção não escrita de que o `input_text` de uma operação era decisão local do seu call site
- Substituído por: nenhum
- Relacionado: ADR-0068 (vocabulário aberto de entity_type), GAP-SG-277, GAP-SG-278, GAP-SG-279


## Contexto

`enrich --operation entity-type-validate` decidia o tipo de uma entidade e escrevia a resposta em `entities.type`. O que ela mostrava ao modelo, por inteiro:

```rust
let input_text = format!("Entity: {ent_name}\nCurrent type: {ent_type}");
```

Duas linhas: o nome da entidade e o rótulo em disputa. Sem descrição, sem corpus ligado, sem vizinhos tipados. O `SELECT` logo acima lia `id, name, type` e parava, embora `entities.description` exista desde a V017 e a linha já estivesse em mãos.

Isso pesava mais do que pesaria em outra operação. A GAP-SG-277 mediu 10.898 de 15.744 entidades colapsadas em `concept` pelo vocabulário fechado, e a GAP-SG-278 apontou `entity-type-validate` como o caminho de volta. A operação encarregada de reparar dois terços do grafo era a que decidia com o insumo mais fraco do pipeline — uma chamada paga por entidade, julgando rótulos como `rd_gs` e `v017` por como são escritos.

Nada estava escondido. Estava invisível, que é diferente. O `format!` ficava entre cinco irmãos no mesmo módulo, cujas chamadas pareciam iguais de relance e carregavam de três a cinco campos cada. Ler o arquivo dizia que ele compilava.

Duas guardas já vigiavam as outras metades daquela decisão. `entity_type_vocabulary_contract` quebra o build quando o prompt e `CANONICAL_ENTITY_TYPES` discordam. `normalize_entity_type` restringe o que chega à coluna. Entre um prompt vigiado e uma saída vigiada ficava um insumo sem testemunha, e era o insumo que estava errado.

A assimetria dentro do próprio crate deixava isso claro. `entity-descriptions` tinha quatro chaves XDG para reunir evidência — `corpus_top_k`, `snippet_chars`, `neighbour_top_k`, `min_corpus_chars` — e um portão pré-chamada que se abstém quando não há de onde descrever. `entity-type-validate` não tinha nada disso. Uma operação foi projetada para olhar antes de escrever uma frase; a outra escrevia um rótulo de olhos fechados.


## Decisão

**Operação de enrich que escreve em `entities` ou `relationships` precisa receber evidência sobre o seu sujeito, e precisa se abster quando não houver nenhuma.**

Três partes, na ordem em que agem.

### 1. A evidência é reunida antes da chamada, a partir da fonte única

`call_entity_type_validate` seleciona `description` na consulta que já rodava e chama `load_entity_evidence_tuned` — a mesma montagem que `entity-descriptions` e o amostrador do `--status` leem. O reuso aqui não é economia de código. É o que faz os três concordarem sobre o que significa "o que sabemos desta entidade"; quando o amostrador media só corpos enquanto o escritor via também arestas, a qualidade reportada descrevia um corpus que nunca existiu.

### 2. O ajuste é por operação, porque elas compram quantidades diferentes

Quatro chaves, espelhando exatamente o caminho de descrição:

- `enrich.entity_type_validate.corpus_top_k` (8)
- `enrich.entity_type_validate.snippet_chars` (2000)
- `enrich.entity_type_validate.neighbour_top_k` (12)
- `enrich.entity_type_validate.min_corpus_chars` (40)

Elas começam nos valores do caminho de descrição porque a evidência necessária é a mesma. São chaves separadas porque uma operação escreve uma frase e a outra reescreve um rótulo em dez mil linhas, e o operador tem toda razão em pagar por mais contexto antes da segunda.

### 3. Ausência de evidência é motivo para abster, não licença para adivinhar

`should_abstain_from_type_judgement` recusa entidade sem descrição e sem corpus ligado suficiente, **antes** da chamada, com custo zero. Descrição sozinha passa: é magra, mas é uma afirmação sobre o sujeito, e não uma leitura do nome dele.

O schema de resposta ganhou `sufficient_evidence` e `validated_type` nulável, para o modelo ter onde pôr "não sei" que não seja um rótulo plausível. Essa forma é imposta pelo transporte, não pelo gosto: a OpenRouter envia todo schema sob `strict: true`, e esse modo exige que toda chave de `properties` esteja em `required` — campo "opcional" é requisição recusada, não requisição tolerante.


## Consequências

### O defeito estava em três operações, não em uma

`tests/enrich_input_evidence_gate.rs` foi escrito para impedir a volta disso. Na primeira execução ele reprovou duas operações que ninguém tinha olhado:

| operação | o que via | o que escrevia |
| --- | --- | --- |
| `weight-calibrate` | dois nomes de entidade, relação, peso atual | `UPDATE relationships SET weight` |
| `relation-reclassify` | dois nomes de entidade, relação atual | `UPDATE relationships SET relation, weight` |

Mesmo defeito, mesma classe, mesmo silêncio. As duas foram corrigidas selecionando `description` de cada extremo — duas colunas a mais no join que já rodava, e nenhuma consulta nova. Isso é típico de defeito de insumo, e não de acesso: o dado costuma estar a uma coluna de quem decidiu sem ele.

### O envelope passou a dizer o que um dreno pago mudou

Reclassificação, confirmação e sugestão descartada emitiam um `Done { entities: 1 }` idêntico. Um operador que pagou dez mil chamadas não conseguia responder "quantos rótulos mudaram" pela saída, só comparando com um backup. `EnrichItemResult::Retyped` carrega o rótulo anterior, o novo e o tamanho da evidência; `retyped` os conta no resumo; e um skip agora leva a razão ao chamador, em vez de deixá-la só no sidecar.

`Retyped` é variante nova, e não campos em `Done`, por aritmética: `Done` é construído em trinta lugares, e o enum é casado exaustivamente em três.

### Custos aceitos

A entrada do prompt cresce cerca do orçamento de evidência por item, multiplicado por quantas entidades o dreno tocar. Esse é o preço de a decisão ser fundamentada, e ele é limitado pelas quatro chaves acima.

Este ADR **não** registra qual insumo tem a melhor relação entre acerto e custo. Isso só é mensurável comparando amostras pagas com insumos diferentes no mesmo corpus, e continua aberto.

Entidade sem descrição, sem vizinho e sem corpus segue indecidível por qualquer insumo. Para ela a abstenção é a única resposta honesta, e agora não custa nada.


## Alternativas consideradas

**Manter o insumo de duas linhas e aceitar o ruído.** Recusada: a operação escreve numa coluna, e um palpite que chega ao armazenamento é indistinguível de uma medição depois que está lá.

**Acrescentar campos a `EnrichItemResult::Done`.** Recusada por custo: trinta sites de construção contra três braços de match.

**Reusar `load_entity_evidence` sem mudança, com as chaves do caminho de descrição.** Recusada: amarraria o orçamento de uma reescrita de dez mil linhas ao orçamento de escrever uma frase, e quem ajustasse um moveria o outro em silêncio.

**Isentar as duas operações de aresta, já que aresta não tem corpo.** Recusada: aresta não tem corpo, mas seus extremos têm descrição, e o grafo já as guardava.
