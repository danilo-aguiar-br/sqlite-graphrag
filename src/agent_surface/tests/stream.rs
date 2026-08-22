//! GAP-SG-215: the NDJSON stream contract.
//!
//! Every assertion here is on OBSERVABLE output — the shaped record, the shaped
//! trailer, the exit code — rather than on `StreamState`'s fields, which are
//! private. That is deliberate: the three defects this closes were all visible
//! in what a consumer receives, and a test that reaches past the emitted value
//! to inspect internals would have passed while the stream stayed broken.

use super::*;
use crate::agent_surface::stream;

/// The record shape `export` emits: no arrays, a fixed key set.
///
/// Synthetic throughout. The defect was found against a real corpus, and the
/// fixture that reproduces it must never be a copy of what was observed there.
fn records() -> Vec<Value> {
    vec![
        json!({
            "name": "alice-martins-souza",
            "type": "note",
            "namespace": "global",
            "body": "abcdefghijklmnop"
        }),
        json!({
            "name": "alice-onboarding-checklist",
            "type": "rule",
            "namespace": "global",
            "body": "qrstuvwxyz"
        }),
    ]
}

/// The line that ends the stream. Its keys are DISJOINT from a record's except
/// for `namespace` — which is exactly what made both measured defects possible.
fn trailer() -> Value {
    json!({ "summary": true, "exported": 2, "namespace": "global", "elapsed_ms": 12 })
}

fn streamed() -> AgentSurface {
    AgentSurface {
        streamed: true,
        ..AgentSurface::default()
    }
}

fn open(surface: &AgentSurface) -> stream::StreamState {
    let sample = records();
    stream::open_with(surface, &sample, sample.len()).expect("this stream must open")
}

fn refusal(surface: &AgentSurface) -> (String, Vec<String>) {
    let sample = records();
    let err =
        stream::open_with(surface, &sample, sample.len()).expect_err("this stream must be refused");
    assert_eq!(err.exit_code(), 2, "usage errors exit 2: {err}");
    let flags = match &err {
        crate::errors::AppError::Usage {
            discarded_flags, ..
        } => discarded_flags.clone(),
        other => panic!("expected a usage refusal, got {other}"),
    };
    (err.to_string(), flags)
}

fn meta(value: &Value) -> &Map<String, Value> {
    value
        .get(META_KEY)
        .and_then(Value::as_object)
        .expect("the trailer must carry the stream's record")
}

#[test]
fn every_whole_set_knob_is_refused_before_the_first_line() {
    // One case per knob, because the point is that NONE of them slips through:
    // `--count-only` answered `{"count":1}` once per line, and `--max-items` was
    // accepted and did nothing at all.
    let cases: Vec<(&str, AgentSurface)> = vec![
        (
            "--count-only",
            AgentSurface {
                count_only: true,
                ..streamed()
            },
        ),
        (
            "--sort",
            AgentSurface {
                sort: Some("name".into()),
                ..streamed()
            },
        ),
        (
            "--dedupe-by",
            AgentSurface {
                dedupe_by: Some("name".into()),
                ..streamed()
            },
        ),
        (
            "--max-output-bytes",
            AgentSurface {
                max_output_bytes: 4_096,
                ..streamed()
            },
        ),
        (
            "--max-items",
            AgentSurface {
                max_items: 2,
                ..streamed()
            },
        ),
    ];
    for (flag, surface) in cases {
        let (_, flags) = refusal(&surface);
        assert!(
            flags.iter().any(|f| f == flag),
            "{flag} must be named in discarded_flags, got {flags:?}"
        );
    }
}

#[test]
fn a_predicate_is_refused_because_it_would_desync_the_tally() {
    let surface = AgentSurface {
        filters: vec![FilterExpr::parse("type=rule").expect("a valid predicate")],
        ..streamed()
    };
    let (message, flags) = refusal(&surface);
    assert_eq!(flags, vec!["--filter".to_string()]);
    // The REASON has to be the tally, not "nothing to act on". Until v1.2.8 the
    // right verdict came from the wrong guard, and a caller told "there is no
    // result array here" learns the wrong lesson about a stream.
    assert!(
        message.contains("--type") || message.contains("--limit"),
        "the refusal must point at the query's own narrowing flags: {message}"
    );
}

#[test]
fn a_select_no_record_carries_is_refused_before_the_first_line() {
    let surface = AgentSurface {
        select: vec!["no_such_key".into()],
        ..streamed()
    };
    let (_, flags) = refusal(&surface);
    assert_eq!(flags, vec!["--select".to_string()]);
}

#[test]
fn per_record_knobs_open_the_stream() {
    let surface = AgentSurface {
        select: vec!["name".into()],
        truncate_content: 4,
        ..streamed()
    };
    let _ = open(&surface);
}

#[test]
fn a_record_is_never_annotated() {
    // The invariant `agent_surface`'s module docs have declared since GAP-SG-142
    // and that nothing enforced: a stream line carries the record and nothing
    // else. Measured at 278 bytes per line of `agent_surface` before this.
    let surface = AgentSurface {
        select: vec!["name".into()],
        truncate_content: 4,
        ..streamed()
    };
    let state = open(&surface);
    for record in records() {
        let shaped = stream::shape_record_with(&state, &surface, record);
        assert!(
            shaped.get(META_KEY).is_none(),
            "a record line must carry no agent_surface: {shaped}"
        );
        assert!(
            shaped.get(TRUNCATED_KEY).is_none(),
            "a record line must carry no truncated flag: {shaped}"
        );
        assert_eq!(
            shaped.as_object().map(Map::len),
            Some(1),
            "--select name must leave exactly one member: {shaped}"
        );
    }
}

