//! `--truncate-content` and the top-level `truncated` flag it raises.

use super::*;

#[test]
fn truncate_content_shortens_strings_and_flags_the_envelope() {
    let mut s = surface();
    s.truncate_content = 4;
    let shaped = apply(&s, envelope());
    for item in results(&shaped) {
        assert!(item["snippet"].as_str().unwrap().chars().count() <= 4);
    }
    assert_eq!(shaped["truncated"], json!(true));
    assert_eq!(shaped["agent_surface"]["content_truncated"], json!(true));
    assert_eq!(shaped["agent_surface"]["truncate_content"], json!(4));
}

#[test]
fn truncate_content_never_splits_a_utf8_sequence() {
    let mut s = surface();
    s.truncate_content = 3;
    let shaped = apply(&s, json!({ "results": [ { "s": "ãéîõü" } ] }));
    let cut = results(&shaped)[0]["s"].as_str().unwrap();
    assert_eq!(cut, "ãéî");
}

#[test]
fn truncate_content_leaves_short_strings_and_the_flag_alone() {
    let mut s = surface();
    s.truncate_content = 1000;
    let shaped = apply(&s, envelope());
    assert!(shaped.get("truncated").is_none());
}

/// A command that ships its own `truncated: false` must not be able to mask a
/// removal the surface performed.
///
/// This assertion is the inverse of the one it replaces. The old test pinned
/// `false` surviving, which read as caution and was in fact the bug: `list`
/// declares `truncated` as a plain `bool`, so the member is always present and
/// the guard meant the surface could never raise the flag there — on the most
/// used command in the binary, against a module doc that promises removal is
/// never silent.
#[test]
fn a_removal_raises_truncated_even_over_a_command_that_shipped_false() {
    let mut s = surface();
    s.truncate_content = 2;
    let shaped = apply(
        &s,
        json!({ "truncated": false, "results": [{ "s": "abcdef" }] }),
    );
    assert_eq!(shaped["truncated"], json!(true));
    assert_eq!(shaped["agent_surface"]["content_truncated"], json!(true));
}

/// The flag is monotonic: a command that already truncated its own rows keeps
/// saying so even when the surface removed nothing.
#[test]
fn an_existing_true_is_never_written_back_to_false() {
    let mut s = surface();
    s.select = vec!["s".into()];
    let shaped = apply(
        &s,
        json!({ "truncated": true, "results": [{ "s": "abcdef" }] }),
    );
    assert_eq!(shaped["truncated"], json!(true));
}
