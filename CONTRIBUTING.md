Read this document in [Portuguese (pt-BR)](CONTRIBUTING.pt-BR.md).


# Contributing to sqlite-graphrag


## Welcome
- Thank you for considering a contribution: every pull request strengthens local GraphRAG memory
- Your improvements directly affect developers using LLMs with durable memory in a single SQLite file
- Code, documentation, tests, bug reports, and ideas are equally valued contributions
- This guide keeps your onboarding under 10 minutes from clone to first local test run


## Quick Start
- Use this repository normally; the public `sqlite-graphrag` repository already exists
- The same validation commands apply locally and in the public repository workflow
- No command should print errors on a clean checkout of `main`
```bash
timeout 120 cargo check --all-targets
timeout 300 cargo nextest run --profile ci
RUSTDOCFLAGS="-D warnings" timeout 120 cargo doc --no-deps --all-features
```


## Development Setup
### Toolchain requirements
- MSRV is Rust 1.88 declared in `rust-version` inside `Cargo.toml`
- JAMAIS bump MSRV without opening an RFC-style issue for discussion first
- Install Rust via `rustup` and pin the toolchain with `rustup default 1.88.0`: this repository has NO remote CI, so the pinned toolchain is the only thing that makes local verification reproducible between contributors
### Dependency pinning
- Direct pin `constant_time_eq = "=0.4.2"` protects MSRV 1.88 from transitive drift via `blake3`
- JAMAIS run `cargo update` indiscriminately; always open a PR explaining the version bump
- Lockfile `Cargo.lock` MUST be committed because this repository ships a binary CLI
### Runtime requirements
- SQLite 3.40 or newer is required at runtime due to `sqlite-vec` and FTS5 external-content
- On Linux you may need `libssl-dev` and `pkg-config` for some transitive dev dependencies


## Branching Strategy
- Branch `main` is protected; no CI pipeline exists, so the operator MUST run the local verification suite by hand before merging
- Feature branches SHOULD use the prefix `feature/<short-kebab-case-description>`
- Bug fix branches SHOULD use the prefix `fix/<short-kebab-case-description>`
- Documentation-only branches SHOULD use the prefix `docs/<short-kebab-case-description>`
- Maintenance branches SHOULD use the prefix `chore/<short-kebab-case-description>`


## Commit Convention
- Follow the Conventional Commits 1.0.0 specification for every commit message on shared branches
- Use `feat` for new user-visible features
- Use `fix` for bug fixes landing on main
- Use `perf` for performance improvements without user-visible behavior changes
- Use `refactor` for code restructuring that neither adds features nor fixes bugs
- Use `docs` for documentation-only changes
- Use `chore` for tooling, CI, or repository maintenance
- Use `test` for adding or improving tests
- Use `ci` for CI pipeline changes
- JAMAIS add `Co-authored-by` for AI agents in commit messages: nothing enforces this automatically, so the reviewer MUST check it by hand


## Pull Request Process
### Before opening the PR
- Rebase onto the latest `main` and resolve conflicts locally
- Keep the PR scope focused on a single logical change when possible
- Write a PR description explaining the motivation, the change, and any trade-offs
### PR Validation Checklist
- [ ] `cargo check --all-targets` passes with zero errors
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes with zero warnings
- [ ] `cargo fmt --all --check` passes with zero diffs
- [ ] `cargo doc --no-deps --all-features` with `RUSTDOCFLAGS="-D warnings"` runs clean
- [ ] `cargo nextest run --profile ci` runs the standard suite to success
- [ ] `cargo llvm-cov nextest --profile heavy --features slow-tests --summary-only` keeps coverage at or above the 80 percent minimum
- [ ] `cargo audit` reports zero vulnerabilities
- [ ] `cargo deny check advisories licenses bans sources` passes with zero violations