#[test]
fn an_unflagged_record_passes_through_byte_for_byte() {
    let surface = streamed();
    let state = open(&surface);
    for record in records() {
        let shaped = stream::shape_record_with(&state, &surface, record.clone());
        assert_eq!(shaped, record, "an inert surface must not touch a record");
    }
}

#[test]
fn the_trailer_survives_a_select_aimed_at_the_records() {
    // The SILENT defect, and the worse of the two: `--select namespace export`
    // exited 0 and left the summary as `{"namespace":…}`, deleting `summary:
    // true` — the only end-of-stream signal a consumer has. A truncated export
    // then reads as a complete one.
    let surface = AgentSurface {
        select: vec!["namespace".into()],
        ..streamed()
    };
    let state = open(&surface);
    let shaped = stream::trailer_with(&state, &surface, None, trailer());
    for key in ["summary", "exported", "namespace", "elapsed_ms"] {
        assert!(
            shaped.get(key).is_some(),
            "the trailer must keep {key}, which its schema marks required: {shaped}"
        );
    }
    assert_eq!(shaped.get("summary"), Some(&Value::Bool(true)));
}

#[test]
fn the_trailer_never_fails_on_a_key_only_records_carry() {
    // The REPORTED defect. `--select name` resolves against every record and
    // against nothing in the trailer, and the old per-line path answered that by
    // refusing on the last line — after three good ones had reached stdout.
    let surface = AgentSurface {
        select: vec!["name".into()],
        ..streamed()
    };
    let state = open(&surface);
    let shaped = stream::trailer_with(&state, &surface, None, trailer());
    assert_eq!(shaped.get("summary"), Some(&Value::Bool(true)));
    assert_eq!(meta(&shaped).get("stream"), Some(&Value::Bool(true)));
}

#[test]
fn the_trailer_carries_the_process_record() {
    let surface = streamed();
    let state = open(&surface);
    let mut target = Map::new();
    target.insert("db_path_source".into(), json!("argv"));
    let shaped = stream::trailer_with(&state, &surface, Some(target), trailer());
    assert_eq!(meta(&shaped).get("db_path_source"), Some(&json!("argv")));
}

#[test]
fn a_bounded_sample_declares_itself_partial() {
    // The prefix is a memory bound, not a claim of completeness. A record
    // measured ~24 KB as a `Value`, so judging all of them at the default
    // --limit 100000 would cost ~2.4 GB; the honest answer is to say so.
    let surface = AgentSurface {
        select: vec!["name".into()],
        ..streamed()
    };
    let sample = records();
    let state = stream::open_with(&surface, &sample, 7_085).expect("this stream must open");
    let shaped = stream::trailer_with(&state, &surface, None, trailer());
    assert_eq!(
        meta(&shaped).get("vocabulary_partial"),
        Some(&Value::Bool(true)),
        "a stream longer than its sample must say the vocabulary was a prefix"
    );
}

#[test]
fn a_sample_that_covered_the_stream_claims_nothing_extra() {
    let surface = AgentSurface {
        select: vec!["name".into()],
        ..streamed()
    };
    let state = open(&surface);
    let shaped = stream::trailer_with(&state, &surface, None, trailer());
    assert!(
        meta(&shaped).get("vocabulary_partial").is_none(),
        "a fully sampled stream must not flag a partial vocabulary: {shaped}"
    );
}

#[test]
fn a_shortened_record_is_counted_on_the_trailer() {
    // Truncation is never silent — the module's oldest promise. With record
    // lines now unannotated, the trailer is the only place left to keep it.
    let surface = AgentSurface {
        truncate_content: 4,
        ..streamed()
    };
    let state = open(&surface);
    for record in records() {
        let _ = stream::shape_record_with(&state, &surface, record);
    }
    let shaped = stream::trailer_with(&state, &surface, None, trailer());
    assert_eq!(meta(&shaped).get("records_truncated"), Some(&json!(2)));
    assert_eq!(shaped.get(TRUNCATED_KEY), Some(&Value::Bool(true)));
}

#[test]
fn an_untouched_stream_reports_no_truncation() {
    let surface = AgentSurface {
        truncate_content: 4_096,
        ..streamed()
    };
    let state = open(&surface);
    for record in records() {
        let _ = stream::shape_record_with(&state, &surface, record);
    }
    let shaped = stream::trailer_with(&state, &surface, None, trailer());
    assert!(meta(&shaped).get("records_truncated").is_none());
    assert!(shaped.get(TRUNCATED_KEY).is_none());
}

#[test]
fn a_write_stream_is_never_refused() {
    // The fence, unchanged. `ingest` streams AND writes; refusing at emission
    // time would report failure for files already persisted, and a caller that
    // retries a succeeded ingest writes them twice.
    let surface = AgentSurface {
        mutates: true,
        count_only: true,
        max_items: 2,
        filters: vec![FilterExpr::parse("type=rule").expect("a valid predicate")],
        ..streamed()
    };
    let sample = records();
    stream::open_with(&surface, &sample, sample.len())
        .expect("a write stream must never be refused");
}

#[test]
fn allow_unknown_keys_still_lets_a_bad_select_through() {
    let surface = AgentSurface {
        select: vec!["no_such_key".into()],
        allow_unknown_keys: true,
        ..streamed()
    };
    let _ = open(&surface);
}
