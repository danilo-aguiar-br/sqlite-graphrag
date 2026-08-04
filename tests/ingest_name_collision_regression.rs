//! GAP-SG-54 / GAP-SG-148: name collision under standard ingest without silent overwrite.
//!
//! Two files that derive the same memory name must:
//! - without `--force-merge`: leave the first body intact and report the second
//!   as skipped/duplicate (never a silent overwrite);
//! - with `--force-merge`: update the existing memory body explicitly.
//!
//! File renamed in v1.2.4 from `ingest_claude_name_collision_regression.rs`:
//! `IngestMode` no longer offers `--mode claude-code`, so the suite exercises
//! the standard ingest path offline (`--llm-backend none`, no API key).

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn bin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sqlite-graphrag"));
    c.env_remove("OPENROUTER_API_KEY");
    c.env_remove("SQLITE_GRAPHRAG_API_KEY");
    c
}

fn write_pair(root: &std::path::Path) {
    let a = root.join("a");
    let b = root.join("b");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    fs::write(a.join("shared-topic.md"), "# one\n\nbody-alpha\n").unwrap();
    fs::write(b.join("shared-topic.md"), "# two\n\nbody-beta-updated\n").unwrap();
}

#[test]
fn same_derived_name_without_force_merge_does_not_overwrite() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t.sqlite");
    write_pair(tmp.path());

    let init = bin()
        .args(["init", "--db"])
        .arg(&db)
        .arg("-q")
        .status()
        .expect("init");
    assert!(init.success(), "init must succeed");

    let out1 = bin()
        .args(["ingest", "--db"])
        .arg(&db)
        .arg(tmp.path().join("a"))
        .args([
            "--pattern",
            "*.md",
            "-q",
            "--no-input",
            "--llm-backend",
            "none",
        ])
        .output()
        .expect("ingest a");
    assert!(
        out1.status.success(),
        "first ingest: {}",
        String::from_utf8_lossy(&out1.stderr)
    );

    let out2 = bin()
        .args(["ingest", "--db"])
        .arg(&db)
        .arg(tmp.path().join("b"))
        .args([
            "--pattern",
            "*.md",
            "-q",
            "--no-input",
            "--llm-backend",
            "none",
        ])
        .output()
        .expect("ingest b");
    // Second run may exit 0 with skipped lines or non-zero on hard fail — body
    // must remain the first write either way.
    let body = bin()
        .args(["read", "--db"])
        .arg(&db)
        .args(["--name", "shared-topic", "--format", "raw", "-q"])
        .output()
        .expect("read");
    assert!(
        body.status.success(),
        "read: {}",
        String::from_utf8_lossy(&body.stderr)
    );
    let text = String::from_utf8_lossy(&body.stdout);
    assert!(
        text.contains("body-alpha"),
        "without --force-merge the first body must survive; got: {text:?}; stderr2={}",
        String::from_utf8_lossy(&out2.stderr)
    );
    assert!(
        !text.contains("body-beta-updated"),
        "without --force-merge must not silently overwrite with second file"
    );
}

#[test]
fn same_derived_name_with_force_merge_updates_body() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t.sqlite");
    write_pair(tmp.path());

    assert!(bin()
        .args(["init", "--db"])
        .arg(&db)
        .arg("-q")
        .status()
        .unwrap()
        .success());

    assert!(bin()
        .args(["ingest", "--db"])
        .arg(&db)
        .arg(tmp.path().join("a"))
        .args([
            "--pattern",
            "*.md",
            "-q",
            "--no-input",
            "--llm-backend",
            "none"
        ])
        .status()
        .unwrap()
        .success());

    let out2 = bin()
        .args(["ingest", "--db"])
        .arg(&db)
        .arg(tmp.path().join("b"))
        .args([
            "--pattern",
            "*.md",
            "--force-merge",
            "-q",
            "--no-input",
            "--llm-backend",
            "none",
        ])
        .output()
        .expect("ingest force-merge");
    assert!(
        out2.status.success(),
        "force-merge ingest: {}",
        String::from_utf8_lossy(&out2.stderr)
    );

    let body = bin()
        .args(["read", "--db"])
        .arg(&db)
        .args(["--name", "shared-topic", "--format", "raw", "-q"])
        .output()
        .expect("read");
    assert!(body.status.success());
    let text = String::from_utf8_lossy(&body.stdout);
    assert!(
        text.contains("body-beta-updated"),
        "with --force-merge the body must update; got: {text:?}"
    );
}
