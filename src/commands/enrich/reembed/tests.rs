//! Tests for the batched re-embed claim (GAP-SG-141 B1).
//!
//! The load-bearing one is [`backfill_of_64_items_issues_two_requests`]: it
//! counts real HTTP requests against a `wiremock` server, which is the only
//! evidence that the batch actually collapses the fan-out instead of merely
//! looking like it does.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rusqlite::Connection;
use secrecy::SecretBox;

use super::batch::{run_reembed_cycle, ReembedCycle, ReembedCycleCtx, ReembedTally};
use crate::cli::{EmbeddingBackendChoice, LlmBackendChoice};
use crate::commands::enrich::queue::{dequeue_batch_pending, open_queue_db};

const TEST_DIM: usize = 1024;

fn open_main_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(&format!(
        "CREATE TABLE memories (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            namespace   TEXT NOT NULL DEFAULT 'global',
            name        TEXT NOT NULL,
            type        TEXT NOT NULL DEFAULT 'note',
            description TEXT NOT NULL DEFAULT '',
            body        TEXT NOT NULL DEFAULT '',
            deleted_at  INTEGER,
            UNIQUE(namespace, name)
        );
        CREATE TABLE entities (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            namespace   TEXT NOT NULL DEFAULT 'global',
            name        TEXT NOT NULL,
            type        TEXT NOT NULL DEFAULT 'concept',
            description TEXT,
            degree      INTEGER NOT NULL DEFAULT 0,
            UNIQUE(namespace, name)
        );
        CREATE TABLE memory_embeddings (
            memory_id   INTEGER PRIMARY KEY,
            namespace   TEXT NOT NULL,
            embedding   BLOB NOT NULL,
            source      TEXT NOT NULL,
            model       TEXT NOT NULL DEFAULT '',
            dim         INTEGER NOT NULL DEFAULT {TEST_DIM},
            created_at  INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE TABLE entity_embeddings (
            entity_id   INTEGER PRIMARY KEY,
            namespace   TEXT NOT NULL,
            embedding   BLOB NOT NULL,
            source      TEXT NOT NULL,
            model       TEXT NOT NULL DEFAULT '',
            dim         INTEGER NOT NULL DEFAULT {TEST_DIM},
            created_at  INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE TABLE memory_chunks (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            memory_id    INTEGER NOT NULL,
            chunk_idx    INTEGER NOT NULL,
            chunk_text   TEXT NOT NULL
        );
        CREATE TABLE chunk_embeddings (
            chunk_id    INTEGER PRIMARY KEY,
            memory_id   INTEGER NOT NULL,
            embedding   BLOB NOT NULL,
            source      TEXT NOT NULL,
            model       TEXT NOT NULL DEFAULT '',
            dim         INTEGER NOT NULL DEFAULT {TEST_DIM},
            created_at  INTEGER NOT NULL DEFAULT (unixepoch())
        );"
    ))
    .expect("schema creation must succeed");
    conn
}

fn temp_queue_path(tag: &str) -> String {
    format!(
        "{}/test-reembed-{tag}-{}-{}.sqlite",
        std::env::temp_dir().display(),
        std::process::id(),
        fastrand::u64(..)
    )
}

fn enqueue_reembed(queue: &Connection, namespace: &str, key: &str) {
    queue
        .execute(
            "INSERT INTO queue (namespace, item_key, item_type, status, operation)
             VALUES (?1, ?2, 'memory', 'pending', 'ReEmbed')",
            rusqlite::params![namespace, key],
        )
        .expect("enqueue must succeed");
}

fn test_paths() -> crate::paths::AppPaths {
    crate::paths::AppPaths {
        db: std::path::PathBuf::from(":memory:"),
        models: std::env::temp_dir(),
    }
}

