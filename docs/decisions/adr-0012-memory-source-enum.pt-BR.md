# ADR-0012 — MemorySource Enum Tipado (v1.0.69)

- **Status.** Aceito.
- **Data.** 2026-06-05.
- **Decisores.** Alice Martins (operador), Claude Code (consultor).
- **Supersede.** Nenhum.
- **Gaps relacionados.** G29 (violação de CHECK constraint), G29 Passo 2 (runtime guard).

## Contexto

A tabela `memories` no SQLite tem uma CHECK constraint: `source TEXT NOT NULL DEFAULT 'agent' CHECK(source IN ('agent','user','system','import','sync'))`. A struct Rust `NewMemory` declarava `pub source: String`, permitindo qualquer string em nível de tipo. A CHECK constraint era a única linha de defesa, e `enrich.rs:902` introduziu um literal `source: "enrich".to_string()` que quebrou o contrato — toda invocação de `enrich --operation body-enrich` falhava com `SQLITE_CONSTRAINT_CHECK`.

O hotfix trocou o literal para `"agent"`, mas a fragilidade subjacente permaneceu: oito call-sites (`remember`, `rename`, `ingest`, `ingest_claude`, `ingest_codex`, `remember_batch`, `enrich`, `edit`) todos usavam literais `String`, e uma refatoração futura poderia reintroduzir o mesmo bug.

## Decisão

1. Criar `src/memory_source.rs` com um enum `MemorySource` (`Agent`, `User`, `System`, `Import`, `Sync`) implementando `as_str`, `Display`, `TryFrom<&str>`, `Serialize` e `Deserialize`. Oito testes unitários cobrem os caminhos válido/inválido/vazio/display/serialização.
2. Adicionar `pub fn validate_source(raw: &str) -> Result<&'static str, AppError>` como guard de runtime. Ele é chamado a partir de `memories::insert` e `memories::update`, oferecendo defesa em profundidade mesmo quando os call-sites ainda usam `String`.
3. Os call-sites existentes seguem usando `String` por compatibilidade binária (nenhuma migração necessária). O enum é a fundação para a migração de schema da v1.0.70 que substituirá o campo `String` pelo tipo enum.
4. O guard de runtime é OBSERVÁVEL no changelog e se comporta de forma idêntica à checagem em nível de tipo: um `source` inválido retorna `AppError::Validation` listando os valores aceitos.

## Consequências

- A CHECK constraint não pode mais ser violada pelos caminhos de código documentados. Qualquer call-site futuro que use um literal fora do conjunto de cinco valores falhará em tempo de compilação ou de execução.
- A migração de `String` para `MemorySource` em `NewMemory` fica adiada para a v1.0.70 para manter a v1.0.69 como mudança sem quebra.
- 8 testes unitários são adicionados; o guard de runtime adiciona mais 4 testes. Total +12 testes.
- O enum é a superfície de API pública para a migração da v1.0.70 e é exportado de `src/lib.rs`.

## Alternativas Consideradas

- Substituir o campo `String` pelo enum já na v1.0.69. REJEITADA. A mudança quebraria todo call-site que constrói `NewMemory` (8 arquivos). Uma release de migração (v1.0.70) deve entregar a mudança quebrante com um guia de upgrade claro.
- Descartar o guard de runtime e confiar somente no enum em nível de tipo. REJEITADA. A migração ainda não foi feita, então o guard de runtime é a rede de segurança para os 8 call-sites `String` existentes.

## Referências

- `src/memory_source.rs` (enum + 8 testes + guard de runtime).
- `src/storage/memories.rs:180-195` (insert chama `validate_source`).
- `src/storage/memories.rs:212-260` (update chama `validate_source`).
- `src/lib.rs:179-181` (`pub mod memory_source`).
- `src/commands/enrich.rs:1227` (literal do hotfix `"agent"`).
- gaps.md G29 linhas 533-1038 (histórico completo).
