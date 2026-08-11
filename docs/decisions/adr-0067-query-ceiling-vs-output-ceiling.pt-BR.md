# ADR-0067: v1.2.6 — Teto de consulta vs teto de saída, e exit 2 para pedido incoerente

- Status: Aceito
- Data: 2026-08-10
- Release: v1.2.6 (crate `1.2.6`), emendado na v1.2.7
- Supersede: nenhum
- Supersedido por: nenhum
- Relacionado: ADR-0042 (`backend_invoked`), GAP-SG-201 até GAP-SG-207


## Contexto

A camada agent-native (`crate::agent_surface`) reformata um envelope já serializado. Ela fica portanto **a jusante do `LIMIT` do SQL** e não enxerga o que a consulta removeu antes de ela rodar.

Dois tetos independentes existiam sem precedência declarada entre eles:

| teto | aplicado por | remove |
| --- | --- | --- |
| teto de consulta | `--limit` / `-k` / `--max-results` do subcomando | linhas, antes da serialização |
| teto de saída | `--max-items`, `--max-output-bytes` | elementos, depois da serialização |

Medido sobre um corpus de 1892 memórias:

```
--filter type=skill --count-only list              → 39   (input_count 1892)
--filter type=skill --count-only list --limit 50   → 0    (input_count 50, exit 0)
```

Os dois números saem do mesmo código. Só um responde à pergunta que o chamador fez. Um array de cinquenta é indistinguível de um corpus de cinquenta, então o predicado mudou de sentido em silêncio e reportou sucesso.

A mesma cegueira produziu outras quatro formas que respondiam a um pedido impossível com `exit 0`: uma chave que nenhum elemento carrega, um predicado redirecionado para um array que o chamador nunca nomeou, um knob declarado contra um envelope sem array de resultado, e um verbo que muta cujo alvo vinha do ambiente em vez do argv.

`exit 0` em qualquer um desses é o modo de falha que importa: o agente lê "vazio" como "o dado não está lá", conclui que a memória não existe, e grava uma duplicata.


## Decisão

1. **O comando declara o próprio teto.** `crate::agent_surface::universe::record` é chamado na linha que resolve o limite efetivo, onde a origem e — num comando paginado — o total do universo são ambos conhecidos.
2. **Paginação e top-k são distintos.** `list` e `graph entities` paginam um universo contável, então "o teto cortou alguma coisa" tem resposta factual. `hybrid-search -k`, `recall -k`, `related --limit` e `deep-research --max-results` limitam um ranking: o top-k **é** a resposta, não a truncagem de uma.
3. **Recusar somente sob evidência.** Um predicado só é recusado quando o teto é `Pagination` **e** removeu linhas de fato. Um top-k nunca é recusado; é reportado, para que sua estreiteza deixe de ser invisível.
4. **Instrumentar sempre, recusar estreito.** Todo comando de leitura reporta `query_limit`, `query_limit_kind`, `query_limit_source` e `filter_scope`, qualquer que seja o veredito.
5. **Nunca recusar depois de uma mutação.** A camada roda no momento da saída, depois que o handler já fez o trabalho. `Commands::mutates` é a cerca: uma escrita é anotada, nunca recusada.
6. **Pedido incoerente sai com exit 2.**
7. **O alvo resolvido é sempre reportado**, e verbo com efeito colateral é recusado quando NADA o nomeou. Só `TargetSource::Default` merece a recusa: `db.path` é chave de primeira classe no registry, então alvo XDG é designação que o operador fez uma vez em vez de a cada invocação, e rejeitá-la tornaria a própria superfície de configuração do produto inutilizável.


## Por que exit 2 e não EX_USAGE 64

O `sysexits.h` define `EX_USAGE` como `64` para uso incorreto do comando — número errado de argumentos, flag ruim, sintaxe ruim. É um encaixe genuíno para as recusas acima, que são exatamente combinações inválidas de argumentos.

Ainda assim foi rejeitado, por um motivo: **este binário já fixou `2` como seu código de uso incorreto**, em `src/main.rs`. A `rules-rust-cli-com-clap-io-exitcodes-erros` proíbe reaproveitar um código de saída para duas semânticas — e adotar `64` agora faria exatamente isso ao contrário, criando **dois** códigos significando "você usou o comando errado" dentro do mesmo binário. O consumidor teria de saber qual subsistema levantou o erro para saber qual número esperar.

Coerência interna vence convenção externa quando as duas conflitam e a convenção nunca foi adotada aqui. O `2` também é o que o próprio `clap` devolve numa falha de parse, então a CLI passa a responder com um número para um significado nas duas camadas.


## Consequências

### Positivas

- Um pedido impossível falha alto, em vez de devolver conjunto vazio com `exit 0`.
- O chamador aprende o que a consulta removeu mesmo quando nada é recusado.
- `discarded_flags` nomeia como dado os argumentos descartados, sem parse de prosa.
- O banco resolvido aparece em todo envelope, tornando detectável uma escrita mal endereçada.

### Negativas

- Um script que filtrava uma página e aceitava a resposta agora falha. `--filter-scope page` restaura, declarando a intenção mais estreita.
- Um verbo que muta cujo alvo ninguém nomeou agora sai com exit 2. `--use-active` restaura o comportamento anterior, de forma explícita, e `config set db.path` segue sendo designação plenamente suportada.
- O envelope de um comando que toca banco deixa de ser byte a byte o que era antes da camada existir: ele carrega `agent_surface.db_path_source`.

### Neutras

- Nenhuma migração de schema. Nenhuma dependência nova. Os 66 schemas que fecham a raiz já declaravam `agent_surface`, então o alvo não precisou de membro na raiz.


## Emenda na v1.2.7

A v1.2.6 anexou o registro do alvo dentro de `base_meta`, que roda a jusante de dois curto-circuitos: `emit_json` pula a camada quando nenhum knob está ligado, e `apply` retorna cedo quando a superfície é inerte. O alvo aparecia portanto **só para um chamador que já tinha ligado uma flag não relacionada**, e era omitido no caminho default — o caminho que todo agente usa.

Um contrato universal não pode ficar pendurado num bloco opcional. A v1.2.7 mudou a condição de entrada na camada de "há um knob" para "há um knob **ou** há um alvo a reportar", e deu ao `apply` um caminho inerte que anota sem reformatar.

Esta foi a terceira vez numa mesma release em que um contrato se apoiou num proxy em vez do fato que dizia observar. As outras duas estão registradas em GAP-SG-202 e GAP-SG-203.


## Referências

- `gaps.md` — GAP-SG-201 até GAP-SG-207
- `docs/schemas/agent-surface.schema.json` — o registro declarado
- `https://man.openbsd.org/sysexits` — `EX_USAGE`
- `https://www.man7.org/linux/man-pages/man3/sysexits.h.3head.html` — `EX_USAGE`