/// Installs a `wiremock`-backed OpenRouter embed client into the process-wide
/// `OnceLock` and returns the counter of requests it served.
///
/// The lock is first-wins and process-wide, so exactly ONE test may install a
/// client; every other test here stays offline by construction (no surviving
/// text reaches the embedder).
async fn install_counting_backend(dim: usize) -> (wiremock::MockServer, Arc<AtomicUsize>) {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    struct CountingResponder {
        hits: Arc<AtomicUsize>,
        dim: usize,
    }
    impl Respond for CountingResponder {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            self.hits.fetch_add(1, Ordering::SeqCst);
            let body: serde_json::Value =
                serde_json::from_slice(&request.body).expect("request body is JSON");
            let n = match body.get("input") {
                Some(serde_json::Value::Array(a)) => a.len(),
                _ => 1,
            };
            let vector: Vec<f32> = vec![0.01_f32; self.dim];
            let data: Vec<serde_json::Value> = (0..n)
                .map(|i| serde_json::json!({ "index": i, "embedding": vector }))
                .collect();
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": data }))
        }
    }

    let server = MockServer::start().await;
    let hits = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .respond_with(CountingResponder {
            hits: Arc::clone(&hits),
            dim,
        })
        .mount(&server)
        .await;

    let client = crate::embedding_api::OpenRouterClient::new_with_base_url(
        SecretBox::new(Box::new("test-key".to_string())),
        "qwen/qwen3-embedding-8b".to_string(),
        dim,
        30,
        format!("{}/embeddings", server.uri()),
    )
    .expect("test client builds");
    crate::embedder::OPENROUTER_CLIENT
        .set(client)
        .map_err(|_| ())
        .expect("no other test may install the process-wide embed client");
    (server, hits)
}

fn ctx<'a>(
    main_conn: &'a Connection,
    queue_conn: &'a Connection,
    paths: &'a crate::paths::AppPaths,
) -> ReembedCycleCtx<'a> {
    ReembedCycleCtx {
        main_conn,
        queue_conn,
        namespace: "global",
        op_label: "ReEmbed",
        backoff_clause: "",
        paths,
        llm_backend: LlmBackendChoice::None,
        embedding_backend: EmbeddingBackendChoice::Openrouter,
        max_attempts: 8,
        total: 0,
        stdout_mu: None,
    }
}

/// GAP-SG-141 (B1): THE measurement. 64 eligible memories drain in two claims
/// of 32, and each claim is exactly ONE HTTP request.
///
/// The one-row-per-call path this replaces would have issued 64 requests for
/// the identical payload — the ~32x overhead the gap reported.
#[tokio::test(flavor = "multi_thread")]
async fn backfill_of_64_items_issues_two_requests() {
    crate::constants::set_active_embedding_dim(TEST_DIM);
    let (_server, hits) = install_counting_backend(TEST_DIM).await;

    let main_conn = open_main_db();
    let queue_path = temp_queue_path("count");
    let queue_conn = open_queue_db(&queue_path).expect("queue db opens");
    for i in 0..64 {
        let name = format!("mem-{i:03}");
        main_conn
            .execute(
                "INSERT INTO memories (namespace, name, body) VALUES ('global', ?1, ?2)",
                rusqlite::params![name, format!("body number {i}")],
            )
            .unwrap();
        enqueue_reembed(&queue_conn, "global", &name);
    }

    let paths = test_paths();
    let c = ctx(&main_conn, &queue_conn, &paths);
    let mut tally = ReembedTally::default();
    // The cycle is synchronous and drives the embedder's own runtime through
    // `block_in_place`, which is why this test needs the multi-thread flavour.
    // A rusqlite `Connection` is not `Send`, so it cannot be moved onto a
    // blocking task.
    let mut cycles = 0;
    loop {
        match run_reembed_cycle(&c, &mut tally, None) {
            ReembedCycle::Progressed => cycles += 1,
            ReembedCycle::Empty => break,
            ReembedCycle::DbBusy => panic!("unexpected cycle outcome: DbBusy"),
            ReembedCycle::BreakerOpen => panic!("unexpected cycle outcome: BreakerOpen"),
        }
    }

    assert_eq!(tally.completed, 64, "every memory must be re-embedded");
    assert_eq!(tally.failed, 0);
    assert_eq!(tally.skipped, 0);
    assert_eq!(cycles, 2, "64 items at a claim width of 32 is two claims");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "one HTTP request per claim; the one-row-per-call path would have issued 64"
    );

    let vectors: i64 = main_conn
        .query_row("SELECT COUNT(*) FROM memory_embeddings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(vectors, 64, "every vector must be persisted");

    // Differential check against the one-row-per-call oracle: same body, same
    // backend, so the persisted row must be byte-identical to what the batch
    // wrote. This is what makes the batch a refactor of the fan-out rather than
    // a reimplementation of the write.
    main_conn
        .execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'mem-oracle', 'body number 0')",
            [],
        )
        .unwrap();
    super::single::call_reembed(
        &main_conn,
        "global",
        "mem-oracle",
        &paths,
        LlmBackendChoice::None,
        EmbeddingBackendChoice::Openrouter,
    )
    .expect("oracle re-embed succeeds");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        3,
        "the oracle costs exactly one extra request — the per-item rate the batch replaces"
    );
    let from_batch: Vec<u8> = main_conn
        .query_row(
            "SELECT embedding FROM memory_embeddings WHERE memory_id =
                 (SELECT id FROM memories WHERE name='mem-000')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let from_oracle: Vec<u8> = main_conn
        .query_row(
            "SELECT embedding FROM memory_embeddings WHERE memory_id =
                 (SELECT id FROM memories WHERE name='mem-oracle')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        from_batch, from_oracle,
        "batch and one-row-per-call must persist the same vector for the same body"
    );
    let _ = std::fs::remove_file(&queue_path);
}

