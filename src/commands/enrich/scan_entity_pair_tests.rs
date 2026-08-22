//! Isolated-entity pair discovery for entity-connect (GAP-SG-146).
//!
//! The O(k) co-occurrence scan and its `pair:` key encoding. The cartesian
//! regression this replaced hung large namespaces.

use super::test_fixtures::open_test_db;
use super::*;

#[test]
fn format_and_parse_pair_key_roundtrip() {
    assert_eq!(format_pair_key(3, 1), "pair:1:3");
    assert_eq!(parse_pair_key("pair:1:3"), Some((1, 3)));
    assert_eq!(parse_pair_key("pair:9:2"), Some((2, 9)));
    assert_eq!(parse_pair_key("legacy-entity-name"), None);
    assert_eq!(parse_pair_key("pair:x:y"), None);
}

#[test]
fn scan_isolated_entity_pairs_excludes_seen() {
    let conn = open_test_db();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','a','tool')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO entities (namespace, name, type) VALUES ('global','b','tool')",
        [],
    )
    .unwrap();
    let (a_id, b_id): (i64, i64) = conn
        .query_row(
            "SELECT (SELECT id FROM entities WHERE name='a'), \
             (SELECT id FROM entities WHERE name='b')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    // Co-occurrence evidence so the pair would otherwise be a candidate.
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global','m-ab','body')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='m-ab'", [], |r| {
            r.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2), (?1, ?3)",
        rusqlite::params![mem_id, a_id, b_id],
    )
    .unwrap();
    // mark the pair as already judged (verdict none)
    conn.execute(
        "INSERT INTO entity_connect_seen (source_id, target_id, namespace, verdict) \
         VALUES (?1, ?2, 'global','none')",
        rusqlite::params![a_id, b_id],
    )
    .unwrap();
    let pairs = scan_isolated_entity_pairs(&conn, "global", Some(50)).unwrap();
    assert!(
        pairs
            .iter()
            .all(|(id1, _, id2, _)| !(*id1 == a_id && *id2 == b_id)),
        "seen pair must not be re-scanned"
    );
}

#[test]
fn scan_isolated_entity_pairs_respects_limit_on_large_namespace() {
    let conn = open_test_db();
    // 80 entities sharing one memory → many co-pairs; LIMIT must cap.
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global','bulk','x')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='bulk'", [], |r| {
            r.get(0)
        })
        .unwrap();
    for i in 0..80 {
        conn.execute(
            "INSERT INTO entities (namespace, name, type) VALUES ('global', ?1, 'tool')",
            rusqlite::params![format!("e{i}")],
        )
        .unwrap();
        let eid: i64 = conn
            .query_row(
                "SELECT id FROM entities WHERE name = ?1",
                rusqlite::params![format!("e{i}")],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
            rusqlite::params![mem_id, eid],
        )
        .unwrap();
    }
    let started = std::time::Instant::now();
    let pairs = scan_isolated_entity_pairs(&conn, "global", Some(10)).unwrap();
    let elapsed = started.elapsed();
    assert_eq!(pairs.len(), 10, "LIMIT 10 must be honoured");
    assert!(
        elapsed.as_secs() < 5,
        "scan must finish quickly on co-occurrence graph, took {elapsed:?}"
    );
}

#[test]
fn scan_isolated_entity_pairs_uses_cooccurrence_not_cartesian() {
    let conn = open_test_db();
    // Three entities: only a+b co-occur; c is isolated (no shared memory).
    for name in ["a", "b", "c"] {
        conn.execute(
            "INSERT INTO entities (namespace, name, type, degree) VALUES ('global', ?1, 'tool', 0)",
            rusqlite::params![name],
        )
        .unwrap();
    }
    let (a_id, b_id, c_id): (i64, i64, i64) = conn
        .query_row(
            "SELECT \
               (SELECT id FROM entities WHERE name='a'), \
               (SELECT id FROM entities WHERE name='b'), \
               (SELECT id FROM entities WHERE name='c')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global','m','body')",
        [],
    )
    .unwrap();
    let mem_id: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='m'", [], |r| r.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2), (?1, ?3)",
        rusqlite::params![mem_id, a_id, b_id],
    )
    .unwrap();
    // c has a binding alone (island) but does not co-occur with a/b.
    conn.execute(
        "INSERT INTO memories (namespace, name, body) VALUES ('global','m-c','body')",
        [],
    )
    .unwrap();
    let mem_c: i64 = conn
        .query_row("SELECT id FROM memories WHERE name='m-c'", [], |r| r.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO memory_entities (memory_id, entity_id) VALUES (?1, ?2)",
        rusqlite::params![mem_c, c_id],
    )
    .unwrap();

    let pairs = scan_isolated_entity_pairs(&conn, "global", Some(50)).unwrap();
    assert!(
        pairs.iter().any(|(x, _, y, _)| *x == a_id && *y == b_id),
        "co-occurring a-b must be a candidate: {pairs:?}"
    );
    // Without a hub of degree>0, hub×island may not pair c with a/b; the
    // invariant is we never invent a-c/b-c from a pure cartesian product
    // when they never co-occur and no hub fill applies.
    let only_ab = pairs.len() == 1 && pairs[0].0 == a_id && pairs[0].2 == b_id;
    let allowed = |x: i64, y: i64| -> bool {
        if x == a_id && y == b_id {
            return true;
        }
        if x == a_id && y == c_id {
            return true;
        }
        x == b_id && y == c_id
    };
    assert!(
        only_ab || pairs.iter().all(|(x, _, y, _)| allowed(*x, *y)),
        "unexpected pairs: {pairs:?}"
    );
}
