//! GAP-SG-211: `cargo fmt --check` was never part of the only automatic gate.
//!
//! `no_ci_workflows_gate` states the design this repository actually follows:
//! there is no CI, `cargo test` is the only automatic gate, and every other
//! guard lives in `tests/` rather than in a pipeline step. Formatting was the
//! exception. It was enforced only by a pre-publish script outside the repo,
//! which means it was enforced only when somebody remembered to invoke it.
//!
//! The v1.2.7 pre-publish run found drift in five places, all of it in code
//! written during the session that had just declared the release ready. That is
//! the signature of a rule nothing checks: it holds until attention lapses.
//!
//! `cargo fmt` reads and rewrites source files without touching `target/` and
//! without taking the build lock, so running it from inside a test costs no
//! contention with the compilation that is already in flight.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Repository root, resolved from the manifest rather than the current dir.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The cargo that is driving this test run, so the pinned toolchain is used.
fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

#[test]
fn the_workspace_is_rustfmt_clean() {
    let out = Command::new(cargo_bin())
        .args(["fmt", "--all", "--check"])
        .current_dir(root())
        .output()
        .expect(
            "could not run `cargo fmt`. `rust-toolchain.toml` pins the toolchain and lists \
             `rustfmt` among its components, so a host without it is not running the toolchain \
             this project declares. Deliberately not skipped: a formatting gate that disappears \
             when the formatter is missing checks nothing on exactly the hosts that need it.",
        );

    if out.status.success() {
        return;
    }

    // `cargo fmt --check` writes the offending diff to stdout and keeps stderr
    // for its own errors, so both are worth surfacing when this fails.
    panic!(
        "`cargo fmt --all --check` reported drift. Run `cargo fmt --all` and commit the result.\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
