# Cross-platform checklist (sqlite-graphrag v1.2.8)

Local-only validation (no GitHub Actions). The CLI must run on **Linux**, **macOS**, and **Windows**.

- Read the Portuguese version at [CROSS_PLATFORM.pt-BR.md](CROSS_PLATFORM.pt-BR.md)
- Back to [README.md](../README.md)

## Build matrix (run on each host)

```bash
# Linux (gnu)
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
scripts/e2e_offline_v120.sh

# Linux (musl, optional)
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl

# macOS
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings

# Windows (MSVC)
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
```

## Platform contracts

| Area | Requirement |
|------|-------------|
| Paths | XDG on Unix (`directories` crate); Windows known-folder equivalents |
| DB | SQLite file via `--db` or XDG `db.path` — never product env |
| Locks / slots | Filesystem locks under XDG runtime/cache |
| Line endings | Accept `\n` and `\r\n` on stdin NDJSON |
| Shell completions | `completions` subcommand: bash/zsh/fish/powershell/elvish |
| Console | UTF-8 + ANSI; honor OS `NO_COLOR` |

## Forbidden

- Product `SQLITE_GRAPHRAG_*` environment variables as a config channel — **forbidden**, and **not read** at runtime
- Hardcoded `/tmp`, `/home/...`, or drive-letter paths in production code
- Recreating `.github/workflows` CI in this project
- Remote telemetry / OTEL export

## Status

| Platform | Build | Unit tests | Offline E2E harness |
|----------|-------|------------|---------------------|
| Linux x86_64 | operator local host of record | host | `scripts/e2e_offline_v120.sh` |
| macOS | operator checklist | operator checklist | same script (bash) |
| Windows | operator checklist | operator checklist | adapt paths; use Git Bash or run checks manually |

## Operator notes v1.2.1 (all platforms)

