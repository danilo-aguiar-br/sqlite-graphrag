# ADR-0063: v1.1.03 — Six Bugs + split-body (Bug-Fix Wave)

- **Status**: Accepted
- **Date**: 2026-07-07
- **Release**: v1.1.03 (crate `1.1.3`)
- **Supersedes**: none
- **Superseded by**: none
- **Related**: ADR-0061 (v1.1.01 twelve-priority roadmap), ADR-0062 (v1.1.02 gap closure), ADR-0058 (`--prune-dead-orphans`), ADR-0059 (`--max-entity-degree` removal)

## Context

After the GraphRAG remediation run on 2026-07-07, six operator-blocking bugs and one cosmetic gap (V8) were catalogued in `gaps.md` against the v1.1.2 binary. All seven were implemented and validated in the v1.1.03 wave:

1. **Bug 1 — enrich scan-enqueue looked like a deadlock.** With ~44k entity candidates, the scan phase emitted `{"phase":"scan","items_total":44163}` and never advanced to drain. The apparent freeze was rooted in stale `processing` claims left by a prior `kill -9` (Bug 4): per-item enqueue acquired the WAL write lock under contention and the unbounded loop starved the scan. The scan-enqueue path was also row-by-row, so large workloads amplified the lock starvation.

2. **Bug 2 — `reclassify-relation` could not migrate legacy underscore edges.** The clap boundary normalized BOTH `--literal-from` and `--to-relation` to kebab-case before the from==to guard, so `--literal-from applies_to --to-relation applies-to` collapsed into `applies-to == applies-to` and raised exit 1. 61 357 legacy edges (39 362 `applies_to`, 17 956 `depends_on`, 4 036 `tracked_in`) were unreachable by any CLI path — blocking the V5 final gate.

3. **Bug 3 — `merge-entities` could not cross namespaces.** The `--ids`/`--into-id` path resolved every ID against a single resolved namespace, so a duplicated entity living in `ai-sdd` could not merge into its `global` twin. 15 cross-namespace duplicates blocked the V6 gate; deleting the `ai-sdd` side is forbidden by the operator rule.

4. **Bug 4 — `kill -9` left three layers of stale lock.** The enrich job holds (1) the file-lock singleton, (2) the SQLite write lock via an open file descriptor, and (3) `queue_processing` claims + `state:draining` in the sidecar. `--force-job-singleton` cleared only layer 1; layers 2 and 3 persisted and produced exit 75 ("job in progress") on the next invocation and the false-`draining` state observed in Bug 1's scan freeze.

5. **Bug 5 — `queue_pending` was misread as a physical queue.** Operators read `queue_pending: 47300` as 47 300 items sitting in a physical queue, but the value is a COMPUTED COUNT over the sidecar (status counts), not a rowset to drain. This produced false alarms ("deadlock!") on cooldown states (`eligible_now == 0`, items parked on `next_retry_at`) and obscured the real signal (`scan_backlog`).

6. **Bug 6 — `enrich re-embed --target chunks` skipped orphan chunks.** The chunk scanner used `JOIN memories m ON m.id = c.memory_id WHERE m.deleted_at IS NULL`. Soft-deleted mothers keep their chunks (CASCADE only fires on HARD delete), so those chunks were invisible to the scanner AND to `count_operation_backlog`, yielding a false `scan_backlog: 0` while `health` reported `vec_chunks_missing > 0` — a dissonance the operator could not close.

7. **V8 — oversized bodies (>25k chars) were never split.** 777 monster bodies remained un-split. The impact was cosmetic (chunks were embedded individually and remained searchable), but the V8 final gate could not close without a CLI-level split command.

## Decision

### D1 — Bug 1: batch transactional scan-enqueue

- The scan-enqueue path writes candidate rows in a single batched transaction instead of row-by-row inserts under the WAL write lock.
- This removes the lock-starvation that, under the stale-claim contention from Bug 4, presented as a frozen scan phase for 44k entities.
- The fix is purely in the enqueue loop; the scan predicates and the drain phase are unchanged.

**Causal link**: Bug 4's stale `processing` claims are the trigger; this fix removes the amplifier (per-row lock churn). The remaining trigger is handled by D4.

### D2 — Bug 2: `--literal-to` for verbatim target write

- New flag `reclassify-relation --literal-to <RELATION>` writes the target value VERBATIM (no kebab normalization), complementing the existing `--literal-from` (verbatim source match).
- The from==to guard now compares the raw `--literal-from` literal against the raw `--literal-to` literal, so `--literal-from applies_to --literal-to applies-to` is the canonical migration of a legacy underscore edge to its canonical hyphen form.
- The migration runbook is one command per legacy relation type:

  ```
  reclassify-relation --literal-from applies_to --literal-to applies-to --batch --dry-run
  reclassify-relation --literal-from applies_to --literal-to applies-to --batch
  # repeat for depends_on and tracked_in
  ```

**Causal link**: This unblocks V5 (61 357 underscore edges become reachable for canonical migration).