## Testing
- Run the standard suite with `cargo nextest run --profile ci` for the fast runner: `[profile.ci]` is defined locally in `.config/nextest.toml`, and the profile name is historical, since the run happens on your machine and not on a remote pipeline
- Run the slow suite separately with `cargo nextest run --profile heavy --features slow-tests`
- Measure full-audit coverage with `cargo llvm-cov nextest --profile heavy --features slow-tests --summary-only`
- Keep the full-audit coverage floor at or above 80 percent
- Unit tests live inside `#[cfg(test)] mod tests` blocks within the implementation file
- Integration tests live under `tests/` and SHOULD use `assert_cmd` plus `wiremock` for HTTP mocks
- A hidden flag `--skip-memory-guard` exists exclusively for tests that do not perform real allocation
- Treat `init`, `remember`, `recall`, and `hybrid-search` as heavy-memory commands during manual validation
- Start heavy-command validation with `--max-concurrency 1` and scale only after measuring RSS and swap behavior
- JAMAIS issue real HTTP requests or touch real filesystem paths outside a `TempDir` in tests
- `cargo test` is the ONLY automatic gate in this repository, and it runs on your machine: `tests/no_ci_workflows_gate.rs` forbids CI workflows outright, so every other check in this document is a LOCAL step the operator has to remember
- Run `cargo test --lib lock::tests retry::circuit_breaker_tests` after touching `lock.rs` or `retry.rs` to exercise the new v1.0.68 singleton and circuit-breaker helpers
- Run `cargo test --test terminal_compile_windows` after touching `src/terminal.rs` to confirm the public surface stays callable; there is no `windows-build-check` CI job, so run `cargo check --target x86_64-pc-windows-msvc --lib --all-features` locally for the full cross-platform type check
- Test assertions involving timestamps MUST be timezone-agnostic — parse ISO via `chrono::DateTime::parse_from_rfc3339` and compare `timestamp()` against `DateTime::UNIX_EPOCH` instead of hardcoded `1970-01-01T00:00:00` strings; this rule was added after a `SQLITE_GRAPHRAG_DISPLAY_TZ` leak in v1.0.66/v1.0.67 made three pre-existing tests flaky. That leak is historical: no product env is read at runtime today
- OpenRouter embedding tests live in `tests/openrouter_embedding.rs` using `wiremock` for HTTP mocking; run with `cargo test --test openrouter_embedding`
- E2E OpenRouter tests with real API are opt-in: use `config add-key openrouter` (or `--openrouter-api-key`) and `--embedding-model` to run against a live endpoint; product never reads `OPENROUTER_API_KEY`; these are NOT part of the default `cargo test` suite
- v1.2.1 enrich-queue CAPA regressions: when editing enqueue/dequeue/re-embed predicates, run `cargo test --lib commands::enrich` and confirm `enqueue_candidate_accepts_entity_prefixed_reembed_key` + `dequeue_next_pending_isolates_by_namespace` stay green (namespace claim isolation / `entity:` strip)

### v1.0.76 Test Matrix (3 features)
- Historical: the v1.0.76 CI matrix RAN `clippy` and `test` jobs across `default` and `llm-only` features (the `embedding-legacy` leg was removed in v1.0.79 together with the feature)
- Historical: those `default` and `llm-only` jobs INSTALLED a stub `mock-llm` CLI on `PATH` so embedding round-trip tests could run without real OAuth credentials
- Today that matrix no longer exists: `.github/` holds only issue and pull request templates, and `tests/no_ci_workflows_gate.rs` forbids workflows and FAILS if any file appears under `.github/workflows/`. Reproduce the feature legs locally with `cargo test --no-default-features --features llm-only`

- New code that touches `src/extract/llm_embedding.rs` MUST be exercised via the mock LLM contract in `tests/fixtures/mock-llm/`
- New code MUST NOT depend on the daemon: the daemon was fully removed in the LLM-only one-shot architecture (v1.0.76+; feature deleted before v1.1.0). Every build is one-shot with no in-process model runtime
- New code that introduces a new migration version MUST round-trip through `migrate --rehash` and `migrate --to-llm-only` integration tests to validate the SipHasher13 checksum rewrite path


