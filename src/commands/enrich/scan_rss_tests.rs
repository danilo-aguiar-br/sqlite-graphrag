//! GAP-SG-185: keyset pagination parity + Linux RSS peak measurement.

use super::*;
use rusqlite::Connection;

fn seed_unbound(conn: &Connection, n: usize) {
    let tx = conn.unchecked_transaction().unwrap();
    for i in 0..n {
        tx.execute(
            "INSERT INTO memories(name, namespace, type, description, body, body_hash, created_at, updated_at)
             VALUES(?1, 'global', 'note', 'd', 'body', ?2, datetime('now'), datetime('now'))",
            rusqlite::params![format!("m-{i:05}"), format!("h{i}")],
        )
        .unwrap();
    }
    tx.commit().unwrap();
}

fn open_mem_db() -> Connection {
    // Reuse the fixture helper when available; fall back to in-memory schema.
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE memories (
           id INTEGER PRIMARY KEY,
           name TEXT NOT NULL,
           namespace TEXT NOT NULL DEFAULT 'global',
           type TEXT,
           description TEXT,
           body TEXT,
           body_hash TEXT,
           deleted_at TEXT,
           created_at TEXT,
           updated_at TEXT
         );
         CREATE TABLE memory_entities (
           memory_id INTEGER NOT NULL,
           entity_id INTEGER NOT NULL
         );",
    )
    .unwrap();
    conn
}

#[test]
fn keyset_page_size_preserves_order_and_set() {
    let conn = open_mem_db();
    seed_unbound(&conn, 137);
    let full = scan_unbound_memories(&conn, "global", None, &[], 10_000).unwrap();
    let paged = scan_unbound_memories(&conn, "global", None, &[], 7).unwrap();
    assert_eq!(full.len(), 137);
    assert_eq!(paged.len(), 137);
    assert_eq!(
        full, paged,
        "keyset pages must concatenate to the same ordered list"
    );
    let limited = scan_unbound_memories(&conn, "global", Some(25), &[], 7).unwrap();
    assert_eq!(limited, full[..25]);
}

#[cfg(target_os = "linux")]
fn read_vm_hwm_kb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").expect("/proc/self/status");
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            return rest
                .split_whitespace()
                .next()
                .unwrap()
                .parse()
                .expect("VmHWM kb");
        }
    }
    panic!("VmHWM not found");
}

/// Measure peak RSS of a paged scan that DROPS each page (no full Vec).
#[cfg(target_os = "linux")]
fn peak_rss_for_n(n: usize, page_size: usize) -> (u64, usize) {
    use super::super::predicates::UNBOUND_MEMORY_PREDICATE;
    use super::sql::keyset_for_each;

    let conn = open_mem_db();
    seed_unbound(&conn, n);
    // Warm-up one page so SQLite page cache is not charged only on the large run.
    let _ = scan_unbound_memories(&conn, "global", Some(1), &[], page_size).unwrap();
    let before = read_vm_hwm_kb();
    let mut total = 0usize;
    let mut fetch =
        |after: i64, want: usize| -> Result<Vec<(i64, String)>, crate::errors::AppError> {
            let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
            let sql = format!(
                "SELECT m.id, m.name FROM memories m
             WHERE m.namespace = ?1 AND m.deleted_at IS NULL AND m.id > ?2
               AND {UNBOUND_MEMORY_PREDICATE}
             ORDER BY m.id LIMIT ?3"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params!["global", after, limit_v], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        };
    keyset_for_each(None, page_size, &mut fetch, |page| {
        total = total.saturating_add(page.len());
        // Drop page on purpose — mirrors production page→enqueue (scan_operation_for_each).
        Ok(())
    })
    .unwrap();
    let after = read_vm_hwm_kb();
    let delta = after.saturating_sub(before);
    (delta.max(1), total)
}

#[test]
#[cfg(target_os = "linux")]
fn scan_rss_peak_is_sublinear_in_eligible_count() {
    // GAP-SG-185 criteria 1 and 2: numeric peak, not linear in N.
    //
    // VmHWM is process-global, so a parallel `cargo test` suite pollutes the
    // high-water mark. We therefore:
    // 1. always record numeric deltas (criterion 1);
    // 2. when the small-N baseline is meaningful (>= 64 KiB), require ratio < 4
    //    for a 10× corpus (criterion 2);
    // 3. otherwise fall back to an absolute bound on the large-N paged-drop
    //    delta (page-bounded working set must stay under 8 MiB — linear body
    //    materialisation of 20k rows would blow past that).
    let page = 256usize;
    let (d2k, n2k) = peak_rss_for_n(2_000, page);
    let (d20k, n20k) = peak_rss_for_n(20_000, page);
    assert_eq!(n2k, 2_000);
    assert_eq!(n20k, 20_000);
    let ratio = d20k as f64 / d2k.max(1) as f64;
    eprintln!("GAP-SG-185 RSS delta_kb: n=2000 -> {d2k} ; n=20000 -> {d20k} ; ratio={ratio:.3}");
    if d2k >= 64 {
        assert!(
            ratio < 4.0,
            "RSS delta grew too close to linear: d2k={d2k} d20k={d20k} ratio={ratio}"
        );
    } else {
        assert!(
            d20k < 8 * 1024,
            "paged-drop RSS delta for N=20000 must stay under 8 MiB, got {d20k} KiB"
        );
    }
}

/// GAP-SG-141 residual: missing embeddings produce a non-zero re-embed scan
/// backlog without calling the network (dry-run / scan-only path).
#[test]
fn reembed_scan_reports_missing_vectors_without_network() {
    let conn = open_mem_db();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memory_embeddings (
           memory_id INTEGER PRIMARY KEY, embedding BLOB, dim INTEGER
         );",
    )
    .unwrap();
    seed_unbound(&conn, 40);
    let rows = scan_memories_without_embeddings(&conn, "global", None, &[], 16).unwrap();
    assert!(
        rows.len() >= 40,
        "expected live re-embed backlog, got {}",
        rows.len()
    );
    eprintln!("GAP-SG-141 re-embed scan items_total={}", rows.len());
}

/// v1.2.4: keyset_for_each pages concatenate to the same ordered names as collect.
#[test]
fn stream_pages_match_keyset_collect_order() {
    use super::sql::{keyset_collect, keyset_for_each};
    let conn = open_mem_db();
    seed_unbound(&conn, 37);
    let collect = keyset_collect(None, 5, |after, want| {
        let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
        let mut stmt = conn
            .prepare(
                "SELECT id, name FROM memories WHERE namespace='global' AND id > ?1 ORDER BY id LIMIT ?2",
            )
            .unwrap();
        let rows = stmt
            .query_map(rusqlite::params![after, limit_v], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        Ok(rows)
    })
    .unwrap();
    let mut streamed = Vec::new();
    let total = keyset_for_each(
        None,
        5,
        &mut |after, want| {
            let limit_v = i64::try_from(want).unwrap_or(i64::MAX);
            let mut stmt = conn
                .prepare(
                    "SELECT id, name FROM memories WHERE namespace='global' AND id > ?1 ORDER BY id LIMIT ?2",
                )
                .unwrap();
            let rows = stmt
                .query_map(rusqlite::params![after, limit_v], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            Ok(rows)
        },
        |page| {
            streamed.extend(page);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(total, collect.len());
    assert_eq!(streamed, collect);
}
