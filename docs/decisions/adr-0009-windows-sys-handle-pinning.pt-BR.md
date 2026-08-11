# ADR-009: Pin Exato de windows-sys 0.59.0 para Estabilidade do Tipo HANDLE

## Status
- Aceito (2026-06-03, v1.0.68)

## Contexto
- O crate `windows-sys` mudou o tipo de `HANDLE` entre as versões 0.48/0.52 (`isize`) e 0.59+ (`*mut c_void`), conforme documentado em [microsoft/windows-rs#171].
- A v1.0.66 introduziu `src/terminal.rs` com a expressão `handle != 0 && handle as isize != -1` — uma checagem que só compila quando `HANDLE = isize`.
- A v1.0.67 foi publicada com esse código, mas a resolução de `windows-sys` a partir de `Cargo.toml:111` (`version = "0.59"`) retornou `windows-sys 0.59.0`, onde `HANDLE = *mut c_void`.  Isso fez `cargo install sqlite-graphrag` no Windows falhar com `error[E0308]: mismatched types` em `src/terminal.rs:29:26`.
- A matriz de CI em `windows-latest` não pegou isso porque o passo `cargo check` do binário roda no SO do runner, mas o runner é Ubuntu (a entrada de matriz "windows-latest" se aplica aos jobs `clippy` e `test`, não a um check dedicado de cross-compile).  Veja [.github/workflows/ci.yml] para a matriz; o job `clippy` (linha 24) e o job `test` (linha 39) têm `os: [ubuntu-latest, macos-latest, windows-latest]`, mas o `cargo check` interno não passa `--target x86_64-pc-windows-msvc`.

## Decisão
### Correção de Código
- Substituir o `handle != 0 && handle as isize != -1` não portável em `src/terminal.rs:29` pelo idioma type-safe:
  ```rust
  use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
  // ...
  let handle: HANDLE = GetStdHandle(handle_id);
  if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
      // ...
  }
  ```
- Esse idioma funciona para ambas as eras de tipo (`isize` e `*mut c_void`) e também captura o sentinela distinto `INVALID_HANDLE_VALUE` (`(HANDLE)-1`), que é diferente de NULL (`(HANDLE)0`).

### Pin de Dependência
- Fixar `windows-sys` em `=0.59.0` exato em `Cargo.toml:111`:
  ```toml
  [target.'cfg(windows)'.dependencies]
  windows-sys = { version = "=0.59.0", features = ["Win32_System_Console"] }
  ```
- Pin exato (`=`) em vez de caret (`^`) porque versões patch futuras da linha 0.59.x poderiam regredir novamente no contrato de tipo.  O usuário deve subir manualmente para 0.59.x ou 0.60+ com revisão de código.
- O comentário em `Cargo.toml:111` documenta a razão do pin explicitamente para que um mantenedor futuro não afrouxe "prestativamente" a restrição de versão.

### Gate de CI
- Novo job `windows-build-check` em `.github/workflows/ci.yml`:
  ```yaml
  windows-build-check:
    name: Windows MSVC cross-compile (G29)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-msvc
      - uses: Swatinem/rust-cache@v2
      - run: timeout 600 cargo check --target x86_64-pc-windows-msvc --lib --all-features
  ```
- Roda no Ubuntu (mais rápido que o runner Windows) instalando o target `x86_64-pc-windows-msvc` via `rustup target add`.  Não é preciso o linker `lib.exe` do Windows porque `cargo check` é somente de tipos.
- Custo: ~US$ 0,024-0,040 por build × ~50 PRs/mês = ~US$ 1-2/mês no GitHub Actions.  Justificado.

### Teste de Regressão
- Novo teste de integração `tests/terminal_compile_windows.rs` que:
  - Em TODAS as plataformas: confirma que `terminal::init_console` e `should_use_ansi` são chamáveis de fora do crate
  - No Windows: adicionalmente referencia a checagem type-safe `HANDLE.is_null() + INVALID_HANDLE_VALUE` para garantir que o build ainda compila
- O job de CI `windows-build-check` é o gate canônico de regressão; o teste de integração é a sonda local de sanidade pré-publicação.

## Consequências
- A v1.0.68 é a primeira release desde a v1.0.65 que compila no Windows via `cargo install`.
- Um usuário atualizando da v1.0.66 ou v1.0.67 no Windows obtém build bem-sucedido sem patch manual.
- Bumps futuros de versão de `windows-sys` exigem um commit deliberado que atualize tanto o contrato de tipo quanto o pin em `Cargo.toml`.
- O job `windows-build-check` adiciona ~3-5 minutos à matriz de CI, mas captura regressões cross-platform antes da publicação.

## Alternativas Consideradas
- **Rebaixar para `windows-sys = "0.52"`** — lá `HANDLE = isize`, então o código original compila.  Rejeitado porque a 0.52 está 7 versões atrás e perde correções e adições de recursos das 0.53-0.58.
- **Migrar para `windows = "0.58"` (crate de alto nível)** — fornece wrappers type-safe e métodos `is_invalid()`.  Rejeitado porque exige refatoração completa dos módulos `terminal.rs` e `claude_runner.rs`, aumenta o tempo de build em ~30% e adiciona pegada significativa de dependências transitivas.
- **Usar `unsafe { transmute }` para forçar a conversão do handle para `isize`** — funciona para ambas as eras de tipo, mas é semanticamente errado (handle é ponteiro, não inteiro).  Rejeitado conforme a política `rules-unsafe-ffi-pointers-nonnull-aliasing-volatile`.

## Referências
- Relatório de gap: `gaps.md#G29`
- Verificação do contrato de tipo: `https://docs.rs/windows-sys/0.59.0/windows_sys/Win32/Foundation/type.HANDLE.html` (atual) e `https://docs.rs/windows-sys/0.52.0/windows_sys/Win32/Foundation/type.HANDLE.html` (legado)
- Issue histórica: `https://github.com/microsoft/windows-rs/issues/171` (a troca do tipo HANDLE)
- Implementação: `src/terminal.rs:1-54`, `Cargo.toml:111`, `.github/workflows/ci.yml:122-137`, `tests/terminal_compile_windows.rs`
- Documentação: `docs/CROSS_PLATFORM.md#handle-type-and-the-windows-sys-0.59-boundary-g29-v1.0.68`, `docs/AGENTS.md#new-in-v1.0.68`
