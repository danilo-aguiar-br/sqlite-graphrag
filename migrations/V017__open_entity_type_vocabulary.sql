-- v1.2.8: Remove the CHECK constraint on entities.type so the entity vocabulary
-- is open, mirroring what V010 did for relationships.relation in v1.0.49.
-- SQLite cannot alter CHECK constraints in place; full table rebuild required.
--
-- NOTE: there is deliberately NO `PRAGMA foreign_keys = OFF` here.
-- refinery runs every migration inside a transaction, and SQLite documents that
-- pragma as a no-op while a transaction is pending, so the copies of it at the
-- top of V006/V008/V009/V010/V013 never took effect. Enforcement is now toggled
-- around the whole runner in `storage::connection::run_migrations_with_foreign_keys_off`,
-- which is the only place where it can actually work. Re-adding it here would be
-- decoration, and would suggest a protection that this file cannot provide.
--
-- That protection is not cosmetic: `entities` has four children with
-- ON DELETE CASCADE (relationships, memory_entities, entity_embeddings,
-- entity_connect_seen), and DROP TABLE under enforcement performs an implicit
-- DELETE FROM that fires all of them.

CREATE TABLE entities_new (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    namespace   TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    type        TEXT    NOT NULL,
    description TEXT,
    aliases     TEXT    NOT NULL DEFAULT '[]' CHECK(json_valid(aliases)),
    degree      INTEGER NOT NULL DEFAULT 0,
    metadata    TEXT    NOT NULL DEFAULT '{}',
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(namespace, name)
);

-- Explicit column lists on both sides. V008 used `SELECT *`, which silently
-- depends on positional order surviving every future edit.
INSERT INTO entities_new (
    id, namespace, name, type, description, aliases, degree, metadata, created_at, updated_at
)
SELECT
    id, namespace, name, type, description, aliases, degree, metadata, created_at, updated_at
FROM entities;

DROP TABLE entities;
ALTER TABLE entities_new RENAME TO entities;

-- Recreated from their origins: V001 for idx_entities_ns, V005 for
-- idx_entities_namespace_degree. UNIQUE(namespace, name) regenerates
-- sqlite_autoindex_entities_1. There are no triggers or FTS tables on
-- `entities`, so nothing else needs restoring.
CREATE INDEX IF NOT EXISTS idx_entities_ns ON entities(namespace);
CREATE INDEX IF NOT EXISTS idx_entities_namespace_degree ON entities(namespace, degree DESC);

-- Deliberately NO `ANALYZE` here. The rebuild does drop this table's rows from
-- sqlite_stat1/sqlite_stat4, but `ANALYZE` recomputes statistics for EVERY
-- index in the database, not just this table's: measured at over four minutes
-- of CPU on the 343 MiB / 213k-edge database of this workspace, all of it
-- inside the migration transaction. Restoring the statistics is the job of the
-- existing `optimize` subcommand, which the release notes point at.