## Documentation
- Every public API MUST have `///` doc comments with at least one testable example when reasonable
- Run `cargo doc --no-deps --all-features` with `RUSTDOCFLAGS="-D warnings"` locally before pushing
- `cargo doc --no-deps --locked` is a real quality gate, not a courtesy step: a broken intra-doc link fails the doc build
- The rustdoc lint levels are a versioned fact in the manifest, under `[lints.rustdoc]` in `Cargo.toml`: `broken_intra_doc_links = "deny"`, `private_intra_doc_links = "deny"`, `invalid_html_tags = "deny"`
- Because the levels live in the manifest, the gate does NOT depend on anyone remembering to export `RUSTDOCFLAGS` from a script outside the repository
- These lints are only available when running rustdoc, so `cargo check`, `cargo clippy -- -D warnings` and `cargo test` are structurally blind to them: that blindness is how the debt grew from 7 warnings in v1.1.1 to 64 errors in v1.2.7 with the other three gates green the whole time
- Operational trap: an `invalid_html_tags` error in a doc comment is almost never on the line rustdoc reports; an unpaired backtick ABOVE the reported point misaligns the code-span pairing, and rustdoc then flags the first `<` it finds after it, so look UPWARD from the reported line for the unbalanced backtick
- Documentation formatting rules are defined in `docs_rules/rules_rust_documentacao.md`
- Bilingual README, CONTRIBUTING, SECURITY, and CODE_OF_CONDUCT MUST stay synchronized across EN and pt-BR
- When adding or modifying CLI commands, update documentation in BOTH English and Portuguese files (e.g., `README.md` and `README.pt-BR.md`, `docs/HOW_TO_USE.md` and `docs/HOW_TO_USE.pt-BR.md`)
- Update the CHANGELOG under the Unreleased section for every user-visible change


## How to Report Bugs
- Open an issue using the Bug Report template on GitHub
- Include a minimal reproduction case, ideally under 20 lines of invocation or code
- Include the output of `cargo --version` and `rustc --version`
- Include your OS, architecture, SQLite version, and sqlite-graphrag version
- Include the exact command you ran, the observed output, and the expected output


## How to Request Features
- Open an issue using the Feature Request template on GitHub
- Describe the concrete use case and who benefits; avoid abstract wish-list framing
- Describe at least one alternative you considered and why it did not fit
- Reference any upstream PRD section or related issue when applicable


## Release Process
- Maintainers bump `version` in `Cargo.toml` following Semantic Versioning 2.0.0
- Maintainers update the CHANGELOG moving Unreleased entries under the new version with ISO date
- Maintainers tag the release commit as `vX.Y.Z` using `git tag -a vX.Y.Z -m "Release vX.Y.Z"`
- Pushing the tag builds NOTHING on its own: this project forbids CI, `.github/workflows/` does not exist, and `tests/no_ci_workflows_gate.rs` fails if it ever appears. Release artifacts are built locally by the maintainer with `cargo build --release`
- Final publication to crates.io is done manually with `cargo publish --locked`

## Recent Releases

### v1.2.1 - 2026-07-31 — Enrich queue CAPA seal (namespace claim, force-redescribe reopen, re-embed BLOB truth)
- Queue regressions (run when touching `src/commands/enrich/queue*.rs`, `predicates.rs`, `scan.rs`): `enqueue_candidate_accepts_entity_prefixed_reembed_key` (accepts `entity:ownership` and bare `ownership`; rejects missing entity); `dequeue_next_pending_isolates_by_namespace` (claim filtered by operation **and** namespace). Queue unit suite: **38** tests OK (`cargo test --lib commands::enrich::queue` or full `cargo test --lib commands::enrich`).
- CAPA themes: namespace claim isolation; `--until-empty` op+ns count; `--force-redescribe` reopens `skipped`/`done` once/process; re-embed eligibility `LENGTH(embedding)=dim*4` + zombie reconcile; chunk ns validate; CAPA-D compound configuration-file markers only.
- No schema migration (main DB stays **v16**). See `CHANGELOG.md` `[1.2.1]`; pin `=1.2.1`.

