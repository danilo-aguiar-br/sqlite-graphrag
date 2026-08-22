//! The Explicit Target Designation fence, under test for the first time.
//!
//! # Why this file exists
//!
//! GAP-SG-207 installed a fence that refuses a mutating verb whose database was
//! never named in the argv, and the fence was later widened from
//! `TargetSource::Default` alone to `TargetSource::Xdg` as well. That widening
//! is correct — `db.path` is a HOST key, so it names one database for every
//! directory on the machine and cannot designate the target of a single write.
//!
//! It also went in without a single test of its own. A scan for
//! `target_not_designated` and `target_inherited_from_config` across `tests/`
//! returned nothing, while 243 tests in 50 targets went red because the shared
//! harness plants `db.path` and passes no dispensation. A rule that breaks the
//! suite and is itself unguarded is one careless revert away from silently
//! disappearing, which is exactly what this file prevents.
//!
//! # What is asserted, and why in this shape
//!
//! Four cases, one per branch the fence can take: the two refusals differ in
//! MESSAGE because the operator's next move differs, and the two acceptances
//! differ in MECHANISM — one names the target, the other accepts the ambient one
//! on purpose.
//!
//! The acceptance cases assert the ABSENCE of the refusal rather than a
//! successful outcome. That is deliberate: what is under test is the fence, not
//! the verb behind it, and asserting the verb's own exit code would couple this
//! gate to unrelated behaviour.

mod common;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

/// Fragment unique to `target_not_designated` in English.
const NOTHING_NAMED_IT: &str = "NOTHING named its target";

/// Fragment unique to `target_inherited_from_config` in English.
///
/// Deliberately short of the preceding "That key is": the catalogue wraps the
/// literal across lines with a backslash continuation, so a longer fragment
/// would match the rendered message and miss the source, breaking the
/// self-check at the bottom of this file for a reason that is not a defect.
const CAME_FROM_THE_KEY: &str = "a HOST setting";

/// The usage exit code both refusals share, per `src/constants/exit_codes.rs`.
const USAGE_EXIT: i32 = 2;

/// Builds a command isolated from the host, with the language pinned.
///
/// The language is pinned because the refusal text is translated and the host
/// locale drives it: without this the assertions below would pass on an English
/// machine and fail on a Portuguese one, which is the developer's machine here.
fn isolated(tmp: &TempDir) -> Command {
    let root = tmp.path();
    let mut c = Command::cargo_bin("sqlite-graphrag").expect("binary must be built");
    c.env("HOME", root.join("home"))
        .env("XDG_CACHE_HOME", root.join("xdg_cache"))
        .env("XDG_CONFIG_HOME", root.join("xdg_config"))
        .env("XDG_DATA_HOME", root.join("xdg_data"))
        .env("XDG_RUNTIME_DIR", root.join("xdg_runtime"))
        .arg("--lang")
        .arg("en")
        .arg("--config-dir")
        .arg(root.join("config"))
        .arg("--cache-dir")
        .arg(root.join("cache"))
        .arg("--skip-memory-guard");
    c
}

/// Everything the process wrote, both streams joined.
///
/// The refusal travels in the JSON envelope on stdout while tracing goes to
/// stderr, and which one carries it is not what this file is about.
fn spoken(out: &std::process::Output) -> String {
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
#[serial]
fn a_write_with_no_target_anywhere_is_refused() {
    let tmp = TempDir::new().expect("temp dir");
    // No `db.path` planted: nothing anywhere names a database.
    let out = isolated(&tmp)
        .args(["forget", "--name", "whatever"])
        .output()
        .expect("process must run");

    assert_eq!(
        out.status.code(),
        Some(USAGE_EXIT),
        "a mutating verb with no target named anywhere must exit {USAGE_EXIT}; got: {}",
        spoken(&out)
    );
    let said = spoken(&out);
    assert!(
        said.contains(NOTHING_NAMED_IT),
        "the refusal must be the one about NOTHING naming the target; got: {said}"
    );
}

#[test]
#[serial]
fn a_write_inheriting_the_host_key_is_refused_with_its_own_message() {
    let tmp = TempDir::new().expect("temp dir");
    let db = tmp.path().join("planted.sqlite");
    common::write_sandbox_config(&tmp.path().join("config"), Some(&db));

    let out = isolated(&tmp)
        .args(["forget", "--name", "whatever"])
        .output()
        .expect("process must run");

    assert_eq!(
        out.status.code(),
        Some(USAGE_EXIT),
        "a mutating verb resolving through `db.path` must exit {USAGE_EXIT}; got: {}",
        spoken(&out)
    );
    let said = spoken(&out);
    assert!(
        said.contains(CAME_FROM_THE_KEY),
        "the refusal must be the one explaining the HOST scope of the key; got: {said}"
    );
    assert!(
        !said.contains(NOTHING_NAMED_IT),
        "the two refusals must stay distinct: something DID name a database here"
    );
}

#[test]
#[serial]
fn naming_the_target_in_the_argv_passes_the_fence() {
    let tmp = TempDir::new().expect("temp dir");
    let db = tmp.path().join("named.sqlite");
    common::write_sandbox_config(&tmp.path().join("config"), Some(&db));

    let out = isolated(&tmp)
        .args(["forget", "--name", "whatever", "--db"])
        .arg(&db)
        .output()
        .expect("process must run");

    let said = spoken(&out);
    assert!(
        !said.contains(NOTHING_NAMED_IT) && !said.contains(CAME_FROM_THE_KEY),
        "an argv-named target must never hit the fence; got: {said}"
    );
}

#[test]
#[serial]
fn the_declared_dispensation_passes_the_fence() {
    let tmp = TempDir::new().expect("temp dir");
    let db = tmp.path().join("planted.sqlite");
    common::write_sandbox_config(&tmp.path().join("config"), Some(&db));

    let out = isolated(&tmp)
        .args(["--use-active", "forget", "--name", "whatever"])
        .output()
        .expect("process must run");

    let said = spoken(&out);
    assert!(
        !said.contains(NOTHING_NAMED_IT) && !said.contains(CAME_FROM_THE_KEY),
        "--use-active is the dispensation the rule itself authorises; got: {said}"
    );
}

/// Guards the guard: the fragments above must still exist in the product.
///
/// Without this, renaming a refusal message would turn every assertion in this
/// file into a tautology — `contains` would go on failing for the refusals and
/// the two acceptance tests would pass over text that no longer exists, so the
/// file would report a fence it stopped watching.
#[test]
fn the_fragments_this_gate_matches_on_still_exist_in_the_source() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/i18n/validation/messages_agent_surface.rs"
    ))
    .expect("the message catalogue must be readable");

    for fragment in [NOTHING_NAMED_IT, CAME_FROM_THE_KEY] {
        assert!(
            src.contains(fragment),
            "this gate matches on '{fragment}', which is no longer in the catalogue; \
             update the constant instead of deleting the assertion"
        );
    }
}
