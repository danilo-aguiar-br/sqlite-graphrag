# Cross-platform checklist (sqlite-graphrag v1.1.8)

Local-only validation (no GitHub Actions). The CLI must run on **Linux**, **macOS**, and **Windows**.

## Build matrix (run on each host)

```bash
# Linux (gnu)
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
bash scripts/e2e_offline_v118.sh

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
| DB | SQLite file via `--db` or XDG `db.default_path` — never product env |
| Locks / slots | Filesystem locks under XDG runtime/cache |
| Line endings | Accept `\n` and `\r\n` on stdin NDJSON |
| Shell completions | `completions` subcommand: bash/zsh/fish/powershell/elvish |
| Console | UTF-8 + ANSI; honor OS `NO_COLOR` |

## Forbidden

- Product `SQLITE_GRAPHRAG_*` environment variables for config
- Hardcoded `/tmp`, `/home/...`, or drive-letter paths in production code
- Recreating `.github/workflows` CI in this project
- Remote telemetry / OTEL export

## Status

| Platform | Build | Unit tests | Offline E2E harness |
|----------|-------|------------|---------------------|
| Linux x86_64 | host CI of record | host | `scripts/e2e_offline_v118.sh` |
| macOS | operator checklist | operator checklist | same script (bash) |
| Windows | operator checklist | operator checklist | adapt paths; use Git Bash or run checks manually |

## Operator notes v1.1.8 (all platforms)

- Config: CLI flag > XDG `config set` > default. Product `SQLITE_GRAPHRAG_*` env is not read at runtime.
- deep-research: `-o` and `--output` use atomwrite (tempfile same dir → fsync → rename) on Linux, macOS, and Windows; parent-dir fsync applies on Unix.
- entity-connect: fully implemented (persists relationships); first-scan timeout exits **1** (not singleton **75**).
- Recommended agent order: write → entity-descriptions (hot, optional `--enqueue-enrich`) → entity-connect (cold).
- Offline gate of record is Linux host + `scripts/e2e_offline_v118.sh` (16/16). macOS/Windows use the same checklist; do not claim three-OS harness validation without host evidence.
- Portuguese version may retain historical narrative below the shared checklist; treat pre-v1.1.8 product-env tables as historical only.
- Portuguese: [CROSS_PLATFORM.pt-BR.md](CROSS_PLATFORM.pt-BR.md)
