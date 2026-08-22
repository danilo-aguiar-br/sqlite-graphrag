# ADR-0069: v1.2.8 — A enforcement de foreign key é alternada em torno do runner de migrations, nunca dentro de uma migration

- Status: Aceito
- Data: 2026-08-18
- Release: v1.2.8 (crate `1.2.8`)
- Substitui: a convenção `PRAGMA foreign_keys = OFF` usada por V006, V008, V009, V010 e V013
- Substituído por: nenhum
- Relacionados: ADR-0068 (vocabulário aberto de entity_type), GAP-SG-140, GAP-SG-277


## Contexto

Cinco migrations abrem com `PRAGMA foreign_keys = OFF;`, seguindo o procedimento do SQLite "making other kinds of table schema changes", cujo primeiro passo é desativar a enforcement antes de reconstruir uma tabela.

Essa linha nunca surtiu efeito neste projeto.

1. `storage::connection::open_rw` aplica `apply_connection_pragmas`, que define `PRAGMA foreign_keys = ON`.
2. O refinery roda cada migration dentro da própria transação (`refinery-core::drivers::rusqlite`); `set_grouped` nunca é chamado, e o padrão dele é uma transação por migration.
3. O SQLite documenta `PRAGMA foreign_keys` como **"a no-op within a transaction"**.
4. O SQLite documenta `DROP TABLE` sob enforcement como executando um `DELETE FROM` implícito antes de dropar, o que dispara `ON DELETE CASCADE` em cada filha.

`entities` tem quatro filhas, todas com `ON DELETE CASCADE`: `relationships`, `memory_entities`, `entity_embeddings` e `entity_connect_seen`.

Medido em 2026-08-18, migrando uma cópia do banco deste workspace do schema 16 para o 17 por um `runner().run(conn)` sem proteção:

| tabela | antes | depois |
| --- | --- | --- |
| `entities` | 15 744 | 15 744 |
| `relationships` | 213 029 | **0** |

Código de saída 0. A migration reportou sucesso.

O defeito sobreviveu a nove migrations porque todo teste de migration existente inicializa um banco **vazio**, onde o cascade não tem nada para deletar. Só um banco populado paga o preço, e nenhum teste usava um.

O procedimento do SQLite também coloca o pragma no passo 1 e a transação no passo 2. Este projeto inverteu essa ordem, porque a transação é aberta pelo refinery e não pela migration.


## Decisão

Alternar a enforcement em Rust, em torno do runner, onde ela fica fora da transação do refinery e pode de fato surtir efeito.

`storage::connection::run_migrations_with_foreign_keys_off` é o ponto de entrada único:

- desativa a enforcement, roda as migrations e restaura a enforcement **mesmo quando uma migration falha**, para que uma execução malsucedida nunca devolva uma conexão que aceite órfãos em silêncio;
- e então roda `PRAGMA foreign_key_check` como **query**, falhando na primeira linha violadora.

Esse último ponto é uma correção em si. A `V010` já continha `PRAGMA foreign_key_check;`, mas ela rodava por `execute_batch`, que descarta o conjunto de resultados. O pragma reporta violações como linhas, nunca como erro, então batê-lo em lote não verifica nada.

Um arquivo de migration, portanto, não deve conter `PRAGMA foreign_keys`. Escrevê-lo ali promete uma proteção que o arquivo não consegue entregar.

Duas garantias adicionais acompanham a decisão:

- Um banco existente é copiado à parte antes de qualquer auto-migração, pela Online Backup API do SQLite e não por cópia de sistema de arquivos, porque o modo WAL faz do `.sqlite` sozinho algo incompleto. Um backup malsucedido aborta a migração.
- O portão de auto-migração parou de consultar `SCHEMA_USER_VERSION`, que é um marcador de identidade fixado em 50 e documentado como não mudando quando migrations são adicionadas. Um valor que nunca muda não pode sinalizar que algo novo está pendente. O portão agora lê `MAX(version)` de `refinery_schema_history`.


## Consequências

Cinco pontos de chamada executavam o runner diretamente e todos passam agora pelo ponto de entrada protegido: dois em `storage::connection`, dois em `commands::migrate` e um em `commands::init`. O quinto foi encontrado pelo gate e não por leitura, depois que quatro já haviam sido corrigidos e o trabalho parecia terminado.

`tests/migration_foreign_key_gate.rs` fecha os dois caminhos de volta: executar o runner fora do módulo dono, e adicionar `PRAGMA foreign_keys` a um arquivo de migration novo. Migrations históricas mantêm suas linhas inertes, porque reescrever uma migration já aplicada é justamente o que o GAP-SG-140 ainda está pagando.

`storage::connection::migration_cascade_tests` é o primeiro teste de migration deste projeto que insere linhas **antes** de migrar, e que afirma que a enforcement está genuinamente ON de antemão — sem essa afirmação o teste passaria vacuamente num ambiente onde ela estivesse desligada.
