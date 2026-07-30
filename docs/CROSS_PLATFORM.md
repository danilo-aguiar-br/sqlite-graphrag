# Cross-platform checklist (sqlite-graphrag v1.2.0)

Local-only validation (no GitHub Actions). The CLI must run on **Linux**, **macOS**, and **Windows**.

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

- Product `SQLITE_GRAPHRAG_*` environment variables for config
- Hardcoded `/tmp`, `/home/...`, or drive-letter paths in production code
- Recreating `.github/workflows` CI in this project
- Remote telemetry / OTEL export

## Status

| Platform | Build | Unit tests | Offline E2E harness |
|----------|-------|------------|---------------------|
| Linux x86_64 | host CI of record | host | `scripts/e2e_offline_v120.sh` |
| macOS | operator checklist | operator checklist | same script (bash) |
| Windows | operator checklist | operator checklist | adapt paths; use Git Bash or run checks manually |

## Operator notes v1.2.0 (all platforms)

- Config: CLI flag > XDG `config set` > default. Product `SQLITE_GRAPHRAG_*` env is not read at runtime.
- **DEFAULT_EMBEDDING_DIM=1024** (flag `--embedding-dim` / XDG `embedding.dim`; existing DBs keep `schema_meta.dim` until re-embed).
- Enrich recovery: `enrich --list-skipped` / `enrich --requeue-skipped` for `skipped` / `preservation_failed` queue rows (no raw SQL).
- **GAP-SG-139:** host/XDG leaves (`config`, `slots`, `cache`, `codex-models`, `completions`) accept `--db` as a documented **no-op** on every platform — agents may always pass `--db`.
- deep-research: `-o` and `--output` use atomwrite (tempfile same dir → fsync → rename) on Linux, macOS, and Windows; parent-dir fsync applies on Unix.
- entity-connect: fully implemented (persists relationships); first-scan timeout exits **1** (not singleton **75**).
- Recommended agent order: write → entity-descriptions (hot, optional `--enqueue-enrich`) → entity-connect (cold).
- Offline gate of record is Linux host + `scripts/e2e_offline_v120.sh` **20/20** (canonical; historical wrapper `e2e_offline_v118.sh` / 16/16 superseded). macOS/Windows use the same checklist; do not claim three-OS harness validation without host evidence.
- Complete top-level CLI inventory (all 50 product commands + nested families): [HOW_TO_USE.md](HOW_TO_USE.md#complete-cli-command-inventory-v120) (mirrored in [COOKBOOK.md](COOKBOOK.md) / [HEADLESS_INVOCATION.md](HEADLESS_INVOCATION.md)).
- Portuguese version may retain historical narrative below the shared checklist; treat pre-v1.1.8 product-env tables as historical only.
- Portuguese: [CROSS_PLATFORM.pt-BR.md](CROSS_PLATFORM.pt-BR.md)
