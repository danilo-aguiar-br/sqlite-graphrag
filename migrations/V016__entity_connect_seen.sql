-- V016__entity_connect_seen.sql
-- GAP-002 (v1.1.04): convergência do enrich entity-connect
-- Marca pares de entidades já avaliados pelo entity-connect (verdict related/none)
-- para que o scan seja seen-aware e o --until-empty convirja em vez de re-escanear infinito.
-- FK CASCADE garante limpeza automática quando uma entidade é deletada.

CREATE TABLE IF NOT EXISTS entity_connect_seen (
    source_id    INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_id    INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    namespace    TEXT    NOT NULL,
    verdict      TEXT    NOT NULL CHECK(verdict IN ('related','none')),
    relation     TEXT,
    evaluated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (source_id, target_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_connect_seen_ns
    ON entity_connect_seen(namespace);

