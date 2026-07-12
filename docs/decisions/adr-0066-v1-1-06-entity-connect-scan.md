# ADR-0066: v1.1.06 — Entity-Connect Scan O(k) (GAP-ENTITY-CONNECT-SCAN-CARTESIAN)

- Status: Accepted
- Date: 2026-07-12
- Release: v1.1.06 (crate `1.1.6`)
- Supersedes: none
- Superseded by: none
- Related: ADR-0064 (GAP-002 `entity_connect_seen` / convergence), ADR-0060 (enrich backlog convergence), ADR-0055 (`--until-empty` / `--max-runtime`)


## Context

v1.1.04 closed GAP-002 so `entity-connect` **converges** via `entity_connect_seen` (V016). That did **not** fix the cost of the first (and each) pair scan when almost no pairs had been seen yet.

On large `global` namespaces (~96 209 entities in the incident report) `scan_isolated_entity_pairs` used:

```sql
FROM entities e1, entities e2
… ORDER BY (SELECT COUNT(*) FROM memory_entities …) DESC
LIMIT 50
```

SQLite materialised candidates for a global sort (`USE TEMP B-TREE FOR ORDER BY`). The process sat at ~100% CPU with near-zero I/O, never emitted `phase: "scan"`, held the enrich singleton, and cascaded **exit 75** to other enrich ops. `--max-runtime` only checked after the first scan inside `--until-empty`. Additionally, `call_entity_connect` re-ran the same scan with `LIMIT 1` on every drain item, and the queue stored only `e1.name` (ambiguous pairs, wrong `item_type=memory`).


## Decision

1. **Replace cartesian generation** with evidence-local candidates:
   - Primary: co-occurrence pairs from `memory_entities` self-join on `memory_id`.
   - Fill: top-degree hubs × degree-0 islands with NER bindings.
2. **Queue keys** `pair:{id1}:{id2}` (`id1 < id2`), `item_type = entity_pair`.
3. **Drain** resolves entities by primary key; never re-calls the pair scan.
4. **Deadline** before the first scan: soft 120s ceiling for pair ops ∩ `--max-runtime`; `InterruptHandle` watchdog; `SQLITE_INTERRUPT` → `AppError::Timeout` (exit 1).
5. **NDJSON** `phase: "scan_start"` before SQL; skip identical re-scan on the first `--until-empty` iteration.
6. **No schema migration** (V016 / schema version 16 unchanged). GAP-002 semantics preserved.


## Consequences

### Positive

- Large `global` entity-connect scans finish in bounded time.
- Singleton is not held for minutes of pure CPU with zero progress.
- Drain cost is O(1) per item; parallel workers no longer re-hang the DB.
- Operators see `scan_start` / `scan` progress for hooks and agents.

### Negative / residual

- `cross-domain-bridges` still shares the same safe scan path (no separate cross-domain semantic model).
- Legacy queue rows with bare entity names are skipped (re-scan enqueues `pair:` keys).
- Co-occurrence still uses GROUP BY on the co-pair set (hundreds of thousands of pairs on dense DBs); LIMIT + interrupt bound the cost.
- Singleton is still acquired before the pair scan (bounded by soft 120s / `--max-runtime`); not held for unbounded cartesian CPU.

### Audit follow-ups closed in the same release

- `scan_start.operation` uses the real CLI kebab-case name (`entity-connect` **or** `cross-domain-bridges`).
- Dual backlog fields: `backlog_degree0_proxy` vs `pairs_enqueued_this_scan`.
- Unit tests: CLI op names, `SQLITE_INTERRUPT` mapping, past-deadline Timeout, live `InterruptHandle`.


## Validation

- Unit tests in `scan.rs` / `queue.rs` (pair keys, limit, seen exclusion, item_type).
- `tests/v1106_entity_connect_scan_regression.rs` (CLI dry-run phases + pair keys).
- Smoke: `timeout 30 … enrich --operation entity-connect --dry-run` on project `graphrag.sqlite`.