### v1.2.0 - 2026-07-29 — DEFAULT_EMBEDDING_DIM=1024, hermetic tests, residual seal
- Hermetic test harness: `IsolatedEnv` / `xdg_isolation_guard` (and `wire_assert_cmd`); **no** product env `SQLITE_GRAPHRAG_*` as the operational config path in tests (negative asserts only; GAP-SG-101/118).
- Offline E2E gate: `scripts/e2e_offline_v120.sh` (**20/20** on release binary 1.2.0); historical wrapper `e2e_offline_v118.sh` superseded.
- **DEFAULT_EMBEDDING_DIM=1024** in `constants` (flag / XDG / `schema_meta.dim` still override; fixtures use 1024, not 384).
- Quality bar: `cargo clippy --lib -D warnings` clean; `#![deny(missing_docs)]` on the lib crate (public items documented EN).
- See `CHANGELOG.md` `[1.2.0]`; pin `=1.2.0`; main-DB schema stays at v16.

### v1.1.06 - 2026-07-12 — Entity-connect scan O(k) (GAP-ENTITY-CONNECT-SCAN-CARTESIAN)
- Integration suite `tests/v1106_entity_connect_scan_regression.rs` plus unit tests under `src/commands/enrich/` cover O(k) co-occurrence + hub×island scan, `pair:{id1}:{id2}` / `entity_pair` queue typing, first-scan InterruptHandle → Timeout exit 1, NDJSON `scan_start` / dual backlog fields, and drain-by-PK without re-scan.
- No schema migration (schema stays at v16). See `CHANGELOG.md` `[1.1.06]`, `gaps.md` (GAP Fechado), and ADR-0066.
- When touching `scan.rs`, `queue.rs`, or enrich drain paths: run `cargo test --test v1106_entity_connect_scan_regression` and `cargo test --lib commands::enrich`. Do not treat scan wall-clock timeout as exit 75.

### v1.1.05 - 2026-07-11 — Deep-research incident (Bugs 1–5)
- Integration suite `tests/v1105_incident_bugs_regression.rs` covers all five operator-blocking bugs at the CLI boundary: single-token deep-research aspect fan-out, `--output` atomwrite + blake3 ack, `graph traverse` fuzzy/suggestions, `merge-entities` self-ref pre-DB rejection, `link --from-id`/`--to-id` plus pure-numeric name rejection.
- No schema migration (schema stays at v16). See `CHANGELOG.md` `[1.1.05]` and `gaps.md` Status v1.1.05.