/// GAP-SG-141 (B1): two workers claiming concurrently must partition the
/// backlog — no row may be handed to both. Previously untested.
#[test]
fn concurrent_batch_claims_are_mutually_exclusive() {
    let queue_path = temp_queue_path("excl");
    let seed = open_queue_db(&queue_path).expect("queue db opens");
    for i in 0..64 {
        enqueue_reembed(&seed, "global", &format!("mem-{i:03}"));
    }
    drop(seed);

    let path_a = queue_path.clone();
    let path_b = queue_path.clone();
    let (a, b) = std::thread::scope(|s| {
        let ha = s.spawn(move || claim_all(&path_a));
        let hb = s.spawn(move || claim_all(&path_b));
        (ha.join().expect("worker a"), hb.join().expect("worker b"))
    });

    let mut all: Vec<i64> = a.iter().chain(b.iter()).copied().collect();
    all.sort_unstable();
    let unique = {
        let mut u = all.clone();
        u.dedup();
        u
    };
    assert_eq!(
        all.len(),
        unique.len(),
        "a queue row was claimed by both workers"
    );
    assert_eq!(all.len(), 64, "every row must be claimed exactly once");
    let _ = std::fs::remove_file(&queue_path);
}

fn claim_all(queue_path: &str) -> Vec<i64> {
    let conn = open_queue_db(queue_path).expect("queue db opens");
    let mut ids = Vec::new();
    loop {
        let rows = match crate::storage::utils::with_busy_retry(|| {
            dequeue_batch_pending(&conn, "ReEmbed", "global", "", 8)
        }) {
            Ok(r) => r,
            Err(_) => break,
        };
        if rows.is_empty() {
            break;
        }
        ids.extend(rows.iter().map(|r| r.id));
    }
    ids
}