### D3 — Bug 3: `merge-entities --cross-namespace` (opt-in)

- New flag `merge-entities --cross-namespace` opts into cross-namespace ID resolution.
- Default behaviour (no flag) is unchanged: every ID must belong to the resolved namespace — safe by default, no silent cross-contamination.
- With the flag, `--ids`/`--into-id` resolve each ID across ALL namespaces; the merge keeps the `--into-id` entity's namespace and re-points the source entity's edges.

**Causal link**: This unblocks V6 (15 cross-namespace duplicates can merge into `global`).

### D4 — Bug 4: `claimed_at` column + reset-on-startup + SIGTERM cleanup + `--reset-stale-claims`

- The sidecar queue gains a `claimed_at` column (idempotent ALTER) so a `processing` claim carries a timestamp.
- On enrich startup, `processing` claims older than a threshold are reset to `pending` automatically (no manual intervention for a clean restart after a crash).
- A SIGTERM handler now performs graceful cleanup (release claims, checkpoint state) before the process exits 19 — so a normal termination never leaves stale claims.
- New flag `enrich --reset-stale-claims` performs a manual reset of `processing` claims older than the threshold, for the operator who needs to clear claims from an un-graceful termination without a full restart.

**Causal links**: This is the root fix for the exit-15 contention and the Bug 1 trigger. With D4 in place, D1's batching runs on a queue whose claims are not stale.

### D5 — Bug 5: clarify `enrich --status` field semantics

- The `--status` flag's doc-comment and help text now explicitly document:
  - `scan_backlog` = candidates a fresh scan WOULD select (REAL pending work, same WHERE predicate as the scanners).
  - `queue_pending` = a COMPUTED COUNT over the sidecar, NOT a physical queue of rows to process — it stays non-zero after a clean drain.
  - `eligible_now == 0` with `queue_pending > 0` is COOLDOWN (rate-limit backoff), NOT a deadlock.
  - `eligible_now > 0` stuck against `state: "draining"` IS a deadlock — run `--reset-stale-claims`.
- No behaviour change; the report already carried these fields. This is a documentation clarification so operators stop misreading cooldown as deadlock.

### D6 — Bug 6: LEFT JOIN in the chunk re-embed scanner

- `scan_chunks_missing_embeddings` and `count_operation_backlog` (the shared predicate) switch from `JOIN memories m` to `LEFT JOIN memories m` with the namespace filter relaxed to `(m.namespace = ?1 OR m.id IS NULL)`.
- Chunks whose mother was soft-deleted are now selected for re-embed, reconciling `enrich --status` (`scan_backlog`) with `health` (`vec_chunks_missing`).
- Coverage reaches a real 100% instead of a dissonant <100%.

**Causal link**: This removes the health-vs-status dissonance and lets `--until-empty` converge on the REAL backlog.

### D7 — V8: `split-body` subcommand

- New subcommand `split-body --name <N>` divides a memory whose body exceeds 25 000 characters into daughter memories at chunk boundaries.
- Default mode splits a single named memory; `--batch --threshold 25000` iterates every memory above the threshold.
- The original memory is marked `SUPERCEDIDO` and `replaces` relations are created from each daughter to the original (so history is preserved and recall can traverse the lineage).
- Daughters are NOT embedded inline by `split-body`; the operator MUST run `enrich --operation re-embed --target memories` afterwards to backfill the daughter vectors.

**Causal link**: This closes the V8 cosmetic gap (777 oversized bodies) without blocking search (chunks were already embedded).

## Consequences

- **Positive**:
  - The 44k-entity scan no longer presents as a deadlock; the batched enqueue completes in seconds.
  - The 61 357 legacy underscore edges (V5) and the 15 cross-namespace duplicates (V6) become reachable via CLI.
  - A normal SIGTERM no longer leaves stale claims; an abnormal one is recoverable with `--reset-stale-claims` instead of PID hunting.
  - `enrich --status` and `health` agree on chunk coverage; operators stop chasing a phantom <100%.
  - V8 oversized bodies can be split, closing the final gate.
- **Negative**:
  - `merge-entities --cross-namespace` is a power-user flag: misuse can merge entities that share a name across unrelated namespaces. The opt-in default (same-namespace only) is the mitigation.
  - `split-body` creates daughter memories that require a follow-up `re-embed --target memories`; an operator who skips the re-embed leaves daughters without vectors until the next enrich sweep.
- **Neutral**:
  - Schema stays at v15; the only sidecar change is the idempotent `claimed_at` column on the enrich queue.

## Validation

- `cargo build --release` — zero errors.
- All five code tasks (Bugs 1, 2, 3, 4, 6 + V8) validated with their dedicated tests prior to this docs task.
- No new tests run in this docs task (tests already validated in the implementation tasks).

## Commits

- Bugs 1–4 + 6 + V8 implementation commits (see the five code tasks).
- This ADR + CHANGELOG + Cargo.toml bump + gaps.md + SKILL alignment close the v1.1.03 release.
