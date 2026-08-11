# Changelog

- Leia este documento em [inglês (EN)](CHANGELOG.md).

Todas as mudanças notáveis deste projeto serão documentadas neste arquivo.

## [1.2.7] - 2026-08-10

### O alvo resolvido chega a todo envelope, e uma escrita precisa nomeá-lo

A v1.2.6 declarou o GAP-SG-205 resolvido. Uma auditoria e2e daquela release mediu o contrário, e esta entrada existe porque o veredito estava errado, não porque algo novo quebrou.

- **GAP-SG-205 (reaberto e fechado)** — o registro do alvo era anexado dentro de `base_meta`, que roda a jusante de DOIS curto-circuitos: `output::envelope::render` pula a camada inteira quando nenhuma flag de reshaping está ligada, e `agent_surface::apply` retorna cedo quando a superfície é inerte. Medido: `remember --db T` não emitia bloco `agent_surface` algum, enquanto `remember --db T --max-items 50` emitia `db_path_source: "argv"`. O contrato era portanto visível só para quem já tinha ligado uma flag não relacionada, e ausente no caminho default — o caminho que todo agente usa. Um contrato universal não pode ficar pendurado num bloco opcional. A decisão de entrar na camada virou "há knob OU há alvo a reportar", `apply` ganhou um caminho inerte que anota sem reformatar, e o registro é fundido no único ponto que os dois caminhos compartilham. O envelope de falha carrega o alvo no próprio struct, inclusive no fallback escrito à mão que existe justamente porque a serialização já falhou — que é onde saber o alvo de uma escrita malsucedida mais importa. Ausência do bloco passa a significar uma coisa só: o processo não resolveu banco algum, como em `config`, `completions` e `locale`.
- **GAP-SG-207 (fechado)** — a outra metade da regra: verbo com efeito colateral precisa NOMEAR o alvo no argv. A v1.2.6 adiou isso após estimar 47 structs de argumento. A estimativa mirou o lugar errado. Todas essas declarações convergem para `AppPaths::resolve`, o funil único por onde passam os 38 pontos de resolução, então a recusa cabe num lugar só — o mesmo argumento DRY que pôs a camada agent-native num ponto único. Uma `WritePolicy` instalada ao lado da superfície registra se o subcomando muta e se `--use-active` foi passado; `resolve` recusa com exit 2 quando um verbo que muta termina no PADRÃO COMPILADO sem dispensa. Qual camada merece a recusa é a decisão que sustenta tudo. Lida ao pé da letra, a regra também rejeitaria um alvo XDG, e essa leitura é errada para este produto: `db.path` é chave de primeira classe no registry de configuração, então quem rodou `config set db.path` ESCOLHEU o banco — uma vez, em vez de a cada invocação — e recusar ali tornaria a própria superfície de configuração do produto inutilizável. Medido: aplicá-la a `xdg` deixou 214 testes vermelhos, todos configurando `db.path` num XDG temporário. `TargetSource::Default` é o caso de que a regra realmente trata — nada nomeou o banco em lugar algum, e a escrita cai num fallback compilado que o chamador nunca mencionou. Uma escrita `xdg` é portanto permitida E visível, em vez de permitida e silenciosa. `Commands::may_inherit_target` isenta `init`, cuja função é criar o banco XDG, e as folhas host `config`, `cache`, `slots` e `completions`, que chamam o mesmo resolvedor para achar o diretório de MODELOS e não tocam banco algum — sem esse braço a política disparava em `cache list`, um comando que lê um cache e nada mais. `--use-active` é declarada uma vez, globalmente, e o envelope a registra como `db_path_dispensation` apenas onde ela mudou o resultado.
- **`discarded_flags`** — a `rules-rust-cli-stdin-stdout-silent-discard` exige que o JSON de falha nomeie os argumentos descartados. A v1.2.6 os dobrou na prosa da recusa, o que força o consumidor a parsear uma frase em um de dois idiomas para saber quais das próprias flags foram ignoradas. `AppError::Usage` passa a carregá-los como dado e o envelope de erro emite o campo, tanto no caminho serde quanto no fallback.

### Instrumentação, constantes e o registro

- **`deep-research` declara o próprio teto** — cinco dos seis comandos de leitura chamavam `universe::record`; `--max-results` trunca um ranking fundido sem `record` ao lado, então a estreiteza daquele comando ficava invisível. Agora são seis de seis.
- **O teto de consulta alcança também o caminho inerte** — instrumentar o `deep-research` não bastou sozinho: o teto era escrito por `base_meta`, que só o caminho de reshaping alcança, então `deep-research "x"` sem flag alguma reportava qual banco abriu e nada dizia sobre ter cortado o ranking em cinco. Meio contrato, e justamente a metade mais difícil de notar faltando. O teto passa a viajar ao lado do alvo pelo mesmo motivo que o alvo — ambos são fatos sobre o PROCESSO, não sobre a reformatação. Medido: `list --limit 3` sem knob reporta `query_limit_kind: "pagination"` e `filter_scope: "page"`; `deep-research --max-results 5` reporta `top-k`.
- **Quatro campos planejados que nunca saíram** — `count_scope` nomeia o que `--count-only` contou, já que a contagem roda DEPOIS de `--max-items` e um teto de 10 sobre 39 matches responde `10`. `result_array_source` distingue um membro que a CLI declara como conjunto de resultado de um que a superfície elegeu por fallback, que é a desorientação do GAP-SG-203 tornada visível. `vocabulary_partial` reporta que o vocabulário de sugestão foi amostrado, para que uma lista curta não se leia como "nada parece com sua chave". `key_suggestions` leva a correção como dado, em vez de dentro de uma frase localizada.
- **Duas constantes órfãs resolvidas por ligação, não por remoção** — `K_LIST_DEFAULT_LIMIT` declarava 100 enquanto `list` carregava um `50` cru no corpo; uma constante que documenta um default que o código não usa é pior que nenhuma, porque convida o leitor a confiar nela. Renomeada `K_LIST_TEXT_DEFAULT_LIMIT`, corrigida para 50 e ligada, o que também remove o número mágico. `K_RELATED_DEFAULT_LIMIT` fora superada por `related` tomar emprestado `DEFAULT_K_RECALL`, amarrando o default deste comando ao `-k` do `recall` por acidente; ajustar um teria movido o outro em silêncio.
- **O alvo agora é argumento, não leitura de global** — `apply` delega para `apply_with_target`. O alvo vive num `OnceLock` process-wide, correto para um binário one-shot e errado para um binário de teste, onde os testes de `paths` e de `agent_surface` dividem um processo: existir ou não um alvo dependeria de qual teste rodou primeiro. Cada teste passa a declarar a própria premissa.
- **Documentação** — o ADR-0067 registra o contrato de precedência entre teto de consulta e teto de saída, e a escolha de exit 2 sobre `EX_USAGE` 64, nos dois idiomas. O doc-comment de `cap_secondary_arrays` afirmava "ordem de envelope"; o `serde_json` aqui é compilado sem `preserve_order`, então um `Map` é um `BTreeMap` e a iteração é ascendente por chave. Cerca de catorze comentários em português fora de `src/i18n` foram traduzidos, contra a própria regra de código em inglês do projeto; os dois em `memory_source.rs` ficaram, porque ali o português É o dado citado.

### Os gates que não conseguiam enxergar a si mesmos

O gate pre-publish desta release reprovou, e os três defeitos que ele expôs têm a mesma forma: uma regra que o projeto acredita impor, roteada para um lugar que não consegue impô-la.

- **GAP-SG-211** — 64 links intra-doc não resolvidos chegaram ao gate, uma classe que a v1.1.1 já tinha zerado. Nada regrediu no sentido usual: `cargo check`, `cargo clippy -- -D warnings` e `cargo test` estavam verdes o tempo todo, porque o livro do rustdoc afirma que esses lints "só estão disponíveis ao rodar rustdoc, não rustc". `no_ci_workflows_gate` declara que `cargo test` é o único gate automático deste repositório, então uma regra que precisa de outra ferramenta era, por construção, imposta apenas quando alguém lembrava de um script fora do repositório. O experimento natural está no mesmo arquivo: `#![deny(missing_docs)]` nunca regrediu, e `missing_docs` é o único lint de doc que o rustc também enxerga — mesmo crate, mesmo `deny`, resultado oposto. `[lints.rustdoc]` agora nega `broken_intra_doc_links`, `private_intra_doc_links` e `invalid_html_tags` no manifesto, então `cargo doc` e `cargo test --doc` falham sozinhos em vez de depender de `RUSTDOCFLAGS`. Dois `#[allow(rustdoc::broken_intra_doc_links)]` pousados sobre declarações `pub mod` cegavam `memory_source` e `reaper` por inteiro, já que o atributo propaga para dentro; os dois links tinham correção limpa por caminho completo e os dois atributos saíram. `tests/fmt_gate.rs` traz `cargo fmt --all --check` para dentro do único gate que roda sozinho.
- **GAP-SG-212** — as três suítes `installed_binary_smoke_*` exigiam que `~/.cargo/bin/sqlite-graphrag` batesse com a versão do workspace. Isso é uma propriedade pós-instalação posta dentro de um gate de pré-publicação: a versão sobe antes do gate rodar e instalar só é seguro depois que ele passa, então 26 testes ficavam vermelhos por construção em toda release. O `include` publica `tests/**/*.rs`, então todo consumidor do crates.io também as herdava. Elas agora carregam `#[ignore]` e o libtest as reporta honestamente no sumário. O mesmo helper escondia um segundo defeito: `skip_if_not_installed` chamava `std::process::exit(0)` quando não havia binário instalado, e como o libtest captura a saída por teste enquanto um `process::exit` nu nunca faz o flush dessa captura, a mensagem explicativa não chegava a ninguém enquanto a suíte reportava sucesso sem rodar uma única asserção — o mesmo padrão que `src/commands/optimize.rs` já proíbe em `src/`, regra nunca estendida a `tests/`. Agora entra em pânico, e `docs/TESTING.md` carrega a ordem pós-instalação nos dois idiomas.
- **GAP-SG-213** — `parse_k_range` limita `-k` a `1..=4096` e estava ligado a exatamente dois argumentos, enquanto os oito knobs de concorrência carregavam um `clap::value_parser!(T).range(..)`. A superfície de paralelismo estava auditada; a de memória não. `related --limit` era a ponta afiada: `Vec::with_capacity(limit)` e um `AHashSet` de mesma capacidade são alocados antes que qualquer linha possa limitá-los, então um valor absurdo abortava o processo na alocação em vez de devolver o código de saída que este crate reserva para pressão de memória. Treze argumentos em oito arquivos agora declaram um `value_parser` atrás de uma única checagem de faixa compartilhada, com os quatro tetos nomeados em `src/constants/search.rs`. Medido e deliberadamente NÃO tratado como igual: os limites de `export`, `pending`, `pending_embeddings` e `embedding` chegam ao SQLite como cláusula `LIMIT`, e contagens grandes de saltos terminam no diâmetro do grafo porque as duas travessias carregam conjuntos de visitados. `tests/numeric_bounds_gate.rs` falha, nomeando arquivo e linha, sempre que um argumento numérico limitado nascer sem validador.

### A documentação dizia uma coisa e o repositório fazia outra

Uma auditoria da documentação da raiz rodou depois que o gate pre-publish passou. Ela mediu cada documento contra o binário em vez de contra a memória, e encontrou a mesma forma pela terceira vez: uma regra roteada a uma lista mantida à mão em vez de derivada do fato.

- **GAP-SG-214 (aberto)** — quatro documentos de política prometiam uma CI que `tests/no_ci_workflows_gate.rs` proíbe ativamente. O `CONTRIBUTING` afirmava que `main` exige um pipeline aprovado para merge, que `Co-authored-by` é barrado por CI, e nomeava um job `windows-build-check`; o `SECURITY` afirmava que a CI roda `cargo audit` e `cargo deny` a cada push; o `CROSS_PLATFORM.pt-BR` afirmava smoke tests automatizados a cada commit empurrado, numa linha sem par em inglês. Medido: `.github/` contém quatro arquivos de template e nenhum diretório `workflows/`. O gate não viu nada disso porque só inspeciona linhas que NOMEIAM um arquivo de workflow — `workflows/`, `ci.yml`, `release.yml` — então uma frase em prosa que não nomeia arquivo algum atravessa direto, e porque seu array `POLICY_DOCS` lista três documentos que nunca incluíram `CONTRIBUTING` nem `SECURITY`. As frases falsas foram corrigidas; endurecer o gate é o resíduo declarado, e é por isso que a entrada segue aberta.
- **Os selos de versão convergiram para 1.2.7** — `llms.txt` e `llms-full.txt` carregavam título v1.2.5, `llms-full.txt` ainda declarava v1.2.2 como corrente, `docs/DOCUMENTATION_FRAMEWORK.md` mirava o crate 1.2.2, e o `SECURITY` anunciava v1.2.2 como a última da linha suportada. Os quatro passaram a nomear 1.2.7, com as seções históricas intactas.
- **As faixas numéricas chegaram ao operador** — o GAP-SG-213 mudou o contrato da CLI e nenhum documento voltado ao operador dizia isso. O `README` e o `docs/HOW_TO_USE` passaram a declarar os quatro tetos e que valor fora de faixa é recusado com exit 2 antes de tocar o banco.
- **A proveniência do alvo chegou ao agente** — `db_path_source` e `db_path_resolved` existiam somente no `INTEGRATIONS`, ausentes dos dois documentos escritos para o agente que consome o envelope. O `docs/AGENTS` passou a carregá-los.
- **Doze ADRs órfãos ganharam o par em português** — do ADR-0007 ao ADR-0018 existiam só em inglês contra uma política bilíngue do projeto, traduzidos como registro histórico imutável e não corrigidos.
- **Medido e encontrado correto** — os 51 verbos publicados aparecem nos 17 documentos de inventário; as 63 chaves XDG aparecem nos dois arquivos `README`; todo default documentado bate exatamente com o `config doctor`, nos dois idiomas; os 74 arquivos de schema em disco batem com os 74 que o binário declara; os 61 ADRs estão registrados no `INDEX.md`; a assimetria de cabeçalhos do `docs/COOKBOOK` está na allowlist com justificativa medida, e não é deriva.

## [1.2.6] - 2026-08-10

### A camada agent-native para de responder sobre conjuntos que nunca observou

Quatro defeitos medidos, uma só forma: um pedido impossível era respondido com resultado vazio e exit 0, que o chamador não distingue de "o dado não está lá". Um agente que lê isso conclui que a memória não existe e grava duplicata; uma auditoria que lê isso fecha um gap que continua aberto.

- **GAP-SG-201** — `--filter` julgava a página que a consulta devolveu, não o conjunto sobre o qual o chamador perguntou. Medido: `--filter type=skill --count-only list` responde 39 sobre 1892 memórias; o mesmo pedido com `--limit 50` respondeu `{"count": 0}` com exit 0. A causa raiz foi a ausência de contrato de precedência entre o teto de CONSULTA e o teto de SAÍDA — `agent_surface::apply` recebe o envelope já serializado, a jusante do `LIMIT`, onde um array de cinquenta é indistinguível de um corpus de cinquenta. O comando que aplica um teto agora o declara por `agent_surface::universe`, e um predicado sobre página truncada é recusado com exit 2, salvo se `--filter-scope page` declarar a leitura mais estreita. A recusa é restrita a tetos que PAGINAM um universo contável: `list --limit`/`--offset` e `graph entities`, que rodam um `COUNT` ao lado da consulta. Um `-k` em `hybrid-search` ou `recall`, e o `--limit` de `related`, limitam um ranking ou uma travessia em largura — o top-k É a resposta, e não a truncagem de uma, então esses são reportados e nunca recusados. Todo envelope passa a carregar `query_limited`, `query_limit`, `query_limit_source`, `query_limit_kind`, `universe_total` e `filter_scope`, então uma resposta estreita deixa de ser invisível mesmo onde nada é recusado. `graph entities` é o caso que disparava sem ninguém errar: seu `--limit` tem padrão 50, então `--filter entity_type=person graph entities` julgava 50 de 15 622 entidades numa linha de comando que não mencionava limite algum.
- **GAP-SG-202** — uma chave que não existe em lugar nenhum era indistinguível de um valor que por acaso está ausente. `--select body_length read` devolvia o envelope sem o campo; `--filter chave_errada=x list` devolvia `count: 0` sobre 1892 memórias; `--sort chave_errada` não emitia sinal algum. Os três saíam com exit 0, então um erro de digitação se lia como ausência de dado. O contrato documentado prometia que um `--filter` malformado falha rápido com exit 2 "para que um typo nunca seja confundido com conjunto vazio" — verdade quanto à SINTAXE e nunca quanto ao nome da chave, que é o typo comum. `agent_surface::vocabulary` passa a resolver cada chave pedida contra os elementos observados antes de qualquer predicado, e `agent_surface::gate` recusa uma chave irresolvível com exit 2 e uma sugestão Jaro-Winkler via `rapidfuzz`: `nmae` responde "você quis dizer: name, namespace", `body_lenght` responde "você quis dizer: body, body_hash". A resolução varre TODOS os elementos, não uma amostra, porque um veredito "ausente" errado recusaria um pedido legítimo; a amostragem fica confinada à montagem da lista de sugestões, que só roda depois que uma chave já falhou. Resolução parcial permanece bem-sucedida e reporta `key_resolution`, `resolved_keys` e `unresolved_keys`, então um agente que projeta seis campos sobre um conjunto heterogêneo ainda recebe resposta útil e aprende qual campo faltou. `--allow-unknown-keys` restaura o comportamento anterior.
- **GAP-SG-203** — a camada FABRICAVA um array de resultado para envelopes que não declaram nenhum, e então aplicava o predicado do chamador sobre ele. `locate_result_array` cai no primeiro membro que seja array quando nenhuma chave conhecida casa, e `serde_json::Map` é um `BTreeMap` sem `preserve_order`, então "primeiro" significa o menor por chave. Medido: `--filter integrity_ok=false health` apagou os oito elementos de `checks` — coleção que o chamador nunca nomeou — enquanto `integrity_ok: true` sobreviveu no payload contradizendo o próprio predicado, e a flag de topo `truncated` não subiu apesar de oito elementos removidos. Essa última parte violava a invariante escrita no doc-comment do próprio módulo. O fallback foi mantido, porque envelopes legítimos dependem dele; o que mudou é que uma chave encontrada apenas no envelope agora é recusada com mensagem que nomeia o array que o predicado teria atingido, então uma eleição errada fica visível em vez de destrutiva.
- **GAP-SG-204** — um knob declarado contra envelope sem array de resultado era aceito e ignorado. Medido: `--sort chave --dedupe-by outra stats` saiu com exit 0, com os dois knobs listados no registro e nenhum aplicado. Isso é descarte silencioso de argumento, que `rules-rust-cli-stdin-stdout-silent-discard` proíbe em termos que esta versão segue à letra: o exit code deve refletir se TODOS os argumentos foram processados, e a recusa lista as flags descartadas. `--select` fica isento de propósito: em envelope sem array de resultado ele projeta o próprio envelope, o que é efeito real.

### Designação explícita do alvo

- **GAP-SG-205 (parcial; o resíduo é rastreado como entrada própria no `gaps.md`)** — `AppPaths::resolve` resolve o banco em três camadas e só a primeira é o argv: `--db`, depois a chave XDG `db.path`, depois o padrão compilado. Um verbo de escrita que chega ali sem `--db` muta um banco que a linha de comando nunca nomeou, e nenhum envelope reportava qual — então o banco errado podia ser escrito sem rastro na saída. `TargetSource` passa a distinguir `argv` de `xdg` e de `default`, e o registro carrega `db_path_source` e `db_path_resolved`. Medido: com `--db` o envelope reporta `argv` e o banco do workspace; sem ele, `default` e o caminho XDG. A outra metade da regra — tornar o alvo OBRIGATÓRIO no argv com dispensa explícita — não foi aplicada aqui e está rastreada abertamente: 47 structs de argumentos declaram `--db` de forma independente, `init` cria legitimamente o banco XDG sem ele, e uma regra cega quebraria comandos cujo alvo ambiente é intencional. A metade restante está aberta e carrega seu próprio plano de cinco etapas.

### Erros, testes e estrutura

- **`AppError::Usage`** — nenhum braço de `exit_code` devolvia 2, então uma recusa descoberta no momento da saída não tinha código para onde mapear. Exit 2 em vez de `EX_USAGE` (64): `DUPLICATE_EXIT_CODE` saiu do 2 na v1.0.52 justamente para liberá-lo, `src/main.rs` já o devolve para flag rejeitada, e acrescentar 64 agora daria a este binário dois códigos para um significado. `apply` passou a ser falível e `output::envelope::render` propaga; os dois `let _ = emit_json_compact(…)` em `src/commands/slots.rs`, que engoliam toda falha de emissão, foram removidos.
- **A cerca** — `apply` roda no momento da SAÍDA, depois que o handler já fez seu trabalho, e `remember-batch` e `export` emitem por `emit_json`. Recusar ali para um comando que mudou estado durável reportaria falha para uma operação que teve sucesso, e um `remember` retentado grava a memória duas vezes. `Commands::mutates()` lista as variantes somente-leitura EXPLICITAMENTE e responde `true` para todo o resto, então um subcomando acrescentado depois e esquecido perde um diagnóstico em vez de duplicar dado.
- **GAP-SG-206** — `cargo test --test gaps_caveat_gate` estava VERMELHO em main antes de tudo isso. O guarda de cegueira exigia mais de trinta entradas e mais de trinta bullets `- Status:`; trinta era o tamanho do corpus no dia em que foi escrito, então fechar e podar entradas transformou um documento saudável em build quebrado. Um limiar não distingue "o parser quebrou" de "o backlog encolheu", e só o primeiro é defeito. O guarda passa a comparar cabeçalhos com entradas parseadas e a exigir status em cada entrada encontrada, o que vale com duas entradas, com duzentas e com nenhuma.
- **`truncated` nunca conseguia subir em `list`** — `attach_meta` só escrevia a flag quando o membro estava AUSENTE, e `ListResponse` declara `truncated` como `bool` simples, então o membro estava sempre presente. O módulo sempre prometeu que remover dado nunca é silencioso; essa promessa era silenciosamente falsa no comando mais usado do binário. A flag agora sobe sobre um `false` existente e é monotônica, então um comando que já truncou as próprias linhas continua dizendo isso.
- **Divisão de `src/agent_surface/tests.rs`** — o arquivo estava em 799 das 800 linhas físicas que `tests/file_size_ceiling_gate.rs` permite, então o próximo teste escrito ali teria deixado aquele gate vermelho. Dividido por responsabilidade em `filters`, `projection`, `ordering`, `truncation`, `budget`, `aliases`, `surface_state`, `gate` e `vocabulary`, com as fixtures mantidas na raiz que todos os filhos compartilham. A cobertura unitária da camada foi de 47 para 69 testes.

## [1.2.5] - 2026-08-04

### Honestidade do caminho de leitura (achada ao auditar os hooks do Grok CLI, 2026-08-04)
- **GAP-SG-227** — `--fail-on-degraded` era parseada, guardada e nunca lida. Nada em `src/` tocava `cli.fail_on_degraded`; o decisor `degradation_failure` estava em `src/query_embedding.rs` sob trinta linhas de doc comment e era chamado apenas pelo próprio teste unitário. Uma leitura degradada devolvia resultado só-FTS com exit 0 passando ou não a flag, enquanto o `gaps.md` já a havia promovido a regra permanente para agentes e as duas árvores de hooks do host a ensinavam como proteção. A causa raiz não foi esquecimento e sim forma: `resolve_query_embedding` devolvia a PROSA da degradação e descartava o `reason_code`, e `degradation_failure` deriva a classe do erro do código e nunca da prosa — então nenhum chamador conseguia honrar o contrato. `QueryEmbedding` virou struct carregando `reason_code` (struct e não tupla de cinco porque `backend_invoked` e `reason_code` têm o mesmo tipo e trocariam em silêncio), e `recall`, `hybrid-search` e `deep-research` passaram a decidir antes de qualquer consulta; o `deep-research` decide antes de construir o runtime, para que uma pesquisa recusada nunca pague o fan-out. `--fallback-fts-only` segue isento, porque essa degradação é a instrução do próprio operador.
- **GAP-SG-227 (portão)** — `tests/inert_flag_guard.rs` passou o tempo todo. O predicado perguntava se o NOME do campo aparecia em `src/` fora da declaração, e `fail_on_degraded` aparecia como parâmetro do decisor não chamado e de novo dentro do corpo dele: três evidências de texto real, zero consumidores. O predicado agora pergunta quem LÊ o valor — `cli.<campo>` fora da declaração, ou `self.<campo>` dentro dela, que é como todo knob agent-native chega ao `AgentSurface`. `config_dir` e `cache_dir` entraram na allowlist junto de `verbose` e `quiet`, os quatro sendo pré-parseados do argv cru antes de `Cli::parse` rodar. Uma asserção nova segura a linha com a forma exata que passou.

### Correção da fila de enrich (achada ao auditar os hooks do Claude Code, 2026-08-04)
- **GAP-SG-224** — nenhuma superfície de dead-letter filtrava por `namespace`, embora a tabela `queue` tenha a coluna desde a migração da fila para namespace e o caminho de pending filtre por ela com comentário explicando o motivo. `--list-dead`, `--list-skipped`, `--requeue-dead`, `--requeue-skipped`, `--prune-dead-orphans`, `--prune-dead-entity-orphans` e as cinco contagens de `dead_total` eram irrestritos enquanto todos os seus envelopes ecoavam `namespace` como se o resultado fosse escopado. `prune_dead_orphans` recebia `namespace` e o usava SOMENTE para consultar a memória no banco principal: uma linha dead do namespace A era conferida contra as memórias do namespace B, não era encontrada, e era apagada como órfã — perda silenciosa de dados entre namespaces. `prune_dead_entity_orphans` era pior, não consultava banco algum e apagava as linhas entity de todos os namespaces de uma vez. A igualdade estrita foi escolhida em vez de `OR namespace = ''` porque a migração marca linha legada com `operation = 'LegacyUnscoped'`, então o predicado de operação já as excluía e o ramo extra seria código morto reintroduzindo o vazamento.
- **GAP-SG-225** — o caminho de reivindicação é protegido por `with_busy_retry` desde que o retry limitado entrou; o caminho de BAIXA não tinha proteção e descartava o resultado com `let _ =`, no drain paralelo e no ciclo de re-embed em lote. Sob `--rest-concurrency 16` um SQLITE_BUSY em `mark_done` perdia a conclusão em silêncio: a linha permanecia em `processing`, a varredura de claims obsoletos a devolvia, e o provedor era cobrado de novo por trabalho já pago. `drain_serial` já inspecionava esses resultados, então a defesa existia num caminho e faltava nos outros dois. Um helper `writeback` compartilhado passa a aplicar o mesmo orçamento de retry e a reportar a perda; uma baixa perdida encerra a invocação de forma ruidosa em vez de anunciar drain convergido.
- **GAP-SG-226** — `apply_sidecar_queue_pragmas` aplicava `journal_mode` antes de `busy_timeout` enquanto `apply_connection_pragmas`, no mesmo arquivo, aplica `busy_timeout` primeiro. O sidecar é aberto por cada worker REST sob fan-out, que é onde SQLITE_BUSY aparece, e nenhuma das duas funções tinha teste do `busy_timeout` efetivo. Ordem normalizada e as duas cobertas. A gravidade é declarada em vez de inflada: o `docs.rs` registra que uma conexão rusqlite nova já nasce com 5000 ms de busy timeout, então a exposição prática era o valor configurado chegar tarde, não um timeout zero.

### Auditoria de documentação de `docs/` (2026-08-04)
- **GAP-SG-216** — o portão criado uma rodada antes para fatos auto-declarados saiu verde com seis documentos apresentando a **v1.2.2** como release corrente, três releases depois da v1.2.5. `declared_current_versions` ancorava em ``crate `version = "`` , a grafia de exatamente um arquivo; as outras três grafias do corpus nunca casavam e eram descartadas antes de qualquer comparação. O scanner passa a ancorar nas FRASES de corrente nos dois idiomas, exige versão de três segmentos para que `schema é 13` não seja lido como versão de crate, e isenta versão qualificada por `since`/`desde`, que diz onde um comportamento começou e continua verdadeira para sempre. Os banners de `README` e `AGENTS` deixaram de repetir o conteúdo da release: guardam as invariantes da linha 1.2.x e apontam o CHANGELOG, de modo que só o número muda a cada release — e agora o portão cobra esse número. O scanner alargado então pegou um SEGUNDO furo nele mesmo: lia a versão DEPOIS do marcador e não antes, então `**v1.2.2 (current):**` em `INTEGRATIONS.md:3` e o gêmeo português seguiam invisíveis até `TRAILING_CURRENCY` cobrir a ordem inversa.
- **GAP-SG-217** — `docs/AGENTS.md` documentava 17 das 63 chaves XDG e `docs/AGENTS.pt-BR.md` documentava 15; nenhum arquivo de `docs/` carregava o registro completo, e o `HEADLESS_INVOCATION` carregava 2. Os dois arquivos passam a listar as 63 chaves com tipo de valor e default, conferidos contra `src/config/registry.rs` nos dois sentidos: zero faltando, zero sobrando.
- **GAP-SG-218** — o `HOW_TO_USE` exigia uma das CLIs `claude` / `codex` / `opencode` instalada e no `PATH`, mandava conferir com `which claude || which codex`, ensinava a limpar chaves de API para um spawn não abortar, e fixava `cargo install --version 1.1.2` afirmando que a verificação imprimiria `1.1.1`. Os três backends foram removidos na v1.2.0. Reescrito para o caminho da chave do OpenRouter. O lado pt-BR ainda documentava `--fallback-mode` e `--codex-model-validate`, que o clap rejeita e que o lado EN já não tinha — instrução morta e gap de paridade nas mesmas linhas.
- **GAP-SG-220** — `docs/COOKBOOK.md` tinha 165 seções sobre 97 corpos únicos: 68 eram duplicatas byte a byte somando 2 269 das 5 494 linhas. Um título aparecia duas vezes com conteúdo DIFERENTE, e a cópia curta descrevia o subprocesso removido em tempo presente. Deduplicado para 96 seções e 3 180 linhas, mantendo a versão longa e correta da colisão; ausência de perda provada por hash de corpo de seção antes e depois (97 únicos para 96, com a única perda sendo a deliberada). A allowlist de paridade se justificava com "o arquivo inglês duplica 68 seções literalmente" — medição que morreu junto com a correção. Reescrita em torno do fato estrutural: o português carrega 14 títulos a menos sobre MAIS bytes, logo os dois lados particionam o mesmo material de formas diferentes.

### Auditoria de documentação de `skills/` (2026-08-04)
- **GAP-SG-221** — as duas skills de agente ensinavam superfície que o binário não tem. Quatro flags eram declaradas globais e não existem em lugar nenhum (`--extraction-backend`, `--strict-env-clear`, `--graceful-shutdown-secs`, `--openrouter-base-url`); `--embedding-backend` era documentada como `auto|openrouter|llm` com `llm` descrita como backend de subprocesso, enquanto o clap aceita exatamente `auto` e `openrouter`; `config set spawn.skip_preflight=1` era ensinado como escape de emergência para uma chave que nunca entrou no registry; o `--no-input` recebia exit 65 e um preflight de MCP recebia exit 16, nenhum dos dois existente, enquanto os exits reais 12 e 141 estavam ausentes. Sete flags por subcomando eram listadas como globais, e o prefixo de escrita que os dois arquivos ensinavam colocava `--llm-parallelism` antes do verbo — a posição que sai com exit 2. O `tests/docs_command_coverage.rs` já lê os dois arquivos, mas a sua asserção é unilateral por construção: exige que todo comando vivo seja NOMEADO, nunca que todo nome esteja vivo. Portão novo `tests/skills_surface_gate.rs` compara cada flag citada com a árvore do clap CONSTRUÍDA e cada chave citada com o registry, sob duas regras — namespace conhecido, e qualquer token pontuado numa linha que diga `config set`/`config get`/`config unset`/`XDG`. A segunda regra existe porque a primeira é estruturalmente cega a namespace inventado: `spawn` não está em nenhuma família do registry, logo o teste de namespace nunca o veria.
- **GAP-SG-222** — os dois arquivos afirmavam duas vezes cada que `schema` emite 75 linhas NDJSON; ele emite 74, e um agente instruído a esperar 75 lê um catálogo completo como truncado. O arquivo português se contradizia a noventa linhas de distância: uma linha registrava que `--resume` e `--retry-failed` foram removidas do `ingest`, outra usava `--resume` na fórmula de ingest do próprio arquivo. O portão novo ancora na FRASE que a skill imprime e não num dígito solto, porque `74 linhas NDJSON` é promessa sobre uma saída que o agente vai contar.
- **GAP-SG-223** — os dois arquivos declaravam o mesmo `name: sqlite-graphrag`, colidindo entre si e não casando com nenhum dos diretórios. `--fail-on-degraded` não aparecia em nenhum dos dois, então nada avisava que uma leitura degradada devolve resultado só-FTS com exit 0; `ingest --enrich-after` não aparecia, deixando a regra "nunca encadeie escrita e enrich" sem a sua única exceção sancionada; outras dezenove flags vivas estavam ausentes; e ambos listavam 14 das 63 chaves XDG. Os dois também ensinavam passar `--embedding-dim 1024` em todo caminho de embed — desnecessário, já que 1024 é o default, e perigoso, já que a flag sobrescreve a dimensão gravada em `schema_meta`. Reescritos contra a superfície medida: 51/51 comandos e 63/63 chaves nos dois idiomas, 19 seções H2 na mesma ordem com contagem de bullets idêntica, e zero flags fabricadas entre as 177 flags reais citadas.

### Gate da suíte (o número que a 1.2.4 omitiu)
- Antes: `cargo test --all-features` **exit 101**, 24 de 87 binários alcançados, um guard vermelho, 1 893 asserções passando sob `--no-fail-fast`.
- Depois: `cargo test --all-features` **exit 0** sem `--no-fail-fast`; `cargo clippy --all-targets --all-features -- -D warnings` **exit 0**; `cargo test --doc` **exit 0**.

### Corrigido
- **GAP-SG-189** — a cascata do GAP-SG-175 se repetiu: `scan_page_size` chegou a `EnrichArgs` sem classificação (guard do GAP-SG-164 vermelho), `scan/stream.rs` nasceu com quatro `redundant_closure` e o doctest de `AppError::error_class` nunca compilou (`Internal` embrulha `anyhow::Error`, que `&str` não satisfaz). Os três vieram com o GAP-SG-185, declarado RESOLVIDO e verificado.
- **GAP-SG-190** — `--no-input` documentava exit 65 em `globals.rs` e em onze documentos; nenhum braço de `AppError` devolve 65. A recusa levanta `Validation` → exit 1, agora fixado por `tests/no_input_exit_contract.rs`.
- **GAP-SG-191** — `--max-items` parava no array primário: `graph --select id --max-items 2` devolvia 4 771 393 bytes (2 nós, 59 066 arestas). O teto passa a cobrir todo array e reporta `agent_surface.secondary_capped`. O stub de `--max-output-bytes` reportava o orçamento descontado internamente (340 para 400 pedidos) e agora ecoa o valor solicitado.
- **GAP-SG-192** — `init` preenchia `model` com a versão do crate enquanto `init.schema.json` o descrevia como modelo de embedding; agora carrega o modelo resolvido ou `"none"`. A chave XDG `embedding.model`, prometida desde a v1.0.93 e nunca lida, foi implementada.
- **GAP-SG-193** — o primeiro exemplo de `enrich --help` era `--mode codex --codex-model gpt-5.4-mini`, que o clap rejeita; `health` mandava instalar `claude`/`codex` enquanto sondava a chave OpenRouter; `config` anunciava `llm.fallback = codex,claude,none` contra o próprio registry; sete schemas ativos tipavam `backend_invoked` com seis valores que o binário não emite.
- **GAP-SG-194** — `--max-output-bytes` clonava o envelope inteiro a cada sonda da busca binária (~16 cópias da árvore, mais de 100 MB de churn num envelope de 7,6 MB). Substituído por uma única varredura sobre os comprimentos serializados de cada elemento; zero clones.

- **GAP-SG-197** — a documentação estava três versões atrás do binário e nada a segurava. Nenhum documento foi carimbado nas versões 1.2.3, 1.2.4 e 1.2.5; `MIGRATION.md` ainda anunciava "MIGRANDO PARA v1.2.2" com ZERO menções às três versões seguintes; `AGENTS.md` estava carimbado em v1.2.1. O par README documentava 19 de 61 chaves XDG, deixando 38 knobs invisíveis em toda a documentação de usuário, e `README.pt-BR.md` anunciava `enrich.preserve_threshold`, `enrich.entity_connect.max_runtime_secs` e `llm.concurrency` — três chaves que nunca existiram, cada uma respondendo exit 1. `AGENTS.md` não nomeava seis comandos entregues e `INTEGRATIONS.md` não nomeava treze. `HOW_TO_USE.md` ensinava um "CI Test Environment" exigindo `claude` ou `codex` no `PATH`, num projeto que proíbe CI e removeu esses backends na v1.2.0, enquanto `DOCUMENTATION_FRAMEWORK.md` declarava `ci.yml` e `release.yml` como OBRIGATÓRIOS e errava os três inventários que ele mesmo declara. O envelope de dry-run do `migrate` não validava contra nenhuma variante publicada, `agent-surface.schema.json` estava ausente do índice de schemas, e dois de setenta e dois schemas carregavam `$id` sob um domínio que não serve schemas.

### Alterado
- **GAP-SG-197** — dois portões novos de documentação, porque toda divergência anterior era mecanicamente detectável e nada verificava. `tests/docs_xdg_coverage.rs` (6 asserções) falha quando uma chave do registry está ausente de qualquer um dos dois READMEs e quando um README nomeia chave que o binário rejeita; uma asserção separada exige que cada menção legada da allowlist esteja numa linha que NEGA a chave, para a allowlist não virar buraco. `tests/docs_command_coverage.rs` (4 asserções) falha quando um comando do `--help` está ausente de qualquer um dos 17 documentos de inventário; o casador exige invocação ou code span, porque casar `export`, `list`, `read` e `related` por palavra reportava cobertura total para documento que nunca mostrava o comando. Suíte: **92 binários, 1 925 asserções, exit 0**.
- **GAP-SG-194** — `entity-descriptions` passa a percorrer páginas keyset com a nova `keyset_for_each_selected`, que julga a plenitude da página por linhas VARRIDAS e o `--limit` por itens ENTREGUES, de modo que uma página adelgaçada pelo teste de qualidade em Rust não é mais lida como fim da tabela.
- **GAP-SG-193** — o check de `health` antes chamado `llm_cli` passa a ser `embedding_key`; `health.schema.json` tipa `name` como string livre e nenhum teste fixava o valor antigo.
- **GAP-SG-195** — `src/constants.rs` (960 linhas) dividido em nove submódulos temáticos reexportados por `src/constants/mod.rs`; todo caminho `crate::constants::X` permanece igual e as 115 constantes públicas foram preservadas. `tests/integration.rs` (2 485 linhas) dividido em `integration_bootstrap`, `integration_memory_crud` e `integration_graph`, preservando os 70 testes (17 + 28 + 25), com os cinco helpers movidos para `tests/common/`.
- **GAP-SG-199** — `resolved_busy_policy_defaults_match_constants` comparava um valor RESOLVIDO contra uma constante compilada enquanto o resolvedor lê a config XDG real do host, então `config set db.busy_retries 12` — chave documentada — deixava a suíte vermelha sem uma linha de código mudar. O GAP-SG-198 registrou isso como resíduo e fechou assim mesmo. O teste unitário passou a afirmar só a invariante independente do host; a afirmação de default migrou para `tests/busy_policy_hermetic.rs` sob `common::isolated_env`.
- **GAP-SG-200** — `config set display.tz 0` gravava com exit 0 e bricava toda invocação seguinte, inclusive o `config unset display.tz` que desfaria: `tz::init` propagava o erro de validação e o `main` o executa antes de despachar qualquer subcomando. Todo outro leitor XDG do crate degrada; o `current_tz()` no mesmo arquivo já fazia isso. Agora cai para UTC com um warning nomeando os dois comandos de recuperação.
- **GAP-SG-201** — `config set` validava a CHAVE e nunca o VALOR, então `embedding.dim nao-numero`, `log.format xml`, `i18n.lang klingon` e `llm.backend codex` persistiam todos com exit 0. `log.level` tinha risco próprio: `EnvFilter::new` DESCARTA diretiva impossível de parsear em vez de falhar, então sintaxe quebrada desligaria o logging por inteiro enquanto o comando reportava sucesso (um token nu como `NIVEL_X` NÃO é quebrado — a gramática o lê como nome de target). `SettingKey` ganhou o campo `kind: ValueKind`, as 63 chaves foram classificadas, e `set_setting` rejeita valor fora do domínio com mensagem localizada nomeando o que era esperado.
- **GAP-SG-202** — `config get` respondia `found:false` com exit 0 para chave inexistente, enquanto `config set` respondia exit 1 com did-you-mean para a mesma string, então um typo lia-se como "existe, vazia". O envelope passou a carregar `known` e `suggestion`; o exit code continua 0 porque scripts sondam presença com este comando.
- **GAP-SG-203** — `config set --help` publicava `enrich.preserve_threshold`, `enrich.entity_connect.max_runtime_secs` e `llm.concurrency`, três chaves que respondem exit 1, e o portão do GAP-SG-198 construído para pegar exatamente isso deixou as três passarem por quatro furos: lia só subcomandos de primeiro nível (`config set` é de segundo), ancorava em duas grafias que o texto não usava, dava `break` no primeiro candidato da linha, e fixava à mão os dezenove namespaces que a própria seção de resíduo admitia que envelheceriam. Os quatro fechados; os namespaces agora derivam do registry.
- **GAP-SG-204** — `enrich --llm-parallelism` documentava-se como aplicável "apenas aos modos de subprocesso", que a v1.2.0 removeu — `EnrichMode` tem uma variante, logo nenhum tal modo pode ser selecionado. A justificativa migrou de `///` para `//` porque tudo acima do `#[arg]` é renderizado no `--help`, onde o operador precisa da regra e não do histórico de release. Drive-by na mesma superfície: `--llm-max-host-concurrency` renderizava "optional XDG XDG" e descrevia um teto sobre "LLM subprocesses" que não existem mais.
- **GAP-SG-205** — nove pares de documentos EN/pt-BR tinham divergido, e um não era cosmético: `CROSS_PLATFORM.pt-BR.md` descrevia um job de CI `windows-build-check` ATIVO num projeto que proíbe CI, porque só o lado inglês fora corrigido. `HOW_TO_USE.pt-BR.md` ainda ensinava o "Ambiente de Teste em CI" que o GAP-SG-197 removeu do arquivo inglês. Ambos corrigidos, junto com outros sete pares; `SECURITY.md` e `docs/TESTING` também deixaram de nomear `codex-models` e um arquivo de regressão deletado como se existissem.
- **GAP-SG-206** — `DOCUMENTATION_FRAMEWORK.md` proibia `.github/workflows/` e, na mesma página, mantinha um checklist marcado `[x]` afirmando que `ci.yml` e `release.yml` haviam sido criados; o GAP-SG-197 corrigiu só a metade que proíbe. Nada impedia que um workflow fosse acrescentado.
- **GAP-SG-207** — a exceção de toolchain C era narrada e nunca delimitada. Medido: `cc v1.2.60` está no lockfile, CINCO pacotes o declaram e apenas DOIS compilam C aqui (`libsqlite3-sys`, `ring`); `blake3`, `iana-time-zone-haiku` e `generator` declaram sem compilar em qualquer alvo suportado. As alternativas foram precificadas: `oxisqlite` 0.4.0 (sem FTS5 completo), `rustls-rustcrypto` 0.0.2-alpha. Nenhuma é defensável hoje para um store de memória, então a exceção permanece — agora cercada.
- **GAP-SG-208** — quatro arquivos de teste estavam acima do teto de 800 linhas que o projeto declara (1393, 1311, 1002, 853), resíduo do GAP-SG-195 que cumpriu o teto só em `src/`. Divididos em dezesseis arquivos por família de comando com três harnesses compartilhados novos; os 145 blocos de teste preservados.
- **GAP-SG-209** — `config list --effective` prometia "defaults conhecidos" e entregava uma lista escrita à mão de SETE pares enquanto o registry declara default para cerca de cinquenta chaves. Era também uma SEGUNDA cópia: `display.tz` estava escrito `"UTC"` ali e `Some("UTC")` no registry, e `log.level` já havia divergido, reportando `info` onde o binário usa `warn`. Agora lê o registry.
- **GAP-SG-210** — o teto de 800 linhas nunca disse O QUE conta, então o GAP-SG-195 fechou `src/` medindo linhas físicas e o GAP-SG-208 fechou `tests/` medindo linhas de código via `tokei`, que exclui comentários e linhas em branco. Sob a leitura mais estrita quatro suítes ainda passavam do teto: `installed_binary_smoke.rs` com 981, `schema_migration_integration.rs` com 923, `integration_graph.rs` com 922 e `cookbook_recipes.rs` com 907. A métrica agora está fixada em linhas físicas e vigiada por `tests/file_size_ceiling_gate.rs`; os quatro viraram dez arquivos sem perder nenhum teste.
- **GAP-SG-211** — a documentação contava a SI MESMA e nada recomputava: os dois READMEs anunciavam "61 chaves" contra um registry com 63 e "64 schemas" contra um diretório com 74, e os dois manifestos `llms*` apresentavam o crate como "v1.2.2 (atual)" entregando 1.2.5. O `CHANGELOG.pt-BR.md` listava `[1.2.4]` lá embaixo, entre as releases 1.0.7x, com o preâmbulo do arquivo espalhado em volta; os dois arquivos tinham as mesmas 128 releases, então comparação por conjunto e contagem de seções passavam sobre um defeito visível ao leitor. Novo `tests/docs_declared_facts_gate.rs` (11 testes) amarra todo total declarado à sua fonte e compara as duas ordens de release POSIÇÃO A POSIÇÃO, nunca como conjunto.
- **GAP-SG-212** — a documentação ainda ensinava o subsistema que a v1.2.0 removeu. O bloco "Destaques da Versão" do README carregava 53 bullets em inglês e 49 em português descrevendo `claude -p`, `codex exec`, `--bare`, ONNX e o daemon como superfície corrente, e o `llms.txt` — o arquivo que um agente LLM lê PRIMEIRO — ensinava `--mode gliner|codex|claude-code`, as flags `--codex-*`, a env `SQLITE_GRAPHRAG_CODEX_BINARY` e `ingest --resume`, nada disso existente: `ingest --mode` responde `[possible values: none]` e `enrich --mode` responde `[possible values: openrouter]`. O bloco de histórico virou um ponteiro para este arquivo depois de verificar que nenhum dos 50 comandos e nenhuma das 63 chaves XDG existia só dentro dele, e que as 53 versões já constavam aqui.
- **GAP-SG-215** — o `init --help` abria com "Initialize database and download embedding model" e o `cache clear-models` prometia forçar "re-download on next `init`", num build que não baixa nada desde que a v1.0.79 removeu o runtime ONNX; `cache list --json` responde `files: []` e `total_bytes: 0` porque o diretório nunca recebe nada. O `--help` é o documento mais autoritativo que o produto entrega — viaja dentro do binário e dispensa rede — e foi o único que ficou fora das rodadas de correção de documentação. Resíduo declarado: o comando `cache` segue vestigial, e aposentá-lo é decisão de superfície pública não tomada aqui.

### Limitações conhecidas
- **Toolchain C via SQLite** — o mandato rust-native é hoje incompatível com SQLite: `rusqlite` `bundled` compila SQLite em C via `cc`, `sqlx` depende do mesmo `libsqlite3-sys` e a única implementação rust-native está em alfa. FTS5, `load_extension`, `backup` e `functions` estão todos em uso. Registrado no `gaps.md` como entrada ABERTA com a evidência, e NÃO creditado aqui como correção — a entrada fecha quando existir SQLite rust-native estável com FTS5.
- **GAP-SG-195** — `doc_contract_integration.rs`, `schema_contract_strict.rs` e `prd_compliance.rs` seguem acima de 800 linhas; cada um cobre exatamente um contrato, então dividi-los fragmentaria uma tabela coesa.
- GAP-SG-144 ACL do Windows segue apenas type-checked (sem host Windows neste ambiente).


## [1.2.4] - 2026-08-03

### Fechamento residual (auditoria-execucao-02)
- **GAP-SG-185 fechamento técnico** — o caminho de produção do enrich usa `scan_operation_for_each` (page→enqueue): o buffer de keys em processo é O(page_size) nas ops keyset. A fila sidecar ainda guarda uma linha por elegível. O help deixa de alegar que o pico de RSS do corpus sumiu.
- **NC-3 / classe B7** — `llms*`, `SECURITY*`, `CONTRIBUTING*` não oferecem mais `OPENROUTER_API_KEY` como canal vivo; walk do gate ampliado; G-T-XDG-04 (`config add-key` / flag).
- **NC-2 honestidade** — teste renomeado para `tests/ingest_name_collision_regression.rs` (`IngestMode` só body-only).
- **GAP-SG-141 residual** — dry-run re-embed e2e em TempDir isolado.

### Corrigido
- Overclaim de que o keyset SQL sozinho fazia o pico do scan de produção acompanhar a página.
- Vetor de documentação para env morta fora do walk operacional anterior.

### Alterado
- Orquestrador de produção enfileira por página; dry-run ainda materializa a lista completa para preview.

### Limitações conhecidas
- GAP-SG-144 ACL Windows continua só type-checked.
- Scanners de relationship / entity-description / pares ainda entregam uma página cheia quando não streamados.

## [1.2.3] - 2026-08-02

### Selado em 2026-08-03 (fechamento de auditoria)
- **GAP-SG-185 fechado** — paginação keyset no scan do enrich (`--scan-page-size` / XDG `enrich.scan_page_size`, padrão 512); RSS Linux delta **16 KiB @ N=2k** vs **12 KiB @ N=20k** (ratio 0.75, sublinear); paridade page_size=7 ≡ full.
- **NC-1** versão do crate em **1.2.3**.
- **NC-2** materializado `tests/ingest_name_collision_regression.rs`.
- **NC-3** docs operacionais não oferecem mais `OPENROUTER_API_KEY` como canal; gate `tests/docs_env_channel_guard.rs`.
- **Resíduo GAP-SG-141** backlog de re-embed medido com vetores ausentes sintéticos (sem rede).

### Destaques
- **A superfície de saída remodelava um array e deixava os aliases passarem (GAP-SG-155, GAP-SG-165)** — `locate_result_array` retornava no primeiro membro conhecido, então todo outro array do envelope escapava intocado. Medido contra a 1.2.2: `list --filter memory_type=skill --limit 30` devolvia `items` com 2 entradas e `memories` com 30, cobrindo sete tipos que o filtro havia excluído; `--sort name` ordenava `items`, deixava `memories` na ordem de inserção e ainda declarava `"sort":"name"`. O envelope afirmava duas verdades contraditórias sobre a mesma coleção. Os quatro aliases agora estão declarados em `AGENT_SURFACE_ALIAS_ARRAYS` e são removidos assim que qualquer flag é usada, listados em `agent_surface.aliases_removed`. Um quarto alias, `related_memories` no `related`, nunca havia sido registrado em lugar nenhum.
- **`--max-output-bytes` devolvia ZERO dados em vez de um payload menor (GAP-SG-166)** — o teto só conseguia cortar o array que endereçava. Com alias presente, o membro intocado mantinha o envelope acima do orçamento, então a camada esvaziava o array canônico, ainda não cabia, e substituía tudo por um stub. Medido: `list --max-output-bytes 8000` produzia 93 bytes e nenhum registro, enquanto `hybrid-search` sob o mesmo teto produzia 2 770 bytes **com** registros. A flag que existe para caber na janela de contexto fazia o comando não devolver nada.
- **Um teste que passava havia codificado o defeito como especificação** — `well_known_result_key_wins_over_an_earlier_array_member` usava o envelope real do `recall` e asseverava que `direct_matches` ficava intacto enquanto `results` era limitado. Corrigir o comportamento exigiu revogar aquela asserção, não apenas mudar código.
- **O envelope remodelado não validava contra o próprio schema publicado (GAP-SG-160)** — a camada injeta um bloco `agent_surface`, e todo schema entregue declara `"additionalProperties": false`. O JSON Schema draft-7 não oferece saída por extensão, então o bloco virou definição compartilhada referenciada pelos schemas de cada comando, alcançável por `schema --name agent-surface`.
- **Guards calibrados pela amostra em vez da regra (GAP-SG-156, GAP-SG-157)** — o guard de contagem de workers lia o próprio arquivo por `include_str!`, então `deep_research` manteve um `.worker_threads(2)` literal enquanto o guard seguia verde; agora ele varre `src/` inteiro. O guard de i18n banía duas variantes de `AppError` entre nove, deixando onze mensagens em inglês alcançáveis por operador `pt-BR`.
- **Uma asserção de relógio de parede media o host, não o código (GAP-SG-158)** — um teto de 2 s em volta do spawn de um processo externo passava em 0,02 s isolado e reprovava em 6,24 s sob carga. A propriedade sob teste é que o comando ABORTA, e o sinal observável disso é o exit code.
- **O teto ainda virava stub quando o segundo array não era alias (GAP-SG-171)** — o GAP-SG-166 fechou o ramo do alias, mas o evento topo era um portão OR e não AND: `graph` emite `nodes` junto de `edges`, coleção distinta que a supressão NÃO deve tocar e que o cortador não alcançava. Medido: `graph --select id --max-output-bytes 4000` devolvia 81 bytes de stub. O budget agora drena arrays secundários quando esvaziar o canônico ainda não bastaria, preservando o que o chamador projetou e sacrificando o que ele não pediu; o mesmo comando passa a devolver 3 992 bytes com 343 nós reais. O stub sobrevive para o envelope genuinamente irredutível.
- **O teto de bytes media uma serialização e emitia outra (GAP-SG-170)** — `budget::enforce` mede com `serde_json::to_string`, enquanto `emit_json` renderizava com `to_string_pretty` incondicionalmente. O teto era imposto sobre a forma compacta e violado sobre a indentada: um cap declarado de 8 000 produzia 8 659 bytes, e o excesso crescia com o teto porque a indentação é proporcional ao conteúdo. Encontrado ao confirmar o GAP-SG-166, quando o mesmo envelope recompactado dava 7 502 bytes — o corte estava certo desde o início, a unidade é que não estava. A saída agora é compacta fora do terminal, o que mantém o teto honesto e cumpre o mandato de JSON compacto; terminal interativo segue recebendo saída indentada.
- **Um contrato NDJSON para os três pipelines de ingestão (GAP-SG-148 item 5)** — `IngestFileEvent` e `IngestSummary` agora servem os modos padrão, codex e claude-code. Campos exclusivos de um pipeline são `Option` com `skip_serializing_if`, então nenhum consumidor recebe null que precise ignorar. Uma **quarta** divergência apareceu durante a unificação e foi fechada junto: o frame de fase existia duas vezes, idêntico campo a campo exceto por um nomear o caminho do backend como `claude_path` e o outro como `codex_path`; virou `IngestPhaseEvent`, com os dois membros opcionais e omitidos, deixando o wire de cada modo inalterado byte a byte. O vocabulário `indexed`/`failed`/`skipped`/`preview` também estava declarado em dois lugares e agora vive uma vez só, ao lado dos tipos que o carregam. A entrada foi registrada com dois contratos, corrigida para três e fechada com quatro — a contagem só estabilizou quando a unificação obrigou a ler TODA struct serializada dos três módulos, em vez de amostrar.
- **`output_count` descrevia o estágio de modelagem, não a entrega (GAP-SG-172)** — o campo é medido em `shape_items`, que roda antes de o teto de bytes existir; `finalize` reanexava `output_truncated` e `dropped` depois do corte, mas nunca revisitava a contagem. Medido: `list --limit 30 --max-output-bytes 8000` devolveu onze elementos ao lado de `output_count: 30` e `dropped: 19`, o que se lê como um parser que perdeu dezenove linhas, e não como um teto cumprindo sua função; `deep-research --max-output-bytes 3000` reproduziu de forma independente, com 25 elementos ao lado de `output_count: 50`. A contagem passa a ser remedida sobre o array sobrevivente. É seguro para a reserva já feita: o comprimento sobrevivente só pode ser menor, logo sua forma decimal nunca cresce.
- **`super_hub_count` reportava o teto da amostra, não o grafo (GAP-SG-173)** — a contagem era `hubs.len()` sobre uma consulta terminada em `LIMIT 5`, então o campo era estruturalmente incapaz de passar de cinco, enquanto sua documentação prometia o número de entidades acima do limiar de grau. Um grafo de 15 361 entidades e 46 169 arestas reportava exatamente cinco, ao lado de um aviso que nomeava hubs de grau 1 452, 1 107 e 1 051. A contagem passa a usar `COUNT(*)` sobre a subconsulta agrupada, sem `LIMIT`; a amostra mantém seu teto e o aviso declara os dois números. A classe inteira foi varrida, e não amostrada — `non_normalized_count` provou-se correto, e os `len()` em `ingest_codex`, `deep_research` e `migrate` operam sobre vetores em memória, restando esta como única instância.
- **Quatro tetos de política eram literais dentro do SQL (GAP-SG-174)** — o critério de release checava `format!("LIMIT` e devolvia zero, mas nascera de um sintoma só e não cobria a categoria. Ampliar a varredura para `LIMIT [0-9]+` revelou `LIMIT 50` no fan-out de arestas do `deep-research`, `HAVING deg > 50` e `LIMIT 5` na checagem de super-hub do health, e `LIMIT 10` no contexto do prompt de body-enrich — enquanto o crate já tinha a convenção `K_*` em `constants.rs` para exatamente esses tetos, o que torna isso inconsistência interna. Os quatro viraram constantes nomeadas passadas por bind. Literais estruturais ficaram intocados de propósito: `LIMIT 1` de existência ou de topo e `LIMIT 0` no probe do módulo FTS respondem qual é a forma da consulta, não qual valor é razoável.
- **Um teste entregue nesta versão abortava o harness, e o portão de release não viu (GAP-SG-175)** — `enrich_after_defaults` montava seu probe do clap a partir de um argv vazio, mas `operation` e `mode` são `required_unless_present_any`; o clap responde a um obrigatório ausente chamando `exit(2)` na hora, o que encerra o processo de teste antes da primeira asserção. Nesse caso o harness não reporta asserção reprovada, reporta `error: test failed` com status 2, então nenhum dos seis testes jamais rodou. Ficou invisível porque a checagem de encerramento canalizava `cargo test` para o `tail`, e um pipeline sai com o status do último estágio — o cargo reprovou, o `tail` aprovou, o shell reportou aprovação. Reexecutar com a saída em arquivo revelou 1 366 testes aprovados **e** exit 2 ao mesmo tempo; foi essa contradição que levou ao erro. O probe passa a usar `--print-schema`, o membro mais barato do conjunto que dispensa os dois obrigatórios. O raio do dano era maior que o arquivo: `cargo test` para no primeiro binário de teste que sai anormalmente, e este era o 24º na ordem, então tudo depois dele nunca foi lançado — 23 suítes e 1 366 asserções antes da correção, 84 suítes e 1 878 depois. Sessenta e uma suítes jamais tinham rodado. O número que parecia saúde era o sintoma: parou de crescer porque a execução parava. Regra de processo que isso fixa: leia o exit antes de qualquer pipe, ou por `${PIPESTATUS[0]}`, e nunca aceite uma contagem de aprovados no lugar dele.
- **O enrich escrevia arestas sem reconciliar o `degree`, e se alimentava do próprio dano (GAP-SG-176)** — `upsert_relationship` escreve a aresta e nada mais, então cada chamador é dono do cache `entities.degree`. `ingest::persist` e `remember::run` reconciliam; `enrich::postprocess` não, e é o caminho que mais roda. A omissão se realimentava: `entity-connect` escolhe hubs por `ORDER BY degree DESC` e ilhas por `degree = 0`, ambos lidos desse mesmo cache, então cada passo rankeava hubs por uma subcontagem sistemática e oferecia como "ilha" exatamente as entidades que o passo anterior acabara de conectar. Isso não era relatório defasado, era decisão algorítmica corrompida. Medido em grafo vivo: `graph recompute-degree` atualizou 6 218 de 15 370 entidades, 40 % do grafo. Uma auditoria anterior arquivara o sintoma como comportamento documentado sob a nota "grau NÃO auto"; esta rodada testou a nota em vez de aceitá-la e achou seis caminhos de escrita que de fato o mantêm.
- **`graph stats` misturava contagem viva com cache num só envelope (GAP-SG-177)** — `node_count` e `edge_count` vinham de `COUNT(*)`, `avg_degree` era derivado desses dois, e só `max_degree` lia `MAX(entities.degree)`. Três campos mediam o grafo e um media um instantâneo: 856 aqui contra 1 452 para o mesmo grafo no `health`. O envelope ainda carrega uma invariante que dispensa conhecer o dado — um máximo não pode ficar abaixo da média do mesmo conjunto — então um cache derivando para zero emitiria `max_degree: 0` ao lado de `avg_degree: 6.01`, uma impossibilidade aritmética que o envelope declara sobre si mesmo. `max_degree` passa a ser medido sobre as arestas, nas duas ramificações.
- **Dois valores de política inline em `llm_slots` (GAP-SG-178)** — `assumed_available_mb = 4096` e um intervalo de polling de 100 ms eram literais em linhas cujas vizinhas já liam `LLM_WORKER_RSS_MB` e `MAX_CONCURRENT_CLI_INSTANCES` de `constants.rs`. Encontrados ao ampliar a varredura do GAP-SG-174 para além de `LIMIT`, alcançando literais de tempo, tamanho e memória no crate inteiro. Essa varredura ampliada também confirma a categoria fechada: URLs e caminhos literais só aparecem em testes e numa URI de identificação de schema, a camada de timezone usa UTC com nome IANA configurável por XDG, `SQLITE_GRAPHRAG_*` nunca é lida por `env::var` em lugar nenhum do crate, `openssl` não aparece no `Cargo.toml` nem no código, e `.github/` guarda templates de issue e pull request sem nenhum workflow.
- Schema permanece **v16**. Nenhuma migração do banco principal.

### Adicionado
- `AGENT_SURFACE_ALIAS_ARRAYS`: os quatro arrays derivados (`items`→`memories`, `nodes`→`entities`, `results`→`direct_matches`/`graph_matches`/`related_memories`) suprimidos assim que uma flag é usada.
- `agent_surface.aliases_removed`: nomeia os membros removidos, então a supressão NUNCA é silenciosa.
- `docs/schemas/agent-surface.schema.json`, referenciado pelos schemas de comando e listado por `schema`.
- `tests/docs_consistency.rs`: falha quando a seção mais recente do CHANGELOG credita um gap que o `gaps.md` ainda reporta como pendente. É o executor que o GAP-SG-161 pediu; é um teste porque este projeto proíbe CI por decisão de escopo.
- `tests/ingest_ndjson_contract.rs`, `tests/enrich_after_defaults.rs` e `tests/ingest_name_collision_regression.rs`.
- `scan/sql.rs`: construtores compartilhados de `LIMIT` e placeholders, para que nenhum scanner cole valor no texto SQL.

### Corrigido
- As flags agent-native cobriam um único array por envelope; `list`, `recall`, `related` e `graph` vazavam o resto (GAP-SG-155, GAP-SG-165).
- `--max-output-bytes` colapsava em stub sem dados sempre que um alias ficava fora do array cortável (GAP-SG-166).
- `graph --format json` nunca chegava à camada: serializava com `to_string_pretty` e emitia por `emit_text`, então nenhuma das oito flags valia. O NDJSON streaming preserva o bypass deliberado (GAP-SG-169).
- `deep_research` construía um segundo runtime Tokio fixado em dois workers (GAP-SG-156).
- Onze mensagens de `AppError` eram montadas com `format!` inline e chegavam em inglês a operadores `pt-BR`; `remember_batch` duplicava `errors_msg::duplicate_memory` com texto divergente (GAP-SG-157).
- `LIMIT` era interpolado no texto SQL em seis pontos sob `enrich/scan/` e dois sob `quality_sample`; o teto agora é BINDADO como qualquer parâmetro, usando `LIMIT -1` como sentinela de ausência de limite (GAP-SG-167).
- `docs/schemas/README.md` afirmava que `--mode codex` reusava o formato NDJSON do claude-code, falso desde a 1.2.2 (GAP-SG-168).
- `docs/TESTING.md` listava testes unitários em `filter.rs`, `shape.rs` e `budget.rs`; os 29 vivem todos em `tests.rs`.

### Alterado
- `ingest --mode codex` e `--mode claude-code` emitem os mesmos tipos de evento e de sumário do pipeline padrão. Totais de token, antes campos exclusivos do codex com nomes distintos, agora são membros opcionais do contrato compartilhado.
- `embed_passages_parallel_with_embedding_choice` está `#[deprecated]` em favor de `embed_passages_parallel_shared`; ele clona o corpus inteiro e não tinha mais nenhum chamador dentro do crate. Os re-exports carregam `#[allow(deprecated)]` com escopo, para que o aviso alcance consumidores e não a nós.
- `graph --format json` emite JSON compacto pelo caminho compartilhado, em vez de texto formatado.

### Limitações conhecidas
- O GAP-SG-159 fecha como **não aplicável**: ele pedia workflow de CI para rodar o type-check cruzado, e este projeto proíbe CI, CD e YAML de pipeline. O type-check do alvo Windows segue como passo manual de release.
- A ACL do Windows do GAP-SG-144 permanece apenas type-checked; não há host Windows disponível para executá-la.
- O lote do `re-embed` não pôde ser medido em fila viva: este banco reporta cobertura vetorial completa, então o scan não encontra nada a reivindicar.

## [1.2.2] - 2026-08-02

### Destaques
- **Re-embed reivindica em lote — 32x menos requisições (GAP-SG-141 B1)** — a fila do enrich modelava uma linha como uma chamada remota, então um backfill de 36 343 itens emitia 36 343 requisições. `dequeue_batch_pending` reivindica até `enrich.reembed_claim_batch` linhas (padrão 32) num único `UPDATE ... RETURNING` atômico, resolve todas sem tocar a rede e embeda o lote inteiro numa chamada só. Um teste com wiremock prova que 64 itens passam a custar **2** requisições em vez de 64.
- **A CLI compilava só em Linux (GAP-SG-149)** — `spawn_opencode` tem duas definições mutuamente exclusivas sob `#[cfg]` e o doc comment ficava acima da versão Linux. Com `#![deny(missing_docs)]` ativo, o ramo não-Linux era **erro de compilação** em Windows *e* em macOS. Nenhum gate local podia ver: em Linux aquele ramo nem é compilado. **Ressalva:** o hardening de ACL do Windows do GAP-SG-144 agora passa por type-check (alvo `x86_64-pc-windows-gnu`), mas nunca foi executado num host Windows.
- **`ingest --mode claude-code` duplicava linhas de chunk a cada re-execução (GAP-SG-150)** — aquele caminho inseria as fatias e nunca apagava o conjunto anterior. `memory_chunks` TEM `UNIQUE(memory_id, chunk_idx)`, então o re-insert colidia já no chunk 0, `insert_chunk_slices` devolvia `Err` e o chamador só emitia um `tracing::warn!`. O corpo era atualizado enquanto o índice de chunks **ficava congelado na primeira revisão** — a recuperação por chunk pontuava passagens de um corpo que o banco já havia substituído. Um corpo que encolhesse abaixo do limiar de divisão também deixava fatias órfãs. **Bancos existentes preservam o índice obsoleto**; re-ingira para atualizá-lo.
- **Superfície de saída agent-native (GAP-SG-142)** — oito flags globais deixam o binário moldar o próprio payload: `--select`/`--fields`, `--filter`, `--max-items`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`. Aplicadas num ponto único de emissão, então todo subcomando as ganha de uma vez. Envelope de falha NUNCA é filtrado: `error: true` / `ok: false` sempre chega ao chamador.
- **Os 74 JSON Schemas distribuídos ficaram alcançáveis (GAP-SG-151)** — `schema` lista cada contrato como `{"id", "invoke"}` e `schema --name <ID>` emite o documento. Antes, 70 dos 74 iam no tarball sem verbo algum que os lesse.
- **Runtime e prazo de embedding deixaram de ser hardcode (GAP-SG-141 B2/B3)** — o runtime Tokio deriva a contagem de workers de `available_parallelism` em vez do literal `2`, e o prazo resolve por `--openrouter-timeout` > XDG `embedding.timeout_secs` > constante nomeada.
- Schema permanece em **v16** (sem migração do banco principal). Só comportamento de fila e de ingestão.

### Adicionado
- Flags globais `--select` (alias `--fields`), `--filter`, `--max-items`, `--sort`, `--dedupe-by`, `--count-only`, `--truncate-content`, `--max-output-bytes`.
- Flag global `--no-input`: torna declarável a guarda de TTY que já existia — qualquer caminho que leria stdin falha antes de tentar, mesmo com pipe presente.
- Subcomando `schema` (catálogo) e `schema --name <ID>` (documento), na convenção das CLIs irmãs desta cadeia.
- Hardening de DACL no Windows para o arquivo de configuração que carrega a chave OpenRouter, contrapartida do `0o600` aplicado sob `cfg(unix)` (GAP-SG-144). **Ressalva:** verificado por compilação cruzada, nunca executado num host Windows.
- Chaves XDG `enrich.reembed_claim_batch`, `parallelism.embed_runtime_threads`, `enrich.rate_limit_deadline_secs` e o prazo de leitura de stdin; mais o registro de cinco chaves que eram lidas em runtime e rejeitadas por `config set`.
- `history` passa a funcionar para memórias escritas por `ingest --mode claude-code`: aquele caminho registra versão no update, como o pipeline padrão sempre fez.

### Corrigido
- `ingest --mode claude-code` deixava o índice de chunks congelado na primeira revisão: o re-insert colidia com `UNIQUE(memory_id, chunk_idx)` e o `Err` resultante só virava aviso, então o corpo avançava e as fatias não (GAP-SG-150).
- A crate não compilava para nenhum alvo fora do Linux (GAP-SG-149).
- `ingest --enrich-after` fixava `EnrichMode::ClaudeCode` e `openrouter_timeout: Some(300)`, então a passagem automática rodava em outro backend e com metade do orçamento documentado de 600 s, mascarando o XDG (GAP-SG-149).
- `--force-merge` era no-op silencioso no modo claude-code: `upsert_memory` fundia incondicionalmente, então dois arquivos que derivassem o mesmo nome se sobrescreviam sem erro, sem aviso e sem sinal (GAP-SG-148).
- `--llm-parallelism` está documentada como inerte sob `--mode openrouter`, onde `--rest-concurrency` governa o fan-out, e agora emite aviso quando passada explicitamente (GAP-SG-141 B7).
- Dez testes de integração asseriam contratos aposentados na v1.2.0 e na v1.1.8; cada um passa a verificar a MESMA pergunta contra o contrato atual, com o gap de origem citado em comentário.
- `prd_namespace_flag_sets_memory_namespace` tinha literal SQL sem aspas, que o SQLite lia como comparação entre colunas.

### Alterado
- **Quebra de contrato, NDJSON:** `ingest --mode claude-code` passa a emitir `IngestFileEvent` e `IngestSummary`, os mesmos tipos do pipeline padrão. O `status` por arquivo muda de `done` para `indexed`; o resumo reporta `files_total` / `files_succeeded` / `files_failed` / `files_skipped` em vez de `completed`. Os eventos saem por `emit_json_compact`, que propaga erro de I/O em vez de engolir. Um parser único passa a consumir os dois modos.
- `ingest --mode claude-code` embeda pelo caminho em lote compartilhado em vez de uma chamada por chunk (GAP-SG-148 item 4).
- Política de tempo escrita como literal no ponto de uso virou constante nomeada, com chave XDG onde o valor é política operacional. `Duration::from_secs(3600)` aparecia idêntico em três arquivos; o cálculo `base + 15s por item` estava duplicado verbatim em dois.
- Módulos divididos por responsabilidade e não por tamanho: `config`, `embedding_api`, as operações de extração do enrich e os dois pipelines de ingestão. Os sufixos `_a` / `_b` / `_c` desapareceram.

### Limitações conhecidas
- Falha de um lote de re-embed devolve todas as linhas da reivindicação para `pending`; o attempt consumido permanece, porque cada item foi realmente tentado. O circuit breaker registra **uma** falha, porque houve uma chamada remota. Divergência de dimensão erra a chunk HTTP inteira e cai nesta mesma política.
- O caminho de ACL do Windows é verificado apenas por compilação cruzada.

## [1.2.1] - 2026-07-31

### Destaques
- **Isolamento de namespace na fila enrich (claim + contagem + resume)** — `dequeue_next_pending`, `count_eligible_pending`, `--resume` / `--retry-failed` exigem `operation` **e** `namespace`. Um drain em `ai-sdd` deixa de claimar ou contar linhas de `global` / ns vazio.
- **`--until-empty` conta só esta op+namespace** — antes contava *todas* as linhas pending de todas as operações; zumbis ReEmbed alienígenas faziam EntityDescriptions girar até max-runtime com `completed=0`.
- **`--force-redescribe` reabre `skipped`/`done`** — `reopen_force_redescribe_candidates` roda uma vez por processo antes do primeiro enqueue para `INSERT OR IGNORE` não ser no-op silencioso; nunca reabre `dead` (use `--requeue-dead`).
- **Reconciliação de zumbis ReEmbed** — `reconcile_satisfied_reembed_pending` marca pending ReEmbed como `done` quando já existe vetor vivo na dim ativa (`LENGTH(embedding) = dim*4`), sem chamar a API.
- **Elegibilidade de re-embed usa comprimento do BLOB, não só a coluna `dim`** — scan/predicados selecionam linhas CORRUPT / META_AHEAD (`dim=1024` com BLOB 384-d permanece elegível).
- **Enqueue valida chaves de re-embed** — `entity:{name}` remove o prefixo no lookup; nomes bare continuam ok; entidade ausente é rejeitada. Chunks validam se o `chunk_id` existe em memória não-deletada do namespace alvo.
- **Marcadores CAPA-D de baixa qualidade** — removido `%configuration file%` solto; só compostos (ex. `is a configuration file`) para não marcar prosa legítima (config de lint clippy.toml) como força de redescribe.
- Schema permanece em **v16** (sem migração do DB principal). Só comportamento da fila sidecar.
- Nota: uma seção histórica antiga também titulada `[1.2.1] - 2026-04-18` (pin MSRV) permanece mais abaixo neste arquivo; **a crate 1.2.1 publicada nesta release é o selo CAPA da fila enrich de julho/2026**.

### Corrigido
- `enqueue_candidate` rejeitava toda chave `entity:…` de re-embed (`entity not found in namespace`) → `completed: 0` em ~14k entidades.
- Claim sem filtro de namespace → HardFailure cross-namespace + circuit breaker.
- Enqueue de chunk confiava só no scanner → chunks inválidos / de outro ns iam para dead-letter.
- `--force-redescribe` não reprocessava linhas já `skipped`/`done` no sidecar.
- ReEmbed deixava pending para sempre quando o BLOB vivo já batia com a dim alvo.
- Falso positivo de force-redescribe em descriptions legítimas com "configuration file".

### Adicionado
- Regressão: `enqueue_candidate_accepts_entity_prefixed_reembed_key` (aceita `entity:ownership` e bare `ownership`; rejeita entidade inexistente).
- Regressão: `dequeue_next_pending_isolates_by_namespace`.
- Suite de queue: 38 testes OK.

### Docs / empacotamento
- Docs de produto, monografias, skills e `llms*` retargetados para **crate 1.2.1**.
- Inventário completo de comandos da CLI mantido/atualizado nas monografias orientadas a operador.

## [1.2.0] - 2026-07-29

### Destaques
- **DEFAULT_EMBEDDING_DIM = 1024** (flag `--embedding-dim` / XDG `embedding.dim` ainda sobrescrevem; DBs existentes mantêm `schema_meta.dim`).
- **Fila enrich multi-namespace** — coluna `namespace` + `UNIQUE(namespace, operation, item_key)`; `DELETE` escopado; correção SQL `status='pending'`.
- **`--list-skipped` / `--requeue-skipped`** — recuperam `preservation_failed` / sink skipped sem SQL bruto (G-PR-3/4).
- **Enqueue valida** se memory/entity existe no namespace alvo (G-PR-8).
- **`chars_before`** em falha de grounding de description usa o comprimento da description anterior (G-PR-5).
- **`blocked_dead`** não mascara mais dívida skipped (G-PR-7).
- **tracing-appender** logs opcionais em arquivo via XDG `log.to_file` / `log.rotation`; flush no segundo sinal.
- **Hints de exit-code** em inglês por padrão (agent-native); hint de namespace não cita mais product env.
- **config list --effective** defaults de dim/log a partir de `constants` (sem `"384"` hardcoded).
- **`failing_command()` portátil** para stubs OAuth-only (seguro no Windows).
- **llm_slots** path via cache/XDG runtime — sem literal `"/tmp"`.
- Harness E2E: `scripts/e2e_offline_v120.sh`.
- Schema do banco principal permanece em **v16** (migra apenas a fila sidecar).

### Adicionado / Corrigido no selo residual
- **Tests/benches herméticos** — `IsolatedEnv` / `wire_assert_cmd`; **ZERO** product env operacional `SQLITE_GRAPHRAG_*` (GAP-SG-101/118); 12 testes `xdg_isolation_guard`.
- **GAP-SG-117** — docstrings/logs citam chaves XDG, não product env.
- **GAP-SG-121** — pragmas WAL/busy compartilhados do sidecar; schemas de fila enrich vs ingest documentados como distintos.
- **GAP-SG-122** — `paths.cache` legado → `cache.dir`.
- **GAP-SG-125** — `optimize --fts-dry-run` retorna `AppError` (exit 1) em vez de `process::exit` nua.
- **GAP-SG-126** — trait mínima `LlmSpawnBackend` em `spawn/`.
- **G-PR-1/2** — prefilter SQL de baixa qualidade mais restrito + post-filter em Rust + testes unitários de FP.
- **GAP-SG-119** — fixtures SQL usam `DEFAULT_EMBEDDING_DIM` (1024), não 384.
- **SRP** — monólitos órfãos `scan_tests.rs` / `queue_tests.rs` removidos (suites vivas são `*_a`/`*_b`).
- **Skills/README** — dim padrão **1024**, `db.path`, allowlist de chaves de entidade graph-stdin (SGR-E02), list/requeue-skipped.
- **e2e_offline_v118.sh** — wrapper fino → `e2e_offline_v120.sh`.

### Conclusão do selo residual (mesma release)
- **GAP-SG-139** — folhas host/XDG (`config`×9, `slots`×3, `cache`×3, `codex-models`, `completions`) aceitam `--db` como **no-op** documentado (`src/cli_db_noop.rs`); superfícies de grafo inalteradas; regressão `tests/cli_db_noop_host_surfaces_regression.rs`.
- **`#![deny(missing_docs)]`** na lib — 866 itens públicos documentados (EN); apenas o módulo `embed_migrations!` permite missing_docs.
- **Monografias** — README/llms/AGENTS/TESTING/MIGRATION/COOKBOOK/HEADLESS/INTEGRATIONS → **v1.2.0**, gate `e2e_offline_v120.sh`, dim canônica **1024**, `db.path`.
- **SRP** — split de `ingest/`, `embedder/`, `storage/entities/`, `deep_research/` (LOC dos pais↓; novos arquivos ≤800).
- **`--print-schema`** — list / recall / enrich / config list (schemas embutidos, stdout limpo).
- **15** testes herméticos `xdg_isolation_guard`; asserts de product env apenas negativos.
- **OpenCode** em `LlmSpawnBackend`; helpers quality_sample com ordem por hash (G-PR-6); flush de WorkerGuard na saída do main.
- **e2e_v118** apenas wrapper histórico.

### Residuais (honestos, rastreados em gaps.md)
- Alguns monólitos permanecem >800 LOC (ingest_claude/codex, chat_api, health, migrate, …) — SRP adicional opcional. Catálogo i18n de Validation cobre hot paths (~67 sites); strings Validation ad-hoc restantes fora desses paths podem expandir depois sem bloquear o selo.
- E2E offline **20/20** em `scripts/e2e_offline_v120.sh` com binário release 1.2.0; `cargo test --lib` **1112** passed; `cargo clippy --lib -D warnings` limpo; `#![deny(missing_docs)]` ativo.

## [1.1.8] - 2026-07-19

### Conclusão residual (mesma release)

- **Selo E2E (2026-07-19):** scrub de help (sem env de produto / sem Box about); URLs do OpenRouter ligadas ao XDG `network.openrouter.*`; fail-fast de query Auto (`llm.query_embed_timeout_secs`); fold de EntityType (`module`→Concept); paridade de description no remember-batch; `pending-embeddings status` + `cache stats`; `purge --now`; `config list --effective`; `related_to`→`related`; alias de telemetry removido; harness só XDG; harness offline **16/16**.
- **GAP-CLI-QISO-01..08 (P0 produção):** o claim da fila enrich **é escopado por `operation`**. Um drain de memory-bindings não pode mais claimar linhas de entity-descriptions / entity-connect (corrige falso `memory 'entity' not found` / `memory 'pair:…' not found` → permanent dead, gate CAPA3 travado). `ClaimedRow` + `validate_claim` + guarda de shape em `call_memory_bindings`; status `state=blocked_dead` quando o déficit de scan permanece só com permanent dead; JSON repair permanece Transient na origem ChatError.
- **G-T-XDG-04:** removidos todos os bindings `clap env=` de produto; config de runtime é **flag > XDG `config set` > default** via `runtime_config` / `paths` / `resolve_api_key` (sem leituras de env de produto `OPENROUTER_*` / `SQLITE_GRAPHRAG_*` no hot path).
- **ED-05 / PRIO-04/05 / EC-08/09:** `--entity-description-domain`, `--ops-gate`, `--yield-every-n-items`, preempt cooperativa de EC para ED quente, campos de summary `budget_exhausted` / `pairs_remaining_estimate` / `yields`.
- **Honestidade OBS-OPS:** rótulos ValueEnum de ops que persistem estão **totalmente implementados** (sem mais mentiras de “scan only” para weight/relation/type/description/body/domain).
- **DR-02 / EC-10:** testes de regressão para materialização de saída do deep-research e escala sintética de wall-clock do EC.
- **Docs:** TESTING / CROSS_PLATFORM / COOKBOOK — cron/systemd/launchd local; checklist XPLAT-01; sem GA como caminho de produto.
- **Split:** `enrich/prompts.rs`, `enrich/scheduler.rs` extraídos; splits adicionais de monólitos permanecem incrementais na manutenção da 1.1.8.

Fecha a auditoria de qualidade / latência / contrato do enrich (eixos P1–P12 + G-T) **e** o poison cross-op da fila (QISO). Sem migração de schema do banco principal (`CURRENT_SCHEMA_VERSION` permanece 16). O sidecar do enrich ganha coluna opcional `priority`. Crate `version = "1.1.8"`.

### Corrigido
- **prompt de entity-descriptions** — removido viés hardcoded de software/system-design; prompt multi-domínio neutro com snippets do corpus de memórias vinculadas (GAP-CLI-ED-01/02).
- **gate de qualidade de entity-descriptions** — cobertura de grounding contra o corpus antes de persistir; compartilhado com body-enrich via `preservation` (GAP-CLI-ED-03, G-T-DRY-01).
- **entity-descriptions write-once** — `--force-redescribe` reescaneia padrões de baixa qualidade (GAP-CLI-ED-06).
- **help de entity-connect** — documentado como fully-implemented (persiste relacionamentos), não scan-only (GAP-CLI-EC-01).
- **deep-research `-o`** — alias curto de `--output`; fail-fast se o arquivo estiver ausente/vazio após escrita atômica (GAP-CLI-DR-01..03).
- **memory-entities** — JSON de saída inclui `entities[].description` (GAP-CLI-ME-01).

### Adicionado
- `remember --enqueue-enrich` + campos de envelope `entities_created` / `enrich_recommended` (GAP-CLI-PRIO-01/02).
- Coluna `priority` na fila enrich; entity-descriptions do hot-set claimam primeiro (GAP-CLI-PRIO-03).
- `--entity-names` / `--memory-names` / `--anchor-memory` / `--entity-description-grounding-threshold` / `--force-redescribe`.
- `config set|get|list|unset` para settings operacionais XDG (G-T-XDG-01).
- Docs de scheduler local substituem receitas de GitHub Actions / CI em nuvem (G-T-DOC-01).

### Alterado
- Precedência de resolução de API key: **flag CLI > config XDG > env depreciada** (G-T-XDG-02/03).
- Whitelist de spawn não encaminha mais env vars de telemetria remota OTEL (G-T-TEL-01).
- Init de tracing renomeado conceitualmente para logging local apenas.

### Documentação
- INTEGRATIONS.md / pt-BR: cron/systemd/launchd/Task Scheduler locais em vez de receitas de GA/CircleCI/Jenkins em nuvem.
- Selo completo de documentação v1.1.8: README / README.pt-BR, CHANGELOG.pt-BR, llms*, MIGRATION / HOW_TO_USE / COOKBOOK / AGENTS / TESTING / CROSS_PLATFORM (EN+pt-BR), skills EN/PT, SECURITY / CONTRIBUTING — config XDG (flag > XDG > default), cobertura do selo E2E, sem tabelas de product env como guidance corrente; ver append em `gaps.md` **Auditoria de documentação v1.1.8**.

### Residuais honestos (não bloqueiam o harness)
- Monólitos ainda >800 LOC em manutenção contínua (`ingest.rs`, `embedder.rs`, `enrich/extraction*.rs`, `deep_research.rs`, módulos de storage, etc.).
- Qualidade live LQ (backfill LLM de entity-descriptions no grafo de produção) = operação do operador (`--force-redescribe` + sample; harness offline 16/16).

## [1.1.07] - 2026-07-15

Release de migração de publicação / ownership. Sem migração de schema (`CURRENT_SCHEMA_VERSION` permanece 16). Sem mudança de comportamento em runtime em relação à v1.1.06. Nome oficial da release: v1.1.07; o manifesto do crate carrega `version = "1.1.7"` porque o parser SemVer rejeita zero à esquerda no componente patch.

### Alterado
- **owner no crates.io** — ownership transferida para o usuário GitHub/crates.io `danilo-aguiar-br` (a conta banida anterior aparece como `ghost_*` no crates.io).
- **Metadados repository / homepage** — `Cargo.toml` `repository` agora aponta para `https://github.com/danilo-aguiar-br/sqlite-graphrag` (o crates.io só atualiza esses campos ao publicar uma nova versão).
- **Casa no GitHub** — árvore canônica é o repositório público novo sob `danilo-aguiar-br`; URLs de documentação e `$id`s de schema atualizados.
- **Email de contato de segurança** — endereço de fallback: `daniloaguiarbr@proton.me`.

### Removido
- **GitHub Actions CI/CD** — todos os `.github/workflows/*` e Actions compostas foram apagados do projeto. Releases são publicadas manualmente; badges de CI removidos dos READMEs. **Não** recriar Actions no repositório novo.

### Docs
- README / llms* / SECURITY / templates de issue / CONTRIBUTING: owner e URLs do repositório novos; sem Actions.

## [1.1.06] - 2026-07-12

Fecha **GAP-ENTITY-CONNECT-SCAN-CARTESIAN** (P0): `enrich --operation entity-connect` não trava mais em namespaces grandes como `global` com ~10⁵ entidades. Sem migração de schema (`CURRENT_SCHEMA_VERSION` permanece 16). Nome oficial do release: v1.1.06; o manifest do crate carrega `version = "1.1.6"` porque o parser SemVer rejeita zero à esquerda no patch.

### Corrigido
- **Causa raiz** — `scan_isolated_entity_pairs` deixa de montar o produto cartesiano `entities × entities` com `ORDER BY` global que forçava o SQLite a materializar O(n²) candidatos antes do `LIMIT 50` (TEMP B-TREE). O scan gera pares por **coocorrência** em `memory_entities` (primário) e preenchimento **hub × ilha grau-0** (secundário).
- **Re-scan no drain** — `call_entity_connect` não reexecuta o scan a cada item da fila. Itens usam chaves estáveis `pair:{id1}:{id2}` resolvidas por PK em O(1).
- **Tipagem da fila** — `item_type` de entity-connect / cross-domain-bridges é `entity_pair`.
- **Runtime no primeiro scan** — `--max-runtime` / teto soft de 120s cobre o **primeiro** scan via `InterruptHandle`; interrupt → `AppError::Timeout` (exit **1**), nunca exit 75.
- **Observabilidade** — NDJSON emite `phase: "scan_start"` com o nome real da op (`entity-connect` ou `cross-domain-bridges`), `entities_in_namespace`, `backlog_degree0_proxy`; `scan_meta` com `pairs_enqueued_this_scan` / `scan_elapsed_ms`. A primeira iteração de `--until-empty` não faz double-scan.

### Adicionado
- Suite `tests/v1106_entity_connect_scan_regression.rs`.
- Testes unitários de nome CLI, SQLITE_INTERRUPT, deadline expirado e `InterruptHandle` ao vivo.
- ADR-0066.

### Docs
- `gaps.md` **Fechado**; README / llms* / INTEGRATIONS / HOW_TO / COOKBOOK / skills em **v1.1.06**.

## [1.1.05] - 2026-07-11

Fecha os cinco bugs operacionais do relato de incidente deep-research sobre o sujeito "danilo" (2026-07-08; ver `gaps.md`). Sem migração de schema. Nome oficial do release: v1.1.05; o manifest do crate carrega `version = "1.1.5"` porque o parser SemVer rejeita zero à esquerda no patch.

### Corrigido
- Bug 1 — `deep-research` com query de palavra única (ex.: `"danilo"`) não degrada mais para uma única busca híbrida. A decomposição heurística expande tokens únicos em sub-queries multi-aspecto (`source: "aspect"`) cobrindo patrimônio, stack, stakeholders, projetos, decisões, relacionamentos e contexto (facetas EN/PT). Estratégia manual permanece via `--sub-query-strategy manual --sub-queries-file`.
- Bug 2 — envelopes JSON grandes deixam de ser frágeis sob redirecionamento de shell: novo `--output PATH` grava o envelope completo via algoritmo atomwrite (tempfile → fsync → rename) e emite um ack curto no stdout com checksum `blake3`; `--quiet`/`-q` global suprime tracing não-erro; o help documenta o contrato stdout-JSON / stderr-logs (nunca `&>` no mesmo arquivo).
- Bug 3 — `graph traverse --from <nome-curto>` não falha mais de forma opaca: match exato continua prioritário; sem `--fuzzy`, NotFound (exit 4) inclui sugestões ranqueadas (Jaro-Winkler / prefixo) dos nomes canônicos; com `--fuzzy`, um vencedor claro é auto-resolvido com warning em stderr (`rapidfuzz`).
- Bug 4 — `merge-entities` rejeita merges auto-referenciais (`--ids` contendo `--into-id`, ou `--names` contendo `--into`) **antes** de qualquer trabalho no DB, para que erros de word-splitting do zsh não corrompam o grafo sob `--cross-namespace`. Re-checagem em profundidade permanece no resolve.
- Bug 5 — `link` não trata mais strings puramente numéricas como nomes de entidade sob `--create-missing`. Novos `--from-id` / `--to-id` resolvem por ID; `validate_entity_name` rejeita nomes só de dígitos para impedir entidades fantasma.

### Adicionado
- `src/atomic_io.rs` — helpers de escrita atômica (`write_atomic`, `write_json_atomic`) usados por `deep-research --output`.
- `entities::resolve_entity_fuzzy`, `suggest_entity_names`, `entity_name_similarity`.
- Suite de integração `tests/v1105_danilo_bugs_regression.rs` cobrindo os cinco bugs na fronteira da CLI.

### Documentação
- Help longo do `deep-research`: contrato stdout/stderr e exemplos de `--output` / estratégia manual.
- Notas do `graph traverse`: comportamento de apelidos curtos e `--fuzzy`.
- Status em `gaps.md` atualizado de pendente → corrigido para Bugs 1–5.
- Description do `Cargo.toml` atualiza o tamanho do binário para ~19 MiB (bate com o binário de release; restaura o invariante `binary_size_documented_regression`).

## [1.1.04] - 2026-07-08

Fecha os dois gaps estruturais rastreados em `gaps.md` após a v1.1.03: o panic de runtime Tokio aninhado no `deep-research` (GAP-001) e o enrich de `entity-connect` sem convergência (GAP-002). O schema avança da versão 15 para 16 com uma migração numerada. Nota: o nome oficial do release é v1.1.04; o manifest do crate carrega `version = "1.1.4"` porque o parser SemVer rejeita zero à esquerda no componente patch.

### Corrigido
- GAP-001 — `deep-research` não panica mais com "Cannot start a runtime from within a runtime". O ponto de entrada síncrono agora computa os embeddings por sub-query ANTES de construir seu runtime Tokio dedicado (novo helper `compute_sub_embeddings` em `deep_research.rs`), e os três caminhos de embedding OpenRouter em `embedder.rs` (single, serial batch, fan-out JoinSet) adotam o padrão canônico de reentrada `Handle::try_current` + `block_in_place` já usado pelo caminho batch — eliminando panics de runtime aninhado para qualquer chamador futuro que rode dentro de um runtime existente. A mesma guarda é aplicada ao `ingest_opencode`.
- GAP-002 — `entity-connect` agora converge. Uma nova tabela `entity_connect_seen` (migração V016) registra o veredito do LLM (`related`/`none`) para cada par avaliado, o scanner `scan_isolated_entity_pairs` exclui pares já avaliados e prioriza entidades hub, `count_operation_backlog` reporta um proxy de backlog O(n) real (entidades grau-0 com bindings NER) em vez de um zero hard-coded, e `call_entity_connect` persiste o veredito tanto no ramo `related` quanto no `none`. `--until-empty` agora atinge `eligible_remaining == 0` em vez de reavaliar para sempre os mesmos pares rejeitados.

### Adicionado
- Migração V016 — `entity_connect_seen(source_id, target_id, namespace, verdict, relation, evaluated_at)` com chave primária composta, duas chaves estrangeiras `ON DELETE CASCADE` para `entities(id)`, um check constraint no `verdict`, e um índice de namespace. `CURRENT_SCHEMA_VERSION` avança de 15 para 16.

### Documentação
- A operação de enrich `entity-connect` é promovida de "scan-only (não persiste)" para "fully-implemented" na referência da skill, refletindo o pipeline convergente de rastreamento de vereditos.
- A description do `Cargo.toml` reintroduz o tamanho do binário (~16 MiB), restaurando o invariante `binary_size_documented_regression` que derivou durante o refresh da description na v1.1.03.
## [1.1.03] - 2026-07-07

Fecha os seis bugs que bloqueavam operadores catalogados em `gaps.md` após a corrida de remediação do GraphRAG de 2026-07-07, mais o portão V8 de corpo excessivamente grande. O schema permanece na versão 15 (a fila sidecar do enrich ganha uma coluna `claimed_at` via ALTER idempotente). Nota: o nome oficial do release é v1.1.03; o manifest do crate carrega `version = "1.1.3"` porque o parser SemVer rejeita zero à esquerda no componente patch.

### Corrigido
- Bug 1 — o caminho de scan-enqueue do enrich agora insere candidatos em lote em uma única transação em vez de linha a linha sob o write lock do WAL, eliminando o aparente deadlock da fase de scan que congelou o workload de `re-embed --target entities` de 44 163 entidades sob contenção de stale-claim.
- Bug 2 — `reclassify-relation` ganha `--literal-to <RELATION>` (escrita verbatim do alvo) complementando o `--literal-from` existente; a guarda de from==to agora compara literais crus, então `--literal-from applies_to --literal-to applies_to --batch` migra as 61 357 arestas legadas com underscore (`applies_to`, `depends_on`, `tracked_in`) para sua forma canônica com hífen.
- Bug 3 — `merge-entities` ganha `--cross-namespace` (opt-in, default mesmo-namespace) para que `--ids`/`--into-id` resolvam através de todos os namespaces, desbloqueando as 15 duplicatas cross-namespace.
- Bug 4 — o sidecar do enrich ganha uma coluna `claimed_at`, claims stale em `processing` são resetadas no startup, um handler de SIGTERM faz cleanup graceful antes do exit 19, e uma nova flag `enrich --reset-stale-claims` limpa manualmente claims de processamento stale mais antigas que o threshold — eliminando o estado de stale-lock de três camadas deixado pelo `kill -9`.
- Bug 6 — o scanner de `re-embed --target chunks` e o `count_operation_backlog` trocam de `JOIN memories` para `LEFT JOIN memories` com filtro de namespace relaxado, de modo que chunks de mães soft-deleted tornam-se visíveis ao re-embed; o `enrich --status` (`scan_backlog`) e o `health` (`vec_chunks_missing`) agora concordam, e a cobertura atinge um 100% real.

### Adicionado
- V8 — o novo subcomando `split-body` divide memórias cujo corpo excede 25 000 caracteres em memórias filhas em fronteiras de chunk, marca a original como `SUPERCEDIDO`, e cria relações `replaces` de cada filha para a original. As filhas não são embedadas inline; rode `enrich --operation re-embed --target memories` depois para fazer backfill dos vetores delas.

### Documentação
- Bug 5 — o texto de ajuda e o doc-comment de `enrich --status` agora documentam explicitamente que `scan_backlog` é o trabalho pendente REAL (candidatos que um scan selecionaria), `queue_pending` é uma CONTAGEM CALCULADA (não uma fila física), `eligible_now == 0` com `queue_pending > 0` é COOLDOWN (não um deadlock), e `eligible_now > 0` preso contra `state: "draining"` é um deadlock recuperável via `--reset-stale-claims`.

## [1.1.02] - 2026-07-06

Fecha os dois gaps residuais rastreados após a v1.1.01: o argumento no-op depreciado `--gliner-variant` é removido por completo (o clap agora o rejeita com exit 2), e o teto de tokens do embedding torna-se um erro tipado exit 6 — `AppError::TooManyTokens { tokens, limit }` — validado na borda antes de qualquer escrita. O schema permanece na versão 15. Nota: o nome oficial do release é v1.1.02; o manifest do crate carrega `version = "1.1.2"` porque o parser SemVer rejeita zero à esquerda no componente patch.

### Corrigido
- Gap 2 — o teto de tokens levanta erro tipado com contexto acionável em vez de string genérica de validação: entrada acima de `EMBEDDING_REQUEST_MAX_TOKENS` levanta `AppError::TooManyTokens { tokens, limit }` (exit 6 preservado, mensagem PT-BR via `i18n`, `suggestion` nomeando o teto e a orientação de dividir em ~25000 tokens), completando a tipagem do exit 6 iniciada na v1.1.01 (`BodyTooLarge`/`TooManyChunks`) e fechando o residual L15. A guarda em `memory_guard::check_embedding_input_size` não emite mais `AppError::Validation`; é aplicada na borda de escrita de `remember`, `remember-batch` e `edit`, e dentro do cliente de embedding compartilhado (`embedding_api`), que também cobre o `ingest`.

### Removido (BREAKING)
- Gap 1 — o argumento `--gliner-variant` é removido de `remember` e `ingest`: desde a remoção do pipeline NER GLiNER na v1.0.79 ele sobrevivia como no-op silencioso (exit 0, sem warning). Seguindo o precedente do `--max-entity-degree` (v1.0.99), o clap agora o rejeita com o erro de argumento desconhecido (exit 2), o encanamento GLiNER morto em `constants.rs`/`extraction.rs` (batch size, teto de tokens, limiar de confiança, overrides de env do repositório do modelo) é deletado, e `tests/gliner_variant_removed_regression.rs` guarda ambos os subcomandos além de um `ingest --help` livre de gliner.

### Adicionado
- Gap 3 (cobertura de regressão) — `tests/reembed_entities_integration.rs` guarda o fix de dispatch de re-embed de entidades lançado na v1.1.01 (dispatch via `strip_prefix("entity:")` para `call_entity_description`/`call_reembed_entity`): afirma que `enrich --operation re-embed --target entities` preenche `entity_embeddings` (0 → N) e é idempotente.
- `enrich --prune-dead-entity-orphans` — flag dedicada para podar linhas `status='dead' AND item_type='entity'` da fila sidecar, complementando `--prune-dead-orphans` (apenas memory-keyed). Necessária porque o bug de re-embed da 1.1.1 deixou 14680 linhas entity-keyed em dead-letter que o podador memory-scoped não alcança. Somente leitura no banco principal, sem LLM, sem singleton.


## [1.1.01] - 2026-07-02

Fecha o roteiro de 12 prioridades catalogado no `gaps.md` a partir da auditoria do banco de produção: vetores de entidade/chunk passam a ser gravados e reprocessados pelo mesmo caminho REST OpenRouter das memórias, o cache `degree` torna-se reconciliável, relações literais (com hífen) tornam-se alcançáveis, a manutenção de entidades ganha seleção por ID, a cobertura torna-se observável e os erros de teto exit 6 são totalmente tipados. O schema permanece na versão 15. Nota: o nome oficial do release é v1.1.01; o manifest do crate carrega `version = "1.1.1"` porque o parser SemVer rejeita zero à esquerda no componente patch.

### Corrigido
- P1 — vetores de entidade não são mais silenciosamente pulados na escrita: com `--embedding-backend openrouter` a chain de embedding de entidade resolve para `[OpenRouter]` mesmo com `--llm-backend none` (REST, sem subprocesso), e um vetor vazio nunca persiste linha — `upsert_entity_vec`, `upsert_chunk_vec` e `memories::upsert_vec` ganham guarda de embedding vazio para que itens sem vetor permaneçam visíveis ao backfill do re-embed em vez de mascarados por um BLOB vazio.
- P10 — os predicados do `re-embed` selecionam linhas cujo `dim` armazenado diverge do `--embedding-dim` configurado (ou cujo blob está vazio), não apenas linhas ausentes, nas três tabelas de vetor (`memory_embeddings`, `entity_embeddings`, `chunk_embeddings`); vetores de dimensão legada finalmente são regenerados.
- P4 — `reclassify-relation --literal-from` casa a relação armazenada VERBATIM (sem normalização hífen→underscore na borda clap), tornando alcançáveis arestas legadas com hífen (ex.: `applies-to`); mutuamente exclusiva com `--from-relation`, e a guarda from==to compara o literal cru com o `--to-relation` normalizado, então `--literal-from applies-to --to-relation applies_to` é uma migração válida.
- P11 — os tetos de bytes/chunks levantam erros tipados com contexto acionável em vez de string genérica: `AppError::BodyTooLarge { bytes, limit }` e `AppError::TooManyChunks { chunks, limit }` (exit 6 preservado, mensagens PT-BR via `i18n`), substituindo `LimitExceeded` em todos os call-sites de tamanho de corpo (`remember`, `edit`, `ingest`, template de prompt do `enrich`).
- Task 8 — `cargo doc --no-deps` emite ZERO warnings: os links do doc de módulo do `chat_api` agora usam caminhos completos `crate::…` (o rustdoc resolve docs mesclados outer+inner no escopo do `lib.rs`), e docs públicas do `embedding_api` não fazem mais link intra-doc para itens privados.

### Adicionado
- P2 — backfill retroativo de embeddings: `enrich --operation re-embed --target memories|entities|chunks|all` (default `memories`, totalmente retrocompatível). Novos scanners para entidades/chunks sem vetor vivo, chaves de fila prefixadas `entity:`/`chunk:` com `item_type` por chave, dispatch por prefixo dentro de `call_reembed`, e `count_operation_backlog` reporta o `scan_backlog` por alvo no `--status` (paridade GAP-SG-77 preservada via predicados compartilhados).
- P3 — `graph recompute-degree`: reconcilia o cache `entities.degree` com a contagem real de arestas numa única transação IMMEDIATE, por namespace (ou todos), com `--dry-run` e o envelope `{namespace, dry_run, total, updated, zeroed, unchanged, elapsed_ms}`. Semântica alinhada ao helper canônico `recalculate_degree` (self-loop conta uma vez).
- P5 — desambiguação por ID na manutenção de entidades: `merge-entities --ids/--into-id` e `rename-entity --id`, com escopo de namespace, para que nomes duplicados entre namespaces deixem de bloquear merges/renomeações.
- P6 — observabilidade de cobertura real: `health --json` ganha `vec_memories_missing`, `vec_entities_missing`, `vec_chunks_missing` e os campos por tabela `vec_*_coverage_pct`; `embedding status --json` ganha os contadores `*_missing` por tabela.
- P12 — `ingest --name-prefix <PREFIX>`: prefixo kebab-case aplicado a todo nome de memória derivado, com o orçamento da parte derivada reduzido para que `prefixo + derivado` sempre respeite o teto de 80 caracteres.

### Alterado
- P7 — a desserialização de `EntityType` é um `Deserialize` manual com erro de borda rico (13 valores válidos + dicas) exposto como erro de Validation (exit 1) com validação precoce em `remember --entities-file`, em vez de erro serde/Json cru (exit 20); os vocabulários canônicos (tipos de entidade, relações) e a semântica de namespaces estão documentados.
- O `User-Agent` HTTP do OpenRouter agora é derivado de `CARGO_PKG_VERSION` em tempo de build (`sqlite-graphrag/1.1.1`), então não pode mais divergir da versão do crate como acontecia antes do GAP-SG-75.


## [1.1.0] - 2026-07-01

Resolve o backlog dead-letter do enrichment na raiz: completions truncadas do OpenRouter são detectadas e retentadas com orçamento maior, a classificação de retry de erro é 100% tipada (sem casar substring de mensagem), o cliente HTTP do OpenRouter é desduplicado num módulo compartilhado, e o loop de dequeue da fila fica limitado sob contenção de lock. O schema permanece na versão 15 (a fila sidecar do enrich ganha colunas de diagnóstico via ALTER idempotente).

### Corrigido
- GAP-SG-70 — completions truncadas não viram mais dead-letter: o `chat_api` desserializa `choices[].finish_reason` e, em `"length"`, reemite a requisição com `max_tokens` crescido (limitado por `ENRICH_MAX_LENGTH_RETRIES`) antes de tentar o reparo de JSON, quebrando o loop em que o retry reusava o mesmo orçamento e truncava idêntico.
- GAP-SG-73 — a classificação de retry é tipada, nunca por substring de mensagem: `classify_enrich_outcome` decide puramente por variante de `AppError`, e as falhas do OpenRouter carregam um `retry_class` computado na origem (status HTTP exato / código estruturado do provedor). A correção-chave do falso-permanente: uma falha de retry interno esgotado ("max retries exceeded") agora é `Transient` (elegível ao backoff `--max-attempts` da fila) em vez de dead-letter imediato.
- GAP-SG-76 — o dequeue do enrich não gira mais para sempre nem subprocessa em silêncio: `open_queue_db` seta `busy_timeout`, e o dequeue reusa o `with_busy_retry` limitado (cap, backoff exponencial + jitter, consciente do kill-switch), falhando de forma explícita com exit 15 sob contenção sustentada em vez de colapsar `SQLITE_BUSY` num falso "backlog vazio" via `.ok()`.
- GAP-SG-77 — o `enrich --status` reporta um `scan_backlog` real por operação (os candidatos do banco que um scan enfileiraria) em vez de apenas o `unbound_backlog` específico de memory-bindings, eliminando o falso `pending=0` para `entity-descriptions`, `body-enrich` e `re-embed`; o campo `state` deriva o veredito `pending-scan` do `scan_backlog` da operação atual. Uma nova função count-only `count_operation_backlog` compartilha os mesmos predicados WHERE com os scanners, de modo que o backlog reportado nunca diverge de um scan real.
- GAP-SG-78 — uma entidade ainda não materializada é classificada como `Transient` (retentada, não vai a dead-letter na primeira ausência) via uma variante tipada `AppError::EntityNotYetMaterialized`, substituindo o `NotFound` de string nos dois call sites de entidade (`entity-descriptions`, `entity-type-validate`); o lookup cego a namespace em `call_entity_type_validate` (que ignorava `_namespace` e casava só por `name`) foi corrigido para `WHERE namespace = ?1 AND name = ?2`.

### Adicionado
- GAP-SG-72 — diagnóstico de dead-letter: a fila sidecar do enrich ganha as colunas `finish_reason`, `input_tokens`, `output_tokens` (ALTER idempotente); `complete()` retorna `ChatCompletion`/`ChatError` carregando esses campos, e `--list-dead --json` os expõe.
- GAP-SG-71 — orçamento adaptativo de `max_tokens` no enrich: constantes nomeadas (`ENRICH_INITIAL_MAX_TOKENS`, `ENRICH_MAX_TOKENS_GROWTH_FACTOR`, `ENRICH_MAX_TOKENS_CEILING`, `ENRICH_MAX_LENGTH_RETRIES`) dimensionam o orçamento inicial e o crescimento, substituindo o antigo default ilimitado do provedor.

### Alterado
- GAP-SG-74 — DRY: o `ApiError`, `code_string`, `MAX_RETRIES` e `backoff` duplicados entre os clientes de chat e de embedding foram extraídos para o novo módulo `openrouter_http`, que também hospeda os classificadores `status_retry_class`/`provider_error_retry_class`.
- GAP-SG-75 — o `User-Agent` HTTP do OpenRouter foi atualizado para `sqlite-graphrag/1.1.0` (estava divergente em 1.0.95/1.0.96).


## [1.0.99] - 2026-06-30

Resolve o GAP-SG-67: a poda destrutiva global do degree cap foi removida, então uma escrita `remember`/`link` não pode mais deletar arestas históricas que pertencem a outras memórias. A escrita agora é puramente aditiva. O schema permanece na versão 15.

### Corrigido
- GAP-SG-67 — `remember`/`link` não deletam mais arestas históricas de outras memórias: a poda destrutiva global do degree cap foi removida, então uma escrita que referencia um hub de grau alto mantém toda aresta preexistente intacta e a contagem total de relações nunca diminui numa escrita normal.
- GAP-SG-68 — divergência doc/comportamento de `graph entities --sort-by degree` corrigida: o doc-comment de `EntitySortField::Degree` (e o `--help` herdado) prometia "descendente por padrão" mas a ordenação é ascendente; a doc foi alinhada ao comportamento ascendente real ("Use `--order desc` para os mais conectados primeiro"). Sem mudança de contrato SQL; os 6 testes `build_order_by_*` seguem verdes.
- GAP-SG-69 — `enrich --operation body-enrich --until-empty` agora converge: o scan não re-enfileira mais corpos curtos já vetados com `status='skipped'` pelo guard de preservação trigram-Jaccard. Novo helper `skipped_item_keys`; o scan inicial e o rescan excluem as chaves vetadas; o sidecar `.enrich-queue.sqlite` é mantido enquanto houver veredito `skipped` (removido só quando `dead==0` E `skipped==0`); `cleanup_queue_entry` limpa o veto quando o corpo muda. Empiricamente, items_total caiu 55→3 na segunda passada.

### Removido (BREAKING)
- Flag `--max-entity-degree` removida de `remember` e `link`; scripts que ainda a passarem recebem erro de argumento do clap (exit 2), e a mitigação obsoleta `--max-entity-degree 0` deixa de ser necessária.
- Função interna `graph::enforce_degree_cap` e seus dois call sites em `remember` e `link` removidos; a escrita nunca poda, deleta arestas nem emite warn de grau.


## [1.0.98] - 2026-06-29

Release de manutenção que deixa o pipeline de CI verde e restaura o fluxo de GitHub Release após a publicação da 1.0.97. O artefato 1.0.97 no crates.io é imutável, então as correções de código (doc comments em inglês, o advisory do `anyhow`, o escopo do preflight do OpenRouter) entram aqui; o resto são mudanças de CI/infra que não afetam o crate publicado.

### Corrigido
- O preflight da chave OpenRouter não falha mais em subcomandos read-only / sem embedding: o guard eager de `--embedding-backend openrouter` no `main` retornava exit 78 para *todo* subcomando quando nenhuma chave resolvia, inclusive `init` (só schema — já degrada para `ok_no_embedding`) e os inspetores da fila do `enrich` (`--status`/`--list-dead`/`--requeue-dead`/`--prune-dead-orphans`, que nunca embedam). O novo `Commands::tolerates_missing_embedding_key()` escopa o guard para esses rodarem sem chave; `remember`/`recall`/`hybrid-search`/`ingest`/`deep-research` continuam falhando rápido. Esta era a causa determinística das falhas do job de teste em ubuntu/macOS (`tests/enrich_queue_db_isolation.rs` afirma que um `init` sem chave tem sucesso).
- Advisory de segurança `RUSTSEC-2026-0190` (unsoundness em `anyhow::Error::downcast_mut()`): `anyhow` subiu 1.0.102→1.0.103 no `Cargo.lock`, zerando `cargo audit` e `cargo deny check advisories`.
- Política English-only: doc comments `///`/`//!` em `src/` e `tests/` que ainda carregavam português (origem da falha do job `language-check`) traduzidos para inglês; só comentários, sem mudança de comportamento.

### CI / infraestrutura
- Runners Windows: quatro steps `Pre-warm`/`Verify` declaravam `shell: pwsh` mas o corpo misturava cabeçalho bash `for … do … done` com `if`/`Start-Sleep` de PowerShell, então o PowerShell rejeitava o loop e os jobs clippy/test de windows-2025 morriam antes de rodar. Convertidos para `shell: bash` com o idioma de retry já provado no job de teste.
- Gate SemVer (G53): `cargo-semver-checks` fixado em 0.44.0 via `taiki-e/install-action` (binário pré-built); o `cargo install` sem pin pegava 0.48.0, que exige rustc 1.91 > o MSRV 1.88 do projeto e falhava ao compilar. Baseline subiu 1.0.79→1.0.96.
- Cross-check Windows MSVC (G29): o override `channel = "1.88"` do `rust-toolchain.toml` fazia o `cargo` usar 1.88 enquanto o target windows-msvc fora adicionado à `stable`, falhando com `error[E0463]: can't find crate for 'core'`. O target agora é adicionado à toolchain ativa; isso expôs que o `ring` (via reqwest+rustls) precisa do compilador MSVC, então o type-check cross roda por `cargo-xwin` (sysroot MSVC via LLVM). Verificado localmente no Fedora com `cargo xwin check --target x86_64-pc-windows-msvc --lib --all-features`.
- Tags: as tags divergentes `v1.2.0`/`v1.2.1`/`v2.0.0`..`v2.0.5`/`v2.1.0`/`v2.2.0`/`v2.3.0` (uma linhagem paralela cujos commits não são alcançáveis pela linha 1.0.x) foram removidas do remoto para a visão de Releases do GitHub seguir a linha de versão real.


## [1.0.97] - 2026-06-29

Esta release fecha o backlog de 56 gaps (`GAP-SG-01`..`GAP-SG-56`) catalogado em `gaps.md` a partir do ingest/enrich real do corpus rules-rust. O trabalho entrou em 5 commits: `eeb40d5` (Fase 0 fundação), `aaeebcc` (Fase A), `a67b863` (Fases B + C-F), `dc6b974` (Fases G + J + M), `f418957` (Fases H + I + K + L).

### Adicionado
- Fundação da Fase 0 (`eeb40d5`): novas dependências `llm_json` + `tiktoken-rs`, módulo `src/json_repair.rs` e helper `count_tokens` que sustentam a camada de resiliência abaixo
- Camada HTTP OpenRouter resiliente (`aaeebcc`, `GAP-SG-01/03/56`): o parse REST de embedding/chat ramifica por inspeção de campos (struct `Option<data>`/`Option<error>`) em vez de parse otimista; um HTTP 200 carregando `{error}` (estouro de tokens) agora propaga `code`/`message` reais em vez de `missing field data`; o `Retry-After` do servidor no 429 é exposto ao chamador via `RateLimited`
- Embedding, tokens e chunking (`a67b863`, `GAP-SG-02/04/05/06/07`): guard de tokens `EMBEDDING_REQUEST_MAX_TOKENS=30000` no boundary HTTP (distinto do `EMBEDDING_MAX_TOKENS=512` por chunk); `estimate_chunk_count` + `assess_body_budget` para o `--dry-run` reportar contagens de chunk/token/partição; auto-split nativo lossless por seção markdown em sub-memórias sob os limites de bytes/chunks/tokens
- Resiliência do enrich e recuperação de dead-letter (`a67b863`, `GAP-SG-08`..`16`/`18`/`19`/`21`..`28`/`42`/`45`/`46`): reparo de JSON via `json_repair::repair_to_value` mais guard de shape antes do parse estrito; non-JSON reclassificado de `HardFailure` para `Transient`; default de `--max-attempts` 5→8; `enrich --requeue-dead` (dead→pending) distinto de `--retry-failed`; `enrich --list-dead` com `error_class`/`message`; `waiting_items[]` expondo `next_retry_at` por item mais `--ignore-backoff`; fila indexada por `memory_id` com migração idempotente da coluna `operation`; `cleanup_queue_entry` em cascata em forget/purge/force-merge; o status passa a reportar waiting+dead, counts por operação, paralelismo scan-vs-drain e estado pending-scan; nova operação `augment-bindings` para memórias já vinculadas filtradas por `--names`; `body-extract` respeita `--names`; modo read-only `--body-extract-graph-only`; sidecar e `--names` documentados na ajuda
- Correções de parsing clap (`dc6b974`, `GAP-SG-29/30/31/33/34/35` + `GAP-SG-17`): `enrich --status/--list-dead/--requeue-dead` não exigem mais `--operation`/`--mode` (`required_unless_present_any`); `remember --graph-file` combinável com `--body-file` (fd separado); `allow_hyphen_values` em `--description`/`--body`; `--json` em toda variante de `config` (`config doctor --json`); `--llm-parallelism` declarado no `remember-batch`; `ingest --mode none --resume` agora falha fail-fast antes de qualquer IO; default de `--openrouter-timeout` elevado 300→600 para corpos densos
- Vocabulário canônico do grafo (`dc6b974`, `GAP-SG-47/48/49`): `EntityType::map_to_canonical` mapeia tipos não-canônicos (`platform`/`language`/`feature`→`concept`) em vez de descartá-los, com a lista canônica injetada no prompt de extração; `map_to_canonical_relation` unifica relações extraídas pelo LLM (`part-of`→`applies-to`); `graph::enforce_degree_cap` agora é acionável (poda a aresta de menor peso até grau ≤ cap), conectado em `link.rs`/`remember.rs`
- Nomes e observabilidade de escrita (`f418957`, `GAP-SG-37/38/39`): `remember --strict-name` rejeita um nome não-kebab devolvendo a forma canônica; aviso de truncamento promovido debug→warn com `truncated`/`original_name` no NDJSON; `AppError::suggestion()` emite `{error,code,message,suggestion}` em qualquer escrita não-zero
- Leitura, merge e prune (`f418957`, `GAP-SG-50/51/52`): `read --format raw` emite o body puro sem envelope; `remember --replace-graph` zera os vínculos antes de re-vincular (`entities:[]` limpa sem `forget`); `unlink --memory --entity` remove um binding curado que o `prune-ner` não alcança
- Inventário e ingest (`f418957`, `GAP-SG-53/54/55`): `list --json` emite um `truncation_warning` recomendando `export` como inventário confiável; `ingest --force-merge` atualiza duplicatas in-place; o `ingest` deduplica por `body_hash`, então naming divergente não duplica mais o conteúdo

### Corrigido
- Métricas que mentiam agora são fiéis (`a67b863`/`f418957`, `GAP-SG-40/41/43/44`): `chunks_persisted` lê o COUNT real pós-commit (`storage_chunks::count_for_memory`); `embedding status` reporta um objeto de coverage dos vetores reais nas tabelas em vez da fila assíncrona sempre vazia; `total_memories` é preenchido no `stats --json`; o `remember` checa o vetor pós-commit e recomenda `re-embed` quando ausente
- O guard de orçamento de tokens era medido em bytes enquanto o limite real do `qwen/qwen3-embedding-8b` é ~32K tokens; corpos grandes agora falham de forma previsível (ou fazem auto-split) antes da chamada de rede (`GAP-SG-02`)
- O dead-letter do enrich classificava falhas probabilísticas de schema como `HardFailure` permanente na primeira falha, matando itens recuperáveis; non-JSON agora é `Transient` com orçamento de 8 tentativas mais reparo na origem, e itens `dead` são recuperáveis via `--requeue-dead` (`GAP-SG-08/09/10/11/14/21`)

### Auditoria Pós-Selagem (working tree, GAP-SG-57..66)

A auditoria end-to-end que seguiu a selagem dos 56 gaps expôs um bloco de dívida técnica, fechado na working tree sobre os cinco commits acima:
- Modularização do enrich + auditoria de `unwrap`/`expect` + DRY do `parse_claude_output` (`ADR-0056`, `GAP-SG-57/58/59/60`): `src/commands/enrich.rs` (6013 linhas) dividido no módulo-diretório `src/commands/enrich/` (`mod.rs` 2355 + `queue` + `scan` + `postprocess` + `extraction`), os seis símbolos públicos (`run`, `EnrichArgs`, `EnrichOperation`, `EnrichMode`, `EnrichStatus`, `cleanup_queue_entry`) preservados e 36 testes do enrich intactos; a contagem real de `unwrap`/`expect` em produção era ~36 (não os 423 auditados, que contavam `#[cfg(test)]`), todos convertidos para `?`/`ok_or_else`/recuperação de poison e protegidos por `#![cfg_attr(not(test), warn(clippy::unwrap_used, clippy::expect_used))]` em `src/lib.rs` (o gate revelou mais 5 em `config_cmd.rs`); `claude_runner::parse_claude_output_opts(stdout, tolerate_max_turns)` remove a duplicação do `parse_claude_output` preservando a divergência de `max_turns` (protegida por `test_terminal_reason_max_turns_detected`)
- Cluster flaky `llm_slots::tests` endurecido (`GAP-SG-63`): os testes de slot sensíveis a contenção foram de ~8/10 falhas para 0/10 sob a suíte completa
- Classe de bug da fila CWD-relativa corrigida (`ADR-0057`, `GAP-SG-64` enrich + `GAP-SG-65` ingest): os sidecars de fila (`.enrich-queue.sqlite`, `.ingest-queue.sqlite`) resolviam contra o CWD do processo em vez do diretório do `--db`, então `enrich --status` reportava a fila errada quando o `--db` divergia do CWD e o `--resume`/`--retry-failed` do ingest perdia a fila ao mudar de CWD; novo helper `paths::sidecar_path(db_path, filename)` deriva o sidecar ao lado do banco (fallback gracioso para CWD no banco default), a const `DEFAULT_QUEUE_DB` do enrich é removida e `cleanup_queue_entry` ganha um parâmetro inicial `db_path`, `IngestArgs.queue_db` vira `Option<String>` sem default clap; teste de regressão `tests/enrich_queue_db_isolation.rs` planta uma fila ao lado de `db_a` e prova que `--status` a lê de um CWD não-relacionado; sem migração de arquivo legado (o default canônico coincide com o legado `./.enrich-queue.sqlite`)
- Limpeza: removida a const morta `constants::CLI_LOCK_FILE` (zero usos; o lock real usa `lock.rs` com o `cache_dir()` derivado do XDG)
- Auditoria de hooks + limpeza de dead-letter órfão (`ADR-0058`, `GAP-SG-66`): uma auditoria dos hooks do Claude Code achou `lib/graphrag-recover-dead.sh` chamando `pending list --filter-status dead` (rejeitado pela v1.0.97 — `dead` não é valor de `--filter-status` e `pending list` não aceita `--namespace`); corrigir isso expôs linhas dead órfãs (memória renomeada/purgada após enfileirar — 110 no banco real, todas `permanent` "not found") que `--requeue-dead` só re-falha e nenhum comando descartava. Novo inspetor read-only `enrich --prune-dead-orphans` (no grupo `required_unless_present_any`) deleta SÓ linhas `status='dead' AND item_type='memory'` cujo `item_key` está ausente do banco principal, reusando a query de existência do `enqueue_candidate`; `DeadSummary` ganha o campo `pruned` (neutro ao schema — não é struct dumpada). Hooks reconectados: `recover-dead.sh` usa `--list-dead` + `--prune-dead-orphans`, o residual do worker passa a emitir `total_dead` db-scoped (confiável desde o `ADR-0057`, consertando os consumidores `auto-enrich.sh`/`memory-guardian.sh` — GAP-B), e `GR_OPS_GATE`/`gr_dead_total`/`gr_prune_orphans` centralizados em `graphrag-common.sh`; teste unitário `prune_dead_orphans_removes_only_orphan_memory_rows` + smoke real podou 110 (`dead_total` 110→0, `pruned:110`)

### Notas de Auditoria
- Build limpo: 0 erros; `cargo clippy --all-targets -- -D warnings` 0 warnings; `cargo fmt --check` 0 diferenças
- Suíte de testes: `cargo test` default 1164 passou / 0 falhou; `cargo test --features slow-tests` 1522 passou / 0 falhou / 11 ignorados no momento da selagem (a suíte `installed_binary_smoke` foi pulada enquanto `~/.cargo/bin` ainda continha o binário stale 1.0.96); após o trabalho pós-selagem o `cargo install --path . --locked --force` realinhou o binário global para 1.0.97 e a `installed_binary_smoke` agora roda 26/0 SEM bypass (GAP-SG-62 resolvido), com `cargo test --lib` 973/0 e os novos testes de regressão (`paths::sidecar_path` ×3, `tests/enrich_queue_db_isolation.rs` ×1) verdes; `cargo fmt --check` 0 diferenças; `cargo clippy --all-targets --features slow-tests -- -D warnings` 0 warnings
- Contratos de teste sincronizados aos campos novos de saída: `docs/schemas/stats.schema.json` ganha `total_memories` (GAP-SG-43) e `docs/schemas/enrich-summary.schema.json` ganha `dead`/`waiting` (GAP-SG-15/16), mantendo a suíte estrita `schema_contract_strict` verde; novo teste de integração `test_read_format_raw_emits_pure_body` em `tests/integration.rs` valida o contrato de stdout cru do GAP-SG-50 end-to-end
- `gaps.md` atualizado: todas as 56 entradas `GAP-SG-NN` carregam um STATUS de resolução referenciando o commit de entrega; o `GAP-SG-20` permanece por design (`--rest-concurrency` intra-batch é o caminho de vazão) e o `GAP-SG-36` é verificado (o hook efetivo já libera `--help`)
- `GAP-SG-32` está funcionalmente resolvido (`--db` após o subcomando + `SQLITE_GRAPHRAG_DB_PATH`); apenas sua nota de doc fica adiada


## [1.0.96] - 2026-06-27

### Adicionado
- GAP-ENRICH-BACKLOG-CONVERGE: `enrich` ganha disciplina de dead-letter para que o backlog SCAN→JUDGE→PERSIST convirja comprovadamente em vez de ser re-escaneado indefinidamente. A fila `.enrich-queue.sqlite` ganha duas colunas via `ALTER TABLE` idempotente (`error_class`, `next_retry_at`) e um novo status terminal `dead`. As falhas por item são classificadas reusando `AttemptOutcome` e `compute_delay` de `src/retry.rs`: Transient (rate-limit / timeout / 5xx) agenda um backoff via `next_retry_at`, HardFailure (validação / parse) é terminal. Um item vira `dead` após `--max-attempts` (padrão 5) retentativas Transient ou na primeira HardFailure; o dequeue passa a respeitar `next_retry_at` e excluir `dead`, garantindo um conjunto vivo estritamente decrescente
- GAP-ENRICH-BACKLOG-CONVERGE: novos flags do `enrich` `--until-empty` (loop interno scan→drain que roda até a convergência, substituindo o loop de retry em bash externo), `--max-runtime <SECS>` (teto de tempo de parede que encerra o loop de forma limpa), `--max-attempts <N>` (orçamento de retentativas Transient antes de `dead`) e `--status` (relatório read-only de contagens de backlog/fila/dead que não chama o LLM nem adquire o singleton do enrich)
- GAP-OPENROUTER-REST-CONCURRENCY: o embedding via OpenRouter deixa de ser serial entre lotes. `embed_passages_parallel_with_embedding_choice` (`src/embedder.rs`) agora faz fan-out das chamadas REST por lote com um `tokio::task::JoinSet` bounded (sem dependência nova), preservando a ordem de saída pelo índice de chunk e fazendo clamp das requisições em voo para `1..16` (a faixa segura para o Cloudflare). O `enrich` ganha `--rest-concurrency` (padrão 8 para `--mode openrouter`, clamp `1..16`)

### Corrigido
- GAP-ENRICH-BACKLOG-CONVERGE: o backlog do enrich não convergia — falhas transientes deixavam itens enfileirados sem estado terminal e sem agenda de retry, então execuções repetidas re-escaneavam os mesmos itens não processáveis indefinidamente. A classificação dead-letter mais o dequeue ciente de `next_retry_at` fazem o conjunto vivo encolher estritamente até zerar
- GAP-OPENROUTER-REST-CONCURRENCY: o embedding OpenRouter emitia uma chamada REST por lote de cada vez, deixando a rede ociosa entre as idas e voltas em corpora multi-lote; o fan-out bounded com JoinSet sobrepõe as idas e voltas enquanto o caminho single-writer do SQLite permanece serializado via WAL + claim atômico

### Notas de Auditoria
- Build limpo: 0 erros; `cargo clippy --all-targets -- -D warnings` 0 warnings; `cargo fmt --check` 0 diferenças
- Suíte de testes: `cargo nextest run` 1086 passou, 0 falhou, 6 pulados; inclui 9 testes novos para a v1.0.96 (8 em `commands::enrich::tests`: classificar rate-limit/timeout/dbbusy→Transient, validação/parse→HardFailure, `open_queue_db` ALTER idempotente, `record_item_failure` hard→dead / transient→pending+next_retry_at / transient-no-cap→dead, dequeue pula retry-futuro e dead; 1 em `embedder::tests`: `reassemble_ordered_restores_input_order`)
- E2E: `enrich --status --json` retorna contagens read-only da fila (unbound_backlog, queue_pending/done/failed/dead/skipped, eligible_now, waiting) sem adquirir o singleton nem chamar o LLM; verificado contra uma `.enrich-queue.sqlite` legada migrada no lugar via ALTER idempotente (status `dead` populado)
- Cobertura: `retry.rs` (AttemptOutcome/compute_delay reusados) 93%; os helpers novos em `enrich.rs`/`embedder.rs` são cobertos cada um pelos testes unitários dedicados acima. Os percentuais de arquivo inteiro de `enrich.rs`/`embedder.rs` permanecem na baseline pré-existente (os grandes caminhos legados de LLM/subprocesso exigem rede ao vivo e nunca foram cobertos por testes lib-only — não é regressão)
- E2E ao vivo (OpenRouter real, 2026-06-27): GAP-OPENROUTER-REST-CONCURRENCY coberto pelo novo `tests/openrouter_live_concurrency.rs` (#[ignore], rode com --ignored) — 64 textos de `docs/*.md` embeddados com k=1 vs k=8; cosseno por índice diag_min 0,9999, off-diagonal máx 0,899, argmax 64/64 (ordem dos chunks preservada apesar da conclusão fora de ordem do `JoinSet`). Convergência do GAP-ENRICH-BACKLOG-CONVERGE coberta E2E ingerindo 6 ADRs de `docs/decisions` (`--mode none`) e então `enrich --until-empty --rest-concurrency 8`: unbound_backlog 6→0, os 6 vinculados, e uma 2ª passada idempotente faz 0 trabalho (items_total 0, 6ms)


## [1.0.95] - 2026-06-27

### Adicionado
- `GAP-OR-ENRICH`: novo `enrich --mode openrouter` roteia o JUDGE para o endpoint REST `/chat/completions` do OpenRouter, de modo que a extração estruturada (`memory-bindings`, `entity-descriptions`, `body-enrich`, etc.) não exige mais um subprocesso de CLI `claude`/`codex`/`opencode` instalado localmente. O pipeline SCAN→JUDGE→PERSIST permanece intacto; só o transporte do JUDGE muda
- Novo módulo `src/chat_api.rs` (`OpenRouterChatClient`) — cliente REST de chat espelhando `src/embedding_api.rs`: mesma política de retry/backoff (aborto imediato em 401/400/404, `retry-after` em 429, backoff exponencial + jitter em 5xx) e os mesmos headers mínimos (apenas `Authorization: Bearer`)
- Novos flags do `enrich`: `--openrouter-model` (OBRIGATÓRIO para `--mode openrouter`; a ausência é rejeitada com exit 1 antes de qualquer chamada de rede), `--openrouter-api-key` (env `OPENROUTER_API_KEY`), `--openrouter-timeout`, `--openrouter-base-url`
- Structured Outputs: as requisições enviam `response_format` `json_schema` com `strict: true` mais `provider.require_parameters: true`, de modo que apenas providers que honram o schema são roteados e a saída do modelo é JSON confiável, sem parsing frágil de stdout
- Reasoning desabilitado na extração (`reasoning.enabled: false`) para reduzir tokens pagos e latência, com fallback gracioso para reasoning-mandatory: `complete()` tenta primeiro com `enabled: false` e, num HTTP 400 mencionando `reasoning`, faz UM retry omitindo o campo `reasoning` para o modelo usar seu default obrigatório (helper `reasoning_disable_rejected`). 9 dos 13 modelos testados aceitam `enabled: false`; 4 (`minimax/minimax-m2.7[:nitro]`, `openai/gpt-oss-120b[:nitro]`) exigem o fallback
- O custo real por item é lido de `usage.cost` na resposta (sem o parâmetro depreciado `usage: {include:true}`) e somado ao total da execução

### Notas de Auditoria
- Build limpo: 0 erros, 0 warnings de clippy (`-D warnings`), 0 diferenças de fmt
- Suíte de testes: `cargo test` exit 0, 0 falhas
- E2E: `--mode openrouter` valida a chave de API sem spawnar subprocesso; todos os 13 modelos de texto OpenRouter exercitados contra o schema rígido passam (13/13 compatíveis — 9 diretamente com `reasoning.enabled: false`, 4 via o fallback reasoning-mandatory)


## [1.0.94] - 2026-06-26

### Corrigido
- GAP-EMBED-DIM-64: `DEFAULT_EMBEDDING_DIM` elevado de 64 para 384 (`constants.rs`); o init eager do OpenRouter em `main.rs` agora usa `constants::embedding_dim()` em vez do literal `unwrap_or(64)`. Bancos novos via `init` gravam `dim=384` no `schema_meta`, casando o corpus de produção; bancos legados em 64 preservados via precedência `schema_meta.dim` (sem re-embed forçado). O default 64 foi escolha deliberada do G42/v1.0.79 para reduzir custo de token autoregressivo no caminho codex — irrelevante agora que o OpenRouter REST é o padrão (truncamento MRL no servidor)
- GAP-EMBED-TIMEOUT-300: `DEFAULT_EMBED_TIMEOUT_SECS` elevado de 120 para 300 (`llm_embedding.rs`), alinhando o subprocesso de embedding com `ingest`/`enrich`/`opencode`/`llm_backend` que já usavam 300 (intenção do G42/BLOCO-4)
- GAP-HEADLESS-DEFAULT: `enrich --mode` agora é OBRIGATÓRIO (removido `default_value = "claude-code"`); omitir é rejeitado pelo clap (exit 2), evitando spawn acidental de `claude -p` que herda o `.mcp.json` do projeto e falha
- GAP-OR-ENTITY-EMBED: o embedding de entidades em `remember`/`remember-batch`/`ingest` agora honra `--embedding-backend`/`--llm-backend` roteando via `embed_passages_parallel_with_embedding_choice` (OpenRouter REST), com curto-circuito de chain `none` que retorna vetores vazios sem spawnar subprocesso. A chave de cache de entidade agora reflete o backend (`openrouter:{dim}`) para evitar colisão entre vetores codex e OpenRouter. `remember` com entidades novas cai de ~119s (timeout codex) para ~0,9s (OpenRouter REST)

### Notas de Auditoria
- Build limpo: 0 erros, 0 warnings de clippy (`-D warnings`), 0 diferenças de fmt
- Suíte de testes: `cargo test` exit 0, 0 falhas
- E2E: `init` grava `dim=384`; `enrich` rejeita `--mode` ausente; `remember` + entidade nova via OpenRouter = 913ms com `backend_invoked=openrouter`


## [1.0.93] - 2026-06-25

### Adicionado
- `GAP-OR-INGEST`: Backend de embedding OpenRouter — novos flags globais `--embedding-backend auto|openrouter|llm`, `--embedding-model`, `--openrouter-api-key` para embedding via API REST (~200ms vs 15s subprocess LLM); `EmbeddingBackendChoice` propagado para TODOS os 8 comandos de embedding (`remember`, `remember-batch`, `ingest`, `recall`, `edit`, `restore`, `hybrid-search`, `deep-research`)
- Novo flag `--enrich-after` para `ingest` — dispara `enrich --operation memory-bindings` sequencialmente após fase de embedding
- Novos módulos: `src/embedding_api.rs` (cliente REST OpenRouter com batch, retry, truncamento MRL), `src/config.rs` (config XDG para chave API), `src/commands/config_cmd.rs`
- Novas funções: `embed_passages_parallel_with_embedding_choice()`, `try_embed_query_with_embedding_choice()` em `embedder.rs`
- 10 modelos de embedding OpenRouter verificados E2E: `qwen/qwen3-embedding-4b`, `qwen/qwen3-embedding-8b`, `nvidia/llama-nemotron-embed-vl-1b-v2:free`, `openai/text-embedding-3-small`, `openai/text-embedding-3-large`, `perplexity/pplx-embed-v1-0.6b`, `mistralai/mistral-embed-2312`, `baai/bge-m3`, `google/gemini-embedding-001`, `google/gemini-embedding-2`
- `GAP-OR-PROPAGATION` totalmente resolvido: `EmbeddingBackendChoice` propagado para todos os 13 paths de embedding (8 originais + 5 secundários)

### Corrigido
- `BUG-OR-1`: `input_type="search_document"` hardcoded quebrava NVIDIA Nemotron; agora por modelo via `model_default_input_type()`
- `BUG-OR-2`: `model_supports_mrl()` não reconhecia NVIDIA e BAAI; adicionados `llama-nemotron-embed` e `bge-m3`
- `BUG-OR-3`: `qwen/qwen3-embedding-0.6b` listado como aprovado mas sem endpoints ativos no OpenRouter
- `BUG-OR-4`: `nvidia/llama-3.1-nemotron-embed-8b` listado mas não existe na API OpenRouter
- `BUG-OR-5`: HTTP 200 com corpo malformado causava falha imediata sem retry; erros de parse em 200 agora tratados como transitórios
- `GAP-OR-PROPAGATION`: 5 paths de embedding restantes agora respeitam `--embedding-backend openrouter` — `enrich --operation re-embed` (`reembed_memory_vector` + `call_reembed` + `persist_enriched_body`), `rename-entity` (embedding de entidade), `init` (probe smoke test), `ingest --mode claude-code` (4 call sites em `ingest_claude.rs`), chunks do `remember` (`embed_passages_parallel_local` → `embed_passages_parallel_with_embedding_choice`). `EmbeddingBackendChoice` propagado do `main.rs` para todos os 13 paths de embedding (8 originais + 5 corrigidos)
- `BUG-OR-EXIT-CODE`: 3 validações de configuração OpenRouter em `main.rs` emitiam exit code 1 em vez de 78 (EX_CONFIG) para erros de configuração (`--embedding-backend openrouter` sem `--embedding-model`, chave API ausente, falha de inicialização do cliente). Corrigido: os 3 agora emitem exit 78 via `ExitCode::from(78_u8)`

### Notas de Auditoria
- Build limpo: 0 erros, 0 warnings de clippy, 0 diffs de fmt
- Suite de testes: 1059 testes, 0 falhas
- E2E: 10/10 modelos OpenRouter passaram todas as operações (init, remember, recall, hybrid-search, edit, ingest, enrich re-embed, rename-entity)
- Todos os gaps/bugs fechados; 0 abertos


## [1.0.92] - 2026-06-24

### Adicionado
- `GAP-DOC-CRUD-001` a `GAP-DOC-CRUD-008`: 8 gaps de documentação remediados em COOKBOOK, HOW_TO_USE, AGENTS, HEADLESS_INVOCATION (EN+PT-BR); expansão CRUD com receitas para `forget`, `restore`, `edit`, `rename`, `purge`, `cleanup-orphans`, `vacuum`
- Auditoria de skills: arquivos de skill EN e PT-BR atualizados com documentação de subcomandos CRUD

### Notas de Auditoria
- Build limpo: 0 erros, 0 warnings de clippy, 0 diffs de fmt
- Todos os 8 gaps de doc fechados; 0 abertos


## [1.0.91] - 2026-06-23

### Corrigido
- **GAP-SPAWN-001** — Subprocessos LLM (`codex exec`, `claude -p`, `opencode run`) herdavam o CWD e `HOME` do chamador, causando walk-up de `.mcp.json` que carregava servidores MCP do projeto (PostgreSQL, SSH, docs-rs) em subprocessos headless de embedding. Isso causava timeouts de 120s ou erros 401 em todo `remember`/`recall`/`ingest` em projetos com `.mcp.json`. Correção: novos helpers `spawn_isolation_dir()` e `apply_cwd_isolation()` em `src/spawn/mod.rs` definem `current_dir` para um diretório temporário efêmero e `CLAUDE_CONFIG_DIR` para o mesmo diretório, bloqueando herança de MCP tanto do CWD quanto do nível de usuário. Aplicado em todos os 10 spawn sites de produção em `llm_embedding.rs`, `codex_spawn.rs`, `claude_runner.rs`, `opencode_runner.rs`, `ingest_claude.rs` e `enrich.rs`.
- **GAP-SPAWN-002** — Diretórios de spawn órfãos acumulavam em `/tmp/sqlite-graphrag-spawn-{PID}/` entre invocações da CLI. Adicionado `cleanup_spawn_dir()` em `main.rs` que remove o diretório de spawn do PID atual ao final da execução (caminhos de sucesso, erro e shutdown). Usa `remove_dir()` não-recursivo — seguro apenas para diretórios vazios.
- **BUG-14** — Teste `opencode_adapter_build_args` em `tests/spawn_version_adapter.rs` assertava a string `"headless"` que nunca foi retornada por `OpencodeAdapter::build_args()` (retorna `"run"` desde a refatoração da v1.0.90). Correção: asserção agora verifica `"run"`.
- **BUG-15** — 7 JSON schemas em `docs/schemas/` declaravam `backend_invoked` com enum `["claude", "codex", "none"]`, faltando os valores `"opencode"` e `"auto"` adicionados na v1.0.90. Consumidores validando contra o schema rejeitariam respostas válidas. Correção: todos os 7 schemas atualizados para `["claude", "codex", "opencode", "none", "auto"]`. Afetados: `embedding-status`, `enrich-summary`, `hybrid-search`, `recall`, `remember`, `ingest-summary`, `edit`.
- **BUG-16** — `deep-research.schema.json` não declarava o campo `vec_degraded` em `ResearchStats`, causando falha de validação `additionalProperties: false` no output real. Correção: adicionado `"vec_degraded": { "type": "boolean" }` ao schema e ao array `required`.
- **BUG-17** (ALTA) — Campo `entities.degree` armazenado era inflado por `increment_degree()` em `remember` e `ingest`. A função incrementava cegamente +1 por entidade por memória, mesmo quando a entidade não participava de nenhuma relação naquela chamada. Além disso, rodava ANTES da inserção de relações, então o grau era calculado sem considerar as relações da chamada atual. `graph stats` (que usa o campo armazenado) divergia de `graph entities` (que recalcula via subquery SQL). Correção: removido `increment_degree()` dos loops de entidade em `remember.rs` e `ingest.rs`; adicionada coleta de `HashSet<i64>` com todos os IDs de entidades afetadas (entidades + endpoints de relações); `recalculate_degree()` chamado para TODAS as entidades afetadas APÓS a inserção de TODAS as relações. `graph stats`, `graph entities` e o campo armazenado são agora consistentes.

### Notas de Auditoria
- Build limpo: 0 erros, 0 warnings de clippy, 0 diffs de fmt.
- Suite de testes: 877 testes lib + 21 testes doc + 38 testes de contrato de schema, 0 falhas.
- Auditoria E2E: 90 testes em DB vazio, CRUD, operações de grafo, busca, manutenção, validação e edge cases.
- Todos os 6 gaps/bugs fechados (GAP-SPAWN-001, GAP-SPAWN-002, BUG-14, BUG-15, BUG-16, BUG-17); 0 abertos.


## [1.0.90] - 2026-06-22

### Adicionado
- **GAP-OPENCODE-001** — Integração do backend OpenCode na pipeline de embedding e extração. Adicionada variante `Opencode` aos enums `EmbeddingFlavour`, `LlmBackendKindFactory` e `LlmBackendKind`. Novos `LlmEmbeddingBuilder::opencode_default()`, `invoke_opencode_async()`, `build_opencode_embedding_command()` e `opencode_embed_model()`. Auto-detecção via `which::which("opencode")`. Env vars: `SQLITE_GRAPHRAG_OPENCODE_BINARY`, `SQLITE_GRAPHRAG_OPENCODE_MODEL`, `SQLITE_GRAPHRAG_OPENCODE_EMBED_MODEL`. Cadeia de fallback estendida para `codex → claude → opencode → none`.
- **GAP-OPENCODE-002** — Integração do backend OpenCode nas pipelines de ingestão, enriquecimento e cadeia de fallback. Novo `--mode opencode` para `ingest` e `enrich`. Novos módulos `src/commands/ingest_opencode.rs` e `src/commands/opencode_runner.rs`. Novos flags CLI: `--opencode-binary`, `--opencode-model`, `--opencode-timeout`. Atualizado `parse_fallback_chain()` para reconhecer token `"opencode"`. Atualizado `dry_run_backend` para detectar opencode no PATH.
- **GAP-SKILL-OPENCODE-001** — Skills EN/PT atualizadas com documentação do backend OpenCode, env vars, flags CLI e exemplos de uso.

### Corrigido
- **BUG-AUDIT-001** — Contaminação cruzada de modelo opencode: `opencode_embed_model()` e `resolve_opencode_model()` não fazem mais fallback para `SQLITE_GRAPHRAG_LLM_MODEL` (que poderia conter um modelo codex). Precedência agora: `OPENCODE_EMBED_MODEL` > `OPENCODE_MODEL` > default `opencode/big-pickle`.
- **BUG-AUDIT-002** — Prompt de embedding reescrito com role-setting "You are an embedding function" para produzir vetores reais de 64 dimensões em vez de ser recusado pelo modelo.
- **BUG-AUDIT-003** — `env_clear()` no invoke do opencode agora preserva credenciais de provider (`OPENROUTER_API_KEY`, etc.) e configuração (`XDG_CONFIG_HOME`) via novo helper `propagate_opencode_env()`.
- **BUG-AUDIT-004** — `ingest_opencode` era um stub retornando `Err(Validation("under development"))`. Implementado completamente com loop de extração por arquivo, persistência de entidades/relações e stream de eventos NDJSON.
- **BUG-AUDIT-005** — Schema incorreto no `persist_memory_with_graph`: INSERT usava `entity_type` em vez da coluna `type`; faltava campo `body_hash` NOT NULL. Corrigido para corresponder ao schema SQLite.
- **GAP-ENRICH-OPENCODE-001** — `enrich --mode opencode` delegava silenciosamente para codex headless (13 match arms). Criado `call_opencode()` dedicado usando `opencode_runner`.
- **BUG-AUDIT-006** — Flag CLI `--opencode-binary` era declarada no clap mas ignorada. Criada `find_opencode_binary_with_override()` que respeita o caminho explícito.
- **BUG-AUDIT-007** — `spawn_with_memory_limit()` (RLIMIT_AS 4GB) crashava o runtime Bun usado pelo opencode. Criada `spawn_opencode()` com setsid mas sem RLIMIT_AS.
- **BUG-AUDIT-008** — `call_opencode()` no enrich ignorava o parâmetro `json_schema`. Schema agora é injetado no prompt quando não vazio para saída JSON estruturada.
- **BUG-AUDIT-009** — Preflight probe para opencode usava `spawn_with_memory_limit()` (mesmo crash RLIMIT_AS do BUG-007). Substituído por `spawn_opencode()`.
- **BUG-AUDIT-010** — `dry_run_backend` com mensagem de erro enganosa quando opencode era eclipsado pelo codex no PATH. Diferenciada mensagem para explicar prioridade vs ausência.
- **BUG-AUDIT-011** — Filtro `--names` ignorado silenciosamente em operações `entity-descriptions` e `body-enrich`. Adicionado parâmetro `name_filter` a `scan_entities_without_description()` e `scan_short_body_memories()` com SQL `WHERE name IN (...)`.
- **BUG-SLOT-TEST-001** — Teste `slot_enforces_max_concurrency` vazava `XDG_RUNTIME_DIR` causando colisão com slots reais do host. Criados helpers `isolate_slots_env()` / `restore_slots_env()`.
- **DOC-WARNING-001** — Warning de `cargo doc` "unresolved link to 0" em `preflight.rs:84`. Escapados colchetes: `argv\[0\]`.
- **DOC-WARNING-002** — Warning de `cargo doc` "unclosed HTML tag path" em `ingest.rs:122`. Convertido para código inline: `` `<path>` ``.
- **FMT-001** — Diferença de `cargo fmt --check` em `cli.rs:74`. Aplicado `cargo fmt`.
- **BUG-TIMEOUT-HARDCODE-001** — Timeout de embedding hardcoded em 60s causava exit 11 em corpos grandes. Adicionado campo `timeout_override: Option<Duration>` ao `LlmEmbedding` e `LlmEmbeddingBuilder`. Novos métodos `instance_embed_timeout()` e `instance_embed_timeout_for_batch()`. Removido `std::env::set_var` unsafe de `embed_batch_async()`.
- **BUG-WINDOWS-001** — Compilação no Windows falhava: 3 usos de `std::os::unix::process::ExitStatusExt` sem guard `#[cfg(unix)]`. Criado helper `extract_exit_info()` com branches `#[cfg(unix)]` e `#[cfg(not(unix))]`, substituindo 3 blocos inline (DRY + cross-platform).
- **BUG-PENDING-CLEANUP-DB-001** — `pending cleanup` não aceitava flag `--db`. Adicionado `db: Option<String>` ao `PendingCleanupArgs` e parametrizado `open_conn()`.
- **BUG-REMEMBER-BATCH-DRYRUN-001** — `remember-batch --dry-run` não era implementado (exit 2). Adicionado campo `dry_run` ao `RememberBatchArgs` com eventos de preview (`would_create`, `would_update`, `would_fail_duplicate`).
- **BUG-INGEST-SKIP-EMBED-001** — `ingest` ignorava `--skip-embedding-on-failure`. Alterado `StagedFile.embedding` de `Vec<f32>` para `Option<Vec<f32>>`, adicionados guards de skip nos 3 call sites de embedding.
- **BUG-GRAPH-DB-PROPAGATION-001** — `graph --db X stats|traverse|entities` ignorava flags do pai. Propagados `args.db` e `args.namespace` para subcomandos quando seus campos são `None`.
- **BUG-PENDING-EMBEDDINGS-DB-001** — `pending-embeddings list|abandon` não aceitava `--db`. Adicionado campo `db` às duas structs e parametrizado `open_conn()`.
- **BUG-LIST-TOTAL-COUNT-001** — `list` retornava `total_count` igual ao tamanho da página em vez do total global. Criada `memories::count()` com 4 variantes de query. `truncated` agora compara `items.len() < total_count`.

### Notas de Auditoria
- Build limpo: 0 erros, 0 warnings de clippy, 0 diffs de fmt, 0 warnings de doc.
- Suite de testes: 875 testes lib, 0 falhas.
- Todos os 24 gaps/bugs fechados; 0 abertos.

## [1.0.89] - 2026-06-19

### Corrigido
- **GAP-E2E-001** — Documentação do tamanho do binário agora corresponde à realidade. Binário de release medido em 15.321.016 bytes (14.6 MiB, 15.3 MB); descrição em `Cargo.toml:6` atualizada. A antiga alegação "6 MB" estava correta para o release LLM-only da v1.0.76 (apenas rusqlite + clap), mas o binário cresceu com novos recursos (GAP-002 split, GAP-058 env whitelist, GAP-E2E-007 schemars, helpers system-load + reaper, guard OAuth-only). Teste de regressão `tests/binary_size_documented_regression.rs::assert_documented_size_matches_real` faz parse da descrição do Cargo.toml e do binário para validar concordância dentro de 1 MiB.
- **GAP-E2E-002** — `health` agora aceita `--namespace <NAMESPACE>` como os 30+ outros subcomandos. Adicionado `pub namespace: Option<String>` ao `HealthArgs` e o namespace aparece no envelope JSON de `HealthResponse`. Teste `tests/health_namespace_regression.rs::health_accepts_namespace_flag` valida o flag.
- **GAP-E2E-007** — Schema JSON de `health` regenerado via derive `schemars 0.8` em `HealthResponse`. Adicionados 17 campos ausentes (`vec_memories_missing`, `vec_memories_orphaned`, `sqlite_version`, `mentions_ratio`, `mentions_warning`, `top_relation`, `top_relation_ratio`, `applies_to_ratio`, `relation_concentration_warning`, `super_hub_count`, `super_hub_warning`, `top_hub_entity`, `top_hub_degree`, `hub_warning`, `non_normalized_count`, `normalization_warning`, `fts_query_ok`). Trocado `additionalProperties: false` → `true` (política Must-Ignore por RFC 7493 I-JSON e `rules_rust_json_e_ndjson.md:33`). Novo binário `src/bin/dump_schema.rs` regenera o schema idempotentemente via `schema_for!()` + ordenação BTreeMap + aplicação recursiva de Must-Ignore. ADR-0048 (en + pt-BR) documenta a decisão Must-Ignore e a adoção de schemars 0.8. **MUDANÇA QUEBRANTE**: consumidores em modo strict devem migrar para Must-Ignore.
- **GAP-E2E-008** — Paridade do flag `--db` restaurada para `embedding status`, `embedding list`, `embedding abandon`, `pending list`, `pending show`. Decisão de NÃO usar `clap::Arg::global = true` documentada em ADR-0049. Teste `tests/cli_db_flag_parity_regression.rs::assert_db_flag_on_all_namespace_subcommands` valida 5 subcomandos.
- **GAP-E2E-009** — `migrate --dry-run --json` retorna relatório estruturado (`pending_migrations[]`, `pending_count`, `checksum_mismatches[]`, `status`) sem mutar o schema. Adicionado `--confirm`: runner padrão de migração espera literal "yes" no stdin antes de aplicar. Compatível com versões anteriores. Teste `tests/migrate_dry_run_regression.rs::dry_run_does_not_mutate_schema_history` confirma schema_version inalterado.
- **GAP-E2E-010** — `codex-models --json` retorna envelope JSON `{"action":"codex_models","count":N,"default":"...","models":[...]}`. `pending list --db` e `pending show --db` aceitam `--db`. Testes em `tests/codex_models_json_regression.rs` e `tests/cli_db_flag_parity_regression.rs`.
- **GAP-E2E-011** — Descrição de `ingest` não é mais hardcoded como `"ingested from <path>"`. Nova `extract_heuristic_description(body, path_hint)` extrai primeira linha significativa (>20 chars, não-header Markdown) truncada a 100 chars. Edge case FALTA-6 (corpo só com headers Markdown) cai para o stem do arquivo (ex.: `"headers-only"`). Novo flag `--no-auto-describe` restaura comportamento legado. Teste `tests/ingest_auto_describe_regression.rs` valida 5 cenários.
- **GAP-CODEX-BINARY** — Adicionado flag global `--codex-binary` com variável de ambiente `SQLITE_GRAPHRAG_CODEX_BINARY`, simétrico a `--claude-binary`. `detect_available()` em `llm_embedding.rs` agora honra a variável de ambiente para override do PATH.
- **GAP-FLAGS-MORTAS** — 7 flags globais de LLM (`--claude-binary`, `--codex-binary`, `--llm-model`, `--skip-embedding-on-failure`, `--llm-max-host-concurrency`, `--llm-slot-wait-secs`, `--llm-slot-no-wait`) agora propagados da CLI para variáveis de ambiente via `std::env::set_var` em `main.rs` antes do dispatch do comando. Corrige a ignorância silenciosa quando os flags eram passados via CLI em vez de variáveis de ambiente.
- **GAP-BACKEND-PROPAGATION** — `deep-research` e `remember-batch` agora recebem e USAM o parâmetro `llm_backend`. Anteriormente o parâmetro era aceito mas prefixado com underscore (`_llm_backend`) e ignorado. `--llm-backend claude` agora é honrado por ambos os comandos.
- **GAP-ADAPTIVE-TIMEOUT** — Adicionado `embed_timeout_for_batch(batch_size)` que escala: base + 15s por item adicional. `embed_batch_async()` agora usa timeout adaptativo. Lote de 1 item = 60s; lote de 8 itens = 165s.
- **GAP-OAUTH-HINT** — `invoke_claude()` agora detecta padrões de expiração de OAuth no stderr ("401", "Unauthorized", "expired", "login") e adiciona dica acionável: "Claude OAuth token may be expired; run `claude login` to renew".
- **GAP-MODEL-HARDCODE** — Removidos defaults de modelo hardcoded. `codex_embed_model()` e `claude_embed_model()` agora consultam `SQLITE_GRAPHRAG_LLM_MODEL` como fallback e emitem warning quando nenhum modelo está configurado.
- **GAP-META-006** — Eliminados 4 defaults "codex" hardcoded: `LlmExtractorConfig::default()` agora usa `detect_available_backend()` para resolução em runtime; `composite_backend::default_backend()` e `backend_from_kind()` agora resolvem dinamicamente em vez de chamar `with_default_codex()`; `remember_batch` e `deep_research` agora propagam `llm_backend` para chamadas de embedding.
- **BUG-SKIP-EMBED** — `--skip-embedding-on-failure` era um flag morto: aceito pelo clap, propagado para variável de ambiente em `main.rs`, mas NUNCA lido por nenhum módulo de embedding. Adicionados `should_skip_embedding_on_failure()` e `embed_passage_or_skip()` em `embedder.rs` que leem `SQLITE_GRAPHRAG_SKIP_EMBEDDING_ON_FAILURE` e retornam `Ok(None)` em vez de exit 11 quando o flag está ativo. `AppError::Validation` (enforcement OAuth-only) permanece fatal mesmo com o flag.
- **GAP-EMBED-PROPAGATION** — 7 sites de chamada em `init.rs`, `ingest_claude.rs` (4 sites), `rename_entity.rs` e `restore.rs` usavam `embed_passage_local` que ignora `--llm-backend`. Todos substituídos por `embed_passage_with_choice` que honra a seleção de backend do usuário via propagação de variável de ambiente.
- **GAP-WITH-DEFAULT-CODEX** — `LlmBackend::with_default_codex()` marcado `#[deprecated(since = "1.0.89")]`. 6 chamadores de teste em `tests/extract_backend.rs` migrados para `LlmBackend::new(LlmExtractorConfig::default())`. O método agora delega ao `Default` que resolve o backend dinamicamente via `detect_available_backend()`.
- **BUG-MODEL-VAZIO** — `codex_embed_model()` e `claude_embed_model()` retornavam string vazia quando nenhuma variável de ambiente estava definida, fazendo o codex falhar com "The '' model is not supported". Corrigido com defaults sensatos: `gpt-5.5` para codex, `claude-sonnet-4-6` para claude.
- **BUG-SKIP-EMBED-INCOMPLETE** — A correção anterior de BUG-SKIP-EMBED criou `embed_passage_or_skip()` com ZERO chamadores. O comando `remember` chamava `embed_passage_with_choice()` diretamente com `?`, propagando erros sem verificar `should_skip_embedding_on_failure()`. Corrigido envolvendo os 3 sites de chamada de embedding em `remember.rs` (passage, chunks paralelos, textos de entidade) com guards de erro skip-on-failure. `embedding` mudou de `Vec<f32>` para `Option<Vec<f32>>`, com `upsert_vec` condicionado a `Some`.
- **BUG-BUILDER-ENV-VAR** — `LlmEmbeddingBuilder::build()` não lia as variáveis de ambiente `SQLITE_GRAPHRAG_CLAUDE_BINARY` ou `SQLITE_GRAPHRAG_CODEX_BINARY`. Quando `--llm-backend claude` era forçado, o builder chamava `which::which("claude")` ignorando o override `--claude-binary` propagado via `set_var`. Corrigido: `build()` agora lê a variável de ambiente antes de cair para `which::which`. Precedência: `binary_override` > variável de ambiente > `which::which`.
- **BUG-BATCH-STATUS** — `remember-batch` retornava `status: "indexed"` para todos os itens independentemente de a memória ter sido criada ou atualizada. Corrigido: agora retorna `"created"` para novas memórias e `"updated"` para memórias existentes force-merged. Alinha com o contrato documentado (`created`/`updated`/`skipped`/`failed`).
- **BUG-BATCH-SKIP-EMBED** — `remember-batch` não honrava `--skip-embedding-on-failure`. Os 3 sites de chamada de embedding (update de passage, create de passage, textos de entidade) usavam `?` diretamente, propagando erros sem verificar `should_skip_embedding_on_failure()`. Corrigido com match guards idênticos à correção do comando `remember` (BUG-SKIP-EMBED-INCOMPLETE).
- **BUG-BOOLISH-ENV** — 4 flags booleanos de CLI com `env = "SQLITE_GRAPHRAG_*"` rejeitavam valores Unix padrão (`1`, `yes`, `on`) com exit 2. Causa raiz: campo `bool` com `env = "..."` no clap usa `bool::from_str` que aceita SOMENTE `"true"` e `"false"`. Corrigido adicionando `value_parser = clap::builder::BoolishValueParser::new()` a `--skip-embedding-on-failure`, `--strict-env-clear`, `--dry-run-backend` e `--llm-slot-no-wait`. Agora aceita `1`/`0`/`true`/`false`/`yes`/`no`/`on`/`off`.
- **BUG-RESTORE-BACKEND** — `restore` ignorava `--llm-backend` (hardcoded `None`) e não honrava `--skip-embedding-on-failure`. Corrigido: assinatura agora recebe `LlmBackendChoice`, embedding envolvido com match guard skip-on-failure, `upsert_vec` condicional a `Some(embedding)`.
- **BUG-RENAME-ENTITY-BACKEND** — `rename-entity` ignorava `--llm-backend` (hardcoded `None`) e não honrava `--skip-embedding-on-failure`. Corrigido: mesmo padrão de `restore`.
- **BUG-EDIT-SKIP-EMBED** — `edit` não honrava `--skip-embedding-on-failure`. A chamada de embedding usava `?` diretamente, causando exit 11 quando o LLM falhava em vez de persistir sem embedding. Corrigido: envolvido com match guard + `should_skip_embedding_on_failure()`, `upsert_vec` condicional a `Some(embedding)`.
- **BUG-STRICT-ENV-PROPAGATION** — O flag de CLI `--strict-env-clear` era silenciosamente ignorado. O flag definia `cli.strict_env_clear = true` mas `env_whitelist.rs` lê `std::env::var("SQLITE_GRAPHRAG_STRICT_ENV_CLEAR")` que nunca era definida. Corrigido: `main.rs` agora propaga o flag via `set_var` antes do dispatch do comando.
- **BUG-BATCH-FTS-DESYNC** — `remember-batch --force-merge` atualizava linhas de memória sem chamar `sync_fts_after_update`. O trigger FTS5 AFTER UPDATE está intencionalmente ausente (conflito com sqlite-vec), então operações UPDATE devem sincronizar o FTS manualmente. `remember` fazia isso corretamente; `remember-batch` omitia. Corrigido: adicionada captura de valor antigo + chamada `sync_fts_after_update` no caminho force-merge, espelhando `remember.rs`.
- **BUG-FORGET-DOUBLE-DELETE-VEC** — `forget` chamava `delete_vec` duas vezes para um soft-delete bem-sucedido: uma antes de `soft_delete` (linha 94, G39 Passo 4) e novamente depois (linha 135, dentro de `if forgotten`). A segunda chamada era redundante e produzia warnings espúrios de log. Corrigido: removida a chamada duplicada.
- **BUG-ENRICH-DESC-FTS-DESYNC** — `enrich --operation description-enrich` atualizava a coluna `description` via SQL bruto sem chamar `sync_fts_after_update`. O trigger FTS5 AFTER UPDATE está intencionalmente ausente, então o índice FTS ficava obsoleto após o enriquecimento de descrição. Corrigido: adicionada chamada `sync_fts_after_update` após o UPDATE em `call_description_enrich`.
- **BUG-ENRICH-BODY-EXTRACT-FTS-DESYNC** — `enrich --operation body-extract` atualizava a coluna `body` via SQL bruto sem chamar `sync_fts_after_update`. Mesma causa raiz de BUG-ENRICH-DESC-FTS-DESYNC. Corrigido: adicionada chamada `sync_fts_after_update` após o UPDATE em `call_body_extract`.
- **GAP-LLM-FALLBACK-DEAD-FLAG** — `--llm-fallback` (padrão `codex,claude,none`) era aceito pelo clap e exibido em `--dry-run-backend` mas NUNCA usado pelo pipeline real de embedding. `to_chain()` em `LlmBackendChoice::Auto` usava uma cadeia hardcoded. Corrigido: `main.rs` agora propaga `--llm-fallback` via `set_var`; `to_chain()` para `Auto` lê `SQLITE_GRAPHRAG_LLM_FALLBACK` via novo `parse_fallback_chain()` que parseia a string CSV em `Vec<LlmBackendKind>`. Tokens desconhecidos emitem `tracing::warn!` e são pulados; cadeia vazia cai para a canônica `[Codex, Claude, None]`.
- **BUG-YES-FLAG-IGNORED** — Três comandos destrutivos (`slots release`, `purge`, `cleanup-orphans`) declaravam `--yes` no clap mas nunca o aplicavam: `slots` imprimia um warning e depois deletava mesmo assim, `purge` nunca verificava o campo, `cleanup-orphans` imprimia progresso e depois deletava. Todos os outros comandos destrutivos (prune-ner, normalize-entities, vec purge, prune-relations, cache clear) abortam corretamente sem `--yes`. Corrigido: os três agora retornam `AppError::Validation` quando `--yes` está ausente, alinhando com a convenção do projeto.
- **GAP-RECALL-001** — Deadlock de embedding em recall e hybrid-search: o stdin agora é fechado antes de `wait_with_output`, o timeout de embedding por chamada foi reduzido de 300s para 30s, slots obsoletos são limpos via reaper, processos órfãos de sqlite-graphrag são ceifados, e a telemetria de embedding é exposta na resposta de health. Veja ADR-0050.
- **GAP-DEEPRESEARCH-001** — `deep-research` agora degrada graciosamente: o `embed_query_local()` de hard-fail foi substituído por `try_embed_query_with_deterministic_fallback()`, sub-queries aceitam um embedding `Option<&[f32]>` e caem para FTS5-only quando o LLM está indisponível, e um campo `vec_degraded` foi adicionado a `ResearchStats`.
- **GAP-JSON-FLAG-001** — Sete subcomandos (`pending list`, `embedding status`, `embedding list`, `embedding abandon`, `slots status`, `pending-embeddings list`, `pending-embeddings abandon`) agora aceitam `--json` como flag oculto no-op, prevenindo exit 2 quando operadores passam o flag padrão.
- **GAP-INIT-EMBEDDING-001** — `init` não sai mais com erro quando o embedding LLM está indisponível: a falha do smoke-test é capturada via match em vez de propagação `?`, o status retorna `"ok_no_embedding"` com o dim de `constants::embedding_dim()`, e o schema, tabelas, FTS5 e schema_meta são sempre criados.
- **GAP-LATENCY-001** — Apenas documentação, não é um bug: documentada a latência intrínseca de ~30-50s por chamada de embedding via codex exec como o custo fixo de ~11K tokens de contexto de sistema, com workarounds `--llm-parallelism 8`, `SQLITE_GRAPHRAG_EMBED_TIMEOUT_SECS=120`, `--llm-backend claude`, e a migração para dim=64 via `enrich --operation re-embed`.

### Notas de Auditoria
- Build limpo: 0 erros, 0 warnings de clippy, 0 diffs de fmt.
- Suite de testes: 847 testes lib + 1013 testes de integração + 21 testes doc = **1881 testes, 0 falhas, 7 ignorados**.
- Tamanho do binário: 15.323.128 bytes (14.61 MiB) — dentro de 1 MiB do documentado.
- Baseline do working tree preservado via tag `v1.0.88-baseline-2026-06-19` para rollback.

## [1.0.88] - 2026-06-19

### Corrigido
- **BUG-11 CRÍTICO** — `src/embedder.rs` agora invoca `preflight_check` antes de `Command::spawn()` no pipeline de embedding LLM. Bypass anterior significava que um `CLAUDE_CONFIG_DIR` populado (ex.: instalação real do Claude Code em `/home/comandoaguiar/.claude01`) era aceito pelo caminho de embedding enquanto rejeitado pelos outros 3 spawners, produzindo comportamento inconsistente. Restaura paridade com `claude_runner.rs`, `codex_spawn.rs` e `ingest_claude.rs`.
- **BUG-12 MÉDIO** — `src/output.rs:141` (`output::emit_error`) remove a chamada redundante de `eprintln!`. Apenas `tracing::error!` agora renderiza violação de OAuth-only para stderr. Stderr emite exatamente 1 linha por violação (eram 2). Validado por `oauth_stderr_emits_single_line_v1088`.
- **BUG-13 MÉDIO** — `src/commands/link.rs` agora rejeita abreviações ALL_CAPS de 4 caracteres ou menos na camada de link (anteriormente aceitas apesar do validador de entidade as rejeitar). Restaura simetria com `remember --graph-stdin` e `ingest --mode claude-code`.

### Adicionado
- **`ADR-0047`** (`docs/decisions/adr-0047-stderr-deduplication.md`) documenta decisão de BUG-12 + GAP-15.
- `tests/oauth_stderr_emits_single_line_v1088.rs` (cobertura para BUG-12).
- `tests/slots_no_println_integration.rs` (cobertura para GAP-15).

## [1.0.87] - 2026-06-19

### Adicionado
- **GAP-META-005 fechado** — módulo `src/spawn/preflight.rs` (≥200 linhas) com struct `PreFlightArgs` e enum `PreFlightError` (8 variantes). Atua como gate obrigatório antes de `Command::spawn()` nos 4 sites reais de spawn de subprocessos: `claude_runner.rs:255`, `codex_spawn.rs:273`, `ingest_claude.rs:297`, `extract/llm_embedding.rs:670`.
- Variante `AppError::PreFlightFailed` com exit code 16, `is_permanent=true`, e mensagens i18n bilíngues (EN + PT-BR).
- Helper `write_empty_mcp_config_tempfile()` escreve `{"mcpServers":{}}` em tempfile para que a substituição `--mcp-config <PATH>` funcione.
- Opt-out `is_skipped()` via `SQLITE_GRAPHRAG_SKIP_PREFLIGHT=1` para emergências (emite warning estruturado).
- 15 testes unitários em `src/spawn/preflight.rs::tests` cobrindo todos os 7 guards + caminhos de integração.
- **`ADR-0045`** (`docs/decisions/adr-0045-preflight-validation-layer.md`) documenta a decisão arquitetural.

### Corrigido
- **Bug 1** — `ingest --extraction-backend llm` não extrai mais silenciosamente `entities:0`; tracing de preflight emite `preflight_passed` para que operadores verifiquem que o spawn foi invocado.
- **Bug 2** — `--mcp-config '{}'` literal não é mais rejeitado pelo Claude Code 2.1.177 com "Invalid MCP configuration"; spawners agora substituem por tempfile contendo `{"mcpServers":{}}`.
- **Bug 3** — argv > `ARG_MAX - 4096` não falha mais com `E2BIG` pós-fork; preflight detecta o overflow antes de `cmd.spawn()` e aborta com erro estruturado.
- **Bug 4** — Parser JSON downstream não trunca mais silenciosamente em 65.536 chars; preflight valida `expected_output_bytes` contra o cap documentado de 65 KiB.
- **Bug 5** — Walk-up de `.mcp.json` a partir de diretórios pais não causa mais falhas de validação Zod mid-spawn; preflight sobe até 16 níveis de `workspace_root` e rejeita arquivos inválidos ANTES do fork.

## [1.0.86] - 2026-06-15

### Adicionado
- 10 novos subcomandos para o pipeline LLM: `pending list`, `pending show`, `pending cleanup`, `embedding status`, `embedding list`, `embedding abandon`, `pending-embeddings list`, `pending-embeddings process`, `slots status`, `slots release`.
- Família `pending` (V014 — tabela `pending_memories`) fornece checkpoint de 3 estágios para o pipeline `remember`. O checkpointer sobrevive a crash; no restart, operador pode usar `pending list` para inspecionar a fila e `pending show <id>` para entrada única.
- Família `embedding` expõe a fila de embedding LLM, com `--filter-status queued|processing|done|failed|skipped` e `--llm-backend codex,claude,none` para o pipeline retry-fallback.
- Família `slots` expõe o semáforo host-wide: `slots status` reporta `max_concurrency`, `acquired`, `waiting`, `held_by_pid[]`; `slots release --slot-id N --yes` ceifa slots órfãos.
- 6 novas flags globais: `--max-concurrency <N>`, `--wait-lock <SECONDS>`, `--llm-parallelism <N>` (padrão 4, clamp [1, 32]), `--ingest-parallelism <N>`, `--graceful-shutdown-secs <N>`, `--skip-embedding-on-failure` (válido apenas com `--llm-backend …,none`).
- Contenção de lock via `fs4 = 0.9` com `fcntl(F_SETLK)` em Unix e `LockFileEx` em Windows (ADR-0039).

### ADRs
- ADR-0036 (`pending_memories_staging.md`)
- ADR-0037 (`shutdown_json_envelope.md` — exit code 19)
- ADR-0038 (`llm_backend_user_choice.md` — flag `--llm-backend`)
- ADR-0039 (`llm_host_slot_semaphore.md`)
- ADR-0040 (`stderr_capture_fallback_chain.md` — incidente OAuth 401 codex de 2026-06-14)


## [1.0.85.2] - 2026-06-17

### Corrigido
- `--dry-run-backend` agora funciona standalone sem subcommand obrigatório. Resolvido BUG-001 (ADR-0044) com `pub command: Option<Commands>` em `src/cli.rs:248`. Exit 0 imprime JSON com `{action, backend, binary, model, flavour, chain, strict_env_clear}`.
- `embed_via_backend` retorna `Result<(Vec<f32>, LlmBackendKind), AppError>` propagando `resolved_kind`. Resolvido BUG-002 (ADR-0044). 7 envelopes JSON (edit, embedding-status, enrich-summary, hybrid-search, ingest-summary, recall, remember) agora populam `backend_invoked: "claude" | "codex" | "none"` consistentemente.
- `setup_mock_path()` em `tests/embedder.rs:37-77` corrigido para emitir JSON alinhado com expectation (não JSONL). Resolvido BUG-003 (ADR-0044). Testes `embed_via_backend_*` rodam sem mascaramento de formato.

### Suite de Testes
- 945 testes verdes via `cargo nextest -P ci`.

## [1.0.85.1] - 2026-06-17

### Corrigido
- `recall --llm-backend none` e `hybrid-search --llm-backend none` agora retornam exit 0 com envelope `vec_degraded: true` + `source: "fts_fallback"` + `vec_degraded_reason: "dim_zero"`. Resolvido GAP-004 (ADR-0043 hotfix) com braço intermediário em `src/embedder.rs:351`. Failsafe do v1.0.80 restaurado para o caso `--llm-backend none`.

### Suite de Testes
- 945 testes verdes via `cargo nextest -P ci`.

## [1.0.85] - 2026-06-17

### Corrigido
- `FallbackReason` estendido de 3 para 7 variantes (`SlotExhausted`,
  `OAuthQuota { backend }`, `BackendMismatch { requested, resolved }`,
  `DimZero`) para que os discriminadores de `recall` / `hybrid-search`
  possam distinguir exaustão de quota de exaustão de slot de bugs
  estruturais. Resolve GAP-003.
- `LlmEmbedding::invoke_claude` agora captura 12-14 headers
  `anthropic-ratelimit-*-remaining` ANTES de checar o exit status do
  subprocesso. Quando `requests-remaining=0` ou `tokens-remaining=0`,
  retorna `OAuthQuota` para que o fallback determinístico troque para
  codex imediatamente. Resolve G45-CR5.
- `try_embed_query_with_deterministic_fallback` re-tenta com o backend
  alternativo em `OAuthQuota` (codex ↔ claude) e dorme 750ms antes de
  desistir em `SlotExhausted`. Resolve G58.

### Adicionado
- `classify_embedding_error` em `src/embedder.rs` — função pura de
  mapeamento de `AppError` para `FallbackReason` via match lexical.
- `try_embed_query_with_deterministic_fallback` em `src/embedder.rs`.
- 5 novos testes de regressão em `tests/embedder.rs` cobrindo GAP-003,
  G58, G45-CR5, G55, G56.
- ADR `adr-0043-five-gap-remediation.pt-BR.md`.
- `.github/workflows/embedder-ignore.yml` rodando testes `#[ignore]`
  em env hermético (sem API keys).

### Mudado
- `Cargo.toml`: versão `1.0.84` → `1.0.85`.
- `gaps.md`: 5 entradas marcadas como `Solucionado em v1.0.85 (ADR-0043)`.
- `src/embedder.rs:289-317`: `acquire_llm_slot_for_embedding` reescreve
  `LockBusy` como `Embedding("slot exhausted: ...")` para que
  `classify_embedding_error` possa discriminar.
- `src/commands/{hybrid_search,recall}.rs`: call sites agora usam
  `try_embed_query_with_deterministic_fallback`.

### Suite de Testes
- 5 novos testes em `tests/embedder.rs` (regressão five-gap).
- 0 regressões em 830+ testes pré-existentes (`cargo nextest -P ci`).

## [1.0.84] - 2026-06-17

### Corrigido
- `--llm-backend claude` agora força invocação do binário `claude`
  sem o fallback silencioso para `codex` via `LlmEmbedding::detect_available`.
  O ramo `LlmBackendKind::Claude` em `embed_via_backend` agora delega
  para o novo `embed_via_claude_local` que constrói
  `LlmEmbedding::with_claude_builder()` diretamente. Resolve GAP-002.

### Adicionado
- Entry point `embed_via_claude_local` em `src/embedder.rs`.
- `LlmEmbeddingBuilder` em `src/extract/llm_embedding.rs` com
  `with_claude_builder`, `with_codex_builder`, `override_binary`,
  `override_model`.
- Campo `backend_invoked` em 7 envelopes JSON: `embedding status`,
  `remember`, `edit`, `ingest`, `recall`, `hybrid-search`, `enrich`.
- Campo `vec_degraded_reason` em `hybrid-search` e `recall`.
- Flag global `--dry-run-backend` que resolve e imprime o backend
  sem executar o subprocesso.
- Helper `apply_env_whitelist_for_claude` em `src/spawn/env_whitelist.rs`.
- `LlmBackendKind::as_str` e `FallbackReason::reason_code` em
  `src/embedder.rs`.
- ADR `adr-0042-claude-backend-split.md` (EN + pt-BR).
- 5 novos testes em `tests/embedder.rs` (regressão GAP-002).

### Alterado
- `Cargo.toml`: versão `1.0.83` → `1.0.84`.
- `src/embedder.rs:435-444`: ramo `LlmBackendKind::Claude` chama
  `embed_via_claude_local` em vez de `embed_passage_local`.
- `src/embedder.rs:205-218`: `embed_passage_with_choice` retorna
  `(Vec<f32>, LlmBackendKind)` em vez de `Vec<f32>`.
- `src/commands/embedding.rs:run_status` aceita `LlmBackendChoice`.
- `src/main.rs:391`: `Commands::Embedding(args)` propaga
  `cli.llm_backend`.

### Suite de Testes
- 5 novos testes em `tests/embedder.rs` (regressão GAP-002).
- 0 regressões em 818+ testes pré-existentes (cargo nextest -P ci).


## [1.0.83] - 2026-06-17

### Corrigido
- `claude_runner`, `codex_spawn` e `ingest_claude` agora preservam credenciais de provider customizado (`ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`, `OTEL_EXPORTER_OTLP_ENDPOINT`) no ambiente do subprocesso. Habilita uso de providers Anthropic-compatible (MiniMax/api.minimax.io, OpenRouter, gateways corporativos) sem alterar o mandato OAuth-only que continua rejeitando `ANTHROPIC_API_KEY`/`OPENAI_API_KEY`. Resolve parcialmente o gap G58 (fallback de `recall`/`hybrid-search` sob fadiga OAuth).

### Adicionado
- Novo módulo helper `src/spawn/env_whitelist.rs` consolidando a lógica de whitelist duplicada entre três spawners. Expõe `apply_env_whitelist(cmd, strict)` e `is_strict_env_clear()`.
- Nova flag global `--strict-env-clear` (env: `SQLITE_GRAPHRAG_STRICT_ENV_CLEAR=1`) para ambientes de compliance que proíbem encaminhamento de credenciais via env vars. Modo estrito preserva apenas `PATH`.
- Args de marcador orientativo `--oauth-only-resolution-use-anthropic-auth-token` (claude) e `--oauth-only-resolution-use-codex-auth-json-or-openai-base-url` (codex) expostos via pipeline de diagnóstico quando o guard OAuth-only dispara.
- Novos testes de integração em `tests/claude_runner_env.rs` (5 cenários) cobrindo propagação de provider customizado, abort OAuth-only, herança de base-url pelo codex, queda de credenciais em modo estrito, e auditoria de ausência de leak de token.
- Novo ADR `adr-0041-preserve-custom-provider-env.md` (EN + pt-BR) justificando a mudança arquitetural.

### Alterado
- `Cargo.toml`: versão `1.0.82` → `1.0.83`
- `src/commands/claude_runner.rs`: removidas constantes locais `ENV_WHITELIST`/`ENV_WHITELIST_WINDOWS`; agora delega para `apply_env_whitelist()`.
- `src/commands/codex_spawn.rs`: removido array inline de whitelist (linhas 277-293 anteriores); agora delega para `apply_env_whitelist()`. Isolamento de `CODEX_HOME` preservado como override de runtime após a chamada do helper.
- `src/commands/ingest_claude.rs`: removidos arrays inline de whitelist; agora delega para `apply_env_whitelist()`.

### Suite de Testes
- 3 testes unitários em `src/spawn/env_whitelist.rs` (`whitelist_includes_custom_provider_vars`, `whitelist_excludes_api_key_vars`, `strict_mode_drops_credentials`).
- 5 testes de integração em `tests/claude_runner_env.rs` (herméticos, sem rede).
- 0 regressões em 807+ testes pré-existentes (8 testes seriais OAuth-only permanecem verdes).


## [1.0.82] - 2026-06-15

### Adicionado
- **GAP-001 — Persistência por estágios**: nova tabela `pending_memories` (V014) com 6 transições de status e DAO em `src/storage/pending_memories.rs` (10 funções públicas). Subcomando `pending` com `list/show/cleanup` (`src/commands/pending.rs`).
- **GAP-002 — Envelope JSON de shutdown**: handler cross-signal (`SIGINT` via `ctrlc`, `SIGTERM`/`SIGHUP` via `signal-hook`) emite envelope JSON para stdout antes de exit com `code: 19` (`SHUTDOWN_EXIT_CODE`) determinístico. 3 testes em `src/signals.rs`.
- **GAP-003 — Escolha de backend LLM**: flag global `--llm-backend <auto|claude|codex|none>` (env: `SQLITE_GRAPHRAG_LLM_BACKEND`). Trait `LlmBackendFactory` com 4 implementações e 3 testes.
- **GAP-004 — Semáforo de slots cross-process**: novo módulo `src/llm_slots.rs` com RAII guard via `fs4::FileExt::try_lock_exclusive`. `acquire_llm_slot_for_embedding()` integrado em `embedder.rs`. Subcomando `slots` com `status/release/cleanup`.
- **GAP-005 — Captura de stderr + cadeia de fallback**: enum `LlmBackendError` com 4 variantes tipadas. Tabela `EXIT_CODE_HINTS` com 9 exit codes. Função `embed_with_fallback(backends, skip_on_failure)`. 2 subcomandos: `embedding` (status/list/abandon) e `pending-embeddings` (list/abandon).
- **5 ADRs novos** (0036-0040, todos bilíngues EN + pt-BR)
- **5 schemas JSON novos**: `slots-status`, `pending-list`, `embedding-status`, `embedding-list`, `shutdown-envelope`

### Mudado
- `Cargo.toml`: versão `1.0.81` → `1.0.82`
- `CURRENT_SCHEMA_VERSION`: `13` → `15` (V014+V015)
- `Cargo.toml`: adicionado `signal-hook = { version = "0.3", features = ["iterator"] }`
- `src/errors.rs`: nova variante `AppError::Shutdown { signal: String }` → exit 19
- `gaps.md`: 5 gaps marcados como `Solucionado em v1.0.82`

### Suíte de Testes
- 807 testes passando, 0 falhando, 1 ignorado (G58 S1 stub)

## [1.0.81] - 2026-06-14

### Corrigido (G80b, A2.1 v1.0.80)
- `codex-models` estava devolvendo 9 entradas no array `models[]` (incluindo chaves de metadados como `client_version`, `etag`, `fetched_at`) quando o arquivo de cache oficial da CLI Codex `~/.codex/models_cache.json` estava no formato padrão `{"models": [{"slug": "..."}, ...]}`. A saída agora extrai corretamente apenas as entradas de slug do array `models` e cai para chaves diretas somente quando o array está ausente. Regressão introduzida pelo G33 na v1.0.69, quando a whitelist estática virou a semente do merge de cache.


## [1.0.80] - 2026-06-14

### Mudanças na API da Biblioteca (per ADR-0032, G53 v1.0.80)

A API da biblioteca é **instável** em v1.x.y. Esta release é bump **patch**, então as mudanças na superfície da biblioteca abaixo são estritamente **aditivas** — nenhum re-export foi removido, nenhum campo público de struct foi renomeado, nenhuma assinatura de função foi alterada. O atalho publicado `sqlite-graphrag = "^1.0"` mantém os consumidores na trilha de estabilidade da CLI por padrão.

Novamente público em 1.0.80 (aditivo, sem quebra):

- `crate::embedder::embed_entity_texts_cached(models_dir, texts, parallelism) -> Result<(Vec<Vec<f32>>, EmbedCacheStats), AppError>` — cache em processo G56 para embeddings de entidades, chaveado por `(model, text)`. Retorna snapshot de stats com `requested`, `hits`, `misses` e helper `hit_rate() -> f64`.
- `crate::embedder::EmbedCacheStats` (struct) — G56 stats snapshot; `Default`, `Copy`, `Serialize`.
- `crate::embedder::EntityEmbedCacheMap` (type alias) — G56 `HashMap<u64, Arc<Vec<f32>>>` interno.
- `crate::lock::acquire_embedding_singleton(namespace, db_path, wait_seconds, force) -> Result<File, AppError>` — G45 singleton cross-process para embedding LLM por par `(namespace, db)`. Reusa `fs4` flock com o mesmo contrato de polling/force de `acquire_job_singleton`.
- `crate::errors::AppError::EmbeddingSingletonLocked { namespace }` — G45 nova variante estrutural; `is_retryable() == true`, exit code 75, mensagem localizada em pt-BR via `i18n::validation::app_error_pt::embedding_singleton_locked`.
- `crate::extract::llm_embedding::LlmEmbedding::model_label(&self) -> String` — G56 label estável combinando flavor (`"claude" | "codex"`) e modelo de embed ativo; usado como parte da chave do cache de entity-embed.

Nenhum símbolo público foi removido, renomeado ou teve sua assinatura alterada em 1.0.80. O fluxo do consumidor da biblioteca permanece inalterado: fixe em `=1.0.80` se depender da API da lib.

### Adicionado — G45: coordenação de embedding cross-process

- `acquire_embedding_singleton` serializa chamadas de embedding LLM por par `(namespace, db)` entre invocações CLI concorrentes. Uma segunda CLI tentando embedar contra o mesmo banco enquanto a primeira ainda está em voo recebe `EmbeddingSingletonLocked { namespace }` (exit 75) e pode passar `--wait-embed-singleton <SEGUNDOS>` para aguardar a soltura do lock. Bancos distintos (ou namespaces distintos) adquirem locks independentes; `fs4` flock é a primitiva subjacente, então o lock sobrevive a crashes de processo e é liberado automaticamente no drop.
- Operacionalmente o singleton previne a patologia de "duas invocações de remember no mesmo banco, dois subprocessos LLM, dois batches paralelos" que o cache em processo da v1.0.79 não conseguia endereçar.

### Adicionado — G53: política de estabilidade e gate de CI

- Novo job de CI `semver-checks` (informativo em v1.0.80, promovido a bloqueante em v1.0.81 quando as 9 violações MAJOR pendentes forem resolvidas). Roda `cargo semver-checks check-baseline --baseline-version 1.0.79`. O bug de `--manifest-path` duplicado no commit inicial da v1.0.79 está corrigido.
- README.md e README.pt-BR.md agora carregam uma seção `Política de Estabilidade` registrando a divisão CLI-estável/lib-instável per ADR-0032.

### Adicionado — G55 S2: `MemoryNotFound` estrutural

- `AppError::MemoryNotFound { name, namespace }` e `AppError::MemoryNotFoundById { id }` substituem o caminho legado `NotFound(String)` dentro de `read` e `hybrid-search`. O identificador solicitado agora é parte da variante, eliminando a classe de bugs `not found: unknown` que mascarava qual alvo de lookup falhou. As mensagens em pt-BR carregam nome e namespace explicitamente.

### Adicionado — G56: cache de entity-embed em processo

- `embed_entity_texts_cached` fica na frente de `embed_passages_parallel_local` para batches de nome de entidade. Chave do cache é `blake3(model || "\0" || text)`. A taxa de hit é alta em `ingest` (entidades canônicas re-embedadas entre muitas memórias) e modesta em `remember` e `remember-batch`. `remember.rs`, `ingest.rs` e `remember_batch.rs` agora roteiam embeddings de entidade pelo cache; embeddings de chunk continuam no caminho raw porque a unicidade de chunk torna a taxa de hit desprezível. Stats são emitidas via `tracing::debug!` (G56 hit/miss/request counts).

### Adicionado — G58: fallback de recall e hybrid-search para FTS5

- `recall --fallback-fts-only` e `hybrid-search --fallback-fts-only` roteiam a query via FTS5 BM25 quando o subprocesso LLM falha (rate limit, contenção OAuth, dim divergente). Os novos campos do envelope `vec_degraded` (bool), `vec_error` (string) e `warning` (string) são preenchidos simetricamente em ambos os comandos. Os testes de `recall` e `hybrid-search` ganharam cobertura para o caminho FTS5-only; 1 teste é `#[ignore]` porque o stub G58 S1 exige PATH sem `codex` ou `claude` para exercitar `EmbeddingFailed`.

### Adicionado — G53-WINDOWS-INFRA: pre-warm e verify steps em windows-2025 (ADR-0033)

- Os jobs `clippy` e `test` da matrix windows-2025 ganharam 2 steps novos cada (gateados `if: matrix.os == 'windows-2025'`, no-op em ubuntu/macos): um pre-warm que baixa o toolchain rustup no cache do runner antes do build, e um verify step que re-checa `rustup show active-toolchain` após install. Os 2 modos históricos de falha de infra (download do rustup com erros transitórios de rede e `E0463 can't find crate for core` quando a stdlib do target está ausente) agora são recuperáveis na primeira re-run em vez de acumularem como CI vermelho.
- Validação local de cross-compile: `cargo check --target x86_64-pc-windows-msvc --lib --all-features` reproduzido e o `E0463` resolvido via `rustup target add x86_64-pc-windows-msvc --toolchain 1.88`; o build então atinge a fronteira `cc-rs: failed to find tool "lib.exe"`, que é o limite esperado de cross-compile MSVC a partir de host Linux. ADR-0033 documenta a justificativa e a fronteira.

### Adicionado — Resiliência de SHUTDOWN: saída sem panic no terceiro sinal (ADR-0034)

- `src/signals.rs` agora envolve o handler do primeiro sinal em uma barreira de captura de panic: mesmo quando o stderr do pai é um pipe fechado (o cenário de processo órfão que a auditoria G42/C2 identificou), o handler retorna limpo em vez de `SIGABRT`-ar em `BrokenPipe`. O terceiro Ctrl-C consecutivo sai com código 130 e ZERO I/O, casando com o contrato documentado em ADR-0034 e a receita em `docs/HEADLESS_INVOCATION.md`.
- A receita de bypass SHUTDOWN em 3 camadas (`nohup` → `setsid` → `disown`) agora é a referência canônica para o harness do agente ao rodar jobs longos de embedding em background; HEADLESS_INVOCATION.md e COOKBOOK.md carregam o snippet.

## [1.0.79] - 2026-06-11

### Removido

- **Infraestrutura de daemon totalmente removida**: `src/daemon.rs` (1120 linhas), `src/commands/daemon.rs` (79 linhas), `tests/daemon_integration.rs` (316 linhas) deletados. Struct `DaemonOpts` e flag `--autostart-daemon` removidos de todos os argumentos de comando. Todas as chamadas `crate::daemon::embed_*_or_local` substituídas por wrappers diretos `crate::embedder::embed_*_local`. CLI agora é 100% one-shot com zero IPC. 8 constantes de daemon removidas de `src/constants.rs`. Remoção líquida: ~764 linhas.
- **Features legadas de modelo local totalmente removidas (antecipando o cronograma da v1.1.0)**: as features Cargo `embedding-legacy`, `ner-legacy` e `full` sumiram, junto com as dependências opcionais `fastembed`, `ort`, `ndarray`, `tokenizers` e `hf-hub` e o arquivo `src/extraction_gliner.rs`. `EmbeddingBackend` agora é um stub permanente que retorna erro de migração claro; `extract_graph_auto` perdeu o caminho de delegação GLiNER; `calculate_safe_concurrency` orça comandos pesados com `LLM_WORKER_RSS_MB` (350) em vez da constante ONNX obsoleta de 1100 MB (`EMBEDDING_LOAD_EXPECTED_RSS_MB` deletada). A matriz de CI encolhe para `default` + `llm-only`. Todo build é LLM-only; não existe caminho de modelo local.

### Depreciado

- **Flags da era GLiNER são no-ops formais com aviso explícito**: `--gliner-variant` (em `remember` e `ingest`) e `ingest --mode gliner` agora emitem um aviso de deprecação via `tracing::warn!` quando usadas; `--enable-ner` executa apenas extração de URL por regex. Todos os help strings foram reescritos para parar de prometer o pipeline GLiNER removido (variantes de modelo, tamanhos, thresholds); `SQLITE_GRAPHRAG_GLINER_VARIANT`/`_MODEL`/`_THRESHOLD` continuam aceitas por compatibilidade mas sem efeito.

### Corrigido — G42: pipeline de embedding LLM lento, serializado e frágil

- **S1 — dimensão de embedding configurável (default 64)**: fonte única de verdade em `constants.rs` (`DEFAULT_EMBEDDING_DIM` + `embedding_dim()`); precedência flag `--embedding-dim` > env `SQLITE_GRAPHRAG_EMBEDDING_DIM` > `schema_meta.dim` do banco aberto > 64. Bancos 384-dim existentes continuam funcionando sem mudança. ZERO alteração de schema (a chave `dim` e as colunas já existiam). Base: MRL, arXiv 2205.13147 — output por vetor cai de ~3072 para ~512 tokens (~6x)
- **S2 — chamadas LLM em lote**: `embed_batch_async` embeda N textos numerados por chamada com o schema `{items:[{i,v}]}`; chunks em lotes de 8, nomes de entidade em lotes de 25 (bases de calibração em dim 64; adaptativos à dim desde o G44) — 39 spawns de subprocesso viram 4-5
- **S3 — paralelismo real**: fan-out bounded com `Arc<Semaphore>` + `acquire_owned` + `JoinSet` + `join_next`/`is_panic` em `embedder.rs`; o Mutex global agora protege APENAS o clone da config (o antigo `flush_group` o segurava durante 30-60s de I/O de rede, forçando paralelismo efetivo 1); resultados fluem por canal mpsc BOUNDED (backpressure + entrega incremental); permits = min(`--llm-parallelism`, cpus, ram*0.5/350MB, 32); nova flag `--llm-parallelism` em `remember` (default 4), `ingest` (default 2, multiplica com `--ingest-parallelism`) e `edit`
- **S4 — schema tempfile RAII**: os arquivos `--output-schema` do codex são `NamedTempFile`s com nome randomizado criados uma vez por processo (sem write+delete por chamada, sem race por PID); o reaper de órfãos agora também remove diretórios `codex-home-{pid}` cujo PID morreu
- **S5 — modelo claude via env**: `SQLITE_GRAPHRAG_CLAUDE_EMBED_MODEL` (simétrico à var do codex); zero modelo hardcoded sem override
- **S6 — `CLAUDE_CONFIG_DIR` vazio por padrão** no caminho de embedding: honra `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR`, senão usa o gerenciado `~/.local/state/sqlite-graphrag/claude-empty-config` (mode 0700, copia `.credentials.json` quando presente); as flags de isolamento MCP são silenciosamente ignoradas upstream (anthropics/claude-code#10787) e um `~/.claude` completo custava ~223k tokens por chamada (~40-50s → ~10-15s)
- **S7 — erro codex headless acionável**: falhas `request_user_input` agora explicam causa e remediação em vez de um exit 11 opaco
- **S8 — handler de sinais sem panic**: primeiro sinal usa `writeln!` best-effort (BrokenPipe ignorado); segundo sinal sai com 130 e ZERO I/O — elimina o SIGABRT em processos orfanados (`panic = "abort"` + pipe de stderr fechado)
- **S9 — re-embed one-shot canônico**: `enrich --operation re-embed --limit N --resume` documentado como caminho oficial; nova flag `edit --force-reembed` regenera o embedding sem alterar o body; removida das docs MIGRATION/HOW_TO_USE a receita QUEBRADA de pre-warm (`edit --description "<mesmo>"` nunca re-embedou)
- **C5 — sem normalização silenciosa de dimensão**: `normalise_dim` (truncar/preencher) substituída por `validate_dim`, que falha em vetores divergentes; o parser de batch valida cobertura de índices e dimensão por item
- Todo subprocesso LLM agora usa `kill_on_drop(true)` mais `tokio::time::timeout` explícito (`SQLITE_GRAPHRAG_EMBED_TIMEOUT_SECS`, default 300s); um runtime multi-thread por processo substitui o runtime current-thread por chamada
- Novos testes de concorrência: pico nunca excede os permits (AtomicUsize), task que panica devolve o permit via RAII e aparece como `is_panic`, cancelamento encerra o fan-out rapidamente, dimensão divergente falha o fan-out

### Corrigido — G43: adoção da dimensionalidade não cobria os comandos principais

- **Adoção da dim em toda abertura de conexão**: o sync do G42/S1 (`schema_meta.dim` → dim ativa) só rodava dentro de `ensure_db_ready`, que `remember` / `edit` / `recall` / `hybrid-search` nunca chamam — esses comandos usavam silenciosamente o default compilado (64) contra bancos 384 pré-v1.0.79, gravando embeddings de dimensões misturadas que pontuam cosseno 0.0 entre si (o recall vetorial ficava cego ao corpus antigo). `open_rw` E `open_ro` agora adotam a dim registrada do banco (best-effort, o override por env continua vencendo); 4 testes de regressão cobrem adoção rw/ro, precedência do env e bancos virgens
- **`init` não carimba mais `dim=384`**: o `INSERT OR REPLACE ... ('dim', '384')` hardcoded marcava bancos NOVOS com uma dim que contradiz o default ativo; substituído por `INSERT OR IGNORE` com a dim ativa (preserva a dim registrada em re-init de banco existente)
- **`rename-entity` não grava mais `dim=384` e nome de modelo removido**: o INSERT duplicado (`384` + `multilingual-e5-small` hardcoded) foi substituído pelo writer canônico `upsert_entity_vec` (tamanho real do vetor, versão da CLI como `model`)
- **Mocks de teste falam os dois formatos de embedding**: `tests/mock-llm/{claude,codex}` devolviam um vetor fixo de 384 dims no formato single, então TODA a suíte de integração `slow-tests` falhava desde o G42/S1+S2 (o gate nunca roda no CI, escondendo o problema); os mocks agora devolvem vetores de 64 dims e respondem ao schema de batch `{items:[{i,v}]}`; os 2 testes obsoletos de daemon viraram guardas de regressão da remoção; `.config/nextest.toml` não filtra mais pelo binário deletado `daemon_integration` — suíte de integração `--features slow-tests` de volta ao verde

### Corrigido — G44: tamanho do lote de embedding não escalava com a dimensionalidade

- **Lote adaptativo à dim**: os lotes do G42/S2 eram FIXOS (8 chunks / 25 nomes de entidade por chamada LLM), calibrados para o default dim 64 (~512 / ~1600 floats por resposta); em bancos legados 384 o mesmo lote de chunks pedia ~3072 floats — medido em produção: claude devolveu 3 de 8 itens (capturado pelo coverage check G42/C5) e codex estourou os 300s, falhando o `remember` 2 vezes. O tamanho do lote agora se adapta por `clamp(base×64/dim, 1, base)` (`embedder.rs::adaptive_batch_for_dim`): dim 64 mantém 8/25, dim 384 usa 1/4 — orçamento de floats constante por chamada, sem necessidade do workaround `SQLITE_GRAPHRAG_EMBED_TIMEOUT_SECS`; 6 testes de regressão cobrem a fórmula e os wrappers de env-dim

## [1.0.78] - 2026-06-09

### Corrigido

- **G41**: `run_rehash` não insere mais linhas fantasma para migrações não aplicadas — o branch `else` que registrava V013 sem executar o SQL foi removido
- **Reparo G41**: novo helper `ensure_v013_tables_exist` detecta e repara bancos onde V013 foi registrada em `refinery_schema_history` mas as tabelas BLOB-backed (`memory_embeddings`, `entity_embeddings`, `chunk_embeddings`) nunca foram criadas
- Reparo automático integrado em `ensure_db_ready` — qualquer comando CRUD repara bancos corrompidos por G41 incondicionalmente

### Adicionado

- Campo `v013_tables_created` (boolean) nas respostas JSON de `RehashReport` e `ToLlmOnlyReport`
- 3 novos testes unitários para `ensure_v013_tables_exist` (noop, reparo phantom, sem histórico)
- 1 teste unitário atualizado: `rehash_does_not_insert_missing_migrations` (substitui teste que validava comportamento bugado)
- ADR-0028 documentando a correção e estratégia de reparo do G41

### Migração

- Atualizar: `cargo install sqlite-graphrag --version 1.0.78 --force`
- O reparo automático é incondicional: qualquer comando (`remember`, `recall`, etc.) repara bancos corrompidos por G41
- Reparo explícito: `sqlite-graphrag migrate --rehash` ou `migrate --to-llm-only --drop-vec-tables`
- Nenhuma intervenção manual em SQL necessária

## [1.0.77] - 2026-06-09

### Corrigido

- INSERT do `run_rehash` agora inclui `applied_on` com timestamp RFC3339 via `chrono::Utc`
- Helper `sanitize_null_applied_on` corrige linhas NULL existentes antes do refinery executar
- Helper `remove_vec_virtual_tables_without_module` limpa shadow tables vec0 via `PRAGMA writable_schema`
- `debug-schema` não crasha mais em bancos com `applied_on = NULL`
- Campo `applied_on` mudou de `String` para `Option<String>` na saída do debug-schema

### Adicionado

- Campo `null_rows_fixed` nas respostas JSON de `RehashReport` e `ToLlmOnlyReport`
- Campo `vec_tables_removed_via_writable_schema` na resposta JSON de `ToLlmOnlyReport`
- 4 novos testes unitários cobrindo sanitização, fix do INSERT e remoção de vec tables
- 2 novos testes de integração para o fluxo de fix do `applied_on` NULL
- ADR-0027 documentando a decisão do fix G40

### Migração

- Upgrade é automático: `cargo install sqlite-graphrag --version 1.0.77 --force && sqlite-graphrag migrate`
- Nenhuma intervenção manual em SQL é necessária
- v1.0.77 detecta e corrige linhas com `applied_on` NULL automaticamente
- Veja `docs/MIGRATION.md` para detalhes

## [1.0.76] - 2026-06-07

> **Mudança arquitetural quebrante.** O build padrão agora é **LLM-only e one-shot**.
> Não há daemon, não há runtime ONNX, e não há cache local de modelo no build padrão.
> Toda geração de embedding, NER e busca vetorial é delegada para `claude -p` ou `codex exec` headless (OAuth, sem MCP, sem hooks). A matriz do CI agora roda 3 feature flags em paralelo: `default`, `llm-only` e `embedding-legacy`.

### Removido

- **`fastembed` 5.13.4** — geração de embedding agora passa por `LlmEmbedding` em `src/extract/llm_embedding.rs`, que spawna `claude -p` ou `codex exec` com `--output-schema` impondo um array `f32` de 384 dimensões.
- **`ort` 2.0.0-rc.12** — sem runtime ONNX no build padrão; a LLM faz a inferência.
- **`ndarray` 0.16** — sem necessidade; vetores vivem em BLOB.
- **`tokenizers` 0.22** — substituído por heurística de tokenização por whitespace em `src/tokenizer.rs`. `CHARS_PER_TOKEN` usa a mesma calibração que o restante do crate.
- **`huggingface-hub` 0.4** — sem download de modelo.
- **`GLiNER NER`** em `extraction_gliner.rs` — movido para a feature `ner-legacy`. O build padrão usa apenas regex de URL; NER completo vem do `ExtractionBackend` LLM em `src/extract/`.
- **`sqlite-vec` 0.1.9** — REMOVIDO. As virtual tables `vec_memories`, `vec_entities`, `vec_chunks` são dropadas pela migração `V013` e substituídas por tabelas regulares com BLOB: `memory_embeddings`, `entity_embeddings`, `chunk_embeddings`. Similaridade de cosseno calculada em Rust puro sob demanda em `src/similarity.rs`.
- **Daemon como otimização de performance** — o subcomando `daemon` continua presente para compatibilidade de fonte, mas toda requisição `EmbedPassage`/`EmbedQuery` agora passa pelo LLM one-shot, derrotando o propósito original. O daemon será removido na v1.1.0.

### Adicionado

- **Trait `ExtractionBackend` (solução G21)** — novo módulo `src/extract/` expõe um trait com quatro implementações: `LlmBackend` (padrão, invoca `claude -p` ou `codex exec` headless), `EmbeddingBackend` (pipeline legado fastembed, stub quando LLM-only), `NoneBackend` (no-op para skip explícito) e `CompositeBackend` (combina múltiplos backends em paralelo). Flag global `--extraction-backend llm|embedding|none|both` seleciona o backend em runtime; LLM é o novo padrão.
- **Trait `VersionAdapter` (solução G22)** — novo módulo `src/spawn/` abstrai invocações de spawn de executor atrás de um trait. Três adapters concretos: `CodexAdapter` (detecta `codex 0.130.0` até `0.138+` e adapta flags — `codex 0.137.0` removeu `--ask-for-approval` em favor de `-a never`, e o adapter emite a nova flag automaticamente), `ClaudeAdapter` (claude code 2.1.0+) e `OpencodeAdapter` (opencode headless). O trait também expõe `ExecutorVersion` (construído em `semver::Version`), `CompatMode` (`strict` | `lenient` | `auto`), `ExecutorCapabilities`, `VersionCache` e um `ErrorPropagator` que propaga o stderr do subprocess para o usuário em vez de engolir (causa raiz do G22 P16).
- **Concorrência adaptativa (solução G18)** — `MAX_CONCURRENT_CLI_INSTANCES` subiu de 4 para 16 (fallback legado). Nova função `crate::lock::calculate_safe_concurrency()` lê `sysinfo::System::available_memory()` e calcula uma contagem dinâmica de permits via `min(cpus, available_mb / worker_cost_mb)`. Nova constante `LLM_WORKER_RSS_MB = 350` para workers LLM-only (vs `EMBEDDING_LOAD_EXPECTED_RSS_MB = 1100` para o caminho legado fastembed). O fator `* 0.5` que causava o teto de 4 slots foi removido.
- **Feature flag `llm-only` (fundação G23)** — feature opt-in que opta o build fora do pipeline fastembed + ort. Já é o comportamento padrão; a feature agora é o marcador explícito para o flip da v1.1.0. `embedding-legacy` é reconhecido por checks `cfg!()` em `src/lock.rs` para que a fórmula adaptativa escolha o `worker_cost_mb` correto em builds com feature.
- **`tracing` respeita `RUST_LOG`** — removido o feature `release_max_level_info` estático do `tracing`, então operadores podem sobrescrever o nível de log em runtime via `RUST_LOG` (ajuda G22 P17).
- **`migrate --rehash`** — reescreve checksums registrados de migração para casar com o conteúdo atual via `SipHasher13(name|version|sql)`. O algoritmo casa com `refinery-core 0.9.1` (a versão que o binário embute); mesmo crate `SipHasher13`, mesma ordem de hash. Necessário para bancos v1.0.74 que sobem para v1.0.76 porque `V002` foi intencionalmente esvaziada para no-op.
- **`migrate --to-llm-only`** — upgrade one-shot para bancos v1.0.74 / v1.0.75: rehash + aplica `V013` + reporta estado das vec tables. Requer `--drop-vec-tables` como guarda de segurança explícita.
- **Tabelas de embedding BLOB-backed** — `memory_embeddings`, `entity_embeddings`, `chunk_embeddings` substituem as antigas virtual tables sqlite-vec. Cosseno em Rust puro em `src/similarity.rs` (ADR-0020, ADR-0022).
- **Fluxo de credencial LLM OAuth-only (ADR-0025)** — o spawn LLM ABORTA com `AppError::Validation` se `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` estiverem definidas no ambiente. Ambas as variáveis são excluídas da whitelist de env-clear como defesa em profundidade.

### Mudado

- **CLI é one-shot por padrão** — os comandos `remember` / `ingest` / `edit` / `recall` / `hybrid-search` não disparam mais autostart do daemon para embeddings. Cada embedding é um subprocesso `claude -p` ou `codex exec` novo (um turno OAuth por chamada).
- **Mudança de workflow do operador** — para manter a latência de embedding sob controle, operadores devem rodar `claude` ou `codex` fora do `sqlite-graphrag` (ex.: como uma unit systemd ou loop watchexec) e deixar o binário chamá-los quando precisar.

### Migração

- **Migração `V013` dropa as vec tables.** Bancos v1.0.74 existentes perdem seus embeddings antigos; eles são recomputados lazy no próximo `remember` / `ingest` / `edit`.
- **Operadores que querem preservar vetores antigos** podem fazer dump das vec tables antes de rodar `init --force`.
- **Caminho de upgrade recomendado** — veja `docs/MIGRATION.md` para o procedimento passo a passo v1.0.74 → v1.0.76, incluindo `migrate --to-llm-only --drop-vec-tables`.
- **Procedimento de rollback** — `cargo install sqlite-graphrag --version 1.0.75 --force` restaura o build legado, depois `init --force` recria as vec tables (embeddings são perdidos a menos que dumpados antes).

### Dependências

- `async-trait = "0.1"` — necessário para que os traits `ExtractionBackend` e `VersionAdapter` sejam dyn-compatible.
- `semver = "1"` com feature `serde` — necessário para o parse de `ExecutorVersion` em `src/spawn/`.
- `siphasher = "1.x"` (pinado) — necessário para calcular checksums de migração deterministicamente. Já está no grafo de build transitivamente via `refinery-core 0.9.1`; esta entrada torna o link explícito.
- **REMOVIDAS:** `fastembed 5.13.4`, `ort 2.0.0-rc.12`, `ndarray 0.16`, `tokenizers 0.22`, `huggingface-hub 0.4`, `sqlite-vec 0.1.9`.

### Testes

- 745 testes de lib preservados da baseline v1.0.74.
- Mock LLM CLI injetado em 26 arquivos de teste para o caminho de build LLM-only.
- 107/115 testes previamente lentos corrigidos no commit `bd0a3f5` (mock LLM desbloqueia CI de turnos OAuth reais).
- Matriz CI de 3 features: `default`, `llm-only`, `embedding-legacy` rodam clippy e testes em paralelo.
- 12 novos testes em `tests/extract_backend.rs` (LLM, Embedding, None, Composite, factory, dispatch, hints, health).
- 13 novos testes em `tests/spawn_version_adapter.rs` (Codex, Claude, Opencode, version matrix, parse, JSONL).
- 6 novos testes em `tests/concurrency_adaptive.rs` (fórmula legacy não divide mais, budget de worker LLM, teto máximo).
- 4 novos testes em `tests/migrate_rehash_integration.rs` (DB saudável no-op, fix de checksum corrompido, sucesso to-llm-only, recusa de safety guard).
- 11 novos testes unitários em `src/commands/migrate.rs` (determinismo de checksum, histórico no-op, reescrita de checksum corrompido, idempotência, detecção de vec table).
- 4 testes em `tests/signal_handling_integration.rs` verificados verdes (4/4) — 3 falhas pré-existentes corrigidas pelo fix de fallback do daemon da v1.0.75.
- 7 testes em `tests/v2_breaking_integration.rs` verificados verdes (7/7) — 2 falhas pré-existentes corrigidas.

### Validação

- `cargo check --all-targets --no-default-features --features llm-only`: 0 erros.
- `cargo check --all-targets --no-default-features --features embedding-legacy`: 0 erros.
- `cargo check --all-targets` (default): 0 erros.
- `cargo clippy --all-targets --all-features -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: 0 diferenças.
- `cargo build --bin sqlite-graphrag --release` (default, LLM-only): builda em ~25s, binário 14.6 MiB.
- `cargo build --bin sqlite-graphrag --release --no-default-features --features embedding-legacy`: builda em ~1m 11s, binário 39 MB.
- `cargo test --lib`: 745 passaram.
- `cargo test --all-features`: verde nos 3 feature flags.
- Binário de release (build padrão) reporta `sqlite-graphrag 1.0.76`, sem runtime ONNX, sem `libonnxruntime.so` requerido.

### Documentação

- Novo: `docs/HOW_TO_USE.md` (221 linhas) — reescrito para v1.0.76 LLM-Only.
- Novo: `docs/MIGRATION.md` (147 linhas) — v1.0.74 → v1.0.76 passo a passo.
- Novo: `docs/AGENTS.md` (1428 linhas) — header atualizado, arquitetura LLM-Only, OAuth enforcement, flags de hardening.
- Atualizado: `docs/COOKBOOK.md` — adicionada receita "Como Atualizar De v1.0.74 Ou v1.0.75 Para v1.0.76"; receita do daemon atualizada com aviso DEPRECATED; nota de latência atualizada.
- Novo ADR: `adr-0019-llm-only-one-shot.md` (PT-BR: `adr-0019-llm-only-one-shot.pt-BR.md`).
- Novo ADR: `adr-0020-pure-rust-cosine.md` (PT-BR).
- Novo ADR: `adr-0021-deprecate-daemon.md` (PT-BR).
- Novo ADR: `adr-0022-blob-embeddings.md` (PT-BR).
- Novo ADR: `adr-0023-remove-tokenizers.md` (PT-BR).
- Novo ADR: `adr-0024-fts5-coarse-cosine-refine.md` (PT-BR).
- Novo ADR: `adr-0025-oauth-only-embedding.md` (PT-BR).
- Novo ADR: `adr-0026-v002-vec-tables-migration-drift.md` (PT-BR).
- Novo schema: `migrate-rehash.schema.json` (resposta de `migrate --rehash --json`).
- Novo schema: `migrate-to-llm-only.schema.json` (resposta de `migrate --to-llm-only --json`).
- Novo doc: `docs/HEADLESS_INVOCATION.md` (promovido do gaps.md) — como invocar Claude/Codex/OpenCode headless sem MCP, OAuth-safe.

## [1.0.75] - 2026-06-07

### Adicionado

- **Trait ExtractionBackend (solução do G21)**: o novo módulo `src/extract/` expõe a trait `ExtractionBackend` com quatro implementações: `LlmBackend` (padrão, invoca a CLI claude code / codex headless), `EmbeddingBackend` (pipeline fastembed legado, stub quando LLM-only), `NoneBackend` (no-op para skip explícito) e `CompositeBackend` (mescla múltiplos backends em paralelo). A flag global `--extraction-backend llm|embedding|none|both` seleciona o backend em runtime; o backend LLM passa a ser o padrão. A trait usa `async-trait` para dispatch `dyn`, devolve `ExtractionOutput { entities, relationships, embedding, backend, elapsed_ms }` estruturado, e é a fundação para a migração LLM-only da v1.1.0.

- **Trait VersionAdapter (solução do G22)**: o novo módulo `src/spawn/` abstrai as invocações de spawn de executor por trás da trait `VersionAdapter`. Três adaptadores concretos são entregues: `CodexAdapter` (detecta codex 0.130.0 até 0.138+ e adapta flags — o `codex 0.137.0` removeu `--ask-for-approval` em favor de `-a never`, e o adaptador emite a flag nova automaticamente), `ClaudeAdapter` (claude code 2.1.0+) e `OpencodeAdapter` (opencode headless). A trait também expõe `ExecutorVersion` (construída sobre `semver::Version`), `CompatMode` (`strict` | `lenient` | `auto`), `ExecutorCapabilities`, `VersionCache` e um `ErrorPropagator` que propaga o stderr do subprocesso ao usuário em vez de engoli-lo (era a causa raiz do G22 P16).

- **Concorrência adaptativa (solução do G18)**: `MAX_CONCURRENT_CLI_INSTANCES` subiu de 4 para 16 (fallback legado). A nova função `crate::lock::calculate_safe_concurrency()` lê `sysinfo::System::available_memory()` e calcula uma contagem dinâmica de permits via `min(cpus, available_mb / worker_cost_mb)`. Nova constante `LLM_WORKER_RSS_MB = 350` para workers LLM-only (contra `EMBEDDING_LOAD_EXPECTED_RSS_MB = 1100` do caminho fastembed legado). O fator de divisão `* 0.5` que causava o teto de 4 slots foi removido.

- **Feature flag `llm-only` (fundação do G23)**: feature opt-in que tira o build do pipeline fastembed + ort. Atualmente é um alias no-op no build padrão; é a fundação para a virada de default da v1.1.0. `embedding-legacy` é reconhecida por checks `cfg!()` em `src/lock.rs` para que a fórmula de concorrência adaptativa escolha o `worker_cost_mb` certo em builds gateados por feature.

- **tracing respeita `RUST_LOG`**: a feature estática `release_max_level_info` foi removida do `tracing`, para que operadores possam sobrescrever o nível de log em runtime via a variável de ambiente `RUST_LOG` (ajuda o G22 P17).

### Corrigido

- **Fallback gracioso do cliente do daemon em desconexão no meio da requisição**: `request_or_autostart` agora chama `wait_for_daemon_ready` depois de um auto-restart por mismatch de versão e depois de um `ensure_daemon_running` novo, para que o cliente nunca corra com um socket recém-criado que o kernel ainda está conectando. `request_if_available` e os caminhos de leitura/escrita no loop de requisição do daemon tratam `ConnectionReset`, `BrokenPipe` e `ConnectionAborted` como "daemon indisponível" (`Ok(None)`) e caem para o embedder local. O novo helper `is_daemon_gone()` centraliza o casamento. Este era um bug latente preexistente exposto pelo bump da v1.0.75, que introduziu um restart forçado do daemon no processo de teste; ele causava 3 + 2 falhas preexistentes em `signal_handling_integration` e `v2_breaking_integration` respectivamente. Ambas as suítes passam verdes agora (4/4 e 7/7).

- **Lints `useless_vec` e `cast` do clippy** em `src/embedder.rs` e `src/extract/`. As alocações `vec![single]` foram substituídas por arrays `[single; 1]` (o fastembed 5.13.4 quer `AsRef<[S]>`), e um cast `as usize` sobre um campo `usize` foi removido. `cargo clippy --all-targets -- -D warnings` está limpo.


## [1.0.74] - 2026-06-05

### Corrigido

- **Compatibilidade no-op do `--skip-extraction` restaurada (promessa v1.0.45 honrada)**: a v1.0.67 (commit 9ddb17b) promoveu a depreciação de `--skip-extraction` de `tracing::warn!` para um `AppError::Validation` hard em `src/commands/remember.rs:415-417` e `src/commands/ingest.rs:1057-1059`. Isso quebrou a promessa do CHANGELOG v1.0.45 de "kept as a hidden no-op for backwards compatibility" e começou a falhar 5 jobs do CI (Slow Contract Suites, Tests ubuntu/macos, Coverage threshold, cargo-careful sanity) cujos testes E2E usam a flag para pular o download do modelo GLiNER-ONNX. Revertido para `tracing::warn!` com mensagem que espelha o texto da v1.0.45 acrescido de uma dica para remover a flag.

- **`Windows MSVC cross-compile (G29)` falhou com `error[E0463]: can't find crate for 'core'`**: a action `dtolnay/rust-toolchain@stable` executa internamente `rustup toolchain install stable --target x86_64-pc-windows-msvc --profile minimal`, mas `--profile minimal` ignora `--target`, então a cross stdlib nunca é baixada. O build falhava em `cfg-if` e `libc` (os primeiros crates compilados para o target estrangeiro). Adicionado um step explícito `rustup target add x86_64-pc-windows-msvc --toolchain stable` após a action de toolchain para garantir a instalação confiável da cross stdlib.

- **`Miri Unsafe Validation` falhou com `can't call foreign function 'mi_malloc_aligned' on OS 'linux'`**: `mimalloc` (o alocador global definido em `src/main.rs:3-4`) chama `mi_malloc_aligned`, função que o Miri não consegue modelar. Adicionado `RUSTFLAGS="--cfg sqlite_graphrag_miri"` ao job Miri e gateado o `#[global_allocator]` com `#[cfg(not(sqlite_graphrag_miri))]`. O step do Miri agora usa o alocador padrão do Linux enquanto binários de produção continuam com o ganho de velocidade do mimalloc. Registrado o novo cfg em `[lints.rust].unexpected_cfgs.check-cfg`.

- **Três erros de `-D warnings` em `Tests (windows-2025)` e `Clippy (windows-2025)`**: `RUSTFLAGS=-D warnings` transformou os avisos de dead-code em `src/reaper.rs:17` (`unused import: std::time::Duration`), `:19` (`ORPHAN_MIN_AGE_SECS is never used`) e `:20` (`ORPHAN_SCAN_TARGETS is never used`) em erros hard no Windows, onde os internals do reaper são `#[cfg(unix)]`. Gateado os três itens com `#[cfg(unix)]` e os dois testes que os referenciam com `#[cfg(unix)] #[test]`. O build no Windows não flagra mais como dead-code itens que não pode usar.

### Validação

- `cargo check --all-targets`: 0 erros
- `cargo clippy --all-targets --all-features -- -D warnings`: 0 warnings
- `cargo fmt --all --check`: 0 diferenças
- Schema YAML: `python3 -c "import yaml; yaml.safe_load(...)"` válido para `ci.yml` (20 jobs), `release.yml` (4 jobs), `action.yml`
- Schema TOML: `python3 tomllib.load(Cross.toml, Cargo.toml)` válido

## [1.0.73] - 2026-06-05

### Corrigido

- **`linker 'clang' not found` em `Build aarch64-unknown-linux-gnu` (cross + Docker)**: a action `cross` cria um contêiner isolado a partir de `ghcr.io/cross-rs/aarch64-unknown-linux-gnu` e executa `cargo build` dentro dele. A imagem base do contêiner NÃO vem com `clang` nem `mold`. A composite action `install-mold-linker` no host instala esses binários apenas no runner do GitHub Actions, não dentro do contêiner cross. O bloco `pre-build` no `Cross.toml` instalava apenas `libssl-dev` + `pkg-config`, deixando o rustc incapaz de localizar `clang` para os build scripts de `proc-macro2`, `quote` e `libc`. Exit code 101. Adicionados `clang`, `mold` e `lld` ao `apt install` do `pre-build` para `[target.aarch64-unknown-linux-gnu]`, além de symlinks `ln -sf` em `/usr/local/bin` para que o contêiner cross os localize via `$PATH` independentemente da tag da imagem base.

- **Avisos de depreciação do Node.js 20 em 4 callsites de `actions/upload-artifact@v5`**: a variável `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"` forçava a action v5 (que declara Node 20 no manifesto) a rodar em Node 24, produzindo 4 avisos idênticos de depreciação (`actions/upload-artifact@v5. For more information see: https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/`). Promovidos todos os 3 callsites para `actions/upload-artifact@v6` (1 em `release.yml`, 2 em `ci.yml`). A v6 declara Node 24 como runtime padrão e elimina o aviso. Os nomes de artefato (`coverage-lcov`, `bench-baseline`, `sqlite-graphrag-${{ matrix.target }}`) são únicos em todo o workflow, portanto a breaking change da v6 (proibição de múltiplos uploads com mesmo nome em um run) não se aplica.

- **Avisos de tap não confiável do Homebrew em `Build aarch64-apple-darwin`**: o passo macOS em `install-mold-linker/action.yml` executava `brew update` em um ambiente com `aws/tap`, `azure/bicep` e `hashicorp/tap` registrados, porém sem confiança explícita. O Homebrew 5.2.0/6.0.0 tornará `HOMEBREW_REQUIRE_TAP_TRUST=1` o padrão, e o texto do aviso estava ficando ruidoso (`brew install mold` dispara avisos estilo `brew doctor` para os taps não confiáveis, mesmo que nenhum deles seja usado). Definido `HOMEBREW_NO_REQUIRE_TAP_TRUST=1` no bloco `env` do passo macOS. Nenhum dos taps removidos é necessário para `brew install mold`.

### Informativo

- **Redirecionamento `windows-2025` para `windows-2025-vs2026` em 15 de junho de 2026**: aviso de uma linha do runner `windows-2025` durante `Build x86_64-pc-windows-msvc` anunciando o redirecionamento automático iminente. O build em si é bem-sucedido; o aviso fica registrado para planejamento futuro. Nenhuma alteração de código é necessária para a v1.0.73; uma release posterior trocará a label do runner após a data de corte.

### Validação

- Schema YAML: `python3 -c "import yaml; yaml.safe_load(...)"` válido para `ci.yml` (20 jobs), `release.yml` (4 jobs), `action.yml`
- Schema TOML: `python3 tomllib.load(Cross.toml)` válido; array `pre-build` possui 6 entradas
- Migração `actions/upload-artifact@v6`: 3/3 callsites atualizados, sem colisões de `name:` em todo o workflow
- `Cross.toml` pre-build: 3 novos pacotes apt (`clang`, `mold`, `lld`) + 3 symlinks; imagem do contêiner será recacheada pelo cross-rs no primeiro run
- Passo macOS da composite action: bloco `env` estendido com `HOMEBREW_NO_REQUIRE_TAP_TRUST: "1"`


O formato é baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/),
e este projeto adere ao [Semantic Versioning](https://semver.org/lang/pt-BR/spec/v2.0.0.html).

## [1.0.72] - 2026-06-05

### Corrigido

- **Linker mold ausente nos runners `ubuntu-latest`**: o arquivo `.cargo/config.toml` (adicionado em v1.0.69) força `linker = "clang"` e `rustflags = ["-C", "link-arg=-fuse-ld=mold"]` para o target `x86_64-unknown-linux-gnu`. Na máquina local de desenvolvimento Fedora o mold é instalado via DNF, e na máquina macOS de desenvolvimento o bloco `x86_64-unknown-linux-gnu` é silenciosamente ignorado (o target é `aarch64-apple-darwin`), de modo que `cargo check`/`cargo test`/`cargo clippy` locais passam sem o binário do linker presente. No runner `ubuntu-latest` do GitHub Actions, contudo, o mold NÃO é instalado por padrão, e o rustc propagou `-fuse-ld=mold` para o clang que então emitiu `error: invalid linker name in argument '-fuse-ld=mold'` e saiu com 1. A compilação do build script (proc-macro2, quote, libc, todos os binários `build_script_build`) falhou primeiro, propagando em cascata para 12+ jobs com falha: `Tests (ubuntu/macos/windows)`, `Clippy (ubuntu/windows)`, `Coverage`, `Coverage threshold`, `Documentation`, `MSRV (1.88)`, `Slow Contract Suites`, `Windows MSVC cross-compile (G29)`, `cargo-careful sanity` e `Benchmark Regression`. A etapa `Annotations` então agregou 15 erros + 1 aviso + 3 notices.

- **Resolução: composite action instala o linker mold em todo job que compila**: adicionado `.github/actions/install-mold-linker/action.yml` (35 linhas) que detecta o SO do runner e instala `mold`+`clang`+`lld` via `apt-get` no Linux e via `brew` no macOS; no Windows o step é no-op porque o caminho do linker MSVC não honra `-fuse-ld=mold`. A composite action foi conectada em 15 jobs em `ci.yml` (14 callsites de `Swatinem/rust-cache` + o job `coverage-threshold` que não usa `rust-cache`) e 3 jobs em `release.yml` (`validate`, `build-matrix`, `publish-crates-io`). Documentada a dependência do mold em `.cargo/config.toml` com um bloco de comentário de 6 linhas.

### Validação

- 745 testes lib passam, 0 falham, 3 ignorados (inalterado desde v1.0.71)
- `cargo check --all-targets`: 0 erros (local, 4.88s)
- `cargo clippy --all-targets --all-features -- -D warnings`: 0 warnings
- `cargo nextest run --profile ci --all-features`: 800+ testes passam (a suíte completa exige 10+ min no macOS; CI ubuntu-latest tem orçamento de 5+ min)
- `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --all-features`: 0 warnings
- `cargo audit --ignore RUSTSEC-2025-0119 --ignore RUSTSEC-2024-0436 --deny warnings`: 0 vulnerabilidades
- `cargo deny check advisories licenses bans sources`: tudo ok (2 avisos `advisory-not-detected` são intencionais para as 2 crates upstream unmaintained)
- `cargo publish --dry-run --allow-dirty`: pacote compila + upload sucede, dry-run aborta antes do registry
- `cargo package --list --allow-dirty`: 268 arquivos, sem `.env`/`.pem`/`.key`/`credentials`/`docs_rules`/`.claude`/`.serena`/`CLAUDE.md`/`AGENTS.md`
- `tokei . -e target -e docs`: 133 arquivos Rust, 56126 linhas totais, 47906 código, 2791 comentários, 5429 em branco
- Schema YAML: `python3 -c "import yaml; yaml.safe_load(...)"` válido para `ci.yml` (20 jobs), `release.yml` (4 jobs), `action.yml`
- Schema TOML: `python3 tomllib.load(.cargo/config.toml)` válido, bloco target inalterado
- **Gate de cobertura (10/10) diferido**: `cargo llvm-cov --all-features` exige >25 min na máquina macOS de desenvolvimento; o operador autorizou pular conforme `feedback-never-publish-without-explicit-request` porque `git diff --stat src/` está vazio (nenhuma mudança relevante para cobertura desde v1.0.71 que passou o gate de 75% no CI). O job `coverage-threshold` do CI revalidará o threshold no commit publicado.

## [1.0.71] - 2026-06-05

### Corrigido

- **Pin do rust-cache em GitHub Actions resolvido**: `Swatinem/rust-cache@v2.8` pinado em 17 call-sites nos arquivos `ci.yml` e `release.yml` era uma ref Git inexistente (apenas `v2.0.0`-`v2.9.1` existem no repositório upstream). Repinamos todos os 17 call-sites para `Swatinem/rust-cache@v2.9.1` (latest estável, lançado em 2026-03-12, "Fix regression in hash calculation"). Resolveu os 22 erros `Unable to resolve action 'Swatinem/rust-cache@v2.8', unable to find version 'v2.8'` que bloqueavam todos os jobs.

- **Resíduo de política de idioma em doc comments**: 2 doc comments referenciavam "Correção A" (português) em `src/commands/claude_runner.rs:231` e `src/commands/codex_spawn.rs:209`. Traduzido para "Fix A" (inglês idiomático) para que o job `language-check` (que escaneia por `[áéíóúâêôãõç]` fora de `i18n.rs`) saia com 0.

- **taiki-e/install-action sem bloco `with:`**: `ci.yml:409` invocava `taiki-e/install-action@v2` sem especificar `tool`, produzindo `install-action: no tool specified; this could be caused by a dependabot bug where @<tool_name> tags on this action are replaced by @<version> tags` e exit 101 no job `coverage-threshold`. Adicionado o bloco `with: { tool: cargo-llvm-cov }` requerido.

- **Timeout do cargo-careful estendido**: `ci.yml:379` tinha `timeout 600 cargo +nightly careful test -- --test-threads=2` que estourava o tempo (exit 124) em execuções completas do `cargo-careful` com 745 testes sob nightly. Dobramos o orçamento para `timeout 1200` (20 min) para que o job de sanidade complete no runner `ubuntu-latest` de 2 cores mesmo com o ciclo mais longo de compile-then-test do nightly.

- **Aviso de redirect do windows-latest**: O GitHub Blog de 2026-05-14 anunciou que `windows-latest` e `windows-2025` serão migrados para `windows-2025-vs2026` (Visual Studio 2026) durante a semana de 2026-06-08 a 2026-06-15. Substituímos as 3 referências a `windows-latest` (matriz clippy em ci.yml x2, build-matrix em release.yml para `x86_64-pc-windows-msvc`) por `windows-2025` explícito para descartar o redirect do VS2026 por ora e evitar os 2 NOTICEs que o operador sinalizou na run de release da v1.0.70.

### Validação

- 745 testes lib passam, 0 falham, 3 ignorados (inalterado)
- `cargo check --all-targets`: 0 erros (4.88s local)
- `cargo clippy --all-targets --all-features -- -D warnings`: 0 warnings
- `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --all-features`: 0 warnings
- `cargo audit`: 0 vulnerabilidades (2 permitidas: RUSTSEC-2024-0436 paste unmaintained, RUSTSEC-2025-0119 tokenizers unmaintained)
- `cargo deny check advisories licenses bans sources`: tudo ok
- `cargo publish --dry-run --allow-dirty`: 268 arquivos, 0 sensíveis
- `cargo package --list --allow-dirty`: sem `.env`/`.pem`/`.key`/`credentials`/`docs_rules`/`.claude`/`.serena`/`CLAUDE.md`/`AGENTS.md`
- Schema YAML: 20 jobs ci.yml + 4 jobs release.yml, 17 call-sites rust-cache validados, 0 actions não resolvidas
- Política de idioma: 0 caracteres portugueses em doc comments `///` ou `//!` fora de `i18n.rs`

## [1.0.70] - 2026-06-05

### Corrigido

- **Precedência POSIX de locale no i18n**: `Language::from_env_or_locale()` em `src/i18n.rs:34` agora implementa precedência POSIX manual `LC_ALL > LC_MESSAGES > LANG` via `std::env::var()` em vez de chamar `sys_locale::get_locale()` diretamente. A implementação anterior ignorava variáveis de ambiente setadas em runtime porque `CFLocaleCopyCurrent()` (macOS) e `GetUserDefaultLocaleName` (Windows) cacheiam o locale do sistema. Três testes de i18n agora passam: `fallback_english_when_env_absent`, `posix_precedence_lc_all_overrides_lang`, `posix_precedence_lc_all_unrecognized_stops_iteration`.

- **Migração Node 24 em GitHub Actions**: Todas as ações JavaScript em `.github/workflows/ci.yml` e `.github/workflows/release.yml` atualizadas antes da migração default para Node 24 em 2026-06-16 e remoção do Node 20 em 2026-09-16. `actions/checkout@v4` → `@v5`, `actions/cache@v4` → `@v5`, `actions/upload-artifact@v4` → `@v5`, `actions/download-artifact@v4` → `@v5`, `taiki-e/install-action` → `@v2`, `Swatinem/rust-cache` pinado em `@v2.8` (sem v3 GA). Adicionado `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"` no env global de ambos os workflows como cinto-e-suspensórios.

- **Chave de job duplicada em ci.yml**: Renomeado o segundo job `coverage:` em `ci.yml:396` para `coverage-threshold:`. O validador estrito de schema do GitHub Actions rejeitava o workflow com `'coverage' is already defined` na linha 396 coluna 3, bloqueando todos os 21 jobs de rodarem.

- **Aviso de dead_code em claude_runner.rs**: Adicionado `#[cfg(target_os = "linux")]` à constante `DEFAULT_SUBPROCESS_MEMORY_LIMIT_MB` (valor 4096) em `src/commands/claude_runner.rs:51`. A constante era referenciada apenas pela função Linux-only `spawn_with_memory_limit` e gerava avisos de `dead_code` em builds de macOS e Windows. Resolvido sem usar `#[allow(dead_code)]` (proibido pelas `docs_rules`).

### Validação

- 745 testes lib passam (eram 742 pass + 3 fail), 0 falharam, 3 ignorados
- `cargo clippy --all-targets --all-features -- -D warnings`: 0 warnings
- `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --all-features`: 0 warnings
- `cargo audit`: 0 vulnerabilidades (2 permitidas: RUSTSEC-2024-0436 paste unmaintained, RUSTSEC-2025-0119 tokenizers unmaintained)
- `cargo deny check advisories licenses bans sources`: tudo ok
- `cargo publish --dry-run --allow-dirty`: 268 arquivos, 0 sensíveis
- `cargo package --list --allow-dirty`: sem `.env`/`.pem`/`.key`/`credentials`/`docs_rules`/`.claude`/`.serena`/`CLAUDE.md`/`AGENTS.md`

## [1.0.69] - 2026-06-05

### Corrigido

- **G28 (CRÍTICA)** Proliferação de processos ao iniciar a CLI. Três mudanças reforçadas eliminam a causa raiz: (a) `claude_runner::build_claude_command` AGORA SEMPRE passa `--strict-mcp-config --mcp-config '{}' --settings '{"hooks":{}}' --dangerously-skip-permissions`, garantindo que o subprocesso Claude nunca herde servidores MCP do escopo do usuário; a variável de ambiente `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` continua disponível para isolamento total. (b) `run_claude` envia `SIGTERM` no timeout antes do `Child` ser descartado, para que processos filhos MCP não sobrevivam ao pai. (c) Novo `src/reaper.rs` varre `/proc` no startup, mata qualquer órfão `claude`/`codex` com `PPID=1` e idade maior que 60 segundos, e o reaper é invocado do `main` ANTES de qualquer trabalho. A suíte de 4 testes do reaper (`orphan_min_age_is_one_minute`, `orphan_targets_include_claude_and_codex`, `reaper_report_starts_zeroed`, `scan_completes_without_panic_on_linux`) executa em menos de 30 segundos no host de teste.
- **G29** `enrich --operation body-enrich` abortava 100% das invocações com `CHECK constraint failed: source IN ('agent','user','system','import','sync')`. O bug era o literal `source: "enrich".to_string()` em `src/commands/enrich.rs:902`, que violava a constraint CHECK do SQLite. Substituído por `source: "agent".to_string()` mais metadados estruturados `{operation, orig_chars, new_chars}` (hotfix do G29).
- **G29 (trilha de auditoria)** `persist_enriched_body` estava contornando o histórico imutável de versões. Cada body-enrich agora insere uma nova linha em `memory_versions` com `change_reason='edit'` ANTES da atualização, de modo que `history --name <X>` lista tanto o corpo original quanto o enriquecido, e `restore --version N` pode reverter ao estado pré-enrich.
- **G31** `enrich --mode codex` estava sem cinco flags críticas de endurecimento em comparação com `ingest --mode codex` (`--ephemeral --skip-git-repo-check --sandbox read-only --ignore-user-config --ignore-rules`). Extraído o pipeline de spawn para `src/commands/codex_spawn.rs` para que AMBOS os call-sites consumam o mesmo comando canônico.
- **G32** `enrich --mode codex` estava chamando `serde_json::from_str` no stdout bruto, mas `codex exec --json` emite JSONL. O novo helper `parse_codex_jsonl` itera linha a linha, escolhe o último `item.completed` do tipo `agent_message` e extrai o uso do último evento `turn.completed` populado. Fonte única de verdade, compartilhada por `enrich` e `ingest --mode codex`.
- **G33** `enrich --mode codex --codex-model <nome>` era rejeitado silenciosamente APÓS consumir um turno OAuth. O novo helper `validate_codex_model` verifica `--codex-model` contra a lista branca do ChatGPT Pro OAuth (`codex-auto-review`, `gpt-5.3-codex-spark`, `gpt-5.4`, `gpt-5.4-mini`, `gpt-5.5`) ANTES de o subprocesso ser iniciado.
- **G34** O aviso `llm_parallelism > 4` era emitido em `mode=codex` (que não gera filhos MCP) com a mesma severidade de `mode=claude-code`. O aviso agora é condicional ao modo: Claude avisa em 5, Codex avisa em 17, Codex 5..16 fica silencioso (validado em 1161 itens, 0 falhas em produção).
- **G36** `optimize` reconstruía o índice FTS5 incondicionalmente, mesmo quando `fts check` reportava que o índice já estava saudável. O comportamento padrão agora é pular a reconstrução quando o índice passa na verificação de integridade. Operadores ainda podem forçar a reconstrução com `--no-fts-skip-when-functional`. A resposta agora expõe `fts_rebuilt`, `fts_skipped_functional`, `fts_unhealthy` para observabilidade.
- **G38** `backup` usava por padrão `run_to_completion(100, Duration::from_millis(50), None)`, o que em um banco de 4.3 GB levava cerca de 9 minutos só de sleep. Os novos padrões são `run_to_completion(1000, Duration::from_millis(5), None)` (≈25x mais rápido) e a resposta agora reporta `pages_copied` e `step_size`. Operadores podem ajustar com `--backup-step-size`, `--backup-step-sleep-ms` e `--backup-no-sleep`.
- **G39** `vec_memories_orphaned` era reportado por `health` sem caminho de remediação. Os novos comandos `vec orphan-list`, `vec purge-orphan --yes` e `vec stats --json` fecham o ciclo. `vec purge-orphan` exige `--yes` para evitar perda acidental; `--dry-run` é suportado.

### Adicionado

- **G30** O lock singleton agora tem escopo por `(job_type, namespace, db_hash)`. Duas invocações concorrentes de `enrich` em bancos DIFERENTES não colidem mais; o mesmo banco continua serializando. O `db_hash` são os primeiros 12 caracteres hex de `blake3(canonicalize(db_path))`.
- **G30+G09** Novas flags CLI `--wait-job-singleton <SEGUNDOS>` (sondagem pelo lock) e `--force-job-singleton` (quebra um lock obsoleto de uma invocação que travou) em `enrich` e `ingest`. A mensagem de erro que antes referenciava uma flag inexistente `--wait-job-singleton` agora é acionável.
- **G35** Novas flags `--preflight-check`, `--fallback-mode <codex|claude-code>` e `--rate-limit-buffer <SEGUNDOS>` em `enrich`. A sondagem de preflight emite um ping de 1 turno antes de varrer N candidatos; em rate limit do Claude, aborta com erro claro (ou troca para `--fallback-mode`). Padrão desligado para manter `--dry-run` e fluxos de CI com custo zero.
- **G37** Novas flags `--names <NOME>` e `--names-file <CAMINHO>` em `enrich` para selecionar um subconjunto específico de nomes de memória. `--names-file` aceita comentários `#` e linhas em branco. Combinado com `--names` como união quando ambos estão setados.
- **G14 (refatoração)** Extraído o módulo `codex_spawn`: pipeline de spawn, parser JSONL e validação de modelo ChatGPT Pro OAuth vivem em um só lugar (`src/commands/codex_spawn.rs`) com 8 testes unitários cobrindo casos de borda do parser, detecção de rate limit e presença de flags do comando.
- **G14 (refatoração)** Extraída a família de subcomandos `vec`: `vec orphan-list`, `vec purge-orphan --yes --dry-run`, `vec stats --json`.
- `src/memory_source.rs` — enum type-safe dos cinco valores CHECK-constraint de `memories.source`. `TryFrom<&str>` retorna `AppError::Validation` listando os valores aceitos. 8 testes unitários cobrem caminhos válido/inválido/vazio/display/serialização. Os call-sites existentes ainda usam `String` por compatibilidade; o enum é a fundação para a migração da v1.0.70.
- **OAuth-only enforcement (mudança COMPORTAMENTAL crítica)**. O spawn de `claude -p` e `codex exec` AGORA ABORTA com `AppError::Validation` se `ANTHROPIC_API_KEY` ou `OPENAI_API_KEY` estiverem definidos no ambiente. A flag `--bare` foi REMOVIDA de todos os caminhos executáveis (era PROIBIDA por gaps.md:49). Variáveis sensíveis foram EXCLUÍDAS dos whitelists de `env_clear()`. 4 testes `#[serial_test::serial(env)]` validam presença de todas as flags canônicas e o aborto. Detalhes em `docs/decisions/adr-0011-oauth-only-enforcement.md`.

### Alterado

- Assinatura de `lock::acquire_job_singleton` ganha os parâmetros `db_path: &Path` e `force: bool`. O nome do arquivo de lock agora é `job-singleton-{tag}-{namespace_slug}-{db_hash}.lock`, de modo que o cache do SO pode ser compartilhado entre bancos.
- `backup::BackupResponse` adiciona os campos `pages_copied` e `step_size`. Compatível com versões anteriores: consumidores existentes que ignoram campos desconhecidos continuam funcionando.
- `optimize::OptimizeResponse` adiciona os campos `fts_skipped_functional` e `fts_unhealthy`.
- `lock::db_path_hash` é `pub`, para que chamadores possam computar o hash sem adquirir o lock.
- O ambiente de spawn do `claude_runner` agora inclui as mesmas variáveis whitelisted do spawn do codex (consistência de caminho para usuários com configurações personalizadas restritas).
- **G36 (novas flags)** `--fts-dry-run`, `--fts-progress <N>` e `--yes` adicionadas a `optimize`. `--fts-dry-run` sai com código 1 quando o índice FTS5 precisa de reconstrução. `--fts-progress` emite polling de linhas a cada N segundos (padrão 30, 0 desabilita). `--yes` está reservada para automação futura.
- **G29 (idempotência blake3)** `call_body_enrich` calcula `blake3::hash` do corpo original e do enriquecido. Se os hashes forem iguais, retorna `EnrichItemResult::Skipped` com motivo `"enriched body hash matches original (blake3:{hash}); idempotency skip"`. Reprocessamento seguro.
- **G29 (preservação Jaccard)** Nova flag `--preserve-threshold <FLOAT>` (padrão 0.7). Módulo `src/preservation.rs` com 10 testes calcula similaridade Jaccard trigrama UTF-8 entre corpo original e enriquecido. Se similaridade menor que o threshold, marca `status='preservation_failed'` e NÃO persiste.

## [1.0.68] - 2026-06-03

### Corrigido
- `cargo install sqlite-graphrag` quebrava no Windows com `error[E0308]: mismatched types` em `src/terminal.rs:29` porque `HANDLE` em `windows-sys >= 0.59` é `*mut c_void` (era `isize` em 0.48/0.52).  Substituímos `handle != 0 && handle as isize != -1` pelo idiom type-safe `!handle.is_null() && handle != INVALID_HANDLE_VALUE`.  Também fixamos `windows-sys` em `=0.59.0` exato e adicionamos o job de CI `windows-build-check` que roda `cargo check --target x86_64-pc-windows-msvc` em todo push (G29).
- `enrich` e `ingest --mode claude-code|codex` podiam ser invocados em paralelo no mesmo namespace e saturar a máquina (causa raiz do incidente de load average 276 em 2026-06-03).  Adicionamos `lock::acquire_job_singleton` por `(job_type, namespace)` e a nova variante `AppError::JobSingletonLocked { job_type, namespace }` com exit 75.  Uma segunda invocação concorrente agora falha rápido em vez de empilhar 4 × N workers × 10 processos MCP (G28-B).
- `claude_runner::build_claude_command` agora respeita `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` — quando definido para um diretório existente e vazio, o subprocesso é iniciado com `CLAUDE_CONFIG_DIR=<esse dir>`, suprimindo servidores MCP do escopo user e a fan-out de 8-10 processos que eles causam.  Deliberadamente NÃO passamos `--strict-mcp-config` / `--mcp-config '{}'` porque [anthropics/claude-code#10787] documenta que o Claude Code CLI ignora ambas as flags.  `CLAUDE_CONFIG_DIR` é o único mecanismo que o upstream honra (G28-A).
- O módulo `retry` ganha um helper `CircuitBreaker` (com `AttemptOutcome::{Success,Transient,HardFailure}` e testes) que `enrich --retry-failed` pode usar para abortar loops de falha persistente.  Erros transient / rate-limited NÃO contam para o threshold, então um provider que se recupera não é penalizado (G28-D).
- 3 falhas de teste pré-existentes em `src/commands/{history,list,read}.rs` que vazavam a env var `SQLITE_GRAPHRAG_DISPLAY_TZ` entre threads de teste paralelos e afirmavam strings hardcoded `1970-01-01T00:00:00` agora parseiam a saída ISO via `chrono::DateTime::parse_from_rfc3339` e comparam `timestamp()` contra `DateTime::UNIX_EPOCH` para asserções timezone-agnostic.  A suíte de testes completa agora fica verde em todo fuso horário (`UTC`, `America/Sao_Paulo`, `Europe/Berlin`, etc.) sem necessidade de setup por teste da env var.

### Adicionado
- `retry::CircuitBreaker` (struct + `record` / `is_open` / `reset`) — helper opt-in para loops de retry limitados.  Erros rate-limited e timeout são explicitamente excluídos da contagem.
- `lock::acquire_job_singleton(job_type, namespace, wait_seconds)` — singleton de processo para comandos pesados.
- `constants::JOB_SINGLETON_POLL_INTERVAL_MS = 1000` — intervalo de polling do singleton.
- `errors::AppError::JobSingletonLocked { job_type, namespace }` — exit 75, classificado como retryable e com mensagem PT-BR localizada.
- Job de CI `windows-build-check` que roda `cargo check --target x86_64-pc-windows-msvc --lib --all-features` para capturar regressões Windows antes do publish.
- `tests/terminal_compile_windows.rs` — teste de regressão para `terminal::init_console` e `should_use_ansi`; no Windows também referencia a checagem type-safe de HANDLE.
- `lock::tests` — 3 testes unitários cobrindo sanitização de namespace, bloqueio da segunda invocação e isolamento por namespace.

### Alterado
- `enrich` emite `tracing::warn!` (visível com `-v`) quando `llm_parallelism > 4`, recomendando combinar com `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` para manter a fan-out de subprocessos administrável (G28-D, não-breaking).
- `Cargo.toml`: `windows-sys` fixado em `=0.59.0` exato (era range `0.59`).

## [1.0.67] - 2026-06-01

### Adicionado
- Comando `remember-batch` — criação em lote de memórias via NDJSON no stdin com `--transaction` para atomicidade, `--force-merge` para atualizações idempotentes, `--fail-fast` para abortar no primeiro erro (G08)
- Comando `completions` — gera completions de shell para Bash, Zsh, Fish, PowerShell e Elvish
- Flag `read --id <N>` para busca direta por `memory_id` inteiro, sem resolução de nome (G17)
- Flag `read --with-graph` para incluir entidades e relacionamentos vinculados na resposta JSON (G22)
- Flag `enrich --llm-parallelism <N>` para threads paralelas de LLM (padrão 1, máximo 32) — reduz tempo de enrich proporcionalmente (G19)
- `health` detecta entidades super-hub (grau > 50) e reporta `super_hub_count`, `super_hub_warning`, `top_hub_entity`, `top_hub_degree` no JSON (G25)
- `health` reporta `non_normalized_count` e `normalization_warning` para entidades fora do padrão kebab-case (G24)
- Aliases em `related`: `--from`/`--to` para `--source`/`--target`, `related_memories` como alias de campo (G23)
- Módulo compartilhado `claude_runner.rs` — lógica DRY de spawn do subprocesso `claude -p` para `enrich` e `ingest-claude` (G02)
- `claude_runner.rs` detecta `terminal_reason: "max_turns"` e retorna erro específico em vez de falha genérica (G03)
- `enrich` passa `max_turns=7` ao subprocesso Claude, absorvendo turns consumidos por hooks (G01)

### Corrigido
- `edit` compara `body_hash` (blake3) antes de re-embedar — edições idempotentes pulam o passo de embedding de ~1.5s (G15)
- `rename` purga memórias ghost (soft-deleted) que ocupam o nome destino antes do UPDATE — elimina crash UNIQUE constraint (exit 10) que antes exigia `purge --retention-days 0` como workaround (G16)
- `hybrid-search` rejeita `--max-hops` e `--min-weight` sem `--with-graph` com erro acionável em vez de descarte silencioso (G20 parcial)
- `recall` rejeita `--max-hops` e `--min-weight` com `--no-graph` com erro acionável em vez de descarte silencioso (G20 parcial)
- `ingest` rejeita flags NER contraditórias e `--low-memory` com `--ingest-parallelism > 1` com erro de validação (G21 parcial)
- `normalize-entities --dry-run` calcula `merge_count_preview` real em vez de sempre 0 (G10)
- Normalização de nomes de entidade mapeia TODOS caracteres não-alfanuméricos para hífens (G11)
- Deserialização de relacionamentos aceita `type` como alias de `relation` via `#[serde(alias)]` (G12)
- `recall`, `hybrid-search`, `deep-research` aceitam `--limit` e `--top-k` como aliases de `--k` (G13)
- `enrich` query `linked_entities` fornece contexto de grafo por entidade para prompts LLM (G26)
- `enrich` suporta todas 13 operações incluindo `relation-cleanup`, `duplicate-detection`, `type-audit`, `hub-analysis` (G27)
- Migração V012 adiciona `created_at`/`updated_at` na tabela relationships com trigger de backfill (G09)
- `memory_guard` remove margem /2 no threshold de memória; teto de lock usa 2*nCPUs dinâmico (G18)

## [1.0.66] - 2026-05-29

### Corrigido
- BUG-01 CRITICO: `reclassify-relation` crash — removido `updated_at = unixepoch()` de 3 SQL UPDATE referenciando coluna inexistente
- BUG-02 ALTO: `link --create-missing` agora normaliza nomes de entidades para kebab-case no storage e no JSON response
- BUG-04 MEDIO: `deep-research` decompoe queries de 3+ palavras sem conjuncoes via word-pair heuristic
- BUG-05 BAIXO: `remember --body-file` tratamento defensivo UTF-8 — bytes invalidos substituidos por U+FFFD
- BUG-06 ALTO: `link` agora atualiza peso de relacoes existentes e reporta peso real do DB no JSON response
- HIGH-01 CRITICO: `deep-research` evidence chains corrigidas — seeds BFS limitados a top-5 memorias por score
- HIGH-01b: `deep-research --graph-min-score` default reduzido de 0.2 para 0.05
- HIGH-04: `link --max-entity-degree` warning agora visivel sem flag -v
- HIGH-08: `deep-research` source classification reporta `hybrid` quando KNN e FTS encontraram a mesma memoria
- HIGH-12: `remember` e `ingest` agora usam `max_relationships_per_memory()` (le env var override) em vez de constante hardcoded

### Adicionado
- `edit --type` para mudar tipo de memoria sem recriar (HIGH-10)
- `deep-research --mode` campo reservado (none; claude-code/codex planejado para v1.1.0) (HIGH-06)
- `deep-research --max-cost-usd` campo reservado para controle de custo LLM (HIGH-09)
- `deep-research` campo `graph_context` no JSON com entidades e relacoes das memorias encontradas (MEDIUM-01b)
- `deep-research` 7 chamadas `tracing::debug!` em `execute_sub_query()` para diagnostico com `-vv` (HIGH-07)
- `graph --format json` inclui campo `entities` como alias de `nodes` (HIGH-05)
- `list --json` inclui campo `memories` como alias de `items` (HIGH-05)
- `graph entities --json` inclui campo `description` por entidade (HIGH-11)
- `health --json` inclui `vec_memories_missing` e `vec_memories_orphaned` (MEDIUM-09)
- `history --diff` primeira versao reporta baseline `changes: {added_chars: N}` em vez de null (MEDIUM-02)
- Validacao de entity_type sugere mapeamento quando memory types sao usados: reference→concept, document→file, user→person (HIGH-10c)
- `debug-schema` renomeado de `__debug_schema` (HIGH-03)
- Diretorio `fuzz/` com targets cargo-fuzz (LOW-01)
- `mutants.toml` para cargo-mutants (LOW-02)
- Job de coverage no CI com threshold 75% (LOW-03)

### Alterado
- `deep-research --graph-min-score` default: 0.2 → 0.05

## [1.0.65] - 2026-05-28

### Adicionado
- Comando `reclassify-relation` — reclassificação em massa ou individual de tipos de relacionamento com merge de duplicatas via `UPDATE OR IGNORE` + `DELETE`, `--dry-run`, `--filter-source-type`/`--filter-target-type` (GAP-13)
- Comando `normalize-entities` — normaliza nomes de entidade existentes para kebab-case ASCII minúsculo e mescla automaticamente colisões de quase-duplicatas, com `--dry-run`/`--yes` (GAP-15)
- Comando `enrich` — qualidade do grafo aumentada por LLM via `--mode claude-code|codex`, pipeline scan→judge→persist, 12 operações (memory-bindings, entity-descriptions, body-enrich e mais), `--dry-run` faz preview sem spawnar LLM, queue DB com resume/retry (GAP-14, GAP-18)
- `health` agora reporta `top_relation`, `top_relation_ratio`, `applies_to_ratio` e `relation_concentration_warning` quando uma relação excede 40% das arestas (GAP-13)
- Flags `--rrf-k`, `--graph-decay`, `--graph-min-score` e `--max-neighbors-per-hop` no `deep-research`
- Warning `--max-entity-degree` em `link` e `remember` para sinalizar crescimento de super-hubs (GAP-17)
- Schemas JSON `deep-research`, `reclassify-relation`, `normalize-entities` e `enrich-{phase,item-event,summary}`, mais testes `contract_36..39` e `schema_36..39` — restaura 100% de cobertura de schema/contrato (GAP-01, GAP-02, GAP-03, GAP-04)

### Corrigido
- GAP-07 CRITICAL: `deep-research` agora computa embedding separado por sub-query — decomposição era cosmética porque todas as sub-queries compartilhavam o embedding da query original para KNN, retornando resultados idênticos (também resolve GAP-10 colapso de centróide e GAP-12 decomposição parcial)
- GAP-08 CRITICAL: `deep-research` agora funde pools KNN, FTS5 e grafo via Reciprocal Rank Fusion (novo módulo compartilhado `storage::fusion`) em vez de atribuir score fixo 0.5 aos resultados FTS
- GAP-11: scoring do pool de grafo no `deep-research` incorpora score do seed, decaimento por hop e peso da aresta, fundido via RRF com filtro de score mínimo
- GAP-09 HIGH: cadeias de evidência do `deep-research` agora são caminhos direcionados seed→target (`from`, `to`, `path`, `total_weight`) filtrados por entidades descobertas, em vez de dump flat das top-20 relações globais
- GAP-15 HIGH: nomes de entidade são normalizados para kebab-case minúsculo em todo path de escrita E leitura (`find_entity_id`, `rename-entity`, `reclassify-relation`, `prune-ner`, `enrich`) — validação roda no nome bruto primeiro para que ruído ALL_CAPS de NER curto ainda seja rejeitado, depois a forma normalizada é armazenada e consultada

### Alterado
- GAP-17: travessia do grafo aceita cap opcional de vizinhos por hop (top-K por peso); comportamento padrão inalterado
- Fusão RRF do hybrid-search extraída para módulo compartilhado `storage::fusion` (sem mudança de comportamento)
- GAP-16: docs esclarecem que relações são aceitas em kebab-case ou snake_case e sempre armazenadas e emitidas como snake_case

## [1.0.64] - 2026-05-28

### Corrigido
- BUG-1 HIGH: `ingest --mode claude-code` agora desabilita hooks via `--settings '{"hooks":{}}'` para usuários OAuth e detecta `terminal_reason: "max_turns"` — previne que hooks Stop consumam turns de extração (falhava em 65% dos arquivos para usuários com hooks configurados)
- BUG-2 HIGH: `ingest --mode claude-code` agora detecta OAuth via `apiKeySource` do JSON init do Claude Code e omite `cost_usd` enganoso do output NDJSON — limite `--max-cost-usd` é ignorado com warning para assinantes que não são cobrados por chamada de API
- BUG-3 HIGH: `ingest --mode claude-code` e `--mode codex` agora validam tamanho do body ANTES de enviar ao subprocesso LLM — arquivos excedendo limite de 512 KB são ignorados com warning acionável ao invés de desperdiçar tokens LLM em extração que será descartada
- `rename` e `rename-entity` agora rejeitam renomeações para o mesmo nome com exit 1 (Validation) — previne inflação de versão, sincronização FTS5 desnecessária e re-embedding desperdiçado

### Adicionado
- Comando `deep-research` para pesquisa profunda multi-hop paralela via decomposição heurística de queries (até 7 sub-queries), fan-out bounded com `tokio::task::JoinSet` e `Arc<Semaphore>`, travessia de grafo com 3 hops e montagem de cadeias de evidência — defaults calibrados contra benchmarks NovelHopQA, StepChain, HopRAG e GraphRAG-Bench (k=20, max-hops=3, max-sub-queries=7)

## [1.0.63] - 2026-05-27

### Corrigido
- BUG-1 ALTO: `restore` não reverte mais o nome da memória para o original da versão — preserva nome atual após rename, elimina crash UNIQUE constraint (exit 10) quando nome antigo está ocupado
- BUG-2 ALTO: `ingest --mode claude-code` e `--mode codex` agora normalizam strings de relação via `normalize_relation()` antes da verificação canônica e inserção no DB — elimina falsos avisos `non-canonical relation` para valores canônicos em kebab-case (`depends-on` → `depends_on`) e previne inconsistência de formato no DB
- FINDING-1: `edit` agora regenera embedding vetorial quando body muda — `recall` e `hybrid-search` retornam scores de similaridade precisos após edit (paridade com `restore` que já faz re-embed)

### Adicionado
- Seção AUTHENTICATION em `ingest --help` documentando princípio OAuth-first para `--mode claude-code` e `--mode codex`
- Detecção de falha de autenticação: `tracing::warn!` acionável quando autenticação do Claude Code ou Codex CLI falha durante ingest

## [1.0.62] - 2026-05-23

### Corrigido
- G01 CRÍTICO: `ingest --mode claude-code` agora computa e persiste embeddings vetoriais — `recall` e `hybrid-search` encontram memórias ingeridas via claude-code (antes criava memórias com zero vec_memories/vec_chunks)
- G02: `validate_claude_version()` agora compara contra `MIN_CLAUDE_VERSION` (2.1.0) — rejeita versões incompatíveis do Claude Code com erro acionável
- G03: whitelist de `env_clear()` para o subprocesso `claude -p` agora inclui variáveis críticas do Windows (`LOCALAPPDATA`, `APPDATA`, `USERPROFILE`, `SystemRoot`, `COMSPEC`, `PATHEXT`) via `#[cfg(windows)]`
- G04: contador `skipped` no resumo de ingest claude-code agora conta entradas `done` pré-existentes no queue DB em vez de sempre reportar 0
- G05: arquivos acima do limite de 10MB para stdin são rejeitados com erro específico antes de spawnar `claude -p`, evitando desperdício de créditos de API
- G06: nomes de memória extraídos pelo Claude são normalizados via `derive_kebab_name()` — impede nomes não-kebab-case de entrar no banco de dados
- G07: nomes de entidade inválidos extraídos pelo Claude agora emitem `tracing::warn!` em vez de serem descartados silenciosamente
- G08: banco de dados de fila claude-code (`.ingest-queue.sqlite`) agora usa modo WAL para resiliência a crashes
- G09: WAL checkpoint executado após a conclusão do loop de processamento do ingest claude-code
- G10: `EXTRACTION_SCHEMA` agora inclui `additionalProperties: false` no nível raiz, de entidade e de relacionamento — compatível com saída estruturada do Claude Code e do Codex

### Adicionado
- `ingest --mode codex` para extração curada por LLM de entidades/relações via OpenAI Codex CLI instalado localmente (`codex exec --json`)
- Novas flags de ingest: `--codex-binary`, `--codex-model`, `--codex-timeout` para configuração do Codex CLI
- Variante `IngestMode::Codex` — usuários podem escolher entre `--mode claude-code` (Anthropic) e `--mode codex` (OpenAI) por ingest
- Parser JSONL para saída do Codex CLI com padrão "last agent_message wins" (verificado contra o adaptador Paperclip de produção)
- Rastreamento de uso de tokens para ingest Codex (input_tokens, output_tokens) — cost_usd indisponível via Codex CLI
- Pipeline completo de embedding para memórias ingeridas via Codex (chunking, vec_memories, vec_chunks, vec_entities)
- 7 testes unitários para parser JSONL do Codex e validação de schema

## [1.0.61] - 2026-05-23

### Corrigido
- **B00 CRÍTICO**: `ingest --mode claude-code` agora usa `--dangerously-skip-permissions` em vez de `--bare` — corrige falha de autenticação OAuth para usuários Pro/Max
- **B00a**: `--max-turns` aumentado de 1 para 3 — Claude precisa de >1 turno para extração estruturada
- **B07a**: campo source da memória alterado de `"claude-code"` para `"agent"` — corrige violação de CHECK constraint no insert
- **B01**: flag `--resume` agora reseta arquivos travados em `processing` para `pending` para reprocessamento
- **B02**: flag `--retry-failed` agora reseta arquivos `failed` para `pending` para retry
- **B03**: `--dry-run` agora funciona com `--mode claude-code` — emite eventos de preview sem spawnar Claude
- **B04**: timeout de subprocesso via crate `wait-timeout` — mata `claude -p` após `--claude-timeout` segundos (padrão 300)
- **B05**: mensagens de erro do `claude -p` agora parseadas do stdout JSON em vez de stderr vazio
- **B06**: re-ingestão do mesmo diretório atualiza memórias existentes em vez de falhar com UNIQUE constraint
- **B07**: falha de cold-start `--json-schema` automaticamente retentada uma vez (workaround para Claude Code Issue #23265)
- **B08**: subprocesso `claude -p` agora roda com `env_clear()` + injeção seletiva de ambiente (hardening de segurança)
- **B10**: parsing fallback do campo `result` quando `structured_output` ausente (workaround para Claude Code Issue #18536)
- **B11**: campo `index` do FileEvent agora usa indexação 0-based consistente em caminhos de sucesso e falha
- **B12**: `entity_type` inválido do Claude agora emite `tracing::warn!` em vez de descarte silencioso
- **B13**: tipos de relacionamento não-canônicos agora validados via `warn_if_non_canonical()` antes da inserção

### Adicionado
- Flag `--claude-timeout` para `ingest --mode claude-code` (padrão: 300 segundos por arquivo)

### Alterado
- `ingest --mode claude-code` usa `--bare` quando `ANTHROPIC_API_KEY` está definido (startup mais rápido, sem plugins), `--dangerously-skip-permissions` para usuários OAuth

## [1.0.60] - 2026-05-23

### Adicionado
- `ingest --mode claude-code` para extração curada por LLM de entidades/relações via Claude Code CLI instalado localmente (`claude -p` headless com `--json-schema`)
- Novas flags do ingest: `--mode`, `--claude-binary`, `--claude-model`, `--resume`, `--retry-failed`, `--keep-queue`, `--queue-db`, `--rate-limit-wait`, `--max-cost-usd`
- Enum `IngestMode`: `none` (padrão body-only), `gliner` (NER), `claude-code` (curado por LLM)
- Queue DB (`.ingest-queue.sqlite`) para ingestão claude-code resumível com rastreamento por arquivo
- `memory-entities-reverse.schema.json` para validação da resposta de reverse lookup (`--entity`)
- Testes `contract_33b_memory_entities_reverse` e `schema_33b_memory_entities_reverse`
- Receitas `delete-entity` e `merge-entities` no COOKBOOK.md (EN/PT)
- Entradas `cleanup-orphans` e `prune-relations` no INTEGRATIONS.md (EN/PT)
- Documentação de modos de ingestão em llms.txt, llms-full.txt, llms.pt-BR.txt, AGENTS.md, SKILL.md (EN/PT)

### Corrigido
- D1: `test_exit_01_validation_invalid_name` — `"x"` alterado para `"___"` (nomes de 1 caractere são válidos)
- D2-D3: testes bilíngues i18n — `"---"` alterado para `"___"` (`"---"` é separador de flags Clap)
- D4: `test_ingest_fail_fast_aborts_on_first_error` — usa arquivos ilegíveis (chmod 000) em vez de path `/proc`; filtro de error envelope no NDJSON; `#[cfg(unix)]`
- D5: `prd_name_double_underscore_rejected` — `"---"` alterado para `"___"`
- D6: `init_creates_11_migrations_v001_to_v011` — vec literal corrigido de `[1..9]` para `[1..11]` correspondendo às 11 migrations reais
- D7: `readme_en_bash_examples_all_run` — `#[cfg_attr(windows, ignore)]` adicionado para testes bash-only

## [1.0.59] - 2026-05-22

### Corrigido
- `rename-entity` agora valida `--new-name` via `validate_entity_name()`, rejeitando nomes com menos de 2 caracteres, nomes com quebras de linha e abreviações ALL_CAPS curtas
- `unlink.schema.json` atualizado de `relationship_id` obsoleto para `relationships_removed` correspondendo ao struct `UnlinkResponse` real
- Teste `contract_16_unlink` atualizado para campos corretos da resposta (`relationships_removed` em vez de `relationship_id`, adicionado `elapsed_ms`)
- `health -vv` agora emite `tracing::info!` para o checkpoint do modelo de embedding, completando os 4 pontos de trace do health

### Adicionado
- Resposta do `reclassify` inclui campo opcional `description_updated: true` quando `--description` é aplicado no modo individual
- Testes `contract_35_rename_entity` e `schema_35_rename_entity` para cobertura completa de contrato e schema do comando rename-entity
- Testes E2E de integração para validação de nome de entidade via CLI (caminhos `link --create-missing` e `rename-entity`)
- `rename-entity` adicionado a `docs/schemas/README.md`, `INTEGRATIONS.md`, `llms.txt`, `llms-full.txt` e contrapartes PT-BR

## [1.0.58] - 2026-05-21

### Corrigido
- **C1 CRÍTICO**: `remember --force-merge` agora chama `sync_fts_after_update` — elimina corrupção silenciosa do índice FTS5 a cada force-merge
- **H1/H3 ALTO**: `merge-entities` usa `UPDATE OR IGNORE` para `memory_entities` — corrige falha de UNIQUE constraint quando entidades compartilham vínculos
- **M6**: resposta do `purge` agora inclui campo `action` (`"purged"` ou `"dry_run"`) para consistência com demais comandos

### Adicionado
- **H2**: Novo comando `rename-entity` — renomeia entidade preservando todos os relacionamentos e vínculos, re-gera vetor
- **M3**: `memory-entities --entity <nome>` busca reversa — lista todas as memórias vinculadas a uma entidade
- **L6**: Flag `reclassify --description` — atualiza descrição da entidade no modo individual
- **H4**: Validação de nomes de entidade — rejeita nomes com quebras de linha, menores que 2 caracteres, ou abreviações ALL_CAPS (ruído de NER)

### Melhorado
- **L1**: `fts --help` agora mostra seção EXAMPLES para subcomandos
- **L3**: Comando `health` emite `tracing::info!` nos checkpoints para debugging com `-vv`
- **L2**: `reclassify --help` agora lista todos os tipos de entidade válidos
- **M1**: Correção de documentação: campo JSON de `history --diff` é `changes` (não `diff`)

## [1.0.57] - 2026-05-21

### Corrigido
- `merge-entities` não falha mais com violação de UNIQUE constraint quando entidades de origem compartilham relacionamentos idênticos — usa `UPDATE OR IGNORE` + limpeza em vez de UPDATE direto (BUG-1).
- `memory-entities` agora usa coluna correta `e.type` em vez de `e.entity_type` inexistente (BUG-2).
- Flag `--clear-body` no `remember` não é mais bloqueada pela validação de body vazio — o guard agora reconhece intenção explícita de limpeza (BUG-3).
- `fts rebuild` e `fts check` agora chamam `PRAGMA wal_checkpoint(TRUNCATE)` após operações de escrita, consistente com todos os outros comandos de escrita (G1, G2).
- `delete-entity --cascade` agora recalcula degree para todas entidades adjacentes após remover relacionamentos, prevenindo valores de degree obsoletos (G3).
- `merge-entities` agora recalcula degree para a entidade alvo E todas entidades adjacentes, não apenas o alvo (G4).
- Caminho destrutivo do `prune-ner` agora executa COUNT e DELETE na mesma transação, eliminando condição de corrida sob acesso concorrente (G5).
- `backup` agora usa padrão atômico tempfile-rename via `NamedTempFile::persist` — backups interrompidos não corrompem mais o arquivo de destino existente (G6).
- `backup` agora registra erros de chmod via `tracing::warn!` em vez de descartá-los silenciosamente (G7).
- `reclassify --batch` agora emite `tracing::warn!` quando `--from-type` corresponde a zero entidades, ajudando a detectar erros de digitação em nomes de tipo (G8).
- `emit_error_json` agora escreve JSON de fallback manualmente se serialização falhar, garantindo que o contrato JSON do stdout nunca é violado (G11).
- `list --limit 0` agora retorna exit 1 com erro de validação em vez de retornar resultado vazio indistinguível de banco vazio (G12).
- `fts rebuild` agora verifica existência da tabela `fts_memories` antes de tentar reconstruir, retornando erro de validação claro em bancos novos (G16).

### Alterado
- Destino de `backup` agora é escrito atomicamente via tempfile-rename; crate `tempfile` promovida de dev-dependency para dependência runtime.
- 5 JSON schemas corrigidos: `merge-entities`, `delete-entity`, `reclassify`, `prune-ner` agora incluem campo `namespace`; `fts-stats` removeu campo fantasma `action`.
- 9 novos contract tests (contract_26–contract_34) e 9 novos schema validation tests (schema_26–schema_34) adicionados para todos os comandos v1.0.56.

## [1.0.56] - 2026-05-21

### Adicionado
- Comando `fts rebuild` reconstrói o índice FTS5 de busca textual do zero (GAP-07).
- Comando `fts check` executa integrity-check do FTS5 sem modificar o índice (GAP-07).
- Comando `fts stats` exibe estatísticas do índice FTS5: contagem de linhas, páginas shadow, status funcional (GAP-32).
- Comando `backup` cria cópia segura do banco via SQLite Online Backup API (GAP-20).
- Comando `delete-entity` remove entidade e cascateia para relacionamentos e bindings NER (GAP-17).
- Comando `reclassify` altera tipo de entidade individual ou em massa via `--from-type`/`--to-type --batch` (GAP-18).
- Comando `merge-entities` funde múltiplas entidades-fonte em um destino, movendo todas as edges (GAP-19).
- Comando `memory-entities` lista entidades vinculadas a uma memória específica (GAP-22).
- Comando `prune-ner` remove bindings NER da tabela `memory_entities` por entidade ou globalmente (GAP-16).
- Flag `--dry-run` em `remember` valida input e reporta ações planejadas sem persistir (GAP-26).
- Flag `--clear-body` em `remember` limpa explicitamente o body durante `--force-merge` (GAP-08/09).
- Flag `--strict-relations` em `link` rejeita tipos de relação não-canônicos com exit 1 (GAP-15).
- Flags `--sort-by degree|name|created_at` e `--order asc|desc` em `graph entities` (GAP-25).
- Flag `--skip-fts` em `optimize` para pular rebuild do FTS5 (GAP-06).
- Flag `--max-name-length` em `ingest` para configurar limite de truncagem de nomes (GAP-34).
- Campos `fts_degraded`, `fts_error` no JSON de `hybrid-search` para degradação graciosa do FTS5 (GAP-04).
- Campo `fts_auto_rebuilt` no JSON de `hybrid-search` quando FTS5 é auto-reparado em corrupção (GAP-05).
- Campo `normalized_score` no JSON de `hybrid-search` para comparabilidade de scores entre métodos (GAP-12).
- Campos `vec_distance`, `fts_bm25` de scores brutos no JSON de `hybrid-search` (GAP-30).
- Campo `fts_query_ok` no JSON de `health` verifica se FTS5 é funcionalmente consultável (GAP-02).
- Campo `sqlite_version` no JSON de `health` reporta versão do SQLite bundled (GAP-28).
- Campos `model_name`, `model_variant` na resposta de `daemon --ping` (GAP-29).
- Campo `degree` no JSON de `graph entities` via subquery COUNT (GAP-13).
- Campo `body_length` no JSON de `list` (GAP-14).
- Campo `body_length` nos eventos NDJSON por arquivo de `ingest` (GAP-27).
- Campos `total_count`, `truncated` na resposta JSON de `list` (GAP-11).
- Campo `warnings` na resposta JSON de `link` para avisos de relações não-canônicas (GAP-15).
- Flag `--diff` em `history` inclui resumo de mudanças por caractere entre versões (GAP-23).
- Envelope JSON de erro no stdout para todos os caminhos de erro: `{"error": true, "code": N, "message": "..."}` (GAP-03).

### Corrigido
- Sync FTS5 de external-content implementado nos handlers `edit`, `rename` e `restore` via `sync_fts_after_update()` — corrige corrupção silenciosa do índice FTS5 onde memórias editadas/renomeadas ficavam invisíveis à busca textual (GAP-01 causa raiz).
- `hybrid-search` não aborta mais quando FTS5 está corrompido — cai para resultados apenas vetoriais com `fts_degraded: true` (GAP-04).
- `hybrid-search` pula consulta FTS5 completamente quando `--weight-fts 0.0` em vez de executar e falhar (GAP-04).
- `hybrid-search` reconstrói automaticamente o índice FTS5 em erros "malformed" e retenta uma vez antes de degradar (GAP-05).
- `health --json` agora faz smoke test funcional com query FTS5 MATCH em vez de apenas verificar existência da tabela em `sqlite_master` (GAP-02).
- `optimize` agora reconstrói índice FTS5 após `PRAGMA optimize` (GAP-06).
- `--force-merge` com body vazio preserva body existente em vez de destruí-lo — use `--clear-body` para limpar explicitamente (GAP-08/09).
- `--type` e `--description` agora opcionais com `--force-merge` — herdados da memória existente quando omitidos (GAP-10).
- Limite padrão de `list --json` alterado de 50 para todas as memórias — output texto mantém padrão 50 (GAP-11).
- `unlink` `--relation` agora opcional — omitir remove todos os relacionamentos entre o par (GAP-24).
- `unlink` suporta `--entity X --all` para remoção em massa de todas edges de uma entidade (GAP-24).
- `ingest` auto-prefixa nomes começando com dígitos com `doc-` em vez de rejeitar (GAP-35).
- Pesos extremos (>= 0.95 ou <= 0.05) agora emitem `tracing::warn!` (GAP-36).
- Entity type "memory" emite `tracing::warn!` quando nome colide com memória existente (GAP-33).

## [1.0.55] - 2026-05-17

### Corrigido
- SKILL.md (EN+PT): campo do summary de export corrigido de `total` para `exported`, conforme o JSON real da struct `ExportSummary` (G1).
- SKILL.md (EN+PT): campos response-level de `list` corrigidos — removidos campos inexistentes `total`, `limit`, `offset`; resposta real contém apenas `items[]` e `elapsed_ms` (G2).
- SKILL.md (EN+PT) e CLAUDE.md: `--tz` com timezone inválido agora corretamente documentado como exit 2 (parsing de argumentos Clap) em vez de exit 1 (validação da aplicação). O `FromStr` do Clap para `chrono_tz::Tz` valida antes do código da aplicação (G3).
- SKILL.md (EN+PT): exit code 2 adicionado à tabela de exit codes com descrição cobrindo erros de parsing do Clap incluindo valores de timezone inválidos (G3+G4).
- SKILL.md (EN+PT): resposta de `stats` agora documenta campos alias legados `db_bytes`, `edges`, `memories_total`, `entities_total`, `relationships_total` (G6).
- AGENTS.md (EN+PT): timezone IANA inválido de `--tz` corrigido de exit 1 para exit 2; `timezone ruim` movido da descrição de exit 1 para exit 2; aliases legados de `stats` documentados.
- HOW_TO_USE.md (EN+PT): campo do summary de export corrigido de `memories_total` para `exported`.
- COOKBOOK.md (EN+PT): contagem de exit codes atualizada de 16 para 17; exit 2 adicionado à tabela de exit codes e ao exemplo bash case.
- SKILL.md, AGENTS.md, CLAUDE.md (EN+PT): default de `--min-weight` corrigido de 0.0 para 0.3, conforme `src/commands/hybrid_search.rs:60`.
- README.md (EN+PT): exit code 2 adicionado à tabela de exit codes — estava ausente entre exit 1 e exit 9.
- README.md (EN+PT), llms.txt (EN+PT): exit code 73 espúrio (`EX_NOPERM`) removido — não implementado no código-fonte; existem apenas 17 exit codes (0-77).

## [1.0.54] - 2026-05-17

### Corrigido
- WAL checkpoint TRUNCATE adicionado ao `prune-relations` — último comando de escrita sem checkpoint (H1).
- `remember --graph-stdin` com body vazio e sem entidades agora retorna corretamente exit 1 (Validation) em vez de criar silenciosamente uma memória inerte com zero chunks (H2).
- Saída JSON de `list` e `export` agora inclui campo `memory_type` junto com `type`, consistente com `read` (H3). Agentes que parseiam `.memory_type` não recebem mais null.

### Alterado
- `Vec::with_capacity()` aplicado em 9 cold paths adicionais: listagem de arquivos do ingest, graph matches do recall, resultados do related, graph matches do hybrid-search, hops do graph-export, entradas do cache, warnings do remember, extração de URLs, candidatos do embedder (M2).

## [1.0.53] - 2026-05-15

### Corrigido
- WAL checkpoint TRUNCATE após cada comando de escrita previne corrupção de B-tree quando o banco é sincronizado pelo Dropbox ou ferramentas de cloud sync similares (C2). Comandos afetados: remember, edit, forget, ingest, link, unlink, rename, restore, cleanup-orphans, purge.
- `export` agora aceita `--json` como flag oculta no-op, consistente com todos os outros subcomandos (H1).

### Alterado
- `Vec::with_capacity()` aplicado em 12 hot paths adicionais de produção: offsets de tokenizer, splitting de chunks, fronteiras BFS de grafo, alocação de tensores GLiNER, coleta de spans candidatos, buffers de extração do ingest, planejamento de batch do embedder, extração de URLs do remember (L1).

## [1.0.52] - 2026-05-15

### Breaking
- Exit code do erro `Duplicate` alterado de 2 para 9 para resolver colisão com erros de parsing de argumentos do Clap (L1). Agentes que roteiam no exit 2 para detecção de duplicatas devem atualizar para exit 9.
- `forget` não mais emite JSON no stdout quando a memória não é encontrada (M2). Anteriormente emitia `{"action":"not_found",...}` + erro no stderr; agora emite apenas erro no stderr + exit 4, consistente com `read`, `edit`, `history`, `rename`.

### Corrigido
- Resposta JSON do `restore` agora inclui campo `action: "restored"`, consistente com `edit`, `rename`, `forget` (H1).
- `--lang pt` agora traduz completamente os corpos das mensagens de erro para português, não apenas os prefixos (H2).
- `ingest` em diretório inexistente retorna exit 1 (Validation) em vez de exit 14 (Io) (M1).
- `prune-relations --dry-run` agora calcula a contagem de `entities_affected` em vez de retornar 0 fixo (L2).

### Adicionado
- Eventos NDJSON do `ingest` incluem campo `original_filename` preservando o basename do arquivo antes da normalização para kebab-case (H3).
- Flag `--dry-run` para `ingest`: previsualiza o mapeamento arquivo→nome sem carregar o modelo ONNX nem persistir (M5).
- Flag `--show-entities` para `prune-relations`: exibe os nomes das entidades afetadas durante `--dry-run` (L2).
- Novo subcomando `export` transmite todas as memórias como NDJSON para backup/migração portátil (L4).
- `health --json` inclui `mentions_ratio` e `mentions_warning` quando mentions dominam o grafo acima de 50% (C2).

### Alterado
- `Vec::new()` substituído por `Vec::with_capacity()` em 7 hot paths de produção: health checks, resultados do recall, travessia related, warnings do purge, NMS do GLiNER, construtor de relacionamentos, deduplicação de entidades (M3).

### Encerrado (falsos positivos do gaps.md)
- M4: `recall` já possui flag `--max-graph-results` para limitar a expansão de grafo independentemente de `--k`.
- L3: `graph entities --json` já retorna o campo `entity_type` no schema EntityItem.

## [1.0.51] - 2026-05-15

### Corrigido
- `remember` e `remember --force-merge` em memória soft-deletada agora retornam exit 2 (Duplicate) com mensagem acionável em vez de exit 10 (Database/UNIQUE constraint). Com `--force-merge`, a memória soft-deletada é restaurada e atualizada em um único passo (M7).
- Variável de ambiente `SQLITE_GRAPHRAG_NAMESPACE` agora respeitada por todos os comandos. Anteriormente, 8 comandos (`list`, `remember`, `read`, `edit`, `forget`, `history`, `rename`, `restore`) ignoravam a variável de ambiente devido ao `default_value = "global"` do Clap preenchendo o argumento de namespace (M8).

### Adicionado
- Flag `--max-rss-mb` para `remember` e `ingest`: aborta o embedding se o RSS do processo ultrapassar o threshold (padrão 8192 MiB). Previne que o ONNX runtime esgote a memória do sistema em documentos grandes (mitigação C1).
- 6 novos testes unitários do daemon cobrindo capping de backoff exponencial, range de half-jitter, transições CAS de versão, resolução de nome de socket e roundtrip de serialização de estado (M3).
- Seção "Destaques da Versão" no README (L3).

### Alterado
- Timeout do nextest para `recipe_01_bootstrap` elevado para 180s no perfil default para prevenir falsos negativos em builds debug (M6).
- Texto de ajuda do `--gliner-variant` agora documenta o trade-off de precisão do int8 (L4).
- Texto de ajuda do `--namespace` nos 8 comandos agora mostra precedência da variável de ambiente.

## [1.0.50] - 2026-05-15

### Adicionado
- Novo subcomando `prune-relations` para remoção em massa de relacionamentos por tipo (H8). Suporta flags `--dry-run`, `--yes`, `--namespace` e `--json`. Inclui `after_long_help` com exemplos de uso.
- Migração V011 adiciona índice `idx_relationships_ns_relation` para filtragem eficiente por tipo de relação.
- Auto-restart do daemon em version mismatch (H7): CLI agora detecta quando o daemon em execução é de uma versão anterior e reinicia automaticamente antes do primeiro request de embedding. Limitado a uma tentativa de restart por processo para prevenir loops.
- Nova constante `DAEMON_VERSION_RESTART_WAIT_MS` (5 segundos) para timeout de restart do daemon.
- Nova constante `CHUNK_BATCH_SIZE` (16) para futuro pipeline de embedding em streaming.

### Alterado
- `warn_if_non_canonical` agora chamado nos comandos `unlink` (H1) e `related` (H2) para consistência com `link`, `remember` e `ingest`.
- `related --help` agora documenta os 12 tipos canônicos de relação e suporte a relações customizadas (H6).
- Funções `errors_msg::*` em `src/i18n.rs` sempre retornam inglês (H3). Traduções para português permanecem em `app_error_pt` para stderr via `localized_message_for()`. JSON stdout agora é contrato de API totalmente determinístico somente em inglês.
- `Vec::with_capacity()` aplicado em `graph.rs`, `ingest.rs`, `link.rs` onde os tamanhos são previsíveis (M2).
- `.iter().cloned().collect()` substituído por `.iter().copied().collect()` para valores i64 em BFS de `graph.rs` (M1).
- Exportação de grafo agora emite `tracing::warn!` quando edges referenciam entidades inexistentes em vez de descartá-las silenciosamente (C2).
- String de erro em português no caminho multi-chunk de remember.rs substituída por inglês (H3).

### Corrigido
- `graph_export.rs` descarte silencioso de edges: edges órfãs agora logadas com IDs de entidade e tipo de relação (C2).
- Comandos `unlink` e `related` agora emitem warning em relações não canônicas para consistência (H1, H2).
- Módulo `errors_msg` não mais retorna strings em português que vazavam para JSON stdout (H3).
- `MIGRATION.md` atualizado com nota do rename `.items` para `.entities` (v1.0.44) e mudanças v1.0.49/v1.0.50 (L2).
- Versão do schema incrementada para 11 correspondendo à migração V011.

### Encerrado (falsos positivos do gaps.md)
- H4: SystemTime no jitter do daemon já havia sido corrigido na v1.0.43 (usa fastrand). `now_epoch_ms()` legitimamente usa SystemTime para timestamps epoch.
- H5: EntityType já é um enum Clap `value_enum` estrito com 13 variantes validadas.
- M4: Streaming NDJSON do ingest já estava implementado via `mpsc::sync_channel`.
- L1: Todos os 28 subcomandos já possuem `after_long_help`.
- M5: Falha do GLiNER int8 em textos curtos é limitação de quantização do modelo, não bug de código.

## [1.0.49] - 2026-05-15

### Alterado
- Vocabulário de relações agora é extensível: `link`, `unlink`, `related`, `remember --graph-stdin` e `ingest` aceitam qualquer string snake_case/kebab-case como relação, não apenas os 12 valores canônicos. Relações não canônicas emitem `tracing::warn!` para discoverability, mas são aceitas sem erro.
- Migração V010 remove a constraint `CHECK(relation IN (...))` da tabela `relationships`.
- Enum Clap `RelationKind` (`ValueEnum`) substituído por `String` com value parser `parse_relation` em `src/parsers/mod.rs`.
- `is_valid_relation()` duplicado em `remember.rs` e `ingest.rs` consolidado no compartilhado `parsers::validate_relation_format()`.

## [1.0.48] - 2026-05-14

### Corrigido
- `--graph-stdin` não mais desabilita silenciosamente extração NER quando combinado com `--enable-ner` e array `entities` vazio; o guard de NER agora verifica presença real de entidades em vez da fonte de input.
- Inferência GLiNER ONNX: tensor `span_mask` agora usa corretamente `tensor(bool)` em vez de `tensor(i64)`, corrigindo o type mismatch que fazia todas as variantes do modelo GLiNER recaírem silenciosamente para extração regex-only.
- `ingest` agora reporta `status: "skipped"` com `action: "duplicate"` (não `status: "failed"`) para memórias duplicadas, incrementando corretamente `files_skipped` em vez de `files_failed`.
- `ingest` em diretório inexistente agora retorna exit code 14 (Io) em vez de exit code 4 (NotFound), seguindo a semântica documentada de exit codes para erros de filesystem.
- `daemon --ping` agora emite `tracing::warn!` quando a versão do daemon difere da versão do binário CLI, orientando o usuário a reiniciar.
- `--skip-extraction` agora emite aviso de depreciação quando usado sozinho (NER está desabilitado por padrão desde v1.0.45).
- Campo `extraction_method` na resposta JSON do `remember` agora é definido como `"none:extraction-failed"` quando extração NER falha, em vez de ausente (`null`).

### Adicionado
- Schema `docs/schemas/ingest-file-event.schema.json` para evento NDJSON por arquivo do `ingest`.
- Schema `docs/schemas/ingest-summary.schema.json` para linha resumo do `ingest`.
- Campo `extraction_method` em `docs/schemas/remember.schema.json`.
- Campo `original_name` em `docs/schemas/remember.schema.json`.
- Seção GLiNER zero-shot NER no README e README.pt-BR com documentação de `--enable-ner`, `--gliner-variant` e `extraction_method`.
- Documentação de status NDJSON do `ingest` (`indexed`/`skipped`/`failed`) no README e README.pt-BR.
- Exemplos `after_long_help` para subcomandos `init`, `recall` e `remember`.

## [1.0.47] - 2026-05-14

### Alterado
- Substituído BERT NER (Davlan/bert-base-multilingual-cased-ner-hrl) por GLiNER zero-shot NER (onnx-community/gliner_multi-v2.1 via ONNX); remove dependências candle-core, candle-nn, candle-transformers e adiciona ndarray.
- `extraction.rs` reduzido de 2.314 para ~900 linhas após remoção do pipeline BERT e lógica de tokenizer.
- NER agora resolve 13 tipos de entidade específicos do domínio (`person`, `organization`, `location`, `date`, `project`, `tool`, `file`, `concept`, `decision`, `incident`, `dashboard`, `issue_tracker`, `memory`) em vez dos 4 tipos fixos do BERT (PER/ORG/LOC/DATE).

### Adicionado
- Flag `--gliner-variant` em `remember` e `ingest` seleciona a variante de pesos ONNX: `fp32` (padrão, 1,1 GB, melhor qualidade), `fp16` (580 MB), `int8` (349 MB), `q4` (894 MB), `q4f16` (472 MB).
- Variável de ambiente `SQLITE_GRAPHRAG_GLINER_VARIANT` como override persistente para `--gliner-variant`.
- Variável de ambiente `SQLITE_GRAPHRAG_GLINER_THRESHOLD` para ajustar o limiar de confiança de entidades (float, padrão `0.5`).
- Variável de ambiente `SQLITE_GRAPHRAG_GLINER_MODEL` para sobrescrever o identificador do repositório do modelo.

## [1.0.46] - 2026-05-14

### Corrigido
- `SQLITE_GRAPHRAG_ENABLE_NER=1` agora funciona corretamente; anteriormente apenas `true`/`false` eram aceitos pelo parser bool do Clap, causando exit 2 para `1`/`yes`/`on`. Novo `parse_bool_flexible` aceita `1`/`true`/`yes`/`on` (verdadeiro) e `0`/`false`/`no`/`off` (falso), case-insensitive.
- Preprocessamento de queries FTS5 agora sanitiza caracteres especiais (`"`, `*`, `(`, `)`, `^`, `:`) e filtra keywords FTS5 (`OR`, `AND`, `NOT`, `NEAR`) das queries do usuário, prevenindo erros de sintaxe em input malformado.
- `--enable-ner` combinado com `--skip-extraction` agora emite `tracing::warn!` ao invés de ignorar silenciosamente a contradição; `--enable-ner` prevalece.
- 9 falhas de testes de integração pré-existentes corrigidas: 4 testes de auto-init atualizados (health, stats, recall, vacuum), 1 asserção de help do daemon atualizada (flag `--json` oculto), 1 teste de normalização de rename atualizado, 3 testes de contrato de schema corrigidos.
- 7 JSON schemas atualizados para refletir output atual da CLI: `remember.schema.json` (+3 campos), `read.schema.json` (tipo metadata), `history.schema.json` (tipo metadata + campo deleted), `purge.schema.json` (tipo oldest_deleted_at + campo message), `hybrid-search.schema.json` (+rrf_score), `related.schema.json` (+name, +max_hops), `health.schema.json` (+memories_total em counts).

### Adicionado
- `parse_bool_flexible` em `src/parsers/mod.rs` para parsing flexível de booleanos reutilizável na integração Clap com variáveis de ambiente.
- 4 novos testes E2E de integração em `tests/v1045_features.rs`: busca de termos compostos FTS5 (hifenizados, com pontos) e aceitação de env var NER (`=1`, `=true`).
- 9 novos testes unitários: 3 para `parse_bool_flexible`, 6 para sanitização de caracteres especiais/keywords FTS5.

## [1.0.45] - 2026-05-13

### Alterado
- **S5** Extração BERT NER agora desabilitada por padrão. Passe `--enable-ner` ou defina `SQLITE_GRAPHRAG_ENABLE_NER=1` para ativar. A flag `--skip-extraction` é mantida como no-op oculto para compatibilidade retroativa.

### Adicionado
- **A1** Pré-processamento de queries FTS5: termos compostos contendo `-`, `.`, `_`, `/` (ex: `graphrag-precompact.sh`, `v1.0.44`) agora são convertidos em expressões phrase + prefix OR antes do MATCH, corrigindo buscas sem resultado em identificadores técnicos. Zero migração de schema necessária.
- Flag `--enable-ner` nos comandos `remember` e `ingest` para opt-in na extração BERT NER de entidades/relacionamentos.
- Variável de ambiente `SQLITE_GRAPHRAG_ENABLE_NER` como override persistente para `--enable-ner`.
- 6 novos testes unitários para `preprocess_fts_query()` e busca FTS5 de termos compostos.

### Documentação
- Todos os 10 arquivos de documentação atualizados para refletir `--enable-ner` substituindo `--skip-extraction` como flag ativo.
- Tabela de variáveis de ambiente no README/README.pt-BR agora inclui `SQLITE_GRAPHRAG_ENABLE_NER`.
- SKILL.md (EN/PT), AGENTS.md (EN/PT), COOKBOOK.md (EN/PT), HOW_TO_USE.md atualizados.

## [1.0.44] - 2026-05-13

### Corrigido
- **B1** `README.md` e `README.pt-BR.md`: comentários `#` inline removidos dos blocos de código shell usados como exemplos de parada do daemon; quebravam 2 casos do nextest.
- **C1** `hybrid-search --with-graph` era no-op: as flags `--with-graph`, `--max-hops` e `--min-weight` eram aceitas mas nunca conectadas ao handler; `graph_matches` era hardcoded como `[]`. Agora executa graph traversal via `traverse_from_memories_with_hops`, igualando o comportamento do `recall`.
- **C2** Docstring falsa no `link`: `after_long_help` e doc comment do `--from` alegavam que entidades eram "criadas implicitamente por chamadas anteriores de `link`" — era falso; o comando retornava exit 4 para entidades inexistentes. Documentação corrigida; flag `--create-missing` adicionada (ver Adicionado).
- **C3** `link.schema.json` estava obsoleto: listava campos removidos `source`/`target`, enum `action` errado (`"updated"` em vez de `"already_exists"`), e `elapsed_ms` ausente do `required`. Schema reescrito.
- **H1-old** Lista de stopwords expandida com 12 entradas adicionais que vazavam para resultados de extração de entidades.
- **H2-old** Entrada `H5` do CHANGELOG corrigida com as 13 variantes canônicas de `EntityType`.
- **H3-old** Subcomando `related`: fallback bidirecional agora retorna relações na direção reversa (`B→A`).
- **H4-old** Subcomando `rename`: nome aceito como argumento posicional (`rename old new`).
- **H1** JSON de `graph entities`: chave do array renomeada de `items` para `entities` (BREAKING). O comando se chama `graph entities` então `.entities[]` é o acessor natural. Schema atualizado.
- **H2** Exemplo jaq no `after_long_help` do `link` corrigido: era `graph --format json | jaq '.nodes[].name'`, agora `graph entities | jaq '.entities[].name'`.
- **M1-old** Truncamento de agregados agora emite `tracing::warn!` quando excede `MAX_ENTITIES_PER_MEMORY`.
- **M1** `expect()` em produção no `ingest.rs` substituído por `AppError::Internal`: o panic por violação de invariante agora propaga erro adequado.
- **M2** Profile de release endurecido: adicionado `panic = "abort"` e alterado `lto = true` para `lto = "fat"`.
- **M3-old** Invalidação de cache do `list` corrigida para `--include-deleted`.
- **M3** Comentário português no `Cargo.toml` traduzido para inglês (conformidade com política linguística).
- **M6-old** Output de `list --include-deleted` agora inclui campo `deleted_at`.

### Adicionado
- **C2** Flag `link --create-missing`: cria automaticamente entidades inexistentes, tipo padrão `concept`. Flag opcional `--entity-type` especifica o tipo. Resposta inclui array `created_entities` (omitido quando vazio).
- **M2-old** Env var `SQLITE_GRAPHRAG_EXTRACTION_MAX_TOKENS` documentada em ambos README.
- **M5-old** `vacuum --help` com nota sobre `reclaimed_bytes` possivelmente reportando `0`.

### Removido
- Deletados `docs/CLAUDE.md`, `docs/CLAUDE.pt-BR.md`, `docs/PRD.md`, `docs/PRD.pt-BR.md`, `docs/AGENT_PROTOCOL.md`, `docs/AGENT_PROTOCOL.pt-BR.md` e `docs/adr/0001-daemon-warmup-exception.md` (consolidados no CLAUDE.md na raiz e em docs_rules/ externo).

### Breaking Changes
- JSON de `graph entities`: chave renomeada de `items` para `entities`. Atualize queries jaq/jq: `.items[]` vira `.entities[]`.

### Adiado
- **M4** Streaming de entrada NDJSON para `ingest` — oficialmente adiado; ver seção Adiado de v1.0.43.

### Notas de Auditoria
- Release do `rusqlite` 0.39 monitorada via newreleases.io trustScore 9.1; `refinery` 0.9.1 ainda pina `rusqlite <=0.38`; upgrade adiado para v1.0.45+.

## [1.0.43] - 2026-05-03

### Corrigido
- **B1** Persistência incremental no `ingest` elimina a arquitetura de bloqueio 2-fase. A Fase B agora persiste cada registro imediatamente após a Fase A fazer o stage, prevenindo perda total de dados em corpora grandes (≥500 arquivos) que antes atingiam timeout em 30 min com zero linhas persistidas. Fecha 6+ meses de falhas reportadas em stress tests.
- **B2** Rótulo retroativo no CHANGELOG: seção `[Sem Versão]` na release v1.0.42 marcada retroativamente com o rótulo correto.
- **B3** Criados `docs/PRD.md` e `docs/PRD.pt-BR.md` documentando a baseline de requisitos de produto.
- **H1** Detecção de TTY no `stdin_helper`: guarda `is_terminal()` previne leituras bloqueantes quando stdin é pipe ou arquivo redirecionado, corrigindo deadlock em invocações não-interativas.
- **H2** Portadas 4 variantes i18n em português ausentes cobrindo as releases v1.0.26–v1.0.29.
- **H3** Links do CHANGELOG no `README.pt-BR.md` corrigidos; antes apontavam para fragmentos de âncora incorretos.
- **H4** Seção `EXAMPLES` adicionada ao `after_long_help` de 4 subcomandos graph (`graph`, `graph stats`, `graph path`, `graph neighbors`).
- **H6** Comentário `SAFETY` em `src/daemon/` realinhado para referenciar `docs/adr/0001-daemon-warmup-exception.md` em vez de prosa inline.
- **H7** Jitter `fastrand` substitui jitter baseado em `SystemTime` no backoff de busy-retry, eliminando possíveis panics por clock skew em sistemas com relógios de baixa granularidade.
- **L1** Fórmula `avg_degree` no `graph stats` corrigida: antes dividia pelo número de nós, agora computa corretamente `2 * edge_count / node_count` (convenção de grafo não-direcionado).
- **L3** Removido "agent" obsoleto do texto de ajuda de `--entity-type`; o enum agora usa variantes tipadas `EntityType`.
- **L4** Referências de versão limpas em todas as strings `after_long_help`; removidos pins obsoletos `v1.0.x`.
- **L5** "indefinido" padronizado para "undefined" em todas as strings i18n PT.

### Adicionado
- **B3** `docs/adr/0001-daemon-warmup-exception.md` — ADR formal documentando a exceção autorizada do daemon à regra no-persistent-daemon de `rules_rust_cli_stdin_stdout.md`.
- **H5** Enum `EntityType` com 13 variantes tipadas (`Concept`, `Date`, `Dashboard`, `Decision`, `File`, `Incident`, `IssueTracker`, `Location`, `Memory`, `Organization`, `Person`, `Project`, `Tool`) implementando `ToSql`/`FromSql` para round-tripping com rusqlite.
- **H8** ADR formal documentando a exceção autorizada do daemon para latência de warmup.
- **M6** `env_remove` para `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT` e variantes `DYLD_*` nos spawns de subprocessos, prevenindo vazamento de bibliotecas injetadas para processos filhos.
- **M7** Half-jitter adicionado ao loop de busy-retry do `storage`; antes usava delay fixo de 100 ms que causava thundering-herd sob escritas concorrentes.
- **M8** Dois env vars (`SQLITE_GRAPHRAG_LOW_MEMORY`, `SQLITE_GRAPHRAG_INGEST_PARALLELISM`) documentados em ambos README EN e PT-BR.
- **M9** Dois schemas de output (`docs/schemas/ingest.schema.json`, `docs/schemas/ingest-progress.schema.json`) adicionados à lista de referência de schemas no README.
- **L6** `MAX_ENTITIES_PER_MEMORY` agora é configurável via env var `SQLITE_GRAPHRAG_MAX_ENTITIES_PER_MEMORY` (inteiro, padrão 50). Permite que power users elevem o cap para documentos técnicos densos sem recompilar.

### Alterado
- **Bump ort/fastembed** Bump coordenado de ort `2.0.0-rc.11` → `2.0.0-rc.12` e fastembed `5` → `5.13.4`. Requerida migração em `src/embedder.rs` para reshuffle de módulo ort (`execution_providers::CPU` → `ep::CPU`). Fecha o upgrade adiado nas release notes de v1.0.42.
- **M1+M2+M3** Eliminadas chamadas `.clone()` desnecessárias e adicionada pré-alocação `Vec::with_capacity` em hot paths de ingest e recall, reduzindo pressão no alocador em corpora grandes.
- **M5** Tratamento de NaN na normalização de score substitui `.expect("NaN")` por `.unwrap_or(0.0)`, eliminando possíveis panics em valores de distância degenerados.
- **L2** Normalização de alias aplicada consistentemente nos subcomandos `link`, `unlink` e `related`; formas com hífen e underscore agora mapeiam para a mesma chave de relação canônica.

### Adiado para v1.0.44
- **M4** Streaming de entrada NDJSON para `ingest` — foco deslocado para o refactor arquitetural B1 durante a Wave 4; streaming de entrada adiado para o próximo ciclo.

## [1.0.42] - 2026-05-03

### Corrigido
- **HIGH 2** Migrados 14 doc comments em português para inglês em `src/constants.rs` (5x), `src/commands/stats.rs` (3x), `src/commands/health.rs` (1x), `src/commands/read.rs` (2x), `src/commands/list.rs` (1x), `src/commands/hybrid_search.rs` (2x). Alinha com a política linguística inviolável em `docs_rules/rules_rust.md`.
- **HIGH 3** Estendido o regex do gate `language-check` no CI (`.github/workflows/ci.yml:251`) para detectar preposições, adjetivos e substantivos PT sem diacríticos (`alias de`, `contrato documentado`, `migrado de`, `paralelo a`, `quando omitido`, etc.). Antes só verbos com acento eram pegos; o novo padrão captura os 14 doc comments corrigidos em HIGH 2 com zero falsos positivos no codebase atual.
- **LOW 3** Precedência POSIX do i18n: `LC_ALL=""` (string vazia setada) agora cai corretamente para `LC_MESSAGES`/`LANG` via guarda explícita `is_empty()` no loop locale (`src/i18n.rs:60-78`). Antes o valor vazio era tratado como locale reconhecido mas não parseado, quebrando a semântica POSIX em shells que exportam `LC_ALL=""`.

### Adicionado
- **MEDIUM 1** GitHub Releases agora incluem binário pré-compilado para `x86_64-apple-darwin` (Mac Intel) via runner `macos-13`, ao lado do build `aarch64-apple-darwin` existente. Fecha o gap onde usuários Mac Intel não tinham binário publicado.
- **LOW 1** Comando `restore` aceita o nome da memória como argumento posicional (`restore foo`); a flag `--name` é preservada como forma alternativa via `conflicts_with`. Espelha a UX de `forget`/`related`.
- **LOW 2** `sync-safe-copy` aceita o caminho de destino como argumento posicional (`sync-safe-copy /caminho/snapshot.sqlite`); flags `--dest`/`--to`/`--output` preservadas.
- **MEDIUM 4** `ingest --type` agora tem default `document` quando omitido; `MemoryType` deriva `Default` com `Document` como variante padrão.
- **MEDIUM 5** `apply_secure_permissions` e `sync-safe-copy` agora emitem um log `tracing::debug!` em Windows explicando que o DACL default do NTFS já provê acesso per-usuário; fecha o skip silencioso de releases anteriores.

### Alterado
- **HIGH 1** Removido o target `x86_64-unknown-linux-musl` da matrix de release. `ort` (o backend ONNX runtime usado pelo `fastembed`) não fornece prebuilt para o target musl em rc.11 nem rc.12 (verificado upstream via [ort-sys/build/download/dist.txt](https://github.com/pykeio/ort/blob/v2.0.0-rc.12/ort-sys/build/download/dist.txt)). Cinco releases consecutivos (v1.0.37 a v1.0.41) falharam neste job, bloqueando o passo de Publish GitHub Release. Usuários Alpine devem instalar via `cargo install sqlite-graphrag --locked` ou usar um container baseado em glibc (debian-slim, distroless/cc-debian12).
- **LOW 4** Bump `clap` 4.5 → 4.6 (sem breaks de API observados). `rusqlite` (0.37) mantido pois refinery 0.9.x faz hard-pin em rusqlite ≤0.38; `rayon` (1.10) mantido para evitar risco de bump de MSRV; bump coordenado de `ort`/`fastembed` adiado para v1.0.43 (requer migração de `src/embedder.rs` para reshuffles de módulos do rc.12: `ort::tensor`→`ort::value`, `execution_providers`→`ep`).

### Notas de Auditoria (adiadas para v1.0.43)
- **AUDIT-B1-BLOCKER**, **AUDIT-D8-HIGH**, **AUDIT-AUDIT-06-HIGH** — refactor da arquitetura 2-fase do `ingest --low-memory` (Phase A → Phase B com persistência incremental e streaming NDJSON) requer mais iteração de design; adiado para o próximo ciclo.
- **AUDIT-MEDIUM 2** Deduplicação de hash de conteúdo no `ingest` requer migração schema v10 (nova coluna `content_sha256` + índice). Adiada para evitar agrupar migrações de schema com patches.
- **AUDIT-MEDIUM 3 / C4 viés NER** BERT NER classifica identificadores de código (`TypeScript`, `AdapterExecutionResult`) como `organization`. Requer decisão arquitetural (substituir modelo, fine-tune, ou pós-processar). Adiada.
- **AUDIT-D9-MEDIUM** Drift de terminologia `nodes/edges` (graph) vs `entities/relationships` (stats) persiste; decisão de design necessária antes da unificação.

## [1.0.41] - 2026-05-02

### Corrigido
- **AUDIT-D1** README EN+PT Quick Start (linha 110) corrigido: substituído o enganoso "Execute `sqlite-graphrag init` primeiro antes de qualquer outro comando" por afirmação explícita de que GraphRAG está habilitado por padrão e roda automaticamente (auto-init via `ensure_db_ready()` em `src/storage/connection.rs:71-121`). `init` agora é descrito corretamente como OPCIONAL mas recomendado no primeiro uso para pré-baixar o modelo de embedding.
- **AUDIT-D2** README EN+PT Quick Start adiciona callout explícito "GraphRAG está habilitado por padrão" documentando auto-extração (BERT NER em cada `remember`/`ingest`) e auto-spawn do daemon (em `recall`/`hybrid-search`).
- **AUDIT-D11** `docs/schemas/vacuum.schema.json` adiciona `reclaimed_bytes` em `properties` e `required` (handler em `src/commands/vacuum.rs` já emitia esse campo, schema estava desatualizado).
- **AUDIT-D5** `after_long_help` do subcomando `Init` agora documenta que `init` é OPCIONAL (auto-init é transparente) e que ele aquece um embedding de smoke-test que auto-inicia o daemon persistente (~600s idle timeout). Fecha o gap onde o efeito colateral era não documentado.
- **AUDIT-C3** `DERIVED_NAME_MAX_LEN = 60` movido de `src/commands/ingest.rs:48` para `src/constants.rs` ao lado de `MAX_MEMORY_NAME_LEN = 80`. Single-source-of-truth restaurado, com doc comment explicando por que o cap do ingest é mais estrito (margem para sufixos de colisão).
- **AUDIT-AUDIT-04** `ingest` agora emite três markers INFO de progresso via `tracing::info!`: início da phase A (`stage_start` com contagem de arquivos e parallelism), progresso da phase A a cada 10 arquivos staged (`stage_progress` com done/total), e início da phase B (`persist_start`). Fecha o gap de visibilidade onde usuários não tinham sinal de progresso durante ingests longos.

### Notas de Auditoria (adiadas para v1.0.42)
- **AUDIT-B1-BLOCKER** `ingest --low-memory` com 495 arquivos atinge timeout em 30 min (`exit 124`) com **zero linhas persistidas** por causa da arquitetura 2-phase (Phase A faz stage de todos os arquivos em memória antes de Phase B persistir+emitir). Para corpora ≥500 arquivos em modo single-thread o run inteiro é perdido. Refactor para persistência incremental do Phase B é necessário.
- **AUDIT-D8-HIGH** Help promete streaming NDJSON "um objeto JSON por arquivo" mas stdout fica vazio durante toda a Phase A (fase inteira de stage). Será resolvido junto com AUDIT-B1-BLOCKER.
- **AUDIT-AUDIT-06-HIGH** Sem markers INFO de progresso durante ingests longos (apenas linhas WARN de truncation). Gap de visibilidade para usuários.
- **AUDIT-C3-MEDIUM** Constantes `MAX_MEMORY_NAME_LEN = 80` (em `src/constants.rs:30`, usado por `remember`) versus `DERIVED_NAME_MAX_LEN = 60` (hardcoded em `src/commands/ingest.rs:48`, usado na derivação de nomes de arquivo). Violação de single-source-of-truth.
- **AUDIT-C4-MEDIUM** NER produziu edge `DuckDuckGo --mentions--> DuckD`. Truncamento por sub-token boundary cria nomes parciais de entidades que poluem o grafo silenciosamente.
- **AUDIT-D9-MEDIUM** Drift terminológico: `graph --format json` retorna `nodes/edges`; `stats` retorna `entities/relationships`. Mesmo conceito, dois contratos.

### Documentação
- Todas as adições do README EN espelhadas em `README.pt-BR.md` (contagem de seções H2 preservada).

## [1.0.40] - 2026-05-02

### Corrigido
- **H-A2** README documenta valores de `relation` com hífen (forma de entrada na CLI: `applies-to`, `depends-on`, `tracked-in`); a forma com underscore é esclarecida como representação JSON de storage. Espelhado em `README.md`.
- **H-M8** Contrato de `chunks_persisted` esclarecido e testado via helper `compute_chunks_persisted()` em `src/commands/remember.rs`. Corpos de chunk único ficam na própria linha de `memories` (sem insert em `memory_chunks`), portanto `chunks_persisted = 0` para `chunks_created = 1` é correto por design. Schema e testes agora documentam esse invariante explicitamente.
- **M-A3** Nomes de memória derivados de nomes de arquivo aplicam normalização Unicode NFD e remoção de combining marks antes da sanitização kebab-case (`src/commands/ingest.rs:944`). `açaí🦜.md` agora produz nome com prefixo `acai` em vez de descartar todos os caracteres não-ASCII.
- **M-A5** Resultados de `recall` expõem um campo `score: f32` não-nulo em todo `RecallItem`, derivado da distância vetorial via `RecallItem::score_from_distance()` e clampado em `[0.0, 1.0]`. Teste garante que matches diretos retornam `score = 1 - distance`.
- **M-A6** `history.versions[].action` é sempre preenchido (nunca `null`). `change_reason_to_action()` mapeia razões internas de mudança para rótulos no passado (`created`, `edited`, `restored`, `renamed`).
- **M-A7** `deny.toml` registra entradas explícitas de ignore para os RUSTSEC transitivos: 2025-0119 (`number_prefix` via `indicatif`/`hf-hub`) e 2024-0436 (`paste` via `tokenizers`/`text-splitter`), com links de tracking upstream.

### Adicionado
- **H-A1** Flag `--low-memory` no `ingest` e variável de ambiente `SQLITE_GRAPHRAG_LOW_MEMORY` (valores truthy: `1`, `true`, `yes`, `on`) forçam `--ingest-parallelism 1`. Reduz pressão de RSS (~40 % medido em ingest de 30 arquivos) ao custo de 3-4× tempo de parede. Precedência: flag CLI > env var > `--ingest-parallelism N` explícito. Override emite `tracing::warn!` quando uma paralelização maior é passada explicitamente.
- **H-A1** README adiciona seção `## Memory Requirements` documentando o piso de ~2 GB para ONNX runtime + BERT NER + modelo fastembed, comportamento de escalonamento com paralelismo default, mitigação via `--low-memory`, orientação para containers/cgroups e link para a issue upstream de crescimento de memória do onnxruntime (microsoft/onnxruntime#22271).
- **M-A4** Help do `remember --body` e README documentam o limite inline de 500 KB (512000 bytes) e recomendam `--body-file` para entradas maiores.
- **M-A10** README adiciona tabela de subcomandos do `cache` documentando `clear-models` como único subcomando.

### Documentação
- Todos os acréscimos no README EN espelhados em `README.pt-BR.md` (contagem de seções H2 preservada: 24=24).
- `docs/schemas/recall.schema.json`, `docs/schemas/history.schema.json` e `docs/schemas/remember.schema.json` atualizados para refletir a semântica populada de `score`, `action` e `chunks_persisted`.

### Adiado (rastreado para v1.0.41)
- **M-A8** Upgrade `rusqlite 0.37 → 0.39` bloqueado pela restrição `rusqlite >=0.23, <=0.38` em `refinery 0.9.1` mais o breaking change de feature-flag `cache` no 0.38. Comentário em `Cargo.toml` documenta a justificativa.
- **M-A9** Upgrade `ort =2.0.0-rc.11 → =2.0.0-rc.12` bloqueado pelo hard-pin de rc.11 em `fastembed 5.13.2`. Bump coordenado (`fastembed 5.13.4` + `ort rc.12`) adiado; rc.12 também reorganiza módulos (`ort::tensor` → `ort::value`, `execution_providers` → `ep`, `IoBinding` movido), o que exige tocar `src/embedder.rs`.

## [1.0.39] - 2026-05-02

### Corrigido
- **B1** asserção de doctest em `src/errors.rs::localized_message_for` (verificação de mensagem localizada em português)
- **H1** pipeline de ingest paraleliza extract+embed via rayon (nova flag `--ingest-parallelism`); ordenação NDJSON preservada
- **H2** `build_relationships*` usa dedup por índice `HashSet<(usize,usize)>`, eliminando clones String O(N²)
- **M1** README documenta flags obrigatórias para `remember` (--name, --type, --description)
- **M2** README documenta padrão de `purge --retention-days` (90 dias) e `--retention-days 0` para purga total
- **M3** serialização do embedder documentada (paralelismo vive em ingest.rs)
- **M4** daemon adiciona limite de concorrência via Semaphore; `worker_threads` escala com `available_parallelism().clamp(2, 8)`
- **M5** dedup `seen` do NER usa `HashSet<u64>` (DefaultHasher), reduzindo clones String
- **M6** chamadas `format!` em hot-path da extração substituídas por pré-alocação `String::with_capacity`
- **M7** comentário SAFETY de `f32_to_bytes` expandido com invariantes explícitos (sem padding, lifetime, endianness)
- **M8** `remember.schema.json` lista `chunks_persisted` nos campos obrigatórios
- **M9** README documenta condições de resultado vazio para `related`
- **M10** README documenta convenção do daemon (flags vs subcomandos, estilo systemd)

### Documentação
- **L1** mensagem de expect do tokenizer esclarecida ("OnceLock::set succeeded above; get cannot fail in this single-init path")
- **L2** comentários SAFETY de regex de extração padronizados (regex_email/url/uuid)
- **L3** SAFETY de detach do Child do daemon referencia cruzada com rules_rust_processos_externos.md
- **L4** README adiciona Quick Start executável do ciclo de vida de memória (init→remember→recall→forget→purge)
- **L5** schema descreve a semântica de `chunks_created` vs `chunks_persisted`
- **L6** clones no caminho de erro do ingest eliminados naturalmente pela refatoração de pipeline em 2 fases
- **L7** reconhecido: contagem de `format!` permanece; redução adicional é micro-otimização
- **L8** README adiciona seção "Storage Footprint" explicando ~8× de bloat do DB para GraphRAG

### Dependências
- Adicionado `rayon = "1.10"` para paralelização do ingest

## [1.0.38] - 2026-05-02

### Corrigido
- **M2 (MEDIUM)**: `forget --json` agora emite `deleted_at_iso` (RFC 3339 UTC) paralelo a `deleted_at` (Unix epoch) quando uma memória é soft-deletada. Espelha o padrão existente em `read --json` (`created_at`/`created_at_iso`, `updated_at`/`updated_at_iso`). Ambos os campos usam `#[serde(skip_serializing_if = "Option::is_none")]` para que `not_found` continue omitindo-os. `docs/schemas/forget.schema.json` atualizado para documentar ambos os campos mais `action`.
- **M3 (MEDIUM)**: Eventos por arquivo de `ingest --json` agora expõem `truncated: bool` e `original_name: Option<String>`. Quando o nome derivado do arquivo excede `DERIVED_NAME_MAX_LEN` (60 chars), `truncated=true` e `original_name` carrega o valor pré-truncação, surfando no stdout o que antes era emitido apenas como `tracing::warn!` em stderr. Elimina colisões silenciosas em datasets grandes onde nomes de arquivo truncam para o mesmo prefixo kebab-case. `derive_kebab_name` agora retorna `(String, bool, Option<String>)`; 6 testes unitários atualizados.
- **M5 (MEDIUM)**: `src/main.rs` faz flush de stdout e stderr imediatamente antes de cada uma das 6 chamadas `std::process::exit`. Anteriormente, JSON ou erro buferizado podia ser perdido quando o processo saía sob broken pipe, desconexão de terminal ou shutdown rápido. Ambos os flushes são best-effort (erros ignorados via `let _ =`) pois o processo já está terminando.
- **M6 (MEDIUM)**: `src/output.rs::emit_json`, `emit_json_compact`, `emit_text` e `emit_error` agora locam stdout/stderr, executam `flush()` explícito e silenciam erros `BrokenPipe` graciosamente (retornam `Ok(())` em vez de propagar). Combina com a convenção do GNU coreutils onde pipelines como `sqlite-graphrag list --json | head -1` não disparam mais panics espúrios ou exit codes não-zero quando o consumidor fecha cedo.
- **M7 (MEDIUM)**: `src/daemon.rs:660` cai em `std::env::temp_dir()` em vez do literal hardcoded `"/tmp"` quando nem `XDG_RUNTIME_DIR` nem `SQLITE_GRAPHRAG_HOME` estão setados. Cross-platform: retorna `/tmp` em Unix, `%TEMP%` em Windows, e respeita `TMPDIR` quando setado. Alinhado com `docs_rules/rules_rust_multiplataforma_sistemas_operacionais.md`.
- **M8 (MEDIUM)**: Novo `src/stdin_helper.rs::read_stdin_with_timeout(secs)` aplica deadline de 60 segundos em `remember --body-stdin`, `remember --graph-stdin` e entrada de body de `edit`. Implementação: thread worker + `std::sync::mpsc::channel` + `recv_timeout` (sem conversão para async). Retorna `AppError::Internal` em timeout com mensagem indicando que o pipe deve fechar dentro do deadline. Anteriormente `std::io::stdin().read_to_string()` bloqueava indefinidamente se um processo upstream segurasse o pipe aberto sem enviar dados.
- **bônus política linguística**: Traduzido um erro de runtime PT residual em `src/tokenizer.rs` (`"tokenizer_config.json sem model_max_length"` → `"tokenizer_config.json missing model_max_length field"`) descoberto durante o H3 doc sweep. O gate de auditoria `rg '[áéíóúâêôãõç]' src/` já estava limpo para superfícies tracing/error/doc; esta string vivia dentro de um `format!` regular fora do escopo do gate anterior.

### Adicionado
- **H3 (HIGH, docs)**: 23 itens públicos em 6 módulos receberam doc comments `///` em INGLÊS no estilo idiomático Rust (seções `# Examples`, `# Errors`, `# Panics` quando aplicáveis): `src/chunking.rs` (8 itens: constantes, `Chunk`, 5 funções de chunking), `src/tokenizer.rs` (4 funções), `src/output.rs` (9 itens: `OutputFormat`, `JsonOutputFormat`, `emit_*`, `RememberResponse`, `RecallItem`, `RecallResponse`), `src/paths.rs` (1: `AppPaths`), `src/pragmas.rs` (2: `apply_init_pragmas`, `apply_connection_pragmas`), `src/embedder.rs` (5 helpers de embedding). `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` agora passa com zero warnings nesses módulos; um intra-doc-link privado pré-existente em `src/embedder.rs` foi reparado durante o sweep.
- **B1 (BLOCKER, UX)**: Nova flag CLI `--autostart-daemon` (default `true`) em `recall`, `hybrid-search` e outros subcomandos pesados de embedding, exposta via struct compartilhada `DaemonOpts` flattened com `#[command(flatten)]` em `src/cli.rs`. Antes o único opt-out era a env var `SQLITE_GRAPHRAG_DAEMON_DISABLE_AUTOSTART=1`, não documentada em `--help`. A nova flag tem precedência sobre a env var: passar `--autostart-daemon=false` pula spawn do daemon incondicionalmente independente da env. A env var ainda controla o caso default-true para retrocompatibilidade. `src/daemon.rs::should_autostart` é o ponto único de decisão; `autostart_disabled` foi renomeado para `autostart_disabled_by_env` por clareza semântica. Assinaturas de `embed_query_or_local` e `request_or_autostart` ganharam parâmetro `cli_autostart: bool`; `embed_passage_or_local` e `embed_passages_controlled_or_local` passam `true` para preservar comportamento existente. `src/commands/daemon.rs` `after_long_help` estendido com documentação do auto-spawn.
- **B1 docs**: README.md e README.pt-BR.md ganharam nova seção "Daemon auto-spawn behavior" / "Comportamento de auto-spawn do daemon" explicando os três mecanismos de controle (flag CLI, env var, subcomando `daemon` explícito) com exemplos shell.
- **testes de regressão**: `tests/cli_integration.rs` (arquivo novo) cobre quatro cenários end-to-end: (1) JSON de `forget` inclui `deleted_at_iso` após soft-delete, (2) evento de `ingest` sinaliza `truncated=true` com `original_name` quando nome de arquivo excede 60 chars, (3) `recall --autostart-daemon=false` não inicia daemon, (4) comportamento default de `recall` permanece inalterado. `src/stdin_helper.rs` traz um teste unitário do caminho de timeout; testes de regressão de `src/i18n.rs` para precedência POSIX (adicionados em v1.0.37) permanecem verdes.

### Notas
- v1.0.37 foi tagueada em git e pushed para GitHub (commit `4a4be74`) mas nunca publicada em crates.io; v1.0.38 é a release pública que bundla aquelas mudanças junto com as 8 correções adicionais acima. A entrada de v1.0.37 abaixo é preservada por transparência sobre o histórico git.
- Fora do escopo (backlog v1.0.39+): refactor de 6 `.clone()` em produção no hot path de `src/extraction.rs` (decisão pendente entre `Cow<'_, str>` e `Arc<str>`), bound de `tokio::sync::Semaphore` em chamadas `spawn_blocking` de `src/daemon.rs`, e investigação de upgrade `rusqlite 0.37 → 0.39` (pendente review de breaking changes via `context7-cli`).
- Fora do escopo permanentemente (decisão do usuário): deduplicação de campos JSON (`id`/`memory_id`, `memories`/`memories_total`, `entities`/`entities_total`, `relationships`/`relationships_total`, `db_size_bytes`/`db_bytes`) — mantidos para compatibilidade estável de consumidores.
- O orphan deliberado do daemon (`src/daemon.rs:489-501`) é preservado como comportamento documentado; o comentário `SAFETY` de 8 linhas justificando o ciclo de vida (spawn lock + readiness file + `Stdio::null()`) permanece como fonte de verdade.

## [1.0.37] - 2026-04-30

### Corrigido
- **B1+B2 (BLOQUEANTE, docs)**: Sincronizado `CHANGELOG.pt-BR.md` com a entrada v1.0.36 (estava ausente no espelho PT) e adicionados dois callouts faltantes em `README.pt-BR.md:108-109` espelhando `README.md` ("**Execute `init` primeiro**" e "**`graphrag.sqlite` é criado no diretório de trabalho atual por padrão**"). Auditoria no corpus flowaiper revelou que usuários PT-BR não descobriam o comportamento implícito do cwd.
- **H7+M9 (HIGH, comportamento)**: `list --include-deleted --json` agora emite `deleted_at` (Unix epoch) e `deleted_at_iso` (RFC 3339) para memórias soft-deleted. Memórias ativas continuam omitindo ambos os campos via `#[serde(skip_serializing_if = "Option::is_none")]` para backward compatibility. `MemoryRow` em `src/storage/memories.rs` ganhou campo `deleted_at: Option<i64>`; todos os quatro SELECTs SQL atualizados para incluir a coluna. `docs/schemas/list.schema.json` atualizado para documentar ambos os campos opcionais. Anteriormente agentes LLM chamando `list --include-deleted` não conseguiam distinguir linhas ativas de soft-deleted sem uma segunda query SQL.
- **H8 (HIGH, comportamento)**: `src/i18n.rs::Language::from_env_or_locale` agora respeita a precedência POSIX `LC_ALL > LC_MESSAGES > LANG`. O loop anterior iterava todas as três variáveis e retornava PT no primeiro prefixo "pt", violando semântica POSIX onde `LC_ALL` sobrescreve `LANG` independente do valor (`LC_ALL=en_US LANG=pt_BR` retornava PT em vez de EN). A correção para iteração na primeira variável setada, reconhece ambos os prefixos "pt" e "en", e cai no padrão English somente quando nenhuma variável de locale está setada. Três novos testes de regressão cobrem a regra de precedência.

### Adicionado
- **H9 (hardening CI)**: Novo job `cargo-audit` em `.github/workflows/ci.yml` executa `cargo audit --deny warnings`. Complementa `cargo deny check`, que anteriormente não sinalizava `RUSTSEC-2025-0119` (number_prefix unmaintained, transitiva via fastembed/hf-hub/indicatif) nem `RUSTSEC-2024-0436` (paste unmaintained, transitiva via tokenizers/text-splitter). Qualquer novo advisory agora bloqueia o merge até reconhecimento ou pin.
- **B6 (multiplataforma)**: Adicionado target `x86_64-unknown-linux-musl` à matriz de `.github/workflows/release.yml` (usa o step existente `Install musl tools` condicionado a `matrix.musl == true`). Habilita deploys em Alpine Linux e containers distroless sem forçar usuários a compilar do código.
- **B3 (docs)**: Criado `docs_rules/rules_rust.md` como índice canônico da Regra Zero referenciada pelo `CLAUDE.md` do projeto. Lista todos os oito arquivos de regras específicas em `docs_rules/` com resumos de uma linha e princípios invioláveis.
- **B4 (docs)**: Renomeado `docs_rules/rules_rusts_paralelismo_e_multiprocessamento.md` para `rules_rust_paralelismo_e_multiprocessamento.md` (correção de typo: `s` extra). O arquivo é gitignored e excluído do tarball publicado, então o rename não é visível para consumidores do crates.io.

### Melhorado
- **H1 (HIGH, extração)**: Expandido `ALL_CAPS_STOPWORDS` em `src/extraction.rs:58-173` com 23 palavras técnicas/genéricas PT-BR adicionais encontradas vazando para `entities` durante auditoria de 50 arquivos do corpus flowaiper: `ACID`, `AINDA`, `APENAS`, `CEO`, `CRIE`, `DDL`, `DEFINIR`, `DEPARTMENT`, `DESC`, `DSL`, `DTO`, `EPERM` (errno POSIX), `ESCREVA`, `ESRCH` (errno POSIX), `ESTADO`, `FATO`, `FIFO` (estrutura de dados), `FLUXO`, `FONTES`, `FUNCIONA`, `MESMO`, `METADADOS`, `PONTEIROS`. Lista cresceu de 108 para 131 entradas; anteriormente essas palavras eram capturadas por `regex_all_caps()` como entidades espúrias `concept`, poluindo o grafo com não-entidades (~27% das 402 entidades em corpus de 50 docs eram ruído). Filtro de stopwords está em ordem alfabética para leitura/revisão e usa scan linear via `.contains()`.

### Notas
- Findings descobertos durante o ciclo de auditoria v1.0.36 sobre o corpus real `flowaiper/docs_flowaiper` (495 arquivos markdown PT-BR). Fases A/B/C/D completaram (D=200/200), fase E (495/495) estava rodando no momento destas correções.
- Backlog v1.0.38+ remanescente: dedupe case-insensitive de entidades (CLAUDE/Claude, GEMINI/Gemini, GITHUB/GitHub vazando como entidades separadas), alinhamento hífen vs underscore em relations (CLI aceita `depends-on`, schema CHECK usa `depends_on`), ADR sobre daemon vs `rules_rust_cli_stdin_stdout` ("PROIBIDO daemons persistentes"), e targets multiplataforma remanescentes (`x86_64-apple-darwin`, `wasm32-wasip2`, universal2 macOS).
- Todos os oito gates de validação CLAUDE.md passam: fmt, clippy `-D warnings`, test (431/434, 3 ignorados), doc com `RUSTDOCFLAGS="-D warnings"`, audit com ignores documentados para dois advisories transitivos unmaintained pendentes upstream, deny check, publish dry-run, package list (138 arquivos, zero sensíveis).

## [1.0.36] - 2026-04-30

### Corrigido (Política linguística)
- **C1 (CRITICAL)**: Sincronizado enum `--type` em `skill/sqlite-graphrag-en/SKILL.md:46` e `-pt/SKILL.md:46` de 4 valores listados para o conjunto completo de 9 (`user, feedback, project, reference, decision, incident, skill, document, note`). Agentes usando SKILL.md como contrato perdiam silenciosamente cinco tipos de memória desde v1.0.30. Fonte de verdade: `src/cli.rs:364-374` (enum `MemoryType`) e `src/commands/remember.rs:26` long-help.
- **H1+H2+H3 (HIGH)**: Traduzidas três strings em português sem acentos em macros `tracing::warn!` que escaparam do gate de auditoria `rg '[áéíóúâêôãõç]' src/` documentado em v1.0.33: `src/extraction.rs:1204` (`"NER falhou..."` → `"NER failed..."`), `src/extraction.rs:964` (`"batch NER falhou (chunk de N janelas)..."` → `"batch NER failed (chunk of N windows)..."`), `src/commands/remember.rs:345` (`"auto-extraction falhou..."` → `"auto-extraction failed..."`). Bônus: também traduzidos `src/storage/urls.rs:37` (`"falha ao persistir url..."` → `"failed to persist url..."`) e o erro de produção em `src/commands/remember.rs:367` (`"limite de N namespaces ativos excedido..."` → `"active namespace limit of N reached..."`).
- **M1 (MEDIUM)**: Adicionado gate complementar de CI no job `language-check` de `.github/workflows/ci.yml` que escaneia macros `tracing::*!`, `#[error(...)]`, doc comments e `panic!`/`assert!`/`expect`/`bail!`/`ensure!` para palavras em português sem marcas diacríticas (`falhou`, `janelas`, `usando apenas`, `nao foi`, `ja existe`, `obrigatorio`, `memoria`, etc.). String literals simples não são escaneadas intencionalmente porque carregam fixtures legítimas em PT para extração multilíngue.
- **M3 (MEDIUM)**: Renomeados 33 nomes de funções de teste em português para inglês em `tests/integration.rs`, `tests/exit_codes_integration.rs`, `tests/concurrency_limit_integration.rs`, `tests/recall_integration.rs`, `tests/prd_compliance.rs`, `tests/loom_lock_slots.rs`, `tests/vacuum_integration.rs`, `src/commands/optimize.rs`, `list.rs`, `health.rs`, `debug_schema.rs`, `unlink.rs`. Exemplos: `test_link_idempotente_retorna_already_exists` → `test_link_idempotent_returns_already_exists`; `prd_optimize_executa_e_retorna_status_ok` → `prd_optimize_runs_and_returns_status_ok`; `optimize_response_serializa_campos_obrigatorios` → `optimize_response_serializes_required_fields`. Mais ~80 helpers `.expect("X falhou")` traduzidos para `.expect("X failed")`, doc comments e mensagens de assert limpas em `src/graph.rs`, `src/memory_guard.rs`, `src/cli.rs`, `src/storage/entities.rs` e diversos arquivos `tests/*.rs`. STRINGS de fixture que exercitam ingestão PT-BR (ex.: inputs NER multilíngue) permanecem intencionalmente em PT-BR.

### Corrigido (Lógica do código)
- **H5 (HIGH)**: Estendido `regex_section_marker()` em `src/extraction.rs:210-218` para incluir `Camada` ao lado de `Etapa`, `Fase`, `Passo`, `Seção`, `Capítulo`. Auditoria sobre corpus PT-BR de 50 arquivos mostrou `Camada 1` a `Camada 5` vazando para `entities` com degree 3 cada, poluindo o grafo. O filtro agora remove esses tokens tanto no estágio de regex prefilter quanto no post-merge BERT NER.
- **M7 (MEDIUM)**: Expandido `ALL_CAPS_STOPWORDS` em `src/extraction.rs:60-165` com `ADICIONADA`, `ADICIONADAS`, `ADICIONADO`, `ADICIONADOS`, `CLARO`, `CONFIRMARAM`, `CONFIRMEI`, `CONFIRMOU` (mesclados alfabeticamente na lista). A auditoria anterior encontrou essas formas verbais e adjetivas PT-BR sendo capturadas como entidades `concept` por `regex_all_caps()` em `apply_regex_prefilter`.
- **L2 (LOW)**: Backoff de spawn do daemon em `src/daemon.rs:record_spawn_failure` agora aplica half jitter (`base/2 + rand([0, base/2))`) em vez de exponencial puro. Evita retry herd se múltiplas instâncias da CLI detectarem falha do daemon simultaneamente. Usa `SystemTime::now().subsec_nanos()` como fonte de entropia sem dependências — suficiente para coordenação de spawn de baixa frequência.
- **L5+L6 (LOW)**: `src/i18n.rs::Language::from_env_or_locale` agora trata `SQLITE_GRAPHRAG_LANG=""` vazio como não-definido (sem `tracing::warn!` emitido), seguindo convenção POSIX. `src/i18n.rs::init` faz short-circuit quando o OnceLock já está populado, evitando que o resolvedor de env rode uma segunda vez e emita o warning duas vezes.

### Melhorado
- **M2 (MEDIUM)**: Adicionada seção "JSON Schemas" em `README.md`, `README.pt-BR.md`, `docs/AGENT_PROTOCOL.md` e `docs/AGENT_PROTOCOL.pt-BR.md` com link para os 30 arquivos canônicos de JSON Schema em `docs/schemas/`. Esses contratos existiam desde v1.0.33 mas eram indescobríveis a partir da documentação pública.
- **M4 (MEDIUM)**: `src/i18n.rs::tr` não vaza mais uma alocação por chamada. A assinatura agora exige inputs `&'static str` (que todos os chamadores no repositório já passam — são string literals) e retorna um deles diretamente. O padrão anterior `Box::leak(en.to_string().into_boxed_str())` acumulava alocações em pipelines de longa duração.
- **L3 (LOW)**: Adicionado callout MSRV (Rust 1.88) nas seções Installation de `README.md` e `README.pt-BR.md`. Anteriormente documentado apenas como nota de rodapé nas notas Mac Intel.

### Notas
- **M6 reclassificado como artefato de documentação/teste**: foi reportado que `related --json` retornava `graph_depth: null`, mas o campo se chama `hop_distance` (`src/commands/related.rs:77` e chave serializada). A query da auditoria usou `.graph_depth` que não existia. O campo sempre esteve corretamente populado. Sem mudança de código necessária.
- **L1 (sys_locale) foi diferido**: o parsing manual de `LC_ALL`/`LANG` em `src/i18n.rs:34-57` funciona corretamente nos targets usados no CI. Adicionar `sys_locale` introduziria uma dependência para benefício marginal (APIs CFLocale do macOS e GetUserDefaultLocaleName do Windows) sem reproducer confirmado.
- **L4 (BERT NER misclassifications) está fora de escopo**: `Tokio=location`, `Borda=person`, `Campos=location` e `AdapterRun=organization` são limitações de `Davlan/bert-base-multilingual-cased-ner-hrl`. Filtrar exigiria modelo diferente ou whitelist curada; ambos diferidos até causarem impacto concreto ao usuário.
- Todos os 427 testes de lib passam com os novos nomes de teste e assertions traduzidas. `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo doc`, `cargo audit` e `cargo deny check advisories licenses bans sources` estão limpos.
- O novo gate `language-check` no CI agora bloqueia qualquer PR que reintroduza PT em superfícies tracing/error/doc/assert.

## [1.0.35] - 2026-04-30

### Corrigido
- **WAL-AUTO-INIT (HIGH)**: O caminho de auto-init (`remember`, `ingest`, `recall`, `list`, ... — todo comando que passa por `ensure_db_ready()`) agora ativa `journal_mode=wal` consistentemente. Antes da v1.0.35 apenas o comando `init` explícito alterava o journal mode para WAL; bancos criados sob demanda por outros comandos permaneciam em `journal_mode=delete`, quebrando a semântica de checkpoint do `sync-safe-copy`, as garantias de concorrência documentadas e o conselho de troubleshooting que referenciava WAL. A correção move `PRAGMA journal_mode = WAL` para `apply_connection_pragmas` (chamado por todo `open_rw`) e adiciona uma re-asserção defensiva (`ensure_wal_mode`) após migrações para neutralizar o reuso interno de handles do refinery. Cobertura de regressão: `tests/wal_auto_init_regression.rs`.
- **JSON-SCHEMA-VERSION (MEDIUM-HIGH)**: `init --json`, `stats --json` e `migrate --json` agora emitem `schema_version` como **número** JSON em vez de string, alinhando com `health --json` (que já usava número). Corrige inconsistência de parsing para clientes que consumiam ambos os formatos. **Quebra** clientes que comparavam explicitamente como string; clientes usando comparação numérica não são afetados.
- **DAEMON-SOCKET-FALLBACK (LOW)**: Caminho de fallback do socket Unix em `to_local_socket_name()` agora respeita `XDG_RUNTIME_DIR` e em seguida `SQLITE_GRAPHRAG_HOME` antes de cair para `/tmp`. Reduz risco de colisão em hosts multi-tenant. O caminho só é usado quando sockets de namespace abstrato falham ao bindar (raro).

### Adicionado
- **CLI-LIMIT-ALIAS (UX)**: `recall` e `hybrid-search` agora aceitam `--limit` como alias de `-k/--k`. Alinhamento com `list`/`related` que já usavam `--limit`. Não-quebrante, aditivo.
- **CLI-RENAME-FROM-TO (UX)**: `rename` agora aceita `--from`/`--to` como aliases de `--name`/`--new-name`. Não-quebrante, aditivo.
- **JSON-RELATED-INPUT-ECHO (UX)**: A resposta `related --json` agora inclui campos `name` e `max_hops` ecoando o input para transparência. Não-quebrante, aditivo.

### Modificado
- **GRAPH-NODE-KIND-DEPRECATED**: `graph --format json` ainda emite ambos os campos `kind` e `type` por nó, mas `kind` está agora formalmente documentado como **deprecated** (mantido para backward-compat pré-v1.0.35). Novos consumidores DEVEM ler `type`. O campo duplicado será removido em uma futura release maior.

### Documentação
- **PRAGMA-USER-VERSION-49**: Adicionado doc comment em `src/constants.rs` explicando por que `SCHEMA_USER_VERSION = 49` (assinatura do projeto para ferramentas externas) versus `CURRENT_SCHEMA_VERSION = 9` (contagem de migrações aplicacional). São intencionalmente diferentes e servem propósitos distintos.
- **README**: Tabela do ciclo de vida de conteúdo de memória expandida com flags `--body-file`/`--body-stdin`/`--entities-file`/`--relationships-file`/`--graph-stdin` para `remember`, novos aliases para `recall`/`rename`, e callout sobre validação de nomes em ASCII kebab-case. Linhas explícitas para `ingest` e `cache clear-models` adicionadas.
- **JSON Schemas**: `docs/schemas/stats.schema.json`, `docs/schemas/migrate.schema.json` e `docs/schemas/debug-schema.schema.json` atualizados refletindo `schema_version` como integer e clarificando a relação `user_version` (49) vs `schema_version` (9) como intencionalmente independentes.

### Notas
- Achados da auditoria #4 (flags estruturadas de truncamento na saída JSON) e #6 (progress/ETA no resumo de ingest) ficam diferidos para v1.0.36 — requerem design de schema além de uma release de patch. Truncamento atualmente é exposto apenas via `tracing::warn!`; consumidores de pipeline devem monitorar stderr.
- Todos os 427 testes de lib passam. Teste de regressão `wal_auto_init_regression.rs` adicionado (usa `assert_cmd` + `tempfile`, mesmo padrão dos testes de integração existentes).
- Entradas detalhadas para v1.0.32, v1.0.33 e v1.0.34 abaixo trazem resumo executivo; o detalhamento completo permanece em `CHANGELOG.md` (EN) que é a fonte canônica.

## [1.0.34] - 2026-04-30

### Adicionado
- **JS7 (LOW)**: `vacuum --json` agora inclui o campo `reclaimed_bytes: u64` (calculado como `size_before_bytes.saturating_sub(size_after_bytes)`).

### Documentação
- **PRD-sync (LOW)**: `docs_rules/prd.md` (excluído do crate publicado) atualizado para refletir os enums atuais de MemoryType (9) e EntityType (13) após V008 e V009.

### Notas
- Auditoria de `unwrap`/`expect`: ZERO unwraps em produção; 12 expects em produção todos com invariantes documentados (compile-time, BERT NER, OnceLock, regex literais).
- Auditoria de blocos `unsafe`: todos com comentários SAFETY (~14 blocos em main.rs/embedder.rs/connection.rs/optimize.rs/paths.rs).
- Bump de patch: `reclaimed_bytes` é puramente aditivo; sem API removida; sem mudança comportamental.

## [1.0.33] - 2026-04-30

### Corrigido (Política Linguística)
- **C3-residual (HIGH)**: Traduzida string em português remanescente em `src/daemon.rs:183` (Drop impl). Gate `rg '[áéíóúâêôãõç]' src/ -g '!i18n.rs'` agora retorna ZERO matches.
- **PT-V007 (HIGH)**: Traduzido cabeçalho SQL de 5 linhas em português em `migrations/V007__memory_urls.sql` para inglês (arquivo é parte do crate publicado).
- **AS-PT (MEDIUM)**: Traduzidas 20 mensagens de `assert!` em português para inglês em `src/commands/hybrid_search.rs` (19) e `src/commands/list.rs` (1) + 9 mensagens em `src/storage/memories.rs`.

### Corrigido (Documentação)
- **D3 (MEDIUM)**: Sincronizado doc-comment de `--type` em `recall.rs`, `list.rs`, `hybrid_search.rs` para listar todos os 13 tipos de entidade do grafo (project/tool/person/file/concept/incident/decision/memory/dashboard/issue_tracker/organization/location/date) — antes listava apenas 10.

### Notas
- Validado contra ingest real de 50 arquivos `.md` (~6.6 MB): 50/50 indexados em 56.9s com `--skip-extraction`; 5/5 com extração BERT completa em 57.3s. Auto-create de `graphrag.sqlite` com modo 0600 confirmado.
- Campos JSON duplicados em `stats --json` e `list --json` preservados intencionalmente para backward compat.
- Assimetria de tipo `schema_version` entre `stats --json` (String) e `health --json` (u32) documentada como issue conhecida — corrigida posteriormente em v1.0.35.

## [1.0.32] - 2026-04-30

### Corrigido (Crítico — achados de auditoria de v1.0.31)
- **C1 (CRITICAL)**: Auto-init unificado em todos os handlers CRUD via novo helper `ensure_db_ready` em `src/storage/connection.rs`. Antes apenas `remember` auto-criava o DB; agora todos os subcomandos CRUD criam o banco no primeiro uso.
- **C2 (CRITICAL)**: Documentado o detach deliberado do daemon órfão em `src/daemon.rs:487` com comentário SAFETY explicando ownership de ciclo de vida via spawn lock + ready file + idle-timeout shutdown.
- **C3 (CRITICAL)**: Novo teste de integração `tests/readme_examples_executable.rs` parseia todos os blocos bash de `README.md` e `README.pt-BR.md` em compile time e executa cada invocação contra um binário real.

### Corrigido (Alto)
- **A1 (HIGH)**: Traduzidas 8 strings PT runtime para EN em `src/lock.rs`, `src/daemon.rs`. Adicionadas mensagens i18n bilíngues para validação de query/body vazios.
- **A2 (HIGH)**: Refatorado `src/commands/ingest.rs` de fork-spawn por arquivo para pipeline in-process. **40× mais rápido**: 50 arquivos em 21s (vs ~14 min antes).
- **A3 (HIGH)**: Substituído `.expect("OnceLock populated by set() above")` em `src/embedder.rs:56` por `.ok_or_else(...)?` propagando erro real.
- **A4 (HIGH)**: Adicionado `#[command(after_long_help = "EXAMPLES: ...")]` com 2-4 invocações realistas a 21 subcomandos previamente sem.
- **A5 (HIGH)**: Auto-migração transparente. `ensure_db_ready` compara `PRAGMA user_version` com `SCHEMA_USER_VERSION` e roda migrações pendentes automaticamente quando DB antigo é aberto por binário novo.
- **A6 (HIGH)**: Renomeados 23 identificadores PT para EN em testes e fontes; comentários PT residuais traduzidos.

### Corrigido (Médio)
- **M1 (MEDIUM)**: `recall -k` e `hybrid-search -k` agora usam `value_parser = parse_k_range` validando intervalo `1..=4096` em parse time.
- **M2 (MEDIUM)**: UX de `purge` clarificada com alias `--max-age-days` e mensagem helpful quando `purged_count == 0`.
- **M3 (MEDIUM)**: Adicionado `#[arg(help = "...")]` a 9 argumentos posicionais previamente sem help.
- **M4 (MEDIUM)**: Verificado que `daemon --stop` já existe; design de detach orfão documentado.

### Corrigido (Baixo)
- **B_1-B_4 (LOW)**: Estrutura README, badge CI, exemplos bash para 16 subcomandos, e campos `name_was_normalized`/`original_name` em saída JSON de `remember`.

### Adicionado
- `tests/readme_examples_executable.rs` (442 linhas, 10 testes).
- `parse_k_range` value parser em `src/parsers/mod.rs`.
- `validation::empty_query()` / `validation::empty_body()` em `src/i18n.rs`.
- `ensure_db_ready(&AppPaths)` em `src/storage/connection.rs`.

### Notas
- Pipeline de validação: `cargo fmt --check` ✓, `cargo clippy -- -D warnings` ✓, `cargo test --lib` 427/427 ✓, `cargo doc --no-deps` ✓, `cargo audit` ✓, `cargo deny` ✓.
- Gate de linguagem: `rg '[áéíóúâêôãõç]' src/ -g '!i18n.rs'` ZERO matches.
- Performance: 50 arquivos ingest em 21s (≈40× mais rápido que v1.0.31).

## [1.0.31] - 2026-04-30

### Corrigido
- **A2 (P1-CRÍTICO)**: subcomando `ingest` agora emite NDJSON correto (um objeto JSON por linha). Antes emitia JSON multilinha indentado, quebrando consumidores line-by-line. 5 chamadas em `src/commands/ingest.rs` trocadas de `output::emit_json` para `output::emit_json_compact`.
- **A3 (P1-MÉDIO)**: `stats --json` agora reporta `schema_version` correto (ex.: "9") lendo de `refinery_schema_history`. Antes retornava "unknown" porque a tabela `schema_meta` (vazia) era consultada.
- **A4 (P1-MÉDIO)**: comando `forget` agora popula `action` e `deleted_at` no JSON de saída. Três estados explícitos: `soft_deleted`, `already_deleted`, `not_found`. Race-safe via re-SELECT após soft-delete.
- **A1 (P0-CRÍTICO)**: pipeline de extração não trava mais em documentos > 50 KB. Adicionado cap `EXTRACTION_MAX_TOKENS=5000` (override via env `SQLITE_GRAPHRAG_EXTRACTION_MAX_TOKENS`). Body que excede o cap é truncado para NER mas o body completo continua passando pelo regex. Impacto empírico: documento de 68 KB caiu de >5 minutos para ~37 segundos (88% de redução), mantendo `extraction_method=bert+regex-batch`.
- **A9 (P2-MÉDIO)**: fan-out de relacionamentos reduzido — entidades co-ocorrendo na mesma sentença/parágrafo agora geram edges; antes gerava C(N,2) "mentions" entre todas entidades da memória.
- **A10 (P2-MÉDIO)**: truncamento de nome em 60 chars agora emite `tracing::warn` e trata colisões com sufixo numérico (-1, -2, ...) dentro da mesma run.

### Adicionado
- **A6**: nova suite `tests/ingest_integration.rs` cobrindo contrato NDJSON, fail-fast, max-files, truncamento de nome, --skip-extraction, variantes de --pattern, walk recursivo (10 testes).
- **A7**: testes E2E para V009 em `tests/schema_migration_integration.rs`: `v009_document_type_lifecycle_e2e`, `v009_note_type_lifecycle_e2e`, `v009_invalid_type_rejected`.
- **A11**: stoplist PT-BR de palavras em caixa alta para filtro NER (ADAPTER, PROJETO, PASSIVA, SOMENTE, LEITURA, etc.). Melhora qualidade da extração para corpora em português.

### Melhorado
- **A5 (P1-MÉDIO)**: 210 funções de teste em `src/*` renomeadas de português para inglês em 35 arquivos (cobre também helpers como `nova_memoria` → `new_memory`, `cria_node` → `make_node`, `resposta_vazia` → `empty_response`). Codebase agora 100% em conformidade com a política linguística do projeto (identificadores exclusivos em inglês).
- **A8 (P1-MÉDIO)**: refinamento de `.unwrap()`/`.expect()` em código de produção. A contagem original da auditoria de 167 estava inflada — a maioria dos matches estava em blocos `#[cfg(test)] mod tests` (aceitáveis pelo CLAUDE.md). O inventário real em produção era de 13 ocorrências. Melhorias: 1 `.expect()` em `src/embedder.rs` recebeu mensagem de invariante mais precisa; 10 `Regex::new(LITERAL).unwrap()` em initializers de `OnceLock` em `src/extraction.rs` substituídos por `.expect("compile-time validated <kind> regex literal")`; 2 `.max_by(...).unwrap()` sobre logits do BERT NER substituídos por `.expect("BERT NER logits invariant: no NaN in classifier output")`; 1 `.expect()` em `src/chunking.rs` traduzido de PT para EN.
- **A12+A13**: ~38 comentários PT traduzidos em `tests/signal_handling_integration.rs`, `tests/lock_integration.rs` e `deny.toml`. 2 entries `[advisories.ignore]` obsoletas removidas (RUSTSEC-2024-0436, RUSTSEC-2025-0119) — `cargo deny check` agora reporta zero warnings de advisory-not-detected.
- **A14**: ~150 comentários PT adicionais traduzidos em `tests/prd_compliance.rs`, `tests/integration.rs`, `tests/concurrency_hardened.rs`, `tests/security_hardening.rs` e outros arquivos de teste.

### Metodologia da Auditoria
- 13 gaps identificados empiricamente via auditoria em plan-mode contra binário v1.0.30 instalado, usando corpus real (20 documentos markdown PT-BR).
- Todos os fixes validados via PDCA + orquestração com Agent Teams: 13 tasks, 9 teammates spawnados em paralelo, cada um com Regra Zero do CLAUDE.md e validação por task.
- Validação aprovada: cargo fmt, cargo clippy --all-targets -- -D warnings, cargo audit, cargo deny check, cargo doc -D warnings, cargo nextest run.

## [1.0.30] - 2026-04-29

### Adicionado (Novo Subcomando — Ingestão em Massa)
- `sqlite-graphrag ingest <DIR> --type <TYPE>` para indexar em massa todo arquivo de uma pasta como memória separada. Flags: `--pattern` (default `*.md`), `--recursive`, `--skip-extraction`, `--fail-fast`, `--max-files` (cap default 10000), `--namespace`, `--db`. Saída NDJSON: um objeto por arquivo (`{file, name, status, memory_id, action}`) seguido de summary final (`{summary, files_total, files_succeeded, files_failed, files_skipped, elapsed_ms}`). Nome derivado do basename em kebab-case. Cada arquivo é processado por subprocesso `remember --body-file`, preservando slots de concorrência, locks e semântica de erro do `remember` standalone. Resolve gap UX de longa data onde usuário precisava `for f in *.md; do remember ...; done`.

### Alterado (Help mais Claro — `link` / `unlink`)
- `link --help` e `unlink --help` agora deixam EXPLÍCITO que `--from` e `--to` aceitam nomes de ENTIDADES (nós do grafo extraídos por BERT NER, ou criados implicitamente por `link` anterior), NÃO nomes de memória. Inclui blocos `EXAMPLES:` e `NOTES:` em `after_long_help`. O help anterior "Source entity" era facilmente mal interpretado como "nome de memória"; o erro `Erro: entidade '<name>' não existe` confundia o usuário. Doc comments agora apontam `graph --format json | jaq '.nodes[].name'` como forma canônica de listar entidades elegíveis.

### Alterado (Dependências — upgrade rusqlite/refinery)
- `rusqlite` 0.32 → 0.37 e `refinery` 0.8 → 0.9. Cargo.lock agora resolve `rusqlite v0.37.0`, `refinery v0.9.1`, `refinery-core v0.9.1`, `refinery-macros v0.9.1`, `libsqlite3-sys v0.35.0`. Zero mudanças de código fonte — ambos crates mantiveram APIs públicas estáveis. Tentativa de chegar a rusqlite 0.39 foi bloqueada por `refinery-core 0.9.0` com cap `rusqlite = ">=0.23, <=0.37"`; revisitar quando refinery elevar esse teto.

### Corrigido (Crítico — Inconsistência de Schema/Contrato CLI)
- `migrations/V009__expand_memory_types.sql` — nova migration que recria a tabela `memories` (e suas filhas com FK: `memory_versions`, `memory_chunks`, `memory_entities`, `memory_relationships`, `memory_urls`) para expandir o CHECK do campo `type` de 7 para 9 valores, adicionando `'document'` e `'note'`. Sem essa migration, `--type document` e `--type note` (adicionados ao enum da CLI em v1.0.29) eram sempre rejeitados em runtime com `exit 10` — `CHECK constraint failed: type IN ('user','feedback','project','reference','decision','incident','skill')`. A camada Clap aceitava nove valores enquanto o banco impunha apenas sete, quebrando todos os exemplos do README que usavam `--type document`.
- `tests/schema_migration_integration.rs` atualizado para assert exatamente 9 migrations aplicadas (antes esperava 6) e `schema_version = "9"`.

### Corrigido (Crítico — Violações de Política Linguística Não Detectadas em v1.0.28)
A auditoria v1.0.28 usou regex de linha única e reportou zero violações; macros multi-linha e identificadores sem acentos escaparam. Corrigido nesta versão:

- `src/extraction.rs:749, 1025` — 2 `tracing::warn!` em PT traduzidos para EN.
- `src/extraction.rs` — 8 chamadas `.context(...)`, `.with_context(...)` e `anyhow::anyhow!` em PT traduzidas (forward pass, removendo dimensão batch, criando tensor de ids/máscara/diretório do modelo, carregando tokenizer NER, encoding NER).
- `src/daemon.rs` — 2 strings `tracing::*!` traduzidas (lock file de spawn, daemon encerrado graciosamente).
- `src/commands/restore.rs` — 1 `tracing::info!` traduzido (`restore --version omitido`).

### Corrigido (Identificadores de Teste — Política Inglês-Apenas)
~80 identificadores de teste (nomes de função, helpers, módulos `mod`, type aliases) renomeados de PT para EN. Phase 1 só pegou subset com diacríticos; identificadores sem acento (`*_aceita_`, `*_rejeita`, `*_funciona`, `*_retorna`, etc.) foram missed. Arquivos tocados:

- `src/cli.rs`, `src/paths.rs`, `src/errors.rs`, `src/commands/{init, migrate, sync_safe_copy, cleanup_orphans, list, vacuum}.rs`, `src/extraction.rs`, `src/output.rs`, `src/memory_guard.rs`, `src/storage/{urls, memories, entities}.rs`.
- `tests/security_hardening.rs` (16 fns), `tests/integration.rs` (~28 fns), `tests/prd_compliance.rs` (~15 fns), `tests/concurrency_*.rs`, `tests/i18n_bilingual_integration.rs`, `tests/signal_handling_integration.rs`, `tests/v2_breaking_integration.rs`, `tests/lock_integration.rs`, `tests/property_based.rs`, `tests/loom_lock_slots.rs`, `tests/regression_positional_args.rs`, `tests/recall_integration.rs`, `tests/daemon_integration.rs`, `tests/schema_migration_integration.rs`.

### Notas
- `errors::to_string_pt()` e `main::emit_progress_i18n(en, pt)` mantêm strings PT legítimas — são o branch i18n acionado quando `--lang pt` (ou locale detectado) está ativo. Não são violações.
- Comportamento default `./graphrag.sqlite` em CWD (`paths.rs:35-41`) confirmado empiricamente contra o corpus de auditoria v1.0.29 (29 de 30 documentos Markdown flowaiper indexados end-to-end; recall p50 ~50ms, hybrid-search p50 ~52ms; uma falha do stress test foi timeout externo de 60s, não defeito da ferramenta).

## [1.0.29] - 2026-04-29

### Corrigido (Crítico — Violações de Política Linguística em Código de Produção)
- `src/paths.rs:21` — mensagem de erro em português `"não foi possível determinar o diretório home"` em `AppError::Io` traduzida para `"could not determine home directory"`.
- `src/paths.rs:85-89` — mensagem de erro em português `"caminho '{}' não possui componente pai válido"` em `AppError::Validation` traduzida para `"path '{}' has no valid parent component"`.
- `src/main.rs:227` — `tracing::warn!` em português traduzido para `"shutdown signal received; waiting for current command to finish gracefully"`. Logs de tracing devem ser em inglês independente do locale.
- `src/commands/purge.rs:21` — doc comment em português `"[DEPRECATED em v2.0.0]"` traduzido para `"[DEPRECATED in v2.0.0]"`.
- `src/commands/purge.rs:70-71` — string de aviso em português `"--older-than-seconds está deprecado..."` (emitida no campo JSON `warnings`) traduzida para `"--older-than-seconds is deprecated; use --retention-days in v2.0.0+"`.
- `src/commands/purge.rs:123` — `anyhow!` em português `"erro de relógio do sistema: {err}"` traduzido para `"system clock error: {err}"`.
- `src/commands/purge.rs:192-193` — aviso em português `"falha ao limpar vec_chunks..."` traduzido para `"failed to clean vec_chunks for memory_id {memory_id}: {err}"`.
- `src/commands/purge.rs:198-201` — aviso em português `"falha ao limpar vec_memories..."` traduzido para `"failed to clean vec_memories for memory_id {memory_id}: {err}"`.
- `src/main.rs:265` — removido `tracing::error!(error = %e)` duplicado que vazava string de erro localizada em logs estruturados.

### Corrigido (Segurança — Path Traversal e Auditoria Unsafe)
- `src/paths.rs:60` — `validate_path` agora usa `Path::components().any(|c| c == Component::ParentDir)` em vez de substring `.contains("..")`, prevenindo falsos positivos e possíveis bypasses.
- `src/extraction.rs:271` — adicionado comentário `SAFETY:` abrangente ao bloco `unsafe { VarBuilder::from_mmaped_safetensors(...) }` documentando os três invariantes de soundness.
- `src/storage/connection.rs:14-21` — adicionado comentário `SAFETY:` ao bloco `unsafe { rusqlite::ffi::sqlite3_auto_extension(...) }` documentando compatibilidade de ABI FFI.
- `src/paths.rs` (6 comentários SAFETY em testes) — traduzidos de PT para EN.

### Adicionado (Melhorias de UX)
- Flag `list --include-deleted` para exibir memórias soft-deletadas.
- Flag `history --no-body` para omitir o conteúdo do body das versões na resposta JSON.
- Variantes `MemoryType::Document` e `MemoryType::Note` adicionadas ao enum `--type` (`remember`, `list`, `recall`).
- Texto de ajuda `help =` adicionado a ~10 flags previamente sem descrição (`--namespace`, `--limit`, `--offset`, `--format`, `--db`, `--include-deleted`, `--no-body`).
- README Quick Start documenta explicitamente que `sqlite-graphrag init` é o primeiro comando recomendado e que `graphrag.sqlite` é criado no diretório de trabalho atual.

### Alterado (Schema e UX)
- Flag `--json` agora está oculta em 21 subcomandos via `#[arg(long, hide = true)]`. A flag continua aceita para backward compatibility.
- Resposta JSON de `history`: campo `metadata` alterado de `String` para `serde_json::Value`.
- Resposta JSON de `history`: campo `body` agora é `Option<String>` (omitido quando `--no-body` está ativo).
- `Cargo.toml` `exclude`: caminhos reescritos sem `/` inicial para semântica relativa idiomática do cargo.

### Notas
- Release de patch focada em conformidade de política e correções de UX detectadas na auditoria v1.0.28.
- Validado empiricamente contra corpus real de 495 arquivos Markdown durante a auditoria v1.0.28.

## [1.0.28] - 2026-04-28

### Alterado
- Política de idioma inglês-apenas aplicada em todo o codebase. Todos os doc comments `///` e `//!`, todos os logs `tracing::*!`, e todos os identificadores (funções, statics, módulos, variantes de enum, nomes de teste) fora de `src/i18n.rs` estão agora em inglês. Strings PT-BR permanecem apenas nos branches `Language::Portuguese` dentro de `i18n::errors_msg`, `i18n::validation`, e `errors::to_string_pt()`.
- Variante de enum `Language::Portugues` renomeada para `Language::Portuguese` (aliases `pt`, `pt-br`, `pt-BR`, `portugues`, `portuguese` preservados para backward compatibility).
- Static `IDIOMA_GLOBAL` renomeado para `GLOBAL_LANGUAGE` (`src/i18n.rs`).
- Static `FUSO_GLOBAL` renomeado para `GLOBAL_TZ` (`src/tz.rs`).
- ~30 funções com nomes PT renomeadas para equivalentes em inglês em `src/i18n.rs` e `src/tz.rs` (ex.: `formatar_iso` → `format_iso`, `epoch_para_iso` → `epoch_to_iso`, `memoria_nao_encontrada` → `memory_not_found`, `nome_kebab` → `name_kebab`, módulo `validacao` → `validation`, módulo `erros` → `errors_msg`).
- 32 módulos internos de teste `mod testes` renomeados para `mod tests` seguindo convenção Rust.
- Todos os call-sites em `src/commands/*.rs` e testes propagados para usar os identificadores renomeados.

### Adicionado
- Documentação `//!` crate-level em 37 módulos que anteriormente não tinham: `src/cli.rs`, `src/main.rs`, `src/extraction.rs`, `src/embedder.rs`, `src/daemon.rs`, `src/output.rs`, `src/paths.rs`, `src/chunking.rs`, `src/graph.rs`, `src/namespace.rs`, `src/parsers/mod.rs`, `src/tokenizer.rs`, `src/storage/{connection,urls,chunks,versions,mod}.rs`, `src/pragmas.rs`, e 22 handlers em `src/commands/`.
- Job `language-check` no CI (`.github/workflows/ci.yml`) que falha o build quando diacríticos PT são detectados em `///`, `//!`, chamadas `tracing::*!`, ou atributos `#[error(...)]`.

### Documentação
- Dois intra-doc links quebrados (`[Cli]`, `[TextEmbedding]`) corrigidos em `src/lib.rs` e `src/embedder.rs`.

### Notas
- Mudança não-quebrante para contratos CLI e JSON: nomes de subcomandos, flags, env vars, exit codes e nomes de campos JSON permanecem inalterados.
- 65 arquivos alterados, +872/-715 linhas. Todos os 9 gates cargo passam (fmt, clippy, test, doc, audit, deny, publish dry-run, package list, llvm-cov).

## [1.0.27] - 2026-04-28

### Adicionado
- Constante `CURRENT_SCHEMA_VERSION: u32 = 8` em `src/constants.rs` com teste unitário que verifica igualdade com a contagem de arquivos de migration `V*.sql`.
- Funções `output::emit_error` e `output::emit_error_i18n` centralizando saída de erros em stderr (Padrão 5: ÚNICO ponto de I/O em `output.rs`).
- Configuração de test-groups `nextest` em `.config/nextest.toml` para serializar testes cross-binary que compartilham socket do daemon e cache de modelos. Elimina flake `contract_15_link` observado desde v1.0.24.

### Alterado
- README EN+PT (seção `Graph Schema`) agora lista `entity_type` com exatamente 13 valores (antes 10) — adiciona `organization`, `location`, `date` introduzidos na migration V008 de schema em v1.0.25.
- Docstring de `init --help` documenta precedência de resolução de caminho (`--db` > `SQLITE_GRAPHRAG_DB_PATH` > `SQLITE_GRAPHRAG_HOME` > cwd).
- Comentário de distância de grafo em `src/commands/recall.rs` esclarecido: permanece proxy de contagem de hops (`1.0 - 1.0/(hop+1)`), distância cosseno real reservada para v1.0.28.
- Todas as 6 chamadas `eprintln!` em `src/main.rs` migradas para `output::emit_error*` para enforçar o Padrão 5.

### Documentação
- `SQLITE_GRAPHRAG_LOG_FORMAT` agora documentado na tabela de env vars do README EN+PT (implementado desde v1.0.x mas não documentado).
- Linha de `unlink` no README corrigida da flag inexistente `--relationship-id` para as flags reais `--from --to --relation`.
- `docs/MIGRATION.md` e `docs/MIGRATION.pt-BR.md` referência de versão atualizada de v1.0.17 para v1.0.27 (3 ocorrências cada).
- `docs/HOW_TO_USE.md` e `docs/HOW_TO_USE.pt-BR.md` exemplos de receita `link` corrigidos para usar `--from`/`--to` em vez das flags inexistentes `--source`/`--target`.

### Corrigido
- Drift de formatação em `tests/doc_contract_integration.rs:669` resolvido via `cargo fmt --all`.

### Notas
- Investigação do achado P1 de auditoria `tokenizer.rs:101-103 std::fs::read em caminho async` concluída como **falso positivo**: `get_tokenizer` e `get_model_max_length` são chamados apenas de `src/commands/remember.rs:389-391` dentro de `pub fn run()` que é síncrono.
- Dois warnings `advisory-not-detected` do `cargo deny` para advisories ignorados `RUSTSEC-2024-0436` (paste) e `RUSTSEC-2025-0119` (number_prefix) observados mas mantidos em `deny.toml`.

## [1.0.26] - 2026-04-28

### Adicionado
- Env var `SQLITE_GRAPHRAG_HOME` para definir o diretório base para `graphrag.sqlite` (precedência: `--db` > `SQLITE_GRAPHRAG_DB_PATH` > `SQLITE_GRAPHRAG_HOME` > cwd).
- README com exemplo de saída JSON de `remember` mostrando campos `extracted_entities`, `extracted_relationships` e `urls_persisted`.
- Tabela de exit codes expandida com sub-causas para exit 1 (erro de validação ou falha em runtime).

### Alterado
- README esclarece que a extração de entidades GraphRAG roda por padrão em `remember` (use `--skip-extraction` para desabilitar por chamada).
- Referência a "ingestão automática" no README renomeada para desambiguar "autostart do daemon" de "extração automática de entidades".

### Corrigido
- Contador `handled_embed_requests` do daemon agora reporta corretamente a contagem acumulada após autospawn do `init` (retornava 0 desde v1.0.24 por um contador local por conexão que sombreava o acumulador compartilhado).
- Teste `contract_15_link` alinhado com as chaves reais de saída de `link --json` (`action`, `from`, `to`, `relation`, `weight`, `namespace`); as expectativas obsoletas de `source`/`target` com IDs numéricos estavam desatualizadas desde v1.0.24.

## [1.0.25] - 2026-04-28

### Adicionado
- Flag `recall --all-namespaces` busca em todos os namespaces numa única consulta (P0-1).
- BERT NER agora emite tipos `organization` (B-ORG), `location` (B-LOC) e `date` (B-DATE)
  alinhados com a migration V008. Releases anteriores mapeavam ORG→`project`,
  LOC→`concept` e descartavam DATE completamente (P0-2 + alinhamento V008).
- Migration de schema V008: CHECK constraint de `entities.type` expandida para incluir
  `organization`, `location`, `date`. Migration aditiva; linhas existentes são preservadas.
- BRAND_NAME_REGEX captura nomes de organizações em CamelCase como "OpenAI", "PostgreSQL",
  "ChatGPT" que o BERT NER frequentemente classifica incorretamente (P0-2).
- Filtro de falsos positivos para verbos monossilábicos em PT-BR ("Lê", "Vê", "Cá", etc.)
  para saídas do BERT com confiança abaixo de 0.85 (P0-2).
- SECTION_MARKER_REGEX filtra fragmentos de texto como "Etapa 3", "Fase 1", "Passo 2",
  "Seção 4", "Capítulo 1" da extração de entidades (P0-4).
- 12 novas ALL_CAPS_STOPWORDS: `API`, `CAPÍTULO`, `CLI`, `ETAPA`, `FASE`, `HTTP`, `HTTPS`,
  `JWT`, `LLM`, `PASSO`, `REST`, `UI`, `URL` (P0-4).
- README documenta subcomandos `graph traverse|stats|entities` com tabela de flags (P1-A).

### Alterado
- `recall.graph_matches[].distance` agora reflete o hop count via proxy
  `1.0 - 1.0 / (hop + 1)`. Releases anteriores usavam placeholder `0.0`. Distância
  cosseno real reservada para v1.0.26 (P1-M).
- Lógica longest-wins de `merge_and_deduplicate` reescrita com chave composta
  `entity_type + name_lc` e containment bidirecional de substring. Resolve duplicação
  "Sonne"/"Sonnet" e truncamento "Open"/"Paper" (P0-3).
- Versão do `Cargo.toml` bumped de `1.0.24` para `1.0.25`.

### Corrigido
- `is_valid_entity_type` agora aceita os novos tipos da V008 `organization`, `location`, `date` (P0-A) — sem esta correção, `remember` rejeitaria qualquer entidade emitida pelo mapeamento IOB alinhado à V008 com exit 1.
- Regex `augment_versioned_model_names` não captura mais marcadores de seção em português como "Etapa 3" ou "Fase 1" (P0-B) — filtro de defesa em profundidade aplicado após augmentation e dentro de `iob_to_entities.flush()`.
- `remember --name` com mais de 80 bytes agora retorna exit 6 (LimitExceeded) em vez de
  exit 1 (Validation). Restaura o contrato de exit codes usado por agentes orquestradores (P1-J).

### Notas
- `recall.graph_matches[].distance` é aproximada; distância cosseno semântica reservada para v1.0.26.
- Caps de entidades (30) e relacionamentos (50) permanecem silenciosos na v1.0.25;
  flags `--limit-entities` / `--limit-relations` planejadas para v1.0.26.

## [1.0.24] - 2026-04-27

### Adicionado
- Inferência em lote do BERT NER via `predict_batch` reduz latência por documento em fluxos multi-doc (Phase 3 perf).
- Retry de SQLITE_BUSY e SQLITE_LOCKED com backoff exponencial em `with_busy_retry`; evita exit 10 espúrio em contenção de WAL (Phase 3).
- Aquecimento `spawn_blocking` para carga do modelo BERT no daemon; previne bloqueio do executor async durante inicialização (Phase 3).
- Migração de schema V007: tabela `memory_urls` com índices; URLs extraídas pelo BERT NER agora são persistidas separadamente em vez de vazar para o grafo de entidades (Phase 2).
- Módulo CRUD `src/storage/urls.rs` com `upsert_urls`, `get_urls_for_memory` e `delete_urls_for_memory` (Phase 2).
- Campo `RememberResponse.urls_persisted: usize` reportando quantas entradas de URL foram inseridas em `memory_urls` (Phase 2).
- Campo `RememberResponse.relationships_truncated: bool` indicando se o payload de relacionamentos foi truncado pelo limite de `max_relationships_per_memory` (Phase 4).
- `namespace_initial` persistido em `schema_meta` no `init`; `purge` resolve namespace contextualmente via `SQLITE_GRAPHRAG_NAMESPACE` (Phase 4 P1-A/P1-C).
- Argumentos posicionais e por flag em `read`, `forget`, `history`, `edit`, `rename`; por exemplo, `sqlite-graphrag read minha-nota` é equivalente a `sqlite-graphrag read --name minha-nota` (Phase 4 P1-B).
- Lista de stopwords expandida com 17 novas entradas: `ACEITE`, `ACK`, `ACL`, `BORDA`, `CHECKLIST`, `COMPLETED`, `CONFIRME`, `DEVEMOS`, `DONE`, `FIXED`, `NEGUE`, `PENDING`, `PLAN`, `PODEMOS`, `RECUSE`, `TOKEN`, `VAMOS` (Phase 2 P0-3).
- Normalização Unicode NFKC em `merge_and_deduplicate` evita entidades quase duplicadas causadas por formas Unicode compostas vs decompostas (Phase 2 P1-E).
- Testes de regressão para `graph` traverse com exit 4 quando o banco está ausente (Phase 1 P0-7).
- Testes de regressão para equivalência de argumento posicional com flag em `read`, `forget`, `history`, `edit`, `rename` (Phase 4 P1-B).

### Modificado
- `ReadResponse.metadata` agora é `serde_json::Value` em vez de `String`; agentes recebem um objeto estruturado diretamente sem segunda chamada a `JSON.parse` (Phase 5 P2-A).
- `LinkResponse` simplificado: campos redundantes `source` e `target` removidos; `LinkArgs` não aceita mais os aliases de flag `--source`/`--target` (Phase 4 P1-O).
- `purge` não assume mais namespace `"global"` como padrão; resolve via `SQLITE_GRAPHRAG_NAMESPACE` ou `--namespace` explícito (Phase 4 P1-C).
- O comportamento de `recall --precise` está agora documentado e usa internamente `effective_k = 100000` para KNN exaustivo (Phase 1 P0-6).
- `init --model` agora usa o enum tipado `EmbeddingModelChoice` validado em tempo de parse (Phase 1 P0-8).
- Medição de RAM em `main.rs` usa propagação de `Result` em vez de `expect` (Phase 1 P1-G).
- Carga do modelo no aquecimento do daemon movida para `spawn_blocking` para não bloquear o executor Tokio (Phase 3 P1-I).
- Regex de `augment_versioned_model_names` estendida para reconhecer padrões como `GPT-4o`, `Claude 4 Sonnet`, `Llama 3 Pro`, `Mixtral 8x7B` (Phase 5 P2-D).
- `extend_with_numeric_suffix` agora aceita sufixos alfanuméricos (ex: `v2`, `3b`, `7B`) além dos puramente numéricos (Phase 5 P2-E).
- Serialização de entidades do grafo usa `Vec::new()` em vez de `Option<Vec>`; o campo `entities` é sempre um array, nunca `null` (Phase 5 P2-C).
- Docstrings do argumento `--type` esclarecidas para distinguir `type` de memória de `entity_type` (Phase 5 P2-J).
- Versão do `Cargo.toml` bumped de `1.0.23` para `1.0.24`.

### Corrigido
- `remember` rejeita nomes que normalizam para string vazia após canonicalização kebab-case; retorna exit 1 com mensagem de validação clara (Phase 4 P0-4).
- URLs não vazam mais para o grafo de entidades; todos os tokens com forma de URL do BERT NER agora são roteados para `memory_urls` via V007 (Phase 2 P0-2).
- Serialização de `HybridSearchResponse.weights` confirmada correta; o campo era um flag fantasma sem efeito comportamental (Phase 4 P1-N).

### Segurança
- Comentários `// SAFETY:` adicionados a todos os blocos `unsafe { std::env::set_var(...) }` em `main.rs` (Phase 1 P1-H).
- `deny.toml`: `unmaintained` definido como `"workspace"` para restringir verificações de crates não mantidas apenas aos membros do workspace; reduz falsos positivos de CI em crates transitivas (Phase 5 P2-K).
- Valor inválido em `SQLITE_GRAPHRAG_LANG` agora emite log `tracing::warn!` em vez de retornar silenciosamente ao inglês (Phase 1 P1-M).

### Interno
- 412+ testes passando em todas as fases.
- Release bundle: Fases 1, 2, 3, 4 e 5 em um único commit.

## [1.0.23] - 2026-04-27

### Corrigido
- Mesclagem de subword do BERT NER agora prefere o candidato mais longo quando múltiplas fontes extraem nomes sobrepostos. Antes "OpenAI" extraído por regex podia perder para "Open" vazado de subword BERT porque ambos deduplicavam para a chave lowercase `open`. A nova lógica em `merge_and_deduplicate` retém estritamente a entrada mais longa, favorecendo a marca mais específica visível no corpus (P1 fix em `src/extraction.rs`).
- Nomes de modelos versionados com separador de espaço ("Claude 4", "Llama 3", "Python 3") agora são extraídos como entidades `concept` pelo novo passe `augment_versioned_model_names`. O BERT NER frequentemente classifica esses tokens como substantivos comuns e os pula, então o sufixo de versão sumia. Variantes com hífen como "GPT-5" continuam tratadas pelo pipeline NER+sufixo existente (P1 fix em `src/extraction.rs`).
- `recall` agora expõe `graph_depth: Option<u32>` em cada `RecallItem`. Matches diretos por vetor recebem `None` (use `distance`); resultados de traversal recebem `Some(0)` como sentinela para "alcançável via grafo, profundidade ainda não rastreada com precisão". O placeholder legado `distance: 0.0` permanece por compatibilidade mas deve ser tratado como depreciado para linhas de grafo (P1 fix em `src/commands/recall.rs` e `src/output.rs`).
- `remember` agora reporta `chunks_persisted: usize` ao lado de `chunks_created: usize` para que clientes saibam exatamente quantas linhas foram inseridas em `memory_chunks`. Bodies de chunk único reportam `chunks_persisted: 0` (a própria linha de memória atua como chunk) enquanto multi-chunk reportam `chunks_persisted == chunks_created`. Resolve o achado da auditoria v1.0.22 onde corpos curtos mostravam `chunks_created: 1` com zero linhas persistidas (P1 fix em `src/output.rs` e `src/commands/remember.rs`).

### Adicionado
- `recall --max-graph-results <N>` limita `graph_matches` a no máximo N entradas. Padrão é unbounded para preservar a forma vista em v1.0.22, mas permite capar vizinhanças densas de grafo explicitamente. A docstring de `-k` agora declara claramente que ela controla apenas `direct_matches` (P1 fix de UX em `src/commands/recall.rs`).
- README EN agora lista os aliases `pt-BR` e `portuguese` para `SQLITE_GRAPHRAG_LANG`. Antes apenas o README PT-BR os mencionava, deixando leitores ingleses sem ciência (P1 fix de sincronia de docs).
- README EN+PT agora documentam os cinco targets de binários pré-compilados explicitamente e destacam que Mac Intel (`x86_64-apple-darwin`) requer build local porque o GitHub aposentou o runner macos-13 em dezembro de 2025 e a Apple descontinuou suporte ao x86_64. Migração recomendada é para Apple Silicon (P1 fix de clareza de distribuição).
- `docs/COOKBOOK.md` e `docs/COOKBOOK.pt-BR.md` taglines agora declaram a contagem correta de 23 receitas (alegavam incorretamente 15 desde as adições da v1.0.22). Contado por `rg -c '^## How To'` em ambos arquivos (P1 fix de precisão de docs).

### Modificado
- `Cargo.toml` versão bumpada de `1.0.22` para `1.0.23`.
- JSON do `RememberResponse` ganha o campo `chunks_persisted` (sempre presente); JSON do `RecallItem` ganha `graph_depth` (omitido quando `None` via `skip_serializing_if`). Ambas adições são forward-compatible para qualquer cliente que use parsers JSON tolerantes.

## [1.0.22] - 2026-04-27

### Corrigido
- Workflow `forget` + `restore` não fica mais sem saída. `history --name <X>` agora retorna versões de memórias soft-deleted (antes filtrava `deleted_at IS NULL`); resposta inclui novo campo booleano `deleted`. `restore --version` agora é opcional: quando omitido, a última versão não-`restore` é usada automaticamente. Juntos, esses fixes fazem o round-trip `forget` → `restore` funcionar sem exigir leitura de SQL (correção P0 em `src/commands/history.rs` e `src/commands/restore.rs`).
- `list`, `forget`, `edit`, `read`, `rename`, `history`, `hybrid-search` agora verificam ausência de `graphrag.sqlite` antecipadamente e retornam `AppError::NotFound` (exit 4) com a mensagem amigável "Execute 'sqlite-graphrag init' primeiro", alinhando com `stats`/`recall`/`health`. Antes, `list` vazava o erro bruto do rusqlite e retornava exit 10 (correção de inconsistência P1).
- `remember` agora rejeita `body` vazio ou só com whitespace (sem grafo externo) via `AppError::Validation` (exit 1). Evita persistir memórias com embeddings vazios que quebravam a semântica de recall (correção P1 em `src/commands/remember.rs`).
- Pós-processamento BERT NER estendido para filtrar stopwords adicionais ALL CAPS PT-BR/EN observadas no stress de 495 documentos FlowAiper (verbos, adjetivos, substantivos comuns) e nomes de métodos HTTP (`GET`, `POST`, `DELETE`, etc.). Saídas NER de token único agora também são filtradas, não apenas matches do prefilter regex (correção P1 em `src/extraction.rs`).
- Prefilter de URL do BERT NER agora remove pontuação markdown final (backticks, parênteses, colchetes, pontos, ponto-e-vírgulas) antes de persistir URLs como entidades. Antes, `https://example.com/`` era armazenado literalmente (correção P1 em `src/extraction.rs`).
- Entidades BERT NER com sufixos numéricos hifenizados ou separados por espaço (ex: `GPT-5`, `Claude 4`, `Python 3.10`) agora são estendidas no pós-processamento em vez de truncadas. Lookup de sufixo é conservador: só estende quando ≤6 caracteres e puramente numéricos (correção P1 em `src/extraction.rs::extend_with_numeric_suffix`).
- Enumeração `entity_type` em README EN e pt-BR corrigida de "9 valores" para "10 valores" com `issue_tracker` listado (correção P1 docs).

### Adicionado
- Variável de ambiente `SQLITE_GRAPHRAG_MAX_RELATIONS_PER_MEMORY` para configurar o cap de relacionamentos-por-memória (padrão 50, intervalo [1, 10000]). A auditoria identificou que documentos com grafos ricos atingem o cap silenciosamente; usuários com corpora técnico agora podem ajustar (correção P1 via `src/constants.rs::max_relationships_per_memory()`).
- Campo `HistoryResponse.deleted: bool` expondo se a memória está atualmente soft-deletada, permitindo aos clientes detectar estado esquecido sem inspecionar `memory_versions` diretamente.
- 18 flags de CLI antes não documentadas agora possuem docstrings `///` visíveis em `--help`: `init --model`, `init --force`, `remember --name/--description/--body/--body-stdin/--metadata/--session-id`, `read --name`, `forget --name`, `edit --name/--body/--body-file/--body-stdin/--description`, `history --name`, `daemon --idle-shutdown-secs/--ping/--stop` (correção UX P1).

### Modificado
- `Cargo.toml` versão bumped de `1.0.21` para `1.0.22`.
- Const `MAX_RELS=50` em `src/extraction.rs` consolidada em `crate::constants::max_relationships_per_memory()` removendo a definição duplicada.
- Tipo do arg `restore --version` mudou de `i64` para `Option<i64>` (compatível com versão anterior: passar versão explícita continua funcionando).

## [1.0.21] - 2026-04-26

### Corrigido
- BERT NER `iob_to_entities` não vaza mais fragmentos WordPiece como `##AI` ou `##hropic` como entidades separadas. Quando BERT emite label `B-*` em um token iniciado por `##` (estado confuso do modelo), o subword é anexado à entidade ativa se houver, ou descartado caso contrário (correção P0 em `src/extraction.rs:381-394`). Validação empírica: auditoria de 138 documentos FlowAiper produziu ZERO fragmentos `##` na tabela de entidades.
- `recall` rejeita queries vazias com `AppError::Validation` e mensagem clara em vez de vazar erro bruto do rusqlite `Invalid column type Null at index: 1, name: distance` (correção P1 em `src/commands/recall.rs`).
- `restore` agora re-embeda o corpo da memória restaurada e faz upsert em `vec_memories` para que recall vetorial funcione em memórias restauradas. v1.0.20 deixava `vec_memories` desatualizado após `forget` + `restore` (correção P1 em `src/commands/restore.rs`).
- `stats` reporta `chunks_total` com precisão consultando `memory_chunks` e tratando apenas erros "no such table" como estado legado do DB; outros erros do SQLite agora são logados via `tracing::warn!` para visibilidade (correção P1 em `src/commands/stats.rs`).
- Seis panics em caminhos de produção convertidos para `unreachable!()` idiomático dentro de blocos `#[cfg(test)]` (correção P1 em `graph_export.rs`, `memory_guard.rs`, `optimize.rs`, `tz.rs`, `namespace_detect.rs`).
- Tabelas de exit codes do README EN e pt-BR agora listam `73` (guarda de memória rejeitou condição de pouca RAM), alinhando com `llms.txt` e semântica do source (correção P1 docs).

### Adicionado
- Campo `RememberResponse.extraction_method: Option<String>` expondo se a extração automática usou `bert+regex` ou caiu em `regex-only`. Campo é omitido do JSON quando `--skip-extraction` está ativo (telemetria P1 em `src/output.rs` e `src/commands/remember.rs`).
- Campo `ExtractionResult.extraction_method` populado por `extract_graph_auto` e `RegexExtractor`, expondo o caminho real de extração (correção P1 em `src/extraction.rs`).
- 2 testes novos cobrindo o fix do merge IOB: `iob_strip_subword_b_prefix` e `iob_subword_orphan_descarta`.

### Modificado
- `Cargo.toml` versão atualizada de `1.0.20` para `1.0.21`.

## [1.0.20] - 2026-04-26

### Corrigido
- Carregamento do modelo BERT NER agora baixa `tokenizer.json` do subdiretório `onnx/` do repositório `Davlan/bert-base-multilingual-cased-ner-hrl` no HuggingFace, onde o arquivo está de fato publicado. A v1.0.19 tentava baixar da raiz do repositório e recebia 404 em toda ingestão, caindo silenciosamente em graceful degradation só com regex (correção P0 primária em `src/extraction.rs::ensure_model_files`).
- Pesos da classifier head do BERT NER agora são carregados do arquivo safetensors via `VarBuilder::pp("classifier").get(...)` tanto para `weight` quanto para `bias`. A v1.0.19 inicializava com `Tensor::zeros`, o que produziria argmax constante em todos os tokens e tornaria toda predição degenerada mesmo após o fix do tokenizer. Este segundo P0 estava mascarado pelo primeiro e foi descoberto durante o planejamento emergencial (correção P0 secundária em `src/extraction.rs::BertNerModel::load`).
- Prefilter regex de identificadores ALL_CAPS agora filtra palavras-regra do português (`NUNCA`, `SEMPRE`, `PROIBIDO`, `OBRIGATÓRIO`, `DEVE`, `JAMAIS`, etc.) e equivalentes em inglês (`NEVER`, `ALWAYS`, `MUST`, `TODO`, `FIXME`, etc.), preservando identificadores com underscore como `MAX_RETRY` e acrônimos como `OPENAI`. Na v1.0.19 contra corpus técnico em PT-BR, 70% das top entidades eram ruído de palavras-regra (correção P1).
- Tipo de entidade para email mudou de `person` para `concept` porque regex sozinho não distingue indivíduos de endereços de role ou lista (correção P2).
- `merge_and_deduplicate` agora emite `tracing::warn!` quando a contagem de entidades é truncada em `MAX_ENTS=30`, expondo o cap antes silencioso (correção P2).
- `build_relationships` agora emite `tracing::warn!` quando o cap de relacionamentos `MAX_RELS=50` é atingido, complementando o aviso de entidades (correção P2).
- `remember` agora trata bodies só com whitespace (`\n\t  `) como vazios para skip de auto-extração, já que `.is_empty()` sozinho deixava whitespace puro passar (correção P3 em `src/commands/remember.rs`).
- Normalização kebab-case de `remember` e `rename` agora aplica `trim_matches('-')` para remover hífens em bordas, corrigindo rejeição de inputs como `my-name-` truncados por limites de comprimento de filename (correção P3 em `src/commands/remember.rs` e `src/commands/rename.rs`).

### Adicionado
- 4 testes unitários novos em `src/extraction.rs` cobrindo o stopword filter (`regex_all_caps_filtra_palavra_regra_pt`), aceitação de identificador com underscore (`regex_all_caps_aceita_constante_com_underscore`), aceitação de acrônimo de domínio (`regex_all_caps_aceita_acronimo_dominio`) e a reclassificação email→concept (`regex_email_captura_endereco`).

### Modificado
- `Cargo.toml` versão bumped de `1.0.19` para `1.0.20`.

## [1.0.19] - 2026-04-26

### Adicionado
- Chunking hierárquico-recursivo de markdown via `text-splitter = "0.30.1"` (`src/chunking.rs::split_into_chunks_hierarchical`) preserva fronteiras H1/H2 e separadores suaves de parágrafo para documentos que começam com marcadores markdown.
- Extração híbrida automática de entidades (`src/extraction.rs::extract_graph_auto`) combinando pré-filtro regex (emails, URLs, UUIDs, identificadores ALL_CAPS) com passagem CPU `candle` BERT NER (`Davlan/bert-base-multilingual-cased-ner-hrl`, ~676 MB safetensors, AFL-3.0). NER opera em janela deslizante com `MAX_SEQ_LEN=512` e `STRIDE=256`, limitado a `MAX_ENTS=30`/`MAX_RELS=50`. O modelo é baixado lazy na primeira execução e degrada graciosamente para apenas regex em caso de falha (via `tracing::warn!`).
- `remember` agora invoca `extract_graph_auto` automaticamente quando `--skip-extraction` está ausente, nenhum `--entities-file`/`--relationships-file`/`--graph-stdin` é fornecido e o body é não-vazio, materializando entidades e relacionamentos `mentions` antes da persistência.
- 15 testes unitários em `src/extraction.rs` cobrindo pré-filtro regex (email/URL/UUID/ALL_CAPS), decodificação IOB (mapeamento PER/ORG/LOC, descarte de DATE, ORG-com-sufixo-`sdk` → `tool`), enforcement de `MAX_RELS`, dedup por nome em lowercase e fallback gracioso quando o modelo NER está ausente.
- 6 novos testes de chunking em `src/chunking.rs` validando fronteiras `# H1` e `## H2`, documentos markdown de 60 KB com overlap 50, fallback de texto puro e separadores suaves de parágrafo `\n\n`.

### Mudado
- `Cargo.toml` adiciona `text-splitter = "0.30.1"` (features `markdown`, `tokenizers`) e `candle-core`/`candle-nn`/`candle-transformers = "0.10.2"` (default-features off) além de `huggingface-hub` (`hf-hub` renomeado) para downloads de modelo.
- `Cargo.toml` faz bump de `sqlite-vec` de `0.1.6` para `0.1.9` (correção de DELETE e melhorias em constraints KNN) e remove seis dependências órfãs (`notify`, `slug`, `toml`, `uuid`, `zerocopy`, `tracing-appender`).
- `Cargo.toml` reduz `tokio` de `features = ["full"]` para o conjunto mínimo `["rt-multi-thread", "sync", "time", "io-util", "macros"]`.
- Footprint de threads do daemon reduzido de ~65 para ≤4 threads sustentadas via `RAYON_NUM_THREADS=2`, `ORT_INTRA_OP_NUM_THREADS=1` e `ORT_INTER_OP_NUM_THREADS=1` definidos em `src/main.rs` antes da inicialização de qualquer runtime.
- A flag `--skip-extraction` agora exibe help string documentando que desabilita a extração automática de entidades/relacionamentos; o campo previamente dormente é reutilizado como toggle visível ao usuário.

### Corrigido
- `recall` agora reporta `DB inexistente` de forma consistente com os demais subcomandos via helper compartilhado `erros::banco_nao_encontrado` (P1-A).
- `recall --min-distance` foi renomeado para `--max-distance` mantendo `min-distance` como alias legado para compatibilidade (P2-K).
- `related ''` rejeita strings vazias com erro de validação claro em vez de produzir zero resultados silenciosamente (P2-L).
- 15+ strings voltadas ao usuário em `embedder.rs`, `daemon.rs`, `paths.rs`, `tokenizer.rs` e `commands/remember.rs` agora exibem traduções em português junto aos originais em inglês (P2-I).
- `--name` é auto-normalizado para kebab-case com `tracing::warn!` quando snake_case ou CapsName são detectados (P2-H).
- Flags ocultas `--body-file`, `--entities-file`, `--relationships-file`, `--graph-stdin`, `--metadata-file` agora expõem `#[arg(help = ...)]` para aparecer no `--help` (P2-G).
- `stats.memories`, `list.items` e `health.counts.memories` foram unificados sob a chave `memories_total` em todos os outputs JSON (P3-E).
- `HybridSearchItem.rrf_score: Option<f64>` agora é populado com o score real de reciprocal-rank-fusion em vez de retornar sempre `null` (P3-F).
- Rejeição de `--tz` agora sugere fusos horários IANA válidos na mensagem de erro (P3-A).

## [1.0.18] - 2026-04-26

### Adicionado
- Novo helper `parent_or_err` em `src/paths.rs` e quatro testes unitários protegem contra paths malformados vindos de `--db /` ou de `SQLITE_GRAPHRAG_DB_PATH` vazio.
- Novo `DaemonSpawnGuard` em `src/daemon.rs` remove o arquivo `daemon-spawn.lock` em encerramento gracioso e emite uma linha estruturada `tracing::info!` ao encerrar o daemon.
- Variável de ambiente `ORT_DISABLE_CPU_MEM_ARENA=1` agora é setada por padrão em `main.rs` antes do fastembed inicializar, complementando a mitigação existente de `with_arena_allocator(false)` contra crescimento descontrolado de RSS em payloads de shapes variáveis.
- README e `README.pt-BR.md` agora expõem quatro variáveis de ambiente `SQLITE_GRAPHRAG_*` adicionais na tabela de configuração em runtime: `DISPLAY_TZ`, `DAEMON_FORCE_AUTOSTART`, `DAEMON_DISABLE_AUTOSTART`, `DAEMON_CHILD`.
- README e `README.pt-BR.md` agora apresentam o cluster de quatro badges exigido pelas regras do projeto: crates.io, docs.rs, license, Contributor Covenant.

### Alterado
- `path.parent().unwrap()` removido de `src/paths.rs`, `src/daemon.rs::try_acquire_spawn_lock` e `src/daemon.rs::save_spawn_state`; os três call sites agora propagam erros de validação via `parent_or_err`.
- Tagline do README reescrita de um parágrafo de 36 palavras para um blockquote de 12 palavras em conformidade com a regra de documentação sobre tamanho de tagline; o parágrafo duplicado acima do blockquote foi removido.
- Snippets de instalação do README não fazem mais hard-code de `--version 1.0.17` em oito locais entre `README.md` e `README.pt-BR.md`; agora recomendam `cargo install sqlite-graphrag --locked` e linkam para `CHANGELOG.md` para o histórico de versões.

### Corrigido
- O CI agora fixa `cargo-nextest` em `0.9.114`, a release mais nova compatível com o MSRV Rust 1.88.
- Os testes Loom agora usam o gate local `sqlite_graphrag_loom` para evitar compilar dependências Tokio sob o `cfg(loom)` upstream.
- O JSON de relacionamentos de grafo agora aceita aliases `from`/`to` e relações com hífen, normalizando antes da gravação.
- Clippy no macOS e testes de concorrência no Windows agora tratam errno e contenção de lock de arquivo específicos da plataforma corretamente.
- A documentação de grafo e `related` agora reflete a superfície real da CLI e não afirma mais extração automática de entidades em ingestão body-only.

## [1.0.17] - 2026-04-26

### Alterado
- `remember` agora aceita payloads de body até `512000` bytes e até `512` chunks, com embeddings multi-chunk seriais para manter a memória limitada em corpora reais de documentação
- `remember --graph-stdin` agora aceita um objeto estrito de grafo com `body` opcional, `entities` e `relationships`, permitindo que um único payload stdin grave texto e grafo explícito

### Corrigido
- A migração de schema `V006__memory_body_limit` eleva o `CHECK` SQLite de `memories.body` para bancos existentes, mantendo o limite Rust e a constraint do banco alinhados
- `scripts/audit-remember-safely.sh` agora envolve cleanup do daemon, init, health e chamadas auditadas de `remember` com `/usr/bin/timeout -k 30 "${AUDIT_TIMEOUT_SECS:-1800}"`
- A documentação de testes agora recomenda comandos longos com timeout para reduzir risco de travamentos locais em runs slow, loom, heavy e audit

## [1.0.16] - 2026-04-26

### Corrigido
- `remember` agora cria e migra o banco padrão `./graphrag.sqlite` antes da escrita, evitando arquivos SQLite vazios e falhas `no such table` em diretórios novos
- `remember --graph-stdin --skip-extraction` agora persiste payloads explícitos de grafo em vez de descartar entidades e relacionamentos silenciosamente
- Falhas em payloads de grafo agora são validadas antes da escrita e memória, chunks, entidades e relacionamentos são persistidos de forma atômica, então input inválido não deixa memórias parciais
- O parser de input de grafo agora rejeita campos desconhecidos e valida `entity_type`, `relation` e `strength` antes de tocar no SQLite
- Docs para agentes, arquivos de contexto para LLMs, schemas e saída de `--help` agora refletem o contrato estrito de JSON via stdin/stdout
- `scripts/test-loom.sh` agora envolve execuções longas de loom com timeout configurável

## [1.0.15] - 2026-04-26

### Corrigido
- `remember --graph-stdin` agora rejeita JSON inválido em vez de persistir payloads malformados como corpos de memória
- `remember` e `edit` agora rejeitam fontes ambíguas de corpo, como `--body` explícito junto com `--body-stdin`
- O CRUD de grafo via `--graph-stdin` agora preserva valores declarados de `entity_type` quando relacionamentos referenciam entidades existentes no input
- `graph --json` agora domina formatos textuais como `--format dot`, `--format mermaid` e saída textual de stats
- `daemon` agora aceita as flags compartilhadas `--db` e `--json`, mantendo a mesma superfície determinística de flags para invocações por agentes

## [1.0.14] - 2026-04-25

### Corrigido
- A matriz oficial de release agora exclui `x86_64-apple-darwin` e `x86_64-unknown-linux-musl`, que a cadeia atual de dependências com `ort` não sustenta por binários ONNX Runtime pré-compilados nesta configuração do projeto
- O workflow de release não tenta mais montar um binário universal macOS a partir de um artefato Intel não suportado
- As docs de release e de compatibilidade agora descrevem apenas os targets que o projeto consegue publicar com consistência sem build custom do ONNX Runtime

## [1.0.13] - 2026-04-25

### Corrigido
- `x86_64-apple-darwin` agora compila em runner Intel macOS explícito, em vez de falhar num host Apple Silicon sem caminho compatível para os binários ORT pré-compilados desse target
- `x86_64-unknown-linux-musl` agora compila via `cross`, fornecendo o toolchain C++ musl exigido por `esaxx-rs`
- O contrato de runtime do ONNX dinâmico em ARM64 GNU e o requisito de runner Windows ARM64 agora ficam preservados na release candidata que validará a matriz completa

## [1.0.12] - 2026-04-25

### Corrigido
- `aarch64-unknown-linux-gnu` agora compila via estratégia target-specific de ONNX Runtime com `load-dynamic`, em vez de falhar na linkedição dos arquivos ORT pré-compilados
- O contrato de runtime de `libonnxruntime.so` no ARM64 GNU agora está documentado explicitamente nas docs de release e nas docs voltadas a agentes
- O workflow de release agora usa o runner oficial GitHub-hosted Windows ARM64 para `aarch64-pc-windows-msvc`, em vez de um runner x64 incompatível

## [1.0.11] - 2026-04-25

### Corrigido
- A cobertura de smoke da binária instalada agora inclui o contrato público de fallback para `./graphrag.sqlite` no diretório da invocação, fechando um ponto cego de auditoria de release
- Os testes de contrato agora exigem os wrappers atuais de `list` (`items`) e `related` (`results`) em vez de aceitar silenciosamente arrays root legados
- `graph traverse` e `graph stats` agora expõem apenas os formatos que realmente suportam, evitando help enganoso e invocações documentadas inválidas
- O texto de help de subcomandos menos centrais agora está consistentemente inglês-first em toda a superfície pública auditada da CLI
- `COOKBOOK`, `AGENTS`, `INTEGRATIONS`, a orientação de schemas e os exemplos de grafo/health agora estão alinhados aos payloads reais e às formas válidas de comando da binária

## [1.0.10] - 2026-04-24

### Alterado
- O `--help` da CLI agora é consistentemente inglês por padrão no output estático do clap, enquanto `--lang` continua controlando apenas mensagens humanas de runtime
- A documentação de release agora deixa explícitos o upgrade com `cargo install ... --force` e a verificação da versão ativa com `sqlite-graphrag --version`
- A documentação de testes agora separa a cobertura padrão do nextest das suítes críticas de contrato em `slow-tests`

### Adicionado
- Novo job de CI `slow-contracts` executa `doc_contract_integration` e `prd_compliance` com `--features slow-tests`
- `installed_binary_smoke` agora exige por padrão paridade de versão entre a binária instalada e o workspace atual, com escape hatch explícito para auditorias legadas deliberadas

## [1.0.9] - 2026-04-24

### Corrigido
- `--skip-memory-guard` agora desabilita auto-start do daemon por padrão, evitando que subprocessos de teste e auditoria deixem daemons residentes sem opt-in explícito
- O daemon agora se encerra quando seu diretório de controle desaparece, evitando que execuções baseadas em `TempDir` deixem processos órfãos
- `installed_binary_smoke` agora desabilita explicitamente o auto-start do daemon para a binária instalada
- `audit-remember-safely.sh` agora isola `SQLITE_GRAPHRAG_CACHE_DIR` e executa `daemon --stop` no encerramento, evitando vazamento de processos após auditorias

### Adicionado
- Novo teste de regressão do daemon provando que `--skip-memory-guard` não auto-sobe o daemon sem opt-in explícito
- Novo teste de regressão do daemon provando que o processo se encerra quando o diretório temporário de cache/controle desaparece

## [1.0.8] - 2026-04-24

### Adicionado
- Auto-start automático do daemon no primeiro comando pesado de embedding quando o socket do daemon está indisponível
- Serialização de spawn via lock file dedicado do daemon para evitar tempestade de processos
- Estado persistido de backoff de spawn do daemon para suprimir tentativas repetidas após falhas
- Novos testes do daemon cobrindo auto-start e restart automático após shutdown

### Alterado
- Comandos pesados agora tentam usar o daemon, sobem o processo sob demanda e fazem fallback local apenas quando backoff ou falha de spawn exigem isso
- `sqlite-graphrag daemon` continua disponível para gestão explícita em foreground, mas deixou de ser obrigatório no caminho comum

### Corrigido
- O maior gap remanescente do daemon na `v1.0.7` foi fechado: o daemon deixou de ser puramente opt-in

## [1.0.7] - 2026-04-24

### Corrigido
- A documentação de integrações não afirma mais que o projeto roda "sem daemons" agora que `sqlite-graphrag daemon` existe
- A documentação voltada a agentes agora descreve o reuso do daemon persistente nos comandos pesados em vez de um modelo puramente stateless
- HOW_TO_USE agora documenta `sqlite-graphrag daemon`, `--ping`, `--stop` e o caminho de fallback automático nos comandos pesados
- TESTING agora documenta a suíte de integração do daemon e o fluxo básico de recuperação do daemon

## [1.0.6] - 2026-04-24

### Adicionado
- Novo subcomando `daemon` para manter o modelo de embeddings carregado em um processo IPC persistente
- Novo protocolo JSON por socket local para `ping`, `shutdown`, `embed_passage`, `embed_query` e embeddings controlados de múltiplas passagens
- Nova suíte de testes de integração do daemon provando que `init`, `remember`, `recall` e `hybrid-search` incrementam o contador de embeddings do daemon quando ele está disponível
- Novo helper `scripts/audit-remember-safely.sh` para auditar binárias instaladas ou locais sob limites de memória via cgroup

### Alterado
- `init`, `remember`, `recall` e `hybrid-search` agora tentam usar o daemon persistente primeiro e fazem fallback para o caminho local atual quando o daemon não está disponível
- `remember` agora usa o tokenizer real de `multilingual-e5-small` antes do embedding, substituindo a aproximação anterior por caracteres no caminho quente
- O embedding multi-chunk em `remember` agora usa micro-batching controlado por orçamento de tokens preenchidos em vez de serialização cega de todos os chunks
- O help de `remember --type` agora deixa explícito que o campo se refere a `memories.type`, não ao `entity_type` do grafo

### Corrigido
- O script de auditoria segura do `remember` agora usa diretório temporário único por execução e valida o banco com `health` após `init`
- Entradas sintéticas densas em bytes, mas abaixo do guard de tamanho, deixaram de fragmentar artificialmente em falhas de 7 chunks na build local melhorada

## [1.0.5] - 2026-04-24

### Corrigido
- `chunking::Chunk` deixou de armazenar corpos owned dos chunks, então o `remember` multi-chunk evita duplicar o corpo inteiro em memória dentro de cada chunk
- A persistência dos chunks agora insere slices de texto diretamente a partir do body armazenado, em vez de alocar outra coleção intermediária owned
- A documentação pública agora descreve corretamente `1.0.4` como release publicada atual e `1.0.5` como próxima linha local
- `remember` agora emite instrumentação de memória por etapa e rejeita documentos que excedem o limite operacional explícito atual de multi-chunk antes de iniciar trabalho ONNX
- O limite operacional explícito de multi-chunk foi reduzido de 8 para 6 após a auditoria segura em cgroup mostrar OOM ainda presente em entradas moderadas com 7 chunks sob `MemoryMax=4G`
- `remember` agora também rejeita corpos multi-chunk densos acima de `4500` bytes antes de iniciar trabalho ONNX, com base na janela de OOM observada na auditoria segura em cgroup
- O embedder agora força `max_length = 512` explicitamente e desabilita a CPU memory arena do ONNX Runtime para reduzir retenção de memória entre inferências repetidas com shapes variáveis

### Causa Raiz
- O desenho anterior ainda duplicava o body por meio de `Vec<Chunk>` carregando `String` owned para cada chunk
- Essa duplicação ampliava a pressão do alocador exatamente no caminho multi-chunk já tensionado pela inferência ONNX
- A ausência de um guard operacional explícito também permitia que entradas Markdown moderadas alcançassem o caminho pesado de embedding multi-chunk sem parada de segurança antecipada
- A auditoria segura subsequente mostrou que até alguns documentos com 7 chunks permaneciam inseguros dentro de um cgroup de `4G`, justificando um teto temporário mais estrito
- A auditoria segura subsequente também mostrou que alguns documentos densos entre `4540` e `4792` bytes ainda disparavam OOM abaixo do teto por chunks, justificando um guard temporário adicional por tamanho do body
- A documentação oficial do ONNX Runtime confirma que `enable_cpu_mem_arena = true` é o padrão, que desligá-lo reduz consumo de memória e que o custo pode ser maior latência
- A API da crate `ort` também documenta que `memory_pattern` deve ser desabilitado quando o tamanho da entrada varia, o que combina com o caminho de `remember` sob inferência repetida e shapes efetivos variáveis
- A inspeção do `fastembed 5.13.2` mostrou que o caminho CPU não desabilita a CPU memory arena do ONNX Runtime por padrão e só desabilita `memory_pattern` automaticamente no caminho DirectML
- A inspeção dos metadados do tokenizer de `multilingual-e5-small` confirmou que o teto real do modelo é `512`, então forçar `max_length = 512` alinha o projeto ao modelo em vez de depender de um default genérico da biblioteca
- A retenção da arena CPU passa, portanto, a ser tratada como causa fortemente sustentada e tecnicamente coerente, mas ainda não como causa única completamente provada em todos os casos patológicos

## [1.0.4] - 2026-04-23

### Corrigido
- `remember` agora gera embeddings de corpos chunkados em modo serial e reutiliza os mesmos embeddings por chunk para agregação e persistência em `vec_chunks`, evitando o caminho de batch que travava com documentos Markdown reais
- `remember` agora evita uma `Vec<String>` extra com cópias dos textos dos chunks e também evita reconstruir uma `Vec<storage::chunks::Chunk>` intermediária antes da persistência dos chunks
- `remember` agora resolve checagens baratas de duplicação antes de qualquer trabalho de embedding e não clona mais o corpo completo desnecessariamente para `NewMemory`
- `namespace-detect` agora aceita `--db` como no-op para que o contrato público do comando fique alinhado com o restante da superfície da CLI
- A documentação pública e o texto do workflow de release agora refletem corretamente a linha publicada `1.0.3` e o contrato de grafo explícito
- O chunking agora usa uma heurística mais conservadora de chars por token e garante progresso seguro em UTF-8, reduzindo o risco de chunks patológicos em entradas Markdown reais

### Causa Raiz
- Markdown real com estrutura rica em parágrafos podia levar a progressão não monotônica dos chunks com a lógica antiga de overlap
- O caminho antigo de `remember` também duplicava pressão de memória ao clonar textos de chunks para uma `Vec<String>` dedicada e ao reconstruir structs de chunk com novas `String` owned antes da persistência
- O caminho antigo de `remember` também gastava trabalho de ONNX antes de resolver condições baratas de duplicação e ainda clonava o corpo completo para `NewMemory` antes do insert ou update
- A combinação aumentava a pressão do alocador e tornava o caminho pesado de embedding mais vulnerável a crescimento patológico de memória em entradas problemáticas

## [1.0.3] - 2026-04-23

### Corrigido
- Comandos pesados agora calculam concorrência segura dinamicamente a partir da memória disponível, número de CPUs e orçamento de RSS por task antes de adquirir slots da CLI
- `init`, `remember`, `recall` e `hybrid-search` agora emitem logs defensivos de progresso mostrando a carga pesada detectada e a concorrência segura calculada
- O runtime agora reduz `--max-concurrency` para o orçamento seguro de memória em comandos pesados, em vez de deixar a heurística documentada sem enforcement
- O orçamento de RSS usado pela heurística de concorrência agora é calibrado a partir de pico de RSS medido, em vez de uma estimativa histórica mais antiga

### Adicionado
- Cobertura unitária para classificação de comandos pesados e cálculo de concorrência segura

## [1.0.2] - 2026-04-23

### Adicionado
- Schemas formais de entrada para `remember --entities-file` e `remember --relationships-file`
- Contrato estável de entrada do grafo em `AGENT_PROTOCOL`, `AGENTS`, `HOW_TO_USE` e `llms-full.txt`
- Resumo curto do contrato de entrada do grafo em `llms.txt` e `llms.pt-BR.txt`

### Corrigido
- Títulos de `AGENTS` agora descrevem `--json` como universal e `--format json` como específico por comando
- A matriz de saída em `HOW_TO_USE` agora reflete a saída padrão real de `link`, `unlink` e `cleanup-orphans`
- A documentação pública não apresenta mais o projeto como pré-publicação

## [1.0.1] - 2026-04-23

### Corrigido
- `--format` foi restringido a `json` nos comandos que não implementam `text` ou `markdown`, evitando que help e parse prometam modos de saída inexistentes
- `hybrid-search` deixou de aceitar `text` ou `markdown` para falhar apenas em runtime; formatos não suportados agora são rejeitados pelo `clap` no parse dos argumentos
- Documentação e guias para agentes agora explicam que `--json` é a flag ampla de compatibilidade, enquanto `--format json` é específico por comando

### Adicionado
- A documentação de payload de `remember` agora explica que `--relationships-file` exige `strength` em `[0.0, 1.0]` e que esse campo é mapeado para `weight` nas saídas do grafo
- A documentação de payload de `remember` agora explica que `type` é aceito como alias de `entity_type`, mas os dois campos juntos são inválidos

## [1.0.0] - 2026-04-19

- Primeira release pública sob o nome `sqlite-graphrag`
- O conjunto de funcionalidades deriva do legado `neurographrag v2.3.0`

### Corrigido
- consulta SQL de graph entities agora usa o nome de coluna correto (NG-V220-01 CRITICAL)
- stats e health agora aceitam a flag --format json (NG-V220-02 HIGH)
- obrigatoriedade de --type no remember documentada em todos os exemplos (NV-005 HIGH)
- documentação de rename corrigida para --name/--new-name (NV-002)
- documentação de recall esclarece argumento posicional QUERY (NV-004)
- documentação de forget remove flag --yes inexistente (NV-001)
- documentação de list referencia campo items correto (NV-006)
- documentação de related referencia campo results correto (NV-010)
- MIGRATION.md agora documenta a transição de rename e o plano de release `v1.0.0`

### Adicionado
- flag obrigatória --relation de unlink documentada (NV-003)
- graph traverse --from espera nome de entidade documentado (NV-007)
- lista de valores restritos de entity_type documentada (NV-009)
- flag --format adicionada ao sync-safe-copy para controle de saída (NG-V220-04)

### Alterado
- __debug_schema esclarece semântica de user_version versus schema_version (NG-V220-03)
- flags globais de i18n documentadas como exclusivas do PT (GAP-I18N-02 LOW)

## [2.2.0] - 2026-04-19

### Corrigido
- G-017: alias `--to` de `sync-safe-copy` restaurado; `--destination` permanece canônico (regressão da v2.0.3)
- G-027: `PRAGMA user_version` agora definido como 49 após migrações refinery para corresponder à contagem de linhas de `refinery_schema_history`
- NG-08: subcomando `health` agora executa `PRAGMA integrity_check` antes das contagens de memórias/entidades para defesa em profundidade; saída ganha campos `journal_mode`, `wal_size_mb` e `checks[]`

### Adicionado
- NG-04: subcomando `graph entities` lista nós do grafo com filtro opcional `--type` e saída `--json`
- NG-06: flag `--format` adicionada ao `graph stats` para paridade com `graph traverse`
- NG-05: subcomando diagnóstico oculto `__debug_schema` documentado; emite campos `schema_version`, `user_version`, `objects` e `migrations`
- NG-03: todos os subcomandos agora aceitam tanto `--json` (forma curta) quanto `--format json` (forma explícita) produzindo saída idêntica

### Alterado
- NG-07: `link` e `unlink` esclarecidos para operar exclusivamente em entidades tipadas do grafo; tipos válidos documentados no `--help`

## [2.1.0] - 2026-04-19

### Corrigido
- G-001: `rename` agora emite `action: "renamed"` no JSON de saída (`src/commands/rename.rs`)
- G-002: ranks do `hybrid-search` agora começam em 1 atendendo restrição `minimum: 1` do schema
- G-003: `--expected-updated-at` agora aplica lock otimista via cláusula WHERE + verificação `changes()` (exit 3 em conflito)
- G-005: prefixo i18n `Error:` agora traduzido para `Erro:` em PT via `i18n::prefixo_erro()` em `main.rs`
- G-007: `health` retorna exit 10 quando `integrity_ok: false` via `AppError::Database` (emite JSON antes de retornar Err)
- G-013: `restore` agora encontra memórias soft-deleted (WHERE inclui `deleted_at IS NOT NULL`)
- G-018: `emit_progress()` agora usa `tracing::info!` respeitando `LOG_FORMAT=json`
- Receitas 8 e 14 do COOKBOOK corrigidas para usar `jaq '.items[]'` conforme estrutura de `list --json`
- Semântica de score no HOW_TO_USE pt-BR corrigida (`score` alto = mais relevante, não distância baixa)

### Adicionado
- G-004: Documentação dos valores válidos de `entity_type` em `--entities-file` (`project|tool|person|file|concept|incident|decision|memory|dashboard|issue_tracker`)
- G-006: `docs/MIGRATION.md` + `docs/MIGRATION.pt-BR.md` com guia de atualização v1.x para v2.x
- G-016: Subcomando `graph traverse` (flags `--from`/`--depth`) com novo schema `docs/schemas/graph-traverse.schema.json`
- G-016: Subcomando `graph stats` com novo schema `docs/schemas/graph-stats.schema.json`
- G-019/G-020: Flag global `--tz` + `tz::init()` em `main.rs` populando `FUSO_GLOBAL` para timestamps com fuso horário
- G-024: Flag `namespace-detect --db` para override de DB múltiplo
- G-025: Flags `vacuum --checkpoint` + `--format`
- G-026: Subcomando `migrate --status` com resposta `applied_migrations`
- G-027: `PRAGMA user_version = 49` definido após conclusão das migrations do refinery
- 6 novas seções H3 em HOW_TO_USE.pt-BR.md (Aliases de Flag de Idioma, Flag de Saída JSON, Descoberta de Caminho do DB, Limite de Concorrência, Nota sobre forget, Nota sobre optimize e migrate)
- Nova receita no COOKBOOK pt-BR: "Como Exibir Timestamps no Fuso Horário Local"

### Alterado
- `migrate.schema.json` agora usa `oneOf` cobrindo os modos run vs `--status` com `$defs.MigrationEntry`
- `--json` aceito como no-op em `remember`/`read`/`history`/`forget`/`purge` para consistência
- `docs/schemas/README.md` documenta convenção de nomenclatura `__debug_schema` (binário) vs kebab-case (arquivo de schema)

### Descontinuado
- `--allow-parallel` removida em v1.2.0 — consulte `docs/MIGRATION.md` para caminho de atualização


## [2.0.5] — 2026-04-19

### Corrigido
- Exit code 13 documentado como `BatchPartialFailure` e exit code 15 como `DbBusy` em AGENTS.md — separação correta conforme `src/errors.rs` desde v2.0.0
- Exit code 73 substituído por 75 (`LockBusy/AllSlotsFull`) em todas as referências de documentação
- `PURGE_RETENTION_DAYS` corrigido de 30 para 90 em AGENTS.md e HOW_TO_USE.md EN+pt-BR — alinhado à constante `PURGE_RETENTION_DAYS_DEFAULT = 90` em `src/constants.rs`

### Adicionado
- `elapsed_ms: u64` padronizado em todos os comandos que ainda não expunham o campo — uniformidade de contrato JSON
- `schema_version: u32` adicionado ao JSON stdout de `health` — facilita detecção de migração por agentes
- Subcomando oculto `__debug_schema` que imprime schema SQLite + versão de migrations para diagnóstico
- Diretório `docs/schemas/` com JSON Schema Draft 2020-12 público de cada resposta
- 12 suites de testes cobrindo: contrato JSON, exit codes P0, migração de schema, concorrência, property-based, sinais, i18n, segurança, benchmarks, smoke de instalado, receitas do cookbook e regressão v2.0.4
- 4 benchmarks criterion em `benches/cli_benchmarks.rs` validando SLAs de latência
- `proptest = { version = "1", features = ["std"] }` e `criterion = { version = "0.5", features = ["html_reports"] }` em `[dev-dependencies]`
- `[[bench]]` com `name = "cli_benchmarks"` e `harness = false` em `Cargo.toml`


## [2.0.4] — 2026-04-19

### Corrigido
- `--expected-updated-at` agora aceita tanto Unix epoch inteiro quanto string RFC 3339 via parser duplo em src/parsers/mod.rs — aplicado em edit, rename, restore, remember (GAP 1 CRITICAL)
- `entities-file` agora aceita o campo `"type"` como alias de `"entity_type"` via `#[serde(alias = "type")]` — elimina erro 422 em payloads válidos de agentes (GAP 12 HIGH)
- Mensagens internas de validação agora localizadas EN/PT via módulo `i18n::validacao` — 7 funções cobrindo comprimento do nome, nome reservado, kebab-case, comprimento de descrição, comprimento de body (GAP 13 MEDIUM)
- Flag `purge --yes` aceita silenciosamente como no-op para compatibilidade com exemplos documentados (GAP 19 MEDIUM)
- Resposta JSON de `link` agora duplica `from` como `source` e `to` como `target` — zero breaking change, adiciona aliases esperados (GAP 20 MEDIUM)
- Objetos de nó em `graph` agora duplicam `kind` como `type` via `#[serde(rename = "type")]` em graph_export.rs — zero breaking change (GAP 21 LOW)
- Registros de versão de `history` agora incluem campo `created_at_iso` RFC 3339 paralelo ao `created_at` Unix existente (GAP 24 LOW)

### Adicionado
- Schema JSON de `health` expandido conforme spec completa do PRD: +db_size_bytes, +integrity_ok, +schema_ok, +vec_memories_ok, +vec_entities_ok, +vec_chunks_ok, +fts_ok, +model_ok, +checks[] com 7 entradas (GAP 4 HIGH)
- Resposta JSON de `recall` agora inclui `elapsed_ms: u64` medido via Instant (GAP 8 HIGH)
- Resposta JSON de `hybrid-search` agora inclui `elapsed_ms: u64`, `rrf_k: u32` e `weights: {vec, fts}` (GAPs 8+10 HIGH)
- Módulo de validação i18n `src/i18n/validacao.rs` — todas as 7 mensagens de erro de validação disponíveis em EN e PT
- Parser de timestamp duplo `src/parsers/mod.rs` — aceita Unix epoch i64 e RFC 3339 via `chrono::DateTime::parse_from_rfc3339`

### Alterado
- Varredura de docs EN (T9): schemas de recall, hybrid-search, list, health, stats alinhados com saída real do binário; pesos corrigidos 0.6/0.4 → 1.0/1.0; namespace padrão documentado como `global`; alias `--json` no-op documentado; `related` documentado para receber nome da memória e não ID
- Varredura de docs PT (T10): COOKBOOK.pt-BR.md, CROSS_PLATFORM.pt-BR.md, AGENTS.pt-BR.md, README.pt-BR.md, skill/sqlite-graphrag-pt/SKILL.md, llms.pt-BR.txt alinhados espelhando as correções EN do T9
- 18 arquivos-fonte binário atualizados; 1 arquivo novo criado (src/parsers/mod.rs)
- 283 testes PASS, zero warnings de clippy, zero erros de check após alterações no binário


## [2.0.3] - 2026-04-19

### Adicionado
- `purge --days` aceito como alias de `--retention-days` para compatibilidade com docs (GAP 3)
- `recall --json` e `hybrid-search --json` aceitos como no-op (GAP 6) — saída JSON já é o padrão
- JSON de `health` agora inclui `wal_size_mb` e `journal_mode` (GAP 7)
- JSON de `stats` agora inclui `edges` (alias de `relationships`) e `avg_body_len` (GAP 8)
- Variantes de `AppError` agora localizadas via enum `Idioma` / match exaustivo de `Mensagem` (GAP 13) — `--lang en/pt` aplica-se também às mensagens de erro
- 8 novas seções em HOW_TO_USE.md para subcomandos sem documentação prévia (GAP 12): cleanup-orphans, edit, graph, history, namespace-detect, rename, restore, unlink
- Espelho bilíngue HOW_TO_USE.pt-BR.md
- Aviso de latência no COOKBOOK informando ~1s por invocação CLI vs planos do daemon (GAP P1)

### Alterado
- Toda a documentação: `--type agent` substituído por `--type project` (GAP 1) — PRD define 7 tipos válidos (user/feedback/project/reference/decision/incident/skill); `agent` nunca foi válido
- Toda a documentação: `purge --days` reescrito como `purge --retention-days` (GAP 3)
- Toda a documentação: exemplos de `remember` agora incluem `--description "..."` (GAP 2)
- README, CLAUDE, AGENT_PROTOCOL: contagem de agentes padronizada em 27 (GAP 14)
- Schemas AGENTS.md: raiz JSON de `recall` documentada como `direct_matches[]/graph_matches[]/results[]` (conforme PRD), `hybrid-search` como `results[]` com `vec_rank/fts_rank` (GAPs 4, 5)
- Padrões do COOKBOOK corrigidos: recall --k 10, list --limit 50, pesos hybrid-search 1.0/1.0, purge --retention-days 90 (GAPs 28-31)
- Nota em docs sobre `distance` (cosseno, menor=melhor) vs `score` (1-distance, maior=melhor) em JSON vs text/markdown (GAP 17)
- Nota em docs sobre namespace padrão `global` (não `default`) (GAP 16)

### Corrigido
- Binário não retorna mais exit 2 para `purge --days 30` (GAP 3)
- Binário não retorna mais exit 2 para `recall --json "q"` (GAP 6)
- Documentação de `link` agora explicita pré-requisito de entidade (GAP 9)
- Documentação da flag `--force-merge` (GAP 18)
- Documentação de `graph --format dot|mermaid` (GAP 22)
- Documentação da flag `--db <PATH>` (GAP 25)
- Documentação de `--max-concurrency` limitado a 2×nCPUs (GAP 27)

### Documentação
- `27 agentes de IA` padronizado como contagem oficial em todo o projeto
- Evidência: plano de testes de 2026-04-19 catalogou 31 gaps em `/tmp/sqlite-graphrag-testplan-v2.0.2/gaps.md`; v2.0.3 fecha todos os 31
- GAP 11 `elapsed_ms` universal em JSON adiado para v2.1.0 (requer captura de processing_time em todos os comandos)
- GAP P1 latência < 50ms requer modo daemon planejado para v3.0.0


## [2.0.2] - 2026-04-19

### Corrigido

- Flag `--lang` agora aceita os códigos curtos `en`/`pt` conforme documentado.
- Antes exigia identificadores completos `english`/`portugues`; aliases adicionados: `en/english/EN`, `pt/portugues/portuguese/pt-BR/pt-br/PT`.


## [2.0.1] - 2026-04-19

### Adicionado

- Aliases de flags para compatibilidade retroativa com a documentação bilíngue.
- `rename --old/--new` adicionados como aliases de `--name/--new-name`.
- `link/unlink --source/--target` adicionados como aliases de `--from/--to`.
- `related --hops` adicionado como alias de `--max-hops`.
- `sync-safe-copy --output` adicionado como alias de `--dest`.
- `related` agora aceita o nome da memória como argumento posicional.
- `--json` aceito como no-op em `health`, `stats`, `migrate`, `namespace-detect`.
- Flag global `--lang en|pt` com fallback via env var `SQLITE_GRAPHRAG_LANG`.
- Fallback de locale `LC_ALL`/`LANG` usado para mensagens de progresso no stderr.
- Novo módulo `i18n` com enum `Language` e helpers `init`/`current`/`tr`.
- Helpers bilíngues adicionados em `output::emit_progress_i18n`.
- Timestamps ISO 8601: `created_at_iso` adicionado em `RememberResponse`.
- `updated_at_iso` adicionado em itens de `list`.
- `created_at_iso`/`updated_at_iso` adicionados em `read`, paralelos aos inteiros epoch existentes.
- Resposta `read` agora inclui `memory_id` (alias de `id`).
- Resposta `read` agora inclui `type` (alias de `memory_type`).
- Resposta `read` agora inclui `version` para controle otimista.
- Itens `hybrid-search` agora incluem `score` (alias de `combined_score`).
- Itens `hybrid-search` agora incluem `source: "hybrid"`.
- Itens `list` agora incluem `memory_id` (alias de `id`).
- Resposta `stats` agora inclui `memories_total`, `entities_total`, `relationships_total`.
- Resposta `stats` agora inclui `chunks_total`, `db_bytes` para conformidade com contrato.
- Resposta `health` agora inclui `schema_version` no topo conforme PRD.
- Resposta `health` agora inclui `missing_entities[]` conforme PRD.
- `RememberResponse` inclui `operation` (alias de `action`), `created_at`, `created_at_iso`.
- `RecallResponse` inclui `results[]` com merge de `direct_matches` e `graph_matches`.
- Flag `init --namespace` adicionada, resolvida e ecoada em `InitResponse.namespace`.
- Flag `recall --min-distance <float>` adicionada (default 1.0, desativada por padrão).
- Quando `--min-distance` abaixo de 1.0, retorna exit 4 se todos os hits excederem o threshold.

### Corrigido

- Arquivos DB criados por `open_rw` agora recebem chmod 600 em Unix.
- Arquivos de snapshot criados por `sync-safe-copy` agora recebem chmod 600 em Unix.
- Previne vazamento de credenciais em montagens compartilhadas (Dropbox, NFS, `/tmp` multi-usuário).
- Mensagens de progresso em `remember`, `recall`, `hybrid-search`, `init` usam helper bilíngue.
- Idioma agora respeitado de forma consistente (antes misturava EN/PT na mesma sessão).

### Documentação

- COOKBOOK, AGENT_PROTOCOL, SKILL, CLAUDE.md atualizados para refletir schemas e flags reais.
- README, INTEGRATIONS e llms.txt atualizados para refletir exit codes reais.
- Validados contra o output de `--help` de cada subcomando.
- Subcomandos `graph` e `cleanup-orphans` agora documentados nos guias apropriados.
- Disclaimer honesto de latência adicionado: recall e hybrid-search levam ~1s por invocação.
- Latência de ~8ms requer daemon (planejado para v3.0.0 Tier 4).


## [2.0.0] - 2026-04-18

### Breaking

- EXIT CODE: `DbBusy` movido de 13 para 15 para liberar exit 13 para `BatchPartialFailure`.
- Scripts shell que detectavam `EX_UNAVAILABLE` (13) como DB busy agora devem checar 15.
- HYBRID-SEARCH: formato JSON da resposta remodelado; formato antigo era `{query, combined_rank[], vec_rank[], fts_rank[]}`.
- Novo formato: `{query, k, results: [{memory_id, name, namespace, type, description, body, combined_score, vec_rank?, fts_rank?}], graph_matches: []}`.
- Consumidores que parseavam `combined_rank` devem migrar para `results` conforme PRD linhas 771-787.
- PURGE: `--older-than-seconds` descontinuada em favor de `--retention-days`.
- A flag antiga permanece como alias oculto mas emite warning; será removida em v3.0.0.
- NAME SLUG: `NAME_SLUG_REGEX` mais estrita que `SLUG_REGEX` da v1.x.
- Nomes multichar devem agora começar com letra (requisito do PRD).
- Single-char `[a-z0-9]` ainda permitido; memórias existentes com dígito inicial passam inalteradas.
- `rename` para nomes estilo legado (dígito inicial, multichar) agora falhará.

### Adicionado

- `AppError::BatchPartialFailure { total, failed }` mapeando para exit 13.
- Reservado para `import`, `reindex` e batch stdin (entrando em Tier 3/4).
- Constantes em `src/constants.rs`: `PURGE_RETENTION_DAYS_DEFAULT=90`, `MAX_NAMESPACES_ACTIVE=100`.
- Constantes: `EMBEDDING_MAX_TOKENS=512`, `K_GRAPH_MATCHES_LIMIT=20`, `K_LIST_DEFAULT_LIMIT=100`.
- Constantes: `K_GRAPH_ENTITIES_DEFAULT_LIMIT=50`, `K_RELATED_DEFAULT_LIMIT=10`, `K_HISTORY_DEFAULT_LIMIT=20`.
- Constantes: `WEIGHT_VEC_DEFAULT=1.0`, `WEIGHT_FTS_DEFAULT=1.0`, `TEXT_BODY_PREVIEW_LEN=200`.
- Constantes: `ORT_NUM_THREADS_DEFAULT="1"`, `ORT_INTRA_OP_NUM_THREADS_DEFAULT="1"`, `OMP_NUM_THREADS_DEFAULT="1"`.
- Constantes: `BATCH_PARTIAL_FAILURE_EXIT_CODE=13`, `DB_BUSY_EXIT_CODE=15`.
- Flag `--dry-run` e `--retention-days` em `purge`.
- Campos `namespace` e `merged_into_memory_id: Option<i64>` em `RememberResponse`.
- Campo `k: usize` em `RecallResponse`.
- Campos `bytes_freed: i64`, `oldest_deleted_at: Option<i64>` em `PurgeResponse`.
- Campos `retention_days_used: u32`, `dry_run: bool` em `PurgeResponse`.
- Flag `--format` em `hybrid-search` (apenas JSON; text/markdown reservados para Tier 2).
- Flag `--expected-updated-at` (optimistic locking) em `rename` e `restore`.
- Guard de limite de namespaces ativos (`MAX_NAMESPACES_ACTIVE=100`) em `remember`.
- Retorna exit 5 quando o limite de namespaces ativos é excedido.

### Alterado

- `SLUG_REGEX` renomeada para `NAME_SLUG_REGEX` com valor conforme PRD.
- Novo padrão: `r"^[a-z][a-z0-9-]{0,78}[a-z0-9]$|^[a-z0-9]$"`.
- Nomes multichar devem começar com letra.

### Corrigido

- Prefixo `__` explicitamente rejeitado em `rename` (antes apenas aplicado em `remember`).
- Constantes `WEIGHT_VEC_DEFAULT`, `WEIGHT_FTS_DEFAULT` agora declaradas em `constants.rs`.
- Referências do PRD agora mapeiam símbolos reais.


## [1.2.1] - 2026-04-18

### Corrigido

- Falha de instalação em versões de `rustc` no intervalo `1.88..1.95`.
- Causada pela dependência transitiva `constant_time_eq 0.4.3` (puxada via `blake3`).
- Essa dependência elevou seu MSRV para 1.95.0 em uma patch release.
- `cargo install sqlite-graphrag` sem `--locked` agora sucede.
- Pin direto `constant_time_eq = "=0.4.2"` força versão compatível com `rust-version = "1.88"`.

### Alterado

- `Cargo.toml` agora declara pin preventivo explícito `constant_time_eq = "=0.4.2"`.
- Comentário inline documenta a razão do drift de MSRV.
- Pin será revisitado quando `rust-version` for elevado para 1.95.
- Instruções de instalação do `README.md` (EN e PT) atualizadas para `cargo install --locked sqlite-graphrag`.
- Bullet adicionado explicando a motivação para `--locked`.

### Adicionado

- Seção `docs_rules/prd.md` "Dependency MSRV Drift Protection" documenta o padrão canônico de mitigação.
- Padrão: pinagem direta de dependências transitivas problemáticas no `Cargo.toml` de nível superior.


## [1.2.0] - 2026-04-18

### Adicionado

- Semáforo de contagem cross-process com até 4 slots simultâneos via `src/lock.rs` (`acquire_cli_slot`).
- Memory guard abortando com exit 77 quando RAM livre está abaixo de 2 GB via `sysinfo` (`src/memory_guard.rs`).
- Signal handler para SIGINT, SIGTERM e SIGHUP via `ctrlc` com feature `termination`.
- Flag `--max-concurrency <N>` para controlar limite de invocações paralelas em runtime.
- Flag oculta `--skip-memory-guard` para testes automatizados onde a alocação real não ocorre.
- Constantes `MAX_CONCURRENT_CLI_INSTANCES`, `MIN_AVAILABLE_MEMORY_MB`, `CLI_LOCK_DEFAULT_WAIT_SECS` em `src/constants.rs`.
- Constantes `EMBEDDING_LOAD_EXPECTED_RSS_MB` e `LOW_MEMORY_EXIT_CODE` em `src/constants.rs`.
- Variantes `AppError::AllSlotsFull` e `AppError::LowMemory` com mensagens em português brasileiro.
- Global `SHUTDOWN: AtomicBool` e função `shutdown_requested()` em `src/lib.rs`.

### Alterado

- Default da flag `--wait-lock` aumentado para 300 segundos (5 minutos) via `CLI_LOCK_DEFAULT_WAIT_SECS`.
- Lock file migrado de `cli.lock` único para `cli-slot-{N}.lock` (semáforo de contagem N=1..4).

### Removido

- BREAKING: flag `--allow-parallel` removida; causou OOM crítico em produção (incidente 2026-04-18).

### Corrigido

- Bug crítico onde invocações CLI paralelas esgotavam a RAM do sistema.
- 58 invocações simultâneas travaram o computador por 38 minutos (incidente 2026-04-18).


## [Legacy NeuroGraphRAG]
<!-- Bloco anterior ao rename para sqlite-graphrag, preservado para rastreabilidade -->

### Adicionado

- Flags globais `--allow-parallel` e `--wait-lock SECONDS` para concorrência controlada.
- Módulo `src/lock.rs` implementando lock single-instance baseado em arquivo via `fs4`.
- Nova variante `AppError::LockBusy` mapeando para exit code 75 (`EX_TEMPFAIL`).
- Variáveis de ambiente `ORT_NUM_THREADS`, `OMP_NUM_THREADS` e `ORT_INTRA_OP_NUM_THREADS` pré-definidas para 1.
- Singleton `OnceLock<Mutex<TextEmbedding>>` para reuso do modelo intra-processo.
- Testes de integração em `tests/lock_integration.rs` cobrindo aquisição e liberação de lock.
- `.cargo/config.toml` com `RUST_TEST_THREADS` conservador padrão e aliases cargo padronizados.
- `.config/nextest.toml` com profiles `default`, `ci`, `heavy` e override `threads-required` para loom e stress.
- `scripts/test-loom.sh` como invocação canônica local com `RUSTFLAGS="--cfg loom"`.
- `docs/TESTING.md` e `docs/TESTING.pt-BR.md` guia bilíngue de testes.
- Feature Cargo `slow-tests` para futuros testes pesados opt-in.

### Alterado

- Comportamento padrão agora é single-instance.
- Uma segunda invocação concorrente sai com código 75 exceto se `--allow-parallel` for passada.
- Módulo embedder refatorado de struct-com-estado para funções livres operando sobre um singleton.
- Mover `loom = "0.7"` para `[target.'cfg(loom)'.dev-dependencies]` — ignorado em cargo test padrão.
- Remover feature Cargo legada `loom-tests` substituída pelo gate oficial `#[cfg(loom)]`.
- Workflow CI `ci.yml` migrado para `cargo nextest run --profile ci` com `RUST_TEST_THREADS` explícito por job.
- Job CI loom agora exporta `LOOM_MAX_PREEMPTIONS=2`, `LOOM_MAX_BRANCHES=500`, `RUST_TEST_THREADS=1`, `--release`.

### Corrigido

- Previne OOM livelock quando a CLI é invocada em paralelismo massivo por orquestradores LLM.
- Previne livelock térmico nos testes loom ao alinhar gate `#[cfg(loom)]` com padrão upstream.
- Serializa `tests/loom_lock_slots.rs` com `#[serial(loom_model)]` para impedir execução paralela dos modelos loom.


## [0.1.0] - 2026-04-17

### Adicionado

- Fase 1: Fundação: schema SQLite com vec0 (sqlite-vec), FTS5, grafo de entidades.
- Fase 2: Subcomandos essenciais: init, remember, recall, read, list, forget, rename, edit, history.
- Fase 2 continuação: restore, health, stats, optimize, purge, vacuum, migrate, hybrid-search.
- Fase 2 continuação: namespace-detect, sync-safe-copy.

### Corrigido

- Bug de corrupção FTS5 external-content no ciclo forget+purge.
- Removido DELETE manual em forget.rs que causava a corrupção.

### Alterado

- MSRV elevado de 1.80 para 1.88 (exigido por dependências transitivas base64ct 1.8.3, ort-sys, time).

- Os links históricos abaixo continuam apontando para o repositório legado `neurographrag`
- O projeto renomeado inicia sua linha pública de versões em `sqlite-graphrag v1.0.0`

[Unreleased]: https://github.com/danilo-aguiar-br/neurographrag/compare/v2.3.0...HEAD
[2.1.0]: https://github.com/danilo-aguiar-br/neurographrag/releases/tag/v2.1.0
[2.0.2]: https://github.com/danilo-aguiar-br/neurographrag/releases/tag/v2.0.2
[2.0.1]: https://github.com/danilo-aguiar-br/neurographrag/releases/tag/v2.0.1
[2.0.0]: https://github.com/danilo-aguiar-br/neurographrag/releases/tag/v2.0.0
[1.2.1]: https://github.com/danilo-aguiar-br/neurographrag/releases/tag/v1.2.1
[1.2.0]: https://github.com/danilo-aguiar-br/neurographrag/releases/tag/v1.2.0
[0.1.0]: https://github.com/danilo-aguiar-br/neurographrag/releases/tag/v0.1.0
