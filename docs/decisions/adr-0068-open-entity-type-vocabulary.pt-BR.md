# ADR-0068: v1.2.8 — O vocabulário de entity_type é aberto, não meramente mais amplo

- Status: Aceito
- Data: 2026-08-18
- Release: v1.2.8 (crate `1.2.8`)
- Substitui: a política implícita introduzida por `V001__init.sql` e estendida por `V008__expand_entity_types.sql`
- Substituído por: nenhum
- Relacionados: ADR-0069 (foreign keys em torno das migrations), GAP-SG-277, GAP-SG-278, GAP-SG-216, `V010__open_relation_vocabulary.sql`


## Contexto

`entity_type` aceitava treze valores. Qualquer outro rótulo era dobrado no mais próximo, terminando em `concept`, e a string que o chamador escreveu era destruída dentro do `impl Deserialize` antes que qualquer camada acima pudesse vê-la.

Medido no banco deste workspace em 2026-08-18:

| tipo | entidades | proporção |
| --- | --- | --- |
| `concept` | 10 902 | 69,3 % |
| todos os outros tipos | 4 842 | 30,7 % |
| **total** | **15 744** | |

Filtrar por `--entity-type concept` devolve dois terços do grafo, o que é indistinguível de não filtrar. O número mais eloquente é o pequeno: `person` tinha 17 nós num corpus que fala de pessoas o tempo todo.

O vocabulário também deixava de descrever o próprio domínio. Este corpus trata da construção de uma CLI em Rust, e os rótulos que o descreveriam — `crate`, `gap`, `flag`, `migration`, `schema` — não eram nenhum dos treze, então todos os cinco colapsavam em `concept`.

Duas restrições emolduraram a decisão:

- As regras do próprio projeto limitam um enum público a 12 variantes. `EntityType` tinha 13, ou seja, já violava essa regra, o que descartou "adicionar mais tipos" como remédio.
- O `gaps.md` já havia descartado recusar rótulos desconhecidos por padrão, porque agentes emitem rótulos livres pelo `--graph-stdin` e recusar os quebraria.


## Decisão

Abrir o vocabulário em vez de alargá-lo, reusando o padrão que este repositório já roda em produção.

A `V010__open_relation_vocabulary.sql` fez exatamente isso com as relações na v1.0.49: removeu o `CHECK` de `relationships.relation`, moveu a lista canônica para `parsers::CANONICAL_RELATIONS` como conselho, e deu ao `link` uma flag `--strict-relations` para chamadores que querem o conjunto fechado. Esse padrão nunca foi aplicado de volta ao `entity_type`, e o próprio comentário dela diz que segue a `V008__expand_entity_types.sql`.

Por consequência:

- `V017__open_entity_type_vocabulary.sql` remove o `CHECK` de `entities.type`.
- `EntityType` deixa de ser um enum. `CANONICAL_ENTITY_TYPES`, `is_canonical_entity_type` e `normalize_entity_type` espelham o trio das relações.
- `normalize_entity_type` impõe **apenas a forma** — trim, minúsculas, hífen vira underscore — e recusa somente rótulos que não poderiam ser palavra em vocabulário nenhum: vazio, só dígitos, com quebra de linha, ou acima de `MAX_ENTITY_TYPE_LEN` caracteres.
- Filiação nunca é motivo de recusa ali. Ela é imposta uma camada acima, pelo `--strict-entity-types`, onde o chamador a pediu.
- Um rótulo não canônico é reportado no array `warnings` da resposta e gravado como escrito.
- `graph entity-types` reporta o vocabulário que um banco realmente usa, com um sinalizador `canonical` por linha.


## Consequências

A correção é por **subtração**. O GAP-SG-277 propunha uma coluna `raw_type` para preservar o rótulo do chamador, e o GAP-SG-278 propunha uma tabela `entity_types` com foreign key mais uma chave XDG. Nenhum dos dois foi construído, e nenhum é necessário: sem nada dobrando o rótulo, ele sobrevive por construção. Uma tabela de vocabulário existiria só para recusar alguém, e a decisão é justamente nunca recusar.

Resíduo declarado, que nenhuma mudança posterior desfaz:

- Entidades gravadas antes da v1.2.8 permanecem em `concept`, e o rótulo original delas se foi. Reclassificá-las sem ele seria adivinhar, e este projeto não registra adivinhação como correção.
- `--entity-type concept` devolve **menos** desta release em diante, porque `framework` agora é gravado como `framework`.
- Um vocabulário aberto pode fragmentar o filtro com o tempo. Não há teto, por decisão do dono. `graph entity-types` é o instrumento que torna essa fragmentação visível, caso aconteça.

O caminho de recuperação existe, mas não foi executado: `enrich --operation entity-type-validate` agora consegue propor um rótulo específico em vez de escolher entre dez, e o scan dele ganhou o filtro de que precisava para mirar `concept`. Rodá-lo custa uma chamada de LLM por entidade, então a decisão é do operador.