### v1.0.96 - 2026-06-27 — Enrich Dead-letter and Bounded REST Fan-out (ADR-0055)
- GAP-ENRICH-BACKLOG-CONVERGE: the `.enrich-queue.sqlite` queue gains `error_class` and `next_retry_at` columns (idempotent ALTER TABLE) plus a terminal `dead` status; Transient failures reschedule with exponential backoff (reusing `AttemptOutcome`/`compute_delay` from `src/retry.rs`), HardFailures go terminal immediately, and an item becomes `dead` after `--max-attempts` (default 5) retries. New `enrich --until-empty` runs an internal scan→drain loop (capped by `--max-runtime`, default 3600s) that replaces the external bash retry loop; `enrich --status` is a read-only JSON queue report that never calls the LLM nor acquires the singleton.
- GAP-OPENROUTER-REST-CONCURRENCY: `embed_passages_parallel_with_embedding_choice` fans out the OpenRouter embedding REST calls via a bounded `tokio::task::JoinSet` (`--rest-concurrency`, clamp 1..=16, default 8, no new dependency); batches of 32 with chunk-index ordering preserved, SQLite writes still serialized via WAL + atomic claim (single-writer intact).
- Validation: nextest 1086 passed, 0 failed, 6 skipped; live ordering proof (cosine diagonal 0.9999, off-diagonal max 0.899, argmax 64/64); ADR-0055 (EN+PT).
### v1.0.95 - 2026-06-27 — OpenRouter Chat Enrich (ADR-0054)
- GAP-OR-ENRICH: new opt-in `enrich --mode openrouter` routes the JUDGE step to the OpenRouter `/chat/completions` REST endpoint, removing the requirement for a locally installed `claude`/`codex`/`opencode` CLI; the four enrich modes are now `claude-code`, `codex`, `opencode`, `openrouter`. New module `src/chat_api.rs` (`OpenRouterChatClient`) mirrors `src/embedding_api.rs`; `--openrouter-model` is required with `--mode openrouter`.
- Validation: SCAN→JUDGE→PERSIST unchanged, 13/13 real models pass, no migration (schema v15); OpenRouter key via flag/`config add-key` (`OPENROUTER_API_KEY` is ignored at runtime), handled via `secrecy`/zeroize, never logged or passed to a subprocess.
### v1.0.94 - 2026-06-26 — Four-Gap Remediation (ADR-0053)
- Fixed GAP-OR-ENTITY-EMBED (entity embedding honours `--embedding-backend`/`--llm-backend`; `remember` with new entities ~119s -> ~0.9s), GAP-EMBED-DIM-64 (default dim 64 -> 384), GAP-EMBED-TIMEOUT-300 (embedding timeout 120s -> 300s), GAP-HEADLESS-DEFAULT (`enrich --mode` now required, clap exit 2 when omitted).
- Validation: `cargo fmt --check` 0 diffs, `cargo clippy -- -D warnings` 0 warnings, `cargo test` exit 0; ADR-0053 (EN+PT), documentation synced across root, docs/, skill/ and llms.
### v1.0.83 - 2026-06-17 — Custom-Provider Credential Preservation (ADR-0041)
- **Custom Anthropic-compatible providers** now work end-to-end: `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, `CODEX_ACCESS_TOKEN`, `CLAUDE_CODE_ENTRYPOINT`, `DISABLE_TELEMETRY`, and `OTEL_EXPORTER_OTLP_ENDPOINT` flow from the orchestrator process to the `claude -p` / `codex exec` subprocess. Provider MiniMax/api.minimax.io (the trigger of this release), OpenRouter, AWS Bedrock custom routes, and corporate Anthropic-compatible gateways are now first-class.
- **OAuth-only mandate intact** as defence in depth: the guards in `claude_runner.rs`, `codex_spawn.rs`, and `ingest_claude.rs` still reject `ANTHROPIC_API_KEY` and `OPENAI_API_KEY` with `AppError::Validation` (exit 1). The eight pre-existing `#[serial_test::serial(env)]` tests in `claude_runner.rs` and `codex_spawn.rs` remain green.
- **DRY achieved** via new helper module `src/spawn/env_whitelist.rs` exposing `apply_env_whitelist(cmd, strict)` and `is_strict_env_clear()`. The three spawners (`claude_runner`, `codex_spawn`, `ingest_claude`) delegate to the helper instead of carrying identical inline whitelist arrays.
- **Compliance opt-in** via flag --strict-env-clear (v1.1.8: product env is not the config path). Strict mode preserved only `PATH` and dropped every other env var, targeting PCI-DSS, SOC2 and HIPAA environments that forbid credential forwarding via env vars. HISTORICAL since v1.2.2: the flag was removed from the global surface — `src/cli/globals.rs` records where it lived — and a 1.2.8 binary answers unexpected argument '--strict-env-clear' found with exit 2. Nothing is left to clear, because the subprocess spawners were removed in v1.2.0 and no credential crosses a process boundary any more.
- **New regression tests** in `tests/claude_runner_env.rs` (311 lines, five `#[serial_test::serial(env)]` scenarios): custom-provider propagation, OAuth-only abort preservation, codex base-URL inheritance, strict-mode credential dropping, and a no-leak audit that scans subprocess stderr for the literal token value with `RUST_LOG=trace`. Three companion unit tests live in `src/spawn/env_whitelist.rs::tests`.
- **No telemetry emitted** by the fix itself; the only new log lines are the existing OAuth-only guard warnings now augmented with orientative marker args `--oauth-only-resolution-use-anthropic-auth-token` (claude) and `--oauth-only-resolution-use-codex-auth-json-or-openai-base-url` (codex).
- **One new ADR**: `docs/decisions/adr-0041-preserve-custom-provider-env.md` (EN + PT-BR) explaining why custom-provider env vars are preserved, why `ANTHROPIC_API_KEY`/`OPENAI_API_KEY` remain rejected, the three alternatives considered (flag opt-in, workaround documentation, full spawner refactor), and cross-references to `gap-g58-recall-sem-fallback-deterministic-2026-06-13` (partially resolved by this release).
- **Documentation updates** in `gaps.md` (new GAP-006 section), `SECURITY.md` / `SECURITY.pt-BR.md` (new "v1.0.83 Custom Provider Credential Preservation" section after the existing v1.0.76 OAuth-Only section, plus two new bullet points in Best Practices), `INTEGRATIONS.md` / `INTEGRATIONS.pt-BR.md` (new "Minimax (since v1.0.83)" section between OpenRouter and POSIX Shells with configuration block, smoke test, and 401 troubleshooting checklist), `README.md` / `README.pt-BR.md` (Custom Anthropic-compatible providers entry at the top of the Highlights block), and `CHANGELOG.md` / `CHANGELOG.pt-BR.md` (full entry under `[1.0.83]`).
- 818 lib tests + 6 env-whitelist unit tests + 5 integration tests pass; `cargo clippy --all-targets --all-features -- -D warnings` zero warnings; `cargo fmt --all --check` clean
- See `gaps.md` GAP-006, `CHANGELOG.md` v1.0.83 entry, and `docs/decisions/adr-0041-preserve-custom-provider-env.md` for the full change rationale
### v1.0.76 - 2026-06-07 — LLM-Only One-Shot, OAuth-Only Embedding
- **BREAKING ARCHITECTURAL CHANGE**: the default build no longer bundles any local model. All embedding generation, NER, and vector search delegate to `claude -p` or `codex exec` headless (OAuth, no MCP, no hooks). The CLI is one-shot. Binary drops from 39 MB to ~6 MB.
- **Removed crates**: `fastembed 5.13.4`, `ort 2.0.0-rc.12`, `ndarray 0.16`, `tokenizers 0.22`, `huggingface-hub 0.4`, `sqlite-vec 0.1.9`
- **Removed features**: `daemon` (removed with the LLM-only one-shot architecture; no longer present in any build), `--enable-ner` GLiNER ONNX path (moved then fully removed with `ner-legacy`)
- **Added**: `ExtractionBackend` trait with `LlmBackend` / `EmbeddingBackend` / `NoneBackend` / `CompositeBackend`; `VersionAdapter` trait with `CodexAdapter` / `ClaudeAdapter` / `OpencodeAdapter`; `migrate --rehash` and `migrate --to-llm-only --drop-vec-tables`; BLOB-backed `memory_embeddings` / `entity_embeddings` / `chunk_embeddings` tables; pure-Rust cosine in `src/similarity.rs`; OAuth-only LLM credential flow with `AppError::Validation` abort on `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` in env
- **Migration V013** drops the `vec_memories` / `vec_entities` / `vec_chunks` virtual tables; old embeddings are recomputed lazily on next write
- **CI matrix**: `default` and `llm-only` since v1.0.79 (`embedding-legacy` removed); mock LLM CLI wired into 26 test files; 107/115 previously-slow tests fixed
- **7 new ADRs**: `adr-0019-llm-only-one-shot`, `adr-0020-pure-rust-cosine`, `adr-0021-deprecate-daemon`, `adr-0022-blob-embeddings`, `adr-0023-remove-tokenizers`, `adr-0024-fts5-coarse-cosine-refine`, `adr-0025-oauth-only-embedding`; all with PT-BR translations
- **2 new JSON schemas**: `migrate-rehash.schema.json`, `migrate-to-llm-only.schema.json`
- **3 new docs**: `docs/HOW_TO_USE.md`, `docs/MIGRATION.md`, `docs/AGENTS.md` (and PT-BR) for the v1.0.76 LLM-Only architecture
- **1 new doc**: `docs/HEADLESS_INVOCATION.md` (and PT-BR) covering Claude/Codex/OpenCode OAuth-safe headless invocation
- 745 lib tests pass, 0 fail, 3 ignored; `cargo clippy --all-targets --all-features -- -D warnings` zero warnings
- See `gaps.md` for the full resolution history and `CHANGELOG.md` for the v1.0.76 entry