- Config: CLI flag > XDG `config set` > default. Product `SQLITE_GRAPHRAG_*` env is not read at runtime.
- **DEFAULT_EMBEDDING_DIM=1024** (flag `--embedding-dim` / XDG `embedding.dim`; existing DBs keep `schema_meta.dim` until re-embed).
- **Enrich CAPA (v1.2.1, all platforms):** (1) claim isolation via `dequeue_next_pending` = **operation + namespace**; (2) `--until-empty` counts only this op+ns (`count_eligible_pending`); (3) `--force-redescribe` reopens skipped/done once per process (never dead); (4) re-embed zombie reconcile `reconcile_satisfied_reembed_pending` when `LENGTH(embedding)=dim*4`; (5) re-embed eligibility by BLOB LENGTH not dim column alone (CORRUPT BLOB re-eligible); (6) enqueue strips `entity:` prefix for lookup (bare ok; missing reject); (7) chunk enqueue validates target namespace (non-deleted memory); (8) CAPA-D compound "configuration file" markers only (no bare `%configuration file%` FP); (9) queue suite **38** tests OK — regressions `enqueue_candidate_accepts_entity_prefixed_reembed_key`, `dequeue_next_pending_isolates_by_namespace`. Schema stays **v16**; crate **1.2.1**; no main-DB migration.
- Enrich recovery: `enrich --list-skipped` / `enrich --requeue-skipped` for `skipped` / `preservation_failed` queue rows (no raw SQL).
- **GAP-SG-139:** host/XDG leaves (`config`, `slots`, `cache`, `completions`) accept `--db` as a documented **no-op** on every platform — agents may always pass `--db`.
- deep-research: `-o` and `--output` use atomwrite (tempfile same dir → fsync → rename) on Linux, macOS, and Windows; parent-dir fsync applies on Unix.
- entity-connect: fully implemented (persists relationships); first-scan timeout exits **1** (not singleton **75**).
- Recommended agent order: write → entity-descriptions (hot, optional `--enqueue-enrich`) → entity-connect (cold). Always pass `--namespace` on enrich drains.
- Offline gate of record is Linux host + `scripts/e2e_offline_v120.sh` **20/20** (canonical; historical wrapper `e2e_offline_v118.sh` / 16/16 superseded). macOS/Windows use the same checklist; do not claim three-OS harness validation without host evidence. Schema stays **v16** (sidecar CAPA only).
- Complete top-level CLI inventory (all 50 top-level verbs, `help` included, plus nested families): [HOW_TO_USE.md](HOW_TO_USE.md#complete-cli-command-inventory-v128) (mirrored in [COOKBOOK.md](COOKBOOK.md) / [HEADLESS_INVOCATION.md](HEADLESS_INVOCATION.md)).
- Portuguese version may retain historical narrative below the shared checklist; treat pre-v1.1.8 product-env tables as historical only.
- Portuguese: [CROSS_PLATFORM.pt-BR.md](CROSS_PLATFORM.pt-BR.md)

## Custom-provider env whitelist on Windows (v1.0.83+, historical)

- HISTORICAL ONLY. The subprocess LLM backends were REMOVED in v1.2.0; the CLI no longer spawns a child process, so no env whitelist is applied on any platform today. Valid backends are `openrouter` and `none`.
- The shared env-whitelist helper `src/spawn/env_whitelist.rs` used to expose a Windows-specific set through `PRESERVED_ENV_VARS_WINDOWS` behind `#[cfg(windows)]`: `LOCALAPPDATA`, `APPDATA`, `USERPROFILE`, `SystemRoot`, `COMSPEC`, `PATHEXT`, `HOMEPATH`, `HOMEDRIVE`
- The Windows set was applied in addition to the POSIX set; `apply_env_whitelist(cmd, false)` covered both through the second `#[cfg(windows)]` loop in the helper
- On Windows the custom-provider env vars `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`, `OTEL_EXPORTER_OTLP_ENDPOINT` flowed to the LLM subprocess identically to Linux/macOS
- `LockFileEx` is used by the slot semaphore (ADR-0039, v1.0.82) on Windows; that release added no new lock primitives
- The no-leak audit test `audit_no_token_leak_in_subprocess_stderr` ran on Linux only; the same assertion held on Windows by construction (env propagation was platform-agnostic in the helper)
- HISTORICAL (v1.0.83): the --strict-env-clear flag, REMOVED in v1.2.0 with the subprocess spawners, behaved identically on Windows; only `PATH` (or `Path` on Windows, which the helper normalized) was forwarded in strict mode. Product `SQLITE_GRAPHRAG_*` environment variables are FORBIDDEN as configuration and are not read at runtime — use CLI flags and XDG `config set`.
- See `docs/decisions/adr-0041-preserve-custom-provider-env.md` and `docs/COOKBOOK.md` for the historical recipe

## Operator notes v1.1.06 (all platforms)

- Official name **v1.1.06**; crate `1.1.6`; **no schema migration** (`CURRENT_SCHEMA_VERSION` stays at **16**). ADR-0066; suite `tests/v1106_entity_connect_scan_regression.rs`.
- Closes **GAP-ENTITY-CONNECT-SCAN-CARTESIAN**: candidates by **co-occurrence** in `memory_entities` + **hub × degree-0 island** fill (O(k); never a cartesian `entities × entities` with a global ORDER BY).
- Queue keys `pair:{id1}:{id2}` with `item_type=entity_pair`; the **drain resolves by primary key** with no per-item re-scan.
- The 120s soft ceiling + `--max-runtime` use `InterruptHandle` on the **first** scan on Linux/macOS/Windows → Timeout exit **1**. Orchestrators MUST NOT treat a scan timeout as singleton exit **75**.
- NDJSON: `scan_start` **before** the SQL (`operation`, `entities_in_namespace`, `backlog_degree0_proxy`) then `scan_meta` (`pairs_enqueued_this_scan`, `scan_elapsed_ms`) — dual backlog fields stay stable for hooks on every platform; do not equate them.
- `cross-domain-bridges` shares the **same** O(k) path + `entity_connect_seen`; **GAP-002** convergence preserved.

## Operator notes v1.1.05 (all platforms)

- The official release name is v1.1.05; the crate manifest carries `version = "1.1.5"`; no schema migration (stays at v16). ADR: [ADR-0065](decisions/adr-0065-v1-1-05-incident-bugs.md). Regression suite: `tests/v1105_incident_bugs_regression.rs`.
- **Bug 1 (aspect fan-out)**: `deep-research` with a single token expands into multi-aspect sub-queries (`source: "aspect"`) on every OS; the optional manual path `--sub-query-strategy manual --sub-queries-file PATH` is path-separator safe (pass a normal filesystem path for the platform).
- **Bug 2 (atomwrite + ack)**: prefer `deep-research --output PATH` plus the global `--quiet`/`-q` so multi-MB envelopes are not truncated by shell redirection; never mix stderr into the JSON file with `&>`. Atomic JSON writes (`atomwrite`: tempfile in the same directory → fsync → rename) work on Linux, macOS, and Windows; parent-directory fsync applies on Unix. Ack fields on stdout: `written`, `bytes`, `blake3`, `sub_queries_total`, `unique_memories_found`, `elapsed_ms`.
- **Bug 4 (merge self-ref)**: on zsh/bash/PowerShell, prefer explicit arrays over unquoted word splitting when scripting `merge-entities` loops; the CLI rejects self-referential `--ids`/`--into-id` (or names) **before** any DB work on every target.
- `graph traverse --fuzzy` and `link --from-id`/`--to-id` behave identically on every target

## Architectural note v1.0.76 (historical)

- HISTORICAL ONLY. At v1.0.76 the default build was LLM-only and one-shot. There is no ONNX runtime to ship, no `libonnxruntime.so` to package, and no `multilingual-e5-small` model to download. Embedding generation delegated to a headless `claude code`, `codex`, or `opencode` subprocess (OAuth) spawned per call; from v1.0.90 opencode was the third backend with auto-detect priority `codex > claude > opencode > none`. From v1.0.95 `enrich --mode openrouter` also extracted entities through the OpenRouter REST `/chat/completions` endpoint with no local CLI required. **Those subprocess backends were REMOVED in v1.2.0**: the only valid backends today are `openrouter` and `none`, and the CLI spawns no child process.
- The `embedding-legacy` feature was REMOVED in v1.0.79 (ahead of the v1.1.0 schedule). Every build is LLM-only; the fastembed + ort + tokenizers pipeline and the ARM64 GNU ONNX contract no longer apply.
- The cross-platform table below describes the LLM-only build, which is now the only build.

## The pain you already know

### Before — a dependency hell that costs two hours

- Installing a Python RAG stack costs two hours between pip, venv, and C extensions
- Alpine containers break constantly with missing glibc symbols in Python wheels
- macOS Gatekeeper quarantines unsigned binaries and blocks the first run
- Windows path separators break shell scripts copied straight from Linux tutorials
- Different shells apply different quoting rules across Bash, Zsh, Fish, and PowerShell

### After — a single binary that just runs

- One `cargo install --locked` delivers the binary on any officially supported target
- No Python runtime, no Node runtime, no JVM, and a single shared-library contract on ARM64 GNU
- Binary startup stays under eighty milliseconds on every supported target
- Exit codes stay identical across the five published targets, guaranteeing reliable orchestration
- JSON output format is byte-for-byte identical on every operating system tested

### Bridge — the command that gets you there

```bash
cargo install --path .
```

## Support matrix

### Targets — five combinations we publish and test

| Target | Operating system | Architecture | Binary size | Startup |
| --- | --- | --- | --- | --- |
| x86_64-unknown-linux-gnu | Linux glibc | x86_64 | ~14.6 MiB | <50ms |
| aarch64-unknown-linux-gnu | Linux glibc | aarch64 | ~14.6 MiB | <60ms |
| aarch64-apple-darwin | macOS | Apple Silicon | ~14.6 MiB | <30ms |
| x86_64-pc-windows-msvc | Windows | x86_64 | ~14.6 MiB | <80ms |
| aarch64-pc-windows-msvc | Windows | ARM64 | ~14.6 MiB | <80ms |

- Each row above is built and verified locally by the operator on that host; this project has no remote pipeline
- Each row above gets its smoke checks run by hand from the build matrix at the top of this document
- A SHA256SUMS manifest accompanies each binary for immediate integrity verification
- Debug symbols ship as separate `.dSYM` or `.pdb` artifacts on demand
- Cross-compilation uses `cross` on Linux hosts for the `aarch64-unknown-linux-gnu` matrix cell

### Unsupported release targets — why they were excluded

- `x86_64-apple-darwin` was excluded because the v1.0.76 build no longer requires a prebuilt ONNX Runtime path (and macOS Intel has long been a deprecated macOS target since 2024)
- `x86_64-unknown-linux-musl` was excluded because no glibc-only native dependency remains in the default build, but a musl build is not part of the release matrix
- Reintroducing either target is a routine cross-compile task since v1.0.76 because no C extension needs to be linked

### ARM64 GNU — no more shared ONNX Runtime contract

- v1.0.76 has NO ONNX runtime dependency in the default build. The previous `aarch64-unknown-linux-gnu` contract (`libonnxruntime.so` next to the binary, `ORT_DYLIB_PATH` env var) is REMOVED.
- Historical note: builds with the removed `embedding-legacy` feature (v1.0.76-v1.0.78) shipped `libonnxruntime.so` on `aarch64-unknown-linux-gnu`. Since v1.0.79 no configuration needs the contract.
- The dynamic-loader contract was an artifact of the v1.0.74 fastembed pipeline. With the remote model as the model, the binary needs zero shared C libraries beyond libc

## Linux notes

### glibc first — the official Linux release path

- The glibc binary runs on Ubuntu 20.04, Debian 11, Fedora 36, and mainstream distros
- `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` are the only Linux assets published today
- `x86_64-unknown-linux-musl` has not been part of the official release matrix since `v1.0.16`
- With the remote-embedding build, no glibc version constraint exists beyond what the OS TLS stack requires

## macOS notes

### Gatekeeper — signing and notarization

- Unsigned binaries downloaded through a browser trigger quarantine on first run
- Remove quarantine with `xattr -d com.apple.quarantine /usr/local/bin/sqlite-graphrag`
- Binaries installed through `cargo install` bypass Gatekeeper because they come from the local rustc
- Official macOS assets currently cover Apple Silicon only

### Apple Silicon — native performance on M1 M2 M3 M4

- The native aarch64 binary runs thirty percent faster than x86_64 through Rosetta
- macOS Intel is currently outside the official release matrix in this project configuration
- The remote OpenRouter model is the model; the Rust binary itself loads no model
- The only LLM-side latency is the REST round-trip to OpenRouter per `remember` / `recall`
- Cold start measures twenty-eight milliseconds on M2 thanks to the improved branch predictor

## Windows notes

### Shell — PowerShell 7 and Windows Terminal

- PowerShell 7 or later runs every README example without any modification
- Windows Terminal renders colored output and progress bars identically to Unix shells
- Legacy CMD.EXE works but strips ANSI colors; product `SQLITE_GRAPHRAG_*` environment variables are FORBIDDEN as configuration and cannot re-enable them — use a modern terminal instead
- WSL2 users should prefer the Linux glibc binary for full Unix parity
- PowerShell ISE does NOT support the interactive prompts used during `init` confirmation

### UTF-8 console — the only adjustment needed

```powershell
chcp 65001
sqlite-graphrag remember --name "memoria-acentuada" --body "unicode characters work"
```

- Code page 65001 switches the console to UTF-8 encoding, rendering characters correctly
- Without UTF-8 the binary still works but stdout shows replacement characters on accents
- Modern Windows Terminal uses UTF-8 by default, removing the need for the `chcp` command
- Line endings stay LF inside the SQLite database regardless of console configuration
- Scripts persist correctly across Windows, Linux, and macOS when saved as UTF-8

### The HANDLE type and the windows-sys 0.59 boundary (G29, v1.0.68)

- The `windows-sys` crate changed the `HANDLE` type between 0.48/0.52 (`isize`) and 0.59+ (`*mut c_void`); the break was made by Microsoft in [windows-rs#171]
- `cargo install sqlite-graphrag` on Windows broke in v1.0.67 with `error[E0308]: mismatched types` at `src/terminal.rs:29:26` because the comparison `handle != 0 && handle as isize != -1` was only valid for the old type
- v1.0.68 replaces the comparison with the type-safe idiom `!handle.is_null() && handle != INVALID_HANDLE_VALUE`, which works for both type eras and also catches the `INVALID_HANDLE_VALUE` sentinel (`(HANDLE)-1`), which differs from NULL
- `windows-sys` is pinned to exactly `=0.59.0` in `Cargo.toml:111` to avoid silent resolution to a future 0.59.x that could break the type contract again
- Windows cross-compile regressions are caught locally by the operator with `cargo check --target x86_64-pc-windows-msvc --lib --all-features`; this project has no remote CI and none must be recreated
- Manual workaround for v1.0.66/v1.0.67 (only if you must stay on those versions): edit `~/.cargo/registry/src/index.crates.io-*/sqlite-graphrag-*/src/terminal.rs`, replace line 29 with `if !handle.is_null() && handle != INVALID_HANDLE_VALUE`, and add `INVALID_HANDLE_VALUE` to `use windows_sys::Win32::Foundation::{...}`.  Then run `cargo install --path .` from the fixed source.
- Reference: `https://docs.rs/windows-sys/0.59.0/windows_sys/Win32/Foundation/type.HANDLE.html` (current) and `https://docs.rs/windows-sys/0.52.0/windows_sys/Win32/Foundation/type.HANDLE.html` (legacy)

### Windows infrastructure resilience (G53-WINDOWS-INFRA, ADR-0033, v1.0.80, historical)

- HISTORICAL ONLY. The two hardening steps described by ADR-0033 targeted a hosted `windows-2025` matrix that no longer exists: this project has no CI, no GitHub Actions, and no workflows, and none must be recreated.
- The two historical infrastructure failure modes were (a) rustup downloads with transient network errors and (b) `E0463 can't find crate for core` when the target stdlib is missing; both are resolved on an operator host by re-running the install step
- Local cross-compile validation: `cargo check --target x86_64-pc-windows-msvc --lib --all-features` reproduced the `E0463` and it was resolved with `rustup target add x86_64-pc-windows-msvc --toolchain 1.88`; the build then reaches the `cc-rs: failed to find tool "lib.exe"` boundary, which is the expected MSVC cross-compile limit from a Linux host
- See ADR-0033 for the full rationale and boundary conditions

## Containers

### glibc images — the official path today

- Prefer Debian or Ubuntu base images for the current official Linux assets
- Alpine and pure musl images have not been part of the supported matrix since `v1.0.16`
- The musl container path requires a backend decision before it can be supported again

## Shell support

### Bash Zsh Fish PowerShell Nushell — all first class

```bash
# Bash and Zsh share identical syntax for every pipeline in this documentation
sqlite-graphrag recall "query" --json | jaq '.results[].name'
```

```fish
# Fish uses the same binary invocation with slightly different syntax for variables
sqlite-graphrag recall "query" --json | jaq '.results[].name'
```

```powershell
# PowerShell pipes objects natively but jaq still accepts raw JSON on stdin
sqlite-graphrag recall "query" --json | jaq '.results[].name'
```

```nu
# Nushell consumes JSON directly into structured tables with no external tooling
sqlite-graphrag recall "query" --json | from json | get results | select name
```

- Every shell above reads the same exit codes, guaranteeing identical orchestration semantics
- JSON output format is byte-identical across the five shells, simplifying automated pipelines
- Completion scripts are supported by the current CLI through `sqlite-graphrag completions <shell>`
- Configuration precedence is CLI flag > XDG `config set` > default in every shell; product `SQLITE_GRAPHRAG_*` environment variables are FORBIDDEN and are not read at runtime
- SIGINT and SIGTERM signals behave identically, enabling graceful shutdown universally

## Paths and XDG

### Paths — the directories crate resolves every operating system

- The default database path resolves to `./graphrag.sqlite` in the invocation directory
- macOS paths resolve to `~/Library/Application Support/sqlite-graphrag/` per the HIG
- Windows paths resolve to `%APPDATA%\sqlite-graphrag\` and `%LOCALAPPDATA%\sqlite-graphrag\`
- Override the database with the `--db` flag or XDG `config set db.path <PATH>` on every operating system; the CLI flag wins over XDG, which wins over the default

### Configuration — flags and XDG only

```bash
sqlite-graphrag config set db.path "/var/lib/graphrag.sqlite"
sqlite-graphrag config set i18n.lang "pt"
sqlite-graphrag config set log.level "debug"
sqlite-graphrag config list --effective --json
```

- Product `SQLITE_GRAPHRAG_*` environment variables are FORBIDDEN as configuration and are NOT read on the hot path on any platform; earlier revisions of this document that taught `export SQLITE_GRAPHRAG_DB_PATH`, `SQLITE_GRAPHRAG_CACHE_DIR`, `SQLITE_GRAPHRAG_LANG`, or `SQLITE_GRAPHRAG_LOG_LEVEL` are historical and must not be followed
- `db.path` overrides the default `./graphrag.sqlite`; the one-shot `--db` flag overrides it per invocation
- `i18n.lang` switches CLI output between English and Brazilian Portuguese immediately
- `log.level` controls tracing verbosity, exposing every SQL query at `debug`
- Precedence is identical on Linux, macOS, and Windows: CLI flag > XDG `config set` > built-in default

## Performance per target

### Benchmarks — selected supported targets

| Target | Cold start | Warm recall | RSS after model | Embedding throughput |
| --- | --- | --- | --- | --- |
| x86_64-linux-gnu (i7-13700) | 48 ms | 4 ms | 820 MB | 1500 tok/s |
| aarch64-linux-gnu (Graviton3) | 58 ms | 5 ms | 810 MB | 1400 tok/s |
| aarch64-apple-darwin (M3 Pro) | 28 ms | 3 ms | 790 MB | 2000 tok/s |
| x86_64-windows-msvc (i7-12700) | 75 ms | 6 ms | 860 MB | 1300 tok/s |

- Cold start measures time from process spawn to the first SQL query completing successfully
- Warm recall measures a second invocation with the database page cache already hot in memory
- The "RSS after model" and "embedding throughput" columns are HISTORICAL: they were measured against the removed local `multilingual-e5-small` pipeline. The current build loads no model and its resident memory is a fraction of these figures; embedding throughput is now bounded by the OpenRouter REST round-trip, not by local compute
- Each number above stays within ten percent variance across ten local benchmark runs
- These figures come from operator hosts; this project has no remote benchmark pipeline

## Agents validated per platform

### Twenty-one agents — verified on each target

- Anthropic Claude Code runs identically on Linux, macOS, and Windows in native shells
- OpenAI Codex uses the same binary in Linux containers and developer macOS laptops
- Google Gemini CLI invokes the binary through the standard subprocess execution path
- Opencode as an open-source harness integrates through stdin and stdout on every supported operating system
- OpenClaw agent framework targets Linux containers primarily but works on macOS too
- Paperclip research assistant runs on macOS and Linux desktop environments simultaneously
- Microsoft VS Code Copilot executes through tasks in the integrated terminal across operating systems
- Google Antigravity platform runs the Linux glibc binary inside its sandbox runtime
- Codeium Windsurf targets predominantly macOS and Windows editor installations
- Cursor editor invokes the binary through its terminal on macOS, Linux, and Windows alike
- Zed editor runs sqlite-graphrag as an external tool on macOS and Linux natively
- Aider code agent focuses on Linux and macOS terminals for daily git-aware flows
- Google Labs Jules runs the Linux glibc binary in pipelines predominantly
- Kilo Code autonomous agent focuses on macOS flows for developers with native bindings
- Roo Code orchestrator executes on Linux servers and macOS workstations interchangeably
- Cline autonomous agent integrates through VS Code on every operating system the editor supports
- Continue open-source assistant executes wherever its host editor runs with native support
- Factory agent framework prefers Linux containers for reproducible multi-agent scenarios
- Augment Code assistant focuses on macOS and Linux engineering environments predominantly
- JetBrains AI Assistant runs sqlite-graphrag alongside IntelliJ IDEA on all three supported desktops
- OpenRouter proxy layer executes the Linux binary on Kubernetes clusters and Docker hosts

## OAuth-only authentication on all platforms (v1.0.69, historical)

### Behavioral change applied identically on every OS

- HISTORICAL ONLY. This section describes the subprocess LLM backends, which were REMOVED in v1.2.0. The CLI spawns no child process today, performs no OAuth login, and accepts only the `openrouter` and `none` backends; OpenRouter credentials live in XDG config through `config add-key --provider openrouter --from-stdin`.
- Spawning `claude -p` and `codex exec` ABORTED with `AppError::Validation` (exit code 1) when `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` were set in the environment, on Linux glibc, aarch64 GNU, macOS, and Windows targets
- OAuth was the ONLY accepted credential mechanism on every published target
- The `--bare` flag was REMOVED from every executable path in every build variant
- Migration at the time: run `claude login` (Claude Pro/Max) or `codex login` (ChatGPT Pro) once per host and remove the env var from the shell rc
- Defense in depth: `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` were INTENTIONALLY ABSENT from the `env_clear` whitelists on every platform, so even if a later refactor moved the OAuth-only guard, the variable never reached the child
- See `docs/decisions/adr-0011-oauth-only-enforcement.md` for the full rationale and `src/commands/claude_runner.rs:574-666` and `src/commands/codex_spawn.rs:684-758` for the four OAuth-only conformance tests that existed in each binary

## Cross-platform behavior in v1.0.97

### Queue sidecar path derivation (GAP-SG-64 / GAP-SG-65, ADR-0057)

- The enrich/ingest queue sidecars are derived from the `--db` directory through `paths::sidecar_path`, which uses `std::path::Path::parent()` + `join` — platform-agnostic and identical on Linux glibc, aarch64 GNU, macOS Apple Silicon, Windows x86_64, and Windows ARM64; the graceful fallback to a bare name (no parent) preserves the legacy CWD layout on every filesystem (ext4, APFS, NTFS)
- No new platform-specific primitive is introduced; the production `unwrap`/`expect` audit (GAP-SG-57..60, ADR-0056), the `llm_slots` test hardening (GAP-SG-63), and the new `enrich --prune-dead-orphans` inspector (GAP-SG-66, ADR-0058) — a SQLite delete on the `.enrich-queue.sqlite` sidecar — are internal and cross-platform by construction

## Cross-platform behavior in v1.0.96

### Bounded REST fan-out (GAP-OPENROUTER-REST-CONCURRENCY, ADR-0055)

- The OpenRouter embedding fan-out in `embed_passages_parallel_with_embedding_choice` uses a bounded `tokio::task::JoinSet` — pure async, with NO new dependency and NO platform-specific code. It runs identically on Linux glibc, aarch64 GNU, macOS Apple Silicon, Windows x86_64, and Windows ARM64
- In-flight concurrency is clamped to 1..16 (the Cloudflare-safe range) and set through `--rest-concurrency <N>` (default 8); the clamp is applied identically on every target
- Chunk order is preserved by index on every target; SQLite writes stay serialized through WAL + atomic claim, so the single-writer invariant holds on every filesystem (ext4, APFS, NTFS)

### Enrich dead-letter convergence (GAP-ENRICH-BACKLOG-CONVERGE, ADR-0055)

- The `.enrich-queue.sqlite` dead-letter columns `error_class` / `next_retry_at`, the `idx_enrich_queue_eligible` index, and the terminal `dead` status are added by IDEMPOTENT `ALTER TABLE` / `CREATE INDEX IF NOT EXISTS` that runs in place on every platform with no operator action
- `enrich --until-empty` (a loop bounded by `--max-runtime` / `--max-attempts`) and `enrich --status --json` (read-only, no LLM, no singleton) behave identically on the five targets; exit codes and the JSON envelope are byte-for-byte identical by construction
