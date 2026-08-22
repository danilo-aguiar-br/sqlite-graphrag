# ADR-0069: v1.2.8 — Foreign key enforcement is toggled around the migration runner, never inside a migration

- Status: Accepted
- Date: 2026-08-18
- Release: v1.2.8 (crate `1.2.8`)
- Supersedes: the `PRAGMA foreign_keys = OFF` convention used by V006, V008, V009, V010 and V013
- Superseded by: none
- Related: ADR-0068 (open entity_type vocabulary), GAP-SG-140, GAP-SG-277


## Context

Five migrations open with `PRAGMA foreign_keys = OFF;`, following the SQLite "making other kinds of table schema changes" procedure, whose first step is to disable enforcement before rebuilding a table.

That line has never taken effect in this project.

1. `storage::connection::open_rw` applies `apply_connection_pragmas`, which sets `PRAGMA foreign_keys = ON`.
2. refinery runs each migration inside its own transaction (`refinery-core::drivers::rusqlite`); `set_grouped` is never called, and its default is one transaction per migration.
3. SQLite documents `PRAGMA foreign_keys` as **"a no-op within a transaction"**.
4. SQLite documents `DROP TABLE` under enforcement as performing an implicit `DELETE FROM` before dropping, which fires `ON DELETE CASCADE` on every child.

`entities` has four children, all `ON DELETE CASCADE`: `relationships`, `memory_entities`, `entity_embeddings` and `entity_connect_seen`.

Measured on 2026-08-18, migrating a copy of this workspace's database from schema 16 to 17 through a bare `runner().run(conn)`:

| table | before | after |
| --- | --- | --- |
| `entities` | 15 744 | 15 744 |
| `relationships` | 213 029 | **0** |

Exit code 0. The migration reported success.

The defect survived nine migrations because every existing migration test bootstraps an **empty** database, where the cascade has nothing to delete. Only a populated database pays, and no test used one.

The SQLite procedure also places the pragma at step 1 and the transaction at step 2. This project inverted that order, because the transaction is opened by refinery rather than by the migration.


## Decision

Toggle enforcement in Rust, around the runner, where it is outside refinery's transaction and can actually take effect.

`storage::connection::run_migrations_with_foreign_keys_off` is the single entry point:

- disables enforcement, runs the migrations, and restores enforcement **even when a migration fails**, so a failed run never returns a connection that silently accepts orphans;
- then runs `PRAGMA foreign_key_check` as a **query** and fails on the first violating row.

That last point is its own correction. `V010` already contained `PRAGMA foreign_key_check;`, but it ran through `execute_batch`, which discards the result set. The pragma reports violations as rows, never as an error, so batching it verifies nothing.

A migration file must therefore not contain `PRAGMA foreign_keys`. Writing it there promises a protection the file cannot deliver.

Two further guarantees ship with the decision:

- An existing database is copied aside before any auto-migration, through the SQLite Online Backup API rather than a filesystem copy, because WAL mode means the `.sqlite` alone is incomplete. A failed backup aborts the migration.
- The auto-migration gate stopped consulting `SCHEMA_USER_VERSION`, which is an identity marker fixed at 50 and documented as not changing when migrations are added. A value that never changes cannot signal that something new is pending. The gate now reads `MAX(version)` from `refinery_schema_history`.


## Consequences

Five call sites executed the runner directly and all now route through the guarded entry point: two in `storage::connection`, two in `commands::migrate`, and one in `commands::init`. The fifth was found by the gate rather than by reading, after four had already been fixed and the work looked finished.

`tests/migration_foreign_key_gate.rs` closes both ways back in: executing the runner outside the owning module, and adding `PRAGMA foreign_keys` to a new migration file. Historical migrations keep their inert lines, because rewriting an applied migration is what GAP-SG-140 is still paying for.

`storage::connection::migration_cascade_tests` is the first migration test in this project that inserts rows **before** migrating, and asserts enforcement is genuinely ON beforehand — without that assertion the test would pass vacuously in an environment where it was off.