### v1.0.68 - 2026-06-03 — Process Lifecycle Governance and Windows Compile Fix
- **G28-A** (historical, subsystem removed in v1.2.0) MCP server isolation via `SQLITE_GRAPHRAG_CLAUDE_EMPTY_CONFIG_DIR` (subprocess receives `CLAUDE_CONFIG_DIR=<empty dir>`; `--strict-mcp-config` and `--mcp-config '{}'` are ignored upstream per anthropics/claude-code#10787)
- **G28-B** `lock::acquire_job_singleton(JobType, namespace, wait_seconds)` plus `AppError::JobSingletonLocked { job_type, namespace }` (exit 75) integrated into `enrich`, `ingest --mode claude-code`, and `ingest --mode codex` to prevent process proliferation against the same database
- **G28-D** `retry::CircuitBreaker` helper with `AttemptOutcome::{Success, Transient, HardFailure}`; rate-limited and timeout errors are explicitly excluded from the failure count; `enrich` emits a `tracing::warn!` when `--llm-parallelism > 4`
- **G29** `src/terminal.rs` rewritten with `!handle.is_null() && handle != INVALID_HANDLE_VALUE` so `cargo install sqlite-graphrag` succeeds on Windows; `windows-sys` pinned to `=0.59.0` exact; new CI job `windows-build-check` runs `cargo check --target x86_64-pc-windows-msvc --lib --all-features` on every push
- **Test Fixes** three pre-existing timezone-leak failures in `src/commands/{history,list,read}.rs` fixed via `chrono::DateTime::parse_from_rfc3339` + `DateTime::UNIX_EPOCH` comparison
- **Documentation** new ADRs `adr-0008-process-lifecycle-singleton`, `adr-0009-windows-sys-handle-pinning`, `adr-0010-mcp-isolation-claude-config-dir`; `SKILL.md` EN+PT, `AGENTS.md` EN+PT, `llms.txt`, `llms.pt-BR.txt`, `llms-full.txt`, `INTEGRATIONS.md` EN+PT, `MIGRATION.md` EN+PT, `TESTING.md` EN+PT, `HOW_TO_USE.md` EN+PT, `CROSS_PLATFORM.md` EN+PT, `COOKBOOK.md` EN+PT updated with the v1.0.68 section; `docs/schemas/error-envelope.schema.json` updated to document the second `code: 75` template
- **CI** new `windows-build-check` job; `language-check` job retained from prior release
- 692 lib tests + 2 integration tests pass; 0 warnings under `clippy -- -D warnings` and `cargo doc --no-deps --all-features` with `RUSTDOCFLAGS="-D warnings"`
- See `gaps.md` for the full resolution history and `CHANGELOG.md` for the v1.0.68 entry

## Mandatory Pre-Push Checklist (since v1.0.68)
- [ ] `cargo fmt --all --check` is clean
- [ ] `cargo check --all-targets` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` reports zero warnings
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` reports zero warnings
- [ ] `cargo test --lib` reports 818 passed, 0 failed (was 692 prior to v1.0.83)
- [ ] `cargo test --test terminal_compile_windows` reports 2 passed
- [ ] PR title is in English and follows Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `ci:`, `build:`, `perf:`)
- [ ] No `Co-authored-by: ...` trailer for any AI agent (Claude, Codex, GPT, Copilot, Cursor, Gemini, Anthropic, OpenAI)
- [ ] CHANGELOG entries added under `[Unreleased]` in BOTH `CHANGELOG.md` and `CHANGELOG.pt-BR.md`
- [ ] If touching `windows-sys` or any FFI crate, run `cargo check --target x86_64-pc-windows-msvc --lib --all-features` locally
- [ ] If touching `lock.rs` or `retry.rs`, run `cargo test --lib lock::tests retry::circuit_breaker_tests`


## Recognition
- Contributors are credited in the CHANGELOG next to the version that shipped their change
- Contributors are also listed in each GitHub Release note when the contribution was user-visible
- JAMAIS add `Co-authored-by` trailers for AI agents in any commit or PR description


## Questions
- Open a GitHub Discussion for design questions or broader topics not tied to a specific issue
- Use Security Advisories for anything that resembles a security issue; see SECURITY.md
