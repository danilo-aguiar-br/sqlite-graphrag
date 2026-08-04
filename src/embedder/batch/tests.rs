//! Allocation and partition invariants of the batch fan-out.

use super::entity_cache::{entity_cache_key, EntityEmbedCacheMap};
use super::fan_out::{chunk_ranges, fan_out_chunk, reassemble_ordered, EMBED_FAN_OUT_CHUNK};
use super::sizing::{adaptive_batch_for_dim, effective_permits, CHUNK_EMBED_BATCH_SIZE};
use crate::constants::{
    joint_parallelism_ceiling, joint_parallelism_ceiling_for, max_total_llm_workers,
};
use std::sync::Arc;

#[test]
fn arc_from_vec_moves_string_buffers_instead_of_copying() {
    // This is the whole point of GAP-SG-147: the corpus must reach the
    // spawned tasks without its heap data being copied. Comparing the
    // buffer pointer before and after proves the move — a clone would
    // allocate a fresh buffer at a different address.
    let corpus: Vec<String> = (0..64)
        .map(|i| format!("passage number {i} with enough bytes to force a heap allocation"))
        .collect();
    let before: Vec<*const u8> = corpus.iter().map(|s| s.as_ptr()).collect();

    let shared: Arc<[String]> = Arc::from(corpus);

    for (i, ptr) in before.iter().enumerate() {
        assert_eq!(
            shared[i].as_ptr(),
            *ptr,
            "string {i} was copied, not moved into the Arc"
        );
    }
}

#[test]
fn arc_clone_per_task_does_not_touch_the_data() {
    let shared: Arc<[String]> = Arc::from(vec!["a".to_string(), "b".to_string()]);
    let ptr = shared[0].as_ptr();
    let handles: Vec<Arc<[String]>> = (0..8).map(|_| Arc::clone(&shared)).collect();
    assert_eq!(Arc::strong_count(&shared), 9);
    for h in &handles {
        assert_eq!(
            h[0].as_ptr(),
            ptr,
            "Arc::clone must be a refcount bump only"
        );
    }
}

#[test]
fn fan_out_and_inner_http_chunking_read_the_same_knob() {
    // GAP-SG-142: the outer fan-out slice and the client's internal
    // re-chunking must resolve from ONE key. If they ever diverge, raising
    // `embedding.batch_size` becomes a silent no-op because the outer layer
    // never hands the inner one enough texts to split.
    assert_eq!(
        fan_out_chunk(),
        crate::runtime_config::embedding_batch_size(EMBED_FAN_OUT_CHUNK),
        "fan-out chunk must resolve through embedding.batch_size"
    );
}

#[test]
fn openrouter_branch_ignores_local_batch_size() {
    // The OpenRouter branch MUST size requests from `embedding.batch_size`,
    // never from the dim-adaptive value the caller passes. Unifying them
    // looks like removing an inconsistency and is actually a regression:
    // `adaptive_batch_for_dim(8, 1024)` is 1, so the REST path would drop
    // to one text per call and undo the 32x batching of GAP-SG-141.
    let src: String = include_str!("passages.rs")
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = src
        .split("pub(crate) fn embed_passages_parallel_shared")
        .nth(1)
        .expect("the shared implementation must exist");
    // Start AFTER the parameter list, or the signature itself would match.
    let openrouter_branch = body
        .split("let chain = embedding_backend.to_chain(llm_backend);")
        .nth(1)
        .expect("the branch begins after the chain is resolved")
        .split("} else {")
        .next()
        .expect("the OpenRouter branch precedes the not-initialised error");
    assert!(
        !openrouter_branch.contains("local_batch_size"),
        "the OpenRouter branch must not read local_batch_size; it sizes \
         requests from embedding.batch_size via fan_out_chunk()"
    );
    // The parameter survives only as published-signature ballast now that
    // the subprocess branch is gone: it is accepted and deliberately unread.
    assert!(
        body.contains("_local_batch_size: usize"),
        "the parameter must stay in the signature, explicitly marked unused"
    );
}

#[test]
fn dim_adaptive_batch_collapses_to_one_at_the_active_dimensionality() {
    // The number that makes the branch split non-negotiable. If this ever
    // stops being 1, revisit whether the two paths could share a knob.
    assert_eq!(adaptive_batch_for_dim(CHUNK_EMBED_BATCH_SIZE, 1024), 1);
}

#[test]
fn fan_out_chunk_is_never_zero() {
    // `chunk_ranges` clamps too, but the resolver must not emit 0 either:
    // a zero would make the serial short-circuit `texts.len() <= 0` false
    // for every non-empty corpus and push everything through the fan-out.
    assert!(fan_out_chunk() >= 1);
}

#[test]
fn chunk_ranges_reproduces_slice_chunks_exactly() {
    // The fan-out swapped `texts.chunks(32)` for index ranges; if the two
    // ever disagree, chunk contents silently shift.
    for len in [0usize, 1, 31, 32, 33, 64, 65, 1000] {
        let corpus: Vec<usize> = (0..len).collect();
        let by_slice: Vec<Vec<usize>> = corpus
            .chunks(EMBED_FAN_OUT_CHUNK)
            .map(|c| c.to_vec())
            .collect();
        let by_range: Vec<Vec<usize>> = chunk_ranges(len, EMBED_FAN_OUT_CHUNK)
            .map(|r| corpus[r].to_vec())
            .collect();
        assert_eq!(by_range, by_slice, "partition diverged at len={len}");
    }
}

#[test]
fn chunk_ranges_survives_a_zero_chunk_size() {
    // `step_by(0)` panics, so the helper clamps. Guards against a caller
    // threading a zeroed batch size through.
    let ranges: Vec<_> = chunk_ranges(3, 0).collect();
    assert_eq!(ranges, vec![0..1, 1..2, 2..3]);
}