/// GAP-SG-141 (B1): a key that no longer resolves is recorded as skipped and
/// must NOT abort the claim it shares with healthy rows.
///
/// The healthy rows here all already hold a live vector, so the batch resolves
/// entirely offline and the assertion does not depend on a backend.
#[test]
fn unresolvable_key_does_not_abort_the_batch() {
    crate::constants::set_active_embedding_dim(TEST_DIM);
    let main_conn = open_main_db();
    let queue_path = temp_queue_path("resolve");
    let queue_conn = open_queue_db(&queue_path).expect("queue db opens");

    for i in 0..3 {
        let name = format!("live-{i}");
        main_conn
            .execute(
                "INSERT INTO memories (namespace, name, body) VALUES ('global', ?1, 'body')",
                rusqlite::params![name],
            )
            .unwrap();
        let id: i64 = main_conn.last_insert_rowid();
        insert_live_memory_vector(&main_conn, id);
        enqueue_reembed(&queue_conn, "global", &name);
    }
    enqueue_reembed(&queue_conn, "global", "ghost-memory");

    let paths = test_paths();
    let c = ctx(&main_conn, &queue_conn, &paths);
    let mut tally = ReembedTally::default();
    assert!(matches!(
        run_reembed_cycle(&c, &mut tally, None),
        ReembedCycle::Progressed
    ));

    assert_eq!(tally.completed, 3, "healthy rows still complete");
    assert_eq!(tally.skipped, 1, "the ghost key is skipped, not fatal");
    assert_eq!(
        tally.failed, 0,
        "an unresolvable key is not a batch failure"
    );

    let ghost_status: String = queue_conn
        .query_row(
            "SELECT status FROM queue WHERE item_key='ghost-memory'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ghost_status, "skipped");
    let _ = std::fs::remove_file(&queue_path);
}

/// CAPA-C1 preserved under batching: an item that already carries a live vector
/// at the active dim resolves offline and consumes no slot in the shared call.
///
/// No embed client is installed in this test, so any attempt to embed would
/// fail loudly instead of passing silently.
#[test]
fn item_with_live_vector_consumes_no_request() {
    crate::constants::set_active_embedding_dim(TEST_DIM);
    let main_conn = open_main_db();
    let queue_path = temp_queue_path("live");
    let queue_conn = open_queue_db(&queue_path).expect("queue db opens");

    main_conn
        .execute(
            "INSERT INTO memories (namespace, name, body) VALUES ('global', 'already-vectorised', 'body')",
            [],
        )
        .unwrap();
    let id = main_conn.last_insert_rowid();
    insert_live_memory_vector(&main_conn, id);
    enqueue_reembed(&queue_conn, "global", "already-vectorised");

    let paths = test_paths();
    let c = ctx(&main_conn, &queue_conn, &paths);
    let mut tally = ReembedTally::default();
    assert!(matches!(
        run_reembed_cycle(&c, &mut tally, None),
        ReembedCycle::Progressed
    ));
    assert_eq!(tally.completed, 1);
    assert_eq!(tally.failed, 0, "no embedding call may have been attempted");

    let status: String = queue_conn
        .query_row(
            "SELECT status FROM queue WHERE item_key='already-vectorised'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "done");
    let _ = std::fs::remove_file(&queue_path);
}

fn insert_live_memory_vector(conn: &Connection, memory_id: i64) {
    let bytes = crate::embedder::f32_to_bytes(&vec![0.02_f32; TEST_DIM]);
    conn.execute(
        "INSERT INTO memory_embeddings (memory_id, namespace, embedding, source, dim)
         VALUES (?1, 'global', ?2, 'test', ?3)",
        rusqlite::params![memory_id, bytes, TEST_DIM as i64],
    )
    .unwrap();
}

/// GAP-SG-141 (B1): the claim width is a named constant plus an XDG key, never
/// a literal, and it is clamped rather than rejected.
#[test]
fn claim_batch_width_defaults_to_the_named_constant() {
    assert_eq!(crate::constants::DEFAULT_REEMBED_CLAIM_BATCH, 32);
    assert!(crate::constants::REEMBED_CLAIM_BATCH_RANGE.contains(&32));
    assert!(crate::config::SETTING_KEYS
        .iter()
        .any(|k| k.key == "enrich.reembed_claim_batch"));
}
