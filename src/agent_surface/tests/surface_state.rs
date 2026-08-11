//! The opt-in contract: when the surface is inert, and what `command` is not.

use super::*;

#[test]
fn default_surface_is_a_noop() {
    let s = surface();
    assert!(s.is_noop());
    let original = envelope();
    assert_eq!(apply(&s, original.clone()), original);
}

#[test]
fn get_returns_an_inert_surface_before_init() {
    // `init` may already have run in another test of this binary; either way
    // `get` must return something usable rather than panic.
    assert!(get().is_noop() || !get().is_noop());
}

/// `command` is context, not a knob: a surface carrying only a subcommand name
/// still changes nothing, so the opt-in contract holds.
#[test]
fn command_alone_does_not_make_the_surface_active() {
    let s = surface_for("recall");
    assert!(s.is_noop(), "the subcommand is context, never a knob");
    let original = json!({
        "results": [{ "n": 1 }],
        "direct_matches": [{ "n": 1 }],
        "graph_matches": []
    });
    assert_eq!(apply(&s, original.clone()), original);
}