#[test]
fn chunk_index_still_drives_reassembly_order() {
    // `reassemble_ordered` keys on the chunk index produced by
    // `enumerate()`; feeding it out of order must still restore input
    // order, which is what makes the index-range fan-out safe.
    let parts = vec![
        (2usize, vec![vec![2.0f32]]),
        (0usize, vec![vec![0.0f32]]),
        (1usize, vec![vec![1.0f32]]),
    ];
    assert_eq!(
        reassemble_ordered(parts),
        vec![vec![0.0f32], vec![1.0f32], vec![2.0f32]]
    );
}

// ---------------------------------------------------------------------------
// Joint concurrency cap: `--max-concurrency` × `--llm-parallelism`
// ---------------------------------------------------------------------------

#[test]
fn joint_ceiling_bounds_the_product_of_both_knobs() {
    // The defect this closes: each knob was validated alone — concurrency
    // against 2 × nCPUs, parallelism against 32 — so their PRODUCT was
    // unbounded. On a 16-core host that authorised 32 × 32 = 1024 workers.
    let budget = max_total_llm_workers();
    for max_concurrency in [1usize, 2, 4, 8, 16, 32, 64, 512] {
        let per_process = joint_parallelism_ceiling_for(max_concurrency);
        assert!(
            per_process >= 1,
            "a joint cap that forbids all work is a deadlock, not a bound"
        );
        // `max(1)` on the per-process share means a concurrency above the whole
        // budget cannot go below one worker each; the product is then exactly
        // `max_concurrency`, which is already bounded by 2 × nCPUs.
        let product = per_process * max_concurrency;
        assert!(
            product <= budget || per_process == 1,
            "max_concurrency={max_concurrency} × parallelism={per_process} = {product} \
             exceeds the joint budget of {budget}"
        );
    }
}

#[test]
fn joint_ceiling_shrinks_as_concurrency_grows() {
    // Monotonicity is the property that makes the cap a real trade: buying more
    // processes must cost fan-out width, never come for free.
    let mut previous = usize::MAX;
    for max_concurrency in [1usize, 2, 4, 8, 16] {
        let current = joint_parallelism_ceiling_for(max_concurrency);
        assert!(
            current <= previous,
            "ceiling grew from {previous} to {current} at max_concurrency={max_concurrency}"
        );
        previous = current;
    }
}

#[test]
fn effective_permits_never_exceeds_the_joint_ceiling() {
    // `effective_permits` is one of the two places a requested parallelism
    // becomes real permits; the other is the `k` in `passages.rs`. Both must
    // honour the cap or the bound is decorative.
    for requested in [0usize, 1, 4, 32, 1000] {
        let permits = effective_permits(requested);
        assert!(permits >= 1, "permits must never reach zero");
        assert!(
            permits <= joint_parallelism_ceiling(),
            "requested={requested} produced {permits} permits, above the joint ceiling"
        );
        assert!(permits <= 32, "the historical per-knob clamp still applies");
    }
}

// ---------------------------------------------------------------------------
// Entity-embedding cache: entry ceiling and TTL
// ---------------------------------------------------------------------------

#[test]
fn entity_cache_stays_under_the_entry_ceiling() {
    // The cache used to be an unbounded HashMap with no TTL: a long ingest over
    // many distinct entity names grew it for the whole invocation.
    let ceiling = crate::constants::entity_embed_cache_max_entries();
    let mut cache = EntityEmbedCacheMap::default();
    // Insert well past the ceiling in batches, evicting exactly where the
    // production path does — right before each insert batch.
    let batch = 64usize;
    let mut inserted = 0usize;
    while inserted < ceiling + 3 * batch {
        cache.evict_expired_and_overflow(batch);
        for i in 0..batch {
            cache.insert(
                entity_cache_key("test-model", &format!("entity-{}", inserted + i)),
                Arc::new(vec![0.0f32; 4]),
            );
        }
        inserted += batch;
        assert!(
            cache.len() <= ceiling,
            "cache holds {} entries, above the ceiling of {ceiling}",
            cache.len()
        );
    }
}

#[test]
fn entity_cache_keeps_the_newest_entries_when_it_trims() {
    // Oldest-first eviction: the entry written last must survive a trim, which
    // is what makes the bound compatible with a hot working set.
    let mut cache = EntityEmbedCacheMap::default();
    let ceiling = crate::constants::entity_embed_cache_max_entries();
    for i in 0..ceiling {
        cache.insert(
            entity_cache_key("test-model", &format!("old-{i}")),
            Arc::new(vec![0.0f32; 4]),
        );
    }
    let newest = entity_cache_key("test-model", "newest");
    cache.insert(newest, Arc::new(vec![1.0f32; 4]));
    cache.evict_expired_and_overflow(1);
    assert!(
        cache.get(&newest).is_some(),
        "the most recent entry must not be the one evicted"
    );
}

#[test]
fn entity_cache_hit_requires_a_live_ttl() {
    // A hit is only valid while the entry is inside its TTL. The default TTL is
    // an hour, so a fresh insert must read back; the expiry branch itself is
    // covered by `evict_expired_and_overflow` dropping stale rows.
    let mut cache = EntityEmbedCacheMap::default();
    let key = entity_cache_key("test-model", "fresh");
    cache.insert(key, Arc::new(vec![0.42f32; 4]));
    let hit = cache.get(&key).expect("a just-written entry must be live");
    assert_eq!(hit.len(), 4);
    assert!((hit[0] - 0.42).abs() < 1e-6);
    assert!(
        crate::constants::entity_embed_cache_ttl_secs() > 0,
        "a zero TTL would make every entry a miss"
    );
}
